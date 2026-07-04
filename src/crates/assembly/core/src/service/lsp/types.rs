//! Compatibility re-exports for LSP protocol and plugin manifest DTOs.
//!
//! The shared contract owner is `bitfun-core-types`; this legacy path remains
//! for downstream callers that import through `bitfun_core::service::lsp`.

pub use bitfun_core_types::lsp::*;
