//! MCP service contracts.
//!
//! `bitfun-core::service::mcp` remains as the compatibility facade for the
//! legacy public path.

mod tool_info;
mod tool_name;

pub mod auth;
pub mod config;
pub mod protocol;
pub mod server;

pub use auth::*;
pub use config::*;
pub use protocol::*;
pub use server::*;
pub use tool_info::McpToolInfo;
pub use tool_name::{
    MCP_TOOL_DELIMITER, MCP_TOOL_PREFIX, build_mcp_tool_name, normalize_name_for_mcp,
};
