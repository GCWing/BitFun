//! Provider-neutral process boundary for executable script tools.

use crate::PortResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ScriptToolRuntimeAvailability {
    Available { executable: String, version: String },
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScriptToolLoadRequest {
    pub target_id: String,
    pub revision: String,
    pub module_source: String,
    pub module_url: String,
    pub working_directory: String,
    pub expected_tools: Vec<ScriptToolExpectedExport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScriptToolExpectedExport {
    pub export_name: String,
    pub tool_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScriptToolDescriptor {
    pub export_name: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScriptToolLoadResponse {
    pub target_id: String,
    pub revision: String,
    pub tools: Vec<ScriptToolDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScriptToolInvokeRequest {
    pub target_id: String,
    pub revision: String,
    pub export_name: String,
    pub operation_id: String,
    pub arguments: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScriptToolInvokeResponse {
    pub output: String,
}

#[async_trait]
pub trait ScriptToolRuntime: Send + Sync {
    async fn availability(&self) -> ScriptToolRuntimeAvailability;

    async fn is_loaded(&self, target_id: &str) -> bool;

    /// Waits until the currently loaded target process exits. Implementations
    /// must not report a replaced or explicitly disposed generation as an
    /// unexpected exit for the new target.
    async fn wait_until_unloaded(&self, target_id: &str) -> PortResult<()>;

    async fn load(&self, request: ScriptToolLoadRequest) -> PortResult<ScriptToolLoadResponse>;

    async fn invoke(
        &self,
        request: ScriptToolInvokeRequest,
    ) -> PortResult<ScriptToolInvokeResponse>;

    async fn cancel(&self, target_id: &str, operation_id: &str) -> PortResult<()>;

    async fn dispose(&self, target_id: &str) -> PortResult<()>;
}
