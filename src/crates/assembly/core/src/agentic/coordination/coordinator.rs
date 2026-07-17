//! Conversation coordinator
//!
//! Top-level component that integrates all subsystems and provides a unified interface

use super::{
    scheduler::{
        abort_thread_goal_continuation_for_session, clear_thread_goal_continuation_abort,
        get_global_scheduler, DialogSubmissionPolicy, HiddenSubagentQueueCancelHandle,
    },
    turn_outcome::TurnOutcome,
};
use crate::agentic::agents::get_agent_registry;
use crate::agentic::context_profile::ContextProfilePolicy;
use crate::agentic::core::{
    InternalReminderKind, Message, MessageContent, MessageSemanticKind, ProcessingPhase, Session,
    SessionConfig, SessionKind, SessionState, SessionSummary, TurnStats,
};
use crate::agentic::events::{
    AgenticEvent, DeepReviewQueueState, EventPriority, EventQueue, EventRouter, EventSubscriber,
};
use crate::agentic::execution::{
    ContextCompactionOutcome, ExecutionContext, ExecutionEngine, ExecutionResult,
};
use crate::agentic::fork_agent::ForkAgentContextSnapshot;
use crate::agentic::goal_mode::{
    effective_subagent_timeout_seconds, is_usage_limit_error, maybe_build_continuation_after_turn,
    should_skip_goal_continuation_after_turn, should_skip_goal_for_turn,
    thread_goal_status_is_resumable, user_facing_thread_goal_error, ThreadGoalRuntime,
    ThreadGoalStore,
};
use crate::agentic::image_analysis::ImageContextData;
use crate::agentic::memories::{start_memory_startup_task, MemoryStartupRequest};
use crate::agentic::round_preempt::DialogRoundInjectionSource;
use crate::agentic::session::session_store_port::CoreSessionStorePort;
use crate::agentic::session::SessionManager;
use crate::agentic::side_question::build_btw_user_input;
use crate::agentic::skill_agent_snapshot::{
    diff_skill_agent_snapshot, resolve_skill_agent_snapshot, TurnSkillAgentSnapshot,
};
use crate::agentic::tools::pipeline::{SubagentParentInfo, ToolPipeline};
use crate::agentic::tools::{
    is_miniapp_headless_agent_run, miniapp_headless_agent_tool_restrictions,
    tool_restrictions_for_delegation_policy as runtime_tool_restrictions_for_delegation_policy,
    ToolRuntimeRestrictions,
};
use crate::agentic::workspace::WorkspaceServices;
use crate::agentic::WorkspaceBinding;
use crate::service::bootstrap::{
    ensure_workspace_persona_files_for_prompt, is_workspace_bootstrap_pending,
};
use crate::service::config::global::GlobalConfigManager;
use crate::service::remote_ssh::normalize_remote_workspace_path;
use crate::service::session::{SessionMemoryMode, SessionRelationship, SessionRelationshipKind};
use crate::service::workspace::{
    get_global_workspace_service, WorkspaceCreateOptions, WorkspaceKind,
};
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_agent_runtime::output_surface::{
    supports_inline_markdown_images_for_source, TOOL_CONTEXT_INLINE_MARKDOWN_IMAGE_DISPLAY_KEY,
};
use bitfun_agent_runtime::remote_file_delivery::{
    needs_computer_links_for_source, remote_file_delivery_reminder,
    TOOL_CONTEXT_REMOTE_FILE_DELIVERY_KEY,
};
use bitfun_agent_runtime::user_questions::USER_INPUT_AVAILABLE_CONTEXT_KEY;
use bitfun_runtime_ports::{
    AgentBackgroundResultRequest, AgentSessionWorkspaceBinding, AgentThreadGoalDeliveryKind,
    AgentThreadGoalDeliveryRequest, DelegationPolicy, RemoteExecPort, SessionStoragePathRequest,
    SessionStorePort, SubagentContextMode, TerminalPort, ThreadGoal, ThreadGoalContinuationPlan,
    ThreadGoalStatus,
};
use dashmap::DashMap;
use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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

fn is_review_agent_type(agent_type: &str) -> bool {
    matches!(
        agent_type.to_ascii_lowercase().as_str(),
        "codereview" | "deepreview"
    )
}

pub(crate) async fn validate_background_subagent_delivery(
    agent_type: &str,
    workspace_root: Option<&Path>,
    allow_review_follow_up: bool,
) -> BitFunResult<()> {
    let is_review = get_agent_registry()
        .get_subagent_is_review_for_workspace(agent_type, workspace_root)
        .await
        .unwrap_or(false);
    if !is_review || allow_review_follow_up {
        return Ok(());
    }

    Err(BitFunError::Validation(
        "Reviews wait for results by default so one final review can be returned. Retry without run_in_background. Only when the user explicitly asked not to wait, retry with run_in_background=true and allow_review_follow_up=true."
            .to_string(),
    ))
}

fn turn_review_manifest_for_agent(
    metadata: Option<&serde_json::Value>,
    agent_type: &str,
) -> Option<serde_json::Value> {
    if !is_review_agent_type(agent_type) {
        return None;
    }
    metadata
        .and_then(|metadata| {
            metadata
                .get("deepReviewRunManifest")
                .or_else(|| metadata.get("deep_review_run_manifest"))
        })
        .cloned()
}

fn metadata_bool(metadata: Option<&serde_json::Value>, key: &str) -> Option<bool> {
    metadata
        .and_then(|metadata| metadata.get(key))
        .and_then(serde_json::Value::as_bool)
}

fn should_require_tool_confirmation(
    policy: DialogSubmissionPolicy,
    user_message_metadata: Option<&serde_json::Value>,
) -> bool {
    (policy.requires_tool_confirmation()
        || metadata_bool(user_message_metadata, "require_tool_confirmation") == Some(true))
        && metadata_bool(user_message_metadata, "acp_transport") != Some(true)
}

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
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentResultStatus {
    Completed,
    PartialTimeout,
}

#[derive(Debug, Clone)]
pub(crate) struct SubagentExecutionRequest {
    pub(crate) task_description: String,
    pub(crate) context_mode: SubagentContextMode,
    pub(crate) target_session_id: Option<String>,
    pub(crate) subagent_type: Option<String>,
    pub(crate) workspace_path: Option<String>,
    pub(crate) model_id: Option<String>,
    pub(crate) subagent_parent_info: SubagentParentInfo,
    pub(crate) context: HashMap<String, String>,
    /// Execution policy for the child subagent session being launched.
    pub(crate) delegation_policy: DelegationPolicy,
}

#[derive(Debug, Clone)]
pub(crate) struct InternalAgentExecutionRequest {
    pub(crate) task_description: String,
    pub(crate) agent_type: String,
    pub(crate) session_name: String,
    pub(crate) workspace_path: String,
    pub(crate) model_id: Option<String>,
    pub(crate) created_by: Option<String>,
    pub(crate) context: HashMap<String, String>,
    pub(crate) delegation_policy: DelegationPolicy,
    pub(crate) runtime_tool_restrictions: ToolRuntimeRestrictions,
    pub(crate) session_kind: SessionKind,
    pub(crate) emit_lifecycle_events: bool,
}

struct WrappedUserInputPayload {
    content: String,
    prepended_messages: Vec<Message>,
    skill_agent_snapshot: TurnSkillAgentSnapshot,
    snapshot_persistence: SkillAgentSnapshotPersistence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillAgentSnapshotPersistence {
    None,
    SaveCurrentTurn,
    RecoverFirstTurnBaseline,
}

impl SubagentResult {
    fn completed(text: String) -> Self {
        Self {
            text,
            status: SubagentResultStatus::Completed,
            reason: None,
            ledger_event_id: None,
            session_id: None,
        }
    }

    fn partial_timeout(text: String, reason: String) -> Self {
        Self {
            text,
            status: SubagentResultStatus::PartialTimeout,
            reason: Some(reason),
            ledger_event_id: None,
            session_id: None,
        }
    }

    fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
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

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }
}

#[derive(Debug, Clone)]
pub struct BackgroundSubagentStartResult {
    pub background_task_id: String,
    pub session_id: String,
}

fn format_background_subagent_delivery_text(
    subagent_session_id: &str,
    outcome: Result<&SubagentResult, &BitFunError>,
) -> String {
    match outcome {
        Ok(result) => {
            if result.is_partial_timeout() {
                format!(
                    "Background subagent (session_id='{}') completed with partial timeout result:\n<partial_result status=\"partial_timeout\">\n{}\n</partial_result>",
                    subagent_session_id, result.text
                )
            } else {
                format!(
                    "Background subagent (session_id='{}') completed successfully:\n<result>\n{}\n</result>",
                    subagent_session_id, result.text
                )
            }
        }
        Err(error) => {
            format!(
                "Background subagent (session_id='{}') failed before producing a final result.\nError: {}",
                subagent_session_id, error
            )
        }
    }
}

fn format_background_subagent_display_text(
    outcome: Result<&SubagentResult, &BitFunError>,
) -> String {
    match outcome {
        Ok(result) => {
            if result.is_partial_timeout() {
                "Background subagent completed with a partial timeout result.".to_string()
            } else {
                "Background subagent completed successfully.".to_string()
            }
        }
        Err(_) => "Background subagent failed before producing a final result.".to_string(),
    }
}

fn background_subagent_delivery_metadata(
    background_task_id: &str,
    parent: &SubagentParentInfo,
    subagent_session_id: &str,
    subagent_dialog_turn_id: &str,
    require_tool_confirmation: bool,
    agent_type: &str,
    task_description: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let mut metadata = serde_json::Map::from_iter([
        ("kind".to_string(), serde_json::json!("background_result")),
        ("sourceKind".to_string(), serde_json::json!("subagent")),
        (
            "backgroundTaskId".to_string(),
            serde_json::json!(background_task_id),
        ),
        (
            "parentSessionId".to_string(),
            serde_json::json!(parent.session_id.as_str()),
        ),
        (
            "parentDialogTurnId".to_string(),
            serde_json::json!(parent.dialog_turn_id.as_str()),
        ),
        (
            "subagentSessionId".to_string(),
            serde_json::json!(subagent_session_id),
        ),
        (
            "subagentDialogTurnId".to_string(),
            serde_json::json!(subagent_dialog_turn_id),
        ),
        ("subagentType".to_string(), serde_json::json!(agent_type)),
        (
            "taskDescription".to_string(),
            serde_json::json!(task_description),
        ),
    ]);
    if require_tool_confirmation {
        metadata.insert(
            "require_tool_confirmation".to_string(),
            serde_json::Value::Bool(true),
        );
    }
    metadata
}

fn build_subagent_session_relationship(
    parent_info: Option<&SubagentParentInfo>,
    agent_type: &str,
) -> SessionRelationship {
    SessionRelationship {
        kind: Some(SessionRelationshipKind::Subagent),
        parent_session_id: parent_info.map(|info| info.session_id.clone()),
        parent_request_id: None,
        parent_dialog_turn_id: parent_info.map(|info| info.dialog_turn_id.clone()),
        parent_turn_index: None,
        parent_tool_call_id: parent_info.map(|info| info.tool_call_id.clone()),
        subagent_type: Some(agent_type.to_string()),
    }
}

fn fork_subagent_system_reminder() -> String {
    "<system_reminder>You are now running as a forked subagent. Messages before this reminder were inherited from the parent agent as context. Messages after this reminder are the request for you. Do not call the Task tool to launch another subagent. Use the tools available to complete the task directly.</system_reminder>".to_string()
}

fn session_created_by_parent(session: &Session, parent_session_id: &str) -> bool {
    let created_by_marker = format!("session-{}", parent_session_id);
    session.created_by.as_deref() == Some(created_by_marker.as_str())
}

fn session_lineage_matches_parent(
    relationship: Option<&SessionRelationship>,
    parent_session_id: &str,
) -> bool {
    relationship.is_some_and(|relationship| {
        relationship.kind == Some(SessionRelationshipKind::Subagent)
            && relationship.parent_session_id.as_deref() == Some(parent_session_id)
    })
}

#[derive(Debug, Clone)]
pub(crate) struct HiddenSubagentExecutionRequest {
    target_session_id: Option<String>,
    dialog_turn_id: Option<String>,
    session_name: String,
    agent_type: String,
    session_config: SessionConfig,
    initial_messages: Vec<Message>,
    user_input_text: String,
    created_by: Option<String>,
    subagent_parent_info: Option<SubagentParentInfo>,
    context: HashMap<String, String>,
    delegation_policy: DelegationPolicy,
    runtime_tool_restrictions: ToolRuntimeRestrictions,
    prompt_cache_source_session_id: Option<String>,
    session_kind: SessionKind,
    emit_lifecycle_events: bool,
    prepared_session_created: bool,
}

impl HiddenSubagentExecutionRequest {
    pub(super) fn target_session_id(&self) -> Option<&str> {
        self.target_session_id.as_deref()
    }

    pub(super) fn set_dialog_turn_id(&mut self, dialog_turn_id: String) {
        self.dialog_turn_id = Some(dialog_turn_id);
    }

    pub(super) fn agent_type(&self) -> &str {
        &self.agent_type
    }

    pub(super) fn user_input_text(&self) -> &str {
        &self.user_input_text
    }

    pub(super) fn parent_dialog_turn_id(&self) -> Option<&str> {
        self.subagent_parent_info
            .as_ref()
            .map(|info| info.dialog_turn_id.as_str())
    }

    fn prepared_session_id_created_by_this_request(&self) -> Option<&str> {
        self.prepared_session_created
            .then_some(self.target_session_id.as_deref())
            .flatten()
    }
}

pub use bitfun_runtime_ports::DialogTriggerSource;

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
struct ActiveSubagentExecution {
    parent_session_id: String,
    parent_dialog_turn_id: String,
    subagent_session_id: String,
    subagent_dialog_turn_id: String,
    cancel_token: CancellationToken,
    abort_handle: tokio::task::AbortHandle,
}

#[derive(Clone)]
enum BackgroundSubagentCancelTarget {
    Scheduler(HiddenSubagentQueueCancelHandle),
    Direct(CancellationToken),
}

#[derive(Clone)]
struct BackgroundSubagentTaskControl {
    parent_session_id: String,
    subagent_session_id: String,
    suppress_delivery: Arc<AtomicBool>,
    cancel_target: BackgroundSubagentCancelTarget,
}

/// Ensures orphaned subagent work is stopped when the parent tool await is dropped.
struct SubagentExecutionScope {
    execution_engine: Arc<ExecutionEngine>,
    tool_pipeline: Arc<ToolPipeline>,
    session_manager: Arc<SessionManager>,
    active_subagent_executions: Arc<DashMap<String, ActiveSubagentExecution>>,
    subagent_session_id: String,
    subagent_dialog_turn_id: String,
    subagent_cancel_token: CancellationToken,
    abort_handle: tokio::task::AbortHandle,
    disarmed: bool,
}

impl SubagentExecutionScope {
    fn disarm(&mut self) {
        self.disarmed = true;
        self.active_subagent_executions
            .remove(&self.subagent_session_id);
    }
}

impl Drop for SubagentExecutionScope {
    fn drop(&mut self) {
        if self.disarmed {
            return;
        }

        warn!(
            "Subagent execution scope dropped without normal completion; stopping orphaned subagent: session_id={}, dialog_turn_id={}",
            self.subagent_session_id, self.subagent_dialog_turn_id
        );

        self.subagent_cancel_token.cancel();
        self.abort_handle.abort();
        self.active_subagent_executions
            .remove(&self.subagent_session_id);

        let execution_engine = self.execution_engine.clone();
        let tool_pipeline = self.tool_pipeline.clone();
        let session_manager = self.session_manager.clone();
        let subagent_session_id = self.subagent_session_id.clone();
        let subagent_dialog_turn_id = self.subagent_dialog_turn_id.clone();

        tokio::spawn(async move {
            if let Err(error) = execution_engine
                .cancel_dialog_turn(&subagent_dialog_turn_id)
                .await
            {
                warn!(
                    "Failed to cancel orphaned subagent dialog turn: session_id={}, dialog_turn_id={}, error={}",
                    subagent_session_id, subagent_dialog_turn_id, error
                );
            }

            if let Err(error) = tool_pipeline
                .cancel_dialog_turn_tools(&subagent_dialog_turn_id)
                .await
            {
                warn!(
                    "Failed to cancel orphaned subagent tools: session_id={}, dialog_turn_id={}, error={}",
                    subagent_session_id, subagent_dialog_turn_id, error
                );
            }

            session_manager
                .reset_session_state_if_processing(&subagent_session_id, &subagent_dialog_turn_id);
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
    /// Active subagent executions keyed by subagent session id.
    active_subagent_executions: Arc<DashMap<String, ActiveSubagentExecution>>,
    /// Background Task runs keyed by background_task_id.
    background_subagent_tasks: Arc<DashMap<String, BackgroundSubagentTaskControl>>,
    /// Cancelled deliveries consumed by the scheduler after it acquires the
    /// parent session's operation lock.
    background_subagent_delivery_suppressions: Arc<DashMap<String, ()>>,
    /// Notifies DialogScheduler of turn outcomes; injected after construction
    scheduler_notify_tx: OnceLock<mpsc::Sender<(String, TurnOutcome)>>,
    /// Round-boundary user steering source (mid-turn user message injection); injected after construction
    round_injection_source: OnceLock<Arc<dyn DialogRoundInjectionSource>>,
    /// In-flight dialog turn tracker per session, used to serialize cancel→start
    /// transitions so a new turn never starts touching the in-memory message
    /// list while the previous (cancelled) turn's spawn task is still draining.
    /// Map value is a counter shared between the coordinator and the spawn
    /// task; spawn task increments on entry and decrements on exit.
    active_turns_per_session: Arc<DashMap<String, Arc<AtomicUsize>>>,
    thread_goal_runtime: Arc<ThreadGoalRuntime>,
    terminal_port: OnceLock<Arc<dyn TerminalPort>>,
    remote_exec_port: OnceLock<Arc<dyn RemoteExecPort>>,
}

impl ConversationCoordinator {
    pub(crate) async fn resolve_workspace_id_for_config(config: &SessionConfig) -> Option<String> {
        let explicit = config
            .workspace_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if explicit.is_some() {
            return explicit;
        }

        let workspace_path = config.workspace_path.as_deref()?;
        let workspace_service = get_global_workspace_service()?;

        if config.remote_connection_id.is_some() || config.remote_ssh_host.is_some() {
            let normalized_path = normalize_remote_workspace_path(workspace_path);
            let desired_connection_id = config
                .remote_connection_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let desired_ssh_host = config
                .remote_ssh_host
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());

            return workspace_service
                .list_workspace_infos()
                .await
                .into_iter()
                .find(|workspace| {
                    if workspace.workspace_kind != WorkspaceKind::Remote {
                        return false;
                    }
                    if normalize_remote_workspace_path(&workspace.root_path.to_string_lossy())
                        != normalized_path
                    {
                        return false;
                    }
                    if let Some(connection_id) = desired_connection_id {
                        if workspace.remote_ssh_connection_id() != Some(connection_id) {
                            return false;
                        }
                    }
                    if let Some(ssh_host) = desired_ssh_host {
                        let workspace_ssh_host = workspace
                            .metadata
                            .get("sshHost")
                            .and_then(|value| value.as_str())
                            .map(str::trim)
                            .filter(|value| !value.is_empty());
                        if workspace_ssh_host != Some(ssh_host) {
                            return false;
                        }
                    }
                    true
                })
                .map(|workspace| workspace.id);
        }

        workspace_service
            .get_workspace_by_path(Path::new(workspace_path))
            .await
            .map(|workspace| workspace.id)
    }

    async fn track_session_workspace_activity_best_effort(config: &SessionConfig, reason: &str) {
        let Some(workspace_path) = config.workspace_path.as_ref() else {
            return;
        };

        let Some(workspace_service) = get_global_workspace_service() else {
            return;
        };

        let mut options = WorkspaceCreateOptions {
            auto_set_current: false,
            add_to_recent: true,
            ..Default::default()
        };

        if config.remote_connection_id.is_some() {
            options.workspace_kind = WorkspaceKind::Remote;
            options.remote_connection_id = config.remote_connection_id.clone();
            options.remote_ssh_host = config.remote_ssh_host.clone();
        }

        if let Err(error) = workspace_service
            .track_workspace_activity(PathBuf::from(workspace_path), options)
            .await
        {
            warn!(
                "Failed to track session workspace activity: reason={}, workspace_path={}, error={}",
                reason, workspace_path, error
            );
        }
    }

    /// Build a workspace binding that is remote-aware.
    /// If the global remote workspace is active and matches the session path,
    /// returns a `WorkspaceBinding` with remote metadata and correct local
    /// session storage path.
    ///
    /// When the session's `remote_connection_id` does not match any active
    /// SSH connection (e.g. the user changed the port and the old ID is now
    /// stale), this method attempts to remap to the current workspace
    /// registration so that historical sessions continue to work.
    pub(crate) async fn build_workspace_binding(
        config: &SessionConfig,
    ) -> Option<WorkspaceBinding> {
        let workspace_path = config.workspace_path.as_ref()?;
        let path_buf = PathBuf::from(workspace_path);
        let workspace_id = Self::resolve_workspace_id_for_config(config).await;

        let identity =
            crate::service::remote_ssh::workspace_state::resolve_workspace_session_identity(
                workspace_path,
                config.remote_connection_id.as_deref(),
                config.remote_ssh_host.as_deref(),
            )
            .await?;

        if let Some(rid) = identity.remote_connection_id.as_deref() {
            // Try to look up the connection by the session's stored ID first.
            let lookup =
                crate::service::remote_ssh::workspace_state::lookup_remote_connection_with_hint(
                    workspace_path,
                    Some(rid),
                )
                .await;

            // If the stored connection_id does not resolve to a registered
            // workspace, attempt a path-only lookup.  This covers the case
            // where the user changed the SSH port: the old connection_id is
            // no longer registered, but the same remote path is now bound to
            // a new connection with the updated port.
            let (effective_rid, entry) = if lookup.is_some() {
                (rid.to_string(), lookup)
            } else {
                let path_entry =
                    crate::service::remote_ssh::workspace_state::lookup_remote_connection(
                        workspace_path,
                    )
                    .await;
                if let Some(ref pe) = path_entry {
                    log::info!(
                        "Session connection_id {} not registered for workspace {}; remapping to {}",
                        rid,
                        workspace_path,
                        pe.connection_id
                    );
                    (pe.connection_id.clone(), path_entry)
                } else {
                    (rid.to_string(), lookup)
                }
            };

            let connection_name = entry
                .map(|e| e.connection_name)
                .unwrap_or_else(|| effective_rid.clone());

            // Re-resolve identity with the effective connection_id so the
            // session storage path is correct.
            let effective_identity =
                crate::service::remote_ssh::workspace_state::resolve_workspace_session_identity(
                    workspace_path,
                    Some(&effective_rid),
                    config.remote_ssh_host.as_deref(),
                )
                .await
                .unwrap_or(identity);

            let binding = WorkspaceBinding::new_remote(
                workspace_id.clone(),
                path_buf,
                effective_rid,
                connection_name,
                effective_identity,
            );

            return Some(binding);
        }

        let binding = WorkspaceBinding::new(workspace_id, path_buf);

        Some(binding)
    }

    async fn build_session_config_for_workspace(
        workspace_path: String,
        model_id: Option<String>,
    ) -> SessionConfig {
        let remote_entry =
            crate::service::remote_ssh::workspace_state::lookup_remote_connection(&workspace_path)
                .await;

        let mut config = SessionConfig {
            workspace_path: Some(workspace_path),
            model_id,
            ..SessionConfig::default()
        };

        if let Some(entry) = remote_entry {
            config.remote_connection_id = Some(entry.connection_id);
            if !entry.ssh_host.trim().is_empty() {
                config.remote_ssh_host = Some(entry.ssh_host);
            }
        }

        config
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

    async fn restore_path_for_existing_session(&self, session_id: &str) -> BitFunResult<PathBuf> {
        if let Some(binding) = self
            .session_manager
            .resolve_session_workspace_binding(session_id)
            .await
        {
            return Ok(binding.session_storage_dir());
        }

        let session = self
            .session_manager
            .get_session(session_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Session not found: {}", session_id)))?;
        session
            .config
            .workspace_path
            .as_deref()
            .map(PathBuf::from)
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "workspace_path is required when restoring session: {}",
                    session_id
                ))
            })
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
            round_group_id: None,
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
                subagent_dialog_turn_id: None,
                attempt_id: None,
                attempt_index: None,
                subagent_model_id: None,
                subagent_model_display_name: None,
                status: Some("completed".to_string()),
                interruption_reason: None,
                queue_wait_ms: None,
                preflight_ms: None,
                confirmation_wait_ms: None,
                execution_ms: Some(outcome.duration_ms),
            }],
            thinking_items: Vec::new(),
            start_time: started_at,
            end_time: Some(completed_at),
            duration_ms: Some(outcome.duration_ms),
            provider_id: None,
            model_id: None,
            model_alias: None,
            first_chunk_ms: None,
            first_visible_output_ms: None,
            stream_duration_ms: None,
            attempt_count: None,
            failure_category: None,
            token_details: None,
            status: "completed".to_string(),
        }
    }

    fn build_manual_compaction_round_failed(
        turn_id: &str,
        compression_id: String,
        error: &str,
        context_window: usize,
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
            round_group_id: None,
            timestamp,
            text_items: Vec::new(),
            tool_items: vec![ToolItemData {
                id: compression_id.clone(),
                tool_name: CONTEXT_COMPRESSION_TOOL_NAME.to_string(),
                tool_call: ToolCallData {
                    input: serde_json::json!({
                        "trigger": "manual",
                        "context_window": context_window,
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
                subagent_dialog_turn_id: None,
                attempt_id: None,
                attempt_index: None,
                subagent_model_id: None,
                subagent_model_display_name: None,
                status: Some("error".to_string()),
                interruption_reason: None,
                queue_wait_ms: None,
                preflight_ms: None,
                confirmation_wait_ms: None,
                execution_ms: None,
            }],
            thinking_items: Vec::new(),
            start_time: timestamp,
            end_time: Some(timestamp),
            duration_ms: Some(0),
            provider_id: None,
            model_id: None,
            model_alias: None,
            first_chunk_ms: None,
            first_visible_output_ms: None,
            stream_duration_ms: None,
            attempt_count: None,
            failure_category: Some("context_compression".to_string()),
            token_details: None,
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
            active_subagent_executions: Arc::new(DashMap::new()),
            background_subagent_tasks: Arc::new(DashMap::new()),
            background_subagent_delivery_suppressions: Arc::new(DashMap::new()),
            scheduler_notify_tx: OnceLock::new(),
            round_injection_source: OnceLock::new(),
            active_turns_per_session: Arc::new(DashMap::new()),
            thread_goal_runtime: Arc::new(ThreadGoalRuntime::new()),
            terminal_port: OnceLock::new(),
            remote_exec_port: OnceLock::new(),
        }
    }

    pub fn thread_goal_runtime(&self) -> Arc<ThreadGoalRuntime> {
        Arc::clone(&self.thread_goal_runtime)
    }

    pub fn set_terminal_port(&self, terminal_port: Arc<dyn TerminalPort>) {
        if self.terminal_port.set(terminal_port).is_err() {
            log::warn!("Terminal port is already configured; ignoring duplicate injection");
        }
    }

    pub fn terminal_port(&self) -> Option<Arc<dyn TerminalPort>> {
        self.terminal_port.get().map(Arc::clone)
    }

    pub fn set_remote_exec_port(&self, remote_exec_port: Arc<dyn RemoteExecPort>) {
        if self.remote_exec_port.set(remote_exec_port).is_err() {
            log::warn!("Remote exec port is already configured; ignoring duplicate injection");
        }
    }

    pub fn remote_exec_port(&self) -> Option<Arc<dyn RemoteExecPort>> {
        self.remote_exec_port.get().map(Arc::clone)
    }

    pub(super) fn execution_cancel_token_for_dialog_turn(
        &self,
        dialog_turn_id: &str,
    ) -> Option<CancellationToken> {
        self.execution_engine
            .cancel_token_for_dialog_turn(dialog_turn_id)
    }

    /// Inject the DialogScheduler notification channel after construction.
    /// Called once during app initialization after the scheduler is created.
    pub fn set_scheduler_notifier(&self, tx: mpsc::Sender<(String, TurnOutcome)>) {
        let _ = self.scheduler_notify_tx.set(tx);
    }

    /// Wire round-boundary injection source (typically the scheduler's
    /// [`SessionRoundInjectionBuffer`](crate::agentic::round_preempt::SessionRoundInjectionBuffer)).
    pub fn set_round_injection_source(&self, source: Arc<dyn DialogRoundInjectionSource>) {
        let _ = self.round_injection_source.set(source);
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
        config.workspace_id = Self::resolve_workspace_id_for_config(&config).await;
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

        Self::track_session_workspace_activity_best_effort(&session.config, "session_created")
            .await;

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
            remote_connection_id: session.config.remote_connection_id.clone(),
            remote_ssh_host: session.config.remote_ssh_host.clone(),
        })
        .await;
        Ok(session)
    }

    /// Create a hidden internal subagent session that is persisted but excluded
    /// from normal user-facing session lists.
    pub async fn create_hidden_subagent_session_with_workspace(
        &self,
        session_id: Option<String>,
        session_name: String,
        agent_type: String,
        mut config: SessionConfig,
        workspace_path: String,
        created_by: Option<String>,
    ) -> BitFunResult<Session> {
        config.workspace_path = Some(workspace_path);
        config.workspace_id = Self::resolve_workspace_id_for_config(&config).await;
        let agent_type = Self::normalize_agent_type(&agent_type);
        self.create_hidden_subagent_session(
            session_id,
            session_name,
            agent_type,
            config,
            created_by,
        )
        .await
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
        agent_type: &str,
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
            let memory_mode = new_session_memory_mode_from_global_config().await;
            let metadata = SessionMetadata {
                session_id: session_id.to_string(),
                session_name: "Recovered Session".to_string(),
                agent_type: "agentic".to_string(),
                last_user_dialog_agent_type: None,
                last_submitted_agent_type: None,
                created_by: None,
                session_kind: SessionKind::Standard,
                memory_mode,
                model_name: "default".to_string(),
                created_at: now_ms,
                last_active_at: now_ms,
                last_finished_at: None,
                turn_count: 0,
                message_count: 0,
                tool_call_count: 0,
                status: SessionStatus::Active,
                terminal_session_id: None,
                snapshot_session_id: None,
                tags: Vec::new(),
                custom_metadata: None,
                relationship: None,
                todos: None,
                review_action_state: None,
                deep_review_run_manifest: None,
                review_target_evidence: None,
                deep_review_cache: None,
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
        turn_data.agent_type = Some(agent_type.to_string());
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

    async fn persist_completed_dialog_turn(
        session_manager: &SessionManager,
        scheduler_notify_tx: Option<&mpsc::Sender<(String, TurnOutcome)>>,
        session_id: &str,
        turn_id: &str,
        execution_result: &ExecutionResult,
    ) -> (crate::service::session::TurnStatus, String) {
        let final_response = match &execution_result.final_message.content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Mixed { text, .. } => text.clone(),
            _ => String::new(),
        };

        info!(
            "Dialog turn completed: session={}, turn={}, rounds={}",
            session_id, turn_id, execution_result.total_rounds
        );

        if let Err(error) = session_manager
            .complete_dialog_turn(
                session_id,
                turn_id,
                final_response.clone(),
                &execution_result.new_messages,
                TurnStats {
                    total_rounds: execution_result.total_rounds,
                    total_tools: 0, // TODO: get from execution_result
                    total_tokens: 0,
                    duration_ms: 0,
                },
            )
            .await
        {
            error!(
                "Failed to complete dialog turn: session_id={}, turn_id={}, error={}",
                session_id, turn_id, error
            );
        }

        match session_manager
            .update_session_state_for_turn_if_processing(session_id, turn_id, SessionState::Idle)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                debug!(
                    "Skipped setting session Idle after completion for stale turn: session_id={}, turn_id={}",
                    session_id, turn_id
                );
            }
            Err(error) => {
                error!(
                    "Failed to set session state to Idle after completion: session_id={}, turn_id={}, error={}",
                    session_id, turn_id, error
                );
            }
        }

        if let Some(tx) = scheduler_notify_tx {
            if let Err(error) = tx.try_send((
                session_id.to_string(),
                TurnOutcome::Completed {
                    turn_id: turn_id.to_string(),
                    final_response: final_response.clone(),
                },
            )) {
                error!(
                    "Failed to notify scheduler of turn completion: session_id={}, turn_id={}, error={}",
                    session_id, turn_id, error
                );
            }
        }

        (
            crate::service::session::TurnStatus::Completed,
            final_response,
        )
    }

    async fn persist_cancelled_dialog_turn(
        event_queue: &EventQueue,
        session_manager: &SessionManager,
        scheduler_notify_tx: Option<&mpsc::Sender<(String, TurnOutcome)>>,
        session_id: &str,
        turn_id: &str,
        emit_lifecycle_events: bool,
    ) -> crate::service::session::TurnStatus {
        info!(
            "Dialog turn cancelled: session={}, turn={}",
            session_id, turn_id
        );

        if emit_lifecycle_events {
            // The execution engine only emits DialogTurnCancelled when cancellation is
            // detected between rounds. If cancellation interrupted streaming mid-round,
            // no event was emitted. Emit it here unconditionally; duplicates are harmless.
            if let Err(error) = event_queue
                .enqueue(
                    AgenticEvent::DialogTurnCancelled {
                        session_id: session_id.to_string(),
                        turn_id: turn_id.to_string(),
                    },
                    Some(EventPriority::Critical),
                )
                .await
            {
                error!(
                    "Failed to emit DialogTurnCancelled event: session_id={}, turn_id={}, error={}",
                    session_id, turn_id, error
                );
            }
        }

        if let Err(error) = session_manager
            .cancel_dialog_turn(session_id, turn_id)
            .await
        {
            error!(
                "Failed to cancel dialog turn in persistence: session_id={}, turn_id={}, error={}",
                session_id, turn_id, error
            );
        }

        match session_manager
            .update_session_state_for_turn_if_processing(session_id, turn_id, SessionState::Idle)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                debug!(
                    "Skipped setting session Idle after cancellation for stale turn: session_id={}, turn_id={}",
                    session_id, turn_id
                );
            }
            Err(error) => {
                error!(
                    "Failed to set session state to Idle after cancellation: session_id={}, turn_id={}, error={}",
                    session_id, turn_id, error
                );
            }
        }

        if let Some(tx) = scheduler_notify_tx {
            if let Err(error) = tx.try_send((
                session_id.to_string(),
                TurnOutcome::Cancelled {
                    turn_id: turn_id.to_string(),
                },
            )) {
                error!(
                    "Failed to notify scheduler of turn cancellation: session_id={}, turn_id={}, error={}",
                    session_id, turn_id, error
                );
            }
        }

        crate::service::session::TurnStatus::Cancelled
    }

    async fn persist_failed_dialog_turn(
        event_queue: &EventQueue,
        session_manager: &SessionManager,
        scheduler_notify_tx: Option<&mpsc::Sender<(String, TurnOutcome)>>,
        session_id: &str,
        turn_id: &str,
        error: &BitFunError,
        emit_lifecycle_events: bool,
    ) -> crate::service::session::TurnStatus {
        let error_text = error.to_string();
        let recoverable = !matches!(error, BitFunError::AIClient(_) | BitFunError::Timeout(_));

        error!("Dialog turn execution failed: {}", error_text);

        if emit_lifecycle_events {
            if let Err(queue_error) = event_queue
                .enqueue(
                    AgenticEvent::DialogTurnFailed {
                        session_id: session_id.to_string(),
                        turn_id: turn_id.to_string(),
                        error: error_text.clone(),
                        error_category: Some(error.error_category()),
                        error_detail: Some(error.error_detail()),
                    },
                    Some(EventPriority::Critical),
                )
                .await
            {
                error!(
                    "Failed to emit DialogTurnFailed event: session_id={}, turn_id={}, error={}",
                    session_id, turn_id, queue_error
                );
            }
        }

        if let Err(persist_error) = session_manager
            .fail_dialog_turn(session_id, turn_id, error_text.clone())
            .await
        {
            error!(
                "Failed to mark dialog turn as failed: session_id={}, turn_id={}, error={}",
                session_id, turn_id, persist_error
            );
        }

        match session_manager
            .update_session_state_for_turn_if_processing(
                session_id,
                turn_id,
                SessionState::Error {
                    error: error_text.clone(),
                    recoverable,
                },
            )
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                debug!(
                    "Skipped setting session Error after failure for stale turn: session_id={}, turn_id={}",
                    session_id, turn_id
                );
            }
            Err(state_error) => {
                error!(
                    "Failed to set session state to Error: session_id={}, turn_id={}, error={}",
                    session_id, turn_id, state_error
                );
            }
        }

        if let Some(tx) = scheduler_notify_tx {
            if let Err(notify_error) = tx.try_send((
                session_id.to_string(),
                TurnOutcome::Failed {
                    turn_id: turn_id.to_string(),
                    error: error_text.clone(),
                },
            )) {
                error!(
                    "Failed to notify scheduler of turn failure: session_id={}, turn_id={}, error={}",
                    session_id, turn_id, notify_error
                );
            }
        }

        if let Some(coordinator) = get_global_coordinator() {
            coordinator
                .maybe_mark_thread_goal_usage_limited(session_id, error)
                .await;
        }

        crate::service::session::TurnStatus::Error
    }

    async fn finalize_persisted_turn_in_workspace_if_needed(
        session_manager: &SessionManager,
        session_id: &str,
        turn_id: &str,
        turn_index: usize,
        agent_type: &str,
        user_input: &str,
        workspace_path: Option<&str>,
        resolved_session_storage_path: Option<&std::path::Path>,
        status: Option<crate::service::session::TurnStatus>,
        user_message_metadata: Option<serde_json::Value>,
    ) {
        if !session_manager.should_persist_session_id(session_id) {
            return;
        }

        if let (Some(workspace_path), Some(status)) = (workspace_path, status) {
            Self::finalize_turn_in_workspace(
                session_id,
                turn_id,
                turn_index,
                agent_type,
                user_input,
                workspace_path,
                resolved_session_storage_path,
                status,
                user_message_metadata,
            )
            .await;
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
        self.create_hidden_agent_session(
            session_id,
            session_name,
            agent_type,
            config,
            created_by,
            SessionKind::Subagent,
        )
        .await
    }

    async fn create_hidden_agent_session(
        &self,
        session_id: Option<String>,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
        created_by: Option<String>,
        kind: SessionKind,
    ) -> BitFunResult<Session> {
        self.session_manager
            .create_session_with_id_and_details(
                session_id,
                session_name,
                agent_type,
                config,
                created_by,
                kind,
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
            match self.restore_path_for_existing_session(session_id).await {
                Ok(restore_path) => {
                    match self
                        .session_manager
                        .restore_session_from_storage_path(&restore_path, session_id)
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
                Err(e) => {
                    debug!(
                        "Failed to resolve parent session restore path for fork capture: session_id={}, error={}",
                        session_id, e
                    );
                }
            }
        }

        Ok(context_messages)
    }

    async fn wrap_user_input(
        &self,
        session_id: &str,
        turn_index: usize,
        agent_type: &str,
        previous_agent_type: Option<&str>,
        user_input: String,
        workspace: Option<&WorkspaceBinding>,
        workspace_services: Option<&WorkspaceServices>,
        enable_tools: bool,
        skill_agent_context_vars: &HashMap<String, String>,
    ) -> BitFunResult<WrappedUserInputPayload> {
        let agent_registry = get_agent_registry();
        agent_registry
            .load_custom_agents(
                workspace
                    .filter(|binding| !binding.is_remote())
                    .map(|binding| binding.root_path()),
            )
            .await;
        let current_agent = agent_registry
            .get_agent(agent_type, workspace.map(|binding| binding.root_path()))
            .ok_or_else(|| BitFunError::NotFound(format!("Agent not found: {}", agent_type)))?;
        let current_agent_reminder = current_agent
            .get_system_reminder(previous_agent_type, workspace)
            .await?;
        let surface_resolution = resolve_skill_agent_snapshot(
            agent_type,
            workspace,
            workspace_services,
            enable_tools,
            skill_agent_context_vars,
        )
        .await;

        let mut prepended_messages = Vec::new();

        let snapshot_persistence = if turn_index == 0 {
            SkillAgentSnapshotPersistence::SaveCurrentTurn
        } else if self
            .session_manager
            .turn_skill_agent_snapshot(session_id, 0)
            .await
            .is_none()
        {
            warn!(
                "First-turn skill-agent snapshot missing; recovering baseline from current skill-agent snapshot: session_id={}, turn_index={}",
                session_id, turn_index
            );
            SkillAgentSnapshotPersistence::RecoverFirstTurnBaseline
        } else if let Some((baseline_turn_index, previous_snapshot)) = self
            .session_manager
            .latest_turn_skill_agent_snapshot_at_or_before(session_id, turn_index - 1)
            .await
        {
            let diff = diff_skill_agent_snapshot(&previous_snapshot, &surface_resolution.snapshot);
            if let Some(skill_update) = diff.render_skill_listing_update() {
                prepended_messages.push(Message::internal_reminder(
                    InternalReminderKind::SkillListingDiff,
                    skill_update,
                ));
            }
            if let Some(agent_update) = diff.render_agent_listing_update() {
                prepended_messages.push(Message::internal_reminder(
                    InternalReminderKind::AgentListingDiff,
                    agent_update,
                ));
            }
            if diff.is_empty() {
                SkillAgentSnapshotPersistence::None
            } else {
                debug!(
                    "Skill-agent snapshot changed; persisting sparse snapshot: session_id={}, turn_index={}, baseline_turn_index={}",
                    session_id, turn_index, baseline_turn_index
                );
                SkillAgentSnapshotPersistence::SaveCurrentTurn
            }
        } else {
            warn!(
                "No prior skill-agent snapshot available for diff; skipping skill-agent diff: session_id={}, turn_index={}",
                session_id, turn_index
            );
            SkillAgentSnapshotPersistence::None
        };

        if !current_agent_reminder.is_empty() {
            prepended_messages.push(Message::internal_reminder(
                InternalReminderKind::AgentMode,
                current_agent_reminder,
            ));
        }

        Ok(WrappedUserInputPayload {
            content: user_input,
            prepended_messages,
            skill_agent_snapshot: surface_resolution.snapshot,
            snapshot_persistence,
        })
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

        let kickoff_reminder =
            Self::assistant_bootstrap_system_reminder(kickoff_query, expected_reply_language);

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
            kickoff_query.to_string(),
            Some(kickoff_query.to_string()),
            None,
            Some(turn_id.clone()),
            ASSISTANT_BOOTSTRAP_AGENT_TYPE.to_string(),
            Some(workspace_root.to_string_lossy().to_string()),
            None,
            None,
            DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopApi)
                .with_skip_tool_confirmation(true),
            Some(metadata),
            vec![Message::internal_reminder(
                InternalReminderKind::Generic,
                kickoff_reminder,
            )],
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
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
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
            remote_connection_id,
            remote_ssh_host,
            submission_policy,
            user_message_metadata,
            Vec::new(),
            false,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn start_dialog_turn_with_prepended_messages(
        &self,
        session_id: String,
        user_input: String,
        original_user_input: Option<String>,
        turn_id: Option<String>,
        agent_type: String,
        workspace_path: Option<String>,
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
        submission_policy: DialogSubmissionPolicy,
        user_message_metadata: Option<serde_json::Value>,
        prepended_messages: Vec<Message>,
    ) -> BitFunResult<()> {
        self.start_dialog_turn_internal(
            session_id,
            user_input,
            original_user_input,
            None,
            turn_id,
            agent_type,
            workspace_path,
            remote_connection_id,
            remote_ssh_host,
            submission_policy,
            user_message_metadata,
            prepended_messages,
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
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
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
            remote_connection_id,
            remote_ssh_host,
            submission_policy,
            user_message_metadata,
            Vec::new(),
            false,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn start_dialog_turn_with_image_contexts_and_prepended_messages(
        &self,
        session_id: String,
        user_input: String,
        original_user_input: Option<String>,
        image_contexts: Vec<ImageContextData>,
        turn_id: Option<String>,
        agent_type: String,
        workspace_path: Option<String>,
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
        submission_policy: DialogSubmissionPolicy,
        user_message_metadata: Option<serde_json::Value>,
        prepended_messages: Vec<Message>,
    ) -> BitFunResult<()> {
        self.start_dialog_turn_internal(
            session_id,
            user_input,
            original_user_input,
            Some(image_contexts),
            turn_id,
            agent_type,
            workspace_path,
            remote_connection_id,
            remote_ssh_host,
            submission_policy,
            user_message_metadata,
            prepended_messages,
            false,
        )
        .await
    }

    fn thread_goal_store(&self) -> ThreadGoalStore<'_> {
        ThreadGoalStore::new(self.session_manager.as_ref())
    }

    async fn resolve_session_restore_path(
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
    ) -> BitFunResult<PathBuf> {
        let request = SessionStoragePathRequest {
            workspace_path: PathBuf::from(workspace_path),
            remote_connection_id: remote_connection_id.map(ToOwned::to_owned),
            remote_ssh_host: remote_ssh_host.map(ToOwned::to_owned),
        };

        CoreSessionStorePort::default()
            .resolve_session_storage_path(request)
            .await
            .map(|resolution| resolution.effective_storage_path)
            .map_err(|error| BitFunError::Session(error.to_string()))
    }

    fn require_main_session_workspace(&self, session_id: &str) -> BitFunResult<PathBuf> {
        let session = self
            .session_manager
            .get_session(session_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Session not found: {session_id}")))?;
        if matches!(
            session.kind,
            SessionKind::Subagent | SessionKind::EphemeralChild
        ) {
            return Err(BitFunError::Validation(
                "Thread goals are only available for main sessions".to_string(),
            ));
        }
        session
            .config
            .workspace_path
            .as_deref()
            .map(Path::new)
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                BitFunError::Validation(format!("Session workspace_path is missing: {session_id}"))
            })
    }

    async fn require_main_session_storage_path(&self, session_id: &str) -> BitFunResult<PathBuf> {
        self.require_main_session_workspace(session_id)?;
        self.session_manager
            .resolve_session_workspace_binding(session_id)
            .await
            .map(|binding| binding.session_storage_dir())
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Session storage path is unavailable: {session_id}"
                ))
            })
    }

    async fn resolve_thread_goal_storage_path(
        &self,
        session_id: &str,
        workspace_path: &Path,
    ) -> BitFunResult<PathBuf> {
        if self.session_manager.get_session(session_id).is_some() {
            self.require_main_session_storage_path(session_id).await
        } else {
            Ok(workspace_path.to_path_buf())
        }
    }

    pub async fn get_thread_goal(
        &self,
        session_id: &str,
        workspace_path: &Path,
    ) -> BitFunResult<Option<ThreadGoal>> {
        let storage_path = self
            .resolve_thread_goal_storage_path(session_id, workspace_path)
            .await?;
        self.thread_goal_store()
            .get_thread_goal(session_id, storage_path.as_path())
            .await
    }

    pub async fn clear_thread_goal(
        &self,
        session_id: &str,
        workspace_path: &Path,
    ) -> BitFunResult<()> {
        let storage_path = self
            .resolve_thread_goal_storage_path(session_id, workspace_path)
            .await?;
        self.thread_goal_runtime.clear_active_goal(None);
        self.thread_goal_store()
            .clear_thread_goal(session_id, storage_path.as_path())
            .await?;
        self.emit_thread_goal_updated(session_id, None).await;
        Ok(())
    }

    pub async fn create_thread_goal(
        &self,
        session_id: &str,
        _workspace_path: &Path,
        objective: String,
        token_budget: Option<i64>,
    ) -> BitFunResult<ThreadGoal> {
        let storage_path = self.require_main_session_storage_path(session_id).await?;
        let goal = self
            .thread_goal_store()
            .create_thread_goal(session_id, storage_path.as_path(), objective, token_budget)
            .await?;
        self.thread_goal_runtime.mark_turn_started("", Some(&goal));
        self.emit_thread_goal_updated(session_id, Some(goal.clone()))
            .await;
        Ok(goal)
    }

    pub async fn update_thread_goal_objective(
        &self,
        session_id: &str,
        _workspace_path: &Path,
        objective: String,
    ) -> BitFunResult<ThreadGoal> {
        let storage_path = self.require_main_session_storage_path(session_id).await?;
        let existing = self
            .thread_goal_store()
            .get_thread_goal(session_id, storage_path.as_path())
            .await?
            .ok_or_else(|| {
                BitFunError::NotFound(format!(
                    "cannot edit goal for session {session_id}: no goal exists"
                ))
            })?;
        let status = match existing.status {
            ThreadGoalStatus::BudgetLimited | ThreadGoalStatus::Complete => {
                Some(ThreadGoalStatus::Active)
            }
            _ => None,
        };
        let result = self
            .thread_goal_store()
            .set_thread_goal(
                session_id,
                storage_path.as_path(),
                Some(objective),
                status,
                None,
                false,
            )
            .await?;
        let objective_changed = existing.objective != result.goal.objective;
        if result.goal.is_active() {
            self.thread_goal_runtime
                .mark_turn_started("", Some(&result.goal));
        }
        self.emit_thread_goal_updated(session_id, Some(result.goal.clone()))
            .await;
        if objective_changed && result.goal.is_active() {
            self.apply_objective_updated_steering(session_id, &result.goal)
                .await;
        }
        Ok(result.goal)
    }

    pub async fn set_thread_goal_objective(
        &self,
        session_id: &str,
        _workspace_path: &Path,
        objective: String,
        replace_existing: bool,
    ) -> BitFunResult<ThreadGoal> {
        let storage_path = self.require_main_session_storage_path(session_id).await?;
        let previous = self
            .thread_goal_store()
            .get_thread_goal(session_id, storage_path.as_path())
            .await?;
        let status = if previous.is_some() && !replace_existing {
            None
        } else {
            Some(ThreadGoalStatus::Active)
        };
        let result = self
            .thread_goal_store()
            .set_thread_goal(
                session_id,
                storage_path.as_path(),
                Some(objective),
                status,
                None,
                replace_existing,
            )
            .await?;
        let objective_changed = previous
            .as_ref()
            .map(|goal| goal.objective != result.goal.objective)
            .unwrap_or(true);
        if result.goal.is_active() {
            self.thread_goal_runtime
                .mark_turn_started("", Some(&result.goal));
        }
        self.emit_thread_goal_updated(session_id, Some(result.goal.clone()))
            .await;
        if objective_changed && result.goal.is_active() {
            self.apply_objective_updated_steering(session_id, &result.goal)
                .await;
        }
        Ok(result.goal)
    }

    async fn apply_objective_updated_steering(&self, session_id: &str, goal: &ThreadGoal) {
        if !goal.is_active() {
            return;
        }
        let agent_type = match self.session_manager.get_session(session_id) {
            Some(session) => {
                let agent_type = session.agent_type.trim();
                if agent_type.is_empty() {
                    "agentic".to_string()
                } else {
                    agent_type.to_string()
                }
            }
            None => "agentic".to_string(),
        };
        let workspace_path = self
            .require_main_session_workspace(session_id)
            .ok()
            .map(|path| path.to_string_lossy().to_string());
        let (remote_connection_id, remote_ssh_host) = self
            .session_manager
            .get_session(session_id)
            .map(|session| {
                (
                    session.config.remote_connection_id.clone(),
                    session.config.remote_ssh_host.clone(),
                )
            })
            .unwrap_or((None, None));
        let runtime = match CoreServiceAgentRuntime::global_agent_runtime_with_lifecycle_delivery()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                warn!(
                    "Agent runtime lifecycle delivery is not available; objective_updated steering skipped: session_id={}, error={}",
                    session_id, error
                );
                return;
            }
        };
        if let Err(error) = runtime
            .deliver_thread_goal(AgentThreadGoalDeliveryRequest {
                session_id: session_id.to_string(),
                agent_type,
                workspace_path,
                remote_connection_id,
                remote_ssh_host,
                kind: AgentThreadGoalDeliveryKind::ObjectiveUpdated,
                goal: goal.clone(),
            })
            .await
        {
            warn!(
                "Failed to deliver objective_updated steering: session_id={}, error={}",
                session_id,
                CoreServiceAgentRuntime::runtime_error_message(error)
            );
        }
    }

    pub async fn maybe_mark_thread_goal_usage_limited(
        &self,
        session_id: &str,
        error: &BitFunError,
    ) {
        if !is_usage_limit_error(error) {
            return;
        }
        let storage_path = match self.require_main_session_storage_path(session_id).await {
            Ok(path) => path,
            Err(_) => return,
        };
        let Ok(Some(goal)) = self
            .thread_goal_store()
            .get_thread_goal(session_id, storage_path.as_path())
            .await
        else {
            return;
        };
        if !goal.is_active() {
            return;
        }
        if let Err(error) = self
            .set_thread_goal_status(
                session_id,
                storage_path.as_path(),
                ThreadGoalStatus::UsageLimited,
            )
            .await
        {
            warn!(
                "Failed to mark thread goal usage limited: session_id={}, error={}",
                session_id, error
            );
        }
    }

    pub async fn set_thread_goal_status(
        &self,
        session_id: &str,
        _workspace_path: &Path,
        status: ThreadGoalStatus,
    ) -> BitFunResult<ThreadGoal> {
        let storage_path = self.require_main_session_storage_path(session_id).await?;
        let previous = self
            .thread_goal_store()
            .get_thread_goal(session_id, storage_path.as_path())
            .await?;
        let resuming = status == ThreadGoalStatus::Active
            && previous
                .as_ref()
                .is_some_and(|goal| thread_goal_status_is_resumable(goal.status));
        let result = self
            .thread_goal_store()
            .set_thread_goal(
                session_id,
                storage_path.as_path(),
                None,
                Some(status),
                None,
                false,
            )
            .await?;
        if !result.goal.is_active() {
            self.thread_goal_runtime.clear_active_goal(None);
        } else if resuming {
            self.thread_goal_runtime
                .mark_turn_started("", Some(&result.goal));
        }
        self.emit_thread_goal_updated(session_id, Some(result.goal.clone()))
            .await;
        if resuming && result.goal.is_active() {
            clear_thread_goal_continuation_abort(session_id);
            self.schedule_thread_goal_resumed_steering(session_id, &result.goal);
        }
        Ok(result.goal)
    }

    /// Pause an active thread goal after the user manually stops a turn so the UI can offer resume.
    pub async fn pause_thread_goal_after_user_cancel(&self, session_id: &str) {
        let storage_path = match self.require_main_session_storage_path(session_id).await {
            Ok(path) => path,
            Err(error) => {
                debug!(
                    "Skipping thread goal pause after cancel (no workspace): session_id={}, error={}",
                    session_id, error
                );
                return;
            }
        };
        let Ok(Some(goal)) = self
            .thread_goal_store()
            .get_thread_goal(session_id, storage_path.as_path())
            .await
        else {
            return;
        };
        if !goal.is_active() {
            return;
        }
        if let Err(error) = self
            .set_thread_goal_status(session_id, storage_path.as_path(), ThreadGoalStatus::Paused)
            .await
        {
            warn!(
                "Failed to pause thread goal after user cancel: session_id={}, error={}",
                session_id, error
            );
        } else {
            info!(
                "Thread goal paused after user cancel: session_id={}, objective={}",
                session_id, goal.objective
            );
        }
    }

    fn schedule_thread_goal_resumed_steering(&self, session_id: &str, goal: &ThreadGoal) {
        if !goal.is_active() {
            return;
        }
        let agent_type = match self.session_manager.get_session(session_id) {
            Some(session) => {
                let agent_type = session.agent_type.trim();
                if agent_type.is_empty() {
                    "agentic".to_string()
                } else {
                    agent_type.to_string()
                }
            }
            None => "agentic".to_string(),
        };
        let workspace_path = self
            .require_main_session_workspace(session_id)
            .ok()
            .map(|path| path.to_string_lossy().to_string());
        let (remote_connection_id, remote_ssh_host) = self
            .session_manager
            .get_session(session_id)
            .map(|session| {
                (
                    session.config.remote_connection_id.clone(),
                    session.config.remote_ssh_host.clone(),
                )
            })
            .unwrap_or((None, None));
        let session_id = session_id.to_string();
        let goal = goal.clone();
        tokio::spawn(async move {
            let runtime =
                match CoreServiceAgentRuntime::global_agent_runtime_with_lifecycle_delivery() {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        warn!(
                            "Agent runtime lifecycle delivery is not available; thread goal resume steering skipped: session_id={}, error={}",
                            session_id, error
                        );
                        return;
                    }
                };
            if let Err(error) = runtime
                .deliver_thread_goal(AgentThreadGoalDeliveryRequest {
                    session_id: session_id.clone(),
                    agent_type,
                    workspace_path,
                    remote_connection_id,
                    remote_ssh_host,
                    kind: AgentThreadGoalDeliveryKind::Resumed,
                    goal,
                })
                .await
            {
                warn!(
                    "Failed to deliver thread goal resume steering: session_id={}, error={}",
                    session_id,
                    CoreServiceAgentRuntime::runtime_error_message(error)
                );
            }
        });
    }

    pub async fn update_thread_goal_status(
        &self,
        session_id: &str,
        workspace_path: &Path,
        status: ThreadGoalStatus,
        turn_id: Option<&str>,
    ) -> BitFunResult<ThreadGoal> {
        let goal = self
            .set_thread_goal_status(session_id, workspace_path, status)
            .await?;
        self.thread_goal_runtime.clear_active_goal(turn_id);
        Ok(goal)
    }

    pub async fn emit_thread_goal_updated(&self, session_id: &str, goal: Option<ThreadGoal>) {
        let goal = bitfun_agent_runtime::thread_goal::thread_goal_event_payload(goal);
        self.emit_event(AgenticEvent::ThreadGoalUpdated {
            session_id: session_id.to_string(),
            goal,
        })
        .await;
    }

    async fn load_active_thread_goal(&self, session_id: &str) -> BitFunResult<Option<ThreadGoal>> {
        let storage_path = self.require_main_session_storage_path(session_id).await?;
        Ok(self
            .thread_goal_store()
            .get_thread_goal(session_id, storage_path.as_path())
            .await?
            .filter(ThreadGoal::is_active))
    }

    /// Set a thread goal from `/goal <objective>` (Codex-style direct objective).
    pub async fn activate_session_goal(
        &self,
        session_id: String,
        user_hint: Option<String>,
    ) -> BitFunResult<ThreadGoal> {
        let objective = user_hint.ok_or_else(|| {
            BitFunError::Validation(
                "Goal objective is required. Use /goal <objective>.".to_string(),
            )
        })?;
        let storage_path = self.require_main_session_storage_path(&session_id).await?;
        let existing = self
            .thread_goal_store()
            .get_thread_goal(&session_id, storage_path.as_path())
            .await?;
        let replace_existing = existing.is_some();
        let goal = self
            .set_thread_goal_objective(
                &session_id,
                storage_path.as_path(),
                objective,
                replace_existing,
            )
            .await
            .map_err(user_facing_thread_goal_error)?;
        info!(
            "Thread goal set from /goal: session_id={}, objective={}",
            session_id, goal.objective
        );
        Ok(goal)
    }

    /// Continue an active thread goal after a dialog turn completes (Codex-style).
    pub async fn prepare_goal_continuation_after_turn(
        &self,
        session_id: &str,
        source_turn_id: &str,
        user_input: &str,
        user_message_metadata: Option<&serde_json::Value>,
        turn_completed: bool,
    ) -> BitFunResult<Option<ThreadGoalContinuationPlan>> {
        if should_skip_goal_continuation_after_turn(user_input, user_message_metadata) {
            return Ok(None);
        }

        let storage_path = match self.require_main_session_storage_path(session_id).await {
            Ok(path) => path,
            Err(_) => return Ok(None),
        };

        let turn_tokens = self
            .thread_goal_runtime
            .turn_cumulative_billable_tokens(source_turn_id);

        let goal_before = self
            .thread_goal_store()
            .get_thread_goal(session_id, storage_path.as_path())
            .await?;

        let plan = maybe_build_continuation_after_turn(
            &self.thread_goal_store(),
            self.thread_goal_runtime.as_ref(),
            session_id,
            storage_path.as_path(),
            source_turn_id,
            turn_tokens,
            turn_completed,
        )
        .await?;

        let goal_after = self
            .thread_goal_store()
            .get_thread_goal(session_id, storage_path.as_path())
            .await?;
        if goal_before.as_ref().map(|goal| goal.status)
            != goal_after.as_ref().map(|goal| goal.status)
        {
            if let Some(goal) = goal_after {
                self.emit_thread_goal_updated(session_id, Some(goal)).await;
            }
        }

        Ok(plan)
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
            let restore_path = self.restore_path_for_existing_session(&session_id).await?;
            self.session_manager
                .restore_session_from_storage_path(&restore_path, &session_id)
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
        })
        .await;

        let manual_workspace = Self::build_workspace_binding(&session.config).await;
        let manual_workspace_services = Self::build_workspace_services(&manual_workspace).await;
        let manual_execution_context = ExecutionContext {
            session_id: session_id.clone(),
            dialog_turn_id: turn_id.clone(),
            turn_index,
            agent_type: session.agent_type.clone(),
            workspace: manual_workspace,
            context: HashMap::new(),
            subagent_parent_info: None,
            delegation_policy: DelegationPolicy::top_level(),
            skip_tool_confirmation: true,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: manual_workspace_services,
            terminal_port: self.terminal_port(),
            remote_exec_port: self.remote_exec_port(),
            round_injection: None,
            emit_lifecycle_events: true,
            recover_partial_on_cancel: false,
        };
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
        match self
            .execution_engine
            .compact_session_context(
                session_id.clone(),
                turn_id.clone(),
                manual_execution_context,
                context_messages,
                "manual",
            )
            .await
        {
            Ok(outcome) => {
                let model_round = Self::build_manual_compaction_round_completed(
                    &turn_id,
                    &outcome,
                    context_window,
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
                    partial_recovery_reason: None,
                    success: Some(true),
                    finish_reason: Some("complete".to_string()),
                    has_final_response: Some(true),
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
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
        submission_policy: DialogSubmissionPolicy,
        extra_user_message_metadata: Option<serde_json::Value>,
        additional_prepended_messages: Vec<Message>,
        suppress_session_title_generation: bool,
    ) -> BitFunResult<()> {
        let requested_restore_path = match workspace_path.as_deref() {
            Some(workspace_path) => Some(
                Self::resolve_session_restore_path(
                    workspace_path,
                    remote_connection_id.as_deref(),
                    remote_ssh_host.as_deref(),
                )
                .await?,
            ),
            None => None,
        };

        // Get latest session, restoring from persistence on demand so every entry
        // point can use the same start_dialog_turn flow. A loaded session must keep
        // the same storage identity as this invocation.
        let session = match self.session_manager.get_session(&session_id) {
            Some(session) => {
                if let Some(restore_path) = requested_restore_path.as_deref() {
                    self.session_manager
                        .ensure_session_storage_path(&session_id, restore_path)?;
                }
                session
            }
            None => {
                debug!(
                    "Session not found in memory, attempting restore before starting dialog: session_id={}",
                    session_id
                );
                let restore_path = requested_restore_path.ok_or_else(|| {
                    BitFunError::Validation(format!(
                        "workspace_path is required when restoring session: {}",
                        session_id
                    ))
                })?;
                self.session_manager
                    .restore_session_from_storage_path(&restore_path, &session_id)
                    .await?
            }
        };

        let previous_agent_type = session.last_user_dialog_agent_type.clone();
        let requested_agent_type = agent_type.trim().to_string();
        let provisional_agent_type = if !requested_agent_type.is_empty() {
            requested_agent_type.clone()
        } else if !session.agent_type.is_empty() {
            session.agent_type.clone()
        } else {
            "agentic".to_string()
        };
        let effective_agent_type = Self::normalize_agent_type(&provisional_agent_type);

        Self::track_session_workspace_activity_best_effort(&session.config, "dialog_started").await;

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

        // P0-8: Even when SessionState is Idle, a previously cancelled turn's
        // spawn task may still be draining (writing tail messages into the
        // in-memory context cache). Wait briefly for it to finish so the new
        // turn does not race with it. This is a no-op when no turn is in flight.
        let pending = self
            .wait_session_drained(&session_id, Duration::from_millis(800))
            .await;
        if pending > 0 {
            warn!(
                "Starting new dialog while previous turn still draining: session_id={}, pending={}",
                session_id, pending
            );
        }

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
            let restore_workspace_path = session
                .config
                .workspace_path
                .as_deref()
                .or(workspace_path.as_deref())
                .ok_or_else(|| {
                    BitFunError::Validation(format!(
                        "workspace_path is required when restoring session: {}",
                        session_id
                    ))
                })?;
            let restore_path = Self::resolve_session_restore_path(
                restore_workspace_path,
                session
                    .config
                    .remote_connection_id
                    .as_deref()
                    .or(remote_connection_id.as_deref()),
                session
                    .config
                    .remote_ssh_host
                    .as_deref()
                    .or(remote_ssh_host.as_deref()),
            )
            .await?;
            match self
                .session_manager
                .restore_session_from_storage_path(&restore_path, &session_id)
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
        let has_user_input = !original_user_input.trim().is_empty()
            || image_contexts
                .as_ref()
                .is_some_and(|images| !images.is_empty());

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

        let turn_index = self.session_manager.get_turn_count(&session_id);
        let mut skill_agent_context_vars = HashMap::new();
        if user_message_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("acp_transport"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            skill_agent_context_vars.insert("acp_transport".to_string(), "true".to_string());
        }

        let wrapped_user_input_payload = self
            .wrap_user_input(
                &session_id,
                turn_index,
                &effective_agent_type,
                previous_agent_type
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty()),
                user_input,
                session_workspace.as_ref(),
                workspace_services.as_ref(),
                session.config.enable_tools,
                &skill_agent_context_vars,
            )
            .await?;
        let effective_user_input = wrapped_user_input_payload.content.clone();
        let prepended_messages = merge_prepended_messages_for_turn(
            additional_prepended_messages,
            wrapped_user_input_payload.prepended_messages.clone(),
            needs_computer_links_for_source(submission_policy.trigger_source),
        );

        if original_user_input != effective_user_input {
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
        // Pass frontend turnId, generate if not provided
        let turn_id = self
            .session_manager
            .start_dialog_turn_with_prepended_messages(
                &session_id,
                effective_agent_type.clone(),
                effective_user_input.clone(),
                turn_id,
                image_contexts,
                prepended_messages,
                user_message_metadata.clone(),
            )
            .await?;
        start_memory_startup_task(MemoryStartupRequest {
            session_id: session_id.clone(),
            session_kind: session.kind,
            agent_type: effective_agent_type.clone(),
            workspace_path: session
                .config
                .workspace_path
                .clone()
                .or(workspace_path.clone()),
            is_remote_workspace: session_workspace
                .as_ref()
                .map(|workspace| workspace.is_remote())
                .unwrap_or(false),
            has_user_input,
        })
        .await;
        if let Ok(Some(goal)) = self.load_active_thread_goal(&session_id).await {
            if !should_skip_goal_for_turn(&original_user_input, user_message_metadata.as_ref()) {
                self.thread_goal_runtime
                    .mark_turn_started(&turn_id, Some(&goal));
            }
        }
        match wrapped_user_input_payload.snapshot_persistence {
            SkillAgentSnapshotPersistence::None => {}
            SkillAgentSnapshotPersistence::SaveCurrentTurn => {
                self.session_manager
                    .remember_turn_skill_agent_snapshot(
                        &session_id,
                        turn_index,
                        wrapped_user_input_payload.skill_agent_snapshot.clone(),
                    )
                    .await;
            }
            SkillAgentSnapshotPersistence::RecoverFirstTurnBaseline => {
                self.session_manager
                    .recover_first_turn_skill_agent_snapshot(
                        &session_id,
                        wrapped_user_input_payload.skill_agent_snapshot.clone(),
                    )
                    .await;
                self.session_manager
                    .remove_listing_diff_internal_reminders(&session_id)
                    .await;
            }
        }

        // Register this turn as in-flight immediately after it becomes visible
        // as Processing. Later await points must not leave a cancel/start
        // window where wait_session_drained observes zero active work.
        let active_counter = self
            .active_turns_per_session
            .entry(session_id.clone())
            .or_insert_with(|| Arc::new(AtomicUsize::new(0)))
            .clone();
        active_counter.fetch_add(1, Ordering::SeqCst);
        struct ActiveTurnRegistration {
            counter: Arc<AtomicUsize>,
            armed: bool,
        }
        impl ActiveTurnRegistration {
            fn disarm(&mut self) {
                self.armed = false;
            }
        }
        impl Drop for ActiveTurnRegistration {
            fn drop(&mut self) {
                if self.armed {
                    self.counter.fetch_sub(1, Ordering::SeqCst);
                }
            }
        }
        let mut active_registration = ActiveTurnRegistration {
            counter: active_counter.clone(),
            armed: true,
        };
        let cancellation_token = CancellationToken::new();
        self.execution_engine
            .register_cancel_token(&turn_id, cancellation_token);

        // Send dialog turn started event with original input and image metadata
        // so all frontends (desktop, mobile, bot) can display correctly.
        self.emit_event(AgenticEvent::DialogTurnStarted {
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            turn_index,
            user_input: effective_user_input.clone(),
            original_user_input: if original_user_input != effective_user_input {
                Some(original_user_input.clone())
            } else {
                None
            },
            user_message_metadata: user_message_metadata.clone(),
        })
        .await;

        // Get context messages (re-fetch as history may have been restored)
        let messages = match self.session_manager.get_context_messages(&session_id).await {
            Ok(messages) => messages,
            Err(error) => {
                self.execution_engine.cleanup_cancel_token(&turn_id).await;
                return Err(error);
            }
        };

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
        let review_agent = is_review_agent_type(&effective_agent_type);
        let turn_review_manifest =
            turn_review_manifest_for_agent(user_message_metadata.as_ref(), &effective_agent_type);
        let persisted_review_manifest = if turn_review_manifest.is_none() && review_agent {
            match session_workspace.as_ref() {
                Some(workspace) => self
                    .session_manager
                    .load_session_metadata(&workspace.session_storage_dir(), &session_id)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|metadata| {
                        metadata.deep_review_run_manifest.or_else(|| {
                            metadata.review_target_evidence.map(
                                |evidence| serde_json::json!({ "reviewTargetEvidence": evidence }),
                            )
                        })
                    }),
                None => None,
            }
        } else {
            None
        };
        if let Some(run_manifest) = turn_review_manifest.or(persisted_review_manifest) {
            context_vars.insert(
                "deep_review_run_manifest".to_string(),
                run_manifest.to_string(),
            );
        }
        if metadata_bool(user_message_metadata.as_ref(), "acp_transport") == Some(true) {
            context_vars.insert("acp_transport".to_string(), "true".to_string());
        }
        if let Some(user_input_available) = metadata_bool(
            user_message_metadata.as_ref(),
            USER_INPUT_AVAILABLE_CONTEXT_KEY,
        ) {
            context_vars.insert(
                USER_INPUT_AVAILABLE_CONTEXT_KEY.to_string(),
                user_input_available.to_string(),
            );
        }
        if needs_computer_links_for_source(submission_policy.trigger_source) {
            context_vars.insert(
                TOOL_CONTEXT_REMOTE_FILE_DELIVERY_KEY.to_string(),
                "true".to_string(),
            );
        }
        if supports_inline_markdown_images_for_source(submission_policy.trigger_source) {
            context_vars.insert(
                TOOL_CONTEXT_INLINE_MARKDOWN_IMAGE_DISPLAY_KEY.to_string(),
                "true".to_string(),
            );
        }
        if should_require_tool_confirmation(submission_policy, user_message_metadata.as_ref()) {
            context_vars.insert("require_tool_confirmation".to_string(), "true".to_string());
        }
        let session_workspace_path = session_workspace
            .as_ref()
            .map(|workspace| workspace.root_path_string());
        // Pre-resolve the on-disk session storage path (mirror dir for remote workspaces)
        // so the safety-net writer never has to re-resolve without remote_connection_id /
        // remote_ssh_host (which would silently fall back to a slugified raw remote path).
        let session_storage_path = session_workspace
            .as_ref()
            .map(|workspace| workspace.session_storage_dir().to_path_buf());

        let runtime_tool_restrictions = if is_miniapp_headless_agent_run(
            user_message_metadata.as_ref(),
            session.created_by.as_deref(),
        ) {
            miniapp_headless_agent_tool_restrictions()
        } else {
            ToolRuntimeRestrictions::default()
        };

        let execution_context = ExecutionContext {
            session_id: session_id.clone(),
            dialog_turn_id: turn_id.clone(),
            turn_index,
            agent_type: effective_agent_type.clone(),
            workspace: session_workspace,
            context: context_vars,
            subagent_parent_info: None,
            delegation_policy: DelegationPolicy::top_level(),
            skip_tool_confirmation: submission_policy.skip_tool_confirmation,
            runtime_tool_restrictions,
            workspace_services,
            terminal_port: self.terminal_port(),
            remote_exec_port: self.remote_exec_port(),
            round_injection: self.round_injection_source.get().cloned(),
            emit_lifecycle_events: true,
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
        let user_input_for_workspace = effective_user_input.clone();
        let session_storage_path_for_finalize = session_storage_path.clone();
        let effective_agent_type_clone = effective_agent_type.clone();
        let user_message_metadata_clone = user_message_metadata;
        let scheduler_notify_tx = self.scheduler_notify_tx.get().cloned();

        tokio::spawn(async move {
            // RAII guard: on drop (ANY exit path, including panic), decrements
            // the in-flight counter and resets Processing → Idle only if this
            // task still owns the current turn.
            //
            // This is the single source of truth for "is this spawn active?".
            // Because `Drop` is synchronous we use an in-memory-only state
            // update here; the async persistence of the state change is done
            // explicitly in the spawn body below.
            struct SessionExecutionGuard {
                session_manager: Arc<SessionManager>,
                session_id: String,
                turn_id: String,
                active_counter: Arc<AtomicUsize>,
            }
            impl SessionExecutionGuard {
                fn new(
                    session_manager: Arc<SessionManager>,
                    session_id: String,
                    turn_id: String,
                    active_counter: Arc<AtomicUsize>,
                ) -> Self {
                    Self {
                        session_manager,
                        session_id,
                        turn_id,
                        active_counter,
                    }
                }
            }
            impl Drop for SessionExecutionGuard {
                fn drop(&mut self) {
                    self.active_counter.fetch_sub(1, Ordering::SeqCst);
                    // If the session is still in Processing (abnormal exit),
                    // synchronously reset to Idle so the user is never stuck.
                    self.session_manager
                        .reset_session_state_if_processing(&self.session_id, &self.turn_id);
                }
            }

            let _guard = SessionExecutionGuard::new(
                session_manager.clone(),
                session_id_clone.clone(),
                turn_id_clone.clone(),
                active_counter,
            );

            // Note: Don't check cancellation here as cancel token hasn't been created yet
            // Cancel token is created in execute_dialog_turn -> execute_round
            // execute_dialog_turn has proper cancellation checks internally

            match session_manager
                .update_session_state_for_turn_if_processing(
                    &session_id_clone,
                    &turn_id_clone,
                    SessionState::Processing {
                        current_turn_id: turn_id_clone.clone(),
                        phase: ProcessingPhase::Thinking,
                    },
                )
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    debug!(
                        "Skipped refreshing Processing state for stale or cancelled turn: session_id={}, turn_id={}",
                        session_id_clone, turn_id_clone
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to set session state to Processing: session_id={}, turn_id={}, error={}",
                        session_id_clone, turn_id_clone, e
                    );
                }
            }

            let workspace_turn_status = match execution_engine
                .execute_dialog_turn(
                    effective_agent_type_clone.clone(),
                    messages,
                    execution_context,
                )
                .await
            {
                Ok(execution_result) => Some(
                    Self::persist_completed_dialog_turn(
                        session_manager.as_ref(),
                        scheduler_notify_tx.as_ref(),
                        &session_id_clone,
                        &turn_id_clone,
                        &execution_result,
                    )
                    .await
                    .0,
                ),
                Err(e) => {
                    if matches!(&e, BitFunError::Cancelled(_)) {
                        Some(
                            Self::persist_cancelled_dialog_turn(
                                event_queue.as_ref(),
                                session_manager.as_ref(),
                                scheduler_notify_tx.as_ref(),
                                &session_id_clone,
                                &turn_id_clone,
                                true,
                            )
                            .await,
                        )
                    } else {
                        Some(
                            Self::persist_failed_dialog_turn(
                                event_queue.as_ref(),
                                session_manager.as_ref(),
                                scheduler_notify_tx.as_ref(),
                                &session_id_clone,
                                &turn_id_clone,
                                &e,
                                true,
                            )
                            .await,
                        )
                    }
                }
            };

            Self::finalize_persisted_turn_in_workspace_if_needed(
                session_manager.as_ref(),
                &session_id_clone,
                &turn_id_clone,
                turn_index,
                &effective_agent_type_clone,
                &user_input_for_workspace,
                session_workspace_path.as_deref(),
                session_storage_path_for_finalize.as_deref(),
                workspace_turn_status,
                user_message_metadata_clone,
            )
            .await;
        });
        active_registration.disarm();

        Ok(())
    }

    /// P0-8: Wait until all in-flight spawn tasks for this session have
    /// drained, or until `deadline` is reached. Returns the number of
    /// in-flight turns still running (0 means fully drained). This is used to
    /// serialize cancel→start so a new turn does not start mutating the
    /// in-memory context cache while a cancelled turn's spawn task is still
    /// finishing its tail.
    async fn wait_session_drained(&self, session_id: &str, max_wait: Duration) -> usize {
        let counter = match self.active_turns_per_session.get(session_id) {
            Some(entry) => entry.value().clone(),
            None => return 0,
        };
        let deadline = Instant::now() + max_wait;
        loop {
            let pending = counter.load(Ordering::SeqCst);
            if pending == 0 {
                return 0;
            }
            if Instant::now() >= deadline {
                return pending;
            }
            sleep(Duration::from_millis(20)).await;
        }
    }

    /// Strict maintenance barrier for callers that must not overlap an older
    /// turn's tail writes. Unlike normal interactive cancellation, timeout is
    /// returned as an error instead of being treated as best effort.
    pub(crate) async fn ensure_session_execution_drained(
        &self,
        session_id: &str,
        max_wait: Duration,
    ) -> BitFunResult<()> {
        let pending = self.wait_session_drained(session_id, max_wait).await;
        if pending == 0 {
            return Ok(());
        }
        Err(BitFunError::Timeout(format!(
            "Session execution did not drain before maintenance: session_id={session_id}, pending={pending}, timeout_ms={}",
            max_wait.as_millis()
        )))
    }

    async fn cancel_active_subagents_for_parent_turn(
        &self,
        parent_session_id: &str,
        parent_dialog_turn_id: &str,
    ) {
        let active_subagents: Vec<ActiveSubagentExecution> = self
            .active_subagent_executions
            .iter()
            .filter(|entry| {
                entry.parent_session_id == parent_session_id
                    && entry.parent_dialog_turn_id == parent_dialog_turn_id
            })
            .map(|entry| entry.value().clone())
            .collect();

        if active_subagents.is_empty() {
            return;
        }

        info!(
            "Cancelling {} active subagent execution(s) for parent turn: parent_session_id={}, parent_dialog_turn_id={}",
            active_subagents.len(),
            parent_session_id,
            parent_dialog_turn_id
        );

        for active in active_subagents {
            self.stop_active_subagent_execution(&active, "Parent dialog turn cancelled")
                .await;
        }
    }

    async fn stop_active_subagent_execution(&self, active: &ActiveSubagentExecution, reason: &str) {
        debug!(
            "Stopping active subagent execution: subagent_session_id={}, subagent_dialog_turn_id={}, parent_session_id={}, parent_dialog_turn_id={}, reason={}",
            active.subagent_session_id,
            active.subagent_dialog_turn_id,
            active.parent_session_id,
            active.parent_dialog_turn_id,
            reason
        );

        active.cancel_token.cancel();
        active.abort_handle.abort();

        if let Err(error) = self
            .execution_engine
            .cancel_dialog_turn(&active.subagent_dialog_turn_id)
            .await
        {
            warn!(
                "Failed to cancel active subagent dialog turn: subagent_session_id={}, subagent_dialog_turn_id={}, error={}",
                active.subagent_session_id, active.subagent_dialog_turn_id, error
            );
        }

        if let Err(error) = self
            .tool_pipeline
            .cancel_dialog_turn_tools(&active.subagent_dialog_turn_id)
            .await
        {
            warn!(
                "Failed to cancel active subagent tools: subagent_session_id={}, subagent_dialog_turn_id={}, error={}",
                active.subagent_session_id, active.subagent_dialog_turn_id, error
            );
        }

        Self::persist_cancelled_dialog_turn(
            self.event_queue.as_ref(),
            self.session_manager.as_ref(),
            None,
            &active.subagent_session_id,
            &active.subagent_dialog_turn_id,
            true,
        )
        .await;

        self.session_manager.reset_session_state_if_processing(
            &active.subagent_session_id,
            &active.subagent_dialog_turn_id,
        );

        self.active_subagent_executions
            .remove(&active.subagent_session_id);
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

        abort_thread_goal_continuation_for_session(session_id);

        let old_state = self
            .session_manager
            .get_session(session_id)
            .map(|s| format!("{:?}", s.state))
            .unwrap_or_else(|| "Unknown".to_string());
        debug!("Current state: {}", old_state);

        // Step 1: Immediately update session state to Idle only if this
        // cancellation still targets the currently processing turn. A delayed
        // cancel request for an older turn must not clear a newer turn.
        debug!("Conditionally updating session state to Idle for cancelled turn");
        let state_updated = self
            .session_manager
            .update_session_state_for_turn_if_processing(
                session_id,
                dialog_turn_id,
                SessionState::Idle,
            )
            .await?;

        let new_state = self
            .session_manager
            .get_session(session_id)
            .map(|s| format!("{:?}", s.state))
            .unwrap_or_else(|| "Unknown".to_string());
        debug!("State updated: {} -> {}", old_state, new_state);

        // Step 2: Immediately send state change event only when this cancel
        // actually changed the active turn state.
        if state_updated {
            self.emit_event(AgenticEvent::SessionStateChanged {
                session_id: session_id.to_string(),
                new_state: "idle".to_string(),
            })
            .await;
            debug!("Session state change event sent");
            self.pause_thread_goal_after_user_cancel(session_id).await;
        } else {
            debug!(
                "Skipped idle event for stale cancellation: session_id={}, dialog_turn_id={}",
                session_id, dialog_turn_id
            );
        }

        // Step 3: Trigger cancellation tokens so the running turn unwinds. We
        // do this synchronously (not spawn) because the calls themselves are
        // cheap (just signalling tokens); the actual long-running work
        // (waiting for the spawn task to drain) is handled via
        // `wait_session_drained` below.
        if let Err(e) = self
            .execution_engine
            .cancel_dialog_turn(dialog_turn_id)
            .await
        {
            warn!("Failed to cancel execution engine: {}", e);
        }
        if let Err(e) = self
            .tool_pipeline
            .cancel_dialog_turn_tools(dialog_turn_id)
            .await
        {
            warn!("Failed to cancel tool execution: {}", e);
        }

        self.cancel_active_subagents_for_parent_turn(session_id, dialog_turn_id)
            .await;

        // Step 4: Wait briefly for the spawn task that owns this turn to drain
        // its in-memory message writes before returning. Capped so the RPC
        // never blocks longer than ~1.5s — beyond that we let the new turn
        // proceed and rely on the cancellation token already being signalled.
        let pending = self
            .wait_session_drained(session_id, Duration::from_millis(1500))
            .await;
        if pending > 0 {
            warn!(
                "Cancelled turn did not fully drain within 1500ms: session_id={}, dialog_turn_id={}, pending={}",
                session_id, dialog_turn_id, pending
            );
        } else {
            debug!(
                "Cancelled turn fully drained: session_id={}, dialog_turn_id={}",
                session_id, dialog_turn_id
            );
        }

        Ok(())
    }

    pub async fn cancel_active_turn_for_session(
        &self,
        session_id: &str,
        wait_timeout: Duration,
    ) -> BitFunResult<Option<String>> {
        abort_thread_goal_continuation_for_session(session_id);

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

    pub async fn delete_hidden_subagent_sessions_for_parent_turns(
        &self,
        workspace_path: &Path,
        parent_session_id: &str,
        parent_dialog_turn_ids: &HashSet<String>,
    ) -> BitFunResult<Vec<String>> {
        let session_ids = self
            .collect_hidden_subagent_sessions_for_parent_turns(
                workspace_path,
                parent_session_id,
                parent_dialog_turn_ids,
            )
            .await?;

        let mut deleted_session_ids = Vec::new();

        for session_id in session_ids {
            self.delete_hidden_subagent_session(workspace_path, parent_session_id, &session_id)
                .await?;
            deleted_session_ids.push(session_id);
        }

        Ok(deleted_session_ids)
    }

    pub(crate) async fn collect_hidden_subagent_sessions_for_parent_turns(
        &self,
        workspace_path: &Path,
        parent_session_id: &str,
        parent_dialog_turn_ids: &HashSet<String>,
    ) -> BitFunResult<Vec<String>> {
        self.session_manager
            .collect_hidden_subagent_cascade_for_parent_turns(
                workspace_path,
                parent_session_id,
                parent_dialog_turn_ids,
            )
            .await
    }

    pub(crate) async fn delete_hidden_subagent_session(
        &self,
        workspace_path: &Path,
        parent_session_id: &str,
        session_id: &str,
    ) -> BitFunResult<()> {
        if let Err(e) = self
            .cancel_active_turn_for_session(session_id, Duration::from_secs(2))
            .await
        {
            warn!(
                "Failed to cancel hidden subagent session before deletion: session_id={}, parent_session_id={}, error={}",
                session_id, parent_session_id, e
            );
        }

        self.delete_session(workspace_path, session_id).await
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

    pub async fn restore_session_from_storage_path(
        &self,
        session_storage_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Session> {
        self.session_manager
            .restore_session_from_storage_path(session_storage_path, session_id)
            .await
    }

    pub async fn restore_internal_session_from_storage_path(
        &self,
        session_storage_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Session> {
        self.session_manager
            .restore_internal_session_from_storage_path(session_storage_path, session_id)
            .await
    }

    pub async fn restore_session_for_workspace(
        &self,
        request: SessionStoragePathRequest,
        session_id: &str,
    ) -> BitFunResult<Session> {
        self.session_manager
            .restore_session_for_workspace(request, session_id)
            .await
    }

    pub async fn restore_internal_session_for_workspace(
        &self,
        request: SessionStoragePathRequest,
        session_id: &str,
    ) -> BitFunResult<Session> {
        self.session_manager
            .restore_internal_session_for_workspace(request, session_id)
            .await
    }

    pub async fn restore_internal_session(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Session> {
        self.session_manager
            .restore_internal_session(workspace_path, session_id)
            .await
    }

    /// Restore session and return the persisted turns read during restore.
    pub async fn restore_session_with_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<crate::service::session::DialogTurnData>)> {
        self.session_manager
            .restore_session_with_turns(workspace_path, session_id)
            .await
    }

    pub async fn restore_session_with_turns_from_storage_path(
        &self,
        session_storage_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<crate::service::session::DialogTurnData>)> {
        self.session_manager
            .restore_session_with_turns_from_storage_path(session_storage_path, session_id)
            .await
    }

    pub async fn restore_internal_session_with_turns_from_storage_path(
        &self,
        session_storage_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<crate::service::session::DialogTurnData>)> {
        self.session_manager
            .restore_internal_session_with_turns_from_storage_path(session_storage_path, session_id)
            .await
    }

    pub async fn restore_session_with_turns_for_workspace(
        &self,
        request: SessionStoragePathRequest,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<crate::service::session::DialogTurnData>)> {
        self.session_manager
            .restore_session_with_turns_for_workspace(request, session_id)
            .await
    }

    pub async fn restore_internal_session_with_turns_for_workspace(
        &self,
        request: SessionStoragePathRequest,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<crate::service::session::DialogTurnData>)> {
        self.session_manager
            .restore_internal_session_with_turns_for_workspace(request, session_id)
            .await
    }

    pub async fn restore_internal_session_with_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<crate::service::session::DialogTurnData>)> {
        self.session_manager
            .restore_internal_session_with_turns(workspace_path, session_id)
            .await
    }

    /// Restore only the UI-visible persisted session view.
    pub async fn restore_session_view(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<crate::service::session::DialogTurnData>)> {
        self.session_manager
            .restore_session_view(workspace_path, session_id)
            .await
    }

    pub async fn restore_session_view_timed(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(
        Session,
        Vec<crate::service::session::DialogTurnData>,
        crate::agentic::session::session_manager::SessionViewRestoreTiming,
    )> {
        self.session_manager
            .restore_session_view_timed(workspace_path, session_id)
            .await
    }

    pub async fn restore_session_view_for_workspace_timed(
        &self,
        request: SessionStoragePathRequest,
        session_id: &str,
    ) -> BitFunResult<(
        Session,
        Vec<crate::service::session::DialogTurnData>,
        crate::agentic::session::session_manager::SessionViewRestoreTiming,
    )> {
        self.session_manager
            .restore_session_view_for_workspace_timed(request, session_id)
            .await
    }

    pub async fn restore_session_view_from_storage_path_timed(
        &self,
        session_storage_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(
        Session,
        Vec<crate::service::session::DialogTurnData>,
        crate::agentic::session::session_manager::SessionViewRestoreTiming,
    )> {
        self.session_manager
            .restore_session_view_from_storage_path_timed(session_storage_path, session_id)
            .await
    }

    pub async fn restore_session_view_tail(
        &self,
        workspace_path: &Path,
        session_id: &str,
        tail_turn_count: usize,
    ) -> BitFunResult<(Session, Vec<crate::service::session::DialogTurnData>, usize)> {
        self.session_manager
            .restore_session_view_tail(workspace_path, session_id, tail_turn_count)
            .await
    }

    pub async fn restore_session_view_tail_timed(
        &self,
        workspace_path: &Path,
        session_id: &str,
        tail_turn_count: usize,
    ) -> BitFunResult<(
        Session,
        Vec<crate::service::session::DialogTurnData>,
        usize,
        crate::agentic::session::session_manager::SessionViewRestoreTiming,
    )> {
        self.session_manager
            .restore_session_view_tail_timed(workspace_path, session_id, tail_turn_count)
            .await
    }

    pub async fn restore_session_view_from_storage_path_tail_timed(
        &self,
        session_storage_path: &Path,
        session_id: &str,
        tail_turn_count: usize,
    ) -> BitFunResult<(
        Session,
        Vec<crate::service::session::DialogTurnData>,
        usize,
        crate::agentic::session::session_manager::SessionViewRestoreTiming,
    )> {
        self.session_manager
            .restore_session_view_from_storage_path_tail_timed(
                session_storage_path,
                session_id,
                tail_turn_count,
            )
            .await
    }

    pub async fn restore_internal_session_view(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<crate::service::session::DialogTurnData>)> {
        self.session_manager
            .restore_internal_session_view(workspace_path, session_id)
            .await
    }

    pub async fn restore_internal_session_view_timed(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(
        Session,
        Vec<crate::service::session::DialogTurnData>,
        crate::agentic::session::session_manager::SessionViewRestoreTiming,
    )> {
        self.session_manager
            .restore_internal_session_view_timed(workspace_path, session_id)
            .await
    }

    pub async fn restore_internal_session_view_for_workspace_timed(
        &self,
        request: SessionStoragePathRequest,
        session_id: &str,
    ) -> BitFunResult<(
        Session,
        Vec<crate::service::session::DialogTurnData>,
        crate::agentic::session::session_manager::SessionViewRestoreTiming,
    )> {
        self.session_manager
            .restore_internal_session_view_for_workspace_timed(request, session_id)
            .await
    }

    pub async fn restore_internal_session_view_from_storage_path_timed(
        &self,
        session_storage_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(
        Session,
        Vec<crate::service::session::DialogTurnData>,
        crate::agentic::session::session_manager::SessionViewRestoreTiming,
    )> {
        self.session_manager
            .restore_internal_session_view_from_storage_path_timed(session_storage_path, session_id)
            .await
    }

    pub async fn restore_internal_session_view_tail(
        &self,
        workspace_path: &Path,
        session_id: &str,
        tail_turn_count: usize,
    ) -> BitFunResult<(Session, Vec<crate::service::session::DialogTurnData>, usize)> {
        self.session_manager
            .restore_internal_session_view_tail(workspace_path, session_id, tail_turn_count)
            .await
    }

    pub async fn restore_internal_session_view_tail_timed(
        &self,
        workspace_path: &Path,
        session_id: &str,
        tail_turn_count: usize,
    ) -> BitFunResult<(
        Session,
        Vec<crate::service::session::DialogTurnData>,
        usize,
        crate::agentic::session::session_manager::SessionViewRestoreTiming,
    )> {
        self.session_manager
            .restore_internal_session_view_tail_timed(workspace_path, session_id, tail_turn_count)
            .await
    }

    pub async fn restore_internal_session_view_from_storage_path_tail_timed(
        &self,
        session_storage_path: &Path,
        session_id: &str,
        tail_turn_count: usize,
    ) -> BitFunResult<(
        Session,
        Vec<crate::service::session::DialogTurnData>,
        usize,
        crate::agentic::session::session_manager::SessionViewRestoreTiming,
    )> {
        self.session_manager
            .restore_internal_session_view_from_storage_path_tail_timed(
                session_storage_path,
                session_id,
                tail_turn_count,
            )
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
                    result = semaphore.acquire_owned() => result
                        .map_err(|error| BitFunError::Semaphore(error.to_string()))?,
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
                    result = semaphore.acquire_owned() => result
                        .map_err(|error| BitFunError::Semaphore(error.to_string()))?,
                    _ = token.cancelled() => {
                        return Err(BitFunError::Cancelled(
                            "Subagent task was cancelled while waiting for a concurrency slot".to_string(),
                        ));
                    }
                }
            }
            (None, Some(deadline)) => {
                tokio::select! {
                    result = semaphore.acquire_owned() => result
                        .map_err(|error| BitFunError::Semaphore(error.to_string()))?,
                    _ = tokio::time::sleep_until(deadline) => {
                        return Err(BitFunError::Timeout(format!(
                            "Timed out while waiting for a {} concurrency slot for subagent '{}'",
                            label, agent_type
                        )));
                    }
                }
            }
            (None, None) => semaphore
                .acquire_owned()
                .await
                .map_err(|error| BitFunError::Semaphore(error.to_string()))?,
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
            target_session_id,
            dialog_turn_id,
            session_name,
            agent_type,
            session_config,
            initial_messages,
            user_input_text,
            created_by,
            subagent_parent_info,
            context,
            delegation_policy,
            runtime_tool_restrictions,
            prompt_cache_source_session_id,
            session_kind,
            emit_lifecycle_events,
            prepared_session_created,
        } = request;
        let prepared_target_session_id = target_session_id.clone();

        let requested_timeout_seconds = timeout_seconds.filter(|seconds| *seconds > 0);
        let parent_thread_goal_active = if let Some(parent_info) = subagent_parent_info.as_ref() {
            matches!(
                self.load_active_thread_goal(&parent_info.session_id).await,
                Ok(Some(_))
            )
        } else {
            false
        };
        if parent_thread_goal_active {
            let parent_session_id = subagent_parent_info
                .as_ref()
                .map(|info| info.session_id.as_str())
                .unwrap_or("-");
            debug!(
                "Subagent timeout disabled by default for active goal mode: agent_type={}, parent_session_id={}",
                agent_type, parent_session_id
            );
        }
        let timeout_seconds = effective_subagent_timeout_seconds(
            requested_timeout_seconds,
            parent_thread_goal_active,
        );
        let timeout_error_message = match timeout_seconds.or(requested_timeout_seconds) {
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
        let subagent_started_at = Instant::now();
        let parent_session_id = subagent_parent_info
            .as_ref()
            .map(|info| info.session_id.as_str())
            .unwrap_or("-");
        let parent_dialog_turn_id = subagent_parent_info
            .as_ref()
            .map(|info| info.dialog_turn_id.as_str())
            .unwrap_or("-");
        let parent_tool_call_id = subagent_parent_info
            .as_ref()
            .map(|info| info.tool_call_id.as_str())
            .unwrap_or("-");

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
                self.cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
                    prepared_target_session_id.clone(),
                    prepared_session_created,
                )
                .await;
                return Err(BitFunError::Cancelled(
                    "Subagent task has been cancelled".to_string(),
                ));
            }
        }

        // Acquire execution capacity before starting the subagent turn. The
        // target hidden session may have been created by the scheduler before
        // this point so per-session queueing can use its real session_id.
        let (permits, wait_ms) = match self
            .acquire_subagent_concurrency_permit(
                &agent_type,
                context_profile_policy.subagent_concurrency_cap,
                cancel_token,
                initial_deadline,
            )
            .await
        {
            Ok(value) => value,
            Err(error) => {
                self.cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
                    prepared_target_session_id.clone(),
                    prepared_session_created,
                )
                .await;
                return Err(error);
            }
        };
        let _permit_guard = SubagentConcurrencyPermitGuard::new(permits, agent_type.clone());

        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                debug!(
                    "Subagent task cancelled after waiting for concurrency slot: agent_type={}",
                    agent_type
                );
                self.cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
                    prepared_target_session_id.clone(),
                    prepared_session_created,
                )
                .await;
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
            self.cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
                prepared_target_session_id.clone(),
                prepared_session_created,
            )
            .await;
            return Err(BitFunError::Timeout(timeout_error_message.clone()));
        }

        let session = match target_session_id {
            Some(target_session_id) => match self.session_manager.get_session(&target_session_id) {
                Some(session) => {
                    if session.kind != session_kind {
                        let error = if session_kind == SessionKind::Subagent {
                            BitFunError::Validation(format!(
                                "Subagent execution target must be a subagent session: {}",
                                target_session_id
                            ))
                        } else {
                            BitFunError::Validation(format!(
                                "Hidden agent execution target has unexpected kind: {}",
                                target_session_id
                            ))
                        };
                        self.cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
                            prepared_target_session_id.clone(),
                            prepared_session_created,
                        )
                        .await;
                        return Err(error);
                    }
                    session
                }
                None => {
                    let error = BitFunError::NotFound(format!(
                        "Subagent session not found: {}",
                        target_session_id
                    ));
                    self.cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
                        prepared_target_session_id.clone(),
                        prepared_session_created,
                    )
                    .await;
                    return Err(error);
                }
            },
            None => {
                let session = self
                    .create_hidden_agent_session(
                        None,
                        session_name.clone(),
                        agent_type.clone(),
                        session_config.clone(),
                        created_by.clone(),
                        session_kind,
                    )
                    .await?;
                let session_id = session.session_id.clone();
                if let Some(source_session_id) = prompt_cache_source_session_id.as_deref() {
                    let copied = self
                        .session_manager
                        .clone_prompt_cache(source_session_id, &session_id)
                        .await;
                    debug!(
                        "Forked prompt cache into hidden agent session: source_session_id={}, session_id={}, copied={}",
                        source_session_id, session_id, copied
                    );
                    self.session_manager
                        .seed_forked_skill_agent_listing_baselines(source_session_id, &session_id)
                        .await;
                }
                self.session_manager
                    .replace_context_messages(&session_id, initial_messages.clone())
                    .await;
                session
            }
        };
        let session_id = session.session_id.clone();
        // Sync context window from AI config so subagents with large-context
        // models are not prematurely capped at SessionConfig::default()'s 128128.
        if let Err(error) = self
            .session_manager
            .refresh_session_context_window(&session_id)
            .await
        {
            self.cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
                Some(session_id.clone()),
                prepared_session_created,
            )
            .await;
            return Err(error);
        }
        drop(session_name);
        drop(session_config);
        drop(created_by);
        drop(prompt_cache_source_session_id);
        if let Err(error) = self
            .session_manager
            .persist_session_lineage(
                &session_id,
                build_subagent_session_relationship(subagent_parent_info.as_ref(), &agent_type),
            )
            .await
        {
            self.cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
                Some(session_id.clone()),
                prepared_session_created,
            )
            .await;
            return Err(error);
        }

        // Register timeout handle so it can be adjusted at runtime.
        let timeout_handle = Arc::new(SubagentTimeoutHandle {
            deadline_tx: deadline_tx.clone(),
            session_id: session_id.clone(),
            original_timeout_seconds: requested_timeout_seconds,
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
                self.cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
                    Some(session_id.clone()),
                    prepared_session_created,
                )
                .await;
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
            self.cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
                Some(session_id.clone()),
                prepared_session_created,
            )
            .await;
            let mut registry = self.subagent_timeout_registry.write().await;
            registry.remove(&session_id);
            return Err(BitFunError::Timeout(timeout_error_message.clone()));
        }

        let turn_index = self.session_manager.get_turn_count(&session_id);
        let requested_dialog_turn_id =
            dialog_turn_id.unwrap_or_else(|| format!("subagent-{}", uuid::Uuid::new_v4()));
        let dialog_turn_id = self
            .session_manager
            .start_dialog_turn_with_existing_context(
                &session_id,
                agent_type.clone(),
                user_input_text.clone(),
                Some(requested_dialog_turn_id.clone()),
                None,
            )
            .await?;
        debug!(
            "Generated unique dialog_turn_id for subagent: {}",
            dialog_turn_id
        );
        self.persist_reused_subagent_user_input_context_if_needed(
            prepared_target_session_id.as_deref(),
            prepared_session_created,
            &session_id,
            &dialog_turn_id,
            &user_input_text,
        )
        .await?;
        if let Some(parent_info) = subagent_parent_info.as_ref() {
            if emit_lifecycle_events {
                self.emit_event(AgenticEvent::SubagentSessionLinked {
                    session_id: session_id.clone(),
                    subagent_dialog_turn_id: dialog_turn_id.clone(),
                    parent_session_id: parent_info.session_id.clone(),
                    parent_dialog_turn_id: parent_info.dialog_turn_id.clone(),
                    parent_tool_call_id: parent_info.tool_call_id.clone(),
                    agent_type: Some(agent_type.clone()),
                })
                .await;
            }
        }

        // Register a dedicated subagent token so both external cancellation and
        // coordinator-enforced timeouts can stop the same dialog turn.
        let subagent_cancel_token = cancel_token
            .map(CancellationToken::child_token)
            .unwrap_or_default();
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

        self.session_manager
            .update_session_state_for_turn_if_processing(
                &session_id,
                &dialog_turn_id,
                SessionState::Processing {
                    current_turn_id: dialog_turn_id.clone(),
                    phase: ProcessingPhase::Thinking,
                },
            )
            .await?;

        if emit_lifecycle_events {
            // Emit DialogTurnStarted after the dedicated linking event.
            self.emit_event(AgenticEvent::DialogTurnStarted {
                session_id: session_id.clone(),
                turn_id: dialog_turn_id.clone(),
                turn_index,
                user_input: user_input_text.clone(),
                original_user_input: None,
                user_message_metadata: None,
            })
            .await;
        }

        let subagent_workspace = Self::build_workspace_binding(&session.config).await;
        let subagent_workspace_path = subagent_workspace
            .as_ref()
            .map(|workspace| workspace.root_path_string());
        let subagent_session_storage_path = subagent_workspace
            .as_ref()
            .map(|workspace| workspace.session_storage_dir().to_path_buf());
        if subagent_cancel_token.is_cancelled() {
            debug!(
                "Subagent task cancelled after dialog turn registration: agent_type={}, session_id={}, dialog_turn_id={}",
                agent_type, session_id, dialog_turn_id
            );
            Self::persist_cancelled_dialog_turn(
                &self.event_queue,
                &self.session_manager,
                None,
                &session_id,
                &dialog_turn_id,
                emit_lifecycle_events,
            )
            .await;
            let _ = self.cleanup_subagent_resources(&session_id).await;
            let mut registry = self.subagent_timeout_registry.write().await;
            registry.remove(&session_id);
            return Err(BitFunError::Cancelled(
                "Subagent task has been cancelled".to_string(),
            ));
        }
        let subagent_services = Self::build_workspace_services(&subagent_workspace).await;
        let execution_context = ExecutionContext {
            session_id: session_id.clone(),
            dialog_turn_id: dialog_turn_id.clone(),
            turn_index,
            agent_type: agent_type.clone(),
            workspace: subagent_workspace,
            context,
            subagent_parent_info: subagent_parent_info.clone(),
            delegation_policy,
            // Subagents run autonomously without user interaction; always skip
            // tool confirmation to prevent them from blocking indefinitely on a
            // confirmation channel that nobody will ever respond to.
            skip_tool_confirmation: true,
            runtime_tool_restrictions,
            workspace_services: subagent_services,
            terminal_port: self.terminal_port(),
            remote_exec_port: self.remote_exec_port(),
            // Subagents are autonomous; user steering is targeted at top-level
            // dialog turns only. Leave None so we don't intercept buffer entries
            // that belong to a different (parent) session/turn.
            round_injection: None,
            emit_lifecycle_events,
            recover_partial_on_cancel: true,
        };

        let execution_engine = self.execution_engine.clone();
        let tool_pipeline = self.tool_pipeline.clone();
        let agent_type_for_execution = agent_type.clone();
        debug!(
            "Subagent execution task starting: agent_type={}, session_id={}, dialog_turn_id={}, parent_session_id={}, parent_dialog_turn_id={}, parent_tool_call_id={}, timeout_seconds={:?}, wait_ms={}",
            agent_type,
            session_id,
            dialog_turn_id,
            parent_session_id,
            parent_dialog_turn_id,
            parent_tool_call_id,
            timeout_seconds,
            wait_ms
        );
        let mut execution_task = tokio::spawn(async move {
            execution_engine
                .execute_dialog_turn(
                    agent_type_for_execution,
                    initial_messages,
                    execution_context,
                )
                .await
        });
        let abort_handle = execution_task.abort_handle();

        if subagent_parent_info.is_some() {
            self.active_subagent_executions.insert(
                session_id.clone(),
                ActiveSubagentExecution {
                    parent_session_id: parent_session_id.to_string(),
                    parent_dialog_turn_id: parent_dialog_turn_id.to_string(),
                    subagent_session_id: session_id.clone(),
                    subagent_dialog_turn_id: dialog_turn_id.clone(),
                    cancel_token: subagent_cancel_token.clone(),
                    abort_handle: abort_handle.clone(),
                },
            );
        }

        let mut execution_scope = SubagentExecutionScope {
            execution_engine: self.execution_engine.clone(),
            tool_pipeline: self.tool_pipeline.clone(),
            session_manager: self.session_manager.clone(),
            active_subagent_executions: self.active_subagent_executions.clone(),
            subagent_session_id: session_id.clone(),
            subagent_dialog_turn_id: dialog_turn_id.clone(),
            subagent_cancel_token: subagent_cancel_token.clone(),
            abort_handle,
            disarmed: false,
        };

        enum SubagentExecutionOutcome<T> {
            Completed(T),
            Cancelled,
            TimedOut,
        }

        // Dynamic timeout loop: deadline can be adjusted via watch channel.
        let execution_outcome = loop {
            let current_deadline = *deadline_rx.borrow_and_update();
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

        let execution_outcome_label = match &execution_outcome {
            SubagentExecutionOutcome::Completed(_) => "completed",
            SubagentExecutionOutcome::Cancelled => "cancelled",
            SubagentExecutionOutcome::TimedOut => "timed_out",
        };
        debug!(
            "Subagent execution outcome resolved: agent_type={}, session_id={}, dialog_turn_id={}, parent_session_id={}, parent_dialog_turn_id={}, parent_tool_call_id={}, outcome={}, duration_ms={}",
            agent_type,
            session_id,
            dialog_turn_id,
            parent_session_id,
            parent_dialog_turn_id,
            parent_tool_call_id,
            execution_outcome_label,
            subagent_started_at.elapsed().as_millis()
        );

        let result = match execution_outcome {
            SubagentExecutionOutcome::Completed(join_result) => match join_result {
                Ok(result) => result,
                Err(error) => {
                    let join_error = BitFunError::tool(format!(
                        "Subagent '{}' failed to join: {}",
                        agent_type, error
                    ));
                    Self::persist_failed_dialog_turn(
                        self.event_queue.as_ref(),
                        self.session_manager.as_ref(),
                        None,
                        &session_id,
                        &dialog_turn_id,
                        &join_error,
                        emit_lifecycle_events,
                    )
                    .await;
                    Self::finalize_persisted_turn_in_workspace_if_needed(
                        self.session_manager.as_ref(),
                        &session_id,
                        &dialog_turn_id,
                        turn_index,
                        &agent_type,
                        &user_input_text,
                        subagent_workspace_path.as_deref(),
                        subagent_session_storage_path.as_deref(),
                        Some(crate::service::session::TurnStatus::Error),
                        None,
                    )
                    .await;
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

                    execution_scope.disarm();
                    return Err(join_error);
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

                Self::persist_cancelled_dialog_turn(
                    self.event_queue.as_ref(),
                    self.session_manager.as_ref(),
                    None,
                    &session_id,
                    &dialog_turn_id,
                    emit_lifecycle_events,
                )
                .await;
                Self::finalize_persisted_turn_in_workspace_if_needed(
                    self.session_manager.as_ref(),
                    &session_id,
                    &dialog_turn_id,
                    turn_index,
                    &agent_type,
                    &user_input_text,
                    subagent_workspace_path.as_deref(),
                    subagent_session_storage_path.as_deref(),
                    Some(crate::service::session::TurnStatus::Cancelled),
                    None,
                )
                .await;

                if let Err(cleanup_err) = self.cleanup_subagent_resources(&session_id).await {
                    warn!(
                        "Failed to cleanup subagent resources after cancellation: session={}, error={}",
                        session_id, cleanup_err
                    );
                }
                let mut registry = self.subagent_timeout_registry.write().await;
                registry.remove(&session_id);

                execution_scope.disarm();
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
                        let (_status, response_text) = Self::persist_completed_dialog_turn(
                            self.session_manager.as_ref(),
                            None,
                            &session_id,
                            &dialog_turn_id,
                            &exec_result,
                        )
                        .await;
                        Self::finalize_persisted_turn_in_workspace_if_needed(
                            self.session_manager.as_ref(),
                            &session_id,
                            &dialog_turn_id,
                            turn_index,
                            &agent_type,
                            &user_input_text,
                            subagent_workspace_path.as_deref(),
                            subagent_session_storage_path.as_deref(),
                            Some(crate::service::session::TurnStatus::Completed),
                            None,
                        )
                        .await;
                        if response_text.trim().is_empty() {
                            None
                        } else {
                            Some(SubagentResult::partial_timeout(
                                response_text,
                                timeout_error_message.clone(),
                            ))
                            .map(|result| result.with_session_id(session_id.clone()))
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

                    execution_scope.disarm();
                    return Ok(partial_result);
                }

                let timeout_error = BitFunError::Timeout(timeout_error_message.clone());
                Self::persist_failed_dialog_turn(
                    self.event_queue.as_ref(),
                    self.session_manager.as_ref(),
                    None,
                    &session_id,
                    &dialog_turn_id,
                    &timeout_error,
                    emit_lifecycle_events,
                )
                .await;
                Self::finalize_persisted_turn_in_workspace_if_needed(
                    self.session_manager.as_ref(),
                    &session_id,
                    &dialog_turn_id,
                    turn_index,
                    &agent_type,
                    &user_input_text,
                    subagent_workspace_path.as_deref(),
                    subagent_session_storage_path.as_deref(),
                    Some(crate::service::session::TurnStatus::Error),
                    None,
                )
                .await;

                if let Err(cleanup_err) = self.cleanup_subagent_resources(&session_id).await {
                    warn!(
                        "Failed to cleanup subagent resources after timeout: session={}, error={}",
                        session_id, cleanup_err
                    );
                }
                let mut registry = self.subagent_timeout_registry.write().await;
                registry.remove(&session_id);

                execution_scope.disarm();
                return Err(BitFunError::Timeout(timeout_error_message.clone()));
            }
        };

        // cleanup_guard automatically cleans up token on scope exit (via Drop trait)

        // Persist turn lifecycle before cleaning up the hidden subagent runtime.
        let (workspace_turn_status, response_text) = match result {
            Ok(exec_result) => {
                Self::persist_completed_dialog_turn(
                    self.session_manager.as_ref(),
                    None,
                    &session_id,
                    &dialog_turn_id,
                    &exec_result,
                )
                .await
            }
            Err(e) => {
                let turn_status = if matches!(&e, BitFunError::Cancelled(_)) {
                    Self::persist_cancelled_dialog_turn(
                        self.event_queue.as_ref(),
                        self.session_manager.as_ref(),
                        None,
                        &session_id,
                        &dialog_turn_id,
                        emit_lifecycle_events,
                    )
                    .await
                } else {
                    Self::persist_failed_dialog_turn(
                        self.event_queue.as_ref(),
                        self.session_manager.as_ref(),
                        None,
                        &session_id,
                        &dialog_turn_id,
                        &e,
                        emit_lifecycle_events,
                    )
                    .await
                };
                Self::finalize_persisted_turn_in_workspace_if_needed(
                    self.session_manager.as_ref(),
                    &session_id,
                    &dialog_turn_id,
                    turn_index,
                    &agent_type,
                    &user_input_text,
                    subagent_workspace_path.as_deref(),
                    subagent_session_storage_path.as_deref(),
                    Some(turn_status),
                    None,
                )
                .await;
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

                execution_scope.disarm();
                return Err(e);
            }
        };
        Self::finalize_persisted_turn_in_workspace_if_needed(
            self.session_manager.as_ref(),
            &session_id,
            &dialog_turn_id,
            turn_index,
            &agent_type,
            &user_input_text,
            subagent_workspace_path.as_deref(),
            subagent_session_storage_path.as_deref(),
            Some(workspace_turn_status),
            None,
        )
        .await;

        // Clean up subagent session resources after successful execution
        debug!(
            "Subagent successful execution produced final text: agent_type={}, session_id={}, dialog_turn_id={}, parent_session_id={}, parent_dialog_turn_id={}, parent_tool_call_id={}, text_len={}, duration_ms={}",
            agent_type,
            session_id,
            dialog_turn_id,
            parent_session_id,
            parent_dialog_turn_id,
            parent_tool_call_id,
            response_text.len(),
            subagent_started_at.elapsed().as_millis()
        );
        let cleanup_started_at = Instant::now();
        debug!(
            "Subagent cleanup starting after successful execution: agent_type={}, session_id={}, dialog_turn_id={}, parent_session_id={}, parent_dialog_turn_id={}, parent_tool_call_id={}",
            agent_type,
            session_id,
            dialog_turn_id,
            parent_session_id,
            parent_dialog_turn_id,
            parent_tool_call_id
        );
        if let Err(e) = self.cleanup_subagent_resources(&session_id).await {
            warn!(
                "Failed to cleanup subagent resources: session={}, error={}",
                session_id, e
            );
        } else {
            debug!(
                "Subagent cleanup completed after successful execution: agent_type={}, session_id={}, dialog_turn_id={}, parent_session_id={}, parent_dialog_turn_id={}, parent_tool_call_id={}, cleanup_duration_ms={}",
                agent_type,
                session_id,
                dialog_turn_id,
                parent_session_id,
                parent_dialog_turn_id,
                parent_tool_call_id,
                cleanup_started_at.elapsed().as_millis()
            );
        }
        debug!(
            "Subagent timeout registry removal starting: agent_type={}, session_id={}, dialog_turn_id={}",
            agent_type, session_id, dialog_turn_id
        );
        let mut registry = self.subagent_timeout_registry.write().await;
        registry.remove(&session_id);
        debug!(
            "Subagent timeout registry removal completed: agent_type={}, session_id={}, dialog_turn_id={}, total_duration_ms={}",
            agent_type,
            session_id,
            dialog_turn_id,
            subagent_started_at.elapsed().as_millis()
        );

        debug!(
            "Subagent result returning to caller: agent_type={}, session_id={}, dialog_turn_id={}, parent_session_id={}, parent_dialog_turn_id={}, parent_tool_call_id={}, status=completed, text_len={}, total_duration_ms={}",
            agent_type,
            session_id,
            dialog_turn_id,
            parent_session_id,
            parent_dialog_turn_id,
            parent_tool_call_id,
            response_text.len(),
            subagent_started_at.elapsed().as_millis()
        );
        execution_scope.disarm();
        Ok(SubagentResult::completed(response_text).with_session_id(session_id))
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
        let mut child_session = self
            .session_manager
            .create_session_with_id_and_details(
                Some(child_session_id.to_string()),
                session_name,
                snapshot.parent_agent_type.clone(),
                snapshot.build_child_session_config(None),
                Some(format!("session-{}", snapshot.parent_session_id)),
                SessionKind::EphemeralChild,
            )
            .await?;
        self.session_manager
            .inherit_session_agent_type_state(
                &child_session.session_id,
                snapshot.last_user_dialog_agent_type.clone(),
                snapshot.last_submitted_agent_type.clone(),
            )
            .await?;
        child_session.last_user_dialog_agent_type = snapshot.last_user_dialog_agent_type.clone();
        child_session.last_submitted_agent_type = snapshot.last_submitted_agent_type.clone();

        let copied = self
            .session_manager
            .clone_prompt_cache(parent_session_id, &child_session.session_id)
            .await;
        debug!(
            "Forked prompt cache into /btw child session: parent_session_id={}, child_session_id={}, copied={}",
            parent_session_id, child_session.session_id, copied
        );
        self.session_manager
            .seed_forked_skill_agent_listing_baselines(parent_session_id, &child_session.session_id)
            .await;

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
        image_contexts: Option<Vec<ImageContextData>>,
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
        let mut user_message_metadata = serde_json::json!({
            "kind": "btw",
            "parentSessionId": parent_session_id,
        });
        if let Some(images) = image_contexts.as_ref().filter(|images| !images.is_empty()) {
            user_message_metadata["images"] = serde_json::Value::Array(
                images
                    .iter()
                    .map(|image| {
                        let name = image
                            .metadata
                            .as_ref()
                            .and_then(|metadata| metadata.get("name"))
                            .and_then(|value| value.as_str())
                            .filter(|name| !name.trim().is_empty())
                            .unwrap_or("image");
                        serde_json::json!({
                            "id": image.id,
                            "name": name,
                            "data_url": image.data_url,
                            "image_path": image.image_path,
                            "mime_type": image.mime_type,
                        })
                    })
                    .collect(),
            );
        }

        let (user_input, prepended_messages) = build_btw_user_input(question);

        self.start_dialog_turn_internal(
            child_session_id.to_string(),
            user_input,
            Some(question.trim().to_string()),
            image_contexts,
            Some(turn_id.clone()),
            child_session.agent_type.clone(),
            child_session.config.workspace_path.clone(),
            child_session.config.remote_connection_id.clone(),
            child_session.config.remote_ssh_host.clone(),
            DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopApi)
                .with_skip_tool_confirmation(true),
            Some(user_message_metadata),
            prepended_messages,
            true,
        )
        .await?;

        Ok(turn_id)
    }

    async fn ensure_subagent_session_loaded_for_reuse(
        &self,
        target_session_id: &str,
        parent_session_id: &str,
        background_allow_review_follow_up: Option<bool>,
    ) -> BitFunResult<Session> {
        let session = match self.session_manager.get_session(target_session_id) {
            Some(session) => session,
            None => {
                let binding = self
                    .session_manager
                    .resolve_session_workspace_binding(parent_session_id)
                    .await
                    .ok_or_else(|| {
                        BitFunError::NotFound(format!(
                            "Parent session workspace not found: {}",
                            parent_session_id
                        ))
                    })?;
                if let Some(allow_review_follow_up) = background_allow_review_follow_up {
                    let metadata = self
                        .session_manager
                        .load_session_metadata(&binding.session_storage_dir(), target_session_id)
                        .await?
                        .ok_or_else(|| {
                            BitFunError::NotFound(format!(
                                "Session metadata not found: {}",
                                target_session_id
                            ))
                        })?;
                    if !metadata.is_subagent() {
                        return Err(BitFunError::Validation(format!(
                            "Subagent execution target must be a subagent session: {}",
                            target_session_id
                        )));
                    }
                    let created_by_marker = format!("session-{parent_session_id}");
                    let owned_subagent = session_lineage_matches_parent(
                        metadata.relationship.as_ref(),
                        parent_session_id,
                    ) || metadata.created_by.as_deref()
                        == Some(created_by_marker.as_str());
                    if !owned_subagent {
                        return Err(BitFunError::Validation(format!(
                            "Subagent session '{}' was not created by parent session '{}'",
                            target_session_id, parent_session_id
                        )));
                    }
                    validate_background_subagent_delivery(
                        &metadata.agent_type,
                        metadata
                            .workspace_path
                            .as_deref()
                            .map(Path::new)
                            .or(Some(binding.root_path())),
                        allow_review_follow_up,
                    )
                    .await?;

                    let (session_view, _, _, _) = self
                        .session_manager
                        .restore_internal_session_view_from_storage_path_tail_timed(
                            &binding.session_storage_dir(),
                            target_session_id,
                            0,
                        )
                        .await?;
                    if session_view.kind != SessionKind::Subagent {
                        return Err(BitFunError::Validation(format!(
                            "Subagent execution target must be a subagent session: {}",
                            target_session_id
                        )));
                    }
                    if !session_created_by_parent(&session_view, parent_session_id)
                        && !session_lineage_matches_parent(
                            metadata.relationship.as_ref(),
                            parent_session_id,
                        )
                    {
                        return Err(BitFunError::Validation(format!(
                            "Subagent session '{}' was not created by parent session '{}'",
                            target_session_id, parent_session_id
                        )));
                    }
                    validate_background_subagent_delivery(
                        &session_view.agent_type,
                        session_view
                            .config
                            .workspace_path
                            .as_deref()
                            .map(Path::new)
                            .or(Some(binding.root_path())),
                        allow_review_follow_up,
                    )
                    .await?;
                }
                self.session_manager
                    .restore_internal_session_from_storage_path(
                        &binding.session_storage_dir(),
                        target_session_id,
                    )
                    .await?
            }
        };

        if session.kind != SessionKind::Subagent {
            return Err(BitFunError::Validation(format!(
                "Subagent execution target must be a subagent session: {}",
                target_session_id
            )));
        }

        if !self
            .subagent_session_owned_by_parent(&session, parent_session_id)
            .await
        {
            return Err(BitFunError::Validation(format!(
                "Subagent session '{}' was not created by parent session '{}'",
                target_session_id, parent_session_id
            )));
        }

        if matches!(session.state, SessionState::Error { .. }) {
            return Err(BitFunError::Validation(format!(
                "Subagent session is in error state and cannot be reused: {}",
                target_session_id
            )));
        }

        if let Some(allow_review_follow_up) = background_allow_review_follow_up {
            validate_background_subagent_delivery(
                &session.agent_type,
                session.config.workspace_path.as_deref().map(Path::new),
                allow_review_follow_up,
            )
            .await?;
        }

        Ok(session)
    }

    async fn subagent_session_owned_by_parent(
        &self,
        session: &Session,
        parent_session_id: &str,
    ) -> bool {
        match self
            .restore_path_for_existing_session(&session.session_id)
            .await
        {
            Ok(storage_path) => {
                match self
                    .session_manager
                    .load_session_metadata(&storage_path, &session.session_id)
                    .await
                {
                    Ok(Some(metadata))
                        if session_lineage_matches_parent(
                            metadata.relationship.as_ref(),
                            parent_session_id,
                        ) =>
                    {
                        return true;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        debug!(
                            "Failed to load subagent session metadata for lineage ownership check: session_id={}, parent_session_id={}, error={}",
                            session.session_id, parent_session_id, error
                        );
                    }
                }
            }
            Err(error) => {
                debug!(
                    "Failed to resolve subagent session storage for lineage ownership check: session_id={}, parent_session_id={}, error={}",
                    session.session_id, parent_session_id, error
                );
            }
        }

        session_created_by_parent(session, parent_session_id)
    }

    async fn load_reusable_subagent_context_messages(
        &self,
        session: &Session,
    ) -> BitFunResult<Vec<Message>> {
        let session_id = &session.session_id;
        let mut context_messages = self
            .session_manager
            .get_context_messages(session_id)
            .await?;
        let needs_restore = if context_messages.is_empty() {
            !session.dialog_turn_ids.is_empty()
        } else {
            context_messages.len() == 1 && !session.dialog_turn_ids.is_empty()
        };

        if needs_restore {
            let restore_path = self.restore_path_for_existing_session(session_id).await?;
            self.session_manager
                .restore_internal_session_from_storage_path(&restore_path, session_id)
                .await?;
            context_messages = self
                .session_manager
                .get_context_messages(session_id)
                .await?;
        }

        Ok(context_messages)
    }

    async fn resolve_hidden_subagent_execution_request(
        &self,
        request: SubagentExecutionRequest,
        background_allow_review_follow_up: Option<bool>,
    ) -> BitFunResult<HiddenSubagentExecutionRequest> {
        let task_description = request.task_description.trim().to_string();
        if task_description.is_empty() {
            return Err(BitFunError::Validation(
                "task_description is required when creating a subagent session".to_string(),
            ));
        }

        let model_id = request
            .model_id
            .as_deref()
            .map(str::trim)
            .filter(|model_id| !model_id.is_empty())
            .map(str::to_string);
        let created_by = Some(format!(
            "session-{}",
            request.subagent_parent_info.session_id
        ));

        match request.context_mode {
            SubagentContextMode::Fresh => {
                if let Some(target_session_id) = request.target_session_id.as_deref() {
                    if request.subagent_type.is_some() {
                        return Err(BitFunError::Validation(
                            "subagent_type is not allowed when target_session_id is provided"
                                .to_string(),
                        ));
                    }
                    if request.workspace_path.is_some() {
                        return Err(BitFunError::Validation(
                            "workspace_path is not allowed when target_session_id is provided"
                                .to_string(),
                        ));
                    }

                    let parent_session_id = request.subagent_parent_info.session_id.clone();
                    let mut session = self
                        .ensure_subagent_session_loaded_for_reuse(
                            target_session_id,
                            &parent_session_id,
                            background_allow_review_follow_up,
                        )
                        .await?;
                    if let Some(model_id) = model_id.as_deref() {
                        let session_id = session.session_id.clone();
                        self.session_manager
                            .update_session_model_id(&session_id, model_id)
                            .await?;
                        session =
                            self.session_manager
                                .get_session(&session_id)
                                .ok_or_else(|| {
                                    BitFunError::NotFound(format!(
                                        "Subagent session not found after model update: {}",
                                        session_id
                                    ))
                                })?;
                    }
                    let mut initial_messages = self
                        .load_reusable_subagent_context_messages(&session)
                        .await?;
                    initial_messages.push(Message::user(task_description.clone()));

                    return Ok(HiddenSubagentExecutionRequest {
                        target_session_id: Some(session.session_id.clone()),
                        dialog_turn_id: None,
                        session_name: session.session_name.clone(),
                        agent_type: session.agent_type.clone(),
                        session_config: session.config.clone(),
                        initial_messages,
                        user_input_text: task_description,
                        created_by: session.created_by.clone(),
                        subagent_parent_info: Some(request.subagent_parent_info),
                        context: request.context,
                        delegation_policy: request.delegation_policy,
                        runtime_tool_restrictions: runtime_tool_restrictions_for_delegation_policy(
                            request.delegation_policy,
                        ),
                        prompt_cache_source_session_id: None,
                        session_kind: SessionKind::Subagent,
                        emit_lifecycle_events: true,
                        prepared_session_created: false,
                    });
                }

                let agent_type = request.subagent_type.ok_or_else(|| {
                    BitFunError::Validation(
                        "subagent_type is required when context_mode is 'fresh'".to_string(),
                    )
                })?;
                let workspace_path = request.workspace_path.ok_or_else(|| {
                    BitFunError::Validation(
                        "workspace_path is required when creating a fresh subagent session"
                            .to_string(),
                    )
                })?;
                if let Some(allow_review_follow_up) = background_allow_review_follow_up {
                    validate_background_subagent_delivery(
                        &agent_type,
                        Some(Path::new(&workspace_path)),
                        allow_review_follow_up,
                    )
                    .await?;
                }

                Ok(HiddenSubagentExecutionRequest {
                    target_session_id: None,
                    dialog_turn_id: None,
                    session_name: format!("Subagent: {}", task_description),
                    agent_type,
                    session_config: Self::build_session_config_for_workspace(
                        workspace_path,
                        model_id,
                    )
                    .await,
                    initial_messages: vec![Message::user(task_description.clone())],
                    user_input_text: task_description,
                    created_by,
                    subagent_parent_info: Some(request.subagent_parent_info),
                    context: request.context,
                    delegation_policy: request.delegation_policy,
                    runtime_tool_restrictions: runtime_tool_restrictions_for_delegation_policy(
                        request.delegation_policy,
                    ),
                    prompt_cache_source_session_id: None,
                    session_kind: SessionKind::Subagent,
                    emit_lifecycle_events: true,
                    prepared_session_created: false,
                })
            }
            SubagentContextMode::Fork => {
                if request.target_session_id.is_some() {
                    return Err(BitFunError::Validation(
                        "target_session_id is not allowed when context_mode is 'fork'".to_string(),
                    ));
                }
                if request.subagent_type.is_some() {
                    return Err(BitFunError::Validation(
                        "subagent_type is not allowed when context_mode is 'fork'".to_string(),
                    ));
                }
                if request.workspace_path.is_some() {
                    return Err(BitFunError::Validation(
                        "workspace_path is not allowed when context_mode is 'fork'".to_string(),
                    ));
                }
                let snapshot = self
                    .capture_fork_agent_context_snapshot(&request.subagent_parent_info.session_id)
                    .await?;
                if let Some(allow_review_follow_up) = background_allow_review_follow_up {
                    validate_background_subagent_delivery(
                        &snapshot.parent_agent_type,
                        Some(Path::new(&snapshot.workspace_path)),
                        allow_review_follow_up,
                    )
                    .await?;
                }
                let mut session_config = snapshot.build_child_session_config(None);
                if let Some(model_id) = model_id {
                    session_config.model_id = Some(model_id);
                }
                let mut initial_messages = snapshot.messages.clone();
                initial_messages.push(Message::internal_reminder(
                    InternalReminderKind::ForkSubagent,
                    fork_subagent_system_reminder(),
                ));
                initial_messages.push(Message::user(task_description.clone()));

                Ok(HiddenSubagentExecutionRequest {
                    target_session_id: None,
                    dialog_turn_id: None,
                    session_name: format!("Fork: {}", task_description),
                    agent_type: snapshot.parent_agent_type.clone(),
                    session_config,
                    initial_messages,
                    user_input_text: task_description,
                    created_by,
                    subagent_parent_info: Some(request.subagent_parent_info),
                    context: request.context,
                    delegation_policy: request.delegation_policy,
                    runtime_tool_restrictions: runtime_tool_restrictions_for_delegation_policy(
                        request.delegation_policy,
                    ),
                    prompt_cache_source_session_id: Some(snapshot.parent_session_id),
                    session_kind: SessionKind::Subagent,
                    emit_lifecycle_events: true,
                    prepared_session_created: false,
                })
            }
        }
    }

    pub(super) async fn prepare_hidden_subagent_execution_request(
        &self,
        mut request: HiddenSubagentExecutionRequest,
    ) -> BitFunResult<HiddenSubagentExecutionRequest> {
        if let Some(target_session_id) = request.target_session_id.as_deref() {
            let session = self
                .session_manager
                .get_session(target_session_id)
                .ok_or_else(|| {
                    BitFunError::NotFound(format!(
                        "Subagent session not found: {}",
                        target_session_id
                    ))
                })?;
            if session.kind != SessionKind::Subagent {
                return Err(BitFunError::Validation(format!(
                    "Subagent execution target must be a subagent session: {}",
                    target_session_id
                )));
            }
            return Ok(request);
        }

        let session = self
            .create_hidden_agent_session(
                None,
                request.session_name.clone(),
                request.agent_type.clone(),
                request.session_config.clone(),
                request.created_by.clone(),
                request.session_kind,
            )
            .await?;
        let session_id = session.session_id.clone();

        if let Some(source_session_id) = request.prompt_cache_source_session_id.as_deref() {
            let copied = self
                .session_manager
                .clone_prompt_cache(source_session_id, &session_id)
                .await;
            debug!(
                "Forked prompt cache into subagent session: source_session_id={}, session_id={}, copied={}",
                source_session_id, session_id, copied
            );
            self.session_manager
                .seed_forked_skill_agent_listing_baselines(source_session_id, &session_id)
                .await;
        }
        self.session_manager
            .replace_context_messages(&session_id, request.initial_messages.clone())
            .await;

        request.target_session_id = Some(session_id);
        request.prepared_session_created = true;
        Ok(request)
    }

    pub(crate) async fn cleanup_prepared_hidden_subagent_session_if_unsubmitted(
        &self,
        request: &HiddenSubagentExecutionRequest,
    ) {
        self.cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
            request
                .prepared_session_id_created_by_this_request()
                .map(str::to_owned),
            request.prepared_session_created,
        )
        .await;
    }

    async fn cleanup_prepared_hidden_subagent_session_id_if_unsubmitted(
        &self,
        session_id: Option<String>,
        prepared_session_created: bool,
    ) {
        if !prepared_session_created {
            return;
        }
        let Some(session_id) = session_id else {
            return;
        };
        if let Err(error) = self.session_manager.delete_session_by_id(&session_id).await {
            warn!(
                "Failed to clean up unsubmitted hidden subagent session: session_id={}, error={}",
                session_id, error
            );
        }
    }

    pub(crate) async fn prepare_subagent_execution_request(
        &self,
        request: SubagentExecutionRequest,
    ) -> BitFunResult<HiddenSubagentExecutionRequest> {
        let request = self
            .resolve_hidden_subagent_execution_request(request, None)
            .await?;
        self.prepare_hidden_subagent_execution_request(request)
            .await
    }

    /// Execute subagent task directly
    /// DialogTurnStarted event not needed for now
    ///
    /// Returns SubagentResult with the final text response
    pub(super) async fn execute_prepared_hidden_subagent(
        &self,
        request: HiddenSubagentExecutionRequest,
        cancel_token: Option<&CancellationToken>,
        timeout_seconds: Option<u64>,
    ) -> BitFunResult<SubagentResult> {
        self.execute_hidden_subagent_internal(request, cancel_token, timeout_seconds)
            .await
    }

    async fn await_hidden_subagent_receiver(
        receiver: tokio::sync::oneshot::Receiver<BitFunResult<SubagentResult>>,
    ) -> BitFunResult<SubagentResult> {
        receiver
            .await
            .map_err(|_| BitFunError::tool("Subagent result channel closed".to_string()))?
    }

    async fn await_hidden_subagent_cancellation(
        receiver: impl std::future::Future<Output = BitFunResult<SubagentResult>>,
        wait_timeout: Duration,
    ) -> BitFunResult<SubagentResult> {
        match tokio::time::timeout(wait_timeout, receiver).await {
            Ok(result) => result,
            Err(_) => Err(BitFunError::Cancelled(
                "Subagent task has been cancelled".to_string(),
            )),
        }
    }

    fn register_background_subagent_task(
        &self,
        background_task_id: String,
        parent_session_id: String,
        subagent_session_id: String,
        cancel_target: BackgroundSubagentCancelTarget,
    ) -> Arc<AtomicBool> {
        let suppress_delivery = Arc::new(AtomicBool::new(false));
        self.background_subagent_tasks.insert(
            background_task_id,
            BackgroundSubagentTaskControl {
                parent_session_id,
                subagent_session_id,
                suppress_delivery: suppress_delivery.clone(),
                cancel_target,
            },
        );
        suppress_delivery
    }

    #[cfg(test)]
    pub(crate) fn register_background_subagent_task_for_test(
        &self,
        background_task_id: &str,
        parent_session_id: &str,
        subagent_session_id: &str,
    ) {
        self.register_background_subagent_task(
            background_task_id.to_string(),
            parent_session_id.to_string(),
            subagent_session_id.to_string(),
            BackgroundSubagentCancelTarget::Direct(CancellationToken::new()),
        );
    }

    #[cfg(test)]
    pub(crate) fn has_background_subagent_task_for_test(&self, background_task_id: &str) -> bool {
        self.background_subagent_tasks
            .contains_key(background_task_id)
    }

    #[cfg(test)]
    pub(crate) fn suppress_background_subagent_task_for_test(
        &self,
        background_task_id: &str,
    ) -> bool {
        self.background_subagent_tasks
            .remove_if(background_task_id, |task_id, control| {
                control.suppress_delivery.store(true, Ordering::SeqCst);
                self.mark_background_subagent_delivery_suppression(task_id.clone());
                true
            })
            .is_some()
    }

    pub(crate) async fn cancel_background_subagents_for_parent(
        &self,
        parent_session_id: &str,
        subagent_session_id: &str,
    ) -> BitFunResult<usize> {
        self.ensure_subagent_session_loaded_for_reuse(subagent_session_id, parent_session_id, None)
            .await?;

        let controls = self.claim_background_subagent_controls(|control| {
            control.parent_session_id == parent_session_id
                && control.subagent_session_id == subagent_session_id
        });
        let background_task_ids = controls
            .iter()
            .map(|(background_task_id, _)| background_task_id.clone())
            .collect::<Vec<_>>();
        let cancelled = self.cancel_background_subagent_controls(controls).await?;
        if let Some(scheduler) = get_global_scheduler() {
            scheduler
                .cancel_background_result_deliveries(parent_session_id, &background_task_ids)
                .await
                .map_err(BitFunError::tool)?;
        }
        Ok(cancelled)
    }

    pub(crate) async fn cancel_background_subagents_for_parent_session(
        &self,
        parent_session_id: &str,
    ) -> BitFunResult<usize> {
        let controls = self.claim_background_subagent_controls(|control| {
            control.parent_session_id == parent_session_id
        });

        self.cancel_background_subagent_controls(controls).await
    }

    pub(crate) fn take_background_subagent_delivery_suppression(
        &self,
        background_task_id: &str,
    ) -> bool {
        self.background_subagent_delivery_suppressions
            .remove(background_task_id)
            .is_some()
    }

    pub(crate) fn mark_background_subagent_delivery_suppression(&self, background_task_id: String) {
        self.background_subagent_delivery_suppressions
            .insert(background_task_id, ());
    }

    pub(crate) fn finish_background_subagent_delivery(&self, background_task_id: &str) {
        self.background_subagent_tasks.remove(background_task_id);
        self.background_subagent_delivery_suppressions
            .remove(background_task_id);
    }

    pub(crate) fn background_subagent_control_available_for_injection(
        &self,
        background_task_id: &str,
    ) -> bool {
        self.background_subagent_tasks
            .get(background_task_id)
            .is_some_and(|control| !control.suppress_delivery.load(Ordering::SeqCst))
    }

    pub(crate) fn claim_background_subagent_control_for_injection(
        &self,
        background_task_id: &str,
    ) -> bool {
        self.background_subagent_tasks
            .remove_if(background_task_id, |_, control| {
                !control.suppress_delivery.load(Ordering::SeqCst)
            })
            .is_some()
    }

    fn claim_background_subagent_controls(
        &self,
        matches: impl Fn(&BackgroundSubagentTaskControl) -> bool,
    ) -> Vec<(String, BackgroundSubagentTaskControl)> {
        let candidate_ids = self
            .background_subagent_tasks
            .iter()
            .filter(|entry| matches(entry.value()))
            .map(|entry| entry.key().clone())
            .collect::<Vec<_>>();
        candidate_ids
            .into_iter()
            .filter_map(|background_task_id| {
                self.background_subagent_tasks
                    .remove_if(&background_task_id, |task_id, control| {
                        if !matches(control) {
                            return false;
                        }
                        control.suppress_delivery.store(true, Ordering::SeqCst);
                        self.mark_background_subagent_delivery_suppression(task_id.clone());
                        true
                    })
            })
            .collect()
    }

    async fn cancel_background_subagent_controls(
        &self,
        controls: Vec<(String, BackgroundSubagentTaskControl)>,
    ) -> BitFunResult<usize> {
        for (background_task_id, control) in &controls {
            debug!(
                "Cancelling background subagent task: background_task_id={}, parent_session_id={}, subagent_session_id={}",
                background_task_id, control.parent_session_id, control.subagent_session_id
            );
            match &control.cancel_target {
                BackgroundSubagentCancelTarget::Scheduler(handle) => {
                    if let Some(scheduler) = get_global_scheduler() {
                        scheduler.request_hidden_subagent_cancellation(handle).await;
                    } else {
                        warn!(
                            "Cannot cancel scheduler-backed background subagent because scheduler is unavailable: background_task_id={}, subagent_session_id={}",
                            background_task_id, control.subagent_session_id
                        );
                    }
                }
                BackgroundSubagentCancelTarget::Direct(token) => {
                    token.cancel();
                }
            }
        }

        Ok(controls.len())
    }

    pub(crate) async fn execute_subagent(
        &self,
        request: SubagentExecutionRequest,
        cancel_token: Option<&CancellationToken>,
        timeout_seconds: Option<u64>,
    ) -> BitFunResult<SubagentResult> {
        let request = self.prepare_subagent_execution_request(request).await?;
        let Some(scheduler) = get_global_scheduler() else {
            return self
                .execute_prepared_hidden_subagent(request, cancel_token, timeout_seconds)
                .await;
        };
        let submit_result = match scheduler
            .submit_hidden_subagent(request.clone(), timeout_seconds)
            .await
        {
            Ok(submit_result) => submit_result,
            Err(error) => {
                self.cleanup_prepared_hidden_subagent_session_if_unsubmitted(&request)
                    .await;
                return Err(BitFunError::tool(error));
            }
        };
        let receiver = submit_result.receiver;
        let result = if let Some(token) = cancel_token {
            let received = Self::await_hidden_subagent_receiver(receiver);
            tokio::pin!(received);
            tokio::select! {
                _ = token.cancelled() => {
                    scheduler
                        .request_hidden_subagent_cancellation(&submit_result.cancel_handle)
                        .await;
                    Self::await_hidden_subagent_cancellation(
                        &mut received,
                        SUBAGENT_TIMEOUT_GRACE_PERIOD,
                    ).await
                },
                result = &mut received => result,
            }
        } else {
            Self::await_hidden_subagent_receiver(receiver).await
        };
        result
    }

    /// Execute a hidden internal agent without requiring a parent Task/session.
    ///
    /// This is used by background product workers such as memory consolidation,
    /// where the agent must run through the normal tool/model loop but must not
    /// appear as a user-facing session or require a parent subagent relationship.
    pub(crate) async fn execute_internal_agent(
        &self,
        request: InternalAgentExecutionRequest,
        cancel_token: Option<&CancellationToken>,
        timeout_seconds: Option<u64>,
    ) -> BitFunResult<SubagentResult> {
        let task_description = request.task_description.trim().to_string();
        if task_description.is_empty() {
            return Err(BitFunError::Validation(
                "task_description is required when creating an internal agent session".to_string(),
            ));
        }

        let hidden_request = HiddenSubagentExecutionRequest {
            target_session_id: None,
            dialog_turn_id: None,
            session_name: request.session_name,
            agent_type: request.agent_type,
            session_config: Self::build_session_config_for_workspace(
                request.workspace_path,
                request.model_id,
            )
            .await,
            initial_messages: vec![Message::user(task_description.clone())],
            user_input_text: task_description,
            created_by: request.created_by,
            subagent_parent_info: None,
            context: request.context,
            delegation_policy: request.delegation_policy,
            runtime_tool_restrictions: request.runtime_tool_restrictions,
            prompt_cache_source_session_id: None,
            session_kind: request.session_kind,
            emit_lifecycle_events: request.emit_lifecycle_events,
            prepared_session_created: false,
        };

        self.execute_hidden_subagent_internal(hidden_request, cancel_token, timeout_seconds)
            .await
    }

    pub(crate) async fn start_background_subagent(
        &self,
        request: SubagentExecutionRequest,
        timeout_seconds: Option<u64>,
        allow_review_follow_up: bool,
    ) -> BitFunResult<BackgroundSubagentStartResult> {
        let request = self
            .resolve_hidden_subagent_execution_request(request, Some(allow_review_follow_up))
            .await?;
        let request = self
            .prepare_hidden_subagent_execution_request(request)
            .await?;
        let subagent_session_id = request
            .target_session_id()
            .ok_or_else(|| {
                BitFunError::Validation(
                    "prepared hidden subagent request is missing target_session_id".to_string(),
                )
            })?
            .to_string();
        let subagent_dialog_turn_id = request
            .dialog_turn_id
            .as_deref()
            .ok_or_else(|| {
                BitFunError::Validation(
                    "prepared hidden subagent request is missing dialog_turn_id".to_string(),
                )
            })?
            .to_string();
        let agent_type = request.agent_type.clone();
        let subagent_parent_info = match request.subagent_parent_info.clone() {
            Some(info) => info,
            None => {
                self.cleanup_prepared_hidden_subagent_session_if_unsubmitted(&request)
                    .await;
                return Err(BitFunError::Validation(
                    "subagent_parent_info is required when creating a background subagent session"
                        .to_string(),
                ));
            }
        };
        let parent_session = match self
            .session_manager
            .get_session(&subagent_parent_info.session_id)
        {
            Some(session) => session,
            None => {
                self.cleanup_prepared_hidden_subagent_session_if_unsubmitted(&request)
                    .await;
                return Err(BitFunError::NotFound(format!(
                    "Parent session not found: {}",
                    subagent_parent_info.session_id
                )));
            }
        };
        let parent_agent_type = parent_session.agent_type.clone();
        let parent_workspace_path = parent_session.config.workspace_path.clone();
        let parent_remote_connection_id = parent_session.config.remote_connection_id.clone();
        let parent_remote_ssh_host = parent_session.config.remote_ssh_host.clone();
        let parent_requires_tool_confirmation = get_global_scheduler().is_some_and(|scheduler| {
            scheduler.active_turn_requires_tool_confirmation(
                &subagent_parent_info.session_id,
                &subagent_parent_info.dialog_turn_id,
            )
        });
        let background_task_id = format!("bg-subagent-{}", uuid::Uuid::new_v4());
        let background_task_id_for_delivery = background_task_id.clone();
        let task_description = request.user_input_text.clone();
        let coordinator = match get_global_coordinator() {
            Some(coordinator) => coordinator,
            None => {
                self.cleanup_prepared_hidden_subagent_session_if_unsubmitted(&request)
                    .await;
                return Err(BitFunError::service(
                    "Coordinator not initialized".to_string(),
                ));
            }
        };
        let parent_cancel_token = self
            .execution_engine
            .cancel_token_for_dialog_turn(&subagent_parent_info.dialog_turn_id)
            .map(|token| token.child_token());

        if let Some(scheduler) = get_global_scheduler() {
            let submit_result = match scheduler
                .submit_hidden_subagent(request.clone(), timeout_seconds)
                .await
            {
                Ok(submit_result) => submit_result,
                Err(error) => {
                    self.cleanup_prepared_hidden_subagent_session_if_unsubmitted(&request)
                        .await;
                    return Err(BitFunError::tool(error));
                }
            };
            let receiver = submit_result.receiver;
            let cancel_handle = submit_result.cancel_handle.clone();
            let scheduler_for_cancel = scheduler.clone();
            let suppress_delivery = self.register_background_subagent_task(
                background_task_id.clone(),
                subagent_parent_info.session_id.clone(),
                subagent_session_id.clone(),
                BackgroundSubagentCancelTarget::Scheduler(cancel_handle.clone()),
            );
            let background_subagent_tasks = self.background_subagent_tasks.clone();
            let background_delivery_suppressions =
                self.background_subagent_delivery_suppressions.clone();
            let subagent_session_id_for_delivery = subagent_session_id.clone();
            let subagent_dialog_turn_id_for_delivery = subagent_dialog_turn_id.clone();

            tokio::spawn(async move {
                let result = match parent_cancel_token.as_ref() {
                    Some(token) => {
                        let received = Self::await_hidden_subagent_receiver(receiver);
                        tokio::pin!(received);
                        tokio::select! {
                            _ = token.cancelled() => {
                                scheduler_for_cancel
                                    .request_hidden_subagent_cancellation(&cancel_handle)
                                    .await;
                                Self::await_hidden_subagent_cancellation(
                                    &mut received,
                                    SUBAGENT_TIMEOUT_GRACE_PERIOD,
                                ).await
                            },
                            result = &mut received => result,
                        }
                    }
                    None => Self::await_hidden_subagent_receiver(receiver).await,
                };
                if suppress_delivery.load(Ordering::SeqCst) {
                    background_subagent_tasks.remove(&background_task_id_for_delivery);
                    background_delivery_suppressions.remove(&background_task_id_for_delivery);
                    debug!(
                        "Suppressing cancelled background subagent result delivery: background_task_id={}, parent_session_id={}",
                        background_task_id_for_delivery, subagent_parent_info.session_id
                    );
                    return;
                }

                let (delivery_text, display_text) = match result {
                    Ok(result) => (
                        format_background_subagent_delivery_text(
                            &subagent_session_id_for_delivery,
                            Ok(&result),
                        ),
                        format_background_subagent_display_text(Ok(&result)),
                    ),
                    Err(error) => (
                        format_background_subagent_delivery_text(
                            &subagent_session_id_for_delivery,
                            Err(&error),
                        ),
                        format_background_subagent_display_text(Err(&error)),
                    ),
                };

                let metadata = background_subagent_delivery_metadata(
                    &background_task_id_for_delivery,
                    &subagent_parent_info,
                    &subagent_session_id_for_delivery,
                    &subagent_dialog_turn_id_for_delivery,
                    parent_requires_tool_confirmation,
                    &agent_type,
                    &task_description,
                );

                let runtime =
                    match CoreServiceAgentRuntime::global_agent_runtime_with_lifecycle_delivery() {
                        Ok(runtime) => runtime,
                        Err(error) => {
                            background_subagent_tasks.remove(&background_task_id_for_delivery);
                            background_delivery_suppressions
                                .remove(&background_task_id_for_delivery);
                            warn!(
                                "Agent runtime lifecycle delivery is not available; background subagent result dropped: background_task_id={}, parent_session_id={}, error={}",
                                background_task_id_for_delivery,
                                subagent_parent_info.session_id,
                                error
                            );
                            return;
                        }
                    };

                if suppress_delivery.load(Ordering::SeqCst) {
                    background_subagent_tasks.remove(&background_task_id_for_delivery);
                    background_delivery_suppressions.remove(&background_task_id_for_delivery);
                    debug!(
                        "Suppressing cancelled background subagent result delivery: background_task_id={}, parent_session_id={}",
                        background_task_id_for_delivery, subagent_parent_info.session_id
                    );
                    return;
                }

                if let Err(error) = runtime
                    .deliver_background_result(AgentBackgroundResultRequest {
                        session_id: subagent_parent_info.session_id.clone(),
                        agent_type: parent_agent_type,
                        workspace_path: parent_workspace_path,
                        remote_connection_id: parent_remote_connection_id,
                        remote_ssh_host: parent_remote_ssh_host,
                        content: delivery_text,
                        display_content: Some(display_text),
                        metadata,
                    })
                    .await
                {
                    background_subagent_tasks.remove(&background_task_id_for_delivery);
                    background_delivery_suppressions.remove(&background_task_id_for_delivery);
                    warn!(
                        "Failed to deliver background subagent result through scheduler path: background_task_id={}, parent_session_id={}, error={}",
                        background_task_id_for_delivery,
                        subagent_parent_info.session_id,
                        CoreServiceAgentRuntime::runtime_error_message(error)
                    );
                }
            });

            return Ok(BackgroundSubagentStartResult {
                background_task_id,
                session_id: subagent_session_id,
            });
        }

        let background_cancel_token = CancellationToken::new();
        let execution_cancel_token = CancellationToken::new();
        let background_cancel_token_for_bridge = background_cancel_token.clone();
        let execution_cancel_token_for_bridge = execution_cancel_token.clone();
        let cancel_bridge_handle = match parent_cancel_token {
            Some(parent_cancel_token) => tokio::spawn(async move {
                tokio::select! {
                    _ = parent_cancel_token.cancelled() => {
                        execution_cancel_token_for_bridge.cancel();
                    }
                    _ = background_cancel_token_for_bridge.cancelled() => {
                        execution_cancel_token_for_bridge.cancel();
                    }
                }
            }),
            None => tokio::spawn(async move {
                background_cancel_token_for_bridge.cancelled().await;
                execution_cancel_token_for_bridge.cancel();
            }),
        };
        let suppress_delivery = self.register_background_subagent_task(
            background_task_id.clone(),
            subagent_parent_info.session_id.clone(),
            subagent_session_id.clone(),
            BackgroundSubagentCancelTarget::Direct(background_cancel_token),
        );
        let background_subagent_tasks = self.background_subagent_tasks.clone();
        let background_delivery_suppressions =
            self.background_subagent_delivery_suppressions.clone();
        let subagent_session_id_for_delivery = subagent_session_id.clone();
        let subagent_dialog_turn_id_for_delivery = subagent_dialog_turn_id.clone();

        tokio::spawn(async move {
            let (delivery_text, display_text) = match coordinator
                .execute_hidden_subagent_internal(
                    request,
                    Some(&execution_cancel_token),
                    timeout_seconds,
                )
                .await
            {
                Ok(result) => (
                    format_background_subagent_delivery_text(
                        &subagent_session_id_for_delivery,
                        Ok(&result),
                    ),
                    format_background_subagent_display_text(Ok(&result)),
                ),
                Err(error) => (
                    format_background_subagent_delivery_text(
                        &subagent_session_id_for_delivery,
                        Err(&error),
                    ),
                    format_background_subagent_display_text(Err(&error)),
                ),
            };
            cancel_bridge_handle.abort();
            if suppress_delivery.load(Ordering::SeqCst) {
                background_subagent_tasks.remove(&background_task_id_for_delivery);
                background_delivery_suppressions.remove(&background_task_id_for_delivery);
                debug!(
                    "Suppressing cancelled background subagent result delivery: background_task_id={}, parent_session_id={}",
                    background_task_id_for_delivery, subagent_parent_info.session_id
                );
                return;
            }

            let metadata = background_subagent_delivery_metadata(
                &background_task_id_for_delivery,
                &subagent_parent_info,
                &subagent_session_id_for_delivery,
                &subagent_dialog_turn_id_for_delivery,
                parent_requires_tool_confirmation,
                &agent_type,
                &task_description,
            );

            let runtime =
                match CoreServiceAgentRuntime::global_agent_runtime_with_lifecycle_delivery() {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        background_subagent_tasks.remove(&background_task_id_for_delivery);
                        background_delivery_suppressions.remove(&background_task_id_for_delivery);
                        warn!(
                            "Agent runtime lifecycle delivery is not available; background subagent result dropped: background_task_id={}, parent_session_id={}, error={}",
                            background_task_id_for_delivery, subagent_parent_info.session_id, error
                        );
                        return;
                    }
                };

            if suppress_delivery.load(Ordering::SeqCst) {
                background_subagent_tasks.remove(&background_task_id_for_delivery);
                background_delivery_suppressions.remove(&background_task_id_for_delivery);
                debug!(
                    "Suppressing cancelled background subagent result delivery: background_task_id={}, parent_session_id={}",
                    background_task_id_for_delivery, subagent_parent_info.session_id
                );
                return;
            }

            if let Err(error) = runtime
                .deliver_background_result(AgentBackgroundResultRequest {
                    session_id: subagent_parent_info.session_id.clone(),
                    agent_type: parent_agent_type,
                    workspace_path: parent_workspace_path,
                    remote_connection_id: parent_remote_connection_id,
                    remote_ssh_host: parent_remote_ssh_host,
                    content: delivery_text,
                    display_content: Some(display_text),
                    metadata,
                })
                .await
            {
                background_subagent_tasks.remove(&background_task_id_for_delivery);
                background_delivery_suppressions.remove(&background_task_id_for_delivery);
                warn!(
                    "Failed to deliver background subagent result: background_task_id={}, parent_session_id={}, error={}",
                    background_task_id_for_delivery,
                    subagent_parent_info.session_id,
                    CoreServiceAgentRuntime::runtime_error_message(error)
                );
            }
        });

        Ok(BackgroundSubagentStartResult {
            background_task_id,
            session_id: subagent_session_id,
        })
    }

    /// Clean up runtime-only subagent resources.
    ///
    /// Subagent sessions are now persisted so users can reopen them from the UI.
    /// This cleanup path must only release ephemeral runtime resources such as
    /// snapshot bookkeeping; it must not delete the persisted session itself.
    async fn cleanup_subagent_resources(&self, session_id: &str) -> BitFunResult<()> {
        let cleanup_started_at = Instant::now();
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
            debug!(
                "Subagent cleanup stage starting: session_id={}, stage=snapshot_cleanup, workspace_path={}",
                session_id,
                workspace_path.display()
            );
            let stage_started_at = Instant::now();
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
            debug!(
                "Subagent cleanup stage completed: session_id={}, stage=snapshot_cleanup, duration_ms={}",
                session_id,
                stage_started_at.elapsed().as_millis()
            );
        }

        debug!(
            "Subagent resource cleanup completed: session_id={}, duration_ms={}",
            session_id,
            cleanup_started_at.elapsed().as_millis()
        );
        Ok(())
    }

    fn should_persist_reused_subagent_user_input_context(
        prepared_target_session_id: Option<&str>,
        prepared_session_created: bool,
        session_id: &str,
    ) -> bool {
        !prepared_session_created && prepared_target_session_id == Some(session_id)
    }

    async fn persist_reused_subagent_user_input_context_if_needed(
        &self,
        prepared_target_session_id: Option<&str>,
        prepared_session_created: bool,
        session_id: &str,
        dialog_turn_id: &str,
        user_input_text: &str,
    ) -> BitFunResult<()> {
        if !Self::should_persist_reused_subagent_user_input_context(
            prepared_target_session_id,
            prepared_session_created,
            session_id,
        ) {
            return Ok(());
        }

        let user_message = Message::user(user_input_text.to_string())
            .with_semantic_kind(MessageSemanticKind::ActualUserInput)
            .with_turn_id(dialog_turn_id.to_string());
        self.session_manager
            .add_message(session_id, user_message)
            .await
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

    /// Update the session-level prompt-cache guard mode for the latest
    /// scheduler-accepted user submission.
    pub async fn update_last_submitted_agent_type(
        &self,
        session_id: &str,
        agent_type: &str,
    ) -> BitFunResult<()> {
        let normalized = Self::normalize_agent_type(agent_type);
        self.session_manager
            .update_last_submitted_agent_type(session_id, &normalized)
            .await
    }

    /// Emit event
    pub(crate) async fn emit_event(&self, event: AgenticEvent) {
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

    pub async fn emit_deep_review_queue_state_changed(
        &self,
        session_id: &str,
        turn_id: &str,
        queue_state: DeepReviewQueueState,
    ) {
        let event = AgenticEvent::DeepReviewQueueStateChanged {
            session_id: session_id.to_string(),
            turn_id: turn_id.to_string(),
            queue_state,
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

fn resolve_agent_submission_turn_id(
    request: &bitfun_runtime_ports::AgentSubmissionRequest,
) -> String {
    request
        .turn_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            request
                .metadata
                .get("turnId")
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

fn resolve_agent_session_create_created_by(
    metadata: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    metadata
        .get("createdBy")
        .or_else(|| metadata.get("created_by"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn runtime_port_backend_error(error: BitFunError) -> bitfun_runtime_ports::PortError {
    bitfun_runtime_ports::PortError::new(
        bitfun_runtime_ports::PortErrorKind::Backend,
        error.to_string(),
    )
}

async fn create_agent_session_from_runtime_request(
    coordinator: &ConversationCoordinator,
    session_id: Option<String>,
    request: bitfun_runtime_ports::AgentSessionCreateRequest,
    map_core_error: fn(BitFunError) -> bitfun_runtime_ports::PortError,
) -> bitfun_runtime_ports::PortResult<bitfun_runtime_ports::AgentSessionCreateResult> {
    let workspace_path = request.workspace_path.clone().ok_or_else(|| {
        bitfun_runtime_ports::PortError::new(
            bitfun_runtime_ports::PortErrorKind::InvalidRequest,
            "workspace_path is required to create an agent session",
        )
    })?;
    let created_by = resolve_agent_session_create_created_by(&request.metadata);
    let session = coordinator
        .create_session_with_workspace_and_creator(
            session_id,
            request.session_name,
            request.agent_type,
            SessionConfig {
                workspace_path: Some(workspace_path.clone()),
                remote_connection_id: request.remote_connection_id,
                remote_ssh_host: request.remote_ssh_host,
                ..Default::default()
            },
            workspace_path,
            created_by,
        )
        .await
        .map_err(map_core_error)?;

    Ok(bitfun_runtime_ports::AgentSessionCreateResult {
        session_id: session.session_id,
        session_name: session.session_name,
        agent_type: session.agent_type,
    })
}

#[async_trait::async_trait]
impl bitfun_runtime_ports::AgentSubmissionPort for ConversationCoordinator {
    async fn create_session(
        &self,
        request: bitfun_runtime_ports::AgentSessionCreateRequest,
    ) -> bitfun_runtime_ports::PortResult<bitfun_runtime_ports::AgentSessionCreateResult> {
        create_agent_session_from_runtime_request(self, None, request, runtime_port_backend_error)
            .await
    }

    async fn create_session_with_id(
        &self,
        session_id: String,
        request: bitfun_runtime_ports::AgentSessionCreateRequest,
    ) -> bitfun_runtime_ports::PortResult<bitfun_runtime_ports::AgentSessionCreateResult> {
        bitfun_core_types::validate_session_id(&session_id).map_err(|message| {
            bitfun_runtime_ports::PortError::new(
                bitfun_runtime_ports::PortErrorKind::InvalidRequest,
                message,
            )
        })?;
        create_agent_session_from_runtime_request(
            self,
            Some(session_id),
            request,
            runtime_port_error_from_bitfun,
        )
        .await
    }

    async fn submit_message(
        &self,
        request: bitfun_runtime_ports::AgentSubmissionRequest,
    ) -> bitfun_runtime_ports::PortResult<bitfun_runtime_ports::AgentSubmissionResult> {
        if !request.attachments.is_empty() {
            return Err(bitfun_runtime_ports::PortError::new(
                bitfun_runtime_ports::PortErrorKind::InvalidRequest,
                "agent submission port does not yet accept generic attachments",
            ));
        }

        let session = self
            .get_session_manager()
            .get_session(&request.session_id)
            .ok_or_else(|| {
                bitfun_runtime_ports::PortError::new(
                    bitfun_runtime_ports::PortErrorKind::NotFound,
                    format!("session not found: {}", request.session_id),
                )
            })?;

        let turn_id = resolve_agent_submission_turn_id(&request);

        let trigger_source = request.source.unwrap_or(DialogTriggerSource::Bot);
        let user_message_metadata = if request.metadata.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(request.metadata.clone()))
        };

        self.start_dialog_turn(
            request.session_id,
            request.message.clone(),
            Some(request.message),
            Some(turn_id.clone()),
            session.agent_type.clone(),
            session.config.workspace_path.clone(),
            session.config.remote_connection_id.clone(),
            session.config.remote_ssh_host.clone(),
            DialogSubmissionPolicy::for_source(trigger_source),
            user_message_metadata,
        )
        .await
        .map_err(|error| {
            bitfun_runtime_ports::PortError::new(
                bitfun_runtime_ports::PortErrorKind::Backend,
                error.to_string(),
            )
        })?;

        Ok(bitfun_runtime_ports::AgentSubmissionResult {
            turn_id,
            accepted: true,
        })
    }

    async fn resolve_session_agent_type(
        &self,
        session_id: &str,
    ) -> bitfun_runtime_ports::PortResult<Option<String>> {
        if let Some(session) = self.get_session_manager().get_session(session_id) {
            return Ok(Some(session.agent_type.clone()));
        }

        let Some(binding) = self
            .get_session_manager()
            .resolve_session_workspace_binding(session_id)
            .await
        else {
            return Ok(None);
        };

        self.restore_session_from_storage_path(&binding.session_storage_dir(), session_id)
            .await
            .map(|session| Some(session.agent_type))
            .map_err(|error| {
                bitfun_runtime_ports::PortError::new(
                    bitfun_runtime_ports::PortErrorKind::Backend,
                    error.to_string(),
                )
            })
    }
}

fn runtime_session_time_ms(time: std::time::SystemTime) -> u64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn runtime_session_summary(session: SessionSummary) -> bitfun_runtime_ports::AgentSessionSummary {
    bitfun_runtime_ports::AgentSessionSummary {
        session_id: session.session_id,
        session_name: session.session_name,
        agent_type: session.agent_type,
        turn_count: session.turn_count,
        created_at_ms: runtime_session_time_ms(session.created_at),
        last_active_at_ms: runtime_session_time_ms(session.last_activity_at),
    }
}

fn runtime_session_workspace_binding(binding: WorkspaceBinding) -> AgentSessionWorkspaceBinding {
    AgentSessionWorkspaceBinding {
        workspace_id: binding.workspace_id.clone(),
        workspace_path: binding.root_path_string(),
        remote_connection_id: binding.connection_id().map(ToOwned::to_owned),
        remote_ssh_host: if binding.is_remote() {
            Some(binding.session_identity.hostname.clone()).filter(|value| !value.trim().is_empty())
        } else {
            None
        },
    }
}

fn runtime_port_error_from_bitfun(error: BitFunError) -> bitfun_runtime_ports::PortError {
    let (kind, message) = match error {
        BitFunError::Validation(message) => {
            (bitfun_runtime_ports::PortErrorKind::InvalidRequest, message)
        }
        BitFunError::NotFound(message) => (bitfun_runtime_ports::PortErrorKind::NotFound, message),
        BitFunError::Cancelled(message) => {
            (bitfun_runtime_ports::PortErrorKind::Cancelled, message)
        }
        BitFunError::Timeout(message) => (bitfun_runtime_ports::PortErrorKind::Timeout, message),
        BitFunError::NotImplemented(message) => {
            (bitfun_runtime_ports::PortErrorKind::NotAvailable, message)
        }
        other => (
            bitfun_runtime_ports::PortErrorKind::Backend,
            other.to_string(),
        ),
    };
    bitfun_runtime_ports::PortError::new(kind, message)
}

fn runtime_port_error_preserving_message(error: BitFunError) -> bitfun_runtime_ports::PortError {
    let message = error.to_string();
    let mut port_error = runtime_port_error_from_bitfun(error);
    port_error.message = message;
    port_error
}

fn user_input_port_error(
    error: bitfun_agent_runtime::user_questions::UserInputSendError,
) -> bitfun_runtime_ports::PortError {
    let kind = match &error {
        bitfun_agent_runtime::user_questions::UserInputSendError::MissingChannel { .. } => {
            bitfun_runtime_ports::PortErrorKind::NotFound
        }
        bitfun_agent_runtime::user_questions::UserInputSendError::ChannelClosed { .. } => {
            bitfun_runtime_ports::PortErrorKind::Cancelled
        }
    };
    bitfun_runtime_ports::PortError::new(kind, format!("Tool error: {error}"))
}

#[async_trait::async_trait]
impl bitfun_runtime_ports::AgentSessionManagementPort for ConversationCoordinator {
    async fn list_sessions(
        &self,
        request: bitfun_runtime_ports::AgentSessionListRequest,
    ) -> bitfun_runtime_ports::PortResult<Vec<bitfun_runtime_ports::AgentSessionSummary>> {
        let effective_storage_path = Self::resolve_session_restore_path(
            &request.workspace_path,
            request.remote_connection_id.as_deref(),
            request.remote_ssh_host.as_deref(),
        )
        .await
        .map_err(|error| {
            bitfun_runtime_ports::PortError::new(
                bitfun_runtime_ports::PortErrorKind::Backend,
                error.to_string(),
            )
        })?;

        self.list_sessions(&effective_storage_path)
            .await
            .map(|sessions| {
                sessions
                    .into_iter()
                    .map(runtime_session_summary)
                    .collect::<Vec<_>>()
            })
            .map_err(|error| {
                bitfun_runtime_ports::PortError::new(
                    bitfun_runtime_ports::PortErrorKind::Backend,
                    error.to_string(),
                )
            })
    }

    async fn delete_session(
        &self,
        request: bitfun_runtime_ports::AgentSessionDeleteRequest,
    ) -> bitfun_runtime_ports::PortResult<()> {
        bitfun_core_types::validate_session_id(&request.session_id).map_err(|message| {
            bitfun_runtime_ports::PortError::new(
                bitfun_runtime_ports::PortErrorKind::InvalidRequest,
                message,
            )
        })?;
        let effective_storage_path = Self::resolve_session_restore_path(
            &request.workspace_path,
            request.remote_connection_id.as_deref(),
            request.remote_ssh_host.as_deref(),
        )
        .await
        .map_err(|error| {
            bitfun_runtime_ports::PortError::new(
                bitfun_runtime_ports::PortErrorKind::Backend,
                error.to_string(),
            )
        })?;

        self.delete_session(&effective_storage_path, &request.session_id)
            .await
            .map_err(|error| {
                bitfun_runtime_ports::PortError::new(
                    bitfun_runtime_ports::PortErrorKind::Backend,
                    error.to_string(),
                )
            })
    }

    async fn resolve_session_workspace_binding(
        &self,
        request: bitfun_runtime_ports::AgentSessionWorkspaceRequest,
    ) -> bitfun_runtime_ports::PortResult<Option<bitfun_runtime_ports::AgentSessionWorkspaceBinding>>
    {
        Ok(self
            .get_session_manager()
            .resolve_session_workspace_binding(&request.session_id)
            .await
            .map(runtime_session_workspace_binding))
    }
}

#[async_trait::async_trait]
impl bitfun_agent_runtime::sdk::AgentSessionRestorePort for ConversationCoordinator {
    async fn restore_session(
        &self,
        request: bitfun_agent_runtime::sdk::AgentSessionRestoreRequest,
    ) -> bitfun_runtime_ports::PortResult<bitfun_agent_runtime::sdk::AgentSessionRestoreResult>
    {
        bitfun_core_types::validate_session_id(&request.session_id).map_err(|message| {
            bitfun_runtime_ports::PortError::new(
                bitfun_runtime_ports::PortErrorKind::InvalidRequest,
                message,
            )
        })?;
        let session = self
            .restore_session_for_workspace(
                SessionStoragePathRequest {
                    workspace_path: PathBuf::from(request.workspace_path),
                    remote_connection_id: request.remote_connection_id,
                    remote_ssh_host: request.remote_ssh_host,
                },
                &request.session_id,
            )
            .await
            .map_err(runtime_port_error_preserving_message)?;

        Ok(bitfun_agent_runtime::sdk::AgentSessionRestoreResult {
            session: bitfun_runtime_ports::AgentSessionSummary {
                session_id: session.session_id,
                session_name: session.session_name,
                agent_type: session.agent_type,
                turn_count: session.dialog_turn_ids.len(),
                created_at_ms: runtime_session_time_ms(session.created_at),
                last_active_at_ms: runtime_session_time_ms(session.last_activity_at),
            },
            state: session.state,
        })
    }
}

#[async_trait::async_trait]
impl bitfun_agent_runtime::sdk::AgentInteractionResponsePort for ConversationCoordinator {
    async fn confirm_tool(
        &self,
        request: bitfun_agent_runtime::sdk::AgentToolConfirmationRequest,
    ) -> bitfun_runtime_ports::PortResult<()> {
        self.confirm_tool(&request.tool_id, request.updated_input)
            .await
            .map_err(runtime_port_error_preserving_message)
    }

    async fn reject_tool(
        &self,
        request: bitfun_agent_runtime::sdk::AgentToolRejectionRequest,
    ) -> bitfun_runtime_ports::PortResult<()> {
        self.reject_tool(&request.tool_id, request.reason)
            .await
            .map_err(runtime_port_error_preserving_message)
    }

    async fn submit_user_answers(
        &self,
        request: bitfun_agent_runtime::sdk::AgentUserAnswersRequest,
    ) -> bitfun_runtime_ports::PortResult<()> {
        crate::agentic::tools::user_input_manager::get_user_input_manager()
            .send_answer(&request.tool_id, request.answers)
            .map_err(user_input_port_error)
    }
}

#[async_trait::async_trait]
impl bitfun_runtime_ports::AgentThreadGoalManagementPort for ConversationCoordinator {
    async fn get_thread_goal(
        &self,
        request: bitfun_runtime_ports::AgentThreadGoalGetRequest,
    ) -> bitfun_runtime_ports::PortResult<Option<ThreadGoal>> {
        self.get_thread_goal(
            &request.session_id,
            std::path::Path::new(&request.workspace_path),
        )
        .await
        .map_err(runtime_port_error_from_bitfun)
    }

    async fn create_thread_goal(
        &self,
        request: bitfun_runtime_ports::AgentThreadGoalCreateRequest,
    ) -> bitfun_runtime_ports::PortResult<ThreadGoal> {
        self.create_thread_goal(
            &request.session_id,
            std::path::Path::new(&request.workspace_path),
            request.objective,
            request.token_budget,
        )
        .await
        .map_err(runtime_port_error_from_bitfun)
    }

    async fn update_thread_goal_status(
        &self,
        request: bitfun_runtime_ports::AgentThreadGoalUpdateStatusRequest,
    ) -> bitfun_runtime_ports::PortResult<ThreadGoal> {
        self.update_thread_goal_status(
            &request.session_id,
            std::path::Path::new(&request.workspace_path),
            request.status,
            request.turn_id.as_deref(),
        )
        .await
        .map_err(runtime_port_error_from_bitfun)
    }
}

#[async_trait::async_trait]
impl bitfun_runtime_ports::AgentTurnCancellationPort for ConversationCoordinator {
    async fn cancel_turn(
        &self,
        request: bitfun_runtime_ports::AgentTurnCancellationRequest,
    ) -> bitfun_runtime_ports::PortResult<bitfun_runtime_ports::AgentTurnCancellationResult> {
        let session_id = request.session_id;
        if let Some(turn_id) = request.turn_id {
            self.cancel_dialog_turn(&session_id, &turn_id)
                .await
                .map_err(|error| {
                    bitfun_runtime_ports::PortError::new(
                        bitfun_runtime_ports::PortErrorKind::Backend,
                        error.to_string(),
                    )
                })?;

            return Ok(bitfun_runtime_ports::AgentTurnCancellationResult {
                session_id,
                turn_id: Some(turn_id),
                requested: true,
            });
        }

        let wait_timeout = Duration::from_millis(request.wait_timeout_ms.unwrap_or(1500));
        let cancelled_turn_id = self
            .cancel_active_turn_for_session(&session_id, wait_timeout)
            .await
            .map_err(|error| {
                bitfun_runtime_ports::PortError::new(
                    bitfun_runtime_ports::PortErrorKind::Backend,
                    error.to_string(),
                )
            })?;
        let requested = cancelled_turn_id.is_some();

        Ok(bitfun_runtime_ports::AgentTurnCancellationResult {
            session_id,
            turn_id: cancelled_turn_id,
            requested,
        })
    }
}

#[async_trait::async_trait]
impl bitfun_runtime_ports::RemoteControlStatePort for ConversationCoordinator {
    async fn read_remote_control_state(
        &self,
        request: bitfun_runtime_ports::RemoteControlStateRequest,
    ) -> bitfun_runtime_ports::PortResult<Option<bitfun_runtime_ports::RemoteControlStateSnapshot>>
    {
        let Some(session) = self.get_session_manager().get_session(&request.session_id) else {
            return Ok(None);
        };

        let mut metadata = serde_json::Map::new();
        let (state, active_turn_id) = match session.state {
            SessionState::Idle => (bitfun_runtime_ports::RemoteControlSessionState::Idle, None),
            SessionState::Processing {
                current_turn_id,
                phase,
            } => {
                metadata.insert(
                    "phase".to_string(),
                    serde_json::Value::String(format!("{:?}", phase)),
                );
                (
                    bitfun_runtime_ports::RemoteControlSessionState::Processing,
                    Some(current_turn_id),
                )
            }
            SessionState::Error { error, recoverable } => {
                metadata.insert("error".to_string(), serde_json::Value::String(error));
                metadata.insert(
                    "recoverable".to_string(),
                    serde_json::Value::Bool(recoverable),
                );
                (bitfun_runtime_ports::RemoteControlSessionState::Error, None)
            }
        };

        Ok(Some(bitfun_runtime_ports::RemoteControlStateSnapshot {
            session_id: request.session_id,
            state,
            active_turn_id,
            queue_depth: 0,
            metadata,
        }))
    }
}

#[async_trait::async_trait]
impl bitfun_runtime_ports::SessionTranscriptReader for ConversationCoordinator {
    async fn read_session_transcript(
        &self,
        request: bitfun_runtime_ports::SessionTranscriptRequest,
    ) -> bitfun_runtime_ports::PortResult<bitfun_runtime_ports::SessionTranscript> {
        let messages = self
            .get_messages(&request.session_id)
            .await
            .map_err(runtime_port_error_preserving_message)?;

        let messages = messages
            .into_iter()
            .filter(|message| match request.turn_id.as_ref() {
                Some(turn_id) => message.metadata.turn_id.as_ref() == Some(turn_id),
                None => true,
            })
            .map(|message| {
                let role = match message.role {
                    crate::agentic::core::MessageRole::User => "user",
                    crate::agentic::core::MessageRole::Assistant => "assistant",
                    crate::agentic::core::MessageRole::Tool => "tool",
                    crate::agentic::core::MessageRole::System => "system",
                }
                .to_string();

                let content = match message.content {
                    MessageContent::Text(text) => {
                        bitfun_runtime_ports::TranscriptContent::Text(text)
                    }
                    MessageContent::Multimodal { text, images } => {
                        bitfun_runtime_ports::TranscriptContent::Multimodal {
                            text,
                            image_count: images.len(),
                        }
                    }
                    MessageContent::ToolResult {
                        tool_id,
                        tool_name,
                        effective_tool_name,
                        result,
                        is_error,
                        ..
                    } => bitfun_runtime_ports::TranscriptContent::ToolResult {
                        tool_id,
                        tool_name,
                        effective_tool_name,
                        result,
                        is_error,
                    },
                    MessageContent::Mixed {
                        reasoning_content,
                        text,
                        tool_calls,
                    } => bitfun_runtime_ports::TranscriptContent::Mixed {
                        reasoning_content,
                        text,
                        tool_calls: tool_calls
                            .into_iter()
                            .map(|tool_call| bitfun_runtime_ports::TranscriptToolCall {
                                tool_id: tool_call.tool_id,
                                tool_name: tool_call.tool_name,
                                arguments: tool_call.arguments,
                            })
                            .collect(),
                    },
                };

                bitfun_runtime_ports::TranscriptMessage {
                    id: Some(message.id),
                    role,
                    turn_id: message.metadata.turn_id,
                    timestamp_ms: Some(runtime_session_time_ms(message.timestamp)),
                    content,
                }
            })
            .collect();

        Ok(bitfun_runtime_ports::SessionTranscript {
            session_id: request.session_id,
            messages,
        })
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

async fn new_session_memory_mode_from_global_config() -> SessionMemoryMode {
    match crate::service::config::get_global_config_service().await {
        Ok(service) => {
            if service
                .get_config(None)
                .await
                .map(|config: crate::service::config::types::GlobalConfig| {
                    config.memories.generate_memories
                })
                .unwrap_or(true)
            {
                SessionMemoryMode::Enabled
            } else {
                SessionMemoryMode::Disabled
            }
        }
        Err(_) => SessionMemoryMode::Enabled,
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

fn merge_prepended_messages_for_turn(
    additional_prepended_messages: Vec<Message>,
    wrapped_prepended_messages: Vec<Message>,
    include_remote_file_delivery: bool,
) -> Vec<Message> {
    let mut prepended_messages = Vec::new();
    let mut scheduled_job_messages = Vec::new();
    let mut remote_file_delivery_messages = Vec::new();

    for message in additional_prepended_messages {
        if matches!(
            message.internal_reminder_kind(),
            Some(InternalReminderKind::ScheduledJob)
        ) {
            scheduled_job_messages.push(message);
        } else {
            prepended_messages.push(message);
        }
    }

    if include_remote_file_delivery {
        remote_file_delivery_messages.push(Message::internal_reminder(
            InternalReminderKind::RemoteFileDelivery,
            remote_file_delivery_reminder(),
        ));
    }

    prepended_messages.extend(wrapped_prepended_messages);
    prepended_messages.extend(remote_file_delivery_messages);
    prepended_messages.extend(scheduled_job_messages);
    prepended_messages
}

#[cfg(test)]
mod tests {
    use super::{
        background_subagent_delivery_metadata, merge_prepended_messages_for_turn,
        normalize_subagent_max_concurrency, resolve_agent_session_create_created_by,
        resolve_agent_submission_turn_id, runtime_port_error_preserving_message,
        should_require_tool_confirmation, turn_review_manifest_for_agent,
        validate_background_subagent_delivery, ConversationCoordinator, SubagentExecutionRequest,
    };
    use crate::agentic::agents::{CustomSubagent, CustomSubagentKind, UserContextPolicy};
    use crate::agentic::core::{
        InternalReminderKind, Message, MessageContent, MessageRole, MessageSemanticKind, Session,
        SessionConfig, SessionKind,
    };
    use crate::agentic::events::{EventQueue, EventQueueConfig, EventRouter};
    use crate::agentic::execution::{
        ExecutionEngine, ExecutionEngineConfig, RoundExecutor, StreamProcessor,
    };
    use crate::agentic::persistence::PersistenceManager;
    use crate::agentic::session::{
        compression::{CompressionConfig, ContextCompressor},
        PromptCachePolicy, SessionContextStore, SessionManager, SessionManagerConfig,
        SystemPromptCacheIdentity, UserContextCacheIdentity,
    };
    use crate::agentic::skill_agent_snapshot::SkillSnapshotEntry;
    use crate::agentic::tools::pipeline::SubagentParentInfo;
    use crate::agentic::tools::registry::ToolRegistry;
    use crate::agentic::tools::{ToolPipeline, ToolStateManager};
    use crate::agentic::TurnSkillAgentSnapshot;
    use crate::infrastructure::PathManager;
    use crate::service::remote_ssh::workspace_state::init_remote_workspace_manager;
    use crate::service::session::SessionMetadata;
    use bitfun_runtime_ports::{
        AgentSessionCreateRequest, AgentSubmissionPort, AgentSubmissionRequest,
        AgentSubmissionSource, DelegationPolicy, DialogQueuePriority, DialogSubmissionPolicy,
        SubagentContextMode,
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn migrated_runtime_ports_preserve_existing_core_error_messages() {
        let error = runtime_port_error_preserving_message(
            crate::util::errors::BitFunError::Validation("invalid session id".to_string()),
        );

        assert_eq!(
            error.kind,
            bitfun_runtime_ports::PortErrorKind::InvalidRequest
        );
        assert_eq!(error.message, "Validation error: invalid session id");
    }

    #[tokio::test]
    async fn interaction_response_port_uses_core_owners_and_typed_stale_errors() {
        use bitfun_agent_runtime::sdk::{
            AgentInteractionResponsePort, AgentToolConfirmationRequest, AgentToolRejectionRequest,
            AgentUserAnswersRequest,
        };

        let (coordinator, _) = test_coordinator();
        let answer_tool_id = format!("answer-{}", uuid::Uuid::new_v4());
        let (sender, receiver) = tokio::sync::oneshot::channel::<
            bitfun_agent_runtime::user_questions::UserInputResponse,
        >();
        crate::agentic::tools::user_input_manager::get_user_input_manager()
            .register_channel(answer_tool_id.clone(), sender);

        AgentInteractionResponsePort::submit_user_answers(
            &coordinator,
            AgentUserAnswersRequest {
                tool_id: answer_tool_id.clone(),
                answers: serde_json::json!({ "0": "continue" }),
            },
        )
        .await
        .expect("deliver user answers through the Core-owned channel");
        assert_eq!(
            receiver.await.expect("receive user answers").answers,
            serde_json::json!({ "0": "continue" })
        );

        let stale_answer = AgentInteractionResponsePort::submit_user_answers(
            &coordinator,
            AgentUserAnswersRequest {
                tool_id: answer_tool_id.clone(),
                answers: serde_json::json!({ "0": "continue" }),
            },
        )
        .await
        .expect_err("consumed answer channel must be reported as stale");
        assert_eq!(
            stale_answer.kind,
            bitfun_runtime_ports::PortErrorKind::NotFound
        );
        assert_eq!(
            stale_answer.message,
            format!("Tool error: Waiting channel not found: {answer_tool_id}")
        );

        let missing_tool_id = format!("tool-{}", uuid::Uuid::new_v4());
        let confirmation = AgentInteractionResponsePort::confirm_tool(
            &coordinator,
            AgentToolConfirmationRequest {
                tool_id: missing_tool_id.clone(),
                updated_input: Some(serde_json::json!({ "path": "updated.txt" })),
            },
        )
        .await
        .expect_err("missing confirmation task");
        assert_eq!(
            confirmation.kind,
            bitfun_runtime_ports::PortErrorKind::NotFound
        );
        assert_eq!(
            confirmation.message,
            format!("Not found: Tool task not found: {missing_tool_id}")
        );

        let rejection = AgentInteractionResponsePort::reject_tool(
            &coordinator,
            AgentToolRejectionRequest {
                tool_id: missing_tool_id.clone(),
                reason: "Use a read-only path".to_string(),
            },
        )
        .await
        .expect_err("missing rejection task");
        assert_eq!(
            rejection.kind,
            bitfun_runtime_ports::PortErrorKind::NotFound
        );
        assert_eq!(
            rejection.message,
            format!("Not found: Tool task not found: {missing_tool_id}")
        );
    }
    use tokio::sync::RwLock as TokioRwLock;

    fn test_coordinator_with_max_active_sessions(
        max_active_sessions: usize,
    ) -> (ConversationCoordinator, Arc<SessionManager>) {
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(
                PersistenceManager::new(Arc::new(PathManager::new().expect("path manager")))
                    .expect("persistence manager"),
            ),
            SessionManagerConfig {
                max_active_sessions,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(TokioRwLock::new(ToolRegistry::new())),
            Arc::new(ToolStateManager::new(event_queue.clone())),
            None,
        ));
        let execution_engine = Arc::new(ExecutionEngine::new(
            Arc::new(RoundExecutor::new(
                Arc::new(StreamProcessor::new(event_queue.clone())),
                event_queue.clone(),
                tool_pipeline.clone(),
            )),
            event_queue.clone(),
            session_manager.clone(),
            Arc::new(ContextCompressor::new(CompressionConfig::default())),
            ExecutionEngineConfig::default(),
        ));
        let coordinator = ConversationCoordinator::new(
            session_manager.clone(),
            execution_engine,
            tool_pipeline,
            event_queue,
            Arc::new(EventRouter::new()),
        );
        coordinator.set_terminal_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::terminal_port(),
        );
        coordinator.set_remote_exec_port(
            bitfun_runtime_services::test_support::FakeRuntimeServicesProvider::remote_exec_port(),
        );

        (coordinator, session_manager)
    }

    fn test_coordinator() -> (ConversationCoordinator, Arc<SessionManager>) {
        test_coordinator_with_max_active_sessions(100)
    }

    #[test]
    fn conversation_coordinator_exposes_remote_runtime_ports() {
        fn assert_cancellation_port<T: bitfun_runtime_ports::AgentTurnCancellationPort>() {}
        fn assert_state_port<T: bitfun_runtime_ports::RemoteControlStatePort>() {}

        assert_cancellation_port::<ConversationCoordinator>();
        assert_state_port::<ConversationCoordinator>();
    }

    #[test]
    fn local_cli_confirmation_override_excludes_acp_transport() {
        let policy = DialogSubmissionPolicy::new(
            AgentSubmissionSource::Cli,
            DialogQueuePriority::Normal,
            false,
        );

        assert!(should_require_tool_confirmation(policy, None));
        assert!(!should_require_tool_confirmation(
            policy,
            Some(&serde_json::json!({ "acp_transport": true })),
        ));
    }

    #[test]
    fn peer_metadata_can_only_strengthen_tool_confirmation() {
        let desktop_policy = DialogSubmissionPolicy::new(
            AgentSubmissionSource::DesktopUi,
            DialogQueuePriority::Normal,
            false,
        );
        let peer_metadata = serde_json::json!({ "require_tool_confirmation": true });

        assert!(should_require_tool_confirmation(
            desktop_policy,
            Some(&peer_metadata)
        ));
        assert!(!should_require_tool_confirmation(
            desktop_policy,
            Some(&serde_json::json!({
                "require_tool_confirmation": true,
                "acp_transport": true,
            }))
        ));
    }

    #[test]
    fn background_subagent_delivery_identifies_its_exact_parent_turn() {
        let parent = SubagentParentInfo {
            session_id: "parent-session".to_string(),
            dialog_turn_id: "parent-turn".to_string(),
            tool_call_id: "tool-call".to_string(),
        };

        let metadata = background_subagent_delivery_metadata(
            "background-task",
            &parent,
            "subagent-session",
            "subagent-turn",
            true,
            "agentic",
            "Investigate",
        );

        assert_eq!(metadata["kind"], serde_json::json!("background_result"));
        assert_eq!(metadata["sourceKind"], serde_json::json!("subagent"));
        assert_eq!(
            metadata["parentSessionId"],
            serde_json::json!("parent-session")
        );
        assert_eq!(
            metadata["parentDialogTurnId"],
            serde_json::json!("parent-turn")
        );
        assert_eq!(
            metadata["subagentSessionId"],
            serde_json::json!("subagent-session")
        );
        assert_eq!(
            metadata["subagentDialogTurnId"],
            serde_json::json!("subagent-turn")
        );
        assert_eq!(
            metadata["require_tool_confirmation"],
            serde_json::json!(true)
        );
        assert!(should_require_tool_confirmation(
            DialogSubmissionPolicy::for_source(AgentSubmissionSource::AgentSession),
            Some(&serde_json::Value::Object(metadata))
        ));

        let non_peer_metadata = background_subagent_delivery_metadata(
            "background-task",
            &parent,
            "subagent-session",
            "subagent-turn",
            false,
            "agentic",
            "Investigate",
        );
        assert!(!non_peer_metadata.contains_key("require_tool_confirmation"));
    }

    #[tokio::test]
    async fn completed_background_control_cannot_be_claimed_for_late_cancellation() {
        let (coordinator, _) = test_coordinator();
        coordinator.register_background_subagent_task(
            "background-task".to_string(),
            "parent-session".to_string(),
            "subagent-session".to_string(),
            super::BackgroundSubagentCancelTarget::Direct(
                tokio_util::sync::CancellationToken::new(),
            ),
        );
        coordinator.finish_background_subagent_delivery("background-task");

        let claimed = coordinator.claim_background_subagent_controls(|control| {
            control.parent_session_id == "parent-session"
                && control.subagent_session_id == "subagent-session"
        });

        assert!(claimed.is_empty());
        assert!(!coordinator.take_background_subagent_delivery_suppression("background-task"));
    }

    #[tokio::test]
    async fn injection_and_cancellation_claim_the_same_background_control_once() {
        let (coordinator, _) = test_coordinator();
        coordinator.register_background_subagent_task(
            "delivery-wins".to_string(),
            "parent-session".to_string(),
            "subagent-session".to_string(),
            super::BackgroundSubagentCancelTarget::Direct(
                tokio_util::sync::CancellationToken::new(),
            ),
        );
        assert!(coordinator.claim_background_subagent_control_for_injection("delivery-wins"));
        assert!(coordinator
            .claim_background_subagent_controls(|control| {
                control.parent_session_id == "parent-session"
                    && control.subagent_session_id == "subagent-session"
            })
            .is_empty());

        coordinator.register_background_subagent_task(
            "cancellation-wins".to_string(),
            "parent-session".to_string(),
            "subagent-session".to_string(),
            super::BackgroundSubagentCancelTarget::Direct(
                tokio_util::sync::CancellationToken::new(),
            ),
        );
        assert_eq!(
            coordinator
                .claim_background_subagent_controls(|control| {
                    control.parent_session_id == "parent-session"
                        && control.subagent_session_id == "subagent-session"
                })
                .len(),
            1
        );
        assert!(!coordinator.claim_background_subagent_control_for_injection("cancellation-wins"));
        assert!(coordinator.take_background_subagent_delivery_suppression("cancellation-wins"));
    }

    #[tokio::test]
    async fn coordinator_test_fixture_injects_terminal_port() {
        let (coordinator, _) = test_coordinator();

        assert!(coordinator.terminal_port().is_some());
        assert!(coordinator.remote_exec_port().is_some());
    }

    #[test]
    fn clamps_subagent_max_concurrency_into_safe_range() {
        assert_eq!(normalize_subagent_max_concurrency(0), 1);
        assert_eq!(normalize_subagent_max_concurrency(5), 5);
        assert_eq!(normalize_subagent_max_concurrency(usize::MAX), 64);
    }

    #[tokio::test]
    async fn review_background_delivery_requires_explicit_follow_up_permission() {
        let error = validate_background_subagent_delivery("CodeReview", None, false)
            .await
            .expect_err("review background delivery should require explicit follow-up permission");

        assert!(error.to_string().contains("one final review"));
        assert!(error.to_string().contains("allow_review_follow_up=true"));
        assert!(
            validate_background_subagent_delivery("CodeReview", None, true)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn explicit_review_follow_up_allows_fresh_background_resolution() {
        let (coordinator, _) = test_coordinator();
        let workspace_path = std::env::temp_dir().join(format!(
            "bitfun-review-follow-up-fresh-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace_path).expect("workspace dir should exist");
        let request = SubagentExecutionRequest {
            task_description: "Review the current diff later".to_string(),
            context_mode: SubagentContextMode::Fresh,
            target_session_id: None,
            subagent_type: Some("CodeReview".to_string()),
            workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
            model_id: None,
            subagent_parent_info: SubagentParentInfo {
                session_id: "parent-session".to_string(),
                dialog_turn_id: "parent-turn".to_string(),
                tool_call_id: "task-tool".to_string(),
            },
            context: HashMap::new(),
            delegation_policy: DelegationPolicy::top_level().spawn_child(),
        };

        let resolved = coordinator
            .resolve_hidden_subagent_execution_request(request, Some(true))
            .await
            .expect("explicit review follow-up should resolve a fresh background request");

        assert_eq!(resolved.agent_type, "CodeReview");
        assert!(resolved.target_session_id.is_none());
        let _ = std::fs::remove_dir_all(workspace_path);
    }

    #[tokio::test]
    async fn explicit_review_follow_up_allows_reused_background_resolution() {
        let (coordinator, _) = test_coordinator();
        let workspace_path = std::env::temp_dir().join(format!(
            "bitfun-review-follow-up-reuse-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace_path).expect("workspace dir should exist");
        let review_session = coordinator
            .create_hidden_agent_session(
                None,
                "Reusable review".to_string(),
                "CodeReview".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
                    ..Default::default()
                },
                Some("session-parent-session".to_string()),
                SessionKind::Subagent,
            )
            .await
            .expect("review session should be created");
        let request = SubagentExecutionRequest {
            task_description: "Continue the review later".to_string(),
            context_mode: SubagentContextMode::Fresh,
            target_session_id: Some(review_session.session_id.clone()),
            subagent_type: None,
            workspace_path: None,
            model_id: None,
            subagent_parent_info: SubagentParentInfo {
                session_id: "parent-session".to_string(),
                dialog_turn_id: "parent-turn".to_string(),
                tool_call_id: "task-tool".to_string(),
            },
            context: HashMap::new(),
            delegation_policy: DelegationPolicy::top_level().spawn_child(),
        };

        let resolved = coordinator
            .resolve_hidden_subagent_execution_request(request, Some(true))
            .await
            .expect("explicit review follow-up should resolve a reused background request");

        assert_eq!(
            resolved.target_session_id.as_deref(),
            Some(review_session.session_id.as_str())
        );
        let _ = std::fs::remove_dir_all(workspace_path);
    }

    #[tokio::test]
    async fn non_review_background_delivery_preserves_existing_behavior() {
        assert!(
            validate_background_subagent_delivery("GeneralPurpose", None, false)
                .await
                .is_ok()
        );
        assert!(
            validate_background_subagent_delivery("ReviewFixer", None, false)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn continued_review_session_cannot_run_in_background_implicitly() {
        let (coordinator, session_manager) = test_coordinator();
        let review_session = coordinator
            .create_hidden_agent_session(
                None,
                "Reusable review".to_string(),
                "CodeReview".to_string(),
                SessionConfig {
                    model_id: Some("primary".to_string()),
                    workspace_path: Some(std::env::temp_dir().to_string_lossy().into_owned()),
                    ..Default::default()
                },
                Some("session-parent-session".to_string()),
                SessionKind::Subagent,
            )
            .await
            .expect("review session should be created");
        let request = SubagentExecutionRequest {
            task_description: "Continue reviewing the current diff".to_string(),
            context_mode: SubagentContextMode::Fresh,
            target_session_id: Some(review_session.session_id.clone()),
            subagent_type: None,
            workspace_path: None,
            model_id: Some("fast".to_string()),
            subagent_parent_info: SubagentParentInfo {
                session_id: "parent-session".to_string(),
                dialog_turn_id: "parent-turn".to_string(),
                tool_call_id: "task-tool".to_string(),
            },
            context: HashMap::new(),
            delegation_policy: DelegationPolicy::top_level().spawn_child(),
        };

        let error = coordinator
            .start_background_subagent(request, None, false)
            .await
            .expect_err(
                "continued review should not run in the background without explicit intent",
            );

        assert!(error.to_string().contains("one final review"));
        assert!(error.to_string().contains("allow_review_follow_up=true"));
        assert_eq!(
            session_manager
                .get_session(&review_session.session_id)
                .expect("review session should remain available")
                .config
                .model_id
                .as_deref(),
            Some("primary"),
            "rejected delivery must not mutate the review session model"
        );
    }

    #[tokio::test]
    async fn cold_review_session_is_rejected_before_full_restore() {
        let (coordinator, session_manager) = test_coordinator();
        let workspace_path = std::env::temp_dir().join(format!(
            "bitfun-cold-review-background-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace_path).expect("workspace dir should exist");
        let parent_session = session_manager
            .create_session(
                "Parent".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("parent session should be created");
        let storage_binding = session_manager
            .resolve_session_workspace_binding(&parent_session.session_id)
            .await
            .expect("parent workspace binding should resolve");
        let review_session_id = format!("session-cold-review-{}", uuid::Uuid::new_v4());
        let mut metadata = SessionMetadata::new(
            review_session_id.clone(),
            "Cold review".to_string(),
            "CodeReview".to_string(),
            "primary".to_string(),
        );
        metadata.session_kind = SessionKind::Subagent;
        metadata.created_by = Some(format!("session-{}", parent_session.session_id));
        metadata.workspace_path = Some(workspace_path.to_string_lossy().into_owned());
        metadata.relationship = Some(super::build_subagent_session_relationship(
            Some(&SubagentParentInfo {
                session_id: parent_session.session_id.clone(),
                dialog_turn_id: "parent-turn".to_string(),
                tool_call_id: "task-tool".to_string(),
            }),
            "CodeReview",
        ));
        session_manager
            .save_session_metadata(&storage_binding.session_storage_dir(), &metadata)
            .await
            .expect("cold review metadata should be persisted");
        assert!(session_manager.get_session(&review_session_id).is_none());

        let request = SubagentExecutionRequest {
            task_description: "Continue the cold review".to_string(),
            context_mode: SubagentContextMode::Fresh,
            target_session_id: Some(review_session_id.clone()),
            subagent_type: None,
            workspace_path: None,
            model_id: Some("fast".to_string()),
            subagent_parent_info: SubagentParentInfo {
                session_id: parent_session.session_id.clone(),
                dialog_turn_id: "parent-turn".to_string(),
                tool_call_id: "task-tool".to_string(),
            },
            context: HashMap::new(),
            delegation_policy: DelegationPolicy::top_level().spawn_child(),
        };

        let error = coordinator
            .resolve_hidden_subagent_execution_request(request, Some(false))
            .await
            .expect_err("cold review should reject background delivery before restore");

        assert!(error.to_string().contains("one final review"));
        assert!(session_manager.get_session(&review_session_id).is_none());
        let persisted = session_manager
            .load_session_metadata(&storage_binding.session_storage_dir(), &review_session_id)
            .await
            .expect("cold review metadata should remain readable")
            .expect("cold review metadata should remain present");
        assert_eq!(persisted.model_name, "primary");
        let _ = std::fs::remove_dir_all(workspace_path);
    }

    #[tokio::test]
    async fn cold_subagent_owned_by_another_parent_is_rejected_before_restore() {
        let (coordinator, session_manager) = test_coordinator();
        let workspace_path = std::env::temp_dir().join(format!(
            "bitfun-cold-foreign-subagent-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace_path).expect("workspace dir should exist");
        let parent_session = session_manager
            .create_session(
                "Parent".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("parent session should be created");
        let storage_binding = session_manager
            .resolve_session_workspace_binding(&parent_session.session_id)
            .await
            .expect("parent workspace binding should resolve");
        let review_session_id = format!("session-foreign-review-{}", uuid::Uuid::new_v4());
        let mut metadata = SessionMetadata::new(
            review_session_id.clone(),
            "Foreign review".to_string(),
            "CodeReview".to_string(),
            "primary".to_string(),
        );
        metadata.session_kind = SessionKind::Subagent;
        metadata.created_by = Some("session-another-parent".to_string());
        metadata.workspace_path = Some(workspace_path.to_string_lossy().into_owned());
        session_manager
            .save_session_metadata(&storage_binding.session_storage_dir(), &metadata)
            .await
            .expect("foreign review metadata should be persisted");

        let request = SubagentExecutionRequest {
            task_description: "Continue a foreign review".to_string(),
            context_mode: SubagentContextMode::Fresh,
            target_session_id: Some(review_session_id.clone()),
            subagent_type: None,
            workspace_path: None,
            model_id: None,
            subagent_parent_info: SubagentParentInfo {
                session_id: parent_session.session_id.clone(),
                dialog_turn_id: "parent-turn".to_string(),
                tool_call_id: "task-tool".to_string(),
            },
            context: HashMap::new(),
            delegation_policy: DelegationPolicy::top_level().spawn_child(),
        };

        let error = coordinator
            .resolve_hidden_subagent_execution_request(request, Some(false))
            .await
            .expect_err("foreign subagent should fail ownership preflight");

        assert!(error
            .to_string()
            .contains("was not created by parent session"));
        assert!(session_manager.get_session(&review_session_id).is_none());
        let _ = std::fs::remove_dir_all(workspace_path);
    }

    #[tokio::test]
    async fn cold_review_uses_read_only_session_workspace_before_restore() {
        let (coordinator, session_manager) = test_coordinator();
        let workspace_path = std::env::temp_dir().join(format!(
            "bitfun-cold-review-workspace-preflight-test-{}",
            uuid::Uuid::new_v4()
        ));
        let review_workspace = workspace_path.join("review-workspace");
        let reviewer_path = review_workspace
            .join(".bitfun")
            .join("agents")
            .join("cold-reviewer.md");
        std::fs::create_dir_all(
            reviewer_path
                .parent()
                .expect("reviewer path should have a parent"),
        )
        .expect("review agent directory should exist");
        let mut reviewer = CustomSubagent::new_with_id(
            "ColdWorkspaceReviewer".to_string(),
            "ColdWorkspaceReviewer".to_string(),
            "Project review subagent".to_string(),
            vec!["Read".to_string()],
            "Review the relevant files.".to_string(),
            true,
            reviewer_path.to_string_lossy().into_owned(),
            CustomSubagentKind::Project,
            "fast".to_string(),
            UserContextPolicy::empty().with_workspace_instructions(),
        );
        reviewer.data.review = true;
        reviewer
            .save_to_file(None)
            .expect("project review subagent should save");

        let parent_session = session_manager
            .create_session(
                "Parent".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("parent session should be created");
        let storage_binding = session_manager
            .resolve_session_workspace_binding(&parent_session.session_id)
            .await
            .expect("parent workspace binding should resolve");
        let review_session_id = format!("session-workspace-review-{}", uuid::Uuid::new_v4());
        let mut persisted_session = Session::new_with_id(
            review_session_id.clone(),
            "Workspace review".to_string(),
            "ColdWorkspaceReviewer".to_string(),
            SessionConfig {
                workspace_path: Some(review_workspace.to_string_lossy().into_owned()),
                ..Default::default()
            },
        );
        persisted_session.kind = SessionKind::Subagent;
        persisted_session.created_by = Some(format!("session-{}", parent_session.session_id));
        let persistence_manager = PersistenceManager::new(Arc::new(
            PathManager::new().expect("path manager should initialize"),
        ))
        .expect("persistence manager should initialize");
        persistence_manager
            .save_session(&storage_binding.session_storage_dir(), &persisted_session)
            .await
            .expect("cold review session should be persisted");
        let mut metadata = session_manager
            .load_session_metadata(&storage_binding.session_storage_dir(), &review_session_id)
            .await
            .expect("cold review metadata should load")
            .expect("cold review metadata should exist");
        metadata.workspace_path = Some(workspace_path.to_string_lossy().into_owned());
        session_manager
            .save_session_metadata(&storage_binding.session_storage_dir(), &metadata)
            .await
            .expect("metadata workspace should be overridden for the mismatch fixture");

        let request = SubagentExecutionRequest {
            task_description: "Continue the workspace review".to_string(),
            context_mode: SubagentContextMode::Fresh,
            target_session_id: Some(review_session_id.clone()),
            subagent_type: None,
            workspace_path: None,
            model_id: None,
            subagent_parent_info: SubagentParentInfo {
                session_id: parent_session.session_id.clone(),
                dialog_turn_id: "parent-turn".to_string(),
                tool_call_id: "task-tool".to_string(),
            },
            context: HashMap::new(),
            delegation_policy: DelegationPolicy::top_level().spawn_child(),
        };

        let error = coordinator
            .resolve_hidden_subagent_execution_request(request, Some(false))
            .await
            .expect_err("stored review workspace should be checked before full restore");

        assert!(error.to_string().contains("one final review"));
        assert!(session_manager.get_session(&review_session_id).is_none());
        let _ = std::fs::remove_dir_all(workspace_path);
    }

    #[test]
    fn subagent_timeout_disable_clears_active_deadline() {
        use super::SubagentTimeoutAction;
        use std::sync::Mutex;
        use tokio::sync::watch;
        use tokio::time::{Duration, Instant};

        let initial_deadline = Instant::now() + Duration::from_secs(1200);
        let (deadline_tx, mut deadline_rx) = watch::channel(Some(initial_deadline));
        let handle = super::SubagentTimeoutHandle {
            deadline_tx,
            session_id: "subagent-session".to_string(),
            original_timeout_seconds: Some(1200),
            remaining_at_pause: Mutex::new(None),
        };

        handle.apply_action(SubagentTimeoutAction::Disable);

        assert!(deadline_rx.borrow_and_update().is_none());
    }

    #[test]
    fn subagent_lineage_ownership_requires_matching_parent() {
        use crate::service::session::{SessionRelationship, SessionRelationshipKind};

        let relationship = SessionRelationship {
            kind: Some(SessionRelationshipKind::Subagent),
            parent_session_id: Some("parent-session".to_string()),
            parent_request_id: None,
            parent_dialog_turn_id: None,
            parent_turn_index: None,
            parent_tool_call_id: None,
            subagent_type: None,
        };

        assert!(super::session_lineage_matches_parent(
            Some(&relationship),
            "parent-session"
        ));
        assert!(!super::session_lineage_matches_parent(
            Some(&relationship),
            "other-parent"
        ));
        assert!(!super::session_lineage_matches_parent(
            None,
            "parent-session"
        ));
    }

    #[test]
    fn background_subagent_delivery_text_uses_session_id() {
        let completed = super::SubagentResult::completed("done".to_string());
        let completed_text =
            super::format_background_subagent_delivery_text("subagent-session-123", Ok(&completed));
        assert!(completed_text.contains(
            "Background subagent (session_id='subagent-session-123') completed successfully:"
        ));
        assert!(completed_text.contains("<result>\n"));
        assert!(!completed_text.contains("background_task_id"));
        assert!(!completed_text.contains("bg-subagent-123"));

        let partial =
            super::SubagentResult::partial_timeout("partial".to_string(), "timeout".to_string());
        let partial_text =
            super::format_background_subagent_delivery_text("subagent-session-456", Ok(&partial));
        assert!(partial_text.contains(
            "Background subagent (session_id='subagent-session-456') completed with partial timeout result:"
        ));
        assert!(partial_text.contains("<partial_result status=\"partial_timeout\">\n"));
        assert!(!partial_text.contains("background_task_id"));
        assert!(!partial_text.contains("bg-subagent-456"));

        let failed_text = super::format_background_subagent_delivery_text(
            "subagent-session-789",
            Err(&crate::util::errors::BitFunError::tool("boom".to_string())),
        );
        assert!(failed_text.contains(
            "Background subagent (session_id='subagent-session-789') failed before producing a final result."
        ));
        assert!(!failed_text.contains("background_task_id"));
        assert!(!failed_text.contains("bg-subagent-789"));
        assert!(failed_text.contains("Error:"));
    }

    #[test]
    fn background_subagent_display_text_is_concise() {
        let completed = super::SubagentResult::completed("done".to_string());
        assert_eq!(
            super::format_background_subagent_display_text(Ok(&completed)),
            "Background subagent completed successfully."
        );

        let partial =
            super::SubagentResult::partial_timeout("partial".to_string(), "timeout".to_string());
        assert_eq!(
            super::format_background_subagent_display_text(Ok(&partial)),
            "Background subagent completed with a partial timeout result."
        );

        assert_eq!(
            super::format_background_subagent_display_text(Err(
                &crate::util::errors::BitFunError::tool("boom".to_string())
            )),
            "Background subagent failed before producing a final result."
        );
    }

    #[test]
    fn agent_submission_turn_id_prefers_explicit_field_over_metadata() {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "turnId".to_string(),
            serde_json::Value::String("legacy_metadata_turn".to_string()),
        );
        let request = AgentSubmissionRequest {
            session_id: "session_1".to_string(),
            message: "hello".to_string(),
            turn_id: Some("explicit_turn".to_string()),
            source: Some(AgentSubmissionSource::RemoteRelay),
            attachments: Vec::new(),
            metadata,
        };

        assert_eq!(resolve_agent_submission_turn_id(&request), "explicit_turn");
    }

    #[test]
    fn agent_submission_turn_id_keeps_metadata_fallback() {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "turnId".to_string(),
            serde_json::Value::String("legacy_metadata_turn".to_string()),
        );
        let request = AgentSubmissionRequest {
            session_id: "session_1".to_string(),
            message: "hello".to_string(),
            turn_id: None,
            source: Some(AgentSubmissionSource::RemoteRelay),
            attachments: Vec::new(),
            metadata,
        };

        assert_eq!(
            resolve_agent_submission_turn_id(&request),
            "legacy_metadata_turn"
        );
    }

    #[test]
    fn agent_session_create_created_by_accepts_camel_case_metadata() {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "createdBy".to_string(),
            serde_json::Value::String("session-parent".to_string()),
        );

        assert_eq!(
            resolve_agent_session_create_created_by(&metadata).as_deref(),
            Some("session-parent")
        );
    }

    #[test]
    fn agent_session_create_created_by_accepts_snake_case_metadata() {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "created_by".to_string(),
            serde_json::Value::String("session-parent".to_string()),
        );

        assert_eq!(
            resolve_agent_session_create_created_by(&metadata).as_deref(),
            Some("session-parent")
        );
    }

    #[tokio::test]
    async fn agent_submission_create_session_preserves_creator_metadata() {
        let (coordinator, session_manager) = test_coordinator();
        let workspace_path = std::env::temp_dir().join(format!(
            "bitfun-agent-session-port-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace_path).expect("workspace dir should exist");
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "createdBy".to_string(),
            serde_json::Value::String("session-parent".to_string()),
        );

        let result = AgentSubmissionPort::create_session(
            &coordinator,
            AgentSessionCreateRequest {
                session_name: "Worker".to_string(),
                agent_type: "agentic".to_string(),
                workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
                remote_connection_id: None,
                remote_ssh_host: None,
                metadata,
            },
        )
        .await
        .expect("port-backed session creation should succeed");
        let created = session_manager
            .get_session(&result.session_id)
            .expect("created session should be persisted");

        assert_eq!(result.session_name, "Worker");
        assert_eq!(result.session_name, created.session_name);
        assert_eq!(created.created_by.as_deref(), Some("session-parent"));

        let _ = std::fs::remove_dir_all(workspace_path);
    }

    #[tokio::test]
    async fn agent_submission_create_session_preserves_v1_backend_error_classification() {
        let (coordinator, _) = test_coordinator_with_max_active_sessions(0);
        let error = AgentSubmissionPort::create_session(
            &coordinator,
            AgentSessionCreateRequest {
                session_name: "Over capacity".to_string(),
                agent_type: "agentic".to_string(),
                workspace_path: Some(std::env::temp_dir().to_string_lossy().into_owned()),
                remote_connection_id: None,
                remote_ssh_host: None,
                metadata: serde_json::Map::new(),
            },
        )
        .await
        .expect_err("v1 create should preserve its backend error classification");

        assert_eq!(error.kind, bitfun_runtime_ports::PortErrorKind::Backend);
    }

    #[tokio::test]
    async fn agent_submission_create_session_preserves_requested_session_id() {
        let (coordinator, session_manager) = test_coordinator();
        let workspace_path = std::env::temp_dir().join(format!(
            "bitfun-agent-session-fixed-id-port-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace_path).expect("workspace dir should exist");

        let result = AgentSubmissionPort::create_session_with_id(
            &coordinator,
            "fixed-session-id".to_string(),
            AgentSessionCreateRequest {
                session_name: "Fixed worker".to_string(),
                agent_type: "agentic".to_string(),
                workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
                remote_connection_id: None,
                remote_ssh_host: None,
                metadata: serde_json::Map::new(),
            },
        )
        .await
        .expect("fixed-id session creation should succeed");

        assert_eq!(result.session_id, "fixed-session-id");
        assert!(session_manager.get_session("fixed-session-id").is_some());

        let duplicate_error = AgentSubmissionPort::create_session_with_id(
            &coordinator,
            "fixed-session-id".to_string(),
            AgentSessionCreateRequest {
                session_name: "Duplicate worker".to_string(),
                agent_type: "agentic".to_string(),
                workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
                remote_connection_id: None,
                remote_ssh_host: None,
                metadata: serde_json::Map::new(),
            },
        )
        .await
        .expect_err("duplicate fixed session id should be rejected");
        assert_eq!(
            duplicate_error.kind,
            bitfun_runtime_ports::PortErrorKind::InvalidRequest
        );
        assert!(duplicate_error.message.contains("already exists"));
        assert_eq!(
            session_manager
                .get_session("fixed-session-id")
                .expect("original fixed-id session should remain")
                .session_name,
            "Fixed worker"
        );

        let _ = std::fs::remove_dir_all(workspace_path);
    }

    #[tokio::test]
    async fn agent_submission_create_session_rejects_invalid_requested_session_id() {
        let (coordinator, _) = test_coordinator();
        let error = AgentSubmissionPort::create_session_with_id(
            &coordinator,
            "../other-session".to_string(),
            AgentSessionCreateRequest {
                session_name: "Invalid worker".to_string(),
                agent_type: "agentic".to_string(),
                workspace_path: Some(std::env::temp_dir().to_string_lossy().into_owned()),
                remote_connection_id: None,
                remote_ssh_host: None,
                metadata: serde_json::Map::new(),
            },
        )
        .await
        .expect_err("invalid fixed session id should be rejected");

        assert_eq!(
            error.kind,
            bitfun_runtime_ports::PortErrorKind::InvalidRequest
        );
    }

    #[tokio::test]
    async fn subagent_session_config_preserves_registered_remote_workspace_identity() {
        let manager = init_remote_workspace_manager();
        manager
            .register_remote_workspace(
                "/remote/subagent-test".to_string(),
                "conn-subagent-test".to_string(),
                "Remote Test".to_string(),
                "remote-host".to_string(),
            )
            .await;
        manager
            .set_active_connection_hint(Some("conn-subagent-test".to_string()))
            .await;

        let config = ConversationCoordinator::build_session_config_for_workspace(
            "/remote/subagent-test/project".to_string(),
            Some("model-fast".to_string()),
        )
        .await;

        assert_eq!(
            config.workspace_path.as_deref(),
            Some("/remote/subagent-test/project")
        );
        assert_eq!(
            config.remote_connection_id.as_deref(),
            Some("conn-subagent-test")
        );
        assert_eq!(config.remote_ssh_host.as_deref(), Some("remote-host"));
        assert_eq!(config.model_id.as_deref(), Some("model-fast"));
    }

    #[tokio::test]
    async fn reused_subagent_send_input_updates_requested_model() {
        let (coordinator, session_manager) = test_coordinator();
        let workspace_path = std::env::temp_dir().join(format!(
            "bitfun-reused-subagent-model-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace_path).expect("workspace dir should exist");
        struct TempWorkspaceGuard(std::path::PathBuf);
        impl Drop for TempWorkspaceGuard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _workspace_guard = TempWorkspaceGuard(workspace_path.clone());

        let subagent_session = coordinator
            .create_hidden_agent_session(
                None,
                "Reusable subagent".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    model_id: Some("parent-model".to_string()),
                    workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
                    ..Default::default()
                },
                Some("session-parent-session".to_string()),
                SessionKind::Subagent,
            )
            .await
            .expect("subagent session should be created");

        let request = SubagentExecutionRequest {
            task_description: "Continue the investigation".to_string(),
            context_mode: SubagentContextMode::Fresh,
            target_session_id: Some(subagent_session.session_id.clone()),
            subagent_type: None,
            workspace_path: None,
            model_id: Some("fast".to_string()),
            subagent_parent_info: SubagentParentInfo {
                session_id: "parent-session".to_string(),
                dialog_turn_id: "parent-turn".to_string(),
                tool_call_id: "task-tool".to_string(),
            },
            context: HashMap::new(),
            delegation_policy: DelegationPolicy::top_level().spawn_child(),
        };

        let prepared = coordinator
            .prepare_subagent_execution_request(request)
            .await
            .expect("send_input request should prepare with a requested model");

        assert_eq!(prepared.session_config.model_id.as_deref(), Some("fast"));
        assert_eq!(
            session_manager
                .get_session(&subagent_session.session_id)
                .expect("subagent session should remain available")
                .config
                .model_id
                .as_deref(),
            Some("fast")
        );
    }

    #[tokio::test]
    async fn fork_subagent_request_allows_requested_model_override() {
        let (coordinator, session_manager) = test_coordinator();
        let workspace_path =
            std::env::temp_dir().join(format!("bitfun-fork-model-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&workspace_path).expect("workspace dir should exist");
        struct TempWorkspaceGuard(std::path::PathBuf);
        impl Drop for TempWorkspaceGuard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _workspace_guard = TempWorkspaceGuard(workspace_path.clone());

        let parent_session = session_manager
            .create_session(
                "Parent".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    model_id: Some("parent-model".to_string()),
                    workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("parent session should be created");
        session_manager
            .replace_context_messages(
                &parent_session.session_id,
                vec![Message::user("parent context".to_string())],
            )
            .await;

        let request = SubagentExecutionRequest {
            task_description: "Fork and inspect the repo".to_string(),
            context_mode: SubagentContextMode::Fork,
            target_session_id: None,
            subagent_type: None,
            workspace_path: None,
            model_id: Some("fast".to_string()),
            subagent_parent_info: SubagentParentInfo {
                session_id: parent_session.session_id.clone(),
                dialog_turn_id: "parent-turn".to_string(),
                tool_call_id: "task-tool".to_string(),
            },
            context: HashMap::new(),
            delegation_policy: DelegationPolicy::top_level().spawn_child(),
        };

        let prepared = coordinator
            .prepare_subagent_execution_request(request)
            .await
            .expect("fork request should prepare with a requested model");

        assert_eq!(prepared.session_config.model_id.as_deref(), Some("fast"));
        assert_eq!(
            prepared.prompt_cache_source_session_id.as_deref(),
            Some(parent_session.session_id.as_str())
        );
        assert_eq!(
            session_manager
                .get_session(prepared.target_session_id().expect("prepared session id"))
                .expect("forked subagent session should exist")
                .config
                .model_id
                .as_deref(),
            Some("fast")
        );
    }

    #[tokio::test]
    async fn hidden_agent_session_uses_requested_ephemeral_kind() {
        let (coordinator, session_manager) = test_coordinator();
        let workspace_path = std::env::temp_dir().join(format!(
            "bitfun-hidden-agent-kind-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace_path).expect("workspace dir should exist");
        struct TempWorkspaceGuard(std::path::PathBuf);
        impl Drop for TempWorkspaceGuard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _workspace_guard = TempWorkspaceGuard(workspace_path.clone());

        let session = coordinator
            .create_hidden_agent_session(
                None,
                "Internal worker".to_string(),
                "MemoryPhase2".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
                    ..Default::default()
                },
                Some("memory-phase2".to_string()),
                SessionKind::EphemeralChild,
            )
            .await
            .expect("hidden agent session should be created");

        assert_eq!(session.kind, SessionKind::EphemeralChild);
        assert_eq!(
            session_manager
                .get_session(&session.session_id)
                .expect("session should remain in memory")
                .kind,
            SessionKind::EphemeralChild
        );
    }

    #[tokio::test]
    async fn reused_subagent_input_is_added_to_runtime_context() {
        let (coordinator, session_manager) = test_coordinator();
        let workspace_path = std::env::temp_dir().join(format!(
            "bitfun-reused-subagent-input-context-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace_path).expect("workspace dir should exist");
        struct TempWorkspaceGuard(std::path::PathBuf);
        impl Drop for TempWorkspaceGuard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _workspace_guard = TempWorkspaceGuard(workspace_path.clone());

        let session = coordinator
            .create_hidden_agent_session(
                None,
                "Reusable subagent".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
                    ..Default::default()
                },
                Some("session-parent".to_string()),
                SessionKind::Subagent,
            )
            .await
            .expect("subagent session should be created");
        session_manager
            .replace_context_messages(
                &session.session_id,
                vec![Message::assistant("previous answer".to_string())],
            )
            .await;

        let turn_id = session_manager
            .start_dialog_turn_with_existing_context(
                &session.session_id,
                "agentic".to_string(),
                "continue investigation".to_string(),
                Some("subagent-turn-reuse".to_string()),
                None,
            )
            .await
            .expect("turn should start");
        coordinator
            .persist_reused_subagent_user_input_context_if_needed(
                Some(&session.session_id),
                false,
                &session.session_id,
                &turn_id,
                "continue investigation",
            )
            .await
            .expect("user input context should persist");

        let context_messages = session_manager
            .get_context_messages(&session.session_id)
            .await
            .expect("context should be readable");
        assert_eq!(context_messages.len(), 2);
        let user_message = context_messages.last().expect("user message should exist");
        assert_eq!(user_message.role, MessageRole::User);
        assert_eq!(
            user_message.metadata.turn_id.as_deref(),
            Some("subagent-turn-reuse")
        );
        assert_eq!(
            user_message.metadata.semantic_kind,
            Some(MessageSemanticKind::ActualUserInput)
        );
        assert!(matches!(
            &user_message.content,
            MessageContent::Text(text) if text == "continue investigation"
        ));
    }

    #[tokio::test]
    async fn hidden_btw_session_seeds_forked_listing_baselines() {
        let (coordinator, session_manager) = test_coordinator();
        let workspace_path =
            std::env::temp_dir().join(format!("bitfun-btw-baseline-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&workspace_path).expect("workspace dir should exist");
        struct TempWorkspaceGuard(std::path::PathBuf);
        impl Drop for TempWorkspaceGuard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _workspace_guard = TempWorkspaceGuard(workspace_path.clone());

        let parent_session = session_manager
            .create_session(
                "Parent".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("parent session should be created");
        session_manager
            .inherit_session_agent_type_state(
                &parent_session.session_id,
                Some("agentic".to_string()),
                Some("agentic".to_string()),
            )
            .await
            .expect("parent agent type state should be set");
        session_manager
            .replace_context_messages(
                &parent_session.session_id,
                vec![crate::agentic::core::Message::user(
                    "parent context".to_string(),
                )],
            )
            .await;

        let system_prompt_identity = SystemPromptCacheIdentity::new("template:agentic_mode");
        let user_context_identity = UserContextCacheIdentity::new("workspace_context");
        session_manager
            .remember_system_prompt(
                &parent_session.session_id,
                system_prompt_identity.clone(),
                "cached system prompt".to_string(),
            )
            .await;
        session_manager
            .remember_user_context(
                &parent_session.session_id,
                user_context_identity.clone(),
                "cached user context".to_string(),
            )
            .await;

        let baseline_snapshot = TurnSkillAgentSnapshot {
            skills: vec![SkillSnapshotEntry {
                name: "interactive-debug".to_string(),
                description: "debug helper".to_string(),
                location: "C:/Users/wsp/.codex/skills/interactive-debug".to_string(),
            }],
            subagents: Vec::new(),
        };
        session_manager
            .remember_turn_skill_agent_snapshot(
                &parent_session.session_id,
                0,
                baseline_snapshot.clone(),
            )
            .await;

        let child_session = coordinator
            .ensure_hidden_btw_session(&parent_session.session_id, "btw-child", None)
            .await
            .expect("btw child session should be created");

        assert_eq!(
            child_session.kind,
            crate::agentic::core::SessionKind::EphemeralChild
        );
        assert_eq!(
            child_session.last_user_dialog_agent_type.as_deref(),
            Some("agentic")
        );
        assert_eq!(
            child_session.last_submitted_agent_type.as_deref(),
            Some("agentic")
        );
        assert_eq!(
            session_manager
                .cached_system_prompt(&child_session.session_id, &system_prompt_identity)
                .await,
            Some("cached system prompt".to_string())
        );
        assert_eq!(
            session_manager
                .cached_user_context(&child_session.session_id, &user_context_identity)
                .await,
            Some("cached user context".to_string())
        );
        assert_eq!(
            session_manager
                .skill_agent_baseline_override_snapshot(&child_session.session_id)
                .await,
            Some(baseline_snapshot.clone())
        );
        assert_eq!(
            session_manager
                .turn_skill_agent_snapshot(&child_session.session_id, 0)
                .await,
            Some(baseline_snapshot)
        );
    }

    #[test]
    fn merge_prepended_messages_places_scheduled_job_after_mode_reminder() {
        let merged = merge_prepended_messages_for_turn(
            vec![
                Message::internal_reminder(InternalReminderKind::ScheduledJob, "scheduled"),
                Message::internal_reminder(InternalReminderKind::Generic, "generic"),
            ],
            vec![
                Message::internal_reminder(InternalReminderKind::SkillListingDiff, "skills"),
                Message::internal_reminder(InternalReminderKind::AgentMode, "mode"),
            ],
            true,
        );

        let kinds = merged
            .iter()
            .map(|message| message.internal_reminder_kind())
            .collect::<Vec<_>>();

        assert_eq!(
            kinds,
            vec![
                Some(InternalReminderKind::Generic),
                Some(InternalReminderKind::SkillListingDiff),
                Some(InternalReminderKind::AgentMode),
                Some(InternalReminderKind::RemoteFileDelivery),
                Some(InternalReminderKind::ScheduledJob),
            ]
        );
    }

    #[test]
    fn turn_review_manifest_is_ignored_for_ordinary_agents() {
        let metadata = serde_json::json!({
            "deepReviewRunManifest": { "reviewTargetEvidence": { "version": 1 } }
        });

        assert!(turn_review_manifest_for_agent(Some(&metadata), "agentic").is_none());
        assert!(turn_review_manifest_for_agent(Some(&metadata), "CodeReview").is_some());
        assert!(turn_review_manifest_for_agent(Some(&metadata), "DeepReview").is_some());
    }
}
