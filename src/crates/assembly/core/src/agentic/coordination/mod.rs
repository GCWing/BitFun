//! Coordination layer
//!
//! Top-level component that integrates all subsystems

mod background_outcomes;
mod coordination_store;
pub mod coordinator;
pub mod scheduler;
pub mod state_manager;
// Phase 1 of persistent subagents: types are not yet wired into production
// call paths (Phase 2 registers the registry in the Coordinator). The expect
// fires a reminder once Phase 2 starts using the module.
#[expect(dead_code, reason = "Phase 2 wires the registry into the Coordinator")]
mod subagent_instance;
pub mod turn_outcome;
mod turn_settlement;

pub use coordinator::*;
pub use scheduler::*;
pub use state_manager::*;
pub use turn_outcome::*;

pub(crate) use background_outcomes::{
    BackgroundSubagentOutcome, BackgroundSubagentOutcomeStore, BackgroundSubagentWaitMode,
    BackgroundSubagentWaitResult,
};

// Re-exported for Phase 2 wiring into the Coordinator; currently unused.
#[expect(unused_imports)]
pub(crate) use subagent_instance::{
    SubagentInstance, SubagentInstanceRegistry, SubagentInstanceStatus,
};

pub use coordinator::get_global_coordinator;
pub use scheduler::get_global_scheduler;
