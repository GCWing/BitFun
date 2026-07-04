use crate::session_state::SessionState;
pub use bitfun_core_types::SessionKind;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

// ============ Session ============

/// Session: contains multiple dialog turns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub session_name: String,
    /// Current/default mode selection for the session.
    ///
    /// This is the mode the next dialog turn should run with by default. It is
    /// not required to match either the last surviving history turn or the last
    /// message submission accepted by the scheduler.
    pub agent_type: String,
    /// Cached mode of the last surviving user dialog turn in history.
    ///
    /// Reminder builders use this value for `previous_agent_type` so
    /// first-entry vs ongoing mode prompts follow the surviving transcript
    /// after rollbacks or turn truncation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_user_dialog_agent_type: Option<String>,
    /// Mode of the most recent user submission accepted by the scheduler.
    ///
    /// Unlike `last_user_dialog_agent_type`, this value is not rewound by
    /// history rollback. It tracks session-level prompt-cache compatibility for
    /// the next accepted submission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_submitted_agent_type: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "created_by",
        alias = "createdBy"
    )]
    pub created_by: Option<String>,
    #[serde(default, alias = "session_kind", alias = "sessionKind")]
    pub kind: SessionKind,

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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompressionState {
    /// Time of last compression
    pub last_compression_at: Option<SystemTime>,
    /// Compression trigger count
    pub compression_count: usize,
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
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            created_by: None,
            kind: SessionKind::Standard,
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
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            created_by: None,
            kind: SessionKind::Standard,
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
    /// Stable workspace id for resolving workspace-scoped metadata such as related directories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// SSH workspace: required for remote tool I/O (file/shell). When set, `workspace_path` is
    /// interpreted as the path on that host; when unset, the workspace is always local regardless
    /// of string shape (avoids inferring remote from path alone). Also disambiguates the same
    /// `workspace_path` on different hosts (e.g. two `/` roots).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    /// SSH config `host` for locating `~/.bitfun/remote_ssh/{host}/.../sessions` when disconnected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
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
            workspace_id: None,
            remote_connection_id: None,
            remote_ssh_host: None,
            model_id: None,
        }
    }
}

/// Session summary (for list display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub session_name: String,
    /// Current/default mode selection for the session.
    pub agent_type: String,
    /// Mode of the last surviving user dialog turn in the session history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_user_dialog_agent_type: Option<String>,
    /// Mode of the most recent user submission accepted by the scheduler.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_submitted_agent_type: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "created_by",
        alias = "createdBy"
    )]
    pub created_by: Option<String>,
    #[serde(default, alias = "session_kind", alias = "sessionKind")]
    pub kind: SessionKind,
    pub turn_count: usize,
    pub created_at: SystemTime,
    pub last_activity_at: SystemTime,
    pub state: SessionState,
}

/// Persisted session state sidecar used by product session storage.
///
/// The runtime owns this wire shape because it contains provider-neutral session
/// facts. Product persistence code still owns file I/O and path resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSessionStateFile {
    pub schema_version: u32,
    pub config: SessionConfig,
    pub snapshot_session_id: Option<String>,
    /// Derived runtime cache for reminder semantics. The source of truth lives
    /// on persisted dialog turns via `DialogTurnData.agent_type`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_user_dialog_agent_type: Option<String>,
    /// Session-level prompt-cache guard state. This records the most recent user
    /// submission accepted by the scheduler and intentionally does not rewind on
    /// history rollback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_submitted_agent_type: Option<String>,
    pub compression_state: CompressionState,
    pub runtime_state: SessionState,
}

pub fn sanitize_persisted_session_state(state: &SessionState) -> SessionState {
    match state {
        SessionState::Processing { .. } => SessionState::Idle,
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        sanitize_persisted_session_state, CompressionState, PersistedSessionStateFile, Session,
        SessionConfig,
    };
    use crate::session_state::{ProcessingPhase, SessionState};
    use serde_json::json;

    #[test]
    fn session_config_default_preserves_existing_context_budget() {
        let config = SessionConfig::default();

        assert_eq!(config.max_context_tokens, 128128);
        assert!(config.auto_compact);
        assert!(config.enable_tools);
        assert!(config.safe_mode);
        assert_eq!(config.max_turns, 200);
        assert!(config.enable_context_compression);
        assert_eq!(config.compression_threshold, 0.8);
        assert!(config.workspace_path.is_none());
        assert!(config.workspace_id.is_none());
        assert!(config.remote_connection_id.is_none());
        assert!(config.remote_ssh_host.is_none());
        assert!(config.model_id.is_none());
    }

    #[test]
    fn new_session_preserves_legacy_runtime_defaults() {
        let session = Session::new(
            "Session".to_string(),
            "agentic".to_string(),
            SessionConfig::default(),
        );

        assert_eq!(session.session_name, "Session");
        assert_eq!(session.agent_type, "agentic");
        assert_eq!(session.dialog_turn_ids, Vec::<String>::new());
        assert_eq!(session.state, SessionState::Idle);
        assert_eq!(session.compression_state.compression_count, 0);
        assert!(session.last_user_dialog_agent_type.is_none());
        assert!(session.last_submitted_agent_type.is_none());
        assert!(session.created_by.is_none());
        assert!(session.snapshot_session_id.is_none());
    }

    #[test]
    fn persisted_session_state_sanitizes_processing_to_idle() {
        let sanitized = sanitize_persisted_session_state(&SessionState::Processing {
            current_turn_id: "turn-1".to_string(),
            phase: ProcessingPhase::Thinking,
        });

        assert_eq!(sanitized, SessionState::Idle);
        assert_eq!(
            sanitize_persisted_session_state(&SessionState::Error {
                error: "boom".to_string(),
                recoverable: true,
            }),
            SessionState::Error {
                error: "boom".to_string(),
                recoverable: true,
            }
        );
    }

    #[test]
    fn persisted_session_state_file_shape_stays_compatible() {
        let file = PersistedSessionStateFile {
            schema_version: 1,
            config: SessionConfig {
                workspace_path: Some("/workspace".to_string()),
                model_id: Some("model-a".to_string()),
                ..SessionConfig::default()
            },
            snapshot_session_id: Some("snapshot-1".to_string()),
            last_user_dialog_agent_type: Some("agentic".to_string()),
            last_submitted_agent_type: Some("DeepReview".to_string()),
            compression_state: CompressionState {
                last_compression_at: None,
                compression_count: 2,
            },
            runtime_state: SessionState::Idle,
        };

        assert_eq!(
            serde_json::to_value(file).expect("persisted session state should serialize"),
            json!({
                "schema_version": 1,
                "config": {
                    "max_context_tokens": 128128,
                    "auto_compact": true,
                    "enable_tools": true,
                    "safe_mode": true,
                    "max_turns": 200,
                    "enable_context_compression": true,
                    "compression_threshold": 0.800000011920929,
                    "workspace_path": "/workspace",
                    "model_id": "model-a"
                },
                "snapshot_session_id": "snapshot-1",
                "last_user_dialog_agent_type": "agentic",
                "last_submitted_agent_type": "DeepReview",
                "compression_state": {
                    "last_compression_at": null,
                    "compression_count": 2
                },
                "runtime_state": "Idle"
            })
        );
    }
}
