//! Skill-domain App Server wire schemas.

use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use serde::{Deserialize, Serialize};

pub use bitfun_product_domains::agent_catalog::SkillSummary;

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "skill/list", response = ListSkillsResponse)]
#[serde(rename_all = "camelCase")]
pub struct ListSkillsRequest {
    pub workspace_path: String,
    pub mode_id: String,
    #[serde(default)]
    pub manageable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct ListSkillsResponse {
    pub skills: Vec<SkillSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "skill/setEnabled", response = SetSkillEnabledResponse)]
#[serde(rename_all = "camelCase")]
pub struct SetSkillEnabledRequest {
    pub workspace_path: String,
    pub mode_id: String,
    pub skill_key: String,
    pub enabled: bool,
    pub default_enabled: bool,
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct SetSkillEnabledResponse {}
