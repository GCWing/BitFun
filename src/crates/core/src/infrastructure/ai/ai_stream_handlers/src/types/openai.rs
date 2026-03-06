use super::unified::{UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall};
use serde::Deserialize;

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
    function: Option<FunctionCall>,
}

impl From<OpenAIToolCall> for UnifiedToolCall {
    fn from(tool_call: OpenAIToolCall) -> Self {
        Self {
            id: tool_call.id,
            name: tool_call.function.as_ref().and_then(|f| f.name.clone()),
            arguments: tool_call
                .function
                .as_ref()
                .and_then(|f| f.arguments.clone()),
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

impl OpenAISSEData {
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
        let mut finish_reason = finish_reason;
        let Delta {
            reasoning_content,
            reasoning_details,
            content,
            tool_calls,
            ..
        } = delta;

        // Treat empty strings the same as absent fields (MiniMax sends `content: ""` in
        // reasoning-only chunks).
        let content = content.filter(|s| !s.is_empty());
        let reasoning_content = reasoning_content.filter(|s| !s.is_empty());

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
                finish_reason: finish_reason.take(),
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
                    finish_reason: if is_first_event {
                        finish_reason.take()
                    } else {
                        None
                    },
                });
            }
        }

        if responses.is_empty() {
            responses.push(UnifiedResponse {
                text: None,
                reasoning_content: None,
                thinking_signature: None,
                tool_call: None,
                usage,
                finish_reason,
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
    use super::OpenAISSEData;

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
        assert_eq!(responses[0].finish_reason.as_deref(), Some("tool_calls"));
        assert!(responses[1].finish_reason.is_none());
        assert!(responses[0].usage.is_some());
        assert!(responses[1].usage.is_none());
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
        assert_eq!(responses[0].finish_reason.as_deref(), Some("tool_calls"));

        assert!(responses[1].text.is_none());
        assert_eq!(
            responses[1]
                .tool_call
                .as_ref()
                .and_then(|tool| tool.id.as_deref()),
            Some("call_1")
        );
        assert!(responses[1].usage.is_none());
        assert!(responses[1].finish_reason.is_none());
    }
}
