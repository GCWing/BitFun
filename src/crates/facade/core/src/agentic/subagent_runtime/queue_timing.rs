//! Queue wait timing shared by subagent runtimes.
//!
//! Queue wait is tracked separately from execution time so callers can pause
//! admission without consuming the downstream task timeout.  The type has no
//! knowledge of reviewer roles, queue reasons, events, or retry policy; those
//! concerns remain in the feature adapter that owns the product behavior.

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub(crate) struct QueueWaitTimer {
    started_at: Instant,
    paused_since: Option<Instant>,
    paused_total: Duration,
}

impl QueueWaitTimer {
    pub(crate) fn start(now: Instant) -> Self {
        Self {
            started_at: now,
            paused_since: None,
            paused_total: Duration::ZERO,
        }
    }

    pub(crate) fn snapshot(&self, now: Instant) -> QueueWaitSnapshot {
        let active_pause = self
            .paused_since
            .map(|paused_at| now.saturating_duration_since(paused_at))
            .unwrap_or_default();
        let queue_elapsed = now
            .saturating_duration_since(self.started_at)
            .saturating_sub(self.paused_total)
            .saturating_sub(active_pause);

        QueueWaitSnapshot {
            queue_elapsed,
            queue_elapsed_ms: u64::try_from(queue_elapsed.as_millis()).unwrap_or(u64::MAX),
        }
    }

    pub(crate) fn pause(&mut self, now: Instant) {
        if self.paused_since.is_none() {
            self.paused_since = Some(now);
        }
    }

    pub(crate) fn continue_now(&mut self, now: Instant) {
        if let Some(paused_at) = self.paused_since.take() {
            self.paused_total += now.saturating_duration_since(paused_at);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueueWaitSnapshot {
    pub(crate) queue_elapsed: Duration,
    pub(crate) queue_elapsed_ms: u64,
}

impl QueueWaitSnapshot {
    pub(crate) fn is_expired(self, max_wait: Duration) -> bool {
        self.queue_elapsed >= max_wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_elapsed_excludes_paused_duration() {
        let start = Instant::now();
        let mut timer = QueueWaitTimer::start(start);

        let before_pause = start + Duration::from_millis(1_200);
        assert_eq!(
            timer.snapshot(before_pause).queue_elapsed,
            Duration::from_millis(1_200)
        );

        timer.pause(before_pause);
        let during_pause = start + Duration::from_millis(5_200);
        assert_eq!(
            timer.snapshot(during_pause).queue_elapsed,
            Duration::from_millis(1_200)
        );

        timer.continue_now(during_pause);
        let after_resume = start + Duration::from_millis(6_200);
        let snapshot = timer.snapshot(after_resume);
        assert_eq!(snapshot.queue_elapsed, Duration::from_millis(2_200));
        assert_eq!(snapshot.queue_elapsed_ms, 2_200);
    }

    #[test]
    fn pause_and_continue_are_idempotent() {
        let start = Instant::now();
        let mut timer = QueueWaitTimer::start(start);

        let first_pause = start + Duration::from_millis(500);
        let second_pause = start + Duration::from_millis(900);
        timer.pause(first_pause);
        timer.pause(second_pause);

        let resume = start + Duration::from_millis(1_500);
        timer.continue_now(resume);
        timer.continue_now(resume + Duration::from_millis(300));

        let snapshot = timer.snapshot(start + Duration::from_millis(2_000));
        assert_eq!(snapshot.queue_elapsed, Duration::from_millis(1_000));
        assert!(!snapshot.is_expired(Duration::from_millis(1_001)));
        assert!(snapshot.is_expired(Duration::from_millis(1_000)));
    }
}
