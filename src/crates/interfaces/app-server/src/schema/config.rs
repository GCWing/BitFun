use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use serde::{Deserialize, Serialize};

pub use bitfun_app_server_protocol::config::{
    SaveCloudSpeechConfigMessage, SaveCloudSpeechConfigRequest, SaveCloudSpeechConfigResponse,
    SaveCloudSpeechConfigResult, ValidateConfigMessage, ValidateConfigResponse,
};

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "config/getAgentProfileConfigs", response = GetAgentProfileConfigsResponse)]
pub struct GetAgentProfileConfigsMessage {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct GetAgentProfileConfigsResponse {
    pub profiles: std::collections::HashMap<String, bitfun_core::service::config::AgentProfileView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "config/getAgentProfileConfig", response = GetAgentProfileConfigResponse)]
pub struct GetAgentProfileConfigMessage {
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GetAgentProfileConfigResponse(pub bitfun_core::service::config::AgentProfileView);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "config/getModelConfigs", response = GetModelConfigsResponse)]
pub struct GetModelConfigsMessage {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct GetModelConfigsResponse {
    pub models: Vec<bitfun_core::service::config::AIModelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "config/getConfig", response = GetConfigResponse)]
#[serde(rename_all = "camelCase")]
pub struct GetConfigMessage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "skip_if_false")]
    pub skip_retry_on_not_found: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GetConfigResponse(pub serde_json::Value);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "config/getConfigs", response = GetConfigsResponse)]
#[serde(rename_all = "camelCase")]
pub struct GetConfigsMessage {
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "skip_if_false")]
    pub skip_retry_on_not_found: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct GetConfigsResponse {
    pub configs: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "config/setConfig", response = SetConfigResponse)]
#[serde(rename_all = "camelCase")]
pub struct SetConfigMessage {
    pub path: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SetConfigResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "config/setAgentProfileConfig", response = SetAgentProfileConfigResponse)]
#[serde(rename_all = "camelCase")]
pub struct SetAgentProfileConfigMessage {
    pub agent_id: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SetAgentProfileConfigResponse(pub bitfun_core::service::config::AgentProfileView);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "config/resetAgentProfileConfig", response = ResetAgentProfileConfigResponse)]
#[serde(rename_all = "camelCase")]
pub struct ResetAgentProfileConfigMessage {
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ResetAgentProfileConfigResponse(pub bitfun_core::service::config::AgentProfileView);

fn skip_if_false(value: &bool) -> bool {
    !*value
}
