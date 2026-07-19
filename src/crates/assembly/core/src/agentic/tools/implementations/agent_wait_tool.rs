use crate::agentic::coordination::{
    get_global_coordinator, BackgroundSubagentOutcome, BackgroundSubagentWaitResult,
};
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashSet;
use tokio::time::Duration;

const DEFAULT_TIMEOUT_MS: u64 = 10 * 60 * 1_000;
const MAX_TIMEOUT_MS: u64 = 60 * 60 * 1_000;

pub struct AgentWaitTool;

impl Default for AgentWaitTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentWaitTool {
    pub fn new() -> Self {
        Self
    }

    fn parse_request(input: &Value) -> BitFunResult<(Vec<String>, u64)> {
        let object = input
            .as_object()
            .ok_or_else(|| BitFunError::tool("AgentWait input must be an object".to_string()))?;
        for key in object.keys() {
            if key != "background_task_ids" && key != "timeout_ms" {
                return Err(BitFunError::tool(format!(
                    "Unsupported AgentWait parameter: {}",
                    key
                )));
            }
        }

        let background_task_ids = match object.get("background_task_ids") {
            None => Vec::new(),
            Some(Value::Array(values)) => {
                let mut seen = HashSet::new();
                values
                    .iter()
                    .map(|value| {
                        let value = value.as_str().ok_or_else(|| {
                            BitFunError::tool(
                                "background_task_ids must contain strings".to_string(),
                            )
                        })?;
                        let value = value.trim();
                        if value.is_empty() {
                            return Err(BitFunError::tool(
                                "background_task_ids cannot contain empty values".to_string(),
                            ));
                        }
                        if !seen.insert(value.to_string()) {
                            return Err(BitFunError::tool(format!(
                                "background_task_ids contains a duplicate task ID: {}",
                                value
                            )));
                        }
                        Ok(value.to_string())
                    })
                    .collect::<BitFunResult<Vec<_>>>()?
            }
            Some(_) => {
                return Err(BitFunError::tool(
                    "background_task_ids must be an array of strings".to_string(),
                ));
            }
        };

        let timeout_ms = match object.get("timeout_ms") {
            None => DEFAULT_TIMEOUT_MS,
            Some(value) => value.as_u64().ok_or_else(|| {
                BitFunError::tool("timeout_ms must be a positive integer".to_string())
            })?,
        };
        if timeout_ms == 0 || timeout_ms > MAX_TIMEOUT_MS {
            return Err(BitFunError::tool(format!(
                "timeout_ms must be between 1 and {}",
                MAX_TIMEOUT_MS
            )));
        }
        Ok((background_task_ids, timeout_ms))
    }

    fn outcome_json(outcome: &BackgroundSubagentOutcome) -> Value {
        json!({
            "background_task_id": outcome.background_task_id,
            "subagent_session_id": outcome.subagent_session_id,
            "outcome": outcome.status.as_str(),
            "content": outcome.content,
            "error": outcome.error,
        })
    }

    fn assistant_result(result: &BackgroundSubagentWaitResult) -> String {
        if result.outcomes.is_empty() {
            return format!(
                "AgentWait finished with status {}. Pending background task IDs: {}.",
                result.status.as_str(),
                result.pending_background_task_ids.join(", ")
            );
        }

        let mut message = format!("AgentWait finished with status {}.", result.status.as_str());
        for outcome in &result.outcomes {
            message.push_str(&format!(
                "\n<background_subagent_result task_id=\"{}\" session_id=\"{}\" status=\"{}\">",
                outcome.background_task_id,
                outcome.subagent_session_id,
                outcome.status.as_str(),
            ));
            if let Some(content) = &outcome.content {
                message.push_str(content);
            }
            if let Some(error) = &outcome.error {
                message.push_str("\nError: ");
                message.push_str(error);
            }
            message.push_str("</background_subagent_result>");
        }
        if !result.pending_background_task_ids.is_empty() {
            message.push_str(&format!(
                "\nPending background task IDs: {}.",
                result.pending_background_task_ids.join(", ")
            ));
        }
        message
    }
}

#[async_trait]
impl Tool for AgentWaitTool {
    fn name(&self) -> &str {
        "AgentWait"
    }

    fn manages_own_execution_timeout(&self) -> bool {
        true
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok("Wait for background Task results that belong to the current parent turn. Provide exact background_task_ids when known, or omit the field to collect unconsumed background tasks created by this turn. Use this only when the unfinished answer depends on those results. The tool returns after matching tasks finish, after its timeout, or when the current turn is cancelled.".to_string())
    }

    fn short_description(&self) -> String {
        "Wait for selected background subagent results.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "background_task_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional exact background task IDs returned by Task. Omit to wait for any unconsumed background task created by the current turn."
                },
                "timeout_ms": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_TIMEOUT_MS,
                    "default": DEFAULT_TIMEOUT_MS,
                    "description": "Maximum time to wait in milliseconds. Defaults to ten minutes."
                }
            },
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    fn render_tool_use_message(&self, _input: &Value, options: &ToolRenderOptions) -> String {
        if options.verbose {
            "Waiting for background subagent results".to_string()
        } else {
            "Waiting for background tasks".to_string()
        }
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        match Self::parse_request(input) {
            Ok(_) => ValidationResult {
                result: true,
                message: None,
                error_code: None,
                meta: None,
            },
            Err(error) => ValidationResult {
                result: false,
                message: Some(error.to_string()),
                error_code: None,
                meta: None,
            },
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let (background_task_ids, timeout_ms) = Self::parse_request(input)?;
        let session_id = context
            .session_id
            .as_deref()
            .ok_or_else(|| BitFunError::tool("session_id is required in context".to_string()))?;
        let dialog_turn_id = context.dialog_turn_id.as_deref().ok_or_else(|| {
            BitFunError::tool("dialog_turn_id is required in context".to_string())
        })?;
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
        let result = coordinator
            .wait_for_background_subagent_outcomes(
                session_id,
                dialog_turn_id,
                &background_task_ids,
                Duration::from_millis(timeout_ms),
                context.cancellation_token(),
            )
            .await?;
        let data = json!({
            "status": result.status.as_str(),
            "results": result.outcomes.iter().map(Self::outcome_json).collect::<Vec<_>>(),
            "pending_background_task_ids": result.pending_background_task_ids,
        });
        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(Self::assistant_result(&result)),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentWaitTool, DEFAULT_TIMEOUT_MS};

    #[test]
    fn empty_input_uses_the_default_timeout_and_current_turn_selector() {
        let (task_ids, timeout_ms) =
            AgentWaitTool::parse_request(&serde_json::json!({})).expect("valid request");
        assert!(task_ids.is_empty());
        assert_eq!(timeout_ms, DEFAULT_TIMEOUT_MS);
    }

    #[test]
    fn duplicate_task_ids_are_rejected() {
        let error = AgentWaitTool::parse_request(&serde_json::json!({
            "background_task_ids": ["bg-1", "bg-1"]
        }))
        .expect_err("duplicate task IDs must fail");
        assert!(error.to_string().contains("duplicate"));
    }
}
