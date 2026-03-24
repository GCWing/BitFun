use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{ensure_session, run_script};
use crate::server::response::{WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformActionsRequest {
    actions: Vec<ActionSequence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ActionSequence {
    #[serde(rename = "key")]
    Key { id: String, actions: Vec<KeyAction> },
    #[serde(rename = "pointer")]
    Pointer {
        id: String,
        #[serde(default)]
        parameters: Option<Value>,
        actions: Vec<PointerAction>,
    },
    #[serde(rename = "wheel")]
    Wheel {
        id: String,
        actions: Vec<WheelAction>,
    },
    #[serde(rename = "none")]
    None {
        id: String,
        actions: Vec<PauseAction>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum KeyAction {
    #[serde(rename = "keyDown")]
    KeyDown { value: String },
    #[serde(rename = "keyUp")]
    KeyUp { value: String },
    #[serde(rename = "pause")]
    Pause { duration: Option<u64> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PointerAction {
    #[serde(rename = "pointerDown")]
    PointerDown { button: u32 },
    #[serde(rename = "pointerUp")]
    PointerUp { button: u32 },
    #[serde(rename = "pointerMove")]
    PointerMove {
        x: i32,
        y: i32,
        duration: Option<u64>,
        #[serde(default)]
        origin: Option<Value>,
    },
    #[serde(rename = "pause")]
    Pause { duration: Option<u64> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WheelAction {
    #[serde(rename = "scroll")]
    Scroll {
        x: i32,
        y: i32,
        #[serde(rename = "deltaX")]
        delta_x: i32,
        #[serde(rename = "deltaY")]
        delta_y: i32,
        #[serde(default)]
        duration: Option<u64>,
        #[serde(default)]
        origin: Option<Value>,
    },
    #[serde(rename = "pause")]
    Pause { duration: Option<u64> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PauseAction {
    #[serde(rename = "pause")]
    Pause { duration: Option<u64> },
}

fn update_action_state(state: &mut crate::webdriver::ActionState, actions: &[ActionSequence]) {
    for action_sequence in actions {
        match action_sequence {
            ActionSequence::Key { actions, .. } => {
                for action in actions {
                    match action {
                        KeyAction::KeyDown { value } => {
                            state.pressed_keys.insert(value.clone());
                        }
                        KeyAction::KeyUp { value } => {
                            state.pressed_keys.remove(value);
                        }
                        KeyAction::Pause { .. } => {}
                    }
                }
            }
            ActionSequence::Pointer { id, actions, .. } => {
                for action in actions {
                    match action {
                        PointerAction::PointerDown { button } => {
                            state
                                .pressed_buttons
                                .entry(id.clone())
                                .or_default()
                                .insert(*button);
                        }
                        PointerAction::PointerUp { button } => {
                            let mut remove_source = false;
                            if let Some(buttons) = state.pressed_buttons.get_mut(id) {
                                buttons.remove(button);
                                remove_source = buttons.is_empty();
                            }
                            if remove_source {
                                state.pressed_buttons.remove(id);
                            }
                        }
                        PointerAction::PointerMove { .. } | PointerAction::Pause { .. } => {}
                    }
                }
            }
            ActionSequence::Wheel { .. } | ActionSequence::None { .. } => {}
        }
    }
}

pub async fn perform(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<PerformActionsRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let actions_value = serde_json::to_value(&request.actions).unwrap_or(Value::Array(Vec::new()));
    run_script(
        state.clone(),
        &session_id,
        "async (actions) => { await window.__bitfunWd.performActions(actions); return null; }",
        vec![actions_value],
        false,
    )
    .await?;

    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id)?;
    update_action_state(&mut session.action_state, &request.actions);
    Ok(WebDriverResponse::null())
}

pub async fn release(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let (pressed_keys, pressed_buttons) = {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id)?;
        let pressed_keys = session
            .action_state
            .pressed_keys
            .drain()
            .collect::<Vec<_>>();
        let pressed_buttons = std::mem::take(&mut session.action_state.pressed_buttons)
            .into_iter()
            .flat_map(|(source_id, buttons)| {
                buttons
                    .into_iter()
                    .map(move |button| json!({ "sourceId": source_id, "button": button }))
            })
            .collect::<Vec<_>>();
        (pressed_keys, pressed_buttons)
    };

    run_script(
        state,
        &session_id,
        "async (pressedKeys, pressedButtons) => { await window.__bitfunWd.releaseActions(pressedKeys, pressedButtons); return null; }",
        vec![json!(pressed_keys), json!(pressed_buttons)],
        false,
    )
    .await?;
    Ok(WebDriverResponse::null())
}
