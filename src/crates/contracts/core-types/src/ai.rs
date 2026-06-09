use crate::ToolImageAttachment;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningMode {
    #[default]
    Default,
    Enabled,
    Disabled,
    Adaptive,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    pub name: String,
    pub base_url: String,
    pub request_url: String,
    pub api_key: String,
    pub model: String,
    pub format: String,
    pub context_window: u32,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub reasoning_mode: ReasoningMode,
    pub inline_think_in_text: bool,
    pub custom_headers: Option<HashMap<String, String>>,
    pub custom_headers_mode: Option<String>,
    pub skip_ssl_verify: bool,
    pub reasoning_effort: Option<String>,
    pub thinking_budget_tokens: Option<u32>,
    pub custom_request_body: Option<Value>,
    pub custom_request_body_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_arguments: Option<String>,
}

impl ToolCall {
    pub fn serialized_arguments(&self) -> String {
        self.raw_arguments
            .as_deref()
            .filter(|raw| serde_json::from_str::<Value>(raw).is_ok())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                serde_json::to_string(&self.arguments).unwrap_or_else(|_| "{}".to_string())
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallConfirmationDetails {
    pub request: ToolCallRequestInfo,
    #[serde(rename = "type")]
    pub confirmation_type: String,
    pub message: Option<String>,
    pub file_diff: Option<String>,
    pub file_name: Option<String>,
    pub original_content: Option<String>,
    pub new_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequestInfo {
    pub call_id: String,
    pub name: String,
    pub args: HashMap<String, Value>,
    pub is_client_initiated: bool,
    pub prompt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResponseInfo {
    pub call_id: String,
    pub response_parts: Value,
    pub result_display: Option<String>,
    pub error: Option<String>,
    pub error_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_image_attachments: Option<Vec<ToolImageAttachment>>,
}

impl Message {
    pub fn user(content: String) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(content),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        }
    }

    pub fn assistant(content: String) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Some(content),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        }
    }

    pub fn assistant_with_tools(tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: None,
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        }
    }

    pub fn system(content: String) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(content),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionTestMessageCode {
    ToolCallsNotDetected,
    ImageInputCheckFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTestResult {
    pub success: bool,
    pub response_time_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_response: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_code: Option<ConnectionTestMessageCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteModelInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}
