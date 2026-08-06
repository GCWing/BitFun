//! Configuration wire contracts shared by App Server hosts and clients.
//!
//! These payloads intentionally contain only wire-owned data. Server adapters
//! translate them to the configuration service's domain request/result types.

use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SaveCloudSpeechConfigRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_id: Option<String>,
    pub preset: String,
    pub name: String,
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_url: Option<String>,
    pub model_name: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[request(method = "config/saveCloudSpeechConfig", response = SaveCloudSpeechConfigResponse)]
pub struct SaveCloudSpeechConfigMessage {
    pub request: SaveCloudSpeechConfigRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SaveCloudSpeechConfigResult {
    pub model_id: String,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SaveCloudSpeechConfigResponse(pub SaveCloudSpeechConfigResult);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "config/validateConfig", response = ValidateConfigResponse)]
pub struct ValidateConfigMessage {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct ValidateConfigResponse(pub serde_json::Value);

#[cfg(test)]
mod tests {
    use super::SaveCloudSpeechConfigRequest;

    #[test]
    fn cloud_speech_request_uses_the_camel_case_wire_shape() {
        let value = serde_json::to_value(SaveCloudSpeechConfigRequest {
            config_id: Some("speech".to_string()),
            preset: "custom".to_string(),
            name: "Speech".to_string(),
            base_url: "https://example.com/v1".to_string(),
            request_url: None,
            model_name: "speech-model".to_string(),
            api_key: "secret".to_string(),
        })
        .expect("request should serialize");

        assert_eq!(value["configId"], "speech");
        assert_eq!(value["baseUrl"], "https://example.com/v1");
        assert_eq!(value["modelName"], "speech-model");
        assert!(value.get("requestUrl").is_none());
    }
}
