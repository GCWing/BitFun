//! MCP configuration data contracts.

mod cursor_format;
mod location;

pub use cursor_format::{config_to_cursor_format, parse_cursor_format};
pub use location::ConfigLocation;
