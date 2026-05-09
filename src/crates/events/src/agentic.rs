//! Agentic Events Definition
pub use bitfun_core_types::errors::{AiErrorDetail, ErrorCategory};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AgenticEventPriority {
    Critical = 0, // Immediately send (error, cancellation)
    High = 1,
    Normal = 2,
    Low = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentParentInfo {
    #[serde(rename = "toolCallId")]
    pub tool_call_id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "dialogTurnId")]
    pub dialog_turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeepReviewQueueStatus {
    QueuedForCapacity,
    PausedByUser,
    Running,
    CapacitySkipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeepReviewQueueReason {
    ProviderRateLimit,
    ProviderConcurrencyLimit,
    RetryAfter,
    LocalConcurrencyCap,
    TemporaryOverload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeepReviewQueueState {
    pub tool_id: String,
    pub subagent_type: String,
    pub status: DeepReviewQueueStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<DeepReviewQueueReason>,
    pub queued_reviewer_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_reviewer_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_parallel_instances: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional_reviewer_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_queue_wait_seconds: Option<u64>,
    #[serde(default)]
    pub session_concurrency_high: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgenticEvent {
    SessionCreated {
        session_id: String,
        session_name: String,
        agent_type: String,
        /// Workspace path this session belongs to. None for locally-created sessions.
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_path: Option<String>,
    },

    SessionStateChanged {
        session_id: String,
        new_state: String,
    },

    SessionDeleted {
        session_id: String,
    },

    SessionTitleGenerated {
        session_id: String,
        title: String,
        method: String,
    },
    ImageAnalysisStarted {
        session_id: String,
        image_count: usize,
        user_input: String,
        /// Image metadata JSON for UI rendering (same as DialogTurnStarted)
        image_metadata: Option<serde_json::Value>,
    },

    ImageAnalysisCompleted {
        session_id: String,
        success: bool,
        duration_ms: u64,
    },

    DialogTurnStarted {
        session_id: String,
        turn_id: String,
        turn_index: usize,
        user_input: String,
        /// Original user input before vision enhancement (for display on all clients)
        original_user_input: Option<String>,
        /// Image metadata JSON for UI rendering (id, name, data_url, mime_type, image_path)
        user_message_metadata: Option<serde_json::Value>,
        subagent_parent_info: Option<SubagentParentInfo>,
    },

    DialogTurnCompleted {
        session_id: String,
        turn_id: String,
        total_rounds: usize,
        total_tools: usize,
        duration_ms: u64,
        subagent_parent_info: Option<SubagentParentInfo>,
        /// When set, the turn finished but the last model round was a partial
        /// recovery (stream aborted mid-way). Contains a human-readable reason.
        #[serde(skip_serializing_if = "Option::is_none")]
        partial_recovery_reason: Option<String>,
        /// Whether the turn completed successfully.
        #[serde(skip_serializing_if = "Option::is_none")]
        success: Option<bool>,
        /// Why the turn finished.
        #[serde(skip_serializing_if = "Option::is_none")]
        finish_reason: Option<String>,
    },

    DialogTurnCancelled {
        session_id: String,
        turn_id: String,
        subagent_parent_info: Option<SubagentParentInfo>,
    },

    DialogTurnFailed {
        session_id: String,
        turn_id: String,
        error: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_category: Option<ErrorCategory>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_detail: Option<AiErrorDetail>,
        subagent_parent_info: Option<SubagentParentInfo>,
    },

    TokenUsageUpdated {
        session_id: String,
        turn_id: String,
        model_id: String,
        input_tokens: usize,
        output_tokens: Option<usize>,
        total_tokens: usize,
        max_context_tokens: Option<usize>,
        is_subagent: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cached_tokens: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token_details: Option<serde_json::Value>,
    },

    ContextCompressionStarted {
        session_id: String,
        turn_id: String,
        compression_id: String,
        trigger: String,
        tokens_before: usize,
        context_window: usize,
        threshold: f32,
        subagent_parent_info: Option<SubagentParentInfo>,
    },

    ContextCompressionCompleted {
        session_id: String,
        turn_id: String,
        compression_id: String,
        compression_count: usize,
        tokens_before: usize,
        tokens_after: usize,
        compression_ratio: f64,
        duration_ms: u64,
        has_summary: bool,
        summary_source: String,
        subagent_parent_info: Option<SubagentParentInfo>,
    },

    ContextCompressionFailed {
        session_id: String,
        turn_id: String,
        compression_id: String,
        error: String,
        subagent_parent_info: Option<SubagentParentInfo>,
    },

    ModelRoundStarted {
        session_id: String,
        turn_id: String,
        round_id: String,
        round_index: usize,
        subagent_parent_info: Option<SubagentParentInfo>,
    },

    ModelRoundCompleted {
        session_id: String,
        turn_id: String,
        round_id: String,
        has_tool_calls: bool,
        subagent_parent_info: Option<SubagentParentInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model_alias: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        first_chunk_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        first_visible_output_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stream_duration_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attempt_count: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        failure_category: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token_details: Option<serde_json::Value>,
    },

    TextChunk {
        session_id: String,
        turn_id: String,
        round_id: String,
        text: String,
        subagent_parent_info: Option<SubagentParentInfo>,
    },

    ThinkingChunk {
        session_id: String,
        turn_id: String,
        round_id: String,
        content: String,
        #[serde(default)]
        is_end: bool,
        subagent_parent_info: Option<SubagentParentInfo>,
    },

    ToolEvent {
        session_id: String,
        turn_id: String,
        tool_event: ToolEventData,
        subagent_parent_info: Option<SubagentParentInfo>,
    },

    DeepReviewQueueStateChanged {
        session_id: String,
        turn_id: String,
        queue_state: DeepReviewQueueState,
        subagent_parent_info: Option<SubagentParentInfo>,
    },

    SystemError {
        session_id: Option<String>,
        error: String,
        recoverable: bool,
    },

    /// User "steering" message injected into a running dialog turn at a model
    /// round boundary (Codex-style mid-turn injection). The frontend renders
    /// this as a synthetic record inside the current turn so the user can see
    /// the message they just steered with.
    UserSteeringInjected {
        session_id: String,
        turn_id: String,
        round_index: usize,
        steering_id: String,
        content: String,
        display_content: String,
        subagent_parent_info: Option<SubagentParentInfo>,
    },

    /// A session's bound model has been automatically migrated because the
    /// previously bound model became unavailable (disabled or deleted).
    /// The frontend should refresh its model selector for the session and
    /// surface a non-blocking notice so the user knows what happened.
    SessionModelAutoMigrated {
        session_id: String,
        /// The model id the session was using before the migration.
        previous_model_id: String,
        /// The model id (or selector such as `"auto"`) the session is now bound
        /// to. This is what `SessionConfig.model_id` was rewritten to.
        new_model_id: String,
        /// Why the migration happened, e.g. `"model_disabled"` or
        /// `"model_deleted"`.
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum ToolEventData {
    EarlyDetected {
        tool_id: String,
        tool_name: String,
    },
    ParamsPartial {
        tool_id: String,
        tool_name: String,
        params: String,
    },
    Queued {
        tool_id: String,
        tool_name: String,
        position: usize,
    },
    Waiting {
        tool_id: String,
        tool_name: String,
        dependencies: Vec<String>,
    },
    Started {
        tool_id: String,
        tool_name: String,
        params: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_seconds: Option<u64>,
    },
    Progress {
        tool_id: String,
        tool_name: String,
        message: String,
        percentage: f32,
    },
    Streaming {
        tool_id: String,
        tool_name: String,
        chunks_received: usize,
    },
    StreamChunk {
        tool_id: String,
        tool_name: String,
        data: serde_json::Value,
    },
    ConfirmationNeeded {
        tool_id: String,
        tool_name: String,
        params: serde_json::Value,
    },
    Confirmed {
        tool_id: String,
        tool_name: String,
    },
    Rejected {
        tool_id: String,
        tool_name: String,
    },
    Completed {
        tool_id: String,
        tool_name: String,
        result: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        result_for_assistant: Option<String>,
        duration_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        queue_wait_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        preflight_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        confirmation_wait_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        execution_ms: Option<u64>,
    },
    Failed {
        tool_id: String,
        tool_name: String,
        error: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        queue_wait_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        preflight_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        confirmation_wait_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        execution_ms: Option<u64>,
    },
    Cancelled {
        tool_id: String,
        tool_name: String,
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        queue_wait_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        preflight_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        confirmation_wait_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        execution_ms: Option<u64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticEventEnvelope {
    pub id: String,
    pub event: AgenticEvent,
    pub priority: AgenticEventPriority,
    pub timestamp: SystemTime,
}

impl PartialEq for AgenticEventEnvelope {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn model_round_completed_serializes_optional_timing_fields() {
        let event = AgenticEvent::ModelRoundCompleted {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            round_id: "round-1".to_string(),
            has_tool_calls: false,
            subagent_parent_info: None,
            duration_ms: Some(123),
            provider_id: Some("provider".to_string()),
            model_id: Some("model".to_string()),
            model_alias: Some("alias".to_string()),
            first_chunk_ms: Some(10),
            first_visible_output_ms: Some(12),
            stream_duration_ms: Some(100),
            attempt_count: Some(1),
            failure_category: None,
            token_details: Some(serde_json::json!({ "reasoningTokens": 7 })),
        };

        let json = serde_json::to_value(&event).expect("serialize event");

        assert_eq!(json["duration_ms"], 123);
        assert_eq!(json["first_chunk_ms"], 10);
        assert_eq!(json["token_details"]["reasoningTokens"], 7);
    }

    #[test]
    fn model_round_completed_deserializes_legacy_payload_without_timing_fields() {
        let json = serde_json::json!({
            "type": "ModelRoundCompleted",
            "session_id": "session-1",
            "turn_id": "turn-1",
            "round_id": "round-1",
            "has_tool_calls": false,
            "subagent_parent_info": null
        });

        let event: AgenticEvent = serde_json::from_value(json).expect("legacy event");

        match event {
            AgenticEvent::ModelRoundCompleted { duration_ms, .. } => {
                assert_eq!(duration_ms, None);
            }
            _ => panic!("unexpected event"),
        }
    }

    #[test]
    fn token_usage_updated_serializes_optional_cache_and_detail_fields() {
        let event = AgenticEvent::TokenUsageUpdated {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            model_id: "model".to_string(),
            input_tokens: 10,
            output_tokens: Some(5),
            total_tokens: 15,
            max_context_tokens: Some(100),
            is_subagent: false,
            cached_tokens: Some(3),
            token_details: Some(serde_json::json!({ "cachedSource": "provider" })),
        };

        let json = serde_json::to_value(&event).expect("serialize event");

        assert_eq!(json["cached_tokens"], 3);
        assert_eq!(json["token_details"]["cachedSource"], "provider");
    }

    #[test]
    fn completed_tool_reports_total_and_execution_duration() {
        let event = ToolEventData::Completed {
            tool_id: "tool-1".to_string(),
            tool_name: "write_file".to_string(),
            result: serde_json::json!({ "ok": true }),
            result_for_assistant: None,
            duration_ms: 120,
            queue_wait_ms: Some(10),
            preflight_ms: Some(20),
            confirmation_wait_ms: Some(0),
            execution_ms: Some(90),
        };

        let json = serde_json::to_value(&event).expect("serialize tool event");

        assert_eq!(json["duration_ms"], 120);
        assert_eq!(json["execution_ms"], 90);
    }

    #[test]
    fn failed_tool_reports_best_effort_total_duration() {
        let event = ToolEventData::Failed {
            tool_id: "tool-1".to_string(),
            tool_name: "write_file".to_string(),
            error: "failed".to_string(),
            duration_ms: Some(120),
            queue_wait_ms: Some(10),
            preflight_ms: Some(20),
            confirmation_wait_ms: None,
            execution_ms: Some(90),
        };

        let json = serde_json::to_value(&event).expect("serialize tool event");

        assert_eq!(json["duration_ms"], 120);
        assert_eq!(json["execution_ms"], 90);
    }

    #[test]
    fn cancelled_tool_reports_best_effort_total_duration() {
        let event = ToolEventData::Cancelled {
            tool_id: "tool-1".to_string(),
            tool_name: "write_file".to_string(),
            reason: "cancelled".to_string(),
            duration_ms: Some(120),
            queue_wait_ms: Some(10),
            preflight_ms: Some(20),
            confirmation_wait_ms: None,
            execution_ms: Some(90),
        };

        let json = serde_json::to_value(&event).expect("serialize tool event");

        assert_eq!(json["duration_ms"], 120);
        assert_eq!(json["execution_ms"], 90);
    }

    #[test]
    fn deep_review_queue_state_event_serializes_stable_contract() {
        let event = AgenticEvent::DeepReviewQueueStateChanged {
            session_id: "review-session".to_string(),
            turn_id: "turn-1".to_string(),
            queue_state: DeepReviewQueueState {
                tool_id: "task-1".to_string(),
                subagent_type: "ReviewSecurity".to_string(),
                status: DeepReviewQueueStatus::QueuedForCapacity,
                reason: Some(DeepReviewQueueReason::ProviderConcurrencyLimit),
                queued_reviewer_count: 2,
                active_reviewer_count: Some(1),
                effective_parallel_instances: Some(2),
                optional_reviewer_count: Some(1),
                queue_elapsed_ms: Some(1200),
                run_elapsed_ms: None,
                max_queue_wait_seconds: Some(60),
                session_concurrency_high: true,
            },
            subagent_parent_info: None,
        };

        assert_eq!(event.session_id(), Some("review-session"));
        assert_eq!(event.default_priority(), AgenticEventPriority::High);

        let serialized = serde_json::to_value(event).expect("serialize event");
        assert_eq!(serialized["type"], "DeepReviewQueueStateChanged");
        assert_eq!(serialized["queue_state"]["status"], "queued_for_capacity");
        assert_eq!(
            serialized["queue_state"]["reason"],
            json!("provider_concurrency_limit")
        );
        assert_eq!(serialized["queue_state"]["queue_elapsed_ms"], json!(1200));
        assert_eq!(
            serialized["queue_state"]["effective_parallel_instances"],
            json!(2)
        );
        assert_eq!(
            serialized["queue_state"]["run_elapsed_ms"],
            serde_json::Value::Null
        );
    }
}

impl Eq for AgenticEventEnvelope {}

impl PartialOrd for AgenticEventEnvelope {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AgenticEventEnvelope {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.priority.cmp(&other.priority) {
            std::cmp::Ordering::Equal => self.timestamp.cmp(&other.timestamp),
            other => other,
        }
    }
}

impl AgenticEventEnvelope {
    pub fn new(event: AgenticEvent, priority: AgenticEventPriority) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            event,
            priority,
            timestamp: SystemTime::now(),
        }
    }
}

impl AgenticEvent {
    /// Get the session ID of the event
    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::SessionCreated { session_id, .. }
            | Self::SessionStateChanged { session_id, .. }
            | Self::SessionDeleted { session_id }
            | Self::SessionTitleGenerated { session_id, .. }
            | Self::ImageAnalysisStarted { session_id, .. }
            | Self::ImageAnalysisCompleted { session_id, .. }
            | Self::DialogTurnStarted { session_id, .. }
            | Self::DialogTurnCompleted { session_id, .. }
            | Self::TokenUsageUpdated { session_id, .. }
            | Self::ContextCompressionStarted { session_id, .. }
            | Self::ContextCompressionCompleted { session_id, .. }
            | Self::ContextCompressionFailed { session_id, .. }
            | Self::DialogTurnCancelled { session_id, .. }
            | Self::DialogTurnFailed { session_id, .. }
            | Self::ModelRoundStarted { session_id, .. }
            | Self::TextChunk { session_id, .. }
            | Self::ThinkingChunk { session_id, .. }
            | Self::ModelRoundCompleted { session_id, .. }
            | Self::ToolEvent { session_id, .. }
            | Self::UserSteeringInjected { session_id, .. }
            | Self::DeepReviewQueueStateChanged { session_id, .. }
            | Self::SessionModelAutoMigrated { session_id, .. } => Some(session_id),
            Self::SystemError { session_id, .. } => session_id.as_deref(),
        }
    }

    /// Get the default priority
    pub fn default_priority(&self) -> AgenticEventPriority {
        match self {
            Self::SystemError { .. }
            | Self::DialogTurnFailed { .. }
            | Self::DialogTurnCancelled { .. } => AgenticEventPriority::Critical,

            Self::SessionStateChanged { .. }
            | Self::SessionTitleGenerated { .. }
            | Self::SessionModelAutoMigrated { .. }
            | Self::DeepReviewQueueStateChanged { .. }
            | Self::ContextCompressionFailed { .. } => AgenticEventPriority::High,

            Self::ImageAnalysisStarted { .. }
            | Self::ImageAnalysisCompleted { .. }
            | Self::TextChunk { .. }
            | Self::ThinkingChunk { .. }
            | Self::ModelRoundStarted { .. }
            | Self::ModelRoundCompleted { .. }
            | Self::TokenUsageUpdated { .. }
            | Self::DialogTurnCompleted { .. }
            | Self::ContextCompressionStarted { .. }
            | Self::UserSteeringInjected { .. }
            | Self::ContextCompressionCompleted { .. } => AgenticEventPriority::Normal,

            Self::ToolEvent { tool_event, .. } => tool_event.default_priority(),

            _ => AgenticEventPriority::Low,
        }
    }
}

impl ToolEventData {
    /// Get the default priority for a specific tool event variant.
    pub fn default_priority(&self) -> AgenticEventPriority {
        match self {
            Self::Cancelled { .. } => AgenticEventPriority::Critical,

            Self::Started { .. }
            | Self::Completed { .. }
            | Self::Failed { .. }
            | Self::ConfirmationNeeded { .. } => AgenticEventPriority::High,

            Self::EarlyDetected { .. }
            | Self::ParamsPartial { .. }
            | Self::Queued { .. }
            | Self::Waiting { .. }
            | Self::Progress { .. }
            | Self::Streaming { .. }
            | Self::StreamChunk { .. }
            | Self::Confirmed { .. }
            | Self::Rejected { .. } => AgenticEventPriority::Normal,
        }
    }
}
