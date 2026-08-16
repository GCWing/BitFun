//! Domain-grouped JSON-RPC request handlers.

pub(in crate::server) mod account;
pub(in crate::server) mod agent;
pub(in crate::server) mod app;
mod capability;
pub(in crate::server) mod config;
pub(in crate::server) mod external_source;
pub(in crate::server) mod git;
pub(in crate::server) mod hook;
pub(in crate::server) mod i18n;
pub(in crate::server) mod mcp;
pub(in crate::server) mod model;
pub(in crate::server) mod permission;
pub(in crate::server) mod search;
pub(in crate::server) mod session;
pub(in crate::server) mod skill;
pub(in crate::server) mod subagent;
pub(in crate::server) mod workspace;
pub(in crate::server) mod worktree;
