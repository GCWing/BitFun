//! Dialog scheduler
//!
//! Message queue manager that automatically dispatches queued messages
//! when the target session becomes idle.
//!
//! Acts as the primary entry point for all user-facing message submissions,
//! wrapping ConversationCoordinator with:
//! - Per-session priority queue (max 20 messages)
//! - Higher-priority messages dispatched before lower-priority ones
//! - FIFO ordering within the same priority level
//! - Queue cleared on unrecoverable failure

use super::coordinator::{
    session_storage_workspace_locator, ConversationCoordinator, DialogTriggerSource,
    HiddenSubagentExecutionRequest, SubagentResult, SubagentResultStatus,
};
use super::plan_todo_binding::{
    auto_mark_todo_completed_if_bound, auto_mark_todo_in_progress_if_bound,
};
use super::turn_outcome::TurnOutcome;
use super::turn_settlement::TurnSettlementRegistration;
use crate::agentic::core::{
    InternalReminderKind, Message, Session, SessionKind, SessionState, SessionSummary,
};
use crate::agentic::events::AgenticEvent;
use crate::agentic::goal_mode::{
    goal_internal_context_message, goal_objective_updated_message, thread_goal_from_custom_metadata,
    GOAL_IDLE_WAKEUP_DELAY_MS,
};
use crate::agentic::image_analysis::ImageContextData;
use crate::agentic::init_agents_md::build_init_agents_md_user_input;
use crate::agentic::keyed_lock::{KeyedAsyncLock, KeyedAsyncLockGuard};
use crate::agentic::round_preempt::{DialogRoundInjectionSource, SessionRoundInjectionBuffer};
use crate::agentic::session::session_store_port::CoreSessionStorePort;
use crate::agentic::session::SessionManager;
use crate::agentic::tools::restrictions::get_session_role;
use crate::agentic::warden::runtime::{warden_enforcement_for_goal, WardenRuntime};
use crate::infrastructure::PathManager;
use crate::service::workspace::get_global_workspace_service;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_runtime_ports::ThreadGoal;
use log::{debug, info, warn};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use bitfun_agent_runtime::scheduler::{
    build_thread_goal_objective_updated_delivery_plan, build_thread_goal_resumed_delivery_plan,
    resolve_agent_session_reply_action, resolve_background_delivery_action,
    resolve_background_delivery_injection, resolve_background_delivery_injection_for_turn,
    resolve_dialog_start_route, resolve_dialog_steering_action,
    resolve_turn_outcome_lifecycle_plan, utc_iso8601_now, ActiveDialogTurn, ActiveDialogTurnStore,
    ActiveDialogTurnTakeResult, AgentSessionReplyAction, AgentSessionReplyPlan,
    BackgroundDeliveryAction, BackgroundDeliveryFacts, BackgroundInjectionKind,
    DialogReplySuppressionSet, DialogStartRoute, DialogStartRouteFacts, DialogSteeringAction,
    DialogTurnQueue, GoalContinuationAfterTurnAction, SessionAbortFlags,
    ThreadGoalDeliveryReminder, ThreadGoalDeliveryReminderKind, TurnOutcomeQueueAction,
    TurnOutcomeStatus,
};
use bitfun_runtime_ports::{
    resolve_dialog_submit_queue_action, AgentBackgroundResultRequest, AgentDialogPrependedReminder,
    AgentDialogSteerRequest, AgentDialogTurnExecution, AgentDialogTurnPort, AgentDialogTurnRequest,
    AgentInputAttachment, AgentLifecycleDeliveryPort, AgentSessionLineageInspection,
    AgentThreadGoalDeliveryKind, AgentThreadGoalDeliveryRequest, AgentTurnCancellationPort,
    AgentTurnCancellationRequest, AgentTurnCancellationResult, DialogSessionStateFact,
    DialogSubmitQueueAction, DialogSubmitQueueFacts, PortError, PortErrorKind, PortResult,
    SessionStoragePathRequest, SessionStorePort, SessionTranscriptRequest,
};
pub use bitfun_runtime_ports::{
    AgentSessionReplyRoute, DialogQueuePriority, DialogSteerOutcome, DialogSubmissionPolicy,
    DialogSubmitOutcome,
};

/// Resolve the configured goal idle-wakeup delay
/// (`ai.thresholds.goal.idle_wakeup_delay_ms`), falling back to
/// `GOAL_IDLE_WAKEUP_DELAY_MS = 600_000` when unset or invalid.
async fn configured_goal_idle_wakeup_delay_ms() -> u64 {
    let Ok(config_service) = crate::service::config::get_global_config_service().await else {
        return GOAL_IDLE_WAKEUP_DELAY_MS;
    };
    let Ok(thresholds) = config_service
        .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
        .await
    else {
        return GOAL_IDLE_WAKEUP_DELAY_MS;
    };
    let delay_ms = thresholds.goal.idle_wakeup_delay_ms;
    if delay_ms == 0 {
        return GOAL_IDLE_WAKEUP_DELAY_MS;
    }
    delay_ms
}

/// A message waiting to be dispatched to the coordinator
#[derive(Debug, Clone)]
pub struct QueuedTurn {
    pub user_input: String,
    pub original_user_input: Option<String>,
    pub prepended_messages: Vec<Message>,
    pub turn_id: Option<String>,
    pub agent_type: String,
    pub workspace_path: Option<String>,
    pub remote_connection_id: Option<String>,
    pub remote_ssh_host: Option<String>,
    pub policy: DialogSubmissionPolicy,
    pub reply_route: Option<AgentSessionReplyRoute>,
    pub user_message_metadata: Option<serde_json::Value>,
    pub image_contexts: Option<Vec<ImageContextData>>,
    #[allow(dead_code)]
    pub enqueued_at: SystemTime,
    _settlement_registration: Option<TurnSettlementRegistration>,
    execution: QueuedTurnExecution,
}

impl QueuedTurn {
    fn accept_settlement(&self) {
        if let Some(registration) = self._settlement_registration.as_ref() {
            registration.accept();
        }
    }
}

#[derive(Debug, Clone, Default)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum QueuedTurnExecution {
    #[default]
    Standard,
    FreshExternalSubagent(ExternalSubagentDelegationQueuedExecution),
    HiddenSubagent(HiddenSubagentQueuedExecution),
}

#[derive(Debug, Clone)]
pub(crate) struct ExternalSubagentDelegationQueuedExecution {
    ecosystem_id: String,
    logical_id: String,
}

fn remove_queued_turn_by_id(
    queues: &DialogTurnQueue<QueuedTurn>,
    session_id: &str,
    turn_id: &str,
) -> Option<QueuedTurn> {
    queues.remove_first_matching(session_id, |turn| turn.turn_id.as_deref() == Some(turn_id))
}

/// Pure decision helper for the goal idle-wakeup safety net: the whole
/// session tree (parent plus all subagent descendants at any depth) must be
/// silent. Any node that is busy or has activity newer than `idle_delay`
/// keeps the tree awake; a node that no longer exists contributes nothing.
fn session_tree_is_silent(
    tree_ids: &[String],
    now: SystemTime,
    idle_delay: Duration,
    is_busy_or_queued: impl Fn(&str) -> bool,
    last_activity_at: impl Fn(&str) -> Option<SystemTime>,
) -> bool {
    tree_ids.iter().all(|id| {
        if is_busy_or_queued(id) {
            return false;
        }
        match last_activity_at(id) {
            None => true,
            Some(activity) => now
                .duration_since(activity)
                .map(|elapsed| elapsed >= idle_delay)
                .unwrap_or(true),
        }
    })
}

/// Walk up the parent-session chain to find the tree root (the primary
/// conversation). Thread goals are only attachable to main sessions, so in
/// practice this returns `session_id` itself; the walk keeps the primary
/// condition robust if subagent goal support is ever added.
fn session_tree_root_id(summaries: &[SessionSummary], session_id: &str) -> String {
    let mut current = session_id.to_string();
    let mut hops = 0u32;
    loop {
        let parent = summaries
            .iter()
            .find(|summary| summary.session_id == current)
            .and_then(|summary| summary.parent_session_id.clone());
        match parent {
            Some(parent) if parent != current && hops < 64 => {
                current = parent;
                hops += 1;
            }
            _ => break,
        }
    }
    current
}

/// Pure decision helper: every conversation in the workspace is quiescent —
/// no session is busy (running or queued). This is the immediate branch of the
/// dual goal trigger: it does NOT require the `GOAL_IDLE_WAKEUP_DELAY_MS`
/// window, so the goal wakes up as soon as nothing in the workspace is running
/// or queued.
fn all_sessions_quiescent(
    all_ids: &[String],
    is_busy_or_queued: impl Fn(&str) -> bool,
) -> bool {
    all_ids.iter().all(|id| !is_busy_or_queued(id))
}

/// Pure decision helper for the dual-trigger goal idle-wakeup: the safety net
/// fires when the primary (tree-root) conversation has been silent for a full
/// idle window, OR every conversation in the workspace is quiescent (no
/// running or queued turn anywhere). Returns `(primary_silent,
/// all_sessions_silent)` so callers can log which condition (if any) held.
fn goal_idle_wakeup_conditions_met(
    primary_ids: &[String],
    all_ids: &[String],
    now: SystemTime,
    idle_delay: Duration,
    is_busy_or_queued: impl Fn(&str) -> bool,
    last_activity_at: impl Fn(&str) -> Option<SystemTime>,
) -> (bool, bool) {
    let primary_silent =
        session_tree_is_silent(primary_ids, now, idle_delay, &is_busy_or_queued, &last_activity_at);
    let all_silent = all_sessions_quiescent(all_ids, &is_busy_or_queued);
    (primary_silent, all_silent)
}

#[derive(Debug)]
enum SchedulerSubmitError {
    Core(BitFunError),
    Port(PortError),
    Message(String),
}

impl SchedulerSubmitError {
    fn into_port_error(self) -> PortError {
        match self {
            Self::Core(BitFunError::Validation(message)) => {
                PortError::new(PortErrorKind::InvalidRequest, message)
            }
            Self::Core(BitFunError::NotFound(message)) => {
                PortError::new(PortErrorKind::NotFound, message)
            }
            Self::Core(BitFunError::Cancelled(message)) => {
                PortError::new(PortErrorKind::Cancelled, message)
            }
            Self::Core(BitFunError::Timeout(message)) => {
                PortError::new(PortErrorKind::Timeout, message)
            }
            Self::Core(BitFunError::SessionInUse { session_id }) => PortError::new(
                PortErrorKind::SessionInUse,
                format!("Session is already open for writing: {session_id}"),
            ),
            Self::Core(BitFunError::OutcomeUnknown(message)) => {
                PortError::new(PortErrorKind::OutcomeUnknown, message)
            }
            Self::Core(BitFunError::NotImplemented(message)) => {
                PortError::new(PortErrorKind::NotAvailable, message)
            }
            Self::Core(error) => PortError::new(PortErrorKind::Backend, error.to_string()),
            Self::Port(error) => error,
            Self::Message(message) => PortError::new(PortErrorKind::Backend, message),
        }
    }
}

impl std::fmt::Display for SchedulerSubmitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(error) => error.fmt(formatter),
            Self::Port(error) => error.fmt(formatter),
            Self::Message(message) => formatter.write_str(message),
        }
    }
}

impl From<BitFunError> for SchedulerSubmitError {
    fn from(error: BitFunError) -> Self {
        Self::Core(error)
    }
}

impl From<String> for SchedulerSubmitError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<PortError> for SchedulerSubmitError {
    fn from(error: PortError) -> Self {
        Self::Port(error)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HiddenSubagentQueuedExecution {
    request: HiddenSubagentExecutionRequest,
    timeout_seconds: Option<u64>,
    result_tx: SharedSubagentResultSender,
    cancellation: HiddenSubagentQueueCancellation,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SharedSubagentResultSender {
    inner: Arc<std::sync::Mutex<Option<oneshot::Sender<BitFunResult<SubagentResult>>>>>,
}

impl SharedSubagentResultSender {
    fn new(sender: oneshot::Sender<BitFunResult<SubagentResult>>) -> Self {
        Self {
            inner: Arc::new(std::sync::Mutex::new(Some(sender))),
        }
    }

    fn send(&self, result: BitFunResult<SubagentResult>) {
        let Some(sender) = self.inner.lock().ok().and_then(|mut guard| guard.take()) else {
            return;
        };
        let _ = sender.send(result);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HiddenSubagentQueueCancellation {
    cancelled: Arc<AtomicBool>,
    token: CancellationToken,
}

impl Default for HiddenSubagentQueueCancellation {
    fn default() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            token: CancellationToken::new(),
        }
    }
}

impl HiddenSubagentQueueCancellation {
    fn cancel(&self) {
        self.cancelled.store(true, AtomicOrdering::SeqCst);
        self.token.cancel();
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(AtomicOrdering::SeqCst)
    }

    fn child_token(&self) -> CancellationToken {
        self.token.child_token()
    }
}

#[derive(Debug)]
pub(crate) struct HiddenSubagentSubmitResult {
    pub receiver: oneshot::Receiver<BitFunResult<SubagentResult>>,
    pub cancel_handle: HiddenSubagentQueueCancelHandle,
}

#[derive(Debug, Clone)]
pub(crate) struct HiddenSubagentQueueCancelHandle {
    session_id: String,
    turn_id: String,
    cancellation: HiddenSubagentQueueCancellation,
    result_tx: SharedSubagentResultSender,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveInternalTurn {
    HiddenSubagent,
}

#[derive(Clone)]
struct BackgroundResultDelivery {
    session_id: String,
    agent_type: String,
    workspace_path: Option<String>,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
    display_content: Option<String>,
    user_message_metadata: Option<serde_json::Value>,
}

/// Message queue manager for dialog turns.
///
/// All user-facing callers (frontend Tauri commands, remote server, bot router)
/// should submit messages through this scheduler instead of calling
/// ConversationCoordinator directly.
pub struct DialogScheduler {
    coordinator: Arc<ConversationCoordinator>,
    session_manager: Arc<SessionManager>,
    /// Per-session priority message queues.
    queues: Arc<DialogTurnQueue<QueuedTurn>>,
    /// Serializes submit, dispatch, and targeted cancellation for one session.
    /// This closes the dequeue-to-start gap where cancellation could otherwise
    /// miss both the queue and the coordinator's active execution.
    session_operation_locks: KeyedAsyncLock,
    /// Currently active turn metadata keyed by target session ID
    active_turns: Arc<ActiveDialogTurnStore>,
    active_internal_turns: Arc<dashmap::DashMap<String, ActiveInternalTurn>>,
    /// Turns whose cancelled auto-reply should be suppressed because the source
    /// agent explicitly cancelled its own outstanding SessionMessage request.
    suppressed_cancelled_replies: Arc<DialogReplySuppressionSet>,
    /// Exact outcomes retired by destructive session maintenance. The outcome
    /// channel may receive them only after the maintenance permit releases its
    /// per-session operation lock; tombstoning prevents them from mutating a
    /// newly created session that reuses the same explicit ID.
    retired_maintenance_outcomes: Arc<DialogReplySuppressionSet>,
    /// Set when the user cancels an in-flight turn; aborts goal-continuation submit retries.
    goal_continuation_abort: Arc<SessionAbortFlags>,
    /// Cloneable sender given to ConversationCoordinator for turn outcome notifications
    outcome_tx: mpsc::Sender<(String, TurnOutcome)>,
    /// Per-session FIFO buffer of round injections drained at round boundaries
    /// by the engine and injected into the running dialog turn. The buffer
    /// itself implements [`DialogRoundInjectionSource`], including
    /// `acknowledge_consumed` for UserSteering dedup, so no core-side wrapper
    /// is needed.
    round_injection_buffer: Arc<SessionRoundInjectionBuffer>,
    round_injection_source: Arc<SessionRoundInjectionBuffer>,
    /// Child sessions already cancelled for a parent maintenance attempt but
    /// not yet observed as drained. Retain them across retryable timeouts even
    /// after their one-shot cancellation controls have been claimed.
    maintenance_background_sessions: Arc<dashmap::DashMap<String, HashSet<String>>>,
    /// Per-session generation counter for goal idle-wakeup tasks. Each user
    /// submission bumps the generation; older wakeup tasks observe a stale
    /// generation when they fire and exit without doing anything (re-entrancy
    /// guard for the idle safety net).
    goal_idle_wakeup_generations: Arc<dashmap::DashMap<String, u64>>,
    /// Short-TTL cache of `session_has_active_goal` results (COORD-02). The
    /// uncached check touches the goal store on disk, which is too expensive
    /// to repeat on every outcome; a few seconds of staleness is harmless for
    /// Warden enforcement gating. Cleaned in `cleanup_session_state`.
    goal_active_cache: Arc<dashmap::DashMap<String, (Instant, bool)>>,
    /// Weak self-reference set after construction so spawned idle-wakeup tasks
    /// can upgrade to a strong reference and submit continuation turns.
    goal_idle_wakeup_self: OnceLock<std::sync::Weak<DialogScheduler>>,
    /// Warden runtime driving turn-level penalties and challenge pokes.
    /// Serialized behind a mutex because turns finalize concurrently.
    warden_runtime: Arc<tokio::sync::Mutex<WardenRuntime>>,
    /// Best-effort archive root for forwarded agent-session replies. Defaults
    /// to `~/.bitfun/agent-replies` on first use; tests inject a tempdir
    /// so outcome-handler tests never touch the real user home.
    agent_reply_archive_root: std::sync::Mutex<Option<PathBuf>>,
}

/// Holds the scheduler's exclusive session-operation boundary while a caller
/// performs maintenance that must not overlap turn dispatch.
pub(crate) struct SessionMaintenancePermit {
    _operation_guard: KeyedAsyncLockGuard,
    retired_turn_ids: Vec<String>,
}

impl SessionMaintenancePermit {
    pub(crate) fn retired_turn_ids(&self) -> &[String] {
        &self.retired_turn_ids
    }
}

fn take_active_turn_for_outcome(
    active_turns: &ActiveDialogTurnStore,
    retired_maintenance_outcomes: &DialogReplySuppressionSet,
    session_id: &str,
    turn_id: &str,
) -> Option<ActiveDialogTurnTakeResult> {
    if retired_maintenance_outcomes.take(session_id, turn_id) {
        None
    } else {
        Some(active_turns.take_for_outcome(session_id, turn_id))
    }
}

fn queued_submission_outcome(
    session_id: String,
    resolved_turn_id: String,
    started_turn_id: Option<String>,
) -> DialogSubmitOutcome {
    match started_turn_id {
        Some(turn_id) if turn_id == resolved_turn_id => DialogSubmitOutcome::Started {
            session_id,
            turn_id,
        },
        _ => DialogSubmitOutcome::Queued {
            session_id,
            turn_id: resolved_turn_id,
        },
    }
}

/// Whether a submission originates from a user-facing entry point. Agent-driven
/// (continuation, subagent) and scheduled-job submissions must not reset the
/// goal idle-wakeup timer.
fn is_user_submission_source(source: DialogTriggerSource) -> bool {
    matches!(
        source,
        DialogTriggerSource::DesktopUi
            | DialogTriggerSource::DesktopApi
            | DialogTriggerSource::Cli
            | DialogTriggerSource::Bot
            | DialogTriggerSource::RemoteRelay
            | DialogTriggerSource::SdkHost
    )
}

/// P-19：主会话通知只含极简元信息（session_id + 身份标识 + 已回复状态）。
///
/// 全量异步消息不回主会话，只由 P-03 persist_background_acp_turn 落盘成
/// turn，经 SessionHistory(session_id) 检索。命中/非命中通知标记一律返回
/// 极简元信息，不再保留全文旁路。
fn background_result_follow_up_user_input(session_id: &str, agent_type: &str) -> String {
    let identity = if agent_type.trim().is_empty() {
        "agent".to_string()
    } else {
        agent_type.to_string()
    };
    format!(
        "Background agent session {session_id} ({identity}) has replied; use SessionHistory to view the full reply."
    )
}

/// Whether `user_input` is a background-result follow-up notification text.
///
/// Both the scheduler follow-up path (`background_result_follow_up_user_input`)
/// and the coordinator direct-submit path (`background_subagent_follow_up_notice`
/// in coordinator.rs) emit the same fixed template, so one detector covers both
/// routes. User messages never match: the template starts with the literal
/// "Background agent session " prefix.
fn is_background_result_follow_up(user_input: &str) -> bool {
    user_input.starts_with("Background agent session ")
        && user_input.contains("has replied; use SessionHistory")
}

impl DialogScheduler {
    /// Create a new DialogScheduler and start its background outcome handler.
    ///
    /// The returned `Arc<DialogScheduler>` should be stored globally.
    /// Call `coordinator.set_scheduler_notifier(scheduler.outcome_sender())`
    /// immediately after to wire up the notification channel.
    pub fn new(
        coordinator: Arc<ConversationCoordinator>,
        session_manager: Arc<SessionManager>,
    ) -> Arc<Self> {
        let (outcome_tx, outcome_rx) = mpsc::channel(128);
        let round_injection_buffer = Arc::new(SessionRoundInjectionBuffer::default());
        let round_injection_source = round_injection_buffer.clone();

        let warden_session_manager = Arc::clone(&session_manager);
        // 持久化 Warden 耻辱墙，使记录的违规跨进程重启存活——
        // `WardenRuntime::new` 的注册表仅存内存。
        let warden_runtime = Arc::new(tokio::sync::Mutex::new(
            WardenRuntime::with_shame_wall_path(
                warden_session_manager,
                Self::resolve_warden_shame_wall_path(),
            ),
        ));
        // 阈值参数配置化：ai.thresholds.warden.max_defer_count / max_rate。
        // `DialogScheduler::new` 是同步构造链，配置读取需 async——通过
        // spawn 的后台任务在构造后异步注入（best-effort；默认 = 现值硬编码，
        // 未初始化 config service 时零回归）。
        let warden_runtime_for_config = warden_runtime.clone();
        tokio::spawn(async move {
            let mut warden = warden_runtime_for_config.lock().await;
            warden.apply_configured_thresholds().await;
        });
        // Inject the Warden runtime into the tool pipeline for tool-level
        // audit (custom point outside the hook dispatch channel). Must happen
        // before `coordinator` is moved into the struct below.
        coordinator.tool_pipeline().set_warden_runtime(warden_runtime.clone());
        let scheduler = Arc::new(Self {
            coordinator,
            session_manager,
            queues: Arc::new(DialogTurnQueue::default()),
            session_operation_locks: KeyedAsyncLock::default(),
            active_turns: Arc::new(ActiveDialogTurnStore::default()),
            active_internal_turns: Arc::new(dashmap::DashMap::new()),
            suppressed_cancelled_replies: Arc::new(DialogReplySuppressionSet::default()),
            retired_maintenance_outcomes: Arc::new(DialogReplySuppressionSet::default()),
            goal_continuation_abort: Arc::new(SessionAbortFlags::default()),
            outcome_tx,
            round_injection_buffer,
            round_injection_source,
            maintenance_background_sessions: Arc::new(dashmap::DashMap::new()),
            goal_idle_wakeup_generations: Arc::new(dashmap::DashMap::new()),
            goal_active_cache: Arc::new(dashmap::DashMap::new()),
            goal_idle_wakeup_self: OnceLock::new(),
            warden_runtime,
            agent_reply_archive_root: std::sync::Mutex::new(None),
        });
        let _ = scheduler
            .goal_idle_wakeup_self
            .set(std::sync::Arc::downgrade(&scheduler));

        let scheduler_for_handler = Arc::clone(&scheduler);
        tokio::spawn(async move {
            scheduler_for_handler.run_outcome_handler(outcome_rx).await;
        });

        // Best-effort recovery for goal idle-wakeup timers lost on process
        // restart (see `rearm_goal_idle_wakeups_after_startup`).
        let scheduler_for_rearm = Arc::clone(&scheduler);
        tokio::spawn(async move {
            scheduler_for_rearm.rearm_goal_idle_wakeups_after_startup().await;
        });

        scheduler
    }

    /// Returns a sender to give to ConversationCoordinator for turn outcome notifications.
    pub fn outcome_sender(&self) -> mpsc::Sender<(String, TurnOutcome)> {
        self.outcome_tx.clone()
    }

    /// Drop all per-session Warden state for `session_id` (session-end cleanup).
    ///
    /// Called by the coordinator when a session is deleted or discarded so a
    /// recycled session id cannot inherit stale enforcement state (failure
    /// counters, queued reminders, poke defer counts).
    pub async fn cleanup_session_state(&self, session_id: &str) {
        let mut warden = self.warden_runtime.lock().await;
        warden.cleanup_session(session_id);
        drop(warden);
        // COORD-11: the per-session in-memory tables only ever grow without
        // this cleanup. Removing them here keeps a recycled session id from
        // inheriting a stale generation counter (which would silently invalidate
        // new idle-wakeup schedules), a stale continuation-abort flag, or a
        // stale cached goal-active fact.
        self.goal_continuation_abort.clear(session_id);
        self.goal_idle_wakeup_generations.remove(session_id);
        self.goal_active_cache.remove(session_id);
        // COORD-11: suppression marks and retired-outcome tombstones are also
        // keyed by session id. A recycled session id must not inherit them:
        // a stale suppression mark would silently drop a cancelled-reply
        // bounce-back, and a stale tombstone would swallow a new turn outcome.
        self.suppressed_cancelled_replies.clear_session(session_id);
        self.retired_maintenance_outcomes.clear_session(session_id);
    }

    /// Inject the model-backed Warden judgement provider for Audit-Poke
    /// decisions (batch-2 warden rework).
    ///
    /// Forwarded to the tool pipeline, mirroring the `set_warden_runtime`
    /// injection in [`DialogScheduler::new`]; the host assembly (desktop)
    /// owns the concrete provider and calls this once after construction.
    pub fn set_warden_model_judgement(&self, port: Arc<dyn bitfun_runtime_ports::WardenModelJudgementPort>) {
        self.coordinator
            .tool_pipeline()
            .set_warden_model_judgement(port);
    }

    async fn lock_session_operation(&self, session_id: &str) -> KeyedAsyncLockGuard {
        self.session_operation_locks.lock(session_id).await
    }

    /// Upgrade the weak self-reference installed at construction, when the
    /// scheduler is still alive. Used to detach scheduler work into spawned
    /// tasks that need an owned `Arc<Self>`.
    fn self_arc(&self) -> Option<Arc<Self>> {
        let weak = self.goal_idle_wakeup_self.get()?.clone();
        weak.upgrade()
    }

    /// Pass to [`ConversationCoordinator::set_round_injection_source`](super::coordinator::ConversationCoordinator::set_round_injection_source).
    pub fn round_injection_monitor(&self) -> Arc<dyn DialogRoundInjectionSource> {
        self.round_injection_source.clone()
    }

    /// Extract the fixed background-notification text from a queued turn, if
    /// the turn is an agent-driven background-result follow-up (either the
    /// scheduler follow-up path or the coordinator direct-submit path).
    fn background_notice_for_queued_turn(queued_turn: &QueuedTurn) -> Option<String> {
        if queued_turn.policy.trigger_source != DialogTriggerSource::AgentSession {
            return None;
        }
        is_background_result_follow_up(&queued_turn.user_input)
            .then(|| queued_turn.user_input.clone())
    }

    /// Current running turn id when the session is `Processing`, otherwise `None`.
    ///
    /// This is the exact turn [`AgentDialogTurnPort::steer_dialog_turn`] can target.
    /// Callers that want to steer (e.g. an urgent agent-to-agent correction) query it
    /// first and fall back to a normal `submit` when no turn is running.
    pub fn current_processing_turn_id(&self, session_id: &str) -> Option<String> {
        match self
            .session_manager
            .get_session(session_id)
            .map(|s| s.state.clone())
        {
            Some(SessionState::Processing {
                current_turn_id, ..
            }) => Some(current_turn_id),
            _ => None,
        }
    }

    /// Submit a user "steering" message into the currently running dialog turn.
    ///
    /// Unlike [`Self::submit`], this never starts or queues a new turn — it only buffers
    /// the message so the [`ExecutionEngine`](super::super::execution::ExecutionEngine)
    /// can inject it at the next model-round boundary. Errors:
    ///
    /// - Session is not currently `Processing` the requested `turn_id` (the targeted turn
    ///   already finished or never existed). Callers must preserve the user's input so it
    ///   can be submitted explicitly after authoritative state is observed.
    async fn buffer_steering(
        &self,
        session_id: String,
        turn_id: String,
        content: String,
        display_content: Option<String>,
        prepended_reminders: Vec<AgentDialogPrependedReminder>,
    ) -> Result<DialogSteerOutcome, String> {
        if content.trim().is_empty() {
            return Err("Steering content cannot be empty".to_string());
        }
        let _operation_guard = self.lock_session_operation(&session_id).await;
        let active_turn_id = match self
            .session_manager
            .get_session(&session_id)
            .map(|s| s.state.clone())
        {
            Some(SessionState::Processing {
                current_turn_id, ..
            }) if self
                .active_turns
                .matches_turn(&session_id, &current_turn_id) =>
            {
                Some(current_turn_id)
            }
            _ => None,
        };

        let steering_id = Uuid::new_v4().to_string();
        match resolve_dialog_steering_action(
            active_turn_id.as_deref(),
            &session_id,
            &turn_id,
            content,
            display_content,
            steering_id,
            SystemTime::now(),
            prepended_reminders,
        ) {
            DialogSteeringAction::Reject { error } => {
                warn!(
                    "Steering rejected: target turn is not running: session_id={}, turn_id={}",
                    session_id, turn_id
                );
                Err(error)
            }
            DialogSteeringAction::Buffer { injection, outcome } => {
                self.round_injection_buffer.push(&session_id, injection);
                let DialogSteerOutcome::Buffered { steering_id, .. } = &outcome;
                info!(
                    "Steering message buffered: session_id={}, turn_id={}, steering_id={}, pending={}",
                    session_id,
                    turn_id,
                    steering_id,
                    self.round_injection_buffer.pending_count(&session_id)
                );

                Ok(outcome)
            }
        }
    }

    /// Resume auto-continuation toward an active thread goal (after pause / blocked / usage limit).
    pub async fn deliver_thread_goal_resumed(
        &self,
        session_id: String,
        agent_type: String,
        workspace_path: Option<String>,
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
        goal: ThreadGoal,
    ) -> Result<(), String> {
        let plan = build_thread_goal_resumed_delivery_plan(&goal);
        let state = self
            .session_manager
            .get_session(&session_id)
            .map(|s| s.state.clone());

        match resolve_background_delivery_action(BackgroundDeliveryFacts {
            session_state: Self::session_state_fact(state.as_ref()),
        }) {
            BackgroundDeliveryAction::InjectIntoRunningTurn => {
                self.round_injection_buffer.push(
                    &session_id,
                    resolve_background_delivery_injection(
                        BackgroundInjectionKind::ThreadGoalObjectiveUpdated,
                        Uuid::new_v4().to_string(),
                        plan.injection_prompt,
                        Some(plan.injection_display),
                        SystemTime::now(),
                    ),
                );
                Ok(())
            }
            BackgroundDeliveryAction::SubmitAgentSessionFollowUp { queue_priority } => {
                let prepended = thread_goal_delivery_messages(plan.prepended_reminders);
                self.submit_with_prepended_messages(
                    session_id,
                    plan.follow_up_user_input,
                    plan.follow_up_original_user_input,
                    None,
                    agent_type,
                    workspace_path,
                    remote_connection_id,
                    remote_ssh_host,
                    DialogSubmissionPolicy::new(DialogTriggerSource::AgentSession, queue_priority),
                    None,
                    Some(plan.user_message_metadata),
                    prepended,
                    None,
                )
                .await
                .map(|_| ())
            }
        }
    }

    /// Inject objective-updated steering into the running turn, or start a follow-up turn when idle.
    pub async fn deliver_thread_goal_objective_updated(
        &self,
        session_id: String,
        agent_type: String,
        workspace_path: Option<String>,
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
        goal: ThreadGoal,
    ) -> Result<(), String> {
        let plan = build_thread_goal_objective_updated_delivery_plan(&goal);
        let state = self
            .session_manager
            .get_session(&session_id)
            .map(|s| s.state.clone());

        match resolve_background_delivery_action(BackgroundDeliveryFacts {
            session_state: Self::session_state_fact(state.as_ref()),
        }) {
            BackgroundDeliveryAction::InjectIntoRunningTurn => {
                self.round_injection_buffer.push(
                    &session_id,
                    resolve_background_delivery_injection(
                        BackgroundInjectionKind::ThreadGoalObjectiveUpdated,
                        Uuid::new_v4().to_string(),
                        plan.injection_prompt,
                        Some(plan.injection_display),
                        SystemTime::now(),
                    ),
                );
                Ok(())
            }
            BackgroundDeliveryAction::SubmitAgentSessionFollowUp { queue_priority } => {
                let prepended = thread_goal_delivery_messages(plan.prepended_reminders);
                self.submit_with_prepended_messages(
                    session_id,
                    plan.follow_up_user_input,
                    plan.follow_up_original_user_input,
                    None,
                    agent_type,
                    workspace_path,
                    remote_connection_id,
                    remote_ssh_host,
                    DialogSubmissionPolicy::new(DialogTriggerSource::AgentSession, queue_priority),
                    None,
                    Some(plan.user_message_metadata),
                    prepended,
                    None,
                )
                .await
                .map(|_| ())
            }
        }
    }

    /// Deliver a completed background result back to the parent session.
    /// If the session is currently processing, inject the result into the
    /// running turn at the next model-round boundary. Otherwise, start a new
    /// turn immediately so the result is handled without waiting for an
    /// unrelated future message.
    #[allow(clippy::too_many_arguments)]
    pub async fn deliver_background_result(
        &self,
        session_id: String,
        agent_type: String,
        workspace_path: Option<String>,
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
        content: String,
        display_content: Option<String>,
        user_message_metadata: Option<serde_json::Value>,
    ) -> Result<(), String> {
        // COORD-16: resolve the session agent type before taking the session
        // operation lock. `resolve_session_agent_type` performs disk I/O when
        // the session is not loaded (storage-path resolution + restore), which
        // must not block concurrent submit/cancel on this session's lock.
        let session_agent_type = self
            .resolve_session_agent_type(
                &session_id,
                workspace_path.as_deref(),
                remote_connection_id.as_deref(),
                remote_ssh_host.as_deref(),
            )
            .await?;
        let _operation_guard = self.lock_session_operation(&session_id).await;
        if session_agent_type != agent_type {
            debug!(
                "Background result delivery replaced execution agent key with Session logical route: session_id={}, execution_agent_type={}, session_agent_type={}",
                session_id, agent_type, session_agent_type
            );
        }
        let display = display_content.unwrap_or_else(|| content.clone());
        let delivery = BackgroundResultDelivery {
            session_id: session_id.clone(),
            agent_type: session_agent_type,
            workspace_path,
            remote_connection_id,
            remote_ssh_host,
            display_content: Some(display),
            user_message_metadata,
        };
        let state = self
            .session_manager
            .get_session(&session_id)
            .map(|s| s.state.clone());

        match resolve_background_delivery_action(BackgroundDeliveryFacts {
            session_state: background_result_delivery_state_fact(
                &session_id,
                state.as_ref(),
                delivery.user_message_metadata.as_ref(),
            ),
        }) {
            BackgroundDeliveryAction::InjectIntoRunningTurn => {
                let Some(current_turn_id) = state.as_ref().and_then(|state| match state {
                    SessionState::Processing {
                        current_turn_id, ..
                    } => Some(current_turn_id.clone()),
                    _ => None,
                }) else {
                    return Err(format!(
                        "Background result resolved to injection without an active turn: session_id={session_id}"
                    ));
                };
                let injection_id = Uuid::new_v4().to_string();
                // B（注入约束）：运行中 turn 只注入 display 摘要（极简），全量
                // 结果内容不注入——全文由 P-03 落盘/子会话 turn 承载，
                // 避免与通知 turn 构成「通知 + 全文」双路。
                let injection = resolve_background_delivery_injection_for_turn(
                    BackgroundInjectionKind::BackgroundResult,
                    injection_id.clone(),
                    delivery.display_content.clone().unwrap_or_default(),
                    delivery.display_content.clone(),
                    SystemTime::now(),
                    current_turn_id,
                );
                self.round_injection_buffer.push(&session_id, injection);
                Ok(())
            }
            BackgroundDeliveryAction::SubmitAgentSessionFollowUp { queue_priority } => {
                // Type-erase the follow-up future so this delivery path no
                // longer embeds the full concrete future chain. The
                // review-reminder delivery route (COORD-04) leads back into
                // `start_turn` -> the hidden-subagent spawn, which would
                // otherwise form a recursive opaque future type that the
                // compiler cannot check for `Send`. The awaited future is
                // unchanged; only its static type is erased.
                let follow_up: std::pin::Pin<
                    Box<dyn std::future::Future<Output = Result<(), String>> + Send>,
                > = Box::pin(self.submit_background_result_follow_up_locked(
                    delivery,
                    queue_priority,
                ));
                follow_up.await
            }
        }
    }

    async fn submit_background_result_follow_up_locked(
        &self,
        delivery: BackgroundResultDelivery,
        queue_priority: DialogQueuePriority,
    ) -> Result<(), String> {
        let resolved_turn_id = Uuid::new_v4().to_string();
        // P-19：主会话通知只含极简元信息（session_id + 身份标识 + 已回复状态）。
        // 全量结果内容由 P-03 persist_background_acp_turn 落盘，
        // 经 SessionHistory(session_id) 检索；不进入主会话 message 历史。
        let user_input =
            background_result_follow_up_user_input(&delivery.session_id, &delivery.agent_type);
        let queued_turn = QueuedTurn {
            user_input,
            original_user_input: None,
            prepended_messages: Vec::new(),
            turn_id: Some(resolved_turn_id.clone()),
            agent_type: delivery.agent_type,
            workspace_path: delivery.workspace_path,
            remote_connection_id: delivery.remote_connection_id,
            remote_ssh_host: delivery.remote_ssh_host,
            policy: DialogSubmissionPolicy::new(DialogTriggerSource::AgentSession, queue_priority),
            reply_route: None,
            user_message_metadata: delivery.user_message_metadata,
            image_contexts: None,
            enqueued_at: SystemTime::now(),
            _settlement_registration: None,
            execution: QueuedTurnExecution::Standard,
        };
        let result = self
            .submit_queued_turn_locked(
                delivery.session_id.clone(),
                resolved_turn_id.clone(),
                queued_turn,
                false,
            )
            .await;
        if result.is_err() {
            if let Some(removed_turn) =
                remove_queued_turn_by_id(&self.queues, &delivery.session_id, &resolved_turn_id)
            {
                self.finish_removed_queued_turn(&delivery.session_id, removed_turn)
                    .await;
            }
        }
        result.map(|_| ()).map_err(|error| error.to_string())
    }

    pub async fn submit_init_agents_md(
        &self,
        session_id: String,
        workspace_path: Option<String>,
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
        policy: DialogSubmissionPolicy,
    ) -> Result<DialogSubmitOutcome, String> {
        let agent_type = self
            .resolve_session_agent_type(
                &session_id,
                workspace_path.as_deref(),
                remote_connection_id.as_deref(),
                remote_ssh_host.as_deref(),
            )
            .await?;
        let (user_input, prepended_messages) = build_init_agents_md_user_input()
            .await
            .map_err(|error| error.to_string())?;

        self.submit_with_prepended_messages(
            session_id,
            user_input.clone(),
            Some(user_input),
            None,
            agent_type,
            workspace_path,
            remote_connection_id,
            remote_ssh_host,
            policy,
            None,
            None,
            prepended_messages,
            None,
        )
        .await
    }

    fn session_state_fact(state: Option<&SessionState>) -> DialogSessionStateFact {
        match state {
            None => DialogSessionStateFact::Missing,
            Some(state) => state.dialog_state_fact(),
        }
    }

    /// Submit a user message for a session.
    ///
    /// - Session idle, queue empty → dispatched immediately.
    /// - Session idle, queue non-empty → enqueued then highest-priority queued message dispatched.
    /// - Session processing → queued up to the runtime-owned queue limit and dispatched after
    ///   the current turn completes.
    /// - Session error → queue cleared, dispatched immediately.
    ///
    /// Returns `Err(String)` if the queue is full or the coordinator returns an error.
    #[allow(clippy::too_many_arguments)]
    pub async fn submit(
        &self,
        session_id: String,
        user_input: String,
        original_user_input: Option<String>,
        turn_id: Option<String>,
        agent_type: String,
        workspace_path: Option<String>,
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
        policy: DialogSubmissionPolicy,
        reply_route: Option<AgentSessionReplyRoute>,
        user_message_metadata: Option<serde_json::Value>,
        image_contexts: Option<Vec<ImageContextData>>,
    ) -> Result<DialogSubmitOutcome, String> {
        self.submit_with_prepended_messages(
            session_id,
            user_input,
            original_user_input,
            turn_id,
            agent_type,
            workspace_path,
            remote_connection_id,
            remote_ssh_host,
            policy,
            reply_route,
            user_message_metadata,
            Vec::new(),
            image_contexts,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn submit_with_prepended_messages(
        &self,
        session_id: String,
        user_input: String,
        original_user_input: Option<String>,
        turn_id: Option<String>,
        agent_type: String,
        workspace_path: Option<String>,
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
        policy: DialogSubmissionPolicy,
        reply_route: Option<AgentSessionReplyRoute>,
        user_message_metadata: Option<serde_json::Value>,
        prepended_messages: Vec<Message>,
        image_contexts: Option<Vec<ImageContextData>>,
    ) -> Result<DialogSubmitOutcome, String> {
        let resolved_turn_id = turn_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let queued_turn = QueuedTurn {
            user_input,
            original_user_input,
            prepended_messages,
            turn_id: Some(resolved_turn_id.clone()),
            agent_type,
            workspace_path,
            remote_connection_id,
            remote_ssh_host,
            policy,
            reply_route,
            user_message_metadata,
            image_contexts,
            enqueued_at: SystemTime::now(),
            _settlement_registration: None,
            execution: QueuedTurnExecution::Standard,
        };
        self.submit_queued_turn(session_id, resolved_turn_id, queued_turn, false)
            .await
            .map_err(|error| error.to_string())
    }

    pub(crate) async fn submit_hidden_subagent(
        &self,
        mut request: HiddenSubagentExecutionRequest,
        timeout_seconds: Option<u64>,
    ) -> Result<HiddenSubagentSubmitResult, String> {
        let session_id = request
            .target_session_id()
            .ok_or_else(|| {
                "prepared hidden subagent request is missing target_session_id".to_string()
            })?
            .to_string();
        let resolved_turn_id = request.ensure_dialog_turn_id();
        let agent_type = request.logical_agent_type().to_string();
        let user_input = request.user_input_text().to_string();
        let session = self
            .session_manager
            .get_session(&session_id)
            .ok_or_else(|| {
                format!(
                    "Subagent session not found before scheduler submit: {}",
                    session_id
                )
            })?;
        let (result_tx, result_rx) = oneshot::channel();
        let result_tx = SharedSubagentResultSender::new(result_tx);
        let cancellation = HiddenSubagentQueueCancellation::default();
        let queued_turn = QueuedTurn {
            user_input: user_input.clone(),
            original_user_input: Some(user_input),
            prepended_messages: Vec::new(),
            turn_id: Some(resolved_turn_id.clone()),
            agent_type,
            workspace_path: session.config.workspace_path.clone(),
            remote_connection_id: session.config.remote_connection_id.clone(),
            remote_ssh_host: session.config.remote_ssh_host.clone(),
            policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
            reply_route: None,
            user_message_metadata: None,
            image_contexts: None,
            enqueued_at: SystemTime::now(),
            _settlement_registration: None,
            execution: QueuedTurnExecution::HiddenSubagent(HiddenSubagentQueuedExecution {
                request,
                timeout_seconds,
                result_tx: result_tx.clone(),
                cancellation: cancellation.clone(),
            }),
        };

        self.submit_queued_turn(
            session_id.clone(),
            resolved_turn_id.clone(),
            queued_turn,
            false,
        )
        .await
        .map_err(|error| error.to_string())?;
        Ok(HiddenSubagentSubmitResult {
            receiver: result_rx,
            cancel_handle: HiddenSubagentQueueCancelHandle {
                session_id,
                turn_id: resolved_turn_id,
                cancellation,
                result_tx,
            },
        })
    }

    pub(crate) async fn request_hidden_subagent_cancellation(
        &self,
        handle: &HiddenSubagentQueueCancelHandle,
    ) {
        handle.cancellation.cancel();
        if let Err(error) = self
            .cancel_queued_or_active_turn(&handle.session_id, &handle.turn_id)
            .await
        {
            debug!(
                "Hidden subagent turn cancellation request did not hit an active turn: session_id={}, turn_id={}, error={}",
                handle.session_id, handle.turn_id, error
            );
            handle.result_tx.send(Err(BitFunError::Cancelled(
                "Subagent task has been cancelled".to_string(),
            )));
        }
    }

    async fn resolve_session_agent_type(
        &self,
        session_id: &str,
        workspace_path: Option<&str>,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
    ) -> Result<String, String> {
        let session = match self.session_manager.get_session(session_id) {
            Some(session) => session,
            None => {
                let workspace_path = workspace_path.ok_or_else(|| {
                    format!(
                        "workspace_path is required when restoring session: {}",
                        session_id
                    )
                })?;
                let restore_path = Self::resolve_session_restore_path(
                    workspace_path,
                    remote_connection_id,
                    remote_ssh_host,
                )
                .await
                .map_err(|error| error.to_string())?;
                // B1（幽灵会话删除修复）：internal restore 替代非 internal，使 evict/
                // 重启后的 Subagent 职位会话仍可被 resolve（SessionMessage/Task/后台
                // 结果投递路径）——hidden 只影响用户列表展示，不阻断内部会话解析。
                self.coordinator
                    .restore_internal_session_from_storage_path(&restore_path, session_id)
                    .await
                    .map_err(|error| error.to_string())?
            }
        };
        let agent_type = session.agent_type.trim();
        if agent_type.is_empty() {
            Ok("agentic".to_string())
        } else {
            Ok(agent_type.to_string())
        }
    }

    async fn resolve_session_restore_path(
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
    ) -> Result<PathBuf, SchedulerSubmitError> {
        let request = SessionStoragePathRequest {
            workspace_path: PathBuf::from(workspace_path),
            remote_connection_id: remote_connection_id.map(ToOwned::to_owned),
            remote_ssh_host: remote_ssh_host.map(ToOwned::to_owned),
        };

        CoreSessionStorePort::default()
            .resolve_session_storage_path(request)
            .await
            .map(|resolution| resolution.effective_storage_path)
            .map_err(SchedulerSubmitError::Port)
    }

    async fn submit_queued_turn(
        &self,
        session_id: String,
        resolved_turn_id: String,
        queued_turn: QueuedTurn,
        reject_if_busy: bool,
    ) -> Result<DialogSubmitOutcome, SchedulerSubmitError> {
        let trigger_source = queued_turn.policy.trigger_source;
        let wakeup_session_id = session_id.clone();
        let _operation_guard = self.lock_session_operation(&session_id).await;
        let outcome = self
            .submit_queued_turn_locked(session_id, resolved_turn_id, queued_turn, reject_if_busy)
            .await;
        // A successful user-initiated submission resets the goal idle-wakeup
        // safety net: a goal continuation is only considered again after the
        // session has been idle for a full GOAL_IDLE_WAKEUP_DELAY_MS window.
        if outcome.is_ok() && is_user_submission_source(trigger_source) {
            self.schedule_goal_idle_wakeup(&wakeup_session_id);
        }
        // Note: the immediate workspace-quiescent condition is evaluated at the
        // outcome handler instead of here — a just-submitted session is busy,
        // so the workspace cannot be quiescent at this point, and spawning a
        // quiescence check from here would create a cyclic Send obligation
        // (the wakeup submit path routes back through this method).
        outcome
    }

    async fn submit_queued_turn_locked(
        &self,
        session_id: String,
        resolved_turn_id: String,
        mut queued_turn: QueuedTurn,
        reject_if_busy: bool,
    ) -> Result<DialogSubmitOutcome, SchedulerSubmitError> {
        // Background-notification coalescing (主人裁决：后台通知 = 必要功能，
        // 修的是"通知风暴"——同一会话重复通知只应通知一次)。The follow-up
        // text is a fixed template derived only from (session_id, agent_type),
        // so an identical notification already running or queued for the same
        // session means the same subagent completion is being reported twice;
        // the duplicate submit is skipped instead of spawning a second model
        // request. Distinct child sessions produce distinct texts (each carries
        // its own session id) and are all delivered — notifications are kept,
        // only the storm is removed. The check happens before any prompt is
        // built, so the kept notification's prompt text, position, and prefix
        // are byte-identical to the pre-fix behavior.
        if let Some(notice) = Self::background_notice_for_queued_turn(&queued_turn) {
            let already_active = self
                .active_turns
                .active_turn_user_input(&session_id)
                .is_some_and(|user_input| user_input == notice);
            let already_queued = self.queues.any_matching(&session_id, |turn| {
                turn.policy.trigger_source == DialogTriggerSource::AgentSession
                    && turn.user_input == notice
            });
            if already_active || already_queued {
                debug!(
                    "Coalesced duplicate background notification: an identical follow-up is already active/queued: session_id={}, notice_len={}",
                    session_id,
                    notice.len()
                );
                return Ok(DialogSubmitOutcome::Queued {
                    session_id,
                    turn_id: resolved_turn_id,
                });
            }
        }
        if let Some(session) = self.session_manager.get_session(&session_id) {
            queued_turn.workspace_path = session_storage_workspace_locator(
                queued_turn.workspace_path.as_deref(),
                session.config.workspace_path.as_deref(),
                session.config.project_workspace_path.as_deref(),
            );
        }
        if let Some(workspace_path) = queued_turn.workspace_path.as_deref() {
            let requested_storage_path = Self::resolve_session_restore_path(
                workspace_path,
                queued_turn.remote_connection_id.as_deref(),
                queued_turn.remote_ssh_host.as_deref(),
            )
            .await?;
            self.session_manager
                .validate_session_storage_path_binding(&session_id, &requested_storage_path)
                .map_err(SchedulerSubmitError::Core)?;
        }
        let state = self
            .session_manager
            .get_session(&session_id)
            .map(|s| s.state.clone());
        let state_fact = if self.active_turns.contains(&session_id) {
            DialogSessionStateFact::Processing
        } else {
            Self::session_state_fact(state.as_ref())
        };

        let queue_has_items = self.queues.has_items(&session_id);
        if matches!(
            &queued_turn.execution,
            QueuedTurnExecution::FreshExternalSubagent(_)
        ) && (!matches!(&state_fact, DialogSessionStateFact::Idle) || queue_has_items)
        {
            return Err(SchedulerSubmitError::Core(BitFunError::Validation(
                "External subagent delegation requires an idle session with an empty queue"
                    .to_string(),
            )));
        }
        let action = resolve_dialog_submit_queue_action(DialogSubmitQueueFacts {
            session_state: state_fact,
            queue_has_items,
            policy: queued_turn.policy,
        });

        if reject_if_busy
            && matches!(
                action,
                DialogSubmitQueueAction::EnqueueThenStartNext
                    | DialogSubmitQueueAction::EnqueueForActiveTurn
            )
        {
            return Err(SchedulerSubmitError::Message(
                "Session state does not allow starting new dialog: Processing".to_string(),
            ));
        }

        // OpenCode-compatible semantics: accepting a new prompt while history
        // is staged permanently discards the hidden suffix before the Turn starts.
        self.coordinator
            .commit_session_revert_before_submission(&session_id)
            .await
            .map_err(SchedulerSubmitError::Core)?;

        match action {
            DialogSubmitQueueAction::StartImmediately => {
                let tid = self.start_turn(&session_id, &queued_turn).await?;
                queued_turn.accept_settlement();
                self.record_last_submitted_agent_type(&session_id, &queued_turn.agent_type)
                    .await;
                Ok(DialogSubmitOutcome::Started {
                    session_id,
                    turn_id: tid,
                })
            }

            DialogSubmitQueueAction::ClearQueueAndStartImmediately => {
                let _ = self.clear_queue(&session_id).await;
                let tid = self.start_turn(&session_id, &queued_turn).await?;
                queued_turn.accept_settlement();
                self.record_last_submitted_agent_type(&session_id, &queued_turn.agent_type)
                    .await;
                Ok(DialogSubmitOutcome::Started {
                    session_id,
                    turn_id: tid,
                })
            }

            DialogSubmitQueueAction::EnqueueThenStartNext => {
                self.enqueue(&session_id, queued_turn.clone())?;
                queued_turn.accept_settlement();
                self.record_last_submitted_agent_type(&session_id, &queued_turn.agent_type)
                    .await;
                let started_tid = self.try_start_next_queued_locked(&session_id).await?;
                let outcome =
                    queued_submission_outcome(session_id.clone(), resolved_turn_id, started_tid);
                Ok(outcome)
            }

            DialogSubmitQueueAction::EnqueueForActiveTurn => {
                let accepted_agent_type = queued_turn.agent_type.clone();
                self.enqueue(&session_id, queued_turn.clone())?;
                queued_turn.accept_settlement();
                self.record_last_submitted_agent_type(&session_id, &accepted_agent_type)
                    .await;
                Ok(DialogSubmitOutcome::Queued {
                    session_id,
                    turn_id: resolved_turn_id,
                })
            }
        }
    }

    async fn record_last_submitted_agent_type(&self, session_id: &str, agent_type: &str) {
        if let Err(error) = self
            .coordinator
            .update_last_submitted_agent_type(session_id, agent_type)
            .await
        {
            warn!(
                "Failed to record last submitted agent type: session_id={}, agent_type={}, error={}",
                session_id, agent_type, error
            );
        }
    }

    /// Number of messages currently queued for a session.
    pub fn queue_depth(&self, session_id: &str) -> usize {
        self.queues.depth(session_id)
    }

    /// Whether a session has a running or queued turn. This is intentionally a
    /// narrow observation API for features that need an idle target without
    /// depending on scheduler internals.
    pub fn is_session_busy_or_queued(&self, session_id: &str) -> bool {
        self.active_turns.contains(session_id)
            || self.queues.has_items(session_id)
            || self
                .session_manager
                .get_session(session_id)
                .is_some_and(|session| matches!(session.state, SessionState::Processing { .. }))
    }

    /// Schedule a goal idle-wakeup check `GOAL_IDLE_WAKEUP_DELAY_MS` from now.
    ///
    /// Safety-net behavior only: when the session stays idle and an active
    /// thread goal exists, the wakeup reuses the continuation state machine to
    /// submit a "wake the commander" turn. A newer user submission bumps the
    /// session generation and invalidates older wakeup tasks; the
    /// auto-continuation budget additionally caps how often a wakeup can fire.
    /// Safe to call repeatedly: each call re-arms the timer so only the newest
    /// wakeup task fires.
    pub fn schedule_goal_idle_wakeup(&self, session_id: &str) {
        let Some(weak) = self.goal_idle_wakeup_self.get().cloned() else {
            return;
        };
        let Some(scheduler) = weak.upgrade() else {
            return;
        };
        let generation = {
            let mut entry = self
                .goal_idle_wakeup_generations
                .entry(session_id.to_string())
                .or_insert(0u64);
            *entry += 1;
            *entry
        };
        let wakeup_session_id = session_id.to_string();
        tokio::spawn(async move {
            // 阈值参数配置化：ai.thresholds.goal.idle_wakeup_delay_ms
            let delay_ms = configured_goal_idle_wakeup_delay_ms().await;
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            scheduler
                .goal_idle_wakeup_check(&wakeup_session_id, generation)
                .await;
        });
    }

    /// Idle-wakeup check, called by a spawned task after the delay window.
    async fn goal_idle_wakeup_check(&self, session_id: &str, generation: u64) {
        if self
            .goal_idle_wakeup_generations
            .get(session_id)
            .map(|value| *value)
            != Some(generation)
        {
            // A newer user submission superseded this wakeup task.
            debug!(
                "Goal idle wakeup skipped (superseded by a newer schedule): session_id={}, generation={}",
                session_id, generation
            );
            return;
        }
        let Some(session) = self.session_manager.get_session(session_id) else {
            debug!(
                "Goal idle wakeup skipped (session no longer loaded): session_id={}",
                session_id
            );
            return;
        };
        let Some(workspace_path) = session.config.workspace_path.as_deref().map(Path::new) else {
            debug!(
                "Goal idle wakeup skipped (session has no workspace path): session_id={}",
                session_id
            );
            return;
        };
        // Cheap guard before the workspace-wide silence scan: a session without
        // an active thread goal cannot produce a wakeup plan, so stop the chain
        // here instead of listing the whole workspace.
        let has_active_goal = match self
            .coordinator
            .get_thread_goal(session_id, workspace_path)
            .await
        {
            Ok(Some(goal)) => goal.is_active(),
            Ok(None) => false,
            Err(error) => {
                warn!(
                    "Goal idle wakeup goal lookup failed: session_id={}, error={}",
                    session_id, error
                );
                return;
            }
        };
        if !has_active_goal {
            debug!(
                "Goal idle wakeup skipped (no active thread goal): session_id={}, generation={}",
                session_id, generation
            );
            return;
        }
        // Dual trigger condition: the safety net fires when EITHER the primary
        // (tree-root / main) conversation has been silent for a full idle
        // window, OR every conversation in the workspace is quiescent (no
        // running or queued turn anywhere). A still-active node keeps the
        // wakeup pending and re-arms the timer.
        let summaries = match self
            .session_manager
            .list_sessions_with_options(workspace_path, true)
            .await
        {
            Ok(summaries) => summaries,
            Err(error) => {
                warn!(
                    "Goal idle wakeup workspace session listing failed: session_id={}, error={}",
                    session_id, error
                );
                return;
            }
        };
        let now = SystemTime::now();
        let idle_delay = Duration::from_millis(GOAL_IDLE_WAKEUP_DELAY_MS);
        let is_busy = |id: &str| self.is_session_busy_or_queued(id);
        let last_activity = |id: &str| {
            self.session_manager
                .get_session(id)
                .map(|session| session.last_activity_at)
                .or_else(|| {
                    summaries
                        .iter()
                        .find(|summary| summary.session_id == id)
                        .map(|summary| summary.last_activity_at)
                })
        };
        let primary_id = session_tree_root_id(&summaries, session_id);
        let all_ids: Vec<String> = summaries
            .iter()
            .map(|summary| summary.session_id.clone())
            .collect();
        let (primary_silent, all_sessions_silent) = goal_idle_wakeup_conditions_met(
            &[primary_id],
            &all_ids,
            now,
            idle_delay,
            is_busy,
            last_activity,
        );
        if !(primary_silent || all_sessions_silent) {
            debug!(
                "Goal idle wakeup deferred; neither trigger condition met: session_id={}, generation={}, primary_silent={}, all_sessions_silent={}",
                session_id, generation, primary_silent, all_sessions_silent
            );
            self.schedule_goal_idle_wakeup(session_id);
            return;
        }
        let _ = self
            .trigger_goal_idle_wakeup(session_id, &session, "idle_timer")
            .await;
    }

    /// Batch-2 goal switch: whether the Warden turn hooks apply for a session.
    ///
    /// Reuses the `get_thread_goal` + `is_active()` pattern of the goal
    /// idle-wakeup check: only sessions with an active thread goal are under
    /// Warden enforcement, so failures of goal-less or non-active-goal
    /// sessions never accumulate consecutive-failure counts.
    ///
    /// WARDEN-05: subagent / ephemeral sessions are **exempt** outright —
    /// thread goals are only attachable to main sessions, so a subagent can
    /// never hold an active goal and must not be pushed into fail-open
    /// enforcement just because its session lacks a workspace or a persisted
    /// goal. This is a hard exemption, not a fail-open: only main
    /// (`SessionKind::Standard`) sessions fall through to the goal lookup. A
    /// goal *lookup failure* on a main session still keeps enforcement
    /// enabled (fail-open) so a transient store error cannot silently disable
    /// discipline.
    ///
    /// COORD-02: this entry point is a short-TTL cache over the disk-backed
    /// check below. Outcome handling can query it once per finished turn; a
    /// few seconds of staleness is acceptable for enforcement gating and
    /// keeps the outcome path off the storage layer.
    async fn session_has_active_goal(&self, session_id: &str) -> bool {
        if let Some(cached) = self.goal_active_cache.get(session_id) {
            if cached.value().0.elapsed() < GOAL_ACTIVE_CACHE_TTL {
                return cached.value().1;
            }
        }
        let active = self.session_has_active_goal_uncached(session_id).await;
        self.goal_active_cache
            .insert(session_id.to_string(), (Instant::now(), active));
        active
    }

    async fn session_has_active_goal_uncached(&self, session_id: &str) -> bool {
        let Some(session) = self.session_manager.get_session(session_id) else {
            return false;
        };
        if !matches!(session.kind, SessionKind::Standard) {
            return false;
        }
        let Some(workspace_path) = session.config.workspace_path.as_deref().map(Path::new) else {
            return true;
        };
        match self
            .coordinator
            .get_thread_goal(session_id, workspace_path)
            .await
        {
            Ok(goal) => warden_enforcement_for_goal(goal.as_ref()),
            Err(error) => {
                warn!(
                    "Warden goal gate lookup failed; keeping Warden turn hooks enabled: session_id={}, error={}",
                    session_id, error
                );
                true
            }
        }
    }

    /// Build and submit a goal wakeup turn for `session_id` (the continuation
    /// state machine in `prepare_goal_idle_wakeup`, then a normal submit).
    /// Returns true when a wakeup turn was submitted. Shared by the idle-wakeup
    /// timer check and the immediate workspace-quiescent trigger. The
    /// auto-continuation budget (`prepare_goal_idle_wakeup`) caps how often a
    /// wakeup can fire, so this cannot loop indefinitely.
    async fn trigger_goal_idle_wakeup(
        &self,
        session_id: &str,
        session: &Session,
        trigger: &str,
    ) -> bool {
        let plan = match self.coordinator.prepare_goal_idle_wakeup(session_id).await {
            Ok(plan) => plan,
            Err(error) => {
                warn!(
                    "Goal idle wakeup plan failed: session_id={}, error={}",
                    session_id, error
                );
                return false;
            }
        };
        let Some(plan) = plan else {
            // No continuation plan: goal missing, completed, paused, or the
            // auto-continuation budget is exhausted. Stop the wakeup chain.
            debug!(
                "Goal idle wakeup produced no continuation plan; stopping wakeup chain: session_id={}",
                session_id
            );
            return false;
        };
        let prepended: Vec<Message> = plan
            .prepended_reminders
            .iter()
            .map(|text| Message::internal_reminder(InternalReminderKind::GoalContinuation, text))
            .collect();
        let agent_type = session.agent_type.trim();
        let agent_type = if agent_type.is_empty() {
            "agentic".to_string()
        } else {
            agent_type.to_string()
        };
        match self
            .submit_with_prepended_messages(
                session_id.to_string(),
                format!(
                    "The active thread goal has been idle for {} minutes. Wake up the commander and continue the remaining goal work.",
                    GOAL_IDLE_WAKEUP_DELAY_MS / 60_000
                ),
                Some(plan.display_message.clone()),
                None,
                agent_type,
                session.config.workspace_path.clone(),
                session.config.remote_connection_id.clone(),
                session.config.remote_ssh_host.clone(),
                DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                None,
                Some(plan.user_message_metadata.clone()),
                prepended,
                None,
            )
            .await
        {
            Ok(_) => {
                info!(
                    "Goal idle wakeup turn submitted: session_id={}, trigger={}",
                    session_id, trigger
                );
                // The wakeup turn itself keeps the goal alive; schedule the
                // next safety-net check so the goal is still picked up if the
                // commander does not respond.
                self.schedule_goal_idle_wakeup(session_id);
                true
            }
            Err(error) => {
                warn!(
                    "Goal idle wakeup submit failed: session_id={}, error={}",
                    session_id, error
                );
                false
            }
        }
    }

    /// Immediate goal-wakeup trigger for the workspace-quiescent condition:
    /// after a scheduling event (a top-level turn finished or a submission was
    /// accepted) a session holding an active thread goal is woken up as soon
    /// as every conversation in its workspace has no running or queued turn.
    /// Unlike the primary (10-minute) condition this fires immediately; the
    /// auto-continuation budget caps how often it can fire.
    async fn maybe_trigger_goal_wakeup_when_workspace_quiescent(&self, session_id: &str) {
        // If the goal session itself is still busy or queued, the workspace
        // cannot be quiescent; skip the (relatively expensive) workspace scan.
        if self.is_session_busy_or_queued(session_id) {
            return;
        }
        let Some(session) = self.session_manager.get_session(session_id) else {
            return;
        };
        let Some(workspace_path) = session.config.workspace_path.as_deref().map(Path::new) else {
            return;
        };
        let has_active_goal = match self
            .coordinator
            .get_thread_goal(session_id, workspace_path)
            .await
        {
            Ok(Some(goal)) => goal.is_active(),
            _ => return,
        };
        if !has_active_goal {
            return;
        }
        let Ok(summaries) = self
            .session_manager
            .list_sessions_with_options(workspace_path, true)
            .await
        else {
            return;
        };
        let all_ids: Vec<String> = summaries
            .iter()
            .map(|summary| summary.session_id.clone())
            .collect();
        if !all_sessions_quiescent(&all_ids, |id| self.is_session_busy_or_queued(id)) {
            return;
        }
        debug!(
            "Goal wakeup immediate trigger: every conversation in workspace is silent: session_id={}",
            session_id
        );
        let _ = self
            .trigger_goal_idle_wakeup(session_id, &session, "workspace_quiescent")
            .await;
    }

    /// Best-effort recovery for goal idle-wakeup timers lost on process
    /// restart.
    ///
    /// The wakeup chain is purely in-memory (spawned timers), so a restart
    /// silently orphans every pending goal. This scans the persisted workspace
    /// sessions for active thread goals and re-arms the safety net. Hosts
    /// register the global workspace service shortly after the scheduler is
    /// constructed (see desktop/server bootstraps), so this polls briefly for
    /// it before giving up quietly.
    async fn rearm_goal_idle_wakeups_after_startup(&self) {
        const REARM_MAX_ATTEMPTS: u32 = 30;
        const REARM_POLL_INTERVAL: Duration = Duration::from_millis(1_000);
        let workspace_service = {
            let mut attempts = 0u32;
            loop {
                if let Some(service) = get_global_workspace_service() {
                    break service;
                }
                attempts += 1;
                if attempts >= REARM_MAX_ATTEMPTS {
                    debug!(
                        "Goal idle-wakeup rearm skipped: global workspace service unavailable"
                    );
                    return;
                }
                tokio::time::sleep(REARM_POLL_INTERVAL).await;
            }
        };
        for workspace in workspace_service.list_workspace_infos().await {
            let workspace_path = workspace.root_path;
            let goal_session_ids = match self
                .active_goal_session_ids(&workspace_path)
                .await
            {
                Ok(ids) => ids,
                Err(error) => {
                    debug!(
                        "Goal idle-wakeup rearm workspace scan failed: workspace={}, error={}",
                        workspace_path.display(),
                        error
                    );
                    continue;
                }
            };
            for session_id in goal_session_ids {
                debug!(
                    "Rearming goal idle-wakeup after restart: session_id={}",
                    session_id
                );
                self.schedule_goal_idle_wakeup(&session_id);
            }
        }
    }

    /// Enumerate sessions in `workspace` that currently hold an active thread
    /// goal. Best-effort: requires persistence to observe goals on sessions
    /// that are not loaded in memory; individual metadata read failures are
    /// skipped rather than aborting the scan.
    async fn active_goal_session_ids(&self, workspace_path: &Path) -> BitFunResult<Vec<String>> {
        let summaries = self
            .session_manager
            .list_sessions_with_options(workspace_path, true)
            .await?;
        let mut goal_session_ids = Vec::new();
        for summary in summaries {
            // Only main sessions can carry thread goals; skip subagent and
            // ephemeral children to keep the startup scan cheap.
            if matches!(
                summary.kind,
                SessionKind::Subagent | SessionKind::EphemeralChild | SessionKind::EphemeralSubagent
            ) {
                continue;
            }
            let Ok(Some(metadata)) = self
                .session_manager
                .load_session_metadata(workspace_path, &summary.session_id)
                .await
            else {
                continue;
            };
            let Some(goal) = thread_goal_from_custom_metadata(metadata.custom_metadata.as_ref())
            else {
                continue;
            };
            if goal.is_active() {
                goal_session_ids.push(summary.session_id);
            }
        }
        Ok(goal_session_ids)
    }

    async fn finish_removed_queued_turn(&self, session_id: &str, removed_turn: QueuedTurn) {
        match removed_turn.execution {
            QueuedTurnExecution::Standard | QueuedTurnExecution::FreshExternalSubagent(_) => {
                if let Some(turn_id) = removed_turn.turn_id {
                    self.coordinator
                        .emit_event(AgenticEvent::DialogTurnCancelled {
                            session_id: session_id.to_string(),
                            turn_id,
                        })
                        .await;
                } else {
                    warn!("Removed queued dialog turn without a turn id: session_id={session_id}");
                }
            }
            QueuedTurnExecution::HiddenSubagent(execution) => {
                execution.cancellation.cancel();
                self.coordinator
                    .cleanup_prepared_hidden_subagent_session_if_unsubmitted(&execution.request)
                    .await;
                execution.result_tx.send(Err(BitFunError::Cancelled(
                    "Subagent task has been cancelled".to_string(),
                )));
            }
        }
    }

    /// Cancel one queued or active turn without allowing it to cross the
    /// scheduler's dequeue-to-coordinator transition.
    ///
    /// Returns `true` when the turn was removed before it started. `false`
    /// means cancellation was delivered to the active coordinator execution.
    pub async fn cancel_queued_or_active_turn(
        &self,
        session_id: &str,
        turn_id: &str,
    ) -> Result<bool, String> {
        let _operation_guard = self.lock_session_operation(session_id).await;
        let removed_turn = remove_queued_turn_by_id(&self.queues, session_id, turn_id);
        if let Some(removed_turn) = removed_turn {
            self.finish_removed_queued_turn(session_id, removed_turn)
                .await;
            debug!(
                "Removed queued turn after targeted cancellation: session_id={}, turn_id={}",
                session_id, turn_id
            );
            return Ok(true);
        }

        if !self.active_turns.matches_turn(session_id, turn_id) {
            debug!(
                "Ignoring cancellation for a turn that is not active in the requested session: session_id={}, turn_id={}",
                session_id, turn_id
            );
            return Ok(false);
        }

        self.coordinator
            .cancel_dialog_turn(session_id, turn_id)
            .await?;
        Ok(false)
    }

    /// Cancel the target session's active turn on behalf of a requester session.
    ///
    /// If the requester is the same source session that originally sent the
    /// in-flight SessionMessage request, the scheduler suppresses the automatic
    /// cancelled-reply bounce-back for that specific turn.
    pub async fn cancel_active_turn_for_session_from_requester(
        &self,
        target_session_id: &str,
        requester_session_id: &str,
        wait_timeout: Duration,
    ) -> crate::util::errors::BitFunResult<Option<String>> {
        let _operation_guard = self.lock_session_operation(target_session_id).await;
        let suppression_key = self
            .active_turns
            .suppression_key_for_requester(target_session_id, requester_session_id);

        if let Some((session_id, turn_id)) = suppression_key.as_ref() {
            debug!(
                "Suppressing cancelled auto-reply for agent-session turn: target_session_id={}, turn_id={}, requester_session_id={}",
                session_id, turn_id, requester_session_id
            );
            self.suppressed_cancelled_replies.mark(session_id, turn_id);
        }

        abort_thread_goal_continuation_for_session(target_session_id);

        match self
            .coordinator
            .cancel_active_turn_for_session(target_session_id, wait_timeout)
            .await
        {
            Ok(cancelled_turn_id) => {
                if cancelled_turn_id.is_none() {
                    if let Some((session_id, turn_id)) = suppression_key {
                        self.suppressed_cancelled_replies
                            .clear(&session_id, &turn_id);
                    }
                }
                Ok(cancelled_turn_id)
            }
            Err(error) => {
                if let Some((session_id, turn_id)) = suppression_key {
                    self.suppressed_cancelled_replies
                        .clear(&session_id, &turn_id);
                }
                Err(error)
            }
        }
    }

    /// Cancel the current active turn without allowing submit or outcome
    /// dispatch to cross the cancellation boundary for this session.
    pub async fn cancel_active_turn_for_session(
        &self,
        session_id: &str,
        wait_timeout: Duration,
    ) -> BitFunResult<Option<String>> {
        self.cancel_active_turn_for_session_with_descendant_policy(session_id, wait_timeout, true)
            .await
    }

    pub(crate) async fn inspect_loaded_lineage_session(
        &self,
        storage_path: &Path,
        request: SessionTranscriptRequest,
        required_settled_turn_ids: &[String],
    ) -> PortResult<Option<AgentSessionLineageInspection>> {
        let _operation_guard = self.lock_session_operation(&request.session_id).await;
        self.coordinator
            .inspect_loaded_lineage_session_in_storage(
                storage_path,
                request,
                required_settled_turn_ids,
            )
            .await
    }

    pub(crate) async fn cancel_lineage_session_in_storage(
        &self,
        storage_path: &Path,
        session_id: &str,
        expected_active_turn_id: Option<&str>,
        wait_timeout: Duration,
    ) -> BitFunResult<Option<String>> {
        let deadline = Instant::now() + wait_timeout;
        let _operation_guard = tokio::time::timeout(
            wait_timeout,
            self.lock_session_operation(session_id),
        )
        .await
        .map_err(|_| {
            BitFunError::Timeout(format!(
                "Timed out acquiring the Session operation lock before lineage cancellation: session_id={session_id}"
            ))
        })?;
        self.coordinator
            .cancel_loaded_lineage_session_in_storage(
                storage_path,
                session_id,
                expected_active_turn_id,
                deadline.saturating_duration_since(Instant::now()),
            )
            .await
    }

    async fn cancel_active_turn_for_session_with_descendant_policy(
        &self,
        session_id: &str,
        wait_timeout: Duration,
        cancel_descendants: bool,
    ) -> BitFunResult<Option<String>> {
        let _operation_guard = self.lock_session_operation(session_id).await;
        abort_thread_goal_continuation_for_session(session_id);
        self.coordinator
            .cancel_active_turn_for_session_with_descendant_policy(
                session_id,
                wait_timeout,
                cancel_descendants,
            )
            .await
    }

    /// Quiesce one session for destructive maintenance. Queued turns receive an explicit
    /// cancelled lifecycle event before active execution is cancelled and
    /// drained, so no accepted turn disappears silently.
    pub(crate) async fn begin_session_maintenance(
        &self,
        session_id: &str,
        requested_storage_path: &std::path::Path,
        wait_timeout: Duration,
    ) -> BitFunResult<SessionMaintenancePermit> {
        bitfun_core_types::validate_session_id(session_id).map_err(BitFunError::Validation)?;
        let operation_guard = self.lock_session_operation(session_id).await;
        self.session_manager
            .validate_session_storage_path_binding(session_id, requested_storage_path)?;
        let mut retired_turn_ids = if self.queue_depth(session_id) > 0 {
            self.clear_queue(session_id).await
        } else {
            Vec::new()
        };
        abort_thread_goal_continuation_for_session(session_id);
        let deadline = Instant::now() + wait_timeout;
        let cancelled_before_parent = self
            .coordinator
            .cancel_background_subagents_for_parent_session(session_id)
            .await?;
        let mut subagent_session_ids = self
            .maintenance_background_sessions
            .get(session_id)
            .map(|sessions| sessions.clone())
            .unwrap_or_default();
        subagent_session_ids.extend(cancelled_before_parent);
        if !subagent_session_ids.is_empty() {
            self.maintenance_background_sessions
                .insert(session_id.to_string(), subagent_session_ids.clone());
        }
        let cancelled_turn_id = self
            .coordinator
            .cancel_active_turn_for_session(
                session_id,
                deadline.saturating_duration_since(Instant::now()),
            )
            .await?;
        let cancelled_during_parent = self
            .coordinator
            .cancel_background_subagents_for_parent_session(session_id)
            .await?;
        subagent_session_ids.extend(cancelled_during_parent);
        if !subagent_session_ids.is_empty() {
            self.maintenance_background_sessions
                .insert(session_id.to_string(), subagent_session_ids.clone());
        }
        for subagent_session_id in &subagent_session_ids {
            self.coordinator
                .ensure_session_execution_drained(
                    subagent_session_id,
                    deadline.saturating_duration_since(Instant::now()),
                )
                .await?;
        }
        self.coordinator
            .ensure_session_execution_drained(
                session_id,
                deadline.saturating_duration_since(Instant::now()),
            )
            .await?;
        self.maintenance_background_sessions.remove(session_id);
        let scheduler_turn_id = self.retire_active_turn_for_maintenance(session_id);
        for retired_turn_id in [cancelled_turn_id, scheduler_turn_id].into_iter().flatten() {
            if !retired_turn_ids.contains(&retired_turn_id) {
                retired_turn_ids.push(retired_turn_id);
            }
        }
        Ok(SessionMaintenancePermit {
            _operation_guard: operation_guard,
            retired_turn_ids,
        })
    }

    pub(crate) async fn begin_session_deletion(
        &self,
        session_id: &str,
        requested_storage_path: &std::path::Path,
        wait_timeout: Duration,
    ) -> BitFunResult<SessionMaintenancePermit> {
        self.begin_session_maintenance(session_id, requested_storage_path, wait_timeout)
            .await
    }

    fn retire_active_turn_for_maintenance(&self, session_id: &str) -> Option<String> {
        let active_turn = self.active_turns.remove(session_id)?;
        let turn_id = active_turn.turn_id().to_string();
        self.retired_maintenance_outcomes.mark(session_id, &turn_id);
        self.active_internal_turns.remove(session_id);
        self.round_injection_buffer
            .drain_for_turn(session_id, &turn_id);
        self.take_suppressed_cancelled_reply(session_id, &turn_id);
        debug!(
            "Retired active turn before destructive session maintenance: session_id={}, turn_id={}",
            session_id, turn_id
        );
        Some(turn_id)
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn enqueue(&self, session_id: &str, queued_turn: QueuedTurn) -> Result<(), String> {
        let priority = queued_turn.policy.queue_priority;
        let new_len = match self.queues.enqueue(session_id, queued_turn, priority) {
            Ok(new_len) => new_len,
            Err(error) => {
                let max_depth = self.queues.max_depth();
                warn!(
                    "Queue full, rejecting message: session_id={}, max={}",
                    session_id, max_depth
                );
                return Err(error.to_string());
            }
        };

        debug!(
            "Message queued: session_id={}, queue_depth={}, priority={:?}",
            session_id, new_len, priority
        );
        Ok(())
    }

    async fn clear_queue(&self, session_id: &str) -> Vec<String> {
        let cleared_turns = self.queues.clear(session_id);
        let count = cleared_turns.len();
        let mut retired_turn_ids = Vec::new();
        for queued_turn in cleared_turns {
            match queued_turn.execution {
                QueuedTurnExecution::Standard | QueuedTurnExecution::FreshExternalSubagent(_) => {
                    if let Some(turn_id) = queued_turn.turn_id {
                        retired_turn_ids.push(turn_id.clone());
                        self.coordinator
                            .emit_event(AgenticEvent::DialogTurnCancelled {
                                session_id: session_id.to_string(),
                                turn_id,
                            })
                            .await;
                    } else {
                        warn!(
                            "Cleared queued dialog turn without a turn id: session_id={session_id}"
                        );
                    }
                }
                QueuedTurnExecution::HiddenSubagent(execution) => {
                    let coordinator = self.coordinator.clone();
                    tokio::spawn(async move {
                        coordinator
                            .cleanup_prepared_hidden_subagent_session_if_unsubmitted(
                                &execution.request,
                            )
                            .await;
                        execution.result_tx.send(Err(BitFunError::Cancelled(
                            "Subagent task was cancelled because a previous queued turn failed"
                                .to_string(),
                        )));
                    });
                }
            }
        }
        if count > 0 {
            info!(
                "Cleared {} queued messages: session_id={}",
                count, session_id
            );
        }
        retired_turn_ids
    }

    fn dequeue_next(&self, session_id: &str) -> Option<QueuedTurn> {
        self.queues.dequeue_next(session_id)
    }

    fn requeue_front(&self, session_id: &str, turn: QueuedTurn) {
        let priority = turn.policy.queue_priority;
        self.queues.requeue_front(session_id, turn, priority);
    }

    async fn try_start_next_queued(
        &self,
        session_id: &str,
    ) -> Result<Option<String>, SchedulerSubmitError> {
        let _operation_guard = self.lock_session_operation(session_id).await;
        self.try_start_next_queued_locked(session_id).await
    }

    async fn try_start_next_queued_locked(
        &self,
        session_id: &str,
    ) -> Result<Option<String>, SchedulerSubmitError> {
        let state = self
            .session_manager
            .get_session(session_id)
            .map(|s| s.state.clone());
        if matches!(state, Some(SessionState::Processing { .. })) {
            return Ok(None);
        }

        let Some(next_turn) = self.dequeue_next(session_id) else {
            return Ok(None);
        };

        let remaining = self.queues.depth(session_id);
        info!(
            "Dispatching queued message: session_id={}, priority={:?}, remaining_queue_depth={}",
            session_id, next_turn.policy.queue_priority, remaining
        );

        match self.start_turn(session_id, &next_turn).await {
            Ok(tid) => Ok(Some(tid)),
            Err(err) => {
                self.requeue_front(session_id, next_turn);
                Err(err)
            }
        }
    }

    async fn start_turn(
        &self,
        session_id: &str,
        queued_turn: &QueuedTurn,
    ) -> Result<String, SchedulerSubmitError> {
        match &queued_turn.execution {
            QueuedTurnExecution::HiddenSubagent(execution) => {
                // The scheduler-side await chain
                // `start_hidden_subagent_turn` -> spawned hidden execution ->
                // coordinator -> `deliver_background_result` -> follow-up
                // submission -> `submit_queued_turn_locked` ->
                // `try_start_next_queued_locked` -> `start_turn` forms a
                // cyclic opaque-future graph; a direct `.await` here would
                // make every future in the cycle non-`Send` and break
                // `tokio::spawn` at the hidden execution boundary. Run the
                // turn start through a detached task and join it: the
                // `JoinHandle` is a concrete `Send` type, so the cycle is
                // broken while the returned turn id and the caller-held
                // session operation permit semantics stay unchanged.
                let Some(scheduler) = self.self_arc() else {
                    return Err(SchedulerSubmitError::Message(
                        "scheduler self-arc unavailable for hidden subagent start".to_string(),
                    ));
                };
                let session_id_owned = session_id.to_string();
                let queued_turn_owned = queued_turn.clone();
                let execution_owned = execution.clone();
                let start_handle = tokio::spawn(async move {
                    scheduler
                        .start_hidden_subagent_turn(
                            &session_id_owned,
                            &queued_turn_owned,
                            &execution_owned,
                        )
                        .await
                });
                let start_result = start_handle.await.map_err(|join_error| {
                    SchedulerSubmitError::Message(format!(
                        "hidden subagent start task failed: {join_error}"
                    ))
                })?;
                return start_result.map_err(SchedulerSubmitError::Message);
            }
            QueuedTurnExecution::FreshExternalSubagent(execution) => {
                self.coordinator
                    .start_external_subagent_delegation_turn(
                        session_id.to_string(),
                        queued_turn.user_input.clone(),
                        queued_turn.original_user_input.clone(),
                        queued_turn.turn_id.clone(),
                        queued_turn.agent_type.clone(),
                        queued_turn.workspace_path.clone(),
                        queued_turn.policy,
                        queued_turn.user_message_metadata.clone(),
                        execution.ecosystem_id.clone(),
                        execution.logical_id.clone(),
                    )
                    .await
                    .map_err(SchedulerSubmitError::Core)?;

                let resolved = queued_turn.turn_id.clone().ok_or_else(|| {
                    format!(
                        "Scheduled external subagent delegation is missing turn_id: session_id={session_id}"
                    )
                })?;
                self.active_turns.insert(
                    session_id,
                    ActiveDialogTurn::new(
                        resolved.clone(),
                        queued_turn.workspace_path.clone(),
                        None,
                        None,
                        queued_turn.agent_type.clone(),
                        queued_turn
                            .original_user_input
                            .clone()
                            .unwrap_or_else(|| queued_turn.user_input.clone()),
                        queued_turn.user_message_metadata.clone(),
                        queued_turn.policy,
                        queued_turn.reply_route.clone(),
                    ),
                );
                return Ok(resolved);
            }
            QueuedTurnExecution::Standard => {}
        }

        let images = queued_turn
            .image_contexts
            .as_ref()
            .filter(|imgs| !imgs.is_empty());
        // Merge Warden pending reminders (penalty pokes / challenge pokes)
        // with the turn's own prepended messages so pokes ride into the next
        // dialog turn. Hidden-subagent turns return above and skip injection.
        let prepended_messages: Option<Vec<Message>> = {
            let mut warden_reminders = self
                .warden_runtime
                .lock()
                .await
                .take_pending_reminders(session_id);
            if warden_reminders.is_empty() {
                (!queued_turn.prepended_messages.is_empty())
                    .then(|| queued_turn.prepended_messages.clone())
            } else {
                warden_reminders.extend(queued_turn.prepended_messages.iter().cloned());
                Some(warden_reminders)
            }
        };
        let route = resolve_dialog_start_route(DialogStartRouteFacts {
            has_image_contexts: images.is_some(),
            has_prepended_messages: prepended_messages.is_some(),
        });

        let res = match route {
            DialogStartRoute::Plain => {
                self.coordinator
                    .start_dialog_turn(
                        session_id.to_string(),
                        queued_turn.user_input.clone(),
                        queued_turn.original_user_input.clone(),
                        queued_turn.turn_id.clone(),
                        queued_turn.agent_type.clone(),
                        queued_turn.workspace_path.clone(),
                        queued_turn.remote_connection_id.clone(),
                        queued_turn.remote_ssh_host.clone(),
                        queued_turn.policy,
                        queued_turn.user_message_metadata.clone(),
                    )
                    .await
            }
            DialogStartRoute::WithPrependedMessages => {
                self.coordinator
                    .start_dialog_turn_with_prepended_messages(
                        session_id.to_string(),
                        queued_turn.user_input.clone(),
                        queued_turn.original_user_input.clone(),
                        queued_turn.turn_id.clone(),
                        queued_turn.agent_type.clone(),
                        queued_turn.workspace_path.clone(),
                        queued_turn.remote_connection_id.clone(),
                        queued_turn.remote_ssh_host.clone(),
                        queued_turn.policy,
                        queued_turn.user_message_metadata.clone(),
                        prepended_messages
                            .clone()
                            .expect("prepended-messages route requires merged messages"),
                    )
                    .await
            }
            DialogStartRoute::WithImageContexts => {
                self.coordinator
                    .start_dialog_turn_with_image_contexts(
                        session_id.to_string(),
                        queued_turn.user_input.clone(),
                        queued_turn.original_user_input.clone(),
                        images
                            .cloned()
                            .expect("image-context route requires image contexts"),
                        queued_turn.turn_id.clone(),
                        queued_turn.agent_type.clone(),
                        queued_turn.workspace_path.clone(),
                        queued_turn.remote_connection_id.clone(),
                        queued_turn.remote_ssh_host.clone(),
                        queued_turn.policy,
                        queued_turn.user_message_metadata.clone(),
                    )
                    .await
            }
            DialogStartRoute::WithImageContextsAndPrependedMessages => {
                self.coordinator
                    .start_dialog_turn_with_image_contexts_and_prepended_messages(
                        session_id.to_string(),
                        queued_turn.user_input.clone(),
                        queued_turn.original_user_input.clone(),
                        images
                            .cloned()
                            .expect("image-context route requires image contexts"),
                        queued_turn.turn_id.clone(),
                        queued_turn.agent_type.clone(),
                        queued_turn.workspace_path.clone(),
                        queued_turn.remote_connection_id.clone(),
                        queued_turn.remote_ssh_host.clone(),
                        queued_turn.policy,
                        queued_turn.user_message_metadata.clone(),
                        prepended_messages
                            .clone()
                            .expect("prepended-messages route requires merged messages"),
                    )
                    .await
            }
        };

        res.map_err(SchedulerSubmitError::Core)?;

        // Plan-todo binding auto-mark (best-effort): when an agent-session
        // execution turn carries a planFile/todoId binding, mark the todo
        // in_progress. Only execution turns (reply_route.is_some()) can carry
        // a binding; reply turns have reply_route = None and never trigger
        // this hook. Failures only warn; they never fail the turn.
        if queued_turn.reply_route.is_some() {
            auto_mark_todo_in_progress_if_bound(
                queued_turn.user_message_metadata.as_ref(),
                queued_turn.workspace_path.as_deref(),
                queued_turn.remote_connection_id.as_deref(),
                queued_turn.remote_ssh_host.as_deref(),
            )
            .await;
        }

        // Standard scheduler submissions resolve and persist their turn ID
        // before entering the coordinator. Reading SessionState here races a
        // very fast terminal transition and can incorrectly turn an accepted,
        // completed turn into a submit error.
        let resolved = queued_turn.turn_id.clone().ok_or_else(|| {
            format!("Scheduled dialog turn is missing turn_id: session_id={session_id}")
        })?;

        self.active_turns.insert(
            session_id,
            ActiveDialogTurn::new(
                resolved.clone(),
                queued_turn.workspace_path.clone(),
                queued_turn.remote_connection_id.clone(),
                queued_turn.remote_ssh_host.clone(),
                queued_turn.agent_type.clone(),
                queued_turn
                    .original_user_input
                    .clone()
                    .unwrap_or_else(|| queued_turn.user_input.clone()),
                queued_turn.user_message_metadata.clone(),
                queued_turn.policy,
                queued_turn.reply_route.clone(),
            ),
        );

        Ok(resolved)
    }

    /// Box the hidden-subagent execution future behind a `dyn Future` trait
    /// object **outside** the scheduler state machine that spawns it.
    ///
    /// The review-reminder delivery path (COORD-04) routes from
    /// `execute_hidden_subagent_internal` back into the scheduler
    /// (`deliver_background_result` -> queued submit -> `start_turn` -> the
    /// hidden-subagent spawn site). A `tokio::spawn` block that awaited the
    /// concrete future directly would embed that whole chain in its own state
    /// machine, forming a self-referential opaque future type the compiler
    /// cannot check for `Send` (`fetching the hidden types of an opaque inside
    /// of the defining scope is not supported`). Returning a `Pin<Box<dyn
    /// Future + Send>>` from a plain function keeps the spawned task's state
    /// machine small and the type chain finite. Semantics are unchanged.
    fn box_hidden_subagent_execution(
        coordinator: Arc<ConversationCoordinator>,
        request: HiddenSubagentExecutionRequest,
        execution_cancel_token: CancellationToken,
        timeout_seconds: Option<u64>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = BitFunResult<SubagentResult>> + Send>,
    > {
        Box::pin(async move {
            coordinator
                .execute_prepared_hidden_subagent(
                    request,
                    Some(&execution_cancel_token),
                    timeout_seconds,
                )
                .await
        })
    }

    async fn start_hidden_subagent_turn(
        &self,
        session_id: &str,
        queued_turn: &QueuedTurn,
        execution: &HiddenSubagentQueuedExecution,
    ) -> Result<String, String> {
        let turn_id = queued_turn
            .turn_id
            .clone()
            .ok_or_else(|| "hidden subagent queued turn is missing turn_id".to_string())?;
        let request = execution.request.clone();
        let parent_cancel_token = request.parent_dialog_turn_id().and_then(|turn_id| {
            self.coordinator
                .execution_cancel_token_for_dialog_turn(turn_id)
                .map(|token| token.child_token())
        });
        let timeout_seconds = execution.timeout_seconds;
        let result_tx = execution.result_tx.clone();
        let coordinator = self.coordinator.clone();
        let outcome_tx = self.outcome_tx.clone();
        let session_id_owned = session_id.to_string();
        let turn_id_for_task = turn_id.clone();

        if execution.cancellation.is_cancelled() {
            self.coordinator
                .cleanup_prepared_hidden_subagent_session_if_unsubmitted(&execution.request)
                .await;
            // This path can run while the caller holds the session operation
            // permit. Never await the bounded outcome channel here: its
            // receiver may be waiting for the same permit.
            tokio::spawn(async move {
                let _ = outcome_tx
                    .send((
                        session_id_owned,
                        TurnOutcome::Cancelled {
                            turn_id: turn_id_for_task,
                        },
                    ))
                    .await;
            });
            result_tx.send(Err(BitFunError::Cancelled(
                "Subagent task has been cancelled".to_string(),
            )));
            return Ok(turn_id);
        }

        let queue_cancel_token = execution.cancellation.child_token();
        let execution_cancel_token = CancellationToken::new();
        let queue_cancel_token_for_bridge = queue_cancel_token.clone();
        let execution_cancel_token_for_bridge = execution_cancel_token.clone();
        let cancel_bridge_handle = match parent_cancel_token {
            Some(parent_cancel_token) => tokio::spawn(async move {
                tokio::select! {
                    _ = parent_cancel_token.cancelled() => {
                        execution_cancel_token_for_bridge.cancel();
                    }
                    _ = queue_cancel_token_for_bridge.cancelled() => {
                        execution_cancel_token_for_bridge.cancel();
                    }
                }
            }),
            None => tokio::spawn(async move {
                queue_cancel_token_for_bridge.cancelled().await;
                execution_cancel_token_for_bridge.cancel();
            }),
        };

        self.active_turns.insert(
            session_id,
            ActiveDialogTurn::new(
                turn_id.clone(),
                queued_turn.workspace_path.clone(),
                queued_turn.remote_connection_id.clone(),
                queued_turn.remote_ssh_host.clone(),
                queued_turn.agent_type.clone(),
                queued_turn
                    .original_user_input
                    .clone()
                    .unwrap_or_else(|| queued_turn.user_input.clone()),
                queued_turn.user_message_metadata.clone(),
                queued_turn.policy,
                queued_turn.reply_route.clone(),
            ),
        );
        self.active_internal_turns
            .insert(session_id.to_string(), ActiveInternalTurn::HiddenSubagent);

        let hidden_subagent_task = Self::box_hidden_subagent_execution(
            coordinator,
            request,
            execution_cancel_token,
            timeout_seconds,
        );
        tokio::spawn(async move {
            let outcome = hidden_subagent_task.await;
            match outcome {
                Ok(result) => {
                    // COORD-08: a partial-timeout result is not a completed
                    // turn; report it as Failed so callers never treat a
                    // half-finished subagent as a successful completion.
                    if result.status == SubagentResultStatus::PartialTimeout {
                        let reason = result
                            .reason
                            .as_deref()
                            .unwrap_or("timed out before completing the subagent task");
                        let _ = outcome_tx
                            .send((
                                session_id_owned.clone(),
                                TurnOutcome::Failed {
                                    turn_id: turn_id_for_task.clone(),
                                    error: format!("hidden subagent partial timeout: {reason}"),
                                },
                            ))
                            .await;
                    } else {
                        let _ = outcome_tx
                            .send((
                                session_id_owned.clone(),
                                TurnOutcome::Completed {
                                    turn_id: turn_id_for_task.clone(),
                                    final_response: result.text.clone(),
                                },
                            ))
                            .await;
                    }
                    result_tx.send(Ok(result));
                }
                Err(BitFunError::Cancelled(error_text)) => {
                    let _ = outcome_tx
                        .send((
                            session_id_owned.clone(),
                            TurnOutcome::Cancelled {
                                turn_id: turn_id_for_task.clone(),
                            },
                        ))
                        .await;
                    result_tx.send(Err(BitFunError::Cancelled(error_text)));
                }
                Err(error) => {
                    let error_text = error.to_string();
                    let _ = outcome_tx
                        .send((
                            session_id_owned.clone(),
                            TurnOutcome::Failed {
                                turn_id: turn_id_for_task.clone(),
                                error: error_text.clone(),
                            },
                        ))
                        .await;
                    result_tx.send(Err(error));
                }
            }
            cancel_bridge_handle.abort();
        });

        Ok(turn_id)
    }

    /// Replace characters unsafe for file names in archive ids (session ids,
    /// turn ids). Falls back to `unknown` when nothing safe remains.
    fn sanitize_archive_id(value: &str) -> String {
        let sanitized: String = value
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                    character
                } else {
                    '_'
                }
            })
            .collect();
        let trimmed = sanitized.trim_matches('_');
        if trimmed.is_empty() {
            "unknown".to_string()
        } else {
            trimmed.chars().take(128).collect()
        }
    }

    /// Extract the `Status: ...` line written into the reply reminder text by
    /// `resolve_agent_session_reply_action`. Best-effort: falls back to
    /// `unknown` when the line is missing.
    fn extract_status_from_reminder(reminder_text: &str) -> String {
        reminder_text
            .lines()
            .find_map(|line| {
                line.strip_prefix("Status: ")
                    .map(str::trim)
                    .filter(|status| !status.is_empty())
            })
            .unwrap_or("unknown")
            .to_string()
    }

    /// Default archive root: `~/.bitfun/agent-replies`, resolved through
    /// the shared `PathManager` so `BITFUN_HOME`/`BITFUN_E2E_HOME` overrides
    /// apply. Falls back to a temp location rather than panicking when the
    /// path manager cannot be constructed.
    fn resolve_default_agent_reply_archive_root() -> PathBuf {
        PathManager::new()
            .map(|path_manager| {
                path_manager
                    .bitfun_home_dir()
                    .join("agent-replies")
            })
            .unwrap_or_else(|_| {
                std::env::temp_dir()
                    .join("bitfun")
                    .join("agent-replies")
            })
    }

    /// Default shame-wall registry path for the embedded Warden runtime.
    ///
    /// Lives under the BitFun home (`~/.bitfun/warden/shame-wall-registry.json`)
    /// so violation records are shared across workspaces and survive process
    /// restarts, without touching any workspace-local file path
    /// (which is the Warden agent's skill-convention path, not the runtime's).
    /// Resolved through the shared `PathManager` so `BITFUN_HOME` overrides
    /// apply; falls back to a deterministic temp path rather than panicking
    /// when the path manager cannot be constructed.
    ///
    /// # Path mapping (d1-P2-4)
    ///
    /// Two violation-registry paths coexist by design:
    /// - this runtime path `~/.bitfun/warden/shame-wall-registry.json` is the
    ///   scheduler-embedded `WardenRuntime` persistence target;
    /// - `SHAME_WALL_FILENAME` (`.bitfun/warden/violation-registry.json`,
    ///   workspace-relative) is the Warden agent's manual write path under
    ///   `ToolRuntimeRestrictions::path_policy`.
    /// Keep this mapping in sync with warden/SKILL.md and
    /// docs/功能文档/10-warden守卫.md §4.
    fn resolve_warden_shame_wall_path() -> PathBuf {
        PathManager::new()
            .map(|path_manager| {
                path_manager
                    .bitfun_home_dir()
                    .join("warden")
                    .join("shame-wall-registry.json")
            })
            .unwrap_or_else(|_| {
                std::env::temp_dir()
                    .join("bitfun")
                    .join("warden")
                    .join("shame-wall-registry.json")
            })
    }

    /// Best-effort archive of a forwarded agent-session reply.
    ///
    /// Writes `<root>/<YYYY-MM>/<session_id>-<turn_id>.md` (UTF-8, no BOM)
    /// containing the reply facts already present on the plan: responder
    /// session, target session, status, server time, and reply text. This is
    /// an audit trail only — the caller must ignore failures so a full or
    /// read-only disk can never block reply delivery.
    async fn archive_agent_session_reply(
        root: &Path,
        responder_session_id: &str,
        turn_id: &str,
        plan: &AgentSessionReplyPlan,
    ) -> std::io::Result<PathBuf> {
        let month_dir = utc_iso8601_now();
        let month_dir = month_dir.get(..7).unwrap_or("unknown");
        let dir = root.join(month_dir);
        tokio::fs::create_dir_all(&dir).await?;
        let file_name = format!(
            "{}-{}.md",
            Self::sanitize_archive_id(responder_session_id),
            Self::sanitize_archive_id(turn_id)
        );
        let path = dir.join(file_name);
        let server_time = plan
            .user_message_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("serverTime"))
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let content = format!(
            "# Agent Session Reply Archive\n\n\
             - source_session: {responder_session_id}\n\
             - target_session: {}\n\
             - status: {}\n\
             - server_time: {server_time}\n\
             - archived_at: {}\n\
             - turn_id: {turn_id}\n\n\
             ## Reply Text\n\n{}\n",
            plan.target_session_id,
            Self::extract_status_from_reminder(&plan.reminder_text),
            utc_iso8601_now(),
            plan.user_input,
        );
        tokio::fs::write(&path, content).await?;
        Ok(path)
    }

    async fn forward_agent_session_reply(
        &self,
        responder_session_id: &str,
        turn_id: &str,
        plan: AgentSessionReplyPlan,
    ) {
        if let Err(error) = Self::archive_agent_session_reply(
            &self.agent_reply_archive_root(),
            responder_session_id,
            turn_id,
            &plan,
        )
        .await
        {
            warn!(
                "Failed to archive agent-session reply (best-effort): responder_session_id={}, target_session_id={}, turn_id={}, error={}",
                responder_session_id, plan.target_session_id, turn_id, error
            );
        }
        let reply_user_input = plan.user_input;
        let target_session_id = plan.target_session_id;
        let target_workspace_path = plan.target_workspace_path;
        let target_remote_connection_id = plan.target_remote_connection_id;
        let target_remote_ssh_host = plan.target_remote_ssh_host;
        let prepended_messages = vec![Message::internal_reminder(
            InternalReminderKind::SessionMessageReply,
            plan.reminder_text,
        )];
        let user_message_metadata = plan.user_message_metadata;

        if let Err(error) = self
            .submit_with_prepended_messages(
                target_session_id.clone(),
                reply_user_input.clone(),
                Some(reply_user_input),
                None,
                String::new(),
                Some(target_workspace_path),
                target_remote_connection_id,
                target_remote_ssh_host,
                DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                None,
                user_message_metadata,
                prepended_messages,
                None,
            )
            .await
        {
            warn!(
                "Failed to forward agent-session reply: responder_session_id={}, source_session_id={}, error={}",
                responder_session_id, target_session_id, error
            );
        }
    }

    /// Resolve the agent-reply archive root, defaulting to
    /// `~/.bitfun/agent-replies` on first use. Poison recovery keeps the
    /// best-effort archive path panic-free.
    fn agent_reply_archive_root(&self) -> PathBuf {
        let configured = {
            let guard = self
                .agent_reply_archive_root
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.clone()
        };
        if let Some(root) = configured {
            return root;
        }
        let default = Self::resolve_default_agent_reply_archive_root();
        let mut guard = self
            .agent_reply_archive_root
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if guard.is_none() {
            *guard = Some(default.clone());
        }
        default
    }

    #[cfg(test)]
    pub(crate) fn set_agent_reply_archive_root(&self, root: PathBuf) {
        let mut guard = self
            .agent_reply_archive_root
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(root);
    }

    fn take_suppressed_cancelled_reply(&self, session_id: &str, turn_id: &str) -> bool {
        self.suppressed_cancelled_replies.take(session_id, turn_id)
    }

    async fn dispatch_next_if_idle(&self, session_id: &str) -> Result<(), String> {
        let _ = self
            .try_start_next_queued(session_id)
            .await
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Background loop that receives turn outcome notifications from the
    /// coordinator.
    ///
    /// COORD-02: each outcome is dispatched into its own spawned task instead
    /// of being processed in one serial loop, so a slow outcome for one
    /// session no longer delays every other session (the bounded 128-slot
    /// channel was a global throughput bottleneck). Same-session ordering and
    /// mutual exclusion against submit/cancel stay intact via the session
    /// operation lock inside `process_turn_outcome`. The semaphore only caps
    /// the number of concurrently processing outcome tasks.
    async fn run_outcome_handler(&self, mut outcome_rx: mpsc::Receiver<(String, TurnOutcome)>) {
        let outcome_concurrency =
            Arc::new(tokio::sync::Semaphore::new(OUTCOME_PROCESSING_MAX_CONCURRENCY));
        while let Some((session_id, outcome)) = outcome_rx.recv().await {
            let Some(scheduler) = self.self_arc() else {
                break;
            };
            let permit = outcome_concurrency.clone();
            tokio::spawn(async move {
                let _permit = permit.acquire_owned().await;
                scheduler.process_turn_outcome(&session_id, outcome).await;
            });
        }
    }

    /// Process a single turn outcome for one session. Runs inside a spawned
    /// task (see `run_outcome_handler`), so different sessions are handled
    /// concurrently; the session operation lock keeps same-session outcome
    /// processing serialized and closed against concurrent submit/cancel.
    async fn process_turn_outcome(&self, session_id: &str, outcome: TurnOutcome) {
        let (active_turn, active_internal_turn, lifecycle_plan) = {
            let _operation_guard = self.lock_session_operation(session_id).await;
            let Some(active_turn_result) = take_active_turn_for_outcome(
                &self.active_turns,
                &self.retired_maintenance_outcomes,
                session_id,
                outcome.turn_id(),
            ) else {
                self.round_injection_buffer
                    .drain_for_turn(session_id, outcome.turn_id());
                self.take_suppressed_cancelled_reply(session_id, outcome.turn_id());
                debug!(
                    "Ignoring outcome retired by session deletion: session_id={}, turn_id={}",
                    session_id,
                    outcome.turn_id()
                );
                return;
            };
            let active_turn = match active_turn_result {
                ActiveDialogTurnTakeResult::Matched(turn) => Some(turn),
                ActiveDialogTurnTakeResult::Absent => None,
                ActiveDialogTurnTakeResult::DifferentTurn => {
                    self.round_injection_buffer
                        .drain_for_turn(session_id, outcome.turn_id());
                    self.take_suppressed_cancelled_reply(session_id, outcome.turn_id());
                    debug!(
                        "Ignoring stale turn outcome: session_id={}, turn_id={}",
                        session_id,
                        outcome.turn_id()
                    );
                    return;
                }
            };
            let active_internal_turn = active_turn.as_ref().and_then(|_| {
                self.active_internal_turns
                    .remove(session_id)
                    .map(|(_, turn)| turn)
            });
            let lifecycle_plan =
                resolve_turn_outcome_lifecycle_plan(&outcome, active_turn.is_some());
            if lifecycle_plan.queue_action == TurnOutcomeQueueAction::ClearQueue {
                debug!(
                    "Turn {}, clearing queue: session_id={}",
                    lifecycle_plan.status, session_id
                );
                let _ = self.clear_queue(session_id).await;
            }
            (active_turn, active_internal_turn, lifecycle_plan)
        };
        let status = lifecycle_plan.status;
        let queue_action = lifecycle_plan.queue_action;
        // Turn-driven Warden runtime: feed the finished turn outcome so
        // consecutive-failure penalties and challenge pokes are queued for
        // the next turn of this session. Batch-2 goal switch: Warden hooks
        // only run while the session has an active thread goal, so
        // failures of goal-less or non-active-goal sessions never
        // accumulate (see `session_has_active_goal`).
        if self.session_has_active_goal(session_id).await {
            let mut warden = self.warden_runtime.lock().await;
            warden
                .on_turn_outcome(session_id, status, outcome.turn_id())
                .await;
        } else {
            // WARDEN-01: the session's goal left the active state (or the
            // session is non-main) — drop the stale consecutive-failure
            // counts here so a later, *new* goal generation starts from a
            // clean ladder instead of firing the previous goal's L2/L3 on
            // its first failure. Idempotent; harmless when already clear.
            self.warden_runtime
                .lock()
                .await
                .clear_failure_counts(session_id);
        }
        // Only drop steering messages targeted at the *finished* turn. We
        // must NOT clear the entire session buffer here: a user might have
        // legitimately submitted steering against a brand-new follow-up
        // turn that the dispatcher will pick up immediately after this
        // outcome is processed (race window between turn finalize and the
        // next turn starting). Targeting by turn_id keeps those alive.
        if lifecycle_plan.drain_finished_turn_injections {
            // 残留 steering 转交（主人裁决：UserSteering 重复消费 = 不必要；
            // 但未送达的真实用户消息不能被静默丢弃）——turn 结束时仍未被
            // round 边界消费的 UserSteering 转为普通 follow-up turn 投递，
            // 注入文本/结构不变，只是改走 turn 通道。
            let undelivered = self
                .round_injection_buffer
                .drain_undelivered_steering(session_id, outcome.turn_id());
            if !undelivered.is_empty() {
                for steering in undelivered {
                    let steering_content = steering.content.clone();
                    let steering_session = session_id.to_string();
                    let agent_type = self
                        .session_manager
                        .get_session(session_id)
                        .map(|session| session.agent_type.clone())
                        .unwrap_or_else(|| "agentic".to_string());
                    if let Err(error) = self
                        .submit_with_prepended_messages(
                            steering_session,
                            steering_content.clone(),
                            Some(steering_content.clone()),
                            None,
                            agent_type,
                            None,
                            None,
                            None,
                            DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                            None,
                            None,
                            Vec::new(),
                            None,
                        )
                        .await
                    {
                        warn!(
                            "Failed to redeliver undelivered steering as follow-up: session_id={}, error={}",
                            session_id, error
                        );
                    }
                    self.round_injection_buffer
                        .mark_steering_consumed(session_id, &steering_content, steering.dedup_key());
                }
            }
            self.round_injection_buffer
                .drain_for_turn(session_id, outcome.turn_id());
        }
        let suppressed_cancelled_reply =
            self.take_suppressed_cancelled_reply(session_id, outcome.turn_id());
        let is_internal_turn = active_internal_turn.is_some();
        if !is_internal_turn {
            if let Some(active_turn) = active_turn.as_ref() {
                // COORD-10: re-acquire the session operation lock around the
                // reply decision and delivery. The take above released it, and
                // this section reads session facts (role, tree depth) and
                // forwards replies into other sessions; serializing it against
                // a concurrent submit/cancel for this session removes the
                // stale-window race.
                let _reply_guard = self.lock_session_operation(session_id).await;
                match resolve_agent_session_reply_action(
                    session_id,
                    get_session_role(session_id).map(|role| role.as_str()),
                    self.coordinator.session_tree().get_depth(session_id),
                    active_turn,
                    &outcome,
                    suppressed_cancelled_reply,
                ) {
                    AgentSessionReplyAction::NoReply => {}
                    AgentSessionReplyAction::SkipSuppressedCancelledReply => {
                        debug!(
                            "Skipping cancelled auto-reply because the source session explicitly cancelled its own SessionMessage request: session_id={}, turn_id={}",
                            session_id,
                            outcome.turn_id()
                        );
                    }
                    AgentSessionReplyAction::Forward(plan) => {
                        self.forward_agent_session_reply(
                            session_id,
                            outcome.turn_id(),
                            plan,
                        )
                        .await;
                    }
                }

                // Plan-todo binding auto-complete (best-effort): when the
                // finished turn is an agent-session execution turn bound
                // to a plan todo (reply_route.is_some()) and it completed
                // normally, mark the todo completed. Failed/Cancelled
                // outcomes are intentionally left untouched (kept pending
                // for the commander to adjudicate). Reply turns have
                // reply_route = None and never trigger this hook. Failures
                // only warn; they never affect the outcome pipeline.
                if active_turn.reply_route().is_some() {
                    auto_mark_todo_completed_if_bound(
                        active_turn.user_message_metadata(),
                        active_turn.workspace_path(),
                        active_turn.remote_connection_id(),
                        active_turn.remote_ssh_host(),
                        &outcome,
                    )
                    .await;
                }
            }
        }

        if !is_internal_turn {
            // The plan already encodes "no active turn" as SkipNoActiveTurn,
            // so no extra active_turn guard is needed here.
            match lifecycle_plan.goal_continuation {
                GoalContinuationAfterTurnAction::SkipNoActiveTurn => {}
                GoalContinuationAfterTurnAction::AbortForCancelled => {
                    self.goal_continuation_abort.mark(session_id);
                    debug!(
                        "Skipping thread goal continuation after user-cancelled turn: session_id={}, turn_id={}",
                        session_id,
                        outcome.turn_id()
                    );
                }
                GoalContinuationAfterTurnAction::Evaluate { .. } => {
                    // COORD-02: `prepare_goal_continuation_after_turn`
                    // always returns `Ok(None)` (the immediate after-turn
                    // continuation channel is closed; only the idle-wakeup
                    // safety net continues goals). The submit-retry loop
                    // below it was therefore unreachable dead code and is
                    // removed. The abort-flag clear is kept so a normal
                    // completion un-sticks the flag for future goal paths.
                    self.goal_continuation_abort.clear(session_id);
                }
            }
        }

        match queue_action {
            TurnOutcomeQueueAction::DispatchNext => {
                if status == TurnOutcomeStatus::Cancelled {
                    debug!(
                        "Turn cancelled, dispatching next queued message if present: session_id={}",
                        session_id
                    );
                }

                if let Err(e) = self.dispatch_next_if_idle(session_id).await {
                    warn!(
                        "Failed to dispatch next queued message after {}: session_id={}, error={}",
                        status, session_id, e
                    );
                }
            }
            TurnOutcomeQueueAction::ClearQueue => {}
        }

        // Top-level turn finished: restart the goal idle-wakeup safety net
        // so it counts from turn end, not from submission. Subagent and
        // other internal turns skip this; they carry no goal of their own.
        // schedule_goal_idle_wakeup bumps the session generation, which
        // invalidates any older wakeup task, so a user submission that
        // raced in ahead of this outcome is still honored.
        if !is_internal_turn {
            self.schedule_goal_idle_wakeup(session_id);
            // Immediate workspace-quiescent condition: this top-level turn
            // just finished and (when nothing else is running or queued)
            // every conversation in the workspace is now silent, so wake
            // the goal right away instead of waiting for the 10-minute
            // timer.
            self.maybe_trigger_goal_wakeup_when_workspace_quiescent(session_id)
                .await;
        }
    }
}

fn metadata_string(
    metadata: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    metadata
        .get(key)
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn mime_type_from_data_url(data_url: &str) -> Option<String> {
    data_url
        .split_once(',')
        .and_then(|(header, _)| {
            header
                .strip_prefix("data:")
                .and_then(|rest| rest.split(';').next())
        })
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn image_context_metadata(attachment: &AgentInputAttachment) -> Option<serde_json::Value> {
    if let Some(metadata) = attachment.metadata.get("metadata").cloned() {
        return Some(metadata);
    }

    let mut metadata = serde_json::Map::new();
    if let Some(name) = metadata_string(&attachment.metadata, "name") {
        metadata.insert("name".to_string(), serde_json::Value::String(name));
    }
    if attachment.metadata.contains_key("dataUrl") {
        metadata.insert(
            "source".to_string(),
            serde_json::Value::String("remote".to_string()),
        );
    }

    if metadata.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(metadata))
    }
}

fn agent_dialog_turn_image_contexts(
    attachments: &[AgentInputAttachment],
) -> PortResult<Option<Vec<ImageContextData>>> {
    if attachments.is_empty() {
        return Ok(None);
    }

    let mut image_contexts = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        if attachment.kind != "remote_image" {
            return Err(PortError::new(
                PortErrorKind::InvalidRequest,
                format!(
                    "unsupported agent dialog attachment kind: {}",
                    attachment.kind
                ),
            ));
        }

        let data_url = metadata_string(&attachment.metadata, "dataUrl");
        let image_path = metadata_string(&attachment.metadata, "imagePath");
        if data_url.is_none() && image_path.is_none() {
            return Err(PortError::new(
                PortErrorKind::InvalidRequest,
                "remote_image attachment requires dataUrl or imagePath",
            ));
        }

        let mime_type = metadata_string(&attachment.metadata, "mimeType")
            .or_else(|| data_url.as_deref().and_then(mime_type_from_data_url))
            .unwrap_or_else(|| "image/png".to_string());

        image_contexts.push(ImageContextData {
            id: attachment.id.clone(),
            image_path,
            data_url,
            mime_type,
            metadata: image_context_metadata(attachment),
        });
    }

    Ok(Some(image_contexts))
}

fn agent_dialog_turn_prepended_messages(
    reminders: &[AgentDialogPrependedReminder],
) -> PortResult<Vec<Message>> {
    reminders
        .iter()
        .map(|reminder| {
            let kind = match reminder.kind.as_str() {
                "session_message_request" => InternalReminderKind::SessionMessageRequest,
                "task_subagent_result" => InternalReminderKind::BackgroundResult,
                "scheduled_job" => InternalReminderKind::ScheduledJob,
                other => {
                    return Err(PortError::new(
                        PortErrorKind::InvalidRequest,
                        format!("unsupported agent dialog prepended reminder kind: {other}"),
                    ));
                }
            };
            Ok(Message::internal_reminder(kind, reminder.text.clone()))
        })
        .collect()
}

impl DialogScheduler {
    pub(crate) async fn submit_agent_dialog_turn_reject_if_busy(
        &self,
        request: AgentDialogTurnRequest,
    ) -> PortResult<DialogSubmitOutcome> {
        self.submit_agent_dialog_turn_with_busy_policy(request, true)
            .await
    }

    async fn submit_agent_dialog_turn_with_busy_policy(
        &self,
        request: AgentDialogTurnRequest,
        reject_if_busy: bool,
    ) -> PortResult<DialogSubmitOutcome> {
        let (execution, reject_if_busy) = match &request.execution {
            AgentDialogTurnExecution::Standard => (QueuedTurnExecution::Standard, reject_if_busy),
            AgentDialogTurnExecution::FreshExternalSubagent {
                ecosystem_id,
                logical_id,
            } => {
                if ecosystem_id.trim().is_empty() || logical_id.trim().is_empty() {
                    return Err(PortError::new(
                        PortErrorKind::InvalidRequest,
                        "External subagent delegation requires non-empty ecosystem_id and logical_id",
                    ));
                }
                if !request.attachments.is_empty() || !request.prepended_reminders.is_empty() {
                    return Err(PortError::new(
                        PortErrorKind::InvalidRequest,
                        "External subagent delegation does not accept attachments or prepended reminders",
                    ));
                }
                if request.remote_connection_id.is_some() || request.remote_ssh_host.is_some() {
                    return Err(PortError::new(
                        PortErrorKind::NotAvailable,
                        "External subagent delegation is unavailable for remote workspaces",
                    ));
                }
                (
                    QueuedTurnExecution::FreshExternalSubagent(
                        ExternalSubagentDelegationQueuedExecution {
                            ecosystem_id: ecosystem_id.trim().to_string(),
                            logical_id: logical_id.trim().to_string(),
                        },
                    ),
                    true,
                )
            }
        };
        let image_contexts = agent_dialog_turn_image_contexts(&request.attachments)?;
        let prepended_messages =
            agent_dialog_turn_prepended_messages(&request.prepended_reminders)?;
        let user_message_metadata = if request.metadata.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(request.metadata))
        };
        let resolved_turn_id = request
            .turn_id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let settlement_registration = self
            .coordinator
            .try_register_turn_settlement(&request.session_id, &resolved_turn_id)
            .ok_or_else(|| {
                PortError::new(
                    PortErrorKind::InvalidRequest,
                    format!(
                        "Dialog turn ID is already active or completed: session_id={}, turn_id={resolved_turn_id}",
                        request.session_id
                    ),
                )
            })?;
        let queued_turn = QueuedTurn {
            user_input: request.message,
            original_user_input: request.original_message,
            prepended_messages,
            turn_id: Some(resolved_turn_id.clone()),
            agent_type: request.agent_type,
            workspace_path: request.workspace_path,
            remote_connection_id: request.remote_connection_id,
            remote_ssh_host: request.remote_ssh_host,
            policy: request.policy,
            reply_route: request.reply_route,
            user_message_metadata,
            image_contexts,
            enqueued_at: SystemTime::now(),
            _settlement_registration: Some(settlement_registration),
            execution,
        };

        self.submit_queued_turn(
            request.session_id,
            resolved_turn_id,
            queued_turn,
            reject_if_busy,
        )
        .await
        .map_err(SchedulerSubmitError::into_port_error)
    }
}

#[async_trait::async_trait]
impl AgentDialogTurnPort for DialogScheduler {
    async fn submit_dialog_turn(
        &self,
        request: AgentDialogTurnRequest,
    ) -> PortResult<DialogSubmitOutcome> {
        self.submit_agent_dialog_turn_with_busy_policy(request, false)
            .await
    }

    async fn steer_dialog_turn(
        &self,
        request: AgentDialogSteerRequest,
    ) -> PortResult<DialogSteerOutcome> {
        let empty_content = request.content.trim().is_empty();
        DialogScheduler::buffer_steering(
            self,
            request.session_id,
            request.turn_id,
            request.content,
            request.display_content,
            request.prepended_reminders,
        )
        .await
        .map_err(|error| {
            PortError::new(
                if empty_content {
                    PortErrorKind::InvalidRequest
                } else {
                    PortErrorKind::SessionInUse
                },
                error,
            )
        })
    }
}

#[async_trait::async_trait]
impl AgentLifecycleDeliveryPort for DialogScheduler {
    async fn deliver_background_result(
        &self,
        request: AgentBackgroundResultRequest,
    ) -> PortResult<()> {
        let metadata = if request.metadata.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(request.metadata))
        };

        DialogScheduler::deliver_background_result(
            self,
            request.session_id,
            request.agent_type,
            request.workspace_path,
            request.remote_connection_id,
            request.remote_ssh_host,
            request.content,
            request.display_content,
            metadata,
        )
        .await
        .map_err(|error| PortError::new(PortErrorKind::Backend, error))
    }

    async fn deliver_thread_goal(&self, request: AgentThreadGoalDeliveryRequest) -> PortResult<()> {
        let result = match request.kind {
            AgentThreadGoalDeliveryKind::Resumed => {
                DialogScheduler::deliver_thread_goal_resumed(
                    self,
                    request.session_id,
                    request.agent_type,
                    request.workspace_path,
                    request.remote_connection_id,
                    request.remote_ssh_host,
                    request.goal,
                )
                .await
            }
            AgentThreadGoalDeliveryKind::ObjectiveUpdated => {
                DialogScheduler::deliver_thread_goal_objective_updated(
                    self,
                    request.session_id,
                    request.agent_type,
                    request.workspace_path,
                    request.remote_connection_id,
                    request.remote_ssh_host,
                    request.goal,
                )
                .await
            }
        };

        result.map_err(|error| PortError::new(PortErrorKind::Backend, error))
    }
}

#[async_trait::async_trait]
impl AgentTurnCancellationPort for DialogScheduler {
    async fn cancel_turn(
        &self,
        request: AgentTurnCancellationRequest,
    ) -> PortResult<AgentTurnCancellationResult> {
        let session_id = request.session_id;
        let wait_timeout = Duration::from_millis(request.wait_timeout_ms.unwrap_or(1500));

        let cancelled_turn_id = if let Some(turn_id) = request.turn_id {
            // COORD-12: map the removal result instead of discarding it. The
            // previous code unconditionally reported `Some(turn_id)`, so
            // `requested` was always true even when the turn was neither
            // queued nor active. `cancel_queued_or_active_turn` returns true
            // only when the turn was actually removed before it started.
            let removed = self
                .cancel_queued_or_active_turn(&session_id, &turn_id)
                .await
                .map_err(|error| PortError::new(PortErrorKind::Backend, error.to_string()))?;
            if removed { Some(turn_id) } else { None }
        } else if let Some(requester_session_id) = request.requester_session_id {
            self.cancel_active_turn_for_session_from_requester(
                &session_id,
                &requester_session_id,
                wait_timeout,
            )
            .await
            .map_err(|error| PortError::new(PortErrorKind::Backend, error.to_string()))?
        } else {
            self.cancel_active_turn_for_session_with_descendant_policy(
                &session_id,
                wait_timeout,
                request.cancel_descendants,
            )
            .await
            .map_err(|error| PortError::new(PortErrorKind::Backend, error.to_string()))?
        };

        Ok(AgentTurnCancellationResult {
            session_id,
            requested: cancelled_turn_id.is_some(),
            turn_id: cancelled_turn_id,
        })
    }
}

fn thread_goal_delivery_messages(reminders: Vec<ThreadGoalDeliveryReminder>) -> Vec<Message> {
    reminders
        .into_iter()
        .map(|reminder| match reminder.kind {
            ThreadGoalDeliveryReminderKind::GoalContinuation => {
                goal_internal_context_message(reminder.content)
            }
            ThreadGoalDeliveryReminderKind::GoalObjectiveUpdated => {
                goal_objective_updated_message(reminder.content)
            }
        })
        .collect()
}

fn background_result_delivery_state_fact(
    session_id: &str,
    state: Option<&SessionState>,
    metadata: Option<&serde_json::Value>,
) -> DialogSessionStateFact {
    let Some(SessionState::Processing {
        current_turn_id, ..
    }) = state
    else {
        return DialogScheduler::session_state_fact(state);
    };
    let Some(metadata) = metadata.and_then(serde_json::Value::as_object) else {
        return DialogSessionStateFact::Processing;
    };
    let has_exact_parent =
        metadata.contains_key("parentSessionId") || metadata.contains_key("parentDialogTurnId");
    if !has_exact_parent {
        return DialogSessionStateFact::Processing;
    }

    let exact_parent_matches = metadata
        .get("parentSessionId")
        .and_then(serde_json::Value::as_str)
        .zip(
            metadata
                .get("parentDialogTurnId")
                .and_then(serde_json::Value::as_str),
        )
        .is_some_and(|(parent_session_id, parent_turn_id)| {
            parent_session_id == session_id && parent_turn_id == current_turn_id
        });
    if exact_parent_matches {
        DialogSessionStateFact::Processing
    } else {
        // The session is busy, but this result does not belong to the running turn.
        // Resolve it as a follow-up; the normal submission path will queue it.
        DialogSessionStateFact::Idle
    }
}

// ── Global instance ──────────────────────────────────────────────────────────

/// TTL for the `session_has_active_goal` short-term cache (COORD-02). Kept
/// small so goal state changes (pause/resume/complete) reach Warden
/// enforcement within a few seconds, while outcome handling stays off the
/// disk-backed goal store.
const GOAL_ACTIVE_CACHE_TTL: Duration = Duration::from_secs(5);

/// Ceiling for concurrently processing outcome tasks (COORD-02). The outcome
/// channel itself stays bounded at 128; this semaphore only prevents an
/// unbounded task pile-up when a burst of outcomes arrives while sessions
/// are busy.
const OUTCOME_PROCESSING_MAX_CONCURRENCY: usize = 64;

static GLOBAL_SCHEDULER: OnceLock<Arc<DialogScheduler>> = OnceLock::new();

pub fn get_global_scheduler() -> Option<Arc<DialogScheduler>> {
    GLOBAL_SCHEDULER.get().cloned()
}

pub fn set_global_scheduler(scheduler: Arc<DialogScheduler>) {
    let _ = GLOBAL_SCHEDULER.set(scheduler);
}

/// Stop in-flight thread-goal continuation submit retries when the user cancels a turn.
pub fn abort_thread_goal_continuation_for_session(session_id: &str) {
    if let Some(scheduler) = get_global_scheduler() {
        scheduler.goal_continuation_abort.mark(session_id);
    }
}

/// Allow goal auto-continuation again after the user explicitly resumes a paused goal.
pub fn clear_thread_goal_continuation_abort(session_id: &str) {
    if let Some(scheduler) = get_global_scheduler() {
        scheduler.goal_continuation_abort.clear(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::core::{ProcessingPhase, SessionConfig};
    use crate::agentic::events::{EventQueue, EventQueueConfig, EventRouter};
    use crate::agentic::execution::{
        ExecutionEngine, ExecutionEngineConfig, RoundExecutor, StreamProcessor,
    };
    use crate::agentic::persistence::PersistenceManager;
    use crate::agentic::session::{
        compression::{CompressionConfig, ContextCompressor},
        revert::{SessionRevertPhase, SessionRevertState, SESSION_REVERT_SCHEMA_VERSION},
        PromptCachePolicy, SessionContextStore, SessionManagerConfig,
    };
    use crate::agentic::tools::registry::ToolRegistry;
    use crate::agentic::tools::{ToolPipeline, ToolStateManager};
    use crate::infrastructure::PathManager;
    use bitfun_runtime_ports::{AgentDialogPrependedReminder, AgentInputAttachment, PortErrorKind};
    use tokio::sync::RwLock as TokioRwLock;

    #[test]
    fn scheduler_preserves_session_writer_conflicts() {
        let error = SchedulerSubmitError::Core(BitFunError::SessionInUse {
            session_id: "session-1".to_string(),
        })
        .into_port_error();

        assert_eq!(error.kind, PortErrorKind::SessionInUse);
    }

    fn test_scheduler() -> (
        Arc<DialogScheduler>,
        Arc<SessionManager>,
        Arc<EventQueue>,
        tempfile::TempDir,
    ) {
        let root = tempfile::tempdir().expect("test root");
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(
                PersistenceManager::new(Arc::new(PathManager::with_user_root_for_tests(
                    root.path().join("user-root"),
                )))
                .expect("persistence manager"),
            ),
            SessionManagerConfig {
                max_active_sessions: 100,
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
        let coordinator = Arc::new(ConversationCoordinator::new(
            session_manager.clone(),
            execution_engine,
            tool_pipeline,
            event_queue.clone(),
            Arc::new(EventRouter::new()),
            Arc::new(
                crate::runtime_ownership::CoreRuntimeOwnership::embedded_with_facts(
                    std::env::temp_dir().join(format!(
                        "bitfun-scheduler-ownership-test-{}",
                        uuid::Uuid::new_v4()
                    )),
                    "bitfun".to_string(),
                    "test",
                ),
            ),
        ));
        let scheduler = DialogScheduler::new(coordinator, session_manager.clone());
        // Isolate the best-effort agent-reply archive so outcome-handler
        // tests never write into the real `~/.bitfun` home.
        scheduler.set_agent_reply_archive_root(root.path().join("agent-replies"));
        (scheduler, session_manager, event_queue, root)
    }
    #[test]
    fn queued_turn_execution_default_is_standard() {
        assert!(matches!(
            QueuedTurnExecution::default(),
            QueuedTurnExecution::Standard
        ));
    }

    #[test]
    fn session_tree_silence_requires_every_descendant_idle() {
        let now = SystemTime::now();
        let idle_delay = Duration::from_millis(GOAL_IDLE_WAKEUP_DELAY_MS);
        let idle = now - idle_delay - Duration::from_secs(60);
        let active = now - Duration::from_secs(1);
        let tree = vec![
            "parent".to_string(),
            "child".to_string(),
            "grandchild".to_string(),
        ];

        // Every node idle -> the whole tree is silent.
        assert!(session_tree_is_silent(&tree, now, idle_delay, |_| false, |_| {
            Some(idle)
        }));

        // A descendant active within the idle window blocks the wakeup even
        // when the parent itself is idle.
        assert!(!session_tree_is_silent(&tree, now, idle_delay, |_| false, |id| {
            if id == "child" { Some(active) } else { Some(idle) }
        }));

        // A busy descendant blocks the wakeup even when every node looks idle.
        assert!(!session_tree_is_silent(&tree, now, idle_delay, |id| {
            id == "grandchild"
        }, |_| {
            Some(idle)
        }));

        // A descendant that no longer exists contributes no activity.
        assert!(session_tree_is_silent(&tree, now, idle_delay, |_| false, |id| {
            if id == "grandchild" { None } else { Some(idle) }
        }));

        // Root-only tree follows the root activity.
        let root_only = vec!["parent".to_string()];
        assert!(session_tree_is_silent(&root_only, now, idle_delay, |_| false, |_| {
            Some(idle)
        }));
        assert!(!session_tree_is_silent(&root_only, now, idle_delay, |_| false, |_| {
            Some(active)
        }));
    }

    fn session_summary(session_id: &str, parent_session_id: Option<&str>) -> SessionSummary {
        SessionSummary {
            session_id: session_id.to_string(),
            session_name: session_id.to_string(),
            agent_type: "agentic".to_string(),
            model_id: None,
            reasoning_preset: None,
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            created_by: None,
            kind: SessionKind::Standard,
            turn_count: 0,
            created_at: SystemTime::now(),
            last_activity_at: SystemTime::now(),
            state: SessionState::Idle,
            parent_session_id: parent_session_id.map(ToOwned::to_owned),
            is_daemon: false,
        }
    }

    #[test]
    fn session_tree_root_walks_up_parent_chain() {
        let summaries = vec![
            session_summary("root", None),
            session_summary("child", Some("root")),
            session_summary("grandchild", Some("child")),
        ];
        // The deepest descendant resolves to the tree root (primary
        // conversation).
        assert_eq!(session_tree_root_id(&summaries, "grandchild"), "root");
        assert_eq!(session_tree_root_id(&summaries, "child"), "root");
        assert_eq!(session_tree_root_id(&summaries, "root"), "root");
        // Unknown sessions fall back to themselves.
        assert_eq!(session_tree_root_id(&summaries, "unknown"), "unknown");
        // A parent chain that never terminates is capped at 64 hops.
        let self_cycle = vec![session_summary("a", Some("b")), session_summary("b", Some("a"))];
        let _ = session_tree_root_id(&self_cycle, "a");
    }

    #[test]
    fn goal_idle_wakeup_fires_when_primary_or_all_conversations_silent() {
        let now = SystemTime::now();
        let idle_delay = Duration::from_millis(GOAL_IDLE_WAKEUP_DELAY_MS);
        let idle = now - idle_delay - Duration::from_secs(60);
        let active = now - Duration::from_secs(1);
        let primary = vec!["primary".to_string()];
        let all = vec!["primary".to_string(), "subagent".to_string()];

        // Primary silent while a subagent is still busy -> condition 1 fires,
        // condition 2 (workspace quiescent) does not.
        let (primary_silent, all_silent) = goal_idle_wakeup_conditions_met(
            &primary,
            &all,
            now,
            idle_delay,
            |id| id == "subagent",
            |_| Some(idle),
        );
        assert!(primary_silent);
        assert!(!all_silent);

        // Everything old-idle -> both conditions fire.
        let (primary_silent, all_silent) = goal_idle_wakeup_conditions_met(
            &primary,
            &all,
            now,
            idle_delay,
            |_| false,
            |_| Some(idle),
        );
        assert!(primary_silent && all_silent);

        // Primary busy -> neither condition fires.
        let (primary_silent, all_silent) = goal_idle_wakeup_conditions_met(
            &primary,
            &all,
            now,
            idle_delay,
            |id| id == "primary",
            |_| Some(idle),
        );
        assert!(!primary_silent && !all_silent);

        // Primary had activity within the window (so condition 1 does not
        // fire) but nothing is busy/queued -> condition 2 fires immediately.
        let (primary_silent, all_silent) = goal_idle_wakeup_conditions_met(
            &primary,
            &all,
            now,
            idle_delay,
            |_| false,
            |id| {
                if id == "primary" { Some(active) } else { Some(idle) }
            },
        );
        assert!(!primary_silent && all_silent);
    }

    #[test]
    fn goal_idle_wakeup_all_sessions_condition_ignores_idle_window() {
        let now = SystemTime::now();
        let idle_delay = Duration::from_millis(GOAL_IDLE_WAKEUP_DELAY_MS);
        // Activity within the idle window: under the old semantics this blocked
        // the whole-workspace condition; the immediate condition-2 fires on
        // quiescence alone.
        let recent = now - Duration::from_secs(1);
        let all = vec!["session-a".to_string(), "session-b".to_string()];

        // No session busy/queued, even with recent activity -> immediate.
        assert!(all_sessions_quiescent(&all, |_| false));

        // Any busy or queued session keeps the workspace from being quiescent.
        assert!(!all_sessions_quiescent(&all, |id| id == "session-b"));

        // Condition-2 helper does not consult last activity.
        let (_, all_silent) = goal_idle_wakeup_conditions_met(
            &["session-a".to_string()],
            &all,
            now,
            idle_delay,
            |_| false,
            |_| Some(recent),
        );
        assert!(all_silent);
    }

    #[tokio::test]
    async fn top_level_turn_outcome_restarts_goal_idle_wakeup() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "goal-wakeup-session";
        let turn_id = "goal-wakeup-turn";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "GoalWakeup".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("create session");
        scheduler
            .active_turns
            .insert(session_id, desktop_active_turn(turn_id));

        scheduler
            .outcome_tx
            .send((
                session_id.to_string(),
                TurnOutcome::Completed {
                    turn_id: turn_id.to_string(),
                    final_response: "done".to_string(),
                },
            ))
            .await
            .expect("send outcome");

        // The outcome handler runs on a background task. Wait until the turn
        // is consumed, then require the idle-wakeup generation to have been
        // bumped (the schedule_goal_idle_wakeup side effect of this hook).
        for _ in 0..100 {
            let turn_consumed = !scheduler.active_turns.matches_turn(session_id, turn_id);
            let generation_bumped = scheduler
                .goal_idle_wakeup_generations
                .get(session_id)
                .is_some_and(|generation| *generation >= 1);
            if turn_consumed && generation_bumped {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("top-level turn outcome did not restart the goal idle-wakeup timer");
    }

    #[tokio::test]
    async fn internal_turn_outcome_skips_goal_idle_wakeup() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "internal-wakeup-session";
        let turn_id = "internal-wakeup-turn";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "InternalWakeup".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("create session");
        scheduler
            .active_turns
            .insert(session_id, desktop_active_turn(turn_id));
        scheduler
            .active_internal_turns
            .insert(session_id.to_string(), ActiveInternalTurn::HiddenSubagent);

        scheduler
            .outcome_tx
            .send((
                session_id.to_string(),
                TurnOutcome::Completed {
                    turn_id: turn_id.to_string(),
                    final_response: "done".to_string(),
                },
            ))
            .await
            .expect("send outcome");

        for _ in 0..100 {
            if !scheduler.active_turns.matches_turn(session_id, turn_id) {
                // Turn consumed; the internal-turn guard must have skipped the
                // idle-wakeup restart entirely.
                assert!(scheduler
                    .goal_idle_wakeup_generations
                    .get(session_id)
                    .is_none());
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("internal turn outcome was not consumed by the outcome handler");
    }

    #[tokio::test]
    async fn submission_preflight_commits_a_persisted_revert_marker() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "reverted-session";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Reverted".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("create session");
        let storage_path = session_manager
            .effective_session_storage_path(session_id)
            .await
            .expect("storage path");
        session_manager
            .persistence_manager()
            .save_session_revert_state(
                &storage_path,
                session_id,
                &SessionRevertState {
                    schema_version: SESSION_REVERT_SCHEMA_VERSION,
                    boundary_turn: 0,
                    original_turn_end: 1,
                    phase: SessionRevertPhase::Staged,
                    workspace_checkpoint: Vec::new(),
                },
            )
            .await
            .expect("persist staged revert");

        scheduler
            .coordinator
            .commit_session_revert_before_submission(session_id)
            .await
            .expect("commit staged revert");

        assert!(session_manager
            .persistence_manager()
            .load_session_revert_state(&storage_path, session_id)
            .await
            .expect("load revert marker")
            .is_none());
        let source = include_str!("scheduler.rs");
        let submission = source
            .split_once("async fn submit_queued_turn_locked(")
            .expect("submission method")
            .1
            .split_once("async fn record_last_submitted_agent_type(")
            .expect("submission method boundary")
            .0;
        assert!(submission.contains("commit_session_revert_before_submission(&session_id)"));
    }

    #[tokio::test]
    async fn background_bash_result_injects_into_its_running_parent_turn() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "parent-session";
        let turn_id = "parent-turn";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Parent".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("create parent session");
        session_manager
            .update_session_state(
                session_id,
                SessionState::Processing {
                    current_turn_id: turn_id.to_string(),
                    phase: ProcessingPhase::Thinking,
                },
            )
            .await
            .expect("mark parent turn active");

        scheduler
            .deliver_background_result(
                session_id.to_string(),
                "agentic".to_string(),
                None,
                None,
                None,
                "Background Bash command completed".to_string(),
                None,
                Some(serde_json::json!({
                    "kind": "background_result",
                    "sourceKind": "bash_command",
                    "parentSessionId": session_id,
                    "parentDialogTurnId": turn_id,
                })),
            )
            .await
            .expect("inject background Bash result");

        let pending = scheduler
            .round_injection_monitor()
            .take_pending(session_id, turn_id);
        assert_eq!(pending.len(), 1);
        assert_eq!(scheduler.queue_depth(session_id), 0);
    }

    #[tokio::test]
    async fn idle_background_result_uses_the_session_logical_agent_route() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "external-parent-session";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "External parent".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("create parent session");

        scheduler
            .deliver_background_result(
                session_id.to_string(),
                "external::opencode::agentic::generation-v1".to_string(),
                None,
                None,
                None,
                "Background Bash command completed".to_string(),
                None,
                None,
            )
            .await
            .expect("lifecycle delivery must follow the persisted logical session route");
    }

    fn standard_queued_turn(turn_id: &str) -> QueuedTurn {
        QueuedTurn {
            user_input: "queued".to_string(),
            original_user_input: None,
            prepended_messages: Vec::new(),
            turn_id: Some(turn_id.to_string()),
            agent_type: "agentic".to_string(),
            workspace_path: Some("/workspace".to_string()),
            remote_connection_id: None,
            remote_ssh_host: None,
            policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopUi),
            reply_route: None,
            user_message_metadata: None,
            image_contexts: None,
            enqueued_at: SystemTime::now(),
            _settlement_registration: None,
            execution: QueuedTurnExecution::Standard,
        }
    }

    #[test]
    fn targeted_queue_removal_cancels_a_standard_turn_by_id() {
        let queues = DialogTurnQueue::default();
        let queued_turn = standard_queued_turn("turn-queued");
        queues
            .enqueue("session-1", queued_turn, DialogQueuePriority::Normal)
            .expect("standard turn should enqueue");

        let removed = remove_queued_turn_by_id(&queues, "session-1", "turn-queued")
            .expect("targeted cancellation should remove the queued turn");

        assert!(matches!(removed.execution, QueuedTurnExecution::Standard));
        assert_eq!(queues.depth("session-1"), 0);
    }

    #[tokio::test]
    async fn targeted_standard_queue_cancellation_emits_one_terminal_event() {
        let (scheduler, _, event_queue, _root) = test_scheduler();
        let mut events = event_queue.subscribe();
        scheduler
            .queues
            .enqueue(
                "session",
                standard_queued_turn("turn-queued"),
                DialogQueuePriority::Normal,
            )
            .expect("queue standard turn");

        assert!(scheduler
            .cancel_queued_or_active_turn("session", "turn-queued")
            .await
            .expect("cancel queued turn"));
        let event = tokio::time::timeout(Duration::from_secs(1), events.recv())
            .await
            .expect("terminal event timeout")
            .expect("terminal event");
        assert!(matches!(
            event.event,
            AgenticEvent::DialogTurnCancelled { session_id, turn_id }
                if session_id == "session" && turn_id == "turn-queued"
        ));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), events.recv())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn maintenance_does_not_release_parent_while_background_child_is_still_running() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let parent_session_id = "parent-session";
        let child_session_id = "background-child-session";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(parent_session_id.to_string()),
                "Parent".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("create parent session");
        let storage_path = session_manager
            .storage_path_binding_for_test(parent_session_id)
            .expect("parent storage binding");
        scheduler
            .coordinator
            .register_background_subagent_task_for_test(1, parent_session_id, child_session_id);
        scheduler
            .coordinator
            .set_active_turn_count_for_test(child_session_id, 1);

        let result = scheduler
            .begin_session_maintenance(parent_session_id, &storage_path, Duration::from_millis(40))
            .await;
        let error = match result {
            Ok(_) => panic!("maintenance must not detach a parent with a running child"),
            Err(error) => error,
        };

        assert!(matches!(error, BitFunError::Timeout(_)));
        assert!(error.to_string().contains(child_session_id));
        assert!(session_manager.get_session(parent_session_id).is_some());

        let retry_error = match scheduler
            .begin_session_maintenance(parent_session_id, &storage_path, Duration::from_millis(40))
            .await
        {
            Ok(_) => panic!("retry must retain ownership of the still-running child"),
            Err(error) => error,
        };
        assert!(matches!(retry_error, BitFunError::Timeout(_)));
        assert!(retry_error.to_string().contains(child_session_id));

        scheduler
            .coordinator
            .set_active_turn_count_for_test(child_session_id, 0);
        let maintenance = scheduler
            .begin_session_maintenance(parent_session_id, &storage_path, Duration::from_millis(40))
            .await
            .expect("maintenance should succeed after the child drains");
        drop(maintenance);
        assert!(!scheduler
            .maintenance_background_sessions
            .contains_key(parent_session_id));
    }

    #[test]
    fn queued_submission_without_started_turn_reports_queued() {
        assert_eq!(
            queued_submission_outcome("session".to_string(), "turn-submitted".to_string(), None,),
            DialogSubmitOutcome::Queued {
                session_id: "session".to_string(),
                turn_id: "turn-submitted".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn dialog_port_preserves_not_found_for_a_missing_session() {
        let (scheduler, _, _, root) = test_scheduler();
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");

        let error = scheduler
            .submit_dialog_turn(AgentDialogTurnRequest {
                session_id: "missing-session".to_string(),
                message: "hello".to_string(),
                original_message: None,
                turn_id: Some("missing-turn".to_string()),
                execution: Default::default(),
                agent_type: "agentic".to_string(),
                workspace_path: Some(workspace.to_string_lossy().to_string()),
                remote_connection_id: None,
                remote_ssh_host: None,
                policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
                reply_route: None,
                prepended_reminders: Vec::new(),
                attachments: Vec::new(),
                metadata: serde_json::Map::new(),
            })
            .await
            .expect_err("a missing session must remain distinguishable");

        assert_eq!(error.kind, PortErrorKind::NotFound);
        assert!(error.message.contains("missing-session"), "{error}");
        assert!(matches!(
            scheduler
                .coordinator
                .wait_for_turn_settlement(
                    "missing-session",
                    "missing-turn",
                    Duration::from_millis(10),
                )
                .await,
            Err(BitFunError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn dialog_port_tracks_settlement_from_queue_admission_through_cancellation() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "queued-session";
        let turn_id = "queued-turn";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Queued".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("create queued session");
        session_manager
            .update_session_state(
                session_id,
                SessionState::Processing {
                    current_turn_id: "active-turn".to_string(),
                    phase: ProcessingPhase::Thinking,
                },
            )
            .await
            .expect("mark another turn active");

        let outcome = scheduler
            .submit_dialog_turn(AgentDialogTurnRequest {
                session_id: session_id.to_string(),
                message: "queued prompt".to_string(),
                original_message: None,
                turn_id: Some(turn_id.to_string()),
                execution: Default::default(),
                agent_type: "agentic".to_string(),
                workspace_path: None,
                remote_connection_id: None,
                remote_ssh_host: None,
                policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
                reply_route: None,
                prepended_reminders: Vec::new(),
                attachments: Vec::new(),
                metadata: serde_json::Map::new(),
            })
            .await
            .expect("queue the submitted turn");

        assert_eq!(
            outcome,
            DialogSubmitOutcome::Queued {
                session_id: session_id.to_string(),
                turn_id: turn_id.to_string(),
            }
        );
        assert!(matches!(
            scheduler
                .coordinator
                .wait_for_turn_settlement(session_id, turn_id, Duration::from_millis(10))
                .await,
            Err(BitFunError::Timeout(_))
        ));

        assert!(scheduler
            .cancel_queued_or_active_turn(session_id, turn_id)
            .await
            .expect("cancel queued turn"));
        scheduler
            .coordinator
            .wait_for_turn_settlement(session_id, turn_id, Duration::from_millis(10))
            .await
            .expect("cancelled queued turn should settle");
    }

    #[tokio::test]
    async fn delegated_dialog_turn_rejects_instead_of_queueing_behind_an_active_turn() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "delegated-busy-session";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Delegated".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("create delegated session");
        session_manager
            .update_session_state(
                session_id,
                SessionState::Processing {
                    current_turn_id: "active-turn".to_string(),
                    phase: ProcessingPhase::Thinking,
                },
            )
            .await
            .expect("mark active turn");

        let error = scheduler
            .submit_dialog_turn(AgentDialogTurnRequest {
                session_id: session_id.to_string(),
                message: "expanded command prompt".to_string(),
                original_message: Some("/review".to_string()),
                turn_id: Some("delegated-turn".to_string()),
                execution: bitfun_runtime_ports::AgentDialogTurnExecution::FreshExternalSubagent {
                    ecosystem_id: "opencode".to_string(),
                    logical_id: "reviewer".to_string(),
                },
                agent_type: "agentic".to_string(),
                workspace_path: None,
                remote_connection_id: None,
                remote_ssh_host: None,
                policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
                reply_route: None,
                prepended_reminders: Vec::new(),
                attachments: Vec::new(),
                metadata: serde_json::Map::new(),
            })
            .await
            .expect_err("delegated commands must not queue behind another turn");

        assert_eq!(error.kind, PortErrorKind::InvalidRequest);
        assert!(error.message.contains("idle session"), "{error}");
        assert_eq!(scheduler.queue_depth(session_id), 0);
    }

    #[tokio::test]
    async fn delegated_dialog_turn_does_not_clear_a_queue_from_an_error_session() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "delegated-error-session";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Delegated".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("create delegated session");
        session_manager
            .update_session_state(
                session_id,
                SessionState::Processing {
                    current_turn_id: "active-turn".to_string(),
                    phase: ProcessingPhase::Thinking,
                },
            )
            .await
            .expect("mark active turn");

        scheduler
            .submit_dialog_turn(AgentDialogTurnRequest {
                session_id: session_id.to_string(),
                message: "queued prompt".to_string(),
                original_message: None,
                turn_id: Some("queued-turn".to_string()),
                execution: Default::default(),
                agent_type: "agentic".to_string(),
                workspace_path: None,
                remote_connection_id: None,
                remote_ssh_host: None,
                policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
                reply_route: None,
                prepended_reminders: Vec::new(),
                attachments: Vec::new(),
                metadata: serde_json::Map::new(),
            })
            .await
            .expect("queue standard turn");
        session_manager
            .update_session_state(
                session_id,
                SessionState::Error {
                    error: "previous turn failed".to_string(),
                    recoverable: true,
                },
            )
            .await
            .expect("mark session recoverable error");

        let error = scheduler
            .submit_dialog_turn(AgentDialogTurnRequest {
                session_id: session_id.to_string(),
                message: "expanded command prompt".to_string(),
                original_message: Some("/review".to_string()),
                turn_id: Some("delegated-turn".to_string()),
                execution: bitfun_runtime_ports::AgentDialogTurnExecution::FreshExternalSubagent {
                    ecosystem_id: "opencode".to_string(),
                    logical_id: "reviewer".to_string(),
                },
                agent_type: "agentic".to_string(),
                workspace_path: None,
                remote_connection_id: None,
                remote_ssh_host: None,
                policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
                reply_route: None,
                prepended_reminders: Vec::new(),
                attachments: Vec::new(),
                metadata: serde_json::Map::new(),
            })
            .await
            .expect_err("delegated commands must not replace a queued turn after an error");

        assert_eq!(error.kind, PortErrorKind::InvalidRequest);
        assert!(error.message.contains("idle"), "{error}");
        assert_eq!(scheduler.queue_depth(session_id), 1);
        assert!(scheduler
            .cancel_queued_or_active_turn(session_id, "queued-turn")
            .await
            .expect("cancel preserved queued turn"));
    }

    #[tokio::test]
    async fn reject_busy_dialog_port_does_not_enqueue_or_replace_the_active_turn() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "acp-session";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "ACP".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("create ACP session");
        session_manager
            .update_session_state(
                session_id,
                SessionState::Processing {
                    current_turn_id: "active-turn".to_string(),
                    phase: ProcessingPhase::Thinking,
                },
            )
            .await
            .expect("mark active turn");

        let error = scheduler
            .submit_agent_dialog_turn_reject_if_busy(AgentDialogTurnRequest {
                session_id: session_id.to_string(),
                message: "second prompt".to_string(),
                original_message: None,
                turn_id: Some("rejected-turn".to_string()),
                execution: Default::default(),
                agent_type: "agentic".to_string(),
                workspace_path: None,
                remote_connection_id: None,
                remote_ssh_host: None,
                policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
                reply_route: None,
                prepended_reminders: Vec::new(),
                attachments: Vec::new(),
                metadata: serde_json::Map::new(),
            })
            .await
            .expect_err("busy ACP prompt must be rejected");

        assert_eq!(error.kind, PortErrorKind::Backend);
        assert!(error.message.contains("Processing"), "{error}");
        assert_eq!(scheduler.queue_depth(session_id), 0);
        assert!(matches!(
            session_manager
                .get_session(session_id)
                .expect("session")
                .state,
            SessionState::Processing { current_turn_id, .. } if current_turn_id == "active-turn"
        ));
        assert!(matches!(
            scheduler
                .coordinator
                .wait_for_turn_settlement(session_id, "rejected-turn", Duration::from_millis(10),)
                .await,
            Err(BitFunError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn dialog_port_rejects_duplicate_active_turn_id() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "duplicate-active-session";
        let turn_id = "duplicate-turn";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Duplicate".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("create session");
        let _active_registration = scheduler
            .coordinator
            .register_turn_settlement(session_id, turn_id);
        session_manager
            .update_session_state(
                session_id,
                SessionState::Processing {
                    current_turn_id: turn_id.to_string(),
                    phase: ProcessingPhase::Thinking,
                },
            )
            .await
            .expect("mark active turn");

        let error = scheduler
            .submit_dialog_turn(AgentDialogTurnRequest {
                session_id: session_id.to_string(),
                message: "duplicate".to_string(),
                original_message: None,
                turn_id: Some(turn_id.to_string()),
                execution: Default::default(),
                agent_type: "agentic".to_string(),
                workspace_path: None,
                remote_connection_id: None,
                remote_ssh_host: None,
                policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
                reply_route: None,
                prepended_reminders: Vec::new(),
                attachments: Vec::new(),
                metadata: serde_json::Map::new(),
            })
            .await
            .expect_err("duplicate active turn ID must be rejected");

        assert_eq!(error.kind, PortErrorKind::InvalidRequest);
    }

    #[tokio::test]
    async fn dialog_port_preserves_invalid_request_for_wrong_workspace() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "workspace-bound-session";
        let turn_id = "wrong-workspace-turn";
        let workspace_a = root.path().join("workspace-a");
        let workspace_b = root.path().join("workspace-b");
        std::fs::create_dir_all(&workspace_a).expect("workspace a");
        std::fs::create_dir_all(&workspace_b).expect("workspace b");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Workspace".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_a.to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("create session");
        let error = scheduler
            .submit_dialog_turn(AgentDialogTurnRequest {
                session_id: session_id.to_string(),
                message: "wrong workspace".to_string(),
                original_message: None,
                turn_id: Some(turn_id.to_string()),
                execution: Default::default(),
                agent_type: "agentic".to_string(),
                workspace_path: Some(workspace_b.to_string_lossy().to_string()),
                remote_connection_id: None,
                remote_ssh_host: None,
                policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
                reply_route: None,
                prepended_reminders: Vec::new(),
                attachments: Vec::new(),
                metadata: serde_json::Map::new(),
            })
            .await
            .expect_err("wrong workspace must be rejected");

        assert_eq!(error.kind, PortErrorKind::InvalidRequest);
        assert!(matches!(
            scheduler
                .coordinator
                .wait_for_turn_settlement(session_id, turn_id, Duration::from_millis(10))
                .await,
            Err(BitFunError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn dialog_port_treats_unknown_agent_as_invalid_request() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "invalid-agent-session";
        let turn_id = "invalid-agent-turn";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Invalid agent".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("create session");

        let error = scheduler
            .submit_dialog_turn(AgentDialogTurnRequest {
                session_id: session_id.to_string(),
                message: "invalid agent".to_string(),
                original_message: None,
                turn_id: Some(turn_id.to_string()),
                execution: Default::default(),
                agent_type: "agent-that-does-not-exist".to_string(),
                workspace_path: None,
                remote_connection_id: None,
                remote_ssh_host: None,
                policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
                reply_route: None,
                prepended_reminders: Vec::new(),
                attachments: Vec::new(),
                metadata: serde_json::Map::new(),
            })
            .await
            .expect_err("unknown agent must be rejected");

        assert_eq!(error.kind, PortErrorKind::InvalidRequest);
    }

    #[tokio::test]
    async fn missing_settlement_evidence_for_known_turn_fails_closed() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "known-turn-session";
        let turn_id = "known-turn";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Known turn".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("create session");
        session_manager
            .start_dialog_turn(
                session_id,
                "agentic".to_string(),
                "hello".to_string(),
                Some(turn_id.to_string()),
                None,
                None,
            )
            .await
            .expect("record turn");
        session_manager
            .update_session_state(session_id, SessionState::Idle)
            .await
            .expect("mark idle");

        let error = scheduler
            .coordinator
            .wait_for_turn_settlement(session_id, turn_id, Duration::from_millis(10))
            .await
            .expect_err("missing settlement evidence must not be treated as success");

        assert!(matches!(error, BitFunError::Service(_)), "{error}");
    }

    fn desktop_active_turn(turn_id: &str) -> ActiveDialogTurn {
        ActiveDialogTurn::new(
            turn_id.to_string(),
            Some("/workspace".to_string()),
            None,
            None,
            "agentic".to_string(),
            "hello".to_string(),
            None,
            DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopUi),
            None,
        )
    }

    async fn mark_session_processing(
        session_manager: &SessionManager,
        root: &tempfile::TempDir,
        session_id: &str,
        turn_id: &str,
    ) {
        let workspace = root.path().join(format!("workspace-{session_id}"));
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Steering".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("create session");
        session_manager
            .update_session_state(
                session_id,
                SessionState::Processing {
                    current_turn_id: turn_id.to_string(),
                    phase: ProcessingPhase::Thinking,
                },
            )
            .await
            .expect("mark turn active");
    }

    #[tokio::test]
    async fn steering_rejects_stale_processing_state_without_authoritative_active_turn() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "stale-steering-session";
        let turn_id = "stale-turn";
        mark_session_processing(&session_manager, &root, session_id, turn_id).await;

        let error = scheduler
            .buffer_steering(
                session_id.to_string(),
                turn_id.to_string(),
                "check tests".to_string(),
                None,
                Vec::new(),
            )
            .await
            .expect_err("stale processing state must not accept steering");

        assert!(error.contains("no longer running"), "{error}");
        assert!(scheduler
            .round_injection_monitor()
            .take_pending(session_id, turn_id)
            .is_empty());
    }

    #[tokio::test]
    async fn steering_rejects_empty_content_as_an_invalid_request() {
        let (scheduler, _, _, _) = test_scheduler();

        let error = AgentDialogTurnPort::steer_dialog_turn(
            scheduler.as_ref(),
            AgentDialogSteerRequest {
                session_id: "session-1".to_string(),
                turn_id: "turn-1".to_string(),
                content: "  ".to_string(),
                display_content: None,
                prepended_reminders: Vec::new(),
            },
        )
        .await
        .expect_err("empty steering must fail");

        assert_eq!(error.kind, PortErrorKind::InvalidRequest);
    }

    #[tokio::test]
    async fn steering_serializes_with_other_operations_for_the_same_session() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "locked-steering-session";
        let turn_id = "active-turn";
        mark_session_processing(&session_manager, &root, session_id, turn_id).await;
        scheduler
            .active_turns
            .insert(session_id, desktop_active_turn(turn_id));

        let operation_guard = scheduler.lock_session_operation(session_id).await;
        let steering_scheduler = scheduler.clone();
        let steering = tokio::spawn(async move {
            steering_scheduler
                .buffer_steering(
                    session_id.to_string(),
                    turn_id.to_string(),
                    "check tests".to_string(),
                    None,
                    Vec::new(),
                )
                .await
        });
        tokio::task::yield_now().await;

        assert!(
            !steering.is_finished(),
            "steering must wait for the session operation lock"
        );
        drop(operation_guard);
        steering
            .await
            .expect("steering task")
            .expect("steering outcome");
    }

    #[tokio::test]
    async fn explicit_cancel_cannot_cross_session_by_reusing_a_turn_id() {
        let (scheduler, _, _, _root) = test_scheduler();
        scheduler
            .active_turns
            .insert("session-a", desktop_active_turn("shared-turn"));

        let removed = scheduler
            .cancel_queued_or_active_turn("session-b", "shared-turn")
            .await
            .expect("stale cancellation is idempotent");

        assert!(!removed);
        assert!(scheduler
            .active_turns
            .matches_turn("session-a", "shared-turn"));
    }

    #[tokio::test]
    async fn wrong_workspace_deletion_leaves_active_and_queued_turns_untouched() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "session-bound-to-a";
        let storage_a = root.path().join("workspace-a-sessions");
        let storage_b = root.path().join("workspace-b-sessions");
        session_manager
            .ensure_session_storage_path(session_id, &storage_a)
            .expect("bind session storage");
        scheduler
            .queues
            .enqueue(
                session_id,
                standard_queued_turn("turn-queued"),
                DialogQueuePriority::Normal,
            )
            .expect("queue turn");
        scheduler
            .active_turns
            .insert(session_id, desktop_active_turn("turn-active"));

        let error = scheduler
            .begin_session_deletion(session_id, &storage_b, Duration::ZERO)
            .await
            .err()
            .expect("wrong workspace must be rejected before quiescence");

        assert!(matches!(error, BitFunError::Validation(_)));
        assert_eq!(scheduler.queue_depth(session_id), 1);
        assert!(scheduler
            .active_turns
            .matches_turn(session_id, "turn-active"));
    }

    #[tokio::test]
    async fn maintenance_retires_scheduler_state_even_when_core_cancel_returns_a_turn_id() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "session-maintenance-retire";
        let turn_id = "turn-active";
        let workspace = root.path().join("workspace-maintenance-retire");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Maintenance retire".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("create session");
        session_manager
            .update_session_state(
                session_id,
                SessionState::Processing {
                    current_turn_id: turn_id.to_string(),
                    phase: ProcessingPhase::ToolCalling,
                },
            )
            .await
            .expect("mark processing");
        scheduler
            .active_turns
            .insert(session_id, desktop_active_turn(turn_id));
        let storage_path = session_manager
            .storage_path_binding_for_test(session_id)
            .expect("storage binding");

        let maintenance = scheduler
            .begin_session_maintenance(session_id, &storage_path, Duration::from_secs(1))
            .await
            .expect("maintenance");

        assert_eq!(maintenance.retired_turn_ids(), &[turn_id.to_string()]);
        assert!(!scheduler.active_turns.matches_turn(session_id, turn_id));
        assert!(take_active_turn_for_outcome(
            &scheduler.active_turns,
            &scheduler.retired_maintenance_outcomes,
            session_id,
            turn_id,
        )
        .is_none());
    }

    #[test]
    fn retired_maintenance_outcome_cannot_mutate_a_recreated_session_generation() {
        let active_turns = ActiveDialogTurnStore::default();
        let retired = DialogReplySuppressionSet::default();
        let session_id = "reused-session";
        active_turns.insert(session_id, desktop_active_turn("turn-old"));
        let old = active_turns
            .remove(session_id)
            .expect("old active turn should be present");
        retired.mark(session_id, old.turn_id());
        active_turns.insert(session_id, desktop_active_turn("turn-new"));

        assert!(
            take_active_turn_for_outcome(&active_turns, &retired, session_id, "turn-old").is_none()
        );
        assert!(active_turns.matches_turn(session_id, "turn-new"));
        assert!(matches!(
            take_active_turn_for_outcome(&active_turns, &retired, session_id, "turn-new"),
            Some(ActiveDialogTurnTakeResult::Matched(_))
        ));
    }

    fn agent_session_active_turn(source_session_id: &str) -> ActiveDialogTurn {
        ActiveDialogTurn::new(
            "turn_1".to_string(),
            Some("/workspace".to_string()),
            None,
            None,
            "agentic".to_string(),
            "hello".to_string(),
            None,
            DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
            Some(AgentSessionReplyRoute {
                source_session_id: source_session_id.to_string(),
                source_workspace_path: "/source".to_string(),
                source_remote_connection_id: None,
                source_remote_ssh_host: None,
            }),
        )
    }

    #[test]
    fn requester_matching_reply_route_suppresses_cancelled_reply() {
        let active_turn = agent_session_active_turn("session_a");
        assert!(active_turn.should_suppress_cancelled_reply_for_requester("session_a"));
        assert!(!active_turn.should_suppress_cancelled_reply_for_requester("session_c"));
    }

    #[test]
    fn cancelled_reply_is_skipped_only_when_suppressed() {
        let active_turn = agent_session_active_turn("session_a");
        let cancelled = TurnOutcome::Cancelled {
            turn_id: "turn_1".to_string(),
        };
        let completed = TurnOutcome::Completed {
            turn_id: "turn_1".to_string(),
            final_response: "done".to_string(),
        };

        assert_eq!(
            resolve_agent_session_reply_action(
                "session_b",
                None,
                None,
                &active_turn,
                &cancelled,
                true
            ),
            AgentSessionReplyAction::SkipSuppressedCancelledReply
        );
        assert!(matches!(
            resolve_agent_session_reply_action(
                "session_b",
                None,
                None,
                &active_turn,
                &cancelled,
                false
            ),
            AgentSessionReplyAction::Forward(_)
        ));
        assert!(matches!(
            resolve_agent_session_reply_action(
                "session_b",
                None,
                None,
                &active_turn,
                &completed,
                true
            ),
            AgentSessionReplyAction::Forward(_)
        ));
    }

    #[test]
    fn cancelled_hidden_subagent_outcome_dispatches_next_queued_turn() {
        let cancelled = TurnOutcome::Cancelled {
            turn_id: "subagent-turn-1".to_string(),
        };
        let failed = TurnOutcome::Failed {
            turn_id: "subagent-turn-1".to_string(),
            error: "provider error".to_string(),
        };

        let cancelled_plan = resolve_turn_outcome_lifecycle_plan(&cancelled, true);
        assert_eq!(
            cancelled_plan.queue_action,
            TurnOutcomeQueueAction::DispatchNext
        );

        let failed_plan = resolve_turn_outcome_lifecycle_plan(&failed, true);
        assert_eq!(failed_plan.queue_action, TurnOutcomeQueueAction::ClearQueue);
    }

    #[test]
    fn goal_verification_observation_covers_all_turn_outcomes() {
        let completed = TurnOutcome::Completed {
            turn_id: "turn_1".to_string(),
            final_response: "done".to_string(),
        };
        let cancelled = TurnOutcome::Cancelled {
            turn_id: "turn_2".to_string(),
        };
        let failed = TurnOutcome::Failed {
            turn_id: "turn_3".to_string(),
            error: "network offline".to_string(),
        };

        assert_eq!(completed.reply_text(), "done");
        assert!(cancelled.reply_text().contains("cancelled"));
        assert!(failed.reply_text().contains("network offline"));
    }

    #[test]
    fn remote_queue_policy_preserves_priority_boundary() {
        let remote = DialogSubmissionPolicy::for_source(DialogTriggerSource::RemoteRelay);
        assert_eq!(remote.queue_priority, DialogQueuePriority::Normal);

        let bot = DialogSubmissionPolicy::for_source(DialogTriggerSource::Bot);
        assert_eq!(bot.queue_priority, DialogQueuePriority::Normal);

        let agent_session = DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession);
        assert_eq!(agent_session.queue_priority, DialogQueuePriority::Low);
    }

    #[test]
    fn agent_dialog_turn_attachments_preserve_remote_image_context() {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "dataUrl".to_string(),
            serde_json::json!("data:image/jpeg;base64,abc"),
        );
        metadata.insert("mimeType".to_string(), serde_json::json!("image/jpeg"));
        metadata.insert(
            "metadata".to_string(),
            serde_json::json!({ "name": "clip.jpg", "source": "remote" }),
        );

        let contexts = agent_dialog_turn_image_contexts(&[AgentInputAttachment {
            kind: "remote_image".to_string(),
            id: "ctx-1".to_string(),
            metadata,
        }])
        .expect("remote image attachment should be supported")
        .expect("non-empty image contexts");

        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].id, "ctx-1");
        assert_eq!(
            contexts[0].data_url.as_deref(),
            Some("data:image/jpeg;base64,abc")
        );
        assert_eq!(contexts[0].mime_type, "image/jpeg");
        assert_eq!(
            contexts[0]
                .metadata
                .as_ref()
                .and_then(|value| value.get("name")),
            Some(&serde_json::json!("clip.jpg"))
        );
    }

    #[test]
    fn agent_dialog_turn_attachments_reject_unknown_kind() {
        let err = agent_dialog_turn_image_contexts(&[AgentInputAttachment {
            kind: "unknown".to_string(),
            id: "attachment-1".to_string(),
            metadata: serde_json::Map::new(),
        }])
        .expect_err("unsupported attachment kind must be explicit");

        assert_eq!(err.kind, PortErrorKind::InvalidRequest);
        assert!(err
            .message
            .contains("unsupported agent dialog attachment kind"));
    }

    #[test]
    fn agent_dialog_turn_prepended_reminders_preserve_session_message_kind() {
        let messages = agent_dialog_turn_prepended_messages(&[AgentDialogPrependedReminder {
            kind: "session_message_request".to_string(),
            text: "sent by another agent".to_string(),
        }])
        .expect("session message reminder should be supported");

        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].internal_reminder_kind(),
            Some(InternalReminderKind::SessionMessageRequest)
        );
    }

    #[test]
    fn agent_dialog_turn_prepended_reminders_preserve_scheduled_job_kind() {
        let messages = agent_dialog_turn_prepended_messages(&[AgentDialogPrependedReminder {
            kind: "scheduled_job".to_string(),
            text: "scheduled job trigger".to_string(),
        }])
        .expect("scheduled job reminder should be supported");

        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].internal_reminder_kind(),
            Some(InternalReminderKind::ScheduledJob)
        );
    }

    #[test]
    fn agent_dialog_turn_prepended_reminders_reject_unknown_kind() {
        let err = agent_dialog_turn_prepended_messages(&[AgentDialogPrependedReminder {
            kind: "unknown".to_string(),
            text: "unsupported".to_string(),
        }])
        .expect_err("unsupported reminder kind must be explicit");

        assert_eq!(err.kind, PortErrorKind::InvalidRequest);
        assert!(err
            .message
            .contains("unsupported agent dialog prepended reminder kind"));
    }

    // ---------------------------------------------------------------------
    // Plan-todo binding hooks (integration-level): verify the scheduler
    // wiring (reply_route.is_some() gates) all the way to the on-disk plan
    // file. The pure binding logic itself lives in plan_todo_binding.rs; these
    // tests cover the scheduler-side hook trigger points:
    //   - start_turn with a binding + reply_route marks the todo in_progress
    //   - a Completed outcome marks the bound todo completed
    //   - Failed/Cancelled outcomes keep the todo pending
    //   - reply turns (reply_route = None) never trigger either hook
    // ---------------------------------------------------------------------

    fn write_bound_plan_file(root: &tempfile::TempDir, file_name: &str) -> (PathBuf, String) {
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let plan_path = workspace.join(file_name);
        let plan_file = plan_path.to_string_lossy().into_owned();
        std::fs::write(
            &plan_path,
            "---\nname: My Plan\noverview: An overview\ntodos:\n- id: setup-auth\n  content: Set up auth\n  status: pending\n---\n\n# My Plan\n\nBody text here.\n",
        )
        .expect("write plan file");
        (plan_path, plan_file)
    }

    fn plan_todo_status(plan_path: &Path) -> String {
        let content = std::fs::read_to_string(plan_path).expect("read plan file");
        let status_line = content
            .lines()
            .find(|line| line.trim_start().starts_with("status:"))
            .expect("plan todo status line");
        status_line
            .split_once("status:")
            .expect("status separator")
            .1
            .trim()
            .to_string()
    }

    fn binding_metadata(plan_file: &str) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "planFile": plan_file,
            "todoId": "setup-auth",
        }))
    }

    fn bound_active_turn(
        turn_id: &str,
        workspace_path: &str,
        plan_file: &str,
        reply_route: Option<AgentSessionReplyRoute>,
    ) -> ActiveDialogTurn {
        ActiveDialogTurn::new(
            turn_id.to_string(),
            Some(workspace_path.to_string()),
            None,
            None,
            "agentic".to_string(),
            "bound execution turn".to_string(),
            binding_metadata(plan_file),
            DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
            reply_route,
        )
    }

    fn sample_reply_route() -> AgentSessionReplyRoute {
        AgentSessionReplyRoute {
            source_session_id: "source-session".to_string(),
            source_workspace_path: "/workspace".to_string(),
            source_remote_connection_id: None,
            source_remote_ssh_host: None,
        }
    }

    async fn create_bound_session(
        session_manager: &SessionManager,
        root: &tempfile::TempDir,
        session_id: &str,
    ) -> String {
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Bound".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("create bound session");
        workspace.to_string_lossy().into_owned()
    }

    async fn wait_for_active_turn_consumed(
        scheduler: &DialogScheduler,
        session_id: &str,
        turn_id: &str,
    ) {
        for _ in 0..100 {
            if !scheduler.active_turns.matches_turn(session_id, turn_id) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("active turn was not consumed by the outcome handler: session_id={session_id}, turn_id={turn_id}");
    }

    /// The in_progress hook function itself, exercised against a real plan
    /// file: binding metadata + workspace resolve the plan path and rewrite
    /// the todo status on disk. (The scheduler-side gate that calls this hook
    /// from start_turn is covered by
    /// `start_turn_binding_hook_wiring_is_gated_on_reply_route`; the full
    /// start_turn pipeline is not reachable in the test harness because it
    /// resolves session storage through the global PathManager.)
    #[tokio::test]
    async fn in_progress_hook_direct_call_marks_real_plan_file() {
        let root = tempfile::tempdir().expect("test root");
        let workspace_path = root
            .path()
            .join("workspace")
            .to_string_lossy()
            .into_owned();
        let (plan_path, plan_file) = write_bound_plan_file(&root, "hook_in_progress_plan.plan.md");

        let _override_guard =
            PathManager::set_plans_dir_override_guard(root.path().join("workspace"));

        auto_mark_todo_in_progress_if_bound(
            binding_metadata(&plan_file).as_ref(),
            Some(&workspace_path),
            None,
            None,
        )
        .await;

        assert_eq!(plan_todo_status(&plan_path), "in_progress");
    }

    /// Source-level wiring assertion (same pattern as
    /// `submission_preflight_commits_a_persisted_revert_marker` above): the
    /// start_turn in_progress hook must exist and must be gated on
    /// `reply_route.is_some()` so reply turns (reply_route = None) never
    /// trigger it. The full start_turn pipeline is not runnable in the test
    /// harness (global PathManager storage resolution), so the wiring itself
    /// is pinned against the source.
    #[test]
    fn start_turn_binding_hook_wiring_is_gated_on_reply_route() {
        let source = include_str!("scheduler.rs");
        let start_turn = source
            .split_once("async fn start_turn(")
            .expect("start_turn method")
            .1
            .split_once("async fn start_hidden_subagent_turn(")
            .expect("start_turn boundary")
            .0;
        let gate_pos = start_turn
            .find("if queued_turn.reply_route.is_some() {")
            .expect("reply_route gate");
        let hook_pos = start_turn
            .find("auto_mark_todo_in_progress_if_bound(")
            .expect("in_progress hook call");
        assert!(
            gate_pos < hook_pos,
            "in_progress hook must be gated on reply_route.is_some()"
        );
        assert!(
            start_turn.contains("// in_progress. Only execution turns (reply_route.is_some()) can carry"),
            "missing gate comment explaining the reply_route condition"
        );
    }

    #[tokio::test]
    async fn bound_execution_turn_completed_marks_todo_completed() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "bound-complete-session";
        let workspace_path = create_bound_session(&session_manager, &root, session_id).await;
        let (plan_path, plan_file) = write_bound_plan_file(&root, "bound_complete_plan.plan.md");
        let turn_id = "bound-complete-turn";
        scheduler.active_turns.insert(
            session_id,
            bound_active_turn(
                turn_id,
                &workspace_path,
                &plan_file,
                Some(sample_reply_route()),
            ),
        );

        let _override_guard =
            PathManager::set_plans_dir_override_guard(PathBuf::from(&workspace_path));

        scheduler
            .outcome_tx
            .send((
                session_id.to_string(),
                TurnOutcome::Completed {
                    turn_id: turn_id.to_string(),
                    final_response: "done".to_string(),
                },
            ))
            .await
            .expect("send completed outcome");

        for _ in 0..100 {
            if plan_todo_status(&plan_path) == "completed" {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("bound todo was not marked completed");
    }

    #[tokio::test]
    async fn bound_execution_turn_failed_keeps_todo_pending() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "bound-failed-session";
        let workspace_path = create_bound_session(&session_manager, &root, session_id).await;
        let (plan_path, plan_file) = write_bound_plan_file(&root, "bound_failed_plan.plan.md");
        let turn_id = "bound-failed-turn";
        scheduler.active_turns.insert(
            session_id,
            bound_active_turn(
                turn_id,
                &workspace_path,
                &plan_file,
                Some(sample_reply_route()),
            ),
        );

        scheduler
            .outcome_tx
            .send((
                session_id.to_string(),
                TurnOutcome::Failed {
                    turn_id: turn_id.to_string(),
                    error: "boom".to_string(),
                },
            ))
            .await
            .expect("send failed outcome");

        wait_for_active_turn_consumed(&scheduler, session_id, turn_id).await;
        assert_eq!(plan_todo_status(&plan_path), "pending");
    }

    #[tokio::test]
    async fn bound_execution_turn_cancelled_keeps_todo_pending() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "bound-cancelled-session";
        let workspace_path = create_bound_session(&session_manager, &root, session_id).await;
        let (plan_path, plan_file) = write_bound_plan_file(&root, "bound_cancelled_plan.plan.md");
        let turn_id = "bound-cancelled-turn";
        scheduler.active_turns.insert(
            session_id,
            bound_active_turn(
                turn_id,
                &workspace_path,
                &plan_file,
                Some(sample_reply_route()),
            ),
        );

        scheduler
            .outcome_tx
            .send((
                session_id.to_string(),
                TurnOutcome::Cancelled {
                    turn_id: turn_id.to_string(),
                },
            ))
            .await
            .expect("send cancelled outcome");

        wait_for_active_turn_consumed(&scheduler, session_id, turn_id).await;
        assert_eq!(plan_todo_status(&plan_path), "pending");
    }

    /// A Completed reply turn (reply_route = None) must not trigger the
    /// completed hook even though the binding metadata is present. The
    /// start_turn side of the same gate (reply_route = None → in_progress
    /// hook not triggered) is covered by the source-level wiring assertion in
    /// `start_turn_binding_hook_wiring_is_gated_on_reply_route` because the
    /// full start_turn pipeline is not runnable in the test harness (global
    /// PathManager storage resolution).
    #[tokio::test]
    async fn reply_turn_without_route_never_triggers_binding_hooks() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let reply_session_id = "bound-reply-outcome-session";
        let reply_workspace_path =
            create_bound_session(&session_manager, &root, reply_session_id).await;
        let (reply_plan_path, reply_plan_file) =
            write_bound_plan_file(&root, "bound_reply_outcome_plan.plan.md");
        let reply_turn_id = "bound-reply-outcome-turn";
        scheduler.active_turns.insert(
            reply_session_id,
            bound_active_turn(
                reply_turn_id,
                &reply_workspace_path,
                &reply_plan_file,
                None,
            ),
        );
        scheduler
            .outcome_tx
            .send((
                reply_session_id.to_string(),
                TurnOutcome::Completed {
                    turn_id: reply_turn_id.to_string(),
                    final_response: "done".to_string(),
                },
            ))
            .await
            .expect("send completed reply outcome");

        wait_for_active_turn_consumed(&scheduler, reply_session_id, reply_turn_id).await;
        assert_eq!(plan_todo_status(&reply_plan_path), "pending");
    }

    // ---------------------------------------------------------------------
    // Agent-session reply best-effort archiving (F9): forwarded replies are
    // written to `<archive-root>/<YYYY-MM>/<session>-<turn>.md` with the
    // reply facts, and archive failures never block reply delivery.
    // ---------------------------------------------------------------------

    fn reply_archive_files(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for month in std::fs::read_dir(root).into_iter().flatten().flatten() {
            if !month.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            for entry in std::fs::read_dir(month.path()).into_iter().flatten().flatten() {
                if entry.path().extension().and_then(|ext| ext.to_str()) == Some("md") {
                    files.push(entry.path());
                }
            }
        }
        files
    }

    #[tokio::test]
    async fn forwarded_agent_session_reply_is_archived_with_reply_facts() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "archive-reply-session";
        let workspace_path = create_bound_session(&session_manager, &root, session_id).await;
        let (_, plan_file) = write_bound_plan_file(&root, "archive_reply_plan.plan.md");
        let turn_id = "archive-reply-turn";
        scheduler.active_turns.insert(
            session_id,
            bound_active_turn(
                turn_id,
                &workspace_path,
                &plan_file,
                Some(sample_reply_route()),
            ),
        );

        scheduler
            .outcome_tx
            .send((
                session_id.to_string(),
                TurnOutcome::Completed {
                    turn_id: turn_id.to_string(),
                    final_response: "archive this reply".to_string(),
                },
            ))
            .await
            .expect("send completed outcome");

        let archive_root = root.path().join("agent-replies");
        for _ in 0..100 {
            if !reply_archive_files(&archive_root).is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let files = reply_archive_files(&archive_root);
        assert_eq!(files.len(), 1, "exactly one reply archive must be written");
        let content = std::fs::read_to_string(&files[0]).expect("read reply archive");
        assert!(content.contains("source_session: archive-reply-session"));
        assert!(content.contains("target_session: source-session"));
        assert!(content.contains("status: completed"));
        assert!(
            content.contains("server_time: ") && !content.contains("server_time: unknown"),
            "the serverTime written into the reply metadata must be archived"
        );
        assert!(content.contains("## Reply Text"));
        assert!(content.contains("archive this reply"));
    }

    #[tokio::test]
    async fn failed_reply_archive_write_does_not_block_delivery() {
        let (scheduler, session_manager, _, root) = test_scheduler();
        // Point the archive root at an existing *file* so create_dir_all must
        // fail; delivery must still proceed past the best-effort archive.
        let blocking_file = root.path().join("blocking-file");
        std::fs::write(&blocking_file, b"not a directory").expect("write blocking file");
        scheduler.set_agent_reply_archive_root(blocking_file);
        let session_id = "archive-blocked-session";
        let workspace_path = create_bound_session(&session_manager, &root, session_id).await;
        let (_, plan_file) = write_bound_plan_file(&root, "archive_blocked_plan.plan.md");
        let turn_id = "archive-blocked-turn";
        scheduler.active_turns.insert(
            session_id,
            bound_active_turn(
                turn_id,
                &workspace_path,
                &plan_file,
                Some(sample_reply_route()),
            ),
        );

        scheduler
            .outcome_tx
            .send((
                session_id.to_string(),
                TurnOutcome::Completed {
                    turn_id: turn_id.to_string(),
                    final_response: "deliver anyway".to_string(),
                },
            ))
            .await
            .expect("send completed outcome");

        wait_for_active_turn_consumed(&scheduler, session_id, turn_id).await;
    }

    #[test]
    fn archive_id_sanitization_replaces_unsafe_characters() {
        assert_eq!(DialogScheduler::sanitize_archive_id("session-1"), "session-1");
        assert_eq!(DialogScheduler::sanitize_archive_id("../evil"), "evil");
        assert_eq!(DialogScheduler::sanitize_archive_id("a b/c"), "a_b_c");
        assert_eq!(DialogScheduler::sanitize_archive_id(""), "unknown");
        assert_eq!(DialogScheduler::sanitize_archive_id(":::"), "unknown");
        let long = "x".repeat(200);
        assert_eq!(DialogScheduler::sanitize_archive_id(&long).len(), 128);
    }

    #[tokio::test]
    async fn warden_goal_gate_follows_thread_goal_activity() {
        let (scheduler, _session_manager, _, root) = test_scheduler();
        let session_id = "warden-gate-session";
        // Create the session through the coordinator (like the coordinator
        // goal tests do) so the workspace binding resolves inside the test
        // root instead of the real user home.
        let workspace_dir = root.path().join("warden-gate-workspace");
        std::fs::create_dir_all(&workspace_dir).expect("workspace dir");
        scheduler
            .coordinator
            .create_session_with_id(
                Some(session_id.to_string()),
                "Warden gate".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_dir.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("session should load");

        // No goal yet: the Warden gate is closed, so failures of this
        // session would never accumulate consecutive-failure counts.
        assert!(!scheduler.session_has_active_goal(session_id).await);

        // A session that is not loaded has no goal and closes the gate.
        assert!(!scheduler.session_has_active_goal("missing-session").await);

        // The active-goal branch of the gate (`goal.is_active()` →
        // `warden_enforcement_for_goal`) is covered by the pure-function
        // tests in `warden::runtime::tests` and `rbac_poke_integration`;
        // persisting a real goal in this harness would write into the real
        // user home because the workspace binding resolver falls back to the
        // global PathManager (existing test-infrastructure limitation).
    }

    #[test]
    fn background_result_follow_up_returns_minimal_metadata_only() {
        // P-19：主会话通知只含极简元信息（session_id + 身份标识 + 已回复状态），
        // 不含内容全文；全文由 P-03 persist_background_acp_turn 落盘后经
        // SessionHistory(session_id) 检索。
        let notice =
            background_result_follow_up_user_input("flow-session-1", "external::opencode");
        assert!(notice.contains("flow-session-1"));
        assert!(notice.contains("external::opencode"));
        assert!(notice.contains("has replied"));
        assert!(notice.contains("use SessionHistory"));
        assert!(!notice.contains("full reply body"));
        assert!(!notice.contains("EXTERNAL_REPLY_MARKER_"));
    }

    #[test]
    fn background_result_follow_up_is_minimal_for_marker_and_full_reply() {
        // P-19：命中/非命中通知标记一律返回极简元信息，不再保留全文旁路。
        let bash_notice =
            "Background Bash command completed; use SessionHistory to view the full reply. Full output was saved to /tmp/out.txt";
        let marker_notice = background_result_follow_up_user_input("flow-session-2", "agentic");
        assert!(marker_notice.contains("flow-session-2"));
        assert!(marker_notice.contains("agentic"));
        assert!(marker_notice.contains("has replied"));
        // 通知式摘要标记内容不再保留为旁路：极简元信息与原文不同且不含原文。
        assert_ne!(marker_notice, bash_notice);
        assert!(!marker_notice.contains("Full output was saved"));
        assert!(!marker_notice.contains("/tmp/out.txt"));
    }

    #[test]
    fn background_result_follow_up_text_is_deterministic() {
        // 缓存前缀稳定性：同一 (session_id, agent_type) 的 follow-up 文本必须
        // 逐字节一致——通知合并后同类场景使用相同文本，杜绝时序抖动变体。
        let first = background_result_follow_up_user_input("flow-session-3", "agentic");
        let second = background_result_follow_up_user_input("flow-session-3", "agentic");
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn duplicate_background_result_follow_up_is_coalesced() {
        // Token 风暴守卫：同一会话、相同 follow-up 文本的重复提交必须被
        // 队列查重吸收，只保留一个 turn（一次模型请求）。
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "coalesce-follow-up-session";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "Coalesce".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("create session");

        // 第一次提交：入队成功。
        scheduler
            .deliver_background_result(
                session_id.to_string(),
                "agentic".to_string(),
                None,
                None,
                None,
                "Background task completed".to_string(),
                None,
                None,
            )
            .await
            .expect("first follow-up accepted");
        let depth_after_first = scheduler.queue_depth(session_id);

        // 第二次提交（同 session、同 agent_type → 同 follow-up 文本）：去重跳过。
        scheduler
            .deliver_background_result(
                session_id.to_string(),
                "agentic".to_string(),
                None,
                None,
                None,
                "Background task completed".to_string(),
                None,
                None,
            )
            .await
            .expect("second follow-up deduplicated");
        assert_eq!(
            scheduler.queue_depth(session_id),
            depth_after_first,
            "duplicate follow-up must not add another queued turn"
        );
    }

    #[tokio::test]
    async fn duplicate_background_notification_is_coalesced_across_submit_routes() {
        // 主人裁决：后台通知 = 必要功能，修的是"通知风暴"。本测试验证咽喉级
        // 查重覆盖两条提交路径（scheduler follow-up + coordinator 直提），
        // 且不同子代理（不同 session_id → 不同通知文本）各自保留。
        let (scheduler, session_manager, _, root) = test_scheduler();
        let session_id = "coalesce-throat-session";
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        session_manager
            .create_session_with_id(
                Some(session_id.to_string()),
                "CoalesceThroat".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("create session");

        // 同一条后台通知（同 child session + 同 agent_type → 同文本）经两条
        // 路径各提交一次：第一次 Started，重复提交必须被查重拦截（队列深度
        // 不变，不启动第二个 turn）。
        let notice = background_result_follow_up_user_input("child-session-1", "GeneralPurpose");
        let first_outcome = scheduler
            .submit_queued_turn(
                session_id.to_string(),
                "throat-turn-1".to_string(),
                QueuedTurn {
                    user_input: notice.clone(),
                    original_user_input: None,
                    prepended_messages: Vec::new(),
                    turn_id: Some("throat-turn-1".to_string()),
                    agent_type: "GeneralPurpose".to_string(),
                    workspace_path: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                    reply_route: None,
                    user_message_metadata: None,
                    image_contexts: None,
                    enqueued_at: SystemTime::now(),
                    _settlement_registration: None,
                    execution: QueuedTurnExecution::Standard,
                },
                false,
            )
            .await
            .expect("first route accepted");
        assert!(matches!(first_outcome, DialogSubmitOutcome::Started { .. }));
        let depth_after_first = scheduler.queue_depth(session_id);

        // 重复提交：查重拦截，队列深度不变。
        let second_outcome = scheduler
            .submit_queued_turn(
                session_id.to_string(),
                "throat-turn-2".to_string(),
                QueuedTurn {
                    user_input: notice.clone(),
                    original_user_input: None,
                    prepended_messages: Vec::new(),
                    turn_id: Some("throat-turn-2".to_string()),
                    agent_type: "GeneralPurpose".to_string(),
                    workspace_path: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                    reply_route: None,
                    user_message_metadata: None,
                    image_contexts: None,
                    enqueued_at: SystemTime::now(),
                    _settlement_registration: None,
                    execution: QueuedTurnExecution::Standard,
                },
                false,
            )
            .await
            .expect("second route accepted (coalesced)");
        assert!(
            matches!(second_outcome, DialogSubmitOutcome::Queued { .. }),
            "duplicate notification across routes must be coalesced: {second_outcome:?}"
        );
        assert_eq!(
            scheduler.queue_depth(session_id),
            depth_after_first,
            "duplicate notification must not add a queued turn"
        );

        // 不同子代理 → 不同通知文本 → 正常入队保留（通知功能不丢失）。
        let other_notice =
            background_result_follow_up_user_input("child-session-2", "GeneralPurpose");
        let third_outcome = scheduler
            .submit_queued_turn(
                session_id.to_string(),
                "throat-turn-3".to_string(),
                QueuedTurn {
                    user_input: other_notice,
                    original_user_input: None,
                    prepended_messages: Vec::new(),
                    turn_id: Some("throat-turn-3".to_string()),
                    agent_type: "GeneralPurpose".to_string(),
                    workspace_path: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                    reply_route: None,
                    user_message_metadata: None,
                    image_contexts: None,
                    enqueued_at: SystemTime::now(),
                    _settlement_registration: None,
                    execution: QueuedTurnExecution::Standard,
                },
                false,
            )
            .await
            .expect("distinct notification accepted");
        assert!(
            matches!(third_outcome, DialogSubmitOutcome::Queued { .. }),
            "distinct child notification must be accepted (queued behind the running turn): {third_outcome:?}"
        );
        assert_eq!(
            scheduler.queue_depth(session_id),
            depth_after_first + 1,
            "distinct child notifications must both be retained"
        );
    }

    #[test]
    fn background_notice_detector_matches_fixed_template_only() {
        let notice = background_result_follow_up_user_input("child-session-1", "GeneralPurpose");
        assert!(is_background_result_follow_up(&notice));
        // 真实用户消息绝不能被误判为后台通知。
        assert!(!is_background_result_follow_up("check tests"));
        assert!(!is_background_result_follow_up("Background agent session foo"));
    }
}
