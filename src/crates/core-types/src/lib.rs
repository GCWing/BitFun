//! Shared low-level product DTOs.
//!
//! This crate must stay lightweight: do not add runtime, network, platform, or
//! product assembly dependencies here.

pub mod errors;

pub use errors::{AiErrorDetail, ErrorCategory};
