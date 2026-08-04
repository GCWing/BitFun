use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(method = "git/isRepository", response = GitIsRepositoryResponse)]
pub struct GitIsRepositoryMessage(pub GitRepositoryPathRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GitIsRepositoryResponse(pub bool);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(method = "git/getStatus", response = GitGetStatusResponse)]
pub struct GitGetStatusMessage(pub GitRepositoryPathRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GitGetStatusResponse(pub bitfun_core::service::git::GitStatus);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[request(method = "git/getBranches", response = GitGetBranchesResponse)]
pub struct GitGetBranchesMessage(pub GitBranchesRequest);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct GitGetBranchesResponse {
    pub branches: Vec<bitfun_core::service::git::GitBranch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct GitRepositoryPathRequest {
    pub repository_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct GitBranchesRequest {
    pub repository_path: String,
    #[serde(default)]
    pub include_remote: Option<bool>,
}
