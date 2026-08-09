//! Compatibility re-exports for stable workspace identity helpers.
//!
//! The implementation is owned by `bitfun-services-core`; remote SSH keeps
//! this module so existing integration and facade paths remain source-compatible.

pub use bitfun_services_core::workspace_identity::*;
