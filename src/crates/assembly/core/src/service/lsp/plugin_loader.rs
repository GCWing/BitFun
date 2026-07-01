//! Compatibility re-exports for LSP plugin package loading.
//!
//! The reusable package loader lives in `bitfun-services-core`; this legacy path
//! remains for downstream callers that import through `bitfun_core::service::lsp`.

pub use bitfun_services_core::lsp::plugin_loader::PluginLoader;
