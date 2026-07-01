use super::*;

pub struct LaunchReviewAgentTool;

#[derive(Debug, Clone)]
struct LaunchReviewAgentInvocation {
    description: String,
    prompt: String,
    subagent_type: String,
    model_id: Option<String>,
    timeout_seconds: Option<u64>,
    is_retry: bool,
    requested_auto_retry: bool,
}

impl Default for LaunchReviewAgentTool {
    fn default() -> Self {
        Self::new()
    }
}

impl LaunchReviewAgentTool {
    pub fn new() -> Self {
        Self
    }

    fn launch_input_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) description of the review agent launch."
                },
                "prompt": {
                    "type": "string",
                    "description": "The review assignment for this DeepReview team member. Keep it scoped to the assigned packet and do not include top-level LaunchReviewAgent arguments inside this string."
                },
                "subagent_type": {
                    "type": "string",
                    "description": "Required DeepReview team member agent type."
                },
                "model_id": {
                    "type": "string",
                    "description": "Optional model or model slot for this reviewer or judge. Use the configured team manifest's preferred model_id for the matching review agent when one is provided; omit it to use the agent default."
                },
                "timeout_seconds": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Optional timeout for this reviewer or judge in seconds. When omitted, the DeepReview execution policy timeout for the role is used. When provided, the effective timeout is capped by the DeepReview execution policy for that role."
                },
                "retry": {
                    "type": "boolean",
                    "description": "True when re-dispatching a reviewer after that same reviewer returned partial_timeout or an explicit transient capacity failure in the current turn."
                },
                "auto_retry": {
                    "type": "boolean",
                    "description": "True only for backend-owned bounded automatic retries. Requires Review Team auto retry opt-in and retry=true. User/model-issued retry actions must omit this field or set it to false."
                },
                "retry_coverage": {
                    "type": "object",
                    "description": "Retry only: structured coverage metadata proving the retry is bounded. Required when retry=true.",
                    "properties": {
                        "source_packet_id": {
                            "type": "string",
                            "description": "The original reviewer packet_id being retried."
                        },
                        "source_status": {
                            "type": "string",
                            "enum": ["partial_timeout", "capacity_skipped"],
                            "description": "The retryable source status."
                        },
                        "capacity_reason": {
                            "type": "string",
                            "description": "Required for capacity_skipped; must be a transient capacity reason such as local_concurrency_cap, launch_batch_blocked, provider_rate_limit, provider_concurrency_limit, retry_after, or temporary_overload."
                        },
                        "covered_files": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Files already covered by the source attempt."
                        },
                        "retry_scope_files": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Smaller file list to retry. Every entry must belong to the source packet and must not overlap covered_files."
                        }
                    },
                    "required": [
                        "source_packet_id",
                        "source_status",
                        "covered_files",
                        "retry_scope_files"
                    ]
                }
            },
            "required": [
                "description",
                "prompt",
                "subagent_type"
            ],
            "additionalProperties": false
        })
    }

    fn parse_invocation(input: &Value) -> BitFunResult<LaunchReviewAgentInvocation> {
        for field in ["action", "fork_context", "session_id", "run_in_background"] {
            if input.get(field).is_some() {
                return Err(BitFunError::tool(format!(
                    "{field} is not supported for LaunchReviewAgent"
                )));
            }
        }

        let required_string = |field: &str| -> BitFunResult<String> {
            let value = input
                .get(field)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    BitFunError::tool(format!("{field} is required for LaunchReviewAgent"))
                })?;
            Ok(value.to_string())
        };
        let timeout_seconds = match input.get("timeout_seconds") {
            Some(value) => {
                let parsed = value.as_u64().ok_or_else(|| {
                    BitFunError::tool("timeout_seconds must be a non-negative integer".to_string())
                })?;
                (parsed > 0).then_some(parsed)
            }
            None => None,
        };
        let model_id = match input.get("model_id") {
            Some(value) => {
                let value = value
                    .as_str()
                    .ok_or_else(|| BitFunError::tool("model_id must be a string".to_string()))?
                    .trim();
                (!value.is_empty()).then(|| value.to_string())
            }
            None => None,
        };

        Ok(LaunchReviewAgentInvocation {
            description: required_string("description")?,
            prompt: required_string("prompt")?,
            subagent_type: required_string("subagent_type")?,
            model_id,
            timeout_seconds,
            is_retry: input.get("retry").and_then(Value::as_bool).unwrap_or(false),
            requested_auto_retry: input
                .get("auto_retry")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    fn render_description() -> String {
        r#"Launch a DeepReview review team member.

Use this tool to dispatch review team members. It starts a new fresh subagent session and waits for the result.

Built-in review agent types:
- `ReviewBusinessLogic`: product behavior, business logic, state transitions, and user-visible correctness.
- `ReviewArchitecture`: module boundaries, ownership, maintainability, API shape, and long-term design risks.
- `ReviewPerformance`: latency, resource use, async/concurrency behavior, hot paths, and scalability.
- `ReviewSecurity`: auth, trust boundaries, injection, filesystem/network safety, secret handling, and privilege risks.
- `ReviewFrontend`: i18n, frontend performance, accessibility, state management, frontend-backend API contracts, and platform boundaries.
- `ReviewJudge`: final quality-inspector pass after reviewer outputs are available.

Extra active reviewers may be provided by the run manifest. Use only a `subagent_type` that belongs to the active DeepReview team for this run.

Do not put `subagent_type`, `description`, `model_id`, `timeout_seconds`, `retry`, `auto_retry`, or `retry_coverage` inside the prompt string.

Retry rules:
- Set `retry=true` only when re-dispatching the same reviewer after `partial_timeout` or a transient capacity skip in the current turn.
- Retry calls must include `retry_coverage` with `source_packet_id`, `source_status`, `covered_files`, and a smaller `retry_scope_files` list.
- Do not set `auto_retry=true` unless this is a backend-owned bounded automatic retry admitted by Review Team settings."#
            .to_string()
    }

    fn render_tool_use_message(input: &Value, options: &ToolRenderOptions) -> String {
        input
            .get("description")
            .and_then(Value::as_str)
            .map(|description| {
                if options.verbose {
                    format!("Launching review agent: {}", description)
                } else {
                    format!("Review agent: {}", description)
                }
            })
            .unwrap_or_else(|| "Launching review agent".to_string())
    }

    pub(super) async fn call_launch_review_agent_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        if !TaskTool::is_deep_review_context(Some(context)) {
            return Err(BitFunError::tool(
                "LaunchReviewAgent is only available in DeepReview context".to_string(),
            ));
        }
        let invocation = Self::parse_invocation(input)?;
        let task_input = json!({
            "description": invocation.description,
            "prompt": invocation.prompt,
            "subagent_type": invocation.subagent_type,
            "model_id": invocation.model_id,
            "timeout_seconds": invocation.timeout_seconds,
            "retry": invocation.is_retry,
            "auto_retry": invocation.requested_auto_retry,
            "retry_coverage": input.get("retry_coverage").cloned().unwrap_or(Value::Null),
        });
        let mut task_input = task_input;
        if task_input.get("timeout_seconds") == Some(&Value::Null) {
            task_input
                .as_object_mut()
                .unwrap()
                .remove("timeout_seconds");
        }
        if task_input.get("model_id") == Some(&Value::Null) {
            task_input.as_object_mut().unwrap().remove("model_id");
        }
        if task_input.get("retry_coverage") == Some(&Value::Null) {
            task_input.as_object_mut().unwrap().remove("retry_coverage");
        }
        if task_input.get("retry") == Some(&Value::Bool(false)) {
            task_input.as_object_mut().unwrap().remove("retry");
        }
        if task_input.get("auto_retry") == Some(&Value::Bool(false)) {
            task_input.as_object_mut().unwrap().remove("auto_retry");
        }

        TaskTool::new()
            .call_deep_review_task_impl(&task_input, context)
            .await
    }
}

#[async_trait]
impl Tool for LaunchReviewAgentTool {
    fn name(&self) -> &str {
        "LaunchReviewAgent"
    }

    fn manages_own_execution_timeout(&self) -> bool {
        true
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(Self::render_description())
    }

    async fn is_available_in_context(&self, context: Option<&ToolUseContext>) -> bool {
        TaskTool::is_deep_review_context(context)
    }

    fn short_description(&self) -> String {
        "Launch a DeepReview review team member and collect the result.".to_string()
    }

    fn input_schema(&self) -> Value {
        Self::launch_input_schema()
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        let subagent_type = input
            .and_then(|value| value.get("subagent_type"))
            .and_then(Value::as_str);
        match subagent_type {
            Some(id) => get_agent_registry()
                .get_subagent_is_readonly(id)
                .unwrap_or(false),
            None => false,
        }
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        match Self::parse_invocation(input) {
            Ok(_) => {
                if let Some(result) = TaskTool::validate_prompt_size(input) {
                    return result;
                }
                ValidationResult {
                    result: true,
                    message: None,
                    error_code: None,
                    meta: None,
                }
            }
            Err(error) => TaskTool::invalid_input(error.to_string()),
        }
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        Self::render_tool_use_message(input, options)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        self.call_launch_review_agent_impl(input, context).await
    }
}
