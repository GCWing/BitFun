//! MCP adapter module
//!
//! Adapts MCP resources, prompts, and tools to BitFun's agentic system.

mod context;
mod prompt;
mod resource;
mod tool;

pub use context::{ContextEnhancer, MCPContextProvider};
pub use prompt::PromptAdapter;
pub use resource::ResourceAdapter;
pub use tool::MCPToolAdapter;
