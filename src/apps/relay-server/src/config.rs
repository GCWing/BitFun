//! Relay server configuration.

use anyhow::Context;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(super) struct RelayConfig {
    pub listen_addr: SocketAddr,
    pub heartbeat_interval_secs: u64,
    pub heartbeat_timeout_secs: u64,
    pub static_dir: Option<String>,
    /// Directory for published Page assets.
    pub asset_dir: String,
    /// Global capacity for content-addressed Page assets.
    pub asset_store_max_bytes: u64,
    pub cors_allow_origins: Vec<String>,
    /// Browser-visible base URL used for published Page content.
    pub page_public_base_url: Option<String>,
    /// Browser-visible base URL used for trusted Relay Page login.
    pub page_auth_base_url: Option<String>,
    /// Path to the SQLite database file used for account storage.
    /// Required at startup; every device connection is account-authenticated.
    pub db_path: Option<String>,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            listen_addr: ([0, 0, 0, 0], 9700).into(),
            heartbeat_interval_secs: 30,
            heartbeat_timeout_secs: 90,
            static_dir: None,
            // Also stores published OpenBitFun Pages under `{asset_dir}/pages/{user_id}/{slug}/`.
            asset_dir: "/tmp/openbitfun-room-web".to_string(),
            asset_store_max_bytes: openbitfun_relay_service::DEFAULT_DISK_ASSET_STORE_MAX_BYTES,
            cors_allow_origins: Vec::new(),
            page_public_base_url: None,
            page_auth_base_url: None,
            db_path: None,
        }
    }
}

impl RelayConfig {
    pub(super) fn from_env() -> anyhow::Result<Self> {
        let mut cfg = Self::default();
        if let Ok(port) = std::env::var("RELAY_PORT") {
            if let Ok(p) = port.parse::<u16>() {
                cfg.listen_addr = ([0, 0, 0, 0], p).into();
            }
        }
        if let Ok(addr) = std::env::var("RELAY_LISTEN_ADDR") {
            cfg.listen_addr = addr
                .parse()
                .context("RELAY_LISTEN_ADDR must be an IP socket address")?;
        }
        if let Ok(dir) = std::env::var("RELAY_STATIC_DIR") {
            cfg.static_dir = Some(dir);
        }
        // Keep the previous storage location readable during upgrades.
        if let Ok(dir) =
            std::env::var("RELAY_ASSET_DIR").or_else(|_| std::env::var("RELAY_ROOM_WEB_DIR"))
        {
            cfg.asset_dir = dir;
        }
        if let Ok(limit) = std::env::var("RELAY_ASSET_STORE_MAX_BYTES") {
            if let Ok(bytes) = limit.parse::<u64>() {
                cfg.asset_store_max_bytes = bytes;
            }
        }
        if let Ok(path) = std::env::var("RELAY_DB_PATH") {
            if path.is_empty() {
                cfg.db_path = None;
            } else {
                cfg.db_path = Some(path);
            }
        }
        if let Ok(origins) = std::env::var("RELAY_CORS_ALLOW_ORIGINS") {
            cfg.cors_allow_origins = origins
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_string)
                .collect();
        }
        cfg.page_public_base_url = std::env::var("RELAY_PAGE_PUBLIC_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty());
        cfg.page_auth_base_url = std::env::var("RELAY_PAGE_AUTH_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty());
        Ok(cfg)
    }
}
