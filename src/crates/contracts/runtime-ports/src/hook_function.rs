use crate::PortResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookFunctionAvailability {
    Available,
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionGeneration {
    pub instance_id: String,
    pub generation_key: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionPluginDeclaration {
    pub spec: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_directory: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionPluginIdentity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub spec: String,
    pub entry: String,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookFunctionContributorOutcome {
    Applied,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionConfigContributor {
    pub plugin: HookFunctionPluginIdentity,
    pub outcome: HookFunctionContributorOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionConfigContribution {
    pub plugin: HookFunctionPluginIdentity,
    pub outcome: HookFunctionContributorOutcome,
    pub config: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookFunctionDiagnosticSeverity {
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionDiagnostic {
    pub severity: HookFunctionDiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookFunctionHookKind {
    ToolExecuteBefore,
    ToolExecuteAfter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionToolRegistration {
    pub registration_id: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<HookFunctionPluginIdentity>,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionRegistrationBatch {
    pub generation: HookFunctionGeneration,
    pub config: Map<String, Value>,
    pub config_contributors: Vec<HookFunctionConfigContributor>,
    pub config_contributions: Vec<HookFunctionConfigContribution>,
    pub diagnostics: Vec<HookFunctionDiagnostic>,
    pub hooks: Vec<HookFunctionHookKind>,
    pub tools: Vec<HookFunctionToolRegistration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionStartRequest {
    pub generation: HookFunctionGeneration,
    pub project_id: String,
    pub project_worktree: String,
    pub project_created_at_ms: u64,
    pub config: Map<String, Value>,
    pub directory: String,
    pub worktree: String,
    pub plugins: Vec<HookFunctionPluginDeclaration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub expected_content_digests: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_review_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionBeforeRequest {
    pub generation: HookFunctionGeneration,
    pub tool_name: String,
    pub session_id: String,
    pub call_id: String,
    pub args: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionBeforeResult {
    pub args: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionAfterOutput {
    pub title: String,
    pub output: String,
    #[serde(default)]
    pub metadata: Map<String, Value>,
}

pub type HookFunctionAfterResult = HookFunctionAfterOutput;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionAfterRequest {
    pub generation: HookFunctionGeneration,
    pub tool_name: String,
    pub session_id: String,
    pub call_id: String,
    pub args: Value,
    pub output: HookFunctionAfterOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionToolContext {
    pub session_id: String,
    pub message_id: String,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionToolRequest {
    pub generation: HookFunctionGeneration,
    pub execution_id: String,
    pub registration_id: String,
    pub args: Value,
    pub context: HookFunctionToolContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionToolAttachment {
    pub mime: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionToolResult {
    pub output: Value,
    pub attachments: Vec<HookFunctionToolAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionReverseMetadata {
    pub generation: HookFunctionGeneration,
    pub execution_id: String,
    pub title: String,
    pub metadata: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionReverseAsk {
    pub generation: HookFunctionGeneration,
    pub execution_id: String,
    pub permission: String,
    pub patterns: Vec<String>,
    pub always: Vec<String>,
    pub metadata: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reply")]
pub enum HookFunctionReverseReply {
    Once,
    Always,
    Reject {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        feedback: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionCancelRequest {
    pub generation: HookFunctionGeneration,
    pub execution_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionCancelResult {
    pub stopped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionDisposeRequest {
    pub generation: HookFunctionGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookFunctionDisposeResult {
    pub closed: bool,
}

#[async_trait]
pub trait HookFunctionRegistrationSink: Send + Sync {
    async fn publish_generation(&self, batch: HookFunctionRegistrationBatch) -> PortResult<()>;
}

#[async_trait]
pub trait HookFunctionReverseSink: Send + Sync {
    async fn metadata(&self, update: HookFunctionReverseMetadata) -> PortResult<()>;
    async fn ask(&self, request: HookFunctionReverseAsk) -> PortResult<HookFunctionReverseReply>;
}

#[async_trait]
pub trait HookFunctionRuntime: Send + Sync {
    fn availability(&self) -> HookFunctionAvailability;

    async fn start(
        &self,
        request: HookFunctionStartRequest,
        registrations: Arc<dyn HookFunctionRegistrationSink>,
        reverse: Arc<dyn HookFunctionReverseSink>,
        deadline: Duration,
    ) -> PortResult<HookFunctionGeneration>;

    async fn transform_tool_before(
        &self,
        request: HookFunctionBeforeRequest,
        deadline: Duration,
    ) -> PortResult<HookFunctionBeforeResult>;

    async fn execute_tool(
        &self,
        request: HookFunctionToolRequest,
        deadline: Duration,
    ) -> PortResult<HookFunctionToolResult>;

    async fn transform_tool_after(
        &self,
        request: HookFunctionAfterRequest,
        deadline: Duration,
    ) -> PortResult<HookFunctionAfterResult>;

    async fn cancel(
        &self,
        request: HookFunctionCancelRequest,
        deadline: Duration,
    ) -> PortResult<HookFunctionCancelResult>;

    async fn dispose(
        &self,
        request: HookFunctionDisposeRequest,
        deadline: Duration,
    ) -> PortResult<HookFunctionDisposeResult>;
}
