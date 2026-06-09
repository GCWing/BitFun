#![allow(non_snake_case)]
#![recursion_limit = "256"]
//! Compatibility facade and full product runtime assembly.
//!
//! New implementation code should live in owner crates under `src/crates/*`.
//! This crate re-exports legacy paths and wires the full BitFun product runtime.

#[cfg(feature = "product-full")]
pub mod agentic; // Agent system, tool system, and product runtime orchestration
#[cfg(feature = "product-domains")]
pub mod function_agents; // Function-based agents
pub mod infrastructure; // AI clients, storage, logging, events
#[cfg(feature = "product-domains")]
pub mod miniapp; // AI-generated instant apps (Zero-Dialect Runtime)
#[cfg(feature = "product-full")]
pub mod product_assembly;
#[cfg(feature = "product-domains")]
pub(crate) mod product_domain_runtime;
#[cfg(feature = "product-full")]
pub mod product_runtime;
pub mod service; // Workspace, Config, FileSystem, Terminal, Git
#[cfg(feature = "service-integrations")]
pub(crate) mod service_agent_runtime;
pub mod util; // General types, errors, helper functions

// Re-export debug_log from infrastructure for backward compatibility.
#[cfg(feature = "product-full")]
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
#[cfg(feature = "ai-adapter-runtime")]
pub use infrastructure::ai::AIClient;
pub use infrastructure::events::BackendEventManager;

// Export Agentic service core types
#[cfg(feature = "product-full")]
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

// Export ToolRegistry separately.
#[cfg(feature = "product-full")]
pub use agentic::tools::registry::ToolRegistry;

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CORE_NAME: &str = "BitFun Core";
