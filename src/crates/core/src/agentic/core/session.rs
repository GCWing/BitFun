use super::state::SessionState;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

// ============ Session ============

/// Session: contains multiple dialog turns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub session_name: String,
    pub agent_type: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "created_by",
        alias = "createdBy"
    )]
    pub created_by: Option<String>,

    /// Associated resources
    #[serde(
        skip_serializing_if = "Option::is_none",
        alias = "sandbox_session_id",
        alias = "sandboxSessionId"
    )]
    pub snapshot_session_id: Option<String>,

    /// Dialog turn ID list
    pub dialog_turn_ids: Vec<String>,

    /// Session state
    pub state: SessionState,

    /// Configuration
    pub config: SessionConfig,

    /// Context compression related
    pub compression_state: CompressionState,

    /// Lifecycle
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub last_activity_at: SystemTime,
}

/// Context compression state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionState {
    /// Time of last compression
    pub last_compression_at: Option<SystemTime>,
    /// Compression trigger count
    pub compression_count: usize,
}

impl Default for CompressionState {
    fn default() -> Self {
        Self {
            last_compression_at: None,
            compression_count: 0,
        }
    }
}

impl CompressionState {
    pub fn increment_compression_count(&mut self) {
        self.last_compression_at = Some(SystemTime::now());
        self.compression_count += 1;
    }
}

impl Session {
    pub fn new(session_name: String, agent_type: String, config: SessionConfig) -> Self {
        let now = SystemTime::now();
        Self {
            session_id: Uuid::new_v4().to_string(),
            session_name,
            agent_type,
            created_by: None,
            snapshot_session_id: None,
            dialog_turn_ids: vec![],
            state: SessionState::Idle,
            config,
            compression_state: CompressionState::default(),
            created_at: now,
            updated_at: now,
            last_activity_at: now,
        }
    }

    pub fn new_with_id(
        session_id: String,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            session_id,
            session_name,
            agent_type,
            created_by: None,
            snapshot_session_id: None,
            dialog_turn_ids: vec![],
            state: SessionState::Idle,
            config,
            compression_state: CompressionState::default(),
            created_at: now,
            updated_at: now,
            last_activity_at: now,
        }
    }
}

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub max_context_tokens: usize,
    pub auto_compact: bool,
    pub enable_tools: bool,
    pub safe_mode: bool,
    pub max_turns: usize,
    pub enable_context_compression: bool,
    /// Compression threshold (token usage rate), compression triggered when exceeded
    pub compression_threshold: f32,
    /// Workspace path bound to this session. Used to run AI in the correct workspace
    /// without changing the desktop's foreground workspace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    /// SSH workspace: disambiguates the same `workspace_path` on different hosts (e.g. two `/` roots).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    /// Model config ID used by this session (for token usage tracking)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: 128128,
            auto_compact: true,
            enable_tools: true,
            safe_mode: true,
            max_turns: 200,
            enable_context_compression: true,
            compression_threshold: 0.8, // 80%
            workspace_path: None,
            remote_connection_id: None,
            model_id: None,
        }
    }
}

/// Session summary (for list display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub session_name: String,
    pub agent_type: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "created_by",
        alias = "createdBy"
    )]
    pub created_by: Option<String>,
    pub turn_count: usize,
    pub created_at: SystemTime,
    pub last_activity_at: SystemTime,
    pub state: SessionState,
}
