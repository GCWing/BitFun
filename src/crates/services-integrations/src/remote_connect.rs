//! Remote-connect integration contracts.
//!
//! This module owns stable remote-facing DTOs and runtime-port request
//! construction. Network lifecycle, tracker state, and product assembly stay in
//! `bitfun-core` until their ports are explicit.

use bitfun_runtime_ports::{
    AgentSessionCreateRequest, AgentSubmissionRequest, AgentSubmissionSource,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteConnectSubmissionSource {
    Relay,
    Bot,
}

impl RemoteConnectSubmissionSource {
    pub const fn agent_submission_source(self) -> AgentSubmissionSource {
        match self {
            RemoteConnectSubmissionSource::Relay => AgentSubmissionSource::RemoteRelay,
            RemoteConnectSubmissionSource::Bot => AgentSubmissionSource::Bot,
        }
    }

    pub const fn metadata_source(self) -> &'static str {
        match self {
            RemoteConnectSubmissionSource::Relay => "remote_relay",
            RemoteConnectSubmissionSource::Bot => "bot",
        }
    }
}

pub fn build_remote_session_create_request(
    session_name: impl Into<String>,
    agent_type: impl Into<String>,
    workspace_path: Option<impl Into<String>>,
    source: RemoteConnectSubmissionSource,
) -> AgentSessionCreateRequest {
    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "source".to_string(),
        serde_json::Value::String(source.metadata_source().to_string()),
    );

    AgentSessionCreateRequest {
        session_name: session_name.into(),
        agent_type: agent_type.into(),
        workspace_path: workspace_path.map(Into::into),
        metadata,
    }
}

pub fn build_remote_submission_request(
    session_id: impl Into<String>,
    message: impl Into<String>,
    turn_id: Option<String>,
    source: RemoteConnectSubmissionSource,
) -> AgentSubmissionRequest {
    AgentSubmissionRequest {
        session_id: session_id.into(),
        message: message.into(),
        turn_id,
        source: Some(source.agent_submission_source()),
        attachments: Vec::new(),
        metadata: serde_json::Map::new(),
    }
}

/// Image sent from a remote client as a base64 data URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageAttachment {
    pub name: String,
    pub data_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub name: String,
    pub agent_type: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatImageAttachment {
    pub name: String,
    pub data_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<RemoteToolStatus>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<ChatMessageItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<ChatImageAttachment>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessageItem {
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<RemoteToolStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_subagent: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentWorkspaceEntry {
    pub path: String,
    pub name: String,
    pub last_opened: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistantEntry {
    pub path: String,
    pub name: String,
    pub assistant_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveTurnSnapshot {
    pub turn_id: String,
    pub status: String,
    pub text: String,
    pub thinking: String,
    pub tools: Vec<RemoteToolStatus>,
    pub round_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<ChatMessageItem>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteToolStatus {
    pub id: String,
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<serde_json::Value>,
}
