//! Round Executor
//!
//! Executes a single model round: calls AI, processes streaming responses, executes tools

use super::stream_processor::{StreamProcessOptions, StreamProcessor, StreamResult};
use super::types::{FinishReason, RoundContext, RoundResult};
use crate::agentic::core::{Message, ToolCall};
use crate::agentic::events::{AgenticEvent, EventPriority, EventQueue, ToolEventData};
use crate::agentic::tools::computer_use_host::ComputerUseHostRef;
use crate::agentic::tools::pipeline::{ToolExecutionContext, ToolExecutionOptions, ToolPipeline};
use crate::agentic::tools::registry::get_global_tool_registry;
use crate::agentic::MessageContent;
use crate::infrastructure::ai::AIClient;
use crate::service::config::GlobalConfigManager;
use crate::util::elapsed_ms_u64;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::Message as AIMessage;
use crate::util::types::ToolDefinition;
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

    fn has_user_visible_assistant_text(text: &str) -> bool {
        !text.trim().is_empty()
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
        let event_subagent_parent_info = subagent_parent_info.clone().map(|info| info.into());

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
                subagent_parent_info: event_subagent_parent_info.clone(),
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
                    if Self::is_transient_network_error(&err_msg) {
                        return Err(BitFunError::AIClient(format!(
                            "AI stream connection retry budget exhausted: {}",
                            err_msg
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
                    subagent_parent_info.clone(),
                    &cancel_token,
                    StreamProcessOptions {
                        recover_partial_on_cancel: context.recover_partial_on_cancel,
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
                                result.tool_calls.iter().filter(|tool_call| !tool_call.is_valid()).count(),
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
                                result.tool_calls.iter().filter(|tool_call| !tool_call.is_valid()).count(),
                                err_msg
                            );
                            self.emit_failed_partial_tool_calls(
                                &context,
                                &result.tool_calls,
                                &err_msg,
                                event_subagent_parent_info.clone(),
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
                            &result.tool_calls,
                            &err_msg,
                            event_subagent_parent_info.clone(),
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
                            &result.tool_calls,
                            err_msg,
                            event_subagent_parent_info.clone(),
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
                            result.partial_recovery_reason.as_deref().unwrap_or("unknown")
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
                subagent_parent_info: event_subagent_parent_info.clone(),
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
        let tool_calls = stream_result.tool_calls.clone();
        let tool_calls = self
            .generate_write_tool_contents(
                ai_client.clone(),
                &context,
                &ai_messages,
                tool_calls,
                &cancel_token,
                event_subagent_parent_info.clone(),
            )
            .await?;

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
                agent_type: context.agent_type.clone(),
                workspace: context.workspace.clone(),
                context_vars: context.context_vars.clone(),
                subagent_parent_info,
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

            // Convert to ToolResult
            execution_results.into_iter().map(|r| r.result).collect()
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
        ai_messages: &[AIMessage],
        mut tool_calls: Vec<ToolCall>,
        cancel_token: &CancellationToken,
        subagent_parent_info: Option<crate::agentic::events::SubagentParentInfo>,
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

            // Emit Started event so the UI can show the tool card
            self.emit_event(
                AgenticEvent::ToolEvent {
                    session_id: context.session_id.clone(),
                    turn_id: context.dialog_turn_id.clone(),
                    tool_event: ToolEventData::Started {
                        tool_id: tool_id.clone(),
                        tool_name: "Write".to_string(),
                        params: tc.arguments.clone(),
                        timeout_seconds: None,
                    },
                    subagent_parent_info: subagent_parent_info.clone(),
                },
                EventPriority::High,
            )
            .await;

            // Build a content-generation prompt
            let content_prompt = format!(
                "Now output the complete file content for the file `{}`. \
                 Output ONLY the raw file content wrapped in <bitfun_contents> tags. \
                 Do NOT include any other text, explanation, or commentary outside the tags.\n\
                 <bitfun_contents>\n",
                file_path
            );

            let mut content_messages = ai_messages.to_vec();
            content_messages.push(AIMessage::user(content_prompt));

            // Send the content-generation request (no tools, pure text output)
            let full_text = match ai_client.send_message_stream(content_messages, None).await {
                Ok(response) => {
                    let mut text = String::new();
                    let mut stream = response.stream;
                    use futures::StreamExt;
                    while let Some(chunk) = stream.next().await {
                        if cancel_token.is_cancelled() {
                            return Err(BitFunError::Cancelled("Execution cancelled".to_string()));
                        }
                        match chunk {
                            Ok(resp) => {
                                let chunk_text = resp.text.unwrap_or_default();
                                if !chunk_text.is_empty() {
                                    text.push_str(&chunk_text);

                                    // Emit streaming ParamsPartial so the UI
                                    // shows a live content preview
                                    let params = serde_json::json!({
                                        "file_path": &file_path,
                                        "content": &text,
                                    });
                                    self.emit_event(
                                        AgenticEvent::ToolEvent {
                                            session_id: context.session_id.clone(),
                                            turn_id: context.dialog_turn_id.clone(),
                                            tool_event: ToolEventData::ParamsPartial {
                                                tool_id: tool_id.clone(),
                                                tool_name: "Write".to_string(),
                                                params: params.to_string(),
                                            },
                                            subagent_parent_info: subagent_parent_info.clone(),
                                        },
                                        EventPriority::Normal,
                                    )
                                    .await;
                                }
                            }
                            Err(e) => {
                                error!("Error in Write content generation stream: {}", e);
                                break;
                            }
                        }
                    }
                    text
                }
                Err(e) => {
                    error!("Write content generation request failed: {}", e);
                    return Err(BitFunError::AIClient(format!(
                        "Write content generation failed for {}: {}",
                        file_path, e
                    )));
                }
            };

            let content = extract_bitfun_contents(&full_text);
            if content.is_empty() {
                warn!(
                    "Write content generation returned empty content for file_path={}",
                    file_path
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
                    tool_event: ToolEventData::ParamsPartial {
                        tool_id: tool_id.clone(),
                        tool_name: "Write".to_string(),
                        params: final_params.to_string(),
                    },
                    subagent_parent_info: subagent_parent_info.clone(),
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

    /// Emit event
    async fn emit_event(&self, event: AgenticEvent, priority: EventPriority) {
        let _ = self.event_queue.enqueue(event, Some(priority)).await;
    }

    async fn emit_failed_partial_tool_calls(
        &self,
        context: &RoundContext,
        tool_calls: &[ToolCall],
        error: &str,
        subagent_parent_info: Option<crate::agentic::events::SubagentParentInfo>,
    ) {
        for tool_call in tool_calls {
            self.emit_event(
                AgenticEvent::ToolEvent {
                    session_id: context.session_id.clone(),
                    turn_id: context.dialog_turn_id.clone(),
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
                    subagent_parent_info: subagent_parent_info.clone(),
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

    (!details.is_empty()).then_some(serde_json::Value::Object(details))
}

/// Extract content from `<bitfun_contents>...</bitfun_contents>` tags.
///
/// If the tags are present, returns the text between them (trimmed).
/// If the tags are not present, returns the full text trimmed (fallback for
/// models that ignore the tag instruction).
fn extract_bitfun_contents(text: &str) -> String {
    const OPEN_TAG: &str = "<bitfun_contents>";
    const CLOSE_TAG: &str = "</bitfun_contents>";

    if let Some(start) = text.find(OPEN_TAG) {
        let content_start = start + OPEN_TAG.len();
        if let Some(end) = text[content_start..].find(CLOSE_TAG) {
            return text[content_start..content_start + end].trim().to_string();
        }
        // Opening tag found but no closing tag — take everything after the
        // opening tag (the model may still be streaming or forgot to close).
        return text[content_start..].trim().to_string();
    }

    // No tags at all — return the full text as a fallback
    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{extract_bitfun_contents, RoundExecutor, StreamProcessor};
    use crate::agentic::events::{EventQueue, EventQueueConfig};
    use dashmap::DashMap;
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
}
