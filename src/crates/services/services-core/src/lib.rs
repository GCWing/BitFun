//! Core service owner crate.
//!
//! This crate owns platform-agnostic service building blocks that can be
//! tested without compiling the full BitFun product runtime.

pub mod diagnostics;
pub mod diff;
pub mod filesystem;
pub mod json_store;
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
pub mod session;
pub mod session_usage;
pub mod storage_cleanup;
pub mod system;
pub mod token_usage;
#[cfg(feature = "workspace-runtime")]
pub mod workspace;
pub mod workspace_instructions;
