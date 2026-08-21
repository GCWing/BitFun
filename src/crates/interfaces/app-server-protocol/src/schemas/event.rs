//! Authoritative, sequenced App Server event schemas.

#[cfg(feature = "rpc")]
use agent_client_protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use bitfun_events::AgenticEventEnvelope;
use bitfun_product_domains::tool_permissions::{PermissionRequest, PermissionRequestEvent};
use serde::{Deserialize, Serialize};

/// Browser-facing projected runtime or permission event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcNotification))]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "rpc", notification(method = "agent/frontendEvent"))]
pub struct FrontendEventNotification {
    pub event: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventStream {
    Agent,
    Permission,
    Config,
    ExternalSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventCursor {
    pub connection_id: String,
    pub stream: EventStream,
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcNotification))]
#[cfg_attr(feature = "rpc", notification(method = "agent/event"))]
#[serde(rename_all = "camelCase")]
pub struct AgentEventNotification {
    pub cursor: EventCursor,
    pub event: AgenticEventEnvelope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcNotification))]
#[cfg_attr(feature = "rpc", notification(method = "agent/permissionEvent"))]
#[serde(rename_all = "camelCase")]
pub struct PermissionEventNotification {
    pub cursor: EventCursor,
    pub event: PermissionRequestEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcNotification))]
#[cfg_attr(feature = "rpc", notification(method = "config/event"))]
#[serde(rename_all = "camelCase")]
pub struct ConfigEventNotification {
    pub cursor: EventCursor,
    pub event: ConfigUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcNotification))]
#[cfg_attr(feature = "rpc", notification(method = "app/eventStreamState"))]
#[serde(rename_all = "camelCase")]
pub struct EventStreamStateNotification {
    pub cursor: EventCursor,
    pub stream: EventStream,
    pub state: EventStreamState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missed: Option<u64>,
    pub resync: ResyncDirective,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventStreamState {
    Lagged,
    Closed,
    Invalidated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResyncDirective {
    pub method: String,
    pub snapshot_available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "app/syncEvents", response = SyncEventsResponse))]
#[serde(rename_all = "camelCase")]
pub struct SyncEventsRequest {
    pub streams: Vec<EventStream>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
#[serde(rename_all = "camelCase")]
pub struct SyncEventsResponse {
    pub cursors: Vec<EventCursor>,
    pub pending_permissions: Vec<PermissionRequest>,
    pub agent_snapshot_available: bool,
    pub config_snapshot_available: bool,
    #[serde(default)]
    pub external_source_snapshot_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ConfigUpdate {
    ModelConfigurationUpdated,
    AiModelUpdated {
        model_id: String,
        model_name: String,
    },
    DefaultAiModelUpdated {
        model_id: String,
        model_name: String,
    },
    AppearanceUpdated {
        appearance_id: String,
    },
    EditorUpdated,
    TerminalUpdated,
    WorkspaceUpdated,
    AppUpdated,
    ConfigReloaded,
    ReasoningCatalogUpdated,
    LogLevelUpdated {
        new_level: String,
    },
    LoggingSensitiveDiagnosticsUpdated {
        include_sensitive_diagnostics: bool,
    },
    ModelsReconciled {
        invalidated_model_ids: Vec<String>,
        default_models_changed: bool,
        task_models_changed: bool,
        agent_model_defaults_changed: bool,
    },
}
