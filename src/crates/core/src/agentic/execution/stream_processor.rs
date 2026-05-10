//! Compatibility wrapper for the extracted agent stream processor.

use crate::agentic::core::ToolCall;
use crate::agentic::events::EventQueue;
use crate::agentic::tools::SubagentParentInfo;
use crate::util::errors::BitFunError;
use crate::util::types::ai::GeminiUsage;
use futures::stream::BoxStream;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub use bitfun_agent_stream::{StreamProcessorError, ToolCall as StreamToolCall};

/// Stream processing result exposed through bitfun-core compatibility types.
#[derive(Debug, Clone)]
pub struct StreamResult {
    pub full_thinking: String,
    pub reasoning_content_present: bool,
    pub thinking_signature: Option<String>,
    pub full_text: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<GeminiUsage>,
    pub provider_metadata: Option<Value>,
    pub has_effective_output: bool,
    pub first_chunk_ms: Option<u64>,
    pub first_visible_output_ms: Option<u64>,
    pub partial_recovery_reason: Option<String>,
}

impl From<bitfun_agent_stream::StreamResult> for StreamResult {
    fn from(result: bitfun_agent_stream::StreamResult) -> Self {
        Self {
            full_thinking: result.full_thinking,
            reasoning_content_present: result.reasoning_content_present,
            thinking_signature: result.thinking_signature,
            full_text: result.full_text,
            tool_calls: result.tool_calls.into_iter().map(Into::into).collect(),
            usage: result.usage,
            provider_metadata: result.provider_metadata,
            has_effective_output: result.has_effective_output,
            first_chunk_ms: result.first_chunk_ms,
            first_visible_output_ms: result.first_visible_output_ms,
            partial_recovery_reason: result.partial_recovery_reason,
        }
    }
}

/// Stream processing error exposed through bitfun-core compatibility errors.
#[derive(Debug)]
pub struct StreamProcessError {
    pub error: BitFunError,
    pub has_effective_output: bool,
}

impl From<bitfun_agent_stream::StreamProcessError> for StreamProcessError {
    fn from(error: bitfun_agent_stream::StreamProcessError) -> Self {
        Self {
            error: error.error.into(),
            has_effective_output: error.has_effective_output,
        }
    }
}

/// Core-facing stream processor wrapper.
pub struct StreamProcessor {
    inner: bitfun_agent_stream::StreamProcessor,
}

impl StreamProcessor {
    pub fn new(event_queue: Arc<EventQueue>) -> Self {
        Self {
            inner: bitfun_agent_stream::StreamProcessor::new(event_queue),
        }
    }

    pub fn derive_watchdog_timeout(stream_idle_timeout: Option<Duration>) -> Option<Duration> {
        bitfun_agent_stream::StreamProcessor::derive_watchdog_timeout(stream_idle_timeout)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn process_stream(
        &self,
        stream: BoxStream<'static, Result<bitfun_ai_adapters::UnifiedResponse, anyhow::Error>>,
        watchdog_timeout: Option<Duration>,
        raw_sse_rx: Option<mpsc::UnboundedReceiver<String>>,
        session_id: String,
        dialog_turn_id: String,
        round_id: String,
        subagent_parent_info: Option<SubagentParentInfo>,
        cancellation_token: &CancellationToken,
    ) -> Result<StreamResult, StreamProcessError> {
        self.inner
            .process_stream(
                stream,
                watchdog_timeout,
                raw_sse_rx,
                session_id,
                dialog_turn_id,
                round_id,
                subagent_parent_info.map(Into::into),
                cancellation_token,
            )
            .await
            .map(Into::into)
            .map_err(Into::into)
    }
}
