//! Backend event system for tool execution and custom events

use crate::infrastructure::events::EventEmitter;
use crate::util::types::event::ToolExecutionProgressInfo;
use anyhow::Result;
use log::{error, trace, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum BackendEvent {
    ToolExecutionProgress(ToolExecutionProgressInfo),
    ToolAwaitingUserInput {
        tool_id: String,
        session_id: String,
        questions: serde_json::Value,
    },
    Custom {
        event_name: String,
        payload: serde_json::Value,
    },
}

pub struct BackendEventSystem {
    emitter: Arc<Mutex<Option<Arc<dyn EventEmitter>>>>,
}

impl BackendEventSystem {
    pub fn new() -> Self {
        Self {
            emitter: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn set_emitter(&self, emitter: Arc<dyn EventEmitter>) {
        let mut e = self.emitter.lock().await;
        *e = Some(emitter);
    }

    pub async fn emit(&self, event: BackendEvent) -> Result<()> {
        trace!("Emitting event: {:?}", event);

        let emitter_guard = self.emitter.lock().await;
        if let Some(ref emitter) = *emitter_guard {
            let event_name = match &event {
                BackendEvent::Custom { event_name, .. } => event_name.clone(),
                BackendEvent::ToolExecutionProgress(_) => {
                    "backend-event-toolexecutionprogress".to_string()
                }
                BackendEvent::ToolAwaitingUserInput { .. } => {
                    "backend-event-toolawaitinguserinput".to_string()
                }
            };

            let event_data = match &event {
                BackendEvent::Custom { payload, .. } => payload.clone(),
                _ => match serde_json::to_value(&event) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Failed to serialize event: {}", e);
                        return Ok(());
                    }
                },
            };

            if let Err(e) = emitter.emit(&event_name, event_data).await {
                warn!("Failed to emit to frontend: {}", e);
            }
        }

        Ok(())
    }
}

impl Default for BackendEventSystem {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_EVENT_SYSTEM: std::sync::OnceLock<Arc<BackendEventSystem>> =
    std::sync::OnceLock::new();

pub fn get_global_event_system() -> Arc<BackendEventSystem> {
    GLOBAL_EVENT_SYSTEM
        .get_or_init(|| Arc::new(BackendEventSystem::new()))
        .clone()
}

pub async fn emit_global_event(event: BackendEvent) -> Result<()> {
    get_global_event_system().emit(event).await
}
