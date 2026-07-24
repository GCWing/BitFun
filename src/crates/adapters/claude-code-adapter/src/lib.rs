//! Runtime-free Claude Code source adapter.

mod agent_source;
mod command_source;
mod hook_source;
mod mcp_source;

pub use agent_source::{ClaudeCodeSubagentProvider, ClaudeCodeSubagentProviderOptions};
pub use command_source::{ClaudeCodeCommandProvider, ClaudeCodeCommandProviderOptions};
pub use hook_source::{ClaudeCodeHookProvider, ClaudeCodeHookProviderOptions};
pub use mcp_source::{ClaudeCodeMcpProvider, ClaudeCodeMcpProviderOptions};
