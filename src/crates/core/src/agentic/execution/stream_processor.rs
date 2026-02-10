//! Stream Processor
//!
//! Processes AI streaming responses, supports tool pre-detection and parameter streaming

use crate::agentic::core::ToolCall;
use crate::agentic::events::{
    AgenticEvent, EventPriority, EventQueue, SubagentParentInfo as EventSubagentParentInfo,
    ToolEventData,
};
use crate::agentic::tools::registry::get_all_end_turn_tool_names;
use crate::agentic::tools::SubagentParentInfo;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::ai::GeminiUsage;
use crate::util::JsonChecker;
use ai_stream_handlers::UnifiedResponse;
use futures::StreamExt;
use log::{debug, error, trace};
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;

//==============================================================================
// SSE Log Collector - Outputs raw SSE data on error
//==============================================================================

/// SSE log collector configuration
#[derive(Debug, Clone)]
pub struct SseLogConfig {
    /// Maximum number of SSE data entries to output on error, None means unlimited
    pub max_output: Option<usize>,
}

impl Default for SseLogConfig {
    fn default() -> Self {
        Self { max_output: None }
    }
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

#[derive(Debug)]
struct ToolCallBuffer {
    tool_id: String,
    tool_name: String,
    json_checker: JsonChecker,
    end_turn_tools: Option<HashSet<String>>,
}

impl ToolCallBuffer {
    fn new() -> Self {
        Self {
            tool_id: String::new(),
            tool_name: String::new(),
            json_checker: JsonChecker::new(),
            end_turn_tools: None,
        }
    }

    pub fn set_end_turn_tools(&mut self, end_turn_tools: HashSet<String>) {
        self.end_turn_tools = Some(end_turn_tools);
    }

    fn reset(&mut self) {
        self.tool_id.clear();
        self.tool_name.clear();
        self.json_checker.reset();
    }

    fn append(&mut self, s: &str) {
        self.json_checker.append(s);
    }

    fn is_valid(&self) -> bool {
        self.json_checker.is_valid()
    }

    fn to_tool_call(&self) -> ToolCall {
        let arguments = serde_json::from_str(&self.json_checker.get_buffer());
        let is_error = arguments.is_err();
        let should_end_turn = self
            .end_turn_tools
            .as_ref()
            .map(|tools| tools.contains(&self.tool_name))
            .unwrap_or(false);
        ToolCall {
            tool_id: self.tool_id.clone(),
            tool_name: self.tool_name.clone(),
            arguments: arguments.unwrap_or(json!({})),
            is_error,
            should_end_turn,
        }
    }
}

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

    // Current tool call state
    tool_call_buffer: ToolCallBuffer,

    // Counters and flags
    text_chunks_count: usize,
    thinking_chunks_count: usize,
    thinking_completed_sent: bool,
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
            tool_call_buffer: ToolCallBuffer::new(),
            text_chunks_count: 0,
            thinking_chunks_count: 0,
            thinking_completed_sent: false,
        }
    }

    fn into_result(self) -> StreamResult {
        StreamResult {
            full_thinking: self.full_thinking,
            thinking_signature: self.thinking_signature,
            full_text: self.full_text,
            tool_calls: self.tool_calls,
            usage: self.usage,
        }
    }

    /// Force finish tool_call_buffer, used to handle cases where toolcall parameters are not fully closed
    /// E.g., when new toolcall arrives and before returning results
    fn force_finish_tool_call_buffer(&mut self) {
        if !self.tool_call_buffer.tool_id.is_empty() {
            error!("force finish tool_call_buffer: {:?}", self.tool_call_buffer);
            // Add to results even if parameters are incomplete, to avoid dialog turn interruption due to no tool calls
            // Caller can detect is_error=true to mark tool execution error
            self.tool_calls.push(self.tool_call_buffer.to_tool_call());
            self.tool_call_buffer.reset();
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

    // ==================== Helper Methods ====================

    /// Send thinking end marker (if needed)
    async fn send_thinking_end_if_needed(&self, ctx: &mut StreamContext) {
        if ctx.thinking_chunks_count > 0 && !ctx.thinking_completed_sent {
            ctx.thinking_completed_sent = true;
            debug!("Thinking process ended, sending ThinkingChunk with end marker");
            let _ = self
                .event_queue
                .enqueue(
                    AgenticEvent::ThinkingChunk {
                        session_id: ctx.session_id.clone(),
                        turn_id: ctx.dialog_turn_id.clone(),
                        round_id: ctx.round_id.clone(),
                        content: "<thinking_end>".to_string(),
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
    ) -> Option<BitFunResult<StreamResult>> {
        if cancellation_token.is_cancelled() {
            debug!(
                "Cancellation detected at {}: location={}",
                location, location
            );
            self.graceful_shutdown_from_ctx(ctx, "User cancelled stream processing".to_string())
                .await;
            Some(Err(BitFunError::Cancelled(
                "Stream processing cancelled".to_string(),
            )))
        } else {
            None
        }
    }

    /// Execute graceful shutdown from context
    async fn graceful_shutdown_from_ctx(&self, ctx: &mut StreamContext, reason: String) {
        ctx.force_finish_tool_call_buffer();
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
        // Handle tool ID and name
        if let Some(tool_id) = tool_call.id {
            if !tool_id.is_empty() {
                // Clear previous tool_call state
                ctx.force_finish_tool_call_buffer();

                // Normally tool_name should not be empty
                let tool_name = tool_call.name.unwrap_or_default();
                debug!("Tool detected: {}", tool_name);
                ctx.tool_call_buffer.tool_id = tool_id.clone();
                ctx.tool_call_buffer.tool_name = tool_name.clone();
                ctx.tool_call_buffer.json_checker.reset();

                // Send early detection event
                let _ = self
                    .event_queue
                    .enqueue(
                        AgenticEvent::ToolEvent {
                            session_id: ctx.session_id.clone(),
                            turn_id: ctx.dialog_turn_id.clone(),
                            tool_event: ToolEventData::EarlyDetected {
                                tool_id: tool_id,
                                tool_name: tool_name,
                            },
                            subagent_parent_info: ctx.event_subagent_parent_info.clone(),
                        },
                        Some(EventPriority::Normal),
                    )
                    .await;
            }
        }

        // Handle tool parameters
        if let Some(tool_call_arguments) = tool_call.arguments {
            // Empty tool_id indicates abnormal premature closure, stop processing subsequent data for this tool_call
            if !ctx.tool_call_buffer.tool_id.is_empty() {
                ctx.tool_call_buffer.append(&tool_call_arguments);

                // Send partial parameters event
                let _ = self
                    .event_queue
                    .enqueue(
                        AgenticEvent::ToolEvent {
                            session_id: ctx.session_id.clone(),
                            turn_id: ctx.dialog_turn_id.clone(),
                            tool_event: ToolEventData::ParamsPartial {
                                tool_id: ctx.tool_call_buffer.tool_id.clone(),
                                tool_name: ctx.tool_call_buffer.tool_name.clone(),
                                params: tool_call_arguments,
                            },
                            subagent_parent_info: ctx.event_subagent_parent_info.clone(),
                        },
                        Some(EventPriority::Normal),
                    )
                    .await;
            }
        }

        // Check if JSON is complete
        if ctx.tool_call_buffer.is_valid() {
            ctx.tool_calls.push(ctx.tool_call_buffer.to_tool_call());

            // Clear buffer
            // Normally there should be no delta data after parameters are complete, but this has been triggered in practice, possibly due to network issues or model output anomalies
            // reset clears the id, subsequent data for this tool_call will not be processed
            ctx.tool_call_buffer.reset();
        }
    }

    /// Handle text chunk
    async fn handle_text_chunk(&self, ctx: &mut StreamContext, text: String) {
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
                Some(EventPriority::Normal),
            )
            .await;
    }

    /// Handle thinking chunk
    async fn handle_thinking_chunk(&self, ctx: &mut StreamContext, thinking_content: String) {
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
                    subagent_parent_info: ctx.event_subagent_parent_info.clone(),
                },
                Some(EventPriority::Normal),
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
            "Returning StreamResult: thinking_len={}, text_len={}, tool_calls={}, has_usage={}",
            ctx.full_thinking.len(),
            ctx.full_text.len(),
            ctx.tool_calls.len(),
            ctx.usage.is_some()
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
    pub async fn process_stream(
        &self,
        mut stream: futures::stream::BoxStream<'static, Result<UnifiedResponse, anyhow::Error>>,
        raw_sse_rx: Option<mpsc::UnboundedReceiver<String>>,
        session_id: String,
        dialog_turn_id: String,
        round_id: String,
        subagent_parent_info: Option<SubagentParentInfo>,
        cancellation_token: &tokio_util::sync::CancellationToken,
    ) -> BitFunResult<StreamResult> {
        let chunk_timeout = std::time::Duration::from_secs(600);
        let mut ctx =
            StreamContext::new(session_id, dialog_turn_id, round_id, subagent_parent_info);
        let end_turn_tools = get_all_end_turn_tool_names().await.into_iter().collect();
        ctx.tool_call_buffer.set_end_turn_tools(end_turn_tools);

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
                    return Err(BitFunError::Cancelled("Stream processing cancelled".to_string()));
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
                            // Network errors/timeouts don't log SSE
                            // flush_sse_on_error(&sse_collector, &error_msg).await;
                            self.graceful_shutdown_from_ctx(&mut ctx, error_msg.clone()).await;
                            return Err(BitFunError::AIClient(error_msg));
                        }
                        Err(_) => {
                            let error_msg = format!("Stream data timeout (no data received for {} seconds)", chunk_timeout.as_secs());
                            error!("Stream data timeout ({} seconds), forcing termination", chunk_timeout.as_secs());
                            self.graceful_shutdown_from_ctx(&mut ctx, error_msg.clone()).await;
                            return Err(BitFunError::AIClient(error_msg));
                        }
                    };

                    // Handle usage
                    if let Some(ref response_usage) = response.usage {
                        self.handle_usage(&mut ctx, response_usage);
                    }

                    // Handle thinking_signature
                    if let Some(signature) = response.thinking_signature {
                        if !signature.is_empty() {
                            ctx.thinking_signature = Some(signature);
                            trace!("Received thinking_signature");
                        }
                    }

                    // Handle different types of response content
                    // Normalize empty strings to None so the if-else chain correctly
                    // falls through (some models send empty text alongside reasoning content)
                    let text = response.text.filter(|t| !t.is_empty());
                    let reasoning_content = response.reasoning_content.filter(|t| !t.is_empty());

                    if let Some(tool_call) = response.tool_call {
                        self.send_thinking_end_if_needed(&mut ctx).await;
                        if let Some(err) = self.check_cancellation(&mut ctx, cancellation_token, "processing tool call").await {
                            return err;
                        }
                        self.handle_tool_call_chunk(&mut ctx, tool_call).await;
                    } else if let Some(text) = text {
                        self.send_thinking_end_if_needed(&mut ctx).await;
                        self.handle_text_chunk(&mut ctx, text).await;
                        if let Some(err) = self.check_cancellation(&mut ctx, cancellation_token, "processing text chunk").await {
                            return err;
                        }
                    } else if let Some(thinking_content) = reasoning_content {
                        self.handle_thinking_chunk(&mut ctx, thinking_content).await;
                        if let Some(err) = self.check_cancellation(&mut ctx, cancellation_token, "processing thinking chunk").await {
                            return err;
                        }
                    }
                }
            }
        }

        // Ensure thinking end marker is sent
        self.send_thinking_end_if_needed(&mut ctx).await;

        // Check if tool parameters are complete, flush SSE logs if incomplete
        // Incomplete parameters that still occur under normal network conditions need detailed logging for problem diagnosis
        let has_incomplete_tool = ctx.tool_calls.iter().any(|tc| !tc.is_valid());
        if has_incomplete_tool {
            flush_sse_on_error(&sse_collector, "Has incomplete tool calls").await;
        }

        ctx.force_finish_tool_call_buffer();
        self.log_stream_result(&ctx);

        Ok(ctx.into_result())
    }
}
