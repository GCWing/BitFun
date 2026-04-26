//! Internationalization (i18n) service module
//!
//! Provides i18n support for backend text.

mod locale_registry;
mod model_copy;
mod service;
mod types;

pub use locale_registry::*;
pub use model_copy::*;
pub use service::*;
pub use types::*;
