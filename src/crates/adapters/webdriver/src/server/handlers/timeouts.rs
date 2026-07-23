use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use super::get_session;
use crate::server::response::{WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct TimeoutsRequest {
    implicit: Option<u64>,
    #[serde(rename = "pageLoad")]
    page_load: Option<u64>,
    script: Option<u64>,
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let session = get_session(&state, &session_id).await?;
    Ok(WebDriverResponse::success(session.timeouts))
}

pub async fn set(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<TimeoutsRequest>,
) -> WebDriverResult {
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id)?;

    if let Some(implicit) = request.implicit {
        session.timeouts.implicit = implicit;
    }
    if let Some(page_load) = request.page_load {
        session.timeouts.page_load = page_load;
    }
    if let Some(script) = request.script {
        session.timeouts.script = script;
    }

    Ok(WebDriverResponse::null())
}
