//! MCP configuration data contracts.

mod cursor_format;
mod json_config;
mod location;
mod service_helpers;

pub use cursor_format::{config_to_cursor_format, parse_cursor_format};
pub use json_config::{
    MCPJsonConfigValidationError, format_mcp_json_config_value, validate_mcp_json_config,
};
pub use location::ConfigLocation;
pub use service_helpers::{
    get_mcp_remote_authorization_source, get_mcp_remote_authorization_value,
    has_mcp_remote_authorization, has_mcp_remote_oauth, has_mcp_remote_xaa,
    merge_mcp_server_config_source, merge_mcp_server_config_sources,
    normalize_mcp_authorization_value, parse_mcp_config_array, remove_mcp_authorization_keys,
};
