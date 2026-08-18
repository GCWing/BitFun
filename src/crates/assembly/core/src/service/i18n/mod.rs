//! Internationalization (i18n) service module
//!
//! Provides i18n support for backend text.

pub mod generated_locale_contract;
#[cfg(feature = "i18n-runtime")]
mod locale_registry;
mod model_copy;
#[cfg(feature = "i18n-runtime")]
mod service;
mod types;

#[cfg(feature = "i18n-runtime")]
pub use locale_registry::*;
pub use model_copy::*;
#[cfg(feature = "i18n-runtime")]
pub use service::*;
pub use types::*;
