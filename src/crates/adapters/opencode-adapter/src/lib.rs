//! OpenCode-compatible plugin adapter.
//!
//! The production surface is intentionally small: load OpenCode-compatible
//! managed package content and optional activation authority as a Plugin Runtime
//! Host adapter plus typed dispatch targets. The adapter does not execute
//! JavaScript, install npm packages, or depend on a user-local `opencode` CLI.

mod agent_source;
mod command_source;
mod mcp_source;
mod source_adapter;
mod tool_source;

pub use agent_source::{OpenCodeSubagentProvider, OpenCodeSubagentProviderOptions};
pub use command_source::{OpenCodeCommandProvider, OpenCodeCommandProviderOptions};
pub use mcp_source::{OpenCodeMcpProvider, OpenCodeMcpProviderOptions};
pub use source_adapter::load_opencode_package_adapter;
pub use tool_source::{OpenCodeToolProvider, OpenCodeToolProviderOptions};
