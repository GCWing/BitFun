//! Debug Log HTTP Ingest Server
//!
//! HTTP server that receives debug logs from web applications.
//! This is platform-agnostic and can be started by any application (desktop, CLI, etc.).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use log::{debug, error, info, trace, warn};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};

use super::types::{
    handle_ingest, IngestLogRequest, IngestResponse, IngestServerConfig, IngestServerState,
    DEFAULT_INGEST_PORT,
};

static GLOBAL_INGEST_MANAGER: OnceLock<Arc<IngestServerManager>> = OnceLock::new();

pub struct IngestServerManager {
    state: Arc<RwLock<Option<IngestServerState>>>,
    cancel_token: Arc<RwLock<Option<CancellationToken>>>,
    actual_port: Arc<RwLock<u16>>,
}

impl IngestServerManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(None)),
            cancel_token: Arc::new(RwLock::new(None)),
            actual_port: Arc::new(RwLock::new(DEFAULT_INGEST_PORT)),
        }
    }

    pub fn global() -> &'static Arc<IngestServerManager> {
        GLOBAL_INGEST_MANAGER.get_or_init(|| Arc::new(IngestServerManager::new()))
    }

    pub async fn start(&self, config: Option<IngestServerConfig>) -> anyhow::Result<()> {
        self.stop().await;

        let cfg = config.unwrap_or_default();
        let base_port = cfg.port;

        let mut listener: Option<tokio::net::TcpListener> = None;
        let mut actual_port = base_port;

        for offset in 0..10u16 {
            let port = base_port + offset;
            if let Some(l) = try_bind_port(port).await {
                listener = Some(l);
                actual_port = port;
                if offset > 0 {
                    info!(
                        "Default port {} is occupied, using port {} instead",
                        base_port, port
                    );
                }
                break;
            }
        }

        let listener = match listener {
            Some(l) => l,
            None => {
                warn!("Debug Log Ingest Server: No available port found in range {}-{}. Server disabled.", 
                    base_port, base_port + 9);
                return Ok(());
            }
        };

        let mut updated_cfg = cfg;
        updated_cfg.port = actual_port;

        let state = IngestServerState::new(updated_cfg);
        let cancel_token = CancellationToken::new();

        *self.state.write().await = Some(state.clone());
        *self.cancel_token.write().await = Some(cancel_token.clone());
        *self.actual_port.write().await = actual_port;

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = Router::new()
            .route("/health", get(health_handler))
            .route("/ingest/:session_id", post(ingest_handler))
            .layer(cors)
            .with_state(state.clone());

        *state.is_running.write().await = true;

        let addr = listener.local_addr()?;
        info!("Debug Log Ingest Server started on http://{}", addr);
        info!("Debug logs will be written to: <workspace>/.bitfun/debug.log");

        let state_clone = state.clone();
        tokio::spawn(async move {
            let server = axum::serve(listener, app);

            tokio::select! {
                result = server => {
                    if let Err(e) = result {
                        error!("Debug Log Ingest Server error: {}", e);
                    }
                }
                _ = cancel_token.cancelled() => {
                    info!("Debug Log Ingest Server shutting down");
                }
            }

            *state_clone.is_running.write().await = false;
        });

        Ok(())
    }

    pub async fn stop(&self) {
        if let Some(token) = self.cancel_token.write().await.take() {
            token.cancel();
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            info!("Debug Log Ingest Server stopped");
        }
        *self.state.write().await = None;
    }

    pub async fn restart(&self, config: IngestServerConfig) -> anyhow::Result<()> {
        debug!(
            "Restarting Debug Log Ingest Server with new config (port: {}, log_path: {:?})",
            config.port, config.log_config.log_path
        );
        self.stop().await;
        self.start(Some(config)).await
    }

    pub async fn update_log_path(&self, log_path: PathBuf) {
        if let Some(state) = self.state.read().await.as_ref() {
            state.update_log_path(log_path).await;
        }
    }

    pub async fn update_port(&self, new_port: u16, log_path: PathBuf) -> anyhow::Result<()> {
        let current_port = *self.actual_port.read().await;
        if current_port != new_port {
            let config = IngestServerConfig::from_debug_mode_config(new_port, log_path);
            self.restart(config).await
        } else {
            self.update_log_path(log_path).await;
            Ok(())
        }
    }

    pub async fn get_actual_port(&self) -> u16 {
        *self.actual_port.read().await
    }

    pub async fn is_running(&self) -> bool {
        if let Some(state) = self.state.read().await.as_ref() {
            *state.is_running.read().await
        } else {
            false
        }
    }
}

async fn try_bind_port(port: u16) -> Option<tokio::net::TcpListener> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tokio::net::TcpListener::bind(addr).await.ok()
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "debug-log-ingest",
        "port": DEFAULT_INGEST_PORT
    }))
}

async fn ingest_handler(
    State(state): State<IngestServerState>,
    Path(session_id): Path<String>,
    Json(mut request): Json<IngestLogRequest>,
) -> Result<Json<IngestResponse>, (StatusCode, Json<IngestResponse>)> {
    if request.session_id.is_none() {
        request.session_id = Some(session_id);
    }

    let config = state.config.read().await;
    let log_config = config.log_config.clone();
    drop(config);

    match handle_ingest(request.clone(), &log_config).await {
        Ok(response) => {
            trace!(
                "Debug log received: [{}] {} | hypothesis: {:?}",
                request.location,
                request.message,
                request.hypothesis_id
            );
            Ok(Json(response))
        }
        Err(e) => {
            warn!("Failed to ingest log: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(IngestResponse {
                    success: false,
                    error: Some(e.to_string()),
                }),
            ))
        }
    }
}
