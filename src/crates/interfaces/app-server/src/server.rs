//! BitFun app-server assembly over the generic `AppServer` role.
//!
//! Request handlers are grouped by product domain under [`handlers`]. This
//! module owns the server lifecycle, handler integration order, transport
//! connection, and event forwarding.

mod event_forwarder;
mod fallback;
mod handlers;
mod wire;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use agent_client_protocol::{BoxFuture, Channel, ConnectTo, ConnectionTo, Result};
use bitfun_app_server_protocol::app::TransportLimits;

use crate::agent::BitfunAppRuntime;
use crate::role::{AppClient, AppServer};

static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

/// Exposes transport closure to the connection foreground.
///
/// ACP 0.12 waits for every protocol actor when one channel direction closes,
/// while the connection foreground itself keeps the outgoing actor alive. The
/// small bridge below observes either transport direction closing so the
/// foreground can release its `ConnectionTo` and let ACP finish normally.
struct ConnectionAwareTransport<T> {
    transport: T,
    closed: tokio::sync::watch::Sender<bool>,
}

impl<T> ConnectionAwareTransport<T> {
    fn new(transport: T) -> (Self, tokio::sync::watch::Receiver<bool>) {
        let (closed, receiver) = tokio::sync::watch::channel(false);
        (Self { transport, closed }, receiver)
    }
}

impl<T> ConnectTo<AppServer> for ConnectionAwareTransport<T>
where
    T: ConnectTo<AppServer>,
{
    async fn connect_to(self, client: impl ConnectTo<AppClient>) -> Result<()> {
        let Self { transport, closed } = self;
        let result = transport.connect_to(client).await;
        closed.send_replace(true);
        result
    }

    fn into_channel_and_future(self) -> (Channel, BoxFuture<'static, Result<()>>) {
        let Self { transport, closed } = self;
        let (transport_channel, transport_future) = transport.into_channel_and_future();
        let (protocol_channel, bridge_channel) = Channel::duplex();

        let future = Box::pin(async move {
            let Channel {
                rx: transport_incoming,
                tx: transport_outgoing,
            } = transport_channel;
            let Channel {
                rx: protocol_outgoing,
                tx: protocol_incoming,
            } = bridge_channel;

            let incoming = Channel {
                rx: transport_incoming,
                tx: protocol_incoming,
            }
            .copy();
            let outgoing = Channel {
                rx: protocol_outgoing,
                tx: transport_outgoing,
            }
            .copy();
            let transport_lifecycle = async move {
                match transport_future.await {
                    Ok(()) => std::future::pending::<Result<()>>().await,
                    Err(error) => Err(error),
                }
            };

            let result = tokio::select! {
                result = incoming => result,
                result = outgoing => result,
                result = transport_lifecycle => result,
            };
            closed.send_replace(true);
            result
        });

        (protocol_channel, future)
    }
}

pub(super) struct ConnectionEventState {
    id: String,
    agent_sequence: AtomicU64,
    permission_sequence: AtomicU64,
    permission_order: std::sync::Mutex<()>,
    config_sequence: AtomicU64,
    external_source_sequence: AtomicU64,
}

impl ConnectionEventState {
    fn new() -> Self {
        Self {
            id: format!(
                "app-server-{}",
                NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed)
            ),
            agent_sequence: AtomicU64::new(0),
            permission_sequence: AtomicU64::new(0),
            permission_order: std::sync::Mutex::new(()),
            config_sequence: AtomicU64::new(0),
            external_source_sequence: AtomicU64::new(0),
        }
    }

    pub(super) fn cursor(
        &self,
        stream: bitfun_app_server_protocol::event::EventStream,
    ) -> bitfun_app_server_protocol::event::EventCursor {
        let sequence = match stream {
            bitfun_app_server_protocol::event::EventStream::Agent => &self.agent_sequence,
            bitfun_app_server_protocol::event::EventStream::Permission => &self.permission_sequence,
            bitfun_app_server_protocol::event::EventStream::Config => &self.config_sequence,
            bitfun_app_server_protocol::event::EventStream::ExternalSource => {
                &self.external_source_sequence
            }
        };
        bitfun_app_server_protocol::event::EventCursor {
            connection_id: self.id.clone(),
            stream,
            sequence: sequence.load(Ordering::Acquire),
        }
    }

    pub(super) fn next_cursor(
        &self,
        stream: bitfun_app_server_protocol::event::EventStream,
    ) -> bitfun_app_server_protocol::event::EventCursor {
        let sequence = match stream {
            bitfun_app_server_protocol::event::EventStream::Agent => &self.agent_sequence,
            bitfun_app_server_protocol::event::EventStream::Permission => &self.permission_sequence,
            bitfun_app_server_protocol::event::EventStream::Config => &self.config_sequence,
            bitfun_app_server_protocol::event::EventStream::ExternalSource => {
                &self.external_source_sequence
            }
        };
        bitfun_app_server_protocol::event::EventCursor {
            connection_id: self.id.clone(),
            stream,
            sequence: sequence.fetch_add(1, Ordering::AcqRel) + 1,
        }
    }

    pub(super) fn capture_permission_snapshot<T>(
        &self,
        capture: impl FnOnce() -> T,
    ) -> (T, bitfun_app_server_protocol::event::EventCursor) {
        let _ordered = self
            .permission_order
            .lock()
            .expect("App Server permission event order poisoned");
        let snapshot = capture();
        let cursor = self.cursor(bitfun_app_server_protocol::event::EventStream::Permission);
        (snapshot, cursor)
    }

    pub(super) fn forward_permission<T>(
        &self,
        forward: impl FnOnce(bitfun_app_server_protocol::event::EventCursor) -> T,
    ) -> T {
        let _ordered = self
            .permission_order
            .lock()
            .expect("App Server permission event order poisoned");
        forward(self.next_cursor(bitfun_app_server_protocol::event::EventStream::Permission))
    }
}

#[cfg(test)]
mod tests {
    use super::ConnectionEventState;
    use bitfun_app_server_protocol::event::EventStream;
    use std::sync::{mpsc, Arc};

    #[test]
    fn permission_event_started_during_snapshot_stays_after_the_snapshot_watermark() {
        let event_state = Arc::new(ConnectionEventState::new());
        let (snapshot_started_tx, snapshot_started_rx) = mpsc::channel();
        let (release_snapshot_tx, release_snapshot_rx) = mpsc::channel();
        let sync_state = event_state.clone();
        let sync = std::thread::spawn(move || {
            sync_state.capture_permission_snapshot(|| {
                snapshot_started_tx
                    .send(())
                    .expect("snapshot start should be observed");
                release_snapshot_rx
                    .recv()
                    .expect("snapshot should be released");
                false
            })
        });

        snapshot_started_rx
            .recv()
            .expect("snapshot should start before forwarding");
        let (forward_attempted_tx, forward_attempted_rx) = mpsc::channel();
        let (forwarded_tx, forwarded_rx) = mpsc::channel();
        let forward_state = event_state.clone();
        let forward = std::thread::spawn(move || {
            forward_attempted_tx
                .send(())
                .expect("forward attempt should be observed");
            forward_state.forward_permission(|cursor| {
                forwarded_tx
                    .send(cursor)
                    .expect("forwarded cursor should be observed");
            });
        });

        forward_attempted_rx
            .recv()
            .expect("event forwarder should attempt to publish");
        assert!(
            forwarded_rx.try_recv().is_err(),
            "permission forwarding must wait for the snapshot watermark"
        );
        release_snapshot_tx
            .send(())
            .expect("snapshot release should be delivered");

        let (snapshot_contains_event, watermark) = sync.join().expect("sync thread should finish");
        forward.join().expect("forward thread should finish");
        let event_cursor = forwarded_rx.recv().expect("event should be forwarded");

        assert_eq!(watermark.stream, EventStream::Permission);
        assert!(!snapshot_contains_event);
        assert!(
            event_cursor.sequence > watermark.sequence,
            "an event absent from the snapshot must remain visible after its watermark"
        );
    }
}

/// BitFun agent kernel server over the generic app-server role.
#[derive(Clone)]
pub struct BitfunAppServer {
    runtime: Arc<BitfunAppRuntime>,
    transport_limits: TransportLimits,
}

impl BitfunAppServer {
    pub fn new(runtime: BitfunAppRuntime) -> Self {
        Self {
            runtime: Arc::new(runtime),
            transport_limits: TransportLimits {
                max_frame_bytes: 16 * 1024 * 1024,
                event_buffer_capacity: 1024,
            },
        }
    }

    /// Override the maximum frame size advertised for this Host transport.
    pub fn with_max_frame_bytes(mut self, max_frame_bytes: u64) -> Self {
        assert!(
            max_frame_bytes > 0,
            "App Server frame limit must be positive"
        );
        self.transport_limits.max_frame_bytes = max_frame_bytes;
        self
    }

    /// Return the shared runtime used by this server.
    pub fn runtime(&self) -> &BitfunAppRuntime {
        &self.runtime
    }

    /// Serve the complete app-server surface on the supplied transport.
    pub async fn serve(self, transport: impl ConnectTo<AppServer> + 'static) -> Result<()> {
        let runtime = self.runtime;
        let transport_limits = self.transport_limits;
        let event_state = Arc::new(ConnectionEventState::new());
        let protocol_state = handlers::app::ConnectionProtocolState::new();
        let (transport, mut transport_closed) = ConnectionAwareTransport::new(transport);

        AppServer
            .builder()
            .name("bitfun-app-server")
            .with_connection_builder(handlers::app::lifecycle_builder(
                protocol_state.clone(),
                runtime.clone(),
                transport_limits,
            ))
            .with_handler(handlers::app::NegotiationGate::new(protocol_state.clone()))
            .with_connection_builder(handlers::app::event_sync_builder(
                runtime.clone(),
                event_state.clone(),
            ))
            .with_connection_builder(handlers::agent::builder(runtime.clone()))
            .with_connection_builder(handlers::account::builder())
            .with_connection_builder(handlers::session::builder(runtime.clone()))
            .with_connection_builder(handlers::permission::builder(runtime.clone()))
            .with_connection_builder(handlers::workspace::builder(runtime.clone()))
            .with_connection_builder(handlers::worktree::builder())
            .with_connection_builder(handlers::model::builder())
            .with_connection_builder(handlers::skill::builder())
            .with_connection_builder(handlers::subagent::builder())
            .with_connection_builder(handlers::mcp::builder())
            .with_connection_builder(handlers::external_source::builder())
            .with_connection_builder(handlers::hook::builder())
            .with_connection_builder(handlers::git::builder())
            .with_connection_builder(handlers::config::builder())
            .with_connection_builder(handlers::i18n::builder())
            .with_connection_builder(fallback::builder())
            .connect_with(transport, async move |cx: ConnectionTo<AppClient>| {
                let connection_lifecycle = async move {
                    match protocol_state.wait_for_decision().await {
                        handlers::app::ProtocolNegotiation::Accepted => {
                            let subscriptions = protocol_state
                                .take_event_subscriptions()
                                .ok_or_else(|| agent_client_protocol::Error::internal_error())?;
                            event_forwarder::run(subscriptions, cx, event_state).await
                        }
                        handlers::app::ProtocolNegotiation::Rejected => {
                            // ACP 0.12 has no response-flush acknowledgement.
                            // Keep handlers alive and universally gated until
                            // the transport bridge observes peer disconnect.
                            std::future::pending::<Result<()>>().await
                        }
                        handlers::app::ProtocolNegotiation::Pending
                        | handlers::app::ProtocolNegotiation::Negotiating => unreachable!(
                            "protocol negotiation wait returned before a decision was recorded"
                        ),
                    }
                };

                tokio::select! {
                    result = connection_lifecycle => result,
                    _ = transport_closed.changed() => Ok(()),
                }
            })
            .await
    }
}
