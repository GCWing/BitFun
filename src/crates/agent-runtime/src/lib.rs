//! Agent runtime owner contracts.
//!
//! This crate owns runtime decisions that can be built and tested without
//! depending on `bitfun-core` concrete session or scheduler lifecycle.

pub mod agents;
pub mod events;
pub mod prompt;
pub mod prompt_cache;
pub mod scheduler;
pub mod thread_goal;
