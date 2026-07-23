//! WsBridge — 轻量级行情 WebSocket 服务器，将 TickData 推送到所有连接的客户端。
//!
//! **与 BitFun relay 的区别：** 这是 taiji 专用的、仅推送行情数据的轻量 WS 服务器，
//! 不包含 BitFun 的会话管理、远程连接、多协议中继等能力。BitFun relay 是通用多协议
//! 中继层，WsBridge 只是一个自包含的行情广播端点。
//!
//! ## 双通道架构（dual-channel bridge）
//!
//! ```
//!   crossbeam Receiver → (dedicated OS thread) → tokio broadcast → axum WS handlers
//! ```
//!
//! 引擎热路径使用 crossbeam 无锁 channel（同步、低延迟、适合实时数据泵），
//! 而 axum WebSocket 处理器运行在 tokio 异步运行时上。两者分属不同的并发模型，
//! 因此需要一个专用 OS 线程作为"桥接适配器"：
//!
//! 1. **crossbeam → thread**：桥接线程通过 `receiver.recv()` 阻塞读取 crossbeam
//!    channel，拿到 `TickData` 后立即序列化为 JSON 字符串。
//! 2. **thread → tokio broadcast**：序列化后的 JSON 通过 `broadcast::Sender::send()`
//!    推入 tokio broadcast channel（容量 256），所有已连接的 WS 客户端各自通过
//!    `subscribe()` 获取独立的 `Receiver` 流。
//! 3. **broadcast → axum WS**：每个 WS 连接 spawn 一个 tokio task，循环从 broadcast
//!    receiver 读取消息并通过 WebSocket 帧发送给客户端。
//!
//! ## 背压（backpressure）处理
//!
//! - **broadcast channel 容量为 256**。当某个 WS 客户端消费速度低于数据产生速度时，
//!   tokio broadcast 会为该客户端的滞后 receiver 返回 `Lagged` 错误，该连接断开。
//!   这是 tokio broadcast 的内置"慢消费者淘汰"机制——不会阻塞生产者，也不会为慢消费者
//!   无限缓冲。
//! - **序列化失败跳过**：若单条 `TickData` 序列化失败（理论上不应发生），桥接线程记录
//!   error 日志并 `continue`，不会中断整个数据流。
//! - **无订阅者时静默丢弃**：`broadcast::Sender::send()` 在无活跃 subscriber 时返回
//!   `Err(0)`，桥接线程忽略此错误（`let _ = tx_clone.send(json)`），不阻塞、不缓冲。
//! - **引擎侧不受 WS 客户端影响**：整个桥接链路中，crossbeam receiver 的消费速度等于
//!   JSON 序列化 + broadcast send 的速度（微秒级），WS 客户端的快慢不会反向传导到引擎
//!   热路径。这是双通道设计的关键保证。
//!
//! ## 设计权衡（tradeoffs）
//!
//! | 维度 | 当前方案 | 代价 |
//! |------|---------|------|
//! | 线程模型 | 额外 spawn 一个 OS 线程做桥接 | 一个常驻线程的开销 |
//! | 序列化位置 | 在桥接线程中序列化（不在引擎热路径） | JSON 序列化耗时在桥接线程，不影响引擎 tick 处理 |
//! | 广播语义 | tokio broadcast 多播（每客户端独立副本） | 慢消费者被强制断开，无法回放历史数据 |
//! | 协议 | 仅 WebSocket JSON 文本帧 | 不支持 binary、SSE、gRPC-stream 等其他推送协议 |
//! | 会话管理 | 无认证、无会话、无重连 | 仅适合本地/localhost 场景，不适合公网暴露 |
//!
//! ## 未来迁移路径
//!
//! **TODO(taiji-realtime): 迁移到 BitFun relay。** 当前 WsBridge 是自包含的 WS 广播器，
//! 长期计划是将行情推送接入 BitFun relay 的统一中继层，从而获得：
//!
//! - 内置的会话管理与认证
//! - 多协议支持（WebSocket + SSE + gRPC-stream）
//! - 远程连接与重连机制
//! - 统一的可观测性（metrics / tracing）
//!
//! 迁移后，桥接线程可简化为"crossbeam → relay ingress channel"的单一适配器，
//! relay 负责所有连接管理、协议适配和背压策略。WsBridge 模块届时可退役。
//!
//! 每个连接的 WebSocket 客户端收到 JSON 格式的 TickData。

use std::net::SocketAddr;
use std::thread;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use crossbeam::channel::Receiver;
use futures_util::{SinkExt, StreamExt};
use taiji_engine::types::tick::TickData;
use tokio::sync::broadcast;

/// WebSocket 桥接——从 crossbeam channel 读取 TickData，广播到所有 WS 客户端。
///
/// 默认监听 `127.0.0.1`，可通过 [`with_bind_address`](Self::with_bind_address) 配置。
/// 可通过 [`with_shutdown`](Self::with_shutdown) 注入优雅关闭信号。
pub struct WsBridge {
    port: u16,
    bind_address: String,
    receiver: Option<Receiver<TickData>>,
    shutdown_rx: Option<tokio::sync::oneshot::Receiver<()>>,
}

impl WsBridge {
    /// 创建 WsBridge。
    ///
    /// `receiver` 来自 `TickChannel::take_receiver()`。
    pub async fn new(port: u16, receiver: Receiver<TickData>) -> Self {
        Self {
            port,
            bind_address: "127.0.0.1".to_string(),
            receiver: Some(receiver),
            shutdown_rx: None,
        }
    }

    /// 设置监听地址（默认 `127.0.0.1`）。
    ///
    /// 支持 IPv4（如 `"0.0.0.0"`）和 IPv6（如 `"::1"`）地址。
    pub fn with_bind_address(mut self, addr: impl Into<String>) -> Self {
        self.bind_address = addr.into();
        self
    }

    /// 设置优雅关闭信号。
    ///
    /// 传入的 `oneshot::Receiver` 被 resolve 时，服务器将停止接受新连接
    /// 并等待已有连接完成。
    pub fn with_shutdown(mut self, rx: tokio::sync::oneshot::Receiver<()>) -> Self {
        self.shutdown_rx = Some(rx);
        self
    }

    /// 启动 axum WebSocket 服务器。
    ///
    /// 内部 spawn 一个线程将 crossbeam 消息桥接到 tokio broadcast，
    /// 然后启动 axum server 监听指定端口。
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let receiver = self.receiver.take().ok_or("WsBridge already started")?;

        // tokio broadcast: 容量 256，用于向所有 WS 客户端转发
        let (broadcast_tx, _) = broadcast::channel::<String>(256);

        // 专用线程：crossbeam → tokio broadcast
        //
        // TODO(taiji-realtime): 迁移到 BitFun relay 后，此线程可简化为
        // "crossbeam → relay ingress" 单一适配器，移除 tokio broadcast 层。
        let tx_clone = broadcast_tx.clone();
        thread::spawn(move || {
            while let Ok(tick) = receiver.recv() {
                let json = match serde_json::to_string(&tick) {
                    Ok(j) => j,
                    Err(e) => {
                        tracing::error!("Failed to serialize tick: {e}");
                        continue;
                    }
                };
                // 忽略无订阅者的错误
                let _ = tx_clone.send(json);
            }
        });

        let app = Router::new()
            .route("/ws", get(ws_handler))
            .with_state(broadcast_tx);

        let ip: std::net::IpAddr = self
            .bind_address
            .parse()
            .map_err(|e| format!("invalid bind address '{}': {e}", self.bind_address))?;
        let addr = SocketAddr::new(ip, self.port);
        tracing::info!("WsBridge listening on ws://{}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;

        match self.shutdown_rx.take() {
            Some(shutdown_rx) => {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async move {
                        let _ = shutdown_rx.await;
                    })
                    .await?;
            }
            None => {
                axum::serve(listener, app).await?;
            }
        }

        Ok(())
    }
}

/// WebSocket 升级处理——每个客户端订阅 broadcast。
async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(tx): axum::extract::State<broadcast::Sender<String>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, tx))
}

/// 处理单个 WebSocket 连接——从 broadcast 读取并写入 WS。
async fn handle_socket(socket: WebSocket, tx: broadcast::Sender<String>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let mut rx = tx.subscribe();

    // 忽略客户端发来的消息（只推送）
    let mut recv_task = tokio::spawn(async move { while ws_receiver.next().await.is_some() {} });

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if ws_sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    tokio::select! {
        _ = &mut recv_task => {}
        _ = send_task => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl WsBridge {
        /// 测试用：创建 receiver 已被消费的 WsBridge。
        #[cfg(test)]
        fn new_without_receiver(port: u16) -> Self {
            Self {
                port,
                bind_address: "127.0.0.1".to_string(),
                receiver: None,
                shutdown_rx: None,
            }
        }
    }

    /// 验证 WsBridge 可构造，receiver 存在。
    #[tokio::test]
    async fn ws_bridge_construct() {
        let (_tx, rx) = crossbeam::channel::bounded::<TickData>(4);
        let bridge = WsBridge::new(9876, rx).await;
        assert_eq!(bridge.port, 9876);
        assert!(bridge.receiver.is_some());
    }

    /// 验证 receiver 已被消费时 start 返回错误。
    #[tokio::test]
    async fn ws_bridge_double_start_error() {
        let mut bridge = WsBridge::new_without_receiver(12347);
        let result = bridge.start().await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "WsBridge already started");
    }

    /// 验证 start 可后台启动（不被 abort panic）。
    #[tokio::test]
    async fn ws_bridge_start_and_abort() {
        let (_tx, rx) = crossbeam::channel::bounded::<TickData>(4);
        let mut bridge = WsBridge::new(12348, rx).await;

        let handle = tokio::spawn(async move { bridge.start().await });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        handle.abort();
        // 不 panic 即通过
    }
}
