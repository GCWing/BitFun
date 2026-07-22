use serde::{Deserialize, Serialize};

/// An order request sent to the execution bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub order_id: String,
    pub instrument: String,
    pub direction: Direction,
    pub offset: Offset,
    pub price: f64,
    pub volume: u32,
    pub order_type: OrderType,
}

/// Buy or sell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Buy,
    Sell,
}

/// Open or close offset flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Offset {
    Open,
    Close,
    CloseToday,
}

/// Order price type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Limit,
    Market,
}

/// Order lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Submitted,
    PartialFilled,
    Filled,
    Cancelled,
    Rejected,
}

/// Acknowledgement from the execution layer after order submission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderAck {
    pub order_id: String,
    pub status: OrderStatus,
    pub filled_volume: u32,
    pub filled_price: Option<f64>,
    pub error: Option<String>,
}

/// Current position snapshot for an instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub instrument: String,
    pub direction: Direction,
    pub volume: u32,
    pub avg_price: f64,
    pub float_pnl: f64,
}

/// Account-level capital snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub available: f64,
    pub frozen_margin: f64,
    pub total_equity: f64,
}

/// A confirmed fill (trade) from the exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub order_id: String,
    pub price: f64,
    pub volume: u32,
    pub time: String,
}
