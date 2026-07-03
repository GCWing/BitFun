pub mod agentic;
pub mod agentic_projection_manifest;
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
};
pub use agentic_projection_manifest::{
    agentic_event_projection_manifest_entry, is_legacy_websocket_agentic_event_type,
    public_agentic_event_projection_manifest, AgenticEventProjectionAggregate,
    AgenticEventProjectionManifestEntry, AgenticEventProjectionReplayPolicy,
    AgenticEventProjectionRetentionPolicy, AgenticEventProjectionUiShape,
    AGENTIC_EVENT_PROJECTION_MANIFEST,
};
pub use backend::{
    BackgroundCommandLifecycleInfo, ToolExecutionCompletedInfo, ToolExecutionErrorInfo,
    ToolExecutionProgressInfo, ToolExecutionStartedInfo, ToolTerminalReadyInfo,
};
pub use emitter::EventEmitter;
pub use frontend_projection::{project_agentic_frontend_event, AgenticFrontendEvent};
pub use types::*;
