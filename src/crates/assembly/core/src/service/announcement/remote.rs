//! Remote announcement fetcher compatibility wrapper.
//!
//! HTTP fetch, cache TTL, and cache hydration live in
//! `bitfun-services-integrations`; core only supplies product configuration.

use crate::infrastructure::app_paths::PathManager;
use bitfun_services_integrations::announcement::{
    AnnouncementRemoteFetchRequest, RemoteAnnouncementFetcher,
};
use std::sync::Arc;

/// Cloneable handle so it can be moved into background tasks.
#[derive(Clone)]
pub struct RemoteFetcher {
    inner: RemoteAnnouncementFetcher,
}

impl RemoteFetcher {
    pub fn new(path_manager: &Arc<PathManager>) -> Self {
        let cache_file = path_manager
            .user_config_dir()
            .join("announcement-remote-cache.json");
        Self {
            inner: RemoteAnnouncementFetcher::new(cache_file),
        }
    }

    /// Return the in-memory cached cards, populated from disk on first access.
    pub async fn cached_cards(&self) -> Vec<super::types::AnnouncementCard> {
        self.inner.cached_cards().await
    }

    /// Fetch from the remote endpoint when the services-owned cache is stale.
    pub async fn fetch_if_stale(&self) {
        self.inner.fetch_if_stale(Self::fetch_request().await).await;
    }

    async fn fetch_request() -> Option<AnnouncementRemoteFetchRequest> {
        let endpoint_url = Self::remote_url().await?;
        if endpoint_url.is_empty() {
            return None;
        }

        Some(AnnouncementRemoteFetchRequest {
            endpoint_url,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            locale: Self::current_locale().await,
            platform: "desktop".to_string(),
        })
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
