//! Relay server configuration.

use std::net::SocketAddr;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RelayConfig {
    pub listen_addr: SocketAddr,
    pub room_ttl_secs: u64,
    pub heartbeat_interval_secs: u64,
    pub heartbeat_timeout_secs: u64,
    pub static_dir: Option<String>,
    /// Directory where per-room uploaded mobile-web files are stored.
    pub room_web_dir: String,
    pub cors_allow_origins: Vec<String>,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            listen_addr: ([0, 0, 0, 0], 9700).into(),
            room_ttl_secs: 3600,
            heartbeat_interval_secs: 30,
            heartbeat_timeout_secs: 90,
            static_dir: None,
            room_web_dir: "/tmp/bitfun-room-web".to_string(),
            cors_allow_origins: vec!["*".to_string()],
        }
    }
}

impl RelayConfig {
    pub fn from_env() -> Self {
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
        if let Ok(ttl) = std::env::var("RELAY_ROOM_TTL") {
            if let Ok(t) = ttl.parse() {
                cfg.room_ttl_secs = t;
            }
        }
        cfg
    }
}
