use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use super::get_session;
use crate::executor::BridgeExecutor;
use crate::platform::Cookie;
use crate::server::response::{WebDriverErrorResponse, WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct AddCookieRequest {
    cookie: Cookie,
}

pub async fn get_all(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let _session = get_session(&state, &session_id).await?;
    let cookies = BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .get_all_cookies()
        .await?;
    Ok(WebDriverResponse::success(cookies))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path((session_id, name)): Path<(String, String)>,
) -> WebDriverResult {
    let _session = get_session(&state, &session_id).await?;
    let cookie = BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .get_cookie(&name)
        .await?;

    let Some(cookie) = cookie else {
        return Err(WebDriverErrorResponse::no_such_cookie(format!(
            "Cookie '{name}' not found"
        )));
    };

    Ok(WebDriverResponse::success(cookie))
}

pub async fn add(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<AddCookieRequest>,
) -> WebDriverResult {
    let _session = get_session(&state, &session_id).await?;
    BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .add_cookie(request.cookie)
        .await?;
    Ok(WebDriverResponse::null())
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path((session_id, name)): Path<(String, String)>,
) -> WebDriverResult {
    let _session = get_session(&state, &session_id).await?;
    BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .delete_cookie(&name)
        .await?;
    Ok(WebDriverResponse::null())
}

pub async fn delete_all(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let _session = get_session(&state, &session_id).await?;
    BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .delete_all_cookies()
        .await?;
    Ok(WebDriverResponse::null())
}
