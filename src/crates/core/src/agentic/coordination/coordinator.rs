//! Conversation coordinator
//!
//! Top-level component that integrates all subsystems and provides a unified interface

use crate::agentic::agents::get_agent_registry;
use crate::agentic::core::{
    Message, MessageContent, ProcessingPhase, Session, SessionConfig, SessionState, SessionSummary,
    TurnStats,
};
use crate::agentic::events::{
    AgenticEvent, EventPriority, EventQueue, EventRouter, EventSubscriber,
};
use crate::agentic::execution::{ExecutionContext, ExecutionEngine};
use crate::agentic::session::SessionManager;
use crate::agentic::tools::pipeline::{SubagentParentInfo, ToolPipeline};
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, error, info, warn};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio_util::sync::CancellationToken;

/// Subagent execution result
///
/// Contains the text response and optional tool arguments after subagent execution
#[derive(Debug, Clone)]
pub struct SubagentResult {
    /// AI text response
    pub text: String,
    /// Tool call arguments for ending the conversation
    pub tool_arguments: Option<serde_json::Value>,
}

/// Cancel token cleanup guard
///
/// Automatically cleans up cancel tokens in ExecutionEngine when dropped
struct CancelTokenGuard {
    execution_engine: Arc<ExecutionEngine>,
    dialog_turn_id: String,
}

impl Drop for CancelTokenGuard {
    fn drop(&mut self) {
        let execution_engine = self.execution_engine.clone();
        let dialog_turn_id = self.dialog_turn_id.clone();

        tokio::spawn(async move {
            execution_engine.cleanup_cancel_token(&dialog_turn_id).await;
        });
    }
}

/// Conversation coordinator
pub struct ConversationCoordinator {
    session_manager: Arc<SessionManager>,
    execution_engine: Arc<ExecutionEngine>,
    tool_pipeline: Arc<ToolPipeline>,
    event_queue: Arc<EventQueue>,
    event_router: Arc<EventRouter>,
}

impl ConversationCoordinator {
    pub fn new(
        session_manager: Arc<SessionManager>,
        execution_engine: Arc<ExecutionEngine>,
        tool_pipeline: Arc<ToolPipeline>,
        event_queue: Arc<EventQueue>,
        event_router: Arc<EventRouter>,
    ) -> Self {
        Self {
            session_manager,
            execution_engine,
            tool_pipeline,
            event_queue,
            event_router,
        }
    }

    /// Create a new session
    pub async fn create_session(
        &self,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
    ) -> BitFunResult<Session> {
        self.session_manager
            .create_session(session_name, agent_type, config)
            .await
    }

    /// Create a new session with optional session ID
    pub async fn create_session_with_id(
        &self,
        session_id: Option<String>,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
    ) -> BitFunResult<Session> {
        self.session_manager
            .create_session_with_id(session_id, session_name, agent_type, config)
            .await
    }

    async fn wrap_user_input(&self, agent_type: &str, user_input: String) -> BitFunResult<String> {
        let agent_registry = get_agent_registry();
        let current_agent = agent_registry
            .get_agent(&agent_type)
            .ok_or_else(|| BitFunError::NotFound(format!("Agent not found: {}", agent_type)))?;
        let system_reminder = current_agent.get_system_reminder(0).await?;

        let mut wrapped_user_input = if agent_type == "agentic" {
            // Only this mode uses user_query tag
            format!("<user_query>\n{}\n</user_query>\n", user_input)
        } else {
            user_input
        };
        if !system_reminder.is_empty() {
            wrapped_user_input.push_str(&format!(
                "<system_reminder>\n{}\n</system_reminder>",
                system_reminder
            ));
        }
        Ok(wrapped_user_input)
    }

    /// Start a new dialog turn
    /// Note: Events are sent to frontend via EventLoop, no Stream returned
    pub async fn start_dialog_turn(
        &self,
        session_id: String,
        user_input: String,
        turn_id: Option<String>,
        agent_type: String,
    ) -> BitFunResult<()> {
        // Get latest session (re-fetch each time to ensure latest state)
        let session = self
            .session_manager
            .get_session(&session_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Session not found: {}", session_id)))?;

        debug!(
            "Checking session state: session_id={}, state={:?}",
            session_id, session.state
        );

        // Check session state
        // Allow Idle or any error state (user can retry after error)
        // If Processing, cancel request hasn't arrived yet, reject new dialog
        match &session.state {
            SessionState::Idle => {
                debug!(
                    "Session state is Idle, allowing new dialog: session_id={}",
                    session_id
                );
            }
            SessionState::Error { .. } => {
                debug!(
                    "Session in error state, allowing new dialog (user retry): session_id={}",
                    session_id
                );
            }
            SessionState::Processing {
                current_turn_id,
                phase,
            } => {
                warn!(
                    "Session still processing, rejecting new dialog: session_id={}, current_turn_id={}, phase={:?}",
                    session_id,
                    current_turn_id,
                    phase
                );
                return Err(BitFunError::Validation(format!(
                    "Session state does not allow starting new dialog: {:?}",
                    session.state
                )));
            }
        }

        // Ensure session history is loaded into memory
        // Critical fix: prevent unloaded history after app restart
        let context_messages = self
            .session_manager
            .get_context_messages(&session_id)
            .await?;

        // Check if restore is needed:
        // - Empty context needs restore
        // - Only 1 message (likely just system prompt) with existing turns needs restore
        // - Sessions with multiple turns should have > 1 messages (at least system + user + assistant)
        let needs_restore = if context_messages.is_empty() {
            debug!(
                "Session {} context is empty, restoring from persistence",
                session_id
            );
            true
        } else if context_messages.len() == 1 && session.dialog_turn_ids.len() > 0 {
            debug!(
                "Session {} has {} turns but only {} messages, restoring history",
                session_id,
                session.dialog_turn_ids.len(),
                context_messages.len()
            );
            true
        } else {
            debug!(
                "Session {} context exists ({} messages, {} turns), no restore needed",
                session_id,
                context_messages.len(),
                session.dialog_turn_ids.len()
            );
            false
        };

        if needs_restore {
            debug!(
                "Starting session history restore: session_id={}",
                session_id
            );
            match self.session_manager.restore_session(&session_id).await {
                Ok(_) => {
                    let restored_messages = self
                        .session_manager
                        .get_context_messages(&session_id)
                        .await?;
                    info!(
                        "Session history restored from persistence: session_id={}, messages: {} -> {}",
                        session_id,
                        context_messages.len(),
                        restored_messages.len()
                    );
                }
                Err(e) => {
                    debug!(
                        "Failed to restore session history (may be new session): session_id={}, error={}",
                        session_id,
                        e
                    );
                }
            }
        }

        let wrapped_user_input = self.wrap_user_input(&agent_type, user_input).await?;

        // Start new dialog turn (sets state to Processing internally)
        let turn_index = self.session_manager.get_turn_count(&session_id);
        // Pass frontend turnId, generate if not provided
        let turn_id = self
            .session_manager
            .start_dialog_turn(&session_id, wrapped_user_input.clone(), turn_id)
            .await?;

        // Send dialog turn started event
        self.emit_event(AgenticEvent::DialogTurnStarted {
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            turn_index,
            user_input: wrapped_user_input.clone(),
            subagent_parent_info: None,
        })
        .await;

        // Get context messages (re-fetch as history may have been restored)
        let messages = self
            .session_manager
            .get_context_messages(&session_id)
            .await?;

        // Create execution context (pass full config and resource IDs)
        let mut context_vars = std::collections::HashMap::new();
        context_vars.insert(
            "max_context_tokens".to_string(),
            session.config.max_context_tokens.to_string(),
        );
        context_vars.insert(
            "enable_tools".to_string(),
            session.config.enable_tools.to_string(),
        );

        // Pass snapshot session ID
        if let Some(snapshot_id) = &session.snapshot_session_id {
            context_vars.insert("snapshot_session_id".to_string(), snapshot_id.clone());
        }

        // Pass turn_index (for operation history/rollback)
        context_vars.insert("turn_index".to_string(), turn_index.to_string());

        let execution_context = ExecutionContext {
            session_id: session_id.clone(),
            dialog_turn_id: turn_id.clone(),
            turn_index,
            agent_type: session.agent_type.clone(),
            context: context_vars,
            subagent_parent_info: None,
        };

        // Start async execution task
        let session_manager = self.session_manager.clone();
        let execution_engine = self.execution_engine.clone();
        let event_queue = self.event_queue.clone();
        let session_id_clone = session_id.clone();
        let turn_id_clone = turn_id.clone();

        tokio::spawn(async move {
            // Note: Don't check cancellation here as cancel token hasn't been created yet
            // Cancel token is created in execute_dialog_turn -> execute_round
            // execute_dialog_turn has proper cancellation checks internally

            let _ = session_manager
                .update_session_state(
                    &session_id_clone,
                    SessionState::Processing {
                        current_turn_id: turn_id_clone.clone(),
                        phase: ProcessingPhase::Thinking,
                    },
                )
                .await;

            match execution_engine
                .execute_dialog_turn(agent_type, messages, execution_context)
                .await
            {
                Ok(execution_result) => {
                    info!(
                        "Dialog turn completed: session={}, turn={}, rounds={}",
                        session_id_clone, turn_id_clone, execution_result.total_rounds
                    );

                    let _ = session_manager
                        .complete_dialog_turn(
                            &session_id_clone,
                            &turn_id_clone,
                            match &execution_result.final_message.content {
                                MessageContent::Text(text) => text.clone(),
                                MessageContent::Mixed { text, .. } => text.clone(),
                                _ => String::new(),
                            },
                            TurnStats {
                                total_rounds: execution_result.total_rounds,
                                total_tools: 0, // TODO: get from execution_result
                                total_tokens: 0,
                                duration_ms: 0,
                            },
                        )
                        .await;

                    let _ = session_manager
                        .update_session_state(&session_id_clone, SessionState::Idle)
                        .await;
                }
                Err(e) => {
                    let is_cancellation = matches!(&e, BitFunError::Cancelled(_));

                    if is_cancellation {
                        // DialogTurnCancelled already sent in execution_engine
                        debug!("Dialog turn cancelled: {}", e);

                        let _ = session_manager
                            .update_session_state(&session_id_clone, SessionState::Idle)
                            .await;
                    } else {
                        error!("Dialog turn execution failed: {}", e);

                        let recoverable =
                            !matches!(&e, BitFunError::AIClient(_) | BitFunError::Timeout(_));

                        let _ = event_queue
                            .enqueue(
                                AgenticEvent::DialogTurnFailed {
                                    session_id: session_id_clone.clone(),
                                    turn_id: turn_id_clone.clone(),
                                    error: e.to_string(),
                                    subagent_parent_info: None,
                                },
                                Some(EventPriority::Critical),
                            )
                            .await;

                        let _ = session_manager
                            .update_session_state(
                                &session_id_clone,
                                SessionState::Error {
                                    error: e.to_string(),
                                    recoverable,
                                },
                            )
                            .await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Cancel dialog turn execution
    /// Immediately set state to Idle to allow new dialog, old turn ends naturally via cancel token
    pub async fn cancel_dialog_turn(
        &self,
        session_id: &str,
        dialog_turn_id: &str,
    ) -> BitFunResult<()> {
        info!(
            "Received cancel request: dialog_turn_id={}, session_id={}",
            dialog_turn_id, session_id
        );

        let old_state = self
            .session_manager
            .get_session(session_id)
            .map(|s| format!("{:?}", s.state))
            .unwrap_or_else(|| "Unknown".to_string());
        debug!("Current state: {}", old_state);

        // Step 1: Immediately update session state to Idle (non-blocking, allows immediate new dialog)
        debug!("Updating session state to Idle");
        self.session_manager
            .update_session_state(session_id, SessionState::Idle)
            .await?;

        let new_state = self
            .session_manager
            .get_session(session_id)
            .map(|s| format!("{:?}", s.state))
            .unwrap_or_else(|| "Unknown".to_string());
        debug!("State updated: {} -> {}", old_state, new_state);

        // Step 2: Immediately send state change event (notify frontend can start new dialog)
        self.emit_event(AgenticEvent::SessionStateChanged {
            session_id: session_id.to_string(),
            new_state: "idle".to_string(),
        })
        .await;
        debug!("Session state change event sent");

        // Step 3: Async cleanup of old turn (let it end naturally via cancel token, non-blocking)
        let execution_engine = self.execution_engine.clone();
        let tool_pipeline = self.tool_pipeline.clone();
        let dialog_turn_id_clone = dialog_turn_id.to_string();

        tokio::spawn(async move {
            debug!(
                "Starting async cleanup for cancelled turn: {}",
                dialog_turn_id_clone
            );

            if let Err(e) = execution_engine
                .cancel_dialog_turn(&dialog_turn_id_clone)
                .await
            {
                warn!("Failed to cancel execution engine: {}", e);
            }

            if let Err(e) = tool_pipeline
                .cancel_dialog_turn_tools(&dialog_turn_id_clone)
                .await
            {
                warn!("Failed to cancel tool execution: {}", e);
            }

            debug!("Async cleanup completed: {}", dialog_turn_id_clone);
        });

        Ok(())
    }

    /// Delete session
    pub async fn delete_session(&self, session_id: &str) -> BitFunResult<()> {
        self.session_manager.delete_session(session_id).await
    }

    /// Restore session
    pub async fn restore_session(&self, session_id: &str) -> BitFunResult<Session> {
        self.session_manager.restore_session(session_id).await
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> BitFunResult<Vec<SessionSummary>> {
        self.session_manager.list_sessions().await
    }

    /// Get session messages
    pub async fn get_messages(&self, session_id: &str) -> BitFunResult<Vec<Message>> {
        self.session_manager.get_messages(session_id).await
    }

    /// Subscribe to internal events
    ///
    /// For internal systems to subscribe to events (e.g., logging, monitoring)
    pub fn subscribe_internal<H>(&self, subscriber_id: String, handler: H)
    where
        H: EventSubscriber + 'static,
    {
        self.event_router
            .subscribe_internal(subscriber_id, Arc::new(handler));
    }

    /// Unsubscribe from internal events
    ///
    /// Remove subscriber previously added via subscribe_internal
    pub fn unsubscribe_internal(&self, subscriber_id: &str) {
        self.event_router.unsubscribe_internal(subscriber_id);
    }

    /// Confirm tool execution
    pub async fn confirm_tool(
        &self,
        tool_id: &str,
        updated_input: Option<serde_json::Value>,
    ) -> BitFunResult<()> {
        self.tool_pipeline
            .confirm_tool(tool_id, updated_input)
            .await
    }

    /// Reject tool execution
    pub async fn reject_tool(&self, tool_id: &str, reason: String) -> BitFunResult<()> {
        self.tool_pipeline.reject_tool(tool_id, reason).await
    }

    /// Cancel tool execution
    pub async fn cancel_tool(&self, tool_id: &str, reason: String) -> BitFunResult<()> {
        self.tool_pipeline.cancel_tool(tool_id, reason).await
    }

    /// Execute subagent task directly
    /// DialogTurnStarted event not needed for now
    ///
    /// Parameters:
    /// - agent_type: Agent type
    /// - task_description: Task description
    /// - subagent_parent_info: Parent info (tool call context)
    /// - context: Additional context
    /// - cancel_token: Optional cancel token (for async cancellation)
    ///
    /// Returns SubagentResult with text response and optional tool arguments
    pub async fn execute_subagent(
        &self,
        agent_type: String,
        task_description: String,
        subagent_parent_info: SubagentParentInfo,
        context: Option<std::collections::HashMap<String, String>>,
        cancel_token: Option<&CancellationToken>,
    ) -> BitFunResult<SubagentResult> {
        // Check cancel token (before creating session)
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                debug!("Subagent task cancelled before execution");
                return Err(BitFunError::Cancelled(
                    "Subagent task has been cancelled".to_string(),
                ));
            }
        }

        // Create independent subagent session
        let session = self
            .create_session(
                format!("Subagent: {}", task_description),
                agent_type.clone(),
                Default::default(),
            )
            .await?;

        // Check cancel token (after creating session, before execution)
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                debug!("Subagent task cancelled before AI call, cleaning up resources");
                let _ = self.cleanup_subagent_resources(&session.session_id).await;
                return Err(BitFunError::Cancelled(
                    "Subagent task has been cancelled".to_string(),
                ));
            }
        }

        // Generate unique dialog_turn_id for cancel token management
        let dialog_turn_id = format!("subagent-{}", uuid::Uuid::new_v4());
        debug!(
            "Generated unique dialog_turn_id for subagent: {}",
            dialog_turn_id
        );

        // If external cancel_token provided, create child_token and register to RoundExecutor
        // This allows execute_dialog_turn internal checks to detect external cancellation
        let _cleanup_guard = if let Some(parent_token) = cancel_token {
            // Create child_token, cancelled when parent_token is cancelled
            let child_token = parent_token.child_token();

            // Register to ExecutionEngine (forwarded to RoundExecutor), using dialog_turn_id as key
            self.execution_engine
                .register_cancel_token(&dialog_turn_id, child_token.clone());

            debug!(
                "Registered cancel token to RoundExecutor: dialog_turn_id={}",
                dialog_turn_id
            );

            // Create cleanup guard to ensure token cleanup on function exit
            Some(CancelTokenGuard {
                execution_engine: self.execution_engine.clone(),
                dialog_turn_id: dialog_turn_id.clone(),
            })
        } else {
            None
        };

        let execution_context = ExecutionContext {
            session_id: session.session_id.clone(),
            dialog_turn_id: dialog_turn_id.clone(),
            turn_index: 0,
            agent_type: agent_type.clone(),
            context: context.unwrap_or_default(),
            subagent_parent_info: Some(subagent_parent_info),
        };

        let initial_messages = vec![Message::user(task_description)];

        let result = self
            .execution_engine
            .execute_dialog_turn(agent_type, initial_messages, execution_context)
            .await;

        // cleanup_guard automatically cleans up token on scope exit (via Drop trait)

        // Extract text response and tool arguments
        let (response_text, tool_arguments) = match result {
            Ok(exec_result) => match exec_result.final_message.content {
                MessageContent::Mixed {
                    text, tool_calls, ..
                } => (text, {
                    // Find first should_end_turn tool arguments, tool_pipeline guarantees only one
                    tool_calls
                        .into_iter()
                        .find(|tc| tc.should_end_turn)
                        .map(|tc| tc.arguments)
                }),
                MessageContent::Text(text) => (text, None),
                _ => (String::new(), None),
            },
            Err(e) => {
                error!(
                    "Subagent execution failed: session={}, error={}",
                    session.session_id, e
                );

                if let Err(cleanup_err) = self.cleanup_subagent_resources(&session.session_id).await
                {
                    warn!(
                        "Failed to cleanup subagent resources: session={}, error={}",
                        session.session_id, cleanup_err
                    );
                }

                return Err(e);
            }
        };

        // Clean up subagent session resources after successful execution
        debug!(
            "Starting subagent resource cleanup: session={}",
            session.session_id
        );
        if let Err(e) = self.cleanup_subagent_resources(&session.session_id).await {
            warn!(
                "Failed to cleanup subagent resources: session={}, error={}",
                session.session_id, e
            );
        } else {
            debug!(
                "Subagent resource cleanup completed: session={}",
                session.session_id
            );
        }

        Ok(SubagentResult {
            text: response_text,
            tool_arguments,
        })
    }

    /// Clean up subagent session resources
    ///
    /// Release resources occupied by subagent session (sandbox, etc.) and delete session
    async fn cleanup_subagent_resources(&self, session_id: &str) -> BitFunResult<()> {
        debug!(
            "Starting subagent resource cleanup: session_id={}",
            session_id
        );

        // Clean up snapshot system resources
        use crate::service::snapshot::get_global_snapshot_manager;
        if let Some(snapshot_manager) = get_global_snapshot_manager() {
            let snapshot_service = snapshot_manager.get_snapshot_service();
            let snapshot_service = snapshot_service.read().await;
            if let Err(e) = snapshot_service.accept_session(session_id).await {
                warn!(
                    "Failed to cleanup snapshot system resources: session={}, error={}",
                    session_id, e
                );
            } else {
                debug!(
                    "Snapshot system resources cleaned up: session={}",
                    session_id
                );
            }
        }

        // Delete subagent session itself (including message history, persistence data, etc.)
        if let Err(e) = self.session_manager.delete_session(session_id).await {
            warn!(
                "Failed to delete subagent session: session={}, error={}",
                session_id, e
            );
        } else {
            debug!("Subagent session deleted: session={}", session_id);
        }

        debug!(
            "Subagent resource cleanup completed: session_id={}",
            session_id
        );
        Ok(())
    }

    /// Generate session title
    ///
    /// Use AI to generate a concise and accurate session title based on user message content
    pub async fn generate_session_title(
        &self,
        session_id: &str,
        user_message: &str,
        max_length: Option<usize>,
    ) -> BitFunResult<String> {
        let title = self
            .session_manager
            .generate_session_title(user_message, max_length)
            .await?;

        let event = AgenticEvent::SessionTitleGenerated {
            session_id: session_id.to_string(),
            title: title.clone(),
            method: "ai".to_string(),
        };
        self.emit_event(event).await;

        debug!(
            "Session title generation event sent: session_id={}, title={}",
            session_id, title
        );

        Ok(title)
    }

    /// Emit event
    async fn emit_event(&self, event: AgenticEvent) {
        let _ = self
            .event_queue
            .enqueue(event, Some(EventPriority::Normal))
            .await;
    }

    /// Get SessionManager reference (for advanced features like mode management)
    pub fn get_session_manager(&self) -> &Arc<SessionManager> {
        &self.session_manager
    }

    /// Set global coordinator (called during initialization)
    ///
    /// Skips if global coordinator already exists
    pub fn set_global(coordinator: Arc<ConversationCoordinator>) {
        match GLOBAL_COORDINATOR.set(coordinator) {
            Ok(_) => {
                debug!("Global coordinator set");
            }
            Err(_) => {
                debug!("Global coordinator already exists, skipping set");
            }
        }
    }
}

// Global coordinator singleton
static GLOBAL_COORDINATOR: OnceLock<Arc<ConversationCoordinator>> = OnceLock::new();

/// Get global coordinator
///
/// Returns `None` if coordinator hasn't been initialized
pub fn get_global_coordinator() -> Option<Arc<ConversationCoordinator>> {
    GLOBAL_COORDINATOR.get().cloned()
}
