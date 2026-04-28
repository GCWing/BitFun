//! Tool pipeline
//!
//! Manages the complete lifecycle of tools:
//! confirmation, execution, caching, retries, etc.

use super::state_manager::ToolStateManager;
use super::types::*;
use crate::agentic::core::{ToolCall, ToolExecutionState, ToolResult as ModelToolResult};
use crate::agentic::events::types::ToolEventData;
use crate::agentic::tools::computer_use_host::ComputerUseHostRef;
use crate::agentic::tools::framework::{ToolResult as FrameworkToolResult, ToolUseContext};
use crate::agentic::tools::registry::ToolRegistry;
use crate::util::elapsed_ms_u64;
use crate::util::errors::{BitFunError, BitFunResult};
use dashmap::DashMap;
use futures::future::join_all;
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tokio::sync::{oneshot, RwLock as TokioRwLock};
use tokio::time::{timeout, Duration};
use tokio_util::sync::CancellationToken;

/// A batch of tool tasks to execute together.
struct ToolBatch {
    task_ids: Vec<String>,
    is_concurrent: bool,
}

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
            // If the tool does not provide result_for_assistant, generate default friendly description
            let assistant_text = result_for_assistant.or_else(|| {
                // Generate natural language description based on data
                generate_default_assistant_text(tool_name, &data)
            });

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
            // Progress message also generates friendly text
            let assistant_text = generate_default_assistant_text(tool_name, &content);

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
            // Streaming data block also generates friendly text
            let assistant_text = generate_default_assistant_text(tool_name, &data);

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

/// Generate default tool result description
fn generate_default_assistant_text(tool_name: &str, data: &serde_json::Value) -> Option<String> {
    // Check if data is null or empty
    if data.is_null() {
        return Some(format!(
            "Tool {} completed, but no result returned.",
            tool_name
        ));
    }

    // If it is an empty object or empty array
    if (data.is_object() && data.as_object().is_some_and(|o| o.is_empty()))
        || (data.is_array() && data.as_array().is_some_and(|a| a.is_empty()))
    {
        return Some(format!(
            "Tool {} completed, returned empty result.",
            tool_name
        ));
    }

    // Try to extract common fields to generate description
    if let Some(obj) = data.as_object() {
        // Check if there is a success field
        if let Some(success) = obj.get("success").and_then(|v| v.as_bool()) {
            if success {
                if let Some(message) = obj.get("message").and_then(|v| v.as_str()) {
                    return Some(format!(
                        "Tool {} completed successfully: {}",
                        tool_name, message
                    ));
                }
                return Some(format!("Tool {} completed successfully.", tool_name));
            } else {
                if let Some(error) = obj.get("error").and_then(|v| v.as_str()) {
                    return Some(format!(
                        "Tool {} completed with error: {}",
                        tool_name, error
                    ));
                }
                return Some(format!("Tool {} completed with error.", tool_name));
            }
        }

        // Check if there is a result/data/content field
        for key in &["result", "data", "content", "output"] {
            if let Some(value) = obj.get(*key) {
                if let Some(text) = value.as_str() {
                    if !text.is_empty() && text.len() < 500 {
                        return Some(format!("Tool {} completed, returned: {}", tool_name, text));
                    }
                }
            }
        }

        // If there are multiple fields, provide field list
        let field_names: Vec<&str> = obj.keys().take(5).map(|s| s.as_str()).collect();
        if !field_names.is_empty() {
            return Some(format!(
                "Tool {} completed, returned data with the following fields: {}",
                tool_name,
                field_names.join(", ")
            ));
        }
    }

    // If it is a string, return directly (but limit length)
    if let Some(text) = data.as_str() {
        if !text.is_empty() {
            if text.len() <= 500 {
                return Some(format!("Tool {} completed: {}", tool_name, text));
            } else {
                return Some(format!(
                    "Tool {} completed, returned {} characters of text result.",
                    tool_name,
                    text.len()
                ));
            }
        }
    }

    // If it is a number or boolean
    if data.is_number() || data.is_boolean() {
        return Some(format!("Tool {} completed, returned: {}", tool_name, data));
    }

    // Default: simply describe data type
    Some(format!(
        "Tool {} completed, returned {} type of result.",
        tool_name,
        if data.is_object() {
            "object"
        } else if data.is_array() {
            "array"
        } else {
            "data"
        }
    ))
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

fn build_error_execution_result(
    task_id: &str,
    task: Option<ToolTask>,
    error: &BitFunError,
) -> ToolExecutionResult {
    let (tool_id, tool_name, execution_time_ms) = if let Some(task) = task {
        (
            task.tool_call.tool_id,
            task.tool_call.tool_name,
            elapsed_ms_since(task.created_at),
        )
    } else {
        warn!("Task not found in state manager: {}", task_id);
        (task_id.to_string(), "unknown".to_string(), 0)
    };
    let error_message = error.to_string();

    ToolExecutionResult {
        tool_id: tool_id.clone(),
        tool_name: tool_name.clone(),
        result: ModelToolResult {
            tool_id,
            tool_name,
            result: serde_json::json!({
                "error": error_message,
                "message": format!("Tool execution failed: {}", error_message)
            }),
            result_for_assistant: Some(format!("Tool execution failed: {}", error_message)),
            is_error: true,
            duration_ms: Some(execution_time_ms),
            image_attachments: None,
        },
        execution_time_ms,
    }
}

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

/// Confirmation response type
#[derive(Debug, Clone)]
pub enum ConfirmationResponse {
    Confirmed,
    Rejected(String),
}

/// Tool pipeline
pub struct ToolPipeline {
    tool_registry: Arc<TokioRwLock<ToolRegistry>>,
    state_manager: Arc<ToolStateManager>,
    /// Confirmation channel management (tool_id -> oneshot sender)
    confirmation_channels: Arc<DashMap<String, oneshot::Sender<ConfirmationResponse>>>,
    /// Cancellation token management (tool_id -> CancellationToken)
    cancellation_tokens: Arc<DashMap<String, CancellationToken>>,
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
            confirmation_channels: Arc::new(DashMap::new()),
            cancellation_tokens: Arc::new(DashMap::new()),
            computer_use_host,
        }
    }

    pub fn computer_use_host(&self) -> Option<ComputerUseHostRef> {
        self.computer_use_host.clone()
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

        // Determine concurrency safety for each tool call
        let concurrency_flags: Vec<bool> = {
            let registry = self.tool_registry.read().await;
            tool_calls
                .iter()
                .map(|tc| {
                    registry
                        .get_tool(&tc.tool_name)
                        .map(|tool| tool.is_concurrency_safe(Some(&tc.arguments)))
                        .unwrap_or(false)
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
        let batches = Self::partition_tool_batches(&task_ids, &concurrency_flags);
        debug!(
            "Tool execution plan: total_tools={}, batches={}, concurrency_safe={}, non_concurrency_safe={}, allow_parallel=true, tools={}",
            task_ids.len(),
            batches.len(),
            concurrency_safe_count,
            task_ids.len().saturating_sub(concurrency_safe_count),
            tool_names.join(", ")
        );

        if batches.len() == 1 {
            let batch = &batches[0];
            if batch.is_concurrent {
                return self.execute_parallel(batch.task_ids.clone()).await;
            } else {
                return self.execute_sequential(batch.task_ids.clone()).await;
            }
        }

        debug!(
            "Partitioned {} tools into {} batches for mixed execution",
            task_ids.len(),
            batches.len()
        );

        let mut all_results = Vec::with_capacity(task_ids.len());
        for (batch_idx, batch) in batches.into_iter().enumerate() {
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

    /// Partition task IDs into batches where consecutive concurrency-safe tasks
    /// are grouped together (parallel batch) and each non-safe task forms its
    /// own batch (serial batch).
    fn partition_tool_batches(task_ids: &[String], flags: &[bool]) -> Vec<ToolBatch> {
        let mut batches: Vec<ToolBatch> = Vec::new();

        for (id, &is_safe) in task_ids.iter().zip(flags.iter()) {
            if is_safe {
                if let Some(last) = batches.last_mut() {
                    if last.is_concurrent {
                        last.task_ids.push(id.clone());
                        continue;
                    }
                }
            }
            batches.push(ToolBatch {
                task_ids: vec![id.clone()],
                is_concurrent: is_safe,
            });
        }

        batches
    }

    /// Execute tools in parallel
    async fn execute_parallel(
        &self,
        task_ids: Vec<String>,
    ) -> BitFunResult<Vec<ToolExecutionResult>> {
        let futures: Vec<_> = task_ids
            .iter()
            .map(|id| self.execute_single_tool(id.clone()))
            .collect();

        let results = join_all(futures).await;

        // Collect results, including failed results
        let mut all_results = Vec::new();
        for (idx, result) in results.into_iter().enumerate() {
            match result {
                Ok(r) => all_results.push(r),
                Err(e) => {
                    error!("Tool execution failed: error={}", e);
                    let task_id = &task_ids[idx];
                    let error_result = build_error_execution_result(
                        task_id,
                        self.state_manager.get_task(task_id),
                        &e,
                    );
                    all_results.push(error_result);
                }
            }
        }

        Ok(all_results)
    }

    /// Execute tools sequentially
    async fn execute_sequential(
        &self,
        task_ids: Vec<String>,
    ) -> BitFunResult<Vec<ToolExecutionResult>> {
        let mut results = Vec::new();

        for task_id in task_ids {
            match self.execute_single_tool(task_id.clone()).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!("Tool execution failed: error={}", e);
                    let error_result = build_error_execution_result(
                        &task_id,
                        self.state_manager.get_task(&task_id),
                        &e,
                    );
                    results.push(error_result);
                }
            }
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
        let queue_wait_ms = elapsed_ms_since(task.created_at);
        let mut confirmation_wait_ms = 0;

        debug!(
            "Tool task details: tool_name={}, tool_id={}, queue_wait_ms={}",
            tool_name, tool_id, queue_wait_ms
        );

        if tool_name.is_empty() || tool_is_error {
            let error_msg = if tool_name.is_empty() && tool_is_error {
                "Missing valid tool name and arguments are invalid JSON.".to_string()
            } else if tool_name.is_empty() {
                "Missing valid tool name.".to_string()
            } else {
                "Arguments are invalid JSON.".to_string()
            };
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Failed {
                        error: error_msg.clone(),
                        is_retryable: false,
                    },
                )
                .await;

            return Err(BitFunError::Validation(error_msg));
        }

        // Security check: check if the tool is in the allowed list
        // If allowed_tools is not empty, only allow execution of tools in the whitelist
        if !task.context.allowed_tools.is_empty()
            && !task.context.allowed_tools.contains(&tool_name)
        {
            let error_msg = format!(
                "Tool '{}' is not in the allowed list: {:?}",
                tool_name, task.context.allowed_tools
            );
            warn!("Tool not allowed: {}", error_msg);

            // Update state to failed
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Failed {
                        error: error_msg.clone(),
                        is_retryable: false,
                    },
                )
                .await;

            return Err(BitFunError::Validation(error_msg));
        }

        if let Err(err) = task
            .context
            .runtime_tool_restrictions
            .ensure_tool_allowed(&tool_name)
        {
            let error_msg = err.to_string();
            warn!("Tool rejected by runtime restrictions: {}", error_msg);

            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Failed {
                        error: error_msg,
                        is_retryable: false,
                    },
                )
                .await;

            return Err(err);
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

        let needs_confirmation =
            task.options.confirm_before_run && tool.needs_permissions(Some(&tool_args));

        if needs_confirmation {
            info!("Tool requires confirmation: tool_name={}", tool_name);

            let (tx, rx) = oneshot::channel::<ConfirmationResponse>();

            // Use 1 year as an approximation of "infinite" when there is no timeout, to avoid overflow
            const ONE_YEAR_SECS: u64 = 365 * 24 * 60 * 60;
            let timeout_at = match task.options.confirmation_timeout_secs {
                Some(secs) => std::time::SystemTime::now() + Duration::from_secs(secs),
                None => std::time::SystemTime::now() + Duration::from_secs(ONE_YEAR_SECS),
            };

            self.confirmation_channels.insert(tool_id.clone(), tx);

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

            let confirmation_result = match task.options.confirmation_timeout_secs {
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

            match confirmation_result {
                Some(Ok(ConfirmationResponse::Confirmed)) => {
                    debug!("Tool confirmed: tool_name={}", tool_name);
                }
                Some(Ok(ConfirmationResponse::Rejected(reason))) => {
                    self.state_manager
                        .update_state(
                            &tool_id,
                            ToolExecutionState::Cancelled {
                                reason: format!("User rejected: {}", reason),
                            },
                        )
                        .await;

                    return Err(BitFunError::Validation(format!(
                        "Tool was rejected by user: {}",
                        reason
                    )));
                }
                Some(Err(_)) => {
                    self.confirmation_channels.remove(&tool_id);

                    // Channel closed
                    self.state_manager
                        .update_state(
                            &tool_id,
                            ToolExecutionState::Cancelled {
                                reason: "Confirmation channel closed".to_string(),
                            },
                        )
                        .await;

                    return Err(BitFunError::service("Confirmation channel closed"));
                }
                None => {
                    self.confirmation_channels.remove(&tool_id);

                    self.state_manager
                        .update_state(
                            &tool_id,
                            ToolExecutionState::Cancelled {
                                reason: "Confirmation timeout".to_string(),
                            },
                        )
                        .await;

                    warn!("Confirmation timeout: {}", tool_name);
                    return Err(BitFunError::Timeout(format!(
                        "Confirmation timeout: {}",
                        tool_name
                    )));
                }
            }

            self.confirmation_channels.remove(&tool_id);
        }

        let preflight_ms = elapsed_ms_u64(start_time).saturating_sub(confirmation_wait_ms);

        if cancellation_token.is_cancelled() {
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Cancelled {
                        reason: "Tool was cancelled before execution".to_string(),
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
        let result = self
            .execute_with_retry(&task, cancellation_token.clone(), tool)
            .await;
        let execution_ms = elapsed_ms_u64(execution_started_at);

        self.cancellation_tokens.remove(&tool_id);

        match result {
            Ok(tool_result) => {
                let duration_ms = elapsed_ms_u64(start_time);
                let mut tool_result = tool_result;
                tool_result.duration_ms = Some(duration_ms);

                self.state_manager
                    .update_state(
                        &tool_id,
                        ToolExecutionState::Completed {
                            result: convert_to_framework_result(&tool_result),
                            duration_ms,
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

                let error_msg = e.to_string();
                let is_retryable = task.options.max_retries > 0;

                self.state_manager
                    .update_state(
                        &tool_id,
                        ToolExecutionState::Failed {
                            error: error_msg.clone(),
                            is_retryable,
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
                    if attempts >= max_attempts || !should_retry_tool_error(&e) {
                        return Err(e);
                    }

                    debug!(
                        "Retrying tool execution: attempt={}/{}, error={}",
                        attempts, max_attempts, e
                    );

                    // Wait for a period of time and retry
                    tokio::time::sleep(Duration::from_millis(100 * attempts as u64)).await;
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

        let tool_results = match task.options.timeout_secs {
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
        ToolUseContext {
            tool_call_id: Some(task.tool_call.tool_id.clone()),
            agent_type: Some(task.context.agent_type.clone()),
            session_id: Some(task.context.session_id.clone()),
            dialog_turn_id: Some(task.context.dialog_turn_id.clone()),
            workspace: task.context.workspace.clone(),
            custom_data: {
                let mut map = HashMap::new();

                if let Some(turn_index) = task.context.context_vars.get("turn_index") {
                    if let Ok(n) = turn_index.parse::<u64>() {
                        map.insert("turn_index".to_string(), serde_json::json!(n));
                    }
                }

                if let Some(provider) = task.context.context_vars.get("primary_model_provider") {
                    if !provider.is_empty() {
                        map.insert(
                            "primary_model_provider".to_string(),
                            serde_json::json!(provider),
                        );
                    }
                }
                if let Some(supports_images) = task
                    .context
                    .context_vars
                    .get("primary_model_supports_image_understanding")
                {
                    if let Ok(flag) = supports_images.parse::<bool>() {
                        map.insert(
                            "primary_model_supports_image_understanding".to_string(),
                            serde_json::json!(flag),
                        );
                    }
                }

                map
            },
            computer_use_host: self.computer_use_host.clone(),
            cancellation_token: Some(cancellation_token),
            runtime_tool_restrictions: task.context.runtime_tool_restrictions.clone(),
            workspace_services: task.context.workspace_services.clone(),
        }
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

        match &task.state {
            ToolExecutionState::Completed { .. }
            | ToolExecutionState::Failed { .. }
            | ToolExecutionState::Cancelled { .. } => {
                debug!(
                    "Ignoring duplicate cancel request for tool in terminal state: tool_id={}, state={:?}",
                    tool_id, task.state
                );
                return Ok(());
            }
            _ => {}
        }

        // 1. Trigger cancellation token
        if let Some((_, token)) = self.cancellation_tokens.remove(tool_id) {
            token.cancel();
            debug!("Cancellation token triggered: tool_id={}", tool_id);
        } else {
            debug!(
                "Cancellation token not found (tool may have completed): tool_id={}",
                tool_id
            );
        }

        // 2. Clean up confirmation channel (if waiting for confirmation)
        if let Some((_, _tx)) = self.confirmation_channels.remove(tool_id) {
            // Channel will be automatically closed, causing await rx to return Err
            debug!("Cleared confirmation channel: tool_id={}", tool_id);
        }

        // 3. Update state to cancelled
        self.state_manager
            .update_state(
                tool_id,
                ToolExecutionState::Cancelled {
                    reason: reason.clone(),
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

        let mut cancelled_count = 0;
        let mut skipped_count = 0;

        for task in tasks {
            // Only cancel tasks in cancellable states
            let can_cancel = matches!(
                task.state,
                ToolExecutionState::Queued { .. }
                    | ToolExecutionState::Waiting { .. }
                    | ToolExecutionState::Running { .. }
                    | ToolExecutionState::AwaitingConfirmation { .. }
            );

            if can_cancel {
                debug!(
                    "Cancelling tool: tool_id={}, state={:?}",
                    task.tool_call.tool_id, task.state
                );
                self.cancel_tool(&task.tool_call.tool_id, "Dialog turn cancelled".to_string())
                    .await?;
                cancelled_count += 1;
            } else {
                debug!(
                    "Skipping tool (state not cancellable): tool_id={}, state={:?}",
                    task.tool_call.tool_id, task.state
                );
                skipped_count += 1;
            }
        }

        info!(
            "Tool cancellation completed: cancelled={}, skipped={}",
            cancelled_count, skipped_count
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
        if let Some((_, tx)) = self.confirmation_channels.remove(tool_id) {
            let _ = tx.send(ConfirmationResponse::Confirmed);
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
        if let Some((_, tx)) = self.confirmation_channels.remove(tool_id) {
            let _ = tx.send(ConfirmationResponse::Rejected(reason.clone()));
            info!(
                "User rejected tool execution: tool_id={}, reason={}",
                tool_id, reason
            );
            Ok(())
        } else {
            // If the channel does not exist, mark it as cancelled directly
            self.state_manager
                .update_state(
                    tool_id,
                    ToolExecutionState::Cancelled {
                        reason: format!("User rejected: {}", reason),
                    },
                )
                .await;

            Ok(())
        }
    }
}
