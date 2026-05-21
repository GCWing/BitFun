//! Yield dialog execution at model-round boundaries when a new user message is queued.
//!
//! The [`DialogRoundPreemptSource`] is implemented by [`DialogScheduler`](super::scheduler::DialogScheduler)
//! and read by [`ExecutionEngine`](super::execution::ExecutionEngine) after each completed model round.
//!
//! In addition, the [`DialogRoundInjectionSource`] trait is read by the engine at the same
//! round boundary to retrieve any pending round injections that should be injected
//! into the current dialog turn (Codex-style mid-turn injection) without ending the turn.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

/// Observes whether the current dialog turn should end after the latest model round
/// (so a queued user message can start as a new turn).
pub trait DialogRoundPreemptSource: Send + Sync {
    fn should_yield_after_round(&self, session_id: &str) -> bool;
    fn clear_yield_after_round(&self, session_id: &str);
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

// ── Round-boundary injection ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundInjectionKind {
    UserSteering,
    BackgroundSubagentResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundInjectionTarget {
    /// Only inject into the exact targeted running turn. If that turn already
    /// finished, the injection is ignored.
    ExactTurn(String),
    /// Inject into whichever turn is currently running for the session.
    CurrentRunningTurn,
}

/// A message to inject into the currently running dialog turn at the next
/// model-round boundary. Produced by the scheduler/coordinator and consumed by
/// [`ExecutionEngine`](super::execution::ExecutionEngine) before each new round.
#[derive(Debug, Clone)]
pub struct RoundInjection {
    pub id: String,
    pub kind: RoundInjectionKind,
    /// Injection target routing policy.
    pub target: RoundInjectionTarget,
    pub content: String,
    /// Original (pre-rendering) text from the user, for UI display when the rendered
    /// `content` differs (e.g. when wrapped with a system reminder envelope).
    pub display_content: String,
    pub created_at: SystemTime,
}

/// Observes whether any round injections are pending for a given (session, turn).
pub trait DialogRoundInjectionSource: Send + Sync {
    /// Check whether the given running turn has pending injections without
    /// consuming it. This lets tool execution stop at a safe boundary while the
    /// execution engine remains responsible for draining and injecting the
    /// messages into the next model round.
    fn has_pending(&self, session_id: &str, turn_id: &str) -> bool;

    /// Drain all pending injections targeted at the given dialog turn.
    /// Implementations must be safe to call concurrently from multiple round boundaries.
    fn take_pending(&self, session_id: &str, turn_id: &str) -> Vec<RoundInjection>;
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
/// Messages are appended via [`SessionRoundInjectionBuffer::push`] and drained at round boundaries.
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

#[cfg(test)]
mod steering_tests {
    use super::*;

    fn exact_turn_msg(turn_id: &str, content: &str) -> RoundInjection {
        RoundInjection {
            id: uuid::Uuid::new_v4().to_string(),
            kind: RoundInjectionKind::UserSteering,
            target: RoundInjectionTarget::ExactTurn(turn_id.to_string()),
            content: content.to_string(),
            display_content: content.to_string(),
            created_at: SystemTime::now(),
        }
    }

    fn current_turn_msg(content: &str) -> RoundInjection {
        RoundInjection {
            id: uuid::Uuid::new_v4().to_string(),
            kind: RoundInjectionKind::BackgroundSubagentResult,
            target: RoundInjectionTarget::CurrentRunningTurn,
            content: content.to_string(),
            display_content: content.to_string(),
            created_at: SystemTime::now(),
        }
    }

    #[test]
    fn drain_for_turn_returns_only_matching_turn_messages_in_fifo_order() {
        let buf = SessionRoundInjectionBuffer::default();
        buf.push("s1", exact_turn_msg("turn_a", "first"));
        buf.push("s1", exact_turn_msg("turn_b", "for_b_only"));
        buf.push("s1", exact_turn_msg("turn_a", "second"));

        assert!(buf.has_pending_for_turn("s1", "turn_a"));
        assert!(buf.has_pending_for_turn("s1", "turn_b"));
        assert!(!buf.has_pending_for_turn("s1", "turn_missing"));

        let drained = buf.drain_for_turn("s1", "turn_a");
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].content, "first");
        assert_eq!(drained[1].content, "second");

        // The unrelated turn_b entry must remain.
        assert_eq!(buf.pending_count("s1"), 1);
        let drained_b = buf.drain_for_turn("s1", "turn_b");
        assert_eq!(drained_b.len(), 1);
        assert_eq!(drained_b[0].content, "for_b_only");
        assert_eq!(buf.pending_count("s1"), 0);
        assert!(!buf.has_pending_for_turn("s1", "turn_b"));
    }

    #[test]
    fn drain_for_turn_on_empty_session_returns_empty() {
        let buf = SessionRoundInjectionBuffer::default();
        assert!(buf.drain_for_turn("missing", "turn_a").is_empty());
    }

    #[test]
    fn clear_drops_all_pending_for_session() {
        let buf = SessionRoundInjectionBuffer::default();
        buf.push("s1", exact_turn_msg("turn_a", "x"));
        buf.push("s1", exact_turn_msg("turn_b", "y"));
        buf.clear("s1");
        assert_eq!(buf.pending_count("s1"), 0);
        assert!(buf.drain_for_turn("s1", "turn_a").is_empty());
    }

    #[test]
    fn current_running_turn_messages_are_delivered_to_active_turn() {
        let buf = SessionRoundInjectionBuffer::default();
        buf.push("s1", current_turn_msg("background result"));

        assert!(buf.has_pending_for_turn("s1", "turn_a"));

        let drained = buf.drain_for_turn("s1", "turn_a");
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].content, "background result");
        assert_eq!(buf.pending_count("s1"), 0);
    }
}
