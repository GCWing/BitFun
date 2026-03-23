//! Session Serializer - Serialize/deserialize terminal sessions for persistence

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::shell::ShellType;
use crate::{TerminalError, TerminalResult};

use super::{SessionMetadata, SessionSource, SessionStatus, TerminalSession};

/// Version of the serialization format
const SERIALIZATION_VERSION: u32 = 1;

/// Serialized terminal state for cross-version compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedTerminalState {
    /// Serialization format version
    pub version: u32,

    /// Serialized sessions
    pub sessions: Vec<SerializedSession>,

    /// Serialization timestamp
    pub timestamp: i64,
}

/// Serialized session data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedSession {
    /// Session ID
    pub id: String,

    /// Display name
    pub name: String,

    /// Shell type
    pub shell_type: ShellType,

    /// Working directory
    pub cwd: String,

    /// Initial working directory
    pub initial_cwd: String,

    /// Terminal dimensions
    pub cols: u16,
    pub rows: u16,

    /// Environment variables
    pub env: HashMap<String, String>,

    /// Session metadata
    pub metadata: SessionMetadata,

    /// Session creation source
    #[serde(default)]
    pub source: SessionSource,

    /// Replay events for restoring terminal content
    pub replay_events: Vec<ReplayEvent>,

    /// Creation timestamp
    pub created_at: i64,
}

/// Event for replaying terminal content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayEvent {
    /// Terminal dimensions at this point
    pub cols: u16,
    pub rows: u16,

    /// Data to replay
    pub data: String,
}

/// Session serializer
pub struct SessionSerializer;

impl SessionSerializer {
    /// Serialize sessions to a string
    pub fn serialize(sessions: &[TerminalSession]) -> TerminalResult<String> {
        let serialized_sessions: Vec<SerializedSession> = sessions
            .iter()
            .filter(|s| s.should_persist && !s.has_exited())
            .map(|s| SerializedSession {
                id: s.id.clone(),
                name: s.name.clone(),
                shell_type: s.shell_type.clone(),
                cwd: s.cwd.clone(),
                initial_cwd: s.initial_cwd.clone(),
                cols: s.cols,
                rows: s.rows,
                env: s.env.clone(),
                metadata: s.metadata.clone(),
                source: s.source.clone(),
                replay_events: Vec::new(), // TODO: Capture replay events
                created_at: s.created_at.timestamp(),
            })
            .collect();

        let state = SerializedTerminalState {
            version: SERIALIZATION_VERSION,
            sessions: serialized_sessions,
            timestamp: chrono::Utc::now().timestamp(),
        };

        serde_json::to_string(&state).map_err(|e| TerminalError::Serialization(e.to_string()))
    }

    /// Deserialize sessions from a string
    pub fn deserialize(data: &str) -> TerminalResult<Vec<SerializedSession>> {
        let state: SerializedTerminalState =
            serde_json::from_str(data).map_err(|e| TerminalError::Serialization(e.to_string()))?;

        if state.version != SERIALIZATION_VERSION {
            return Err(TerminalError::Serialization(format!(
                "Unsupported serialization version: {} (expected: {})",
                state.version, SERIALIZATION_VERSION
            )));
        }

        Ok(state.sessions)
    }

    /// Create a TerminalSession from a SerializedSession
    pub fn to_session(serialized: &SerializedSession) -> TerminalSession {
        let mut session = TerminalSession::new(
            serialized.id.clone(),
            serialized.name.clone(),
            serialized.shell_type.clone(),
            serialized.cwd.clone(),
            serialized.cols,
            serialized.rows,
            serialized.source.clone(),
        );

        session.initial_cwd = serialized.initial_cwd.clone();
        session.env = serialized.env.clone();
        session.metadata = serialized.metadata.clone();
        session.metadata.was_restored = true;
        session.status = SessionStatus::Starting;

        session
    }

    /// Serialize a single session with replay data
    pub fn serialize_with_replay(
        session: &TerminalSession,
        replay_data: &str,
    ) -> TerminalResult<String> {
        let replay_event = ReplayEvent {
            cols: session.cols,
            rows: session.rows,
            data: replay_data.to_string(),
        };

        let serialized = SerializedSession {
            id: session.id.clone(),
            name: session.name.clone(),
            shell_type: session.shell_type.clone(),
            cwd: session.cwd.clone(),
            initial_cwd: session.initial_cwd.clone(),
            cols: session.cols,
            rows: session.rows,
            env: session.env.clone(),
            metadata: session.metadata.clone(),
            source: session.source.clone(),
            replay_events: vec![replay_event],
            created_at: session.created_at.timestamp(),
        };

        serde_json::to_string(&serialized).map_err(|e| TerminalError::Serialization(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize() {
        let session = TerminalSession::new(
            "test-id".to_string(),
            "Test Terminal".to_string(),
            ShellType::Bash,
            "/home/user".to_string(),
            80,
            24,
            SessionSource::Manual,
        );

        let serialized = SessionSerializer::serialize(&[session.clone()])
            .expect("serialize should succeed for a valid terminal session");
        let deserialized = SessionSerializer::deserialize(&serialized)
            .expect("deserialize should succeed for serialized session payload");

        assert_eq!(deserialized.len(), 1);
        assert_eq!(deserialized[0].id, session.id);
        assert_eq!(deserialized[0].name, session.name);
        assert_eq!(deserialized[0].cwd, session.cwd);
    }
}
