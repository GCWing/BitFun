//! Core Agent adapter
//!
//! Adapts bitfun-core's Agentic system to CLI's Agent interface.
//! Event consumption is NOT done here — it's done in the chat/exec mode main loops.

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::Agent;
use bitfun_core::agentic::coordination::{
    ConversationCoordinator, DialogSubmissionPolicy, DialogTriggerSource,
};
use bitfun_core::agentic::core::SessionConfig;
use bitfun_core::agentic::events::EventQueue;

/// Core-based Agent implementation.
/// Stateless regarding agent_type — callers pass it per-call.
pub struct CoreAgentAdapter {
    coordinator: Arc<ConversationCoordinator>,
    event_queue: Arc<EventQueue>,
    workspace_path: Arc<Mutex<Option<PathBuf>>>,
    /// Session ID — uses Mutex for interior mutability
    session_id: Arc<Mutex<Option<String>>>,
    /// Current turn ID (for cancellation)
    current_turn_id: Arc<Mutex<Option<String>>>,
}

impl CoreAgentAdapter {
    pub fn new(
        coordinator: Arc<ConversationCoordinator>,
        event_queue: Arc<EventQueue>,
        workspace_path: Option<PathBuf>,
    ) -> Self {
        Self {
            coordinator,
            event_queue,
            workspace_path: Arc::new(Mutex::new(workspace_path)),
            session_id: Arc::new(Mutex::new(None)),
            current_turn_id: Arc::new(Mutex::new(None)),
        }
    }

    /// Get the event queue (for external event consumption)
    pub fn event_queue(&self) -> &Arc<EventQueue> {
        &self.event_queue
    }

    /// Get the coordinator (for advanced operations like list_sessions, get_messages)
    #[allow(dead_code)]
    pub fn coordinator(&self) -> &Arc<ConversationCoordinator> {
        &self.coordinator
    }

    pub fn workspace_path_buf(&self) -> PathBuf {
        self.workspace_path
            .try_lock()
            .ok()
            .and_then(|guard| guard.clone())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn workspace_path_string(&self) -> String {
        self.workspace_path_buf().to_string_lossy().to_string()
    }

    pub async fn set_workspace_path(&self, workspace_path: Option<PathBuf>) {
        let mut guard = self.workspace_path.lock().await;
        *guard = workspace_path;
    }

    fn build_default_session_name() -> String {
        format!(
            "CLI Session - {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        )
    }

    fn is_session_not_found_error(error_msg: &str) -> bool {
        let msg = error_msg.to_lowercase();
        msg.contains("session not found")
            || msg.contains("session does not exist")
            || msg.contains("not found")
    }

    async fn recreate_session_with_id(&self, session_id: &str, agent_type: &str) -> Result<()> {
        let mut session_name = Self::build_default_session_name();
        let mut effective_agent_type = agent_type.to_string();

        let workspace = self.workspace_path_buf();
        if let Ok(sessions) = self.coordinator.list_sessions(&workspace).await {
            if let Some(summary) = sessions.iter().find(|s| s.session_id == session_id) {
                session_name = summary.session_name.clone();
                effective_agent_type = summary.agent_type.clone();
            }
        }

        self.coordinator
            .create_session_with_id(
                Some(session_id.to_string()),
                session_name,
                effective_agent_type,
                SessionConfig {
                    workspace_path: Some(self.workspace_path_string()),
                    ..Default::default()
                },
            )
            .await?;

        tracing::info!("Recreated backend session with existing id: {}", session_id);
        Ok(())
    }

    async fn ensure_backend_session_alive(&self, session_id: &str, agent_type: &str) -> Result<()> {
        if self
            .coordinator
            .get_session_manager()
            .get_session(session_id)
            .is_some()
        {
            return Ok(());
        }

        tracing::warn!(
            "Backend session not present in memory, attempting restore: {}",
            session_id
        );

        let workspace = self.workspace_path_buf();
        match self.coordinator.restore_session(&workspace, session_id).await {
            Ok(_) => {
                tracing::info!("Backend session restored: {}", session_id);
                Ok(())
            }
            Err(restore_err) => {
                tracing::warn!(
                    "Restore failed, recreating backend session: {}, error={}",
                    session_id,
                    restore_err
                );
                self.recreate_session_with_id(session_id, agent_type).await
            }
        }
    }
}

#[async_trait::async_trait]
impl Agent for CoreAgentAdapter {
    async fn ensure_session(&self, agent_type: &str) -> Result<String> {
        let mut session_id_guard = self.session_id.lock().await;

        if let Some(ref id) = *session_id_guard {
            self.ensure_backend_session_alive(id, agent_type).await?;
            return Ok(id.clone());
        }

        let session = self
            .coordinator
            .create_session(
                Self::build_default_session_name(),
                agent_type.to_string(),
                SessionConfig {
                    workspace_path: Some(self.workspace_path_string()),
                    ..Default::default()
                },
            )
            .await?;

        let id = session.session_id.clone();

        *session_id_guard = Some(id.clone());
        tracing::info!("Created core session: {}", id);

        Ok(id)
    }

    async fn send_message(&self, message: String, agent_type: &str) -> Result<String> {
        let session_id = self.ensure_session(agent_type).await?;
        tracing::info!("Sending message to session {}: {}", session_id, message);

        // Generate a turn_id
        let turn_id = uuid::Uuid::new_v4().to_string();

        // Store current turn_id for cancellation
        {
            let mut turn_guard = self.current_turn_id.lock().await;
            *turn_guard = Some(turn_id.clone());
        }

        // Start the dialog turn — this is async, events will arrive via EventQueue
        let start_result = self
            .coordinator
            .start_dialog_turn(
                session_id.clone(),
                message.clone(),
                None,
                Some(turn_id.clone()),
                agent_type.to_string(),
                Some(self.workspace_path_string()),
                DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
                None,
            )
            .await;

        if let Err(err) = start_result {
            if Self::is_session_not_found_error(&err.to_string()) {
                tracing::warn!(
                    "Session missing when starting turn, attempting recovery and retry: session_id={}, error={}",
                    session_id,
                    err
                );
                self.ensure_backend_session_alive(&session_id, agent_type).await?;
                self.coordinator
                    .start_dialog_turn(
                        session_id,
                        message,
                        None,
                        Some(turn_id.clone()),
                        agent_type.to_string(),
                        Some(self.workspace_path_string()),
                        DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
                        None,
                    )
                    .await?;
            } else {
                return Err(err.into());
            }
        }

        Ok(turn_id)
    }

    async fn cancel_current_turn(&self) -> Result<()> {
        let session_id_guard = self.session_id.lock().await;
        let turn_id_guard = self.current_turn_id.lock().await;

        if let (Some(session_id), Some(turn_id)) = (&*session_id_guard, &*turn_id_guard) {
            tracing::info!(
                "Cancelling turn: session={}, turn={}",
                session_id,
                turn_id
            );
            self.coordinator
                .cancel_dialog_turn(session_id, turn_id)
                .await?;
        }

        Ok(())
    }

    async fn create_new_session(&self, agent_type: &str) -> Result<String> {
        let mut session_id_guard = self.session_id.lock().await;

        let session = self
            .coordinator
            .create_session(
                Self::build_default_session_name(),
                agent_type.to_string(),
                SessionConfig {
                    workspace_path: Some(self.workspace_path_string()),
                    ..Default::default()
                },
            )
            .await?;

        let id = session.session_id.clone();

        *session_id_guard = Some(id.clone());
        tracing::info!("Created new core session: {}", id);

        Ok(id)
    }

    async fn restore_session(&self, session_id: &str) -> Result<()> {
        tracing::info!("Restoring session: {}", session_id);
        let workspace = self.workspace_path_buf();
        self.coordinator.restore_session(&workspace, session_id).await?;

        let mut session_id_guard = self.session_id.lock().await;
        *session_id_guard = Some(session_id.to_string());

        Ok(())
    }

    async fn confirm_tool(&self, tool_id: &str, updated_input: Option<serde_json::Value>) -> Result<()> {
        tracing::info!("Confirming tool execution: {}", tool_id);
        self.coordinator
            .confirm_tool(tool_id, updated_input)
            .await
            .map_err(|e| anyhow::anyhow!("Confirm tool failed: {}", e))
    }

    async fn reject_tool(&self, tool_id: &str, reason: String) -> Result<()> {
        tracing::info!("Rejecting tool execution: {}, reason: {}", tool_id, reason);
        self.coordinator
            .reject_tool(tool_id, reason)
            .await
            .map_err(|e| anyhow::anyhow!("Reject tool failed: {}", e))
    }

    async fn submit_user_answers(&self, tool_id: &str, answers: serde_json::Value) -> Result<()> {
        tracing::info!("Submitting user answers for tool: {}", tool_id);
        use bitfun_core::agentic::tools::user_input_manager::get_user_input_manager;
        let manager = get_user_input_manager();
        manager.send_answer(tool_id, answers)
            .map_err(|e| anyhow::anyhow!("Submit user answers failed: {}", e))
    }

    fn session_id(&self) -> Option<String> {
        // Try to get session_id without blocking (best effort for sync context)
        self.session_id.try_lock().ok().and_then(|guard| guard.clone())
    }
}
