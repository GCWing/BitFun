//! Compatibility re-exports for LSP server process lifecycle.
//!
//! The reusable LSP process owner lives in `bitfun-services-core`.

pub use bitfun_services_core::lsp::process::{
    CrashCallback, DiagnosticsCallback, LspServerProcess, ProgressCallback, TokenCreateCallback,
};
