use serde_json::{json, Value};

use crate::{ToolResult, ValidationResult};

pub const ACP_TOOL_PREFIX: &str = "acp__";
pub const ACP_TOOL_SUFFIX: &str = "__prompt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpExternalAgentToolDefinitionInput<'a> {
    pub client_id: &'a str,
    pub display_name: Option<&'a str>,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpExternalAgentToolDefinition {
    pub client_id: String,
    pub tool_name: String,
    pub display_name: String,
    pub user_facing_name: String,
    pub description: String,
    pub short_description: String,
    pub read_only: bool,
}

pub fn normalize_name_for_acp_tool_part(value: &str) -> String {
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

pub fn build_acp_external_agent_tool_name(client_id: &str) -> String {
    format!(
        "{ACP_TOOL_PREFIX}{}{ACP_TOOL_SUFFIX}",
        normalize_name_for_acp_tool_part(client_id)
    )
}

pub fn build_acp_external_agent_tool_definition(
    input: AcpExternalAgentToolDefinitionInput<'_>,
) -> AcpExternalAgentToolDefinition {
    let display_name = input
        .display_name
        .map(str::to_string)
        .unwrap_or_else(|| input.client_id.to_string());
    AcpExternalAgentToolDefinition {
        client_id: input.client_id.to_string(),
        tool_name: build_acp_external_agent_tool_name(input.client_id),
        user_facing_name: format!("{display_name} (ACP)"),
        description: format!(
            "Send a prompt to the external ACP agent '{}'. Use this when another local ACP-compatible agent is better suited for a delegated task.",
            display_name
        ),
        short_description: format!("Delegate a task to the external ACP agent '{}'.", display_name),
        read_only: input.read_only,
        display_name,
    }
}

pub fn acp_external_agent_tool_input_schema() -> Value {
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

pub fn validate_acp_external_agent_tool_input(input: &Value) -> ValidationResult {
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

pub fn render_acp_external_agent_use_message(display_name: &str, input: &Value) -> String {
    let prompt_preview = input
        .get("prompt")
        .and_then(|value| value.as_str())
        .map(truncate_prompt)
        .unwrap_or_else(|| "prompt".to_string());
    format!("Sending ACP prompt to '{}': {prompt_preview}", display_name)
}

pub fn render_acp_external_agent_rejected_message(display_name: &str) -> String {
    format!("ACP prompt to '{}' was rejected", display_name)
}

pub fn render_acp_external_agent_result_message(display_name: &str, output: &Value) -> String {
    output
        .get("response")
        .and_then(|value| value.as_str())
        .map(|response| format!("ACP agent '{}' responded:\n{response}", display_name))
        .unwrap_or_else(|| format!("ACP agent '{}' completed", display_name))
}

pub fn render_acp_external_agent_result_for_assistant(output: &Value) -> String {
    output
        .get("response")
        .and_then(|value| value.as_str())
        .unwrap_or("ACP agent completed without text output")
        .to_string()
}

pub fn build_acp_external_agent_tool_result(
    client_id: &str,
    response: impl Into<String>,
) -> ToolResult {
    let data = json!({
        "client_id": client_id,
        "response": response.into(),
    });
    ToolResult::Result {
        result_for_assistant: Some(render_acp_external_agent_result_for_assistant(&data)),
        data,
        image_attachments: None,
    }
}

fn truncate_prompt(prompt: &str) -> String {
    const LIMIT: usize = 160;
    if prompt.chars().count() <= LIMIT {
        prompt.to_string()
    } else {
        format!("{}...", prompt.chars().take(LIMIT).collect::<String>())
    }
}
