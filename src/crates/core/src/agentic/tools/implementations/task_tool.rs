use crate::agentic::agents::{get_agent_registry, AgentInfo};
use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::deep_review_policy::{
    load_default_deep_review_policy, record_deep_review_task_budget, DeepReviewRunManifestGate,
    DEEP_REVIEW_AGENT_TYPE,
};
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::pipeline::SubagentParentInfo;
use crate::agentic::tools::InputValidator;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::warn;
use serde_json::{json, Value};
use std::path::Path;

pub struct TaskTool;

const LARGE_TASK_PROMPT_SOFT_LINE_LIMIT: usize = 180;
const LARGE_TASK_PROMPT_SOFT_BYTE_LIMIT: usize = 16 * 1024;

impl Default for TaskTool {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskTool {
    pub fn new() -> Self {
        Self
    }

    fn format_agent_descriptions(&self, agents: &[AgentInfo]) -> String {
        if agents.is_empty() {
            return String::new();
        }
        let mut out = String::from("<available_agents>\n");
        for agent in agents {
            out.push_str(&format!(
                "<agent type=\"{}\">\n<description>\n{}\n</description>\n<tools>{}</tools>\n</agent>\n",
                agent.id,
                agent.description,
                agent.default_tools.join(", ")
            ));
        }
        out.push_str("</available_agents>");
        out
    }

    fn render_description(&self, agent_descriptions: String) -> String {
        let agent_descriptions = if agent_descriptions.is_empty() {
            "<agents>No agents available</agents>".to_string()
        } else {
            agent_descriptions
        };

        format!(
            r#"Launch a new agent to handle complex, multi-step tasks autonomously. 

The Task tool launches specialized agents (subprocesses) that autonomously handle complex tasks. Each agent type has specific capabilities and tools available to it.

Available agents and the tools they have access to:
{}

When using the Task tool, you must specify a subagent_type parameter to select which agent type to use.

When NOT to use the Task tool:
- If you want to read a specific file path, use the Read or Glob tool instead of the Task tool, to find the match more quickly
- If you are searching for a specific class definition like "class Foo", use the Glob tool instead, to find the match more quickly
- If you are searching for code within a specific file or set of 2-3 files, use the Read tool instead of the Task tool, to find the match more quickly
- For subagent_type=Explore: do not use it for simple lookups above; reserve it for broad or multi-area exploration where many tool rounds would be needed
- Other tasks that are not related to the agent descriptions above


Usage notes:
- Always include a short description (3-5 words) summarizing what the agent will do
- Provide clear, detailed prompt so the agent can work autonomously and return exactly the information you need.
- If 'workspace_path' is omitted, the task inherits the current workspace by default.
- The 'workspace_path' parameter must still be provided explicitly for the Explore and FileFinder agent.
- Use 'model_id' when a caller needs a specific model or model slot for the subagent. Omit it to use the agent default.
- Use 'timeout_seconds' when you need a hard deadline for the subagent. Omit it or set it to 0 to disable the timeout.
- For DeepReview only, set 'retry' to true when re-dispatching a reviewer after that same reviewer failed or timed out in the current turn.
- Launch multiple agents concurrently whenever possible, to maximize performance; to do that, use a single message with multiple tool calls
- When the agent is done, it will return a single message back to you.
- The agent's outputs should generally be trusted
- Clearly tell the agent whether you expect it to write code or just to do research (search, file reads, web fetches, etc.), since it is not aware of the user's intent
- If the agent description mentions that it should be used proactively, then you should try your best to use it without the user having to ask for it first. Use your judgement.
- If the user specifies that they want you to run agents "in parallel", you MUST send a single message with multiple Task tool calls. For example, if you need to launch both a code-reviewer agent and a test-runner agent in parallel, send a single message with both tool calls.

Example usage:

<example_agent_descriptions>
"code-reviewer": use this agent after you are done writing a signficant piece of code
"greeting-responder": use this agent when to respond to user greetings with a friendly joke
</example_agent_description>

<example>
user: "Please write a function that checks if a number is prime"
assistant: Sure let me write a function that checks if a number is prime
assistant: First let me use the Write tool to write a function that checks if a number is prime
assistant: I'm going to use the Write tool to write the following code:
<code>
function isPrime(n) {{
  if (n <= 1) return false
  for (let i = 2; i * i <= n; i++) {{
    if (n % i === 0) return false
  }}
  return true
}}
</code>
<commentary>
Since a signficant piece of code was written and the task was completed, now use the code-reviewer agent to review the code
</commentary>
assistant: Now let me use the code-reviewer agent to review the code
assistant: Uses the Task tool to launch the code-reviewer agent 
</example>

<example>
user: "Hello"
<commentary>
Since the user is greeting, use the greeting-responder agent to respond with a friendly joke
</commentary>
assistant: "I'm going to use the Task tool to launch the greeting-responder agent"
</example>"#,
            agent_descriptions
        )
    }

    async fn build_description(&self, workspace_root: Option<&Path>) -> String {
        let agents = self.get_enabled_agents(workspace_root).await;
        let agent_descriptions = self.format_agent_descriptions(&agents);
        self.render_description(agent_descriptions)
    }

    async fn get_enabled_agents(&self, workspace_root: Option<&Path>) -> Vec<AgentInfo> {
        let registry = get_agent_registry();
        if let Some(workspace_root) = workspace_root {
            registry.load_custom_subagents(workspace_root).await;
        }
        registry
            .get_subagents_info(workspace_root)
            .await
            .into_iter()
            .filter(|agent| agent.enabled) // Only return enabled subagents
            .collect()
    }

    async fn get_agents_types(&self, workspace_root: Option<&Path>) -> Vec<String> {
        self.get_enabled_agents(workspace_root)
            .await
            .into_iter()
            .map(|agent| agent.id)
            .collect()
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "Task"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(self.build_description(None).await)
    }

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Ok(self
            .build_description(context.and_then(|ctx| ctx.workspace_root()))
            .await)
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) description of the task"
                },
                "prompt": {
                    "type": "string",
                    "description": "The task for the agent to perform. Keep it scoped and concise. The 180-line / 16KB guideline is a soft reliability threshold, not a hard cap. For large delegations, split into multiple Task calls with clear ownership, and pass file paths, symbols, constraints, and exact questions instead of pasting large file contents."
                },
                "subagent_type": {
                    "type": "string",
                    "description": "The type of specialized agent to use for this task"
                },
                "workspace_path": {
                    "type": "string",
                    "description": "The absolute path of the workspace for this task. If omitted, inherits the current workspace. Explore/FileFinder must provide it explicitly."
                },
                "model_id": {
                    "type": "string",
                    "description": "Optional model ID or model slot alias for this subagent task. Omit it to use the agent default."
                },
                "timeout_seconds": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Optional timeout for this subagent task in seconds. Use 0 or omit it to disable the timeout."
                },
                "retry": {
                    "type": "boolean",
                    "description": "DeepReview only: true when this Task call is a retry for the same reviewer role after a timeout or failure in the current turn."
                }
            },
            "required": [
                "description",
                "prompt",
                "subagent_type"
            ]
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        let subagent_type = input
            .and_then(|v| v.get("subagent_type"))
            .and_then(|v| v.as_str());
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
        let validation = InputValidator::new(input)
            .validate_required("prompt")
            .validate_required("subagent_type")
            .finish();
        if !validation.result {
            return validation;
        }

        if let Some(prompt) = input.get("prompt").and_then(|value| value.as_str()) {
            let line_count = prompt.lines().count();
            let byte_count = prompt.len();
            if line_count > LARGE_TASK_PROMPT_SOFT_LINE_LIMIT
                || byte_count > LARGE_TASK_PROMPT_SOFT_BYTE_LIMIT
            {
                return ValidationResult {
                    result: true,
                    message: Some(format!(
                        "Large Task prompt: {} lines, {} bytes. This is allowed when necessary, but prefer staged delegation: split large work into multiple Task calls with clear ownership, and pass file paths, symbols, constraints, and exact questions instead of large pasted context.",
                        line_count, byte_count
                    )),
                    error_code: None,
                    meta: Some(json!({
                        "large_task_prompt": true,
                        "line_count": line_count,
                        "byte_count": byte_count,
                        "soft_line_limit": LARGE_TASK_PROMPT_SOFT_LINE_LIMIT,
                        "soft_byte_limit": LARGE_TASK_PROMPT_SOFT_BYTE_LIMIT
                    })),
                };
            }
        }

        validation
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        if let Some(description) = input.get("description").and_then(|v| v.as_str()) {
            if options.verbose {
                format!("Creating task: {}", description)
            } else {
                format!("Task: {}", description)
            }
        } else {
            "Creating task".to_string()
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let start_time = std::time::Instant::now();

        // description is only used for frontend display

        let mut prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BitFunError::tool(
                    "Required parameters: subagent_type, prompt, description. Missing prompt"
                        .to_string(),
                )
            })?
            .to_string();

        let subagent_type = input
            .get("subagent_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("Required parameters: subagent_type, prompt, description. Missing subagent_type".to_string()))?
            .to_string();
        let workspace_root = context.workspace_root();
        let all_agent_types = self.get_agents_types(workspace_root).await;
        if !all_agent_types.contains(&subagent_type) {
            return Err(BitFunError::tool(format!(
                "subagent_type {} is not valid, must be one of: {}",
                subagent_type,
                all_agent_types.join(", ")
            )));
        }

        let requested_workspace_path = input
            .get("workspace_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let model_id = match input.get("model_id") {
            Some(value) => {
                let value = value
                    .as_str()
                    .ok_or_else(|| BitFunError::tool("model_id must be a string".to_string()))?;
                let value = value.trim();
                (!value.is_empty()).then(|| value.to_string())
            }
            None => None,
        };
        let mut timeout_seconds = match input.get("timeout_seconds") {
            Some(value) => {
                let parsed = value.as_u64().ok_or_else(|| {
                    BitFunError::tool("timeout_seconds must be a non-negative integer".to_string())
                })?;
                (parsed > 0).then_some(parsed)
            }
            None => None,
        };
        let is_retry = input.get("retry").and_then(Value::as_bool).unwrap_or(false);
        let current_workspace_path = context
            .workspace_root()
            .map(|path| path.to_string_lossy().into_owned());
        if subagent_type == "Explore" || subagent_type == "FileFinder" {
            let workspace_path = requested_workspace_path
                .as_deref()
                .or(current_workspace_path.as_deref())
                .ok_or_else(|| {
                    BitFunError::tool(
                        "workspace_path is required for Explore/FileFinder agent".to_string(),
                    )
                })?;

            if workspace_path.is_empty() {
                return Err(BitFunError::tool(
                    "workspace_path cannot be empty for Explore/FileFinder agent".to_string(),
                ));
            }

            // For remote workspaces, skip local filesystem validation — the path
            // exists on the remote server, not locally.
            if !context.is_remote() {
                let path = std::path::Path::new(&workspace_path);
                if !path.exists() {
                    return Err(BitFunError::tool(format!(
                        "workspace_path '{}' does not exist",
                        workspace_path
                    )));
                }
                if !path.is_dir() {
                    return Err(BitFunError::tool(format!(
                        "workspace_path '{}' is not a directory",
                        workspace_path
                    )));
                }
            }

            prompt.push_str(&format!(
                "\n\nThe workspace you need to explore: {workspace_path}"
            ));
        }
        let effective_workspace_path = requested_workspace_path
            .clone()
            .or(current_workspace_path)
            .ok_or_else(|| {
                BitFunError::tool(
                    "workspace_path is required when the current workspace is unavailable"
                        .to_string(),
                )
            })?;

        let session_id = if let Some(session_id) = &context.session_id {
            session_id.clone()
        } else {
            return Err(BitFunError::tool(
                "session_id is required in context".to_string(),
            ));
        };

        // Get parent tool ID (tool_call_id)
        let tool_call_id = if let Some(tool_id) = &context.tool_call_id {
            tool_id.clone()
        } else {
            return Err(BitFunError::tool(
                "tool_call_id is required in context".to_string(),
            ));
        };

        // Get parent dialog turn ID (dialog_turn_id)
        let dialog_turn_id = if let Some(turn_id) = &context.dialog_turn_id {
            turn_id.clone()
        } else {
            return Err(BitFunError::tool(
                "dialog_turn_id is required in context".to_string(),
            ));
        };

        // Get global coordinator
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;

        if context
            .agent_type
            .as_deref()
            .map(str::trim)
            .is_some_and(|agent_type| agent_type == DEEP_REVIEW_AGENT_TYPE)
        {
            let base_policy = load_default_deep_review_policy().await.map_err(|error| {
                BitFunError::tool(format!(
                    "Failed to load DeepReview execution policy: {}",
                    error
                ))
            })?;
            let mut run_manifest = context.custom_data.get("deep_review_run_manifest").cloned();
            if run_manifest.is_none() {
                if let Some(workspace) = context.workspace.as_ref() {
                    let session_storage_path = workspace.session_storage_path();
                    match coordinator
                        .get_session_manager()
                        .load_session_metadata(&session_storage_path, &session_id)
                        .await
                    {
                        Ok(Some(metadata)) => {
                            run_manifest = metadata.deep_review_run_manifest;
                        }
                        Ok(None) => {}
                        Err(error) => {
                            warn!(
                                "Failed to load DeepReview session metadata for run-manifest policy: session_id={}, error={}",
                                session_id, error
                            );
                        }
                    }
                }
            }
            let policy = if let Some(manifest) = run_manifest.as_ref() {
                base_policy.with_run_manifest_execution_policy(manifest)
            } else {
                base_policy
            };
            let role = policy
                .classify_subagent(&subagent_type)
                .map_err(|violation| {
                    BitFunError::tool(format!(
                        "DeepReview Task policy violation: {}",
                        violation.to_tool_error_message()
                    ))
                })?;
            if let Some(gate) = run_manifest
                .as_ref()
                .and_then(DeepReviewRunManifestGate::from_value)
            {
                gate.ensure_active(&subagent_type).map_err(|violation| {
                    BitFunError::tool(format!(
                        "DeepReview Task policy violation: {}",
                        violation.to_tool_error_message()
                    ))
                })?;
            }
            let is_readonly = get_agent_registry()
                .get_subagent_is_readonly(&subagent_type)
                .unwrap_or(false);
            if !is_readonly {
                return Err(BitFunError::tool(format!(
                    "DeepReview Task policy violation: {}",
                    json!({
                        "code": "deep_review_subagent_not_readonly",
                        "message": format!(
                            "DeepReview review-phase subagent '{}' must be read-only",
                            subagent_type
                        )
                    })
                )));
            }
            let is_review = get_agent_registry()
                .get_subagent_is_review(&subagent_type)
                .unwrap_or(false);
            if !is_review {
                return Err(BitFunError::tool(format!(
                    "DeepReview Task policy violation: {}",
                    json!({
                        "code": "deep_review_subagent_not_review",
                        "message": format!(
                            "DeepReview review-phase subagent '{}' must be marked for review",
                            subagent_type
                        )
                    })
                )));
            }
            record_deep_review_task_budget(
                &dialog_turn_id,
                &policy,
                role,
                &subagent_type,
                is_retry,
            )
            .map_err(|violation| {
                BitFunError::tool(format!(
                    "DeepReview Task policy violation: {}",
                    violation.to_tool_error_message()
                ))
            })?;
            timeout_seconds = policy.effective_timeout_seconds(role, timeout_seconds);
        }

        let parent_info = SubagentParentInfo {
            tool_call_id,
            session_id,
            dialog_turn_id,
        };
        let result = coordinator
            .execute_subagent(
                subagent_type.clone(),
                prompt,
                parent_info,
                Some(effective_workspace_path.clone()),
                None,
                context.cancellation_token.as_ref(),
                model_id,
                timeout_seconds,
            )
            .await?;

        let duration = start_time.elapsed().as_millis();
        let status = if result.is_partial_timeout() {
            "partial_timeout"
        } else {
            "completed"
        };
        let result_for_assistant = if result.is_partial_timeout() {
            format!(
                "Subagent '{}' timed out with partial result:\n<partial_result status=\"partial_timeout\">\n{}\n</partial_result>",
                subagent_type, result.text
            )
        } else {
            format!(
                "Subagent '{}' completed successfully with result:\n<result>\n{}\n</result>",
                subagent_type, result.text
            )
        };
        let mut data = json!({
            "duration": duration,
            "status": status
        });
        if result.is_partial_timeout() {
            data["partial_output"] = json!(result.text);
            if let Some(reason) = result.reason.as_deref() {
                data["reason"] = json!(reason);
            }
            if let Some(event_id) = result.ledger_event_id() {
                data["ledger_event_id"] = json!(event_id);
            }
        }

        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::TaskTool;
    use crate::agentic::deep_review_policy::{
        DeepReviewBudgetTracker, DeepReviewExecutionPolicy, DeepReviewSubagentRole,
    };
    use crate::agentic::tools::framework::Tool;
    use serde_json::json;

    #[test]
    fn task_schema_accepts_optional_model_id() {
        let schema = TaskTool::new().input_schema();

        assert_eq!(schema["properties"]["model_id"]["type"], "string");
        assert!(!schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str() == Some("model_id")));
    }

    #[test]
    fn deep_review_policy_allows_only_configured_team_members() {
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "extra_subagent_ids": [
                "ExtraReviewer",
                "DeepReview",
                "ReviewFixer",
                "ReviewJudge",
                "ReviewBusinessLogic"
            ]
        })));

        assert_eq!(
            policy.classify_subagent("ReviewBusinessLogic").unwrap(),
            DeepReviewSubagentRole::Reviewer
        );
        assert_eq!(
            policy.classify_subagent("ExtraReviewer").unwrap(),
            DeepReviewSubagentRole::Reviewer
        );
        assert_eq!(
            policy.classify_subagent("ReviewJudge").unwrap(),
            DeepReviewSubagentRole::Judge
        );
        assert!(policy.classify_subagent("ReviewFixer").is_err());
        assert!(policy.classify_subagent("CodeReview").is_err());
        assert!(policy.classify_subagent("DeepReview").is_err());
    }

    #[test]
    fn deep_review_policy_caps_reviewer_and_judge_timeouts() {
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "reviewer_timeout_seconds": 300,
            "judge_timeout_seconds": 240
        })));

        assert_eq!(
            policy.effective_timeout_seconds(DeepReviewSubagentRole::Reviewer, Some(900)),
            Some(300)
        );
        assert_eq!(
            policy.effective_timeout_seconds(DeepReviewSubagentRole::Reviewer, None),
            Some(300)
        );
        assert_eq!(
            policy.effective_timeout_seconds(DeepReviewSubagentRole::Judge, Some(900)),
            Some(240)
        );
    }

    #[test]
    fn deep_review_policy_saturates_oversized_numeric_limits() {
        let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
            "reviewer_timeout_seconds": u64::MAX,
            "judge_timeout_seconds": u64::MAX
        })));

        assert_eq!(policy.reviewer_timeout_seconds, 3600);
        assert_eq!(policy.judge_timeout_seconds, 3600);
    }

    #[test]
    fn deep_review_budget_tracker_caps_judge_per_turn() {
        let policy = DeepReviewExecutionPolicy::default();
        let tracker = DeepReviewBudgetTracker::default();

        tracker
            .record_task(
                "turn-1",
                &policy,
                DeepReviewSubagentRole::Judge,
                "ReviewJudge",
                false,
            )
            .unwrap();
        assert!(tracker
            .record_task(
                "turn-1",
                &policy,
                DeepReviewSubagentRole::Judge,
                "ReviewJudge",
                false,
            )
            .is_err());

        tracker
            .record_task(
                "turn-2",
                &policy,
                DeepReviewSubagentRole::Judge,
                "ReviewJudge",
                false,
            )
            .unwrap();
    }
}
