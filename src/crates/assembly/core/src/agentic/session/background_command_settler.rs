//! Settles sessions back to `Idle` when a background ExecCommand child process
//! that pinned the session to `Processing` exits.
//!
//! R-WF-25: the turn-completion path keeps the session `Processing` while a
//! background command is still running (keep_processing_turns marker). This
//! subscriber listens for the mirrored `BackgroundCommandLifecycleChanged`
//! agentic events and, once no `Running` background command remains for the
//! session, transitions it back to `Idle` and clears the marker. The watchdog
//! spawned at pin time is the fallback if a lifecycle event is missed.

use super::SessionManager;
use crate::agentic::core::SessionState;
use crate::agentic::events::{AgenticEvent, EventSubscriber};
use bitfun_agent_runtime::event_bus::EventSubscriberResult;
use log::{debug, warn};
use std::sync::Arc;

/// Settles a keep-processing session back to `Idle` after its background
/// command exits.
pub struct BackgroundCommandSettlerSubscriber {
    session_manager: Arc<SessionManager>,
}

impl BackgroundCommandSettlerSubscriber {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }
}

#[async_trait::async_trait]
impl EventSubscriber for BackgroundCommandSettlerSubscriber {
    async fn on_event(&self, event: &AgenticEvent) -> EventSubscriberResult {
        let AgenticEvent::BackgroundCommandLifecycleChanged { session_id, status } = event else {
            return Ok(());
        };
        if status == "running" {
            return Ok(());
        }

        let Some(turn_id) = self.session_manager.keep_processing_turn(session_id) else {
            return Ok(());
        };

        // Double-check the registry: only settle when no Running command
        // remains for the session (another child could still be alive).
        let response = tool_runtime::background_command_output::background_command_output_capture()
            .list(
                tool_runtime::background_command_output::ListBackgroundCommandOutputRequest {
                    agent_session_id: Some(session_id.clone()),
                },
            )
            .await;
        if response
            .activities
            .iter()
            .any(|metadata| metadata.status == tool_runtime::background_command_output::BackgroundCommandOutputStatus::Running)
        {
            debug!(
                "Background command lifecycle settled but another command still running; keeping Processing: session_id={}",
                session_id
            );
            return Ok(());
        }

        debug!(
            "Background command settled; transitioning session back to Idle: session_id={}, turn_id={}",
            session_id, turn_id
        );
        if let Err(error) = self
            .session_manager
            .update_session_state_for_turn_if_processing(session_id, &turn_id, SessionState::Idle)
            .await
        {
            warn!(
                "Failed to settle session to Idle after background command exit: session_id={}, error={}",
                session_id, error
            );
        }
        self.session_manager.clear_keep_processing_turn(session_id);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::core::{ProcessingPhase, SessionConfig};
    use crate::agentic::events::AgenticEvent;
    use crate::agentic::persistence::PersistenceManager;
    use crate::agentic::session::session_manager::SessionManagerConfig;
    use crate::agentic::session::{PromptCachePolicy, SessionContextStore};
    use crate::infrastructure::PathManager;
    use uuid::Uuid;

    fn test_manager() -> Arc<SessionManager> {
        let root = std::env::temp_dir().join(format!("bitfun-settler-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create test root");
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            root.join("user-root"),
        ));
        let persistence_manager =
            Arc::new(PersistenceManager::new(path_manager).expect("persistence manager"));
        Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            persistence_manager,
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: std::time::Duration::from_secs(3600),
                auto_save_interval: std::time::Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ))
    }

    async fn processing_session_with_marker(
        manager: &SessionManager,
        session_id: &str,
        turn_id: &str,
    ) {
        let workspace = std::env::temp_dir().join(format!("bitfun-settler-ws-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&workspace).expect("create workspace dir");
        manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "settler test".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("create session");
        // Force the session into Processing for the expected turn using the
        // public state API (the raw `sessions` map is private to the manager).
        manager
            .update_session_state(
                session_id,
                SessionState::Processing {
                    current_turn_id: turn_id.to_string(),
                    phase: ProcessingPhase::ToolCalling,
                },
            )
            .await
            .expect("set processing state");
        manager.set_keep_processing_turn(session_id, turn_id);
    }

    #[tokio::test]
    async fn running_status_is_ignored() {
        let manager = test_manager();
        let session_id = format!("session-run-{}", Uuid::new_v4());
        processing_session_with_marker(&manager, &session_id, "turn-1").await;
        let subscriber = BackgroundCommandSettlerSubscriber::new(manager.clone());
        let event = AgenticEvent::BackgroundCommandLifecycleChanged {
            session_id: session_id.clone(),
            status: "running".to_string(),
        };
        subscriber.on_event(&event).await.expect("no error");

        let session = manager.get_session(&session_id).expect("session");
        assert!(matches!(
            session.state,
            SessionState::Processing { ref current_turn_id, .. }
                if current_turn_id == "turn-1"
        ));
        assert_eq!(
            manager.keep_processing_turn(&session_id),
            Some("turn-1".to_string())
        );
    }

    #[tokio::test]
    async fn terminal_status_settles_to_idle_and_clears_marker() {
        // R-WF-25 assertion 2 (event-track full chain): a terminal lifecycle
        // event with no Running command left in the registry settles the
        // session back to Idle and clears the keep-processing marker.
        let manager = test_manager();
        let session_id = format!("session-settle-{}", Uuid::new_v4());
        processing_session_with_marker(&manager, &session_id, "turn-1").await;
        let subscriber = BackgroundCommandSettlerSubscriber::new(manager.clone());

        // Capture registry: start + finish (no Running remains).
        use tool_runtime::background_command_output::{
            background_command_output_capture, BackgroundCommandOutputStatus,
            StartBackgroundCommandOutputCapture,
        };
        let capture_id = format!(
            "settler-capture-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        );
        let capture = background_command_output_capture();
        let _tx = capture
            .start_capture(StartBackgroundCommandOutputCapture {
                capture_id: capture_id.clone(),
                agent_session_id: Some(session_id.clone()),
                command: "echo hi".to_string(),
                workdir: None,
                remote: false,
                tty: false,
            })
            .await;
        capture
            .update_lifecycle(
                &capture_id,
                5555,
                BackgroundCommandOutputStatus::Exited,
                Some(0),
            )
            .await
            .expect("record exists");

        let event = AgenticEvent::BackgroundCommandLifecycleChanged {
            session_id: session_id.clone(),
            status: "exited".to_string(),
        };
        subscriber.on_event(&event).await.expect("no error");

        let session = manager.get_session(&session_id).expect("session");
        assert!(matches!(session.state, SessionState::Idle));
        assert_eq!(manager.keep_processing_turn(&session_id), None);
    }

    #[tokio::test]
    async fn terminal_status_keeps_processing_when_another_command_still_running() {
        // R-WF-25 assertion 2 branch: if another Running command remains for
        // the session, the settle must NOT happen yet.
        let manager = test_manager();
        let session_id = format!("session-hold-{}", Uuid::new_v4());
        processing_session_with_marker(&manager, &session_id, "turn-1").await;
        let subscriber = BackgroundCommandSettlerSubscriber::new(manager.clone());

        use tool_runtime::background_command_output::{
            background_command_output_capture, BackgroundCommandOutputStatus,
            StartBackgroundCommandOutputCapture,
        };
        let capture_id = format!(
            "settler-running-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        );
        let capture = background_command_output_capture();
        let _tx = capture
            .start_capture(StartBackgroundCommandOutputCapture {
                capture_id: capture_id.clone(),
                agent_session_id: Some(session_id.clone()),
                command: "sleep 30".to_string(),
                workdir: None,
                remote: false,
                tty: false,
            })
            .await;
        capture
            .update_lifecycle(
                &capture_id,
                5556,
                BackgroundCommandOutputStatus::Running,
                None,
            )
            .await
            .expect("record exists");

        let event = AgenticEvent::BackgroundCommandLifecycleChanged {
            session_id: session_id.clone(),
            status: "exited".to_string(),
        };
        subscriber.on_event(&event).await.expect("no error");

        let session = manager.get_session(&session_id).expect("session");
        assert!(matches!(
            session.state,
            SessionState::Processing { ref current_turn_id, .. }
                if current_turn_id == "turn-1"
        ));
        assert_eq!(
            manager.keep_processing_turn(&session_id),
            Some("turn-1".to_string())
        );
    }

    #[tokio::test]
    async fn no_marker_means_noop() {
        let manager = test_manager();
        let subscriber = BackgroundCommandSettlerSubscriber::new(manager.clone());
        let event = AgenticEvent::BackgroundCommandLifecycleChanged {
            session_id: "session-unknown".to_string(),
            status: "exited".to_string(),
        };
        subscriber.on_event(&event).await.expect("no error");
        // No panic, no state change needed (marker absent).
    }
}
