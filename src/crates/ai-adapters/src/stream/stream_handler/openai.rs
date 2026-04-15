use super::stream_stats::StreamStats;
use crate::stream::types::openai::{OpenAISSEData, OpenAIToolCallArgumentsNormalizer};
use crate::stream::types::unified::{UnifiedResponse, UnifiedTokenUsage};
use anyhow::{anyhow, Result};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use log::{error, trace, warn};
use reqwest::Response;
use serde_json::Value;
use std::collections::HashSet;
use std::mem;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

const OPENAI_CHAT_COMPLETION_CHUNK_OBJECT: &str = "chat.completion.chunk";
const INLINE_THINK_OPEN_TAG: &str = "<think>";
const INLINE_THINK_CLOSE_TAG: &str = "</think>";
const AI_STREAM_RESPONSE_TARGET: &str = "ai::openai_stream_response";

#[derive(Debug, Default)]
struct OpenAIToolCallFilter {
    seen_tool_call_ids: HashSet<String>,
    pending_tool_call_id: Option<String>,
}

impl OpenAIToolCallFilter {
    fn normalize_response(&mut self, mut response: UnifiedResponse) -> Option<UnifiedResponse> {
        self.resolve_pending_tool_call_id(&mut response);

        let Some(tool_call) = response.tool_call.as_ref() else {
            return Some(response);
        };

        let tool_id = tool_call
            .id
            .as_ref()
            .filter(|value| !value.is_empty())
            .cloned();
        let has_name = tool_call
            .name
            .as_ref()
            .is_some_and(|value| !value.is_empty());
        let has_arguments = tool_call
            .arguments
            .as_ref()
            .is_some_and(|value| !value.is_empty());

        if let Some(tool_id) = tool_id {
            let seen_before = self.seen_tool_call_ids.contains(&tool_id);
            self.seen_tool_call_ids.insert(tool_id.clone());

            // Some OpenAI-compatible providers emit "id only" tool-call chunks.
            // They can be either:
            // 1. a harmless trailing/orphan chunk that should be dropped, or
            // 2. a prelude chunk where later deltas carry the actual name/arguments.
            //
            // For (2), keep the id around and reattach it to the next meaningful tool-call
            // delta when that delta omits the id. For (1), stripping this chunk is safe
            // because it carries no semantic payload on its own.
            if !has_name && !has_arguments {
                if !seen_before {
                    self.pending_tool_call_id = Some(tool_id);
                }
                response.tool_call = None;
                return Self::keep_if_non_empty(response);
            }
        } else if !has_name && !has_arguments {
            response.tool_call = None;
            return Self::keep_if_non_empty(response);
        }

        Some(response)
    }

    fn resolve_pending_tool_call_id(&mut self, response: &mut UnifiedResponse) {
        let Some(pending_tool_call_id) = self.pending_tool_call_id.clone() else {
            return;
        };

        let Some(tool_call) = response.tool_call.as_mut() else {
            self.pending_tool_call_id = None;
            return;
        };

        let has_name = tool_call
            .name
            .as_ref()
            .is_some_and(|value| !value.is_empty());
        let has_arguments = tool_call
            .arguments
            .as_ref()
            .is_some_and(|value| !value.is_empty());
        let has_payload = has_name || has_arguments;

        match tool_call.id.as_ref() {
            Some(id) if !id.is_empty() && id == &pending_tool_call_id => {
                self.pending_tool_call_id = None;
            }
            Some(id) if !id.is_empty() => {
                self.pending_tool_call_id = None;
            }
            _ if has_payload => {
                tool_call.id = Some(pending_tool_call_id);
                self.pending_tool_call_id = None;
            }
            _ => {}
        }
    }

    fn keep_if_non_empty(response: UnifiedResponse) -> Option<UnifiedResponse> {
        if response.text.is_some()
            || response.reasoning_content.is_some()
            || response.thinking_signature.is_some()
            || response.tool_call.is_some()
            || response.usage.is_some()
            || response.finish_reason.is_some()
            || response.provider_metadata.is_some()
        {
            Some(response)
        } else {
            None
        }
    }
}

#[derive(Debug, Default)]
struct DeferredResponseMeta {
    usage: Option<UnifiedTokenUsage>,
    finish_reason: Option<String>,
    provider_metadata: Option<Value>,
}

impl DeferredResponseMeta {
    fn from_response(response: &mut UnifiedResponse) -> Self {
        Self {
            usage: response.usage.take(),
            finish_reason: response.finish_reason.take(),
            provider_metadata: response.provider_metadata.take(),
        }
    }

    fn merge(&mut self, other: Self) {
        if other.usage.is_some() {
            self.usage = other.usage;
        }
        if other.finish_reason.is_some() {
            self.finish_reason = other.finish_reason;
        }
        if other.provider_metadata.is_some() {
            self.provider_metadata = other.provider_metadata;
        }
    }

    fn apply_to(self, response: &mut UnifiedResponse) {
        if response.usage.is_none() {
            response.usage = self.usage;
        }
        if response.finish_reason.is_none() {
            response.finish_reason = self.finish_reason;
        }
        if response.provider_metadata.is_none() {
            response.provider_metadata = self.provider_metadata;
        }
    }

    fn is_empty(&self) -> bool {
        self.usage.is_none() && self.finish_reason.is_none() && self.provider_metadata.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineThinkActivation {
    Unknown,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineThinkMode {
    Text,
    Thinking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InlineThinkSegment {
    Text(String),
    Thinking(String),
}

#[derive(Debug)]
struct OpenAIInlineThinkParser {
    enabled: bool,
    activation: InlineThinkActivation,
    mode: InlineThinkMode,
    pending_tail: String,
    initial_probe: String,
    deferred_meta: DeferredResponseMeta,
}

impl OpenAIInlineThinkParser {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            activation: InlineThinkActivation::Unknown,
            mode: InlineThinkMode::Text,
            pending_tail: String::new(),
            initial_probe: String::new(),
            deferred_meta: DeferredResponseMeta::default(),
        }
    }

    fn normalize_response(&mut self, mut response: UnifiedResponse) -> Vec<UnifiedResponse> {
        if !self.enabled {
            return vec![response];
        }

        let Some(text) = response.text.take() else {
            return vec![response];
        };

        // Respect providers that already emit native reasoning chunks.
        if response.reasoning_content.is_some()
            || response.tool_call.is_some()
            || response.thinking_signature.is_some()
        {
            response.text = Some(text);
            return vec![response];
        }

        let current_meta = DeferredResponseMeta::from_response(&mut response);
        let segments = match self.activation {
            InlineThinkActivation::Unknown => self.consume_unknown_text(text),
            InlineThinkActivation::Enabled => self.parse_enabled_text(text),
            InlineThinkActivation::Disabled => vec![InlineThinkSegment::Text(text)],
        };

        self.attach_meta_to_segments(segments, current_meta)
    }

    fn flush(&mut self) -> Vec<UnifiedResponse> {
        if !self.enabled {
            return Vec::new();
        }

        let segments = match self.activation {
            InlineThinkActivation::Unknown => {
                let pending = mem::take(&mut self.initial_probe);
                if pending.is_empty() {
                    Vec::new()
                } else {
                    vec![InlineThinkSegment::Text(pending)]
                }
            }
            InlineThinkActivation::Enabled => {
                let pending = mem::take(&mut self.pending_tail);
                if pending.is_empty() {
                    Vec::new()
                } else if self.mode == InlineThinkMode::Thinking {
                    vec![InlineThinkSegment::Thinking(pending)]
                } else {
                    vec![InlineThinkSegment::Text(pending)]
                }
            }
            InlineThinkActivation::Disabled => Vec::new(),
        };

        self.attach_meta_to_segments(segments, DeferredResponseMeta::default())
    }

    fn consume_unknown_text(&mut self, text: String) -> Vec<InlineThinkSegment> {
        self.initial_probe.push_str(&text);

        let trimmed = self.initial_probe.trim_start_matches(char::is_whitespace);
        if trimmed.is_empty() {
            return Vec::new();
        }

        if trimmed.starts_with(INLINE_THINK_OPEN_TAG) {
            self.activation = InlineThinkActivation::Enabled;
            let buffered = mem::take(&mut self.initial_probe);
            return self.parse_enabled_text(buffered);
        }

        if INLINE_THINK_OPEN_TAG.starts_with(trimmed) {
            return Vec::new();
        }

        self.activation = InlineThinkActivation::Disabled;
        vec![InlineThinkSegment::Text(mem::take(&mut self.initial_probe))]
    }

    fn parse_enabled_text(&mut self, text: String) -> Vec<InlineThinkSegment> {
        let mut data = mem::take(&mut self.pending_tail);
        data.push_str(&text);

        let mut segments = Vec::new();

        loop {
            let marker = match self.mode {
                InlineThinkMode::Text => INLINE_THINK_OPEN_TAG,
                InlineThinkMode::Thinking => INLINE_THINK_CLOSE_TAG,
            };

            if let Some(marker_idx) = data.find(marker) {
                let before_marker = data[..marker_idx].to_string();
                self.push_segment(&mut segments, before_marker);

                data = data[marker_idx + marker.len()..].to_string();
                self.mode = match self.mode {
                    InlineThinkMode::Text => InlineThinkMode::Thinking,
                    InlineThinkMode::Thinking => InlineThinkMode::Text,
                };
                continue;
            }

            let tail_len = longest_suffix_prefix_len(&data, marker);
            let flush_len = data.len() - tail_len;
            let ready = data[..flush_len].to_string();
            self.push_segment(&mut segments, ready);
            self.pending_tail = data[flush_len..].to_string();
            break;
        }

        segments
    }

    fn push_segment(&self, segments: &mut Vec<InlineThinkSegment>, content: String) {
        if content.is_empty() {
            return;
        }

        match self.mode {
            InlineThinkMode::Text => segments.push(InlineThinkSegment::Text(content)),
            InlineThinkMode::Thinking => segments.push(InlineThinkSegment::Thinking(content)),
        }
    }

    fn attach_meta_to_segments(
        &mut self,
        segments: Vec<InlineThinkSegment>,
        current_meta: DeferredResponseMeta,
    ) -> Vec<UnifiedResponse> {
        let mut merged_meta = mem::take(&mut self.deferred_meta);
        merged_meta.merge(current_meta);

        let mut responses: Vec<UnifiedResponse> = segments
            .into_iter()
            .map(|segment| match segment {
                InlineThinkSegment::Text(text) => UnifiedResponse {
                    text: Some(text),
                    ..Default::default()
                },
                InlineThinkSegment::Thinking(reasoning_content) => UnifiedResponse {
                    reasoning_content: Some(reasoning_content),
                    ..Default::default()
                },
            })
            .collect();

        if let Some(last_response) = responses.last_mut() {
            merged_meta.apply_to(last_response);
        } else if !merged_meta.is_empty() {
            self.deferred_meta = merged_meta;
        }

        responses
    }
}

#[derive(Debug)]
struct OpenAIResponseNormalizer {
    tool_arguments_normalizer: OpenAIToolCallArgumentsNormalizer,
    tool_call_filter: OpenAIToolCallFilter,
    inline_think_parser: OpenAIInlineThinkParser,
}

impl OpenAIResponseNormalizer {
    fn new(inline_think_in_text: bool) -> Self {
        Self {
            tool_arguments_normalizer: OpenAIToolCallArgumentsNormalizer::default(),
            tool_call_filter: OpenAIToolCallFilter::default(),
            inline_think_parser: OpenAIInlineThinkParser::new(inline_think_in_text),
        }
    }

    fn normalize_sse_data(&mut self, sse_data: &mut OpenAISSEData) {
        sse_data.normalize_tool_call_arguments(&mut self.tool_arguments_normalizer);
    }

    fn normalize_response(&mut self, response: UnifiedResponse) -> Vec<UnifiedResponse> {
        let Some(response) = self.tool_call_filter.normalize_response(response) else {
            return Vec::new();
        };

        self.inline_think_parser.normalize_response(response)
    }

    fn flush(&mut self) -> Vec<UnifiedResponse> {
        self.inline_think_parser.flush()
    }
}

fn longest_suffix_prefix_len(value: &str, marker: &str) -> usize {
    let max_len = value.len().min(marker.len().saturating_sub(1));
    (1..=max_len)
        .rev()
        .find(|&len| value.ends_with(&marker[..len]))
        .unwrap_or(0)
}

fn is_valid_chat_completion_chunk_weak(event_json: &Value) -> bool {
    matches!(
        event_json.get("object").and_then(|value| value.as_str()),
        Some(OPENAI_CHAT_COMPLETION_CHUNK_OBJECT)
    )
}

fn extract_sse_api_error_message(event_json: &Value) -> Option<String> {
    let error = event_json.get("error")?;
    if let Some(message) = error.get("message").and_then(|value| value.as_str()) {
        return Some(message.to_string());
    }
    if let Some(message) = error.as_str() {
        return Some(message.to_string());
    }
    Some("An error occurred during streaming".to_string())
}

/// Convert a byte stream into a structured response stream
///
/// # Arguments
/// * `response` - HTTP response
/// * `tx_event` - parsed event sender
/// * `tx_raw_sse` - optional raw SSE sender (collect raw data for diagnostics)
pub async fn handle_openai_stream(
    response: Response,
    tx_event: mpsc::UnboundedSender<Result<UnifiedResponse>>,
    tx_raw_sse: Option<mpsc::UnboundedSender<String>>,
    inline_think_in_text: bool,
) {
    let mut stream = response.bytes_stream().eventsource();
    let idle_timeout = Duration::from_secs(600);
    let mut stats = StreamStats::new("OpenAI");
    // Track whether a chunk with `finish_reason` was received.
    // Some providers (e.g. MiniMax) close the stream after the final chunk
    // without sending `[DONE]`, so we treat `Ok(None)` as a normal termination
    // when a finish_reason has already been seen.
    let mut received_finish_reason = false;
    let mut normalizer = OpenAIResponseNormalizer::new(inline_think_in_text);

    loop {
        let sse_event = timeout(idle_timeout, stream.next()).await;
        let sse = match sse_event {
            Ok(Some(Ok(sse))) => sse,
            Ok(None) => {
                if received_finish_reason {
                    for normalized_response in normalizer.flush() {
                        stats.record_unified_response(&normalized_response);
                        let _ = tx_event.send(Ok(normalized_response));
                    }
                    stats.log_summary("stream_closed_after_finish_reason");
                    return;
                }
                let error_msg = "SSE stream closed before response completed";
                stats.log_summary("stream_closed_before_completion");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            Ok(Some(Err(e))) => {
                let error_msg = format!("SSE stream error: {}", e);
                stats.log_summary("sse_stream_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            Err(_) => {
                let error_msg = format!("SSE stream timeout after {}s", idle_timeout.as_secs());
                stats.log_summary("sse_stream_timeout");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        let raw = sse.data;
        stats.record_sse_event("data");
        trace!(target: AI_STREAM_RESPONSE_TARGET, "OpenAI SSE: {:?}", raw);
        if let Some(ref tx) = tx_raw_sse {
            let _ = tx.send(raw.clone());
        }
        if raw == "[DONE]" {
            for normalized_response in normalizer.flush() {
                stats.record_unified_response(&normalized_response);
                let _ = tx_event.send(Ok(normalized_response));
            }
            stats.increment("marker:done");
            stats.log_summary("done_marker_received");
            return;
        }

        let event_json: Value = match serde_json::from_str(&raw) {
            Ok(json) => json,
            Err(e) => {
                let error_msg = format!("SSE parsing error: {}, data: {}", e, &raw);
                stats.increment("error:sse_parsing");
                stats.log_summary("sse_parsing_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        if let Some(api_error_message) = extract_sse_api_error_message(&event_json) {
            let error_msg = format!("SSE API error: {}, data: {}", api_error_message, raw);
            stats.increment("error:api");
            stats.log_summary("sse_api_error");
            error!("{}", error_msg);
            let _ = tx_event.send(Err(anyhow!(error_msg)));
            return;
        }

        if !is_valid_chat_completion_chunk_weak(&event_json) {
            stats.increment("skip:non_standard_event");
            warn!(
                "Skipping non-standard OpenAI SSE event; object={}",
                event_json
                    .get("object")
                    .and_then(|value| value.as_str())
                    .unwrap_or("<missing>")
            );
            continue;
        }

        stats.increment("chunk:chat_completion");
        let mut sse_data: OpenAISSEData = match serde_json::from_value(event_json) {
            Ok(event) => event,
            Err(e) => {
                let error_msg = format!("SSE data schema error: {}, data: {}", e, &raw);
                stats.increment("error:schema");
                stats.log_summary("sse_data_schema_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        let tool_call_count = sse_data.first_choice_tool_call_count();
        if tool_call_count > 1 {
            stats.increment("chunk:multi_tool_call");
            warn!(
                "OpenAI SSE chunk contains {} tool calls in the first choice; splitting and sending sequentially",
                tool_call_count
            );
        }

        normalizer.normalize_sse_data(&mut sse_data);

        let has_empty_choices = sse_data.is_choices_empty();
        let unified_responses = sse_data.into_unified_responses();
        trace!(
            target: AI_STREAM_RESPONSE_TARGET,
            "OpenAI unified responses: {:?}",
            unified_responses
        );
        if unified_responses.is_empty() {
            if has_empty_choices {
                stats.increment("skip:empty_choices_no_usage");
                warn!(
                    "Ignoring OpenAI SSE chunk with empty choices and no usage payload: {}",
                    raw
                );
                // Ignore keepalive/metadata chunks with empty choices and no usage payload.
                continue;
            }
            // Defensive fallback: this should be unreachable if OpenAISSEData::into_unified_responses
            // keeps returning at least one event for all non-empty-choices chunks.
            let error_msg = format!("OpenAI SSE chunk produced no unified events, data: {}", raw);
            stats.increment("error:no_unified_events");
            stats.log_summary("no_unified_events");
            error!("{}", error_msg);
            let _ = tx_event.send(Err(anyhow!(error_msg)));
            return;
        }

        for unified_response in unified_responses {
            let normalized_responses = normalizer.normalize_response(unified_response);
            if normalized_responses.is_empty() {
                continue;
            }

            for normalized_response in normalized_responses {
                if normalized_response.finish_reason.is_some() {
                    received_finish_reason = true;
                }
                stats.record_unified_response(&normalized_response);
                let _ = tx_event.send(Ok(normalized_response));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        extract_sse_api_error_message, is_valid_chat_completion_chunk_weak,
        longest_suffix_prefix_len, InlineThinkActivation, InlineThinkMode, OpenAIInlineThinkParser,
        OpenAIToolCallFilter,
    };
    use crate::stream::types::openai::OpenAISSEData;
    use crate::stream::types::unified::{UnifiedResponse, UnifiedToolCall};

    fn normalize_raw_with_filter(
        filter: &mut OpenAIToolCallFilter,
        raw: &str,
    ) -> Vec<UnifiedResponse> {
        let sse_data: OpenAISSEData = serde_json::from_str(raw).expect("valid openai sse data");
        sse_data
            .into_unified_responses()
            .into_iter()
            .filter_map(|response| filter.normalize_response(response))
            .collect()
    }

    #[test]
    fn weak_filter_accepts_chat_completion_chunk() {
        let event = serde_json::json!({
            "object": "chat.completion.chunk"
        });
        assert!(is_valid_chat_completion_chunk_weak(&event));
    }

    #[test]
    fn weak_filter_rejects_non_standard_object() {
        let event = serde_json::json!({
            "object": ""
        });
        assert!(!is_valid_chat_completion_chunk_weak(&event));
    }

    #[test]
    fn weak_filter_rejects_missing_object() {
        let event = serde_json::json!({
            "id": "chatcmpl_test"
        });
        assert!(!is_valid_chat_completion_chunk_weak(&event));
    }

    #[test]
    fn extracts_api_error_message_from_object_shape() {
        let event = serde_json::json!({
            "error": {
                "message": "provider error"
            }
        });
        assert_eq!(
            extract_sse_api_error_message(&event).as_deref(),
            Some("provider error")
        );
    }

    #[test]
    fn extracts_api_error_message_from_string_shape() {
        let event = serde_json::json!({
            "error": "provider error"
        });
        assert_eq!(
            extract_sse_api_error_message(&event).as_deref(),
            Some("provider error")
        );
    }

    #[test]
    fn returns_none_when_no_error_payload_exists() {
        let event = serde_json::json!({
            "object": "chat.completion.chunk"
        });
        assert!(extract_sse_api_error_message(&event).is_none());
    }

    #[test]
    fn drops_redundant_empty_tool_call_after_same_id_was_seen() {
        let mut filter = OpenAIToolCallFilter::default();

        let first = UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                id: Some("call_1".to_string()),
                name: Some("read_file".to_string()),
                arguments: Some("{\"path\":\"a.txt\"}".to_string()),
                arguments_is_snapshot: false,
            }),
            ..Default::default()
        };
        let trailing_empty = UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                id: Some("call_1".to_string()),
                name: None,
                arguments: Some(String::new()),
                arguments_is_snapshot: false,
            }),
            ..Default::default()
        };

        assert!(filter.normalize_response(first).is_some());
        assert!(filter.normalize_response(trailing_empty).is_none());
    }

    #[test]
    fn keeps_finish_reason_when_redundant_tool_call_is_stripped() {
        let mut filter = OpenAIToolCallFilter::default();

        let first = UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                id: Some("call_1".to_string()),
                name: Some("read_file".to_string()),
                arguments: Some("{\"path\":\"a.txt\"}".to_string()),
                arguments_is_snapshot: false,
            }),
            ..Default::default()
        };
        let trailing_empty = UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                id: Some("call_1".to_string()),
                name: None,
                arguments: None,
                arguments_is_snapshot: false,
            }),
            finish_reason: Some("tool_calls".to_string()),
            ..Default::default()
        };

        assert!(filter.normalize_response(first).is_some());
        let normalized = filter
            .normalize_response(trailing_empty)
            .expect("finish_reason should be preserved");
        assert!(normalized.tool_call.is_none());
        assert_eq!(normalized.finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn strips_unseen_id_only_tool_call_but_keeps_finish_reason() {
        let mut filter = OpenAIToolCallFilter::default();

        let orphan = UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                id: Some("call_orphan".to_string()),
                name: None,
                arguments: None,
                arguments_is_snapshot: false,
            }),
            finish_reason: Some("tool_calls".to_string()),
            ..Default::default()
        };

        let normalized = filter
            .normalize_response(orphan)
            .expect("finish_reason should be preserved");
        assert!(normalized.tool_call.is_none());
        assert_eq!(normalized.finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn reattaches_pending_id_to_following_payload_chunk() {
        let mut filter = OpenAIToolCallFilter::default();

        let prelude = UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                id: Some("call_1".to_string()),
                name: None,
                arguments: None,
                arguments_is_snapshot: false,
            }),
            ..Default::default()
        };
        let payload = UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                id: None,
                name: Some("read_file".to_string()),
                arguments: Some("{\"path\":\"a.txt\"}".to_string()),
                arguments_is_snapshot: false,
            }),
            ..Default::default()
        };

        assert!(filter.normalize_response(prelude).is_none());
        let normalized = filter
            .normalize_response(payload)
            .expect("payload chunk should be kept");
        let tool_call = normalized.tool_call.expect("tool call should exist");
        assert_eq!(tool_call.id.as_deref(), Some("call_1"));
        assert_eq!(tool_call.name.as_deref(), Some("read_file"));
        assert_eq!(tool_call.arguments.as_deref(), Some("{\"path\":\"a.txt\"}"));
    }

    #[test]
    fn drops_orphan_id_only_tool_call_when_it_shares_sse_with_normal_final_tool_chunk() {
        let mut filter = OpenAIToolCallFilter::default();

        let responses = normalize_raw_with_filter(
            &mut filter,
            r#"{
                "id": "chatcmpl_test",
                "created": 123,
                "model": "gpt-test",
                "choices": [{
                    "index": 0,
                    "delta": {
                        "tool_calls": [
                            {
                                "index": 0,
                                "id": "call_1",
                                "type": "function",
                                "function": {
                                    "name": "read_file",
                                    "arguments": "{\"path\":\"a.txt\"}"
                                }
                            },
                            {
                                "index": 1,
                                "id": "call_orphan",
                                "type": "function",
                                "function": {}
                            }
                        ]
                    },
                    "finish_reason": "tool_calls"
                }]
            }"#,
        );

        assert_eq!(responses.len(), 1);
        let tool_call = responses[0]
            .tool_call
            .as_ref()
            .expect("tool call should exist");
        assert_eq!(tool_call.id.as_deref(), Some("call_1"));
        assert_eq!(tool_call.name.as_deref(), Some("read_file"));
        assert_eq!(tool_call.arguments.as_deref(), Some("{\"path\":\"a.txt\"}"));
        assert_eq!(responses[0].finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn drops_orphan_id_only_tool_call_when_it_shares_sse_with_redundant_empty_tail() {
        let mut filter = OpenAIToolCallFilter::default();

        assert!(filter
            .normalize_response(UnifiedResponse {
                tool_call: Some(UnifiedToolCall {
                    id: Some("call_1".to_string()),
                    name: Some("read_file".to_string()),
                    arguments: Some("{\"path\":\"a.txt\"}".to_string()),
                    arguments_is_snapshot: false,
                }),
                ..Default::default()
            })
            .is_some());

        let responses = normalize_raw_with_filter(
            &mut filter,
            r#"{
                "id": "chatcmpl_test",
                "created": 123,
                "model": "gpt-test",
                "choices": [{
                    "index": 0,
                    "delta": {
                        "tool_calls": [
                            {
                                "index": 0,
                                "id": "call_1",
                                "type": "function",
                                "function": {}
                            },
                            {
                                "index": 1,
                                "id": "call_orphan",
                                "type": "function",
                                "function": {}
                            }
                        ]
                    },
                    "finish_reason": "tool_calls"
                }]
            }"#,
        );

        assert_eq!(responses.len(), 1);
        assert!(responses[0].tool_call.is_none());
        assert_eq!(responses[0].finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn longest_suffix_prefix_len_detects_partial_tag_boundary() {
        assert_eq!(longest_suffix_prefix_len("<thi", "<think>"), 4);
        assert_eq!(longest_suffix_prefix_len("answer", "<think>"), 0);
    }

    #[test]
    fn inline_think_parser_streams_thinking_and_text_per_chunk() {
        let mut parser = OpenAIInlineThinkParser::new(true);

        let chunk1 = parser.normalize_response(UnifiedResponse {
            text: Some("<think>abc".to_string()),
            ..Default::default()
        });
        let chunk2 = parser.normalize_response(UnifiedResponse {
            text: Some("def</think>ghi".to_string()),
            ..Default::default()
        });

        assert_eq!(chunk1.len(), 1);
        assert_eq!(chunk1[0].reasoning_content.as_deref(), Some("abc"));
        assert_eq!(chunk2.len(), 2);
        assert_eq!(chunk2[0].reasoning_content.as_deref(), Some("def"));
        assert_eq!(chunk2[1].text.as_deref(), Some("ghi"));
    }

    #[test]
    fn inline_think_parser_handles_split_opening_tag() {
        let mut parser = OpenAIInlineThinkParser::new(true);

        let first = parser.normalize_response(UnifiedResponse {
            text: Some("<thi".to_string()),
            ..Default::default()
        });
        let second = parser.normalize_response(UnifiedResponse {
            text: Some("nk>hello".to_string()),
            ..Default::default()
        });

        assert!(first.is_empty());
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].reasoning_content.as_deref(), Some("hello"));
    }

    #[test]
    fn inline_think_parser_disables_when_first_text_is_not_think_tag() {
        let mut parser = OpenAIInlineThinkParser::new(true);

        let first = parser.normalize_response(UnifiedResponse {
            text: Some("hello <think>literal".to_string()),
            ..Default::default()
        });
        let second = parser.normalize_response(UnifiedResponse {
            text: Some("</think> world".to_string()),
            ..Default::default()
        });

        assert_eq!(first.len(), 1);
        assert_eq!(first[0].text.as_deref(), Some("hello <think>literal"));
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].text.as_deref(), Some("</think> world"));
        assert_eq!(parser.activation, InlineThinkActivation::Disabled);
        assert_eq!(parser.mode, InlineThinkMode::Text);
    }

    #[test]
    fn inline_think_parser_preserves_finish_reason_on_last_segment() {
        let mut parser = OpenAIInlineThinkParser::new(true);

        let responses = parser.normalize_response(UnifiedResponse {
            text: Some("<think>abc</think>done".to_string()),
            finish_reason: Some("stop".to_string()),
            ..Default::default()
        });

        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].reasoning_content.as_deref(), Some("abc"));
        assert_eq!(responses[1].text.as_deref(), Some("done"));
        assert_eq!(responses[1].finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn inline_think_parser_flushes_unclosed_thinking_at_stream_end() {
        let mut parser = OpenAIInlineThinkParser::new(true);

        let first = parser.normalize_response(UnifiedResponse {
            text: Some("<think>abc".to_string()),
            ..Default::default()
        });
        let flushed = parser.flush();

        assert_eq!(first.len(), 1);
        assert_eq!(first[0].reasoning_content.as_deref(), Some("abc"));
        assert!(flushed.is_empty());
    }

    #[test]
    fn inline_think_parser_passthrough_when_feature_disabled() {
        let mut parser = OpenAIInlineThinkParser::new(false);

        let responses = parser.normalize_response(UnifiedResponse {
            text: Some("<think>abc</think>done".to_string()),
            ..Default::default()
        });

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].text.as_deref(), Some("<think>abc</think>done"));
        assert!(responses[0].reasoning_content.is_none());
    }
}
