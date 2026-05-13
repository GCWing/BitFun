//! Agent tool contracts.
//!
//! Pure tool DTOs and helpers live here before the concrete tool framework and
//! tool packs are moved out of the core facade.

pub mod framework;
pub mod input_validator;

pub use bitfun_core_types::ToolImageAttachment;
pub use bitfun_runtime_ports::{
    DynamicToolDescriptor, DynamicToolProvider, PortError, PortErrorKind, PortResult, ToolDecorator,
};
pub use framework::{
    DynamicMcpToolInfo, DynamicToolInfo, ToolPathBackend, ToolPathOperation, ToolPathPolicy,
    ToolPathResolution, ToolRenderOptions, ToolRestrictionError, ToolResult,
    ToolRuntimeRestrictions, ValidationResult,
};
pub use input_validator::InputValidator;
