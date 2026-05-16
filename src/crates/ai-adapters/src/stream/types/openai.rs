use super::unified::{UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
struct PromptTokensDetails {
    cached_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
    prompt_tokens_details: Option<PromptTokensDetails>,
}

impl From<OpenAIUsage> for UnifiedTokenUsage {
    fn from(usage: OpenAIUsage) -> Self {
        Self {
            prompt_token_count: usage.prompt_tokens,
            candidates_token_count: usage.completion_tokens,
            total_token_count: usage.total_tokens,
            reasoning_token_count: None,
            cached_content_token_count: usage
                .prompt_tokens_details
                .and_then(|prompt_tokens_details| prompt_tokens_details.cached_tokens),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Choice {
    #[allow(dead_code)]
    index: usize,
    delta: Delta,
    finish_reason: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_stringish")]
    stop_reason: Option<String>,
}

/// MiniMax `reasoning_details` array element.
/// Only elements with `type == "reasoning.text"` carry thinking text.
#[derive(Debug, Deserialize)]
struct ReasoningDetail {
    #[serde(rename = "type")]
    detail_type: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[allow(dead_code)]
    role: Option<String>,
    /// Standard OpenAI-compatible reasoning field (DeepSeek, Qwen, etc.)
    reasoning_content: Option<String>,
    /// MiniMax-specific reasoning field; used as fallback when `reasoning_content` is absent.
    reasoning_details: Option<Vec<ReasoningDetail>>,
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Deserialize, Clone)]
struct OpenAIToolCall {
    #[allow(dead_code)]
    index: usize,
    #[allow(dead_code)]
    id: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    tool_type: Option<String>,
    #[serde(default)]
    arguments_is_snapshot: bool,
    function: Option<FunctionCall>,
}

impl From<OpenAIToolCall> for UnifiedToolCall {
    fn from(tool_call: OpenAIToolCall) -> Self {
        Self {
            tool_call_index: Some(tool_call.index),
            id: tool_call.id,
            name: tool_call.function.as_ref().and_then(|f| f.name.clone()),
            arguments: tool_call
                .function
                .as_ref()
                .and_then(|f| f.arguments.clone()),
            arguments_is_snapshot: tool_call.arguments_is_snapshot,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
struct FunctionCall {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAISSEData {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    created: u64,
    #[allow(dead_code)]
    model: String,
    choices: Vec<Choice>,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Default)]
pub struct OpenAIToolCallArgumentsNormalizer;

fn deserialize_optional_stringish<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(value)) => Some(value),
        Some(serde_json::Value::Number(value)) => Some(value.to_string()),
        Some(serde_json::Value::Bool(value)) => Some(value.to_string()),
        Some(other) => Some(other.to_string()),
    })
}

impl OpenAIToolCallArgumentsNormalizer {
    fn normalize_choice(&mut self, choice: &mut Choice) {
        let has_stop_reason = choice.stop_reason.is_some();
        let Some(tool_calls) = choice.delta.tool_calls.as_mut() else {
            return;
        };

        for tool_call in tool_calls.iter_mut() {
            self.normalize_tool_call(tool_call, has_stop_reason);
        }
    }

    fn normalize_tool_call(&mut self, tool_call: &mut OpenAIToolCall, has_stop_reason: bool) {
        let has_id = tool_call.id.as_ref().is_some_and(|value| !value.is_empty());
        let has_name = tool_call
            .function
            .as_ref()
            .and_then(|function| function.name.as_ref())
            .is_some_and(|value| !value.is_empty());

        let Some(function) = tool_call.function.as_mut() else {
            return;
        };
        let Some(arguments) = function.arguments.as_ref() else {
            return;
        };

        if arguments.is_empty() {
            return;
        }

        if has_stop_reason && !has_id && !has_name {
            tool_call.arguments_is_snapshot = true;
        }
    }
}

impl OpenAISSEData {
    pub fn normalize_tool_call_arguments(
        &mut self,
        normalizer: &mut OpenAIToolCallArgumentsNormalizer,
    ) {
        if let Some(first_choice) = self.choices.first_mut() {
            normalizer.normalize_choice(first_choice);
        }
    }

    pub fn is_choices_empty(&self) -> bool {
        self.choices.is_empty()
    }

    pub fn first_choice_tool_call_count(&self) -> usize {
        self.choices
            .first()
            .and_then(|choice| choice.delta.tool_calls.as_ref())
            .map(|tool_calls| tool_calls.len())
            .unwrap_or(0)
    }

    pub fn into_unified_responses(self) -> Vec<UnifiedResponse> {
        let mut usage = self.usage.map(|usage| usage.into());

        let Some(first_choice) = self.choices.into_iter().next() else {
            // OpenAI can emit `choices: []` for the final usage chunk.
            return usage
                .map(|usage_data| {
                    vec![UnifiedResponse {
                        usage: Some(usage_data),
                        ..Default::default()
                    }]
                })
                .unwrap_or_default();
        };

        let Choice {
            delta,
            finish_reason,
            ..
        } = first_choice;
        let Delta {
            reasoning_content,
            reasoning_details,
            content,
            tool_calls,
            ..
        } = delta;

        // Treat empty strings the same as absent fields for assistant text (MiniMax sends
        // `content: ""` in reasoning-only chunks). Keep empty reasoning content so downstream
        // can replay structurally present thinking blocks when a provider requires it.
        let content = content.filter(|s| !s.is_empty());
        let reasoning_content = reasoning_content;

        // MiniMax uses `reasoning_details` instead of `reasoning_content`.
        // Collect all "reasoning.text" entries and join them as a fallback.
        let reasoning_content = reasoning_content.or_else(|| {
            reasoning_details.and_then(|details| {
                let text: String = details
                    .into_iter()
                    .filter(|d| d.detail_type.as_deref() == Some("reasoning.text"))
                    .filter_map(|d| d.text)
                    .collect();
                if text.is_empty() {
                    None
                } else {
                    Some(text)
                }
            })
        });

        let mut responses = Vec::new();

        if content.is_some() || reasoning_content.is_some() {
            responses.push(UnifiedResponse {
                text: content,
                reasoning_content,
                thinking_signature: None,
                tool_call: None,
                usage: usage.take(),
                finish_reason: None,
                provider_metadata: None,
            });
        }

        if let Some(tool_calls) = tool_calls {
            for tool_call in tool_calls {
                let is_first_event = responses.is_empty();
                responses.push(UnifiedResponse {
                    text: None,
                    reasoning_content: None,
                    thinking_signature: None,
                    tool_call: Some(UnifiedToolCall::from(tool_call)),
                    usage: if is_first_event { usage.take() } else { None },
                    finish_reason: None,
                    provider_metadata: None,
                });
            }
        }

        if let Some(finish_reason) = finish_reason {
            if let Some(last_response) = responses.last_mut() {
                last_response.finish_reason = Some(finish_reason);
                return responses;
            }

            responses.push(UnifiedResponse {
                text: None,
                reasoning_content: None,
                thinking_signature: None,
                tool_call: None,
                usage,
                finish_reason: Some(finish_reason),
                provider_metadata: None,
            });
            return responses;
        }

        if responses.is_empty() {
            responses.push(UnifiedResponse {
                text: None,
                reasoning_content: None,
                thinking_signature: None,
                tool_call: None,
                usage,
                finish_reason,
                provider_metadata: None,
            });
        }

        responses
    }
}

impl From<OpenAISSEData> for UnifiedResponse {
    fn from(data: OpenAISSEData) -> Self {
        data.into_unified_responses()
            .into_iter()
            .next()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::{OpenAISSEData, OpenAIToolCallArgumentsNormalizer};

    #[test]
    fn splits_multiple_tool_calls_in_first_choice() {
        let raw = r#"{
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
                                "name": "tool_a",
                                "arguments": "{\"a\":1}"
                            }
                        },
                        {
                            "index": 1,
                            "id": "call_2",
                            "type": "function",
                            "function": {
                                "name": "tool_b",
                                "arguments": "{\"b\":2}"
                            }
                        }
                    ]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15,
                "prompt_tokens_details": {
                    "cached_tokens": 3
                }
            }
        }"#;

        let sse_data: OpenAISSEData = serde_json::from_str(raw).expect("valid openai sse data");
        let responses = sse_data.into_unified_responses();

        assert_eq!(responses.len(), 2);
        assert_eq!(
            responses[0]
                .tool_call
                .as_ref()
                .and_then(|tool| tool.tool_call_index),
            Some(0)
        );
        assert_eq!(
            responses[1]
                .tool_call
                .as_ref()
                .and_then(|tool| tool.tool_call_index),
            Some(1)
        );
        assert_eq!(
            responses[0]
                .tool_call
                .as_ref()
                .and_then(|tool| tool.id.as_deref()),
            Some("call_1")
        );
        assert_eq!(
            responses[1]
                .tool_call
                .as_ref()
                .and_then(|tool| tool.id.as_deref()),
            Some("call_2")
        );
        assert!(responses[0].finish_reason.is_none());
        assert_eq!(responses[1].finish_reason.as_deref(), Some("tool_calls"));
        assert!(responses[0].usage.is_some());
        assert!(responses[1].usage.is_none());
    }

    #[test]
    fn preserves_empty_reasoning_content_chunk() {
        let raw = r#"{
            "id": "chatcmpl_test",
            "created": 123,
            "model": "deepseek-test",
            "choices": [{
                "index": 0,
                "delta": {
                    "reasoning_content": ""
                },
                "finish_reason": "stop"
            }]
        }"#;

        let sse_data: OpenAISSEData = serde_json::from_str(raw).expect("valid openai sse data");
        let responses = sse_data.into_unified_responses();

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].reasoning_content.as_deref(), Some(""));
        assert_eq!(responses[0].finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn handles_empty_choices_with_usage_chunk() {
        let raw = r#"{
            "id": "chatcmpl_test",
            "created": 123,
            "model": "gpt-test",
            "choices": [],
            "usage": {
                "prompt_tokens": 7,
                "completion_tokens": 3,
                "total_tokens": 10
            }
        }"#;

        let sse_data: OpenAISSEData = serde_json::from_str(raw).expect("valid openai sse data");
        let responses = sse_data.into_unified_responses();

        assert_eq!(responses.len(), 1);
        assert!(responses[0].usage.is_some());
        assert!(responses[0].text.is_none());
        assert!(responses[0].tool_call.is_none());
    }

    #[test]
    fn handles_empty_choices_without_usage_chunk() {
        let raw = r#"{
            "id": "chatcmpl_test",
            "created": 123,
            "model": "gpt-test",
            "choices": [],
            "usage": null
        }"#;

        let sse_data: OpenAISSEData = serde_json::from_str(raw).expect("valid openai sse data");
        let responses = sse_data.into_unified_responses();

        assert!(responses.is_empty());
    }

    #[test]
    fn preserves_text_when_tool_calls_exist_in_same_chunk() {
        let raw = r#"{
            "id": "chatcmpl_test",
            "created": 123,
            "model": "gpt-test",
            "choices": [{
                "index": 0,
                "delta": {
                    "content": "hello",
                    "tool_calls": [
                        {
                            "index": 0,
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "tool_a",
                                "arguments": "{\"a\":1}"
                            }
                        }
                    ]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        }"#;

        let sse_data: OpenAISSEData = serde_json::from_str(raw).expect("valid openai sse data");
        let responses = sse_data.into_unified_responses();

        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].text.as_deref(), Some("hello"));
        assert!(responses[0].tool_call.is_none());
        assert!(responses[0].usage.is_some());
        assert!(responses[0].finish_reason.is_none());

        assert!(responses[1].text.is_none());
        assert_eq!(
            responses[1]
                .tool_call
                .as_ref()
                .and_then(|tool| tool.id.as_deref()),
            Some("call_1")
        );
        assert!(responses[1].usage.is_none());
        assert_eq!(responses[1].finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn marks_stop_reason_tool_chunk_as_snapshot() {
        let mut normalizer = OpenAIToolCallArgumentsNormalizer::default();

        let mut first_chunk: OpenAISSEData = serde_json::from_str(
            r#"{
                "id": "chatcmpl_test",
                "created": 123,
                "model": "gpt-test",
                "choices": [{
                    "index": 0,
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "tool_a",
                                "arguments": "{\"city\":\"Bei"
                            }
                        }]
                    },
                    "finish_reason": null
                }]
            }"#,
        )
        .expect("valid first chunk");
        first_chunk.normalize_tool_call_arguments(&mut normalizer);
        let first_responses = first_chunk.into_unified_responses();
        assert_eq!(
            first_responses[0]
                .tool_call
                .as_ref()
                .and_then(|tool| tool.arguments.as_deref()),
            Some("{\"city\":\"Bei")
        );
        assert!(
            !first_responses[0]
                .tool_call
                .as_ref()
                .expect("tool call")
                .arguments_is_snapshot
        );

        let mut snapshot_chunk: OpenAISSEData = serde_json::from_str(
            r#"{
                "id": "chatcmpl_test",
                "created": 123,
                "model": "gpt-test",
                "choices": [{
                    "index": 0,
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "type": "function",
                            "function": {
                                "arguments": "{\"city\":\"Beijing\"}"
                            }
                        }]
                    },
                    "stop_reason": "stop"
                }]
            }"#,
        )
        .expect("valid snapshot chunk");
        snapshot_chunk.normalize_tool_call_arguments(&mut normalizer);
        let snapshot_responses = snapshot_chunk.into_unified_responses();
        assert_eq!(
            snapshot_responses[0]
                .tool_call
                .as_ref()
                .and_then(|tool| tool.arguments.as_deref()),
            Some("{\"city\":\"Beijing\"}")
        );
        assert!(
            snapshot_responses[0]
                .tool_call
                .as_ref()
                .expect("tool call")
                .arguments_is_snapshot
        );
        assert!(snapshot_responses[0].finish_reason.is_none());
    }

    #[test]
    fn leaves_normal_tool_delta_chunks_as_non_snapshot() {
        let mut normalizer = OpenAIToolCallArgumentsNormalizer::default();

        let mut chunk: OpenAISSEData = serde_json::from_str(
            r#"{
                "id": "chatcmpl_test",
                "created": 123,
                "model": "gpt-test",
                "choices": [{
                    "index": 0,
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "type": "function",
                            "function": {
                                "arguments": "jing"
                            }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }]
            }"#,
        )
        .expect("valid chunk");
        chunk.normalize_tool_call_arguments(&mut normalizer);
        let responses = chunk.into_unified_responses();
        assert_eq!(responses.len(), 1);
        assert!(
            !responses[0]
                .tool_call
                .as_ref()
                .expect("tool call")
                .arguments_is_snapshot
        );
    }

    #[test]
    fn parses_numeric_stop_reason_as_string() {
        let data: OpenAISSEData = serde_json::from_str(
            r#"{
                "id": "chatcmpl_test",
                "created": 123,
                "model": "gpt-test",
                "choices": [{
                    "index": 0,
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "type": "function",
                            "function": {
                                "arguments": "{\"a\":1}"
                            }
                        }]
                    },
                    "stop_reason": 154829
                }]
            }"#,
        )
        .expect("valid numeric stop_reason payload");

        let mut normalizer = OpenAIToolCallArgumentsNormalizer::default();
        let mut data = data;
        data.normalize_tool_call_arguments(&mut normalizer);
        let responses = data.into_unified_responses();

        assert_eq!(responses.len(), 1);
        assert!(responses[0].tool_call.is_some());
    }

    #[test]
    fn parses_string_stop_reason_unchanged() {
        let data: OpenAISSEData = serde_json::from_str(
            r#"{
                "id": "chatcmpl_test",
                "created": 123,
                "model": "gpt-test",
                "choices": [{
                    "index": 0,
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "type": "function",
                            "function": {
                                "arguments": "{\"a\":1}"
                            }
                        }]
                    },
                    "stop_reason": "154829"
                }]
            }"#,
        )
        .expect("valid string stop_reason payload");

        let mut normalizer = OpenAIToolCallArgumentsNormalizer::default();
        let mut data = data;
        data.normalize_tool_call_arguments(&mut normalizer);
        let responses = data.into_unified_responses();

        assert_eq!(responses.len(), 1);
        assert!(responses[0].tool_call.is_some());
    }
}
