use crate::agentic::coordination::{
    get_global_coordinator, BackgroundSubagentOutcome, BackgroundSubagentWaitMode,
    BackgroundSubagentWaitResult,
};
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashSet;
use tokio::time::Duration;

const DEFAULT_TIMEOUT_MS: u64 = 10 * 60 * 1_000;
const MAX_TIMEOUT_MS: u64 = 60 * 60 * 1_000;

pub struct AgentWaitTool;

#[derive(Debug, PartialEq, Eq)]
struct AgentWaitRequest {
    bg_task_ids: Vec<String>,
    wait_mode: BackgroundSubagentWaitMode,
    timeout_ms: u64,
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
            wait_mode: Self::parse_wait_mode(object.get("wait_mode"))?,
            timeout_ms: Self::parse_timeout_ms(object.get("timeout_ms")),
        })
    }

    fn parse_wait_mode(wait_mode: Option<&Value>) -> BitFunResult<BackgroundSubagentWaitMode> {
        let wait_mode = match wait_mode {
            None => BackgroundSubagentWaitMode::All,
            Some(Value::String(value)) => match value.trim() {
                "any" => BackgroundSubagentWaitMode::Any,
                "all" => BackgroundSubagentWaitMode::All,
                value => {
                    return Err(BitFunError::tool(format!(
                        "wait_mode must be \"any\" or \"all\"; got: {}",
                        value
                    )));
                }
            },
            Some(_) => {
                return Err(BitFunError::tool(
                    "wait_mode must be \"any\" or \"all\"".to_string(),
                ));
            }
        };
        Ok(wait_mode)
    }

    fn parse_timeout_ms(timeout_ms: Option<&Value>) -> u64 {
        timeout_ms
            .and_then(Value::as_u64)
            .filter(|timeout_ms| *timeout_ms > 0)
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS)
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
        if result.outcomes.is_empty() {
            return format!(
                "AgentWait finished with status {}. Pending background task IDs: {}.",
                result.status.as_str(),
                result.pending_bg_task_ids.join(", ")
            );
        }

        let mut message = format!("AgentWait finished with status {}.", result.status.as_str());
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

    async fn description(&self) -> BitFunResult<String> {
        Ok("Wait for background subagent results.
Set wait_mode to `any` to return after any selected task completes, or `all` to wait for every selected task.
Provide bg_task_ids when known; omit it or pass [] to select all unconsumed background tasks.
The selected task set is fixed when the call starts. wait_mode defaults to `all`; the tool also returns when `timeout_ms` has elapsed.".to_string())
    }

    fn short_description(&self) -> String {
        "Wait for selected background subagent results.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bg_task_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional background task IDs returned by Task tool. Omit this field or pass [] to select all unconsumed background subagent results."
                },
                "wait_mode": {
                    "type": "string",
                    "enum": ["any", "all"],
                    "default": "all",
                    "description": "Defaults to `all`."
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Maximum time to wait in milliseconds. Defaults to ten minutes."
                }
            },
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
                request.wait_mode,
                Duration::from_millis(request.timeout_ms),
                dialog_turn_id,
                context.cancellation_token(),
            )
            .await?;
        let data = json!({
            "status": result.status.as_str(),
            "wait_mode": request.wait_mode.as_str(),
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
    use super::{AgentWaitTool, DEFAULT_TIMEOUT_MS, MAX_TIMEOUT_MS};
    use crate::agentic::coordination::BackgroundSubagentWaitMode;
    use crate::agentic::tools::framework::Tool;

    #[test]
    fn schema_exposes_only_parent_scoped_background_task_ids() {
        let schema = AgentWaitTool::new().input_schema();

        assert_eq!(schema["properties"]["bg_task_ids"]["type"], "array");
        assert!(schema["properties"].get("background_task_ids").is_none());
    }

    #[test]
    fn empty_input_uses_the_default_timeout_and_session_selector() {
        let request = AgentWaitTool::parse_request(&serde_json::json!({})).expect("valid request");
        assert!(request.bg_task_ids.is_empty());
        assert_eq!(request.wait_mode, BackgroundSubagentWaitMode::All);
        assert_eq!(request.timeout_ms, DEFAULT_TIMEOUT_MS);
    }

    #[test]
    fn explicit_wait_mode_applies_to_session_and_exact_task_selectors() {
        let any = AgentWaitTool::parse_request(&serde_json::json!({
            "wait_mode": "any"
        }))
        .expect("any wait mode must be valid");
        assert!(any.bg_task_ids.is_empty());
        assert_eq!(any.wait_mode, BackgroundSubagentWaitMode::Any);

        let all = AgentWaitTool::parse_request(&serde_json::json!({
            "wait_mode": "all"
        }))
        .expect("all wait mode must be valid");
        assert!(all.bg_task_ids.is_empty());
        assert_eq!(all.wait_mode, BackgroundSubagentWaitMode::All);

        let empty = AgentWaitTool::parse_request(&serde_json::json!({
            "bg_task_ids": [],
            "wait_mode": "any"
        }))
        .expect("an empty selector must be valid");
        assert_eq!(empty.wait_mode, BackgroundSubagentWaitMode::Any);

        let exact = AgentWaitTool::parse_request(&serde_json::json!({
            "bg_task_ids": ["bg1", "bg2"],
            "wait_mode": "any"
        }))
        .expect("exact task IDs must be valid");
        assert_eq!(exact.wait_mode, BackgroundSubagentWaitMode::Any);
        assert_eq!(exact.bg_task_ids, ["bg1", "bg2"]);
    }

    #[test]
    fn a_single_task_id_string_is_accepted() {
        let request = AgentWaitTool::parse_request(&serde_json::json!({
            "bg_task_ids": " bg1 "
        }))
        .expect("a single task ID string must be accepted");
        assert_eq!(request.bg_task_ids, ["bg1"]);
        assert_eq!(request.wait_mode, BackgroundSubagentWaitMode::All);
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
    fn timeout_and_unknown_parameters_are_tolerated() {
        let defaulted = AgentWaitTool::parse_request(&serde_json::json!({
            "timeout_ms": "invalid",
            "unused": true
        }))
        .expect("invalid timeout and unknown parameters must be tolerated");
        assert_eq!(defaulted.timeout_ms, DEFAULT_TIMEOUT_MS);

        let capped = AgentWaitTool::parse_request(&serde_json::json!({
            "timeout_ms": MAX_TIMEOUT_MS + 1
        }))
        .expect("large timeout must be capped");
        assert_eq!(capped.timeout_ms, MAX_TIMEOUT_MS);

        let zero = AgentWaitTool::parse_request(&serde_json::json!({
            "timeout_ms": 0
        }))
        .expect("zero timeout must use the default");
        assert_eq!(zero.timeout_ms, DEFAULT_TIMEOUT_MS);
    }
}
