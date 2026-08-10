use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use bitfun_agent_runtime::sdk::{
    PermissionAuditRecord, PermissionGrant, PermissionGrantKey, PermissionReply, PermissionRequest,
};
use serde::{Deserialize, Serialize};

/// `agent/respondPermission` request body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "agent/respondPermission", response = RespondPermissionResponse)]
pub struct RespondPermissionMessage {
    pub request_id: String,
    pub reply: PermissionReply,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RespondPermissionResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "agent/respondPermissionBatch", response = RespondPermissionBatchResponse)]
pub struct RespondPermissionBatchMessage {
    pub request_id: String,
    pub reply: PermissionReply,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RespondPermissionBatchResponse {
    pub request_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "agent/listPendingPermissionRequests", response = ListPendingPermissionRequestsResponse)]
pub struct ListPendingPermissionRequestsMessage {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ListPendingPermissionRequestsResponse {
    pub requests: Vec<PermissionRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "agent/listProjectPermissionGrants", response = ListProjectPermissionGrantsResponse)]
pub struct ListProjectPermissionGrantsMessage {
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ListProjectPermissionGrantsResponse {
    pub grants: Vec<PermissionGrant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(method = "agent/removeProjectPermissionGrant", response = RemoveProjectPermissionGrantResponse)]
pub struct RemoveProjectPermissionGrantMessage(pub PermissionGrantKey);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RemoveProjectPermissionGrantResponse {
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "agent/clearProjectPermissionGrants", response = ClearProjectPermissionGrantsResponse)]
pub struct ClearProjectPermissionGrantsMessage {
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ClearProjectPermissionGrantsResponse {
    pub cleared: usize,
}

pub use bitfun_app_server_protocol::event::PermissionEventNotification;

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "agent/listProjectPermissionAudit", response = ListProjectPermissionAuditResponse)]
pub struct ListProjectPermissionAuditMessage {
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ListProjectPermissionAuditResponse {
    pub records: Vec<PermissionAuditRecord>,
}
