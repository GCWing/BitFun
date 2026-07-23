use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use tauri::Manager;

use super::{ensure_session, get_session};
use crate::executor::BridgeExecutor;
use crate::server::response::{WebDriverErrorResponse, WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct UrlRequest {
    url: String,
}

pub async fn get_url(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    let webview = state
        .app
        .get_webview(&session.current_window)
        .ok_or_else(|| {
            WebDriverErrorResponse::no_such_window(format!(
                "Webview not found: {}",
                session.current_window
            ))
        })?;

    let url = webview.url().map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Failed to read URL: {error}"))
    })?;

    Ok(WebDriverResponse::success(url.to_string()))
}

pub async fn navigate(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<UrlRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        session.frame_context.clear();
        session.action_state = Default::default();
    }

    let executor = BridgeExecutor::from_session_id(state, &session_id).await?;
    executor.navigate_to(&request.url).await?;
    executor.wait_for_page_load().await?;
    Ok(WebDriverResponse::null())
}

pub async fn back(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        session.frame_context.clear();
        session.action_state = Default::default();
    }

    let executor = BridgeExecutor::from_session_id(state, &session_id).await?;
    executor.go_back().await?;
    executor.wait_for_page_load().await?;
    Ok(WebDriverResponse::null())
}

pub async fn forward(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        session.frame_context.clear();
        session.action_state = Default::default();
    }

    let executor = BridgeExecutor::from_session_id(state, &session_id).await?;
    executor.go_forward().await?;
    executor.wait_for_page_load().await?;
    Ok(WebDriverResponse::null())
}

pub async fn refresh(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        session.frame_context.clear();
        session.action_state = Default::default();
    }

    let executor = BridgeExecutor::from_session_id(state, &session_id).await?;
    executor.refresh_page().await?;
    executor.wait_for_page_load().await?;
    Ok(WebDriverResponse::null())
}

pub async fn get_title(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let title = BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .get_title()
        .await?;
    Ok(WebDriverResponse::success(title))
}

pub async fn get_source(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let source = BridgeExecutor::from_session_id(state, &session_id)
        .await?
        .get_source()
        .await?;
    Ok(WebDriverResponse::success(source))
}
