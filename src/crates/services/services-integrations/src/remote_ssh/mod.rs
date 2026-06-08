//! Remote SSH service contracts.
//!
//! `bitfun-core::service::remote_ssh` remains as the compatibility facade for
//! the legacy public path.

pub mod paths;
pub mod types;
pub mod workspace_registry;
#[cfg(feature = "workspace-search")]
pub mod workspace_search;

pub use paths::*;
pub use types::*;
pub use workspace_registry::*;
