//! Round Executor
//!
//! Executes a single model round: calls AI, processes streaming responses, executes tools

use super::model_exchange_trace::prepare_model_exchange_trace;
use super::stream_processor::{StreamProcessOptions, StreamProcessor, StreamResult};
use super::types::{FinishReason, RoundContext, RoundResult};
use crate::agentic::core::{Message, ToolCall};
use crate::agentic::events::{AgenticEvent, EventPriority, EventQueue, ToolEventData};
use crate::agentic::memories::{
    parse_bitfun_memory_citation, parse_bitfun_memory_citation_payloads,
    strip_bitfun_memory_citations,
};
use crate::agentic::tools::computer_use_host::ComputerUseHostRef;
use crate::agentic::tools::pipeline::{
    SubagentBatchExecutionPolicy as PipelineSubagentBatchExecutionPolicy, ToolExecutionContext,
    ToolExecutionOptions, ToolPipeline,
};
use crate::agentic::tools::registry::get_global_tool_registry;
use crate::agentic::tools::tool_context_runtime;
use crate::agentic::tools::tool_result_storage;
use crate::agentic::MessageContent;
use crate::infrastructure::ai::AIClient;
use crate::service::config::types::SubagentBatchExecutionPolicy as ConfigSubagentBatchExecutionPolicy;
use crate::service::config::GlobalConfigManager;
use crate::util::elapsed_ms_u64;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::Message as AIMessage;
use crate::util::types::ToolDefinition;
use bitfun_agent_runtime::tool_confirmation::{
    resolve_tool_confirmation_policy_gate, ToolConfirmationContextPolicy,
    ToolConfirmationPolicyGateFacts,
};
use bitfun_agent_runtime::turn_cancellation::DialogTurnCancellationTokenStore;
use bitfun_ai_adapters::{
    ModelExchangeRequestTraceHandle, ModelExchangeResponseTrace, ModelExchangeTraceConfig,
};
use log::{debug, error, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// Round executor
pub struct RoundExecutor {
    stream_processor: Arc<StreamProcessor>,
    tool_pipeline: Option<Arc<ToolPipeline>>,
    event_queue: Arc<EventQueue>,
    cancellation_tokens: DialogTurnCancellationTokenStore,
}

impl RoundExecutor {
    const MAX_STREAM_ATTEMPTS: usize = 10;
    const RETRY_BASE_DELAY_MS: u64 = 500;

    fn has_user_visible_assistant_text(text: &str) -> bool {
        !text.trim().is_empty()
    }

    fn parsed_memory_citation_from_stream_result(
        stream_result: &StreamResult,
    ) -> Option<crate::agentic::core::message::MemoryCitation> {
        let payloads = stream_result
            .hidden_text_blocks
            .iter()
            .filter(|block| block.name == "memory_citation")
            .map(|block| block.payload.as_str())
            .collect::<Vec<_>>();

        parse_bitfun_memory_citation_payloads(payloads)
            .or_else(|| parse_bitfun_memory_citation(&stream_result.full_text))
            .map(Into::into)
    }

    fn map_subagent_batch_execution_policy(
        policy: ConfigSubagentBatchExecutionPolicy,
    ) -> PipelineSubagentBatchExecutionPolicy {
        match policy {
            ConfigSubagentBatchExecutionPolicy::SafeOnly => {
                PipelineSubagentBatchExecutionPolicy::SafeOnly
            }
            ConfigSubagentBatchExecutionPolicy::ForceParallel => {
                PipelineSubagentBatchExecutionPolicy::ForceParallel
            }
            ConfigSubagentBatchExecutionPolicy::Serial => {
                PipelineSubagentBatchExecutionPolicy::Serial
            }
        }
    }

    async fn sleep_with_cancellation(
        delay_ms: u64,
        cancel_token: &CancellationToken,
    ) -> BitFunResult<()> {
        tokio::select! {
            _ = cancel_token.cancelled() => Err(BitFunError::Cancelled("Execution cancelled".to_string())),
            _ = tokio::time::sleep(Duration::from_millis(delay_ms)) => Ok(()),
        }
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
            cancellation_tokens: DialogTurnCancellationTokenStore::new(),
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
        let cancel_token = self
            .cancellation_tokens
            .get_or_insert_new(&context.dialog_turn_id);

        // Emit model round started event
        self.emit_event(
            AgenticEvent::ModelRoundStarted {
                session_id: context.session_id.clone(),
                turn_id: context.dialog_turn_id.clone(),
                round_id: round_id.clone(),
                round_group_id: context.round_group_id.clone(),
                round_index: context.round_number,
                model_id: Some(context.model_name.clone()),
            },
            EventPriority::High,
        )
        .await;

        let trace_config =
            prepare_model_exchange_trace(&context, &round_id, ai_client.as_ref()).await;
        let max_attempts = Self::MAX_STREAM_ATTEMPTS;
        let mut attempt_index = 0usize;
        let (stream_result, send_to_stream_ms, stream_processing_ms, final_trace_handle) = loop {
            let attempt_number = (attempt_index + 1) as u32;
            let attempt_id = format!("{round_id}:attempt:{attempt_number}");
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
            let send_future = ai_client.send_message_stream(
                ai_messages.clone(),
                tool_definitions.clone(),
                trace_config.clone(),
            );
            let send_result = tokio::select! {
                _ = cancel_token.cancelled() => {
                    return Err(BitFunError::Cancelled("Execution cancelled".to_string()));
                }
                result = send_future => result,
            };
            let (stream_response, send_to_stream_ms) = match send_result {
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
                        Self::sleep_with_cancellation(delay_ms, &cancel_token).await?;
                        attempt_index += 1;
                        continue;
                    }
                    if Self::is_transient_network_error(&err_msg) {
                        return Err(BitFunError::AIClient(format!(
                            "Stream retry budget exhausted after {} attempts: {}",
                            max_attempts, err_msg
                        )));
                    }
                    // Non-transient errors (429 budget exhausted, context
                    // overflow, auth, etc.) are returned directly. The error
                    // message is classified downstream via
                    // `BitFunError::error_category()` into `ErrorCategory` for
                    // frontend recovery actions (wait_and_retry, switch_model,
                    // etc.).
                    let error = BitFunError::AIClient(err_msg);
                    warn!(
                        "AI request terminal failure: session_id={}, round_id={}, category={:?}, error={}",
                        context.session_id,
                        round_id,
                        error.error_category(),
                        error
                    );
                    return Err(error);
                }
            };

            // Destructure StreamResponse: get stream and raw SSE data receiver
            let ai_stream = stream_response.stream;
            let raw_sse_rx = stream_response.raw_sse_rx;
            let trace_handle = stream_response.trace_handle;

            // Check cancellation token before calling stream processing.
            if cancel_token.is_cancelled() {
                Self::complete_model_exchange_trace(
                    trace_config.as_ref(),
                    trace_handle.as_ref(),
                    Self::error_trace_response("cancelled", "Execution cancelled".to_string()),
                )
                .await;
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
                    attempt_id.clone(),
                    attempt_number,
                    &cancel_token,
                    StreamProcessOptions {
                        recover_partial_on_cancel: context.recover_partial_on_cancel,
                        ..Default::default()
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
                            Self::complete_model_exchange_trace(
                                trace_config.as_ref(),
                                trace_handle.as_ref(),
                                Self::trace_response_from_stream_result("partial", &result),
                            )
                            .await;
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
                            Self::sleep_with_cancellation(delay_ms, &cancel_token).await?;
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
                            break (
                                recovered,
                                send_to_stream_ms,
                                stream_processing_ms,
                                trace_handle,
                            );
                        }

                        self.emit_failed_partial_tool_calls(
                            &context,
                            &round_id,
                            &result.tool_calls,
                            &err_msg,
                        )
                        .await;
                        Self::complete_model_exchange_trace(
                            trace_config.as_ref(),
                            trace_handle.as_ref(),
                            Self::error_trace_response_from_stream_result(
                                "error",
                                err_msg.clone(),
                                &result,
                            ),
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
                        Self::complete_model_exchange_trace(
                            trace_config.as_ref(),
                            trace_handle.as_ref(),
                            Self::trace_response_from_stream_result("partial", &result),
                        )
                        .await;
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
                        Self::sleep_with_cancellation(delay_ms, &cancel_token).await?;
                        attempt_index += 1;
                        continue;
                    }

                    if Self::is_invalid_tool_only_without_text(&result) {
                        let err_msg = "Provider returned only invalid tool arguments".to_string();
                        if attempt_index < max_attempts - 1 {
                            Self::complete_model_exchange_trace(
                                trace_config.as_ref(),
                                trace_handle.as_ref(),
                                Self::error_trace_response_from_stream_result(
                                    "error",
                                    err_msg.clone(),
                                    &result,
                                ),
                            )
                            .await;
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
                            Self::sleep_with_cancellation(delay_ms, &cancel_token).await?;
                            attempt_index += 1;
                            continue;
                        }

                        self.emit_failed_partial_tool_calls(
                            &context,
                            &round_id,
                            &result.tool_calls,
                            &err_msg,
                        )
                        .await;
                        Self::complete_model_exchange_trace(
                            trace_config.as_ref(),
                            trace_handle.as_ref(),
                            Self::error_trace_response_from_stream_result(
                                "error",
                                err_msg.clone(),
                                &result,
                            ),
                        )
                        .await;
                        return Err(BitFunError::AIClient(format!(
                            "Stream retry budget exhausted after {} attempts: {}",
                            max_attempts, err_msg
                        )));
                    }

                    if no_effective_output && attempt_index < max_attempts - 1 {
                        Self::complete_model_exchange_trace(
                            trace_config.as_ref(),
                            trace_handle.as_ref(),
                            Self::error_trace_response(
                                "error",
                                "No effective output received".to_string(),
                            ),
                        )
                        .await;
                        let delay_ms = Self::retry_delay_ms(attempt_index);
                        warn!(
                            "Retrying stream because no effective output was received: session_id={}, round_id={}, attempt={}/{}, delay_ms={}",
                            context.session_id,
                            round_id,
                            attempt_index + 1,
                            max_attempts,
                            delay_ms
                        );
                        Self::sleep_with_cancellation(delay_ms, &cancel_token).await?;
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

                    break (
                        result,
                        send_to_stream_ms,
                        stream_processing_ms,
                        trace_handle,
                    );
                }
                Err(stream_err) => {
                    let err_msg = stream_err.error.to_string();
                    let can_retry = !stream_err.has_effective_output
                        && attempt_index < max_attempts - 1
                        && Self::is_transient_network_error(&err_msg);
                    Self::complete_model_exchange_trace(
                        trace_config.as_ref(),
                        trace_handle.as_ref(),
                        Self::error_trace_response("error", err_msg.clone()),
                    )
                    .await;
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
                        Self::sleep_with_cancellation(delay_ms, &cancel_token).await?;
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

        Self::complete_model_exchange_trace(
            trace_config.as_ref(),
            final_trace_handle.as_ref(),
            Self::final_trace_response(&stream_result),
        )
        .await;

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

        // If stream response contains usage info, record it before the
        // post-stream cancellation gate. A user can press stop after the
        // provider returned usage but before this round settles; dropping that
        // usage makes cancelled turns look unaccounted even though the provider
        // already supplied authoritative counts.
        if let Some(ref usage) = stream_result.usage {
            self.emit_token_usage_update(&context, usage, context_window, is_subagent)
                .await;
        }

        // Check cancellation token again after stream processing completes.
        if cancel_token.is_cancelled() {
            debug!(
                "Cancel token detected after stream processing, stopping execution: session_id={}",
                context.session_id
            );
            return Err(BitFunError::Cancelled("Execution cancelled".to_string()));
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
            let parsed_memory_citation =
                Self::parsed_memory_citation_from_stream_result(&stream_result);
            let (clean_text, _) = strip_bitfun_memory_citations(&stream_result.full_text);
            let assistant_message =
                Message::assistant_with_reasoning(reasoning, clean_text, vec![])
                    .with_turn_id(context.dialog_turn_id.clone())
                    .with_round_id(round_id.clone())
                    .with_thinking_signature(stream_result.thinking_signature.clone())
                    .with_memory_citation(parsed_memory_citation);

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

        let tool_calls = stream_result.tool_calls.clone();

        // Execute tool calls
        debug!(
            "Preparing to execute tool calls: count={}",
            tool_calls.len()
        );

        let tool_phase_started_at = Instant::now();
        let tool_results = if let Some(tool_pipeline) = &self.tool_pipeline {
            // Create tool execution context
            let allowed_tools = context.available_tools.clone();
            let tool_context = ToolExecutionContext {
                session_id: context.session_id.clone(),
                dialog_turn_id: context.dialog_turn_id.clone(),
                round_id: round_id.clone(),
                attempt_id: Some(format!("{round_id}:attempt:{}", attempt_index + 1)),
                attempt_index: Some((attempt_index + 1) as u32),
                agent_type: context.agent_type.clone(),
                workspace: context.workspace.clone(),
                primary_model_facts: context.primary_model_facts.clone(),
                context_vars: context.context_vars.clone(),
                subagent_parent_info,
                delegation_policy: context.delegation_policy,
                deferred_tools: context.deferred_tools.clone(),
                loaded_deferred_tool_specs: context.loaded_deferred_tool_specs.clone(),
                catalog_generation: context.catalog_generation,
                allowed_tools,
                runtime_tool_restrictions: context.runtime_tool_restrictions.clone(),
                steering_interrupt: context.steering_interrupt.clone(),
                workspace_services: context.workspace_services.clone(),
                terminal_port: context.terminal_port.clone(),
                remote_exec_port: context.remote_exec_port.clone(),
            };

            // Read tool execution related configuration from global config
            let (
                needs_confirmation,
                tool_execution_timeout,
                tool_confirmation_timeout,
                subagent_batch_execution_policy,
            ) = {
                let config_service = GlobalConfigManager::get_service().await.ok();

                // Timeout and skip confirmation settings
                let (exec_timeout, confirm_timeout, skip_confirmation, task_policy) =
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
                            Self::map_subagent_batch_execution_policy(
                                ai_config.subagent_batch_execution_policy,
                            ),
                        )
                    } else {
                        (
                            None,
                            None,
                            false,
                            PipelineSubagentBatchExecutionPolicy::default(),
                        ) // Default: no timeout, requires confirmation
                    };

                let skip_from_context = context
                    .context_vars
                    .get("skip_tool_confirmation")
                    .map(|v| v == "true")
                    .unwrap_or(false);
                let require_from_context = context
                    .context_vars
                    .get("require_tool_confirmation")
                    .map(|v| v == "true")
                    .unwrap_or(false);
                let context_policy = if require_from_context {
                    ToolConfirmationContextPolicy::Require
                } else if skip_from_context {
                    ToolConfirmationContextPolicy::Skip
                } else {
                    ToolConfirmationContextPolicy::Inherit
                };

                let skips_confirmation = match context_policy {
                    ToolConfirmationContextPolicy::Require => false,
                    ToolConfirmationContextPolicy::Skip => true,
                    ToolConfirmationContextPolicy::Inherit => skip_confirmation,
                };
                let any_tool_needs_permission = if skips_confirmation {
                    false
                } else {
                    let registry = get_global_tool_registry();
                    let tool_registry = registry.read().await;

                    stream_result.tool_calls.iter().any(|tool_call| {
                        tool_registry
                            .get_tool(&tool_call.tool_name)
                            .map(|tool| tool.needs_permissions(Some(&tool_call.arguments)))
                            .unwrap_or(false)
                    })
                };
                let needs_confirm =
                    resolve_tool_confirmation_policy_gate(ToolConfirmationPolicyGateFacts {
                        global_skip_tool_confirmation: skip_confirmation,
                        context_policy,
                        any_tool_needs_permission,
                    })
                    .confirm_before_run();

                (needs_confirm, exec_timeout, confirm_timeout, task_policy)
            };

            // Create tool execution options (use configured timeout values)
            let tool_options = ToolExecutionOptions {
                confirm_before_run: needs_confirmation,
                timeout_secs: tool_execution_timeout,
                confirmation_timeout_secs: tool_confirmation_timeout,
                subagent_batch_execution_policy,
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
                            effective_tool_name: tc.tool_name.clone(),
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
        let parsed_memory_citation =
            Self::parsed_memory_citation_from_stream_result(&stream_result);
        let (clean_text, _) = strip_bitfun_memory_citations(&stream_result.full_text);
        let assistant_message =
            Message::assistant_with_reasoning(reasoning, clean_text, tool_calls.clone())
                .with_turn_id(context.dialog_turn_id.clone())
                .with_round_id(round_id.clone())
                .with_thinking_signature(stream_result.thinking_signature.clone())
                .with_memory_citation(parsed_memory_citation);

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
            tool_calls,
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
        self.cancellation_tokens.has_active(dialog_turn_id)
    }

    /// Check if dialog turn cancellation has been requested.
    pub fn is_dialog_turn_cancelled(&self, dialog_turn_id: &str) -> bool {
        self.cancellation_tokens.is_cancelled(dialog_turn_id)
    }

    /// Register cancellation token (for external control, e.g., execute_subagent)
    pub fn register_cancel_token(&self, dialog_turn_id: &str, token: CancellationToken) {
        self.cancellation_tokens.insert(dialog_turn_id, token);
    }

    /// Return a clone of the cancellation token registered for a dialog turn.
    pub fn cancel_token_for_dialog_turn(&self, dialog_turn_id: &str) -> Option<CancellationToken> {
        self.cancellation_tokens.token(dialog_turn_id)
    }

    /// Cancel dialog turn (using dialog_turn_id)
    pub async fn cancel_dialog_turn(&self, dialog_turn_id: &str) -> BitFunResult<()> {
        debug!("Cancelling dialog turn: dialog_turn_id={}", dialog_turn_id);

        if self.cancellation_tokens.cancel(dialog_turn_id) {
            debug!("Found cancel token, triggering cancellation");
            debug!("Cancel token triggered");
        } else {
            debug!("Cancel token not found (dialog may have completed or not started)");
        }

        Ok(())
    }

    /// Cleanup dialog turn token (called on normal completion)
    pub async fn cleanup_dialog_turn(&self, dialog_turn_id: &str) {
        if self.cancellation_tokens.remove(dialog_turn_id) {
            debug!("Cleaned up cancel token: dialog_turn_id={}", dialog_turn_id);
        }
    }

    /// Emit event
    async fn emit_event(&self, event: AgenticEvent, priority: EventPriority) {
        let _ = self.event_queue.enqueue(event, Some(priority)).await;
    }

    async fn emit_token_usage_update(
        &self,
        context: &RoundContext,
        usage: &crate::util::types::ai::GeminiUsage,
        context_window: Option<usize>,
        is_subagent: bool,
    ) {
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
                    attempt_id: None,
                    attempt_index: None,
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

    async fn complete_model_exchange_trace(
        trace_config: Option<&ModelExchangeTraceConfig>,
        trace_handle: Option<&ModelExchangeRequestTraceHandle>,
        response: ModelExchangeResponseTrace,
    ) {
        let (Some(trace_config), Some(trace_handle)) = (trace_config, trace_handle) else {
            return;
        };

        trace_config
            .sink
            .request_attempt_completed(trace_handle, &response)
            .await;
    }

    fn final_trace_response(result: &StreamResult) -> ModelExchangeResponseTrace {
        let kind = if result.partial_recovery_reason.is_some() {
            "partial"
        } else {
            "completed"
        };
        Self::trace_response(kind, Some(result), None)
    }

    fn trace_response_from_stream_result(
        kind: &str,
        result: &StreamResult,
    ) -> ModelExchangeResponseTrace {
        Self::trace_response(kind, Some(result), None)
    }

    fn error_trace_response_from_stream_result(
        kind: &str,
        error: String,
        result: &StreamResult,
    ) -> ModelExchangeResponseTrace {
        Self::trace_response(kind, Some(result), Some(error))
    }

    fn error_trace_response(kind: &str, error: String) -> ModelExchangeResponseTrace {
        Self::trace_response(kind, None, Some(error))
    }

    fn trace_response(
        kind: &str,
        result: Option<&StreamResult>,
        error: Option<String>,
    ) -> ModelExchangeResponseTrace {
        let (
            assistant_text,
            thinking,
            tool_calls,
            usage,
            provider_metadata,
            partial_recovery_reason,
        ) = if let Some(result) = result {
            (
                Some(result.full_text.clone()),
                Self::stream_result_reasoning(result),
                serde_json::to_value(&result.tool_calls).ok(),
                result
                    .usage
                    .as_ref()
                    .and_then(|usage| serde_json::to_value(usage).ok()),
                result.provider_metadata.clone(),
                result.partial_recovery_reason.clone(),
            )
        } else {
            (None, None, None, None, None, None)
        };

        ModelExchangeResponseTrace {
            kind: kind.to_string(),
            assistant_text,
            thinking,
            tool_calls,
            usage,
            provider_metadata,
            partial_recovery_reason,
            error,
        }
    }

    fn stream_result_reasoning(result: &StreamResult) -> Option<String> {
        if result.full_thinking.is_empty() {
            result.reasoning_content_present.then(String::new)
        } else {
            Some(result.full_thinking.clone())
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

    /// Check whether an error message represents a transient (retryable) condition.
    ///
    /// Errors that already exhausted the SSE-layer retry budget (e.g. "failed
    /// after N attempts:" or "Stream retry budget exhausted") are **not**
    /// transient from the round-executor perspective — the SSE transport layer
    /// already retried with exponential backoff and `Retry-After` parsing.
    /// Re-entering the send loop would multiply attempts (10 × 10 = 100) and
    /// hold the user in a long silent stall.
    fn is_transient_network_error(error_message: &str) -> bool {
        let msg = error_message.to_lowercase();

        // The SSE layer already exhausted its own retry budget — do not
        // re-enter another round of attempts from the round executor.
        // We require BOTH "failed after " and "attempts:" to co-occur,
        // which uniquely identifies the SSE/round-executor budget-exhausted
        // format without catching generic errors like "failed after timeout".
        if msg.contains("failed after ") && msg.contains("attempts:") {
            return false;
        }
        if msg.contains("retry budget exhausted") {
            return false;
        }

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

#[cfg(test)]
mod tests {
    use super::{RoundExecutor, StreamProcessor};
    use crate::agentic::core::ToolCall;
    use crate::agentic::events::{EventQueue, EventQueueConfig};
    use crate::agentic::execution::stream_processor::StreamResult;
    use crate::agentic::execution::types::RoundContext;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::util::errors::BitFunError;
    use crate::util::types::ai::GeminiUsage;
    use bitfun_agent_runtime::turn_cancellation::DialogTurnCancellationTokenStore;
    use bitfun_runtime_ports::DelegationPolicy;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    fn test_round_executor() -> RoundExecutor {
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        RoundExecutor {
            stream_processor: Arc::new(StreamProcessor::new(event_queue.clone())),
            tool_pipeline: None,
            event_queue,
            cancellation_tokens: DialogTurnCancellationTokenStore::new(),
        }
    }

    fn test_round_context() -> RoundContext {
        RoundContext {
            session_id: "session-1".to_string(),
            subagent_parent_info: None,
            dialog_turn_id: "turn-1".to_string(),
            turn_index: 0,
            round_number: 0,
            round_group_id: None,
            workspace: None,
            model_exchange_trace_dir: None,
            available_tools: Vec::new(),
            deferred_tools: Vec::new(),
            loaded_deferred_tool_specs: Vec::new(),
            catalog_generation: 0,
            model_name: "model-1".to_string(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::new(
                "model-1", "model-1", "openai", true,
            ),
            agent_type: "agentic".to_string(),
            context_vars: HashMap::new(),
            delegation_policy: DelegationPolicy::top_level(),
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            steering_interrupt: None,
            cancellation_token: CancellationToken::new(),
            workspace_services: None,
            terminal_port: None,
            remote_exec_port: None,
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
    async fn emits_token_usage_before_post_stream_cancel_stops_round() {
        let executor = test_round_executor();
        let context = test_round_context();
        let usage = GeminiUsage {
            prompt_token_count: 100,
            candidates_token_count: 20,
            total_token_count: 120,
            reasoning_token_count: None,
            cached_content_token_count: Some(30),
            cache_creation_token_count: None,
        };

        executor
            .emit_token_usage_update(&context, &usage, Some(128_000), false)
            .await;

        let events = executor.event_queue.dequeue_batch(10).await;
        assert!(events.iter().any(|envelope| matches!(
            &envelope.event,
            crate::agentic::events::AgenticEvent::TokenUsageUpdated {
                session_id,
                turn_id,
                model_id,
                input_tokens: 100,
                output_tokens: Some(20),
                total_tokens: 120,
                max_context_tokens: Some(128_000),
                is_subagent: false,
                cached_tokens: Some(30),
                ..
            } if session_id == "session-1" && turn_id == "turn-1" && model_id == "model-1"
        )));
    }

    #[tokio::test]
    async fn cancellable_sleep_returns_cancelled_when_token_fires() {
        let token = CancellationToken::new();
        let token_for_task = token.clone();

        let waiter = tokio::spawn(async move {
            RoundExecutor::sleep_with_cancellation(5_000, &token_for_task).await
        });

        tokio::time::sleep(Duration::from_millis(20)).await;
        token.cancel();

        let result = waiter.await.expect("sleep task should join");
        assert!(matches!(result, Err(BitFunError::Cancelled(_))));
    }

    #[tokio::test]
    async fn cancellable_sleep_completes_normally_without_cancel() {
        let token = CancellationToken::new();

        let result = RoundExecutor::sleep_with_cancellation(10, &token).await;

        assert!(result.is_ok());
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

    #[test]
    fn error_trace_response_from_stream_result_preserves_structured_context() {
        let stream_result = StreamResult {
            full_thinking: "reasoning".to_string(),
            reasoning_content_present: true,
            thinking_signature: Some("sig".to_string()),
            full_text: String::new(),
            hidden_text_blocks: Vec::new(),
            tool_calls: vec![ToolCall {
                tool_id: "tool-1".to_string(),
                tool_name: "Bash".to_string(),
                arguments: json!({}),
                raw_arguments: Some("{\"command\":".to_string()),
                is_error: true,
                recovered_from_truncation: false,
            }],
            usage: Some(GeminiUsage {
                prompt_token_count: 100,
                candidates_token_count: 20,
                total_token_count: 120,
                reasoning_token_count: Some(5),
                cached_content_token_count: Some(30),
                cache_creation_token_count: None,
            }),
            provider_metadata: Some(json!({ "finish_reason": "tool_calls" })),
            has_effective_output: false,
            first_chunk_ms: Some(10),
            first_visible_output_ms: None,
            partial_recovery_reason: Some("tool arguments invalid".to_string()),
        };

        let trace = RoundExecutor::error_trace_response_from_stream_result(
            "error",
            "Provider returned only invalid tool arguments".to_string(),
            &stream_result,
        );

        assert_eq!(trace.kind, "error");
        assert_eq!(
            trace.error.as_deref(),
            Some("Provider returned only invalid tool arguments")
        );
        assert_eq!(trace.assistant_text.as_deref(), Some(""));
        assert_eq!(trace.thinking.as_deref(), Some("reasoning"));
        assert_eq!(
            trace.partial_recovery_reason.as_deref(),
            Some("tool arguments invalid")
        );
        assert_eq!(
            trace.provider_metadata,
            Some(json!({ "finish_reason": "tool_calls" }))
        );
        assert_eq!(
            trace.usage,
            Some(json!({
                "promptTokenCount": 100,
                "candidatesTokenCount": 20,
                "totalTokenCount": 120,
                "reasoningTokenCount": 5,
                "cachedContentTokenCount": 30
            }))
        );
        assert_eq!(
            trace.tool_calls,
            Some(json!([{
                "tool_id": "tool-1",
                "tool_name": "Bash",
                "arguments": {},
                "raw_arguments": "{\"command\":",
                "is_error": true
            }]))
        );
    }

    #[test]
    fn error_trace_response_without_stream_result_stays_empty() {
        let trace = RoundExecutor::error_trace_response("error", "request failed".to_string());

        assert_eq!(trace.kind, "error");
        assert!(trace.assistant_text.is_none());
        assert!(trace.thinking.is_none());
        assert!(trace.tool_calls.is_none());
        assert!(trace.usage.is_none());
        assert!(trace.provider_metadata.is_none());
        assert!(trace.partial_recovery_reason.is_none());
        assert_eq!(trace.error.as_deref(), Some("request failed"));
    }

    #[test]
    fn is_transient_error_treats_rate_limit_as_transient() {
        assert!(RoundExecutor::is_transient_network_error(
            "OpenAI Streaming API error 429 Too Many Requests"
        ));
        assert!(RoundExecutor::is_transient_network_error(
            "rate limit exceeded"
        ));
    }

    #[test]
    fn is_transient_error_treats_network_errors_as_transient() {
        assert!(RoundExecutor::is_transient_network_error(
            "connection reset by peer"
        ));
        assert!(RoundExecutor::is_transient_network_error("timeout"));
    }

    #[test]
    fn is_transient_error_treats_context_overflow_as_non_transient() {
        assert!(!RoundExecutor::is_transient_network_error(
            "prompt is too long"
        ));
    }

    #[test]
    fn is_transient_error_treats_budget_exhausted_as_non_transient() {
        // After SSE layer exhausts its retry budget, the round executor must
        // NOT re-enter another round of attempts (would cause 10×10 = 100
        // retries).
        assert!(!RoundExecutor::is_transient_network_error(
            "OpenAI Streaming API failed after 10 attempts: \
             OpenAI Streaming API error 429 Too Many Requests"
        ));
        assert!(!RoundExecutor::is_transient_network_error(
            "Stream retry budget exhausted after 10 attempts: timeout"
        ));
    }

    #[test]
    fn is_transient_error_does_not_misclassify_failed_after_without_attempts() {
        // "failed after " without "attempts:" should NOT be treated as budget
        // exhausted — it may be a legitimately retryable transient error.
        assert!(RoundExecutor::is_transient_network_error(
            "stream failed after connection reset"
        ));
        assert!(RoundExecutor::is_transient_network_error(
            "request failed after timeout"
        ));
    }
}
