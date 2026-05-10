//! Types for session persistence

use crate::agentic::core::SessionKind;
use serde::{Deserialize, Serialize};

pub const SESSION_STORAGE_SCHEMA_VERSION: u32 = 2;

/// Session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    /// Session ID
    #[serde(alias = "session_id")]
    pub session_id: String,

    /// Session name (user-editable)
    #[serde(alias = "session_name")]
    pub session_name: String,

    /// Agent type
    #[serde(alias = "agent_type")]
    pub agent_type: String,

    /// Creator identity for future permission checks
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "created_by")]
    pub created_by: Option<String>,
    #[serde(default, alias = "session_kind", alias = "sessionKind")]
    pub session_kind: SessionKind,

    /// Model name
    #[serde(alias = "model_name")]
    pub model_name: String,

    /// Created time (Unix timestamp ms)
    #[serde(alias = "created_at")]
    pub created_at: u64,

    /// Last active time (Unix timestamp ms)
    #[serde(alias = "last_active_at")]
    pub last_active_at: u64,

    /// Turn count
    #[serde(alias = "turn_count")]
    pub turn_count: usize,

    /// Total message count (user + AI)
    #[serde(alias = "message_count")]
    pub message_count: usize,

    /// Total tool call count
    #[serde(alias = "tool_call_count")]
    pub tool_call_count: usize,

    /// Session status
    pub status: SessionStatus,

    /// Terminal session ID (if any)
    #[serde(skip_serializing_if = "Option::is_none", alias = "terminal_session_id")]
    pub terminal_session_id: Option<String>,

    /// Snapshot session ID (if any)
    #[serde(
        skip_serializing_if = "Option::is_none",
        alias = "sandbox_session_id",
        alias = "sandboxSessionId"
    )]
    pub snapshot_session_id: Option<String>,

    /// Tags (for categorization and search)
    #[serde(default)]
    pub tags: Vec<String>,

    /// Custom metadata
    #[serde(skip_serializing_if = "Option::is_none", alias = "custom_metadata")]
    pub custom_metadata: Option<serde_json::Value>,

    /// Todo list (for persisting the session's todo state)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub todos: Option<serde_json::Value>,

    /// Deep Review run manifest for this session, when the session was launched
    /// from Code Review Team.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "deep_review_run_manifest",
        alias = "deepReviewRunManifest"
    )]
    pub deep_review_run_manifest: Option<serde_json::Value>,

    /// Cached reviewer outputs from previous deep review runs in this session.
    /// Keyed by packet_id, value is the reviewer's output text.
    /// Used for incremental review: when the fingerprint matches, skip re-dispatching.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "deep_review_cache",
        alias = "deepReviewCache"
    )]
    pub deep_review_cache: Option<serde_json::Value>,

    /// Workspace path this session belongs to (normalized source workspace root, not mirror dir)
    #[serde(skip_serializing_if = "Option::is_none", alias = "workspace_path")]
    pub workspace_path: Option<String>,

    /// Unified hostname for workspace identity: `localhost` for local workspaces,
    /// SSH host for remote workspaces.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "workspace_hostname"
    )]
    pub workspace_hostname: Option<String>,

    /// Unread completion status for the session.
    /// 'completed' → green dot, 'error' → red dot.
    /// Cleared after the user switches to the session and the content renders.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "unread_completion",
        alias = "unreadCompletion"
    )]
    pub unread_completion: Option<String>,

    /// High-priority attention status for the session.
    /// Set when the session requires user action while not the active session.
    /// 'ask_user' → pending AskUserQuestion waiting for answer.
    /// 'tool_confirm' → pending tool confirmations.
    /// Takes precedence over unread_completion in the UI.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "needs_user_attention",
        alias = "needsUserAttention"
    )]
    pub needs_user_attention: Option<String>,
}

/// Session status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Archived,
    Completed,
}

/// Session list (metadata for all sessions)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionList {
    pub sessions: Vec<SessionMetadata>,
    #[serde(alias = "last_updated")]
    pub last_updated: u64,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSessionMetadataFile {
    pub schema_version: u32,
    #[serde(flatten)]
    pub metadata: SessionMetadata,
}

impl StoredSessionMetadataFile {
    pub fn new(metadata: SessionMetadata) -> Self {
        Self {
            schema_version: SESSION_STORAGE_SCHEMA_VERSION,
            metadata,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSessionIndexFile {
    pub schema_version: u32,
    pub updated_at: u64,
    pub sessions: Vec<SessionMetadata>,
}

impl StoredSessionIndexFile {
    pub fn new(updated_at: u64, sessions: Vec<SessionMetadata>) -> Self {
        Self {
            schema_version: SESSION_STORAGE_SCHEMA_VERSION,
            updated_at,
            sessions,
        }
    }
}

impl Default for SessionList {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            last_updated: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            version: "1.0".to_string(),
        }
    }
}

/// Full dialog turn data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogTurnData {
    /// Turn ID
    #[serde(alias = "turn_id")]
    pub turn_id: String,

    /// Turn index (starting from 0)
    #[serde(alias = "turn_index")]
    pub turn_index: usize,

    /// Session ID
    #[serde(alias = "session_id")]
    pub session_id: String,

    /// Timestamp
    pub timestamp: u64,

    /// Turn kind
    #[serde(default, alias = "turn_kind")]
    pub kind: DialogTurnKind,

    /// User message
    #[serde(alias = "user_message")]
    pub user_message: UserMessageData,

    /// Model interaction rounds
    #[serde(alias = "model_rounds")]
    pub model_rounds: Vec<ModelRoundData>,

    /// Turn start time
    #[serde(alias = "start_time")]
    pub start_time: u64,

    /// Turn end time
    #[serde(skip_serializing_if = "Option::is_none", alias = "end_time")]
    pub end_time: Option<u64>,

    /// Turn duration (milliseconds)
    #[serde(skip_serializing_if = "Option::is_none", alias = "duration_ms")]
    pub duration_ms: Option<u64>,

    /// Turn status
    pub status: TurnStatus,
}

/// Persisted dialog turn kind.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum DialogTurnKind {
    #[default]
    UserDialog,
    ManualCompaction,
    LocalCommand,
}

impl DialogTurnKind {
    pub fn is_model_visible(self) -> bool {
        matches!(self, Self::UserDialog)
    }
}

/// User message data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserMessageData {
    pub id: String,
    pub content: String,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Model interaction round data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRoundData {
    pub id: String,
    #[serde(alias = "turn_id")]
    pub turn_id: String,
    #[serde(alias = "round_index")]
    pub round_index: usize,
    pub timestamp: u64,

    /// Text item entries
    #[serde(default, alias = "text_items")]
    pub text_items: Vec<TextItemData>,

    /// Tool call entries
    #[serde(default, alias = "tool_items")]
    pub tool_items: Vec<ToolItemData>,

    /// Thinking item entries
    #[serde(default, alias = "thinking_items")]
    pub thinking_items: Vec<ThinkingItemData>,

    #[serde(alias = "start_time")]
    pub start_time: u64,
    #[serde(skip_serializing_if = "Option::is_none", alias = "end_time")]
    pub end_time: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "duration_ms"
    )]
    pub duration_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "provider_id"
    )]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "model_id")]
    pub model_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "model_alias"
    )]
    pub model_alias: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "first_chunk_ms"
    )]
    pub first_chunk_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "first_visible_output_ms"
    )]
    pub first_visible_output_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "stream_duration_ms"
    )]
    pub stream_duration_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "attempt_count"
    )]
    pub attempt_count: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "failure_category"
    )]
    pub failure_category: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "token_details"
    )]
    pub token_details: Option<serde_json::Value>,
    pub status: String,
}

/// Text item data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextItemData {
    pub id: String,
    pub content: String,
    #[serde(alias = "is_streaming")]
    pub is_streaming: bool,
    pub timestamp: u64,
    /// Whether Markdown format (default `true`)
    #[serde(default = "default_is_markdown", alias = "is_markdown")]
    pub is_markdown: bool,

    /// Original order index (to restore the correct insertion order)
    #[serde(skip_serializing_if = "Option::is_none", alias = "order_index")]
    pub order_index: Option<usize>,

    /// Subagent marker field
    #[serde(skip_serializing_if = "Option::is_none", alias = "is_subagent_item")]
    pub is_subagent_item: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none", alias = "parent_task_tool_id")]
    pub parent_task_tool_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", alias = "subagent_session_id")]
    pub subagent_session_id: Option<String>,

    /// Status field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

fn default_is_markdown() -> bool {
    true
}

/// Thinking item data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingItemData {
    pub id: String,
    pub content: String,
    #[serde(alias = "is_streaming")]
    pub is_streaming: bool,
    #[serde(alias = "is_collapsed")]
    pub is_collapsed: bool,
    pub timestamp: u64,

    /// Original order index (to restore the correct insertion order)
    #[serde(skip_serializing_if = "Option::is_none", alias = "order_index")]
    pub order_index: Option<usize>,

    /// Status field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    /// Subagent marker field (fixes incorrect placement of subagent thinking content after restart)
    #[serde(skip_serializing_if = "Option::is_none", alias = "is_subagent_item")]
    pub is_subagent_item: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none", alias = "parent_task_tool_id")]
    pub parent_task_tool_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", alias = "subagent_session_id")]
    pub subagent_session_id: Option<String>,
}

/// Tool item data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolItemData {
    pub id: String,
    #[serde(alias = "tool_name")]
    pub tool_name: String,
    #[serde(alias = "tool_call")]
    pub tool_call: ToolCallData,
    #[serde(skip_serializing_if = "Option::is_none", alias = "tool_result")]
    pub tool_result: Option<ToolResultData>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "ai_intent")]
    pub ai_intent: Option<String>,
    #[serde(alias = "start_time")]
    pub start_time: u64,
    #[serde(skip_serializing_if = "Option::is_none", alias = "end_time")]
    pub end_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "duration_ms")]
    pub duration_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "queue_wait_ms"
    )]
    pub queue_wait_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "preflight_ms"
    )]
    pub preflight_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "confirmation_wait_ms"
    )]
    pub confirmation_wait_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "execution_ms"
    )]
    pub execution_ms: Option<u64>,

    /// Original order index (to restore the correct insertion order)
    #[serde(skip_serializing_if = "Option::is_none", alias = "order_index")]
    pub order_index: Option<usize>,

    /// Subagent marker field
    #[serde(skip_serializing_if = "Option::is_none", alias = "is_subagent_item")]
    pub is_subagent_item: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none", alias = "parent_task_tool_id")]
    pub parent_task_tool_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", alias = "subagent_session_id")]
    pub subagent_session_id: Option<String>,

    /// Status field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", alias = "interruption_reason")]
    pub interruption_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallData {
    pub input: serde_json::Value,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultData {
    pub result: serde_json::Value,
    pub success: bool,
    #[serde(
        skip_serializing_if = "Option::is_none",
        alias = "result_for_assistant"
    )]
    pub result_for_assistant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "duration_ms")]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptLineRange {
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionTranscriptIndexEntry {
    #[serde(alias = "turn_index")]
    pub turn_index: usize,
    pub preview: String,
    #[serde(alias = "turn_range")]
    pub turn_range: TranscriptLineRange,
    #[serde(alias = "user_range")]
    pub user_range: TranscriptLineRange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct SessionTranscriptExportOptions {
    #[serde(default)]
    pub tools: bool,
    #[serde(default)]
    pub tool_inputs: bool,
    #[serde(default)]
    pub thinking: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionTranscriptExport {
    #[serde(alias = "session_id")]
    pub session_id: String,
    #[serde(alias = "transcript_path")]
    pub transcript_path: String,
    #[serde(alias = "generated_at")]
    pub generated_at: u64,
    #[serde(alias = "source_fingerprint")]
    pub source_fingerprint: String,
    #[serde(alias = "includes_tools")]
    pub includes_tools: bool,
    #[serde(default, alias = "includes_tool_inputs")]
    pub includes_tool_inputs: bool,
    #[serde(alias = "includes_thinking")]
    pub includes_thinking: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turns: Option<Vec<String>>,
    #[serde(alias = "turn_count")]
    pub turn_count: usize,
    #[serde(alias = "line_count")]
    pub line_count: usize,
    #[serde(default = "default_transcript_line_range", alias = "index_range")]
    pub index_range: TranscriptLineRange,
    pub index: Vec<SessionTranscriptIndexEntry>,
}

fn default_transcript_line_range() -> TranscriptLineRange {
    TranscriptLineRange {
        start_line: 0,
        end_line: 0,
    }
}

/// Turn status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TurnStatus {
    InProgress,
    Completed,
    Error,
    Cancelled,
}

impl SessionMetadata {
    /// Creates a new session metadata.
    pub fn new(
        session_id: String,
        session_name: String,
        agent_type: String,
        model_name: String,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            session_id,
            session_name,
            agent_type,
            created_by: None,
            session_kind: SessionKind::Standard,
            model_name,
            created_at: now,
            last_active_at: now,
            turn_count: 0,
            message_count: 0,
            tool_call_count: 0,
            status: SessionStatus::Active,
            terminal_session_id: None,
            snapshot_session_id: None,
            tags: Vec::new(),
            custom_metadata: None,
            todos: None,
            deep_review_run_manifest: None,
            deep_review_cache: None,
            workspace_path: None,
            workspace_hostname: None,
            unread_completion: None,
            needs_user_attention: None,
        }
    }

    /// Updates the last active time.
    pub fn touch(&mut self) {
        self.last_active_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
    }

    /// Increments the turn count.
    pub fn increment_turn(&mut self) {
        self.turn_count += 1;
    }

    /// Adds to the message count.
    pub fn add_messages(&mut self, count: usize) {
        self.message_count += count;
    }

    /// Adds to the tool call count.
    pub fn add_tool_calls(&mut self, count: usize) {
        self.tool_call_count += count;
    }

    pub fn is_subagent(&self) -> bool {
        matches!(self.session_kind, SessionKind::Subagent)
    }

    pub fn is_standard(&self) -> bool {
        !self.is_subagent()
    }

    pub fn is_legacy_leaked_subagent_candidate(&self) -> bool {
        let Some(created_by) = self.created_by.as_deref() else {
            return false;
        };
        if !created_by.starts_with("session-") {
            return false;
        }

        self.session_name.starts_with("Subagent: ")
    }

    pub fn should_hide_from_user_lists(&self) -> bool {
        self.is_subagent() || self.is_legacy_leaked_subagent_candidate()
    }
}

impl DialogTurnData {
    /// Creates a new dialog turn.
    pub fn new(
        turn_id: String,
        turn_index: usize,
        session_id: String,
        user_message: UserMessageData,
    ) -> Self {
        Self::new_with_kind(
            DialogTurnKind::UserDialog,
            turn_id,
            turn_index,
            session_id,
            user_message,
        )
    }

    /// Creates a new dialog turn with an explicit persisted kind.
    pub fn new_with_kind(
        kind: DialogTurnKind,
        turn_id: String,
        turn_index: usize,
        session_id: String,
        user_message: UserMessageData,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            turn_id,
            turn_index,
            session_id,
            timestamp: now,
            kind,
            user_message,
            model_rounds: Vec::new(),
            start_time: now,
            end_time: None,
            duration_ms: None,
            status: TurnStatus::InProgress,
        }
    }

    /// Marks this turn as completed.
    pub fn mark_completed(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.end_time = Some(now);
        self.duration_ms = Some(now.saturating_sub(self.start_time));
        self.status = TurnStatus::Completed;
    }

    /// Counts total tool calls.
    pub fn count_tool_calls(&self) -> usize {
        self.model_rounds
            .iter()
            .map(|round| round.tool_items.len())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DialogTurnData, DialogTurnKind, ModelRoundData, SessionMetadata, ToolItemData,
        UserMessageData,
    };
    use crate::agentic::core::SessionKind;

    #[test]
    fn dialog_turn_kind_defaults_to_user_dialog_for_legacy_payloads() {
        let payload = serde_json::json!({
            "turnId": "turn-1",
            "turnIndex": 0,
            "sessionId": "session-1",
            "timestamp": 1,
            "userMessage": {
                "id": "user-1",
                "content": "hello",
                "timestamp": 1
            },
            "modelRounds": [],
            "startTime": 1,
            "status": "completed"
        });

        let turn: DialogTurnData =
            serde_json::from_value(payload).expect("legacy payload should deserialize");

        assert_eq!(turn.kind, DialogTurnKind::UserDialog);
    }

    #[test]
    fn dialog_turn_data_new_defaults_to_user_dialog() {
        let turn = DialogTurnData::new(
            "turn-1".to_string(),
            0,
            "session-1".to_string(),
            UserMessageData {
                id: "user-1".to_string(),
                content: "hello".to_string(),
                timestamp: 1,
                metadata: None,
            },
        );

        assert_eq!(turn.kind, DialogTurnKind::UserDialog);
    }

    #[test]
    fn local_usage_report_turn_is_model_invisible() {
        assert!(!DialogTurnKind::LocalCommand.is_model_visible());
    }

    #[test]
    fn manual_compaction_turn_is_model_invisible() {
        assert!(!DialogTurnKind::ManualCompaction.is_model_visible());
    }

    #[test]
    fn session_metadata_marks_explicit_subagent_as_non_standard() {
        let mut metadata = SessionMetadata::new(
            "session-1".to_string(),
            "Subagent: explore repo".to_string(),
            "Explore".to_string(),
            "model".to_string(),
        );
        metadata.session_kind = SessionKind::Subagent;

        assert!(metadata.is_subagent());
        assert!(!metadata.is_standard());
    }

    #[test]
    fn session_metadata_does_not_treat_standard_session_as_subagent_from_name_or_creator() {
        let mut metadata = SessionMetadata::new(
            "session-1".to_string(),
            "Subagent: repo sweep".to_string(),
            "Explore".to_string(),
            "model".to_string(),
        );
        metadata.created_by = Some("session-parent".to_string());

        assert!(!metadata.is_subagent());
        assert!(metadata.is_standard());
    }

    #[test]
    fn session_metadata_detects_legacy_leaked_subagent_candidate() {
        let mut metadata = SessionMetadata::new(
            "session-1".to_string(),
            "Subagent: repo sweep".to_string(),
            "Explore".to_string(),
            "model".to_string(),
        );
        metadata.created_by = Some("session-parent".to_string());

        assert!(!metadata.is_subagent());
        assert!(metadata.is_legacy_leaked_subagent_candidate());
        assert!(metadata.should_hide_from_user_lists());
    }

    #[test]
    fn session_metadata_keeps_normal_sessions_visible() {
        let metadata = SessionMetadata::new(
            "session-1".to_string(),
            "Normal Session".to_string(),
            "agentic".to_string(),
            "model".to_string(),
        );

        assert!(!metadata.is_subagent());
        assert!(metadata.is_standard());
    }

    #[test]
    fn persisted_runtime_span_fields_are_optional_and_round_trip() {
        let legacy_round_payload = serde_json::json!({
            "id": "round-legacy",
            "turnId": "turn-1",
            "roundIndex": 0,
            "timestamp": 1,
            "textItems": [],
            "toolItems": [],
            "thinkingItems": [],
            "startTime": 1,
            "endTime": 2,
            "status": "completed"
        });

        let legacy_round: ModelRoundData =
            serde_json::from_value(legacy_round_payload).expect("legacy round should deserialize");
        assert_eq!(legacy_round.duration_ms, None);
        assert_eq!(legacy_round.model_id, None);
        assert_eq!(legacy_round.first_chunk_ms, None);

        let round_payload = serde_json::json!({
            "id": "round-1",
            "turnId": "turn-1",
            "roundIndex": 0,
            "timestamp": 1,
            "textItems": [],
            "toolItems": [],
            "thinkingItems": [],
            "startTime": 1,
            "endTime": 121,
            "durationMs": 120,
            "providerId": "provider-a",
            "modelId": "model-a",
            "modelAlias": "Model A",
            "firstChunkMs": 10,
            "firstVisibleOutputMs": 12,
            "streamDurationMs": 90,
            "attemptCount": 2,
            "failureCategory": "rate_limit",
            "tokenDetails": { "reasoningTokens": 7 },
            "status": "completed"
        });

        let round: ModelRoundData =
            serde_json::from_value(round_payload).expect("P1 round should deserialize");
        assert_eq!(round.duration_ms, Some(120));
        assert_eq!(round.provider_id.as_deref(), Some("provider-a"));
        assert_eq!(round.model_id.as_deref(), Some("model-a"));
        assert_eq!(round.first_visible_output_ms, Some(12));
        assert_eq!(round.attempt_count, Some(2));
        assert_eq!(round.failure_category.as_deref(), Some("rate_limit"));

        let encoded = serde_json::to_value(&round).expect("round should serialize");
        assert_eq!(encoded["durationMs"], 120);
        assert_eq!(encoded["modelId"], "model-a");
        assert_eq!(encoded["firstChunkMs"], 10);

        let tool_payload = serde_json::json!({
            "id": "tool-1",
            "toolName": "write_file",
            "toolCall": { "id": "call-1", "input": { "file_path": "src/main.rs" } },
            "startTime": 5,
            "endTime": 105,
            "durationMs": 100,
            "queueWaitMs": 7,
            "preflightMs": 11,
            "confirmationWaitMs": 13,
            "executionMs": 69,
            "status": "completed"
        });

        let tool: ToolItemData =
            serde_json::from_value(tool_payload).expect("P1 tool should deserialize");
        assert_eq!(tool.queue_wait_ms, Some(7));
        assert_eq!(tool.preflight_ms, Some(11));
        assert_eq!(tool.confirmation_wait_ms, Some(13));
        assert_eq!(tool.execution_ms, Some(69));

        let encoded = serde_json::to_value(&tool).expect("tool should serialize");
        assert_eq!(encoded["queueWaitMs"], 7);
        assert_eq!(encoded["executionMs"], 69);
    }

    #[test]
    fn session_metadata_preserves_deep_review_run_manifest() {
        let payload = serde_json::json!({
            "sessionId": "session-1",
            "sessionName": "Deep Review",
            "agentType": "DeepReview",
            "sessionKind": "standard",
            "modelName": "fast",
            "createdAt": 1,
            "lastActiveAt": 1,
            "turnCount": 0,
            "messageCount": 0,
            "toolCallCount": 0,
            "status": "active",
            "deep_review_run_manifest": {
                "reviewMode": "deep",
                "coreReviewers": [
                    { "subagentId": "ReviewBusinessLogic" }
                ],
                "skippedReviewers": [
                    { "subagentId": "ReviewFrontend", "reason": "not_applicable" }
                ]
            }
        });

        let metadata: SessionMetadata =
            serde_json::from_value(payload).expect("metadata should deserialize");

        assert_eq!(
            metadata.deep_review_run_manifest.as_ref().unwrap()["reviewMode"],
            "deep"
        );

        let serialized = serde_json::to_value(&metadata).expect("metadata should serialize");
        assert_eq!(
            serialized["deepReviewRunManifest"]["coreReviewers"][0]["subagentId"],
            "ReviewBusinessLogic"
        );
    }
}
