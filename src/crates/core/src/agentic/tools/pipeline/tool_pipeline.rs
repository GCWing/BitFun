//! Tool pipeline
//!
//! Manages the complete lifecycle of tools:
//! confirmation, execution, caching, retries, etc.

use super::state_manager::ToolStateManager;
use super::types::*;
use crate::agentic::core::{ToolCall, ToolExecutionState, ToolResult as ModelToolResult};
use crate::agentic::events::types::ToolEventData;
use crate::agentic::tools::framework::{
    ToolOptions, ToolResult as FrameworkToolResult, ToolUseContext,
};
use crate::agentic::tools::image_context::ImageContextProviderRef;
use crate::agentic::tools::registry::ToolRegistry;
use crate::util::errors::{BitFunError, BitFunResult};
use dashmap::DashMap;
use futures::future::join_all;
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{oneshot, RwLock as TokioRwLock};
use tokio::time::{timeout, Duration};
use tokio_util::sync::CancellationToken;

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
    if (data.is_object() && data.as_object().map_or(false, |o| o.is_empty()))
        || (data.is_array() && data.as_array().map_or(false, |a| a.is_empty()))
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
    }
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
    /// Image context provider (dependency injection)
    image_context_provider: Option<ImageContextProviderRef>,
}

impl ToolPipeline {
    pub fn new(
        tool_registry: Arc<TokioRwLock<ToolRegistry>>,
        state_manager: Arc<ToolStateManager>,
        image_context_provider: Option<ImageContextProviderRef>,
    ) -> Self {
        Self {
            tool_registry,
            state_manager,
            confirmation_channels: Arc::new(DashMap::new()),
            cancellation_tokens: Arc::new(DashMap::new()),
            image_context_provider,
        }
    }

    /// Execute multiple tool calls
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

        // Check should_end_turn tool count, if more than one, mark as error
        let end_turn_tool_ids: Vec<String> = {
            let end_turn_tools: Vec<&ToolCall> =
                tool_calls.iter().filter(|tc| tc.should_end_turn).collect();

            if end_turn_tools.len() > 1 {
                warn!(
                    "Multiple should_end_turn tools detected: count={}, tools={:?}",
                    end_turn_tools.len(),
                    end_turn_tools
                        .iter()
                        .map(|tc| &tc.tool_name)
                        .collect::<Vec<_>>()
                );
                end_turn_tools.iter().map(|tc| tc.tool_id.clone()).collect()
            } else {
                vec![]
            }
        };

        // Separate tools that need to be errors and tools that are normally executed
        let (error_tool_calls, normal_tool_calls): (Vec<ToolCall>, Vec<ToolCall>) = tool_calls
            .into_iter()
            .partition(|tc| end_turn_tool_ids.contains(&tc.tool_id));

        // Check if all tools that are normally executed are concurrency safe
        let all_concurrency_safe = {
            let registry = self.tool_registry.read().await;
            normal_tool_calls.iter().all(|tc| {
                registry
                    .get_tool(&tc.tool_name)
                    .map(|tool| tool.is_concurrency_safe(Some(&tc.arguments)))
                    .unwrap_or(false) // If the tool does not exist, it is considered unsafe
            })
        };

        // Generate error results for tools that need to be errors
        if !error_tool_calls.is_empty() {
            error!(
                "Multiple should_end_turn tools detected: {:?}",
                error_tool_calls
                    .iter()
                    .map(|tc| tc.tool_name.clone())
                    .collect::<Vec<_>>()
            );
        }
        let mut error_results: Vec<ToolExecutionResult> = error_tool_calls.into_iter().map(|tc| {
            let error_msg = format!("Tool '{}' will end the current dialog turn. Such tools must be called separately.", tc.tool_name);
            
            ToolExecutionResult {
                tool_id: tc.tool_id.clone(),
                tool_name: tc.tool_name.clone(),
                result: ModelToolResult {
                    tool_id: tc.tool_id.clone(),
                    tool_name: tc.tool_name.clone(),
                    result: serde_json::json!({
                        "error": error_msg,
                        "message": error_msg
                    }),
                    result_for_assistant: Some(error_msg),
                    is_error: true,
                    duration_ms: Some(0),
                },
                execution_time_ms: 0,
            }
        }).collect();

        // If there are no tools that are normally executed, return error results directly
        if normal_tool_calls.is_empty() {
            return Ok(error_results);
        }

        // Create tasks (only for tools that are normally executed)
        let mut tasks = Vec::new();
        for tool_call in normal_tool_calls {
            let task = ToolTask::new(tool_call, context.clone(), options.clone());
            let tool_id = self.state_manager.create_task(task).await;
            tasks.push(tool_id);
        }

        // Execute tasks: only when allow_parallel is true and all tools are concurrency safe
        let should_parallel = options.allow_parallel && all_concurrency_safe;
        if !all_concurrency_safe && options.allow_parallel {
            debug!("Non-concurrency-safe tools detected, switching to sequential execution");
        }

        let normal_results = if should_parallel {
            self.execute_parallel(tasks).await
        } else {
            self.execute_sequential(tasks).await
        };

        match normal_results {
            Ok(mut results) => {
                // Merge error results and normal execution results
                error_results.append(&mut results);
                Ok(error_results)
            }
            Err(e) => {
                error!("Tool execution failed: error={}", e);
                Err(e)
            }
        }
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

                    // Get task information from state manager
                    if let Some(task) = self.state_manager.get_task(&task_ids[idx]) {
                        // Create error result to return to model
                        let error_result = ToolExecutionResult {
                            tool_id: task.tool_call.tool_id.clone(),
                            tool_name: task.tool_call.tool_name.clone(),
                            result: ModelToolResult {
                                tool_id: task.tool_call.tool_id.clone(),
                                tool_name: task.tool_call.tool_name.clone(),
                                result: serde_json::json!({
                                    "error": e.to_string(),
                                    "message": format!("Tool execution failed: {}", e)
                                }),
                                result_for_assistant: Some(format!("Tool execution failed: {}", e)),
                                is_error: true,
                                duration_ms: None,
                            },
                            execution_time_ms: 0,
                        };
                        all_results.push(error_result);
                    }
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

                    // Get task information from state manager
                    if let Some(task) = self.state_manager.get_task(&task_id) {
                        // Create error result to return to model
                        let error_result = ToolExecutionResult {
                            tool_id: task.tool_call.tool_id.clone(),
                            tool_name: task.tool_call.tool_name.clone(),
                            result: ModelToolResult {
                                tool_id: task.tool_call.tool_id.clone(),
                                tool_name: task.tool_call.tool_name.clone(),
                                result: serde_json::json!({
                                    "error": e.to_string(),
                                    "message": format!("Tool execution failed: {}", e)
                                }),
                                result_for_assistant: Some(format!("Tool execution failed: {}", e)),
                                is_error: true,
                                duration_ms: None,
                            },
                            execution_time_ms: 0,
                        };
                        results.push(error_result);
                    }
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

        debug!(
            "Tool task details: tool_name={}, tool_id={}",
            tool_name, tool_id
        );

        if tool_name.is_empty() || tool_is_error {
            let error_msg = format!(
                "Missing tool name or tool arguments are invalid. \
                This may be caused by network errors (packet loss, connection issues) or model output anomalies. \
                Please regenerate the tool call with valid tool name and arguments."
            );
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

        // Create cancellation token
        let cancellation_token = CancellationToken::new();
        self.cancellation_tokens
            .insert(tool_id.clone(), cancellation_token.clone());

        debug!("Executing tool: tool_name={}", tool_name);

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

            let confirmation_result = match task.options.confirmation_timeout_secs {
                Some(timeout_secs) => {
                    debug!(
                        "Waiting for user confirmation with timeout: timeout_secs={}, tool_name={}",
                        timeout_secs, tool_name
                    );
                    // There is a timeout limit
                    match timeout(Duration::from_secs(timeout_secs), rx).await {
                        Ok(result) => Some(result),
                        Err(_) => None,
                    }
                }
                None => {
                    debug!(
                        "Waiting for user confirmation without timeout: tool_name={}",
                        tool_name
                    );
                    Some(rx.await)
                }
            };

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

        let result = self
            .execute_with_retry(&task, cancellation_token.clone(), tool)
            .await;

        self.cancellation_tokens.remove(&tool_id);

        match result {
            Ok(tool_result) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;

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
                    "Tool completed: tool_name={}, duration_ms={}",
                    tool_name, duration_ms
                );

                Ok(ToolExecutionResult {
                    tool_id,
                    tool_name,
                    result: tool_result,
                    execution_time_ms: duration_ms,
                })
            }
            Err(e) => {
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

                error!("Tool failed: tool_name={}, error={}", tool_name, error_msg);

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
                    if attempts >= max_attempts {
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

        // Build tool context (pass all resource IDs)
        let tool_context = ToolUseContext {
            tool_call_id: Some(task.tool_call.tool_id.clone()),
            message_id: None,
            agent_type: Some(task.context.agent_type.clone()),
            session_id: Some(task.context.session_id.clone()),
            dialog_turn_id: Some(task.context.dialog_turn_id.clone()),
            safe_mode: None,
            abort_controller: None,
            read_file_timestamps: Default::default(),
            options: Some(ToolOptions {
                commands: vec![],
                tools: vec![],
                verbose: None,
                slow_and_capable_model: None,
                safe_mode: None,
                fork_number: None,
                message_log_name: None,
                max_thinking_tokens: None,
                is_koding_request: None,
                koding_context: None,
                is_custom_command: None,
                custom_data: Some({
                    let mut map = HashMap::new();

                    if let Some(snapshot_id) = task
                        .context
                        .context_vars
                        .get("snapshot_session_id")
                        .or_else(|| task.context.context_vars.get("sandbox_session_id"))
                    {
                        map.insert(
                            "snapshot_session_id".to_string(),
                            serde_json::json!(snapshot_id),
                        );
                    }
                    if let Some(turn_index) = task.context.context_vars.get("turn_index") {
                        if let Ok(n) = turn_index.parse::<u64>() {
                            map.insert("turn_index".to_string(), serde_json::json!(n));
                        }
                    }

                    map
                }),
            }),
            response_state: None,
            image_context_provider: self.image_context_provider.clone(),
            subagent_parent_info: task.context.subagent_parent_info.clone(),
            cancellation_token: Some(cancellation_token),
        };

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
