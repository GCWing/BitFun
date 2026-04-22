/*!
 * Function Agents module
 *
 * Provides various function agents for automating specific tasks
 */

pub mod common;

#[path = "git-func-agent/mod.rs"]
pub mod git_func_agent;

#[path = "startchat-func-agent/mod.rs"]
pub mod startchat_func_agent;

// Re-export shared types from common module
pub use common::{AgentError, AgentErrorType, AgentResult, Language};

// Re-export agents and specific types
pub use git_func_agent::{
    CommitFormat, CommitMessage, CommitMessageOptions, CommitType, GitFunctionAgent,
};
pub use startchat_func_agent::{
    CurrentWorkState, GitWorkState, GreetingMessage, PredictedAction, QuickAction,
    StartchatFunctionAgent, WorkStateAnalysis, WorkStateOptions,
};
