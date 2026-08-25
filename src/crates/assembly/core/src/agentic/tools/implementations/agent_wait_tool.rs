use crate::agentic::coordination::{
    get_global_coordinator, BackgroundSubagentOutcome, BackgroundSubagentWaitMode,
    BackgroundSubagentWaitResult, BackgroundSubagentWaitStatus,
};
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashSet;
use tokio::time::Duration;

const MIN_TIMEOUT_SECONDS: u64 = 30 * 60;
const DEFAULT_TIMEOUT_SECONDS: u64 = MIN_TIMEOUT_SECONDS;
const MAX_TIMEOUT_SECONDS: u64 = 60 * 60;

pub struct AgentWaitTool;

#[derive(Debug, PartialEq, Eq)]
struct AgentWaitRequest {
    bg_task_ids: Vec<String>,
    timeout_seconds: u64,
}

impl Default for AgentWaitTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentWaitTool {
    pub fn new() -> Self {
        Self
    }

    fn parse_request(input: &Value) -> BitFunResult<AgentWaitRequest> {
        let object = input
            .as_object()
            .ok_or_else(|| BitFunError::tool("AgentWait input must be an object".to_string()))?;

        let task_ids = object
            .get("bg_task_ids")
            .or_else(|| object.get("background_task_ids"));
        let (values, require_task_id) = match task_ids {
            None => (Vec::new(), false),
            Some(value @ Value::String(_)) => (vec![value], true),
            Some(Value::Array(values)) if values.is_empty() => (Vec::new(), false),
            Some(Value::Array(values)) => (values.iter().collect::<Vec<_>>(), true),
            Some(_) => {
                return Err(BitFunError::tool(
                    "bg_task_ids must be a string or an array".to_string(),
                ));
            }
        };

        let mut seen = HashSet::new();
        let bg_task_ids = values
            .into_iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .filter(|value| seen.insert((*value).to_string()))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if require_task_id && bg_task_ids.is_empty() {
            return Err(BitFunError::tool(
                "bg_task_ids must contain at least one non-empty string".to_string(),
            ));
        }

        Ok(AgentWaitRequest {
            bg_task_ids,
            timeout_seconds: Self::parse_timeout_seconds(object.get("timeout_seconds")),
        })
    }

    fn parse_timeout_seconds(timeout_seconds: Option<&Value>) -> u64 {
        timeout_seconds
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_TIMEOUT_SECONDS)
            .clamp(MIN_TIMEOUT_SECONDS, MAX_TIMEOUT_SECONDS)
    }

    fn outcome_json(outcome: &BackgroundSubagentOutcome) -> Value {
        json!({
            "bg_task_id": outcome.model_bg_task_id(),
            "agent_id": outcome.model_agent_id(),
            "outcome": outcome.status.as_str(),
            "content": outcome.content,
            "error": outcome.error,
        })
    }

    fn assistant_result(result: &BackgroundSubagentWaitResult) -> String {
        let finished = if result.status == BackgroundSubagentWaitStatus::Steered {
            "AgentWait ended early because user steering arrived. Background agents continue running."
                .to_string()
        } else {
            format!("AgentWait finished with status {}.", result.status.as_str())
        };
        if result.outcomes.is_empty() {
            return format!(
                "{} Pending background task IDs: {}.",
                finished,
                result.pending_bg_task_ids.join(", ")
            );
        }

        let mut message = finished;
        for outcome in &result.outcomes {
            message.push_str(&format!(
                "\n<result bg_task_id=\"{}\" agent_id=\"{}\" status=\"{}\">",
                outcome.model_bg_task_id(),
                outcome.model_agent_id(),
                outcome.status.as_str(),
            ));
            if let Some(content) = &outcome.content {
                message.push_str(content);
            }
            if let Some(error) = &outcome.error {
                message.push_str("\nError: ");
                message.push_str(error);
            }
            message.push_str("</result>");
        }
        if !result.pending_bg_task_ids.is_empty() {
            message.push_str(&format!(
                "\nPending background task IDs: {}.",
                result.pending_bg_task_ids.join(", ")
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

    fn round_injection_yieldable(&self) -> bool {
        true
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok("Wait for background agent results.
Wait for every selected task to complete. The tool also returns when `timeout_seconds` has elapsed.".to_string())
    }

    fn short_description(&self) -> String {
        "Wait for selected background agent results.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bg_task_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Background task IDs whose results should be collected."
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "Maximum time to wait in seconds, with a minimum of 30 minutes (default) and a maximum of 1 hour."
                }
            },
            "required": ["bg_task_ids"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn permission_intents(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        Ok(Vec::new())
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
        let request = Self::parse_request(input)?;
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
                &request.bg_task_ids,
                BackgroundSubagentWaitMode::All,
                Duration::from_secs(request.timeout_seconds),
                dialog_turn_id,
                context.cancellation_token(),
                context.round_injection_preemption_token(),
            )
            .await?;
        let data = json!({
            "status": result.status.as_str(),
            "results": result.outcomes.iter().map(Self::outcome_json).collect::<Vec<_>>(),
            "pending_bg_task_ids": result.pending_bg_task_ids,
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
    use super::{AgentWaitTool, DEFAULT_TIMEOUT_SECONDS, MAX_TIMEOUT_SECONDS, MIN_TIMEOUT_SECONDS};
    use crate::agentic::tools::framework::Tool;

    #[test]
    fn agent_wait_owns_timeout_and_yields_to_round_injection() {
        let tool = AgentWaitTool::new();
        assert!(tool.manages_own_execution_timeout());
        assert!(tool.round_injection_yieldable());
    }

    #[test]
    fn missing_or_empty_task_ids_are_tolerated_by_the_parser() {
        let request = AgentWaitTool::parse_request(&serde_json::json!({})).expect("valid request");
        assert!(request.bg_task_ids.is_empty());
        assert_eq!(request.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);

        let empty = AgentWaitTool::parse_request(&serde_json::json!({
            "bg_task_ids": []
        }))
        .expect("an empty selector must be valid");
        assert!(empty.bg_task_ids.is_empty());
    }

    #[test]
    fn a_single_task_id_string_is_accepted() {
        let request = AgentWaitTool::parse_request(&serde_json::json!({
            "bg_task_ids": " bg1 "
        }))
        .expect("a single task ID string must be accepted");
        assert_eq!(request.bg_task_ids, ["bg1"]);
    }

    #[test]
    fn legacy_background_task_ids_are_tolerated_without_schema_exposure() {
        let request = AgentWaitTool::parse_request(&serde_json::json!({
            "background_task_ids": ["bg1"]
        }))
        .expect("legacy task IDs must remain unambiguous at runtime");
        assert_eq!(request.bg_task_ids, ["bg1"]);
    }

    #[test]
    fn task_ids_filter_empty_values_and_deduplicate() {
        let request = AgentWaitTool::parse_request(&serde_json::json!({
            "bg_task_ids": [" bg1 ", "", null, 1, "bg1", "bg2", "   "]
        }))
        .expect("valid task IDs must be retained");
        assert_eq!(request.bg_task_ids, ["bg1", "bg2"]);
    }

    #[test]
    fn task_ids_require_a_string_after_filtering_non_empty_inputs() {
        let error = AgentWaitTool::parse_request(&serde_json::json!({
            "bg_task_ids": ["", null, 1]
        }))
        .expect_err("non-empty selectors without usable IDs must fail");
        assert!(error.to_string().contains("at least one non-empty string"));
    }

    #[test]
    fn timeout_uses_seconds_and_is_clamped_to_supported_bounds() {
        let defaulted = AgentWaitTool::parse_request(&serde_json::json!({
            "timeout_seconds": "invalid",
            "unused": true
        }))
        .expect("invalid timeout and unknown parameters must be tolerated");
        assert_eq!(defaulted.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);

        let capped = AgentWaitTool::parse_request(&serde_json::json!({
            "timeout_seconds": MAX_TIMEOUT_SECONDS + 1
        }))
        .expect("large timeout must be capped");
        assert_eq!(capped.timeout_seconds, MAX_TIMEOUT_SECONDS);

        let raised = AgentWaitTool::parse_request(&serde_json::json!({
            "timeout_seconds": MIN_TIMEOUT_SECONDS - 1
        }))
        .expect("short timeout must be raised to the minimum");
        assert_eq!(raised.timeout_seconds, MIN_TIMEOUT_SECONDS);
    }
}
