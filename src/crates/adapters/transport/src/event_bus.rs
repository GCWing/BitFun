/// Unified event bus - Manages event distribution for all platforms
use crate::traits::TransportAdapter;
use bitfun_events::AgenticEvent;
use dashmap::DashMap;
use log::{error, warn};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Event bus - Core event dispatcher
#[derive(Clone)]
pub struct EventBus {
    /// Active transport adapters (indexed by session_id)
    adapters: Arc<DashMap<String, Arc<dyn TransportAdapter>>>,

    /// Event queue (async buffer)
    event_tx: mpsc::UnboundedSender<EventEnvelope>,

    /// Whether logging is enabled
    #[allow(dead_code)]
    enable_logging: bool,
}

/// Event envelope
#[derive(Debug)]
struct EventEnvelope {
    session_id: String,
    event: AgenticEvent,
    #[allow(dead_code)]
    priority: EventPriority,
}

/// Event priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

impl EventBus {
    /// Create a new event bus
    pub fn new(enable_logging: bool) -> Self {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<EventEnvelope>();
        let adapters: Arc<DashMap<String, Arc<dyn TransportAdapter>>> = Arc::new(DashMap::new());

        let adapters_clone = adapters.clone();
        tokio::spawn(async move {
            while let Some(envelope) = event_rx.recv().await {
                if let Some(adapter) = adapters_clone.get(&envelope.session_id) {
                    if let Err(e) = adapter
                        .emit_event(&envelope.session_id, envelope.event)
                        .await
                    {
                        error!(
                            "Failed to emit event for session {}: {}",
                            envelope.session_id, e
                        );
                    }
                } else {
                    warn!("No adapter registered for session: {}", envelope.session_id);
                }
            }
        });

        Self {
            adapters,
            event_tx,
            enable_logging,
        }
    }

    /// Register transport adapter
    pub fn register_adapter(&self, session_id: String, adapter: Arc<dyn TransportAdapter>) {
        self.adapters.insert(session_id, adapter);
    }

    /// Unregister adapter
    pub fn unregister_adapter(&self, session_id: &str) {
        self.adapters.remove(session_id);
    }

    /// Emit event
    pub async fn emit(
        &self,
        session_id: String,
        event: AgenticEvent,
        priority: EventPriority,
    ) -> anyhow::Result<()> {
        let envelope = EventEnvelope {
            session_id,
            event,
            priority,
        };

        self.event_tx
            .send(envelope)
            .map_err(|e| anyhow::anyhow!("Failed to send event to queue: {}", e))?;

        Ok(())
    }

    /// Get active session count
    pub fn active_sessions(&self) -> usize {
        self.adapters.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_creation() {
        let bus = EventBus::new(true);
        assert_eq!(bus.active_sessions(), 0);
    }
}
