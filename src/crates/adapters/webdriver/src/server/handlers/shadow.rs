use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use super::ensure_session;
use crate::executor::BridgeExecutor;
use crate::server::response::{WebDriverErrorResponse, WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct FindShadowRequest {
    using: String,
    value: String,
}

pub async fn get_shadow_root(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let value = BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .get_shadow_root(&element_id)
        .await
        .map_err(|_| {
            WebDriverErrorResponse::no_such_shadow_root("Element does not have a shadow root")
        })?;

    if value.is_null() {
        return Err(WebDriverErrorResponse::no_such_shadow_root(
            "Element does not have a shadow root",
        ));
    }

    Ok(WebDriverResponse::success(value))
}

pub async fn find_element_in_shadow(
    State(state): State<Arc<AppState>>,
    Path((session_id, shadow_id)): Path<(String, String)>,
    Json(request): Json<FindShadowRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let results = BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .find_elements_from_shadow(&shadow_id, &request.using, &request.value)
        .await?;
    let value = results.into_iter().next().unwrap_or(Value::Null);

    if value.is_null() {
        return Err(WebDriverErrorResponse::no_such_element(
            "No shadow child element matched the selector",
        ));
    }

    Ok(WebDriverResponse::success(value))
}

pub async fn find_elements_in_shadow(
    State(state): State<Arc<AppState>>,
    Path((session_id, shadow_id)): Path<(String, String)>,
    Json(request): Json<FindShadowRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let value = BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .find_elements_from_shadow(&shadow_id, &request.using, &request.value)
        .await?;
    Ok(WebDriverResponse::success(value))
}
