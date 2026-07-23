//! Compatibility re-exports for LSP protocol encoding and decoding.
//!
//! The reusable protocol helpers live in `bitfun-services-core`.

pub use bitfun_services_core::lsp::protocol::{
    create_notification, create_request, extract_result, read_message, write_message,
};
