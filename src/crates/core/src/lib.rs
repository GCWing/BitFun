#![allow(non_snake_case)]
#![recursion_limit = "256"]
//! Compatibility facade and full product runtime assembly.
//!
//! New implementation code should live in owner crates under `src/crates/*`.
//! This crate re-exports legacy paths and wires the full BitFun product runtime.

pub mod agentic; // Agent system, tool system, and product runtime orchestration
pub mod function_agents; // Function-based agents
pub mod infrastructure; // AI clients, storage, logging, events
pub mod miniapp; // AI-generated instant apps (Zero-Dialect Runtime)
pub mod service; // Workspace, Config, FileSystem, Terminal, Git
pub mod util; // General types, errors, helper functions

// Re-export debug_log from infrastructure for backward compatibility.
pub use infrastructure::debug_log as debug;

// Export main types
pub use bitfun_runtime_ports as runtime_ports;
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
    core::{Message, Session},
    // NOTE: agentic::core::DialogTurn / ModelRound used to be re-exported here
    // but were dead code (never persisted, never read). On-disk shape lives in
    // service::session::{DialogTurnData, ModelRoundData}; lifecycle state is
    // tracked through SessionState + TurnStatus.
    events::{AgenticEvent, EventQueue, EventRouter},
    execution::{ExecutionEngine, StreamProcessor},
    tools::{Tool, ToolPipeline},
};

// Export ToolRegistry separately
pub use agentic::tools::registry::ToolRegistry;

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CORE_NAME: &str = "BitFun Core";
