//! Tool pipeline
//!
//! Manages the complete lifecycle of tools:
//! confirmation, execution, caching, retries, etc.

use super::state_manager::{tool_task_state_kind, ToolStateManager};
use super::types::*;
use crate::agentic::core::{ToolCall, ToolExecutionState, ToolResult as ModelToolResult};
use crate::agentic::events::types::ToolEventData;
use crate::agentic::tools::computer_use_host::ComputerUseHostRef;
use crate::agentic::tools::framework::ToolResult as FrameworkToolResult;
use crate::agentic::tools::registry::ToolRegistry;
use crate::agentic::tools::tool_context_runtime;
use crate::agentic::tools::tool_context_runtime::ToolUseContext;
use crate::agentic::tools::tool_result_storage;
use crate::util::elapsed_ms_u64;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_agent_runtime::tool_confirmation::{
    resolve_confirmation_failure, resolve_confirmation_wait_result, resolve_tool_confirmation_plan,
    ConfirmationFailureKind, ToolConfirmationChannelStore, ToolConfirmationPlan,
    ToolConfirmationRequestFacts, ToolConfirmationResponse, ToolConfirmationWaitResult,
};
use bitfun_agent_tools::{
    build_invalid_tool_call_error_message, build_tool_call_truncation_recovery_notice,
    build_tool_confirmation_timeout_presentation, build_tool_execution_error_presentation,
    build_tool_execution_timeout_presentation,
    build_user_rejected_tool_presentation_with_instruction,
    build_user_steering_interrupted_presentation, render_tool_result_for_assistant,
    truncate_raw_tool_arguments_preview, truncate_tool_arguments_preview,
    validate_tool_execution_admission, ToolExecutionAdmissionRejection,
    ToolExecutionAdmissionRequest, GET_TOOL_SPEC_TOOL_NAME, USER_STEERING_INTERRUPTED_MESSAGE,
};
use bitfun_runtime_ports::RoundInjectionToolPreemption;
use futures::future::join_all;
use log::{debug, error, info, warn};
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tokio::sync::RwLock as TokioRwLock;
use tokio::time::{timeout, Duration};
use tokio_util::sync::CancellationToken;
use tool_runtime::pipeline::{
    partition_tool_batches, retry_delay_ms, should_cancel_tool_state, should_retry_tool_attempt,
    summarize_dialog_turn_cancellation, tool_call_concurrency_safe_for_batch,
    ToolCancellationTokenStore, ToolExecutionErrorClass, ToolRetryAttemptFacts,
};

/// Convert framework::ToolResult to core::ToolResult
///
/// Ensure always has result_for_assistant, avoid tool message content being empty
fn convert_tool_result(
    framework_result: FrameworkToolResult,
    tool_id: &str,
    tool_name: &str,
) -> ModelToolResult {
    match framework_result {
        FrameworkToolResult::Result {
            data,
            result_for_assistant,
            image_attachments,
        } => {
            // If the tool does not provide result_for_assistant, pass the full
            // structured result through to the model. Summaries like
            // "completed successfully" can hide fields the model needs for the
            // next decision.
            let assistant_text = result_for_assistant
                .or_else(|| Some(render_tool_result_for_assistant(tool_name, &data)));

            ModelToolResult {
                tool_id: tool_id.to_string(),
                tool_name: tool_name.to_string(),
                result: data,
                result_for_assistant: assistant_text,
                is_error: false,
                duration_ms: None,
                image_attachments,
            }
        }
        FrameworkToolResult::Progress { content, .. } => {
            let assistant_text = Some(render_tool_result_for_assistant(tool_name, &content));

            ModelToolResult {
                tool_id: tool_id.to_string(),
                tool_name: tool_name.to_string(),
                result: content,
                result_for_assistant: assistant_text,
                is_error: false,
                duration_ms: None,
                image_attachments: None,
            }
        }
        FrameworkToolResult::StreamChunk { data, .. } => {
            let assistant_text = Some(render_tool_result_for_assistant(tool_name, &data));

            ModelToolResult {
                tool_id: tool_id.to_string(),
                tool_name: tool_name.to_string(),
                result: data,
                result_for_assistant: assistant_text,
                is_error: false,
                duration_ms: None,
                image_attachments: None,
            }
        }
    }
}

/// Convert core::ToolResult to framework::ToolResult
fn convert_to_framework_result(model_result: &ModelToolResult) -> FrameworkToolResult {
    FrameworkToolResult::Result {
        data: model_result.result.clone(),
        result_for_assistant: model_result.result_for_assistant.clone(),
        image_attachments: model_result.image_attachments.clone(),
    }
}

fn elapsed_ms_since(time: SystemTime) -> u64 {
    time.elapsed()
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn classify_tool_error(error: &BitFunError) -> &'static str {
    match error {
        BitFunError::Validation(_) => "invalid_arguments",
        BitFunError::Cancelled(_) => "cancelled",
        BitFunError::Timeout(_) => "timeout",
        BitFunError::NotFound(_) => "not_found",
        _ => "execution_error",
    }
}

fn build_error_execution_result(
    task_id: &str,
    task: Option<ToolTask>,
    error: &BitFunError,
) -> ToolExecutionResult {
    let (tool_id, tool_name, execution_time_ms, provided_arguments) = if let Some(task) = task {
        let preview = task
            .tool_call
            .raw_arguments
            .as_deref()
            .map(truncate_raw_tool_arguments_preview)
            .unwrap_or_else(|| truncate_tool_arguments_preview(&task.tool_call.arguments));
        (
            task.tool_call.tool_id,
            task.tool_call.tool_name,
            elapsed_ms_since(task.created_at),
            Some(preview),
        )
    } else {
        warn!("Task not found in state manager: {}", task_id);
        (task_id.to_string(), "unknown".to_string(), 0, None)
    };
    let error_message = error.to_string();
    let category = classify_tool_error(error);
    let presentation = build_tool_execution_error_presentation(
        &tool_name,
        category,
        &error_message,
        provided_arguments,
    );

    ToolExecutionResult {
        tool_id: tool_id.clone(),
        tool_name: tool_name.clone(),
        result: ModelToolResult {
            tool_id,
            tool_name,
            result: presentation.result_json,
            result_for_assistant: Some(presentation.result_for_assistant),
            is_error: true,
            duration_ms: Some(execution_time_ms),
            image_attachments: None,
        },
        execution_time_ms,
    }
}

fn build_user_steering_interrupted_result(
    task_id: &str,
    task: Option<ToolTask>,
) -> ToolExecutionResult {
    let (tool_id, tool_name, execution_time_ms) = if let Some(task) = task {
        (
            task.tool_call.tool_id,
            task.tool_call.tool_name,
            elapsed_ms_since(task.created_at),
        )
    } else {
        warn!(
            "Task not found while building steering-interrupted result: {}",
            task_id
        );
        (task_id.to_string(), "unknown".to_string(), 0)
    };

    let presentation = build_user_steering_interrupted_presentation(&tool_name);

    ToolExecutionResult {
        tool_id: tool_id.clone(),
        tool_name: tool_name.clone(),
        result: ModelToolResult {
            tool_id,
            tool_name,
            result: presentation.result_json,
            result_for_assistant: Some(presentation.result_for_assistant),
            is_error: true,
            duration_ms: Some(execution_time_ms),
            image_attachments: None,
        },
        execution_time_ms,
    }
}

fn build_user_rejected_tool_result(
    task_id: &str,
    task: Option<ToolTask>,
    instruction: Option<&str>,
) -> ToolExecutionResult {
    let (tool_id, tool_name, execution_time_ms) = if let Some(task) = task {
        (
            task.tool_call.tool_id,
            task.tool_call.tool_name,
            elapsed_ms_since(task.created_at),
        )
    } else {
        warn!(
            "Task not found while building user-rejected result: {}",
            task_id
        );
        (task_id.to_string(), "unknown".to_string(), 0)
    };

    let presentation =
        build_user_rejected_tool_presentation_with_instruction(&tool_name, instruction);

    ToolExecutionResult {
        tool_id: tool_id.clone(),
        tool_name: tool_name.clone(),
        result: ModelToolResult {
            tool_id,
            tool_name,
            result: presentation.result_json,
            result_for_assistant: Some(presentation.result_for_assistant),
            is_error: false,
            duration_ms: Some(execution_time_ms),
            image_attachments: None,
        },
        execution_time_ms,
    }
}

const ROUND_INJECTION_RUNNING_TOOL_CANCELLED_MESSAGE: &str =
    "Tool execution cancelled because a pending round injection requested running-tool preemption for this turn.";

fn should_retry_tool_error(error: &BitFunError) -> bool {
    matches!(
        error,
        BitFunError::Timeout(_)
            | BitFunError::Io(_)
            | BitFunError::Http(_)
            | BitFunError::Service(_)
            | BitFunError::MCPError(_)
            | BitFunError::ProcessError(_)
            | BitFunError::Other(_)
    )
}

fn classify_tool_retry_error(error: &BitFunError) -> ToolExecutionErrorClass {
    if should_retry_tool_error(error) {
        ToolExecutionErrorClass::Retryable
    } else {
        ToolExecutionErrorClass::Terminal
    }
}

fn map_tool_execution_admission_rejection(error: ToolExecutionAdmissionRejection) -> BitFunError {
    match error {
        ToolExecutionAdmissionRejection::RuntimeRestriction(error) => error.into(),
        ToolExecutionAdmissionRejection::AllowedList(error) => {
            BitFunError::Validation(error.to_string())
        }
        ToolExecutionAdmissionRejection::Collapsed(error) => {
            BitFunError::Validation(error.to_string())
        }
    }
}

const SUBAGENT_LAUNCH_TOOL_NAME: &str = "Task";

/// Tool pipeline
#[derive(Clone)]
pub struct ToolPipeline {
    tool_registry: Arc<TokioRwLock<ToolRegistry>>,
    state_manager: Arc<ToolStateManager>,
    confirmation_channels: ToolConfirmationChannelStore,
    cancellation_tokens: ToolCancellationTokenStore,
    computer_use_host: Option<ComputerUseHostRef>,
}

impl ToolPipeline {
    pub fn new(
        tool_registry: Arc<TokioRwLock<ToolRegistry>>,
        state_manager: Arc<ToolStateManager>,
        computer_use_host: Option<ComputerUseHostRef>,
    ) -> Self {
        Self {
            tool_registry,
            state_manager,
            confirmation_channels: ToolConfirmationChannelStore::new(),
            cancellation_tokens: ToolCancellationTokenStore::new(),
            computer_use_host,
        }
    }

    pub fn computer_use_host(&self) -> Option<ComputerUseHostRef> {
        self.computer_use_host.clone()
    }

    fn pending_round_injection_tool_preemption(
        &self,
        context: &ToolExecutionContext,
    ) -> RoundInjectionToolPreemption {
        context
            .steering_interrupt
            .as_ref()
            .map(|interrupt| interrupt.pending_tool_preemption())
            .unwrap_or(RoundInjectionToolPreemption::None)
    }

    fn should_interrupt_for_round_injection(&self, context: &ToolExecutionContext) -> bool {
        self.pending_round_injection_tool_preemption(context)
            .should_interrupt_after_current_atomic_unit()
    }

    async fn build_steering_interrupted_results(
        &self,
        task_ids: impl IntoIterator<Item = String>,
    ) -> Vec<ToolExecutionResult> {
        let mut results = Vec::new();
        for task_id in task_ids {
            let task = self.state_manager.get_task(&task_id);
            self.state_manager
                .update_state(
                    &task_id,
                    ToolExecutionState::Cancelled {
                        reason: USER_STEERING_INTERRUPTED_MESSAGE.to_string(),
                        duration_ms: None,
                        queue_wait_ms: None,
                        preflight_ms: None,
                        confirmation_wait_ms: None,
                        execution_ms: None,
                    },
                )
                .await;
            results.push(build_user_steering_interrupted_result(&task_id, task));
        }
        results
    }

    fn append_execution_result(
        &self,
        task_id: &str,
        result: BitFunResult<ToolExecutionResult>,
        all_results: &mut Vec<ToolExecutionResult>,
    ) {
        match result {
            Ok(execution_result) => all_results.push(execution_result),
            Err(error) => {
                error!("Tool execution failed: error={}", error);
                let error_result = build_error_execution_result(
                    task_id,
                    self.state_manager.get_task(task_id),
                    &error,
                );
                all_results.push(error_result);
            }
        }
    }

    async fn cancel_tools_for_round_injection(
        &self,
        task_ids: impl IntoIterator<Item = String>,
    ) -> BitFunResult<()> {
        for task_id in task_ids {
            self.cancel_tool(
                &task_id,
                ROUND_INJECTION_RUNNING_TOOL_CANCELLED_MESSAGE.to_string(),
            )
            .await?;
        }
        Ok(())
    }

    fn spawn_round_injection_cancellation_watch(
        &self,
        task_ids: Vec<String>,
        interrupt: Option<crate::agentic::round_preempt::DialogRoundInjectionInterrupt>,
    ) -> Option<tokio::task::JoinHandle<()>> {
        if interrupt.is_none() {
            return None;
        }

        let pipeline = self.clone();
        Some(tokio::spawn(async move {
            let Some(interrupt) = interrupt else {
                return;
            };

            loop {
                if interrupt.should_cancel_running_tools() {
                    let _ = pipeline.cancel_tools_for_round_injection(task_ids).await;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }))
    }

    /// Execute multiple tool calls using partitioned mixed scheduling.
    ///
    /// Consecutive concurrency-safe calls are grouped into a single batch and
    /// run in parallel; each non-safe call forms its own batch and runs serially.
    /// Batches are executed in order so that write-after-read dependencies are
    /// respected while reads still benefit from parallelism.
    pub async fn execute_tools(
        &self,
        tool_calls: Vec<ToolCall>,
        context: ToolExecutionContext,
        options: ToolExecutionOptions,
    ) -> BitFunResult<Vec<ToolExecutionResult>> {
        if tool_calls.is_empty() {
            return Ok(vec![]);
        }

        info!("Executing tools: count={}", tool_calls.len());
        let tool_names: Vec<String> = tool_calls
            .iter()
            .map(|tool_call| tool_call.tool_name.clone())
            .collect();

        let subagent_call_count = tool_calls
            .iter()
            .filter(|tool_call| tool_call.tool_name == SUBAGENT_LAUNCH_TOOL_NAME)
            .count();

        // Determine concurrency safety for each tool call
        let concurrency_flags: Vec<bool> = {
            let registry = self.tool_registry.read().await;
            tool_calls
                .iter()
                .map(|tc| {
                    let tool_is_concurrency_safe = registry
                        .get_tool(&tc.tool_name)
                        .map(|tool| tool.is_concurrency_safe(Some(&tc.arguments)))
                        .unwrap_or(false);
                    tool_call_concurrency_safe_for_batch(
                        &tc.tool_name,
                        tool_is_concurrency_safe,
                        subagent_call_count,
                        options.subagent_batch_execution_policy,
                    )
                })
                .collect()
        };
        let concurrency_safe_count = concurrency_flags.iter().filter(|&&flag| flag).count();

        // Create tasks for all tool calls
        let mut task_ids = Vec::with_capacity(tool_calls.len());
        for tool_call in tool_calls {
            let task = ToolTask::new(tool_call, context.clone(), options.clone());
            let tool_id = self.state_manager.create_task(task).await;
            task_ids.push(tool_id);
        }

        if !options.allow_parallel {
            debug!(
                "Tool execution plan: total_tools={}, batches=1, concurrency_safe={}, non_concurrency_safe={}, allow_parallel=false, tools={}",
                task_ids.len(),
                concurrency_safe_count,
                task_ids.len().saturating_sub(concurrency_safe_count),
                tool_names.join(", ")
            );
            return self.execute_sequential(task_ids).await;
        }

        // Partition into batches of consecutive same-safety tool calls
        let batches = partition_tool_batches(&task_ids, &concurrency_flags);
        debug!(
            "Tool execution plan: total_tools={}, batches={}, concurrency_safe={}, non_concurrency_safe={}, allow_parallel=true, tools={}",
            task_ids.len(),
            batches.len(),
            concurrency_safe_count,
            task_ids.len().saturating_sub(concurrency_safe_count),
            tool_names.join(", ")
        );

        debug!(
            "Partitioned {} tools into {} batches for mixed execution",
            task_ids.len(),
            batches.len()
        );

        let mut all_results = Vec::with_capacity(task_ids.len());
        let mut batch_iter = batches.into_iter().enumerate().peekable();
        while let Some((batch_idx, batch)) = batch_iter.next() {
            let batch_context = batch
                .task_ids
                .first()
                .and_then(|task_id| self.state_manager.get_task(task_id))
                .map(|task| task.context);
            if batch_context
                .as_ref()
                .is_some_and(|context| self.should_interrupt_for_round_injection(context))
            {
                let remaining_task_ids = batch
                    .task_ids
                    .into_iter()
                    .chain(batch_iter.flat_map(|(_, batch)| batch.task_ids.into_iter()));
                all_results.extend(
                    self.build_steering_interrupted_results(remaining_task_ids)
                        .await,
                );
                break;
            }

            debug!(
                "Executing batch {}: {} tool(s), concurrent={}",
                batch_idx,
                batch.task_ids.len(),
                batch.is_concurrent
            );
            let batch_results = if batch.is_concurrent {
                self.execute_parallel(batch.task_ids).await?
            } else {
                self.execute_sequential(batch.task_ids).await?
            };
            all_results.extend(batch_results);
        }

        Ok(all_results)
    }

    /// Execute tools in parallel
    async fn execute_parallel(
        &self,
        task_ids: Vec<String>,
    ) -> BitFunResult<Vec<ToolExecutionResult>> {
        let batch_interrupt = task_ids
            .first()
            .and_then(|task_id| self.state_manager.get_task(task_id))
            .and_then(|task| task.context.steering_interrupt.clone());
        let watch_handle =
            self.spawn_round_injection_cancellation_watch(task_ids.clone(), batch_interrupt);

        let futures: Vec<_> = task_ids
            .iter()
            .map(|id| self.execute_single_tool(id.clone()))
            .collect();

        let results = join_all(futures).await;
        if let Some(handle) = watch_handle {
            handle.abort();
            let _ = handle.await;
        }

        // Collect results, including failed results
        let mut all_results = Vec::new();
        for (idx, result) in results.into_iter().enumerate() {
            let task_id = &task_ids[idx];
            self.append_execution_result(task_id, result, &mut all_results);
        }

        Ok(all_results)
    }

    /// Execute tools sequentially
    async fn execute_sequential(
        &self,
        task_ids: Vec<String>,
    ) -> BitFunResult<Vec<ToolExecutionResult>> {
        let mut results = Vec::new();

        let mut task_iter = task_ids.into_iter().peekable();
        while let Some(task_id) = task_iter.next() {
            let task = self.state_manager.get_task(&task_id);
            if task
                .as_ref()
                .is_some_and(|task| self.should_interrupt_for_round_injection(&task.context))
            {
                let remaining_task_ids = std::iter::once(task_id).chain(task_iter);
                results.extend(
                    self.build_steering_interrupted_results(remaining_task_ids)
                        .await,
                );
                break;
            }

            let interrupt = task.and_then(|task| task.context.steering_interrupt.clone());
            let watch_handle =
                self.spawn_round_injection_cancellation_watch(vec![task_id.clone()], interrupt);
            let result = self.execute_single_tool(task_id.clone()).await;
            if let Some(handle) = watch_handle {
                handle.abort();
                let _ = handle.await;
            }
            self.append_execution_result(&task_id, result, &mut results);
        }

        Ok(results)
    }

    /// Execute single tool
    async fn execute_single_tool(&self, tool_id: String) -> BitFunResult<ToolExecutionResult> {
        let start_time = Instant::now();

        debug!("Starting tool execution: tool_id={}", tool_id);

        // Get task
        let task = self
            .state_manager
            .get_task(&tool_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Tool task not found: {}", tool_id)))?;

        let tool_name = task.tool_call.tool_name.clone();
        let tool_args = task.tool_call.arguments.clone();
        let tool_is_error = task.tool_call.is_error;
        let recovered_from_truncation = task.tool_call.recovered_from_truncation;
        let queue_wait_ms = elapsed_ms_since(task.created_at);
        let mut confirmation_wait_ms = 0;

        debug!(
            "Tool task details: tool_name={}, tool_id={}, queue_wait_ms={}",
            tool_name, tool_id, queue_wait_ms
        );

        if recovered_from_truncation {
            warn!(
                "Tool '{}' arguments were recovered from a truncated stream (tool_id={}, session_id={}). Executing with patched arguments — content may be incomplete.",
                tool_name, tool_id, task.context.session_id
            );
        }

        if tool_name.is_empty() || tool_is_error {
            let raw_arguments_preview = task
                .tool_call
                .raw_arguments
                .as_deref()
                .map(truncate_raw_tool_arguments_preview);
            let error_msg = build_invalid_tool_call_error_message(
                &tool_name,
                tool_is_error,
                recovered_from_truncation,
                raw_arguments_preview,
            );

            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Failed {
                        error: error_msg.clone(),
                        is_retryable: false,
                        duration_ms: None,
                        queue_wait_ms: None,
                        preflight_ms: None,
                        confirmation_wait_ms: None,
                        execution_ms: None,
                    },
                )
                .await;

            return Err(BitFunError::Validation(error_msg));
        }

        // Repetition alone is not execution failure: polling and status checks
        // may legitimately reuse identical arguments. The execution engine
        // evaluates repeated patterns only after observing actual tool results.
        if let Err(err) = validate_tool_execution_admission(ToolExecutionAdmissionRequest {
            tool_name: &tool_name,
            allowed_tools: &task.context.allowed_tools,
            runtime_tool_restrictions: &task.context.runtime_tool_restrictions,
            collapsed_tools: &task.context.collapsed_tools,
            loaded_collapsed_tools: &task.context.unlocked_collapsed_tools,
            get_tool_spec_tool_name: GET_TOOL_SPEC_TOOL_NAME,
        }) {
            let error_msg = err.to_string();
            warn!("Tool execution admission rejected: {}", error_msg);

            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Failed {
                        error: error_msg,
                        is_retryable: false,
                        duration_ms: None,
                        queue_wait_ms: None,
                        preflight_ms: None,
                        confirmation_wait_ms: None,
                        execution_ms: None,
                    },
                )
                .await;

            return Err(map_tool_execution_admission_rejection(err));
        }

        let tool = {
            let registry = self.tool_registry.read().await;
            registry
                .get_tool(&task.tool_call.tool_name)
                .ok_or_else(|| {
                    let error_msg = format!(
                        "Tool '{}' is not registered or enabled.",
                        task.tool_call.tool_name,
                    );
                    error!("{}", error_msg);
                    BitFunError::tool(error_msg)
                })?
        };

        let cancellation_token = CancellationToken::new();
        let tool_context = self.build_tool_use_context(&task, cancellation_token.clone());
        let validation = tool.validate_input(&tool_args, Some(&tool_context)).await;
        if !validation.result {
            let error_msg = validation
                .message
                .unwrap_or_else(|| format!("Invalid input for tool '{}'", tool_name));
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Failed {
                        error: error_msg.clone(),
                        is_retryable: false,
                        duration_ms: None,
                        queue_wait_ms: None,
                        preflight_ms: None,
                        confirmation_wait_ms: None,
                        execution_ms: None,
                    },
                )
                .await;
            return Err(BitFunError::Validation(error_msg));
        }
        if let Some(message) = validation
            .message
            .filter(|message| !message.trim().is_empty())
        {
            warn!(
                "Tool input validation warning: tool_name={}, warning={}",
                tool_name, message
            );
        }

        // Register cancellation only after deterministic validation and registry lookup succeed.
        self.cancellation_tokens
            .insert(tool_id.clone(), cancellation_token.clone());

        debug!("Executing tool: tool_name={}", tool_name);

        let is_streaming = tool.supports_streaming();
        let preflight_ms = elapsed_ms_u64(start_time);

        let confirmation_plan = resolve_tool_confirmation_plan(ToolConfirmationRequestFacts {
            confirm_before_run: task.options.confirm_before_run,
            tool_needs_permission: tool.needs_permissions(Some(&tool_args)),
            confirmation_timeout_secs: task.options.confirmation_timeout_secs,
            now: SystemTime::now(),
        });

        if let ToolConfirmationPlan::Await {
            timeout_at,
            timeout_secs,
        } = confirmation_plan
        {
            info!("Tool requires confirmation: tool_name={}", tool_name);

            let rx = self.confirmation_channels.register(tool_id.clone());

            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::AwaitingConfirmation {
                        params: tool_args.clone(),
                        timeout_at,
                    },
                )
                .await;

            debug!("Waiting for confirmation: tool_name={}", tool_name);
            let confirmation_started_at = Instant::now();

            let confirmation_result = match timeout_secs {
                Some(timeout_secs) => {
                    debug!(
                        "Waiting for user confirmation with timeout: timeout_secs={}, tool_name={}",
                        timeout_secs, tool_name
                    );
                    // There is a timeout limit
                    timeout(Duration::from_secs(timeout_secs), rx).await.ok()
                }
                None => {
                    debug!(
                        "Waiting for user confirmation without timeout: tool_name={}",
                        tool_name
                    );
                    Some(rx.await)
                }
            };
            confirmation_wait_ms = elapsed_ms_u64(confirmation_started_at);

            let confirmation_wait_result = match confirmation_result {
                Some(Ok(ToolConfirmationResponse::Confirmed)) => {
                    debug!("Tool confirmed: tool_name={}", tool_name);
                    ToolConfirmationWaitResult::Confirmed
                }
                Some(Ok(ToolConfirmationResponse::Rejected(reason))) => {
                    ToolConfirmationWaitResult::Rejected(reason)
                }
                Some(Err(_)) => ToolConfirmationWaitResult::ChannelClosed,
                None => ToolConfirmationWaitResult::TimedOut,
            };
            let confirmation_outcome =
                resolve_confirmation_wait_result(confirmation_wait_result, &tool_name);

            if let Some(failure) = resolve_confirmation_failure(confirmation_outcome) {
                if matches!(
                    failure.kind,
                    ConfirmationFailureKind::ChannelClosed | ConfirmationFailureKind::Timeout
                ) {
                    self.confirmation_channels.cancel(&tool_id);
                }

                if matches!(failure.kind, ConfirmationFailureKind::Timeout) {
                    warn!("{}", failure.error_message);
                }

                self.state_manager
                    .update_state(
                        &tool_id,
                        match failure.kind {
                            ConfirmationFailureKind::Rejected => ToolExecutionState::Rejected {
                                reason: failure.state_reason,
                                duration_ms: Some(elapsed_ms_u64(start_time)),
                                queue_wait_ms: Some(queue_wait_ms),
                                preflight_ms: Some(preflight_ms),
                                confirmation_wait_ms: Some(elapsed_ms_u64(confirmation_started_at)),
                                execution_ms: None,
                            },
                            ConfirmationFailureKind::ChannelClosed
                            | ConfirmationFailureKind::Timeout => ToolExecutionState::Cancelled {
                                reason: failure.state_reason,
                                duration_ms: Some(elapsed_ms_u64(start_time)),
                                queue_wait_ms: Some(queue_wait_ms),
                                preflight_ms: Some(preflight_ms),
                                confirmation_wait_ms: Some(elapsed_ms_u64(confirmation_started_at)),
                                execution_ms: None,
                            },
                        },
                    )
                    .await;

                match failure.kind {
                    ConfirmationFailureKind::Rejected => {
                        return Ok(build_user_rejected_tool_result(
                            &tool_id,
                            self.state_manager.get_task(&tool_id),
                            failure.rejection_instruction.as_deref(),
                        ));
                    }
                    ConfirmationFailureKind::ChannelClosed => {
                        return Err(BitFunError::service(failure.error_message));
                    }
                    ConfirmationFailureKind::Timeout => {
                        let presentation = build_tool_confirmation_timeout_presentation(&tool_name);
                        return Ok(ToolExecutionResult {
                            tool_id: tool_id.clone(),
                            tool_name: tool_name.clone(),
                            result: ModelToolResult {
                                tool_id,
                                tool_name,
                                result: presentation.result_json,
                                result_for_assistant: Some(presentation.result_for_assistant),
                                is_error: false,
                                duration_ms: Some(elapsed_ms_u64(start_time)),
                                image_attachments: None,
                            },
                            execution_time_ms: elapsed_ms_u64(start_time),
                        });
                    }
                }
            }

            self.confirmation_channels.cancel(&tool_id);
        }

        let preflight_ms = elapsed_ms_u64(start_time).saturating_sub(confirmation_wait_ms);

        if cancellation_token.is_cancelled() {
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Cancelled {
                        reason: "Tool was cancelled before execution".to_string(),
                        duration_ms: Some(elapsed_ms_u64(start_time)),
                        queue_wait_ms: Some(queue_wait_ms),
                        preflight_ms: Some(preflight_ms),
                        confirmation_wait_ms: Some(confirmation_wait_ms),
                        execution_ms: None,
                    },
                )
                .await;
            self.cancellation_tokens.remove(&tool_id);
            return Err(BitFunError::Cancelled(
                "Tool was cancelled before execution".to_string(),
            ));
        }

        // Set initial state
        if is_streaming {
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Streaming {
                        started_at: std::time::SystemTime::now(),
                        chunks_received: 0,
                    },
                )
                .await;
        } else {
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Running {
                        started_at: std::time::SystemTime::now(),
                        progress: None,
                    },
                )
                .await;
        }

        let execution_started_at = Instant::now();
        let tool_context = self.build_tool_use_context(&task, cancellation_token.clone());
        let result = self
            .execute_with_retry(&task, cancellation_token.clone(), tool)
            .await;
        let execution_ms = elapsed_ms_u64(execution_started_at);

        self.cancellation_tokens.remove(&tool_id);

        match result {
            Ok(tool_result) => {
                let duration_ms = elapsed_ms_u64(start_time);
                let mut tool_result = tool_result_storage::maybe_persist_large_tool_result(
                    tool_result,
                    &tool_context,
                )
                .await;
                tool_result.duration_ms = Some(duration_ms);

                // The tool call succeeded with arguments that we patched
                // because the model's output was truncated mid-stream. Tell
                // the model so it can decide whether the partial call needs
                // to be continued or regenerated.
                if recovered_from_truncation {
                    let original = tool_result.result_for_assistant.unwrap_or_default();
                    let notice = build_tool_call_truncation_recovery_notice(&tool_name);
                    tool_result.result_for_assistant = Some(if original.is_empty() {
                        notice.trim_end().to_string()
                    } else {
                        format!("{notice}{original}")
                    });
                }

                self.state_manager
                    .update_state(
                        &tool_id,
                        ToolExecutionState::Completed {
                            result: convert_to_framework_result(&tool_result),
                            duration_ms,
                            queue_wait_ms: Some(queue_wait_ms),
                            preflight_ms: Some(preflight_ms),
                            confirmation_wait_ms: Some(confirmation_wait_ms),
                            execution_ms: Some(execution_ms),
                        },
                    )
                    .await;

                info!(
                    "Tool completed: tool_name={}, duration_ms={}, queue_wait_ms={}, preflight_ms={}, confirmation_wait_ms={}, execution_ms={}, streaming={}",
                    tool_name,
                    duration_ms,
                    queue_wait_ms,
                    preflight_ms,
                    confirmation_wait_ms,
                    execution_ms,
                    is_streaming
                );

                Ok(ToolExecutionResult {
                    tool_id,
                    tool_name,
                    result: tool_result,
                    execution_time_ms: duration_ms,
                })
            }
            Err(e) => {
                // Cancellation is a first-class terminal state, not a failure.
                // Preserve Cancelled here so a late cancel cannot be overwritten
                // by the generic Failed branch below.
                if let BitFunError::Cancelled(reason) = &e {
                    self.state_manager
                        .update_state(
                            &tool_id,
                            ToolExecutionState::Cancelled {
                                reason: reason.clone(),
                                duration_ms: Some(elapsed_ms_u64(start_time)),
                                queue_wait_ms: Some(queue_wait_ms),
                                preflight_ms: Some(preflight_ms),
                                confirmation_wait_ms: Some(confirmation_wait_ms),
                                execution_ms: Some(execution_ms),
                            },
                        )
                        .await;

                    info!(
                        "Tool cancelled during execution: tool_name={}, reason={}, duration_ms={}, queue_wait_ms={}, preflight_ms={}, confirmation_wait_ms={}, execution_ms={}",
                        tool_name,
                        reason,
                        elapsed_ms_u64(start_time),
                        queue_wait_ms,
                        preflight_ms,
                        confirmation_wait_ms,
                        execution_ms
                    );

                    return Err(e);
                }

                if matches!(e, BitFunError::Timeout(_)) {
                    let duration_ms = elapsed_ms_u64(start_time);
                    let presentation = build_tool_execution_timeout_presentation(
                        &tool_name,
                        task.options.timeout_secs,
                    );
                    let timed_out_tool_id = tool_id.clone();
                    let timed_out_tool_name = tool_name.clone();

                    self.state_manager
                        .update_state(
                            &tool_id,
                            ToolExecutionState::Cancelled {
                                reason: presentation.result_for_assistant.clone(),
                                duration_ms: Some(duration_ms),
                                queue_wait_ms: Some(queue_wait_ms),
                                preflight_ms: Some(preflight_ms),
                                confirmation_wait_ms: Some(confirmation_wait_ms),
                                execution_ms: Some(execution_ms),
                            },
                        )
                        .await;

                    warn!(
                        "Tool execution timed out: tool_name={}, duration_ms={}, queue_wait_ms={}, preflight_ms={}, confirmation_wait_ms={}, execution_ms={}",
                        tool_name,
                        duration_ms,
                        queue_wait_ms,
                        preflight_ms,
                        confirmation_wait_ms,
                        execution_ms
                    );

                    return Ok(ToolExecutionResult {
                        tool_id: timed_out_tool_id.clone(),
                        tool_name: timed_out_tool_name.clone(),
                        result: ModelToolResult {
                            tool_id: timed_out_tool_id,
                            tool_name: timed_out_tool_name,
                            result: presentation.result_json,
                            result_for_assistant: Some(presentation.result_for_assistant),
                            is_error: false,
                            duration_ms: Some(duration_ms),
                            image_attachments: None,
                        },
                        execution_time_ms: duration_ms,
                    });
                }

                let error_msg = e.to_string();
                let is_retryable = task.options.max_retries > 0;

                self.state_manager
                    .update_state(
                        &tool_id,
                        ToolExecutionState::Failed {
                            error: error_msg.clone(),
                            is_retryable,
                            duration_ms: Some(elapsed_ms_u64(start_time)),
                            queue_wait_ms: Some(queue_wait_ms),
                            preflight_ms: Some(preflight_ms),
                            confirmation_wait_ms: Some(confirmation_wait_ms),
                            execution_ms: Some(execution_ms),
                        },
                    )
                    .await;

                error!(
                    "Tool failed: tool_name={}, error={}, duration_ms={}, queue_wait_ms={}, preflight_ms={}, confirmation_wait_ms={}, execution_ms={}",
                    tool_name,
                    error_msg,
                    elapsed_ms_u64(start_time),
                    queue_wait_ms,
                    preflight_ms,
                    confirmation_wait_ms,
                    execution_ms
                );

                Err(e)
            }
        }
    }

    /// Execute with retry
    async fn execute_with_retry(
        &self,
        task: &ToolTask,
        cancellation_token: CancellationToken,
        tool: Arc<dyn crate::agentic::tools::framework::Tool>,
    ) -> BitFunResult<ModelToolResult> {
        let mut attempts = 0;
        let max_attempts = task.options.max_retries + 1;

        loop {
            // Check cancellation token
            if cancellation_token.is_cancelled() {
                return Err(BitFunError::Cancelled(
                    "Tool execution was cancelled".to_string(),
                ));
            }

            attempts += 1;

            let result = self
                .execute_tool_impl(task, cancellation_token.clone(), tool.clone())
                .await;

            match result {
                Ok(r) => return Ok(r),
                Err(e) => {
                    if !should_retry_tool_attempt(ToolRetryAttemptFacts {
                        attempts,
                        max_attempts,
                        error_class: classify_tool_retry_error(&e),
                    }) {
                        return Err(e);
                    }

                    debug!(
                        "Retrying tool execution: attempt={}/{}, error={}",
                        attempts, max_attempts, e
                    );

                    // Wait for a period of time and retry
                    tokio::time::sleep(Duration::from_millis(retry_delay_ms(attempts))).await;
                }
            }
        }
    }

    /// Actual execution of tool
    async fn execute_tool_impl(
        &self,
        task: &ToolTask,
        cancellation_token: CancellationToken,
        tool: Arc<dyn crate::agentic::tools::framework::Tool>,
    ) -> BitFunResult<ModelToolResult> {
        // Check cancellation token
        if cancellation_token.is_cancelled() {
            return Err(BitFunError::Cancelled(
                "Tool execution was cancelled".to_string(),
            ));
        }

        let tool_context = self.build_tool_use_context(task, cancellation_token);

        let execution_future = tool.call(&task.tool_call.arguments, &tool_context);

        let pipeline_timeout_secs = if tool.manages_own_execution_timeout() {
            None
        } else {
            task.options.timeout_secs
        };

        let tool_results = match pipeline_timeout_secs {
            Some(timeout_secs) => {
                let timeout_duration = Duration::from_secs(timeout_secs);
                let result = timeout(timeout_duration, execution_future)
                    .await
                    .map_err(|_| {
                        BitFunError::Timeout(format!(
                            "Tool execution timeout: {}",
                            task.tool_call.tool_name
                        ))
                    })?;
                result?
            }
            None => execution_future.await?,
        };

        if tool.supports_streaming() && tool_results.len() > 1 {
            self.handle_streaming_results(task, &tool_results).await?;
        }

        tool_results
            .into_iter()
            .last()
            .map(|r| convert_tool_result(r, &task.tool_call.tool_id, &task.tool_call.tool_name))
            .ok_or_else(|| {
                BitFunError::Tool(format!(
                    "Tool did not return result: {}",
                    task.tool_call.tool_name
                ))
            })
    }

    fn build_tool_use_context(
        &self,
        task: &ToolTask,
        cancellation_token: CancellationToken,
    ) -> ToolUseContext {
        tool_context_runtime::build_tool_use_context_for_task(
            task,
            self.computer_use_host.clone(),
            cancellation_token,
        )
    }

    /// Handle streaming results
    async fn handle_streaming_results(
        &self,
        task: &ToolTask,
        results: &[FrameworkToolResult],
    ) -> BitFunResult<()> {
        let mut chunks_received = 0;

        for result in results {
            if let FrameworkToolResult::StreamChunk {
                data,
                chunk_index: _,
                is_final: _,
            } = result
            {
                chunks_received += 1;

                // Update state
                self.state_manager
                    .update_state(
                        &task.tool_call.tool_id,
                        ToolExecutionState::Streaming {
                            started_at: std::time::SystemTime::now(),
                            chunks_received,
                        },
                    )
                    .await;

                // Send StreamChunk event
                let _event_data = ToolEventData::StreamChunk {
                    tool_id: task.tool_call.tool_id.clone(),
                    tool_name: task.tool_call.tool_name.clone(),
                    data: data.clone(),
                };
            }
        }

        Ok(())
    }

    /// Cancel tool execution
    pub async fn cancel_tool(&self, tool_id: &str, reason: String) -> BitFunResult<()> {
        let Some(task) = self.state_manager.get_task(tool_id) else {
            debug!(
                "Ignoring cancel request for unknown tool: tool_id={}",
                tool_id
            );
            return Ok(());
        };

        if tool_task_state_kind(&task.state).is_terminal() {
            debug!(
                    "Ignoring duplicate cancel request for tool in terminal state: tool_id={}, state={:?}",
                    tool_id, task.state
                );
            return Ok(());
        }

        // 1. Trigger cancellation token
        if self.cancellation_tokens.cancel(tool_id) {
            debug!("Cancellation token triggered: tool_id={}", tool_id);
        } else {
            debug!(
                "Cancellation token not found (tool may have completed): tool_id={}",
                tool_id
            );
        }

        // 2. Clean up confirmation channel (if waiting for confirmation)
        if self.confirmation_channels.cancel(tool_id) {
            // Channel will be automatically closed, causing await rx to return Err
            debug!("Cleared confirmation channel: tool_id={}", tool_id);
        }

        // 3. Update state to cancelled
        self.state_manager
            .update_state(
                tool_id,
                ToolExecutionState::Cancelled {
                    reason: reason.clone(),
                    duration_ms: None,
                    queue_wait_ms: None,
                    preflight_ms: None,
                    confirmation_wait_ms: None,
                    execution_ms: None,
                },
            )
            .await;

        info!(
            "Tool execution cancelled: tool_id={}, reason={}",
            tool_id, reason
        );
        Ok(())
    }

    /// Cancel all tools for a dialog turn
    pub async fn cancel_dialog_turn_tools(&self, dialog_turn_id: &str) -> BitFunResult<()> {
        info!(
            "Cancelling all tools for dialog turn: dialog_turn_id={}",
            dialog_turn_id
        );

        let tasks = self.state_manager.get_dialog_turn_tasks(dialog_turn_id);
        debug!("Found {} tool tasks for dialog turn", tasks.len());

        let summary = summarize_dialog_turn_cancellation(
            tasks.iter().map(|task| tool_task_state_kind(&task.state)),
        );

        for task in tasks {
            if should_cancel_tool_state(tool_task_state_kind(&task.state)) {
                debug!(
                    "Cancelling tool: tool_id={}, state={:?}",
                    task.tool_call.tool_id, task.state
                );
                self.cancel_tool(&task.tool_call.tool_id, "Dialog turn cancelled".to_string())
                    .await?;
            } else {
                debug!(
                    "Skipping tool (state not cancellable): tool_id={}, state={:?}",
                    task.tool_call.tool_id, task.state
                );
            }
        }

        info!(
            "Tool cancellation completed: cancelled={}, skipped={}",
            summary.cancelled, summary.skipped
        );
        Ok(())
    }

    /// Confirm tool execution
    pub async fn confirm_tool(
        &self,
        tool_id: &str,
        updated_input: Option<serde_json::Value>,
    ) -> BitFunResult<()> {
        let task = self
            .state_manager
            .get_task(tool_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Tool task not found: {}", tool_id)))?;

        // Check if the state is waiting for confirmation
        if !matches!(task.state, ToolExecutionState::AwaitingConfirmation { .. }) {
            return Err(BitFunError::Validation(format!(
                "Tool is not in awaiting confirmation state: {:?}",
                task.state
            )));
        }

        // If the user modified the parameters, update the task parameters first
        if let Some(new_args) = updated_input {
            debug!("User updated tool arguments: tool_id={}", tool_id);
            self.state_manager.update_task_arguments(tool_id, new_args);
        }

        // Get sender from map and send confirmation response
        if self.confirmation_channels.confirm(tool_id) {
            info!("User confirmed tool execution: tool_id={}", tool_id);
            Ok(())
        } else {
            Err(BitFunError::NotFound(format!(
                "Confirmation channel not found: {}",
                tool_id
            )))
        }
    }

    /// Reject tool execution
    pub async fn reject_tool(&self, tool_id: &str, reason: String) -> BitFunResult<()> {
        let task = self
            .state_manager
            .get_task(tool_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Tool task not found: {}", tool_id)))?;

        // Check if the state is waiting for confirmation
        if !matches!(task.state, ToolExecutionState::AwaitingConfirmation { .. }) {
            return Err(BitFunError::Validation(format!(
                "Tool is not in awaiting confirmation state: {:?}",
                task.state
            )));
        }

        // Get sender from map and send rejection response
        if self.confirmation_channels.reject(tool_id, reason.clone()) {
            info!(
                "User rejected tool execution: tool_id={}, reason={}",
                tool_id, reason
            );
            Ok(())
        } else {
            // If the channel does not exist, mark it as rejected directly.
            self.state_manager
                .update_state(
                    tool_id,
                    ToolExecutionState::Rejected {
                        reason: format!("User rejected: {}", reason),
                        duration_ms: None,
                        queue_wait_ms: None,
                        preflight_ms: None,
                        confirmation_wait_ms: None,
                        execution_ms: None,
                    },
                )
                .await;

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::core::ToolExecutionState;
    use crate::agentic::events::{EventQueue, EventQueueConfig};
    use crate::agentic::round_preempt::{
        DialogRoundInjectionInterrupt, SessionRoundInjectionBuffer,
    };
    use crate::agentic::tools::framework::{Tool, ToolResult, ValidationResult};
    use crate::agentic::tools::implementations::task::TaskTool;
    use crate::agentic::tools::tool_context_runtime::ToolUseContext;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use async_trait::async_trait;
    use bitfun_runtime_ports::{
        RoundInjection, RoundInjectionExecutionPolicy, RoundInjectionKind, RoundInjectionTarget,
        RoundInjectionToolPreemption,
    };
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::SystemTime;
    use tokio::time::{sleep, Duration};

    struct StaticTestTool {
        name: String,
        response: serde_json::Value,
        delay_ms: u64,
        needs_permissions: bool,
    }

    #[async_trait]
    impl Tool for StaticTestTool {
        fn name(&self) -> &str {
            &self.name
        }

        async fn description(&self) -> BitFunResult<String> {
            Ok("static test tool".to_string())
        }

        fn short_description(&self) -> String {
            "static test tool".to_string()
        }

        fn is_readonly(&self) -> bool {
            !self.needs_permissions
        }

        fn needs_permissions(&self, _input: Option<&serde_json::Value>) -> bool {
            self.needs_permissions
        }

        fn input_schema(&self) -> serde_json::Value {
            json!({ "type": "object" })
        }

        async fn validate_input(
            &self,
            _input: &serde_json::Value,
            _context: Option<&ToolUseContext>,
        ) -> ValidationResult {
            ValidationResult {
                result: true,
                message: None,
                error_code: None,
                meta: None,
            }
        }

        async fn call_impl(
            &self,
            _input: &serde_json::Value,
            _context: &ToolUseContext,
        ) -> BitFunResult<Vec<ToolResult>> {
            if self.delay_ms > 0 {
                sleep(Duration::from_millis(self.delay_ms)).await;
            }
            Ok(vec![ToolResult::Result {
                data: self.response.clone(),
                result_for_assistant: Some(render_tool_result_for_assistant(
                    &self.name,
                    &self.response,
                )),
                image_attachments: None,
            }])
        }
    }

    fn test_tool_pipeline() -> ToolPipeline {
        let registry = Arc::new(TokioRwLock::new(ToolRegistry::new()));
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let state_manager = Arc::new(ToolStateManager::new(event_queue));
        ToolPipeline::new(registry, state_manager, None)
    }

    fn test_tool_call(tool_id: &str, tool_name: &str) -> ToolCall {
        ToolCall {
            tool_id: tool_id.to_string(),
            tool_name: tool_name.to_string(),
            arguments: json!({ "path": "src/main.rs" }),
            raw_arguments: None,
            is_error: false,
            recovered_from_truncation: false,
        }
    }

    fn test_tool_execution_context() -> ToolExecutionContext {
        ToolExecutionContext {
            session_id: "session_1".to_string(),
            dialog_turn_id: "turn_1".to_string(),
            round_id: "round_1".to_string(),
            attempt_id: None,
            attempt_index: None,
            agent_type: "agent".to_string(),
            workspace: None,
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            context_vars: HashMap::new(),
            subagent_parent_info: None,
            delegation_policy: bitfun_runtime_ports::DelegationPolicy::top_level(),
            collapsed_tools: Vec::new(),
            unlocked_collapsed_tools: Vec::new(),
            allowed_tools: Vec::new(),
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            steering_interrupt: None,
            workspace_services: None,
            terminal_port: None,
            remote_exec_port: None,
        }
    }

    fn test_tool_task(tool_id: &str, tool_name: &str) -> ToolTask {
        ToolTask::new(
            test_tool_call(tool_id, tool_name),
            test_tool_execution_context(),
            ToolExecutionOptions::default(),
        )
    }

    async fn register_static_test_tool(
        pipeline: &ToolPipeline,
        name: &str,
        response: serde_json::Value,
        delay_ms: u64,
    ) {
        pipeline
            .tool_registry
            .write()
            .await
            .register_tool(Arc::new(StaticTestTool {
                name: name.to_string(),
                response,
                delay_ms,
                needs_permissions: false,
            }));
    }

    async fn register_permissioned_static_test_tool(
        pipeline: &ToolPipeline,
        name: &str,
        response: serde_json::Value,
        delay_ms: u64,
    ) {
        pipeline
            .tool_registry
            .write()
            .await
            .register_tool(Arc::new(StaticTestTool {
                name: name.to_string(),
                response,
                delay_ms,
                needs_permissions: true,
            }));
    }

    fn test_round_injection(
        kind: RoundInjectionKind,
        tool_preemption: RoundInjectionToolPreemption,
    ) -> RoundInjection {
        RoundInjection {
            id: format!("injection-{:?}-{:?}", kind, tool_preemption),
            kind,
            execution_policy: RoundInjectionExecutionPolicy::new(tool_preemption),
            target: RoundInjectionTarget::CurrentRunningTurn,
            content: "test injection".to_string(),
            display_content: "test injection".to_string(),
            created_at: SystemTime::now(),
        }
    }

    fn assert_failed_task_contains(pipeline: &ToolPipeline, tool_id: &str, expected: &str) {
        let task = pipeline
            .state_manager
            .get_task(tool_id)
            .unwrap_or_else(|| panic!("{tool_id} task should be retained"));
        match task.state {
            ToolExecutionState::Failed { error, .. } => assert!(
                error.contains(expected),
                "failed task error should contain '{expected}', got '{error}'"
            ),
            state => panic!("expected failed task state, got {state:?}"),
        }
    }

    #[test]
    fn steering_interrupted_result_preserves_tool_call_identity() {
        let task = test_tool_task("tool_1", "Read");
        let result = build_user_steering_interrupted_result("tool_1", Some(task));

        assert_eq!(result.tool_id, "tool_1");
        assert_eq!(result.tool_name, "Read");
        assert!(result.result.is_error);
        assert_eq!(
            result.result.result["category"],
            serde_json::Value::String("user_steering_interrupted".to_string())
        );
        assert_eq!(
            result.result.result_for_assistant.as_deref(),
            Some(USER_STEERING_INTERRUPTED_MESSAGE)
        );
    }

    #[test]
    fn error_result_prefers_raw_arguments_preview_when_available() {
        let mut task = test_tool_task("tool_1", "Git");
        task.tool_call.arguments = json!({});
        task.tool_call.raw_arguments = Some("{\"operation\":\"log\"".to_string());

        let result = build_error_execution_result(
            "tool_1",
            Some(task),
            &BitFunError::Validation("Arguments are invalid JSON.".to_string()),
        );

        assert_eq!(
            result.result.result["provided_arguments"],
            serde_json::Value::String("{\"operation\":\"log\"".to_string())
        );
        assert!(result
            .result
            .result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("Provided arguments: {\"operation\":\"log\""));
        assert!(!result
            .result
            .result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("Raw arguments:"));
    }

    #[tokio::test]
    async fn confirmation_timeout_returns_timeout_result_without_argument_error() {
        let pipeline = test_tool_pipeline();
        register_permissioned_static_test_tool(
            &pipeline,
            "ExecCommand",
            json!({ "unexpected": true }),
            0,
        )
        .await;

        let task_id = "tool_1".to_string();
        pipeline
            .state_manager
            .create_task(ToolTask::new(
                test_tool_call(&task_id, "ExecCommand"),
                test_tool_execution_context(),
                {
                    let mut options = ToolExecutionOptions::default();
                    options.confirm_before_run = true;
                    options.confirmation_timeout_secs = Some(1);
                    options
                },
            ))
            .await;

        // The public execute_tools path is easier to keep stable via timeout
        // by not delivering confirmation. We use a second task with the same
        // setup to exercise the actual pipeline path.
        let results = pipeline
            .execute_tools(
                vec![test_tool_call("tool_1", "ExecCommand")],
                test_tool_execution_context(),
                {
                    let mut options = ToolExecutionOptions::default();
                    options.confirmation_timeout_secs = Some(0);
                    options
                },
            )
            .await
            .expect("timeout should be returned as a tool result");

        assert_eq!(results.len(), 1);
        let result = &results[0].result;
        assert!(!result.is_error);
        assert_eq!(result.result["category"], json!("confirmation_timeout"));
        assert_eq!(result.result["tool_name"], json!("ExecCommand"));
        assert!(result.result["provided_arguments"].is_null());
        let assistant_text = result.result_for_assistant.as_deref().unwrap_or_default();
        assert!(assistant_text.contains("confirmation window expired"));
        assert!(!assistant_text.contains("failed"));
        assert!(!assistant_text.contains("Provided arguments"));
    }

    #[tokio::test]
    async fn confirmation_rejection_returns_user_rejected_result_without_argument_error() {
        let pipeline = test_tool_pipeline();
        register_permissioned_static_test_tool(
            &pipeline,
            "ExecCommand",
            json!({ "unexpected": true }),
            0,
        )
        .await;

        let reject_pipeline = pipeline.clone();
        let reject_handle = tokio::spawn(async move {
            for _ in 0..50 {
                if reject_pipeline
                    .state_manager
                    .get_task("tool_1")
                    .is_some_and(|task| {
                        matches!(task.state, ToolExecutionState::AwaitingConfirmation { .. })
                    })
                {
                    reject_pipeline
                        .reject_tool(
                            "tool_1",
                            "Use the built-in status view instead.".to_string(),
                        )
                        .await
                        .expect("reject tool confirmation");
                    return;
                }
                sleep(Duration::from_millis(10)).await;
            }
            panic!("tool should enter awaiting confirmation");
        });

        let results = pipeline
            .execute_tools(
                vec![test_tool_call("tool_1", "ExecCommand")],
                test_tool_execution_context(),
                ToolExecutionOptions::default(),
            )
            .await
            .expect("user rejection should be returned as a tool result");
        reject_handle.await.expect("rejection task should finish");

        assert_eq!(results.len(), 1);
        let result = &results[0].result;
        assert!(!result.is_error);
        assert_eq!(result.result["status"], json!("rejected"));
        assert_eq!(result.result["category"], json!("user_rejected"));
        assert_eq!(result.result["tool_name"], json!("ExecCommand"));
        assert_eq!(
            result.result["instruction"],
            json!("Use the built-in status view instead.")
        );
        assert!(result.result["provided_arguments"].is_null());

        let assistant_text = result.result_for_assistant.as_deref().unwrap_or_default();
        assert!(assistant_text.contains(
            "The user rejected this tool call with the following instruction: \"Use the built-in status view instead.\""
        ));
        assert!(assistant_text.contains("Do not retry it unless the user explicitly asks you to."));
        assert!(!assistant_text.contains("invalid_arguments"));
        assert!(!assistant_text.contains("Provided arguments"));
        assert!(!assistant_text.contains("failed"));

        let task = pipeline
            .state_manager
            .get_task("tool_1")
            .expect("tool task should be retained");
        assert!(matches!(task.state, ToolExecutionState::Rejected { .. }));
    }

    #[tokio::test]
    async fn pipeline_admission_allowed_list_rejection_updates_failed_state_before_registry_lookup()
    {
        let pipeline = test_tool_pipeline();
        let mut context = test_tool_execution_context();
        context.allowed_tools = vec!["Read".to_string()];

        let results = pipeline
            .execute_tools(
                vec![test_tool_call("tool_1", "UnregisteredBlockedTool")],
                context,
                ToolExecutionOptions::default(),
            )
            .await
            .expect("admission rejection should be returned as an error result");

        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_error);
        assert_failed_task_contains(
            &pipeline,
            "tool_1",
            "Tool 'UnregisteredBlockedTool' is not in the allowed list",
        );
        assert!(
            results[0]
                .result
                .result_for_assistant
                .as_deref()
                .unwrap_or_default()
                .contains("UnregisteredBlockedTool"),
            "error result should preserve rejected tool identity"
        );
    }

    #[tokio::test]
    async fn pipeline_admission_runtime_restriction_rejection_updates_failed_state() {
        let pipeline = test_tool_pipeline();
        let mut context = test_tool_execution_context();
        context
            .runtime_tool_restrictions
            .denied_tool_names
            .insert("Read".to_string());

        let results = pipeline
            .execute_tools(
                vec![test_tool_call("tool_1", "Read")],
                context,
                ToolExecutionOptions::default(),
            )
            .await
            .expect("admission rejection should be returned as an error result");

        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_error);
        assert_failed_task_contains(
            &pipeline,
            "tool_1",
            "Tool 'Read' is denied by runtime restrictions",
        );
    }

    #[tokio::test]
    async fn pipeline_admission_collapsed_tool_rejection_updates_failed_state_before_validation() {
        let pipeline = test_tool_pipeline();
        let mut context = test_tool_execution_context();
        context.collapsed_tools = vec!["WebFetch".to_string()];

        let results = pipeline
            .execute_tools(
                vec![test_tool_call("tool_1", "WebFetch")],
                context,
                ToolExecutionOptions::default(),
            )
            .await
            .expect("admission rejection should be returned as an error result");

        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_error);
        assert_failed_task_contains(
            &pipeline,
            "tool_1",
            "Call GetToolSpec first with {\"tool_name\":\"WebFetch\"}",
        );
    }

    #[tokio::test]
    async fn background_result_pending_does_not_skip_tool_execution() {
        let pipeline = test_tool_pipeline();
        register_static_test_tool(&pipeline, "Read", json!({ "ok": true }), 0).await;

        let buffer = Arc::new(SessionRoundInjectionBuffer::default());
        buffer.push(
            "session_1",
            test_round_injection(
                RoundInjectionKind::BackgroundResult,
                RoundInjectionKind::BackgroundResult
                    .default_execution_policy()
                    .tool_preemption,
            ),
        );

        let mut context = test_tool_execution_context();
        context.steering_interrupt = Some(DialogRoundInjectionInterrupt::new(
            "session_1".to_string(),
            "turn_1".to_string(),
            buffer,
        ));

        let results = pipeline
            .execute_tools(
                vec![test_tool_call("tool_1", "Read")],
                context,
                ToolExecutionOptions::default(),
            )
            .await
            .expect("background result should not skip tool execution");

        assert_eq!(results.len(), 1);
        assert!(!results[0].result.is_error);
        assert_eq!(results[0].result.result["ok"], json!(true));
    }

    #[tokio::test]
    async fn user_steering_pending_still_skips_remaining_tool_plan() {
        let pipeline = test_tool_pipeline();
        let buffer = Arc::new(SessionRoundInjectionBuffer::default());
        buffer.push(
            "session_1",
            test_round_injection(
                RoundInjectionKind::UserSteering,
                RoundInjectionKind::UserSteering
                    .default_execution_policy()
                    .tool_preemption,
            ),
        );

        let mut context = test_tool_execution_context();
        context.steering_interrupt = Some(DialogRoundInjectionInterrupt::new(
            "session_1".to_string(),
            "turn_1".to_string(),
            buffer,
        ));

        let results = pipeline
            .execute_tools(
                vec![
                    test_tool_call("tool_1", "Read"),
                    test_tool_call("tool_2", "Write"),
                ],
                context,
                ToolExecutionOptions::default(),
            )
            .await
            .expect("user steering skip should be surfaced as tool results");

        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].result.result["category"],
            json!("user_steering_interrupted")
        );
        assert_eq!(
            results[1].result.result["category"],
            json!("user_steering_interrupted")
        );
    }

    #[tokio::test]
    async fn custom_round_injection_can_cancel_running_tool_cooperatively() {
        let pipeline = test_tool_pipeline();
        register_static_test_tool(&pipeline, "Read", json!({ "ok": true }), 30_000).await;

        let buffer = Arc::new(SessionRoundInjectionBuffer::default());
        let buffer_for_injection = buffer.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            buffer_for_injection.push(
                "session_1",
                test_round_injection(
                    RoundInjectionKind::UserSteering,
                    RoundInjectionToolPreemption::CancelRunningCooperatively,
                ),
            );
        });

        let mut context = test_tool_execution_context();
        context.steering_interrupt = Some(DialogRoundInjectionInterrupt::new(
            "session_1".to_string(),
            "turn_1".to_string(),
            buffer,
        ));
        let mut options = ToolExecutionOptions::default();
        options.allow_parallel = false;

        let results = pipeline
            .execute_tools(vec![test_tool_call("tool_1", "Read")], context, options)
            .await
            .expect("cooperative cancel should still return a tool result");

        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_error);
        assert_eq!(results[0].result.result["category"], json!("cancelled"));
    }

    #[test]
    fn fallback_assistant_text_preserves_full_structured_result() {
        let result = convert_tool_result(
            FrameworkToolResult::Result {
                data: json!({
                    "success": false,
                    "exit_code": 1,
                    "working_directory": "/private/tmp",
                    "output": "ERR_PNPM_NO_PKG_MANIFEST"
                }),
                result_for_assistant: None,
                image_attachments: None,
            },
            "tool_1",
            "Bash",
        );

        let assistant_text = result.result_for_assistant.unwrap_or_default();
        assert!(assistant_text.contains("\"success\": false"));
        assert!(assistant_text.contains("\"exit_code\": 1"));
        assert!(assistant_text.contains("\"working_directory\": \"/private/tmp\""));
        assert!(!assistant_text.contains("completed with error"));
    }

    #[test]
    fn truncation_notice_for_interactive_tools_does_not_claim_file_write() {
        let notice = build_tool_call_truncation_recovery_notice("AskUserQuestion");

        assert!(notice.contains("AskUserQuestion call was truncated"));
        assert!(notice.contains("fresh complete AskUserQuestion call"));
        assert!(!notice.contains("file was written"));
        assert!(!notice.contains("issue ONE Edit call"));
    }

    #[test]
    fn truncation_notice_for_write_tools_keeps_write_continuation_guidance() {
        let notice = build_tool_call_truncation_recovery_notice("Write");

        assert!(notice.contains("file may have been written with partial content"));
        assert!(notice.contains("latest Read result"));
        assert!(notice.contains("issue ONE Write call"));
        assert!(notice.contains("`mode`: \"a\""));
    }

    #[test]
    fn pipeline_preserves_core_owned_tool_context_without_portable_runtime_leak() {
        let pipeline = test_tool_pipeline();
        let mut task = test_tool_task("tool_context_1", "WebFetch");
        task.context
            .context_vars
            .insert("turn_index".to_string(), "7".to_string());
        task.context
            .context_vars
            .insert("acp_transport".to_string(), "true".to_string());
        task.context.collapsed_tools = vec!["WebFetch".to_string()];
        task.context.unlocked_collapsed_tools = vec!["WebFetch".to_string()];
        task.context.runtime_tool_restrictions = ToolRuntimeRestrictions {
            allowed_tool_names: ["WebFetch"].into_iter().map(str::to_string).collect(),
            denied_tool_names: ["Bash"].into_iter().map(str::to_string).collect(),
            denied_tool_messages: Default::default(),
            path_policy: Default::default(),
        };

        let context = pipeline.build_tool_use_context(&task, CancellationToken::new());

        assert_eq!(context.tool_call_id.as_deref(), Some("tool_context_1"));
        assert_eq!(context.agent_type.as_deref(), Some("agent"));
        assert_eq!(context.session_id.as_deref(), Some("session_1"));
        assert_eq!(context.dialog_turn_id.as_deref(), Some("turn_1"));
        assert_eq!(context.unlocked_collapsed_tools, vec!["WebFetch"]);
        assert!(context.cancellation_token().is_some());
        assert!(context
            .runtime_tool_restrictions
            .is_tool_allowed("WebFetch"));
        assert!(!context.runtime_tool_restrictions.is_tool_allowed("Bash"));
        assert_eq!(context.custom_data["turn_index"], json!(7));
        assert!(!context.custom_data.contains_key("primary_model_provider"));
        assert!(!context
            .custom_data
            .contains_key("primary_model_supports_image_understanding"));
        assert_eq!(context.custom_data["acp_transport"], json!(true));

        let facts = context.to_tool_context_facts();
        let value = serde_json::to_value(&facts).expect("serialize context facts");
        assert_eq!(value["toolCallId"], "tool_context_1");
        assert_eq!(value["sessionId"], "session_1");
        assert!(value.get("unlockedCollapsedTools").is_none());
        assert!(value.get("customData").is_none());
        assert!(value.get("cancellationToken").is_none());
        assert!(value.get("workspaceServices").is_none());
    }

    #[test]
    fn collapsed_tool_requires_tool_catalog_unlock() {
        let mut task = test_tool_task("tool_1", "WebFetch");
        task.context.collapsed_tools = vec!["WebFetch".to_string()];

        let err = validate_tool_execution_admission(ToolExecutionAdmissionRequest {
            tool_name: &task.tool_call.tool_name,
            allowed_tools: &task.context.allowed_tools,
            runtime_tool_restrictions: &task.context.runtime_tool_restrictions,
            collapsed_tools: &task.context.collapsed_tools,
            loaded_collapsed_tools: &task.context.unlocked_collapsed_tools,
            get_tool_spec_tool_name: GET_TOOL_SPEC_TOOL_NAME,
        })
        .expect_err("collapsed tool should require GetToolSpec unlock");

        assert!(err
            .to_string()
            .contains("Call GetToolSpec first with {\"tool_name\":\"WebFetch\"}"));
    }

    #[test]
    fn tool_catalog_rejects_reloading_already_unlocked_tool() {
        let mut task = test_tool_task("tool_1", "GetToolSpec");
        task.tool_call.arguments = json!({ "tool_name": "WebFetch" });
        task.context.unlocked_collapsed_tools = vec!["WebFetch".to_string()];

        let result = validate_tool_execution_admission(ToolExecutionAdmissionRequest {
            tool_name: &task.tool_call.tool_name,
            allowed_tools: &task.context.allowed_tools,
            runtime_tool_restrictions: &task.context.runtime_tool_restrictions,
            collapsed_tools: &task.context.collapsed_tools,
            loaded_collapsed_tools: &task.context.unlocked_collapsed_tools,
            get_tool_spec_tool_name: GET_TOOL_SPEC_TOOL_NAME,
        });

        assert!(
            result.is_ok(),
            "GetToolSpec duplicate-load validation moved into GetToolSpec itself"
        );
    }

    #[test]
    fn task_tool_manages_its_own_execution_timeout() {
        let task_tool = TaskTool::new();
        assert!(task_tool.manages_own_execution_timeout());
    }
}
