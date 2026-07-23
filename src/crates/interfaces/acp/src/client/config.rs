use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientConfigFile {
    #[serde(default)]
    pub acp_clients: HashMap<String, AcpClientConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientConfig {
    #[serde(default)]
    pub name: Option<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub readonly: bool,
    #[serde(default)]
    pub permission_mode: AcpClientPermissionMode,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AcpClientPermissionMode {
    #[default]
    Ask,
    AllowOnce,
    RejectOnce,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientInfo {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
    pub readonly: bool,
    pub permission_mode: AcpClientPermissionMode,
    pub status: AcpClientStatus,
    pub tool_name: String,
    pub session_count: usize,
    pub category: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpClientRequirementProbe {
    pub id: String,
    pub tool: AcpRequirementProbeItem,
    #[serde(default)]
    pub adapter: Option<AcpRequirementProbeItem>,
    pub runnable: bool,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAcpClientRequirementSnapshot {
    pub connection_id: String,
    pub last_probed_at: u64,
    #[serde(default)]
    pub probes: Vec<AcpClientRequirementProbe>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpRequirementProbeItem {
    pub name: String,
    pub installed: bool,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AcpClientStatus {
    Configured,
    Starting,
    Running,
    Stopped,
    Failed,
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::AcpClientPermissionMode;

    #[test]
    fn permission_mode_default_remains_ask_on_the_wire() {
        let mode = AcpClientPermissionMode::default();

        assert_eq!(mode, AcpClientPermissionMode::Ask);
        assert_eq!(serde_json::to_string(&mode).unwrap(), "\"ask\"");
    }
}
