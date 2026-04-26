//! Agentic Module
//!
//! Core AI Agent service system

// Core module
pub mod core;
pub mod events;
pub mod persistence;

// Session management module
pub mod session;

// Execution engine module
pub mod execution;

// Tools module
pub mod tools;

// Coordination module
pub mod coordination;
pub mod deep_review_policy;

// Shared-context fork execution module
pub mod fork;

/// Round-boundary yield when user queues a message during an active turn
pub mod round_preempt;

// Image analysis module
pub mod image_analysis;

// Ephemeral side-question module (used by desktop /btw overlay)
pub mod side_question;

// Agents module
pub mod agents;
pub mod workspace;

mod util;

// Insights module
pub mod insights;

pub use agents::*;
pub use coordination::*;
pub use core::*;
pub use events::{queue, router, types as event_types};
pub use execution::*;
pub use fork::*;
pub use image_analysis::{ImageAnalyzer, MessageEnhancer};
pub use persistence::PersistenceManager;
pub use round_preempt::{
    DialogRoundPreemptSource, NoopDialogRoundPreemptSource, SessionRoundYieldFlags,
};
pub use session::*;
pub use side_question::*;
pub use workspace::{WorkspaceBackend, WorkspaceBinding};
