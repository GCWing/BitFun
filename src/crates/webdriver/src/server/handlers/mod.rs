use std::sync::Arc;

use axum::extract::State;
use serde_json::json;

use crate::server::response::{WebDriverErrorResponse, WebDriverResponse};
use crate::server::AppState;
use crate::webdriver::Session;

pub mod actions;
pub mod alert;
pub mod cookie;
pub mod element;
pub mod frame;
pub mod logs;
pub mod navigation;
pub mod print;
pub mod screenshot;
pub mod script;
pub mod session;
pub mod shadow;
pub mod timeouts;
pub mod window;

pub async fn status(State(state): State<Arc<AppState>>) -> WebDriverResponse {
    WebDriverResponse::success(json!({
        "ready": state.initial_window_label().is_some(),
        "message": "BitFun embedded WebDriver is ready",
        "build": {
            "version": env!("CARGO_PKG_VERSION"),
            "name": "bitfun-embedded-webdriver"
        }
    }))
}

pub(crate) async fn get_session(
    state: &Arc<AppState>,
    session_id: &str,
) -> Result<Session, WebDriverErrorResponse> {
    state.sessions.read().await.get_cloned(session_id)
}

pub(crate) async fn ensure_session(
    state: &Arc<AppState>,
    session_id: &str,
) -> Result<(), WebDriverErrorResponse> {
    let _ = get_session(state, session_id).await?;
    Ok(())
}
