//! Coordination layer
//!
//! Top-level component that integrates all subsystems

pub mod coordinator;
pub mod state_manager;

pub use coordinator::*;
pub use state_manager::*;

pub use coordinator::get_global_coordinator;
