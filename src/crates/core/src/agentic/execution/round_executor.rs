//! Round Executor
//!
//! Executes a single model round: calls AI, processes streaming responses, executes tools

use super::stream_processor::{StreamProcessOptions, StreamProcessor, StreamResult};
use super::types::{FinishReason, RoundContext, RoundResult};
use crate::agentic::core::{Message, ToolCall};
use crate::agentic::events::{AgenticEvent, EventPriority, EventQueue, ToolEventData};
use crate::agentic::tools::computer_use_host::ComputerUseHostRef;
use crate::agentic::tools::framework::ToolUseContext;
use crate::agentic::tools::implementations::file_write_tool::{
    FileWriteTool, WRITE_TOOL_MODE_CONTEXT_KEY,
};
use crate::agentic::tools::pipeline::{ToolExecutionContext, ToolExecutionOptions, ToolPipeline};
use crate::agentic::tools::registry::get_global_tool_registry;
use crate::agentic::tools::tool_context_runtime;
use crate::agentic::tools::tool_result_storage;
use crate::agentic::MessageContent;
use crate::infrastructure::ai::AIClient;
use crate::service::config::types::WriteToolMode;
use crate::service::config::GlobalConfigManager;
use crate::util::elapsed_ms_u64;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::Message as AIMessage;
use crate::util::types::ToolDefinition;
use bitfun_ai_adapters::types::ReasoningMode;
use dashmap::DashMap;
use log::{debug, error, info, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// Round executor
pub struct RoundExecutor {
    stream_processor: Arc<StreamProcessor>,
    tool_pipeline: Option<Arc<ToolPipeline>>,
    event_queue: Arc<EventQueue>,
    /// Cancellation tokens: use dialog_turn_id as key
    cancellation_tokens: Arc<DashMap<String, CancellationToken>>,
}

impl RoundExecutor {
    const MAX_STREAM_ATTEMPTS: usize = 10;
    const RETRY_BASE_DELAY_MS: u64 = 500;
    const WRITE_CONTENT_STREAM_IDLE_TIMEOUT_SECS: u64 = 45;

    fn has_user_visible_assistant_text(text: &str) -> bool {
        !text.trim().is_empty()
    }

    fn write_tool_mode(context: &RoundContext) -> WriteToolMode {
        WriteToolMode::from_context_var(
            context
                .context_vars
                .get(WRITE_TOOL_MODE_CONTEXT_KEY)
                .map(String::as_str),
        )
    }

    pub fn new(
        stream_processor: Arc<StreamProcessor>,
        event_queue: Arc<EventQueue>,
        tool_pipeline: Arc<ToolPipeline>,
    ) -> Self {
        Self {
            stream_processor,
            tool_pipeline: Some(tool_pipeline),
            event_queue,
            cancellation_tokens: Arc::new(DashMap::new()),
        }
    }

    pub fn computer_use_host(&self) -> Option<ComputerUseHostRef> {
        self.tool_pipeline
            .as_ref()
            .and_then(|p| p.computer_use_host())
    }

    /// Execute a single model round
    pub async fn execute_round(
        &self,
        ai_client: Arc<AIClient>,
        context: RoundContext,
        ai_messages: Vec<AIMessage>,
        tool_definitions: Option<Vec<ToolDefinition>>,
        context_window: Option<usize>,
    ) -> BitFunResult<RoundResult> {
        let round_started_at = Instant::now();
        let subagent_parent_info = context.subagent_parent_info.clone();
        let is_subagent = subagent_parent_info.is_some();

        let round_id = uuid::Uuid::new_v4().to_string();

        // Create or reuse cancellation token
        let cancel_token = if let Some(existing_token) = self
            .cancellation_tokens
            .get(&context.dialog_turn_id.clone())
        {
            existing_token.clone()
        } else {
            // Create new token
            let new_token = CancellationToken::new();
            self.cancellation_tokens
                .insert(context.dialog_turn_id.clone(), new_token.clone());
            new_token
        };

        // Emit model round started event
        self.emit_event(
            AgenticEvent::ModelRoundStarted {
                session_id: context.session_id.clone(),
                turn_id: context.dialog_turn_id.clone(),
                round_id: round_id.clone(),
                round_index: context.round_number,
                model_id: Some(context.model_name.clone()),
            },
            EventPriority::High,
        )
        .await;

        let max_attempts = Self::MAX_STREAM_ATTEMPTS;
        let mut attempt_index = 0usize;
        let (stream_result, send_to_stream_ms, stream_processing_ms) = loop {
            // Check cancellation before opening a model stream. This catches
            // early cancellation registered before the first round starts.
            if cancel_token.is_cancelled() {
                debug!(
                    "Cancel token detected before AI request, stopping execution: session_id={}",
                    context.session_id
                );
                return Err(BitFunError::Cancelled("Execution cancelled".to_string()));
            }

            let request_started_at = Instant::now();
            debug!(
                "Sending request: model={}, messages={}, tools={}, attempt={}/{}",
                context.model_name,
                ai_messages.len(),
                tool_definitions.as_ref().map(|t| t.len()).unwrap_or(0),
                attempt_index + 1,
                max_attempts
            );

            // Use dynamically obtained client for call
            let (stream_response, send_to_stream_ms) = match ai_client
                .send_message_stream(ai_messages.clone(), tool_definitions.clone())
                .await
            {
                Ok(response) => {
                    let send_to_stream_ms = elapsed_ms_u64(request_started_at);
                    debug!(
                        "AI stream opened: session_id={}, round_id={}, attempt={}/{}, send_to_stream_ms={}",
                        context.session_id,
                        round_id,
                        attempt_index + 1,
                        max_attempts,
                        send_to_stream_ms
                    );
                    (response, send_to_stream_ms)
                }
                Err(e) => {
                    error!("AI request failed: {}", e);
                    let err_msg = e.to_string();
                    if Self::is_transient_network_error(&err_msg)
                        && attempt_index < max_attempts - 1
                    {
                        let delay_ms = Self::retry_delay_ms(attempt_index);
                        warn!(
                            "Retrying AI request after connection failure: session_id={}, round_id={}, attempt={}/{}, delay_ms={}, error={}",
                            context.session_id,
                            round_id,
                            attempt_index + 1,
                            max_attempts,
                            delay_ms,
                            err_msg
                        );
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        attempt_index += 1;
                        continue;
                    }
                    if Self::is_transient_network_error(&err_msg) {
                        return Err(BitFunError::AIClient(format!(
                            "Stream retry budget exhausted after {} attempts: {}",
                            max_attempts, err_msg
                        )));
                    }
                    return Err(BitFunError::AIClient(err_msg));
                }
            };

            // Destructure StreamResponse: get stream and raw SSE data receiver
            let ai_stream = stream_response.stream;
            let raw_sse_rx = stream_response.raw_sse_rx;

            // Check cancellation token before calling stream processing.
            if cancel_token.is_cancelled() {
                debug!(
                    "Cancel token detected after AI stream opened, stopping execution: session_id={}",
                    context.session_id
                );
                return Err(BitFunError::Cancelled("Execution cancelled".to_string()));
            }

            debug!(
                "Starting AI stream processing: session={}, round={}, thread={:?}, attempt={}/{}",
                context.session_id,
                round_id,
                std::thread::current().id(),
                attempt_index + 1,
                max_attempts
            );

            let stream_started_at = Instant::now();
            match self
                .stream_processor
                .process_stream_with_options(
                    ai_stream,
                    StreamProcessor::derive_watchdog_timeout(ai_client.stream_idle_timeout()),
                    raw_sse_rx, // Pass raw SSE data receiver (for error diagnosis)
                    context.session_id.clone(),
                    context.dialog_turn_id.clone(),
                    round_id.clone(),
                    &cancel_token,
                    StreamProcessOptions {
                        recover_partial_on_cancel: context.recover_partial_on_cancel,
                        strip_write_inline_content: matches!(
                            Self::write_tool_mode(&context),
                            WriteToolMode::PlaintextFollowup
                        ),
                    },
                )
                .await
            {
                Ok(result) => {
                    let stream_processing_ms = elapsed_ms_u64(stream_started_at);
                    if Self::has_interrupted_invalid_tool_calls(&result) {
                        let err_msg = result.partial_recovery_reason.clone().unwrap_or_else(|| {
                            "Interrupted while streaming tool arguments".to_string()
                        });

                        if !Self::has_user_visible_assistant_text(&result.full_text)
                            && attempt_index < max_attempts - 1
                            && Self::is_transient_network_error(&err_msg)
                        {
                            let delay_ms = Self::retry_delay_ms(attempt_index);
                            warn!(
                                "Retrying stream because tool arguments were interrupted before valid JSON completed: session_id={}, round_id={}, attempt={}/{}, delay_ms={}, invalid_tool_calls={}, error={}",
                                context.session_id,
                                round_id,
                                attempt_index + 1,
                                max_attempts,
                                delay_ms,
                                result
                                    .tool_calls
                                    .iter()
                                    .filter(|tool_call| !tool_call.is_valid())
                                    .count(),
                                err_msg
                            );
                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                            attempt_index += 1;
                            continue;
                        }

                        if Self::has_user_visible_assistant_text(&result.full_text) {
                            warn!(
                                "Dropping invalid partial tool calls from interrupted stream; preserving already-streamed assistant text: session_id={}, round_id={}, invalid_tool_calls={}, error={}",
                                context.session_id,
                                round_id,
                                result
                                    .tool_calls
                                    .iter()
                                    .filter(|tool_call| !tool_call.is_valid())
                                    .count(),
                                err_msg
                            );
                            self.emit_failed_partial_tool_calls(
                                &context,
                                &round_id,
                                &result.tool_calls,
                                &err_msg,
                            )
                            .await;
                            let mut recovered = result;
                            recovered
                                .tool_calls
                                .retain(|tool_call| tool_call.is_valid());
                            break (recovered, send_to_stream_ms, stream_processing_ms);
                        }

                        self.emit_failed_partial_tool_calls(
                            &context,
                            &round_id,
                            &result.tool_calls,
                            &err_msg,
                        )
                        .await;
                        return Err(BitFunError::AIClient(format!(
                            "Stream retry budget exhausted after {} attempts: {}",
                            max_attempts, err_msg
                        )));
                    }

                    let no_effective_output = !result.has_effective_output;
                    let is_partial_recovery = result.partial_recovery_reason.is_some();
                    let partial_recovery_reason =
                        result.partial_recovery_reason.as_deref().unwrap_or("");

                    if is_partial_recovery
                        && !Self::has_user_visible_assistant_text(&result.full_text)
                        && !result.tool_calls.is_empty()
                        && Self::is_transient_network_error(partial_recovery_reason)
                        && attempt_index < max_attempts - 1
                    {
                        let delay_ms = Self::retry_delay_ms(attempt_index);
                        warn!(
                            "Retrying stream because tool calls arrived on an interrupted network stream without assistant text: session_id={}, round_id={}, attempt={}/{}, delay_ms={}, tool_calls={}, reason={}",
                            context.session_id,
                            round_id,
                            attempt_index + 1,
                            max_attempts,
                            delay_ms,
                            result.tool_calls.len(),
                            partial_recovery_reason
                        );
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        attempt_index += 1;
                        continue;
                    }

                    if Self::is_invalid_tool_only_without_text(&result) {
                        if attempt_index < max_attempts - 1 {
                            let delay_ms = Self::retry_delay_ms(attempt_index);
                            warn!(
                                "Retrying stream because provider returned only invalid tool arguments: session_id={}, round_id={}, attempt={}/{}, delay_ms={}, tool_calls={}",
                                context.session_id,
                                round_id,
                                attempt_index + 1,
                                max_attempts,
                                delay_ms,
                                result.tool_calls.len()
                            );
                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                            attempt_index += 1;
                            continue;
                        }

                        let err_msg = "Provider returned only invalid tool arguments";
                        self.emit_failed_partial_tool_calls(
                            &context,
                            &round_id,
                            &result.tool_calls,
                            err_msg,
                        )
                        .await;
                        return Err(BitFunError::AIClient(format!(
                            "Stream retry budget exhausted after {} attempts: {}",
                            max_attempts, err_msg
                        )));
                    }

                    if no_effective_output && attempt_index < max_attempts - 1 {
                        let delay_ms = Self::retry_delay_ms(attempt_index);
                        warn!(
                            "Retrying stream because no effective output was received: session_id={}, round_id={}, attempt={}/{}, delay_ms={}",
                            context.session_id,
                            round_id,
                            attempt_index + 1,
                            max_attempts,
                            delay_ms
                        );
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        attempt_index += 1;
                        continue;
                    }

                    if is_partial_recovery {
                        warn!(
                            "Accepting stream partial recovery without retry: session_id={}, round_id={}, attempt={}/{}, reason={}",
                            context.session_id,
                            round_id,
                            attempt_index + 1,
                            max_attempts,
                            result
                                .partial_recovery_reason
                                .as_deref()
                                .unwrap_or("unknown")
                        );
                    }

                    break (result, send_to_stream_ms, stream_processing_ms);
                }
                Err(stream_err) => {
                    let err_msg = stream_err.error.to_string();
                    let can_retry = !stream_err.has_effective_output
                        && attempt_index < max_attempts - 1
                        && Self::is_transient_network_error(&err_msg);
                    if can_retry {
                        let delay_ms = Self::retry_delay_ms(attempt_index);
                        warn!(
                            "Retrying stream after transient error with no effective output: session_id={}, round_id={}, attempt={}/{}, delay_ms={}, error={}",
                            context.session_id,
                            round_id,
                            attempt_index + 1,
                            max_attempts,
                            delay_ms,
                            err_msg
                        );
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        attempt_index += 1;
                        continue;
                    }
                    if Self::is_transient_network_error(&err_msg) {
                        return Err(BitFunError::AIClient(format!(
                            "Stream retry budget exhausted after {} attempts: {}",
                            max_attempts, err_msg
                        )));
                    }
                    return Err(stream_err.error);
                }
            }
        };

        // Model returned successfully (output to AI log file)
        if let Some(ref reason) = stream_result.partial_recovery_reason {
            warn!(
                "Stream recovered with partial output: session_id={}, round_id={}, reason={}, text_len={}, tool_calls={}",
                context.session_id,
                round_id,
                reason,
                stream_result.full_text.len(),
                stream_result.tool_calls.len()
            );
        }

        let tool_names: Vec<&str> = stream_result
            .tool_calls
            .iter()
            .map(|tc| tc.tool_name.as_str())
            .collect();
        debug!(
            target: "ai::model_response",
            "Model response received: text_length={}, tool_calls={}, token_usage={:?}, send_to_stream_ms={}, stream_processing_ms={}, first_chunk_ms={:?}, first_visible_output_ms={:?}",
            stream_result.full_text.len(),
            if tool_names.is_empty() { "none".to_string() } else { tool_names.join(", ") },
            stream_result.usage.as_ref().map(|u| format!("input={}, output={}, total={}", u.prompt_token_count, u.candidates_token_count, u.total_token_count)).unwrap_or_else(|| "none".to_string()),
            send_to_stream_ms,
            stream_processing_ms,
            stream_result.first_chunk_ms,
            stream_result.first_visible_output_ms
        );

        // Check cancellation token again after stream processing completes
        if cancel_token.is_cancelled() {
            debug!(
                "Cancel token detected after stream processing, stopping execution: session_id={}",
                context.session_id
            );
            return Err(BitFunError::Cancelled("Execution cancelled".to_string()));
        }

        // If stream response contains usage info, update token statistics
        if let Some(ref usage) = stream_result.usage {
            debug!(
                "Updating token stats from model response: input={}, output={}, total={}, is_subagent={}",
                usage.prompt_token_count,
                usage.candidates_token_count,
                usage.total_token_count,
                is_subagent
            );

            self.emit_event(
                AgenticEvent::TokenUsageUpdated {
                    session_id: context.session_id.clone(),
                    turn_id: context.dialog_turn_id.clone(),
                    model_id: context.model_name.clone(),
                    input_tokens: usage.prompt_token_count as usize,
                    output_tokens: Some(usage.candidates_token_count as usize),
                    total_tokens: usage.total_token_count as usize,
                    max_context_tokens: context_window,
                    is_subagent,
                    cached_tokens: usage.cached_content_token_count.map(|v| v as usize),
                    token_details: token_details_from_usage(usage),
                },
                EventPriority::Normal,
            )
            .await;
        }

        // Emit model round completed event
        debug!(
            "Preparing to send ModelRoundCompleted event: round={}, has_tools={}",
            round_id,
            !stream_result.tool_calls.is_empty()
        );

        self.emit_event(
            AgenticEvent::ModelRoundCompleted {
                session_id: context.session_id.clone(),
                turn_id: context.dialog_turn_id.clone(),
                round_id: round_id.clone(),
                has_tool_calls: !stream_result.tool_calls.is_empty(),
                duration_ms: Some(elapsed_ms_u64(round_started_at)),
                provider_id: None,
                model_id: Some(context.model_name.clone()),
                model_alias: Some(context.model_name.clone()),
                first_chunk_ms: stream_result.first_chunk_ms,
                first_visible_output_ms: stream_result.first_visible_output_ms,
                stream_duration_ms: Some(stream_processing_ms),
                attempt_count: Some((attempt_index + 1) as u32),
                failure_category: None,
                token_details: stream_result
                    .usage
                    .as_ref()
                    .and_then(token_details_from_usage),
            },
            EventPriority::High,
        )
        .await;

        debug!("ModelRoundCompleted event sent");

        // If no tool calls, this round ends
        if stream_result.tool_calls.is_empty() {
            debug!("No tool calls, round completed: round={}", round_id);

            // Create assistant message (includes thinking content, supports interleaved thinking mode)
            let reasoning = if stream_result.full_thinking.is_empty() {
                if stream_result.reasoning_content_present {
                    Some(String::new())
                } else {
                    None
                }
            } else {
                Some(stream_result.full_thinking.clone())
            };
            let assistant_message = Message::assistant_with_reasoning(
                reasoning,
                stream_result.full_text.clone(),
                vec![],
            )
            .with_turn_id(context.dialog_turn_id.clone())
            .with_round_id(round_id.clone())
            .with_thinking_signature(stream_result.thinking_signature.clone());

            debug!("Returning RoundResult: has_more_rounds=false");
            debug!(
                "Model round timing summary: session_id={}, turn_id={}, round_id={}, tool_calls=0, send_to_stream_ms={}, stream_processing_ms={}, first_chunk_ms={:?}, first_visible_output_ms={:?}, tool_phase_ms=0, round_total_ms={}, has_more_rounds=false",
                context.session_id,
                context.dialog_turn_id,
                round_id,
                send_to_stream_ms,
                stream_processing_ms,
                stream_result.first_chunk_ms,
                stream_result.first_visible_output_ms,
                elapsed_ms_u64(round_started_at)
            );

            // Note: Do not cleanup cancellation token here, as this is only the end of a single model round
            // Cancellation token will be cleaned up by ExecutionEngine when the entire dialog turn ends

            return Ok(RoundResult {
                assistant_message,
                tool_calls: vec![],
                tool_result_messages: vec![],
                has_more_rounds: false,
                finish_reason: FinishReason::Complete,
                usage: stream_result.usage.clone(),
                provider_metadata: stream_result.provider_metadata.clone(),
                partial_recovery_reason: stream_result.partial_recovery_reason.clone(),
                had_assistant_text: Self::has_user_visible_assistant_text(&stream_result.full_text),
                had_thinking_content: !stream_result.full_thinking.is_empty(),
            });
        }

        // Check cancellation token before executing tools
        if cancel_token.is_cancelled() {
            debug!(
                "Cancel token detected before tool execution, stopping execution: session_id={}",
                context.session_id
            );
            return Err(BitFunError::Cancelled("Execution cancelled".to_string()));
        }

        // ---- Write tool content generation ----
        // For Write tool calls without a "content" field, spawn a separate AI
        // request with the full session history to generate the file content as
        // plain text wrapped in <bitfun_contents> tags. This avoids having the
        // model emit large file contents inside JSON tool-call arguments, which
        // is a major source of JSON parse failures.
        let mut tool_calls = stream_result.tool_calls.clone();
        if matches!(
            Self::write_tool_mode(&context),
            WriteToolMode::PlaintextFollowup
        ) {
            FileWriteTool::strip_plaintext_followup_inline_content_from_tool_calls(&mut tool_calls);
        }
        let tool_calls = if matches!(
            Self::write_tool_mode(&context),
            WriteToolMode::PlaintextFollowup
        ) {
            self.generate_write_tool_contents(
                ai_client.clone(),
                &context,
                &round_id,
                &ai_messages,
                tool_calls,
                &cancel_token,
            )
            .await?
        } else {
            tool_calls
        };

        // Execute tool calls
        debug!(
            "Preparing to execute tool calls: count={}",
            tool_calls.len()
        );

        let tool_phase_started_at = Instant::now();
        let tool_results = if let Some(tool_pipeline) = &self.tool_pipeline {
            // Create tool execution context
            let tool_context = ToolExecutionContext {
                session_id: context.session_id.clone(),
                dialog_turn_id: context.dialog_turn_id.clone(),
                round_id: round_id.clone(),
                agent_type: context.agent_type.clone(),
                workspace: context.workspace.clone(),
                context_vars: context.context_vars.clone(),
                subagent_parent_info,
                collapsed_tools: context.collapsed_tools.clone(),
                unlocked_collapsed_tools: context.unlocked_collapsed_tools.clone(),
                allowed_tools: context.available_tools.clone(),
                runtime_tool_restrictions: context.runtime_tool_restrictions.clone(),
                steering_interrupt: context.steering_interrupt.clone(),
                workspace_services: context.workspace_services.clone(),
            };

            // Read tool execution related configuration from global config
            let (needs_confirmation, tool_execution_timeout, tool_confirmation_timeout) = {
                let config_service = GlobalConfigManager::get_service().await.ok();

                // Timeout and skip confirmation settings
                let (exec_timeout, confirm_timeout, skip_confirmation) =
                    if let Some(ref service) = config_service {
                        let ai_config: crate::service::config::types::AIConfig =
                            service.get_config(Some("ai")).await.unwrap_or_default();

                        if ai_config.skip_tool_confirmation {
                            debug!("Global config skips tool confirmation");
                        }

                        (
                            ai_config.tool_execution_timeout_secs,
                            ai_config.tool_confirmation_timeout_secs,
                            ai_config.skip_tool_confirmation,
                        )
                    } else {
                        (None, None, false) // Default: no timeout, requires confirmation
                    };

                let skip_from_context = context
                    .context_vars
                    .get("skip_tool_confirmation")
                    .map(|v| v == "true")
                    .unwrap_or(false);

                let needs_confirm = if skip_confirmation || skip_from_context {
                    false
                } else {
                    // Otherwise judge based on tool's needs_permissions()
                    let registry = get_global_tool_registry();
                    let tool_registry = registry.read().await;
                    let mut requires_permission = false;

                    for tool_call in &stream_result.tool_calls {
                        if let Some(tool) = tool_registry.get_tool(&tool_call.tool_name) {
                            if tool.needs_permissions(Some(&tool_call.arguments)) {
                                requires_permission = true;
                                break;
                            }
                        }
                    }

                    requires_permission
                };

                (needs_confirm, exec_timeout, confirm_timeout)
            };

            // Create tool execution options (use configured timeout values)
            let tool_options = ToolExecutionOptions {
                confirm_before_run: needs_confirmation,
                timeout_secs: tool_execution_timeout,
                confirmation_timeout_secs: tool_confirmation_timeout,
                ..ToolExecutionOptions::default()
            };

            let storage_context =
                tool_context_runtime::build_tool_use_context_for_execution_context(
                    &tool_context,
                    Some(format!("round-budget-{}", round_id)),
                    self.computer_use_host(),
                    CancellationToken::new(),
                );

            // Execute tools — convert pipeline-level Err into per-tool error results
            // so the model always receives a tool_result for every tool_call.
            let execution_results = match tool_pipeline
                .execute_tools(tool_calls.clone(), tool_context, tool_options)
                .await
            {
                Ok(results) => results,
                Err(e) => {
                    error!(
                        "Tool pipeline execution failed, generating error results for all {} tool calls: {}",
                        tool_calls.len(),
                        e
                    );
                    tool_calls
                        .iter()
                        .map(|tc| crate::agentic::tools::pipeline::ToolExecutionResult {
                            tool_id: tc.tool_id.clone(),
                            tool_name: tc.tool_name.clone(),
                            result: crate::agentic::core::ToolResult {
                                tool_id: tc.tool_id.clone(),
                                tool_name: tc.tool_name.clone(),
                                result: serde_json::json!({
                                    "error": e.to_string(),
                                    "message": format!("Tool pipeline execution failed: {}", e)
                                }),
                                result_for_assistant: Some(format!("Tool execution failed: {}", e)),
                                is_error: true,
                                duration_ms: None,
                                image_attachments: None,
                            },
                            execution_time_ms: 0,
                        })
                        .collect()
                }
            };

            // Convert to ToolResult, then enforce the aggregate budget for this model round.
            let tool_results = execution_results.into_iter().map(|r| r.result).collect();
            tool_result_storage::apply_round_tool_result_budget(tool_results, &storage_context)
                .await
        } else {
            vec![]
        };
        let tool_phase_ms = elapsed_ms_u64(tool_phase_started_at);

        // Create assistant message (includes tool calls and thinking content, supports interleaved thinking mode)
        let reasoning = if stream_result.full_thinking.is_empty() {
            if stream_result.reasoning_content_present {
                Some(String::new())
            } else {
                None
            }
        } else {
            Some(stream_result.full_thinking.clone())
        };
        let assistant_message = Message::assistant_with_reasoning(
            reasoning,
            stream_result.full_text.clone(),
            tool_calls.clone(),
        )
        .with_turn_id(context.dialog_turn_id.clone())
        .with_round_id(round_id.clone())
        .with_thinking_signature(stream_result.thinking_signature.clone());

        debug!(
            "Tool execution completed, creating message: assistant_msg_len={}, tool_results={}",
            match &assistant_message.content {
                MessageContent::Text(t) => t.len(),
                MessageContent::Mixed { text, .. } => text.len(),
                _ => 0,
            },
            tool_results.len()
        );

        // Create tool result messages (also need to set turn_id and round_id)
        let dialog_turn_id = context.dialog_turn_id.clone();
        let round_id_clone = round_id.clone();
        let tool_result_messages: Vec<Message> = tool_results
            .iter()
            .map(|result| {
                Message::tool_result(result.clone())
                    .with_turn_id(dialog_turn_id.clone())
                    .with_round_id(round_id_clone.clone())
            })
            .collect();

        let has_more_rounds = !tool_result_messages.is_empty();

        debug!(
            "Returning RoundResult: has_more_rounds={}, tool_result_messages={}",
            has_more_rounds,
            tool_result_messages.len()
        );
        debug!(
            "Model round timing summary: session_id={}, turn_id={}, round_id={}, tool_calls={}, tool_results={}, send_to_stream_ms={}, stream_processing_ms={}, first_chunk_ms={:?}, first_visible_output_ms={:?}, tool_phase_ms={}, round_total_ms={}, has_more_rounds={}",
            context.session_id,
            context.dialog_turn_id,
            round_id,
            stream_result.tool_calls.len(),
            tool_result_messages.len(),
            send_to_stream_ms,
            stream_processing_ms,
            stream_result.first_chunk_ms,
            stream_result.first_visible_output_ms,
            tool_phase_ms,
            elapsed_ms_u64(round_started_at),
            has_more_rounds
        );

        // Note: Do not cleanup cancellation token here, as there may be subsequent model rounds
        // Cancellation token will be cleaned up by ExecutionEngine when the entire dialog turn ends

        Ok(RoundResult {
            assistant_message,
            tool_calls: tool_calls.clone(),
            tool_result_messages,
            has_more_rounds,
            finish_reason: if has_more_rounds {
                FinishReason::ToolCalls
            } else {
                FinishReason::Complete
            },
            usage: stream_result.usage.clone(),
            provider_metadata: stream_result.provider_metadata.clone(),
            partial_recovery_reason: stream_result.partial_recovery_reason.clone(),
            had_assistant_text: Self::has_user_visible_assistant_text(&stream_result.full_text),
            had_thinking_content: !stream_result.full_thinking.is_empty(),
        })
    }

    /// Check if dialog turn is still active (used to detect cancellation)
    pub fn has_active_dialog_turn(&self, dialog_turn_id: &str) -> bool {
        self.cancellation_tokens.contains_key(dialog_turn_id)
    }

    /// Check if dialog turn cancellation has been requested.
    pub fn is_dialog_turn_cancelled(&self, dialog_turn_id: &str) -> bool {
        self.cancellation_tokens
            .get(dialog_turn_id)
            .is_some_and(|token| token.is_cancelled())
    }

    /// Register cancellation token (for external control, e.g., execute_subagent)
    pub fn register_cancel_token(&self, dialog_turn_id: &str, token: CancellationToken) {
        self.cancellation_tokens
            .insert(dialog_turn_id.to_string(), token);
    }

    /// Return a clone of the cancellation token registered for a dialog turn.
    pub fn cancel_token_for_dialog_turn(&self, dialog_turn_id: &str) -> Option<CancellationToken> {
        self.cancellation_tokens
            .get(dialog_turn_id)
            .map(|entry| entry.clone())
    }

    /// Cancel dialog turn (using dialog_turn_id)
    pub async fn cancel_dialog_turn(&self, dialog_turn_id: &str) -> BitFunResult<()> {
        debug!("Cancelling dialog turn: dialog_turn_id={}", dialog_turn_id);

        if let Some(token) = self
            .cancellation_tokens
            .get(dialog_turn_id)
            .map(|entry| entry.clone())
        {
            debug!("Found cancel token, triggering cancellation");
            token.cancel();
            debug!("Cancel token triggered");
        } else {
            debug!("Cancel token not found (dialog may have completed or not started)");
        }

        Ok(())
    }

    /// Cleanup dialog turn token (called on normal completion)
    pub async fn cleanup_dialog_turn(&self, dialog_turn_id: &str) {
        if self.cancellation_tokens.remove(dialog_turn_id).is_some() {
            debug!("Cleaned up cancel token: dialog_turn_id={}", dialog_turn_id);
        }
    }

    /// Generate file content for Write tool calls that lack a `content` field.
    ///
    /// When a Write tool call arrives without `content`, this method spawns a
    /// separate AI request with the full session history and a directive to
    /// output the file content as plain text inside `<bitfun_contents>` tags.
    /// The extracted content is then injected into the tool call arguments so
    /// the downstream Write tool execution proceeds as normal.
    async fn generate_write_tool_contents(
        &self,
        ai_client: Arc<AIClient>,
        context: &RoundContext,
        round_id: &str,
        ai_messages: &[AIMessage],
        mut tool_calls: Vec<ToolCall>,
        cancel_token: &CancellationToken,
    ) -> BitFunResult<Vec<ToolCall>> {
        // Find indices of Write tool calls that need content generation
        let write_indices: Vec<usize> = tool_calls
            .iter()
            .enumerate()
            .filter(|(_, tc)| {
                tc.tool_name == "Write"
                    && tc.arguments.get("content").is_none()
                    && tc
                        .arguments
                        .get("file_path")
                        .and_then(|v| v.as_str())
                        .is_some()
            })
            .map(|(i, _)| i)
            .collect();

        if write_indices.is_empty() {
            return Ok(tool_calls);
        }

        info!(
            "Generating content for {} Write tool call(s) via separate AI request",
            write_indices.len()
        );

        for idx in &write_indices {
            if cancel_token.is_cancelled() {
                return Err(BitFunError::Cancelled("Execution cancelled".to_string()));
            }

            let tc = &tool_calls[*idx];
            let file_path = tc
                .arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tool_id = tc.tool_id.clone();

            if let Some(error) = Self::write_content_preflight_error(context, &file_path).await {
                debug!(
                    "Skipping Write content generation after preflight failure: file_path={}, error={}",
                    file_path, error
                );
                self.emit_event(
                    AgenticEvent::ToolEvent {
                        session_id: context.session_id.clone(),
                        turn_id: context.dialog_turn_id.clone(),
                        round_id: round_id.to_string(),
                        tool_event: ToolEventData::Failed {
                            tool_id: tool_id.clone(),
                            tool_name: "Write".to_string(),
                            error,
                            duration_ms: None,
                            queue_wait_ms: None,
                            preflight_ms: None,
                            confirmation_wait_ms: None,
                            execution_ms: None,
                        },
                    },
                    EventPriority::High,
                )
                .await;
                continue;
            }

            // Emit Started event so the UI can show the tool card
            self.emit_event(
                AgenticEvent::ToolEvent {
                    session_id: context.session_id.clone(),
                    turn_id: context.dialog_turn_id.clone(),
                    round_id: round_id.to_string(),
                    tool_event: ToolEventData::Started {
                        tool_id: tool_id.clone(),
                        tool_name: "Write".to_string(),
                        params: tc.arguments.clone(),
                        timeout_seconds: None,
                    },
                },
                EventPriority::High,
            )
            .await;

            // Build a content-generation prompt
            let content_prompt = format!(
                "Now output the COMPLETE file content for the file `{file_path}`.\n\
                 CRITICAL RULES — you MUST follow all of them:\n\
                 1. Output the ENTIRE file content — every single line, every character that should end up on disk.\n\
                 2. Do NOT abbreviate, summarize, or insert placeholder comments referring to omitted code, such as: \
                 \"// ... rest of the code\", \"// rest omitted\", \"// implementation follows\", \"// existing code unchanged\", \
                 \"// same as before\", \"# rest omitted\", \"# rest of file\", or any equivalent in any language. \
                 If a section is unchanged, write it out in full anyway.\n\
                 3. Literal `...` is allowed only when it is genuinely part of the file content (e.g. inside a string, \
                 inside XML/JSON/YAML data, inside docs). Never use it as a stand-in for omitted code.\n\
                 4. Wrap the content inside <bitfun_contents> tags exactly as shown below.\n\
                 5. Do NOT output anything outside the <bitfun_contents> tags — no explanations, no commentary, \
                 no thinking blocks, no markdown fences (```), no extra XML wrapper tags.\n\
                 6. The text between the tags must be EXACTLY what gets written to disk — raw file content only.\n\
                 7. Do NOT call any tools in this turn. Do NOT output tool_call XML, JSON tool invocations, \
                 function_call blocks, or agent framework syntax inside or outside the tags. \
                 You are not calling a tool here — you are outputting raw file content only.\n\
                 8. Do NOT repeat, summarize, or narrate prior tool calls (Read, Bash, Edit, etc.). Start writing the actual file body immediately.\n\
                 9. Do NOT output `[called tools:` markers, tool parameter JSON, or `<bitfun_contents>` / `</bitfun_contents>` tags — the opening tag is already provided via prefill. Begin with the first byte of the file content immediately after that opening tag.",
                file_path = file_path
            );

            let content_messages = Self::build_write_content_messages(ai_messages, &content_prompt);
            let write_client = ai_client.with_reasoning_mode(ReasoningMode::Disabled);

            let full_text = self
                .stream_write_tool_content(
                    &write_client,
                    content_messages,
                    &file_path,
                    &tool_id,
                    context,
                    round_id,
                    cancel_token,
                )
                .await?;

            let content = extract_bitfun_contents_with_options(&full_text, true);
            if content.is_empty() {
                warn!(
                    "Write content generation returned empty content for file_path={}",
                    file_path
                );
            }

            // Detect strong "omission marker" phrases that indicate the model
            // wrote a summary instead of the full file content. This is a
            // best-effort warning only — we do not block the write, because
            // Write must remain general enough to produce any kind of file
            // (including ones that legitimately discuss these phrases).
            if let Some(marker) = detect_placeholder_patterns(&content) {
                warn!(
                    "Write content for file_path={} contains an omission marker comment ({:?}); \
                     the generated content may be an outline rather than the full file",
                    file_path, marker
                );
            }

            let final_params = serde_json::json!({
                "file_path": &file_path,
                "content": &content,
            });
            self.emit_event(
                AgenticEvent::ToolEvent {
                    session_id: context.session_id.clone(),
                    turn_id: context.dialog_turn_id.clone(),
                    round_id: round_id.to_string(),
                    tool_event: ToolEventData::ParamsPartial {
                        tool_id: tool_id.clone(),
                        tool_name: "Write".to_string(),
                        params: final_params.to_string(),
                    },
                },
                EventPriority::Normal,
            )
            .await;

            // Inject content into the tool call arguments
            tool_calls[*idx]
                .arguments
                .as_object_mut()
                .expect("Write tool arguments must be a JSON object")
                .insert("content".to_string(), serde_json::Value::String(content));

            debug!(
                "Write content generated: file_path={}, content_len={}",
                file_path,
                tool_calls[*idx]
                    .arguments
                    .get("content")
                    .and_then(|v| v.as_str())
                    .map(|s| s.len())
                    .unwrap_or(0)
            );
        }

        Ok(tool_calls)
    }

    /// Build the message list for Write content generation.
    ///
    /// Reuses the exact conversation prefix that was sent to the model for this
    /// round so tool results, prior assistant tool-call turns, and other
    /// context stay aligned (including provider-side prefix/KV reuse). Only
    /// appends the write-content directive and an assistant prefill.
    fn build_write_content_messages(
        ai_messages: &[AIMessage],
        content_prompt: &str,
    ) -> Vec<AIMessage> {
        let mut content_messages = ai_messages.to_vec();
        content_messages.push(AIMessage::user(content_prompt.to_string()));
        content_messages.push(AIMessage::assistant("<bitfun_contents>".to_string()));
        content_messages
    }

    async fn stream_write_tool_content(
        &self,
        ai_client: &AIClient,
        content_messages: Vec<AIMessage>,
        file_path: &str,
        tool_id: &str,
        context: &RoundContext,
        round_id: &str,
        cancel_token: &CancellationToken,
    ) -> BitFunResult<String> {
        let mut attempt_index = 0usize;
        loop {
            if cancel_token.is_cancelled() {
                return Err(BitFunError::Cancelled("Execution cancelled".to_string()));
            }

            let stream_response = match ai_client
                .send_message_stream_with_extra_body(
                    content_messages.clone(),
                    None,
                    ai_client.write_content_generation_extra_body(),
                )
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    let err_msg = e.to_string();
                    if Self::is_transient_network_error(&err_msg)
                        && attempt_index < Self::MAX_STREAM_ATTEMPTS - 1
                    {
                        let delay_ms = Self::retry_delay_ms(attempt_index);
                        warn!(
                            "Retrying Write content generation after transient error: file_path={}, attempt={}/{}, delay_ms={}, error={}",
                            file_path,
                            attempt_index + 1,
                            Self::MAX_STREAM_ATTEMPTS,
                            delay_ms,
                            err_msg
                        );
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        attempt_index += 1;
                        continue;
                    }
                    error!("Write content generation request failed: {}", err_msg);
                    return Err(BitFunError::AIClient(format!(
                        "Write content generation failed for {}: {}",
                        file_path, err_msg
                    )));
                }
            };

            let mut text = String::new();
            let mut stream = stream_response.stream;
            let watchdog_timeout =
                StreamProcessor::derive_watchdog_timeout(ai_client.stream_idle_timeout())
                    .unwrap_or_else(|| {
                        Duration::from_secs(Self::WRITE_CONTENT_STREAM_IDLE_TIMEOUT_SECS)
                    });
            use futures::StreamExt;
            loop {
                if cancel_token.is_cancelled() {
                    return Err(BitFunError::Cancelled("Execution cancelled".to_string()));
                }

                let chunk = match tokio::time::timeout(watchdog_timeout, stream.next()).await {
                    Ok(Some(chunk)) => chunk,
                    Ok(None) => return Ok(text),
                    Err(_) => {
                        return Err(BitFunError::Timeout(format!(
                            "Write content generation timed out for {} after {} seconds without stream progress",
                            file_path,
                            watchdog_timeout.as_secs()
                        )));
                    }
                };

                match chunk {
                    Ok(resp) => {
                        let chunk_text = resp.text.unwrap_or_default();
                        if chunk_text.is_empty() {
                            continue;
                        }
                        text.push_str(&chunk_text);

                        let preview_content = extract_bitfun_contents_with_options(&text, true);
                        let params = serde_json::json!({
                            "file_path": file_path,
                            "content": &preview_content,
                        });
                        self.emit_event(
                            AgenticEvent::ToolEvent {
                                session_id: context.session_id.clone(),
                                turn_id: context.dialog_turn_id.clone(),
                                round_id: round_id.to_string(),
                                tool_event: ToolEventData::ParamsPartial {
                                    tool_id: tool_id.to_string(),
                                    tool_name: "Write".to_string(),
                                    params: params.to_string(),
                                },
                            },
                            EventPriority::Normal,
                        )
                        .await;
                    }
                    Err(e) => {
                        error!("Error in Write content generation stream: {}", e);
                        break;
                    }
                }
            }

            if !text.trim().is_empty() {
                return Ok(text);
            }

            if attempt_index < Self::MAX_STREAM_ATTEMPTS - 1 {
                let delay_ms = Self::retry_delay_ms(attempt_index);
                warn!(
                    "Retrying Write content generation after empty stream: file_path={}, attempt={}/{}, delay_ms={}",
                    file_path,
                    attempt_index + 1,
                    Self::MAX_STREAM_ATTEMPTS,
                    delay_ms
                );
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                attempt_index += 1;
                continue;
            }

            return Ok(text);
        }
    }

    async fn write_content_preflight_error(
        context: &RoundContext,
        file_path: &str,
    ) -> Option<String> {
        let tool_context = Self::build_write_preflight_context(context);
        FileWriteTool::preflight_write_error(&tool_context, file_path).await
    }

    fn build_write_preflight_context(context: &RoundContext) -> ToolUseContext {
        tool_context_runtime::build_write_preflight_context(
            &context.agent_type,
            &context.session_id,
            &context.dialog_turn_id,
            context.workspace.clone(),
            context.unlocked_collapsed_tools.clone(),
            context.runtime_tool_restrictions.clone(),
            context.workspace_services.clone(),
        )
    }

    /// Emit event
    async fn emit_event(&self, event: AgenticEvent, priority: EventPriority) {
        let _ = self.event_queue.enqueue(event, Some(priority)).await;
    }

    async fn emit_failed_partial_tool_calls(
        &self,
        context: &RoundContext,
        round_id: &str,
        tool_calls: &[ToolCall],
        error: &str,
    ) {
        for tool_call in tool_calls {
            self.emit_event(
                AgenticEvent::ToolEvent {
                    session_id: context.session_id.clone(),
                    turn_id: context.dialog_turn_id.clone(),
                    round_id: round_id.to_string(),
                    tool_event: ToolEventData::Failed {
                        tool_id: tool_call.tool_id.clone(),
                        tool_name: tool_call.tool_name.clone(),
                        error: format!("Tool arguments stream interrupted: {}", error),
                        duration_ms: None,
                        queue_wait_ms: None,
                        preflight_ms: None,
                        confirmation_wait_ms: None,
                        execution_ms: None,
                    },
                },
                EventPriority::High,
            )
            .await;
        }
    }

    fn has_interrupted_invalid_tool_calls(result: &StreamResult) -> bool {
        result.partial_recovery_reason.is_some()
            && !result.tool_calls.is_empty()
            && result
                .tool_calls
                .iter()
                .any(|tool_call| !tool_call.is_valid())
    }

    #[cfg(test)]
    fn is_interrupted_invalid_tool_only(result: &StreamResult) -> bool {
        Self::has_interrupted_invalid_tool_calls(result)
            && result.full_text.is_empty()
            && result
                .tool_calls
                .iter()
                .all(|tool_call| !tool_call.is_valid())
    }

    fn is_invalid_tool_only_without_text(result: &StreamResult) -> bool {
        result.partial_recovery_reason.is_none()
            && !Self::has_user_visible_assistant_text(&result.full_text)
            && !result.tool_calls.is_empty()
            && result
                .tool_calls
                .iter()
                .all(|tool_call| !tool_call.is_valid())
    }

    fn retry_delay_ms(attempt_index: usize) -> u64 {
        Self::RETRY_BASE_DELAY_MS * (1u64 << attempt_index.min(3))
    }

    fn is_transient_network_error(error_message: &str) -> bool {
        let msg = error_message.to_lowercase();

        let non_retryable_keywords = [
            "invalid api key",
            "unauthorized",
            "forbidden",
            "model not found",
            "unsupported model",
            "invalid request",
            "bad request",
            "prompt is too long",
            "content policy",
            "proxy authentication required",
            "provider quota",
            "provider billing",
            "insufficient_quota",
            "insufficient quota",
            "insufficient balance",
            "not_enough_balance",
            "not enough balance",
            "余额不足",
            "无可用资源包",
            "账户已欠费",
            "code=1113",
            "\"code\":\"1113\"",
            "client error 400",
            "client error 401",
            "client error 402",
            "client error 403",
            "client error 404",
            "client error 413",
            "client error 422",
            "sse parsing error",
            "schema error",
            "unknown api format",
        ];

        let transient_keywords = [
            "transport error",
            "error decoding response body",
            "stream closed before response completed",
            "stream processing error",
            "sse stream error",
            "sse error",
            "sse timeout",
            "stream data timeout",
            "timeout",
            "request timeout",
            "deadline exceeded",
            "connection reset",
            "connection closed",
            "broken pipe",
            "unexpected eof",
            "connection refused",
            "socket closed",
            "temporarily unavailable",
            "service unavailable",
            "bad gateway",
            "gateway timeout",
            "overloaded",
            "proxy",
            "tunnel",
            "dns",
            "network",
            "econnreset",
            "econnrefused",
            "etimedout",
            "rate limit",
            "too many requests",
            "408",
            "409",
            "425",
            "429",
            "502",
            "503",
            "504",
        ];

        if non_retryable_keywords.iter().any(|k| msg.contains(k)) {
            return false;
        }

        transient_keywords.iter().any(|k| msg.contains(k))
    }
}

fn token_details_from_usage(
    usage: &crate::util::types::ai::GeminiUsage,
) -> Option<serde_json::Value> {
    let mut details = serde_json::Map::new();
    if let Some(reasoning_tokens) = usage.reasoning_token_count {
        details.insert(
            "reasoningTokenCount".to_string(),
            serde_json::json!(reasoning_tokens),
        );
    }
    if let Some(cached_tokens) = usage.cached_content_token_count {
        details.insert(
            "cachedContentTokenCount".to_string(),
            serde_json::json!(cached_tokens),
        );
    }
    // Cache writes (Anthropic only at the moment). Disjoint from reads.
    if let Some(creation_tokens) = usage.cache_creation_token_count {
        details.insert(
            "cacheCreationTokenCount".to_string(),
            serde_json::json!(creation_tokens),
        );
    }

    (!details.is_empty()).then_some(serde_json::Value::Object(details))
}

/// Extract content from `<bitfun_contents>...</bitfun_contents>` tags.
///
/// When `prefill_open_tag` is true, the assistant prefill already opened the tag
/// and streamed tokens are inner file content even if the opening tag is absent.
#[cfg(test)]
fn extract_bitfun_contents(text: &str) -> String {
    extract_bitfun_contents_with_options(text, false)
}

fn extract_bitfun_contents_with_options(text: &str, prefill_open_tag: bool) -> String {
    const OPEN_TAG: &str = "<bitfun_contents>";
    const CLOSE_TAG: &str = "</bitfun_contents>";

    let raw = if let Some(start) = text.rfind(OPEN_TAG) {
        let content_start = start + OPEN_TAG.len();
        if let Some(end) = text[content_start..].find(CLOSE_TAG) {
            &text[content_start..content_start + end]
        } else {
            &text[content_start..]
        }
    } else if prefill_open_tag {
        if let Some(end) = text.find(CLOSE_TAG) {
            &text[..end]
        } else {
            text
        }
    } else {
        text
    };

    sanitize_write_content(raw.trim())
}

/// Sanitize model-generated file content by stripping common artifacts that
/// some models emit despite being told not to.
fn sanitize_write_content(content: &str) -> String {
    let mut s = content.to_string();

    s = strip_called_tools_artifacts(&s);
    s = strip_bitfun_content_tags(&s);

    // Strip multi-line thinking/reasoning XML blocks (e.g. <think ...>..</think >)
    // These are very common with reasoning models.
    s = strip_thinking_blocks(&s);

    // Strip leading/trailing markdown code fences (```lang ... ```)
    // that some models wrap around file content.
    s = strip_markdown_fences(&s);

    s.trim().to_string()
}

fn strip_bitfun_content_tags(content: &str) -> String {
    content
        .replace("<bitfun_contents>", "")
        .replace("</bitfun_contents>", "")
}

fn strip_called_tools_artifacts(content: &str) -> String {
    let mut result = content.to_string();
    while let Some(start) = result.find("[called tools:") {
        let Some(end) = find_called_tools_block_end(&result[start..]) else {
            break;
        };
        result = format!("{}{}", &result[..start], &result[start + end..]);
    }
    result
}

fn find_called_tools_block_end(block: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in block.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index + ch.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

/// Strip thinking-style XML blocks from content. Handles multi-line blocks
/// like `<think ...>content</think >` and `<reasoning>content</reasoning>`.
/// Also handles non-standard formats like `<think\ncontent\n</think >` where
/// the opening tag may not have a closing `>`.
fn strip_thinking_blocks(content: &str) -> String {
    let thinking_open_tags = ["<think", "<reasoning", "<reflection", "<analysis"];
    let mut result = content.to_string();

    for open_tag_prefix in &thinking_open_tags {
        loop {
            // Find the opening tag
            let Some(open_start) = result.find(open_tag_prefix) else {
                break;
            };

            // Find the end of the opening tag — look for '>' or newline
            let after_open = &result[open_start..];
            let tag_end_offset = after_open
                .find(|c: char| c == '>' || c == '\n')
                .unwrap_or(after_open.len());

            // Extract tag name from <tagname...>
            let tag_inner = &result[open_start + 1..open_start + tag_end_offset];
            let tag_name = tag_inner.split_whitespace().next().unwrap_or("");

            // Skip if tag_name is empty (shouldn't happen but guard)
            if tag_name.is_empty() {
                break;
            }

            // Build the closing tag. Note: some models output `</tagname >` with
            // trailing space or `</tagname\n` with newline. Search broadly.
            let close_tag_prefix = format!("</{}", tag_name);

            // Find the closing tag
            if let Some(close_pos) = result[open_start..].find(&close_tag_prefix) {
                let abs_close_pos = open_start + close_pos;
                // Find the end of the closing tag (next '>' or newline or end)
                let close_end = result[abs_close_pos..]
                    .find(|c: char| c == '>' || c == '\n')
                    .map(|p| abs_close_pos + p + 1)
                    .unwrap_or(result.len());
                result = format!("{}{}", &result[..open_start], &result[close_end..]);
            } else {
                // No closing tag found — strip from open_start to end of opening
                // tag line and continue
                let line_end = after_open
                    .find('\n')
                    .map(|p| open_start + p + 1)
                    .unwrap_or(result.len());
                result = format!("{}{}", &result[..open_start], &result[line_end..]);
            }
        }
    }

    result
}

/// Strip markdown code fences that wrap the entire content.
/// Handles ```lang\n...\n``` patterns at the outermost level.
fn strip_markdown_fences(content: &str) -> String {
    let trimmed = content.trim();
    if !trimmed.starts_with("```") {
        return content.to_string();
    }

    // Find the end of the opening fence line
    let fence_end = trimmed.find('\n').unwrap_or(3);
    // let _lang = &trimmed[3..fence_end].trim(); // language hint, ignored

    // Check if content ends with ```
    let inner = trimmed[fence_end + 1..].trim_end();
    if inner.ends_with("```") {
        return inner[..inner.len() - 3].trim_end().to_string();
    }

    // No closing fence — strip opening fence only
    trimmed[fence_end + 1..].to_string()
}

/// Detect "omission marker" phrases that strongly indicate the model wrote a
/// summary/outline instead of the full file. Returns the matched marker on the
/// first hit, or `None` otherwise.
///
/// Design notes:
/// - Only match phrases that are very unlikely to legitimately appear in real
///   source/data files. Plain `...`, `…`, `TODO:` and `FIXME:` are NOT included
///   because they show up in real code, docs, XML/JSON data, etc., and would
///   trigger false positives on legitimate Write usage (the tool can write any
///   kind of file).
/// - Patterns are matched in a comment-like context (after `//`, `#`, `/*`, `--`,
///   or `<!--`) to further reduce false positives on prose/data that happens to
///   contain similar wording.
/// - A single hit is enough to warn; we do not use a percentage threshold,
///   because even one "// ... rest of the code" comment means the file is wrong.
fn detect_placeholder_patterns(content: &str) -> Option<&'static str> {
    if content.is_empty() {
        return None;
    }

    // Phrases below are normalized to lowercase before comparison.
    // Keep this list conservative — every entry should be something a
    // careful human would essentially never write verbatim in a real file.
    const OMISSION_MARKERS: &[&str] = &[
        "... rest of the code",
        "... rest of code",
        "... rest of the file",
        "... rest of file",
        "... existing code",
        "rest of the code unchanged",
        "rest of the file unchanged",
        "rest omitted for brevity",
        "rest omitted",
        "remainder omitted",
        "implementation follows",
        "implementation continues",
        "implementation unchanged",
        "existing code unchanged",
        "existing implementation unchanged",
        "code omitted for brevity",
        "code omitted",
        "previous code unchanged",
        "same as before",
        "(unchanged)",
        "// snip",
        "/* snip */",
        "<!-- snip -->",
        "<unchanged>",
        "<omitted>",
    ];

    // Comment lead-ins we look for. Empty string means "no comment marker
    // required" — used for the strongest phrases that are unmistakable on
    // their own (e.g. `<!-- snip -->`).
    const COMMENT_LEADS: &[&str] = &["//", "#", "/*", "--", "<!--", ";", "%"];

    for raw_line in content.lines() {
        let line = raw_line.trim().to_lowercase();
        if line.is_empty() {
            continue;
        }

        for marker in OMISSION_MARKERS {
            let marker_lc = marker.to_lowercase();
            if !line.contains(&marker_lc) {
                continue;
            }

            // Markers that already contain a comment-style wrapper are accepted
            // on their own.
            let already_commented =
                marker.starts_with("//") || marker.starts_with("/*") || marker.starts_with("<!--");
            if already_commented {
                return Some(marker);
            }

            // Otherwise require the line to look like a comment, so we don't
            // flag prose/data lines that happen to mention the phrase.
            if COMMENT_LEADS.iter().any(|lead| line.starts_with(lead)) {
                return Some(marker);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{
        extract_bitfun_contents, extract_bitfun_contents_with_options, RoundExecutor,
        StreamProcessor,
    };
    use crate::agentic::events::{EventQueue, EventQueueConfig};
    use crate::agentic::execution::types::RoundContext;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::agentic::WorkspaceBinding;
    use dashmap::DashMap;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    fn test_round_executor() -> RoundExecutor {
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        RoundExecutor {
            stream_processor: Arc::new(StreamProcessor::new(event_queue.clone())),
            tool_pipeline: None,
            event_queue,
            cancellation_tokens: Arc::new(DashMap::new()),
        }
    }

    fn test_round_context(workspace_root: PathBuf) -> RoundContext {
        RoundContext {
            session_id: "session-1".to_string(),
            subagent_parent_info: None,
            dialog_turn_id: "turn-1".to_string(),
            turn_index: 0,
            round_number: 0,
            workspace: Some(WorkspaceBinding::new(None, workspace_root)),
            messages: Vec::new(),
            available_tools: Vec::new(),
            collapsed_tools: Vec::new(),
            unlocked_collapsed_tools: Vec::new(),
            model_name: "test-model".to_string(),
            agent_type: "test-agent".to_string(),
            context_vars: HashMap::new(),
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            steering_interrupt: None,
            cancellation_token: CancellationToken::new(),
            workspace_services: None,
            recover_partial_on_cancel: false,
        }
    }

    #[tokio::test]
    async fn cancel_token_for_dialog_turn_returns_registered_token() {
        let executor = test_round_executor();
        let token = CancellationToken::new();
        executor.register_cancel_token("turn-1", token.clone());

        assert!(executor.cancel_token_for_dialog_turn("turn-1").is_some());
        assert!(executor.cancel_token_for_dialog_turn("missing").is_none());
    }

    #[tokio::test]
    async fn cancel_keeps_token_registered_until_cleanup() {
        let executor = test_round_executor();
        let token = CancellationToken::new();
        executor.register_cancel_token("turn-1", token.clone());

        executor
            .cancel_dialog_turn("turn-1")
            .await
            .expect("cancel should succeed");

        assert!(token.is_cancelled());
        assert!(executor.has_active_dialog_turn("turn-1"));
        assert!(executor.is_dialog_turn_cancelled("turn-1"));

        executor.cleanup_dialog_turn("turn-1").await;
        assert!(!executor.has_active_dialog_turn("turn-1"));
        assert!(!executor.is_dialog_turn_cancelled("turn-1"));
    }

    #[tokio::test]
    async fn write_preflight_allows_new_file_target() {
        let root =
            std::env::temp_dir().join(format!("bitfun-write-preflight-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");
        let context = test_round_context(root.clone());

        let error = RoundExecutor::write_content_preflight_error(&context, "target.txt").await;

        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(error, None);
    }

    #[tokio::test]
    async fn write_preflight_allows_existing_file_without_read_state_tracking() {
        let root =
            std::env::temp_dir().join(format!("bitfun-write-preflight-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp workspace");
        std::fs::write(root.join("target.txt"), "old").expect("create target file");
        let context = test_round_context(root.clone());

        let error = RoundExecutor::write_content_preflight_error(&context, "target.txt").await;

        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(error, None);
    }

    #[test]
    fn detects_transient_stream_transport_error() {
        let msg = "Error: Stream processing error: SSE Error: Transport Error: Error decoding response body";
        assert!(RoundExecutor::is_transient_network_error(msg));
    }

    #[test]
    fn rejects_non_retryable_auth_error() {
        let msg = "OpenAI Streaming API client error 401: unauthorized";
        assert!(!RoundExecutor::is_transient_network_error(msg));
    }

    #[test]
    fn rejects_sse_schema_error() {
        let msg = "Stream processing error: SSE data schema error: missing field choices";
        assert!(!RoundExecutor::is_transient_network_error(msg));
    }

    #[test]
    fn rejects_provider_quota_errors_even_when_stream_closed() {
        let msg = "AI client error: Stream processing error: Provider error: provider=glm, code=1113, message=余额不足或无可用资源包,请充值。; SSE Error: stream closed before response completed";
        assert!(!RoundExecutor::is_transient_network_error(msg));
    }

    #[test]
    fn rejects_provider_auth_and_billing_errors() {
        let auth = "Provider error: provider=kimi, code=401, message=invalid API key";
        let billing =
            "OpenAI error: insufficient_quota, please check your plan and billing details";

        assert!(!RoundExecutor::is_transient_network_error(auth));
        assert!(!RoundExecutor::is_transient_network_error(billing));
    }

    #[test]
    fn detects_common_transient_provider_and_gateway_errors() {
        for msg in [
            "Anthropic API is temporarily overloaded",
            "OpenAI Streaming API error 503: service unavailable",
            "Gemini SSE stream timeout after 60s",
            "connection closed before message completed",
            "deadline exceeded while reading response body",
        ] {
            assert!(
                RoundExecutor::is_transient_network_error(msg),
                "expected retryable network error: {msg}"
            );
        }
    }

    #[test]
    fn detects_interrupted_invalid_tool_only_recovery() {
        let result = crate::agentic::execution::stream_processor::StreamResult {
            full_thinking: String::new(),
            reasoning_content_present: false,
            thinking_signature: None,
            full_text: String::new(),
            tool_calls: vec![crate::agentic::core::ToolCall {
                tool_id: "call_1".to_string(),
                tool_name: "Write".to_string(),
                arguments: serde_json::json!({}),
                raw_arguments: Some("{\"file_path\":\"src/lib.rs\"".to_string()),
                is_error: true,
                recovered_from_truncation: false,
            }],
            usage: None,
            provider_metadata: None,
            has_effective_output: true,
            first_chunk_ms: Some(1),
            first_visible_output_ms: Some(1),
            partial_recovery_reason: Some("Stream processing error: SSE stream error".to_string()),
        };

        assert!(RoundExecutor::is_interrupted_invalid_tool_only(&result));
    }

    #[test]
    fn keeps_partial_text_recovery_as_non_retryable_output() {
        let result = crate::agentic::execution::stream_processor::StreamResult {
            full_thinking: String::new(),
            reasoning_content_present: false,
            thinking_signature: None,
            full_text: "I started answering before the stream failed.".to_string(),
            tool_calls: vec![crate::agentic::core::ToolCall {
                tool_id: "call_1".to_string(),
                tool_name: "Write".to_string(),
                arguments: serde_json::json!({}),
                raw_arguments: Some("{\"file_path\":\"src/lib.rs\"".to_string()),
                is_error: true,
                recovered_from_truncation: false,
            }],
            usage: None,
            provider_metadata: None,
            has_effective_output: true,
            first_chunk_ms: Some(1),
            first_visible_output_ms: Some(1),
            partial_recovery_reason: Some("Stream processing error: SSE stream error".to_string()),
        };

        assert!(!RoundExecutor::is_interrupted_invalid_tool_only(&result));
    }

    #[test]
    fn whitespace_only_text_is_not_user_visible_assistant_text() {
        assert!(!RoundExecutor::has_user_visible_assistant_text("\n\n "));
        assert!(RoundExecutor::has_user_visible_assistant_text(
            "I can help with that."
        ));
    }

    #[test]
    fn write_content_messages_preserve_full_conversation_prefix() {
        use crate::util::types::Message as CoreAIMessage;
        use bitfun_ai_adapters::types::{Message as AIMessage, ToolCall};

        let ai_messages = vec![
            CoreAIMessage::user("Create the benchmark doc".to_string()),
            CoreAIMessage::assistant_with_tools(vec![ToolCall {
                id: "write-1".to_string(),
                name: "Write".to_string(),
                arguments: serde_json::json!({ "file_path": "notes.md" }),
                raw_arguments: None,
            }]),
            AIMessage {
                role: "tool".to_string(),
                content: Some("file body from Read".to_string()),
                reasoning_content: None,
                thinking_signature: None,
                tool_calls: None,
                tool_call_id: Some("read-1".to_string()),
                name: Some("Read".to_string()),
                is_error: None,
                tool_image_attachments: None,
            },
        ];

        let messages =
            RoundExecutor::build_write_content_messages(&ai_messages, "Write the full file.");

        assert_eq!(messages.len(), 5);
        assert!(messages[1].tool_calls.is_some());
        assert_eq!(messages[2].role, "tool");
        assert_eq!(messages[3].role, "user");
        assert_eq!(messages[4].role, "assistant");
        assert_eq!(messages[4].content.as_deref(), Some("<bitfun_contents>"));
        assert!(
            messages[4]
                .content
                .as_deref()
                .is_some_and(|content| !content.ends_with(char::is_whitespace))
        );
    }

    #[test]
    fn extract_bitfun_contents_with_tags() {
        let text =
            "Some preamble\n<bitfun_contents>\nfn main() {}\n</bitfun_contents>\nSome trailing";
        assert_eq!(extract_bitfun_contents(text), "fn main() {}");
    }

    #[test]
    fn extract_bitfun_contents_without_tags_fallback() {
        let text = "fn main() {}";
        assert_eq!(extract_bitfun_contents(text), "fn main() {}");
    }

    #[test]
    fn extract_bitfun_contents_open_tag_only() {
        let text = "<bitfun_contents>\nfn main() {}";
        assert_eq!(extract_bitfun_contents(text), "fn main() {}");
    }

    #[test]
    fn extract_bitfun_contents_empty() {
        let text = "<bitfun_contents></bitfun_contents>";
        assert_eq!(extract_bitfun_contents(text), "");
    }

    #[test]
    fn extract_bitfun_contents_prefilled_stream_without_open_tag() {
        let text = "# Title\n\nBody paragraph.\n";
        assert_eq!(
            extract_bitfun_contents_with_options(text, true),
            "# Title\n\nBody paragraph."
        );
    }

    #[test]
    fn extract_bitfun_contents_prefilled_stream_strips_called_tools_preamble() {
        let text = concat!(
            "[called tools: Read with params: {\"file_path\":\"docs/plan.md\"}]",
            "[called tools: Bash with params: {\"command\":\"cat docs/plan.md\"}]",
            "<bitfun_contents>\n# Plan\n\n## Section\n"
        );
        assert_eq!(
            extract_bitfun_contents_with_options(text, true),
            "# Plan\n\n## Section"
        );
    }

    #[test]
    fn extract_bitfun_contents_prefilled_stream_uses_last_open_tag() {
        let text = concat!(
            "[called tools: Read with params: {\"file_path\":\"a.md\"}]",
            "<bitfun_contents>\n# Wrong\n",
            "<bitfun_contents>\n# Correct\n"
        );
        assert_eq!(
            extract_bitfun_contents_with_options(text, true),
            "# Correct"
        );
    }

    #[test]
    fn sanitize_strips_called_tools_blocks_without_tags() {
        let text = "[called tools: Write with params: {\"file_path\":\"a.md\"}]fn main() {}";
        assert_eq!(extract_bitfun_contents(text), "fn main() {}");
    }

    // --- Sanitization tests ---

    #[test]
    fn sanitization_strips_leading_thinking_block() {
        let text = "<think\nLet me think about this...\n</think\nfn main() {}";
        assert_eq!(extract_bitfun_contents(text), "fn main() {}");
    }

    #[test]
    fn sanitization_strips_thinking_block_with_attrs() {
        let text = "<think type=\"deep\">\nReasoning here\n</think\nfn main() {}";
        assert_eq!(extract_bitfun_contents(text), "fn main() {}");
    }

    #[test]
    fn sanitization_strips_markdown_fences() {
        let text = "<bitfun_contents>\n```rust\nfn main() {}\n```\n</bitfun_contents>";
        assert_eq!(extract_bitfun_contents(text), "fn main() {}");
    }

    #[test]
    fn sanitization_strips_markdown_fences_without_tags() {
        // Model ignored tag instructions but used markdown fences
        let text = "```rust\nfn main() {}\n```";
        assert_eq!(extract_bitfun_contents(text), "fn main() {}");
    }

    #[test]
    fn sanitization_strips_xml_thinking_tags_with_content() {
        let text = "<bitfun_contents>\n<thinking>\nI need to write a function\n</thinking>\nfn main() {}\n</bitfun_contents>";
        assert_eq!(extract_bitfun_contents(text), "fn main() {}");
    }

    #[test]
    fn sanitization_strips_reasoning_block() {
        let text = "<bitfun_contents>\n<reasoning>\nAnalyzing code...\n</reasoning>\nfn main() {}\n</bitfun_contents>";
        assert_eq!(extract_bitfun_contents(text), "fn main() {}");
    }

    #[test]
    fn sanitization_preserves_xml_in_file_content() {
        // Real XML that should be part of the file
        let text = "<bitfun_contents>\n<config><name>test</name></config>\n</bitfun_contents>";
        assert_eq!(
            extract_bitfun_contents(text),
            "<config><name>test</name></config>"
        );
    }

    // --- Placeholder detection tests ---

    #[test]
    fn detect_placeholder_in_outline() {
        use super::detect_placeholder_patterns;
        let content = "fn main() {\n    // ... rest of the code\n}\n";
        assert!(detect_placeholder_patterns(content).is_some());
    }

    #[test]
    fn detect_placeholder_existing_code_unchanged_comment() {
        use super::detect_placeholder_patterns;
        let content = "class Foo {\n    # existing code unchanged\n    def bar(): pass\n}\n";
        assert!(detect_placeholder_patterns(content).is_some());
    }

    #[test]
    fn detect_placeholder_html_snip_marker() {
        use super::detect_placeholder_patterns;
        let content = "<html>\n  <!-- snip -->\n</html>\n";
        assert!(detect_placeholder_patterns(content).is_some());
    }

    #[test]
    fn no_false_positive_on_normal_code() {
        use super::detect_placeholder_patterns;
        let content = "fn main() {\n    println!(\"hello\");\n}\n\nstruct Foo {\n    x: i32,\n}\n";
        assert!(detect_placeholder_patterns(content).is_none());
    }

    #[test]
    fn no_false_positive_on_single_todo() {
        use super::detect_placeholder_patterns;
        // Plain TODO/FIXME comments must NOT trigger — they are common in real code.
        let content = "fn main() {\n    println!(\"hello\");\n}\n\nfn helper() {\n    // TODO: refactor later\n    // FIXME: handle errors\n    42\n}\n";
        assert!(detect_placeholder_patterns(content).is_none());
    }

    #[test]
    fn no_false_positive_on_xml_with_ellipsis() {
        use super::detect_placeholder_patterns;
        // XML/data files that genuinely contain "..." or "rest of" as data must NOT trigger.
        let content = "<doc>\n  <item>The rest of the story is told elsewhere.</item>\n  <item>Three dots: ...</item>\n</doc>\n";
        assert!(detect_placeholder_patterns(content).is_none());
    }

    #[test]
    fn no_false_positive_on_prose_mentioning_omission_phrase() {
        use super::detect_placeholder_patterns;
        // A markdown/doc file that talks about the phrase but isn't a code comment must NOT trigger.
        let content = "# Style guide\n\nDo not write \"rest omitted for brevity\" inside committed source files.\n";
        assert!(detect_placeholder_patterns(content).is_none());
    }

    #[test]
    fn detect_placeholder_empty_content() {
        use super::detect_placeholder_patterns;
        assert!(detect_placeholder_patterns("").is_none());
    }

    #[test]
    fn token_details_emits_both_cache_keys_when_present() {
        use crate::util::types::ai::GeminiUsage;
        let usage = GeminiUsage {
            prompt_token_count: 100,
            candidates_token_count: 20,
            total_token_count: 120,
            reasoning_token_count: None,
            cached_content_token_count: Some(30),
            cache_creation_token_count: Some(20),
        };
        let details = super::token_details_from_usage(&usage).expect("details");
        assert_eq!(
            details
                .get("cachedContentTokenCount")
                .and_then(|v| v.as_u64()),
            Some(30)
        );
        assert_eq!(
            details
                .get("cacheCreationTokenCount")
                .and_then(|v| v.as_u64()),
            Some(20)
        );
    }

    #[test]
    fn token_details_emits_only_read_when_creation_absent() {
        use crate::util::types::ai::GeminiUsage;
        let usage = GeminiUsage {
            prompt_token_count: 100,
            candidates_token_count: 20,
            total_token_count: 120,
            reasoning_token_count: None,
            cached_content_token_count: Some(30),
            cache_creation_token_count: None,
        };
        let details = super::token_details_from_usage(&usage).expect("details");
        assert_eq!(
            details
                .get("cachedContentTokenCount")
                .and_then(|v| v.as_u64()),
            Some(30)
        );
        assert!(details.get("cacheCreationTokenCount").is_none());
    }

    #[test]
    fn token_details_is_none_when_no_cache_info() {
        use crate::util::types::ai::GeminiUsage;
        let usage = GeminiUsage {
            prompt_token_count: 100,
            candidates_token_count: 20,
            total_token_count: 120,
            reasoning_token_count: None,
            cached_content_token_count: None,
            cache_creation_token_count: None,
        };
        assert!(super::token_details_from_usage(&usage).is_none());
    }
}
