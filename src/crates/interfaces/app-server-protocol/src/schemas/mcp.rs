//! MCP-domain App Server wire schemas.
//!
//! Mutation payloads can contain credentials and other sensitive values. Their
//! custom `Debug` implementations expose only configuration metadata.

use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use serde::{Deserialize, Serialize};

macro_rules! unit_response {
    ($name:ident) => {
        #[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
        pub struct $name {}
    };
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "mcp/list", response = ListMcpServersResponse)]
#[serde(rename_all = "camelCase")]
pub struct ListMcpServersRequest {
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct ListMcpServersResponse {
    pub servers: Vec<McpServerSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSummary {
    pub id: String,
    pub name: String,
    pub server_type: String,
    pub status: String,
    pub tool_count: usize,
    pub source_label: String,
    pub external: bool,
    pub detail: String,
    pub action: McpServerAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum McpServerAction {
    NativeToggle,
    ReadOnly {
        reason: String,
    },
    ExternalDecision {
        candidate_id: String,
        decision_key: String,
        approved: bool,
        expected_mcp_generation: u64,
        expected_preference_revision: u64,
    },
    ConflictChoice {
        conflict_key: String,
        candidate_id: String,
        approve_external: bool,
        expected_mcp_generation: u64,
        expected_preference_revision: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "mcp/toggle", response = ToggleMcpServerResponse)]
pub struct ToggleMcpServerRequest {
    pub server_id: String,
}

unit_response!(ToggleMcpServerResponse);

#[derive(Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "mcp/add", response = AddMcpServerResponse)]
pub struct AddMcpServerRequest {
    pub name: String,
    pub config: McpServerMutation,
}

impl std::fmt::Debug for AddMcpServerRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AddMcpServerRequest")
            .field("name", &self.name)
            .field("config", &self.config)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerMutation {
    pub transport: McpTransport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default = "default_true")]
    pub auto_start: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xaa: Option<serde_json::Value>,
}

impl std::fmt::Debug for McpServerMutation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpServerMutation")
            .field("transport", &self.transport)
            .field("command_configured", &self.command.is_some())
            .field("argument_count", &self.args.len())
            .field("environment_keys", &self.env.keys().collect::<Vec<_>>())
            .field("header_names", &self.headers.keys().collect::<Vec<_>>())
            .field("url_configured", &self.url.is_some())
            .field("auto_start", &self.auto_start)
            .field("enabled", &self.enabled)
            .field("oauth_configured", &self.oauth.is_some())
            .field("xaa_configured", &self.xaa.is_some())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpTransport {
    Stdio,
    Sse,
    StreamableHttp,
}

fn default_true() -> bool {
    true
}

unit_response!(AddMcpServerResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "mcp/delete", response = DeleteMcpServerResponse)]
pub struct DeleteMcpServerRequest {
    pub server_id: String,
}

unit_response!(DeleteMcpServerResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "mcp/externalDecision", response = ExternalMcpDecisionResponse)]
#[serde(rename_all = "camelCase")]
pub struct ExternalMcpDecisionRequest {
    pub workspace_path: String,
    pub candidate_id: String,
    pub decision_key: String,
    pub approved: bool,
    pub expected_mcp_generation: u64,
    pub expected_preference_revision: u64,
}

unit_response!(ExternalMcpDecisionResponse);

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "mcp/conflictChoice", response = McpConflictChoiceResponse)]
#[serde(rename_all = "camelCase")]
pub struct McpConflictChoiceRequest {
    pub workspace_path: String,
    pub conflict_key: String,
    pub candidate_id: String,
    pub approve_external: bool,
    pub expected_mcp_generation: u64,
    pub expected_preference_revision: u64,
}

unit_response!(McpConflictChoiceResponse);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn mcp_mutation_debug_redacts_arguments_and_secret_values() {
        let request = AddMcpServerRequest {
            name: "server".to_string(),
            config: McpServerMutation {
                transport: McpTransport::Stdio,
                command: Some("secret-command".to_string()),
                args: vec!["--token=secret-argument".to_string()],
                env: HashMap::from([("API_TOKEN".to_string(), "secret-env".to_string())]),
                headers: HashMap::from([(
                    "Authorization".to_string(),
                    "Bearer secret-header".to_string(),
                )]),
                url: Some("https://example.com?token=secret-url".to_string()),
                auto_start: true,
                enabled: true,
                oauth: Some(serde_json::json!({"clientSecret": "secret-oauth"})),
                xaa: Some(serde_json::json!({"token": "secret-xaa"})),
            },
        };

        let debug = format!("{request:?}");
        for secret in [
            "secret-command",
            "secret-argument",
            "secret-env",
            "secret-header",
            "secret-url",
            "secret-oauth",
            "secret-xaa",
        ] {
            assert!(!debug.contains(secret), "debug leaked {secret}");
        }
        assert!(debug.contains("API_TOKEN"));
        assert!(debug.contains("Authorization"));
    }
}
