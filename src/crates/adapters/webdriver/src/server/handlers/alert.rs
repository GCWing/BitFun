use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use super::ensure_session;
use crate::executor::BridgeExecutor;
use crate::server::response::{WebDriverErrorResponse, WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct SendAlertTextRequest {
    text: String,
}

pub async fn dismiss(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .dismiss_alert()
        .await
        .map_err(|_| WebDriverErrorResponse::no_such_alert("No alert is currently open"))?;
    Ok(WebDriverResponse::null())
}

pub async fn accept(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .accept_alert()
        .await
        .map_err(|_| WebDriverErrorResponse::no_such_alert("No alert is currently open"))?;
    Ok(WebDriverResponse::null())
}

pub async fn get_text(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let text = BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .get_alert_text()
        .await
        .map_err(|_| WebDriverErrorResponse::no_such_alert("No alert is currently open"))?;
    Ok(WebDriverResponse::success(text))
}

pub async fn send_text(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<SendAlertTextRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .send_alert_text(&request.text)
        .await
        .map_err(|error| {
            if error.error == "javascript error" {
                WebDriverErrorResponse::no_such_alert("No prompt is currently open")
            } else {
                error
            }
        })?;
    Ok(WebDriverResponse::null())
}
