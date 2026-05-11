//! Agent tool contracts.
//!
//! Pure tool DTOs and helpers live here before the concrete tool framework and
//! tool packs are moved out of the core facade.

pub mod framework;
pub mod input_validator;

pub use framework::{ToolResult, ValidationResult};
pub use input_validator::InputValidator;
