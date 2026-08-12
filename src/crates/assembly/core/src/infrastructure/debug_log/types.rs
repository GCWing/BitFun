//! Debug Log Ingest Server
//!
//! A lightweight HTTP server that receives debug logs from web applications
//! and writes them to the local NDJSON log file.

use super::{DebugLogConfig, DebugLogEntry};
#[cfg(feature = "workspace-runtime")]
use crate::service::workspace::get_global_workspace_service;
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
    let log_config = config.clone();
    #[cfg(feature = "workspace-runtime")]
    let log_config = if let Some(workspace_path) =
        get_global_workspace_service().and_then(|service| service.try_get_current_workspace_path())
    {
        let mut cfg = log_config;
        cfg.log_path = workspace_path.join(".bitfun").join("debug.log");
        cfg
    } else {
        log_config
    };

    let entry: DebugLogEntry = request.into();

    use super::append_log_async;
    append_log_async(entry, Some(log_config), false).await?;

    Ok(IngestResponse {
        success: true,
        error: None,
    })
}

#[cfg(all(test, not(feature = "workspace-runtime")))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn standalone_debug_log_uses_the_configured_path() {
        let temp = tempfile::tempdir().expect("temp directory");
        let log_path = temp.path().join("standalone").join("debug.log");
        let config = DebugLogConfig {
            log_path: log_path.clone(),
            ingest_url: None,
            session_id: "standalone-session".to_string(),
        };

        handle_ingest(
            IngestLogRequest {
                location: "standalone-test".to_string(),
                message: "uses configured path".to_string(),
                data: serde_json::Value::Null,
                session_id: None,
                run_id: None,
                hypothesis_id: None,
                timestamp: Some(1),
            },
            &config,
        )
        .await
        .expect("ingest succeeds without the workspace runtime");

        let contents = std::fs::read_to_string(log_path).expect("configured log is written");
        assert!(contents.contains("\"location\":\"standalone-test\""));
        assert!(contents.contains("\"sessionId\":\"standalone-session\""));
    }
}
