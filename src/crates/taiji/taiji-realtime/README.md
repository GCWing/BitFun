# taiji-realtime — Real-time Market Data Hub

Wraps a CTP data source into a crossbeam SPSC channel and exposes tick data via an axum WebSocket server with JSON-serialized `TickData` push.

## Usage

```rust
use taiji_realtime::channel::TickChannel;
use taiji_realtime::ws_bridge::WsBridge;

let (tx, rx) = TickChannel::new();
let bridge = WsBridge::new(rx);
bridge.start("127.0.0.1:9001").await?;

// In another thread: push ticks from CTP
tx.send(tick_data)?;
```

```bash
cargo add taiji-realtime
```

## Modules

| Module | Description |
|--------|-------------|
| `channel` | `TickChannel` — crossbeam SPSC channel for tick distribution |
| `datasource` | `CtpDataSource` — CTP market data adapter |
| `ws_bridge` | `WsBridge` — axum WebSocket server, JSON-serialized `TickData` push |
