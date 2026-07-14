//! Provider-neutral runtime event queue.

use crate::event_bus::EventBusResult;
use bitfun_agent_stream::StreamEventSink;
use bitfun_events::{
    AgenticEvent, AgenticEventEnvelope as EventEnvelope, AgenticEventPriority as EventPriority,
};
use log::{debug, trace, warn};
use std::collections::BinaryHeap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, Notify};

const MIN_EVENT_BROADCAST_BUFFER: usize = 1024;
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
        // Keep subscriber backlog capacity at least as large as the existing
        // dequeue queue budget so switching a consumer to broadcast does not
        // reduce the amount of burst traffic it can tolerate.
        let broadcast_capacity = config.max_queue_size.max(MIN_EVENT_BROADCAST_BUFFER);
        let (broadcast_tx, _) = broadcast::channel(broadcast_capacity);
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
    ) -> EventBusResult<String> {
        let priority = priority.unwrap_or_else(|| event.default_priority());
        let envelope = EventEnvelope::new(event, priority);
        let event_id = envelope.id.clone();

        let (queue_len, queued) = {
            let mut queue = self.queue.lock().await;
            if queue.len() >= self.config.max_queue_size {
                warn!(
                    "Event queue full, skipping legacy queue storage: event_id={}",
                    event_id
                );
                (queue.len(), false)
            } else {
                queue.push(std::cmp::Reverse(envelope.clone()));
                (queue.len(), true)
            }
        };

        // Broadcast delivery is authoritative for non-consuming runtime
        // subscribers and must not depend on capacity in the legacy dequeue
        // buffer.
        let _ = self.broadcast_tx.send(envelope);

        {
            let mut stats = self.stats.lock().await;
            stats.total_enqueued += 1;
            stats.pending_events = queue_len;
        }

        if queued {
            self.notify.notify_one();
        }

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

    /// Clear all events for a session
    pub async fn clear_session(&self, session_id: &str) -> EventBusResult<()> {
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

#[async_trait::async_trait]
impl StreamEventSink for EventQueue {
    async fn enqueue(&self, event: AgenticEvent, priority: Option<EventPriority>) {
        let _ = EventQueue::enqueue(self, event, priority).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{EventQueue, EventQueueConfig};
    use bitfun_events::AgenticEvent;
    use std::sync::Arc;
    use tokio::sync::Barrier;

    #[tokio::test]
    async fn full_legacy_queue_does_not_drop_broadcast_delivery() {
        let queue = EventQueue::new(EventQueueConfig {
            max_queue_size: 1,
            batch_size: 1,
        });
        let mut events = queue.subscribe();

        for session_id in ["first", "second"] {
            queue
                .enqueue(
                    AgenticEvent::SessionStateChanged {
                        session_id: session_id.to_string(),
                        new_state: "idle".to_string(),
                    },
                    None,
                )
                .await
                .expect("event should enqueue");
        }

        assert_eq!(queue.len().await, 1);
        assert_eq!(
            events
                .recv()
                .await
                .expect("first broadcast")
                .event
                .session_id(),
            Some("first")
        );
        assert_eq!(
            events
                .recv()
                .await
                .expect("second broadcast")
                .event
                .session_id(),
            Some("second")
        );
    }

    #[tokio::test]
    async fn default_sized_broadcast_preserves_bursts_above_legacy_1024_limit() {
        let queue = EventQueue::new(EventQueueConfig::default());
        let mut events = queue.subscribe();
        const EVENT_COUNT: usize = 2048;

        for index in 0..EVENT_COUNT {
            queue
                .enqueue(
                    AgenticEvent::SessionStateChanged {
                        session_id: "session".to_string(),
                        new_state: index.to_string(),
                    },
                    None,
                )
                .await
                .expect("event should enqueue");
        }

        for expected in 0..EVENT_COUNT {
            let envelope = events.recv().await.expect("burst event must be retained");
            assert!(matches!(
                envelope.event,
                AgenticEvent::SessionStateChanged { ref new_state, .. }
                    if new_state == &expected.to_string()
            ));
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_publishers_have_one_order_for_all_subscribers() {
        const EVENT_COUNT: usize = 64;
        let queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let mut first = queue.subscribe();
        let mut second = queue.subscribe();
        let barrier = Arc::new(Barrier::new(EVENT_COUNT));
        let mut tasks = Vec::with_capacity(EVENT_COUNT);

        for index in 0..EVENT_COUNT {
            let queue = queue.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                queue
                    .enqueue(
                        AgenticEvent::SessionStateChanged {
                            session_id: format!("event-{index}"),
                            new_state: "idle".to_string(),
                        },
                        None,
                    )
                    .await
                    .expect("event should enqueue")
            }));
        }
        for task in tasks {
            task.await.expect("publisher should complete");
        }

        let mut first_ids = Vec::with_capacity(EVENT_COUNT);
        let mut second_ids = Vec::with_capacity(EVENT_COUNT);
        for _ in 0..EVENT_COUNT {
            first_ids.push(first.recv().await.expect("first broadcast").id);
            second_ids.push(second.recv().await.expect("second broadcast").id);
        }
        assert_eq!(first_ids, second_ids);
    }
}
