use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::server::response::{WebDriverErrorResponse, WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct NewSessionRequest {
    capabilities: Option<Value>,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(request): Json<NewSessionRequest>,
) -> WebDriverResult {
    let Some(initial_window) = state.initial_window_label() else {
        return Err(WebDriverErrorResponse::session_not_created(format!(
            "Webview not available: {}",
            state.preferred_label
        )));
    };

    let session = state.sessions.write().await.create(initial_window.clone());

    Ok(WebDriverResponse::success(json!({
        "sessionId": session.id,
        "capabilities": {
            "browserName": "bitfun",
            "browserVersion": "unknown",
            "platformName": std::env::consts::OS,
            "acceptInsecureCerts": true,
            "pageLoadStrategy": "normal",
            "setWindowRect": true,
            "takesScreenshot": true,
            "printPage": cfg!(any(target_os = "macos", target_os = "windows", target_os = "linux")),
            "timeouts": session.timeouts,
            "bitfun:embedded": true,
            "bitfun:webviewLabel": initial_window,
            "alwaysMatch": request.capabilities.unwrap_or(Value::Null)
        }
    })))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let removed = state.sessions.write().await.delete(&session_id);
    if !removed {
        return Err(WebDriverErrorResponse::invalid_session_id(&session_id));
    }

    Ok(WebDriverResponse::null())
}
