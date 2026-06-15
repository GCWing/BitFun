use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use super::get_session;
use crate::executor::BridgeExecutor;
use crate::server::response::{WebDriverErrorResponse, WebDriverResponse, WebDriverResult};
use crate::server::AppState;
use crate::webdriver::FrameId;

#[derive(Debug, Deserialize)]
pub struct SwitchFrameRequest {
    id: Value,
}

pub async fn switch_to_frame(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<SwitchFrameRequest>,
) -> WebDriverResult {
    match request.id {
        Value::Null => {
            let mut sessions = state.sessions.write().await;
            let session = sessions.get_mut(&session_id)?;
            session.frame_context.clear();
            Ok(WebDriverResponse::null())
        }
        Value::Number(number) => {
            let index = number.as_u64().ok_or_else(|| {
                WebDriverErrorResponse::invalid_argument(
                    "Frame index must be a non-negative integer",
                )
            })?;
            let index = u32::try_from(index)
                .map_err(|_| WebDriverErrorResponse::invalid_argument("Frame index too large"))?;

            BridgeExecutor::from_session_id(state.clone(), &session_id)
                .await?
                .validate_frame_index(index)
                .await
                .map_err(|_| WebDriverErrorResponse::no_such_frame("Unable to locate frame"))?;

            let mut sessions = state.sessions.write().await;
            let session = sessions.get_mut(&session_id)?;
            session.frame_context.push(FrameId::Index(index));
            Ok(WebDriverResponse::null())
        }
        Value::Object(obj) => {
            let element_id = obj
                .get("element-6066-11e4-a52e-4f735466cecf")
                .or_else(|| obj.get("ELEMENT"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    WebDriverErrorResponse::invalid_argument(
                        "Frame reference must be null, an index, or an element reference",
                    )
                })?
                .to_string();

            BridgeExecutor::from_session_id(state.clone(), &session_id)
                .await?
                .validate_frame_element(&element_id)
                .await
                .map_err(|_| WebDriverErrorResponse::no_such_frame("Unable to locate frame"))?;

            let mut sessions = state.sessions.write().await;
            let session = sessions.get_mut(&session_id)?;
            session.frame_context.push(FrameId::Element(element_id));
            Ok(WebDriverResponse::null())
        }
        _ => Err(WebDriverErrorResponse::invalid_argument(
            "Frame reference must be null, an index, or an element reference",
        )),
    }
}

pub async fn switch_to_parent_frame(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    let _session = get_session(&state, &session_id).await?;
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id)?;
    session.frame_context.pop();
    Ok(WebDriverResponse::null())
}
