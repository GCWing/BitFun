//! Shared AI-facing protocol types.

pub mod ai;
pub mod config;
pub mod message;
pub mod tool;
pub mod tool_image_attachment;

pub use ai::*;
pub use config::*;
pub use message::*;
pub use tool::*;
pub use tool_image_attachment::ToolImageAttachment;
