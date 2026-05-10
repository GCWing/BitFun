/// Events Layer
///
/// Independent event definition layer, providing:
/// - EventEmitter trait (event sending interface)
/// - Various event type definitions
/// - Event abstraction independent of platforms
pub mod agentic;
pub mod emitter;
pub mod types;

pub use agentic::{
    AgenticEvent, AgenticEventEnvelope, AgenticEventPriority, DeepReviewQueueReason,
    DeepReviewQueueState, DeepReviewQueueStatus, SubagentParentInfo, ToolEventData,
};
pub use emitter::EventEmitter;
pub use types::*;
