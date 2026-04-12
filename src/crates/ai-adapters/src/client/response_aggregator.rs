use crate::stream::UnifiedResponse;
use crate::tool_call_accumulator::{PendingToolCall, ToolCallBoundary};
use crate::types::{GeminiResponse, GeminiUsage, ToolCall};
use anyhow::Result;
use futures::StreamExt;
use log::{debug, warn};

use super::StreamResponse;

pub(crate) async fn aggregate_stream_response(
    stream_response: StreamResponse,
) -> Result<GeminiResponse> {
    let mut stream = stream_response.stream;

    let mut full_text = String::new();
    let mut full_reasoning = String::new();
    let mut finish_reason = None;
    let mut usage = None;
    let mut provider_metadata: Option<serde_json::Value> = None;

    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut pending_tool_call = PendingToolCall::default();

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                let UnifiedResponse {
                    text,
                    reasoning_content,
                    thinking_signature: _,
                    tool_call,
                    usage: chunk_usage,
                    finish_reason: chunk_finish_reason,
                    provider_metadata: chunk_provider_metadata,
                } = chunk;

                if let Some(text) = text {
                    full_text.push_str(&text);
                }

                if let Some(reasoning_content) = reasoning_content {
                    full_reasoning.push_str(&reasoning_content);
                }

                if let Some(tool_call) = tool_call {
                    let crate::stream::UnifiedToolCall {
                        id,
                        name,
                        arguments,
                        arguments_is_snapshot,
                    } = tool_call;

                    if let Some(tool_call_id) = id {
                        if !tool_call_id.is_empty() {
                            let is_new_tool = pending_tool_call.tool_id() != tool_call_id;
                            if is_new_tool {
                                if let Some(finalized) =
                                    pending_tool_call.finalize(ToolCallBoundary::NewTool)
                                {
                                    if finalized.is_error {
                                        warn!(
                                            "[send_message] Dropping invalid tool call at boundary=new_tool: tool_id={}, tool_name={}, raw_len={}",
                                            finalized.tool_id,
                                            finalized.tool_name,
                                            finalized.raw_arguments.len()
                                        );
                                    } else {
                                        let arguments = finalized.arguments_as_object_map();
                                        tool_calls.push(ToolCall {
                                            id: finalized.tool_id,
                                            name: finalized.tool_name,
                                            arguments,
                                        });
                                    }
                                }
                                pending_tool_call.start_new(tool_call_id, name.clone());
                                debug!(
                                    "[send_message] Detected tool call: {}",
                                    pending_tool_call.tool_name()
                                );
                            } else {
                                pending_tool_call.update_tool_name_if_missing(name.clone());
                            }
                        }
                    }

                    if let Some(tool_call_arguments) = arguments {
                        if pending_tool_call.has_pending() {
                            if arguments_is_snapshot {
                                pending_tool_call.replace_arguments(&tool_call_arguments);
                            } else {
                                pending_tool_call.append_arguments(&tool_call_arguments);
                            }
                        }
                    }
                }

                if let Some(finish_reason_) = chunk_finish_reason {
                    if let Some(finalized) =
                        pending_tool_call.finalize(ToolCallBoundary::FinishReason)
                    {
                        if finalized.is_error {
                            warn!(
                                "[send_message] Dropping invalid tool call at boundary=finish_reason: tool_id={}, tool_name={}, raw_len={}",
                                finalized.tool_id,
                                finalized.tool_name,
                                finalized.raw_arguments.len()
                            );
                        } else {
                            let arguments = finalized.arguments_as_object_map();
                            tool_calls.push(ToolCall {
                                id: finalized.tool_id,
                                name: finalized.tool_name,
                                arguments,
                            });
                        }
                    }
                    finish_reason = Some(finish_reason_);
                }

                if let Some(chunk_usage) = chunk_usage {
                    usage = Some(unified_usage_to_gemini_usage(chunk_usage));
                }

                if let Some(chunk_provider_metadata) = chunk_provider_metadata {
                    match provider_metadata.as_mut() {
                        Some(existing) => {
                            crate::client::utils::merge_json_value(
                                existing,
                                chunk_provider_metadata,
                            );
                        }
                        None => provider_metadata = Some(chunk_provider_metadata),
                    }
                }
            }
            Err(e) => return Err(e),
        }
    }

    if let Some(finalized) = pending_tool_call.finalize(ToolCallBoundary::EndOfAggregation) {
        if finalized.is_error {
            warn!(
                "[send_message] Dropping invalid tool call at boundary=end_of_aggregation: tool_id={}, tool_name={}, raw_len={}",
                finalized.tool_id,
                finalized.tool_name,
                finalized.raw_arguments.len()
            );
        } else {
            let arguments = finalized.arguments_as_object_map();
            tool_calls.push(ToolCall {
                id: finalized.tool_id,
                name: finalized.tool_name,
                arguments,
            });
        }
    }

    Ok(GeminiResponse {
        text: full_text,
        reasoning_content: (!full_reasoning.is_empty()).then_some(full_reasoning),
        tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
        usage,
        finish_reason,
        provider_metadata,
    })
}

pub(crate) fn unified_usage_to_gemini_usage(
    usage: crate::stream::UnifiedTokenUsage,
) -> GeminiUsage {
    GeminiUsage {
        prompt_token_count: usage.prompt_token_count,
        candidates_token_count: usage.candidates_token_count,
        total_token_count: usage.total_token_count,
        reasoning_token_count: usage.reasoning_token_count,
        cached_content_token_count: usage.cached_content_token_count,
    }
}
