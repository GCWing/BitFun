use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::json;

use super::ensure_session;
use crate::executor::BridgeExecutor;
use crate::server::response::{WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct GetLogsRequest {
    #[serde(rename = "type")]
    log_type: String,
}

pub async fn get_types(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    Ok(WebDriverResponse::success(json!(["browser"])))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<GetLogsRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    if request.log_type != "browser" {
        return Ok(WebDriverResponse::success(json!([])));
    }

    let result = BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .take_logs()
        .await?;
    Ok(WebDriverResponse::success(result))
}
