//! DeepReview tool — dispatch a background CodeReview subagent from any context.
//!
//! Unlike `LaunchReviewAgentTool` (which is bound to a prepared DeepReview run
//! manifest and always runs foreground), this tool lets a commander / agent in
//! any session start a read-only CodeReview subagent in the background and
//! receive the spawned task handle (`bg_task_id`) for asynchronous result
//! collection via AgentWait / SessionMessage.

use super::*;

/// Tool name exposed to models and the product tool runtime.
pub(super) const DEEP_REVIEW_TOOL_NAME: &str = "DeepReview";

/// Background CodeReview subagent type id.
const DEEP_REVIEW_SUBAGENT_TYPE: &str = "CodeReview";

#[derive(Debug, Clone)]
struct DeepReviewInvocation {
    description: String,
    target: Option<String>,
    focus: Option<String>,
    strategy: Option<String>,
    model_id: Option<String>,
    timeout_seconds: Option<u64>,
}

pub struct DeepReviewTool;

impl Default for DeepReviewTool {
    fn default() -> Self {
        Self::new()
    }
}

impl DeepReviewTool {
    pub fn new() -> Self {
        Self
    }

    fn input_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) description of the review being dispatched."
                },
                "target": {
                    "type": "string",
                    "description": "Optional review target: a file path, a comma-separated list of paths, or a git range (e.g. HEAD~3..HEAD). When omitted the reviewer inspects the workspace state."
                },
                "focus": {
                    "type": "string",
                    "description": "Optional review lens, e.g. security, performance, logic correctness, architecture, UI. When omitted the reviewer applies an adversarial full-spectrum lens."
                },
                "strategy": {
                    "type": "string",
                    "enum": ["quick", "standard", "deep"],
                    "description": "Optional review intensity. quick = critical/high only, standard = + medium, deep = exhaustive including cosmetic. Defaults to standard."
                },
                "model_id": {
                    "type": "string",
                    "description": "Optional model or model slot for the reviewer. Omit to use the agent default."
                },
                "timeout_seconds": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Optional timeout for the background reviewer in seconds. When omitted, the agent default applies."
                }
            },
            "required": ["description"],
            "additionalProperties": false
        })
    }

    fn parse_invocation(input: &Value) -> BitFunResult<DeepReviewInvocation> {
        let required_string = |field: &str| -> BitFunResult<String> {
            input
                .get(field)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .ok_or_else(|| BitFunError::tool(format!("{field} is required for DeepReview")))
        };
        let optional_string = |field: &str| -> Option<String> {
            input
                .get(field)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
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
        Ok(DeepReviewInvocation {
            description: required_string("description")?,
            target: optional_string("target"),
            focus: optional_string("focus"),
            strategy: optional_string("strategy"),
            model_id: optional_string("model_id"),
            timeout_seconds,
        })
    }

    fn render_description() -> String {
        r#"Dispatch a background read-only code review.

Creates a CodeReview subagent in the background and returns the spawned task handle immediately. Collect the result asynchronously with AgentWait (bg_task_id) or SessionMessage once the reviewer replies.

- `description`: short label for the review run.
- `target`: optional file path, comma-separated paths, or git range (e.g. HEAD~3..HEAD).
- `focus`: optional review lens (security, performance, logic correctness, architecture, UI).
- `strategy`: quick (critical/high only) | standard (+ medium) | deep (exhaustive incl. cosmetic). Defaults to standard.
- `model_id`: optional model or model slot for the reviewer.
- `timeout_seconds`: optional timeout for the background reviewer.

The reviewer is read-only: it inspects and reports findings, it never modifies files."#
            .to_string()
    }

    fn build_review_prompt(invocation: &DeepReviewInvocation) -> String {
        let mut parts = Vec::new();
        parts.push("独立对抗性代码审查。只读：检查并报告发现，绝不修改任何文件。\n".to_string());
        if let Some(target) = &invocation.target {
            parts.push(format!("审查目标：{target}\n"));
        }
        if let Some(focus) = &invocation.focus {
            parts.push(format!("聚焦维度：{focus}\n"));
        }
        let strategy = invocation.strategy.as_deref().unwrap_or("standard");
        let depth = match strategy {
            "quick" => "仅 critical/high 级问题，忽略 cosmetic。",
            "deep" => "穷尽式：含 cosmetic，任何死角不留。",
            _ => "critical/high/medium 级问题 + 关键 cosmetic。",
        };
        parts.push(format!("审查强度：{strategy}（{depth}）\n"));
        parts.push(
            "输出：按严重度（critical/high/medium/low/info）分级列出发现，每条附证据（文件:行号）、影响、修复建议；最后给总体判定（approve / approve_with_suggestions / request_changes / block）。"
                .to_string(),
        );
        parts.join("\n")
    }

    async fn call_deep_review_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let invocation = Self::parse_invocation(input)?;
        let review_prompt = Self::build_review_prompt(&invocation);

        let mut task_input = json!({
            "description": invocation.description,
            "prompt": review_prompt,
            "subagent_type": DEEP_REVIEW_SUBAGENT_TYPE,
            "run_in_background": true,
        });
        if let Some(model_id) = &invocation.model_id {
            task_input["model_id"] = json!(model_id);
        }
        if let Some(timeout_seconds) = invocation.timeout_seconds {
            task_input["timeout_seconds"] = json!(timeout_seconds);
        }

        TaskTool::new().call_task_impl(&task_input, context).await
    }
}

#[async_trait]
impl Tool for DeepReviewTool {
    fn name(&self) -> &str {
        DEEP_REVIEW_TOOL_NAME
    }

    fn manages_own_execution_timeout(&self) -> bool {
        true
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(Self::render_description())
    }

    async fn is_available_in_context(&self, _context: Option<&ToolUseContext>) -> bool {
        true
    }

    fn short_description(&self) -> String {
        "Dispatch a background read-only code review (CodeReview subagent).".to_string()
    }

    fn input_schema(&self) -> Value {
        Self::input_schema()
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        // Background CodeReview spawns are intentionally serialized (same
        // policy as TaskTool spawning CodeReview) to avoid review overlap.
        false
    }

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        let _ = Self::parse_invocation(input)?;
        Ok(vec![PermissionIntent::new(
            "task",
            vec![DEEP_REVIEW_SUBAGENT_TYPE.to_string()],
        )])
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        match Self::parse_invocation(input) {
            Ok(invocation) => {
                if let Some(result) = TaskTool::validate_prompt_size(
                    &json!({ "prompt": Self::build_review_prompt(&invocation) }),
                ) {
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
        input
            .get("description")
            .and_then(Value::as_str)
            .map(|description| {
                if options.verbose {
                    format!("Dispatching DeepReview: {}", description)
                } else {
                    format!("DeepReview: {}", description)
                }
            })
            .unwrap_or_else(|| "Dispatching DeepReview".to_string())
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        self.call_deep_review_impl(input, context).await
    }
}
