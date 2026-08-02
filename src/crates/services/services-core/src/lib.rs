//! Core service owner crate.
//!
//! This crate owns platform-agnostic service building blocks that can be
//! tested without compiling the full BitFun product runtime.

pub mod bounded_fs;
pub mod diagnostics;
pub mod diff;
pub mod dispatch_contract;
#[cfg(feature = "dispatch-workspace")]
pub mod dispatch_workspace;
mod file_lock;
pub mod filesystem;
pub mod json_store;
pub mod jsonc;
pub mod local_instructions;
#[cfg(feature = "workspace-runtime")]
pub mod local_runtime_ports;
#[cfg(feature = "lsp")]
pub mod lsp;
pub mod managed_runtime;
#[cfg(feature = "markdown")]
pub mod markdown;
#[cfg(feature = "permission")]
pub mod permission_store;
pub mod persistence;
pub mod process_manager;
pub mod process_tree;
#[cfg(feature = "runtime-ownership")]
pub mod runtime_ownership;
pub mod session;
pub mod session_usage;
pub mod storage_cleanup;
pub mod system;
pub mod token_usage;
#[cfg(feature = "workspace-runtime")]
pub mod workspace;
#[cfg(feature = "workspace-identity")]
pub mod workspace_identity;
pub mod workspace_instructions;
pub mod workspace_text;
