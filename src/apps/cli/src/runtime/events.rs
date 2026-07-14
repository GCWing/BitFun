use std::sync::Arc;

use bitfun_core::agentic::events::EventQueue;
use bitfun_events::AgenticEventEnvelope;
use tokio::sync::broadcast;

struct EventQueueDrain {
    task: tokio::task::JoinHandle<()>,
}

impl EventQueueDrain {
    fn start(queue: Arc<EventQueue>) -> Self {
        let task = tokio::spawn(async move {
            loop {
                queue.wait_for_events().await;
                loop {
                    if queue.dequeue_configured_batch().await.is_empty() {
                        break;
                    }
                }
            }
        });
        Self { task }
    }
}

impl Drop for EventQueueDrain {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone)]
pub(crate) struct CliAgentEventSource {
    queue: Arc<EventQueue>,
    _drain: Arc<EventQueueDrain>,
}

impl CliAgentEventSource {
    pub(crate) fn new(queue: Arc<EventQueue>) -> Self {
        Self {
            _drain: Arc::new(EventQueueDrain::start(queue.clone())),
            queue,
        }
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<AgenticEventEnvelope> {
        self.queue.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bitfun_core::agentic::events::{EventQueue, EventQueueConfig};
    use bitfun_events::AgenticEvent;

    use super::CliAgentEventSource;

    #[tokio::test]
    async fn subscribers_observe_every_event_while_the_legacy_queue_stays_bounded() {
        let queue = Arc::new(EventQueue::new(EventQueueConfig {
            max_queue_size: 4,
            batch_size: 2,
        }));
        let source = CliAgentEventSource::new(queue.clone());
        let mut first = source.subscribe();
        let mut second = source.subscribe();

        for index in 0..32 {
            queue
                .enqueue(
                    AgenticEvent::SessionStateChanged {
                        session_id: "session-1".to_string(),
                        new_state: format!("state-{index}"),
                    },
                    None,
                )
                .await
                .expect("enqueue event");
        }

        let mut first_event = None;
        let mut second_event = None;
        for _ in 0..32 {
            first_event = Some(
                tokio::time::timeout(std::time::Duration::from_secs(1), first.recv())
                    .await
                    .expect("first subscriber must not stall")
                    .expect("first subscriber event"),
            );
            second_event = Some(
                tokio::time::timeout(std::time::Duration::from_secs(1), second.recv())
                    .await
                    .expect("second subscriber must not stall")
                    .expect("second subscriber event"),
            );
        }

        let first_event = first_event.expect("last first event");
        let second_event = second_event.expect("last second event");
        assert_eq!(first_event.id, second_event.id);
        assert!(matches!(
            first_event.event,
            AgenticEvent::SessionStateChanged { ref new_state, .. } if new_state == "state-31"
        ));
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !queue.is_empty().await {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("queue drainer must keep the legacy queue bounded");
    }
}
