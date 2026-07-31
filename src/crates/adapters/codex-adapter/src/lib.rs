//! Runtime-free Codex source adapter.

mod agent_source;
mod hook_source;
mod instruction_source;
mod mcp_source;

pub use agent_source::{CodexSubagentProvider, CodexSubagentProviderOptions};
pub use hook_source::{CodexHookProvider, CodexHookProviderOptions};
pub use instruction_source::{load_codex_user_instructions, CodexInstructionSourceOptions};
pub use mcp_source::{CodexMcpProvider, CodexMcpProviderOptions};
