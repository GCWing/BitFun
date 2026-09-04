//! Workspace-domain App Server wire schemas.

#[cfg(feature = "rpc")]
use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use openbitfun_runtime_ports::{
    AgentMessageWorkspaceReferencesRequest, AgentWorkspaceReference,
    AgentWorkspaceReferenceSearchRequest, AgentWorkspaceReferenceSearchResult,
    WorkspaceDiffSnapshot,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "workspace/diff", response = WorkspaceDiffResponse))]
pub struct WorkspaceDiffRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
pub struct WorkspaceDiffResponse(pub WorkspaceDiffSnapshot);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "workspace/searchReferences", response = SearchWorkspaceReferencesResponse))]
pub struct SearchWorkspaceReferencesRequest(pub AgentWorkspaceReferenceSearchRequest);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
pub struct SearchWorkspaceReferencesResponse(pub AgentWorkspaceReferenceSearchResult);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "workspace/messageReferences", response = MessageReferencesResponse))]
pub struct MessageReferencesRequest(pub AgentMessageWorkspaceReferencesRequest);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
pub struct MessageReferencesResponse(pub Vec<AgentWorkspaceReference>);
