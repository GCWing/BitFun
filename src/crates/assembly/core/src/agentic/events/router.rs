//! Event Router
//!
//! Responsible for distributing events to internal subscribers (frontend events are sent directly using Tauri emit)

use super::types::{AgenticEvent, EventEnvelope};
use crate::util::errors::BitFunResult;
use dashmap::DashMap;
use log::{debug, trace, warn};
use std::sync::Arc;

/// Event subscriber trait
///
/// Used for internal system subscribers (e.g. logging system, monitoring system, etc.)
#[async_trait::async_trait]
pub trait EventSubscriber: Send + Sync + 'static {
    async fn on_event(&self, event: &AgenticEvent) -> BitFunResult<()>;
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
    pub async fn route(&self, envelope: EventEnvelope) -> BitFunResult<()> {
        let event = &envelope.event;

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
            if let Err(e) = subscriber.on_event(event).await {
                warn!(
                    "Internal subscriber {} failed to process event: {}",
                    subscriber_id, e
                );
            }
        }

        Ok(())
    }

    /// Route batch of events
    pub async fn route_batch(&self, envelopes: Vec<EventEnvelope>) -> BitFunResult<()> {
        // First collect subscribers list (avoid holding DashMap iterator across await points)
        let subscribers: Vec<(String, Arc<dyn EventSubscriber>)> = self
            .internal_subscribers
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        for envelope in envelopes {
            let event = &envelope.event;
            for (subscriber_id, subscriber) in &subscribers {
                if let Err(e) = subscriber.on_event(event).await {
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
