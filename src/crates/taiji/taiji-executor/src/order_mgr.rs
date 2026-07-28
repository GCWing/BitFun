use dashmap::DashMap;

use crate::types::{Direction, Fill, Offset, OrderAck, OrderRequest, OrderStatus, OrderType};

/// Tracks the lifecycle of a single order through its state machine.
#[derive(Debug, Clone)]
pub struct OrderState {
    pub order_id: String,
    pub instrument: String,
    pub direction: Direction,
    pub offset: Offset,
    pub price: f64,
    pub volume: u32,
    pub order_type: OrderType,
    pub status: OrderStatus,
    pub filled_volume: u32,
    pub filled_price: Option<f64>,
    pub error: Option<String>,
}

/// Manages the lifecycle of all outstanding orders.
///
/// The state machine transitions:
///
/// ```text
/// Submitted ──┬── PartialFilled ── Filled
///              ├── Cancelled
///              └── Rejected
/// ```
pub struct OrderManager {
    orders: DashMap<String, OrderState>,
}

impl Default for OrderManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderManager {
    pub fn new() -> Self {
        Self {
            orders: DashMap::new(),
        }
    }

    /// Register a newly submitted order. Returns a `Submitted` ack.
    pub fn submit(&self, order: OrderRequest) -> OrderAck {
        let ack = OrderAck {
            order_id: order.order_id.clone(),
            status: OrderStatus::Submitted,
            filled_volume: 0,
            filled_price: None,
            error: None,
        };
        let state = OrderState {
            order_id: order.order_id.clone(),
            instrument: order.instrument,
            direction: order.direction,
            offset: order.offset,
            price: order.price,
            volume: order.volume,
            order_type: order.order_type,
            status: OrderStatus::Submitted,
            filled_volume: 0,
            filled_price: None,
            error: None,
        };
        self.orders.insert(state.order_id.clone(), state);
        ack
    }

    /// Apply a fill event. Transitions `Submitted` → `PartialFilled` or `Filled`.
    pub fn on_fill(&self, fill: &Fill) -> Option<OrderAck> {
        let mut state = self.orders.get_mut(&fill.order_id)?;
        state.filled_volume += fill.volume;
        state.filled_price = Some(fill.price);

        state.status = if state.filled_volume >= state.volume {
            OrderStatus::Filled
        } else {
            OrderStatus::PartialFilled
        };

        Some(OrderAck {
            order_id: state.order_id.clone(),
            status: state.status,
            filled_volume: state.filled_volume,
            filled_price: state.filled_price,
            error: None,
        })
    }

    /// Mark an order as rejected with a reason.
    pub fn on_reject(&self, order_id: &str, reason: String) -> Option<OrderAck> {
        let mut state = self.orders.get_mut(order_id)?;
        state.status = OrderStatus::Rejected;
        state.error = Some(reason.clone());

        Some(OrderAck {
            order_id: state.order_id.clone(),
            status: OrderStatus::Rejected,
            filled_volume: state.filled_volume,
            filled_price: state.filled_price,
            error: Some(reason),
        })
    }

    /// Mark an order as cancelled.
    pub fn on_cancel(&self, order_id: &str) -> Option<OrderAck> {
        let mut state = self.orders.get_mut(order_id)?;
        state.status = OrderStatus::Cancelled;

        Some(OrderAck {
            order_id: state.order_id.clone(),
            status: OrderStatus::Cancelled,
            filled_volume: state.filled_volume,
            filled_price: state.filled_price,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_order(order_id: &str, volume: u32) -> OrderRequest {
        OrderRequest {
            order_id: order_id.into(),
            instrument: "ag2506".into(),
            direction: Direction::Buy,
            offset: Offset::Open,
            price: 5625.0,
            volume,
            order_type: OrderType::Limit,
        }
    }

    fn make_fill(order_id: &str, volume: u32, price: f64) -> Fill {
        Fill {
            order_id: order_id.into(),
            price,
            volume,
            time: "2026-07-24T09:30:01".into(),
        }
    }

    #[test]
    fn submit_returns_submitted() {
        let mgr = OrderManager::new();
        let ack = mgr.submit(make_order("ord-001", 3));
        assert_eq!(ack.order_id, "ord-001");
        assert_eq!(ack.status, OrderStatus::Submitted);
        assert_eq!(ack.filled_volume, 0);
    }

    #[test]
    fn partial_fill_then_filled() {
        let mgr = OrderManager::new();
        mgr.submit(make_order("ord-001", 3));

        // Partial fill: volume 1 of 3.
        let ack = mgr.on_fill(&make_fill("ord-001", 1, 5625.0)).unwrap();
        assert_eq!(ack.status, OrderStatus::PartialFilled);
        assert_eq!(ack.filled_volume, 1);

        // Second partial fill: volume 2 of 3 → now filled.
        let ack = mgr.on_fill(&make_fill("ord-001", 2, 5626.0)).unwrap();
        assert_eq!(ack.status, OrderStatus::Filled);
        assert_eq!(ack.filled_volume, 3);
        assert_eq!(ack.filled_price, Some(5626.0));
    }

    #[test]
    fn full_fill_in_one_shot() {
        let mgr = OrderManager::new();
        mgr.submit(make_order("ord-002", 2));

        let ack = mgr.on_fill(&make_fill("ord-002", 2, 5700.0)).unwrap();
        assert_eq!(ack.status, OrderStatus::Filled);
        assert_eq!(ack.filled_volume, 2);
    }

    #[test]
    fn reject_transition() {
        let mgr = OrderManager::new();
        mgr.submit(make_order("ord-003", 1));

        let ack = mgr
            .on_reject("ord-003", "margin insufficient".into())
            .unwrap();
        assert_eq!(ack.status, OrderStatus::Rejected);
        assert_eq!(ack.error.unwrap(), "margin insufficient");
    }

    #[test]
    fn cancel_transition() {
        let mgr = OrderManager::new();
        mgr.submit(make_order("ord-004", 1));

        let ack = mgr.on_cancel("ord-004").unwrap();
        assert_eq!(ack.status, OrderStatus::Cancelled);
    }

    #[test]
    fn fill_unknown_order_returns_none() {
        let mgr = OrderManager::new();
        assert!(mgr
            .on_fill(&make_fill("no-such-order", 1, 5000.0))
            .is_none());
    }
}
