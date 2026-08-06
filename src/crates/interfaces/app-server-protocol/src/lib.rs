//! Behavior-light wire contracts for BitFun App Server clients and hosts.
//!
//! This crate intentionally has no dependency on Core, Runtime implementations,
//! services, product assembly, or a UI framework. Server adapters translate
//! these wire DTOs to owner types at the interface boundary.
//! Wire schemas live under [`schemas`] and are grouped into modules named after
//! their JSON-RPC method domain; shared event envelopes remain in
//! [`schemas::event`].

pub mod config;
pub mod role;
pub mod schemas;
pub mod transport;

pub use role::{AppClient, AppServer};
// Keep the established public domain paths while the schema sources live in a
// single physical directory.
pub use schemas::{
    account, agent, app, error, event, external_source, hook, mcp, method, model, session, skill,
    subagent, workspace,
};

/// Current App Server protocol version.
pub const PROTOCOL_VERSION: u32 = 3;

/// Oldest protocol version this implementation accepts.
pub const MIN_PROTOCOL_VERSION: u32 = 2;
