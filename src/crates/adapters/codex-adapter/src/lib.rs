//! Runtime-free Codex source adapter.

mod agent_source;
mod hook_source;
mod mcp_source;

pub use agent_source::{CodexSubagentProvider, CodexSubagentProviderOptions};
pub use hook_source::{CodexHookProvider, CodexHookProviderOptions};
pub use mcp_source::{CodexMcpProvider, CodexMcpProviderOptions};
