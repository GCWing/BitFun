//! Stream Processor
//!
//! Processes AI streaming responses, supports tool pre-detection and parameter streaming

use crate::agentic::core::ToolCall;
use crate::agentic::events::{
    AgenticEvent, EventPriority, EventQueue, SubagentParentInfo as EventSubagentParentInfo,
    ToolEventData,
};
use crate::agentic::tools::SubagentParentInfo;
use crate::infrastructure::ai::tool_call_accumulator::{
    FinalizedToolCall, PendingToolCall, ToolCallBoundary,
};
use crate::util::errors::BitFunError;
use crate::util::types::ai::GeminiUsage;
use ai_stream_handlers::UnifiedResponse;
use futures::StreamExt;
use log::{debug, error, trace};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;

//==============================================================================
// SSE Log Collector - Outputs raw SSE data on error
//==============================================================================

/// SSE log collector configuration
#[derive(Debug, Clone, Default)]
pub struct SseLogConfig {
    /// Maximum number of SSE data entries to output on error, None means unlimited
    pub max_output: Option<usize>,
}

/// SSE log collector - Collects raw SSE data, outputs only on error
pub struct SseLogCollector {
    buffer: Vec<String>,
    config: SseLogConfig,
}

impl SseLogCollector {
    pub fn new(config: SseLogConfig) -> Self {
        Self {
            buffer: Vec::new(),
            config,
        }
    }

    /// Push one SSE data entry
    pub fn push(&mut self, data: String) {
        self.buffer.push(data);
    }

    /// Get number of collected data entries
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Flush all SSE data to log on error
    pub fn flush_on_error(&self, error_context: &str) {
        if self.buffer.is_empty() {
            error!("SSE Error: {} (no SSE data collected)", error_context);
            return;
        }

        error!("SSE Error: {}", error_context);
        let mut sse_msg = format!("SSE history ({} events):\n", self.buffer.len());

        match self.config.max_output {
            None => {
                // No limit, output all
                for (i, data) in self.buffer.iter().enumerate() {
                    sse_msg.push_str(&format!("{:>6}: {}\n", i, data));
                }
            }
            Some(max) if self.buffer.len() <= max => {
                // Within limit, output all
                for (i, data) in self.buffer.iter().enumerate() {
                    sse_msg.push_str(&format!("{:>6}: {}\n", i, data));
                }
            }
            Some(max) => {
                // Exceeds limit, smart truncation: output beginning + end
                let head = 50.min(max / 2);
                let tail = max - head;
                let total = self.buffer.len();

                for (i, data) in self.buffer.iter().take(head).enumerate() {
                    sse_msg.push_str(&format!("{:>6}: {}\n", i, data));
                }
                sse_msg.push_str(&format!("... ({} events omitted) ...\n", total - max));
                for (i, data) in self.buffer.iter().skip(total - tail).enumerate() {
                    sse_msg.push_str(&format!("{:>6}: {}\n", total - tail + i, data));
                }
            }
        }

        error!("{}", sse_msg);
    }
}

/// Placeholder name for tool calls whose name was not received before the stream terminated.
const UNKNOWN_TOOL_PLACEHOLDER: &str = "unknown_tool";

/// Stream processing result
#[derive(Debug, Clone)]
pub struct StreamResult {
    pub full_thinking: String,
    /// Signature of Anthropic extended thinking (passed back in multi-turn conversations)
    pub thinking_signature: Option<String>,
    pub full_text: String,
    pub tool_calls: Vec<ToolCall>,
    /// Token usage statistics (from model response)
    pub usage: Option<GeminiUsage>,
    /// Provider-specific metadata captured from the stream tail.
    pub provider_metadata: Option<Value>,
    /// Whether this stream produced any user-visible output (text/thinking/tool events)
    pub has_effective_output: bool,
    /// When set, the stream terminated abnormally but was recovered with partial output.
    /// Contains a human-readable reason (e.g. "Stream processing error: ..." or "Stream data timeout ...").
    pub partial_recovery_reason: Option<String>,
}

/// Stream processing error with output diagnostics.
#[derive(Debug)]
pub struct StreamProcessError {
    pub error: BitFunError,
    pub has_effective_output: bool,
}

impl StreamProcessError {
    fn new(error: BitFunError, has_effective_output: bool) -> Self {
        Self {
            error,
            has_effective_output,
        }
    }
}

/// Stream processing context, encapsulates state during stream processing
struct StreamContext {
    session_id: String,
    dialog_turn_id: String,
    round_id: String,
    event_subagent_parent_info: Option<EventSubagentParentInfo>,
    subagent_parent_info: Option<SubagentParentInfo>,

    // Accumulated results
    full_thinking: String,
    /// Signature of Anthropic extended thinking (passed back in multi-turn conversations)
    thinking_signature: Option<String>,
    full_text: String,
    tool_calls: Vec<ToolCall>,
    usage: Option<GeminiUsage>,
    provider_metadata: Option<Value>,

    // Current tool call state
    pending_tool_call: PendingToolCall,

    // Counters and flags
    text_chunks_count: usize,
    thinking_chunks_count: usize,
    thinking_completed_sent: bool,
    has_effective_output: bool,
    partial_recovery_reason: Option<String>,
}

impl StreamContext {
    fn new(
        session_id: String,
        dialog_turn_id: String,
        round_id: String,
        subagent_parent_info: Option<SubagentParentInfo>,
    ) -> Self {
        let event_subagent_parent_info = subagent_parent_info.clone().map(|info| info.into());
        Self {
            session_id,
            dialog_turn_id,
            round_id,
            event_subagent_parent_info,
            subagent_parent_info,
            full_thinking: String::new(),
            thinking_signature: None,
            full_text: String::new(),
            tool_calls: Vec::new(),
            usage: None,
            provider_metadata: None,
            pending_tool_call: PendingToolCall::default(),
            text_chunks_count: 0,
            thinking_chunks_count: 0,
            thinking_completed_sent: false,
            has_effective_output: false,
            partial_recovery_reason: None,
        }
    }

    fn into_result(self) -> StreamResult {
        StreamResult {
            full_thinking: self.full_thinking,
            thinking_signature: self.thinking_signature,
            full_text: self.full_text,
            tool_calls: self.tool_calls,
            usage: self.usage,
            provider_metadata: self.provider_metadata,
            has_effective_output: self.has_effective_output,
            partial_recovery_reason: self.partial_recovery_reason,
        }
    }

    fn can_recover_as_partial_result(&self) -> bool {
        self.has_effective_output
    }

    fn finalize_pending_tool_call(
        &mut self,
        boundary: ToolCallBoundary,
    ) -> Option<FinalizedToolCall> {
        let finalized = self.pending_tool_call.finalize(boundary)?;
        let tool_name = if finalized.tool_name.is_empty() {
            UNKNOWN_TOOL_PLACEHOLDER.to_string()
        } else {
            finalized.tool_name.clone()
        };
        let tool_id = if finalized.tool_id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            finalized.tool_id.clone()
        };
        self.tool_calls.push(ToolCall {
            tool_id,
            tool_name,
            arguments: finalized.arguments.clone(),
            is_error: finalized.is_error,
        });
        Some(finalized)
    }

    /// Force finish pending_tool_call, used when the stream is shutting down before a natural tool boundary.
    fn force_finish_pending_tool_call(&mut self) {
        if let Some(finalized) = self.finalize_pending_tool_call(ToolCallBoundary::GracefulShutdown)
        {
            error!(
                "force finish pending tool call: tool_id={}, tool_name={}, raw_len={}, is_error={}",
                finalized.tool_id,
                finalized.tool_name,
                finalized.raw_arguments.len(),
                finalized.is_error
            );
        }
    }
}

/// Stream processor
pub struct StreamProcessor {
    event_queue: Arc<EventQueue>,
}

impl StreamProcessor {
    pub fn new(event_queue: Arc<EventQueue>) -> Self {
        Self { event_queue }
    }

    fn merge_json_value(target: &mut Value, overlay: Value) {
        match (target, overlay) {
            (Value::Object(target_map), Value::Object(overlay_map)) => {
                for (key, value) in overlay_map {
                    let entry = target_map.entry(key).or_insert(Value::Null);
                    Self::merge_json_value(entry, value);
                }
            }
            (target_slot, overlay_value) => {
                *target_slot = overlay_value;
            }
        }
    }

    // ==================== Helper Methods ====================

    /// Send thinking end event (if needed)
    async fn send_thinking_end_if_needed(&self, ctx: &mut StreamContext) {
        if ctx.thinking_chunks_count > 0 && !ctx.thinking_completed_sent {
            ctx.thinking_completed_sent = true;
            debug!("Thinking process ended, sending ThinkingChunk end event");
            let _ = self
                .event_queue
                .enqueue(
                    AgenticEvent::ThinkingChunk {
                        session_id: ctx.session_id.clone(),
                        turn_id: ctx.dialog_turn_id.clone(),
                        round_id: ctx.round_id.clone(),
                        content: String::new(),
                        is_end: true,
                        subagent_parent_info: ctx.event_subagent_parent_info.clone(),
                    },
                    Some(EventPriority::Normal),
                )
                .await;
        }
    }

    /// Check cancellation and execute graceful shutdown, returns Some(Err) if processing needs to be interrupted
    async fn check_cancellation(
        &self,
        ctx: &mut StreamContext,
        cancellation_token: &tokio_util::sync::CancellationToken,
        location: &str,
    ) -> Option<Result<StreamResult, StreamProcessError>> {
        if cancellation_token.is_cancelled() {
            debug!(
                "Cancellation detected at {}: location={}",
                location, location
            );
            self.graceful_shutdown_from_ctx(ctx, "User cancelled stream processing".to_string())
                .await;
            Some(Err(StreamProcessError::new(
                BitFunError::Cancelled("Stream processing cancelled".to_string()),
                ctx.has_effective_output,
            )))
        } else {
            None
        }
    }

    /// Execute graceful shutdown from context
    async fn graceful_shutdown_from_ctx(&self, ctx: &mut StreamContext, reason: String) {
        ctx.force_finish_pending_tool_call();
        self.graceful_shutdown(
            ctx.session_id.clone(),
            ctx.dialog_turn_id.clone(),
            ctx.tool_calls.clone(),
            reason,
            ctx.subagent_parent_info.clone(),
        )
        .await;
    }

    /// Graceful shutdown: cleanup all unfinished tool states and notify frontend
    async fn graceful_shutdown(
        &self,
        session_id: String,
        turn_id: String,
        tool_calls: Vec<ToolCall>,
        reason: String,
        subagent_parent_info: Option<SubagentParentInfo>,
    ) {
        debug!(
            "Starting graceful shutdown: session_id={}, reason={}",
            session_id, reason
        );

        let is_user_cancellation = reason.contains("cancelled") || reason.contains("cancelled");
        let tool_call_count = tool_calls.len();
        let event_subagent_parent_info = subagent_parent_info.map(|info| info.clone().into());

        // 1. Cleanup all tool calls
        for tool_call in tool_calls {
            trace!(
                "Cleaning up tool: {} ({})",
                tool_call.tool_name,
                tool_call.tool_id
            );

            let tool_event = if is_user_cancellation {
                ToolEventData::Cancelled {
                    tool_id: tool_call.tool_id,
                    tool_name: tool_call.tool_name,
                    reason: reason.clone(),
                }
            } else {
                ToolEventData::Failed {
                    tool_id: tool_call.tool_id,
                    tool_name: tool_call.tool_name,
                    error: reason.clone(),
                }
            };

            let _ = self
                .event_queue
                .enqueue(
                    AgenticEvent::ToolEvent {
                        session_id: session_id.clone(),
                        turn_id: turn_id.clone(),
                        tool_event,
                        subagent_parent_info: event_subagent_parent_info.clone(),
                    },
                    Some(EventPriority::High),
                )
                .await;
        }

        // 2. Send dialog turn status update (if tools were cleaned up)
        if tool_call_count > 0 {
            let event = if is_user_cancellation {
                AgenticEvent::DialogTurnCancelled {
                    session_id: session_id.clone(),
                    turn_id: turn_id.clone(),
                    subagent_parent_info: event_subagent_parent_info.clone(),
                }
            } else {
                AgenticEvent::DialogTurnFailed {
                    session_id: session_id.clone(),
                    turn_id: turn_id.clone(),
                    error: reason,
                    subagent_parent_info: event_subagent_parent_info.clone(),
                }
            };
            let _ = self
                .event_queue
                .enqueue(event, Some(EventPriority::Critical))
                .await;
        }

        debug!(
            "Graceful shutdown completed: cleaned up {} tools",
            tool_call_count
        );
    }

    /// Handle usage statistics
    fn handle_usage(
        &self,
        ctx: &mut StreamContext,
        response_usage: &ai_stream_handlers::UnifiedTokenUsage,
    ) {
        ctx.usage = Some(GeminiUsage {
            prompt_token_count: response_usage.prompt_token_count,
            candidates_token_count: response_usage.candidates_token_count,
            total_token_count: response_usage.total_token_count,
            reasoning_token_count: response_usage.reasoning_token_count,
            cached_content_token_count: response_usage.cached_content_token_count,
        });
        debug!(
            "Received token usage stats: input={}, output={}, total={}",
            response_usage.prompt_token_count,
            response_usage.candidates_token_count,
            response_usage.total_token_count
        );
    }

    /// Handle tool call chunk
    async fn handle_tool_call_chunk(
        &self,
        ctx: &mut StreamContext,
        tool_call: ai_stream_handlers::UnifiedToolCall,
    ) {
        let ai_stream_handlers::UnifiedToolCall {
            id,
            name,
            arguments,
        } = tool_call;

        // Handle tool ID and name
        if let Some(tool_id) = id {
            if !tool_id.is_empty() {
                ctx.has_effective_output = true;
                // Some providers repeat the tool id on every delta; only treat a new id as a new tool call.
                let is_new_tool = ctx.pending_tool_call.tool_id() != tool_id;
                if is_new_tool {
                    let _ = ctx.finalize_pending_tool_call(ToolCallBoundary::NewTool);

                    // Normally tool_name should not be empty
                    let tool_name = name.clone().unwrap_or_default();
                    debug!("Tool detected: {}", tool_name);
                    ctx.pending_tool_call
                        .start_new(tool_id.clone(), name.clone());

                    // Send early detection event
                    let _ = self
                        .event_queue
                        .enqueue(
                            AgenticEvent::ToolEvent {
                                session_id: ctx.session_id.clone(),
                                turn_id: ctx.dialog_turn_id.clone(),
                                tool_event: ToolEventData::EarlyDetected { tool_id, tool_name },
                                subagent_parent_info: ctx.event_subagent_parent_info.clone(),
                            },
                            None,
                        )
                        .await;
                } else if ctx.pending_tool_call.tool_name().is_empty() {
                    // Best-effort: keep name if provider repeats it.
                    ctx.pending_tool_call
                        .update_tool_name_if_missing(name.clone());
                }
            }
        }

        // Handle tool parameters
        if let Some(tool_call_arguments) = arguments {
            // Providers often omit tool_id on follow-up argument deltas. Append as long as we already
            // have a pending tool call; otherwise treat this as an orphaned delta and ignore it.
            if ctx.pending_tool_call.has_pending() {
                ctx.has_effective_output = true;
                ctx.pending_tool_call.append_arguments(&tool_call_arguments);

                // Send partial parameters event
                let _ = self
                    .event_queue
                    .enqueue(
                        AgenticEvent::ToolEvent {
                            session_id: ctx.session_id.clone(),
                            turn_id: ctx.dialog_turn_id.clone(),
                            tool_event: ToolEventData::ParamsPartial {
                                tool_id: ctx.pending_tool_call.tool_id().to_string(),
                                tool_name: ctx.pending_tool_call.tool_name().to_string(),
                                params: tool_call_arguments,
                            },
                            subagent_parent_info: ctx.event_subagent_parent_info.clone(),
                        },
                        None,
                    )
                    .await;
            }
        }
    }

    /// Handle text chunk
    async fn handle_text_chunk(&self, ctx: &mut StreamContext, text: String) {
        ctx.has_effective_output = true;
        ctx.full_text.push_str(&text);
        ctx.text_chunks_count += 1;

        // Send streaming text event
        let _ = self
            .event_queue
            .enqueue(
                AgenticEvent::TextChunk {
                    session_id: ctx.session_id.clone(),
                    turn_id: ctx.dialog_turn_id.clone(),
                    round_id: ctx.round_id.clone(),
                    text,
                    subagent_parent_info: ctx.event_subagent_parent_info.clone(),
                },
                None,
            )
            .await;
    }

    /// Handle thinking chunk
    async fn handle_thinking_chunk(&self, ctx: &mut StreamContext, thinking_content: String) {
        // Thinking-only output does NOT count as "effective" for retry purposes:
        // if the stream fails after producing only thinking (no text/tool calls),
        // it is safe to retry because the model will re-think from scratch.
        ctx.full_thinking.push_str(&thinking_content);
        ctx.thinking_chunks_count += 1;

        // Send thinking chunk event
        let _ = self
            .event_queue
            .enqueue(
                AgenticEvent::ThinkingChunk {
                    session_id: ctx.session_id.clone(),
                    turn_id: ctx.dialog_turn_id.clone(),
                    round_id: ctx.round_id.clone(),
                    content: thinking_content,
                    is_end: false,
                    subagent_parent_info: ctx.event_subagent_parent_info.clone(),
                },
                None,
            )
            .await;
    }

    /// Print stream processing end log
    fn log_stream_result(&self, ctx: &StreamContext) {
        debug!(
            "Stream loop ended: text_chunks={}, thinking_chunks={}, tool_calls({}): {}",
            ctx.text_chunks_count,
            ctx.thinking_chunks_count,
            ctx.tool_calls.len(),
            ctx.tool_calls
                .iter()
                .map(|tc| tc.tool_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );

        if log::log_enabled!(log::Level::Debug) {
            if !ctx.full_thinking.is_empty() {
                debug!(target: "ai::stream_processor", "Full thinking content: \n{}", ctx.full_thinking);
            }
            if !ctx.full_text.is_empty() {
                debug!(target: "ai::stream_processor", "Full text content: \n{}", ctx.full_text);
            }
            if !ctx.tool_calls.is_empty() {
                let log_str: String = ctx
                    .tool_calls
                    .iter()
                    .map(|tc| {
                        format!(
                            "Tool name: {}, arguments: {}\n",
                            tc.tool_name,
                            serde_json::to_string(&tc.arguments)
                                .unwrap_or_else(|_| "Serialization failed".to_string())
                        )
                    })
                    .collect();
                debug!(target: "ai::stream_processor", "Tool call details: \n{}", log_str);
            }
        }

        trace!(
            "Returning StreamResult: thinking_len={}, text_len={}, tool_calls={}, has_usage={}, has_effective_output={}",
            ctx.full_thinking.len(),
            ctx.full_text.len(),
            ctx.tool_calls.len(),
            ctx.usage.is_some(),
            ctx.has_effective_output
        );
    }

    // ==================== Main Processing Methods ====================

    /// Process AI streaming response
    ///
    /// # Arguments
    /// * `stream` - Parsed response stream
    /// * `raw_sse_rx` - Optional raw SSE data receiver (for collecting raw data during error diagnosis)
    /// * `session_id` - Session ID
    /// * `dialog_turn_id` - Dialog turn ID
    /// * `round_id` - Model round ID
    /// * `subagent_parent_info` - Subagent parent info
    /// * `cancellation_token` - Cancellation token
    #[allow(clippy::too_many_arguments)]
    pub async fn process_stream(
        &self,
        mut stream: futures::stream::BoxStream<'static, Result<UnifiedResponse, anyhow::Error>>,
        raw_sse_rx: Option<mpsc::UnboundedReceiver<String>>,
        session_id: String,
        dialog_turn_id: String,
        round_id: String,
        subagent_parent_info: Option<SubagentParentInfo>,
        cancellation_token: &tokio_util::sync::CancellationToken,
    ) -> Result<StreamResult, StreamProcessError> {
        let chunk_timeout = std::time::Duration::from_secs(600);
        let mut ctx =
            StreamContext::new(session_id, dialog_turn_id, round_id, subagent_parent_info);
        // Start SSE log collector (if raw_sse_rx is provided)
        let sse_collector = if let Some(mut rx) = raw_sse_rx {
            let collector = Arc::new(tokio::sync::Mutex::new(SseLogCollector::new(
                SseLogConfig::default(), // No limit for now
            )));
            let collector_clone = collector.clone();

            // Start background task to collect SSE data
            tokio::spawn(async move {
                while let Some(data) = rx.recv().await {
                    collector_clone.lock().await.push(data);
                }
            });

            Some(collector)
        } else {
            None
        };

        // Define a helper closure to flush SSE logs on error
        let flush_sse_on_error = |collector: &Option<Arc<tokio::sync::Mutex<SseLogCollector>>>,
                                  error_context: &str| {
            let collector = collector.clone();
            let error_context = error_context.to_string();
            async move {
                if let Some(c) = collector {
                    // Wait a short time for background task to finish collecting data
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    c.lock().await.flush_on_error(&error_context);
                }
            }
        };

        loop {
            tokio::select! {
                // Check cancellation token
                _ = cancellation_token.cancelled() => {
                    debug!("Cancel token detected, stopping stream processing: session_id={}", ctx.session_id);
                    self.graceful_shutdown_from_ctx(&mut ctx, "User cancelled stream processing".to_string()).await;
                    return Err(StreamProcessError::new(
                        BitFunError::Cancelled("Stream processing cancelled".to_string()),
                        ctx.has_effective_output,
                    ));
                }

                // Wait for next chunk (with timeout)
                next_result = tokio::time::timeout(chunk_timeout, stream.next()) => {
                    let response = match next_result {
                        Ok(Some(Ok(response))) => response,
                        Ok(None) => {
                            debug!("Stream ended normally (no more data)");
                            break;
                        }
                        Ok(Some(Err(e))) => {
                            let error_msg = format!("Stream processing error: {}", e);
                            error!("{}", error_msg);
                            if ctx.can_recover_as_partial_result() {
                                flush_sse_on_error(&sse_collector, &error_msg).await;
                                self.send_thinking_end_if_needed(&mut ctx).await;
                                ctx.force_finish_pending_tool_call();
                                ctx.partial_recovery_reason = Some(error_msg.clone());
                                self.log_stream_result(&ctx);
                                break;
                            }
                            // log SSE for network errors
                            flush_sse_on_error(&sse_collector, &error_msg).await;
                            self.graceful_shutdown_from_ctx(&mut ctx, error_msg.clone()).await;
                            return Err(StreamProcessError::new(
                                BitFunError::AIClient(error_msg),
                                ctx.has_effective_output,
                            ));
                        }
                        Err(_) => {
                            let error_msg = format!("Stream data timeout (no data received for {} seconds)", chunk_timeout.as_secs());
                            error!("Stream data timeout ({} seconds), forcing termination", chunk_timeout.as_secs());
                            // log SSE for timeout errors
                            flush_sse_on_error(&sse_collector, &error_msg).await;
                            if ctx.can_recover_as_partial_result() {
                                self.send_thinking_end_if_needed(&mut ctx).await;
                                ctx.force_finish_pending_tool_call();
                                ctx.partial_recovery_reason = Some(error_msg.clone());
                                self.log_stream_result(&ctx);
                                break;
                            }
                            self.graceful_shutdown_from_ctx(&mut ctx, error_msg.clone()).await;
                            return Err(StreamProcessError::new(
                                BitFunError::AIClient(error_msg),
                                ctx.has_effective_output,
                            ));
                        }
                    };

                    let UnifiedResponse {
                        text,
                        reasoning_content,
                        thinking_signature,
                        tool_call,
                        usage,
                        finish_reason,
                        provider_metadata,
                    } = response;

                    // Handle thinking_signature
                    if let Some(signature) = thinking_signature {
                        if !signature.is_empty() {
                            ctx.thinking_signature = Some(signature);
                            trace!("Received thinking_signature");
                        }
                    }

                    // Handle different types of response content
                    // Normalize empty strings to None
                    //  (some models send empty text alongside reasoning content)
                    let text = text.filter(|t| !t.is_empty());
                    let reasoning_content = reasoning_content.filter(|t| !t.is_empty());

                    if let Some(thinking_content) = reasoning_content {
                        self.handle_thinking_chunk(&mut ctx, thinking_content).await;
                        if let Some(err) = self.check_cancellation(&mut ctx, cancellation_token, "processing thinking chunk").await {
                            return err;
                        }
                    }

                    if let Some(text) = text {
                        self.send_thinking_end_if_needed(&mut ctx).await;
                        self.handle_text_chunk(&mut ctx, text).await;
                        if let Some(err) = self.check_cancellation(&mut ctx, cancellation_token, "processing text chunk").await {
                            return err;
                        }
                    }

                    if let Some(tool_call) = tool_call {
                        self.send_thinking_end_if_needed(&mut ctx).await;
                        self.handle_tool_call_chunk(&mut ctx, tool_call).await;
                        if let Some(err) = self.check_cancellation(&mut ctx, cancellation_token, "processing tool call").await {
                            return err;
                        }
                    }

                    if let Some(ref response_usage) = usage {
                        self.handle_usage(&mut ctx, response_usage);
                    }

                    if let Some(provider_metadata) = provider_metadata {
                        match ctx.provider_metadata.as_mut() {
                            Some(existing) => Self::merge_json_value(existing, provider_metadata),
                            None => ctx.provider_metadata = Some(provider_metadata),
                        }
                    }

                    if finish_reason.is_some() {
                        let _ = ctx.finalize_pending_tool_call(ToolCallBoundary::FinishReason);
                    }
                }
            }
        }

        // Ensure thinking end marker is sent
        self.send_thinking_end_if_needed(&mut ctx).await;

        let _ = ctx.finalize_pending_tool_call(ToolCallBoundary::StreamEnd);

        // Invalid tool payloads that survive to finalization still need detailed SSE logs for diagnosis.
        if ctx.tool_calls.iter().any(|tc| !tc.is_valid()) {
            flush_sse_on_error(&sse_collector, "Has invalid tool calls").await;
        }

        self.log_stream_result(&ctx);

        Ok(ctx.into_result())
    }
}

#[cfg(test)]
mod tests {
    use super::StreamProcessor;
    use crate::agentic::events::{EventQueue, EventQueueConfig};
    use ai_stream_handlers::{UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall};
    use futures::StreamExt;
    use serde_json::json;
    use std::sync::Arc;
    use tokio_stream::iter;
    use tokio_util::sync::CancellationToken;

    fn build_processor() -> StreamProcessor {
        StreamProcessor::new(Arc::new(EventQueue::new(EventQueueConfig::default())))
    }

    fn sample_usage(total_tokens: u32) -> UnifiedTokenUsage {
        UnifiedTokenUsage {
            prompt_token_count: 1,
            candidates_token_count: total_tokens.saturating_sub(1),
            total_token_count: total_tokens,
            reasoning_token_count: None,
            cached_content_token_count: None,
        }
    }

    #[tokio::test]
    async fn keeps_collecting_tool_args_across_usage_chunks() {
        let processor = build_processor();
        let stream = iter(vec![
            Ok(UnifiedResponse {
                tool_call: Some(UnifiedToolCall {
                    id: Some("call_1".to_string()),
                    name: Some("tool_a".to_string()),
                    arguments: Some("{\"a\":".to_string()),
                }),
                usage: Some(sample_usage(5)),
                ..Default::default()
            }),
            Ok(UnifiedResponse {
                tool_call: Some(UnifiedToolCall {
                    id: None,
                    name: None,
                    arguments: Some("1}".to_string()),
                }),
                usage: Some(sample_usage(7)),
                ..Default::default()
            }),
        ])
        .boxed();

        let result = processor
            .process_stream(
                stream,
                None,
                "session_1".to_string(),
                "turn_1".to_string(),
                "round_1".to_string(),
                None,
                &CancellationToken::new(),
            )
            .await
            .expect("stream result");

        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].tool_id, "call_1");
        assert_eq!(result.tool_calls[0].tool_name, "tool_a");
        assert_eq!(result.tool_calls[0].arguments, json!({"a": 1}));
        assert!(!result.tool_calls[0].is_error);
        assert_eq!(result.usage.as_ref().map(|u| u.total_token_count), Some(7));
    }

    #[tokio::test]
    async fn finalizes_tool_after_same_chunk_finish_reason() {
        let processor = build_processor();
        let stream = iter(vec![Ok(UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                id: Some("call_1".to_string()),
                name: Some("tool_a".to_string()),
                arguments: Some("{\"a\":1}".to_string()),
            }),
            usage: Some(sample_usage(9)),
            finish_reason: Some("tool_calls".to_string()),
            ..Default::default()
        })])
        .boxed();

        let result = processor
            .process_stream(
                stream,
                None,
                "session_1".to_string(),
                "turn_1".to_string(),
                "round_1".to_string(),
                None,
                &CancellationToken::new(),
            )
            .await
            .expect("stream result");

        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].arguments, json!({"a": 1}));
        assert_eq!(result.usage.as_ref().map(|u| u.total_token_count), Some(9));
    }

    #[tokio::test]
    async fn repairs_tool_args_with_one_extra_trailing_right_brace() {
        let processor = build_processor();
        let stream = iter(vec![Ok(UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                id: Some("call_1".to_string()),
                name: Some("tool_a".to_string()),
                arguments: Some("{\"a\":1}}".to_string()),
            }),
            finish_reason: Some("tool_calls".to_string()),
            ..Default::default()
        })])
        .boxed();

        let result = processor
            .process_stream(
                stream,
                None,
                "session_1".to_string(),
                "turn_1".to_string(),
                "round_1".to_string(),
                None,
                &CancellationToken::new(),
            )
            .await
            .expect("stream result");

        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].tool_id, "call_1");
        assert_eq!(result.tool_calls[0].tool_name, "tool_a");
        assert_eq!(result.tool_calls[0].arguments, json!({"a": 1}));
        assert!(!result.tool_calls[0].is_error);
    }
}
