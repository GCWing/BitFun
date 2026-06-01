//! Agentic facade and product runtime assembly.
//!
//! Portable contracts move to owner crates first; concrete orchestration stays
//! here until it can be split without changing tool, session, or review flows.

// Core module
pub mod core;
pub mod events;
pub mod persistence;
pub mod skill_agent_snapshot;

// Session management module
pub mod session;

// Execution engine module
pub mod execution;

// Tools module
pub mod tools;

// Coordination module
pub mod context_profile;
pub mod coordination;
pub mod deep_review;
pub mod deep_review_policy;
pub mod harness;
pub(crate) mod subagent_runtime;

// Shared-context fork-agent execution module
pub mod fork_agent;

/// Round-boundary yield when user queues a message during an active turn
pub mod round_preempt;

// Image analysis module
pub mod image_analysis;

// Ephemeral side-question module (used by desktop /btw overlay)
pub mod side_question;

// Session goal mode (/goal command)
pub mod goal_mode;
pub(crate) mod init_agents_md;
pub mod system;

// Agents module
pub mod agents;
pub mod workspace;

mod util;

// Insights module
pub mod insights;

pub use agents::*;
pub use context_profile::*;
pub use coordination::*;
pub use core::*;
pub use events::{queue, router, types as event_types};
pub use execution::*;
pub use fork_agent::*;
pub use goal_mode::*;
pub use image_analysis::{ImageAnalyzer, MessageEnhancer};
pub use persistence::PersistenceManager;
pub use round_preempt::{
    DialogRoundInjectionInterrupt, DialogRoundInjectionSource, DialogRoundPreemptSource,
    NoopDialogRoundInjectionSource, NoopDialogRoundPreemptSource, RoundInjection,
    RoundInjectionKind, RoundInjectionTarget, SessionRoundInjectionBuffer, SessionRoundYieldFlags,
};
pub use session::*;
pub use side_question::*;
pub use skill_agent_snapshot::*;
pub use system::{init_agentic_system, AgenticSystem};
pub use workspace::{WorkspaceBackend, WorkspaceBinding};
