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
use super::turn_outcome::{TurnOutcome, TurnOutcomeQueueAction, TurnOutcomeStatus};
use crate::agentic::core::{PromptEnvelope, SessionState};
use crate::agentic::image_analysis::ImageContextData;
use crate::agentic::round_preempt::{
    DialogRoundPreemptSource, DialogRoundSteeringSource, SessionRoundYieldFlags,
    SessionSteeringBuffer, SteeringMessage,
};
use crate::agentic::session::SessionManager;
use dashmap::DashMap;
use log::{debug, info, warn};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use uuid::Uuid;

const MAX_QUEUE_DEPTH: usize = 20;

/// Result of [`DialogScheduler::submit`]: whether this message began executing immediately
/// or was placed in the per-session queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogSubmitOutcome {
    Started { session_id: String, turn_id: String },
    Queued { session_id: String, turn_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DialogQueuePriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogSubmissionPolicy {
    pub trigger_source: DialogTriggerSource,
    pub queue_priority: DialogQueuePriority,
    pub skip_tool_confirmation: bool,
}

impl DialogSubmissionPolicy {
    pub const fn new(
        trigger_source: DialogTriggerSource,
        queue_priority: DialogQueuePriority,
        skip_tool_confirmation: bool,
    ) -> Self {
        Self {
            trigger_source,
            queue_priority,
            skip_tool_confirmation,
        }
    }

    pub const fn for_source(trigger_source: DialogTriggerSource) -> Self {
        let (queue_priority, skip_tool_confirmation) = match trigger_source {
            DialogTriggerSource::AgentSession => (DialogQueuePriority::Low, true),
            DialogTriggerSource::ScheduledJob => (DialogQueuePriority::Low, true),
            DialogTriggerSource::DesktopUi
            | DialogTriggerSource::DesktopApi
            | DialogTriggerSource::Cli => (DialogQueuePriority::Normal, false),
            DialogTriggerSource::RemoteRelay | DialogTriggerSource::Bot => {
                (DialogQueuePriority::Normal, true)
            }
        };
        Self::new(trigger_source, queue_priority, skip_tool_confirmation)
    }

    pub const fn with_queue_priority(mut self, queue_priority: DialogQueuePriority) -> Self {
        self.queue_priority = queue_priority;
        self
    }

    pub const fn with_skip_tool_confirmation(mut self, skip_tool_confirmation: bool) -> Self {
        self.skip_tool_confirmation = skip_tool_confirmation;
        self
    }
}

#[derive(Debug, Clone)]
pub struct AgentSessionReplyRoute {
    pub source_session_id: String,
    pub source_workspace_path: String,
}

#[derive(Debug, Clone)]
struct ActiveTurn {
    turn_id: String,
    workspace_path: Option<String>,
    policy: DialogSubmissionPolicy,
    reply_route: Option<AgentSessionReplyRoute>,
}

impl ActiveTurn {
    fn from_queued_turn(turn: &QueuedTurn, turn_id: String) -> Self {
        Self {
            turn_id,
            workspace_path: turn.workspace_path.clone(),
            policy: turn.policy,
            reply_route: turn.reply_route.clone(),
        }
    }

    fn is_agent_session_request(&self) -> bool {
        self.policy.trigger_source == DialogTriggerSource::AgentSession
            && self.reply_route.is_some()
    }

    fn should_suppress_cancelled_reply_for_requester(&self, requester_session_id: &str) -> bool {
        self.is_agent_session_request()
            && self
                .reply_route
                .as_ref()
                .is_some_and(|reply_route| reply_route.source_session_id == requester_session_id)
    }
}

/// A message waiting to be dispatched to the coordinator
#[derive(Debug, Clone)]
pub struct QueuedTurn {
    pub user_input: String,
    pub original_user_input: Option<String>,
    pub turn_id: Option<String>,
    pub agent_type: String,
    pub workspace_path: Option<String>,
    pub policy: DialogSubmissionPolicy,
    pub reply_route: Option<AgentSessionReplyRoute>,
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
    /// Per-session priority message queues
    queues: Arc<DashMap<String, VecDeque<QueuedTurn>>>,
    /// Currently active turn metadata keyed by target session ID
    active_turns: Arc<DashMap<String, ActiveTurn>>,
    /// Turns whose cancelled auto-reply should be suppressed because the source
    /// agent explicitly cancelled its own outstanding SessionMessage request.
    suppressed_cancelled_replies: Arc<DashMap<(String, String), ()>>,
    /// Cloneable sender given to ConversationCoordinator for turn outcome notifications
    outcome_tx: mpsc::Sender<(String, TurnOutcome)>,
    /// When a user submits while `Processing`, engine yields after the current model round.
    round_yield_flags: Arc<SessionRoundYieldFlags>,
    /// Per-session FIFO buffer of user "steering" messages drained at round boundaries
    /// by the engine and injected into the running dialog turn.
    steering_buffer: Arc<SessionSteeringBuffer>,
}

/// Outcome of [`DialogScheduler::submit_steering`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogSteerOutcome {
    /// Steering message was buffered for the running turn. The engine will pick it up
    /// at the next model-round boundary.
    Buffered {
        session_id: String,
        turn_id: String,
        steering_id: String,
    },
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
            queues: Arc::new(DashMap::new()),
            active_turns: Arc::new(DashMap::new()),
            suppressed_cancelled_replies: Arc::new(DashMap::new()),
            outcome_tx,
            round_yield_flags: Arc::new(SessionRoundYieldFlags::default()),
            steering_buffer: Arc::new(SessionSteeringBuffer::default()),
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

    /// Pass to [`ConversationCoordinator::set_round_steering_source`](super::coordinator::ConversationCoordinator::set_round_steering_source).
    pub fn steering_monitor(&self) -> Arc<dyn DialogRoundSteeringSource> {
        self.steering_buffer.clone()
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
        let active_matches_turn = match self
            .session_manager
            .get_session(&session_id)
            .map(|s| s.state.clone())
        {
            Some(SessionState::Processing {
                current_turn_id, ..
            }) => current_turn_id == turn_id,
            _ => false,
        };

        if !active_matches_turn {
            warn!(
                "submit_steering rejected: target turn is not running: session_id={}, turn_id={}",
                session_id, turn_id
            );
            return Err(format!(
                "Dialog turn is no longer running and cannot be steered: session_id={}, turn_id={}",
                session_id, turn_id
            ));
        }

        let steering_id = Uuid::new_v4().to_string();
        let display = display_content.unwrap_or_else(|| content.clone());
        let message = SteeringMessage {
            id: steering_id.clone(),
            turn_id: turn_id.clone(),
            content,
            display_content: display,
            created_at: SystemTime::now(),
        };

        self.steering_buffer.push(&session_id, message);
        info!(
            "Steering message buffered: session_id={}, turn_id={}, steering_id={}, pending={}",
            session_id,
            turn_id,
            steering_id,
            self.steering_buffer.pending_count(&session_id)
        );

        Ok(DialogSteerOutcome::Buffered {
            session_id,
            turn_id,
            steering_id,
        })
    }

    fn user_message_may_preempt(policy: &DialogSubmissionPolicy) -> bool {
        matches!(
            policy.trigger_source,
            DialogTriggerSource::DesktopUi
                | DialogTriggerSource::DesktopApi
                | DialogTriggerSource::Cli
                | DialogTriggerSource::RemoteRelay
                | DialogTriggerSource::Bot
        )
    }

    /// Submit a user message for a session.
    ///
    /// - Session idle, queue empty → dispatched immediately.
    /// - Session idle, queue non-empty → enqueued then highest-priority queued message dispatched.
    /// - Session processing → queued (up to MAX_QUEUE_DEPTH). For interactive sources
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
        image_contexts: Option<Vec<ImageContextData>>,
    ) -> Result<DialogSubmitOutcome, String> {
        let resolved_turn_id = turn_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let queued_turn = QueuedTurn {
            user_input,
            original_user_input,
            turn_id: Some(resolved_turn_id.clone()),
            agent_type,
            workspace_path,
            policy,
            reply_route,
            image_contexts,
            enqueued_at: SystemTime::now(),
        };
        let state = self
            .session_manager
            .get_session(&session_id)
            .map(|s| s.state.clone());

        match state {
            None => {
                let tid = self.start_turn(&session_id, &queued_turn).await?;
                Ok(DialogSubmitOutcome::Started {
                    session_id,
                    turn_id: tid,
                })
            }

            Some(SessionState::Error { .. }) => {
                self.clear_queue(&session_id);
                let tid = self.start_turn(&session_id, &queued_turn).await?;
                Ok(DialogSubmitOutcome::Started {
                    session_id,
                    turn_id: tid,
                })
            }

            Some(SessionState::Idle) => {
                let queue_non_empty = self
                    .queues
                    .get(&session_id)
                    .map(|q| !q.is_empty())
                    .unwrap_or(false);

                if queue_non_empty {
                    self.enqueue(&session_id, queued_turn.clone())?;
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
                } else {
                    let tid = self.start_turn(&session_id, &queued_turn).await?;
                    Ok(DialogSubmitOutcome::Started {
                        session_id,
                        turn_id: tid,
                    })
                }
            }

            Some(SessionState::Processing { .. }) => {
                let may_preempt = Self::user_message_may_preempt(&queued_turn.policy);
                self.enqueue(&session_id, queued_turn)?;
                if may_preempt {
                    self.round_yield_flags.request_yield(&session_id);
                }
                Ok(DialogSubmitOutcome::Queued {
                    session_id,
                    turn_id: resolved_turn_id,
                })
            }
        }
    }

    /// Number of messages currently queued for a session.
    pub fn queue_depth(&self, session_id: &str) -> usize {
        self.queues.get(session_id).map(|q| q.len()).unwrap_or(0)
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
            .get(target_session_id)
            .and_then(|active_turn| {
                active_turn
                    .should_suppress_cancelled_reply_for_requester(requester_session_id)
                    .then(|| (target_session_id.to_string(), active_turn.turn_id.clone()))
            });

        if let Some((session_id, turn_id)) = suppression_key.as_ref() {
            debug!(
                "Suppressing cancelled auto-reply for agent-session turn: target_session_id={}, turn_id={}, requester_session_id={}",
                session_id, turn_id, requester_session_id
            );
            self.suppressed_cancelled_replies
                .insert((session_id.clone(), turn_id.clone()), ());
        }

        match self
            .coordinator
            .cancel_active_turn_for_session(target_session_id, wait_timeout)
            .await
        {
            Ok(cancelled_turn_id) => {
                if cancelled_turn_id.is_none() {
                    if let Some((session_id, turn_id)) = suppression_key {
                        self.suppressed_cancelled_replies
                            .remove(&(session_id, turn_id));
                    }
                }
                Ok(cancelled_turn_id)
            }
            Err(error) => {
                if let Some((session_id, turn_id)) = suppression_key {
                    self.suppressed_cancelled_replies
                        .remove(&(session_id, turn_id));
                }
                Err(error)
            }
        }
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn enqueue(&self, session_id: &str, queued_turn: QueuedTurn) -> Result<(), String> {
        let queue_len = self.queues.get(session_id).map(|q| q.len()).unwrap_or(0);

        if queue_len >= MAX_QUEUE_DEPTH {
            warn!(
                "Queue full, rejecting message: session_id={}, max={}",
                session_id, MAX_QUEUE_DEPTH
            );
            return Err(format!(
                "Message queue full for session {} (max {} messages)",
                session_id, MAX_QUEUE_DEPTH
            ));
        }

        self.queues
            .entry(session_id.to_string())
            .or_default()
            .push_back(queued_turn.clone());
        if let Some(mut queue) = self.queues.get_mut(session_id) {
            if let Some(reordered_turn) = queue.pop_back() {
                let insert_at = queue.iter().position(|existing| {
                    existing.policy.queue_priority < reordered_turn.policy.queue_priority
                });
                if let Some(index) = insert_at {
                    queue.insert(index, reordered_turn);
                } else {
                    queue.push_back(reordered_turn);
                }
            }
        }

        let new_len = self.queues.get(session_id).map(|q| q.len()).unwrap_or(0);
        debug!(
            "Message queued: session_id={}, queue_depth={}, priority={:?}",
            session_id, new_len, queued_turn.policy.queue_priority
        );
        Ok(())
    }

    fn clear_queue(&self, session_id: &str) {
        if let Some(mut queue) = self.queues.get_mut(session_id) {
            let count = queue.len();
            queue.clear();
            if count > 0 {
                info!(
                    "Cleared {} queued messages: session_id={}",
                    count, session_id
                );
            }
        }
    }

    fn dequeue_next(&self, session_id: &str) -> Option<QueuedTurn> {
        self.queues
            .get_mut(session_id)
            .and_then(|mut q| q.pop_front())
    }

    fn requeue_front(&self, session_id: &str, turn: QueuedTurn) {
        self.queues
            .entry(session_id.to_string())
            .or_default()
            .push_front(turn);
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

        let remaining = self.queues.get(session_id).map(|q| q.len()).unwrap_or(0);
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
                    )
                    .await
            }
            None => {
                self.coordinator
                    .start_dialog_turn(
                        session_id.to_string(),
                        queued_turn.user_input.clone(),
                        queued_turn.original_user_input.clone(),
                        queued_turn.turn_id.clone(),
                        queued_turn.agent_type.clone(),
                        queued_turn.workspace_path.clone(),
                        queued_turn.policy,
                    )
                    .await
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
            session_id.to_string(),
            ActiveTurn::from_queued_turn(queued_turn, resolved.clone()),
        );

        Ok(resolved)
    }

    async fn forward_agent_session_reply(
        &self,
        responder_session_id: &str,
        active_turn: &ActiveTurn,
        outcome: &TurnOutcome,
    ) {
        if !active_turn.is_agent_session_request() {
            return;
        }

        let Some(reply_route) = active_turn.reply_route.as_ref() else {
            return;
        };

        let responder_workspace = active_turn
            .workspace_path
            .as_deref()
            .unwrap_or("<unknown workspace>");
        let reply_user_input = outcome.reply_text();
        let reply_message =
            Self::format_agent_session_reply(responder_session_id, responder_workspace, outcome);

        if let Err(error) = self
            .submit(
                reply_route.source_session_id.clone(),
                reply_message,
                Some(reply_user_input),
                None,
                String::new(),
                Some(reply_route.source_workspace_path.clone()),
                DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
                None,
                None,
            )
            .await
        {
            warn!(
                "Failed to forward agent-session reply: responder_session_id={}, source_session_id={}, error={}",
                responder_session_id, reply_route.source_session_id, error
            );
        }
    }

    fn take_suppressed_cancelled_reply(&self, session_id: &str, turn_id: &str) -> bool {
        self.suppressed_cancelled_replies
            .remove(&(session_id.to_string(), turn_id.to_string()))
            .is_some()
    }

    fn should_skip_agent_session_reply(
        outcome: &TurnOutcome,
        suppressed_cancelled_reply: bool,
    ) -> bool {
        matches!(outcome, TurnOutcome::Cancelled { .. }) && suppressed_cancelled_reply
    }

    fn format_agent_session_reply(
        responder_session_id: &str,
        responder_workspace: &str,
        outcome: &TurnOutcome,
    ) -> String {
        let mut envelope = PromptEnvelope::new();
        let status = outcome.status();
        let reply_text = outcome.reply_text();
        envelope.push_system_reminder(format!(
            "This message is an automated reply to a previous SessionMessage call, not a human user message.\n\
From session: {responder_session_id}\n\
From workspace: {responder_workspace}\n\
Status: {status}"
        ));
        envelope.push_user_query(reply_text);
        envelope.render()
    }

    async fn dispatch_next_if_idle(&self, session_id: &str) -> Result<(), String> {
        let _ = self.try_start_next_queued(session_id).await?;
        Ok(())
    }

    /// Background loop that receives turn outcome notifications from the coordinator.
    async fn run_outcome_handler(&self, mut outcome_rx: mpsc::Receiver<(String, TurnOutcome)>) {
        while let Some((session_id, outcome)) = outcome_rx.recv().await {
            self.round_yield_flags.clear(&session_id);
            // Only drop steering messages targeted at the *finished* turn. We
            // must NOT clear the entire session buffer here: a user might have
            // legitimately submitted steering against a brand-new follow-up
            // turn that the dispatcher will pick up immediately after this
            // outcome is processed (race window between turn finalize and the
            // next turn starting). Targeting by turn_id keeps those alive.
            let _drained = self
                .steering_buffer
                .drain_for_turn(&session_id, outcome.turn_id());
            let suppressed_cancelled_reply =
                self.take_suppressed_cancelled_reply(&session_id, outcome.turn_id());

            let active_turn = self.active_turns.remove(&session_id).map(|(_, turn)| turn);
            if let Some(active_turn) = active_turn.as_ref() {
                if Self::should_skip_agent_session_reply(&outcome, suppressed_cancelled_reply) {
                    debug!(
                        "Skipping cancelled auto-reply because the source session explicitly cancelled its own SessionMessage request: session_id={}, turn_id={}",
                        session_id,
                        outcome.turn_id()
                    );
                } else {
                    self.forward_agent_session_reply(&session_id, active_turn, &outcome)
                        .await;
                }
            }

            let status = outcome.status();
            match outcome.queue_action() {
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
                            status,
                            session_id,
                            e
                        );
                    }
                }
                TurnOutcomeQueueAction::ClearQueue => {
                    debug!("Turn {}, clearing queue: session_id={}", status, session_id);
                    self.clear_queue(&session_id);
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_session_active_turn(source_session_id: &str) -> ActiveTurn {
        ActiveTurn {
            turn_id: "turn_1".to_string(),
            workspace_path: Some("/workspace".to_string()),
            policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession),
            reply_route: Some(AgentSessionReplyRoute {
                source_session_id: source_session_id.to_string(),
                source_workspace_path: "/source".to_string(),
            }),
        }
    }

    #[test]
    fn requester_matching_reply_route_suppresses_cancelled_reply() {
        let active_turn = agent_session_active_turn("session_a");
        assert!(active_turn.should_suppress_cancelled_reply_for_requester("session_a"));
        assert!(!active_turn.should_suppress_cancelled_reply_for_requester("session_c"));
    }

    #[test]
    fn cancelled_reply_is_skipped_only_when_suppressed() {
        let cancelled = TurnOutcome::Cancelled {
            turn_id: "turn_1".to_string(),
        };
        let completed = TurnOutcome::Completed {
            turn_id: "turn_1".to_string(),
            final_response: "done".to_string(),
        };

        assert!(DialogScheduler::should_skip_agent_session_reply(
            &cancelled, true
        ));
        assert!(!DialogScheduler::should_skip_agent_session_reply(
            &cancelled, false
        ));
        assert!(!DialogScheduler::should_skip_agent_session_reply(
            &completed, true
        ));
    }
}
