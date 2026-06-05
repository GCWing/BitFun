use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use super::ensure_session;
use crate::executor::BridgeExecutor;
use crate::server::response::{WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct ExecuteRequest {
    script: String,
    #[serde(default)]
    args: Vec<Value>,
}

pub async fn execute_sync(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<ExecuteRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let result = BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .run_script(&request.script, request.args, false)
        .await?;
    Ok(WebDriverResponse::success(result))
}

pub async fn execute_async(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<ExecuteRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let result = BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .run_script(&request.script, request.args, true)
        .await?;
    Ok(WebDriverResponse::success(result))
}
