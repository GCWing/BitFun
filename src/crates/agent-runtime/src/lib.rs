//! Agent runtime owner contracts.
//!
//! This crate owns runtime decisions that can be built and tested without
//! depending on `bitfun-core` concrete session or scheduler lifecycle.

pub mod agents;
pub mod custom_subagent;
pub mod events;
pub mod post_call_hooks;
pub mod prompt;
pub mod prompt_cache;
pub mod scheduled_job;
pub mod scheduler;
pub mod thread_goal;
pub mod tool_confirmation;
