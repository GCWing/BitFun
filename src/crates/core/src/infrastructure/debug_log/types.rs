//! Debug Log Ingest Server
//!
//! A lightweight HTTP server that receives debug logs from web applications
//! and writes them to the local NDJSON log file.

use super::{DebugLogConfig, DebugLogEntry};
use crate::infrastructure::get_workspace_path;
use anyhow::Result;
use log::debug;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

pub const DEFAULT_INGEST_PORT: u16 = 7242;

#[derive(Debug, Clone)]
pub struct IngestServerConfig {
    pub port: u16,
    pub log_config: DebugLogConfig,
}

impl Default for IngestServerConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_INGEST_PORT,
            log_config: DebugLogConfig::default(),
        }
    }
}

impl IngestServerConfig {
    pub fn from_debug_mode_config(port: u16, log_path: PathBuf) -> Self {
        Self {
            port,
            log_config: DebugLogConfig {
                log_path,
                ingest_url: None,
                session_id: "debug-session".to_string(),
            },
        }
    }
}

#[derive(Clone)]
pub struct IngestServerState {
    pub config: Arc<RwLock<IngestServerConfig>>,
    pub is_running: Arc<RwLock<bool>>,
}

impl IngestServerState {
    pub fn new(config: IngestServerConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn update_log_path(&self, log_path: PathBuf) {
        let mut config = self.config.write().await;
        config.log_config.log_path = log_path;
        debug!(
            "Debug log path updated to: {:?}",
            config.log_config.log_path
        );
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestLogRequest {
    pub location: String,
    pub message: String,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub hypothesis_id: Option<String>,
    #[serde(default)]
    pub timestamp: Option<i64>,
}

impl From<IngestLogRequest> for DebugLogEntry {
    fn from(req: IngestLogRequest) -> Self {
        DebugLogEntry {
            location: req.location,
            message: req.message,
            data: req.data,
            session_id: req.session_id.unwrap_or_default(),
            run_id: req.run_id,
            hypothesis_id: req.hypothesis_id,
            timestamp: req.timestamp,
            id: None,
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct IngestResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn handle_ingest(
    request: IngestLogRequest,
    config: &DebugLogConfig,
) -> Result<IngestResponse> {
    let log_config = if let Some(workspace_path) = get_workspace_path() {
        let mut cfg = config.clone();
        cfg.log_path = workspace_path.join(".bitfun").join("debug.log");
        cfg
    } else {
        config.clone()
    };

    let entry: DebugLogEntry = request.into();

    use super::append_log_async;
    append_log_async(entry, Some(log_config), false).await?;

    Ok(IngestResponse {
        success: true,
        error: None,
    })
}
