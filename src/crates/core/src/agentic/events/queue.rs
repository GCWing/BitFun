//! Event Queue
//!
//! Provides priority queue and batch processing functionality

use super::types::{AgenticEvent, EventEnvelope, EventPriority};
use crate::util::errors::BitFunResult;
use log::{debug, trace, warn};
use std::collections::BinaryHeap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, Notify};

const EVENT_BROADCAST_BUFFER: usize = 1024;
const SLOW_EVENT_QUEUE_LATENCY_MS: u128 = 250;

/// Event queue configuration
#[derive(Debug, Clone)]
pub struct EventQueueConfig {
    pub max_queue_size: usize,
    pub batch_size: usize,
}

impl Default for EventQueueConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10000,
            batch_size: 10, // Reduce to 10 to reduce latency
        }
    }
}

/// Queue statistics
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    pub pending_events: usize,
    pub total_enqueued: u64,
    pub total_processed: u64,
}

/// Event queue
///
/// Core functionality:
/// - Priority sorting (Critical > High > Normal > Low)
/// - Batch processing (reduce frontend pressure)
/// - Event driven (Notify mechanism)
pub struct EventQueue {
    /// Priority queue
    queue: Arc<Mutex<BinaryHeap<std::cmp::Reverse<EventEnvelope>>>>,

    /// Notifier (used to wake up waiting consumers)
    notify: Arc<Notify>,

    /// Broadcast stream for non-consuming subscribers.
    broadcast_tx: broadcast::Sender<EventEnvelope>,

    /// Configuration
    config: EventQueueConfig,

    /// Statistics
    stats: Arc<Mutex<QueueStats>>,
}

impl EventQueue {
    pub fn new(config: EventQueueConfig) -> Self {
        let (broadcast_tx, _) = broadcast::channel(EVENT_BROADCAST_BUFFER);
        Self {
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
            notify: Arc::new(Notify::new()),
            broadcast_tx,
            config,
            stats: Arc::new(Mutex::new(QueueStats::default())),
        }
    }

    /// Enqueue event
    pub async fn enqueue(
        &self,
        event: AgenticEvent,
        priority: Option<EventPriority>,
    ) -> BitFunResult<String> {
        let priority = priority.unwrap_or_else(|| event.default_priority());
        let envelope = EventEnvelope::new(event, priority);
        let event_id = envelope.id.clone();

        // Check queue size
        {
            let queue = self.queue.lock().await;
            if queue.len() >= self.config.max_queue_size {
                warn!("Event queue full, dropping event: event_id={}", event_id);
                return Ok(event_id);
            }
        }

        // Add to queue
        {
            let mut queue = self.queue.lock().await;
            queue.push(std::cmp::Reverse(envelope.clone()));
        }

        let _ = self.broadcast_tx.send(envelope);

        // Update statistics: get queue size first, then update statistics (avoid getting queue lock while holding stats lock)
        let queue_len = self.queue.lock().await.len();
        {
            let mut stats = self.stats.lock().await;
            stats.total_enqueued += 1;
            stats.pending_events = queue_len;
        }

        // Notify waiting consumers
        self.notify.notify_one();

        trace!(
            "Event enqueued: event_id={}, priority={:?}",
            event_id,
            priority
        );

        Ok(event_id)
    }

    /// Dequeue batch of events
    pub async fn dequeue_batch(&self, max_size: usize) -> Vec<EventEnvelope> {
        let mut batch = Vec::new();
        let mut queue = self.queue.lock().await;

        let take_count = max_size.min(queue.len());

        for _ in 0..take_count {
            if let Some(std::cmp::Reverse(envelope)) = queue.pop() {
                batch.push(envelope);
            }
        }
        let remaining_queue_len = queue.len();
        drop(queue);

        if let Some((max_age_ms, event_id, priority)) = batch
            .iter()
            .filter_map(|envelope| {
                envelope
                    .timestamp
                    .elapsed()
                    .ok()
                    .map(|age| (age.as_millis(), envelope.id.as_str(), envelope.priority))
            })
            .max_by_key(|(age_ms, _, _)| *age_ms)
        {
            if max_age_ms >= SLOW_EVENT_QUEUE_LATENCY_MS {
                warn!(
                    "Slow agentic event queue delivery: max_age_ms={}, batch_size={}, remaining_queue_len={}, event_id={}, priority={:?}",
                    max_age_ms,
                    batch.len(),
                    remaining_queue_len,
                    event_id,
                    priority
                );
            }
        }

        // Update statistics
        if !batch.is_empty() {
            let mut stats = self.stats.lock().await;
            stats.total_processed += batch.len() as u64;
            stats.pending_events = remaining_queue_len;
        }

        batch
    }

    /// Dequeue a batch using the queue's configured batch size.
    pub async fn dequeue_configured_batch(&self) -> Vec<EventEnvelope> {
        self.dequeue_batch(self.config.batch_size).await
    }

    /// Subscribe to events without consuming them from the queue.
    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.broadcast_tx.subscribe()
    }

    #[cfg(test)]
    pub(crate) async fn lock_queue_for_test(
        &self,
    ) -> tokio::sync::MutexGuard<'_, BinaryHeap<std::cmp::Reverse<EventEnvelope>>> {
        self.queue.lock().await
    }

    /// Clear all events for a session
    pub async fn clear_session(&self, session_id: &str) -> BitFunResult<()> {
        // Remove all events for this session from the queue
        let queue_len = {
            let mut queue = self.queue.lock().await;
            let mut new_queue = BinaryHeap::new();

            while let Some(std::cmp::Reverse(envelope)) = queue.pop() {
                if envelope.event.session_id() != Some(session_id) {
                    new_queue.push(std::cmp::Reverse(envelope));
                }
            }

            *queue = new_queue;
            queue.len() // Get size before releasing queue lock
        };

        // Update statistics: use the size obtained earlier
        {
            let mut stats = self.stats.lock().await;
            stats.pending_events = queue_len;
        }

        debug!("Cleared all events for session: session_id={}", session_id);

        Ok(())
    }

    /// Get queue statistics
    pub async fn stats(&self) -> QueueStats {
        self.stats.lock().await.clone()
    }

    /// Wait for events (used for consumers)
    pub async fn wait_for_events(&self) {
        self.notify.notified().await;
    }

    /// Get queue size
    pub async fn len(&self) -> usize {
        self.queue.lock().await.len()
    }

    /// Check if the queue is empty
    pub async fn is_empty(&self) -> bool {
        self.queue.lock().await.is_empty()
    }
}
