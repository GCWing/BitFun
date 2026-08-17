//! DeepReview and long-running thread-goal contracts.
#![cfg(feature = "agent-runtime")]

#[path = "agent_long_horizon_contracts/deep_review_policy_contracts.rs"]
mod deep_review_policy_contracts;
#[path = "agent_long_horizon_contracts/thread_goal_contracts.rs"]
mod thread_goal_contracts;
#[path = "agent_long_horizon_contracts/thread_goal_tool_handler_contracts.rs"]
mod thread_goal_tool_handler_contracts;
