//! MCP protocol layer
//!
//! Implements the core protocol definitions of Model Context Protocol and JSON-RPC 2.0
//! communication.

mod jsonrpc;
mod transport;
mod transport_remote;
mod types;

pub use jsonrpc::*;
pub use transport::*;
pub use transport_remote::*;
pub use types::*;
