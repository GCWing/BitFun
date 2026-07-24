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
    ConversationCoordinator, DialogTriggerSource, HiddenSubagentExecutionRequest, SubagentResult,
};
use super::turn_outcome::TurnOutcome;
use super::turn_settlement::TurnSettlementRegistration;
use crate::agentic::core::{InternalReminderKind, Message, SessionState};
use crate::agentic::events::AgenticEvent;
use crate::agentic::goal_mode::{
    goal_continuation_submit_retry_delay_ms, goal_internal_context_message,
    goal_objective_updated_message,
};
use crate::agentic::image_analysis::ImageContextData;
use crate::agentic::init_agents_md::build_init_agents_md_user_input;
use crate::agentic::keyed_lock::{KeyedAsyncLock, KeyedAsyncLockGuard};
use crate::agentic::round_preempt::{DialogRoundInjectionSource, SessionRoundInjectionBuffer};
use crate::agentic::session::session_store_port::CoreSessionStorePort;
use crate::agentic::session::SessionManager;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_runtime_ports::{ThreadGoal, MAX_THREAD_GOAL_AUTO_CONTINUATIONS};
use log::{debug, info, warn};
use std::collections::HashSet;
use std::path::PathBuf;
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
    resolve_turn_outcome_lifecycle_plan, ActiveDialogTurn, ActiveDialogTurnStore,
    ActiveDialogTurnTakeResult, AgentSessionReplyAction, AgentSessionReplyPlan,
    BackgroundDeliveryAction, BackgroundDeliveryFacts, BackgroundInjectionKind,
    DialogReplySuppressionSet, DialogStartRoute, DialogStartRouteFacts, DialogSteeringAction,
    DialogTurnQueue, GoalContinuationAfterTurnAction, SessionAbortFlags,
    ThreadGoalDeliveryReminder, ThreadGoalDeliveryReminderKind, TurnOutcomeQueueAction,
    TurnOutcomeStatus,
};
use bitfun_runtime_ports::{
    resolve_dialog_submit_queue_action, AgentBackgroundResultRequest, AgentDialogPrependedReminder,
    AgentDialogTurnPort, AgentDialogTurnRequest, AgentInputAttachment, AgentLifecycleDeliveryPort,
    AgentThreadGoalDeliveryKind, AgentThreadGoalDeliveryRequest, AgentTurnCancellationPort,
    AgentTurnCancellationRequest, AgentTurnCancellationResult, DialogSessionStateFact,
    DialogSubmitQueueAction, DialogSubmitQueueFacts, PortError, PortErrorKind, PortResult,
    RoundInjection, RoundInjectionKind, SessionStoragePathRequest, SessionStorePort,
};
pub use bitfun_runtime_ports::{
    AgentSessionReplyRoute, DialogQueuePriority, DialogSteerOutcome, DialogSubmissionPolicy,
    DialogSubmitOutcome,
};

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
pub(crate) enum QueuedTurnExecution {
    #[default]
    Standard,
    HiddenSubagent(HiddenSubagentQueuedExecution),
}

fn remove_queued_turn_by_id(
    queues: &DialogTurnQueue<QueuedTurn>,
    session_id: &str,
    turn_id: &str,
) -> Option<QueuedTurn> {
    queues.remove_first_matching(session_id, |turn| turn.turn_id.as_deref() == Some(turn_id))
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
    content: String,
    display_content: Option<String>,
    user_message_metadata: Option<serde_json::Value>,
}

struct SchedulerRoundInjectionSource {
    buffer: Arc<SessionRoundInjectionBuffer>,
}

impl DialogRoundInjectionSource for SchedulerRoundInjectionSource {
    fn has_pending(&self, session_id: &str, turn_id: &str) -> bool {
        self.buffer.has_pending_for_turn(session_id, turn_id)
    }

    fn pending_tool_preemption(
        &self,
        session_id: &str,
        turn_id: &str,
    ) -> bitfun_runtime_ports::RoundInjectionToolPreemption {
        self.buffer
            .pending_tool_preemption_for_turn(session_id, turn_id)
    }

    fn take_pending(&self, session_id: &str, turn_id: &str) -> Vec<RoundInjection> {
        self.buffer.drain_for_turn(session_id, turn_id)
    }

    fn acknowledge_consumed(
        &self,
        _session_id: &str,
        _turn_id: &str,
        _injection_id: &str,
        _kind: RoundInjectionKind,
    ) {
    }
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
    /// by the engine and injected into the running dialog turn.
    round_injection_buffer: Arc<SessionRoundInjectionBuffer>,
    round_injection_source: Arc<SchedulerRoundInjectionSource>,
    /// Child sessions already cancelled for a parent maintenance attempt but
    /// not yet observed as drained. Retain them across retryable timeouts even
    /// after their one-shot cancellation controls have been claimed.
    maintenance_background_sessions: Arc<dashmap::DashMap<String, HashSet<String>>>,
}

/// Holds the scheduler's exclusive session-operation boundary while a caller
/// performs maintenance that must not overlap turn dispatch.
pub(crate) struct SessionMaintenancePermit {
    _operation_guard: KeyedAsyncLockGuard,
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
        let round_injection_source = Arc::new(SchedulerRoundInjectionSource {
            buffer: round_injection_buffer.clone(),
        });

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
        });

        let scheduler_for_handler = Arc::clone(&scheduler);
        tokio::spawn(async move {
            scheduler_for_handler.run_outcome_handler(outcome_rx).await;
        });

        scheduler
    }

    /// Returns a sender to give to ConversationCoordinator for turn outcome notifications.
    pub fn outcome_sender(&self) -> mpsc::Sender<(String, TurnOutcome)> {
        self.outcome_tx.clone()
    }

    async fn lock_session_operation(&self, session_id: &str) -> KeyedAsyncLockGuard {
        self.session_operation_locks.lock(session_id).await
    }

    /// Pass to [`ConversationCoordinator::set_round_injection_source`](super::coordinator::ConversationCoordinator::set_round_injection_source).
    pub fn round_injection_monitor(&self) -> Arc<dyn DialogRoundInjectionSource> {
        self.round_injection_source.clone()
    }

    /// Submit a user "steering" message into the currently running dialog turn.
    ///
    /// Unlike [`Self::submit`], this never starts or queues a new turn — it only buffers
    /// the message so the [`ExecutionEngine`](super::super::execution::ExecutionEngine)
    /// can inject it at the next model-round boundary. Errors:
    ///
    /// - Session is not currently `Processing` the requested `turn_id` (the targeted turn
    ///   already finished or never existed). Caller should fall back to `submit`.
    pub async fn submit_steering(
        &self,
        session_id: String,
        turn_id: String,
        content: String,
        display_content: Option<String>,
    ) -> Result<DialogSteerOutcome, String> {
        let active_turn_id = match self
            .session_manager
            .get_session(&session_id)
            .map(|s| s.state.clone())
        {
            Some(SessionState::Processing {
                current_turn_id, ..
            }) => Some(current_turn_id),
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
        ) {
            DialogSteeringAction::Reject { error } => {
                warn!(
                    "submit_steering rejected: target turn is not running: session_id={}, turn_id={}",
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
        let _operation_guard = self.lock_session_operation(&session_id).await;
        let display = display_content.unwrap_or_else(|| content.clone());
        let delivery = BackgroundResultDelivery {
            session_id: session_id.clone(),
            agent_type,
            workspace_path,
            remote_connection_id,
            remote_ssh_host,
            content,
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
                let injection = resolve_background_delivery_injection_for_turn(
                    BackgroundInjectionKind::BackgroundResult,
                    injection_id.clone(),
                    delivery.content.clone(),
                    delivery.display_content.clone(),
                    SystemTime::now(),
                    current_turn_id,
                );
                self.round_injection_buffer.push(&session_id, injection);
                Ok(())
            }
            BackgroundDeliveryAction::SubmitAgentSessionFollowUp { queue_priority } => {
                self.submit_background_result_follow_up_locked(delivery, queue_priority)
                    .await
            }
        }
    }

    async fn submit_background_result_follow_up_locked(
        &self,
        delivery: BackgroundResultDelivery,
        queue_priority: DialogQueuePriority,
    ) -> Result<(), String> {
        let resolved_turn_id = Uuid::new_v4().to_string();
        let queued_turn = QueuedTurn {
            user_input: delivery.content,
            original_user_input: delivery.display_content,
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
                self.session_manager
                    .restore_session_from_storage_path(&restore_path, session_id)
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
        let _operation_guard = self.lock_session_operation(&session_id).await;
        self.submit_queued_turn_locked(session_id, resolved_turn_id, queued_turn, reject_if_busy)
            .await
    }

    async fn submit_queued_turn_locked(
        &self,
        session_id: String,
        resolved_turn_id: String,
        queued_turn: QueuedTurn,
        reject_if_busy: bool,
    ) -> Result<DialogSubmitOutcome, SchedulerSubmitError> {
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
                self.clear_queue(&session_id).await;
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

    async fn finish_removed_queued_turn(&self, session_id: &str, removed_turn: QueuedTurn) {
        match removed_turn.execution {
            QueuedTurnExecution::Standard => {
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
        let _operation_guard = self.lock_session_operation(session_id).await;
        abort_thread_goal_continuation_for_session(session_id);
        self.coordinator
            .cancel_active_turn_for_session(session_id, wait_timeout)
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
        if self.queue_depth(session_id) > 0 {
            self.clear_queue(session_id).await;
        }
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
        self.coordinator
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
        self.retire_active_turn_for_maintenance(session_id);
        Ok(SessionMaintenancePermit {
            _operation_guard: operation_guard,
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

    fn retire_active_turn_for_maintenance(&self, session_id: &str) {
        let Some(active_turn) = self.active_turns.remove(session_id) else {
            return;
        };
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

    async fn clear_queue(&self, session_id: &str) {
        let cleared_turns = self.queues.clear(session_id);
        let count = cleared_turns.len();
        for queued_turn in cleared_turns {
            match queued_turn.execution {
                QueuedTurnExecution::Standard => {
                    if let Some(turn_id) = queued_turn.turn_id {
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
        if let QueuedTurnExecution::HiddenSubagent(execution) = &queued_turn.execution {
            return self
                .start_hidden_subagent_turn(session_id, queued_turn, execution)
                .await
                .map_err(SchedulerSubmitError::Message);
        }

        let images = queued_turn
            .image_contexts
            .as_ref()
            .filter(|imgs| !imgs.is_empty());
        let route = resolve_dialog_start_route(DialogStartRouteFacts {
            has_image_contexts: images.is_some(),
            has_prepended_messages: !queued_turn.prepended_messages.is_empty(),
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
                        queued_turn.prepended_messages.clone(),
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
                        queued_turn.prepended_messages.clone(),
                    )
                    .await
            }
        };

        res.map_err(SchedulerSubmitError::Core)?;

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

        tokio::spawn(async move {
            let outcome = coordinator
                .execute_prepared_hidden_subagent(
                    request,
                    Some(&execution_cancel_token),
                    timeout_seconds,
                )
                .await;
            match outcome {
                Ok(result) => {
                    let _ = outcome_tx
                        .send((
                            session_id_owned.clone(),
                            TurnOutcome::Completed {
                                turn_id: turn_id_for_task.clone(),
                                final_response: result.text.clone(),
                            },
                        ))
                        .await;
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

    async fn forward_agent_session_reply(
        &self,
        responder_session_id: &str,
        plan: AgentSessionReplyPlan,
    ) {
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

    /// Background loop that receives turn outcome notifications from the coordinator.
    async fn run_outcome_handler(&self, mut outcome_rx: mpsc::Receiver<(String, TurnOutcome)>) {
        while let Some((session_id, outcome)) = outcome_rx.recv().await {
            let (active_turn, active_internal_turn, lifecycle_plan) = {
                let _operation_guard = self.lock_session_operation(&session_id).await;
                let Some(active_turn_result) = take_active_turn_for_outcome(
                    &self.active_turns,
                    &self.retired_maintenance_outcomes,
                    &session_id,
                    outcome.turn_id(),
                ) else {
                    self.round_injection_buffer
                        .drain_for_turn(&session_id, outcome.turn_id());
                    self.take_suppressed_cancelled_reply(&session_id, outcome.turn_id());
                    debug!(
                        "Ignoring outcome retired by session deletion: session_id={}, turn_id={}",
                        session_id,
                        outcome.turn_id()
                    );
                    continue;
                };
                let active_turn = match active_turn_result {
                    ActiveDialogTurnTakeResult::Matched(turn) => Some(turn),
                    ActiveDialogTurnTakeResult::Absent => None,
                    ActiveDialogTurnTakeResult::DifferentTurn => {
                        self.round_injection_buffer
                            .drain_for_turn(&session_id, outcome.turn_id());
                        self.take_suppressed_cancelled_reply(&session_id, outcome.turn_id());
                        debug!(
                            "Ignoring stale turn outcome: session_id={}, turn_id={}",
                            session_id,
                            outcome.turn_id()
                        );
                        continue;
                    }
                };
                let active_internal_turn = active_turn.as_ref().and_then(|_| {
                    self.active_internal_turns
                        .remove(&session_id)
                        .map(|(_, turn)| turn)
                });
                let lifecycle_plan =
                    resolve_turn_outcome_lifecycle_plan(&outcome, active_turn.is_some());
                if lifecycle_plan.queue_action == TurnOutcomeQueueAction::ClearQueue {
                    debug!(
                        "Turn {}, clearing queue: session_id={}",
                        lifecycle_plan.status, session_id
                    );
                    self.clear_queue(&session_id).await;
                }
                (active_turn, active_internal_turn, lifecycle_plan)
            };
            let status = lifecycle_plan.status;
            let queue_action = lifecycle_plan.queue_action;
            // Only drop steering messages targeted at the *finished* turn. We
            // must NOT clear the entire session buffer here: a user might have
            // legitimately submitted steering against a brand-new follow-up
            // turn that the dispatcher will pick up immediately after this
            // outcome is processed (race window between turn finalize and the
            // next turn starting). Targeting by turn_id keeps those alive.
            if lifecycle_plan.drain_finished_turn_injections {
                self.round_injection_buffer
                    .drain_for_turn(&session_id, outcome.turn_id());
            }
            let suppressed_cancelled_reply =
                self.take_suppressed_cancelled_reply(&session_id, outcome.turn_id());
            let is_internal_turn = active_internal_turn.is_some();
            if !is_internal_turn {
                if let Some(active_turn) = active_turn.as_ref() {
                    match resolve_agent_session_reply_action(
                        &session_id,
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
                            self.forward_agent_session_reply(&session_id, plan).await;
                        }
                    }
                }
            }

            if !is_internal_turn {
                if let Some(active_turn) = active_turn.as_ref() {
                    match lifecycle_plan.goal_continuation {
                        GoalContinuationAfterTurnAction::SkipNoActiveTurn => {}
                        GoalContinuationAfterTurnAction::AbortForCancelled => {
                            self.goal_continuation_abort.mark(&session_id);
                            debug!(
                            "Skipping thread goal continuation after user-cancelled turn: session_id={}, turn_id={}",
                            session_id,
                            outcome.turn_id()
                        );
                        }
                        GoalContinuationAfterTurnAction::Evaluate { turn_completed } => {
                            // Skip goal continuation when there are queued messages
                            // pending or any other session in the workspace still has
                            // an active turn — async subagent replies may still be in
                            // flight. Goal checks only trigger when all sessions are
                            // idle and the queue is drained.
                            let has_queued = self.queues.has_items(&session_id);
                            let has_active_sessions = !self.active_turns.is_empty();
                            if has_queued || has_active_sessions {
                                debug!(
                                    "Skipping goal continuation: queued_messages={}, active_sessions={} for session_id={}",
                                    has_queued, has_active_sessions, session_id
                                );
                            } else {
                                self.goal_continuation_abort.clear(&session_id);
                                match self
                                    .coordinator
                                    .prepare_goal_continuation_after_turn(
                                        &session_id,
                                        outcome.turn_id(),
                                        active_turn.user_input(),
                                        active_turn.user_message_metadata(),
                                        turn_completed,
                                    )
                                    .await
                                {
                                    Ok(Some(plan)) => {
                                        let prepended: Vec<Message> = plan
                                            .prepended_reminders
                                            .into_iter()
                                            .map(|text| {
                                                Message::internal_reminder(
                                                    InternalReminderKind::GoalContinuation,
                                                    text,
                                                )
                                            })
                                            .collect();
                                        let mut last_error = None;
                                        for attempt in 1..=MAX_THREAD_GOAL_AUTO_CONTINUATIONS {
                                            if self.goal_continuation_abort.contains(&session_id) {
                                                debug!(
                                        "Aborting goal continuation submit retries after user cancellation: session_id={}",
                                        session_id
                                    );
                                                break;
                                            }
                                            match self
                                            .submit_with_prepended_messages(
                                                session_id.clone(),
                                                "Continue working toward the active thread goal."
                                                    .to_string(),
                                                Some(plan.display_message.clone()),
                                                None,
                                                active_turn.agent_type_owned(),
                                                active_turn.workspace_path_owned(),
                                                active_turn.remote_connection_id_owned(),
                                                active_turn.remote_ssh_host_owned(),
                                                DialogSubmissionPolicy::for_source(
                                                    DialogTriggerSource::AgentSession,
                                                ),
                                                None,
                                                Some(plan.user_message_metadata.clone()),
                                                prepended.clone(),
                                                None,
                                            )
                                            .await
                                        {
                                            Ok(_) => {
                                                last_error = None;
                                                break;
                                            }
                                            Err(error) => {
                                                last_error = Some(error);
                                                if self
                                                    .goal_continuation_abort
                                                    .contains(&session_id)
                                                {
                                                    debug!(
                                                "Aborting goal continuation submit retries after user cancellation: session_id={}",
                                                session_id
                                            );
                                                    break;
                                                }
                                                if attempt < MAX_THREAD_GOAL_AUTO_CONTINUATIONS {
                                                    let delay_ms =
                                                        goal_continuation_submit_retry_delay_ms(
                                                            attempt,
                                                        );
                                                    warn!(
                                                "Goal continuation submit failed; retrying: session_id={}, attempt={}/{}, delay_ms={}, error={}",
                                                session_id,
                                                attempt,
                                                MAX_THREAD_GOAL_AUTO_CONTINUATIONS,
                                                delay_ms,
                                                last_error.as_ref().unwrap()
                                            );
                                                    tokio::time::sleep(
                                                        std::time::Duration::from_millis(delay_ms),
                                                    )
                                                    .await;
                                                }
                                            }
                                        }
                                        }
                                        if let Some(error) = last_error {
                                            if !self.goal_continuation_abort.contains(&session_id) {
                                                warn!(
                                        "Failed to submit goal continuation turn after retries: session_id={}, error={}",
                                        session_id, error
                                    );
                                            }
                                        }
                                    }
                                    Ok(None) => {}
                                    Err(error) => {
                                        warn!(
                                "Goal verification failed after turn stopped: session_id={}, status={}, error={}",
                                session_id, status, error
                            );
                                    }
                                }
                            }
                        }
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

                    if let Err(e) = self.dispatch_next_if_idle(&session_id).await {
                        warn!(
                            "Failed to dispatch next queued message after {}: session_id={}, error={}",
                            status, session_id, e
                        );
                    }
                }
                TurnOutcomeQueueAction::ClearQueue => {}
            }
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
            execution: QueuedTurnExecution::Standard,
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
            self.cancel_queued_or_active_turn(&session_id, &turn_id)
                .await
                .map_err(|error| PortError::new(PortErrorKind::Backend, error.to_string()))?;
            Some(turn_id)
        } else if let Some(requester_session_id) = request.requester_session_id {
            self.cancel_active_turn_for_session_from_requester(
                &session_id,
                &requester_session_id,
                wait_timeout,
            )
            .await
            .map_err(|error| PortError::new(PortErrorKind::Backend, error.to_string()))?
        } else {
            self.cancel_active_turn_for_session(&session_id, wait_timeout)
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
        PromptCachePolicy, SessionContextStore, SessionManagerConfig,
    };
    use crate::agentic::tools::registry::ToolRegistry;
    use crate::agentic::tools::{ToolPipeline, ToolStateManager};
    use crate::infrastructure::PathManager;
    use bitfun_runtime_ports::{AgentDialogPrependedReminder, AgentInputAttachment, PortErrorKind};
    use tokio::sync::RwLock as TokioRwLock;

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
        ));
        (
            DialogScheduler::new(coordinator, session_manager.clone()),
            session_manager,
            event_queue,
            root,
        )
    }

    #[test]
    fn queued_turn_execution_default_is_standard() {
        assert!(matches!(
            QueuedTurnExecution::default(),
            QueuedTurnExecution::Standard
        ));
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
            resolve_agent_session_reply_action("session_b", &active_turn, &cancelled, true),
            AgentSessionReplyAction::SkipSuppressedCancelledReply
        );
        assert!(matches!(
            resolve_agent_session_reply_action("session_b", &active_turn, &cancelled, false),
            AgentSessionReplyAction::Forward(_)
        ));
        assert!(matches!(
            resolve_agent_session_reply_action("session_b", &active_turn, &completed, true),
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
}
