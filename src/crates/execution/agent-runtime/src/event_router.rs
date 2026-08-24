//! Provider-neutral runtime event router.

use crate::event_bus::{EventBusResult, EventSubscriberResult};
use bitfun_events::{AgenticEvent, AgenticEventEnvelope as EventEnvelope};
use dashmap::DashMap;
use log::{debug, trace, warn};
use std::sync::Arc;

/// Event subscriber trait
///
/// Used for internal system subscribers (e.g. logging system, monitoring system, etc.)
#[async_trait::async_trait]
pub trait EventSubscriber: Send + Sync + 'static {
    async fn on_event(&self, event: &AgenticEvent) -> EventSubscriberResult;

    /// Default forwards to [`Self::on_event`]. Override only when origin
    /// isolation is required (for example Cron).
    async fn on_envelope(&self, envelope: &EventEnvelope) -> EventSubscriberResult {
        self.on_event(&envelope.event).await
    }
}

/// Event router
///
/// Core functionality:
/// - Manage internal subscribers
/// - Distribute events to all subscribers
pub struct EventRouter {
    /// Internal subscribers (by subscriber ID)
    internal_subscribers: Arc<DashMap<String, Arc<dyn EventSubscriber>>>,
}

impl EventRouter {
    pub fn new() -> Self {
        Self {
            internal_subscribers: Arc::new(DashMap::new()),
        }
    }

    /// Route event to internal subscribers
    ///
    /// Note: frontend events are sent directly using lib.rs:emit_to_frontend(), not through this router
    pub async fn route(&self, envelope: EventEnvelope) -> EventBusResult<()> {
        // First collect subscribers list (avoid holding DashMap iterator across await points)
        let subscribers: Vec<(String, Arc<dyn EventSubscriber>)> = self
            .internal_subscribers
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        // Only log if there are subscribers (to avoid flooding)
        if !subscribers.is_empty() {
            trace!(
                "Routing event to {} subscribers: {:?}",
                subscribers.len(),
                subscribers
                    .iter()
                    .map(|(id, _)| id.as_str())
                    .collect::<Vec<_>>()
            );
        }

        // Send to all internal subscribers
        for (subscriber_id, subscriber) in subscribers {
            if let Err(e) = subscriber.on_envelope(&envelope).await {
                warn!(
                    "Internal subscriber {} failed to process event: {}",
                    subscriber_id, e
                );
            }
        }

        Ok(())
    }

    /// Route batch of events
    pub async fn route_batch(&self, envelopes: Vec<EventEnvelope>) -> EventBusResult<()> {
        // First collect subscribers list (avoid holding DashMap iterator across await points)
        let subscribers: Vec<(String, Arc<dyn EventSubscriber>)> = self
            .internal_subscribers
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        for envelope in envelopes {
            for (subscriber_id, subscriber) in &subscribers {
                if let Err(e) = subscriber.on_envelope(&envelope).await {
                    warn!(
                        "Internal subscriber {} failed to process event: {}",
                        subscriber_id, e
                    );
                }
            }
        }
        Ok(())
    }

    /// Add internal subscriber
    pub fn subscribe_internal(&self, subscriber_id: String, subscriber: Arc<dyn EventSubscriber>) {
        self.internal_subscribers
            .insert(subscriber_id.clone(), subscriber);
        debug!("Added internal subscriber: subscriber_id={}", subscriber_id);
    }

    /// Remove internal subscriber
    pub fn unsubscribe_internal(&self, subscriber_id: &str) {
        self.internal_subscribers.remove(subscriber_id);
        debug!(
            "Removed internal subscriber: subscriber_id={}",
            subscriber_id
        );
    }

    /// Get subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.internal_subscribers.len()
    }
}

impl Default for EventRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{EventRouter, EventSubscriber};
    use crate::event_bus::EventSubscriberResult;
    use bitfun_events::{
        AgenticEvent, AgenticEventEnvelope, AgenticEventOrigin, AgenticEventPriority,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct RecordingSubscriber {
        events: AtomicUsize,
        external_envelopes: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl EventSubscriber for RecordingSubscriber {
        async fn on_event(&self, _event: &AgenticEvent) -> EventSubscriberResult {
            self.events.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn on_envelope(&self, envelope: &AgenticEventEnvelope) -> EventSubscriberResult {
            if envelope.origin == AgenticEventOrigin::ExternalAcp {
                self.external_envelopes.fetch_add(1, Ordering::SeqCst);
            }
            self.on_event(&envelope.event).await
        }
    }

    struct DefaultForwardSubscriber {
        events: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl EventSubscriber for DefaultForwardSubscriber {
        async fn on_event(&self, _event: &AgenticEvent) -> EventSubscriberResult {
            self.events.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn route_delivers_external_origin_to_envelope_override() {
        let router = EventRouter::new();
        let recording = Arc::new(RecordingSubscriber {
            events: AtomicUsize::new(0),
            external_envelopes: AtomicUsize::new(0),
        });
        let forwarding = Arc::new(DefaultForwardSubscriber {
            events: AtomicUsize::new(0),
        });
        router.subscribe_internal("recording".to_string(), recording.clone());
        router.subscribe_internal("forwarding".to_string(), forwarding.clone());

        router
            .route(AgenticEventEnvelope::new_with_origin(
                AgenticEvent::SessionStateChanged {
                    session_id: "session-1".to_string(),
                    new_state: "idle".to_string(),
                },
                AgenticEventPriority::High,
                AgenticEventOrigin::ExternalAcp,
            ))
            .await
            .expect("route");

        assert_eq!(recording.events.load(Ordering::SeqCst), 1);
        assert_eq!(recording.external_envelopes.load(Ordering::SeqCst), 1);
        assert_eq!(forwarding.events.load(Ordering::SeqCst), 1);
    }
}
