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
pub mod speech;
pub mod types;

pub use agentic::{
    AgenticEvent, AgenticEventEnvelope, AgenticEventPriority, DeepReviewQueueReason,
    DeepReviewQueueState, DeepReviewQueueStatus, ModelRoundAttemptDiagnostic,
    ModelRoundAttemptToolDiagnostic, SubagentParentInfo, ToolEventData, ToolEventIdentity,
};
pub use backend::{
    BackgroundCommandLifecycleInfo, ToolExecutionCompletedInfo, ToolExecutionErrorInfo,
    ToolExecutionProgressInfo, ToolExecutionStartedInfo, ToolTerminalReadyInfo,
};
pub use bitfun_core_types::ToolImageAttachment;
pub use emitter::EventEmitter;
pub use frontend_projection::{project_agentic_frontend_event, AgenticFrontendEvent};
pub use speech::{SPEECH_MODEL_PROGRESS_EVENT, SPEECH_MODEL_STATUS_CHANGED_EVENT};
pub use types::*;
