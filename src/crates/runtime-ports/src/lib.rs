//! Thin runtime ports for boundaries that currently cross service and agentic
//! concrete implementations.
//!
//! This crate intentionally contains only DTOs and traits. It must not depend
//! on concrete managers, platform adapters, `bitfun-core`, or app crates.

use serde::{Deserialize, Serialize};

pub type PortResult<T> = Result<T, PortError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortErrorKind {
    NotAvailable,
    NotFound,
    InvalidRequest,
    PermissionDenied,
    Cancelled,
    Timeout,
    Backend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortError {
    pub kind: PortErrorKind,
    pub message: String,
}

impl PortError {
    pub fn new(kind: PortErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for PortError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionCreateRequest {
    pub session_name: String,
    pub agent_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionCreateResult {
    pub session_id: String,
    pub agent_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSubmissionRequest {
    pub session_id: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AgentSubmissionSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<AgentInputAttachment>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSubmissionSource {
    DesktopUi,
    DesktopApi,
    AgentSession,
    ScheduledJob,
    RemoteRelay,
    Bot,
    Cli,
}

pub type DialogTriggerSource = AgentSubmissionSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogQueuePriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogSubmissionPolicy {
    pub trigger_source: DialogTriggerSource,
    pub queue_priority: DialogQueuePriority,
    pub skip_tool_confirmation: bool,
}

impl DialogSubmissionPolicy {
    pub const fn new(
        trigger_source: DialogTriggerSource,
        queue_priority: DialogQueuePriority,
        skip_tool_confirmation: bool,
    ) -> Self {
        Self {
            trigger_source,
            queue_priority,
            skip_tool_confirmation,
        }
    }

    pub const fn for_source(trigger_source: DialogTriggerSource) -> Self {
        let (queue_priority, skip_tool_confirmation) = match trigger_source {
            DialogTriggerSource::AgentSession => (DialogQueuePriority::Low, true),
            DialogTriggerSource::ScheduledJob => (DialogQueuePriority::Low, true),
            DialogTriggerSource::DesktopUi
            | DialogTriggerSource::DesktopApi
            | DialogTriggerSource::Cli => (DialogQueuePriority::Normal, false),
            DialogTriggerSource::RemoteRelay | DialogTriggerSource::Bot => {
                (DialogQueuePriority::Normal, true)
            }
        };
        Self::new(trigger_source, queue_priority, skip_tool_confirmation)
    }

    pub const fn with_queue_priority(mut self, queue_priority: DialogQueuePriority) -> Self {
        self.queue_priority = queue_priority;
        self
    }

    pub const fn with_skip_tool_confirmation(mut self, skip_tool_confirmation: bool) -> Self {
        self.skip_tool_confirmation = skip_tool_confirmation;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogSubmitOutcome {
    Started { session_id: String, turn_id: String },
    Queued { session_id: String, turn_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInputAttachment {
    pub kind: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

impl AgentInputAttachment {
    pub fn remote_image(
        id: impl Into<String>,
        name: impl Into<String>,
        data_url: impl Into<String>,
    ) -> Self {
        let mut metadata = serde_json::Map::new();
        metadata.insert("name".to_string(), serde_json::Value::String(name.into()));
        metadata.insert(
            "dataUrl".to_string(),
            serde_json::Value::String(data_url.into()),
        );

        Self {
            kind: "remote_image".to_string(),
            id: id.into(),
            metadata,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSubmissionResult {
    pub turn_id: String,
    #[serde(default)]
    pub accepted: bool,
}

#[async_trait::async_trait]
pub trait AgentSubmissionPort: Send + Sync {
    async fn create_session(
        &self,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult>;

    async fn submit_message(
        &self,
        request: AgentSubmissionRequest,
    ) -> PortResult<AgentSubmissionResult>;

    async fn resolve_session_agent_type(&self, session_id: &str) -> PortResult<Option<String>>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTurnCancellationRequest {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AgentSubmissionSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTurnCancellationResult {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub requested: bool,
}

#[async_trait::async_trait]
pub trait AgentTurnCancellationPort: Send + Sync {
    async fn cancel_turn(
        &self,
        request: AgentTurnCancellationRequest,
    ) -> PortResult<AgentTurnCancellationResult>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteControlSessionState {
    Idle,
    Processing,
    Error,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlStateRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlStateSnapshot {
    pub session_id: String,
    pub state: RemoteControlSessionState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_turn_id: Option<String>,
    #[serde(default)]
    pub queue_depth: usize,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

#[async_trait::async_trait]
pub trait RemoteControlStatePort: Send + Sync {
    async fn read_remote_control_state(
        &self,
        request: RemoteControlStateRequest,
    ) -> PortResult<Option<RemoteControlStateSnapshot>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEventType {
    TurnStarted,
    TurnCompleted,
    TurnFailed,
    TurnCancelled,
    SessionStateChanged,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEventEnvelope {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AgentSubmissionSource>,
    pub event_type: RuntimeEventType,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[async_trait::async_trait]
pub trait RuntimeEventSink: Send + Sync {
    async fn publish_runtime_event(&self, event: RuntimeEventEnvelope) -> PortResult<()>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
}

#[async_trait::async_trait]
pub trait DynamicToolProvider: Send + Sync {
    async fn list_dynamic_tools(&self) -> PortResult<Vec<DynamicToolDescriptor>>;
}

pub trait ToolDecorator<Tool>: Send + Sync {
    fn decorate(&self, tool: Tool) -> Tool;
}

#[async_trait::async_trait]
pub trait ConfigReadPort: Send + Sync {
    async fn get_config_value(&self, key: &str) -> PortResult<Option<serde_json::Value>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTranscriptRequest {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTranscript {
    pub session_id: String,
    #[serde(default)]
    pub messages: Vec<TranscriptMessage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub content: serde_json::Value,
}

#[async_trait::async_trait]
pub trait SessionTranscriptReader: Send + Sync {
    async fn read_session_transcript(
        &self,
        request: SessionTranscriptRequest,
    ) -> PortResult<SessionTranscript>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DelegationPolicy {
    pub allow_subagent_spawn: bool,
    pub nesting_depth: u8,
}

impl Default for DelegationPolicy {
    fn default() -> Self {
        Self::top_level()
    }
}

impl DelegationPolicy {
    pub fn top_level() -> Self {
        Self {
            allow_subagent_spawn: true,
            nesting_depth: 0,
        }
    }

    pub fn spawn_child(self) -> Self {
        Self {
            allow_subagent_spawn: false,
            nesting_depth: self.nesting_depth.saturating_add(1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentContextMode {
    #[default]
    Fresh,
    Fork,
}

impl SubagentContextMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Fork => "fork",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_error_display_keeps_kind_and_message() {
        let error = PortError::new(PortErrorKind::NotAvailable, "coordinator missing");

        assert_eq!(
            error.to_string(),
            "NotAvailable: coordinator missing".to_string()
        );
    }

    #[test]
    fn agent_submission_request_serializes_with_stable_camel_case() {
        let request = AgentSubmissionRequest {
            session_id: "session_1".to_string(),
            message: "hello".to_string(),
            turn_id: None,
            source: None,
            attachments: Vec::new(),
            metadata: serde_json::Map::new(),
        };

        let json = serde_json::to_value(request).expect("serialize request");

        assert_eq!(json["sessionId"], "session_1");
        assert_eq!(json["message"], "hello");
        assert!(json.get("source").is_none());
        assert!(json.get("attachments").is_none());
    }

    #[test]
    fn agent_submission_request_serializes_source_without_changing_field_case() {
        let request = AgentSubmissionRequest {
            session_id: "session_1".to_string(),
            message: "hello".to_string(),
            turn_id: None,
            source: Some(AgentSubmissionSource::RemoteRelay),
            attachments: Vec::new(),
            metadata: serde_json::Map::new(),
        };

        let json = serde_json::to_value(request).expect("serialize request");

        assert_eq!(json["source"], "remote_relay");
        assert!(json.get("turnId").is_none());
    }

    #[test]
    fn dialog_trigger_source_reuses_agent_submission_source_contract() {
        let json = serde_json::to_value(DialogTriggerSource::Cli)
            .expect("serialize dialog trigger source");

        assert_eq!(json, serde_json::json!("cli"));
    }

    #[test]
    fn dialog_submission_policy_preserves_current_surface_queue_defaults() {
        let remote = DialogSubmissionPolicy::for_source(DialogTriggerSource::RemoteRelay);
        assert_eq!(remote.queue_priority, DialogQueuePriority::Normal);
        assert!(remote.skip_tool_confirmation);

        let bot = DialogSubmissionPolicy::for_source(DialogTriggerSource::Bot);
        assert_eq!(bot.queue_priority, DialogQueuePriority::Normal);
        assert!(bot.skip_tool_confirmation);

        let agent_session = DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession);
        assert_eq!(agent_session.queue_priority, DialogQueuePriority::Low);
        assert!(agent_session.skip_tool_confirmation);

        let cli = DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli);
        assert_eq!(cli.queue_priority, DialogQueuePriority::Normal);
        assert!(!cli.skip_tool_confirmation);
    }

    #[test]
    fn dialog_submit_outcome_preserves_started_and_queued_fields() {
        let started = DialogSubmitOutcome::Started {
            session_id: "session_1".to_string(),
            turn_id: "turn_1".to_string(),
        };
        let queued = DialogSubmitOutcome::Queued {
            session_id: "session_1".to_string(),
            turn_id: "turn_2".to_string(),
        };

        assert_eq!(
            started,
            DialogSubmitOutcome::Started {
                session_id: "session_1".to_string(),
                turn_id: "turn_1".to_string(),
            }
        );
        assert_ne!(started, queued);
    }

    #[test]
    fn agent_submission_request_serializes_explicit_turn_id_contract() {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "turnId".to_string(),
            serde_json::Value::String("legacy_metadata_turn".to_string()),
        );
        let request = AgentSubmissionRequest {
            session_id: "session_1".to_string(),
            message: "hello".to_string(),
            turn_id: Some("explicit_turn".to_string()),
            source: Some(AgentSubmissionSource::RemoteRelay),
            attachments: Vec::new(),
            metadata,
        };

        let json = serde_json::to_value(request).expect("serialize request");

        assert_eq!(json["turnId"], "explicit_turn");
        assert_eq!(json["metadata"]["turnId"], "legacy_metadata_turn");
    }

    #[test]
    fn remote_image_attachment_serializes_portable_metadata_contract() {
        let attachment =
            AgentInputAttachment::remote_image("image-1", "clip.png", "data:image/png;base64,abc");

        let json = serde_json::to_value(attachment).expect("serialize attachment");

        assert_eq!(json["kind"], "remote_image");
        assert_eq!(json["id"], "image-1");
        assert_eq!(json["metadata"]["name"], "clip.png");
        assert_eq!(json["metadata"]["dataUrl"], "data:image/png;base64,abc");
    }

    #[test]
    fn agent_turn_cancellation_request_serializes_current_contract() {
        let request = AgentTurnCancellationRequest {
            session_id: "session_1".to_string(),
            turn_id: Some("turn_1".to_string()),
            source: Some(AgentSubmissionSource::Bot),
            reason: Some("user_cancelled".to_string()),
            wait_timeout_ms: Some(1500),
        };

        let json = serde_json::to_value(request).expect("serialize cancel request");

        assert_eq!(json["sessionId"], "session_1");
        assert_eq!(json["turnId"], "turn_1");
        assert_eq!(json["source"], "bot");
        assert_eq!(json["reason"], "user_cancelled");
        assert_eq!(json["waitTimeoutMs"], 1500);
    }

    #[test]
    fn remote_control_state_snapshot_serializes_active_turn_contract() {
        let snapshot = RemoteControlStateSnapshot {
            session_id: "session_1".to_string(),
            state: RemoteControlSessionState::Processing,
            active_turn_id: Some("turn_1".to_string()),
            queue_depth: 2,
            metadata: serde_json::Map::new(),
        };

        let json = serde_json::to_value(snapshot).expect("serialize state snapshot");

        assert_eq!(json["sessionId"], "session_1");
        assert_eq!(json["state"], "processing");
        assert_eq!(json["activeTurnId"], "turn_1");
        assert_eq!(json["queueDepth"], 2);
    }

    #[test]
    fn runtime_event_envelope_serializes_observational_surface_facts() {
        let event = RuntimeEventEnvelope {
            session_id: "session_1".to_string(),
            turn_id: Some("turn_1".to_string()),
            source: Some(AgentSubmissionSource::RemoteRelay),
            event_type: RuntimeEventType::TurnCancelled,
            payload: serde_json::json!({ "reason": "user_cancelled" }),
        };

        let json = serde_json::to_value(event).expect("serialize event");

        assert_eq!(json["sessionId"], "session_1");
        assert_eq!(json["turnId"], "turn_1");
        assert_eq!(json["source"], "remote_relay");
        assert_eq!(json["eventType"], "turn_cancelled");
        assert_eq!(json["payload"]["reason"], "user_cancelled");
    }

    #[test]
    fn session_transcript_request_serializes_turn_id_contract() {
        let request = SessionTranscriptRequest {
            session_id: "session_1".to_string(),
            turn_id: Some("turn_1".to_string()),
        };

        let json = serde_json::to_value(request).expect("serialize transcript request");

        assert_eq!(json["sessionId"], "session_1");
        assert_eq!(json["turnId"], "turn_1");
        assert!(json.get("fromTurnId").is_none());
    }

    #[test]
    fn dynamic_tool_descriptor_serializes_current_wire_shape() {
        let descriptor = DynamicToolDescriptor {
            name: "external_search".to_string(),
            description: "Search external docs".to_string(),
            input_schema: serde_json::json!({ "type": "object" }),
            provider_id: Some("provider-a".to_string()),
        };

        let json = serde_json::to_value(descriptor).expect("serialize descriptor");

        assert_eq!(json["name"], "external_search");
        assert_eq!(json["description"], "Search external docs");
        assert_eq!(json["inputSchema"]["type"], "object");
        assert_eq!(json["providerId"], "provider-a");
        assert!(json.get("provider_id").is_none());
    }

    #[test]
    fn subagent_context_mode_preserves_fork_wire_value() {
        assert_eq!(SubagentContextMode::default(), SubagentContextMode::Fresh);
        assert_eq!(SubagentContextMode::Fresh.as_str(), "fresh");
        assert_eq!(SubagentContextMode::Fork.as_str(), "fork");

        let json = serde_json::to_value(SubagentContextMode::Fork)
            .expect("serialize subagent context mode");

        assert_eq!(json, serde_json::json!("fork"));
    }

    #[test]
    fn delegation_policy_child_blocks_recursive_spawn_without_losing_depth() {
        let top_level = DelegationPolicy::top_level();
        assert!(top_level.allow_subagent_spawn);
        assert_eq!(top_level.nesting_depth, 0);

        let child = top_level.spawn_child();

        assert!(!child.allow_subagent_spawn);
        assert_eq!(child.nesting_depth, 1);
        assert_eq!(child.spawn_child().nesting_depth, 2);
    }

    #[test]
    fn dynamic_tool_descriptor_omits_missing_provider_id() {
        let descriptor = DynamicToolDescriptor {
            name: "local_tool".to_string(),
            description: "Local tool".to_string(),
            input_schema: serde_json::json!({ "type": "object" }),
            provider_id: None,
        };

        let json = serde_json::to_value(descriptor).expect("serialize descriptor");

        assert!(json.get("providerId").is_none());
    }
}
