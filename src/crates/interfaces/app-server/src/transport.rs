//! Transport helpers for wiring an in-process app-server connection.

use agent_client_protocol::Channel;

/// Build a paired in-process server/client transport over two `mpsc` channels.
///
/// Returns `(server_channel, client_channel)`, a connected pair of
/// [`Channel`]s from [`Channel::duplex`]. The pair moves typed
/// [`jsonrpcmsg::Message`](agent_client_protocol) values directly between the
/// two endpoints -- no `serde_json::to_string`/`from_str` on the wire, only
/// the typed-request ↔ `Message::Request(params: Value)` value conversion
/// (`to_value`/`from_value`, which is value conversion, not serialization).
///
/// `Channel` implements [`agent_client_protocol::ConnectTo`] for any role, so
/// either endpoint can be passed to [`crate::BitfunAppServer::serve`] or an
/// `AppClient` builder directly. For a byte-stream boundary (stdio, sockets,
/// ...) the caller should construct an `agent_client_protocol::ByteStreams`/
/// `Lines` transport directly; this helper is for same-process Rust pairs.
pub fn in_memory_channel_pair() -> (Channel, Channel) {
    Channel::duplex()
}
