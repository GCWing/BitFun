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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AcpClientPermissionMode {
    Ask,
    AllowOnce,
    RejectOnce,
}

impl Default for AcpClientPermissionMode {
    fn default() -> Self {
        Self::Ask
    }
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
