//! MCP adapter module
//!
//! Adapts MCP resources, prompts, and tools to OpenBitFun's agentic system.

mod context;
mod prompt;
mod resource;
mod tool;

pub use context::MCPContextProvider;
pub use openbitfun_services_integrations::mcp::adapter::MCPContextEnhancer as ContextEnhancer;
pub use prompt::PromptAdapter;
pub use resource::ResourceAdapter;
pub use tool::MCPToolAdapter;
pub(crate) use tool::{MCPToolContextPolicy, MCPWorkspaceToolRoute};
