//! Agent runtime owner contracts.
//!
//! This crate owns runtime decisions that can be built and tested without
//! depending on `bitfun-core` concrete session or scheduler lifecycle.

pub mod agents;
pub mod checkpoint;
pub mod context_profile;
pub mod custom_agent;
pub mod custom_subagent;
pub mod deep_research;
pub mod deep_review;
pub mod dialog_turn;
pub mod event_bus;
pub mod event_queue;
pub mod event_router;
pub mod events;
pub mod evidence_ledger;
pub mod file_read_state;
pub mod post_call_hooks;
pub mod prompt;
pub mod prompt_cache;
pub mod prompt_markup;
pub mod remote_file_delivery;
pub mod runtime;
pub mod scheduled_job;
pub mod scheduler;
pub mod sdk;
pub mod session;
pub mod session_control;
pub mod session_state;
pub mod session_state_manager;
pub mod side_question;
pub mod skill_agent_snapshot;
pub mod skills;
pub mod thread_goal;
pub mod thread_goal_tools;
pub mod tool_confirmation;
pub mod turn_cancellation;
pub mod user_questions;
