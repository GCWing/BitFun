//! Shared low-level product DTOs.
//!
//! This crate must stay lightweight: do not add runtime, network, platform, or
//! product assembly dependencies here.

pub mod errors;
pub mod session;
pub mod tool_image_attachment;

pub use errors::{AiErrorDetail, ErrorCategory};
pub use session::SessionKind;
pub use tool_image_attachment::ToolImageAttachment;
