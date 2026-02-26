//! Tool framework - Tool interface definition and execution context
use super::image_context::ImageContextProviderRef;
use super::pipeline::SubagentParentInfo;
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

/// Tool use context
#[derive(Debug, Clone)]
pub struct ToolUseContext {
    pub tool_call_id: Option<String>,
    pub message_id: Option<String>,
    pub agent_type: Option<String>,
    pub session_id: Option<String>,
    pub dialog_turn_id: Option<String>,
    pub safe_mode: Option<bool>,
    pub abort_controller: Option<String>,
    pub read_file_timestamps: HashMap<String, u64>,
    pub options: Option<ToolOptions>,
    pub response_state: Option<ResponseState>,
    /// Image context provider (dependency injection)
    pub image_context_provider: Option<ImageContextProviderRef>,
    pub subagent_parent_info: Option<SubagentParentInfo>,
    // Cancel tool execution more timely, especially for tools like TaskTool that need to run for a long time
    pub cancellation_token: Option<CancellationToken>,
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
        result_for_assistant: Option<String>,
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
}

/// Tool trait
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name
    fn name(&self) -> &str;

    /// Tool description
    async fn description(&self) -> BitFunResult<String>;

    /// Input mode definition - using JSON Schema
    fn input_schema(&self) -> Value;

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

    /// Whether to end conversation turn after calling (CreatePlan)
    fn should_end_turn(&self) -> bool {
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
