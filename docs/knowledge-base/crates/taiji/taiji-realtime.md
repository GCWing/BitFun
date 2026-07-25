# taiji-realtime

**路径**: src/crates/taiji/taiji-realtime
**描述**: Real-time market data hub — CTP data source + WebSocket bridge

## 依赖
- 内部: taiji-engine
- 外部: serde, serde_json, tokio, axum, tokio-tungstenite, futures-util, crossbeam, parking_lot, tracing

## 模块结构
- `channel` — crossbeam SPSC 通道封装（TickChannel）
- `datasource` — CtpDataSource，实现 DataSource trait
- `ws_bridge` — axum WebSocket 服务器，JSON 推送 TickData

## 核心类型
- `TickChannel` — 高性能 tick 通道
- `CtpDataSource` — CTP 行情数据源
- `WsBridge` — WebSocket 推送桥

## 属于领域
- realtime / data
