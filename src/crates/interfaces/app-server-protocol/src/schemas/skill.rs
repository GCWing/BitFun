//! Skill-domain App Server wire schemas.

use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub key: String,
    pub name: String,
    pub description: String,
    pub level: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_slot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_label: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub selected_for_runtime: bool,
    #[serde(default)]
    pub default_enabled: bool,
    #[serde(default)]
    pub is_shadowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadowed_by_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
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
