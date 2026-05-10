//! Stream Processor
//!
//! Processes AI streaming responses, supports tool pre-detection and parameter streaming

use bitfun_ai_adapters::tool_call_accumulator::{
    FinalizedToolCall, PendingToolCalls, ToolCallBoundary, ToolCallStreamKey,
};
use bitfun_ai_adapters::{GeminiUsage, UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall};
use bitfun_events::{
    AgenticEvent, AgenticEventPriority as EventPriority,
    SubagentParentInfo as EventSubagentParentInfo, ToolEventData,
};
use futures::{Stream, StreamExt};
use log::{debug, error, trace};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

/// Minimal tool-call value emitted by the stream processor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    /// Original provider-emitted argument JSON, preserved for replay stability when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_arguments: Option<String>,
    /// Record whether tool parameters are valid.
    pub is_error: bool,
    /// True when truncated raw JSON arguments were repaired into a partial tool call.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recovered_from_truncation: bool,
}

impl ToolCall {
    pub fn is_valid(&self) -> bool {
        !self.tool_id.is_empty() && !self.tool_name.is_empty() && !self.is_error
    }
}

/// Parent task metadata needed to scope stream events for subagents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentParentInfo {
    #[serde(rename = "toolCallId")]
    pub tool_call_id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "dialogTurnId")]
    pub dialog_turn_id: String,
}

impl From<SubagentParentInfo> for EventSubagentParentInfo {
    fn from(info: SubagentParentInfo) -> Self {
        Self {
            tool_call_id: info.tool_call_id,
            session_id: info.session_id,
            dialog_turn_id: info.dialog_turn_id,
        }
    }
}

/// Stream-processor specific error that avoids depending on core runtime errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamProcessorError {
    AiClient(String),
    Cancelled(String),
}

impl fmt::Display for StreamProcessorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AiClient(msg) => write!(f, "AI client error: {}", msg),
            Self::Cancelled(msg) => write!(f, "Operation cancelled: {}", msg),
        }
    }
}

impl std::error::Error for StreamProcessorError {}

/// Event sink abstraction used by stream processing. Product crates can adapt
/// their own queue implementation without making this crate depend on core.
#[async_trait::async_trait]
pub trait StreamEventSink: Send + Sync {
    async fn enqueue(&self, event: AgenticEvent, priority: Option<EventPriority>);
}

fn elapsed_ms_u64(started_at: Instant) -> u64 {
    started_at
        .elapsed()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

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
    /// Whether the provider emitted a reasoning/thinking field even if its content was empty.
    pub reasoning_content_present: bool,
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
    /// Milliseconds from stream processing start to the first upstream response item.
    pub first_chunk_ms: Option<u64>,
    /// Milliseconds from stream processing start to the first event visible to the UI.
    pub first_visible_output_ms: Option<u64>,
    /// When set, the stream terminated abnormally but was recovered with partial output.
    /// Contains a human-readable reason (e.g. "Stream processing error: ..." or
    /// "Stream processor watchdog timeout ...").
    pub partial_recovery_reason: Option<String>,
}

/// Stream processing error with output diagnostics.
#[derive(Debug)]
pub struct StreamProcessError {
    pub error: StreamProcessorError,
    pub has_effective_output: bool,
}

impl StreamProcessError {
    fn new(error: StreamProcessorError, has_effective_output: bool) -> Self {
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
    reasoning_content_present: bool,
    /// Signature of Anthropic extended thinking (passed back in multi-turn conversations)
    thinking_signature: Option<String>,
    full_text: String,
    tool_calls: Vec<ToolCall>,
    usage: Option<GeminiUsage>,
    provider_metadata: Option<Value>,

    // Current tool call state
    pending_tool_calls: PendingToolCalls,

    // Counters and flags
    stream_started_at: Instant,
    first_chunk_ms: Option<u64>,
    first_visible_output_ms: Option<u64>,
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
            reasoning_content_present: false,
            thinking_signature: None,
            full_text: String::new(),
            tool_calls: Vec::new(),
            usage: None,
            provider_metadata: None,
            pending_tool_calls: PendingToolCalls::default(),
            stream_started_at: Instant::now(),
            first_chunk_ms: None,
            first_visible_output_ms: None,
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
            reasoning_content_present: self.reasoning_content_present,
            thinking_signature: self.thinking_signature,
            full_text: self.full_text,
            tool_calls: self.tool_calls,
            usage: self.usage,
            provider_metadata: self.provider_metadata,
            has_effective_output: self.has_effective_output,
            first_chunk_ms: self.first_chunk_ms,
            first_visible_output_ms: self.first_visible_output_ms,
            partial_recovery_reason: self.partial_recovery_reason,
        }
    }

    fn mark_first_stream_chunk(&mut self) {
        if self.first_chunk_ms.is_none() {
            self.first_chunk_ms = Some(elapsed_ms_u64(self.stream_started_at));
        }
    }

    fn mark_first_visible_output(&mut self) {
        if self.first_visible_output_ms.is_none() {
            self.first_visible_output_ms = Some(elapsed_ms_u64(self.stream_started_at));
        }
    }

    fn can_recover_as_partial_result(&self) -> bool {
        self.has_effective_output
    }

    fn record_finalized_tool_call(&mut self, finalized: &FinalizedToolCall) {
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
            raw_arguments: (!finalized.raw_arguments.is_empty())
                .then_some(finalized.raw_arguments.clone()),
            is_error: finalized.is_error,
            recovered_from_truncation: finalized.recovered_from_truncation,
        });
    }

    fn finalize_all_pending_tool_calls(
        &mut self,
        boundary: ToolCallBoundary,
    ) -> Vec<FinalizedToolCall> {
        let finalized = self.pending_tool_calls.finalize_all(boundary);
        for tool_call in &finalized {
            self.record_finalized_tool_call(tool_call);
        }
        finalized
    }

    /// Force finish pending tool calls, used when the stream is shutting down before a natural tool boundary.
    fn force_finish_pending_tool_calls(&mut self) {
        for finalized in self.finalize_all_pending_tool_calls(ToolCallBoundary::GracefulShutdown) {
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

enum TimedStreamItem<T> {
    Item(T),
    End,
    TimedOut,
}

async fn next_stream_item<S>(
    stream: &mut S,
    watchdog_timeout: Option<std::time::Duration>,
) -> TimedStreamItem<S::Item>
where
    S: Stream + Unpin,
{
    match watchdog_timeout {
        Some(timeout) => match tokio::time::timeout(timeout, stream.next()).await {
            Ok(Some(item)) => TimedStreamItem::Item(item),
            Ok(None) => TimedStreamItem::End,
            Err(_) => TimedStreamItem::TimedOut,
        },
        None => match stream.next().await {
            Some(item) => TimedStreamItem::Item(item),
            None => TimedStreamItem::End,
        },
    }
}

/// Stream processor
pub struct StreamProcessor {
    event_sink: Arc<dyn StreamEventSink>,
}

impl StreamProcessor {
    const WATCHDOG_GRACE_SECS: u64 = 5;

    pub fn new<E>(event_sink: Arc<E>) -> Self
    where
        E: StreamEventSink + 'static,
    {
        Self { event_sink }
    }

    pub fn derive_watchdog_timeout(
        stream_idle_timeout: Option<std::time::Duration>,
    ) -> Option<std::time::Duration> {
        stream_idle_timeout.map(|timeout| {
            timeout
                .checked_add(std::time::Duration::from_secs(Self::WATCHDOG_GRACE_SECS))
                .unwrap_or(std::time::Duration::MAX)
        })
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
                .event_sink
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
                StreamProcessorError::Cancelled("Stream processing cancelled".to_string()),
                ctx.has_effective_output,
            )))
        } else {
            None
        }
    }

    /// Execute graceful shutdown from context
    async fn graceful_shutdown_from_ctx(&self, ctx: &mut StreamContext, reason: String) {
        ctx.force_finish_pending_tool_calls();
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
                    duration_ms: None,
                    queue_wait_ms: None,
                    preflight_ms: None,
                    confirmation_wait_ms: None,
                    execution_ms: None,
                }
            } else {
                ToolEventData::Failed {
                    tool_id: tool_call.tool_id,
                    tool_name: tool_call.tool_name,
                    error: reason.clone(),
                    duration_ms: None,
                    queue_wait_ms: None,
                    preflight_ms: None,
                    confirmation_wait_ms: None,
                    execution_ms: None,
                }
            };

            let _ = self
                .event_sink
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
                    error_category: None,
                    error_detail: None,
                    subagent_parent_info: event_subagent_parent_info.clone(),
                }
            };
            let _ = self
                .event_sink
                .enqueue(event, Some(EventPriority::Critical))
                .await;
        }

        debug!(
            "Graceful shutdown completed: cleaned up {} tools",
            tool_call_count
        );
    }

    /// Handle usage statistics
    fn handle_usage(&self, ctx: &mut StreamContext, response_usage: &UnifiedTokenUsage) {
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
    async fn handle_tool_call_chunk(&self, ctx: &mut StreamContext, tool_call: UnifiedToolCall) {
        let UnifiedToolCall {
            tool_call_index,
            id,
            name,
            arguments,
            arguments_is_snapshot,
        } = tool_call;
        let outcome = ctx.pending_tool_calls.apply_delta(
            ToolCallStreamKey::from(tool_call_index),
            id,
            name,
            arguments,
            arguments_is_snapshot,
        );

        if let Some(finalized) = outcome.finalized_previous {
            ctx.record_finalized_tool_call(&finalized);
        }

        if let Some(early_detected) = outcome.early_detected {
            ctx.has_effective_output = true;
            ctx.mark_first_visible_output();
            debug!("Tool detected: {}", early_detected.tool_name);
            let _ = self
                .event_sink
                .enqueue(
                    AgenticEvent::ToolEvent {
                        session_id: ctx.session_id.clone(),
                        turn_id: ctx.dialog_turn_id.clone(),
                        tool_event: ToolEventData::EarlyDetected {
                            tool_id: early_detected.tool_id,
                            tool_name: early_detected.tool_name,
                        },
                        subagent_parent_info: ctx.event_subagent_parent_info.clone(),
                    },
                    None,
                )
                .await;
        }

        if let Some(params_partial) = outcome.params_partial {
            ctx.has_effective_output = true;
            ctx.mark_first_visible_output();
            let _ = self
                .event_sink
                .enqueue(
                    AgenticEvent::ToolEvent {
                        session_id: ctx.session_id.clone(),
                        turn_id: ctx.dialog_turn_id.clone(),
                        tool_event: ToolEventData::ParamsPartial {
                            tool_id: params_partial.tool_id,
                            tool_name: params_partial.tool_name,
                            params: params_partial.params_chunk,
                        },
                        subagent_parent_info: ctx.event_subagent_parent_info.clone(),
                    },
                    None,
                )
                .await;
        }
    }

    /// Handle text chunk
    async fn handle_text_chunk(&self, ctx: &mut StreamContext, text: String) {
        if !text.trim().is_empty() {
            ctx.has_effective_output = true;
            ctx.mark_first_visible_output();
        }
        ctx.full_text.push_str(&text);
        ctx.text_chunks_count += 1;

        // Send streaming text event
        let _ = self
            .event_sink
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
        ctx.mark_first_visible_output();
        ctx.thinking_chunks_count += 1;

        // Send thinking chunk event
        let _ = self
            .event_sink
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
            "Stream loop ended: text_chunks={}, thinking_chunks={}, tool_calls({}), first_chunk_ms={:?}, first_visible_output_ms={:?}: {}",
            ctx.text_chunks_count,
            ctx.thinking_chunks_count,
            ctx.tool_calls.len(),
            ctx.first_chunk_ms,
            ctx.first_visible_output_ms,
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
        watchdog_timeout: Option<std::time::Duration>,
        raw_sse_rx: Option<mpsc::UnboundedReceiver<String>>,
        session_id: String,
        dialog_turn_id: String,
        round_id: String,
        subagent_parent_info: Option<SubagentParentInfo>,
        cancellation_token: &tokio_util::sync::CancellationToken,
    ) -> Result<StreamResult, StreamProcessError> {
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
                        StreamProcessorError::Cancelled("Stream processing cancelled".to_string()),
                        ctx.has_effective_output,
                    ));
                }

                // Watch the adapter -> processor stream only when the upstream stream idle timeout is configured.
                next_result = next_stream_item(&mut stream, watchdog_timeout) => {
                    let response = match next_result {
                        TimedStreamItem::Item(Ok(response)) => response,
                        TimedStreamItem::End => {
                            debug!("Stream ended normally (no more data)");
                            break;
                        }
                        TimedStreamItem::Item(Err(e)) => {
                            let error_msg = format!("Stream processing error: {}", e);
                            error!("{}", error_msg);
                            let non_recoverable_stream_error =
                                error_msg.contains("SSE Parsing Error");
                            if !non_recoverable_stream_error && ctx.can_recover_as_partial_result()
                            {
                                flush_sse_on_error(&sse_collector, &error_msg).await;
                                self.send_thinking_end_if_needed(&mut ctx).await;
                                ctx.force_finish_pending_tool_calls();
                                ctx.partial_recovery_reason = Some(error_msg.clone());
                                self.log_stream_result(&ctx);
                                break;
                            }
                            // log SSE for network errors
                            flush_sse_on_error(&sse_collector, &error_msg).await;
                            self.graceful_shutdown_from_ctx(&mut ctx, error_msg.clone()).await;
                            return Err(StreamProcessError::new(
                                StreamProcessorError::AiClient(error_msg),
                                ctx.has_effective_output,
                            ));
                        }
                        TimedStreamItem::TimedOut => {
                            let timeout_secs =
                                watchdog_timeout.map(|timeout| timeout.as_secs()).unwrap_or(0);
                            let error_msg = format!(
                                "Stream processor watchdog timeout (no data received for {} seconds)",
                                timeout_secs
                            );
                            error!(
                                "Stream processor watchdog timeout ({} seconds), forcing termination",
                                timeout_secs
                            );
                            // log SSE for timeout errors
                            flush_sse_on_error(&sse_collector, &error_msg).await;
                            if ctx.can_recover_as_partial_result() {
                                self.send_thinking_end_if_needed(&mut ctx).await;
                                ctx.force_finish_pending_tool_calls();
                                ctx.partial_recovery_reason = Some(error_msg.clone());
                                self.log_stream_result(&ctx);
                                break;
                            }
                            self.graceful_shutdown_from_ctx(&mut ctx, error_msg.clone()).await;
                            return Err(StreamProcessError::new(
                                StreamProcessorError::AiClient(error_msg),
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
                    ctx.mark_first_stream_chunk();

                    // Handle thinking_signature
                    if let Some(signature) = thinking_signature {
                        if !signature.is_empty() {
                            ctx.reasoning_content_present = true;
                            ctx.thinking_signature = Some(signature);
                            trace!("Received thinking_signature");
                        }
                    }

                    // Handle different types of response content
                    // Normalize empty strings to None
                    //  (some models send empty text alongside reasoning content)
                    let text = text.filter(|t| !t.is_empty());

                    if let Some(thinking_content) = reasoning_content {
                        ctx.reasoning_content_present = true;
                        if !thinking_content.is_empty() {
                            self.handle_thinking_chunk(&mut ctx, thinking_content).await;
                            if let Some(err) = self.check_cancellation(&mut ctx, cancellation_token, "processing thinking chunk").await {
                                return err;
                            }
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
                        let _ = ctx.finalize_all_pending_tool_calls(ToolCallBoundary::FinishReason);
                    }
                }
            }
        }

        // Ensure thinking end marker is sent
        self.send_thinking_end_if_needed(&mut ctx).await;

        let _ = ctx.finalize_all_pending_tool_calls(ToolCallBoundary::StreamEnd);

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
    use super::{StreamEventSink, StreamProcessor};
    use bitfun_ai_adapters::{UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall};
    use bitfun_events::{AgenticEvent, AgenticEventPriority as EventPriority};
    use futures::StreamExt;
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio_stream::iter;
    use tokio_util::sync::CancellationToken;

    struct NoopEventSink;

    #[async_trait::async_trait]
    impl StreamEventSink for NoopEventSink {
        async fn enqueue(&self, _event: AgenticEvent, _priority: Option<EventPriority>) {}
    }

    fn build_processor() -> StreamProcessor {
        StreamProcessor::new(Arc::new(NoopEventSink))
    }

    #[test]
    fn derives_watchdog_timeout_from_stream_idle_timeout() {
        assert_eq!(StreamProcessor::derive_watchdog_timeout(None), None);
        assert_eq!(
            StreamProcessor::derive_watchdog_timeout(Some(Duration::from_secs(10))),
            Some(Duration::from_secs(15))
        );
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
                    tool_call_index: None,
                    id: Some("call_1".to_string()),
                    name: Some("tool_a".to_string()),
                    arguments: Some("{\"a\":".to_string()),
                    arguments_is_snapshot: false,
                }),
                usage: Some(sample_usage(5)),
                ..Default::default()
            }),
            Ok(UnifiedResponse {
                tool_call: Some(UnifiedToolCall {
                    tool_call_index: None,
                    id: None,
                    name: None,
                    arguments: Some("1}".to_string()),
                    arguments_is_snapshot: false,
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
        assert_eq!(
            result.tool_calls[0].raw_arguments.as_deref(),
            Some("{\"a\":1}")
        );
        assert!(!result.tool_calls[0].is_error);
        assert_eq!(result.usage.as_ref().map(|u| u.total_token_count), Some(7));
    }

    #[tokio::test]
    async fn whitespace_only_text_is_not_effective_output() {
        let processor = build_processor();
        let stream = iter(vec![Ok(UnifiedResponse {
            text: Some("\n\n ".to_string()),
            ..Default::default()
        })])
        .boxed();

        let result = processor
            .process_stream(
                stream,
                None,
                None,
                "session_1".to_string(),
                "turn_1".to_string(),
                "round_1".to_string(),
                None,
                &CancellationToken::new(),
            )
            .await
            .expect("stream result");

        assert_eq!(result.full_text, "\n\n ");
        assert!(!result.has_effective_output);
        assert_eq!(result.first_visible_output_ms, None);
    }

    #[tokio::test]
    async fn finalizes_tool_after_same_chunk_finish_reason() {
        let processor = build_processor();
        let stream = iter(vec![Ok(UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                tool_call_index: None,
                id: Some("call_1".to_string()),
                name: Some("tool_a".to_string()),
                arguments: Some("{\"a\":1}".to_string()),
                arguments_is_snapshot: false,
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
    async fn does_not_repair_tool_args_with_one_extra_trailing_right_brace() {
        let processor = build_processor();
        let stream = iter(vec![Ok(UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                tool_call_index: None,
                id: Some("call_1".to_string()),
                name: Some("tool_a".to_string()),
                arguments: Some("{\"a\":1}}".to_string()),
                arguments_is_snapshot: false,
            }),
            finish_reason: Some("tool_calls".to_string()),
            ..Default::default()
        })])
        .boxed();

        let result = processor
            .process_stream(
                stream,
                None,
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
        assert_eq!(result.tool_calls[0].arguments, json!({}));
        assert_eq!(
            result.tool_calls[0].raw_arguments.as_deref(),
            Some("{\"a\":1}}")
        );
        assert!(result.tool_calls[0].is_error);
    }

    #[tokio::test]
    async fn replaces_tool_args_when_snapshot_chunk_arrives() {
        let processor = build_processor();
        let stream = iter(vec![
            Ok(UnifiedResponse {
                tool_call: Some(UnifiedToolCall {
                    tool_call_index: None,
                    id: Some("call_1".to_string()),
                    name: Some("tool_a".to_string()),
                    arguments: Some("{\"city\":\"Bei".to_string()),
                    arguments_is_snapshot: false,
                }),
                ..Default::default()
            }),
            Ok(UnifiedResponse {
                tool_call: Some(UnifiedToolCall {
                    tool_call_index: None,
                    id: None,
                    name: None,
                    arguments: Some("{\"city\":\"Beijing\"}".to_string()),
                    arguments_is_snapshot: true,
                }),
                finish_reason: Some("tool_calls".to_string()),
                ..Default::default()
            }),
        ])
        .boxed();

        let result = processor
            .process_stream(
                stream,
                None,
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
        assert_eq!(result.tool_calls[0].arguments, json!({"city": "Beijing"}));
        assert_eq!(
            result.tool_calls[0].raw_arguments.as_deref(),
            Some("{\"city\":\"Beijing\"}")
        );
        assert!(!result.tool_calls[0].is_error);
    }

    #[tokio::test]
    async fn keeps_interleaved_indexed_tool_calls_separate() {
        let processor = build_processor();
        let stream = iter(vec![
            Ok(UnifiedResponse {
                tool_call: Some(UnifiedToolCall {
                    tool_call_index: Some(0),
                    id: Some("call_0".to_string()),
                    name: Some("tool_a".to_string()),
                    arguments: None,
                    arguments_is_snapshot: false,
                }),
                ..Default::default()
            }),
            Ok(UnifiedResponse {
                tool_call: Some(UnifiedToolCall {
                    tool_call_index: Some(1),
                    id: Some("call_1".to_string()),
                    name: Some("tool_b".to_string()),
                    arguments: None,
                    arguments_is_snapshot: false,
                }),
                ..Default::default()
            }),
            Ok(UnifiedResponse {
                tool_call: Some(UnifiedToolCall {
                    tool_call_index: Some(0),
                    id: None,
                    name: None,
                    arguments: Some("{\"a\":1}".to_string()),
                    arguments_is_snapshot: false,
                }),
                ..Default::default()
            }),
            Ok(UnifiedResponse {
                tool_call: Some(UnifiedToolCall {
                    tool_call_index: Some(1),
                    id: None,
                    name: None,
                    arguments: Some("{\"b\":2}".to_string()),
                    arguments_is_snapshot: false,
                }),
                finish_reason: Some("tool_calls".to_string()),
                ..Default::default()
            }),
        ])
        .boxed();

        let result = processor
            .process_stream(
                stream,
                None,
                None,
                "session_1".to_string(),
                "turn_1".to_string(),
                "round_1".to_string(),
                None,
                &CancellationToken::new(),
            )
            .await
            .expect("stream result");

        assert_eq!(result.tool_calls.len(), 2);
        assert_eq!(result.tool_calls[0].tool_id, "call_0");
        assert_eq!(result.tool_calls[0].tool_name, "tool_a");
        assert_eq!(result.tool_calls[0].arguments, json!({"a": 1}));
        assert_eq!(result.tool_calls[1].tool_id, "call_1");
        assert_eq!(result.tool_calls[1].tool_name, "tool_b");
        assert_eq!(result.tool_calls[1].arguments, json!({"b": 2}));
    }

    #[tokio::test]
    async fn preserves_empty_reasoning_presence_for_replay() {
        let processor = build_processor();
        let stream = iter(vec![Ok(UnifiedResponse {
            reasoning_content: Some(String::new()),
            finish_reason: Some("stop".to_string()),
            ..Default::default()
        })])
        .boxed();

        let result = processor
            .process_stream(
                stream,
                None,
                None,
                "session_1".to_string(),
                "turn_1".to_string(),
                "round_1".to_string(),
                None,
                &CancellationToken::new(),
            )
            .await
            .expect("stream result");

        assert!(result.reasoning_content_present);
        assert!(result.full_thinking.is_empty());
        assert!(!result.has_effective_output);
    }
}
