//! Diff service module
//!
//! Provides unified diff calculation, merge, and status management.

pub mod service;
pub mod types;

pub use service::DiffService;
pub use types::*;
