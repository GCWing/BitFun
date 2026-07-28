//! taiji-realtime — Real-time market data hub.
//!
//! ## 模块
//! - `channel`  — crossbeam SPSC 通道封装（TickChannel）
//! - `datasource` — CtpDataSource，实现 DataSource trait
//! - `ws_bridge` — axum WebSocket 服务器，JSON 推送 TickData

pub mod channel;
pub mod datasource;
pub mod ws_bridge;

pub use channel::TickChannel;
pub use datasource::CtpDataSource;
pub use ws_bridge::WsBridge;
