//! Conversation coordinator
//!
//! Top-level component that integrates all subsystems and provides a unified interface

use super::{scheduler::DialogSubmissionPolicy, turn_outcome::TurnOutcome};
use crate::agentic::agents::get_agent_registry;
use crate::agentic::core::{
    has_prompt_markup, Message, MessageContent, ProcessingPhase, PromptEnvelope, Session,
    SessionConfig, SessionKind, SessionState, SessionSummary, TurnStats,
};
use crate::agentic::events::{
    AgenticEvent, EventPriority, EventQueue, EventRouter, EventSubscriber,
};
use crate::agentic::execution::{ContextCompactionOutcome, ExecutionContext, ExecutionEngine};
use crate::agentic::image_analysis::ImageContextData;
use crate::agentic::round_preempt::DialogRoundPreemptSource;
use crate::agentic::session::SessionManager;
use crate::agentic::tools::pipeline::{SubagentParentInfo, ToolPipeline};
use crate::agentic::WorkspaceBinding;
use crate::service::bootstrap::{
    ensure_workspace_persona_files_for_prompt, is_workspace_bootstrap_pending,
};
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, error, info, warn};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration, Instant};
use tokio_util::sync::CancellationToken;

const MANUAL_COMPACTION_COMMAND: &str = "/compact";
const CONTEXT_COMPRESSION_TOOL_NAME: &str = "ContextCompression";

/// Subagent execution result
///
/// Contains the text response after subagent execution
#[derive(Debug, Clone)]
pub struct SubagentResult {
    /// AI text response
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogTriggerSource {
    DesktopUi,
    DesktopApi,
    AgentSession,
    ScheduledJob,
    RemoteRelay,
    Bot,
    Cli,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssistantBootstrapSkipReason {
    BootstrapNotRequired,
    SessionHasExistingTurns,
    SessionNotIdle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssistantBootstrapBlockReason {
    ModelUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantBootstrapEnsureOutcome {
    Started {
        session_id: String,
        turn_id: String,
    },
    Skipped {
        session_id: String,
        reason: AssistantBootstrapSkipReason,
    },
    Blocked {
        session_id: String,
        reason: AssistantBootstrapBlockReason,
        detail: String,
    },
}

const ASSISTANT_BOOTSTRAP_AGENT_TYPE: &str = "Claw";

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
    /// Notifies DialogScheduler of turn outcomes; injected after construction
    scheduler_notify_tx: OnceLock<mpsc::Sender<(String, TurnOutcome)>>,
    /// Round-boundary yield (same source as scheduler's yield flags); injected after construction
    round_preempt_source: OnceLock<Arc<dyn DialogRoundPreemptSource>>,
}

impl ConversationCoordinator {
    /// Build a workspace binding that is remote-aware.
    /// If the global remote workspace is active and matches the session path,
    /// returns a `WorkspaceBinding` with remote metadata and correct local
    /// session storage path.
    async fn build_workspace_binding(config: &SessionConfig) -> Option<WorkspaceBinding> {
        let workspace_path = config.workspace_path.as_ref()?;
        let path_buf = PathBuf::from(workspace_path);

        let identity =
            crate::service::remote_ssh::workspace_state::resolve_workspace_session_identity(
                workspace_path,
                config.remote_connection_id.as_deref(),
                config.remote_ssh_host.as_deref(),
            )
            .await?;

        if let Some(rid) = identity.remote_connection_id.as_deref() {
            let connection_name =
                crate::service::remote_ssh::workspace_state::lookup_remote_connection_with_hint(
                    workspace_path,
                    Some(rid),
                )
                .await
                .map(|e| e.connection_name)
                .unwrap_or_else(|| rid.to_string());

            return Some(WorkspaceBinding::new_remote(
                None,
                path_buf,
                rid.to_string(),
                connection_name,
                identity,
            ));
        }

        Some(WorkspaceBinding::new(None, path_buf))
    }

    /// Build `WorkspaceServices` from a resolved `WorkspaceBinding`.
    /// For remote bindings, wires up SSH-backed FS/shell; for local ones,
    /// returns local implementations.
    async fn build_workspace_services(
        binding: &Option<WorkspaceBinding>,
    ) -> Option<crate::agentic::workspace::WorkspaceServices> {
        let binding = binding.as_ref()?;

        if binding.is_remote() {
            let manager =
                match crate::service::remote_ssh::workspace_state::get_remote_workspace_manager() {
                    Some(m) => m,
                    None => {
                        log::warn!(
                            "build_workspace_services: RemoteWorkspaceStateManager not initialized"
                        );
                        return None;
                    }
                };
            let ssh_manager = match manager.get_ssh_manager().await {
                Some(m) => m,
                None => {
                    log::warn!(
                        "build_workspace_services: SSH manager not available in state manager"
                    );
                    return None;
                }
            };
            let file_service = match manager.get_file_service().await {
                Some(f) => f,
                None => {
                    log::warn!(
                        "build_workspace_services: File service not available in state manager"
                    );
                    return None;
                }
            };
            let connection_id = match binding.connection_id() {
                Some(id) => id.to_string(),
                None => {
                    log::warn!("build_workspace_services: No connection_id in workspace binding");
                    return None;
                }
            };
            log::info!(
                "build_workspace_services: Built remote services for connection_id={}",
                connection_id
            );
            Some(crate::agentic::workspace::remote_workspace_services(
                connection_id,
                file_service,
                ssh_manager,
                binding.root_path_string(),
            ))
        } else {
            Some(crate::agentic::workspace::local_workspace_services(
                binding.root_path_string(),
            ))
        }
    }

    fn normalize_agent_type(agent_type: &str) -> String {
        if agent_type.trim().is_empty() {
            "agentic".to_string()
        } else {
            agent_type.trim().to_string()
        }
    }

    fn ensure_user_message_metadata_object(
        metadata: Option<serde_json::Value>,
    ) -> serde_json::Value {
        match metadata {
            Some(value) if value.is_object() => value,
            Some(value) => serde_json::json!({ "raw_metadata": value }),
            None => serde_json::json!({}),
        }
    }

    fn assistant_bootstrap_kickoff_query(is_chinese: bool) -> &'static str {
        if is_chinese {
            "请开始初始化"
        } else {
            "Please start bootstrap"
        }
    }

    async fn is_chinese_locale() -> bool {
        use crate::service::config::get_global_config_service;
        use crate::service::config::types::AppConfig;
        let Ok(config_service) = get_global_config_service().await else {
            return false;
        };
        let app: AppConfig = config_service
            .get_config(Some("app"))
            .await
            .unwrap_or_default();
        app.language.starts_with("zh")
    }

    fn assistant_bootstrap_system_reminder(
        kickoff_query: &str,
        expected_reply_language: &str,
    ) -> String {
        format!(
            "This is an automatic bootstrap kickoff generated by the system because this assistant workspace still contains BOOTSTRAP.md. \
Treat the user message `{kickoff_query}` only as a start signal, begin bootstrap immediately, and finish it in this session. \
Use {expected_reply_language} for all user-facing replies during bootstrap unless the user later asks to switch languages. \
Update the persona files and delete BOOTSTRAP.md as soon as bootstrap is complete."
        )
    }

    fn estimate_context_tokens(messages: &[Message]) -> usize {
        let mut cloned = messages.to_vec();
        cloned.iter_mut().map(|message| message.get_tokens()).sum()
    }

    fn manual_compaction_metadata() -> serde_json::Value {
        serde_json::json!({
            "kind": "manual_compaction",
            "command": MANUAL_COMPACTION_COMMAND,
        })
    }

    fn build_manual_compaction_round_completed(
        turn_id: &str,
        outcome: &ContextCompactionOutcome,
        context_window: usize,
        threshold: f32,
    ) -> crate::service::session::ModelRoundData {
        use crate::service::session::{ModelRoundData, ToolCallData, ToolItemData, ToolResultData};

        let completed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let started_at = completed_at.saturating_sub(outcome.duration_ms);

        ModelRoundData {
            id: format!("{}-manual-compaction-round", turn_id),
            turn_id: turn_id.to_string(),
            round_index: 0,
            timestamp: started_at,
            text_items: Vec::new(),
            tool_items: vec![ToolItemData {
                id: outcome.compression_id.clone(),
                tool_name: CONTEXT_COMPRESSION_TOOL_NAME.to_string(),
                tool_call: ToolCallData {
                    input: serde_json::json!({
                        "trigger": "manual",
                        "tokens_before": outcome.tokens_before,
                        "context_window": context_window,
                        "threshold": threshold,
                    }),
                    id: outcome.compression_id.clone(),
                },
                tool_result: Some(ToolResultData {
                    result: serde_json::json!({
                        "compression_count": outcome.compression_count,
                        "tokens_before": outcome.tokens_before,
                        "tokens_after": outcome.tokens_after,
                        "compression_ratio": outcome.compression_ratio,
                        "duration": outcome.duration_ms,
                        "applied": outcome.applied,
                        "has_summary": outcome.has_summary,
                        "summary_source": outcome.summary_source,
                    }),
                    success: true,
                    result_for_assistant: None,
                    error: None,
                    duration_ms: Some(outcome.duration_ms),
                }),
                ai_intent: None,
                start_time: started_at,
                end_time: Some(completed_at),
                duration_ms: Some(outcome.duration_ms),
                order_index: Some(0),
                is_subagent_item: None,
                parent_task_tool_id: None,
                subagent_session_id: None,
                status: Some("completed".to_string()),
                interruption_reason: None,
            }],
            thinking_items: Vec::new(),
            start_time: started_at,
            end_time: Some(completed_at),
            status: "completed".to_string(),
        }
    }

    fn build_manual_compaction_round_failed(
        turn_id: &str,
        compression_id: String,
        error: &str,
        context_window: usize,
        threshold: f32,
    ) -> crate::service::session::ModelRoundData {
        use crate::service::session::{ModelRoundData, ToolCallData, ToolItemData, ToolResultData};

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        ModelRoundData {
            id: format!("{}-manual-compaction-round", turn_id),
            turn_id: turn_id.to_string(),
            round_index: 0,
            timestamp,
            text_items: Vec::new(),
            tool_items: vec![ToolItemData {
                id: compression_id.clone(),
                tool_name: CONTEXT_COMPRESSION_TOOL_NAME.to_string(),
                tool_call: ToolCallData {
                    input: serde_json::json!({
                        "trigger": "manual",
                        "context_window": context_window,
                        "threshold": threshold,
                        "summary_source": "none",
                    }),
                    id: compression_id,
                },
                tool_result: Some(ToolResultData {
                    result: serde_json::Value::Null,
                    success: false,
                    result_for_assistant: None,
                    error: Some(error.to_string()),
                    duration_ms: None,
                }),
                ai_intent: None,
                start_time: timestamp,
                end_time: Some(timestamp),
                duration_ms: Some(0),
                order_index: Some(0),
                is_subagent_item: None,
                parent_task_tool_id: None,
                subagent_session_id: None,
                status: Some("error".to_string()),
                interruption_reason: None,
            }],
            thinking_items: Vec::new(),
            start_time: timestamp,
            end_time: Some(timestamp),
            status: "error".to_string(),
        }
    }

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
            scheduler_notify_tx: OnceLock::new(),
            round_preempt_source: OnceLock::new(),
        }
    }

    /// Inject the DialogScheduler notification channel after construction.
    /// Called once during app initialization after the scheduler is created.
    pub fn set_scheduler_notifier(&self, tx: mpsc::Sender<(String, TurnOutcome)>) {
        let _ = self.scheduler_notify_tx.set(tx);
    }

    /// Wire round-boundary preempt (typically the scheduler's [`SessionRoundYieldFlags`](crate::agentic::round_preempt::SessionRoundYieldFlags)).
    pub fn set_round_preempt_source(&self, source: Arc<dyn DialogRoundPreemptSource>) {
        let _ = self.round_preempt_source.set(source);
    }

    /// Create a new session
    pub async fn create_session(
        &self,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
    ) -> BitFunResult<Session> {
        let workspace_path = config.workspace_path.clone().ok_or_else(|| {
            BitFunError::Validation(
                "workspace_path is required when creating a session".to_string(),
            )
        })?;
        self.create_session_with_workspace_and_creator(
            None,
            session_name,
            agent_type,
            config,
            workspace_path,
            None,
        )
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
        let workspace_path = config.workspace_path.clone().ok_or_else(|| {
            BitFunError::Validation(
                "workspace_path is required when creating a session".to_string(),
            )
        })?;
        self.create_session_with_workspace_and_creator(
            session_id,
            session_name,
            agent_type,
            config,
            workspace_path,
            None,
        )
        .await
    }

    /// Create a new session with optional session ID and workspace binding.
    /// `workspace_path` is forwarded in the `SessionCreated` event and also stored
    /// in the session's in-memory config so it can be retrieved without disk access.
    pub async fn create_session_with_workspace(
        &self,
        session_id: Option<String>,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
        workspace_path: String,
    ) -> BitFunResult<Session> {
        self.create_session_with_workspace_and_creator(
            session_id,
            session_name,
            agent_type,
            config,
            workspace_path,
            None,
        )
        .await
    }

    pub async fn update_session_model(&self, session_id: &str, model_id: &str) -> BitFunResult<()> {
        let normalized_model_id = model_id.trim();
        let normalized_model_id = if normalized_model_id.is_empty() {
            "auto"
        } else {
            normalized_model_id
        };

        self.session_manager
            .update_session_model_id(session_id, normalized_model_id)
            .await?;

        info!(
            "Coordinator updated session model: session_id={}, model_id={}",
            session_id, normalized_model_id
        );

        Ok(())
    }

    /// Create a new session with explicit creator identity.
    pub async fn create_session_with_workspace_and_creator(
        &self,
        session_id: Option<String>,
        session_name: String,
        agent_type: String,
        mut config: SessionConfig,
        workspace_path: String,
        created_by: Option<String>,
    ) -> BitFunResult<Session> {
        // Persist the workspace binding inside the session config so execution can
        // consistently restore the correct workspace regardless of the entry point.
        config.workspace_path = Some(workspace_path.clone());
        let agent_type = Self::normalize_agent_type(&agent_type);
        let session = self
            .session_manager
            .create_session_with_id_and_creator(
                session_id,
                session_name,
                agent_type,
                config,
                created_by,
            )
            .await?;

        // SessionManager::create_session_with_id_and_creator already persists the
        // session into the effective workspace session storage path. Avoid writing
        // a second copy here using the raw workspace path, because remote workspaces
        // resolve to a different effective storage path and double-writing can leave
        // metadata/turn files split across two locations.

        self.emit_event(AgenticEvent::SessionCreated {
            session_id: session.session_id.clone(),
            session_name: session.session_name.clone(),
            agent_type: session.agent_type.clone(),
            workspace_path: Some(workspace_path),
        })
        .await;
        Ok(session)
    }

    /// Ensure the completed/failed/cancelled turn is persisted to the workspace
    /// session storage. If the frontend already saved a richer version
    /// during streaming, we only update the final status; otherwise we create
    /// a minimal record with the user message so the turn is never lost.
    /// Safety-net persistence: only creates a minimal record when the frontend
    /// has not saved anything yet.  The frontend's PersistenceModule is the
    /// authoritative writer for turn content (model rounds, text, tools, etc.)
    /// and final status.  This function must NOT overwrite frontend-managed
    /// data, because the spawned task always runs before the frontend receives
    /// the DialogTurnCompleted event via the transport layer, and the existing
    /// disk data from debounced saves may have incomplete model rounds.
    async fn finalize_turn_in_workspace(
        session_id: &str,
        turn_id: &str,
        turn_index: usize,
        user_input: &str,
        workspace_path: &str,
        // Pre-resolved on-disk session storage path (mirror dir for remote workspaces).
        // When present we use it directly so we never re-resolve without remote SSH info
        // (which would slugify a raw remote POSIX path under `~/.bitfun/projects/`).
        resolved_session_storage_path: Option<&std::path::Path>,
        status: crate::service::session::TurnStatus,
        user_message_metadata: Option<serde_json::Value>,
    ) {
        use crate::agentic::persistence::PersistenceManager;
        use crate::infrastructure::PathManager;
        use crate::service::session::{
            DialogTurnData, SessionMetadata, SessionStatus, UserMessageData,
        };

        let path_manager = match PathManager::new() {
            Ok(pm) => std::sync::Arc::new(pm),
            Err(_) => return,
        };

        let workspace_path_buf = match resolved_session_storage_path {
            Some(p) => p.to_path_buf(),
            None => std::path::PathBuf::from(workspace_path),
        };
        let persistence_manager = match PersistenceManager::new(path_manager) {
            Ok(manager) => manager,
            Err(_) => return,
        };

        if let Ok(Some(_existing)) = persistence_manager
            .load_dialog_turn(&workspace_path_buf, session_id, turn_index)
            .await
        {
            return;
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if let Ok(None) = persistence_manager
            .load_session_metadata(&workspace_path_buf, session_id)
            .await
        {
            let metadata = SessionMetadata {
                session_id: session_id.to_string(),
                session_name: "Recovered Session".to_string(),
                agent_type: "agentic".to_string(),
                created_by: None,
                session_kind: SessionKind::Standard,
                model_name: "default".to_string(),
                created_at: now_ms,
                last_active_at: now_ms,
                turn_count: 0,
                message_count: 0,
                tool_call_count: 0,
                status: SessionStatus::Active,
                terminal_session_id: None,
                snapshot_session_id: None,
                tags: Vec::new(),
                custom_metadata: None,
                todos: None,
                workspace_path: Some(workspace_path.to_string()),
                workspace_hostname: None,
                storage_scope: None,
            };
            if let Err(e) = persistence_manager
                .save_session_metadata(&workspace_path_buf, &metadata)
                .await
            {
                warn!(
                    "Failed to create fallback session metadata during turn finalization: session_id={}, error={}",
                    session_id, e
                );
                // Do not return: on read-only or transient IO errors we still try to persist the
                // minimal dialog turn so local/remote UI history is not silently empty.
            }
        }

        let mut turn_data = DialogTurnData::new(
            turn_id.to_string(),
            turn_index,
            session_id.to_string(),
            UserMessageData {
                id: format!("{}-user", turn_id),
                content: user_input.to_string(),
                timestamp: now_ms,
                metadata: user_message_metadata,
            },
        );
        turn_data.status = status;
        turn_data.end_time = Some(now_ms);
        turn_data.duration_ms = Some(now_ms.saturating_sub(turn_data.start_time));

        if let Err(e) = persistence_manager
            .save_dialog_turn(&workspace_path_buf, &turn_data)
            .await
        {
            warn!(
                "Failed to finalize turn in workspace: session_id={}, turn_index={}, error={}",
                session_id, turn_index, e
            );
        }
    }

    /// Create a subagent session for internal AI execution.
    /// Unlike `create_session`, this does NOT emit `SessionCreated` to the transport layer,
    /// because subagent sessions are internal implementation details of the execution engine
    /// and must never appear as top-level items in the UI.
    async fn create_subagent_session(
        &self,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
        parent_info: &SubagentParentInfo,
    ) -> BitFunResult<Session> {
        self.session_manager
            .create_session_with_id_and_details(
                None,
                session_name,
                agent_type,
                config,
                Some(format!("session-{}", parent_info.session_id)),
                SessionKind::Subagent,
            )
            .await
    }

    async fn wrap_user_input(
        &self,
        agent_type: &str,
        user_input: String,
        workspace: Option<&WorkspaceBinding>,
    ) -> BitFunResult<String> {
        let agent_registry = get_agent_registry();
        if let Some(workspace) = workspace {
            agent_registry
                .load_custom_subagents(workspace.root_path())
                .await;
        }
        let current_agent = agent_registry
            .get_agent(agent_type, workspace.map(|binding| binding.root_path()))
            .ok_or_else(|| BitFunError::NotFound(format!("Agent not found: {}", agent_type)))?;
        let system_reminder = current_agent.get_system_reminder(0).await?;

        let mut wrapped_user_input = if has_prompt_markup(&user_input) {
            user_input
        } else {
            let mut envelope = PromptEnvelope::new();
            envelope.push_user_query(user_input);
            envelope.render()
        };
        if !system_reminder.is_empty() {
            let mut envelope = PromptEnvelope::new();
            envelope.push_system_reminder(system_reminder);
            if !wrapped_user_input.is_empty() {
                wrapped_user_input.push('\n');
            }
            wrapped_user_input.push_str(&envelope.render());
        }
        Ok(wrapped_user_input)
    }

    pub async fn ensure_assistant_bootstrap(
        &self,
        session_id: String,
        workspace_path: String,
    ) -> BitFunResult<AssistantBootstrapEnsureOutcome> {
        let workspace_root = PathBuf::from(&workspace_path);
        // Empty or partial assistant dirs may never have run create_assistant_workspace; fill only
        // missing persona stubs (never overwrite), while preserving completed bootstrap state.
        ensure_workspace_persona_files_for_prompt(&workspace_root).await?;
        let bootstrap_pending = is_workspace_bootstrap_pending(&workspace_root);
        if !bootstrap_pending {
            return Ok(AssistantBootstrapEnsureOutcome::Skipped {
                session_id,
                reason: AssistantBootstrapSkipReason::BootstrapNotRequired,
            });
        }

        let session = match self.session_manager.get_session(&session_id) {
            Some(session) => session,
            None => {
                self.session_manager
                    .restore_session(&workspace_root, &session_id)
                    .await?
            }
        };

        let turn_count = self.session_manager.get_turn_count(&session_id);

        if turn_count > 0 {
            return Ok(AssistantBootstrapEnsureOutcome::Skipped {
                session_id,
                reason: AssistantBootstrapSkipReason::SessionHasExistingTurns,
            });
        }

        if !matches!(session.state, SessionState::Idle) {
            return Ok(AssistantBootstrapEnsureOutcome::Skipped {
                session_id,
                reason: AssistantBootstrapSkipReason::SessionNotIdle,
            });
        }

        let is_chinese = Self::is_chinese_locale().await;
        let kickoff_query = Self::assistant_bootstrap_kickoff_query(is_chinese);
        let expected_reply_language = if is_chinese { "Chinese" } else { "English" };
        let workspace_binding = WorkspaceBinding::new(None, workspace_root.clone());
        let model_id = self
            .execution_engine
            .resolve_model_id_for_turn(
                &session,
                ASSISTANT_BOOTSTRAP_AGENT_TYPE,
                Some(&workspace_binding),
                kickoff_query,
                0,
            )
            .await?;

        let ai_client_factory =
            match crate::infrastructure::ai::get_global_ai_client_factory().await {
                Ok(factory) => factory,
                Err(error) => {
                    return Ok(AssistantBootstrapEnsureOutcome::Blocked {
                        session_id,
                        reason: AssistantBootstrapBlockReason::ModelUnavailable,
                        detail: format!("Failed to get AI client factory: {error}"),
                    });
                }
            };

        if let Err(error) = ai_client_factory.get_client_resolved(&model_id).await {
            return Ok(AssistantBootstrapEnsureOutcome::Blocked {
                session_id,
                reason: AssistantBootstrapBlockReason::ModelUnavailable,
                detail: format!("Failed to get AI client (model_id={model_id}): {error}"),
            });
        }

        let mut envelope = PromptEnvelope::new();
        envelope.push_system_reminder(Self::assistant_bootstrap_system_reminder(
            kickoff_query,
            expected_reply_language,
        ));
        envelope.push_user_query(kickoff_query.to_string());

        let turn_id = format!("assistant-bootstrap-{}", uuid::Uuid::new_v4());
        let metadata = serde_json::json!({
            "assistant_bootstrap": {
                "trigger": "lazy_auto",
                "system_generated": true,
                "workspace_path": workspace_path,
            }
        });

        self.start_dialog_turn_internal(
            session_id.clone(),
            envelope.render(),
            Some(kickoff_query.to_string()),
            None,
            Some(turn_id.clone()),
            ASSISTANT_BOOTSTRAP_AGENT_TYPE.to_string(),
            Some(workspace_root.to_string_lossy().to_string()),
            DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopApi)
                .with_skip_tool_confirmation(true),
            Some(metadata),
            true,
        )
        .await?;

        Ok(AssistantBootstrapEnsureOutcome::Started {
            session_id,
            turn_id,
        })
    }

    /// Start a new dialog turn
    /// Note: Events are sent to frontend via EventLoop, no Stream returned.
    /// Submission behavior is controlled by `submission_policy`, which provides
    /// default per-source behavior while still allowing selective overrides.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    pub async fn start_dialog_turn(
        &self,
        session_id: String,
        user_input: String,
        original_user_input: Option<String>,
        turn_id: Option<String>,
        agent_type: String,
        workspace_path: Option<String>,
        submission_policy: DialogSubmissionPolicy,
        extra_user_message_metadata: Option<serde_json::Value>,
    ) -> BitFunResult<()> {
        self.start_dialog_turn_internal(
            session_id,
            user_input,
            original_user_input,
            None,
            turn_id,
            agent_type,
            workspace_path,
            submission_policy,
            extra_user_message_metadata,
            false,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn start_dialog_turn_with_image_contexts(
        &self,
        session_id: String,
        user_input: String,
        original_user_input: Option<String>,
        image_contexts: Vec<ImageContextData>,
        turn_id: Option<String>,
        agent_type: String,
        workspace_path: Option<String>,
        submission_policy: DialogSubmissionPolicy,
        extra_user_message_metadata: Option<serde_json::Value>,
    ) -> BitFunResult<()> {
        self.start_dialog_turn_internal(
            session_id,
            user_input,
            original_user_input,
            Some(image_contexts),
            turn_id,
            agent_type,
            workspace_path,
            submission_policy,
            extra_user_message_metadata,
            false,
        )
        .await
    }

    /// Compact the active session context as a persisted maintenance turn.
    pub async fn compact_session_manually(&self, session_id: String) -> BitFunResult<()> {
        let session = self
            .session_manager
            .get_session(&session_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Session not found: {}", session_id)))?;

        match &session.state {
            SessionState::Idle => {}
            SessionState::Processing {
                current_turn_id,
                phase,
            } => {
                return Err(BitFunError::Validation(format!(
                    "Session is still processing: current_turn_id={}, phase={:?}",
                    current_turn_id, phase
                )));
            }
            SessionState::Error { error, .. } => {
                return Err(BitFunError::Validation(format!(
                    "Session must be idle before manual compaction: {}",
                    error
                )));
            }
        }

        let context_messages = self
            .session_manager
            .get_context_messages(&session_id)
            .await?;
        let needs_restore = if context_messages.is_empty() {
            true
        } else {
            context_messages.len() == 1 && !session.dialog_turn_ids.is_empty()
        };

        if needs_restore {
            let workspace_path = session.config.workspace_path.as_deref().ok_or_else(|| {
                BitFunError::Validation(format!(
                    "workspace_path is required when restoring session: {}",
                    session_id
                ))
            })?;
            self.session_manager
                .restore_session(Path::new(workspace_path), &session_id)
                .await?;
        }

        let context_messages = self
            .session_manager
            .get_context_messages(&session_id)
            .await?;
        let turn_index = self.session_manager.get_turn_count(&session_id);
        let user_message_metadata = Some(Self::manual_compaction_metadata());
        let turn_id = self
            .session_manager
            .start_maintenance_turn(
                &session_id,
                MANUAL_COMPACTION_COMMAND.to_string(),
                None,
                user_message_metadata.clone(),
            )
            .await?;

        self.emit_event(AgenticEvent::DialogTurnStarted {
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            turn_index,
            user_input: MANUAL_COMPACTION_COMMAND.to_string(),
            original_user_input: None,
            user_message_metadata: user_message_metadata.clone(),
            subagent_parent_info: None,
        })
        .await;

        let current_tokens = Self::estimate_context_tokens(&context_messages);
        let session_max_tokens = session.config.max_context_tokens;

        // Unify context_window: min(model capability, session config)
        let model_context_window =
            match crate::infrastructure::ai::get_global_ai_client_factory().await {
                Ok(factory) => {
                    let model_id = session.config.model_id.as_deref().unwrap_or("default");
                    match factory.get_client_resolved(model_id).await {
                        Ok(client) => Some(client.config.context_window as usize),
                        Err(_) => None,
                    }
                }
                Err(_) => None,
            };
        let context_window = match model_context_window {
            Some(mcw) => mcw.min(session_max_tokens),
            None => session_max_tokens,
        };
        let compression_threshold = session.config.compression_threshold;

        match self
            .execution_engine
            .compact_session_context(
                &session_id,
                &turn_id,
                context_messages,
                current_tokens,
                context_window,
                "manual",
                crate::agentic::session::CompressionTailPolicy::CollapseAll,
            )
            .await
        {
            Ok(outcome) => {
                let model_round = Self::build_manual_compaction_round_completed(
                    &turn_id,
                    &outcome,
                    context_window,
                    compression_threshold,
                );
                self.session_manager
                    .complete_maintenance_turn(
                        &session_id,
                        &turn_id,
                        vec![model_round],
                        outcome.duration_ms,
                    )
                    .await?;
                self.session_manager
                    .update_session_state(&session_id, SessionState::Idle)
                    .await?;

                self.emit_event(AgenticEvent::DialogTurnCompleted {
                    session_id,
                    turn_id,
                    total_rounds: 1,
                    total_tools: 1,
                    duration_ms: outcome.duration_ms,
                    subagent_parent_info: None,
                })
                .await;

                Ok(())
            }
            Err(err) => {
                let error_text = err.to_string();
                let compression_id = format!("compression_{}", uuid::Uuid::new_v4());
                let model_round = Self::build_manual_compaction_round_failed(
                    &turn_id,
                    compression_id,
                    &error_text,
                    context_window,
                    compression_threshold,
                );
                let _ = self
                    .session_manager
                    .fail_maintenance_turn(
                        &session_id,
                        &turn_id,
                        error_text.clone(),
                        vec![model_round],
                    )
                    .await;
                let _ = self
                    .session_manager
                    .update_session_state(&session_id, SessionState::Idle)
                    .await;
                self.emit_event(AgenticEvent::DialogTurnFailed {
                    session_id,
                    turn_id,
                    error: error_text.clone(),
                    subagent_parent_info: None,
                })
                .await;
                Err(err)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn start_dialog_turn_internal(
        &self,
        session_id: String,
        user_input: String,
        original_user_input: Option<String>,
        image_contexts: Option<Vec<ImageContextData>>,
        turn_id: Option<String>,
        agent_type: String,
        workspace_path: Option<String>,
        submission_policy: DialogSubmissionPolicy,
        extra_user_message_metadata: Option<serde_json::Value>,
        suppress_session_title_generation: bool,
    ) -> BitFunResult<()> {
        // Get latest session, restoring from persistence on demand so every entry
        // point can use the same start_dialog_turn flow.
        let session = match self.session_manager.get_session(&session_id) {
            Some(session) => session,
            None => {
                debug!(
                    "Session not found in memory, attempting restore before starting dialog: session_id={}",
                    session_id
                );
                let workspace_path = workspace_path.clone().ok_or_else(|| {
                    BitFunError::Validation(format!(
                        "workspace_path is required when restoring session: {}",
                        session_id
                    ))
                })?;
                self.session_manager
                    .restore_session(Path::new(&workspace_path), &session_id)
                    .await?
            }
        };

        let requested_agent_type = agent_type.trim().to_string();
        let provisional_agent_type = if !requested_agent_type.is_empty() {
            requested_agent_type.clone()
        } else if !session.agent_type.is_empty() {
            session.agent_type.clone()
        } else {
            "agentic".to_string()
        };
        let effective_agent_type = Self::normalize_agent_type(&provisional_agent_type);

        debug!(
            "Resolved dialog turn agent type: session_id={}, turn_id={}, requested_agent_type={}, session_agent_type={}, effective_agent_type={}, trigger_source={:?}, queue_priority={:?}, skip_tool_confirmation={}",
            session_id,
            turn_id.as_deref().unwrap_or(""),
            if requested_agent_type.is_empty() {
                "<empty>"
            } else {
                requested_agent_type.as_str()
            },
            if session.agent_type.is_empty() {
                "<empty>"
            } else {
                session.agent_type.as_str()
            },
            effective_agent_type,
            submission_policy.trigger_source,
            submission_policy.queue_priority,
            submission_policy.skip_tool_confirmation
        );

        if session.agent_type != effective_agent_type {
            self.session_manager
                .update_session_agent_type(&session_id, &effective_agent_type)
                .await?;
        }

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
        } else if context_messages.len() == 1 && !session.dialog_turn_ids.is_empty() {
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
            match self
                .session_manager
                .restore_session(
                    Path::new(
                        session
                            .config
                            .workspace_path
                            .as_deref()
                            .or(workspace_path.as_deref())
                            .ok_or_else(|| {
                                BitFunError::Validation(format!(
                                    "workspace_path is required when restoring session: {}",
                                    session_id
                                ))
                            })?,
                    ),
                    &session_id,
                )
                .await
            {
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

        let original_user_input = original_user_input.unwrap_or_else(|| user_input.clone());

        let mut user_message_metadata = extra_user_message_metadata;

        // Build image metadata for workspace turn persistence (before image_contexts is consumed)
        // Also stores original_text so the UI can display the user's actual input
        // instead of the vision-enhanced text.
        if let Some(imgs) = image_contexts.as_ref().filter(|imgs| !imgs.is_empty()) {
            let image_meta: Vec<serde_json::Value> = imgs
                .iter()
                .map(|img| {
                    let name = img
                        .metadata
                        .as_ref()
                        .and_then(|m| m.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("image.png");
                    let mut meta = serde_json::json!({
                        "id": &img.id,
                        "name": name,
                        "mime_type": &img.mime_type,
                    });
                    if let Some(url) = &img.data_url {
                        meta["data_url"] = serde_json::json!(url);
                    }
                    if let Some(path) = &img.image_path {
                        meta["image_path"] = serde_json::json!(path);
                    }
                    meta
                })
                .collect();

            let mut metadata =
                Self::ensure_user_message_metadata_object(user_message_metadata.take());
            if let Some(obj) = metadata.as_object_mut() {
                obj.insert("images".to_string(), serde_json::json!(image_meta));
                obj.insert(
                    "original_text".to_string(),
                    serde_json::json!(original_user_input.clone()),
                );
            }
            user_message_metadata = Some(metadata);
        }

        let session_workspace = Self::build_workspace_binding(&session.config).await;

        // Build WorkspaceServices based on the workspace type
        let workspace_services = Self::build_workspace_services(&session_workspace).await;

        info!(
            "Dialog turn workspace context: session_id={}, workspace_path={:?}, is_remote={}, workspace_services={}",
            session_id,
            session.config.workspace_path,
            session_workspace.as_ref().map(|ws| ws.is_remote()).unwrap_or(false),
            if workspace_services.is_some() { "available" } else { "NONE" }
        );

        let wrapped_user_input = self
            .wrap_user_input(
                &effective_agent_type,
                user_input,
                session_workspace.as_ref(),
            )
            .await?;

        if original_user_input != wrapped_user_input {
            let mut metadata =
                Self::ensure_user_message_metadata_object(user_message_metadata.take());
            if let Some(obj) = metadata.as_object_mut() {
                obj.insert(
                    "original_text".to_string(),
                    serde_json::json!(original_user_input.clone()),
                );
            }
            user_message_metadata = Some(metadata);
        }

        // Inject trigger source into metadata so the frontend can distinguish human
        // user input from agent-session notifications, scheduled jobs, bots, etc.
        {
            let trigger_source_str = match submission_policy.trigger_source {
                DialogTriggerSource::DesktopUi => "desktop_ui",
                DialogTriggerSource::DesktopApi => "desktop_api",
                DialogTriggerSource::AgentSession => "agent_session",
                DialogTriggerSource::ScheduledJob => "scheduled_job",
                DialogTriggerSource::Bot => "bot",
                DialogTriggerSource::Cli => "cli",
                DialogTriggerSource::RemoteRelay => "remote_relay",
            };
            let mut metadata =
                Self::ensure_user_message_metadata_object(user_message_metadata.take());
            if let Some(obj) = metadata.as_object_mut() {
                obj.insert(
                    "triggerSource".to_string(),
                    serde_json::json!(trigger_source_str),
                );
            }
            user_message_metadata = Some(metadata);
        }

        // Start new dialog turn (sets state to Processing internally)
        let turn_index = self.session_manager.get_turn_count(&session_id);
        // Pass frontend turnId, generate if not provided
        let turn_id = self
            .session_manager
            .start_dialog_turn(
                &session_id,
                wrapped_user_input.clone(),
                turn_id,
                image_contexts,
                user_message_metadata.clone(),
            )
            .await?;

        // Send dialog turn started event with original input and image metadata
        // so all frontends (desktop, mobile, bot) can display correctly.
        self.emit_event(AgenticEvent::DialogTurnStarted {
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            turn_index,
            user_input: wrapped_user_input.clone(),
            original_user_input: if original_user_input != wrapped_user_input {
                Some(original_user_input.clone())
            } else {
                None
            },
            user_message_metadata: user_message_metadata.clone(),
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
        context_vars.insert(
            "original_user_input".to_string(),
            original_user_input.clone(),
        );

        // Pass model_id for token usage tracking
        if let Some(model_id) = &session.config.model_id {
            context_vars.insert("model_name".to_string(), model_id.clone());
        }

        // Pass snapshot session ID
        if let Some(snapshot_id) = &session.snapshot_session_id {
            context_vars.insert("snapshot_session_id".to_string(), snapshot_id.clone());
        }

        // Pass turn_index (for operation history/rollback)
        context_vars.insert("turn_index".to_string(), turn_index.to_string());
        let session_workspace_path = session_workspace
            .as_ref()
            .map(|workspace| workspace.root_path_string());
        // Pre-resolve the on-disk session storage path (mirror dir for remote workspaces)
        // so the safety-net writer never has to re-resolve without remote_connection_id /
        // remote_ssh_host (which would silently fall back to a slugified raw remote path).
        let session_storage_path = session_workspace
            .as_ref()
            .map(|workspace| workspace.session_storage_path().to_path_buf());

        let execution_context = ExecutionContext {
            session_id: session_id.clone(),
            dialog_turn_id: turn_id.clone(),
            turn_index,
            agent_type: effective_agent_type.clone(),
            workspace: session_workspace,
            context: context_vars,
            subagent_parent_info: None,
            skip_tool_confirmation: submission_policy.skip_tool_confirmation,
            workspace_services,
            round_preempt: self.round_preempt_source.get().cloned(),
        };

        // Auto-generate session title on first message
        if turn_index == 0 && !suppress_session_title_generation {
            let sm = self.session_manager.clone();
            let eq = self.event_queue.clone();
            let sid = session_id.clone();
            let msg = original_user_input;
            let expected_title = self
                .session_manager
                .get_session(&session_id)
                .map(|session| session.session_name)
                .unwrap_or_default();
            tokio::spawn(async move {
                let allow_ai = is_ai_session_title_generation_enabled().await;
                let resolved = sm.resolve_session_title(&msg, Some(20), allow_ai).await;

                match sm
                    .update_session_title_if_current(&sid, &expected_title, &resolved.title)
                    .await
                {
                    Ok(true) => {
                        let _ = eq
                            .enqueue(
                                AgenticEvent::SessionTitleGenerated {
                                    session_id: sid,
                                    title: resolved.title,
                                    method: resolved.method.as_str().to_string(),
                                },
                                Some(EventPriority::Normal),
                            )
                            .await;
                    }
                    Ok(false) => {
                        debug!("Skipped auto session title update because title changed");
                    }
                    Err(error) => {
                        debug!("Auto session title generation failed to apply: {error}");
                    }
                }
            });
        }

        // Start async execution task
        let session_manager = self.session_manager.clone();
        let execution_engine = self.execution_engine.clone();
        let event_queue = self.event_queue.clone();
        let session_id_clone = session_id.clone();
        let turn_id_clone = turn_id.clone();
        let user_input_for_workspace = wrapped_user_input.clone();
        let session_storage_path_for_finalize = session_storage_path.clone();
        let effective_agent_type_clone = effective_agent_type.clone();
        let user_message_metadata_clone = user_message_metadata;
        let scheduler_notify_tx = self.scheduler_notify_tx.get().cloned();

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

            let workspace_turn_status = match execution_engine
                .execute_dialog_turn(effective_agent_type_clone, messages, execution_context)
                .await
            {
                Ok(execution_result) => {
                    let final_response = match &execution_result.final_message.content {
                        MessageContent::Text(text) => text.clone(),
                        MessageContent::Mixed { text, .. } => text.clone(),
                        _ => String::new(),
                    };
                    info!(
                        "Dialog turn completed: session={}, turn={}, rounds={}",
                        session_id_clone, turn_id_clone, execution_result.total_rounds
                    );

                    let _ = session_manager
                        .complete_dialog_turn(
                            &session_id_clone,
                            &turn_id_clone,
                            final_response.clone(),
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

                    if let Some(tx) = &scheduler_notify_tx {
                        let _ = tx.try_send((
                            session_id_clone.clone(),
                            TurnOutcome::Completed {
                                turn_id: turn_id_clone.clone(),
                                final_response,
                            },
                        ));
                    }

                    Some(crate::service::session::TurnStatus::Completed)
                }
                Err(e) => {
                    let is_cancellation = matches!(&e, BitFunError::Cancelled(_));

                    if is_cancellation {
                        info!(
                            "Dialog turn cancelled: session={}, turn={}",
                            session_id_clone, turn_id_clone
                        );

                        // The execution engine only emits DialogTurnCancelled when
                        // cancellation is detected between rounds.  If cancellation
                        // interrupted streaming mid-round, no event was emitted.
                        // Emit it here unconditionally (duplicates are harmless).
                        let _ = event_queue
                            .enqueue(
                                AgenticEvent::DialogTurnCancelled {
                                    session_id: session_id_clone.clone(),
                                    turn_id: turn_id_clone.clone(),
                                    subagent_parent_info: None,
                                },
                                Some(EventPriority::Critical),
                            )
                            .await;

                        // Mark the turn as completed in persistence so its partial
                        // content appears in historical messages (turns_to_chat_messages
                        // skips InProgress turns).
                        let _ = session_manager
                            .complete_dialog_turn(
                                &session_id_clone,
                                &turn_id_clone,
                                String::new(),
                                TurnStats {
                                    total_rounds: 0,
                                    total_tools: 0,
                                    total_tokens: 0,
                                    duration_ms: 0,
                                },
                            )
                            .await;

                        let _ = session_manager
                            .update_session_state(&session_id_clone, SessionState::Idle)
                            .await;

                        if let Some(tx) = &scheduler_notify_tx {
                            let _ = tx.try_send((
                                session_id_clone.clone(),
                                TurnOutcome::Cancelled {
                                    turn_id: turn_id_clone.clone(),
                                },
                            ));
                        }

                        Some(crate::service::session::TurnStatus::Cancelled)
                    } else {
                        let error_text = e.to_string();
                        error!("Dialog turn execution failed: {}", error_text);

                        let recoverable =
                            !matches!(&e, BitFunError::AIClient(_) | BitFunError::Timeout(_));

                        let _ = event_queue
                            .enqueue(
                                AgenticEvent::DialogTurnFailed {
                                    session_id: session_id_clone.clone(),
                                    turn_id: turn_id_clone.clone(),
                                    error: error_text.clone(),
                                    subagent_parent_info: None,
                                },
                                Some(EventPriority::Critical),
                            )
                            .await;

                        let _ = session_manager
                            .fail_dialog_turn(&session_id_clone, &turn_id_clone, error_text.clone())
                            .await;

                        let _ = session_manager
                            .update_session_state(
                                &session_id_clone,
                                SessionState::Error {
                                    error: error_text.clone(),
                                    recoverable,
                                },
                            )
                            .await;

                        if let Some(tx) = &scheduler_notify_tx {
                            let _ = tx.try_send((
                                session_id_clone.clone(),
                                TurnOutcome::Failed {
                                    turn_id: turn_id_clone.clone(),
                                    error: error_text,
                                },
                            ));
                        }

                        Some(crate::service::session::TurnStatus::Error)
                    }
                }
            };

            if let (Some(ref wp), Some(status)) = (&session_workspace_path, workspace_turn_status) {
                Self::finalize_turn_in_workspace(
                    &session_id_clone,
                    &turn_id_clone,
                    turn_index,
                    &user_input_for_workspace,
                    wp,
                    session_storage_path_for_finalize.as_deref(),
                    status,
                    user_message_metadata_clone,
                )
                .await;
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

        // Step 3: Notify the scheduler immediately so that any agent-session reply_route
        // (e.g. Dispatcher waiting for a sub-agent to finish) is forwarded without delay.
        // The execution task will also send TurnOutcome::Cancelled once the in-flight AI
        // call returns, but by then active_turns will already have been removed, making
        // the second forward_agent_session_reply call a no-op.
        if let Some(tx) = self.scheduler_notify_tx.get() {
            let _ = tx.try_send((
                session_id.to_string(),
                TurnOutcome::Cancelled {
                    turn_id: dialog_turn_id.to_string(),
                },
            ));
            debug!(
                "Immediately notified scheduler of cancellation: session_id={}, turn_id={}",
                session_id, dialog_turn_id
            );
        }

        // Step 4: Async cleanup of old turn (let it end naturally via cancel token, non-blocking)
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

    pub async fn cancel_active_turn_for_session(
        &self,
        session_id: &str,
        wait_timeout: Duration,
    ) -> BitFunResult<Option<String>> {
        let Some(session) = self.session_manager.get_session(session_id) else {
            return Ok(None);
        };

        let SessionState::Processing {
            current_turn_id, ..
        } = session.state
        else {
            return Ok(None);
        };

        self.cancel_dialog_turn(session_id, &current_turn_id)
            .await?;

        let deadline = Instant::now() + wait_timeout;
        while self.execution_engine.has_active_turn(&current_turn_id) {
            if Instant::now() >= deadline {
                warn!(
                    "Timed out waiting for active turn cancellation: session_id={}, dialog_turn_id={}, timeout_ms={}",
                    session_id,
                    current_turn_id,
                    wait_timeout.as_millis()
                );
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        Ok(Some(current_turn_id))
    }

    /// Delete session
    pub async fn delete_session(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<()> {
        self.session_manager
            .delete_session(workspace_path, session_id)
            .await?;
        self.emit_event(AgenticEvent::SessionDeleted {
            session_id: session_id.to_string(),
        })
        .await;
        Ok(())
    }

    /// Restore session
    pub async fn restore_session(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Session> {
        self.session_manager
            .restore_session(workspace_path, session_id)
            .await
    }

    /// List all sessions
    pub async fn list_sessions(&self, workspace_path: &Path) -> BitFunResult<Vec<SessionSummary>> {
        self.session_manager.list_sessions(workspace_path).await
    }

    /// Get a best-effort message view for a session.
    pub async fn get_messages(&self, session_id: &str) -> BitFunResult<Vec<Message>> {
        self.session_manager.get_messages(session_id).await
    }

    /// Get a paginated best-effort message view for a session.
    pub async fn get_messages_paginated(
        &self,
        session_id: &str,
        limit: usize,
        before_message_id: Option<&str>,
    ) -> BitFunResult<(Vec<Message>, bool)> {
        self.session_manager
            .get_messages_paginated(session_id, limit, before_message_id)
            .await
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
    /// Returns SubagentResult with the final text response
    pub async fn execute_subagent(
        &self,
        agent_type: String,
        task_description: String,
        subagent_parent_info: SubagentParentInfo,
        workspace_path: Option<String>,
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

        // Create independent subagent session.
        // Use create_subagent_session (not create_session) so that no SessionCreated
        // event is emitted to the transport layer — subagent sessions are internal
        // implementation details and must not appear in the UI session list.
        let workspace_path = workspace_path.ok_or_else(|| {
            BitFunError::Validation(
                "workspace_path is required when creating a subagent session".to_string(),
            )
        })?;
        let subagent_config = SessionConfig {
            workspace_path: Some(workspace_path),
            ..SessionConfig::default()
        };
        let session = self
            .create_subagent_session(
                format!("Subagent: {}", task_description),
                agent_type.clone(),
                subagent_config,
                &subagent_parent_info,
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

        let subagent_workspace = Self::build_workspace_binding(&session.config).await;
        let subagent_services = Self::build_workspace_services(&subagent_workspace).await;
        let execution_context = ExecutionContext {
            session_id: session.session_id.clone(),
            dialog_turn_id: dialog_turn_id.clone(),
            turn_index: 0,
            agent_type: agent_type.clone(),
            workspace: subagent_workspace,
            context: context.unwrap_or_default(),
            subagent_parent_info: Some(subagent_parent_info),
            // Subagents run autonomously without user interaction; always skip
            // tool confirmation to prevent them from blocking indefinitely on a
            // confirmation channel that nobody will ever respond to.
            skip_tool_confirmation: true,
            workspace_services: subagent_services,
            round_preempt: self.round_preempt_source.get().cloned(),
        };

        let initial_messages = vec![Message::user(task_description)];

        let result = self
            .execution_engine
            .execute_dialog_turn(agent_type, initial_messages, execution_context)
            .await;

        // cleanup_guard automatically cleans up token on scope exit (via Drop trait)

        // Extract text response
        let response_text = match result {
            Ok(exec_result) => match exec_result.final_message.content {
                MessageContent::Mixed { text, .. } => text,
                MessageContent::Text(text) => text,
                _ => String::new(),
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
        if let Some(workspace_path) = self
            .session_manager
            .get_session(session_id)
            .and_then(|session| session.config.workspace_path.map(std::path::PathBuf::from))
        {
            if let Ok(snapshot_manager) =
                crate::service::snapshot::ensure_snapshot_manager_for_workspace(&workspace_path)
            {
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
        }

        // Delete the subagent session itself, including runtime context and persisted turn data.
        let workspace_path = self
            .session_manager
            .get_session(session_id)
            .and_then(|session| session.config.workspace_path.map(std::path::PathBuf::from));

        if let Some(workspace_path) = workspace_path {
            if let Err(e) = self
                .session_manager
                .delete_session(&workspace_path, session_id)
                .await
            {
                warn!(
                    "Failed to delete subagent session: session={}, error={}",
                    session_id, e
                );
            } else {
                debug!("Subagent session deleted: session={}", session_id);
            }
        } else {
            warn!(
                "Failed to delete subagent session because workspace_path is missing: session={}",
                session_id
            );
        }

        debug!(
            "Subagent resource cleanup completed: session_id={}",
            session_id
        );
        Ok(())
    }

    /// Generate session title
    ///
    /// Use AI to generate a concise and accurate session title based on user message content.
    /// Also persists the title to the session backend. Callers that go through
    /// `start_dialog_turn` do NOT need to call this separately — first-message
    /// title generation is handled automatically inside `start_dialog_turn`.
    pub async fn generate_session_title(
        &self,
        session_id: &str,
        user_message: &str,
        max_length: Option<usize>,
    ) -> BitFunResult<String> {
        let allow_ai = is_ai_session_title_generation_enabled().await;
        let resolved = self
            .session_manager
            .resolve_session_title(user_message, max_length, allow_ai)
            .await;

        self.session_manager
            .update_session_title(session_id, &resolved.title)
            .await?;

        let event = AgenticEvent::SessionTitleGenerated {
            session_id: session_id.to_string(),
            title: resolved.title.clone(),
            method: resolved.method.as_str().to_string(),
        };
        self.emit_event(event).await;

        debug!(
            "Session title generation event sent: session_id={}, title={}",
            session_id, resolved.title
        );

        Ok(resolved.title)
    }

    pub async fn update_session_title(
        &self,
        session_id: &str,
        title: &str,
    ) -> BitFunResult<String> {
        let normalized = title.trim().to_string();
        if normalized.is_empty() {
            return Err(BitFunError::validation(
                "Session title must not be empty".to_string(),
            ));
        }

        self.session_manager
            .update_session_title(session_id, &normalized)
            .await?;

        Ok(normalized)
    }

    /// Emit event
    async fn emit_event(&self, event: AgenticEvent) {
        let _ = self
            .event_queue
            .enqueue(event, Some(EventPriority::Normal))
            .await;
    }

    /// Emit a `SessionModelAutoMigrated` event with `High` priority so the
    /// frontend can refresh its model selector and surface a notice promptly.
    ///
    /// Callers (e.g. `SessionManager`) reach this method via
    /// [`get_global_coordinator`] so they don't need to thread an
    /// `Arc<EventQueue>` through every constructor.
    pub async fn emit_session_model_auto_migrated(
        &self,
        session_id: &str,
        previous_model_id: &str,
        new_model_id: &str,
        reason: &str,
    ) {
        let event = AgenticEvent::SessionModelAutoMigrated {
            session_id: session_id.to_string(),
            previous_model_id: previous_model_id.to_string(),
            new_model_id: new_model_id.to_string(),
            reason: reason.to_string(),
        };
        let _ = self
            .event_queue
            .enqueue(event, Some(EventPriority::High))
            .await;
    }

    /// Get SessionManager reference (for advanced features like mode management)
    pub fn get_session_manager(&self) -> &Arc<SessionManager> {
        &self.session_manager
    }

    /// Persist a completed `/btw` side-question turn into an existing child session.
    #[allow(clippy::too_many_arguments)]
    pub async fn persist_btw_turn(
        &self,
        workspace_path: &Path,
        child_session_id: &str,
        request_id: &str,
        question: &str,
        full_text: &str,
        parent_session_id: &str,
        parent_dialog_turn_id: Option<&str>,
        parent_turn_index: Option<usize>,
    ) -> BitFunResult<()> {
        self.session_manager
            .persist_btw_turn(
                workspace_path,
                child_session_id,
                request_id,
                question,
                full_text,
                parent_session_id,
                parent_dialog_turn_id,
                parent_turn_index,
            )
            .await
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

async fn is_ai_session_title_generation_enabled() -> bool {
    match crate::service::config::get_global_config_service().await {
        Ok(service) => service
            .get_config::<bool>(Some("app.ai_experience.enable_session_title_generation"))
            .await
            .unwrap_or(true),
        Err(_) => true,
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
