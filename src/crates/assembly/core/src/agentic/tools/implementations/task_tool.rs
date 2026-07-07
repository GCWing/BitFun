use crate::agentic::agents::{
    get_agent_registry, AgentInfo, SubagentListScope, SubagentQueryContext,
};
use crate::agentic::coordination::{get_global_coordinator, SubagentExecutionRequest};
use crate::agentic::deep_review::task_adapter::{
    self as deep_review_task_adapter, DeepReviewLaunchBatchInfo,
    DeepReviewProviderQueueWaitOutcome, DeepReviewQueueWaitOutcome, DeepReviewQueueWaitSkipReason,
};
use crate::agentic::deep_review_policy::{
    deep_review_active_reviewer_count, deep_review_effective_parallel_instances,
    deep_review_has_judge_been_launched, deep_review_turn_elapsed_seconds,
    load_default_deep_review_policy, record_deep_review_effective_concurrency_success,
    record_deep_review_runtime_auto_retry, record_deep_review_runtime_auto_retry_suppressed,
    record_deep_review_runtime_manual_retry, record_deep_review_task_budget,
    DeepReviewActiveReviewerGuard, DeepReviewCapacityQueueReason, DeepReviewConcurrencyPolicy,
    DeepReviewExecutionPolicy, DeepReviewPolicyViolation, DeepReviewRunManifestGate,
    DeepReviewSubagentRole, DEEP_REVIEW_AGENT_TYPE,
};
use crate::agentic::events::DeepReviewQueueStatus;
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::pipeline::SubagentParentInfo;
use crate::agentic::tools::InputValidator;
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::types::AIConfig;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::timing::elapsed_ms_u64;
use async_trait::async_trait;
use bitfun_runtime_ports::SubagentContextMode;
use log::{debug, warn};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Instant;
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

    fn context_mode_from_input(input: &Value) -> BitFunResult<SubagentContextMode> {
        match input.get("fork_context") {
            None => Ok(SubagentContextMode::Fresh),
            Some(value) => {
                let fork_context = value.as_bool().ok_or_else(|| {
                    BitFunError::tool("fork_context must be a boolean".to_string())
                })?;
                Ok(if fork_context {
                    SubagentContextMode::Fork
                } else {
                    SubagentContextMode::Fresh
                })
            }
        }
    }

    fn invalid_input(message: impl Into<String>) -> ValidationResult {
        ValidationResult {
            result: false,
            message: Some(message.into()),
            error_code: None,
            meta: None,
        }
    }

    fn ensure_delegation_allowed(context: &ToolUseContext) -> BitFunResult<()> {
        let delegation_policy = context.delegation_policy();
        if delegation_policy.allow_subagent_spawn {
            return Ok(());
        }

        Err(BitFunError::tool(
            "Recursive subagent delegation is blocked. Use direct tools instead.".to_string(),
        ))
    }

    async fn load_configured_tool_execution_timeout() -> Option<u64> {
        let service = GlobalConfigManager::get_service().await.ok()?;
        let ai_config: AIConfig = service.get_config(Some("ai")).await.ok()?;
        ai_config
            .tool_execution_timeout_secs
            .filter(|seconds| *seconds > 0)
    }

    fn resolve_subagent_timeout_seconds(
        requested_timeout_seconds: Option<u64>,
        configured_execution_timeout_secs: Option<u64>,
    ) -> Option<u64> {
        match (
            requested_timeout_seconds.filter(|seconds| *seconds > 0),
            configured_execution_timeout_secs.filter(|seconds| *seconds > 0),
        ) {
            (Some(requested), Some(configured)) => Some(requested.max(configured)),
            (Some(requested), None) => Some(requested),
            (None, Some(configured)) => Some(configured),
            (None, None) => None,
        }
    }

    fn deep_review_launch_batch_for_task(
        subagent_type: &str,
        description: Option<&str>,
        run_manifest: Option<&Value>,
    ) -> Option<DeepReviewLaunchBatchInfo> {
        deep_review_task_adapter::deep_review_launch_batch_for_task(
            subagent_type,
            description,
            run_manifest,
        )
    }

    fn attach_deep_review_cache(run_manifest: &mut Value, cache_value: Option<Value>) {
        deep_review_task_adapter::attach_deep_review_cache(run_manifest, cache_value);
    }

    fn deep_review_retry_guidance_max_retries(
        effective_policy: Option<&DeepReviewExecutionPolicy>,
        dialog_turn_id: &str,
    ) -> usize {
        deep_review_task_adapter::deep_review_retry_guidance_max_retries(
            effective_policy,
            dialog_turn_id,
        )
    }

    fn should_emit_deep_review_retry_guidance(
        is_partial_timeout: bool,
        is_retry: bool,
        deep_review_subagent_role: Option<DeepReviewSubagentRole>,
    ) -> bool {
        deep_review_task_adapter::should_emit_deep_review_retry_guidance(
            is_partial_timeout,
            is_retry,
            deep_review_subagent_role,
        )
    }

    fn ensure_deep_review_retry_coverage(
        input: &Value,
        subagent_type: &str,
        run_manifest: Option<&Value>,
    ) -> Result<Vec<String>, DeepReviewPolicyViolation> {
        deep_review_task_adapter::ensure_deep_review_retry_coverage(
            input,
            subagent_type,
            run_manifest,
        )
    }

    fn is_deep_review_auto_retry(input: &Value) -> bool {
        input
            .get("auto_retry")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    fn auto_retry_suppression_reason(code: &str) -> &'static str {
        deep_review_task_adapter::auto_retry_suppression_reason(code)
    }

    fn ensure_deep_review_auto_retry_allowed(
        conc_policy: &DeepReviewConcurrencyPolicy,
        dialog_turn_id: &str,
    ) -> Result<(), DeepReviewPolicyViolation> {
        deep_review_task_adapter::ensure_deep_review_auto_retry_allowed(
            conc_policy,
            deep_review_turn_elapsed_seconds(dialog_turn_id),
        )
    }

    fn prompt_with_deep_review_retry_scope(prompt: &str, retry_scope_files: &[String]) -> String {
        deep_review_task_adapter::prompt_with_deep_review_retry_scope(prompt, retry_scope_files)
    }

    fn deep_review_capacity_decision_for_provider_error(
        error: &BitFunError,
    ) -> crate::agentic::deep_review_policy::DeepReviewCapacityQueueDecision {
        deep_review_task_adapter::capacity_decision_for_provider_error(error)
    }

    fn deep_review_capacity_skip_result_for_provider_queue_outcome(
        reason: DeepReviewCapacityQueueReason,
        dialog_turn_id: &str,
        subagent_type: &str,
        conc_policy: &DeepReviewConcurrencyPolicy,
        duration_ms: u128,
        queue_elapsed_ms: u64,
        terminal_skip_reason: Option<DeepReviewQueueWaitSkipReason>,
    ) -> (Value, String) {
        deep_review_task_adapter::capacity_skip_result_for_provider_queue_outcome(
            reason,
            dialog_turn_id,
            subagent_type,
            conc_policy,
            duration_ms,
            queue_elapsed_ms,
            terminal_skip_reason,
        )
    }

    async fn wait_for_deep_review_provider_capacity_retry(
        session_id: &str,
        dialog_turn_id: &str,
        tool_id: &str,
        subagent_type: &str,
        conc_policy: &DeepReviewConcurrencyPolicy,
        reason: DeepReviewCapacityQueueReason,
        max_wait_seconds: u64,
        is_optional_reviewer: bool,
    ) -> DeepReviewProviderQueueWaitOutcome {
        deep_review_task_adapter::wait_for_provider_capacity_retry(
            session_id,
            dialog_turn_id,
            tool_id,
            subagent_type,
            conc_policy,
            reason,
            max_wait_seconds,
            is_optional_reviewer,
        )
        .await
    }

    fn record_deep_review_provider_capacity_retry(
        dialog_turn_id: &str,
        reason: DeepReviewCapacityQueueReason,
    ) {
        deep_review_task_adapter::record_provider_capacity_retry(dialog_turn_id, reason);
    }

    fn record_deep_review_provider_capacity_retry_success(
        dialog_turn_id: &str,
        reason: DeepReviewCapacityQueueReason,
    ) {
        deep_review_task_adapter::record_provider_capacity_retry_success(dialog_turn_id, reason);
    }

    async fn emit_deep_review_queue_state(
        session_id: &str,
        dialog_turn_id: &str,
        tool_id: &str,
        subagent_type: &str,
        status: DeepReviewQueueStatus,
        reason: Option<DeepReviewCapacityQueueReason>,
        queued_reviewer_count: usize,
        active_reviewer_count: usize,
        optional_reviewer_count: Option<usize>,
        effective_parallel_instances: Option<usize>,
        queue_elapsed_ms: u64,
        max_queue_wait_seconds: u64,
    ) {
        deep_review_task_adapter::emit_queue_state(
            session_id,
            dialog_turn_id,
            tool_id,
            subagent_type,
            status,
            reason,
            queued_reviewer_count,
            active_reviewer_count,
            optional_reviewer_count,
            effective_parallel_instances,
            queue_elapsed_ms,
            max_queue_wait_seconds,
        )
        .await;
    }

    fn try_begin_deep_review_reviewer_admission(
        dialog_turn_id: &str,
        effective_parallel_instances: usize,
        launch_batch_info: Option<&DeepReviewLaunchBatchInfo>,
    ) -> Result<Option<DeepReviewActiveReviewerGuard<'static>>, DeepReviewPolicyViolation> {
        deep_review_task_adapter::try_begin_reviewer_admission(
            dialog_turn_id,
            effective_parallel_instances,
            launch_batch_info,
        )
    }

    async fn wait_for_deep_review_reviewer_admission(
        session_id: &str,
        dialog_turn_id: &str,
        tool_id: &str,
        subagent_type: &str,
        conc_policy: &DeepReviewConcurrencyPolicy,
        is_optional_reviewer: bool,
        launch_batch_info: Option<&DeepReviewLaunchBatchInfo>,
    ) -> BitFunResult<DeepReviewQueueWaitOutcome> {
        deep_review_task_adapter::wait_for_reviewer_admission(
            session_id,
            dialog_turn_id,
            tool_id,
            subagent_type,
            conc_policy,
            is_optional_reviewer,
            launch_batch_info,
        )
        .await
    }

    fn deep_review_local_capacity_skip_tool_result(
        dialog_turn_id: &str,
        subagent_type: &str,
        conc_policy: &DeepReviewConcurrencyPolicy,
        capacity_reason: DeepReviewCapacityQueueReason,
        skip_reason: DeepReviewQueueWaitSkipReason,
        queue_elapsed_ms: u64,
        duration_ms: u128,
    ) -> ToolResult {
        let (data, assistant_message) =
            deep_review_task_adapter::capacity_skip_result_for_local_queue_outcome(
                dialog_turn_id,
                subagent_type,
                conc_policy,
                capacity_reason,
                skip_reason,
                queue_elapsed_ms,
                duration_ms,
            );
        ToolResult::Result {
            data,
            result_for_assistant: Some(assistant_message),
            image_attachments: None,
        }
    }

    fn deep_review_cancelled_reviewer_tool_result(
        subagent_type: &str,
        reason: &str,
        duration_ms: u128,
    ) -> ToolResult {
        let (data, result_for_assistant) =
            deep_review_task_adapter::deep_review_cancelled_reviewer_result(
                subagent_type,
                reason,
                duration_ms,
            );

        ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }
    }

    fn format_agent_descriptions(agents: &[AgentInfo]) -> String {
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

    fn render_description(&self) -> String {
        r#"Launch a new agent to handle complex, multi-step tasks autonomously.

The Task tool launches specialized agents (subprocesses) that autonomously handle complex tasks. Each agent type has specific capabilities and tools available to it.

The current agent listing includes an <available_agents> section when subagents are available. Use the exact `type` attribute from that section as `subagent_type` when `fork_context` is false or omitted.

Supported context behaviors:
- `fork_context=false` (default): start a new subagent context from scratch. You must provide `subagent_type`. In this mode, include the necessary background information in the prompt.
- `fork_context=true`: start an isolated child session that inherits your current context and tools. Do not provide `subagent_type`, `workspace_path`, or `model_id` in this mode. Here the prompt is a directive: what to do, not what the situation is. Be specific about scope: what's in, what's out, and what another agent is handling. Don't re-explain background.

Do not put `fork_context`, `subagent_type`, `description`, `workspace_path`, `model_id`, or `timeout_seconds` inside the prompt string.

When to use the Task tool:
- Delegate when a specialized subagent or separate context is likely to improve coverage, independence, or parallelism.
- Use direct tools instead for focused lookups, known paths, single symbols, or code that can be inspected in a few reads/searches.

Usage notes:
- Include a short description summarizing what the agent will do.
- Provide a clear prompt so the agent can work autonomously and return the information you need.
- When `fork_context` is false, if 'workspace_path' is omitted, the task inherits the current workspace by default.
- When `fork_context` is false, provide 'workspace_path' when the selected agent requires an explicit workspace.
- When `fork_context` is false, use 'model_id' when a caller needs a specific model or model slot for the subagent. Omit it to use the agent default.
- When `fork_context` is true, the child session reuses the parent session's agent type, workspace, and prompt cache while still running in isolation.
- Use 'timeout_seconds' when you need a hard deadline for the subagent. When omitted, the session execution timeout from settings is used. When provided, the effective timeout is the larger of the requested value and the session execution timeout. Set it to 0 with no configured session execution timeout to disable the timeout.
- For DeepReview only, set 'retry' to true when re-dispatching a reviewer after that same reviewer returned partial_timeout or an explicit transient capacity failure in the current turn. Retry calls must include retry_coverage with source_packet_id, source_status, covered_files, and a smaller retry_scope_files list. Do not set 'auto_retry' unless this is a backend-owned automatic retry admitted by Review Team settings; model-issued retry decisions should omit it or set it to false. Example retry_coverage: {{ "source_packet_id": "reviewer-123", "source_status": "partial_timeout", "covered_files": ["src/main.rs"], "retry_scope_files": ["src/parser.rs"] }}.
- Launch independent agents concurrently when that improves coverage or latency; send parallel Task calls in a single assistant message.
- When launching multiple non-read-only subagents in parallel, assign non-overlapping scopes and outputs so their file edits, commands, or external side effects do not conflict.
- When the agent is done, it will return a single message back to you.
- Treat subagent outputs as useful evidence, but verify details yourself before making edits or final claims that depend on exact code.
- Clearly tell the agent whether you expect it to write code or just to do research (search, file reads, web fetches, etc.), since it is not aware of the user's intent.
- If the agent description mentions proactive use, consider it when relevant and use your judgement.
- If the user explicitly asks to run agents in parallel, send the independent Task calls together in one message."#
            .to_string()
    }

    pub(crate) async fn build_available_agents_context_section(
        context: Option<&ToolUseContext>,
    ) -> Option<String> {
        let agents = Self::get_enabled_agents(context).await;
        let agent_descriptions = Self::format_agent_descriptions(&agents);
        if agent_descriptions.trim().is_empty() {
            None
        } else {
            Some(agent_descriptions)
        }
    }

    async fn get_enabled_agents(context: Option<&ToolUseContext>) -> Vec<AgentInfo> {
        let registry = get_agent_registry();
        let workspace_root = context.and_then(|ctx| ctx.workspace_root());
        registry.load_custom_agents(workspace_root).await;
        registry
            .get_subagents_for_query(&SubagentQueryContext {
                parent_agent_type: context.and_then(|ctx| ctx.agent_type.as_deref()),
                workspace_root,
                list_scope: SubagentListScope::TaskVisible,
                include_disabled: false,
            })
            .await
    }

    async fn get_agents_types(&self, context: Option<&ToolUseContext>) -> Vec<String> {
        Self::get_enabled_agents(context)
            .await
            .into_iter()
            .map(|agent| agent.id)
            .collect()
    }

    fn background_subagent_started_assistant_message(
        delegate_target_label: &str,
        background_task_id: &str,
    ) -> String {
        format!(
            "Background {} started successfully.\n<background_task status=\"started\" id=\"{}\">Its final result will be delivered back automatically to you when it is finished. Do not poll for status updates. If your current path is blocked on this result and there is no other useful local work to do, it is fine to end the current turn.</background_task>",
            delegate_target_label, background_task_id
        )
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "Task"
    }

    fn manages_own_execution_timeout(&self) -> bool {
        true
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(self.render_description())
    }

    async fn is_available_in_context(&self, _context: Option<&ToolUseContext>) -> bool {
        // Keep Task prompt-visible even when no fresh subagents are currently
        // available. Hiding it based on transient subagent availability makes
        // the tool manifest drift across turns and causes provider prefix/KV
        // cache misses. Task also still supports `fork_context=true` in that
        // state, so removing it from the manifest would be behaviorally wrong.
        true
    }

    fn short_description(&self) -> String {
        "Delegate work to a subagent task and collect the result.".to_string()
    }

    async fn description_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Ok(self.render_description())
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
                    "description": "The task for the agent to perform. Keep it scoped and concise. Do not include top-level Task arguments such as fork_context or subagent_type inside this string. The 180-line / 16KB guideline is a soft reliability threshold, not a hard cap. For large delegations, split into multiple Task calls with clear ownership, and pass file paths, symbols, constraints, and exact questions instead of pasting large file contents."
                },
                "fork_context": {
                    "type": "boolean",
                    "default": false,
                    "description": "Optional. Defaults to false. Set true to fork the parent session into an isolated child session that reuses the parent agent type, latest runtime context, and prompt cache. Leave false to launch a specialized fresh subagent chosen by subagent_type."
                },
                "subagent_type": {
                    "type": "string",
                    "description": "Required top-level agent type id when fork_context is false or omitted."
                },
                "workspace_path": {
                    "type": "string",
                    "description": "Only used when fork_context is false. The absolute path of the workspace for this task. If omitted, inherits the current workspace."
                },
                "model_id": {
                    "type": "string",
                    "description": "Only used when fork_context is false. Optional model ID or model slot alias for this subagent task. Omit it to use the agent default."
                },
                "timeout_seconds": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Optional timeout for this subagent task in seconds. When omitted, the session execution timeout from settings is used. When provided, the effective timeout is the larger of this value and the session execution timeout. Use 0 with no configured session execution timeout to disable the timeout."
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Optional. When true, start the subagent in the background and return immediately. The final result will be delivered back to the parent agent by steering if it is still running, or by starting a new turn if it is idle."
                },
                "retry": {
                    "type": "boolean",
                    "description": "DeepReview only: true when this Task call is a retry for the same reviewer role after partial_timeout or an explicit transient capacity failure in the current turn."
                },
                "auto_retry": {
                    "type": "boolean",
                    "description": "DeepReview only: true only for backend-owned bounded automatic retries. Requires Review Team auto retry opt-in and retry=true. User/model-issued retry actions must omit this field or set it to false."
                },
                "retry_coverage": {
                    "type": "object",
                    "description": "DeepReview retry only: structured coverage metadata proving the retry is bounded. Required when retry=true.",
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
                "prompt"
            ],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        if input
            .and_then(|value| Self::context_mode_from_input(value).ok())
            .is_some_and(|mode| mode == SubagentContextMode::Fork)
        {
            return false;
        }
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
            .validate_required("description")
            .validate_required("prompt")
            .finish();
        if !validation.result {
            return validation;
        }

        let context_mode = match Self::context_mode_from_input(input) {
            Ok(mode) => mode,
            Err(error) => return Self::invalid_input(error.to_string()),
        };

        match context_mode {
            SubagentContextMode::Fresh => {
                if input.get("subagent_type").is_none() {
                    return Self::invalid_input(
                        "subagent_type is required when fork_context is false or omitted",
                    );
                }
            }
            SubagentContextMode::Fork => {
                for field in [
                    "subagent_type",
                    "workspace_path",
                    "model_id",
                    "retry",
                    "auto_retry",
                    "retry_coverage",
                ] {
                    if input.get(field).is_some() {
                        return Self::invalid_input(format!(
                            "{field} is not allowed when fork_context is true"
                        ));
                    }
                }
            }
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

        Self::ensure_delegation_allowed(context)?;

        // description is only used for frontend display
        let description = input
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string);

        let mut prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BitFunError::tool(
                    "Required parameters: prompt and description. Missing prompt".to_string(),
                )
            })?
            .to_string();
        let context_mode = Self::context_mode_from_input(input)?;

        let subagent_type = match context_mode {
            SubagentContextMode::Fresh => {
                let subagent_type = input
                    .get("subagent_type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "subagent_type is required when fork_context is false or omitted"
                                .to_string(),
                        )
                    })?
                    .to_string();
                let all_agent_types = self.get_agents_types(Some(context)).await;
                if !all_agent_types.contains(&subagent_type) {
                    return Err(BitFunError::tool(format!(
                        "subagent_type {} is not valid, must be one of: {}",
                        subagent_type,
                        all_agent_types.join(", ")
                    )));
                }
                Some(subagent_type)
            }
            SubagentContextMode::Fork => None,
        };
        let delegate_target_label = match subagent_type.as_deref() {
            Some(subagent_type) => format!("subagent '{}'", subagent_type),
            None => "forked subagent".to_string(),
        };

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
        let run_in_background = input
            .get("run_in_background")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let is_retry = input.get("retry").and_then(Value::as_bool).unwrap_or(false);
        let requested_auto_retry = Self::is_deep_review_auto_retry(input);
        let is_auto_retry = is_retry && requested_auto_retry;
        let current_workspace_path = context
            .workspace_root()
            .map(|path| path.to_string_lossy().into_owned());
        if context_mode == SubagentContextMode::Fork {
            if requested_workspace_path.is_some() {
                return Err(BitFunError::tool(
                    "workspace_path is not allowed when fork_context is true".to_string(),
                ));
            }
            if model_id.is_some() {
                return Err(BitFunError::tool(
                    "model_id is not allowed when fork_context is true".to_string(),
                ));
            }
            if is_retry || requested_auto_retry || input.get("retry_coverage").is_some() {
                return Err(BitFunError::tool(
                    "DeepReview retry fields are not allowed when fork_context is true".to_string(),
                ));
            }
        }
        let effective_workspace_path = if let Some(subagent_type) = subagent_type.as_deref() {
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

                // For remote workspaces, skip local filesystem validation - the path
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

            Some(
                requested_workspace_path
                    .clone()
                    .or(current_workspace_path.clone())
                    .ok_or_else(|| {
                        BitFunError::tool(
                            "workspace_path is required when the current workspace is unavailable"
                                .to_string(),
                        )
                    })?,
            )
        } else {
            None
        };

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
        let mut deep_review_effective_policy: Option<DeepReviewExecutionPolicy> = None;
        let mut deep_review_active_guard: Option<DeepReviewActiveReviewerGuard<'static>> = None;
        let mut deep_review_reviewer_configured_max_parallel_instances: Option<usize> = None;
        let mut deep_review_concurrency_policy: Option<DeepReviewConcurrencyPolicy> = None;
        let mut deep_review_is_optional_reviewer = false;
        let mut deep_review_launch_batch_info: Option<DeepReviewLaunchBatchInfo> = None;
        let mut deep_review_retry_scope_files: Option<Vec<String>> = None;
        let mut deep_review_subagent_role: Option<DeepReviewSubagentRole> = None;

        // Get global coordinator
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;

        let is_deep_review_parent = context
            .agent_type
            .as_deref()
            .map(str::trim)
            .is_some_and(|agent_type| agent_type == DEEP_REVIEW_AGENT_TYPE);
        if context_mode == SubagentContextMode::Fork && is_deep_review_parent {
            return Err(BitFunError::tool(
                "fork_context=true is not supported for DeepReview Task calls".to_string(),
            ));
        }

        if is_deep_review_parent {
            let subagent_type = subagent_type.as_deref().ok_or_else(|| {
                BitFunError::tool("subagent_type is required for DeepReview Task calls".to_string())
            })?;
            let base_policy = load_default_deep_review_policy().await.map_err(|error| {
                BitFunError::tool(format!(
                    "Failed to load DeepReview execution policy: {}",
                    error
                ))
            })?;
            let mut run_manifest = context.custom_data.get("deep_review_run_manifest").cloned();
            if let Some(workspace) = context.workspace.as_ref() {
                let session_storage_dir = workspace.session_storage_dir();
                match coordinator
                    .get_session_manager()
                    .load_session_metadata(&session_storage_dir, &session_id)
                    .await
                {
                    Ok(Some(metadata)) => {
                        if run_manifest.is_none() {
                            run_manifest = metadata.deep_review_run_manifest;
                        }
                        if let Some(run_manifest) = run_manifest.as_mut() {
                            Self::attach_deep_review_cache(
                                run_manifest,
                                metadata.deep_review_cache,
                            );
                        }
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
            let policy = if let Some(manifest) = run_manifest.as_ref() {
                base_policy.with_run_manifest_execution_policy(manifest)
            } else {
                base_policy
            };
            deep_review_effective_policy = Some(policy.clone());
            let role = policy
                .classify_subagent(subagent_type)
                .map_err(|violation| {
                    BitFunError::tool(format!(
                        "DeepReview Task policy violation: {}",
                        violation.to_tool_error_message()
                    ))
                })?;
            deep_review_subagent_role = Some(role);
            if requested_auto_retry && !is_retry {
                return Err(BitFunError::tool(
                    "auto_retry requires retry=true for DeepReview Task calls".to_string(),
                ));
            }
            if let Some(gate) = run_manifest
                .as_ref()
                .and_then(DeepReviewRunManifestGate::from_value)
            {
                gate.ensure_active(subagent_type).map_err(|violation| {
                    BitFunError::tool(format!(
                        "DeepReview Task policy violation: {}",
                        violation.to_tool_error_message()
                    ))
                })?;
            }
            let conc_policy = policy
                .concurrency_policy_from_manifest(run_manifest.as_ref().unwrap_or(&Value::Null));
            deep_review_concurrency_policy = Some(conc_policy.clone());
            if is_retry && role == DeepReviewSubagentRole::Reviewer {
                deep_review_retry_scope_files = Some(
                    match Self::ensure_deep_review_retry_coverage(
                        input,
                        subagent_type,
                        run_manifest.as_ref(),
                    ) {
                        Ok(retry_scope_files) => retry_scope_files,
                        Err(violation) => {
                            if is_auto_retry {
                                record_deep_review_runtime_auto_retry_suppressed(
                                    &dialog_turn_id,
                                    Self::auto_retry_suppression_reason(violation.code),
                                );
                            }
                            return Err(BitFunError::tool(format!(
                                "DeepReview Task policy violation: {}",
                                violation.to_tool_error_message()
                            )));
                        }
                    },
                );
                if is_auto_retry {
                    Self::ensure_deep_review_auto_retry_allowed(&conc_policy, &dialog_turn_id)
                        .map_err(|violation| {
                            record_deep_review_runtime_auto_retry_suppressed(
                                &dialog_turn_id,
                                Self::auto_retry_suppression_reason(violation.code),
                            );
                            BitFunError::tool(format!(
                                "DeepReview Task policy violation: {}",
                                violation.to_tool_error_message()
                            ))
                        })?;
                }
            }
            let is_readonly = get_agent_registry()
                .get_subagent_is_readonly(subagent_type)
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
                .get_subagent_is_review(subagent_type)
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
            timeout_seconds = policy.effective_timeout_seconds(role, timeout_seconds);

            // Check incremental review cache before queueing. A cache hit does
            // not consume runtime reviewer capacity or reviewer timeout.
            if role == DeepReviewSubagentRole::Reviewer && !is_retry {
                if let Some(cache_hit) =
                    deep_review_task_adapter::deep_review_incremental_cache_hit_for_task(
                        subagent_type,
                        description.as_deref(),
                        run_manifest.as_ref(),
                    )
                {
                    let (data, cached_result) =
                        deep_review_task_adapter::deep_review_incremental_cache_hit_result(
                            subagent_type,
                            &cache_hit,
                        );
                    return Ok(vec![ToolResult::ok(data, Some(cached_result))]);
                }
            }

            // Enforce dynamic concurrency policy from the run manifest.
            match role {
                DeepReviewSubagentRole::Reviewer => {
                    deep_review_reviewer_configured_max_parallel_instances =
                        Some(conc_policy.max_parallel_instances);
                    let effective_parallel_instances = deep_review_effective_parallel_instances(
                        &dialog_turn_id,
                        conc_policy.max_parallel_instances,
                    );
                    let is_optional_reviewer = policy
                        .extra_subagent_ids
                        .iter()
                        .any(|id| id == subagent_type);
                    deep_review_is_optional_reviewer = is_optional_reviewer;
                    deep_review_launch_batch_info = Self::deep_review_launch_batch_for_task(
                        subagent_type,
                        description.as_deref(),
                        run_manifest.as_ref(),
                    );
                    match Self::try_begin_deep_review_reviewer_admission(
                        &dialog_turn_id,
                        effective_parallel_instances,
                        deep_review_launch_batch_info.as_ref(),
                    ) {
                        Ok(Some(guard)) => {
                            deep_review_active_guard = Some(guard);
                        }
                        Ok(None)
                        | Err(DeepReviewPolicyViolation {
                            code: "deep_review_launch_batch_blocked",
                            ..
                        }) => {
                            match Self::wait_for_deep_review_reviewer_admission(
                                &session_id,
                                &dialog_turn_id,
                                &tool_call_id,
                                subagent_type,
                                &conc_policy,
                                is_optional_reviewer,
                                deep_review_launch_batch_info.as_ref(),
                            )
                            .await?
                            {
                                DeepReviewQueueWaitOutcome::Ready { guard } => {
                                    deep_review_active_guard = Some(guard);
                                }
                                DeepReviewQueueWaitOutcome::Skipped {
                                    queue_elapsed_ms,
                                    skip_reason,
                                    capacity_reason,
                                } => {
                                    return Ok(vec![
                                        Self::deep_review_local_capacity_skip_tool_result(
                                            &dialog_turn_id,
                                            subagent_type,
                                            &conc_policy,
                                            capacity_reason,
                                            skip_reason,
                                            queue_elapsed_ms,
                                            start_time.elapsed().as_millis(),
                                        ),
                                    ]);
                                }
                            }
                        }
                        Err(violation) => {
                            return Err(BitFunError::tool(format!(
                                "DeepReview Task policy violation: {}",
                                violation.to_tool_error_message()
                            )));
                        }
                    }
                }
                DeepReviewSubagentRole::Judge => {
                    let active_reviewers = deep_review_active_reviewer_count(&dialog_turn_id);
                    let judge_pending = deep_review_has_judge_been_launched(&dialog_turn_id);
                    conc_policy
                        .check_launch_allowed(active_reviewers, role, judge_pending)
                        .map_err(|violation| {
                            BitFunError::tool(format!(
                                "DeepReview concurrency policy violation: {}",
                                violation.to_tool_error_message()
                            ))
                        })?;
                }
            }
            record_deep_review_task_budget(&dialog_turn_id, &policy, role, subagent_type, is_retry)
                .map_err(|violation| {
                    if is_auto_retry {
                        record_deep_review_runtime_auto_retry_suppressed(
                            &dialog_turn_id,
                            Self::auto_retry_suppression_reason(violation.code),
                        );
                    }
                    BitFunError::tool(format!(
                        "DeepReview Task policy violation: {}",
                        violation.to_tool_error_message()
                    ))
                })?;
            if is_retry && role == DeepReviewSubagentRole::Reviewer {
                if is_auto_retry {
                    record_deep_review_runtime_auto_retry(&dialog_turn_id);
                } else {
                    record_deep_review_runtime_manual_retry(&dialog_turn_id);
                }
            }
        }

        if deep_review_subagent_role.is_none() {
            let configured_timeout = Self::load_configured_tool_execution_timeout().await;
            timeout_seconds =
                Self::resolve_subagent_timeout_seconds(timeout_seconds, configured_timeout);
        }

        if let Some(retry_scope_files) = deep_review_retry_scope_files.as_ref() {
            prompt = Self::prompt_with_deep_review_retry_scope(&prompt, retry_scope_files);
        }

        let subagent_context = deep_review_subagent_role.map(|role| {
            let mut values = HashMap::new();
            values.insert(
                "deep_review_subagent_role".to_string(),
                match role {
                    DeepReviewSubagentRole::Reviewer => "reviewer",
                    DeepReviewSubagentRole::Judge => "judge",
                }
                .to_string(),
            );
            if let Some(subagent_type) = subagent_type.as_ref() {
                values.insert(
                    "deep_review_subagent_type".to_string(),
                    subagent_type.clone(),
                );
            }
            values
        });
        let prepared_prompt = prompt;
        if run_in_background {
            let parent_info = SubagentParentInfo {
                tool_call_id: tool_call_id.clone(),
                session_id: session_id.clone(),
                dialog_turn_id: dialog_turn_id.clone(),
            };
            let background_result = coordinator
                .start_background_subagent(
                    SubagentExecutionRequest {
                        task_description: prepared_prompt.clone(),
                        context_mode,
                        subagent_type: subagent_type.clone(),
                        workspace_path: effective_workspace_path.clone(),
                        model_id: model_id.clone(),
                        subagent_parent_info: parent_info,
                        context: subagent_context.clone().unwrap_or_default(),
                        delegation_policy: context.delegation_policy().spawn_child(),
                    },
                    timeout_seconds,
                )
                .await?;
            return Ok(vec![ToolResult::Result {
                data: json!({
                    "context_mode": context_mode.as_str(),
                    "status": "started",
                    "run_in_background": true,
                    "background_task_id": background_result.background_task_id,
                }),
                result_for_assistant: Some(Self::background_subagent_started_assistant_message(
                    &delegate_target_label,
                    &background_result.background_task_id,
                )),
                image_attachments: None,
            }]);
        }
        let mut provider_capacity_retry =
            deep_review_task_adapter::DeepReviewProviderCapacityRetryRuntime::default();
        let deep_review_subagent_id = subagent_type.as_deref().unwrap_or("");
        let result = loop {
            let parent_info = SubagentParentInfo {
                tool_call_id: tool_call_id.clone(),
                session_id: session_id.clone(),
                dialog_turn_id: dialog_turn_id.clone(),
            };
            let subagent_execution_started_at = Instant::now();
            debug!(
                "TaskTool awaiting subagent result: parent_session_id={}, dialog_turn_id={}, tool_call_id={}, context_mode={}, delegate_target={}, timeout_seconds={:?}, workspace_path={:?}, model_id={:?}",
                session_id,
                dialog_turn_id,
                tool_call_id,
                context_mode.as_str(),
                delegate_target_label,
                timeout_seconds,
                effective_workspace_path,
                model_id
            );
            let execution_result = coordinator
                .execute_subagent(
                    SubagentExecutionRequest {
                        task_description: prepared_prompt.clone(),
                        context_mode,
                        subagent_type: subagent_type.clone(),
                        workspace_path: effective_workspace_path.clone(),
                        model_id: model_id.clone(),
                        subagent_parent_info: parent_info,
                        context: subagent_context.clone().unwrap_or_default(),
                        delegation_policy: context.delegation_policy().spawn_child(),
                    },
                    context.cancellation_token(),
                    timeout_seconds,
                )
                .await;

            match execution_result {
                Ok(result) => {
                    debug!(
                        "TaskTool subagent returned: parent_session_id={}, dialog_turn_id={}, tool_call_id={}, context_mode={}, delegate_target={}, status={:?}, text_len={}, duration_ms={}, ledger_event_id={:?}",
                        session_id,
                        dialog_turn_id,
                        tool_call_id,
                        context_mode.as_str(),
                        delegate_target_label,
                        result.status,
                        result.text.len(),
                        elapsed_ms_u64(subagent_execution_started_at),
                        result.ledger_event_id()
                    );
                    if let Some(reason) = provider_capacity_retry.last_retry_reason() {
                        Self::record_deep_review_provider_capacity_retry_success(
                            &dialog_turn_id,
                            reason,
                        );
                    }
                    break result;
                }
                Err(error) => {
                    warn!(
                        "TaskTool subagent failed: parent_session_id={}, dialog_turn_id={}, tool_call_id={}, context_mode={}, delegate_target={}, duration_ms={}, error={}",
                        session_id,
                        dialog_turn_id,
                        tool_call_id,
                        context_mode.as_str(),
                        delegate_target_label,
                        elapsed_ms_u64(subagent_execution_started_at),
                        error
                    );
                    if matches!(
                        deep_review_subagent_role,
                        Some(DeepReviewSubagentRole::Reviewer)
                    ) && matches!(error, BitFunError::Cancelled(_))
                        && !context
                            .cancellation_token()
                            .as_ref()
                            .is_some_and(|token| token.is_cancelled())
                    {
                        let reason = match &error {
                            BitFunError::Cancelled(reason) => reason.as_str(),
                            _ => "",
                        };
                        return Ok(vec![Self::deep_review_cancelled_reviewer_tool_result(
                            deep_review_subagent_id,
                            reason,
                            start_time.elapsed().as_millis(),
                        )]);
                    }
                    if matches!(
                        deep_review_subagent_role,
                        Some(DeepReviewSubagentRole::Reviewer)
                    ) {
                        if let Some(conc_policy) = deep_review_concurrency_policy.as_ref() {
                            let decision =
                                Self::deep_review_capacity_decision_for_provider_error(&error);
                            match provider_capacity_retry
                                .decide_after_error(&decision, conc_policy)
                            {
                                deep_review_task_adapter::DeepReviewProviderCapacityRetryDecision::NotQueueable => {}
                                deep_review_task_adapter::DeepReviewProviderCapacityRetryDecision::CapacitySkipped {
                                    reason,
                                    queue_elapsed_ms,
                                } => {
                                    drop(deep_review_active_guard.take());
                                    let (data, assistant_message) = Self::deep_review_capacity_skip_result_for_provider_queue_outcome(
                                        reason,
                                        &dialog_turn_id,
                                        deep_review_subagent_id,
                                        conc_policy,
                                        start_time.elapsed().as_millis(),
                                        queue_elapsed_ms,
                                        None,
                                    );
                                    let effective_parallel_instances = data
                                        .get("effective_parallel_instances")
                                        .and_then(Value::as_u64)
                                        .and_then(|value| usize::try_from(value).ok());
                                    Self::emit_deep_review_queue_state(
                                        &session_id,
                                        &dialog_turn_id,
                                        &tool_call_id,
                                        deep_review_subagent_id,
                                        DeepReviewQueueStatus::CapacitySkipped,
                                        Some(reason),
                                        0,
                                        deep_review_active_reviewer_count(&dialog_turn_id),
                                        deep_review_is_optional_reviewer.then_some(1),
                                        effective_parallel_instances,
                                        queue_elapsed_ms,
                                        conc_policy.max_queue_wait_seconds,
                                    )
                                    .await;
                                    return Ok(vec![ToolResult::Result {
                                        data,
                                        result_for_assistant: Some(assistant_message),
                                        image_attachments: None,
                                    }]);
                                }
                                deep_review_task_adapter::DeepReviewProviderCapacityRetryDecision::WaitForCapacity {
                                    reason,
                                    max_wait_seconds,
                                } => {
                                    drop(deep_review_active_guard.take());
                                    match Self::wait_for_deep_review_provider_capacity_retry(
                                        &session_id,
                                        &dialog_turn_id,
                                        &tool_call_id,
                                        deep_review_subagent_id,
                                        conc_policy,
                                        reason,
                                        max_wait_seconds,
                                        deep_review_is_optional_reviewer,
                                    )
                                    .await
                                    {
                                        DeepReviewProviderQueueWaitOutcome::ReadyToRetry {
                                            queue_elapsed_ms,
                                            early_capacity_probe,
                                        } => {
                                            provider_capacity_retry.record_ready_to_retry(
                                                reason,
                                                queue_elapsed_ms,
                                                early_capacity_probe,
                                            );
                                            let effective_parallel_instances =
                                                deep_review_effective_parallel_instances(
                                                    &dialog_turn_id,
                                                    conc_policy.max_parallel_instances,
                                                );
                                            match Self::try_begin_deep_review_reviewer_admission(
                                                &dialog_turn_id,
                                                effective_parallel_instances,
                                                deep_review_launch_batch_info.as_ref(),
                                            ) {
                                                Ok(Some(guard)) => {
                                                    deep_review_active_guard = Some(guard);
                                                }
                                                Ok(None)
                                                | Err(DeepReviewPolicyViolation {
                                                    code: "deep_review_launch_batch_blocked",
                                                    ..
                                                }) => {
                                                    match Self::wait_for_deep_review_reviewer_admission(
                                                        &session_id,
                                                        &dialog_turn_id,
                                                        &tool_call_id,
                                                        deep_review_subagent_id,
                                                        conc_policy,
                                                        deep_review_is_optional_reviewer,
                                                        deep_review_launch_batch_info.as_ref(),
                                                    )
                                                    .await?
                                                    {
                                                        DeepReviewQueueWaitOutcome::Ready { guard } => {
                                                            deep_review_active_guard = Some(guard);
                                                        }
                                                        DeepReviewQueueWaitOutcome::Skipped {
                                                            queue_elapsed_ms,
                                                            skip_reason,
                                                            capacity_reason,
                                                        } => {
                                                            return Ok(vec![
                                                                Self::deep_review_local_capacity_skip_tool_result(
                                                                    &dialog_turn_id,
                                                                    deep_review_subagent_id,
                                                                    conc_policy,
                                                                    capacity_reason,
                                                                    skip_reason,
                                                                    queue_elapsed_ms,
                                                                    start_time.elapsed().as_millis(),
                                                                ),
                                                            ]);
                                                        }
                                                    }
                                                }
                                                Err(violation) => {
                                                    return Err(BitFunError::tool(format!(
                                                        "DeepReview Task policy violation: {}",
                                                        violation.to_tool_error_message()
                                                    )));
                                                }
                                            }
                                            Self::record_deep_review_provider_capacity_retry(
                                                &dialog_turn_id,
                                                reason,
                                            );
                                            continue;
                                        }
                                        DeepReviewProviderQueueWaitOutcome::Skipped {
                                            queue_elapsed_ms,
                                            skip_reason,
                                        } => {
                                            let total_provider_capacity_queue_elapsed_ms =
                                                provider_capacity_retry
                                                    .record_queue_skipped(queue_elapsed_ms);
                                            let (data, assistant_message) = Self::deep_review_capacity_skip_result_for_provider_queue_outcome(
                                                reason,
                                                &dialog_turn_id,
                                                deep_review_subagent_id,
                                                conc_policy,
                                                start_time.elapsed().as_millis(),
                                                total_provider_capacity_queue_elapsed_ms,
                                                Some(skip_reason),
                                            );
                                            return Ok(vec![ToolResult::Result {
                                                data,
                                                result_for_assistant: Some(assistant_message),
                                                image_attachments: None,
                                            }]);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return Err(error);
                }
            }
        };
        if !result.is_partial_timeout() {
            if let Some(configured_max_parallel_instances) =
                deep_review_reviewer_configured_max_parallel_instances
            {
                record_deep_review_effective_concurrency_success(
                    &dialog_turn_id,
                    configured_max_parallel_instances,
                );
            }
        }
        drop(deep_review_active_guard);

        let duration = start_time.elapsed().as_millis();

        // Build retry hint for deep review reviewer timeouts.
        let retry_hint = if Self::should_emit_deep_review_retry_guidance(
            result.is_partial_timeout(),
            is_retry,
            deep_review_subagent_role,
        ) {
            let retries_used = crate::agentic::deep_review_policy::deep_review_retries_used(
                &dialog_turn_id,
                deep_review_subagent_id,
            );
            let max_retries = Self::deep_review_retry_guidance_max_retries(
                deep_review_effective_policy.as_ref(),
                &dialog_turn_id,
            );
            deep_review_task_adapter::deep_review_retry_guidance(retries_used, max_retries)
        } else {
            String::new()
        };

        let (data, result_for_assistant) =
            deep_review_task_adapter::deep_review_task_completion_result(
                &delegate_target_label,
                &result.text,
                context_mode.as_str(),
                duration,
                result.is_partial_timeout(),
                result.reason.as_deref(),
                result.ledger_event_id(),
                &retry_hint,
            );

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
    use crate::agentic::agents::CustomSubagentConfig;
    use crate::agentic::agents::{
        get_agent_registry, Agent, AgentCategory, SubAgentSource, UserContextPolicy,
    };
    use crate::agentic::deep_review::task_adapter as deep_review_task_adapter;
    use crate::agentic::deep_review_policy::{
        DeepReviewBudgetTracker, DeepReviewExecutionPolicy, DeepReviewSubagentRole,
    };
    use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use async_trait::async_trait;
    use bitfun_runtime_ports::DelegationPolicy;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;

    struct PromptOrderTestAgent {
        id: String,
    }

    #[async_trait]
    impl Agent for PromptOrderTestAgent {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.id
        }

        fn description(&self) -> &str {
            "Prompt ordering test agent"
        }

        fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
            "test_prompt_order_agent"
        }

        fn user_context_policy(&self) -> UserContextPolicy {
            UserContextPolicy::empty()
        }

        fn default_tools(&self) -> Vec<String> {
            vec!["Read".to_string()]
        }
    }

    fn register_prompt_order_test_subagent(
        id: &str,
        source: SubAgentSource,
        custom_config: Option<CustomSubagentConfig>,
    ) {
        get_agent_registry().register_agent(
            Arc::new(PromptOrderTestAgent { id: id.to_string() }),
            AgentCategory::SubAgent,
            match source {
                SubAgentSource::Builtin => crate::agentic::agents::AgentSource::Builtin,
                SubAgentSource::Project => crate::agentic::agents::AgentSource::Project,
                SubAgentSource::User => crate::agentic::agents::AgentSource::User,
            },
            Some(source),
            custom_config,
        );
    }

    fn find_agent_block_index(description: &str, agent_id: &str) -> usize {
        description
            .find(&format!("<agent type=\"{}\">", agent_id))
            .unwrap_or_else(|| panic!("expected agent block for {}", agent_id))
    }

    #[test]
    fn task_prompt_guidance_omits_subagent_name_examples() {
        let description = TaskTool::new().render_description();
        assert!(!description.contains("subagent_type=\"Explore\""));
        assert!(!description.contains("subagent_type=\"FileFinder\""));
        assert!(!description.contains("For Explore"));
        assert!(!description.contains("Explore/FileFinder"));
        assert!(!description.contains("file-discovery"));
        assert!(!description.contains("listed investigation"));

        let schema = TaskTool::new().input_schema();
        let subagent_description = schema["properties"]["subagent_type"]["description"]
            .as_str()
            .expect("subagent_type description should be a string");
        assert!(!subagent_description.contains("Explore"));
        assert!(!subagent_description.contains("FileFinder"));
        assert!(!subagent_description.contains("available_agents"));
    }

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
    fn task_schema_supports_fork_context_flag() {
        let schema = TaskTool::new().input_schema();

        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"]["fork_context"]["type"], "boolean");
        assert_eq!(schema["properties"]["fork_context"]["default"], false);
        assert!(schema["properties"]["subagent_type"]["description"]
            .as_str()
            .unwrap()
            .contains("fork_context is false or omitted"));
        assert!(schema["properties"]["prompt"]["description"]
            .as_str()
            .unwrap()
            .contains("Do not include top-level Task arguments"));
        assert!(schema.get("allOf").is_none());
    }

    #[test]
    fn background_subagent_start_acknowledgement_keeps_structured_task_marker() {
        let message = TaskTool::background_subagent_started_assistant_message(
            "GeneralPurpose",
            "bg-subagent-123",
        );

        assert!(message.starts_with("Background GeneralPurpose started successfully."));
        assert!(message.contains("<background_task status=\"started\" id=\"bg-subagent-123\">"));
        assert!(message.contains("Do not poll for status updates."));
        assert!(message.ends_with("</background_task>"));
        assert!(!message.contains("background_task_id="));
    }

    #[tokio::test]
    async fn validate_input_requires_subagent_type_when_not_forking() {
        let validation = TaskTool::new()
            .validate_input(
                &json!({
                    "description": "delegate",
                    "prompt": "Inspect the repo"
                }),
                None,
            )
            .await;

        assert!(!validation.result);
        assert!(validation
            .message
            .as_deref()
            .is_some_and(|message| message.contains("subagent_type is required")));
    }

    #[tokio::test]
    async fn task_tool_stays_available_without_enabled_subagents() {
        assert!(
            TaskTool::new().is_available_in_context(None).await,
            "Task should remain prompt-visible even when no fresh subagents are currently available"
        );
    }

    #[tokio::test]
    async fn validate_input_rejects_fork_context_conflicting_fields() {
        let validation = TaskTool::new()
            .validate_input(
                &json!({
                    "description": "delegate",
                    "prompt": "Continue with inherited context",
                    "fork_context": true,
                    "subagent_type": "Explore"
                }),
                None,
            )
            .await;

        assert!(!validation.result);
        assert!(validation
            .message
            .as_deref()
            .is_some_and(|message| message.contains("subagent_type is not allowed")));
    }

    #[tokio::test]
    async fn call_impl_rejects_nested_subagent_delegation() {
        let policy = DelegationPolicy::top_level().spawn_child();
        let context = ToolUseContext {
            tool_call_id: Some("tool-call-1".to_string()),
            agent_type: Some("agentic".to_string()),
            session_id: Some("session-1".to_string()),
            dialog_turn_id: Some("turn-1".to_string()),
            workspace: None,
            unlocked_collapsed_tools: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::from([
                (
                    "delegation_allow_subagent_spawn".to_string(),
                    json!(policy.allow_subagent_spawn),
                ),
                (
                    "delegation_nesting_depth".to_string(),
                    json!(policy.nesting_depth),
                ),
            ]),
            computer_use_host: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        };

        let error = TaskTool::new()
            .call_impl(
                &json!({
                    "description": "delegate",
                    "prompt": "Inspect the repo",
                    "subagent_type": "Explore"
                }),
                &context,
            )
            .await
            .expect_err("nested subagent delegation should be rejected");

        assert!(error
            .to_string()
            .contains("Recursive subagent delegation is blocked. Use direct tools instead."));
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
    fn resolve_subagent_timeout_uses_session_execution_timeout_as_floor() {
        assert_eq!(
            TaskTool::resolve_subagent_timeout_seconds(Some(300), Some(1200)),
            Some(1200)
        );
        assert_eq!(
            TaskTool::resolve_subagent_timeout_seconds(None, Some(1200)),
            Some(1200)
        );
        assert_eq!(
            TaskTool::resolve_subagent_timeout_seconds(Some(1800), Some(1200)),
            Some(1800)
        );
        assert_eq!(
            TaskTool::resolve_subagent_timeout_seconds(Some(300), None),
            Some(300)
        );
        assert_eq!(TaskTool::resolve_subagent_timeout_seconds(None, None), None);
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
    fn deep_review_cancelled_reviewer_result_tells_parent_not_to_relaunch() {
        let result = TaskTool::deep_review_cancelled_reviewer_tool_result(
            "ReviewArchitecture",
            "Subagent task has been cancelled",
            42,
        );

        let ToolResult::Result {
            data,
            result_for_assistant,
            image_attachments,
        } = result
        else {
            panic!("cancelled reviewer should return a structured tool result");
        };

        assert_eq!(data["status"], "cancelled");
        assert_eq!(data["reason"], "Subagent task has been cancelled");
        assert_eq!(data["duration"], 42);
        assert!(image_attachments.is_none());

        let assistant_message = result_for_assistant.expect("assistant message should be present");
        assert!(assistant_message.contains("status=\"cancelled\""));
        assert!(assistant_message.contains("do not relaunch it automatically"));
    }

    #[tokio::test]
    async fn description_with_context_filters_restricted_subagents_by_parent_agent() {
        let agentic_context = ToolUseContext {
            tool_call_id: None,
            agent_type: Some("agentic".to_string()),
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            unlocked_collapsed_tools: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        };
        let deep_review_context = ToolUseContext {
            agent_type: Some("DeepReview".to_string()),
            ..agentic_context.clone()
        };

        let agentic_description =
            TaskTool::build_available_agents_context_section(Some(&agentic_context))
                .await
                .expect("agentic available agents should render");
        assert!(agentic_description.contains("<agent type=\"Explore\">"));
        assert!(!agentic_description.contains("<agent type=\"ReviewSecurity\">"));
        assert!(!agentic_description.contains("<agent type=\"ResearchSpecialist\">"));

        let deep_review_description =
            TaskTool::build_available_agents_context_section(Some(&deep_review_context))
                .await
                .expect("deep review available agents should render");
        assert!(deep_review_description.contains("<agent type=\"ReviewSecurity\">"));
        assert!(!deep_review_description.contains("<agent type=\"ResearchSpecialist\">"));
    }

    #[tokio::test]
    async fn prompt_stability_description_with_context_renders_available_agents_in_stable_order() {
        let context = ToolUseContext {
            tool_call_id: None,
            agent_type: Some("agentic".to_string()),
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            unlocked_collapsed_tools: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        };

        let builtin_a = "AAAPromptOrderBuiltin";
        let builtin_z = "ZZZPromptOrderBuiltin";
        let project_a = "AAAPromptOrderProject";
        let project_z = "ZZZPromptOrderProject";
        register_prompt_order_test_subagent(builtin_z, SubAgentSource::Builtin, None);
        register_prompt_order_test_subagent(builtin_a, SubAgentSource::Builtin, None);
        register_prompt_order_test_subagent(
            project_z,
            SubAgentSource::Project,
            Some(CustomSubagentConfig {
                model: "fast".to_string(),
            }),
        );
        register_prompt_order_test_subagent(
            project_a,
            SubAgentSource::Project,
            Some(CustomSubagentConfig {
                model: "fast".to_string(),
            }),
        );

        let description = TaskTool::build_available_agents_context_section(Some(&context))
            .await
            .expect("available agents should render");

        let builtin_a_index = find_agent_block_index(&description, builtin_a);
        let builtin_z_index = find_agent_block_index(&description, builtin_z);
        let project_a_index = find_agent_block_index(&description, project_a);
        let project_z_index = find_agent_block_index(&description, project_z);

        assert!(
            builtin_a_index < builtin_z_index,
            "builtin subagents should be sorted alphabetically"
        );
        assert!(
            builtin_z_index < project_a_index,
            "builtin subagents should render before project subagents"
        );
        assert!(
            project_a_index < project_z_index,
            "project subagents should be sorted alphabetically"
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

    #[test]
    fn deep_review_concurrency_policy_blocks_reviewer_at_cap() {
        use crate::agentic::deep_review_policy::DeepReviewConcurrencyPolicy;

        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 2,
            stagger_seconds: 0,
            max_queue_wait_seconds: 60,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };
        // 0 active -> allowed
        assert!(policy
            .check_launch_allowed(0, DeepReviewSubagentRole::Reviewer, false)
            .is_ok());
        // 1 active -> allowed
        assert!(policy
            .check_launch_allowed(1, DeepReviewSubagentRole::Reviewer, false)
            .is_ok());
        // 2 active (at cap) -> blocked
        assert!(policy
            .check_launch_allowed(2, DeepReviewSubagentRole::Reviewer, false)
            .is_err());
    }

    #[test]
    fn deep_review_concurrency_policy_returns_structured_cap_rejection() {
        use crate::agentic::deep_review_policy::DeepReviewConcurrencyPolicy;

        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 2,
            stagger_seconds: 0,
            max_queue_wait_seconds: 60,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };
        let violation = policy
            .check_launch_allowed(2, DeepReviewSubagentRole::Reviewer, false)
            .expect_err("reviewer launch at cap should be rejected");
        let message = format!(
            "DeepReview concurrency policy violation: {}",
            violation.to_tool_error_message()
        );

        assert!(message.contains("deep_review_concurrency_cap_reached"));
        assert!(message.contains("Maximum parallel reviewer instances reached"));
    }

    #[tokio::test]
    async fn deep_review_capacity_queue_waits_while_active_reviewer_is_running() {
        use crate::agentic::deep_review_policy::{
            deep_review_capacity_skip_count, deep_review_concurrency_cap_rejection_count,
            deep_review_effective_parallel_instances, try_begin_deep_review_active_reviewer,
            DeepReviewConcurrencyPolicy,
        };

        let turn_id = "turn-queue-active-wait";
        let tool_id = "tool-queue-active-wait";
        let occupied_a = try_begin_deep_review_active_reviewer(turn_id, 2)
            .expect("precondition should occupy first reviewer capacity");
        let occupied_b = try_begin_deep_review_active_reviewer(turn_id, 2)
            .expect("precondition should occupy second reviewer capacity");
        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 2,
            stagger_seconds: 0,
            max_queue_wait_seconds: 0,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };
        let turn_id_owned = turn_id.to_string();
        let tool_id_owned = tool_id.to_string();

        let handle = tokio::spawn(async move {
            deep_review_task_adapter::wait_for_reviewer_admission(
                "session-queue-active-wait",
                &turn_id_owned,
                &tool_id_owned,
                "ReviewSecurity",
                &policy,
                false,
                None,
            )
            .await
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        assert!(
            !handle.is_finished(),
            "active Deep Review reviewers should keep the queued reviewer alive"
        );

        drop(occupied_a);
        drop(occupied_b);

        let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(500), handle)
            .await
            .expect("queue should become ready after active reviewers finish")
            .expect("spawned wait should not panic")
            .expect("queue wait should resolve");

        match outcome {
            super::DeepReviewQueueWaitOutcome::Ready { .. } => {}
            super::DeepReviewQueueWaitOutcome::Skipped { .. } => {
                panic!("active Deep Review reviewers should not cause a queue-expired skip");
            }
        }
        assert_eq!(deep_review_capacity_skip_count(turn_id), 0);
        assert_eq!(deep_review_concurrency_cap_rejection_count(turn_id), 0);
        assert_eq!(deep_review_effective_parallel_instances(turn_id, 2), 2);
    }

    #[tokio::test]
    async fn deep_review_capacity_queue_starts_later_batch_when_reviewer_capacity_frees() {
        use crate::agentic::deep_review::task_adapter::DeepReviewLaunchBatchInfo;
        use crate::agentic::deep_review_policy::{
            deep_review_capacity_skip_count, deep_review_effective_parallel_instances,
            try_begin_deep_review_active_reviewer_for_launch_batch, DeepReviewConcurrencyPolicy,
        };

        let turn_id = "turn-launch-batch-fill-free-slot";
        let tool_id = "tool-launch-batch-fill-free-slot";
        let occupied_a =
            try_begin_deep_review_active_reviewer_for_launch_batch(turn_id, 2, 1, Some("packet-a"))
                .expect("launch batch admission should not fail")
                .expect("first batch reviewer should start");
        let occupied_b =
            try_begin_deep_review_active_reviewer_for_launch_batch(turn_id, 2, 1, Some("packet-b"))
                .expect("launch batch admission should not fail")
                .expect("second first-batch reviewer should start");
        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 2,
            stagger_seconds: 0,
            max_queue_wait_seconds: 0,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };
        let launch_batch_info = DeepReviewLaunchBatchInfo {
            packet_id: Some("packet-b".to_string()),
            launch_batch: 2,
        };
        let turn_id_owned = turn_id.to_string();
        let tool_id_owned = tool_id.to_string();

        let handle = tokio::spawn(async move {
            TaskTool::wait_for_deep_review_reviewer_admission(
                "session-launch-batch-queue-wait",
                &turn_id_owned,
                &tool_id_owned,
                "ReviewSecurity",
                &policy,
                false,
                Some(&launch_batch_info),
            )
            .await
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        assert!(
            !handle.is_finished(),
            "later launch batch should wait while reviewer capacity is full"
        );
        drop(occupied_a);

        let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(500), handle)
            .await
            .expect("later launch batch should become ready as soon as reviewer capacity frees")
            .expect("spawned wait should not panic")
            .expect("queue wait should resolve");

        match outcome {
            super::DeepReviewQueueWaitOutcome::Ready { .. } => {}
            super::DeepReviewQueueWaitOutcome::Skipped { .. } => {
                panic!("later launch batch should not expire after reviewer capacity frees");
            }
        }
        drop(occupied_b);
        assert_eq!(deep_review_capacity_skip_count(turn_id), 0);
        assert_eq!(deep_review_effective_parallel_instances(turn_id, 2), 2);
    }

    #[tokio::test]
    async fn deep_review_capacity_queue_cancel_control_skips_waiting_reviewer() {
        use crate::agentic::deep_review_policy::{
            apply_deep_review_queue_control, deep_review_capacity_skip_count,
            try_begin_deep_review_active_reviewer, DeepReviewConcurrencyPolicy,
            DeepReviewQueueControlAction,
        };

        let turn_id = "turn-queue-cancel";
        let tool_id = "tool-queue-cancel";
        let _occupied = try_begin_deep_review_active_reviewer(turn_id, 1)
            .expect("precondition should occupy reviewer capacity");
        apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Cancel);
        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 1,
            stagger_seconds: 0,
            max_queue_wait_seconds: 60,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };

        let outcome = deep_review_task_adapter::wait_for_reviewer_admission(
            "session-queue-cancel",
            turn_id,
            tool_id,
            "ReviewSecurity",
            &policy,
            false,
            None,
        )
        .await
        .expect("queue wait should resolve");

        match outcome {
            super::DeepReviewQueueWaitOutcome::Skipped {
                queue_elapsed_ms, ..
            } => {
                assert!(queue_elapsed_ms < 100);
            }
            super::DeepReviewQueueWaitOutcome::Ready { .. } => {
                panic!("cancelled queue control should skip the waiting reviewer");
            }
        }
        assert_eq!(deep_review_capacity_skip_count(turn_id), 1);
    }

    #[tokio::test]
    async fn deep_review_capacity_queue_records_one_runtime_wait_when_ready() {
        use crate::agentic::deep_review_policy::{
            deep_review_runtime_diagnostics_snapshot, try_begin_deep_review_active_reviewer,
            DeepReviewConcurrencyPolicy,
        };

        let turn_id = "turn-queue-ready-diagnostics";
        let tool_id = "tool-queue-ready-diagnostics";
        let occupied = try_begin_deep_review_active_reviewer(turn_id, 1)
            .expect("precondition should occupy reviewer capacity");
        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 1,
            stagger_seconds: 0,
            max_queue_wait_seconds: 1,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };
        let turn_id_owned = turn_id.to_string();
        let tool_id_owned = tool_id.to_string();

        let handle = tokio::spawn(async move {
            deep_review_task_adapter::wait_for_reviewer_admission(
                "session-queue-ready-diagnostics",
                &turn_id_owned,
                &tool_id_owned,
                "ReviewSecurity",
                &policy,
                false,
                None,
            )
            .await
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        drop(occupied);

        let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(500), handle)
            .await
            .expect("queue should become ready after capacity frees")
            .expect("spawned wait should not panic")
            .expect("queue wait should resolve");
        match outcome {
            super::DeepReviewQueueWaitOutcome::Ready { .. } => {}
            super::DeepReviewQueueWaitOutcome::Skipped { .. } => {
                panic!("freed capacity should allow the queued reviewer to run");
            }
        }

        let diagnostics = deep_review_runtime_diagnostics_snapshot(turn_id)
            .expect("runtime diagnostics should record terminal queue wait");
        assert_eq!(diagnostics.queue_wait_count, 1);
        assert_eq!(
            diagnostics.queue_wait_total_ms,
            diagnostics.queue_wait_max_ms
        );
    }

    #[tokio::test]
    async fn deep_review_capacity_queue_pause_does_not_expire_until_continued() {
        use crate::agentic::deep_review_policy::{
            apply_deep_review_queue_control, try_begin_deep_review_active_reviewer,
            DeepReviewConcurrencyPolicy, DeepReviewQueueControlAction,
        };

        let turn_id = "turn-queue-pause";
        let tool_id = "tool-queue-pause";
        let occupied = try_begin_deep_review_active_reviewer(turn_id, 1)
            .expect("precondition should occupy reviewer capacity");
        apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Pause);
        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 1,
            stagger_seconds: 0,
            max_queue_wait_seconds: 0,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };
        let turn_id_owned = turn_id.to_string();
        let tool_id_owned = tool_id.to_string();

        let handle = tokio::spawn(async move {
            deep_review_task_adapter::wait_for_reviewer_admission(
                "session-queue-pause",
                &turn_id_owned,
                &tool_id_owned,
                "ReviewSecurity",
                &policy,
                false,
                None,
            )
            .await
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        assert!(
            !handle.is_finished(),
            "paused queue wait should not expire while user pause is active"
        );

        apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Continue);
        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        assert!(
            !handle.is_finished(),
            "continued queue wait should stay alive while reviewer capacity is still active"
        );
        drop(occupied);

        let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(500), handle)
            .await
            .expect("continued queue wait should finish")
            .expect("spawned wait should not panic")
            .expect("queue wait should resolve");
        match outcome {
            super::DeepReviewQueueWaitOutcome::Ready { .. } => {}
            super::DeepReviewQueueWaitOutcome::Skipped { .. } => {
                panic!("continued queue wait should run after reviewer capacity frees");
            }
        }
    }

    #[tokio::test]
    async fn deep_review_capacity_queue_skip_optional_skips_optional_waiter() {
        use crate::agentic::deep_review_policy::{
            apply_deep_review_queue_control, try_begin_deep_review_active_reviewer,
            DeepReviewConcurrencyPolicy, DeepReviewQueueControlAction,
        };

        let turn_id = "turn-queue-skip-optional";
        let tool_id = "tool-queue-skip-optional";
        let _occupied = try_begin_deep_review_active_reviewer(turn_id, 1)
            .expect("precondition should occupy reviewer capacity");
        apply_deep_review_queue_control(
            turn_id,
            tool_id,
            DeepReviewQueueControlAction::SkipOptional,
        );
        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 1,
            stagger_seconds: 0,
            max_queue_wait_seconds: 60,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };

        let outcome = deep_review_task_adapter::wait_for_reviewer_admission(
            "session-queue-skip-optional",
            turn_id,
            tool_id,
            "ReviewCustom",
            &policy,
            true,
            None,
        )
        .await
        .expect("queue wait should resolve");

        match outcome {
            super::DeepReviewQueueWaitOutcome::Skipped {
                queue_elapsed_ms, ..
            } => {
                assert!(queue_elapsed_ms < 100);
            }
            super::DeepReviewQueueWaitOutcome::Ready { .. } => {
                panic!("optional queue control should skip optional reviewer");
            }
        }
    }

    #[test]
    fn deep_review_concurrency_policy_blocks_judge_with_active_reviewers() {
        use crate::agentic::deep_review_policy::DeepReviewConcurrencyPolicy;

        let policy = DeepReviewConcurrencyPolicy::default();
        // 1 active reviewer -> judge blocked
        assert!(policy
            .check_launch_allowed(1, DeepReviewSubagentRole::Judge, false)
            .is_err());
        // 0 active reviewers, no judge pending -> judge allowed
        assert!(policy
            .check_launch_allowed(0, DeepReviewSubagentRole::Judge, false)
            .is_ok());
        // 0 active reviewers, judge already pending -> blocked
        assert!(policy
            .check_launch_allowed(0, DeepReviewSubagentRole::Judge, true)
            .is_err());
    }

    #[test]
    fn deep_review_retry_guidance_includes_budget_info() {
        // Verify that the retry budget tracking functions work correctly
        // for the retry guidance injected in task_tool.
        use crate::agentic::deep_review_policy::{
            deep_review_max_retries_per_role, deep_review_retries_used,
        };

        // Default max retries should be 1
        assert_eq!(deep_review_max_retries_per_role("nonexistent-turn"), 1);

        // Retries used for a nonexistent turn should be 0
        assert_eq!(
            deep_review_retries_used("nonexistent-turn", "ReviewSecurity"),
            0
        );
    }

    #[test]
    fn deep_review_retry_guidance_uses_manifest_policy_limit() {
        use crate::agentic::deep_review_policy::DeepReviewExecutionPolicy;

        let manifest = serde_json::json!({
            "reviewMode": "deep",
            "executionPolicy": {
                "maxRetriesPerRole": 2
            }
        });
        let policy =
            DeepReviewExecutionPolicy::default().with_run_manifest_execution_policy(&manifest);

        assert_eq!(
            TaskTool::deep_review_retry_guidance_max_retries(Some(&policy), "nonexistent-turn"),
            2
        );
    }

    #[test]
    fn deep_review_retry_guidance_only_applies_to_initial_reviewer_timeout() {
        assert!(TaskTool::should_emit_deep_review_retry_guidance(
            true,
            false,
            Some(DeepReviewSubagentRole::Reviewer)
        ));
        assert!(!TaskTool::should_emit_deep_review_retry_guidance(
            true, false, None
        ));
        assert!(!TaskTool::should_emit_deep_review_retry_guidance(
            true,
            false,
            Some(DeepReviewSubagentRole::Judge)
        ));
        assert!(!TaskTool::should_emit_deep_review_retry_guidance(
            true,
            true,
            Some(DeepReviewSubagentRole::Reviewer)
        ));
        assert!(!TaskTool::should_emit_deep_review_retry_guidance(
            false,
            false,
            Some(DeepReviewSubagentRole::Reviewer)
        ));
    }

    #[test]
    fn deep_review_auto_retry_requires_review_team_opt_in() {
        use crate::agentic::deep_review_policy::DeepReviewConcurrencyPolicy;

        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 4,
            stagger_seconds: 0,
            max_queue_wait_seconds: 60,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };

        let violation =
            TaskTool::ensure_deep_review_auto_retry_allowed(&policy, "turn-auto-retry-disabled")
                .expect_err("auto retry must be disabled by default");

        assert_eq!(violation.code, "deep_review_auto_retry_disabled");
        assert_eq!(
            TaskTool::auto_retry_suppression_reason(violation.code),
            "auto_retry_disabled"
        );
    }

    #[test]
    fn deep_review_auto_retry_opt_in_allows_guarded_admission() {
        use crate::agentic::deep_review_policy::DeepReviewConcurrencyPolicy;

        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 4,
            stagger_seconds: 0,
            max_queue_wait_seconds: 60,
            batch_extras_separately: true,
            allow_bounded_auto_retry: true,
            auto_retry_elapsed_guard_seconds: 180,
        };

        TaskTool::ensure_deep_review_auto_retry_allowed(&policy, "turn-auto-retry-enabled")
            .expect("opted-in auto retry should pass the admission gate before budget checks");
    }

    #[test]
    fn deep_review_retry_rejects_missing_structured_coverage() {
        let manifest = json!({
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewSecurity:group-1-of-1",
                    "phase": "reviewer",
                    "subagentId": "ReviewSecurity",
                    "timeoutSeconds": 600,
                    "assignedScope": {
                        "files": [
                            "src/crates/assembly/core/src/auth.rs",
                            "src/crates/assembly/core/src/token.rs"
                        ]
                    }
                }
            ]
        });
        let input = json!({
            "retry": true
        });

        let violation =
            TaskTool::ensure_deep_review_retry_coverage(&input, "ReviewSecurity", Some(&manifest))
                .expect_err("missing retry coverage should be rejected");

        assert_eq!(violation.code, "deep_review_retry_missing_coverage");
    }

    #[test]
    fn deep_review_retry_rejects_broad_scope() {
        let manifest = json!({
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewSecurity:group-1-of-1",
                    "phase": "reviewer",
                    "subagentId": "ReviewSecurity",
                    "timeoutSeconds": 600,
                    "assignedScope": {
                        "files": [
                            "src/crates/assembly/core/src/auth.rs",
                            "src/crates/assembly/core/src/token.rs"
                        ]
                    }
                }
            ]
        });
        let input = json!({
            "retry": true,
            "timeout_seconds": 300,
            "retry_coverage": {
                "source_packet_id": "reviewer:ReviewSecurity:group-1-of-1",
                "source_status": "partial_timeout",
                "covered_files": [
                    "src/crates/assembly/core/src/auth.rs"
                ],
                "retry_scope_files": [
                    "src/crates/assembly/core/src/auth.rs",
                    "src/crates/assembly/core/src/token.rs"
                ]
            }
        });

        let violation =
            TaskTool::ensure_deep_review_retry_coverage(&input, "ReviewSecurity", Some(&manifest))
                .expect_err("retrying the full packet should be rejected");

        assert_eq!(violation.code, "deep_review_retry_scope_not_reduced");
    }

    #[test]
    fn deep_review_retry_rejects_timeout_that_is_not_lowered() {
        let manifest = json!({
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewSecurity:group-1-of-1",
                    "phase": "reviewer",
                    "subagentId": "ReviewSecurity",
                    "timeoutSeconds": 600,
                    "assignedScope": {
                        "files": [
                            "src/crates/assembly/core/src/auth.rs",
                            "src/crates/assembly/core/src/token.rs"
                        ]
                    }
                }
            ]
        });
        let input = json!({
            "retry": true,
            "timeout_seconds": 600,
            "retry_coverage": {
                "source_packet_id": "reviewer:ReviewSecurity:group-1-of-1",
                "source_status": "partial_timeout",
                "covered_files": [
                    "src/crates/assembly/core/src/auth.rs"
                ],
                "retry_scope_files": [
                    "src/crates/assembly/core/src/token.rs"
                ]
            }
        });

        let violation =
            TaskTool::ensure_deep_review_retry_coverage(&input, "ReviewSecurity", Some(&manifest))
                .expect_err("retry timeout must be lower than source timeout");

        assert_eq!(violation.code, "deep_review_retry_timeout_not_reduced");
    }

    #[test]
    fn deep_review_retry_rejects_non_queueable_capacity_reason() {
        let manifest = json!({
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewSecurity:group-1-of-1",
                    "phase": "reviewer",
                    "subagentId": "ReviewSecurity",
                    "timeoutSeconds": 600,
                    "assignedScope": {
                        "files": [
                            "src/crates/assembly/core/src/auth.rs",
                            "src/crates/assembly/core/src/token.rs"
                        ]
                    }
                }
            ]
        });
        let input = json!({
            "retry": true,
            "retry_coverage": {
                "source_packet_id": "reviewer:ReviewSecurity:group-1-of-1",
                "source_status": "capacity_skipped",
                "capacity_reason": "auth_error",
                "covered_files": [],
                "retry_scope_files": [
                    "src/crates/assembly/core/src/token.rs"
                ]
            }
        });

        let violation =
            TaskTool::ensure_deep_review_retry_coverage(&input, "ReviewSecurity", Some(&manifest))
                .expect_err("non-queueable capacity failures must fail fast");

        assert_eq!(violation.code, "deep_review_retry_non_retryable_status");
    }

    #[test]
    fn deep_review_provider_capacity_error_builds_capacity_skipped_payload_and_lowers_effective_cap(
    ) {
        use crate::agentic::deep_review_policy::{
            deep_review_effective_concurrency_snapshot, DeepReviewConcurrencyPolicy,
        };
        use crate::util::BitFunError;

        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 3,
            stagger_seconds: 0,
            max_queue_wait_seconds: 30,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };
        let turn_id = "turn-provider-capacity-skip";
        let decision =
            TaskTool::deep_review_capacity_decision_for_provider_error(&BitFunError::ai(
                "Provider error: provider=openai, code=429, message=rate limit exceeded",
            ));
        assert!(decision.queueable);
        let reason = decision
            .reason
            .expect("provider rate limit should surface as capacity_skipped");
        let (data, assistant_message) =
            TaskTool::deep_review_capacity_skip_result_for_provider_queue_outcome(
                reason,
                turn_id,
                "ReviewSecurity",
                &policy,
                42,
                0,
                None,
            );

        assert_eq!(data["status"], "capacity_skipped");
        assert_eq!(data["queue_skip_reason"], "provider_rate_limit");
        assert_eq!(data["effective_parallel_instances"], 2);
        assert!(assistant_message.contains("status=\"capacity_skipped\""));
        assert!(assistant_message.contains("reason=\"provider_rate_limit\""));
        assert_eq!(
            deep_review_effective_concurrency_snapshot(turn_id, 3).effective_parallel_instances,
            2
        );
    }

    #[test]
    fn deep_review_provider_quota_error_is_not_capacity_skipped() {
        use crate::util::BitFunError;

        let decision = TaskTool::deep_review_capacity_decision_for_provider_error(
            &BitFunError::ai("Provider error: provider=glm, code=1113, message=insufficient quota"),
        );

        assert!(
            !decision.queueable,
            "quota errors should remain fail-fast instead of entering capacity queue flow"
        );
    }

    #[tokio::test]
    async fn deep_review_provider_capacity_queue_retries_when_active_reviewer_frees_capacity() {
        use crate::agentic::deep_review::task_adapter::DeepReviewProviderQueueWaitOutcome;
        use crate::agentic::deep_review_policy::{
            try_begin_deep_review_active_reviewer, DeepReviewCapacityQueueReason,
            DeepReviewConcurrencyPolicy,
        };

        let turn_id = "turn-provider-queue-active-release";
        let tool_id = "tool-provider-queue-active-release";
        let occupied = try_begin_deep_review_active_reviewer(turn_id, 2)
            .expect("precondition should occupy another reviewer slot");
        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 2,
            stagger_seconds: 0,
            max_queue_wait_seconds: 60,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };
        let turn_id_owned = turn_id.to_string();
        let tool_id_owned = tool_id.to_string();

        let handle = tokio::spawn(async move {
            TaskTool::wait_for_deep_review_provider_capacity_retry(
                "session-provider-queue-active-release",
                &turn_id_owned,
                &tool_id_owned,
                "ReviewSecurity",
                &policy,
                DeepReviewCapacityQueueReason::ProviderConcurrencyLimit,
                60,
                false,
            )
            .await
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        assert!(
            !handle.is_finished(),
            "provider queue should keep waiting while no additional reviewer capacity freed"
        );
        drop(occupied);

        let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(500), handle)
            .await
            .expect("provider queue should wake when another active reviewer frees capacity")
            .expect("spawned wait should not panic");

        match outcome {
            DeepReviewProviderQueueWaitOutcome::ReadyToRetry {
                queue_elapsed_ms,
                early_capacity_probe,
            } => {
                assert!(
                    queue_elapsed_ms < 500,
                    "early capacity wake should not wait for the full backoff window"
                );
                assert!(
                    early_capacity_probe,
                    "active reviewer release should be marked as an early provider capacity probe"
                );
            }
            DeepReviewProviderQueueWaitOutcome::Skipped { .. } => {
                panic!("provider queue should retry after active reviewer capacity frees")
            }
        }
    }

    #[tokio::test]
    async fn deep_review_provider_retry_after_wait_ignores_active_reviewer_release() {
        use crate::agentic::deep_review::task_adapter::DeepReviewProviderQueueWaitOutcome;
        use crate::agentic::deep_review_policy::{
            try_begin_deep_review_active_reviewer, DeepReviewCapacityQueueReason,
            DeepReviewConcurrencyPolicy,
        };

        let turn_id = "turn-provider-retry-after-hard-wait";
        let tool_id = "tool-provider-retry-after-hard-wait";
        let occupied = try_begin_deep_review_active_reviewer(turn_id, 2)
            .expect("precondition should occupy another reviewer slot");
        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 2,
            stagger_seconds: 0,
            max_queue_wait_seconds: 1,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };
        let turn_id_owned = turn_id.to_string();
        let tool_id_owned = tool_id.to_string();

        let handle = tokio::spawn(async move {
            TaskTool::wait_for_deep_review_provider_capacity_retry(
                "session-provider-retry-after-hard-wait",
                &turn_id_owned,
                &tool_id_owned,
                "ReviewSecurity",
                &policy,
                DeepReviewCapacityQueueReason::RetryAfter,
                1,
                false,
            )
            .await
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        drop(occupied);
        tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
        assert!(
            !handle.is_finished(),
            "retry-after waits should not be interrupted by local reviewer capacity release"
        );

        let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(1500), handle)
            .await
            .expect("retry-after wait should eventually finish")
            .expect("spawned wait should not panic");

        match outcome {
            DeepReviewProviderQueueWaitOutcome::ReadyToRetry {
                early_capacity_probe,
                ..
            } => {
                assert!(
                    !early_capacity_probe,
                    "retry-after completion should be a natural cooldown retry"
                );
            }
            DeepReviewProviderQueueWaitOutcome::Skipped { .. } => {
                panic!("retry-after wait should retry after its bounded cooldown")
            }
        }
    }

    #[tokio::test]
    async fn deep_review_provider_capacity_queue_cancel_control_skips_retry() {
        use crate::agentic::deep_review::task_adapter::{
            DeepReviewProviderQueueWaitOutcome, DeepReviewQueueWaitSkipReason,
        };
        use crate::agentic::deep_review_policy::{
            apply_deep_review_queue_control, deep_review_runtime_diagnostics_snapshot,
            DeepReviewCapacityQueueReason, DeepReviewConcurrencyPolicy,
            DeepReviewQueueControlAction,
        };

        let turn_id = "turn-provider-queue-cancel";
        let tool_id = "tool-provider-queue-cancel";
        apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Cancel);
        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 2,
            stagger_seconds: 0,
            max_queue_wait_seconds: 60,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };

        let outcome = TaskTool::wait_for_deep_review_provider_capacity_retry(
            "session-provider-queue-cancel",
            turn_id,
            tool_id,
            "ReviewSecurity",
            &policy,
            DeepReviewCapacityQueueReason::ProviderRateLimit,
            60,
            false,
        )
        .await;

        match outcome {
            DeepReviewProviderQueueWaitOutcome::Skipped {
                queue_elapsed_ms,
                skip_reason,
            } => {
                assert!(queue_elapsed_ms < 100);
                assert_eq!(skip_reason, DeepReviewQueueWaitSkipReason::UserCancelled);
            }
            DeepReviewProviderQueueWaitOutcome::ReadyToRetry { .. } => {
                panic!("cancelled provider queue should not retry")
            }
        }

        let diagnostics = deep_review_runtime_diagnostics_snapshot(turn_id)
            .expect("provider queue should record diagnostics");
        assert_eq!(diagnostics.provider_capacity_queue_count, 1);
        assert_eq!(
            diagnostics
                .provider_capacity_queue_reason_counts
                .get("provider_rate_limit"),
            Some(&1)
        );
    }

    #[tokio::test]
    async fn deep_review_provider_capacity_queue_pause_does_not_count_against_wait() {
        use crate::agentic::deep_review::task_adapter::DeepReviewProviderQueueWaitOutcome;
        use crate::agentic::deep_review_policy::{
            apply_deep_review_queue_control, DeepReviewCapacityQueueReason,
            DeepReviewConcurrencyPolicy, DeepReviewQueueControlAction,
        };

        let turn_id = "turn-provider-queue-pause";
        let tool_id = "tool-provider-queue-pause";
        apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Pause);
        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 2,
            stagger_seconds: 0,
            max_queue_wait_seconds: 1,
            batch_extras_separately: true,
            allow_bounded_auto_retry: false,
            auto_retry_elapsed_guard_seconds: 180,
        };
        let turn_id_owned = turn_id.to_string();
        let tool_id_owned = tool_id.to_string();

        let handle = tokio::spawn(async move {
            TaskTool::wait_for_deep_review_provider_capacity_retry(
                "session-provider-queue-pause",
                &turn_id_owned,
                &tool_id_owned,
                "ReviewSecurity",
                &policy,
                DeepReviewCapacityQueueReason::ProviderConcurrencyLimit,
                1,
                false,
            )
            .await
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        assert!(
            !handle.is_finished(),
            "paused provider queue should not expire before continue"
        );

        apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Continue);
        let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(1500), handle)
            .await
            .expect("continued provider queue should finish")
            .expect("spawned wait should not panic");

        match outcome {
            DeepReviewProviderQueueWaitOutcome::ReadyToRetry {
                queue_elapsed_ms, ..
            } => {
                assert!(queue_elapsed_ms >= 900);
            }
            DeepReviewProviderQueueWaitOutcome::Skipped { .. } => {
                panic!("continued provider queue should retry after bounded wait")
            }
        }
    }

    #[test]
    fn deep_review_retry_accepts_reduced_partial_timeout_scope() {
        let manifest = json!({
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewSecurity:group-1-of-1",
                    "phase": "reviewer",
                    "subagentId": "ReviewSecurity",
                    "timeoutSeconds": 600,
                    "assignedScope": {
                        "files": [
                            "src/crates/assembly/core/src/auth.rs",
                            "src/crates/assembly/core/src/token.rs"
                        ]
                    }
                }
            ]
        });
        let input = json!({
            "retry": true,
            "timeout_seconds": 300,
            "retry_coverage": {
                "source_packet_id": "reviewer:ReviewSecurity:group-1-of-1",
                "source_status": "partial_timeout",
                "covered_files": [
                    "src/crates/assembly/core/src/auth.rs"
                ],
                "retry_scope_files": [
                    "src/crates/assembly/core/src/token.rs"
                ]
            }
        });

        let retry_scope =
            TaskTool::ensure_deep_review_retry_coverage(&input, "ReviewSecurity", Some(&manifest))
                .expect("reduced retry scope should be accepted");

        assert_eq!(retry_scope, vec!["src/crates/assembly/core/src/token.rs"]);
    }

    #[test]
    fn deep_review_retry_scope_prompt_prepend_bounds_review_files() {
        let prompt = TaskTool::prompt_with_deep_review_retry_scope(
            "Continue the security review.",
            &["src/crates/assembly/core/src/token.rs".to_string()],
        );

        assert!(prompt.starts_with("<deep_review_retry_scope>"));
        assert!(prompt.contains("Review only the following retry_scope_files"));
        assert!(prompt.contains("- src/crates/assembly/core/src/token.rs"));
        assert!(prompt.ends_with("Continue the security review."));
    }
}
