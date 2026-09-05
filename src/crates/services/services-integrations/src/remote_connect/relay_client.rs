//! WebSocket client for connecting to the Relay Server.
//!
//! Manages the desktop-side WebSocket connection. In the new architecture the
//! relay bridges HTTP requests from mobile to the desktop via WebSocket.
//! The desktop receives `PairRequest` and `Command` messages (with correlation
//! IDs) and responds with `RelayResponse`.
//!
//! Supports automatic reconnect with exponential backoff and room re-creation
//! so that in-flight QR codes remain valid.

use anyhow::{anyhow, Result};
use futures::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot, RwLock};
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
const RELAY_INBOUND_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(75);

/// Messages in the relay protocol (both directions).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RelayMessage {
    // ── Outbound (desktop → relay) ──────────────────────────────────
    CreateRoom {
        room_id: Option<String>,
        device_id: String,
        device_type: String,
        public_key: String,
    },
    /// Respond to a bridged HTTP request identified by `correlation_id`.
    RelayResponse {
        correlation_id: String,
        encrypted_data: String,
        nonce: String,
    },
    Heartbeat,
    /// Account-authenticated connect (parallel to CreateRoom for device
    /// routing). Validates the token and registers this device.
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
    RoomCreated {
        room_id: String,
    },
    /// Mobile pairing request forwarded by the relay.
    PairRequest {
        correlation_id: String,
        public_key: String,
        device_id: String,
        device_name: String,
    },
    /// Encrypted command from mobile forwarded by the relay.
    Command {
        correlation_id: String,
        encrypted_data: String,
        nonce: String,
    },
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
    RoomCreated {
        room_id: String,
    },
    /// Mobile wants to pair.
    PairRequest {
        correlation_id: String,
        public_key: String,
        device_id: String,
        device_name: String,
    },
    /// Mobile sent an encrypted command.
    CommandReceived {
        correlation_id: String,
        encrypted_data: String,
        nonce: String,
    },
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
    device_id: String,
    room_id: String,
    public_key: String,
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
    room_id: Arc<RwLock<Option<String>>>,
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
            room_id: Arc::new(RwLock::new(None)),
        };
        (client, event_rx)
    }

    pub async fn connection_state(&self) -> ConnectionState {
        self.lifecycle.lock().unwrap().state.clone()
    }

    pub async fn connect(&self, ws_url: &str) -> Result<()> {
        let (ready_tx, ready_rx) = oneshot::channel();
        let generation = {
            let mut room_id = self.room_id.write().await;
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
            *room_id = None;
            let generation = owner.generation;
            owner.task = Some(tokio::spawn(Self::run_connection(
                self.lifecycle.clone(),
                self.room_id.clone(),
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
        room_id: Arc<RwLock<Option<String>>>,
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
            Self::run_socket(socket, cmd_rx, &lifecycle, &room_id, &event_tx, generation).await;
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
        if !ctx.room_id.is_empty() {
            write_relay_message(
                &mut socket,
                &RelayMessage::CreateRoom {
                    room_id: Some(ctx.room_id.clone()),
                    device_id: ctx.device_id.clone(),
                    device_type: "desktop".to_string(),
                    public_key: ctx.public_key.clone(),
                },
            )
            .await?;
        }
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
        room_id: &Arc<RwLock<Option<String>>>,
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
                        Ok(msg) => {
                            Self::dispatch(msg, event_tx, room_id, lifecycle, generation).await
                        }
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
        room_id_store: &Arc<RwLock<Option<String>>>,
        lifecycle: &ConnectionOwner,
        generation: u64,
    ) {
        let mut room_id_store = room_id_store.write().await;
        let mut owner = lifecycle.lock().unwrap();
        if owner.generation != generation {
            return;
        }
        match msg {
            RelayMessage::RoomCreated { room_id } => {
                debug!("Room created/restored: {room_id}");
                *room_id_store = Some(room_id.clone());
                if let Some(ctx) = owner.reconnect_ctx.as_mut() {
                    ctx.room_id = room_id.clone();
                }
                let _ = event_tx.send(RelayEvent::RoomCreated { room_id });
            }
            RelayMessage::PairRequest {
                correlation_id,
                public_key,
                device_id,
                device_name,
            } => {
                info!("PairRequest from {device_id}");
                let _ = event_tx.send(RelayEvent::PairRequest {
                    correlation_id,
                    public_key,
                    device_id,
                    device_name,
                });
            }
            RelayMessage::Command {
                correlation_id,
                encrypted_data,
                nonce,
            } => {
                debug!("Command received, corr={correlation_id}");
                let _ = event_tx.send(RelayEvent::CommandReceived {
                    correlation_id,
                    encrypted_data,
                    nonce,
                });
            }
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

    pub async fn create_room(
        &self,
        device_id: &str,
        public_key: &str,
        room_id: Option<&str>,
    ) -> Result<()> {
        let mut owner = self.lifecycle.lock().unwrap();
        Self::enqueue(
            &owner,
            RelayMessage::CreateRoom {
                room_id: room_id.map(str::to_string),
                device_id: device_id.to_string(),
                device_type: "desktop".to_string(),
                public_key: public_key.to_string(),
            },
        )?;
        if let Some(ctx) = owner.reconnect_ctx.as_mut() {
            ctx.device_id = device_id.to_string();
            ctx.room_id = room_id.unwrap_or_default().to_string();
            ctx.public_key = public_key.to_string();
        }
        Ok(())
    }

    /// Send a relay response back to the relay server for a bridged HTTP request.
    pub async fn send_relay_response(
        &self,
        correlation_id: &str,
        encrypted_data: &str,
        nonce: &str,
    ) -> Result<()> {
        self.send(RelayMessage::RelayResponse {
            correlation_id: correlation_id.to_string(),
            encrypted_data: encrypted_data.to_string(),
            nonce: nonce.to_string(),
        })
        .await
    }

    /// Authenticate this connection with an account token (parallel to
    /// `create_room` for the device-routing pathway). The relay validates the
    /// token and registers the device; success arrives as `RelayEvent::AuthOk`.
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

    /// Send an encrypted payload to another device in the same account. The
    /// relay routes by `target_device_id` without decrypting.
    pub async fn send_device_message(
        &self,
        target_device_id: &str,
        correlation_id: &str,
        encrypted_data: &str,
        nonce: &str,
    ) -> Result<()> {
        self.send(RelayMessage::DeviceMessage {
            target_device_id: target_device_id.to_string(),
            correlation_id: correlation_id.to_string(),
            encrypted_data: encrypted_data.to_string(),
            nonce: nonce.to_string(),
        })
        .await
    }

    pub async fn disconnect(&self) {
        let task = {
            let mut room_id = self.room_id.write().await;
            let mut owner = self.lifecycle.lock().unwrap();
            owner.generation += 1;
            owner.state = ConnectionState::Disconnected;
            owner.cmd_tx = None;
            owner.reconnect_ctx = None;
            *room_id = None;
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

    pub fn room_id(&self) -> &Arc<RwLock<Option<String>>> {
        &self.room_id
    }
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
    // must not starve the keepalive that preserves the room and inbound health.
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
    async fn reconnect_restores_server_assigned_room_and_account_before_new_commands() {
        let (client, mut events, listener, mut socket) = connected_fixture().await;
        client
            .create_room("device", "public-key", None)
            .await
            .unwrap();
        client
            .connect_authenticated("test-token", "test-device")
            .await
            .unwrap();
        for _ in 0..2 {
            socket.next().await.unwrap().unwrap();
        }
        socket
            .send(Message::Text(
                serde_json::to_string(&RelayMessage::RoomCreated {
                    room_id: "assigned-room".into(),
                })
                .unwrap()
                .into(),
            ))
            .await
            .unwrap();
        while !matches!(events.recv().await, Some(RelayEvent::RoomCreated { .. })) {}
        socket.close(None).await.unwrap();
        let mut replacement = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio_tungstenite::accept_async(listener.accept().await.unwrap().0)
                .await
                .unwrap()
        })
        .await
        .unwrap();
        let room: RelayMessage = serde_json::from_str(
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
            matches!(room, RelayMessage::CreateRoom { room_id: Some(id), device_id, public_key, .. }
            if id == "assigned-room" && device_id == "device" && public_key == "public-key")
        );
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
                room_id: "accepted-room".into(),
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
            .create_room("device", "key", Some("rejected-room"))
            .await
            .is_err());
        assert!(client
            .connect_authenticated("rejected-token", "device")
            .await
            .is_err());
        let owner = client.lifecycle.lock().unwrap();
        let ctx = owner.reconnect_ctx.as_ref().unwrap();
        assert_eq!(ctx.room_id, "accepted-room");
        assert_eq!(ctx.token, "accepted-token");
    }

    #[tokio::test(start_paused = true)]
    async fn heartbeat_deadline_is_not_starved_by_queued_commands() {
        let (tx, mut commands) = mpsc::channel(2);
        let period = std::time::Duration::from_secs(30);
        let mut heartbeat = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        for id in ["first", "second"] {
            tx.try_send(RelayMessage::RelayResponse {
                correlation_id: id.into(),
                encrypted_data: "test-payload".into(),
                nonce: "test-nonce".into(),
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
                Some(RelayMessage::RelayResponse { correlation_id, .. }) if correlation_id == expected
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
