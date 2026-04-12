//! Remote announcement fetcher.
//!
//! Fetches `AnnouncementCard` JSON from an optional remote endpoint and caches
//! the result for 24 hours.  The remote URL is read from the global config
//! under the key `announcements.remote_url`.  When the key is absent or empty
//! the fetch is silently skipped.

use super::types::AnnouncementCard;
use crate::infrastructure::app_paths::PathManager;
use log::{debug, info, warn};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

const CACHE_TTL_SECS: i64 = 86_400; // 24 hours

/// Cloneable handle so it can be moved into background tasks.
#[derive(Clone)]
pub struct RemoteFetcher {
    cache_file: PathBuf,
    cached: Arc<RwLock<Vec<AnnouncementCard>>>,
}

impl RemoteFetcher {
    pub fn new(path_manager: &Arc<PathManager>) -> Self {
        let cache_file = path_manager
            .user_config_dir()
            .join("announcement-remote-cache.json");
        Self {
            cache_file,
            cached: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Return the in-memory cached cards (populated after a successful fetch).
    pub async fn cached_cards(&self) -> Vec<AnnouncementCard> {
        // Try reading from disk on first access if in-memory cache is empty.
        let cards = self.cached.read().await.clone();
        if !cards.is_empty() {
            return cards;
        }
        self.load_disk_cache().await
    }

    /// Fetch from the remote endpoint if the cache is older than `CACHE_TTL_SECS`.
    /// Runs silently: failures are logged at `warn` level and do not propagate.
    pub async fn fetch_if_stale(&self) {
        let url = match Self::remote_url().await {
            Some(u) if !u.is_empty() => u,
            _ => {
                debug!("No remote announcement URL configured; skipping fetch");
                return;
            }
        };

        // Check cache age.
        if let Ok(meta) = fs::metadata(&self.cache_file).await {
            if let Ok(modified) = meta.modified() {
                let age = std::time::SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_default()
                    .as_secs() as i64;
                if age < CACHE_TTL_SECS {
                    debug!("Remote announcement cache is fresh ({} s old)", age);
                    return;
                }
            }
        }

        info!("Fetching remote announcements from {}", url);

        let locale = Self::current_locale().await;
        let version = env!("CARGO_PKG_VERSION");
        let request_url = format!(
            "{}?app_version={}&locale={}&platform=desktop",
            url, version, locale
        );

        match reqwest::get(&request_url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<Vec<AnnouncementCard>>().await {
                    Ok(cards) => {
                        info!("Fetched {} remote announcement cards", cards.len());
                        // Persist to disk.
                        if let Ok(json) = serde_json::to_string_pretty(&cards) {
                            let _ = fs::write(&self.cache_file, json).await;
                        }
                        *self.cached.write().await = cards;
                    }
                    Err(e) => warn!("Failed to parse remote announcements: {}", e),
                }
            }
            Ok(resp) => warn!(
                "Remote announcement endpoint returned HTTP {}",
                resp.status()
            ),
            Err(e) => warn!("Failed to fetch remote announcements: {}", e),
        }
    }

    // ── Private helpers ───────────────────────────────────────────────

    async fn load_disk_cache(&self) -> Vec<AnnouncementCard> {
        match fs::read_to_string(&self.cache_file).await {
            Ok(content) => {
                let cards =
                    serde_json::from_str::<Vec<AnnouncementCard>>(&content).unwrap_or_default();
                let mut lock = self.cached.write().await;
                *lock = cards.clone();
                cards
            }
            Err(_) => Vec::new(),
        }
    }

    async fn remote_url() -> Option<String> {
        use crate::service::config::get_global_config_service;
        match get_global_config_service().await {
            Ok(svc) => svc
                .get_config::<String>(Some("announcements.remote_url"))
                .await
                .ok(),
            Err(_) => None,
        }
    }

    async fn current_locale() -> String {
        use crate::service::config::get_global_config_service;
        get_global_config_service()
            .await
            .ok()
            .and_then(|svc| {
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(svc.get_config::<String>(Some("general.language")))
                        .ok()
                })
            })
            .unwrap_or_else(|| "en-US".to_string())
    }
}
