//! LSP request debouncer
//!
//! Prevents sending a burst of duplicate requests in a short time, improving performance and
//! stability.

use log::debug;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Request debouncer.
pub struct RequestDebouncer {
    /// Last request time (`uri + method -> last_time`).
    last_requests: Arc<RwLock<HashMap<String, Instant>>>,
    /// Debounce delay (milliseconds).
    debounce_ms: u64,
}

impl RequestDebouncer {
    /// Creates a new debouncer.
    pub fn new(debounce_ms: u64) -> Self {
        Self {
            last_requests: Arc::new(RwLock::new(HashMap::new())),
            debounce_ms,
        }
    }

    /// Returns whether a request should be sent.
    /// Returns `true` if it can be sent, or `false` if it should be skipped (too frequent).
    pub async fn should_send(&self, uri: &str, method: &str) -> bool {
        let key = format!("{}:{}", uri, method);
        let now = Instant::now();

        let mut requests = self.last_requests.write().await;

        if let Some(last_time) = requests.get(&key) {
            let elapsed = now.duration_since(*last_time);
            if elapsed < Duration::from_millis(self.debounce_ms) {
                debug!(
                    "Request debounced: {} ({}ms elapsed)",
                    key,
                    elapsed.as_millis()
                );
                return false;
            }
        }

        requests.insert(key, now);
        true
    }

    /// Cleans up expired records (call periodically).
    pub async fn cleanup(&self, max_age: Duration) {
        let mut requests = self.last_requests.write().await;
        let now = Instant::now();

        requests.retain(|_, last_time| now.duration_since(*last_time) < max_age);
    }
}
