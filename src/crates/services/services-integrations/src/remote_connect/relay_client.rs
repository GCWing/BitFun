//! WebSocket client for connecting to the Relay Server.
//!
//! Account devices authenticate over WebSocket and receive presence and opaque
//! device messages. Payload submission uses bounded HTTP; reconnect repeats
//! account authentication before sending further control messages.

use anyhow::{anyhow, Result};
use futures::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
#[cfg(windows)]
use tokio_tungstenite::{tungstenite::client::IntoClientRequest, Connector};

/// Install the rustls ring CryptoProvider as the process-level default.
///
/// Call this once at application startup so that all subsequent TLS operations
/// (relay_client, reqwest, tokio-tungstenite) reuse the same provider.
/// `install_default()` returns `Err` only when a provider is already installed,
/// which is harmless — we silently ignore it.
///
/// This is safe to call multiple times and from any thread. Installing it
/// explicitly keeps provider choice deterministic for every product client.
pub fn ensure_rustls_crypto_provider() {
    openbitfun_services_core::tls_provider::ensure_ring_crypto_provider();
}

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

const RELAY_DIAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
/// Heartbeats are sent every 30 seconds. Two missed acknowledgements plus
/// scheduling/network slack indicates a half-open socket that should be
/// replaced even when the OS has not surfaced a read error yet.
pub const RELAY_INBOUND_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(75);

/// Messages in the relay protocol (both directions).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RelayMessage {
    // ── Outbound (desktop → relay) ──────────────────────────────────
    Heartbeat,
    /// Authenticate the socket and register this account device.
    AuthConnect {
        token: String,
        device_name: String,
        device_kind: String,
    },
    /// Route an encrypted payload to another device in the same account.
    DeviceMessage {
        target_device_id: String,
        correlation_id: String,
        encrypted_data: String,
        nonce: String,
    },

    // ── Inbound (relay → desktop) ───────────────────────────────────
    HeartbeatAck,
    Error {
        message: String,
    },
    /// Account connect succeeded — relay validated the token.
    AuthOk {
        user_id: String,
        device_id: String,
    },
    AuthError {
        message: String,
    },
    /// A device-to-device message routed from another device in the account.
    IncomingDeviceMessage {
        source_device_id: String,
        correlation_id: String,
        encrypted_data: String,
        nonce: String,
    },
    /// Current online devices in the account (presence broadcast).
    DevicePresence {
        devices: Vec<DevicePresenceEntry>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicePresenceEntry {
    pub device_id: String,
    pub device_name: String,
}

/// Events emitted by the relay client to the upper layers.
#[derive(Debug, Clone)]
pub enum RelayEvent {
    Connected,
    Reconnected,
    Disconnected,
    Error {
        message: String,
    },
    /// Account auth-connect succeeded.
    AuthOk {
        user_id: String,
        device_id: String,
    },
    AuthError {
        message: String,
    },
    /// Encrypted device-to-device message from another device in the account.
    DeviceMessageReceived {
        source_device_id: String,
        correlation_id: String,
        encrypted_data: String,
        nonce: String,
    },
    /// Online device list for the account.
    DevicePresence {
        devices: Vec<DevicePresenceEntry>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

#[derive(Debug, Clone, Default)]
struct ReconnectCtx {
    ws_url: String,
    /// Account token for device-routing re-auth after reconnect.
    token: String,
    /// Device name for re-auth after reconnect.
    device_name: String,
}

// One owner controls the socket, heartbeat, write deadline and reconnect loop.
// A generation fences late completion when connect replaces an earlier run.
struct ConnectionLifecycle {
    generation: u64,
    state: ConnectionState,
    task: Option<tokio::task::JoinHandle<()>>,
    cmd_tx: Option<mpsc::Sender<RelayMessage>>,
    reconnect_ctx: Option<ReconnectCtx>,
}

type ConnectionOwner = Arc<Mutex<ConnectionLifecycle>>;

// This is transport backpressure, not a limit on Agent work. A full queue
// rejects enqueue explicitly; unacknowledged commands are never replayed.
const RELAY_COMMAND_QUEUE_CAPACITY: usize = 64;
const RELAY_WRITE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

pub struct RelayClient {
    lifecycle: ConnectionOwner,
    event_tx: mpsc::UnboundedSender<RelayEvent>,
}

impl RelayClient {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<RelayEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let client = Self {
            lifecycle: Arc::new(Mutex::new(ConnectionLifecycle {
                generation: 0,
                state: ConnectionState::Disconnected,
                task: None,
                cmd_tx: None,
                reconnect_ctx: None,
            })),
            event_tx,
        };
        (client, event_rx)
    }

    pub async fn connection_state(&self) -> ConnectionState {
        self.lifecycle.lock().unwrap().state.clone()
    }

    pub async fn connect(&self, ws_url: &str) -> Result<()> {
        let (ready_tx, ready_rx) = oneshot::channel();
        let generation = {
            let mut owner = self.lifecycle.lock().unwrap();
            if let Some(task) = owner.task.take() {
                task.abort();
            }
            owner.generation += 1;
            owner.state = ConnectionState::Connecting;
            owner.cmd_tx = None;
            owner.reconnect_ctx = Some(ReconnectCtx {
                ws_url: ws_url.to_string(),
                ..Default::default()
            });
            let generation = owner.generation;
            owner.task = Some(tokio::spawn(Self::run_connection(
                self.lifecycle.clone(),
                self.event_tx.clone(),
                generation,
                ws_url.to_string(),
                ready_tx,
            )));
            generation
        };
        ready_rx
            .await
            .map_err(|_| anyhow!("Relay connection attempt cancelled"))??;
        if self.lifecycle.lock().unwrap().generation != generation {
            return Err(anyhow!("Relay connection attempt superseded"));
        }
        Ok(())
    }

    async fn run_connection(
        lifecycle: ConnectionOwner,
        event_tx: mpsc::UnboundedSender<RelayEvent>,
        generation: u64,
        ws_url: String,
        ready: oneshot::Sender<Result<()>>,
    ) {
        let mut socket = match dial(&ws_url).await {
            Ok(socket) => socket,
            Err(error) => {
                let mut owner = lifecycle.lock().unwrap();
                if owner.generation == generation {
                    owner.state = ConnectionState::Disconnected;
                    owner.cmd_tx = None;
                    owner.reconnect_ctx = None;
                    let _ = event_tx.send(RelayEvent::Disconnected);
                }
                let _ = ready.send(Err(error));
                return;
            }
        };
        let mut ready = Some(ready);
        loop {
            let (cmd_tx, cmd_rx) = mpsc::channel(RELAY_COMMAND_QUEUE_CAPACITY);
            {
                let mut owner = lifecycle.lock().unwrap();
                if owner.generation != generation {
                    return;
                }
                owner.state = ConnectionState::Connected;
                owner.cmd_tx = Some(cmd_tx);
                let event = if let Some(ready) = ready.take() {
                    let _ = ready.send(Ok(()));
                    RelayEvent::Connected
                } else {
                    RelayEvent::Reconnected
                };
                let _ = event_tx.send(event);
            }
            info!("Relay transport connected");
            Self::run_socket(socket, cmd_rx, &lifecycle, &event_tx, generation).await;
            {
                let mut owner = lifecycle.lock().unwrap();
                if owner.generation != generation {
                    return;
                }
                owner.state = ConnectionState::Reconnecting;
                // Drop all commands from the failed socket. Delivery may have
                // happened without a response; the protocol caller owns recovery.
                owner.cmd_tx = None;
            }
            let mut backoff = 2;
            socket = loop {
                tokio::time::sleep(std::time::Duration::from_secs(backoff)).await;
                let ctx = {
                    let owner = lifecycle.lock().unwrap();
                    if owner.generation != generation {
                        return;
                    }
                    let Some(ctx) = owner.reconnect_ctx.clone() else {
                        return;
                    };
                    ctx
                };
                match Self::reconnect(&ctx).await {
                    Ok(socket) => break socket,
                    Err(error) => {
                        warn!("Relay reconnect failed: {error}");
                        backoff = std::cmp::min(backoff * 2, 30);
                    }
                }
            };
        }
    }

    async fn reconnect(ctx: &ReconnectCtx) -> Result<WsStream> {
        let mut socket = dial(&ctx.ws_url).await?;
        if !ctx.token.is_empty() {
            write_relay_message(
                &mut socket,
                &RelayMessage::AuthConnect {
                    token: ctx.token.clone(),
                    device_name: ctx.device_name.clone(),
                    device_kind: "desktop".to_string(),
                },
            )
            .await?;
        }
        Ok(socket)
    }

    async fn run_socket(
        socket: WsStream,
        mut commands: mpsc::Receiver<RelayMessage>,
        lifecycle: &ConnectionOwner,
        event_tx: &mpsc::UnboundedSender<RelayEvent>,
        generation: u64,
    ) {
        let (mut writer, mut reader) = socket.split();
        // Keep full-duplex progress under backpressure, but keep both futures
        // inside this owner. Either failure drops both halves and the old queue.
        let read = async {
            loop {
                match await_relay_inbound(reader.next()).await {
                    Ok(Some(Ok(Message::Text(text)))) => match serde_json::from_str(&text) {
                        Ok(msg) => Self::dispatch(msg, event_tx, lifecycle, generation).await,
                        Err(error) => warn!("Unparseable relay message: {error}"),
                    },
                    Ok(Some(Ok(Message::Close(_)))) | Ok(None) => break,
                    Ok(Some(Err(error))) => {
                        warn!("Relay WebSocket read failed: {error}");
                        break;
                    }
                    Err(()) => {
                        warn!("Relay inbound traffic timed out");
                        break;
                    }
                    _ => {}
                }
            }
        };
        let write = async {
            let period = std::time::Duration::from_secs(30);
            let mut heartbeat =
                tokio::time::interval_at(tokio::time::Instant::now() + period, period);
            heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            while let Some(command) = next_relay_outbound(&mut commands, &mut heartbeat).await {
                if let Err(error) = write_relay_message(&mut writer, &command).await {
                    warn!("Relay WebSocket write failed: {error}");
                    break;
                }
            }
        };
        let _ = futures::future::select(std::pin::pin!(read), std::pin::pin!(write)).await;
    }

    async fn dispatch(
        msg: RelayMessage,
        event_tx: &mpsc::UnboundedSender<RelayEvent>,
        lifecycle: &ConnectionOwner,
        generation: u64,
    ) {
        let mut owner = lifecycle.lock().unwrap();
        if owner.generation != generation {
            return;
        }
        match msg {
            RelayMessage::HeartbeatAck => {
                debug!("Heartbeat acknowledged");
            }
            RelayMessage::Error { message } => {
                error!("Relay error: {message}");
                let _ = event_tx.send(RelayEvent::Error { message });
            }
            RelayMessage::AuthOk { user_id, device_id } => {
                info!("Account auth-connect ok: user_id={user_id}");
                let _ = event_tx.send(RelayEvent::AuthOk { user_id, device_id });
            }
            RelayMessage::AuthError { message } => {
                warn!("Account auth-connect failed: {message}");
                let _ = event_tx.send(RelayEvent::AuthError { message });
            }
            RelayMessage::IncomingDeviceMessage {
                source_device_id,
                correlation_id,
                encrypted_data,
                nonce,
            } => {
                debug!("DeviceMessage from {source_device_id} corr={correlation_id}");
                let _ = event_tx.send(RelayEvent::DeviceMessageReceived {
                    source_device_id,
                    correlation_id,
                    encrypted_data,
                    nonce,
                });
            }
            RelayMessage::DevicePresence { devices } => {
                debug!("DevicePresence: {} online", devices.len());
                let _ = event_tx.send(RelayEvent::DevicePresence { devices });
            }
            _ => {}
        }
    }

    pub async fn send(&self, msg: RelayMessage) -> Result<()> {
        let owner = self.lifecycle.lock().unwrap();
        Self::enqueue(&owner, msg)
    }

    fn enqueue(owner: &ConnectionLifecycle, msg: RelayMessage) -> Result<()> {
        if owner.state != ConnectionState::Connected {
            return Err(anyhow!("Relay transport is not connected"));
        }
        let tx = owner
            .cmd_tx
            .as_ref()
            .ok_or_else(|| anyhow!("Relay transport is not connected"))?;
        tx.try_send(msg).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => {
                anyhow!("Relay send queue is full; request was not queued")
            }
            mpsc::error::TrySendError::Closed(_) => anyhow!("Relay connection is closed"),
        })
    }

    pub async fn connect_authenticated(&self, token: &str, device_name: &str) -> Result<()> {
        let mut owner = self.lifecycle.lock().unwrap();
        // Only desktops hold a relay WebSocket — phones and watches talk HTTP —
        // so the kind is a constant here rather than a parameter.
        Self::enqueue(
            &owner,
            RelayMessage::AuthConnect {
                token: token.to_string(),
                device_name: device_name.to_string(),
                device_kind: "desktop".to_string(),
            },
        )?;
        if let Some(ctx) = owner.reconnect_ctx.as_mut() {
            ctx.token = token.to_string();
            ctx.device_name = device_name.to_string();
        }
        Ok(())
    }

    /// Submit device payloads over memory-admitted HTTP. The WebSocket remains
    /// the receiving/control channel and does not accept attachment-sized input.
    pub async fn send_device_message(
        &self,
        target_device_id: &str,
        correlation_id: &str,
        encrypted_data: &str,
        nonce: &str,
    ) -> Result<()> {
        let context = self
            .lifecycle
            .lock()
            .unwrap()
            .reconnect_ctx
            .clone()
            .filter(|context| !context.token.is_empty())
            .ok_or_else(|| anyhow!("Authenticated relay connection is unavailable"))?;
        let endpoint = device_message_endpoint(&context.ws_url, target_device_id)?;
        let response = super::relay_http::relay_http_client()
            .post(endpoint)
            .bearer_auth(&context.token)
            .timeout(RELAY_WRITE_TIMEOUT)
            .json(&RelayMessage::DeviceMessage {
                target_device_id: target_device_id.to_string(),
                correlation_id: correlation_id.to_string(),
                encrypted_data: encrypted_data.to_string(),
                nonce: nonce.to_string(),
            })
            .send()
            .await?;
        if response.status() != reqwest::StatusCode::NO_CONTENT {
            return Err(anyhow!(
                "Relay device message rejected (HTTP {})",
                response.status()
            ));
        }
        Ok(())
    }

    pub async fn disconnect(&self) {
        let task = {
            let mut owner = self.lifecycle.lock().unwrap();
            owner.generation += 1;
            owner.state = ConnectionState::Disconnected;
            owner.cmd_tx = None;
            owner.reconnect_ctx = None;
            let task = owner.task.take();
            if let Some(task) = &task {
                task.abort();
            }
            let _ = self.event_tx.send(RelayEvent::Disconnected);
            task
        };
        // The supervisor owns every socket/timer/future; joining cancellation
        // releases them before returning, including an in-progress handshake.
        if let Some(task) = task {
            let _ = task.await;
        }
        info!("Relay client disconnected");
    }
}

fn device_message_endpoint(ws_url: &str, target_device_id: &str) -> Result<reqwest::Url> {
    if target_device_id.is_empty()
        || target_device_id.len() > 128
        || !target_device_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        || matches!(target_device_id, "." | "..")
    {
        return Err(anyhow!("Invalid relay target device id"));
    }
    let mut url = reqwest::Url::parse(ws_url)?;
    let scheme = match url.scheme() {
        "wss" => "https",
        "ws" => "http",
        _ => return Err(anyhow!("Invalid relay WebSocket scheme")),
    };
    url.set_scheme(scheme)
        .map_err(|_| anyhow!("Invalid relay HTTP scheme"))?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(anyhow!("Invalid relay WebSocket endpoint"));
    }
    let base = url
        .path()
        .strip_suffix("/ws")
        .ok_or_else(|| anyhow!("Invalid relay WebSocket path"))?;
    let path = format!("{base}/api/devices/{target_device_id}/messages");
    url.set_path(&path);
    Ok(url)
}

impl Drop for RelayClient {
    fn drop(&mut self) {
        let mut owner = self.lifecycle.lock().unwrap();
        owner.generation += 1;
        owner.state = ConnectionState::Disconnected;
        owner.cmd_tx = None;
        owner.reconnect_ctx = None;
        if let Some(task) = owner.task.take() {
            task.abort();
        }
    }
}

async fn next_relay_outbound(
    commands: &mut mpsc::Receiver<RelayMessage>,
    heartbeat: &mut tokio::time::Interval,
) -> Option<RelayMessage> {
    let received = std::pin::pin!(commands.recv());
    let tick = std::pin::pin!(heartbeat.tick());
    // select polls its first future first. A continuously ready command queue
    // must not starve the keepalive that preserves device presence and inbound health.
    match futures::future::select(tick, received).await {
        futures::future::Either::Left(_) => Some(RelayMessage::Heartbeat),
        futures::future::Either::Right((command, _)) => command,
    }
}

async fn write_relay_message<S>(socket: &mut S, message: &RelayMessage) -> Result<()>
where
    S: futures::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let json = serde_json::to_string(message)?;
    tokio::time::timeout(RELAY_WRITE_TIMEOUT, socket.send(Message::Text(json.into())))
        .await
        .map_err(|_| anyhow!("Relay WebSocket write timed out"))??;
    Ok(())
}

async fn dial(ws_url: &str) -> Result<WsStream> {
    // Ensure CryptoProvider is installed before any rustls TLS handshake.
    // Startup already calls this; calling again is a no-op once installed and
    // protects reconnect / late-init paths.
    ensure_rustls_crypto_provider();

    let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(64 * 1024 * 1024))
        .max_frame_size(Some(64 * 1024 * 1024))
        .max_write_buffer_size(64 * 1024 * 1024);

    #[cfg(windows)]
    {
        await_dial(ws_url, async move {
            let request = ws_url
                .into_client_request()
                .map_err(|e| anyhow!("dial {ws_url}: build request failed: {e}"))?;

            // Wrap TLS connector construction in catch_unwind so that a panic
            // (e.g. duplicate CryptoProvider install) is converted to an error
            // instead of unwinding the tokio task and potentially crashing the
            // process.
            let connector = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_windows_rustls_connector()
            }))
            .map_err(|_| anyhow!("dial {ws_url}: TLS connector construction panicked"))??;

            let (stream, _) = tokio_tungstenite::connect_async_tls_with_config(
                request,
                Some(config),
                false,
                Some(connector),
            )
            .await
            .map_err(|e| anyhow!("dial {ws_url}: {e}"))?;
            Ok(stream)
        })
        .await
    }

    #[cfg(not(windows))]
    {
        // Non-Windows uses tokio-tungstenite's built-in rustls connector.
        // CryptoProvider must already be installed (see ensure_rustls_crypto_provider).
        await_dial(ws_url, async move {
            let (stream, _) =
                tokio_tungstenite::connect_async_with_config(ws_url, Some(config), false)
                    .await
                    .map_err(|e| anyhow!("dial {ws_url}: {e}"))?;
            Ok(stream)
        })
        .await
    }
}

async fn await_dial<T, F>(ws_url: &str, dial_future: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    tokio::time::timeout(RELAY_DIAL_TIMEOUT, dial_future)
        .await
        .map_err(|_| {
            anyhow!(
                "dial {ws_url}: connection timed out after {} seconds",
                RELAY_DIAL_TIMEOUT.as_secs()
            )
        })?
}

async fn await_relay_inbound<T, F>(inbound_future: F) -> std::result::Result<T, ()>
where
    F: std::future::Future<Output = T>,
{
    tokio::time::timeout(RELAY_INBOUND_IDLE_TIMEOUT, inbound_future)
        .await
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    #[test]
    fn device_http_endpoint_preserves_version_prefix_and_rejects_path_injection() {
        assert_eq!(
            super::device_message_endpoint("wss://remote.example/v/1.0.0/ws", "desktop-1")
                .unwrap()
                .as_str(),
            "https://remote.example/v/1.0.0/api/devices/desktop-1/messages"
        );
        assert_eq!(
            super::device_message_endpoint("ws://127.0.0.1:3000/ws", "desktop")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:3000/api/devices/desktop/messages"
        );
        for id in [
            "",
            ".",
            "..",
            "../other",
            "device?x=1",
            "%2f",
            "device#fragment",
        ] {
            assert!(super::device_message_endpoint("wss://remote.example/ws", id).is_err());
        }
        for url in [
            "https://remote.example/ws",
            "wss://user@remote.example/ws",
            "wss://remote.example/ws?token=x",
            "wss://remote.example/wrong",
        ] {
            assert!(super::device_message_endpoint(url, "desktop").is_err());
        }
    }

    use super::*;

    async fn connected_fixture() -> (
        RelayClient,
        mpsc::UnboundedReceiver<RelayEvent>,
        tokio::net::TcpListener,
        tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/ws", listener.local_addr().unwrap());
        let (client, events) = RelayClient::new();
        let (connected, socket) = tokio::join!(client.connect(&url), async {
            let (stream, _) = listener.accept().await.unwrap();
            tokio_tungstenite::accept_async(stream).await.unwrap()
        });
        connected.unwrap();
        (client, events, listener, socket)
    }

    #[tokio::test]
    async fn device_payload_uses_authenticated_http_and_reports_rejection() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (client, _events, listener, _socket) = connected_fixture().await;
        client
            .connect_authenticated("fixture-token", "Desktop")
            .await
            .unwrap();
        let server = tokio::spawn(async move {
            for status in ["204 No Content", "503 Service Unavailable"] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let (header_end, length) = loop {
                    let mut buffer = [0u8; 8192];
                    assert!(bytes.len() < 1024 * 1024);
                    let count = stream.read(&mut buffer).await.unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                        assert!(
                            header.starts_with("post /api/devices/controller/messages http/1.1")
                        );
                        assert!(header.contains("authorization: bearer fixture-token"));
                        let length: usize = header
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length:"))
                            .unwrap()
                            .trim()
                            .parse()
                            .unwrap();
                        break (end + 4, length);
                    }
                };
                while bytes.len() < header_end + length {
                    let mut buffer = [0u8; 8192];
                    let count = stream.read(&mut buffer).await.unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buffer[..count]);
                }
                let body: serde_json::Value =
                    serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap();
                assert_eq!(body["encrypted_data"].as_str().unwrap().len(), 256 * 1024);
                assert_eq!(body["correlation_id"], "correlation");
                let reply =
                    format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                stream.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        let payload = "a".repeat(256 * 1024);
        client
            .send_device_message("controller", "correlation", &payload, "nonce")
            .await
            .unwrap();
        let error = client
            .send_device_message("controller", "correlation", &payload, "nonce")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("503"));
        server.await.unwrap();
        client.disconnect().await;
    }

    #[tokio::test]
    async fn failed_initial_dial_returns_to_disconnected() {
        let (client, _) = RelayClient::new();
        assert!(client.connect("invalid://relay").await.is_err());
        assert_eq!(
            client.connection_state().await,
            ConnectionState::Disconnected
        );
        assert!(client.send(RelayMessage::Heartbeat).await.is_err());
    }

    #[tokio::test]
    async fn disconnect_closes_the_socket_without_waiting_for_inbound_timeout() {
        let (client, _, _listener, mut socket) = connected_fixture().await;
        client.disconnect().await;
        let closed = tokio::time::timeout(std::time::Duration::from_millis(500), socket.next())
            .await
            .expect("disconnect must release the socket promptly");
        assert!(!matches!(closed, Some(Ok(Message::Text(_)))));
    }

    #[tokio::test]
    async fn dropping_client_closes_its_socket() {
        let (client, _, _listener, mut socket) = connected_fixture().await;
        drop(client);
        tokio::time::timeout(std::time::Duration::from_millis(500), socket.next())
            .await
            .expect("dropping the owner must stop its transport tasks");
    }

    #[tokio::test]
    async fn disconnect_during_backoff_does_not_reconnect() {
        let (client, _, listener, mut socket) = connected_fixture().await;
        socket.close(None).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while client.connection_state().await != ConnectionState::Reconnecting {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        // Let the reconnect task enter its first backoff before disconnecting.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        client.disconnect().await;
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(2200), listener.accept())
                .await
                .is_err(),
            "a disconnected owner must not dial again"
        );
        assert_eq!(
            client.connection_state().await,
            ConnectionState::Disconnected
        );
    }

    #[tokio::test]
    async fn disconnect_cancels_an_in_progress_handshake() {
        use tokio::io::AsyncReadExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/ws", listener.local_addr().unwrap());
        let (client, _) = RelayClient::new();
        let client = Arc::new(client);
        let connecting_client = client.clone();
        let connecting = tokio::spawn(async move { connecting_client.connect(&url).await });
        let (mut socket, _) = listener.accept().await.unwrap();
        // Never answer the HTTP upgrade. Disconnect must cancel the dial too.
        client.disconnect().await;
        assert!(connecting.await.unwrap().is_err());
        let mut bytes = Vec::new();
        tokio::time::timeout(
            std::time::Duration::from_millis(500),
            socket.read_to_end(&mut bytes),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            client.connection_state().await,
            ConnectionState::Disconnected
        );
    }

    #[tokio::test]
    async fn replacement_connection_retires_the_old_socket_and_preserves_the_new_one() {
        let (client, _, _old_listener, mut old_socket) = connected_fixture().await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/ws", listener.local_addr().unwrap());
        let (result, mut socket) = tokio::join!(client.connect(&url), async {
            tokio_tungstenite::accept_async(listener.accept().await.unwrap().0)
                .await
                .unwrap()
        });
        result.unwrap();
        tokio::time::timeout(std::time::Duration::from_millis(500), old_socket.next())
            .await
            .expect("superseded socket must close");
        client.send(RelayMessage::Heartbeat).await.unwrap();
        let message = tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(
            serde_json::from_str::<RelayMessage>(&message.into_text().unwrap()).unwrap(),
            RelayMessage::Heartbeat
        ));
        assert_eq!(client.connection_state().await, ConnectionState::Connected);
        client.disconnect().await;
    }

    #[tokio::test]
    async fn reconnect_authenticates_account_before_new_commands() {
        let (client, _events, listener, mut socket) = connected_fixture().await;
        client
            .connect_authenticated("test-token", "test-device")
            .await
            .unwrap();
        socket.next().await.unwrap().unwrap();
        socket.close(None).await.unwrap();
        let mut replacement = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio_tungstenite::accept_async(listener.accept().await.unwrap().0)
                .await
                .unwrap()
        })
        .await
        .unwrap();
        let auth: RelayMessage = serde_json::from_str(
            &replacement
                .next()
                .await
                .unwrap()
                .unwrap()
                .into_text()
                .unwrap(),
        )
        .unwrap();
        assert!(
            matches!(auth, RelayMessage::AuthConnect { token, device_name, .. }
            if token == "test-token" && device_name == "test-device")
        );
        client.disconnect().await;
    }

    #[tokio::test]
    async fn full_outbound_queue_rejects_without_leaking_the_payload() {
        let (client, _) = RelayClient::new();
        let (tx, _rx) = mpsc::channel(1);
        {
            let mut owner = client.lifecycle.lock().unwrap();
            owner.state = ConnectionState::Connected;
            owner.cmd_tx = Some(tx);
            owner.reconnect_ctx = Some(ReconnectCtx {
                token: "accepted-token".into(),
                ..Default::default()
            });
        }
        client.send(RelayMessage::Heartbeat).await.unwrap();
        let error = client
            .send(RelayMessage::AuthConnect {
                token: "must-not-appear-in-error".into(),
                device_name: "device".into(),
                device_kind: "desktop".into(),
            })
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Relay send queue is full; request was not queued"
        );
        assert!(client
            .connect_authenticated("rejected-token", "device")
            .await
            .is_err());
        let owner = client.lifecycle.lock().unwrap();
        let ctx = owner.reconnect_ctx.as_ref().unwrap();
        assert_eq!(ctx.token, "accepted-token");
    }

    #[tokio::test(start_paused = true)]
    async fn heartbeat_deadline_is_not_starved_by_queued_commands() {
        let (tx, mut commands) = mpsc::channel(2);
        let period = std::time::Duration::from_secs(30);
        let mut heartbeat = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        for id in ["first", "second"] {
            tx.try_send(RelayMessage::AuthConnect {
                token: id.into(),
                device_name: "test-device".into(),
                device_kind: "desktop".into(),
            })
            .unwrap();
        }
        tokio::time::advance(period).await;
        assert!(
            matches!(
                next_relay_outbound(&mut commands, &mut heartbeat).await,
                Some(RelayMessage::Heartbeat)
            ),
            "a due heartbeat must progress even while the command queue is full"
        );
        for expected in ["first", "second"] {
            assert!(matches!(
                next_relay_outbound(&mut commands, &mut heartbeat).await,
                Some(RelayMessage::AuthConnect { token, .. }) if token == expected
            ));
        }
        drop(tx);
        assert!(next_relay_outbound(&mut commands, &mut heartbeat)
            .await
            .is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_writes_are_bounded() {
        let mut sink = Box::pin(futures::sink::unfold((), |_, _: Message| async {
            std::future::pending::<std::result::Result<(), tokio_tungstenite::tungstenite::Error>>()
                .await
        }));
        let error = write_relay_message(&mut sink, &RelayMessage::Heartbeat)
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "Relay WebSocket write timed out");
    }

    #[tokio::test(start_paused = true)]
    async fn dial_timeout_bounds_a_pending_connection_attempt() {
        let result = await_dial(
            "wss://relay.example.invalid/ws",
            std::future::pending::<Result<()>>(),
        )
        .await;

        let error = result.expect_err("pending dial must be bounded by the connection timeout");
        assert_eq!(
            error.to_string(),
            "dial wss://relay.example.invalid/ws: connection timed out after 15 seconds"
        );
    }

    #[tokio::test]
    async fn dial_timeout_preserves_connection_errors() {
        let result = await_dial::<(), _>(
            "wss://relay.example.invalid/ws",
            std::future::ready(Err(anyhow!("dial failed before timeout"))),
        )
        .await;

        assert_eq!(
            result.expect_err("dial error must be returned").to_string(),
            "dial failed before timeout"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn inbound_idle_timeout_detects_a_half_open_socket() {
        let result = await_relay_inbound(std::future::pending::<()>()).await;
        assert!(result.is_err(), "an idle relay stream must time out");
    }
}

#[cfg(windows)]
fn build_windows_rustls_connector() -> Result<Connector> {
    openbitfun_services_core::tls_provider::ensure_ring_crypto_provider();

    let mut root_store = rustls::RootCertStore::empty();

    let native_certs = rustls_native_certs::load_native_certs();
    if !native_certs.errors.is_empty() {
        warn!(
            "Windows native root certificate loading errors: {:?}",
            native_certs.errors
        );
    }
    let (added, ignored) = root_store.add_parsable_certificates(native_certs.certs);
    debug!(
        "Loaded current-user Windows root certificates, added={}, ignored={}",
        added, ignored
    );

    if let Ok(local_machine_root) = schannel::cert_store::CertStore::open_local_machine("ROOT") {
        let local_machine_der_certs = local_machine_root
            .certs()
            .map(|cert| rustls::pki_types::CertificateDer::from(cert.to_der().to_vec()))
            .collect::<Vec<_>>();
        let total = local_machine_der_certs.len();
        let (added, ignored) = root_store.add_parsable_certificates(local_machine_der_certs);
        debug!(
            "Loaded local-machine Windows root certificates, total={}, added={}, ignored={}",
            total, added, ignored
        );
    } else {
        warn!("Failed to open local-machine Windows ROOT certificate store");
    }

    if root_store.is_empty() {
        return Err(anyhow!(
            "No trusted Windows root certificates available for relay connection"
        ));
    }

    let client_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(Connector::Rustls(std::sync::Arc::new(client_config)))
}
