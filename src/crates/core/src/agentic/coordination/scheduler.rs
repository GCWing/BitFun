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
use crate::agentic::session::SessionManager;
use dashmap::DashMap;
use log::{debug, info, warn};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::SystemTime;
use tokio::sync::mpsc;

const MAX_QUEUE_DEPTH: usize = 20;

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
    workspace_path: Option<String>,
    policy: DialogSubmissionPolicy,
    reply_route: Option<AgentSessionReplyRoute>,
}

impl ActiveTurn {
    fn from_queued_turn(turn: &QueuedTurn) -> Self {
        Self {
            workspace_path: turn.workspace_path.clone(),
            policy: turn.policy,
            reply_route: turn.reply_route.clone(),
        }
    }

    fn is_agent_session_request(&self) -> bool {
        self.policy.trigger_source == DialogTriggerSource::AgentSession
            && self.reply_route.is_some()
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
    /// Cloneable sender given to ConversationCoordinator for turn outcome notifications
    outcome_tx: mpsc::Sender<(String, TurnOutcome)>,
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
            outcome_tx,
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

    /// Submit a user message for a session.
    ///
    /// - Session idle, queue empty → dispatched immediately.
    /// - Session idle, queue non-empty → enqueued then highest-priority queued message dispatched.
    /// - Session processing → queued (up to MAX_QUEUE_DEPTH).
    /// - Session error → queue cleared, dispatched immediately.
    ///
    /// Returns `Err(String)` if the queue is full or the coordinator returns an error.
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
    ) -> Result<(), String> {
        let queued_turn = QueuedTurn {
            user_input,
            original_user_input,
            turn_id,
            agent_type,
            workspace_path,
            policy,
            reply_route,
            enqueued_at: SystemTime::now(),
        };
        let state = self
            .session_manager
            .get_session(&session_id)
            .map(|s| s.state.clone());

        match state {
            None => self.start_turn(&session_id, &queued_turn).await,

            Some(SessionState::Error { .. }) => {
                self.clear_queue(&session_id);
                self.start_turn(&session_id, &queued_turn).await
            }

            Some(SessionState::Idle) => {
                let queue_non_empty = self
                    .queues
                    .get(&session_id)
                    .map(|q| !q.is_empty())
                    .unwrap_or(false);

                if queue_non_empty {
                    self.enqueue(&session_id, queued_turn)?;
                    self.dispatch_next_if_idle(&session_id).await
                } else {
                    self.start_turn(&session_id, &queued_turn).await
                }
            }

            Some(SessionState::Processing { .. }) => {
                self.enqueue(&session_id, queued_turn)?;
                Ok(())
            }
        }
    }

    /// Number of messages currently queued for a session.
    pub fn queue_depth(&self, session_id: &str) -> usize {
        self.queues.get(session_id).map(|q| q.len()).unwrap_or(0)
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

    async fn start_turn(&self, session_id: &str, queued_turn: &QueuedTurn) -> Result<(), String> {
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
            .map_err(|e| e.to_string())?;

        self.active_turns.insert(
            session_id.to_string(),
            ActiveTurn::from_queued_turn(queued_turn),
        );
        Ok(())
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
            )
            .await
        {
            warn!(
                "Failed to forward agent-session reply: responder_session_id={}, source_session_id={}, error={}",
                responder_session_id, reply_route.source_session_id, error
            );
        }
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
        let state = self
            .session_manager
            .get_session(session_id)
            .map(|s| s.state.clone());
        if matches!(state, Some(SessionState::Processing { .. })) {
            return Ok(());
        }

        let Some(next_turn) = self.dequeue_next(session_id) else {
            return Ok(());
        };

        let remaining = self.queues.get(session_id).map(|q| q.len()).unwrap_or(0);
        info!(
            "Dispatching queued message: session_id={}, priority={:?}, remaining_queue_depth={}",
            session_id, next_turn.policy.queue_priority, remaining
        );

        if let Err(err) = self.start_turn(session_id, &next_turn).await {
            self.requeue_front(session_id, next_turn);
            return Err(err);
        }

        Ok(())
    }

    /// Background loop that receives turn outcome notifications from the coordinator.
    async fn run_outcome_handler(&self, mut outcome_rx: mpsc::Receiver<(String, TurnOutcome)>) {
        while let Some((session_id, outcome)) = outcome_rx.recv().await {
            let active_turn = self.active_turns.remove(&session_id).map(|(_, turn)| turn);
            if let Some(active_turn) = active_turn.as_ref() {
                self.forward_agent_session_reply(&session_id, active_turn, &outcome)
                    .await;
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
