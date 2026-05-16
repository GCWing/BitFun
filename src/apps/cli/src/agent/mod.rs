/// Agent integration module
///
/// Wraps interaction with bitfun-core's Agentic system.
/// The Agent trait provides a thin adapter over ConversationCoordinator.
/// Event consumption is done externally (in the chat/exec mode main loops).

pub mod agentic_system;
pub mod core_adapter;

use anyhow::Result;

/// Agent interface — thin wrapper over core's ConversationCoordinator.
/// Agent is stateless regarding agent_type; callers pass it per-call.
#[allow(dead_code)]
#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    /// Ensure a core session exists, return session_id
    async fn ensure_session(&self, agent_type: &str) -> Result<String>;

    /// Send a message to start a new dialog turn.
    /// Returns the turn_id. Events are consumed externally via EventQueue.
    async fn send_message(&self, message: String, agent_type: &str) -> Result<String>;

    /// Cancel the current dialog turn (if any)
    async fn cancel_current_turn(&self) -> Result<()>;

    /// Create a brand-new session (ignoring any existing session)
    async fn create_new_session(&self, agent_type: &str) -> Result<String>;

    /// Restore an existing session from persistence
    async fn restore_session(&self, session_id: &str) -> Result<()>;

    /// Confirm tool execution (allow once)
    async fn confirm_tool(&self, tool_id: &str, updated_input: Option<serde_json::Value>) -> Result<()>;

    /// Reject tool execution
    async fn reject_tool(&self, tool_id: &str, reason: String) -> Result<()>;

    /// Submit answers for AskUserQuestion tool
    async fn submit_user_answers(&self, tool_id: &str, answers: serde_json::Value) -> Result<()>;

    /// Get the current core session_id (if created)
    fn session_id(&self) -> Option<String>;
}
