use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::types::{AccountInfo, Fill, OrderAck, OrderRequest, Position};

/// Execution bridge trait — the abstraction over a live trading gateway.
///
/// Implementations may wrap CTP, a paper-trading simulator, or a mock for testing.
#[async_trait]
pub trait ExecutionBridge: Send + Sync {
    /// Submit a new order to the market.
    async fn place_order(&self, order: OrderRequest) -> Result<OrderAck, String>;

    /// Cancel an existing order by its ID.
    async fn cancel_order(&self, order_id: &str) -> Result<OrderAck, String>;

    /// Query current positions for a given instrument.
    async fn query_position(&self, instrument: &str) -> Result<Vec<Position>, String>;

    /// Query current account capital information.
    async fn query_account(&self) -> Result<AccountInfo, String>;

    /// Subscribe to a stream of fill (trade) events.
    async fn subscribe_fills(&self) -> Result<mpsc::Receiver<Fill>, String>;
}
