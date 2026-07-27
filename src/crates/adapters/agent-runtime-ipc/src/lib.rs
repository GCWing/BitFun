//! Private local IPC foundation for a future shared BitFun Agent Runtime.
//!
//! The crate implements only discovery, authentication, protocol framing, and
//! Health. All items remain crate-internal until a reviewed first-party adapter
//! becomes a production consumer. This is not a public SDK or Runtime owner.

#![allow(dead_code, unreachable_pub)]

mod client;
mod discovery;
mod framing;
mod ipc;
mod operation;
mod protocol;
mod server;

#[cfg(test)]
pub(crate) use client::{RuntimeIpcClient, RuntimeIpcClientError};
pub(crate) use discovery::{
    DiscoveryRecord, DiscoveryStore, RuntimeInstanceIdentity, RuntimeInstanceLock,
    RuntimeIpcDiscoveryError,
};
#[cfg(test)]
pub(crate) use framing::MAX_FRAME_BYTES;
pub(crate) use framing::{read_frame, write_frame, RuntimeIpcIoError};
pub(crate) use ipc::{
    LocalIpcEndpoint, LocalIpcListener, LocalIpcStream, RuntimeIpcTransportError,
};
pub(crate) use operation::{RuntimeIpcOperation, RuntimeIpcOperationResult};
pub(crate) use protocol::{
    HealthResult, InitializeRequest, InitializeResult, RuntimeIpcCapabilities, RuntimeIpcError,
    RuntimeIpcErrorCode, RuntimeIpcFrame, PROTOCOL_VERSION,
};
#[cfg(test)]
pub(crate) use server::{RuntimeIpcServer, RuntimeIpcServerConfig};

#[cfg(test)]
#[path = "tests/discovery_and_framing.rs"]
mod discovery_and_framing_tests;
#[cfg(test)]
#[path = "tests/local_health.rs"]
mod local_health_tests;
#[cfg(test)]
#[path = "tests/protocol_contracts.rs"]
mod protocol_contract_tests;
