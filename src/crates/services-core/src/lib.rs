//! Core service owner crate.
//!
//! This crate owns platform-agnostic service building blocks that can be
//! tested without compiling the full BitFun product runtime.

pub mod diagnostics;
pub mod diff;
pub mod process_manager;
pub mod session;
pub mod session_usage;
pub mod system;
pub mod token_usage;
