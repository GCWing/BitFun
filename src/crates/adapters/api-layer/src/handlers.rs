//! Platform-agnostic business handlers
//!
//! These functions encapsulate all business logic and can be called by different platforms

use crate::dto::*;
use anyhow::Result;
use bitfun_transport::TransportAdapter;
use log::{debug, info};
use std::sync::Arc;

/// Core application state
pub struct CoreAppState {
    pub app_start_time: std::time::Instant,
}

impl CoreAppState {
    pub fn new() -> Self {
        Self {
            app_start_time: std::time::Instant::now(),
        }
    }
}

impl Default for CoreAppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute agent task
pub async fn handle_execute_agent_task(
    _state: &CoreAppState,
    _transport: Arc<dyn TransportAdapter>,
    request: ExecuteAgentRequest,
) -> Result<ExecuteAgentResponse> {
    info!(
        "Executing agent task: agent_type={}, message_length={}",
        request.agent_type,
        request.user_message.len()
    );

    Ok(ExecuteAgentResponse {
        session_id: uuid::Uuid::new_v4().to_string(),
        turn_id: uuid::Uuid::new_v4().to_string(),
        status: "started".to_string(),
        message: Some("Task execution started".to_string()),
    })
}

/// Get session history
pub async fn handle_get_session_history(
    _state: &CoreAppState,
    request: GetSessionHistoryRequest,
) -> Result<SessionHistoryResponse> {
    debug!("Getting session history: session_id={}", request.session_id);

    Ok(SessionHistoryResponse {
        session_id: request.session_id,
        turns: vec![],
    })
}

/// Read file content
pub async fn handle_read_file(
    _state: &CoreAppState,
    request: ReadFileRequest,
) -> Result<ReadFileResponse> {
    let content = std::fs::read_to_string(&request.path)?;

    Ok(ReadFileResponse {
        content,
        total_lines: None,
    })
}

/// Write file content
pub async fn handle_write_file(
    _state: &CoreAppState,
    request: WriteFileRequest,
) -> Result<SuccessResponse> {
    if request.create_dirs.unwrap_or(false) {
        if let Some(parent) = std::path::Path::new(&request.path).parent() {
            std::fs::create_dir_all(parent)?;
        }
    }

    std::fs::write(&request.path, request.content)?;

    Ok(SuccessResponse {
        success: true,
        message: Some(format!("File written: {}", request.path)),
        data: None,
    })
}

/// Health check
pub async fn handle_health_check(state: &CoreAppState) -> Result<HealthResponse> {
    Ok(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: state.app_start_time.elapsed().as_secs(),
        active_sessions: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let state = CoreAppState::new();
        let response = handle_health_check(&state)
            .await
            .expect("health check should always succeed");
        assert_eq!(response.status, "healthy");
    }
}
