//! PTY module - Process management and data handling
//!
//! This module provides the core PTY functionality including:
//! - Process spawning and lifecycle management
//! - Data buffering for performance optimization
//! - Flow control to prevent data overflow
//! - PTY service for managing multiple processes
//!
//! ## Architecture
//!
//! The PTY system uses a component-based design
//!
//! - **PtyWriter**: For writing data to the PTY (can be cloned and shared)
//! - **PtyEventStream**: For receiving events (move to a dedicated task)
//! - **PtyController**: For control operations (resize, signal, shutdown)
//! - **FlowControl**: For backpressure management
//!
//! This design eliminates locks during normal operation, preventing deadlocks
//! and improving performance.

mod data_bufferer;
mod process;
mod service;

pub use data_bufferer::DataBufferer;
pub use process::{
    spawn_pty, FlowControl, PtyCommand, PtyController, PtyEvent, PtyEventStream, PtyInfo,
    PtyWriter, SpawnResult,
};
pub use service::{ProcessInfo, ProcessProperty, PtyService, PtyServiceEvent};

/// Flow control constants
pub mod flow_control {
    /// High water mark - pause when unacknowledged chars exceed this
    pub const HIGH_WATER_MARK: usize = 100_000;

    /// Low water mark - resume when unacknowledged chars fall below this
    pub const LOW_WATER_MARK: usize = 5_000;
}
