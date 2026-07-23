//! MCP adapter module
//!
//! Adapts MCP resources, prompts, and tools to BitFun's agentic system.

mod context;
mod prompt;
mod resource;
mod tool;

pub use bitfun_services_integrations::mcp::adapter::MCPContextEnhancer as ContextEnhancer;
pub use context::MCPContextProvider;
pub use prompt::PromptAdapter;
pub use resource::ResourceAdapter;
pub use tool::MCPToolAdapter;
pub(crate) use tool::{MCPToolContextPolicy, MCPWorkspaceToolRoute};
