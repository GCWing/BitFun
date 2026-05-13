use std::sync::Arc;

use async_trait::async_trait;
use bitfun_core::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use serde_json::{json, Value};

use super::config::AcpClientConfig;
use super::manager::AcpClientService;

pub struct AcpAgentTool {
    client_id: String,
    config: AcpClientConfig,
    service: Arc<AcpClientService>,
    full_name: String,
}

impl AcpAgentTool {
    pub fn new(client_id: String, config: AcpClientConfig, service: Arc<AcpClientService>) -> Self {
        let full_name = Self::tool_name_for(&client_id);
        Self {
            client_id,
            config,
            service,
            full_name,
        }
    }

    pub fn tool_name_for(client_id: &str) -> String {
        format!("acp__{}__prompt", sanitize_tool_part(client_id))
    }

    fn display_name(&self) -> String {
        self.config
            .name
            .clone()
            .unwrap_or_else(|| self.client_id.clone())
    }
}

#[async_trait]
impl Tool for AcpAgentTool {
    fn name(&self) -> &str {
        &self.full_name
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(format!(
            "Send a prompt to the external ACP agent '{}'. Use this when another local ACP-compatible agent is better suited for a delegated task.",
            self.display_name()
        ))
    }

    fn short_description(&self) -> String {
        format!(
            "Delegate a task to the external ACP agent '{}'.",
            self.display_name()
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The task or question to send to the external ACP agent."
                },
                "workspace_path": {
                    "type": "string",
                    "description": "Optional absolute workspace path. Defaults to the current BitFun workspace."
                },
                "timeout_seconds": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Optional timeout in seconds. Use 0 or omit it to wait without a fixed timeout."
                }
            },
            "required": ["prompt"],
            "additionalProperties": false
        })
    }

    fn user_facing_name(&self) -> String {
        format!("{} (ACP)", self.display_name())
    }

    fn is_readonly(&self) -> bool {
        self.config.readonly
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        !self.config.readonly
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        match input.get("prompt").and_then(|value| value.as_str()) {
            Some(prompt) if !prompt.trim().is_empty() => ValidationResult::default(),
            Some(_) => ValidationResult {
                result: false,
                message: Some("prompt cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            },
            None => ValidationResult {
                result: false,
                message: Some("prompt is required".to_string()),
                error_code: Some(400),
                meta: None,
            },
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let prompt_preview = input
            .get("prompt")
            .and_then(|value| value.as_str())
            .map(truncate_prompt)
            .unwrap_or_else(|| "prompt".to_string());
        format!(
            "Sending ACP prompt to '{}': {}",
            self.display_name(),
            prompt_preview
        )
    }

    fn render_tool_use_rejected_message(&self) -> String {
        format!("ACP prompt to '{}' was rejected", self.display_name())
    }

    fn render_tool_result_message(&self, output: &Value) -> String {
        output
            .get("response")
            .and_then(|value| value.as_str())
            .map(|response| {
                format!(
                    "ACP agent '{}' responded:\n{}",
                    self.display_name(),
                    response
                )
            })
            .unwrap_or_else(|| format!("ACP agent '{}' completed", self.display_name()))
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        output
            .get("response")
            .and_then(|value| value.as_str())
            .unwrap_or("ACP agent completed without text output")
            .to_string()
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let bitfun_session_id = context.session_id.clone().ok_or_else(|| {
            BitFunError::tool("ACP tool requires an active BitFun session".to_string())
        })?;
        let prompt = input
            .get("prompt")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| BitFunError::tool("prompt is required".to_string()))?
            .to_string();

        let workspace_path = input
            .get("workspace_path")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .or_else(|| {
                context
                    .workspace_root()
                    .map(|path| path.to_string_lossy().to_string())
            });
        let timeout_seconds = input
            .get("timeout_seconds")
            .and_then(|value| value.as_u64());

        let response = self
            .service
            .prompt_agent(
                &self.client_id,
                prompt,
                workspace_path,
                None,
                bitfun_session_id,
                None,
                timeout_seconds,
            )
            .await?;

        let data = json!({
            "client_id": self.client_id,
            "response": response,
        });
        Ok(vec![ToolResult::Result {
            result_for_assistant: Some(self.render_result_for_assistant(&data)),
            data,
            image_attachments: None,
        }])
    }
}

fn sanitize_tool_part(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    sanitized.trim_matches('_').to_string()
}

fn truncate_prompt(prompt: &str) -> String {
    const LIMIT: usize = 160;
    if prompt.chars().count() <= LIMIT {
        prompt.to_string()
    } else {
        format!("{}...", prompt.chars().take(LIMIT).collect::<String>())
    }
}
