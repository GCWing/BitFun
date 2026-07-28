# taiji-executor — Execution Bridge

Order management and position tracking. Provides a trait-based execution bridge, an order manager with state machine transitions, and a multi-instrument position tracker.

## Usage

```rust
use taiji_executor::order_mgr::OrderManager;
use taiji_executor::position::PositionTracker;

let mut order_mgr = OrderManager::new();
let mut positions = PositionTracker::new();

let order_id = order_mgr.submit("ag2506", "Long", 5625.0, 2)?;
order_mgr.fill(&order_id, 5625.0, 2)?;
positions.apply_fill("ag2506", "Long", 5625.0, 2)?;

println!("Position: {:?}", positions.get("ag2506"));
```

```bash
cargo add taiji-executor
```

## Modules

| Module | Description |
|--------|-------------|
| `bridge` | `ExecutionBridge` trait — abstract order routing |
| `order_mgr` | `OrderManager` with state transitions (Submitted → PartialFilled → Filled / Cancelled / Rejected) |
| `position` | `PositionTracker` — average price, PnL, multi-instrument support |
| `types` | Shared types: `OrderRequest`, `Fill`, `Position` |
