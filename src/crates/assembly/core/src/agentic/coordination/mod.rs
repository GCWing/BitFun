//! Coordination layer
//!
//! Top-level component that integrates all subsystems

mod background_outcomes;
mod coordination_store;
pub mod coordinator;
pub mod scheduler;
pub mod state_manager;
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

pub use coordinator::get_global_coordinator;
pub use scheduler::get_global_scheduler;
