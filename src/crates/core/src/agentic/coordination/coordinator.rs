//! Conversation coordinator
//!
//! Top-level component that integrates all subsystems and provides a unified interface

use super::{scheduler::DialogSubmissionPolicy, turn_outcome::TurnOutcome};
use crate::agentic::agents::get_agent_registry;
use crate::agentic::context_profile::ContextProfilePolicy;
use crate::agentic::core::{
    has_prompt_markup, Message, MessageContent, ProcessingPhase, PromptEnvelope, Session,
    SessionConfig, SessionKind, SessionState, SessionSummary, TurnStats,
};
use crate::agentic::events::{
    AgenticEvent, EventPriority, EventQueue, EventRouter, EventSubscriber,
};
use crate::agentic::execution::{ContextCompactionOutcome, ExecutionContext, ExecutionEngine};
use crate::agentic::fork_agent::{
    ForkAgentContextSnapshot, ForkAgentExecutionRequest, ForkAgentExecutionResult,
};
use crate::agentic::image_analysis::ImageContextData;
use crate::agentic::round_preempt::DialogRoundPreemptSource;
use crate::agentic::session::SessionManager;
use crate::agentic::side_question::build_btw_user_input;
use crate::agentic::tools::pipeline::{SubagentParentInfo, ToolPipeline};
use crate::agentic::tools::ToolRuntimeRestrictions;
use crate::agentic::WorkspaceBinding;
use crate::service::bootstrap::{
    ensure_workspace_persona_files_for_prompt, is_workspace_bootstrap_pending,
};
use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::{mpsc, watch, OwnedSemaphorePermit, RwLock, Semaphore};
use tokio::time::{sleep, Duration, Instant};
use tokio_util::sync::CancellationToken;

const MANUAL_COMPACTION_COMMAND: &str = "/compact";
const CONTEXT_COMPRESSION_TOOL_NAME: &str = "ContextCompression";
const DEFAULT_SUBAGENT_MAX_CONCURRENCY: usize = 5;
const MAX_SUBAGENT_MAX_CONCURRENCY: usize = 64;
const SUBAGENT_TIMEOUT_GRACE_PERIOD: Duration = Duration::from_secs(10);

/// Subagent execution result
///
/// Contains the text response after subagent execution
#[derive(Debug, Clone)]
pub struct SubagentResult {
    /// AI text response
    pub text: String,
    pub status: SubagentResultStatus,
    pub reason: Option<String>,
    pub ledger_event_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentResultStatus {
    Completed,
    PartialTimeout,
}

impl SubagentResult {
    fn completed(text: String) -> Self {
        Self {
            text,
            status: SubagentResultStatus::Completed,
            reason: None,
            ledger_event_id: None,
        }
    }

    fn partial_timeout(text: String, reason: String) -> Self {
        Self {
            text,
            status: SubagentResultStatus::PartialTimeout,
            reason: Some(reason),
            ledger_event_id: None,
        }
    }

    fn with_ledger_event_id(mut self, event_id: String) -> Self {
        self.ledger_event_id = Some(event_id);
        self
    }

    pub fn is_partial_timeout(&self) -> bool {
        self.status == SubagentResultStatus::PartialTimeout
    }

    pub fn ledger_event_id(&self) -> Option<&str> {
        self.ledger_event_id.as_deref()
    }
}

struct HiddenSubagentExecutionRequest {
    session_name: String,
    agent_type: String,
    session_config: SessionConfig,
    initial_messages: Vec<Message>,
    created_by: Option<String>,
    subagent_parent_info: Option<SubagentParentInfo>,
    context: HashMap<String, String>,
    runtime_tool_restrictions: ToolRuntimeRestrictions,
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

#[derive(Clone)]
struct SubagentConcurrencyLimiter {
    semaphore: Arc<Semaphore>,
    max_concurrency: usize,
}

struct SubagentConcurrencyPermitGuard {
    permits: Vec<(OwnedSemaphorePermit, SubagentConcurrencyLimiter)>,
    agent_type: String,
}

impl SubagentConcurrencyPermitGuard {
    fn new(
        permits: Vec<(OwnedSemaphorePermit, SubagentConcurrencyLimiter)>,
        agent_type: String,
    ) -> Self {
        Self {
            permits,
            agent_type,
        }
    }
}

impl Drop for SubagentConcurrencyPermitGuard {
    fn drop(&mut self) {
        for (permit, limiter) in std::mem::take(&mut self.permits) {
            drop(permit);

            let active_subagents = limiter
                .max_concurrency
                .saturating_sub(limiter.semaphore.available_permits());
            debug!(
                "Released subagent concurrency permit: agent_type={}, active_subagents={}, max_concurrency={}",
                self.agent_type, active_subagents, limiter.max_concurrency
            );
        }
    }
}

fn normalize_subagent_max_concurrency(raw: usize) -> usize {
    raw.clamp(1, MAX_SUBAGENT_MAX_CONCURRENCY)
}

/// Actions for dynamically adjusting a subagent's timeout.
#[derive(Debug, Clone)]
pub enum SubagentTimeoutAction {
    /// Disable timeout (run without limit).
    Disable,
    /// Restore timeout using the remaining time captured at disable.
    Restore,
    /// Extend timeout by specified seconds from now.
    Extend { seconds: u64 },
}

/// Shared handle for dynamically adjusting a subagent's timeout deadline.
pub(crate) struct SubagentTimeoutHandle {
    /// watch sender: None = no timeout, Some(instant) = deadline.
    deadline_tx: watch::Sender<Option<Instant>>,
    /// Session ID this handle belongs to.
    #[allow(dead_code)]
    session_id: String,
    /// Original timeout in seconds (for restore calculations).
    original_timeout_seconds: Option<u64>,
    /// Remaining seconds at the moment timeout was disabled.
    remaining_at_pause: std::sync::Mutex<Option<u64>>,
}

impl SubagentTimeoutHandle {
    fn disable_timeout(&self) {
        let remaining = match *self.deadline_tx.borrow() {
            Some(deadline) => {
                let now = Instant::now();
                if deadline > now {
                    deadline.duration_since(now).as_secs()
                } else {
                    0
                }
            }
            None => self.original_timeout_seconds.unwrap_or(0),
        };
        let _ = self.remaining_at_pause.lock().map(|mut guard| {
            *guard = Some(remaining);
        });
        let _ = self.deadline_tx.send(None);
    }

    fn restore_timeout(&self) {
        let remaining = self
            .remaining_at_pause
            .lock()
            .ok()
            .and_then(|guard| *guard)
            .unwrap_or_else(|| self.original_timeout_seconds.unwrap_or(0));
        let new_deadline = Instant::now() + Duration::from_secs(remaining);
        let _ = self.deadline_tx.send(Some(new_deadline));
        let _ = self.remaining_at_pause.lock().map(|mut guard| {
            *guard = None;
        });
    }

    fn extend_timeout(&self, seconds: u64) {
        let new_deadline = Instant::now() + Duration::from_secs(seconds);
        let _ = self.deadline_tx.send(Some(new_deadline));
        let _ = self.remaining_at_pause.lock().map(|mut guard| {
            *guard = None;
        });
    }

    fn apply_action(&self, action: SubagentTimeoutAction) {
        match action {
            SubagentTimeoutAction::Disable => self.disable_timeout(),
            SubagentTimeoutAction::Restore => self.restore_timeout(),
            SubagentTimeoutAction::Extend { seconds } => self.extend_timeout(seconds),
        }
    }
}

/// Conversation coordinator
pub struct ConversationCoordinator {
    session_manager: Arc<SessionManager>,
    execution_engine: Arc<ExecutionEngine>,
    tool_pipeline: Arc<ToolPipeline>,
    event_queue: Arc<EventQueue>,
    event_router: Arc<EventRouter>,
    subagent_concurrency_limiter: Arc<RwLock<Option<SubagentConcurrencyLimiter>>>,
    subagent_profile_concurrency_limiters: Arc<RwLock<HashMap<usize, SubagentConcurrencyLimiter>>>,
    /// Registry for dynamically adjusting subagent timeouts.
    subagent_timeout_registry: Arc<RwLock<HashMap<String, Arc<SubagentTimeoutHandle>>>>,
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
            let binding = WorkspaceBinding::new_remote(
                None,
                path_buf,
                rid.to_string(),
                connection_name,
                identity,
            );

            return Some(binding);
        }

        let binding = WorkspaceBinding::new(None, path_buf);

        Some(binding)
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
            subagent_concurrency_limiter: Arc::new(RwLock::new(None)),
            subagent_profile_concurrency_limiters: Arc::new(RwLock::new(HashMap::new())),
            subagent_timeout_registry: Arc::new(RwLock::new(HashMap::new())),
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

    /// Dynamically adjust a running subagent's timeout.
    pub async fn set_subagent_timeout(
        &self,
        session_id: &str,
        action: SubagentTimeoutAction,
    ) -> BitFunResult<()> {
        let registry = self.subagent_timeout_registry.read().await;
        let handle = registry.get(session_id).cloned().ok_or_else(|| {
            BitFunError::tool(format!(
                "No active subagent timeout handle for session {}",
                session_id
            ))
        })?;
        drop(registry);
        handle.apply_action(action.clone());
        info!(
            "Subagent timeout adjusted: session_id={}, action={:?}",
            session_id,
            std::mem::discriminant(&action)
        );
        Ok(())
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
                deep_review_run_manifest: None,
                workspace_path: Some(workspace_path.to_string()),
                workspace_hostname: None,
                unread_completion: None,
                needs_user_attention: None,
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

    /// Create a hidden subagent session for internal AI execution.
    /// Unlike `create_session`, this does NOT emit `SessionCreated` to the transport layer,
    /// because hidden child sessions are internal implementation details of the execution engine
    /// and must never appear as top-level items in the UI.
    async fn create_hidden_subagent_session(
        &self,
        session_id: Option<String>,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
        created_by: Option<String>,
    ) -> BitFunResult<Session> {
        self.session_manager
            .create_session_with_id_and_details(
                session_id,
                session_name,
                agent_type,
                config,
                created_by,
                SessionKind::Subagent,
            )
            .await
    }

    async fn load_session_context_messages(&self, session: &Session) -> BitFunResult<Vec<Message>> {
        let session_id = &session.session_id;
        let mut context_messages = self
            .session_manager
            .get_context_messages(session_id)
            .await?;

        if context_messages.is_empty() && !session.dialog_turn_ids.is_empty() {
            if let Some(workspace_path) = session.config.workspace_path.as_deref() {
                match self
                    .session_manager
                    .restore_session(Path::new(workspace_path), session_id)
                    .await
                {
                    Ok(_) => {
                        context_messages = self
                            .session_manager
                            .get_context_messages(session_id)
                            .await?;
                    }
                    Err(e) => {
                        debug!(
                            "Failed to restore parent session context for fork capture: session_id={}, error={}",
                            session_id, e
                        );
                    }
                }
            }
        }

        Ok(context_messages)
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
    pub async fn start_dialog_turn(
        &self,
        session_id: String,
        user_input: String,
        original_user_input: Option<String>,
        turn_id: Option<String>,
        agent_type: String,
        workspace_path: Option<String>,
        submission_policy: DialogSubmissionPolicy,
        user_message_metadata: Option<serde_json::Value>,
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
            user_message_metadata,
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
        user_message_metadata: Option<serde_json::Value>,
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
            user_message_metadata,
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
                    partial_recovery_reason: None,
                    success: Some(true),
                    finish_reason: Some("complete".to_string()),
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
                    error_category: Some(err.error_category()),
                    error_detail: Some(err.error_detail()),
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
                    session_id, current_turn_id, phase
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
                        session_id, e
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
            session_workspace
                .as_ref()
                .map(|ws| ws.is_remote())
                .unwrap_or(false),
            if workspace_services.is_some() {
                "available"
            } else {
                "NONE"
            }
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
        if let Some(run_manifest) = user_message_metadata.as_ref().and_then(|metadata| {
            metadata
                .get("deepReviewRunManifest")
                .or_else(|| metadata.get("deep_review_run_manifest"))
        }) {
            context_vars.insert(
                "deep_review_run_manifest".to_string(),
                run_manifest.to_string(),
            );
        }
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
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services,
            round_preempt: self.round_preempt_source.get().cloned(),
            recover_partial_on_cancel: false,
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
                                    error_category: Some(e.error_category()),
                                    error_detail: Some(e.error_detail()),
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

            let should_finalize_in_workspace =
                session_manager.should_persist_session_id(&session_id_clone);

            if should_finalize_in_workspace {
                if let (Some(ref wp), Some(status)) =
                    (&session_workspace_path, workspace_turn_status)
                {
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

    async fn get_subagent_concurrency_limiter(&self) -> SubagentConcurrencyLimiter {
        let configured = match GlobalConfigManager::get_service().await {
            Ok(config_service) => match config_service
                .get_config::<usize>(Some("ai.subagent_max_concurrency"))
                .await
            {
                Ok(value) => value,
                Err(error) => {
                    warn!(
                        "Failed to read ai.subagent_max_concurrency, using default {}: {}",
                        DEFAULT_SUBAGENT_MAX_CONCURRENCY, error
                    );
                    DEFAULT_SUBAGENT_MAX_CONCURRENCY
                }
            },
            Err(error) => {
                warn!(
                    "Config service unavailable while reading ai.subagent_max_concurrency, using default {}: {}",
                    DEFAULT_SUBAGENT_MAX_CONCURRENCY, error
                );
                DEFAULT_SUBAGENT_MAX_CONCURRENCY
            }
        };

        let normalized = normalize_subagent_max_concurrency(configured);
        if normalized != configured {
            warn!(
                "Normalized ai.subagent_max_concurrency from {} to {}",
                configured, normalized
            );
        }

        {
            let limiter_guard = self.subagent_concurrency_limiter.read().await;
            if let Some(limiter) = limiter_guard.as_ref() {
                if limiter.max_concurrency == normalized {
                    return limiter.clone();
                }
            }
        }

        let mut limiter_guard = self.subagent_concurrency_limiter.write().await;
        if let Some(limiter) = limiter_guard.as_ref() {
            if limiter.max_concurrency == normalized {
                return limiter.clone();
            }
        }

        let limiter = SubagentConcurrencyLimiter {
            semaphore: Arc::new(Semaphore::new(normalized)),
            max_concurrency: normalized,
        };
        *limiter_guard = Some(limiter.clone());
        limiter
    }

    async fn get_subagent_profile_concurrency_limiter(
        &self,
        max_concurrency: usize,
    ) -> SubagentConcurrencyLimiter {
        let max_concurrency = normalize_subagent_max_concurrency(max_concurrency);

        {
            let limiter_guard = self.subagent_profile_concurrency_limiters.read().await;
            if let Some(limiter) = limiter_guard.get(&max_concurrency) {
                return limiter.clone();
            }
        }

        let mut limiter_guard = self.subagent_profile_concurrency_limiters.write().await;
        if let Some(limiter) = limiter_guard.get(&max_concurrency) {
            return limiter.clone();
        }

        let limiter = SubagentConcurrencyLimiter {
            semaphore: Arc::new(Semaphore::new(max_concurrency)),
            max_concurrency,
        };
        limiter_guard.insert(max_concurrency, limiter.clone());
        limiter
    }

    async fn acquire_permit_from_limiter(
        &self,
        limiter: &SubagentConcurrencyLimiter,
        agent_type: &str,
        cancel_token: Option<&CancellationToken>,
        deadline: Option<Instant>,
        label: &str,
    ) -> BitFunResult<OwnedSemaphorePermit> {
        let semaphore = limiter.semaphore.clone();
        let permit = match (cancel_token, deadline) {
            (Some(token), Some(deadline)) => {
                tokio::select! {
                    result = semaphore.acquire_owned() => result?,
                    _ = token.cancelled() => {
                        return Err(BitFunError::Cancelled(
                            "Subagent task was cancelled while waiting for a concurrency slot".to_string(),
                        ));
                    }
                    _ = tokio::time::sleep_until(deadline) => {
                        return Err(BitFunError::Timeout(format!(
                            "Timed out while waiting for a {} concurrency slot for subagent '{}'",
                            label, agent_type
                        )));
                    }
                }
            }
            (Some(token), None) => {
                tokio::select! {
                    result = semaphore.acquire_owned() => result?,
                    _ = token.cancelled() => {
                        return Err(BitFunError::Cancelled(
                            "Subagent task was cancelled while waiting for a concurrency slot".to_string(),
                        ));
                    }
                }
            }
            (None, Some(deadline)) => {
                tokio::select! {
                    result = semaphore.acquire_owned() => result?,
                    _ = tokio::time::sleep_until(deadline) => {
                        return Err(BitFunError::Timeout(format!(
                            "Timed out while waiting for a {} concurrency slot for subagent '{}'",
                            label, agent_type
                        )));
                    }
                }
            }
            (None, None) => semaphore.acquire_owned().await?,
        };

        let active_subagents = limiter
            .max_concurrency
            .saturating_sub(limiter.semaphore.available_permits());
        debug!(
            "Acquired subagent {} concurrency permit: agent_type={}, active_subagents={}, max_concurrency={}",
            label, agent_type, active_subagents, limiter.max_concurrency
        );

        Ok(permit)
    }

    async fn acquire_subagent_concurrency_permit(
        &self,
        agent_type: &str,
        profile_concurrency_cap: usize,
        cancel_token: Option<&CancellationToken>,
        deadline: Option<Instant>,
    ) -> BitFunResult<(
        Vec<(OwnedSemaphorePermit, SubagentConcurrencyLimiter)>,
        u128,
    )> {
        let started_waiting = Instant::now();

        let profile_limiter = self
            .get_subagent_profile_concurrency_limiter(profile_concurrency_cap)
            .await;
        let profile_permit = self
            .acquire_permit_from_limiter(
                &profile_limiter,
                agent_type,
                cancel_token,
                deadline,
                "profile",
            )
            .await?;

        let global_limiter = self.get_subagent_concurrency_limiter().await;
        let global_permit = self
            .acquire_permit_from_limiter(
                &global_limiter,
                agent_type,
                cancel_token,
                deadline,
                "global",
            )
            .await?;

        let wait_ms = started_waiting.elapsed().as_millis();
        debug!(
            "Acquired subagent concurrency permits: agent_type={}, wait_ms={}, profile_max_concurrency={}, global_max_concurrency={}",
            agent_type, wait_ms, profile_limiter.max_concurrency, global_limiter.max_concurrency
        );

        Ok((
            vec![
                (profile_permit, profile_limiter),
                (global_permit, global_limiter),
            ],
            wait_ms,
        ))
    }

    fn context_profile_policy_for_subagent(
        &self,
        agent_type: &str,
        session_config: &SessionConfig,
        subagent_parent_info: Option<&SubagentParentInfo>,
    ) -> ContextProfilePolicy {
        if let Some(parent_info) = subagent_parent_info {
            if let Some(parent_session) = self.session_manager.get_session(&parent_info.session_id)
            {
                let parent_is_review_subagent = get_agent_registry()
                    .get_subagent_is_review(&parent_session.agent_type)
                    .unwrap_or(false);
                let is_review_subagent = get_agent_registry()
                    .get_subagent_is_review(agent_type)
                    .unwrap_or(false);
                return ContextProfilePolicy::for_subagent_context_and_models(
                    agent_type,
                    is_review_subagent,
                    session_config.model_id.as_deref(),
                    Some(&parent_session.agent_type),
                    parent_is_review_subagent,
                    parent_session.config.model_id.as_deref(),
                );
            }
        }

        let is_review_subagent = get_agent_registry()
            .get_subagent_is_review(agent_type)
            .unwrap_or(false);
        let model_id = session_config.model_id.as_deref().unwrap_or_default();
        ContextProfilePolicy::for_agent_context_and_model(
            agent_type,
            is_review_subagent,
            model_id,
            model_id,
        )
    }

    async fn execute_hidden_subagent_internal(
        &self,
        request: HiddenSubagentExecutionRequest,
        cancel_token: Option<&CancellationToken>,
        timeout_seconds: Option<u64>,
    ) -> BitFunResult<SubagentResult> {
        let HiddenSubagentExecutionRequest {
            session_name,
            agent_type,
            session_config,
            initial_messages,
            created_by,
            subagent_parent_info,
            context,
            runtime_tool_restrictions,
        } = request;

        let timeout_seconds = timeout_seconds.filter(|seconds| *seconds > 0);
        let timeout_error_message = match timeout_seconds {
            Some(seconds) => format!(
                "Subagent '{}' timed out after {} seconds",
                agent_type, seconds
            ),
            None => format!("Subagent '{}' timed out", agent_type),
        };

        // Create dynamic deadline via watch channel so it can be adjusted at runtime.
        let initial_deadline =
            timeout_seconds.map(|seconds| Instant::now() + Duration::from_secs(seconds));
        let (deadline_tx, mut deadline_rx) = watch::channel(initial_deadline);

        let context_profile_policy = self.context_profile_policy_for_subagent(
            &agent_type,
            &session_config,
            subagent_parent_info.as_ref(),
        );
        debug!(
            "Subagent context profile policy selected: agent_type={}, profile={:?}, profile_concurrency_cap={}",
            agent_type,
            context_profile_policy.profile,
            context_profile_policy.subagent_concurrency_cap
        );

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
        let (permits, wait_ms) = self
            .acquire_subagent_concurrency_permit(
                &agent_type,
                context_profile_policy.subagent_concurrency_cap,
                cancel_token,
                initial_deadline,
            )
            .await?;
        let _permit_guard = SubagentConcurrencyPermitGuard::new(permits, agent_type.clone());

        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                debug!(
                    "Subagent task cancelled after waiting for concurrency slot: agent_type={}",
                    agent_type
                );
                return Err(BitFunError::Cancelled(
                    "Subagent task has been cancelled".to_string(),
                ));
            }
        }
        if initial_deadline.is_some_and(|expires_at| Instant::now() >= expires_at) {
            warn!(
                "Subagent timed out before session creation after waiting for concurrency slot: agent_type={}, wait_ms={}",
                agent_type, wait_ms
            );
            return Err(BitFunError::Timeout(timeout_error_message.clone()));
        }

        let session = self
            .create_hidden_subagent_session(
                None,
                session_name,
                agent_type.clone(),
                session_config,
                created_by,
            )
            .await?;
        let session_id = session.session_id.clone();

        // Register timeout handle so it can be adjusted at runtime.
        let timeout_handle = Arc::new(SubagentTimeoutHandle {
            deadline_tx: deadline_tx.clone(),
            session_id: session_id.clone(),
            original_timeout_seconds: timeout_seconds,
            remaining_at_pause: std::sync::Mutex::new(None),
        });
        {
            let mut registry = self.subagent_timeout_registry.write().await;
            registry.insert(session_id.clone(), timeout_handle);
        }

        // Check cancel token (after creating session, before execution)
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                debug!("Subagent task cancelled before AI call, cleaning up resources");
                let _ = self.cleanup_subagent_resources(&session_id).await;
                let mut registry = self.subagent_timeout_registry.write().await;
                registry.remove(&session_id);
                return Err(BitFunError::Cancelled(
                    "Subagent task has been cancelled".to_string(),
                ));
            }
        }
        if initial_deadline.is_some_and(|expires_at| Instant::now() >= expires_at) {
            warn!(
                "Subagent timed out before AI call after session creation: agent_type={}, session={}, wait_ms={}",
                agent_type, session_id, wait_ms
            );
            let _ = self.cleanup_subagent_resources(&session_id).await;
            let mut registry = self.subagent_timeout_registry.write().await;
            registry.remove(&session_id);
            return Err(BitFunError::Timeout(timeout_error_message.clone()));
        }

        // Generate unique dialog_turn_id for cancel token management
        let dialog_turn_id = format!("subagent-{}", uuid::Uuid::new_v4());
        debug!(
            "Generated unique dialog_turn_id for subagent: {}",
            dialog_turn_id
        );

        // Register a dedicated subagent token so both external cancellation and
        // coordinator-enforced timeouts can stop the same dialog turn.
        let subagent_cancel_token = cancel_token
            .map(CancellationToken::child_token)
            .unwrap_or_else(CancellationToken::new);
        self.execution_engine
            .register_cancel_token(&dialog_turn_id, subagent_cancel_token.clone());

        debug!(
            "Registered cancel token to RoundExecutor: dialog_turn_id={}",
            dialog_turn_id
        );

        let _cleanup_guard = CancelTokenGuard {
            execution_engine: self.execution_engine.clone(),
            dialog_turn_id: dialog_turn_id.clone(),
        };

        // Emit DialogTurnStarted with subagent_parent_info so the frontend can
        // associate the subagent session ID with the parent tool (enabling the
        // "ignore timeout" feature for deep-review subagents).
        let user_input_text = initial_messages
            .first()
            .map(|m| match &m.content {
                MessageContent::Text(text) => text.clone(),
                _ => String::new(),
            })
            .unwrap_or_default();
        self.emit_event(AgenticEvent::DialogTurnStarted {
            session_id: session_id.clone(),
            turn_id: dialog_turn_id.clone(),
            turn_index: 0,
            user_input: user_input_text,
            original_user_input: None,
            user_message_metadata: None,
            subagent_parent_info: subagent_parent_info.clone().map(Into::into),
        })
        .await;

        let subagent_workspace = Self::build_workspace_binding(&session.config).await;
        let subagent_services = Self::build_workspace_services(&subagent_workspace).await;
        let execution_context = ExecutionContext {
            session_id: session_id.clone(),
            dialog_turn_id: dialog_turn_id.clone(),
            turn_index: 0,
            agent_type: agent_type.clone(),
            workspace: subagent_workspace,
            context,
            subagent_parent_info: subagent_parent_info.clone(),
            // Subagents run autonomously without user interaction; always skip
            // tool confirmation to prevent them from blocking indefinitely on a
            // confirmation channel that nobody will ever respond to.
            skip_tool_confirmation: true,
            runtime_tool_restrictions,
            workspace_services: subagent_services,
            round_preempt: self.round_preempt_source.get().cloned(),
            recover_partial_on_cancel: true,
        };

        let execution_engine = self.execution_engine.clone();
        let tool_pipeline = self.tool_pipeline.clone();
        let agent_type_for_execution = agent_type.clone();
        let mut execution_task = tokio::spawn(async move {
            execution_engine
                .execute_dialog_turn(
                    agent_type_for_execution,
                    initial_messages,
                    execution_context,
                )
                .await
        });

        enum SubagentExecutionOutcome<T> {
            Completed(T),
            Cancelled,
            TimedOut,
        }

        // Dynamic timeout loop: deadline can be adjusted via watch channel.
        let execution_outcome = loop {
            let current_deadline = *deadline_rx.borrow();
            match current_deadline {
                Some(expires_at) if Instant::now() >= expires_at => {
                    break SubagentExecutionOutcome::TimedOut;
                }
                Some(expires_at) => {
                    let sleep = tokio::time::sleep_until(expires_at);
                    tokio::pin!(sleep);
                    tokio::select! {
                        join_result = &mut execution_task => {
                            break SubagentExecutionOutcome::Completed(join_result);
                        }
                        _ = subagent_cancel_token.cancelled() => {
                            break SubagentExecutionOutcome::Cancelled;
                        }
                        _ = &mut sleep => {
                            // Sleep expired; check if deadline was updated.
                            continue;
                        }
                        _ = deadline_rx.changed() => {
                            // Deadline changed externally; re-evaluate.
                            // If sender was dropped, treat as no timeout and
                            // let execution_task/cancel_token branches handle it.
                            continue;
                        }
                    }
                }
                None => {
                    // No timeout (disabled).
                    tokio::select! {
                        join_result = &mut execution_task => {
                            break SubagentExecutionOutcome::Completed(join_result);
                        }
                        _ = subagent_cancel_token.cancelled() => {
                            break SubagentExecutionOutcome::Cancelled;
                        }
                        _ = deadline_rx.changed() => {
                            // Deadline was set; re-evaluate.
                            // If sender was dropped, remain in no-timeout mode
                            // and let execution_task/cancel_token branches handle it.
                            continue;
                        }
                    }
                }
            }
        };

        let result = match execution_outcome {
            SubagentExecutionOutcome::Completed(join_result) => match join_result {
                Ok(result) => result,
                Err(error) => {
                    error!(
                        "Subagent execution failed to join: agent_type={}, session={}, error={}",
                        agent_type, session_id, error
                    );

                    if let Err(cleanup_err) = self.cleanup_subagent_resources(&session_id).await {
                        warn!(
                            "Failed to cleanup subagent resources after join failure: session={}, error={}",
                            session_id, cleanup_err
                        );
                    }
                    let mut registry = self.subagent_timeout_registry.write().await;
                    registry.remove(&session_id);

                    return Err(BitFunError::tool(format!(
                        "Subagent '{}' failed to join: {}",
                        agent_type, error
                    )));
                }
            },
            SubagentExecutionOutcome::Cancelled => {
                warn!(
                    "Stopping subagent execution after cancellation: agent_type={}, session={}, dialog_turn_id={}",
                    agent_type, session_id, dialog_turn_id
                );
                subagent_cancel_token.cancel();

                if let Err(error) = self
                    .execution_engine
                    .cancel_dialog_turn(&dialog_turn_id)
                    .await
                {
                    warn!(
                        "Failed to cancel subagent dialog turn after cancellation: dialog_turn_id={}, error={}",
                        dialog_turn_id, error
                    );
                }

                if let Err(error) = tool_pipeline
                    .cancel_dialog_turn_tools(&dialog_turn_id)
                    .await
                {
                    warn!(
                        "Failed to cancel subagent tools after cancellation: dialog_turn_id={}, error={}",
                        dialog_turn_id, error
                    );
                }

                match tokio::time::timeout(SUBAGENT_TIMEOUT_GRACE_PERIOD, &mut execution_task).await
                {
                    Ok(Ok(Ok(_))) | Ok(Ok(Err(_))) => {}
                    Ok(Err(error)) => {
                        warn!(
                            "Subagent join failed during cancellation grace period: agent_type={}, session={}, error={}",
                            agent_type, session_id, error
                        );
                        execution_task.abort();
                    }
                    Err(_) => {
                        warn!(
                            "Subagent did not stop within cancellation grace period, aborting task: agent_type={}, session={}",
                            agent_type, session_id
                        );
                        execution_task.abort();
                    }
                }

                if let Err(cleanup_err) = self.cleanup_subagent_resources(&session_id).await {
                    warn!(
                        "Failed to cleanup subagent resources after cancellation: session={}, error={}",
                        session_id, cleanup_err
                    );
                }
                let mut registry = self.subagent_timeout_registry.write().await;
                registry.remove(&session_id);

                return Err(BitFunError::Cancelled(
                    "Subagent task has been cancelled".to_string(),
                ));
            }
            SubagentExecutionOutcome::TimedOut => {
                warn!(
                    "Stopping subagent execution after timeout: agent_type={}, session={}, dialog_turn_id={}",
                    agent_type, session_id, dialog_turn_id
                );
                subagent_cancel_token.cancel();

                if let Err(error) = self
                    .execution_engine
                    .cancel_dialog_turn(&dialog_turn_id)
                    .await
                {
                    warn!(
                        "Failed to cancel subagent dialog turn after timeout: dialog_turn_id={}, error={}",
                        dialog_turn_id, error
                    );
                }

                if let Err(error) = tool_pipeline
                    .cancel_dialog_turn_tools(&dialog_turn_id)
                    .await
                {
                    warn!(
                        "Failed to cancel subagent tools after timeout: dialog_turn_id={}, error={}",
                        dialog_turn_id, error
                    );
                }

                let partial_timeout_result = match tokio::time::timeout(
                    SUBAGENT_TIMEOUT_GRACE_PERIOD,
                    &mut execution_task,
                )
                .await
                {
                    Ok(Ok(Ok(exec_result))) => {
                        let response_text = match exec_result.final_message.content {
                            MessageContent::Mixed { text, .. } => text,
                            MessageContent::Text(text) => text,
                            _ => String::new(),
                        };
                        if response_text.trim().is_empty() {
                            None
                        } else {
                            Some(SubagentResult::partial_timeout(
                                response_text,
                                timeout_error_message.clone(),
                            ))
                        }
                    }
                    Ok(Ok(Err(error))) => {
                        debug!(
                            "Subagent returned error during timeout grace period: agent_type={}, session={}, error={}",
                            agent_type, session_id, error
                        );
                        None
                    }
                    Ok(Err(error)) => {
                        warn!(
                            "Subagent join failed during timeout grace period: agent_type={}, session={}, error={}",
                            agent_type, session_id, error
                        );
                        execution_task.abort();
                        None
                    }
                    Err(_) => {
                        warn!(
                            "Subagent did not stop within timeout grace period, aborting task: agent_type={}, session={}",
                            agent_type, session_id
                        );
                        execution_task.abort();
                        None
                    }
                };

                if let Some(mut partial_result) = partial_timeout_result {
                    warn!(
                        "Subagent timed out with partial output: agent_type={}, session={}, text_len={}",
                        agent_type,
                        session_id,
                        partial_result.text.len()
                    );
                    if let Some(parent_info) = subagent_parent_info.as_ref() {
                        let event = self.session_manager.record_subagent_partial_timeout(
                            &parent_info.session_id,
                            &parent_info.dialog_turn_id,
                            &agent_type,
                            &partial_result.text,
                            Some("timeout"),
                        );
                        partial_result = partial_result.with_ledger_event_id(event.event_id);
                    }
                    if let Err(cleanup_err) = self.cleanup_subagent_resources(&session_id).await {
                        warn!(
                            "Failed to cleanup subagent resources after partial timeout: session={}, error={}",
                            session_id, cleanup_err
                        );
                    }
                    let mut registry = self.subagent_timeout_registry.write().await;
                    registry.remove(&session_id);

                    return Ok(partial_result);
                }

                if let Err(cleanup_err) = self.cleanup_subagent_resources(&session_id).await {
                    warn!(
                        "Failed to cleanup subagent resources after timeout: session={}, error={}",
                        session_id, cleanup_err
                    );
                }
                let mut registry = self.subagent_timeout_registry.write().await;
                registry.remove(&session_id);

                return Err(BitFunError::Timeout(timeout_error_message.clone()));
            }
        };

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
                    session_id, e
                );

                if let Err(cleanup_err) = self.cleanup_subagent_resources(&session_id).await {
                    warn!(
                        "Failed to cleanup subagent resources: session={}, error={}",
                        session_id, cleanup_err
                    );
                }
                let mut registry = self.subagent_timeout_registry.write().await;
                registry.remove(&session_id);

                return Err(e);
            }
        };

        // Clean up subagent session resources after successful execution
        debug!("Starting subagent resource cleanup: session={}", session_id);
        if let Err(e) = self.cleanup_subagent_resources(&session_id).await {
            warn!(
                "Failed to cleanup subagent resources: session={}, error={}",
                session_id, e
            );
        } else {
            debug!(
                "Subagent resource cleanup completed: session={}",
                session_id
            );
        }
        let mut registry = self.subagent_timeout_registry.write().await;
        registry.remove(&session_id);

        Ok(SubagentResult::completed(response_text))
    }

    pub async fn capture_fork_agent_context_snapshot(
        &self,
        parent_session_id: &str,
    ) -> BitFunResult<ForkAgentContextSnapshot> {
        let parent_session = self
            .session_manager
            .get_session(parent_session_id)
            .ok_or_else(|| {
                BitFunError::NotFound(format!("Parent session not found: {}", parent_session_id))
            })?;
        let context_messages = self.load_session_context_messages(&parent_session).await?;
        ForkAgentContextSnapshot::from_parent_session(&parent_session, context_messages)
    }

    async fn ensure_hidden_btw_session(
        &self,
        parent_session_id: &str,
        child_session_id: &str,
        child_session_name: Option<&str>,
    ) -> BitFunResult<Session> {
        if let Some(session) = self.session_manager.get_session(child_session_id) {
            return Ok(session);
        }

        let snapshot = self
            .capture_fork_agent_context_snapshot(parent_session_id)
            .await?;
        let session_name = child_session_name
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or("Side thread")
            .to_string();
        let child_session = self
            .create_hidden_subagent_session(
                Some(child_session_id.to_string()),
                session_name,
                snapshot.parent_agent_type.clone(),
                snapshot.build_child_session_config(None),
                Some(format!("session-{}", snapshot.parent_session_id)),
            )
            .await?;

        self.session_manager
            .replace_context_messages(&child_session.session_id, snapshot.messages)
            .await;

        Ok(child_session)
    }

    pub async fn start_hidden_btw_turn(
        &self,
        request_id: &str,
        parent_session_id: &str,
        child_session_id: &str,
        child_session_name: Option<&str>,
        question: &str,
        model_id: Option<&str>,
    ) -> BitFunResult<String> {
        if request_id.trim().is_empty() {
            return Err(BitFunError::Validation(
                "request_id is required".to_string(),
            ));
        }
        if parent_session_id.trim().is_empty() {
            return Err(BitFunError::Validation(
                "parent_session_id is required".to_string(),
            ));
        }
        if child_session_id.trim().is_empty() {
            return Err(BitFunError::Validation(
                "child_session_id is required".to_string(),
            ));
        }
        if question.trim().is_empty() {
            return Err(BitFunError::Validation("question is required".to_string()));
        }

        let child_session = self
            .ensure_hidden_btw_session(parent_session_id, child_session_id, child_session_name)
            .await?;

        if let Some(model_id) = model_id
            .map(str::trim)
            .filter(|model_id| !model_id.is_empty())
        {
            self.session_manager
                .update_session_model_id(child_session_id, model_id)
                .await?;
        }

        let turn_id = format!("btw-turn-{}", request_id.trim());
        let user_message_metadata = Some(serde_json::json!({
            "kind": "btw",
            "parentSessionId": parent_session_id,
        }));

        self.start_dialog_turn_internal(
            child_session_id.to_string(),
            build_btw_user_input(question),
            Some(question.trim().to_string()),
            None,
            Some(turn_id.clone()),
            child_session.agent_type.clone(),
            child_session.config.workspace_path.clone(),
            DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopApi)
                .with_skip_tool_confirmation(true),
            user_message_metadata,
            true,
        )
        .await?;

        Ok(turn_id)
    }

    /// Execute a hidden child agent that inherits the parent session's current
    /// model-visible context.
    pub async fn execute_fork_agent(
        &self,
        request: ForkAgentExecutionRequest,
        cancel_token: Option<&CancellationToken>,
    ) -> BitFunResult<ForkAgentExecutionResult> {
        if request.agent_type.trim().is_empty() {
            return Err(BitFunError::Validation(
                "ForkAgentExecutionRequest.agent_type is required".to_string(),
            ));
        }
        if request.description.trim().is_empty() {
            return Err(BitFunError::Validation(
                "ForkAgentExecutionRequest.description is required".to_string(),
            ));
        }
        if request.prompt_messages.is_empty() {
            return Err(BitFunError::Validation(
                "ForkAgentExecutionRequest.prompt_messages must not be empty".to_string(),
            ));
        }

        let inherited_message_count = request.snapshot.inherited_message_count();
        let prompt_message_count = request.prompt_messages.len();
        let agent_type = request.agent_type.clone();
        let session_config = request.child_session_config();
        let initial_messages = request.composed_initial_messages();
        let created_by = Some(format!("session-{}", request.snapshot.parent_session_id));
        let child_result = self
            .execute_hidden_subagent_internal(
                HiddenSubagentExecutionRequest {
                    session_name: format!("Fork: {}", request.description),
                    agent_type,
                    session_config,
                    initial_messages,
                    created_by,
                    subagent_parent_info: None,
                    context: request.context,
                    runtime_tool_restrictions: request.runtime_tool_restrictions,
                },
                cancel_token,
                None,
            )
            .await?;

        Ok(ForkAgentExecutionResult {
            text: child_result.text,
            inherited_message_count,
            prompt_message_count,
        })
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
    /// - model_id: Optional model override for the subagent session
    ///
    /// Returns SubagentResult with the final text response
    pub async fn execute_subagent(
        &self,
        agent_type: String,
        task_description: String,
        subagent_parent_info: SubagentParentInfo,
        workspace_path: Option<String>,
        context: Option<HashMap<String, String>>,
        cancel_token: Option<&CancellationToken>,
        model_id: Option<String>,
        timeout_seconds: Option<u64>,
    ) -> BitFunResult<SubagentResult> {
        let workspace_path = workspace_path.ok_or_else(|| {
            BitFunError::Validation(
                "workspace_path is required when creating a subagent session".to_string(),
            )
        })?;
        let model_id = model_id
            .map(|model_id| model_id.trim().to_string())
            .filter(|model_id| !model_id.is_empty());

        self.execute_hidden_subagent_internal(
            HiddenSubagentExecutionRequest {
                session_name: format!("Subagent: {}", task_description),
                agent_type,
                session_config: SessionConfig {
                    workspace_path: Some(workspace_path),
                    model_id,
                    ..SessionConfig::default()
                },
                initial_messages: vec![Message::user(task_description)],
                created_by: Some(format!("session-{}", subagent_parent_info.session_id)),
                subagent_parent_info: Some(subagent_parent_info),
                context: context.unwrap_or_default(),
                runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            },
            cancel_token,
            timeout_seconds,
        )
        .await
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

    pub async fn update_session_agent_type(
        &self,
        session_id: &str,
        agent_type: &str,
    ) -> BitFunResult<()> {
        let normalized = Self::normalize_agent_type(agent_type);
        self.session_manager
            .update_session_agent_type(session_id, &normalized)
            .await
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

#[cfg(test)]
mod tests {
    use super::normalize_subagent_max_concurrency;

    #[test]
    fn clamps_subagent_max_concurrency_into_safe_range() {
        assert_eq!(normalize_subagent_max_concurrency(0), 1);
        assert_eq!(normalize_subagent_max_concurrency(5), 5);
        assert_eq!(normalize_subagent_max_concurrency(usize::MAX), 64);
    }
}
