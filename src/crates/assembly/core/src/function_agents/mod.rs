/*!
 * Function Agents module
 *
 * Provides various function agents for automating specific tasks
 */

pub use openbitfun_product_domains::function_agents::common;

#[path = "git-func-agent/mod.rs"]
pub mod git_func_agent;

pub mod port_adapters;

// Re-export shared types from common module
pub use common::{AgentError, AgentErrorType, AgentResult, Language};

// Re-export agents and specific types
pub use git_func_agent::{
    CommitFormat, CommitMessage, CommitMessageOptions, CommitType, GitFunctionAgent,
};
