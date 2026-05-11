//! Integration service owner crate.
//!
//! Heavy external integrations live here behind feature groups so local checks
//! can opt into only the integration family they need.

#[cfg(feature = "file-watch")]
pub mod file_watch;
