//! Coordination layer
//!
//! Top-level component that integrates all subsystems

pub mod coordinator;
pub mod scheduler;
pub mod state_manager;

pub use coordinator::*;
pub use scheduler::*;
pub use state_manager::*;

pub use coordinator::get_global_coordinator;
pub use scheduler::get_global_scheduler;
