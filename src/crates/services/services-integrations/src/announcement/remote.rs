//! Remote announcement fetcher.
//!
//! Product assembly supplies configuration values. This module owns remote HTTP
//! fetch, cache TTL, disk cache hydration, and in-memory cache state.

use crate::announcement::AnnouncementCard;
use log::{debug, info, warn};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

const CACHE_TTL_SECS: i64 = 86_400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncementRemoteFetchRequest {
    pub endpoint_url: String,
    pub app_version: String,
    pub locale: String,
    pub platform: String,
}

impl AnnouncementRemoteFetchRequest {
    pub fn request_url(&self) -> String {
        format!(
            "{}?app_version={}&locale={}&platform={}",
            self.endpoint_url, self.app_version, self.locale, self.platform
        )
    }
}

#[derive(Clone)]
pub struct RemoteAnnouncementFetcher {
    cache_file: PathBuf,
    cached: Arc<RwLock<Vec<AnnouncementCard>>>,
}

impl RemoteAnnouncementFetcher {
    pub fn new(cache_file: impl Into<PathBuf>) -> Self {
        Self {
            cache_file: cache_file.into(),
            cached: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn cached_cards(&self) -> Vec<AnnouncementCard> {
        let cards = self.cached.read().await.clone();
        if !cards.is_empty() {
            return cards;
        }
        self.load_disk_cache().await
    }

    pub async fn fetch_if_stale(&self, request: Option<AnnouncementRemoteFetchRequest>) {
        let Some(request) = request.filter(|request| !request.endpoint_url.is_empty()) else {
            debug!("No remote announcement URL configured; skipping fetch");
            return;
        };

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

        let request_url = request.request_url();
        info!(
            "Fetching remote announcements from {}",
            request.endpoint_url
        );

        match reqwest::get(&request_url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<Vec<AnnouncementCard>>().await {
                    Ok(cards) => {
                        info!("Fetched {} remote announcement cards", cards.len());
                        if let Ok(json) = serde_json::to_string_pretty(&cards) {
                            let _ = fs::write(&self.cache_file, json).await;
                        }
                        *self.cached.write().await = cards;
                    }
                    Err(error) => warn!("Failed to parse remote announcements: {}", error),
                }
            }
            Ok(resp) => warn!(
                "Remote announcement endpoint returned HTTP {}",
                resp.status()
            ),
            Err(error) => warn!("Failed to fetch remote announcements: {}", error),
        }
    }

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
}
