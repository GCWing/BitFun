//! MCP configuration data contracts.

mod cursor_format;
mod json_config;
mod location;

pub use cursor_format::{config_to_cursor_format, parse_cursor_format};
pub use json_config::{
    MCPJsonConfigValidationError, format_mcp_json_config_value, validate_mcp_json_config,
};
pub use location::ConfigLocation;
