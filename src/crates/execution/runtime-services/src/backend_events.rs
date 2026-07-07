use std::sync::Arc;

use anyhow::Result;
use bitfun_events::{
    BackgroundCommandLifecycleInfo, EventEmitter, ToolExecutionProgressInfo, ToolTerminalReadyInfo,
};
use log::{error, trace, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum BackendEvent {
    ToolExecutionProgress(ToolExecutionProgressInfo),
    ToolTerminalReady(ToolTerminalReadyInfo),
    BackgroundCommandLifecycle(BackgroundCommandLifecycleInfo),
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

impl BackendEvent {
    pub fn event_name(&self) -> String {
        match self {
            Self::Custom { event_name, .. } => event_name.clone(),
            Self::ToolExecutionProgress(_) => "backend-event-toolexecutionprogress".to_string(),
            Self::ToolTerminalReady(_) => "backend-event-toolterminalready".to_string(),
            Self::BackgroundCommandLifecycle(_) => {
                "backend-event-backgroundcommandlifecycle".to_string()
            }
            Self::ToolAwaitingUserInput { .. } => "backend-event-toolawaitinguserinput".to_string(),
        }
    }

    pub fn payload(&self) -> Result<serde_json::Value, serde_json::Error> {
        match self {
            Self::Custom { payload, .. } => Ok(payload.clone()),
            _ => serde_json::to_value(self),
        }
    }
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

        let emitter = { self.emitter.lock().await.clone() };
        if let Some(emitter) = emitter {
            let event_name = event.event_name();
            let event_data = match event.payload() {
                Ok(value) => value,
                Err(error) => {
                    error!("Failed to serialize event: {}", error);
                    return Ok(());
                }
            };

            if let Err(error) = emitter.emit(&event_name, event_data).await {
                warn!("Failed to emit to frontend: {}", error);
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

#[cfg(test)]
mod tests {
    use super::{BackendEvent, BackendEventSystem};
    use async_trait::async_trait;
    use bitfun_events::{
        BackgroundCommandLifecycleInfo, EventEmitter, ToolExecutionProgressInfo,
        ToolTerminalReadyInfo,
    };
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingEmitter {
        events: Mutex<Vec<(String, serde_json::Value)>>,
    }

    #[async_trait]
    impl EventEmitter for RecordingEmitter {
        async fn emit(&self, event_name: &str, payload: serde_json::Value) -> anyhow::Result<()> {
            self.events
                .lock()
                .expect("events should be recorded")
                .push((event_name.to_string(), payload));
            Ok(())
        }
    }

    #[test]
    fn backend_event_names_remain_stable() {
        assert_eq!(
            BackendEvent::ToolExecutionProgress(progress()).event_name(),
            "backend-event-toolexecutionprogress"
        );
        assert_eq!(
            BackendEvent::ToolTerminalReady(terminal_ready()).event_name(),
            "backend-event-toolterminalready"
        );
        assert_eq!(
            BackendEvent::BackgroundCommandLifecycle(background_lifecycle()).event_name(),
            "backend-event-backgroundcommandlifecycle"
        );
        assert_eq!(
            BackendEvent::ToolAwaitingUserInput {
                tool_id: "tool-1".to_string(),
                session_id: "session-1".to_string(),
                questions: json!([]),
            }
            .event_name(),
            "backend-event-toolawaitinguserinput"
        );
        assert_eq!(
            BackendEvent::Custom {
                event_name: "custom-event".to_string(),
                payload: json!({"ok": true}),
            }
            .event_name(),
            "custom-event"
        );
    }

    #[test]
    fn custom_backend_event_payload_is_emitted_without_wrapper() {
        let payload = BackendEvent::Custom {
            event_name: "custom-event".to_string(),
            payload: json!({"ok": true}),
        }
        .payload()
        .expect("custom event payload should serialize");

        assert_eq!(payload, json!({"ok": true}));
    }

    #[tokio::test]
    async fn backend_event_system_delivers_event_name_and_payload() {
        let emitter = Arc::new(RecordingEmitter::default());
        let system = BackendEventSystem::new();
        system.set_emitter(emitter.clone()).await;

        system
            .emit(BackendEvent::ToolExecutionProgress(progress()))
            .await
            .expect("event should be emitted");

        let events = emitter.events.lock().expect("events should be recorded");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "backend-event-toolexecutionprogress");
        assert_eq!(events[0].1["type"], "ToolExecutionProgress");
        assert_eq!(events[0].1["value"]["tool_use_id"], "tool-1");
    }

    fn progress() -> ToolExecutionProgressInfo {
        ToolExecutionProgressInfo {
            tool_use_id: "tool-1".to_string(),
            tool_name: "Bash".to_string(),
            progress_message: "running".to_string(),
            percentage: Some(50.0),
            timestamp: 1,
        }
    }

    fn terminal_ready() -> ToolTerminalReadyInfo {
        ToolTerminalReadyInfo {
            tool_use_id: "tool-1".to_string(),
            terminal_session_id: "terminal-1".to_string(),
            timestamp: 1,
        }
    }

    fn background_lifecycle() -> BackgroundCommandLifecycleInfo {
        BackgroundCommandLifecycleInfo {
            agent_session_id: Some("session-1".to_string()),
            exec_session_id: 1,
            command: "echo ok".to_string(),
            workdir: Some("/workspace".to_string()),
            remote: false,
            tty: false,
            status: "started".to_string(),
            exit_code: None,
            started_at: 1,
            ended_at: None,
            timestamp: 1,
        }
    }
}
