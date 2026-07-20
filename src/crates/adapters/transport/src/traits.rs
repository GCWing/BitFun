use async_trait::async_trait;
use bitfun_events::AgenticEvent;
use std::fmt::Debug;

/// Event delivery implemented by a product host adapter.
#[async_trait]
pub trait TransportAdapter: Send + Sync + Debug {
    /// Emit agentic event to frontend
    async fn emit_event(&self, event: AgenticEvent) -> anyhow::Result<()>;

    /// Emit a non-agentic event owned by an existing product service.
    async fn emit_generic(
        &self,
        event_name: &str,
        payload: serde_json::Value,
    ) -> anyhow::Result<()>;
}
