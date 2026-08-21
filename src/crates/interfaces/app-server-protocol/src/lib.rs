//! Behavior-light wire contracts for BitFun App Server clients and hosts.
//!
//! This crate intentionally has no dependency on Core, Runtime implementations,
//! services, product assembly, or a UI framework. Server adapters translate
//! these wire DTOs to owner types at the interface boundary.
//! Wire schemas live under [`schemas`] and are grouped into modules named after
//! their JSON-RPC method domain; shared event envelopes remain in
//! [`schemas::event`].
//! The default `rpc` feature attaches ACP JSON-RPC traits and exposes the role
//! and transport helpers. The independent `ts` feature exports wire DTOs
//! without compiling that runtime integration.

pub mod config;
#[cfg(feature = "rpc")]
pub mod role;
pub mod schemas;
#[cfg(feature = "rpc")]
pub mod transport;

#[cfg(feature = "rpc")]
pub use role::{AppClient, AppServer};
// Keep the established public domain paths while the schema sources live in a
// single physical directory.
pub use schemas::{
    account, agent, app, error, event, external_source, git, hook, i18n, mcp, method, model,
    permission, search, session, skill, subagent, workspace, worktree,
};

/// Current App Server protocol version.
pub const PROTOCOL_VERSION: u32 = 3;

/// Oldest protocol version this implementation accepts.
pub const MIN_PROTOCOL_VERSION: u32 = 2;

#[cfg(test)]
mod protocol_version_tests {
    use super::PROTOCOL_VERSION;

    #[test]
    fn application_protocol_stays_at_version_3() {
        assert_eq!(PROTOCOL_VERSION, 3);
    }
}
