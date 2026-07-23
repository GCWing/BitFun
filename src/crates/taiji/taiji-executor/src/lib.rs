//! Taiji executor — execution bridge abstraction, order management, and position tracking.

pub mod bridge;
pub mod order_mgr;
pub mod position;
pub mod types;

pub use bridge::ExecutionBridge;
pub use order_mgr::{OrderManager, OrderState};
pub use position::PositionTracker;
pub use types::*;
