//! Transport helpers shared by App Server clients and hosts.

use agent_client_protocol::Channel;

/// Build a paired in-process server/client transport.
pub fn in_memory_channel_pair() -> (Channel, Channel) {
    Channel::duplex()
}
