//! Round Executor
//!
//! Executes a single model round: calls AI, processes streaming responses, executes tools

use super::stream_processor::StreamProcessor;
use super::types::{FinishReason, RoundContext, RoundResult};
use crate::agentic::core::Message;
use crate::agentic::events::{AgenticEvent, EventPriority, EventQueue};
use crate::agentic::tools::pipeline::{ToolExecutionContext, ToolExecutionOptions, ToolPipeline};
use crate::agentic::tools::registry::get_global_tool_registry;
use crate::agentic::MessageContent;
use crate::infrastructure::ai::AIClient;
use crate::service::config::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::Message as AIMessage;
use crate::util::types::ToolDefinition;
use dashmap::DashMap;
use log::{debug, error, warn};
use std::sync::Arc;
use std::time::Duration;
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
    const MAX_RETRIES_WITHOUT_OUTPUT: usize = 1;
    const RETRY_BASE_DELAY_MS: u64 = 500;

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

    /// Execute a single model round
    pub async fn execute_round(
        &self,
        ai_client: Arc<AIClient>,
        context: RoundContext,
        ai_messages: Vec<AIMessage>,
        tool_definitions: Option<Vec<ToolDefinition>>,
        context_window: Option<usize>,
    ) -> BitFunResult<RoundResult> {
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
            },
            EventPriority::High,
        )
        .await;

        let max_attempts = Self::MAX_RETRIES_WITHOUT_OUTPUT + 1;
        let mut attempt_index = 0usize;
        let stream_result = loop {
            debug!(
                "Sending request: model={}, messages={}, tools={}, attempt={}/{}",
                context.model_name,
                ai_messages.len(),
                tool_definitions.as_ref().map(|t| t.len()).unwrap_or(0),
                attempt_index + 1,
                max_attempts
            );

            // Use dynamically obtained client for call
            let stream_response = match ai_client
                .send_message_stream(ai_messages.clone(), tool_definitions.clone())
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    error!("AI request failed: {}", e);
                    let err_msg = e.to_string();
                    let can_retry = attempt_index < max_attempts - 1
                        && Self::is_transient_network_error(&err_msg);
                    if can_retry {
                        let delay_ms = Self::retry_delay_ms(attempt_index);
                        warn!(
                            "Retrying request after transient error with no output: session_id={}, round_id={}, attempt={}/{}, delay_ms={}, error={}",
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
                    return Err(BitFunError::AIClient(err_msg));
                }
            };

            // Destructure StreamResponse: get stream and raw SSE data receiver
            let ai_stream = stream_response.stream;
            let raw_sse_rx = stream_response.raw_sse_rx;

            // Check cancellation token before calling stream processing
            if cancel_token.is_cancelled() {
                debug!(
                    "Cancel token detected before AI call, stopping execution: session_id={}",
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

            match self
                .stream_processor
                .process_stream(
                    ai_stream,
                    raw_sse_rx, // Pass raw SSE data receiver (for error diagnosis)
                    context.session_id.clone(),
                    context.dialog_turn_id.clone(),
                    round_id.clone(),
                    subagent_parent_info.clone(),
                    &cancel_token,
                )
                .await
            {
                Ok(result) => {
                    let no_effective_output = !result.has_effective_output;
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
                    break result;
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
                    return Err(stream_err.error);
                }
            }
        };

        // Model returned successfully (output to AI log file)
        let tool_names: Vec<&str> = stream_result
            .tool_calls
            .iter()
            .map(|tc| tc.tool_name.as_str())
            .collect();
        debug!(
            target: "ai::model_response",
            "Model response received: text_length={}, tool_calls={}, token_usage={:?}",
            stream_result.full_text.len(),
            if tool_names.is_empty() { "none".to_string() } else { tool_names.join(", ") },
            stream_result.usage.as_ref().map(|u| format!("input={}, output={}, total={}", u.prompt_token_count, u.candidates_token_count, u.total_token_count)).unwrap_or_else(|| "none".to_string())
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
                "Updating token stats from model response: input={}, output={}, total={}",
                usage.prompt_token_count, usage.candidates_token_count, usage.total_token_count
            );

            // Subagent does not send token events
            if !is_subagent {
                self.emit_event(
                    AgenticEvent::TokenUsageUpdated {
                        session_id: context.session_id.clone(),
                        turn_id: context.dialog_turn_id.clone(),
                        input_tokens: usage.prompt_token_count as usize,
                        output_tokens: Some(usage.candidates_token_count as usize),
                        total_tokens: usage.total_token_count as usize,
                        max_context_tokens: context_window,
                    },
                    EventPriority::Normal,
                )
                .await;
            }
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
                None
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

            // Note: Do not cleanup cancellation token here, as this is only the end of a single model round
            // Cancellation token will be cleaned up by ExecutionEngine when the entire dialog turn ends

            return Ok(RoundResult {
                assistant_message,
                tool_calls: vec![],
                tool_result_messages: vec![],
                has_more_rounds: false,
                finish_reason: FinishReason::Complete,
                usage: stream_result.usage.clone(),
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

        // Execute tool calls
        debug!(
            "Preparing to execute tool calls: count={}",
            stream_result.tool_calls.len()
        );

        let tool_results = if let Some(tool_pipeline) = &self.tool_pipeline {
            // Create tool execution context
            let tool_context = ToolExecutionContext {
                session_id: context.session_id.clone(),
                dialog_turn_id: context.dialog_turn_id.clone(),
                agent_type: context.agent_type.clone(),
                context_vars: context.context_vars.clone(),
                subagent_parent_info,
                allowed_tools: context.available_tools.clone(), // Pass allowed tools list for security validation
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

                // If config skips confirmation, directly return false
                let needs_confirm = if skip_confirmation {
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

            // Execute tools
            let execution_results = tool_pipeline
                .execute_tools(stream_result.tool_calls.clone(), tool_context, tool_options)
                .await?;

            // Convert to ToolResult
            execution_results.into_iter().map(|r| r.result).collect()
        } else {
            vec![]
        };

        // Create assistant message (includes tool calls and thinking content, supports interleaved thinking mode)
        let reasoning = if stream_result.full_thinking.is_empty() {
            None
        } else {
            Some(stream_result.full_thinking.clone())
        };
        let assistant_message = Message::assistant_with_reasoning(
            reasoning,
            stream_result.full_text.clone(),
            stream_result.tool_calls.clone(),
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

        // Detect tool calls that should end the dialog turn (requires should_end_turn and execution result has no error)
        // When there is only 1 should_end_turn tool → Execute normally, end turn on success
        // When there are multiple should_end_turn tools → These tools are marked as errors in pipeline → Because is_error=true, won't trigger end turn → Model can re-call correctly in next round after receiving error message
        let has_end_turn_tool = {
            stream_result.tool_calls.iter().any(|tool_call| {
                // Check if tool should_end_turn
                if !tool_call.should_end_turn {
                    return false;
                }

                // Check if this tool's execution result has no error
                let result_not_error = tool_results
                    .iter()
                    .find(|r| r.tool_id == tool_call.tool_id)
                    .map(|r| !r.is_error)
                    .unwrap_or(false);

                result_not_error
            })
        };

        // Create tool result messages (also need to set turn_id and round_id)
        let dialog_turn_id = context.dialog_turn_id.clone();
        let round_id_clone = round_id.clone();
        let tool_result_messages: Vec<Message> = tool_results
            .into_iter()
            .map(|result| {
                Message::tool_result(result)
                    .with_turn_id(dialog_turn_id.clone())
                    .with_round_id(round_id_clone.clone())
            })
            .collect();

        let has_more_rounds = !has_end_turn_tool && !tool_result_messages.is_empty();

        debug!(
            "Returning RoundResult: has_more_rounds={}, tool_result_messages={}, has_end_turn_tool={}",
            has_more_rounds,
            tool_result_messages.len(),
            has_end_turn_tool
        );

        // Note: Do not cleanup cancellation token here, as there may be subsequent model rounds
        // Cancellation token will be cleaned up by ExecutionEngine when the entire dialog turn ends

        Ok(RoundResult {
            assistant_message,
            tool_calls: stream_result.tool_calls.clone(),
            tool_result_messages,
            has_more_rounds,
            finish_reason: if has_more_rounds {
                FinishReason::ToolCalls
            } else {
                FinishReason::Complete
            },
            usage: stream_result.usage.clone(),
        })
    }

    /// Check if dialog turn is still active (used to detect cancellation)
    pub fn has_active_dialog_turn(&self, dialog_turn_id: &str) -> bool {
        self.cancellation_tokens.contains_key(dialog_turn_id)
    }

    /// Register cancellation token (for external control, e.g., execute_subagent)
    pub fn register_cancel_token(&self, dialog_turn_id: &str, token: CancellationToken) {
        self.cancellation_tokens
            .insert(dialog_turn_id.to_string(), token);
    }

    /// Cancel dialog turn (using dialog_turn_id)
    pub async fn cancel_dialog_turn(&self, dialog_turn_id: &str) -> BitFunResult<()> {
        debug!("Cancelling dialog turn: dialog_turn_id={}", dialog_turn_id);

        if let Some((_, token)) = self.cancellation_tokens.remove(dialog_turn_id) {
            debug!("Found cancel token, triggering cancellation");
            token.cancel();
            debug!("Cancel token triggered and cleaned up");
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

    /// Emit event
    async fn emit_event(&self, event: AgenticEvent, priority: EventPriority) {
        let _ = self.event_queue.enqueue(event, Some(priority)).await;
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
            "client error 400",
            "client error 401",
            "client error 403",
            "client error 404",
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
            "connection reset",
            "broken pipe",
            "unexpected eof",
            "connection refused",
            "temporarily unavailable",
            "gateway timeout",
            "proxy",
            "tunnel",
            "dns",
            "network",
            "econnreset",
            "econnrefused",
            "etimedout",
            "rate limit",
            "too many requests",
            "429",
        ];

        if non_retryable_keywords.iter().any(|k| msg.contains(k)) {
            return false;
        }

        transient_keywords.iter().any(|k| msg.contains(k))
    }
}

#[cfg(test)]
mod tests {
    use super::RoundExecutor;

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
}
