//! Scheduler owner decisions.

use bitfun_runtime_ports::{
    DialogQueuePriority, DialogRoundInjectionSource, DialogRoundPreemptSource,
    DialogSessionStateFact, DialogSubmissionPolicy, DialogTriggerSource, RoundInjection,
    RoundInjectionTarget,
};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackgroundDeliveryFacts {
    pub session_state: DialogSessionStateFact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundDeliveryAction {
    InjectIntoRunningTurn,
    SubmitAgentSessionFollowUp {
        queue_priority: DialogQueuePriority,
        skip_tool_confirmation: bool,
    },
}

impl BackgroundDeliveryAction {
    pub const fn follow_up_submission_policy(self) -> Option<DialogSubmissionPolicy> {
        match self {
            Self::InjectIntoRunningTurn => None,
            Self::SubmitAgentSessionFollowUp {
                queue_priority,
                skip_tool_confirmation,
            } => Some(DialogSubmissionPolicy::new(
                DialogTriggerSource::AgentSession,
                queue_priority,
                skip_tool_confirmation,
            )),
        }
    }
}

/// Used when no scheduler is wired (e.g. tests, isolated execution).
pub struct NoopDialogRoundPreemptSource;

impl DialogRoundPreemptSource for NoopDialogRoundPreemptSource {
    fn should_yield_after_round(&self, _session_id: &str) -> bool {
        false
    }

    fn clear_yield_after_round(&self, _session_id: &str) {}
}

/// Shared flag storage keyed by session; scheduler sets, engine reads and clears.
#[derive(Debug, Default)]
pub struct SessionRoundYieldFlags {
    inner: dashmap::DashMap<String, Arc<AtomicBool>>,
}

impl SessionRoundYieldFlags {
    pub fn request_yield(&self, session_id: &str) {
        self.inner
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .store(true, Ordering::SeqCst);
    }

    pub fn should_yield(&self, session_id: &str) -> bool {
        self.inner
            .get(session_id)
            .map(|r| r.value().load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    pub fn clear(&self, session_id: &str) {
        self.inner.remove(session_id);
    }
}

impl DialogRoundPreemptSource for SessionRoundYieldFlags {
    fn should_yield_after_round(&self, session_id: &str) -> bool {
        self.should_yield(session_id)
    }

    fn clear_yield_after_round(&self, session_id: &str) {
        self.clear(session_id);
    }
}

/// Used when no scheduler is wired (e.g. tests, isolated execution).
pub struct NoopDialogRoundInjectionSource;

impl DialogRoundInjectionSource for NoopDialogRoundInjectionSource {
    fn has_pending(&self, _session_id: &str, _turn_id: &str) -> bool {
        false
    }

    fn take_pending(&self, _session_id: &str, _turn_id: &str) -> Vec<RoundInjection> {
        Vec::new()
    }
}

#[derive(Clone)]
pub struct DialogRoundInjectionInterrupt {
    session_id: String,
    turn_id: String,
    source: Arc<dyn DialogRoundInjectionSource>,
}

impl std::fmt::Debug for DialogRoundInjectionInterrupt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DialogRoundInjectionInterrupt")
            .field("session_id", &self.session_id)
            .field("turn_id", &self.turn_id)
            .finish_non_exhaustive()
    }
}

impl DialogRoundInjectionInterrupt {
    pub fn new(
        session_id: String,
        turn_id: String,
        source: Arc<dyn DialogRoundInjectionSource>,
    ) -> Self {
        Self {
            session_id,
            turn_id,
            source,
        }
    }

    pub fn should_interrupt(&self) -> bool {
        self.source.has_pending(&self.session_id, &self.turn_id)
    }
}

/// Per-session FIFO buffer of round injections keyed by `session_id`.
#[derive(Debug, Default)]
pub struct SessionRoundInjectionBuffer {
    inner: dashmap::DashMap<String, Vec<RoundInjection>>,
}

impl SessionRoundInjectionBuffer {
    pub fn push(&self, session_id: &str, message: RoundInjection) {
        self.inner
            .entry(session_id.to_string())
            .or_default()
            .push(message);
    }

    /// Drain all messages eligible for the currently running turn. Exact-turn
    /// injections that target a different turn are retained until the targeted
    /// turn consumes them or the session is cleared.
    pub fn drain_for_turn(&self, session_id: &str, turn_id: &str) -> Vec<RoundInjection> {
        let Some(mut entry) = self.inner.get_mut(session_id) else {
            return Vec::new();
        };
        let mut taken = Vec::new();
        let mut keep = Vec::new();
        for msg in entry.drain(..) {
            match &msg.target {
                RoundInjectionTarget::ExactTurn(target_turn_id) if target_turn_id == turn_id => {
                    taken.push(msg);
                }
                RoundInjectionTarget::CurrentRunningTurn => taken.push(msg),
                RoundInjectionTarget::ExactTurn(_) => keep.push(msg),
            }
        }
        *entry = keep;
        taken
    }

    pub fn has_pending_for_turn(&self, session_id: &str, turn_id: &str) -> bool {
        self.inner
            .get(session_id)
            .map(|entry| {
                entry.iter().any(|msg| match &msg.target {
                    RoundInjectionTarget::ExactTurn(target_turn_id) => target_turn_id == turn_id,
                    RoundInjectionTarget::CurrentRunningTurn => true,
                })
            })
            .unwrap_or(false)
    }

    /// Drop all messages for a session (e.g. session deleted or unrecoverable error).
    pub fn clear(&self, session_id: &str) {
        self.inner.remove(session_id);
    }

    pub fn pending_count(&self, session_id: &str) -> usize {
        self.inner.get(session_id).map(|v| v.len()).unwrap_or(0)
    }
}

impl DialogRoundInjectionSource for SessionRoundInjectionBuffer {
    fn has_pending(&self, session_id: &str, turn_id: &str) -> bool {
        self.has_pending_for_turn(session_id, turn_id)
    }

    fn take_pending(&self, session_id: &str, turn_id: &str) -> Vec<RoundInjection> {
        self.drain_for_turn(session_id, turn_id)
    }
}

pub const fn resolve_background_delivery_action(
    facts: BackgroundDeliveryFacts,
) -> BackgroundDeliveryAction {
    match facts.session_state {
        DialogSessionStateFact::Processing => BackgroundDeliveryAction::InjectIntoRunningTurn,
        DialogSessionStateFact::Missing
        | DialogSessionStateFact::Idle
        | DialogSessionStateFact::Error => {
            let policy = DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession);
            BackgroundDeliveryAction::SubmitAgentSessionFollowUp {
                queue_priority: policy.queue_priority,
                skip_tool_confirmation: policy.skip_tool_confirmation,
            }
        }
    }
}

/// Outcome of a completed dialog turn, used to notify the concrete scheduler.
#[derive(Debug, Clone)]
pub enum TurnOutcome {
    /// Turn completed normally.
    Completed {
        turn_id: String,
        final_response: String,
    },
    /// Turn was cancelled by user.
    Cancelled { turn_id: String },
    /// Turn failed with an error.
    Failed { turn_id: String, error: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOutcomeQueueAction {
    DispatchNext,
    ClearQueue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOutcomeStatus {
    Completed,
    Cancelled,
    Failed,
}

impl TurnOutcomeStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for TurnOutcomeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TurnOutcome {
    pub fn turn_id(&self) -> &str {
        match self {
            Self::Completed { turn_id, .. }
            | Self::Cancelled { turn_id }
            | Self::Failed { turn_id, .. } => turn_id,
        }
    }

    pub fn status(&self) -> TurnOutcomeStatus {
        match self {
            Self::Completed { .. } => TurnOutcomeStatus::Completed,
            Self::Cancelled { .. } => TurnOutcomeStatus::Cancelled,
            Self::Failed { .. } => TurnOutcomeStatus::Failed,
        }
    }

    pub fn status_str(&self) -> &'static str {
        self.status().as_str()
    }

    pub fn reply_text(&self) -> String {
        match self {
            Self::Completed { final_response, .. } => {
                if final_response.trim().is_empty() {
                    "(no final text response)".to_string()
                } else {
                    final_response.clone()
                }
            }
            Self::Cancelled { .. } => {
                "The target session cancelled this request before producing a final answer."
                    .to_string()
            }
            Self::Failed { error, .. } => {
                format!("The target session failed to complete this request.\nError: {error}")
            }
        }
    }

    pub fn queue_action(&self) -> TurnOutcomeQueueAction {
        match self {
            Self::Completed { .. } | Self::Cancelled { .. } => TurnOutcomeQueueAction::DispatchNext,
            Self::Failed { .. } => TurnOutcomeQueueAction::ClearQueue,
        }
    }
}
