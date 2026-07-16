pub mod agentic;
/// Events Layer
///
/// Independent event definition layer, providing:
/// - EventEmitter trait (event sending interface)
/// - Various event type definitions
/// - Event abstraction independent of platforms
pub mod backend;
pub mod emitter;
pub mod frontend_projection;
pub mod types;

pub use agentic::{
    AgenticEvent, AgenticEventEnvelope, AgenticEventPriority, DeepReviewQueueReason,
    DeepReviewQueueState, DeepReviewQueueStatus, SubagentParentInfo, ToolEventData,
    ToolEventIdentity,
};
pub use backend::{
    BackgroundCommandLifecycleInfo, ToolExecutionCompletedInfo, ToolExecutionErrorInfo,
    ToolExecutionProgressInfo, ToolExecutionStartedInfo, ToolTerminalReadyInfo,
};
pub use emitter::EventEmitter;
pub use frontend_projection::{project_agentic_frontend_event, AgenticFrontendEvent};
pub use types::*;
