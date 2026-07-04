//! MCP reconnect scheduling state.

use crate::mcp::server::compute_mcp_backoff_delay;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy)]
struct MCPReconnectPolicy {
    poll_interval: Duration,
    base_delay: Duration,
    max_delay: Duration,
}

impl MCPReconnectPolicy {
    #[cfg(test)]
    fn new(poll_interval: Duration, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            poll_interval,
            base_delay,
            max_delay,
        }
    }

    fn poll_interval(&self) -> Duration {
        self.poll_interval
    }
}

impl Default for MCPReconnectPolicy {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            base_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Clone)]
struct MCPReconnectAttemptState {
    attempts: u32,
    next_retry_at: Instant,
}

impl MCPReconnectAttemptState {
    fn new(now: Instant) -> Self {
        Self {
            attempts: 0,
            next_retry_at: now,
        }
    }
}

pub struct MCPReconnectTracker {
    policy: MCPReconnectPolicy,
    states: RwLock<HashMap<String, MCPReconnectAttemptState>>,
}

impl MCPReconnectTracker {
    fn new(policy: MCPReconnectPolicy) -> Self {
        Self {
            policy,
            states: RwLock::new(HashMap::new()),
        }
    }

    pub fn poll_interval(&self) -> Duration {
        self.policy.poll_interval()
    }

    pub async fn has_pending(&self) -> bool {
        !self.states.read().await.is_empty()
    }

    pub async fn next_due_attempt(&self, server_id: &str) -> Option<(u32, Duration)> {
        let now = Instant::now();
        let mut states = self.states.write().await;
        let state = states
            .entry(server_id.to_string())
            .or_insert_with(|| MCPReconnectAttemptState::new(now));

        if now < state.next_retry_at {
            return None;
        }

        state.attempts += 1;
        let next_delay = compute_mcp_backoff_delay(
            self.policy.base_delay,
            self.policy.max_delay,
            state.attempts,
        );
        state.next_retry_at = now + next_delay;

        Some((state.attempts, next_delay))
    }

    pub async fn clear(&self, server_id: &str) {
        self.states.write().await.remove(server_id);
    }

    pub async fn clear_all(&self) {
        self.states.write().await.clear();
    }
}

impl Default for MCPReconnectTracker {
    fn default() -> Self {
        Self::new(MCPReconnectPolicy::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tracker_schedules_first_attempt_and_blocks_until_delay_expires() {
        let tracker = MCPReconnectTracker::new(MCPReconnectPolicy::new(
            Duration::from_millis(1),
            Duration::from_secs(2),
            Duration::from_secs(60),
        ));

        let (attempt_number, next_delay) = tracker.next_due_attempt("server-a").await.unwrap();
        assert_eq!(attempt_number, 1);
        assert_eq!(next_delay, Duration::from_secs(2));
        assert!(tracker.next_due_attempt("server-a").await.is_none());
    }

    #[tokio::test]
    async fn tracker_can_clear_pending_state() {
        let tracker = MCPReconnectTracker::default();

        assert!(!tracker.has_pending().await);
        assert!(tracker.next_due_attempt("server-a").await.is_some());
        assert!(tracker.has_pending().await);

        tracker.clear("server-a").await;
        assert!(!tracker.has_pending().await);
    }
}
