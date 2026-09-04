//! Permission-domain App Server wire schemas.

#[cfg(feature = "rpc")]
use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use openbitfun_product_domains::tool_permissions::{
    PermissionAuditRecord, PermissionGrant, PermissionGrantKey, PermissionReply,
};
use serde::{Deserialize, Serialize};

pub type RespondPermissionMessage = super::agent::RespondPermissionRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "agent/respondPermissionBatch", response = RespondPermissionBatchResponse))]
pub struct RespondPermissionBatchMessage {
    pub request_id: String,
    pub reply: PermissionReply,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
pub struct RespondPermissionBatchResponse {
    pub request_ids: Vec<String>,
}

pub type ListPendingPermissionRequestsMessage = super::agent::PendingPermissionsRequest;
pub type ListPendingPermissionRequestsResponse = super::agent::PendingPermissionsResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "agent/listProjectPermissionGrants", response = ListProjectPermissionGrantsResponse))]
pub struct ListProjectPermissionGrantsMessage {
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ListProjectPermissionGrantsResponse {
    pub grants: Vec<PermissionGrant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "agent/removeProjectPermissionGrant", response = RemoveProjectPermissionGrantResponse))]
pub struct RemoveProjectPermissionGrantMessage(pub PermissionGrantKey);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RemoveProjectPermissionGrantResponse {
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "agent/clearProjectPermissionGrants", response = ClearProjectPermissionGrantsResponse))]
pub struct ClearProjectPermissionGrantsMessage {
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
pub struct ClearProjectPermissionGrantsResponse {
    pub cleared: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "agent/listProjectPermissionAudit", response = ListProjectPermissionAuditResponse))]
pub struct ListProjectPermissionAuditMessage {
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
pub struct ListProjectPermissionAuditResponse {
    pub records: Vec<PermissionAuditRecord>,
}
