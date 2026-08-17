//! MCP-management projections shared by product surfaces.

use serde::{Deserialize, Serialize};

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
