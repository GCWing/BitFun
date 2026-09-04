//! Versioned local SDK Host adapter for the shared OpenBitFun Agent Runtime.
//!
//! This crate owns only protocol and connection lifecycle. Agent execution,
//! Session persistence, Tool/MCP, Permission, and Hook behavior remain in the
//! existing runtime owners supplied through [`openbitfun_agent_runtime`].

pub mod host;
pub mod protocol;
