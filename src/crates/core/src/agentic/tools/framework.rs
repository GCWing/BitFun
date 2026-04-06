//! Tool framework - Tool interface definition and execution context
use crate::util::types::ToolImageAttachment;
use super::image_context::ImageContextProviderRef;
use super::pipeline::SubagentParentInfo;
use crate::agentic::workspace::WorkspaceServices;
use crate::agentic::WorkspaceBinding;
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use tokio_util::sync::CancellationToken;

/// Tool use context
#[derive(Debug, Clone)]
pub struct ToolUseContext {
    pub tool_call_id: Option<String>,
    pub message_id: Option<String>,
    pub agent_type: Option<String>,
    pub session_id: Option<String>,
    pub dialog_turn_id: Option<String>,
    pub workspace: Option<WorkspaceBinding>,
    pub safe_mode: Option<bool>,
    pub abort_controller: Option<String>,
    pub read_file_timestamps: HashMap<String, u64>,
    pub options: Option<ToolOptions>,
    pub response_state: Option<ResponseState>,
    /// Image context provider (dependency injection)
    pub image_context_provider: Option<ImageContextProviderRef>,
    /// Desktop automation (Computer use); only set in BitFun desktop.
    pub computer_use_host: Option<crate::agentic::tools::computer_use_host::ComputerUseHostRef>,
    pub subagent_parent_info: Option<SubagentParentInfo>,
    // Cancel tool execution more timely, especially for tools like TaskTool that need to run for a long time
    pub cancellation_token: Option<CancellationToken>,
    /// Workspace I/O services (filesystem + shell) — use these instead of
    /// checking `get_remote_workspace_manager()` inside individual tools.
    pub workspace_services: Option<WorkspaceServices>,
}

impl ToolUseContext {
    pub fn workspace_root(&self) -> Option<&Path> {
        self.workspace.as_ref().map(|binding| binding.root_path())
    }

    pub fn is_remote(&self) -> bool {
        self.workspace
            .as_ref()
            .map(|ws| ws.is_remote())
            .unwrap_or(false)
    }

    pub fn ws_fs(&self) -> Option<&dyn crate::agentic::workspace::WorkspaceFileSystem> {
        self.workspace_services.as_ref().map(|s| s.fs.as_ref())
    }

    pub fn ws_shell(&self) -> Option<&dyn crate::agentic::workspace::WorkspaceShell> {
        self.workspace_services.as_ref().map(|s| s.shell.as_ref())
    }

    /// Whether the session primary model accepts image inputs (from tool-definition / pipeline context).
    /// Defaults to **true** when unset (e.g. API listings without model metadata).
    pub fn primary_model_supports_image_understanding(&self) -> bool {
        self.options
            .as_ref()
            .and_then(|o| o.custom_data.as_ref())
            .and_then(|m| m.get("primary_model_supports_image_understanding"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    /// Resolve a user or model-supplied path for file/shell tools. Uses POSIX semantics when the
    /// workspace is remote SSH so Windows-hosted clients still resolve `/home/...` correctly.
    pub fn resolve_workspace_tool_path(&self, path: &str) -> BitFunResult<String> {
        let workspace_root_owned = self
            .workspace
            .as_ref()
            .map(|w| w.root_path_string());
        crate::agentic::tools::workspace_paths::resolve_workspace_tool_path(
            path,
            workspace_root_owned.as_deref(),
            self.is_remote(),
        )
    }

    /// Whether `path` is absolute for the active workspace (POSIX `/` for remote SSH).
    pub fn workspace_path_is_effectively_absolute(&self, path: &str) -> bool {
        if self.is_remote() {
            crate::agentic::tools::workspace_paths::posix_style_path_is_absolute(path)
        } else {
            Path::new(path).is_absolute()
        }
    }
}

/// Tool options
#[derive(Debug, Clone)]
pub struct ToolOptions {
    pub commands: Vec<Value>,
    pub tools: Vec<String>,
    pub verbose: Option<bool>,
    pub slow_and_capable_model: Option<String>,
    pub safe_mode: Option<bool>,
    pub fork_number: Option<u32>,
    pub message_log_name: Option<String>,
    pub max_thinking_tokens: Option<u32>,
    pub is_koding_request: Option<bool>,
    pub koding_context: Option<String>,
    pub is_custom_command: Option<bool>,
    /// Extended data fields, for passing extra context information
    pub custom_data: Option<HashMap<String, Value>>,
}

/// Response state - for model state management like GPT-5
#[derive(Debug, Clone)]
pub struct ResponseState {
    pub previous_response_id: Option<String>,
    pub conversation_id: Option<String>,
}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub result: bool,
    pub message: Option<String>,
    pub error_code: Option<i32>,
    pub meta: Option<Value>,
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolResult {
    #[serde(rename = "result")]
    Result {
        data: Value,
        #[serde(default)]
        result_for_assistant: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image_attachments: Option<Vec<ToolImageAttachment>>,
    },
    #[serde(rename = "progress")]
    Progress {
        content: Value,
        normalized_messages: Option<Vec<Value>>,
        tools: Option<Vec<String>>,
    },
    #[serde(rename = "stream_chunk")]
    StreamChunk {
        data: Value,
        chunk_index: usize,
        is_final: bool,
    },
}

impl ToolResult {
    /// Get content (for display)
    pub fn content(&self) -> Value {
        match self {
            ToolResult::Result { data, .. } => data.clone(),
            ToolResult::Progress { content, .. } => content.clone(),
            ToolResult::StreamChunk { data, .. } => data.clone(),
        }
    }

    /// Standard tool success without images.
    pub fn ok(data: Value, result_for_assistant: Option<String>) -> Self {
        Self::Result {
            data,
            result_for_assistant,
            image_attachments: None,
        }
    }

    /// Tool success with optional images for multimodal tool results (Anthropic).
    pub fn ok_with_images(
        data: Value,
        result_for_assistant: Option<String>,
        image_attachments: Vec<ToolImageAttachment>,
    ) -> Self {
        Self::Result {
            data,
            result_for_assistant,
            image_attachments: Some(image_attachments),
        }
    }
}

/// Tool trait
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name
    fn name(&self) -> &str;

    /// Tool description
    async fn description(&self) -> BitFunResult<String>;

    /// Tool description with execution context.
    async fn description_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        self.description().await
    }

    /// Input mode definition - using JSON Schema
    fn input_schema(&self) -> Value;

    /// JSON Schema sent to the model (may depend on app language or other runtime config).
    /// Default: same as [`input_schema`].
    async fn input_schema_for_model(&self) -> Value {
        self.input_schema()
    }

    /// JSON Schema for the model when tool listing has a [`ToolUseContext`] (e.g. primary model vision capability).
    /// Default: ignores context and delegates to [`input_schema_for_model`].
    async fn input_schema_for_model_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> Value {
        let _ = context;
        self.input_schema_for_model().await
    }

    /// Input JSON Schema - optional extra schema
    fn input_json_schema(&self) -> Option<Value> {
        None
    }

    /// MCP Apps: URI of UI resource (ui://) declared in tool metadata. Used when tool result
    /// does not contain a resource - the host fetches from this pre-declared URI.
    fn ui_resource_uri(&self) -> Option<String> {
        None
    }

    /// User friendly name
    fn user_facing_name(&self) -> String {
        self.name().to_string()
    }

    /// Whether to enable
    async fn is_enabled(&self) -> bool {
        true
    }

    /// Whether to be readonly
    fn is_readonly(&self) -> bool {
        false
    }

    /// Whether to be concurrency safe
    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        self.is_readonly()
    }

    /// Whether to need permissions
    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        !self.is_readonly()
    }

    /// Whether to support streaming output
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Validate input
    async fn validate_input(
        &self,
        _input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    /// Render result for assistant
    fn render_result_for_assistant(&self, _output: &Value) -> String {
        "Tool result".to_string()
    }

    /// Render tool use message
    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        format!("Using {}: {}", self.name(), input)
    }

    /// Render tool use rejected message
    fn render_tool_use_rejected_message(&self) -> String {
        format!("{} tool use was rejected", self.name())
    }

    /// Render tool result message
    fn render_tool_result_message(&self, _output: &Value) -> String {
        format!("{} completed", self.name())
    }

    /// Call tool - return async generator
    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>>;

    async fn call(&self, input: &Value, context: &ToolUseContext) -> BitFunResult<Vec<ToolResult>> {
        if let Some(cancellation_token) = context.cancellation_token.as_ref() {
            tokio::select! {
                result = self.call_impl(input, context) => {
                    result
                }

                _ = cancellation_token.cancelled() => {
                    Err(crate::util::errors::BitFunError::Cancelled("Tool execution cancelled".to_string()))
                }
            }
        } else {
            self.call_impl(input, context).await
        }
    }
}

/// Tool render options
#[derive(Debug, Clone)]
pub struct ToolRenderOptions {
    pub verbose: bool,
}
