use crate::{RuntimeIpcError, RuntimeIpcEvent, RuntimeIpcOperation, RuntimeIpcOperationResult};
use async_trait::async_trait;
use tokio::sync::{broadcast, watch};

#[async_trait]
pub trait RuntimeIpcRequestHandler: Send + Sync {
    /// Rejects clients once authoritative event delivery is permanently lost.
    fn ensure_available(&self) -> Result<(), RuntimeIpcError> {
        Ok(())
    }

    /// Sticky process-level availability for authenticated connections that
    /// have not attached to a Session yet.
    fn subscribe_availability(&self) -> Option<watch::Receiver<bool>> {
        None
    }

    async fn execute(
        &self,
        operation: RuntimeIpcOperation,
    ) -> Result<RuntimeIpcOperationResult, RuntimeIpcError>;

    fn subscribe_events(
        &self,
        session_id: &str,
    ) -> Result<broadcast::Receiver<RuntimeIpcEvent>, RuntimeIpcError>;
}
