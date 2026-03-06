//! Dialog scheduler
//!
//! Message queue manager that automatically dispatches queued messages
//! when the target session becomes idle.
//!
//! Acts as the primary entry point for all user-facing message submissions,
//! wrapping ConversationCoordinator with:
//! - Per-session FIFO queue (max 20 messages)
//! - 1-second debounce after session becomes idle (resets on each new incoming message)
//! - Automatic message merging when queue has multiple entries
//! - Queue cleared on cancel or error

use super::coordinator::{ConversationCoordinator, DialogTriggerSource, TurnOutcome};
use crate::agentic::core::SessionState;
use crate::agentic::session::SessionManager;
use dashmap::DashMap;
use log::{debug, info, warn};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::SystemTime;
use tokio::sync::mpsc;
use tokio::task::AbortHandle;
use tokio::time::Duration;

const MAX_QUEUE_DEPTH: usize = 20;
const DEBOUNCE_DELAY: Duration = Duration::from_secs(1);

/// A message waiting to be dispatched to the coordinator
#[derive(Debug)]
pub struct QueuedTurn {
    pub user_input: String,
    pub turn_id: Option<String>,
    pub agent_type: String,
    pub trigger_source: DialogTriggerSource,
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
    /// Per-session FIFO message queues
    queues: Arc<DashMap<String, std::collections::VecDeque<QueuedTurn>>>,
    /// Per-session pending debounce task handles (present = debounce window active)
    debounce_handles: Arc<DashMap<String, AbortHandle>>,
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
            debounce_handles: Arc::new(DashMap::new()),
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
    /// - Session idle, no debounce window active → dispatched immediately.
    /// - Session idle, debounce window active (collecting messages) → queued, timer reset.
    /// - Session processing → queued (up to MAX_QUEUE_DEPTH).
    /// - Session error → queue cleared, dispatched immediately.
    ///
    /// Returns `Err(String)` if the queue is full or the coordinator returns an error.
    pub async fn submit(
        &self,
        session_id: String,
        user_input: String,
        turn_id: Option<String>,
        agent_type: String,
        trigger_source: DialogTriggerSource,
    ) -> Result<(), String> {
        let state = self
            .session_manager
            .get_session(&session_id)
            .map(|s| s.state.clone());

        match state {
            None => {
                self.coordinator
                    .start_dialog_turn(
                        session_id,
                        user_input,
                        turn_id,
                        agent_type,
                        trigger_source,
                    )
                    .await
                    .map_err(|e| e.to_string())
            }

            Some(SessionState::Error { .. }) => {
                self.clear_queue_and_debounce(&session_id);
                self.coordinator
                    .start_dialog_turn(
                        session_id,
                        user_input,
                        turn_id,
                        agent_type,
                        trigger_source,
                    )
                    .await
                    .map_err(|e| e.to_string())
            }

            Some(SessionState::Idle) => {
                let in_debounce = self.debounce_handles.contains_key(&session_id);
                let queue_non_empty = self
                    .queues
                    .get(&session_id)
                    .map(|q| !q.is_empty())
                    .unwrap_or(false);

                if in_debounce || queue_non_empty {
                    self.enqueue(
                        &session_id,
                        user_input,
                        turn_id,
                        agent_type,
                        trigger_source,
                    )?;
                    self.schedule_debounce(session_id);
                    Ok(())
                } else {
                    self.coordinator
                        .start_dialog_turn(
                            session_id,
                            user_input,
                            turn_id,
                            agent_type,
                            trigger_source,
                        )
                        .await
                        .map_err(|e| e.to_string())
                }
            }

            Some(SessionState::Processing { .. }) => {
                self.enqueue(
                    &session_id,
                    user_input,
                    turn_id,
                    agent_type,
                    trigger_source,
                )?;
                Ok(())
            }
        }
    }

    /// Number of messages currently queued for a session.
    pub fn queue_depth(&self, session_id: &str) -> usize {
        self.queues.get(session_id).map(|q| q.len()).unwrap_or(0)
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn enqueue(
        &self,
        session_id: &str,
        user_input: String,
        turn_id: Option<String>,
        agent_type: String,
        trigger_source: DialogTriggerSource,
    ) -> Result<(), String> {
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
            .push_back(QueuedTurn {
                user_input,
                turn_id,
                agent_type,
                trigger_source,
                enqueued_at: SystemTime::now(),
            });

        let new_len = self.queues.get(session_id).map(|q| q.len()).unwrap_or(0);
        debug!(
            "Message queued: session_id={}, queue_depth={}",
            session_id, new_len
        );
        Ok(())
    }

    fn clear_queue_and_debounce(&self, session_id: &str) {
        if let Some((_, handle)) = self.debounce_handles.remove(session_id) {
            handle.abort();
        }
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

    /// Start (or restart) the 1-second debounce timer for a session.
    /// When the timer fires, all queued messages are merged and dispatched.
    fn schedule_debounce(&self, session_id: String) {
        // Cancel the existing timer (if any)
        if let Some((_, old)) = self.debounce_handles.remove(&session_id) {
            old.abort();
        }

        let queues = Arc::clone(&self.queues);
        let coordinator = Arc::clone(&self.coordinator);
        let debounce_handles = Arc::clone(&self.debounce_handles);
        let session_id_clone = session_id.clone();

        let join_handle = tokio::spawn(async move {
            tokio::time::sleep(DEBOUNCE_DELAY).await;

            // Remove our own handle - we are now executing
            debounce_handles.remove(&session_id_clone);

            // Drain all queued messages
            let messages: Vec<QueuedTurn> = {
                let mut entry = queues.entry(session_id_clone.clone()).or_default();
                entry.drain(..).collect()
            };

            if messages.is_empty() {
                return;
            }

            info!(
                "Dispatching {} queued message(s) after debounce: session_id={}",
                messages.len(),
                session_id_clone
            );

            let (merged_input, turn_id, agent_type, trigger_source) =
                merge_messages(messages);

            if let Err(e) = coordinator
                .start_dialog_turn(
                    session_id_clone.clone(),
                    merged_input,
                    turn_id,
                    agent_type,
                    trigger_source,
                )
                .await
            {
                warn!(
                    "Failed to dispatch queued messages: session_id={}, error={}",
                    session_id_clone, e
                );
            }
        });

        // Store abort handle; drop the JoinHandle (task is detached but remains abortable)
        self.debounce_handles
            .insert(session_id, join_handle.abort_handle());
    }

    /// Background loop that receives turn outcome notifications from the coordinator.
    async fn run_outcome_handler(&self, mut outcome_rx: mpsc::Receiver<(String, TurnOutcome)>) {
        while let Some((session_id, outcome)) = outcome_rx.recv().await {
            match outcome {
                TurnOutcome::Completed => {
                    let has_queued = self
                        .queues
                        .get(&session_id)
                        .map(|q| !q.is_empty())
                        .unwrap_or(false);

                    if has_queued {
                        debug!(
                            "Turn completed, queue non-empty, starting debounce: session_id={}",
                            session_id
                        );
                        self.schedule_debounce(session_id);
                    }
                }
                TurnOutcome::Cancelled => {
                    debug!("Turn cancelled, clearing queue: session_id={}", session_id);
                    self.clear_queue_and_debounce(&session_id);
                }
                TurnOutcome::Failed => {
                    debug!("Turn failed, clearing queue: session_id={}", session_id);
                    self.clear_queue_and_debounce(&session_id);
                }
            }
        }
    }
}

/// Merge multiple queued turns into a single user input string.
///
/// Single message → returned as-is (no wrapping).
/// Multiple messages → formatted as:
/// ```text
/// [Queued messages while agent was busy]
///
/// ---
/// Queued #1
/// <first message>
///
/// ---
/// Queued #2
/// <second message>
/// ```
fn merge_messages(messages: Vec<QueuedTurn>) -> (String, Option<String>, String, DialogTriggerSource) {
    if messages.len() == 1 {
        let m = messages.into_iter().next().unwrap();
        return (
            m.user_input,
            m.turn_id,
            m.agent_type,
            m.trigger_source,
        );
    }

    let agent_type = messages[0].agent_type.clone();
    let trigger_source = messages[0].trigger_source;

    let entries: Vec<String> = messages
        .iter()
        .enumerate()
        .map(|(i, m)| format!("---\nQueued #{}\n{}", i + 1, m.user_input))
        .collect();

    let merged = format!(
        "[Queued messages while agent was busy]\n\n{}",
        entries.join("\n\n")
    );

    (merged, None, agent_type, trigger_source)
}

// ── Global instance ──────────────────────────────────────────────────────────

static GLOBAL_SCHEDULER: OnceLock<Arc<DialogScheduler>> = OnceLock::new();

pub fn get_global_scheduler() -> Option<Arc<DialogScheduler>> {
    GLOBAL_SCHEDULER.get().cloned()
}

pub fn set_global_scheduler(scheduler: Arc<DialogScheduler>) {
    let _ = GLOBAL_SCHEDULER.set(scheduler);
}
