//! Agentic Events Definition
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionBackgroundActivityKind {
    AutoMemory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionBackgroundActivityStatus {
    Running,
    Completed,
    Failed,
    Dismissed,
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
        #[serde(default)]
        hidden_session: bool,
        subagent_parent_info: Option<SubagentParentInfo>,
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

    SystemError {
        session_id: Option<String>,
        error: String,
        recoverable: bool,
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

    SessionBackgroundActivityUpdated {
        session_id: String,
        activity_id: String,
        kind: SessionBackgroundActivityKind,
        status: SessionBackgroundActivityStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_turn_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
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
    },
    Failed {
        tool_id: String,
        tool_name: String,
        error: String,
    },
    Cancelled {
        tool_id: String,
        tool_name: String,
        reason: String,
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
            | Self::SessionModelAutoMigrated { session_id, .. }
            | Self::SessionBackgroundActivityUpdated { session_id, .. } => Some(session_id),
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
            | Self::SessionBackgroundActivityUpdated { .. }
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
