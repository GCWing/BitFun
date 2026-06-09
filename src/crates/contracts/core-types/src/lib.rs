//! Shared low-level product DTOs.
//!
//! This crate must stay lightweight: do not add runtime, network, platform, or
//! product assembly dependencies here.

pub mod ai;
pub mod errors;
pub mod session;
pub mod surface;
pub mod tool_image_attachment;

pub use ai::{
    AIConfig, ConnectionTestMessageCode, ConnectionTestResult, Message, ProxyConfig, ReasoningMode,
    RemoteModelInfo, ToolCall, ToolCallConfirmationDetails, ToolCallRequestInfo,
    ToolCallResponseInfo, ToolDefinition,
};
pub use errors::{AiErrorDetail, ErrorCategory};
pub use session::SessionKind;
pub use surface::{
    ApprovalSource, CapabilityRequest, CapabilityRequestKind, PermissionDecision, PermissionScope,
    RuntimeArtifactKind, RuntimeArtifactRef, SurfaceKind, ThreadEnvironment, ThreadEnvironmentKind,
};
pub use tool_image_attachment::ToolImageAttachment;
