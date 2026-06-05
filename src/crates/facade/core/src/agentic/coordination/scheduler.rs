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

use super::coordinator::{ConversationCoordinator, DialogTriggerSource};
use super::turn_outcome::TurnOutcome;
use crate::agentic::core::{InternalReminderKind, Message, SessionState};
use crate::agentic::goal_mode::{
    goal_continuation_submit_retry_delay_ms, goal_internal_context_message,
    goal_objective_updated_message,
};
use crate::agentic::image_analysis::ImageContextData;
use crate::agentic::init_agents_md::build_init_agents_md_user_input;
use crate::agentic::round_preempt::{
    DialogRoundInjectionSource, DialogRoundPreemptSource, SessionRoundInjectionBuffer,
    SessionRoundYieldFlags,
};
use crate::agentic::session::SessionManager;
use bitfun_runtime_ports::ThreadGoal;
use bitfun_runtime_ports::MAX_THREAD_GOAL_AUTO_CONTINUATIONS;
use log::{debug, info, warn};
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use uuid::Uuid;

use bitfun_agent_runtime::scheduler::{
    build_thread_goal_objective_updated_delivery_plan, build_thread_goal_resumed_delivery_plan,
    resolve_agent_session_reply_action, resolve_background_delivery_action,
    resolve_background_delivery_injection, resolve_dialog_steering_action,
    resolve_turn_outcome_lifecycle_plan, ActiveDialogTurn, ActiveDialogTurnStore,
    AgentSessionReplyAction, AgentSessionReplyPlan, BackgroundDeliveryAction,
    BackgroundDeliveryFacts, BackgroundInjectionKind, DialogReplySuppressionSet,
    DialogSteeringAction, DialogTurnQueue, GoalContinuationAfterTurnAction, SessionAbortFlags,
    ThreadGoalDeliveryReminder, ThreadGoalDeliveryReminderKind, TurnOutcomeQueueAction,
    TurnOutcomeStatus,
};
use bitfun_runtime_ports::{
    resolve_dialog_submit_queue_action, DialogSessionStateFact, DialogSubmitQueueAction,
    DialogSubmitQueueFacts,
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
    pub policy: DialogSubmissionPolicy,
    pub reply_route: Option<AgentSessionReplyRoute>,
    pub user_message_metadata: Option<serde_json::Value>,
    pub image_contexts: Option<Vec<ImageContextData>>,
    #[allow(dead_code)]
    pub enqueued_at: SystemTime,
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
    /// Currently active turn metadata keyed by target session ID
    active_turns: Arc<ActiveDialogTurnStore>,
    /// Turns whose cancelled auto-reply should be suppressed because the source
    /// agent explicitly cancelled its own outstanding SessionMessage request.
    suppressed_cancelled_replies: Arc<DialogReplySuppressionSet>,
    /// Set when the user cancels an in-flight turn; aborts goal-continuation submit retries.
    goal_continuation_abort: Arc<SessionAbortFlags>,
    /// Cloneable sender given to ConversationCoordinator for turn outcome notifications
    outcome_tx: mpsc::Sender<(String, TurnOutcome)>,
    /// When a user submits while `Processing`, engine yields after the current model round.
    round_yield_flags: Arc<SessionRoundYieldFlags>,
    /// Per-session FIFO buffer of round injections drained at round boundaries
    /// by the engine and injected into the running dialog turn.
    round_injection_buffer: Arc<SessionRoundInjectionBuffer>,
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

        let scheduler = Arc::new(Self {
            coordinator,
            session_manager,
            queues: Arc::new(DialogTurnQueue::default()),
            active_turns: Arc::new(ActiveDialogTurnStore::default()),
            suppressed_cancelled_replies: Arc::new(DialogReplySuppressionSet::default()),
            goal_continuation_abort: Arc::new(SessionAbortFlags::default()),
            outcome_tx,
            round_yield_flags: Arc::new(SessionRoundYieldFlags::default()),
            round_injection_buffer: Arc::new(SessionRoundInjectionBuffer::default()),
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

    /// Pass to [`ConversationCoordinator::set_round_preempt_source`](super::coordinator::ConversationCoordinator::set_round_preempt_source).
    pub fn preempt_monitor(&self) -> Arc<dyn DialogRoundPreemptSource> {
        self.round_yield_flags.clone()
    }

    /// Pass to [`ConversationCoordinator::set_round_injection_source`](super::coordinator::ConversationCoordinator::set_round_injection_source).
    pub fn round_injection_monitor(&self) -> Arc<dyn DialogRoundInjectionSource> {
        self.round_injection_buffer.clone()
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
            BackgroundDeliveryAction::SubmitAgentSessionFollowUp {
                queue_priority,
                skip_tool_confirmation,
            } => {
                let prepended = thread_goal_delivery_messages(plan.prepended_reminders);
                self.submit_with_prepended_messages(
                    session_id,
                    plan.follow_up_user_input,
                    plan.follow_up_original_user_input,
                    None,
                    agent_type,
                    workspace_path,
                    DialogSubmissionPolicy::new(
                        DialogTriggerSource::AgentSession,
                        queue_priority,
                        skip_tool_confirmation,
                    ),
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
            BackgroundDeliveryAction::SubmitAgentSessionFollowUp {
                queue_priority,
                skip_tool_confirmation,
            } => {
                let prepended = thread_goal_delivery_messages(plan.prepended_reminders);
                self.submit_with_prepended_messages(
                    session_id,
                    plan.follow_up_user_input,
                    plan.follow_up_original_user_input,
                    None,
                    agent_type,
                    workspace_path,
                    DialogSubmissionPolicy::new(
                        DialogTriggerSource::AgentSession,
                        queue_priority,
                        skip_tool_confirmation,
                    ),
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
        content: String,
        display_content: Option<String>,
        user_message_metadata: Option<serde_json::Value>,
    ) -> Result<(), String> {
        let display = display_content.unwrap_or_else(|| content.clone());
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
                        BackgroundInjectionKind::BackgroundResult,
                        Uuid::new_v4().to_string(),
                        content,
                        Some(display),
                        SystemTime::now(),
                    ),
                );
                Ok(())
            }
            BackgroundDeliveryAction::SubmitAgentSessionFollowUp {
                queue_priority,
                skip_tool_confirmation,
            } => self
                .submit(
                    session_id,
                    content,
                    Some(display),
                    None,
                    agent_type,
                    workspace_path,
                    DialogSubmissionPolicy::new(
                        DialogTriggerSource::AgentSession,
                        queue_priority,
                        skip_tool_confirmation,
                    ),
                    None,
                    user_message_metadata,
                    None,
                )
                .await
                .map(|_| ()),
        }
    }

    pub async fn submit_init_agents_md(
        &self,
        session_id: String,
        workspace_path: Option<String>,
        policy: DialogSubmissionPolicy,
    ) -> Result<DialogSubmitOutcome, String> {
        let agent_type = self
            .resolve_session_agent_type(&session_id, workspace_path.as_deref())
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
            Some(SessionState::Idle) => DialogSessionStateFact::Idle,
            Some(SessionState::Processing { .. }) => DialogSessionStateFact::Processing,
            Some(SessionState::Error { .. }) => DialogSessionStateFact::Error,
        }
    }

    /// Submit a user message for a session.
    ///
    /// - Session idle, queue empty → dispatched immediately.
    /// - Session idle, queue non-empty → enqueued then highest-priority queued message dispatched.
    /// - Session processing → queued up to the runtime-owned queue limit. For interactive sources
    ///   (desktop, CLI, bot, …), also requests a yield after the current model round so
    ///   the queued message can start sooner than a full multi-round turn.
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
            policy,
            reply_route,
            user_message_metadata,
            image_contexts,
            enqueued_at: SystemTime::now(),
        };
        self.submit_queued_turn(session_id, resolved_turn_id, queued_turn)
            .await
    }

    async fn resolve_session_agent_type(
        &self,
        session_id: &str,
        workspace_path: Option<&str>,
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
                self.session_manager
                    .restore_session(Path::new(workspace_path), session_id)
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

    async fn submit_queued_turn(
        &self,
        session_id: String,
        resolved_turn_id: String,
        queued_turn: QueuedTurn,
    ) -> Result<DialogSubmitOutcome, String> {
        let state = self
            .session_manager
            .get_session(&session_id)
            .map(|s| s.state.clone());

        let queue_has_items = self.queues.has_items(&session_id);
        let action = resolve_dialog_submit_queue_action(DialogSubmitQueueFacts {
            session_state: Self::session_state_fact(state.as_ref()),
            queue_has_items,
            policy: queued_turn.policy,
        });

        match action {
            DialogSubmitQueueAction::StartImmediately => {
                let tid = self.start_turn(&session_id, &queued_turn).await?;
                self.record_last_submitted_agent_type(&session_id, &queued_turn.agent_type)
                    .await;
                Ok(DialogSubmitOutcome::Started {
                    session_id,
                    turn_id: tid,
                })
            }

            DialogSubmitQueueAction::ClearQueueAndStartImmediately => {
                self.clear_queue(&session_id);
                let tid = self.start_turn(&session_id, &queued_turn).await?;
                self.record_last_submitted_agent_type(&session_id, &queued_turn.agent_type)
                    .await;
                Ok(DialogSubmitOutcome::Started {
                    session_id,
                    turn_id: tid,
                })
            }

            DialogSubmitQueueAction::EnqueueThenStartNext => {
                self.enqueue(&session_id, queued_turn.clone())?;
                self.record_last_submitted_agent_type(&session_id, &queued_turn.agent_type)
                    .await;
                let started_tid = self.try_start_next_queued(&session_id).await?;
                let outcome = match started_tid {
                    Some(tid) if tid == resolved_turn_id => DialogSubmitOutcome::Started {
                        session_id: session_id.clone(),
                        turn_id: tid,
                    },
                    _ => DialogSubmitOutcome::Queued {
                        session_id: session_id.clone(),
                        turn_id: resolved_turn_id,
                    },
                };
                Ok(outcome)
            }

            DialogSubmitQueueAction::EnqueueForActiveTurn { request_yield } => {
                let accepted_agent_type = queued_turn.agent_type.clone();
                self.enqueue(&session_id, queued_turn)?;
                self.record_last_submitted_agent_type(&session_id, &accepted_agent_type)
                    .await;
                if request_yield {
                    self.round_yield_flags.request_yield(&session_id);
                }
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

    fn clear_queue(&self, session_id: &str) {
        let count = self.queues.clear(session_id);
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

    async fn try_start_next_queued(&self, session_id: &str) -> Result<Option<String>, String> {
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
    ) -> Result<String, String> {
        let res = match queued_turn
            .image_contexts
            .as_ref()
            .filter(|imgs| !imgs.is_empty())
        {
            Some(imgs) => {
                if queued_turn.prepended_messages.is_empty() {
                    self.coordinator
                        .start_dialog_turn_with_image_contexts(
                            session_id.to_string(),
                            queued_turn.user_input.clone(),
                            queued_turn.original_user_input.clone(),
                            imgs.clone(),
                            queued_turn.turn_id.clone(),
                            queued_turn.agent_type.clone(),
                            queued_turn.workspace_path.clone(),
                            queued_turn.policy,
                            queued_turn.user_message_metadata.clone(),
                        )
                        .await
                } else {
                    self.coordinator
                        .start_dialog_turn_with_image_contexts_and_prepended_messages(
                            session_id.to_string(),
                            queued_turn.user_input.clone(),
                            queued_turn.original_user_input.clone(),
                            imgs.clone(),
                            queued_turn.turn_id.clone(),
                            queued_turn.agent_type.clone(),
                            queued_turn.workspace_path.clone(),
                            queued_turn.policy,
                            queued_turn.user_message_metadata.clone(),
                            queued_turn.prepended_messages.clone(),
                        )
                        .await
                }
            }
            None => {
                if queued_turn.prepended_messages.is_empty() {
                    self.coordinator
                        .start_dialog_turn(
                            session_id.to_string(),
                            queued_turn.user_input.clone(),
                            queued_turn.original_user_input.clone(),
                            queued_turn.turn_id.clone(),
                            queued_turn.agent_type.clone(),
                            queued_turn.workspace_path.clone(),
                            queued_turn.policy,
                            queued_turn.user_message_metadata.clone(),
                        )
                        .await
                } else {
                    self.coordinator
                        .start_dialog_turn_with_prepended_messages(
                            session_id.to_string(),
                            queued_turn.user_input.clone(),
                            queued_turn.original_user_input.clone(),
                            queued_turn.turn_id.clone(),
                            queued_turn.agent_type.clone(),
                            queued_turn.workspace_path.clone(),
                            queued_turn.policy,
                            queued_turn.user_message_metadata.clone(),
                            queued_turn.prepended_messages.clone(),
                        )
                        .await
                }
            }
        };

        res.map_err(|e| e.to_string())?;

        let resolved = self
            .session_manager
            .get_session(session_id)
            .and_then(|s| match &s.state {
                SessionState::Processing {
                    current_turn_id, ..
                } => Some(current_turn_id.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                format!(
                    "Failed to resolve turn_id after starting dialog: session_id={}",
                    session_id
                )
            })?;

        self.active_turns.insert(
            session_id,
            ActiveDialogTurn::new(
                resolved.clone(),
                queued_turn.workspace_path.clone(),
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

    async fn forward_agent_session_reply(
        &self,
        responder_session_id: &str,
        plan: AgentSessionReplyPlan,
    ) {
        let reply_user_input = plan.user_input;
        let target_session_id = plan.target_session_id;
        let target_workspace_path = plan.target_workspace_path;
        let prepended_messages = vec![Message::internal_reminder(
            InternalReminderKind::SessionMessageReply,
            plan.reminder_text,
        )];

        if let Err(error) = self
            .submit_with_prepended_messages(
                target_session_id.clone(),
                reply_user_input.clone(),
                Some(reply_user_input),
                None,
                String::new(),
                Some(target_workspace_path),
                DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                None,
                None,
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
        let _ = self.try_start_next_queued(session_id).await?;
        Ok(())
    }

    /// Background loop that receives turn outcome notifications from the coordinator.
    async fn run_outcome_handler(&self, mut outcome_rx: mpsc::Receiver<(String, TurnOutcome)>) {
        while let Some((session_id, outcome)) = outcome_rx.recv().await {
            let lifecycle_plan = resolve_turn_outcome_lifecycle_plan(
                &outcome,
                self.active_turns.contains(&session_id),
            );

            if lifecycle_plan.clear_round_yield {
                self.round_yield_flags.clear(&session_id);
            }
            // Only drop steering messages targeted at the *finished* turn. We
            // must NOT clear the entire session buffer here: a user might have
            // legitimately submitted steering against a brand-new follow-up
            // turn that the dispatcher will pick up immediately after this
            // outcome is processed (race window between turn finalize and the
            // next turn starting). Targeting by turn_id keeps those alive.
            if lifecycle_plan.drain_finished_turn_injections {
                let _drained = self
                    .round_injection_buffer
                    .drain_for_turn(&session_id, outcome.turn_id());
            }
            let suppressed_cancelled_reply =
                self.take_suppressed_cancelled_reply(&session_id, outcome.turn_id());

            let active_turn = self.active_turns.remove(&session_id);
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

            let status = lifecycle_plan.status;
            let queue_action = lifecycle_plan.queue_action;
            if queue_action == TurnOutcomeQueueAction::ClearQueue {
                debug!("Turn {}, clearing queue: session_id={}", status, session_id);
                self.clear_queue(&session_id);
            }

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
                        self.goal_continuation_abort.clear(&session_id);
                        match self
                            .coordinator
                            .prepare_goal_continuation_after_turn(
                                &session_id,
                                &outcome.turn_id(),
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
                                            if self.goal_continuation_abort.contains(&session_id) {
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

    fn agent_session_active_turn(source_session_id: &str) -> ActiveDialogTurn {
        ActiveDialogTurn::new(
            "turn_1".to_string(),
            Some("/workspace".to_string()),
            "agentic".to_string(),
            "hello".to_string(),
            None,
            DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
            Some(AgentSessionReplyRoute {
                source_session_id: source_session_id.to_string(),
                source_workspace_path: "/source".to_string(),
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
    fn remote_queue_policy_preserves_interactive_preempt_and_confirmation_boundary() {
        let remote = DialogSubmissionPolicy::for_source(DialogTriggerSource::RemoteRelay);
        assert_eq!(remote.queue_priority, DialogQueuePriority::Normal);
        assert!(remote.skip_tool_confirmation);
        assert!(bitfun_runtime_ports::dialog_policy_may_preempt(&remote));

        let bot = DialogSubmissionPolicy::for_source(DialogTriggerSource::Bot);
        assert_eq!(bot.queue_priority, DialogQueuePriority::Normal);
        assert!(bot.skip_tool_confirmation);
        assert!(bitfun_runtime_ports::dialog_policy_may_preempt(&bot));

        let agent_session = DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession);
        assert_eq!(agent_session.queue_priority, DialogQueuePriority::Low);
        assert!(agent_session.skip_tool_confirmation);
        assert!(!bitfun_runtime_ports::dialog_policy_may_preempt(
            &agent_session
        ));
    }
}
