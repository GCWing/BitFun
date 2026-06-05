use super::stream_stats::StreamStats;
use super::{next_stream_item, TimedStreamItem};
use crate::stream::types::responses::{
    parse_responses_output_item, ResponsesCompleted, ResponsesDone, ResponsesStreamEvent,
};
use crate::stream::types::unified::UnifiedResponse;
use anyhow::{anyhow, Result};
use eventsource_stream::Eventsource;
use log::{error, trace};
use reqwest::Response;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;

const AI_STREAM_RESPONSE_TARGET: &str = "ai::responses_stream_response";

#[derive(Debug, Default, Clone)]
struct InProgressToolCall {
    call_id: Option<String>,
    name: Option<String>,
    args_so_far: String,
    saw_any_delta: bool,
    sent_header: bool,
}

impl InProgressToolCall {
    fn from_item_value(item: &Value) -> Option<Self> {
        if item.get("type").and_then(Value::as_str) != Some("function_call") {
            return None;
        }
        Some(Self {
            call_id: item
                .get("call_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            name: item
                .get("name")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            args_so_far: String::new(),
            saw_any_delta: false,
            sent_header: false,
        })
    }
}

fn emit_unified_response(
    tx_event: &mpsc::UnboundedSender<Result<UnifiedResponse>>,
    stats: &mut StreamStats,
    unified_response: UnifiedResponse,
) {
    trace!(
        target: AI_STREAM_RESPONSE_TARGET,
        "Responses unified response: {:?}",
        unified_response
    );
    stats.record_unified_response(&unified_response);
    let _ = tx_event.send(Ok(unified_response));
}

fn emit_tool_call_item(
    tx_event: &mpsc::UnboundedSender<Result<UnifiedResponse>>,
    stats: &mut StreamStats,
    output_index: Option<usize>,
    item_value: Value,
) {
    if let Some(unified_response) = parse_responses_output_item(item_value, output_index) {
        if unified_response.tool_call.is_some() {
            emit_unified_response(tx_event, stats, unified_response);
        }
    }
}

fn cleanup_tool_call_tracking(
    output_index: usize,
    tool_calls_by_output_index: &mut HashMap<usize, InProgressToolCall>,
    tool_call_index_by_id: &mut HashMap<String, usize>,
) {
    if let Some(tc) = tool_calls_by_output_index.remove(&output_index) {
        if let Some(call_id) = tc.call_id {
            tool_call_index_by_id.remove(&call_id);
        }
    }
}

fn handle_function_call_arguments_delta(
    tx_event: &mpsc::UnboundedSender<Result<UnifiedResponse>>,
    stats: &mut StreamStats,
    output_index: Option<usize>,
    delta: Option<String>,
    tool_calls_by_output_index: &mut HashMap<usize, InProgressToolCall>,
) -> Result<()> {
    let Some(delta) = delta.filter(|delta| !delta.is_empty()) else {
        return Ok(());
    };
    let Some(output_index) = output_index else {
        return Err(anyhow!(
            "Responses function_call_arguments.delta missing output_index"
        ));
    };
    let Some(tc) = tool_calls_by_output_index.get_mut(&output_index) else {
        return Err(anyhow!(
            "Responses function_call_arguments.delta for untracked output_index {}",
            output_index
        ));
    };

    tc.saw_any_delta = true;
    tc.args_so_far.push_str(&delta);

    // Some consumers treat `id` as a "new tool call" marker and reset buffers when it repeats.
    // Only send id/name once per tool call; deltas that follow carry arguments only.
    let (id, name) = if tc.sent_header {
        (None, None)
    } else {
        tc.sent_header = true;
        (tc.call_id.clone(), tc.name.clone())
    };

    let unified_response = UnifiedResponse {
        tool_call: Some(crate::stream::types::unified::UnifiedToolCall {
            tool_call_index: Some(output_index),
            id,
            name,
            arguments: Some(delta),
            arguments_is_snapshot: false,
        }),
        ..Default::default()
    };
    emit_unified_response(tx_event, stats, unified_response);
    Ok(())
}

fn handle_function_call_output_item_done(
    tx_event: &mpsc::UnboundedSender<Result<UnifiedResponse>>,
    stats: &mut StreamStats,
    event_output_index: Option<usize>,
    item_value: Value,
    tool_calls_by_output_index: &mut HashMap<usize, InProgressToolCall>,
    tool_call_index_by_id: &mut HashMap<String, usize>,
) {
    // Resolve output_index either directly or via call_id mapping.
    let output_index = event_output_index.or_else(|| {
        item_value
            .get("call_id")
            .and_then(Value::as_str)
            .and_then(|id| tool_call_index_by_id.get(id).copied())
    });

    let Some(output_index) = output_index else {
        emit_tool_call_item(tx_event, stats, event_output_index, item_value);
        return;
    };

    let Some(tc) = tool_calls_by_output_index.get_mut(&output_index) else {
        // The provider may send `output_item.done` with an output_index even when the
        // earlier `output_item.added` event was omitted or missed. Fall back to the full item.
        emit_tool_call_item(tx_event, stats, Some(output_index), item_value);
        return;
    };

    let full_args = item_value
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let need_fallback_full = !tc.saw_any_delta;
    let need_tail = tc.saw_any_delta
        && tc.args_so_far.len() < full_args.len()
        && full_args.starts_with(&tc.args_so_far);

    if need_fallback_full || need_tail {
        let delta = if need_fallback_full {
            full_args.to_string()
        } else {
            full_args[tc.args_so_far.len()..].to_string()
        };

        if !delta.is_empty() {
            tc.args_so_far.push_str(&delta);
            let (id, name) = if tc.sent_header {
                (None, None)
            } else {
                tc.sent_header = true;
                (tc.call_id.clone(), tc.name.clone())
            };
            let unified_response = UnifiedResponse {
                tool_call: Some(crate::stream::types::unified::UnifiedToolCall {
                    tool_call_index: Some(output_index),
                    id,
                    name,
                    arguments: Some(delta),
                    arguments_is_snapshot: false,
                }),
                ..Default::default()
            };
            emit_unified_response(tx_event, stats, unified_response);
        }
    }

    cleanup_tool_call_tracking(
        output_index,
        tool_calls_by_output_index,
        tool_call_index_by_id,
    );
}

fn extract_api_error_message(event_json: &Value) -> Option<String> {
    let response = event_json.get("response")?;
    let error = response.get("error")?;

    if error.is_null() {
        return None;
    }

    if let Some(message) = error.get("message").and_then(Value::as_str) {
        return Some(message.to_string());
    }
    if let Some(message) = error.as_str() {
        return Some(message.to_string());
    }

    Some("An error occurred during responses streaming".to_string())
}

pub async fn handle_responses_stream(
    response: Response,
    tx_event: mpsc::UnboundedSender<Result<UnifiedResponse>>,
    tx_raw_sse: Option<mpsc::UnboundedSender<String>>,
    idle_timeout: Option<Duration>,
) {
    let mut stream = response.bytes_stream().eventsource();
    // Some providers close the stream after emitting the terminal event and may not send `[DONE]`.
    let mut received_finish_reason = false;
    let mut received_text_delta = false;
    let mut tool_calls_by_output_index: HashMap<usize, InProgressToolCall> = HashMap::new();
    let mut tool_call_index_by_id: HashMap<String, usize> = HashMap::new();
    let mut stats = StreamStats::new("Responses");

    loop {
        let sse = match next_stream_item(&mut stream, idle_timeout).await {
            TimedStreamItem::Item(Ok(sse)) => sse,
            TimedStreamItem::End => {
                if received_finish_reason {
                    stats.log_summary("stream_closed_after_finish_reason");
                    return;
                }
                let error_msg = "Responses SSE stream closed before response completed";
                stats.log_summary("stream_closed_before_completion");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            TimedStreamItem::Item(Err(e)) => {
                let error_msg = format!("Responses SSE stream error: {}", e);
                stats.log_summary("sse_stream_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            TimedStreamItem::TimedOut => {
                let timeout_secs = idle_timeout.map(|timeout| timeout.as_secs()).unwrap_or(0);
                let error_msg = format!("Responses SSE stream timeout after {}s", timeout_secs);
                stats.log_summary("sse_stream_timeout");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        let raw = sse.data;
        stats.record_sse_event("data");
        trace!(target: AI_STREAM_RESPONSE_TARGET, "Responses SSE: {:?}", raw);
        if let Some(ref tx) = tx_raw_sse {
            let _ = tx.send(raw.clone());
        }
        if raw == "[DONE]" {
            stats.increment("marker:done");
            stats.log_summary("done_marker_received");
            return;
        }

        let event_json: Value = match serde_json::from_str(&raw) {
            Ok(json) => json,
            Err(e) => {
                let error_msg = format!("Responses SSE parsing error: {}, data: {}", e, &raw);
                stats.increment("error:sse_parsing");
                stats.log_summary("sse_parsing_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };

        if let Some(api_error_message) = extract_api_error_message(&event_json) {
            let error_msg = format!(
                "Responses SSE API error: {}, data: {}",
                api_error_message, raw
            );
            stats.increment("error:api");
            stats.log_summary("sse_api_error");
            error!("{}", error_msg);
            let _ = tx_event.send(Err(anyhow!(error_msg)));
            return;
        }

        let event: ResponsesStreamEvent = match serde_json::from_value(event_json) {
            Ok(event) => event,
            Err(e) => {
                let error_msg = format!("Responses SSE schema error: {}, data: {}", e, &raw);
                stats.increment("error:schema");
                stats.log_summary("sse_schema_error");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
        };
        stats.increment(format!("event:{}", event.kind));

        match event.kind.as_str() {
            "response.output_item.added" => {
                // Track tool calls so we can stream arguments via `response.function_call_arguments.delta`.
                if let Some(item) = event.item.as_ref() {
                    if let Some(tc) = InProgressToolCall::from_item_value(item) {
                        let Some(output_index) = event.output_index else {
                            let error_msg =
                                "Responses function_call output_item.added missing output_index";
                            stats.increment("error:missing_output_index");
                            stats.log_summary("responses_tool_call_missing_output_index");
                            error!("{}", error_msg);
                            let _ = tx_event.send(Err(anyhow!(error_msg)));
                            return;
                        };
                        if let Some(ref call_id) = tc.call_id {
                            tool_call_index_by_id.insert(call_id.clone(), output_index);
                        }
                        tool_calls_by_output_index.insert(output_index, tc);
                    }
                }
            }
            "response.output_text.delta" => {
                if let Some(delta) = event.delta.filter(|delta| !delta.is_empty()) {
                    received_text_delta = true;
                    let unified_response = UnifiedResponse {
                        text: Some(delta),
                        ..Default::default()
                    };
                    emit_unified_response(&tx_event, &mut stats, unified_response);
                }
            }
            "response.reasoning_text.delta" | "response.reasoning_summary_text.delta" => {
                if let Some(delta) = event.delta.filter(|delta| !delta.is_empty()) {
                    let unified_response = UnifiedResponse {
                        reasoning_content: Some(delta),
                        ..Default::default()
                    };
                    emit_unified_response(&tx_event, &mut stats, unified_response);
                }
            }
            "response.function_call_arguments.delta" => {
                if let Err(err) = handle_function_call_arguments_delta(
                    &tx_event,
                    &mut stats,
                    event.output_index,
                    event.delta,
                    &mut tool_calls_by_output_index,
                ) {
                    let error_msg = err.to_string();
                    stats.increment("error:function_call_arguments_delta");
                    stats.log_summary("responses_function_call_arguments_delta_error");
                    error!("{}", error_msg);
                    let _ = tx_event.send(Err(anyhow!(error_msg)));
                    return;
                }
            }
            "response.output_item.done" => {
                let Some(item_value) = event.item else {
                    continue;
                };

                // For tool calls, prefer streaming deltas and only use item.done as a tail-filler / fallback.
                if item_value.get("type").and_then(Value::as_str) == Some("function_call") {
                    handle_function_call_output_item_done(
                        &tx_event,
                        &mut stats,
                        event.output_index,
                        item_value,
                        &mut tool_calls_by_output_index,
                        &mut tool_call_index_by_id,
                    );
                    continue;
                }

                if let Some(mut unified_response) =
                    parse_responses_output_item(item_value, event.output_index)
                {
                    if received_text_delta && unified_response.text.is_some() {
                        unified_response.text = None;
                    }
                    if unified_response.text.is_some() || unified_response.tool_call.is_some() {
                        emit_unified_response(&tx_event, &mut stats, unified_response);
                    }
                }
            }
            "response.completed" => {
                if received_finish_reason {
                    continue;
                }
                // Best-effort: use the final response object to fill any missing tool-call argument tail.
                if let Some(response_val) = event.response.as_ref() {
                    if let Some(output) = response_val.get("output").and_then(Value::as_array) {
                        for (idx, item) in output.iter().enumerate() {
                            if item.get("type").and_then(Value::as_str) != Some("function_call") {
                                continue;
                            }
                            let Some(tc) = tool_calls_by_output_index.get_mut(&idx) else {
                                continue;
                            };
                            let full_args = item
                                .get("arguments")
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            if tc.args_so_far.len() < full_args.len()
                                && full_args.starts_with(&tc.args_so_far)
                            {
                                let delta = full_args[tc.args_so_far.len()..].to_string();
                                if !delta.is_empty() {
                                    tc.args_so_far.push_str(&delta);
                                    let (id, name) = if tc.sent_header {
                                        (None, None)
                                    } else {
                                        tc.sent_header = true;
                                        (tc.call_id.clone(), tc.name.clone())
                                    };
                                    let unified_response = UnifiedResponse {
                                        tool_call: Some(
                                            crate::stream::types::unified::UnifiedToolCall {
                                                tool_call_index: Some(idx),
                                                id,
                                                name,
                                                arguments: Some(delta),
                                                arguments_is_snapshot: false,
                                            },
                                        ),
                                        ..Default::default()
                                    };
                                    emit_unified_response(&tx_event, &mut stats, unified_response);
                                }
                            }
                        }
                    }
                }
                match event
                    .response
                    .map(serde_json::from_value::<ResponsesCompleted>)
                {
                    Some(Ok(response)) => {
                        received_finish_reason = true;
                        let unified_response = UnifiedResponse {
                            usage: response.usage.map(Into::into),
                            finish_reason: Some("stop".to_string()),
                            ..Default::default()
                        };
                        emit_unified_response(&tx_event, &mut stats, unified_response);
                        continue;
                    }
                    Some(Err(e)) => {
                        let error_msg =
                            format!("Failed to parse response.completed payload: {}", e);
                        stats.increment("error:completed_payload");
                        stats.log_summary("response_completed_parse_error");
                        error!("{}", error_msg);
                        let _ = tx_event.send(Err(anyhow!(error_msg)));
                        return;
                    }
                    None => {
                        received_finish_reason = true;
                        let unified_response = UnifiedResponse {
                            finish_reason: Some("stop".to_string()),
                            ..Default::default()
                        };
                        emit_unified_response(&tx_event, &mut stats, unified_response);
                        continue;
                    }
                }
            }
            "response.done" => {
                if received_finish_reason {
                    continue;
                }
                match event.response.map(serde_json::from_value::<ResponsesDone>) {
                    Some(Ok(response)) => {
                        received_finish_reason = true;
                        let unified_response = UnifiedResponse {
                            usage: response.usage.map(Into::into),
                            finish_reason: Some("stop".to_string()),
                            ..Default::default()
                        };
                        emit_unified_response(&tx_event, &mut stats, unified_response);
                        continue;
                    }
                    Some(Err(e)) => {
                        let error_msg = format!("Failed to parse response.done payload: {}", e);
                        stats.increment("error:done_payload");
                        stats.log_summary("response_done_parse_error");
                        error!("{}", error_msg);
                        let _ = tx_event.send(Err(anyhow!(error_msg)));
                        return;
                    }
                    None => {
                        received_finish_reason = true;
                        let unified_response = UnifiedResponse {
                            finish_reason: Some("stop".to_string()),
                            ..Default::default()
                        };
                        emit_unified_response(&tx_event, &mut stats, unified_response);
                        continue;
                    }
                }
            }
            "response.failed" => {
                let error_msg = event
                    .response
                    .as_ref()
                    .and_then(|response| response.get("error"))
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("Responses API returned response.failed")
                    .to_string();
                stats.increment("error:failed");
                stats.log_summary("response_failed");
                error!("{}", error_msg);
                let _ = tx_event.send(Err(anyhow!(error_msg)));
                return;
            }
            "response.incomplete" => {
                // Prefer returning partial output (rust-genai behavior) instead of hard-failing the round.
                // Still mark finish_reason so the caller can decide how to handle it.
                if received_finish_reason {
                    continue;
                }
                let reason = event
                    .response
                    .as_ref()
                    .and_then(|response| response.get("incomplete_details"))
                    .and_then(|details| details.get("reason"))
                    .and_then(Value::as_str)
                    .map(|s| s.to_string());

                let finish_reason = reason
                    .as_deref()
                    .map(|r| format!("incomplete:{r}"))
                    .unwrap_or_else(|| "incomplete".to_string());

                let usage = event
                    .response
                    .clone()
                    .and_then(|v| serde_json::from_value::<ResponsesDone>(v).ok())
                    .and_then(|r| r.usage)
                    .map(Into::into);

                received_finish_reason = true;
                let unified_response = UnifiedResponse {
                    usage,
                    finish_reason: Some(finish_reason),
                    ..Default::default()
                };
                emit_unified_response(&tx_event, &mut stats, unified_response);
                continue;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        super::stream_stats::StreamStats, extract_api_error_message,
        handle_function_call_arguments_delta, handle_function_call_output_item_done,
        InProgressToolCall,
    };
    use serde_json::json;
    use std::collections::HashMap;
    use tokio::sync::mpsc;

    #[test]
    fn extracts_api_error_message_from_response_error() {
        let event = json!({
            "type": "response.failed",
            "response": {
                "error": {
                    "message": "provider error"
                }
            }
        });

        assert_eq!(
            extract_api_error_message(&event).as_deref(),
            Some("provider error")
        );
    }

    #[test]
    fn returns_none_when_no_response_error_exists() {
        let event = json!({
            "type": "response.created",
            "response": {
                "id": "resp_1"
            }
        });

        assert!(extract_api_error_message(&event).is_none());
    }

    #[test]
    fn returns_none_when_response_error_is_null() {
        let event = json!({
            "type": "response.created",
            "response": {
                "id": "resp_1",
                "error": null
            }
        });

        assert!(extract_api_error_message(&event).is_none());
    }

    #[test]
    fn output_item_done_falls_back_when_output_index_is_untracked() {
        let (tx_event, mut rx_event) = mpsc::unbounded_channel();
        let mut tool_calls_by_output_index: HashMap<usize, InProgressToolCall> = HashMap::new();
        let mut tool_call_index_by_id: HashMap<String, usize> = HashMap::new();
        let mut stats = StreamStats::new("Responses");

        handle_function_call_output_item_done(
            &tx_event,
            &mut stats,
            Some(3),
            json!({
                "type": "function_call",
                "call_id": "call_1",
                "name": "get_weather",
                "arguments": "{\"city\":\"Beijing\"}"
            }),
            &mut tool_calls_by_output_index,
            &mut tool_call_index_by_id,
        );

        let response = rx_event
            .try_recv()
            .expect("tool call event")
            .expect("ok response");
        let tool_call = response.tool_call.expect("tool call");
        assert_eq!(tool_call.tool_call_index, Some(3));
        assert_eq!(tool_call.id.as_deref(), Some("call_1"));
        assert_eq!(tool_call.name.as_deref(), Some("get_weather"));
        assert_eq!(
            tool_call.arguments.as_deref(),
            Some("{\"city\":\"Beijing\"}")
        );
    }

    #[test]
    fn function_call_delta_requires_output_index() {
        let (tx_event, _rx_event) = mpsc::unbounded_channel();
        let mut tool_calls_by_output_index: HashMap<usize, InProgressToolCall> = HashMap::new();
        let mut stats = StreamStats::new("Responses");

        let err = handle_function_call_arguments_delta(
            &tx_event,
            &mut stats,
            None,
            Some("{\"city\"".to_string()),
            &mut tool_calls_by_output_index,
        )
        .expect_err("missing output_index should fail");

        assert!(err.to_string().contains("missing output_index"));
    }

    #[test]
    fn function_call_delta_requires_tracked_output_item() {
        let (tx_event, _rx_event) = mpsc::unbounded_channel();
        let mut tool_calls_by_output_index: HashMap<usize, InProgressToolCall> = HashMap::new();
        let mut stats = StreamStats::new("Responses");

        let err = handle_function_call_arguments_delta(
            &tx_event,
            &mut stats,
            Some(2),
            Some("{\"city\"".to_string()),
            &mut tool_calls_by_output_index,
        )
        .expect_err("untracked output_index should fail");

        assert!(err.to_string().contains("untracked output_index 2"));
    }
}
