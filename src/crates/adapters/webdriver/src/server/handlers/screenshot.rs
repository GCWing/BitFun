use std::sync::Arc;

use axum::extract::{Path, State};

use crate::executor::BridgeExecutor;
use crate::server::response::{WebDriverResponse, WebDriverResult};
use crate::server::AppState;

pub async fn take(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let executor = BridgeExecutor::from_session_id(state, &session_id).await?;
    let screenshot = executor.take_screenshot().await?;
    Ok(WebDriverResponse::success(screenshot))
}

pub async fn take_element(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> WebDriverResult {
    let executor = BridgeExecutor::from_session_id(state, &session_id).await?;
    let screenshot = executor.take_element_screenshot(&element_id).await?;
    Ok(WebDriverResponse::success(screenshot))
}
