//! Unified event model
//!
//! Uses bitfun-events layer event definitions, extending core-specific functionality here

use crate::agentic::core::SessionState;
use bitfun_agent_runtime::session_state::session_state_label_for_state;

// ============ Re-export events layer types ============
pub use bitfun_events::agentic::ErrorCategory;
pub use bitfun_events::{
    AgenticEvent as BaseAgenticEvent, AgenticEventEnvelope as EventEnvelope,
    AgenticEventPriority as EventPriority, DeepReviewQueueReason, DeepReviewQueueState,
    DeepReviewQueueStatus, SubagentParentInfo, ToolEventData,
};

// ============ Core layer AgenticEvent extension ============

/// Core layer AgenticEvent type alias.
///
/// Currently an alias for `BaseAgenticEvent` (from `bitfun_events`). In earlier phases
/// this was intended to wrap `BaseAgenticEvent` with core-specific extensions (e.g.,
/// `SessionState`), but that enrichment now happens through re-exports rather than a
/// newtype. If core-specific fields are needed in the future, replace this alias with
/// a struct wrapping `BaseAgenticEvent`.
///
/// When sent to the transport layer, this is serialized as `BaseAgenticEvent`
/// (using `serde_json::Value`).
pub type AgenticEvent = BaseAgenticEvent;

// ============ Helper conversion functions ============

/// Convert SessionState to String (for transmission)
pub fn session_state_to_string(state: &SessionState) -> String {
    session_state_label_for_state(state).to_string()
}
