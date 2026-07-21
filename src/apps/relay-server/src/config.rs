//! Relay server configuration.

use std::net::SocketAddr;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(super) struct RelayConfig {
    pub listen_addr: SocketAddr,
    pub room_ttl_secs: u64,
    pub heartbeat_interval_secs: u64,
    pub heartbeat_timeout_secs: u64,
    pub static_dir: Option<String>,
    /// Directory where per-room uploaded mobile-web files are stored.
    pub room_web_dir: String,
    /// Global capacity for content-addressed room and Page assets.
    pub asset_store_max_bytes: u64,
    pub cors_allow_origins: Vec<String>,
    /// Path to the SQLite database file used for account storage.
    /// When None, account features are disabled (relay acts as pure relay only).
    pub db_path: Option<String>,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            listen_addr: ([0, 0, 0, 0], 9700).into(),
            room_ttl_secs: 300,
            heartbeat_interval_secs: 30,
            heartbeat_timeout_secs: 90,
            static_dir: None,
            // Also stores published BitFun Pages under `{room_web_dir}/pages/{user_id}/{slug}/`.
            room_web_dir: "/tmp/bitfun-room-web".to_string(),
            asset_store_max_bytes: bitfun_relay_service::DEFAULT_DISK_ASSET_STORE_MAX_BYTES,
            cors_allow_origins: Vec::new(),
            db_path: None,
        }
    }
}

impl RelayConfig {
    pub(super) fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(port) = std::env::var("RELAY_PORT") {
            if let Ok(p) = port.parse::<u16>() {
                cfg.listen_addr = ([0, 0, 0, 0], p).into();
            }
        }
        if let Ok(dir) = std::env::var("RELAY_STATIC_DIR") {
            cfg.static_dir = Some(dir);
        }
        if let Ok(dir) = std::env::var("RELAY_ROOM_WEB_DIR") {
            cfg.room_web_dir = dir;
        }
        if let Ok(limit) = std::env::var("RELAY_ASSET_STORE_MAX_BYTES") {
            if let Ok(bytes) = limit.parse::<u64>() {
                cfg.asset_store_max_bytes = bytes;
            }
        }
        if let Ok(ttl) = std::env::var("RELAY_ROOM_TTL") {
            if let Ok(t) = ttl.parse() {
                cfg.room_ttl_secs = t;
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
        cfg
    }
}
