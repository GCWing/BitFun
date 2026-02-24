#![allow(non_snake_case)]
// BitFun Core Library - Platform-agnostic business logic
// Four-layer architecture: Util -> Infrastructure -> Service -> Agentic

pub mod agentic; // Agentic service layer - Agent system, tool system
pub mod function_agents;
pub mod infrastructure; // Infrastructure layer - AI clients, storage, logging, events
pub mod service; // Service layer - Workspace, Config, FileSystem, Terminal, Git
pub mod util; // Utility layer - General types, errors, helper functions // Function Agents - Function-based agents
              // Re-export debug_log from infrastructure for backward compatibility
pub use infrastructure::debug_log as debug;

// Export main types
pub use util::errors::*;
pub use util::types::*;

// Export service layer components
pub use service::{
    config::{ConfigManager, ConfigService},
    workspace::{WorkspaceManager, WorkspaceProvider, WorkspaceService},
};

// Export infrastructure components
pub use infrastructure::{ai::AIClient, events::BackendEventManager};

// Export Agentic service core types
pub use agentic::{
    core::{DialogTurn, Message, ModelRound, Session},
    events::{AgenticEvent, EventQueue, EventRouter},
    execution::{ExecutionEngine, StreamProcessor},
    tools::{Tool, ToolPipeline},
};

// Export ToolRegistry separately
pub use agentic::tools::registry::ToolRegistry;

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CORE_NAME: &str = "BitFun Core";
