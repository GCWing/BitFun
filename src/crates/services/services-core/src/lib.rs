//! Core service owner crate.
//!
//! This crate owns platform-agnostic service building blocks that can be
//! tested without compiling the full BitFun product runtime.

pub mod bounded_fs;
#[cfg(feature = "credential-vault")]
pub mod credential_vault;
#[cfg(feature = "diagnostics")]
pub mod diagnostics;
#[cfg(feature = "diff")]
pub mod diff;
pub mod dispatch_contract;
#[cfg(feature = "dispatch-workspace")]
pub mod dispatch_workspace;
#[cfg(any(feature = "local-storage", feature = "runtime-ownership"))]
mod file_lock;
#[cfg(feature = "filesystem")]
pub mod filesystem;
#[cfg(any(feature = "markdown", feature = "workspace-instructions"))]
pub mod instruction_scope;
#[cfg(any(feature = "json-io", feature = "local-storage"))]
pub mod json_store;
pub mod jsonc;
pub mod local_instructions;
#[cfg(feature = "workspace-runtime")]
pub mod local_runtime_ports;
#[cfg(feature = "process-runtime")]
pub mod managed_runtime;
#[cfg(feature = "markdown")]
pub mod markdown;
#[cfg(feature = "permission")]
pub mod permission_store;
#[cfg(feature = "local-storage")]
pub mod persistence;
#[cfg(feature = "process-runtime")]
pub mod process_manager;
#[cfg(feature = "process-runtime")]
pub mod process_tree;
#[cfg(feature = "runtime-ownership")]
pub mod runtime_ownership;
#[cfg(feature = "local-storage")]
pub mod session;
#[cfg(feature = "session-search")]
pub mod session_search;
#[cfg(feature = "local-storage")]
pub mod session_usage;
#[cfg(feature = "local-storage")]
pub mod storage_cleanup;
#[cfg(feature = "process-runtime")]
pub mod system;
#[cfg(feature = "tls-provider")]
pub mod tls_provider;
#[cfg(feature = "local-storage")]
pub mod token_usage;
#[cfg(feature = "workspace-runtime")]
pub mod workspace;
#[cfg(feature = "workspace-identity")]
pub mod workspace_identity;
#[cfg(feature = "workspace-instructions")]
pub mod workspace_instructions;
pub mod workspace_text;
