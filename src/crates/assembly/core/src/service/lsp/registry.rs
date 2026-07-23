//! Compatibility re-exports for LSP plugin registry rules.
//!
//! The pure registry owner is `bitfun-services-core`; this legacy path remains
//! for downstream callers that import through `bitfun_core::service::lsp`.

pub use bitfun_services_core::lsp::{
    LspPluginRegistryError, LspSupportedExtensions, PluginRegistry,
};
