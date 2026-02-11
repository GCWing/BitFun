/// Generic event definitions
///
/// Supports multiple event types, uniformly distributed by transport layer
use serde::{Deserialize, Serialize};

/// Unified event enum - All events to be sent to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "payload")]
pub enum UnifiedEvent {
    /// Agentic system event
    Agentic(AgenticEventPayload),

    /// LSP event
    Lsp(LspEventPayload),

    /// File watch event
    FileWatch(FileWatchEventPayload),

    /// Profile generation event
    Profile(ProfileEventPayload),

    /// Snapshot event
    Snapshot(SnapshotEventPayload),

    /// Generic backend event
    Backend(BackendEventPayload),
}

/// Agentic event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticEventPayload {
    pub session_id: String,
    pub turn_id: Option<String>,
    pub event_data: serde_json::Value,
}

/// LSP event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspEventPayload {
    pub workspace_path: String,
    pub language: Option<String>,
    pub event_data: serde_json::Value,
}

/// File watch event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWatchEventPayload {
    pub path: String,
    pub event_type: String, // "create", "modify", "delete"
    pub timestamp: i64,
}

/// Profile event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileEventPayload {
    pub workspace_path: String,
    pub event_data: serde_json::Value,
}

/// Snapshot event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEventPayload {
    pub snapshot_id: String,
    pub event_data: serde_json::Value,
}

/// Backend event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendEventPayload {
    pub event_name: String,
    pub data: serde_json::Value,
}

/// Event priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventPriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

impl Default for EventPriority {
    fn default() -> Self {
        Self::Normal
    }
}
