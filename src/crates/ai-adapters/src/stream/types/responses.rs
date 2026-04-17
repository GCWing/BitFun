use super::unified::{UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct ResponsesStreamEvent {
    #[serde(rename = "type")]
    pub kind: String,
    /// Output item index in the `response.output` array.
    #[serde(default)]
    pub output_index: Option<usize>,
    /// Content part index within an output item (for content-part events).
    #[allow(dead_code)]
    #[serde(default)]
    pub content_index: Option<usize>,
    #[serde(default)]
    pub response: Option<Value>,
    #[serde(default)]
    pub item: Option<Value>,
    #[serde(default)]
    pub delta: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesCompleted {
    #[allow(dead_code)]
    pub id: String,
    #[serde(default)]
    pub usage: Option<ResponsesUsage>,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesDone {
    #[serde(default)]
    #[allow(dead_code)]
    pub id: Option<String>,
    #[serde(default)]
    pub usage: Option<ResponsesUsage>,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesUsage {
    pub input_tokens: u32,
    #[serde(default)]
    pub input_tokens_details: Option<ResponsesInputTokensDetails>,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesInputTokensDetails {
    pub cached_tokens: u32,
}

impl From<ResponsesUsage> for UnifiedTokenUsage {
    fn from(usage: ResponsesUsage) -> Self {
        Self {
            prompt_token_count: usage.input_tokens,
            candidates_token_count: usage.output_tokens,
            total_token_count: usage.total_tokens,
            reasoning_token_count: None,
            cached_content_token_count: usage
                .input_tokens_details
                .map(|details| details.cached_tokens),
        }
    }
}

pub fn parse_responses_output_item(
    item_value: Value,
    tool_call_index: Option<usize>,
) -> Option<UnifiedResponse> {
    let item_type = item_value.get("type")?.as_str()?;

    match item_type {
        "function_call" => Some(UnifiedResponse {
            text: None,
            reasoning_content: None,
            thinking_signature: None,
            tool_call: Some(UnifiedToolCall {
                tool_call_index,
                id: item_value
                    .get("call_id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                name: item_value
                    .get("name")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                arguments: item_value
                    .get("arguments")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                arguments_is_snapshot: false,
            }),
            usage: None,
            finish_reason: None,
            provider_metadata: None,
        }),
        "message" => {
            let text = item_value
                .get("content")
                .and_then(Value::as_array)
                .map(|content| {
                    content
                        .iter()
                        .filter(|item| {
                            item.get("type").and_then(Value::as_str) == Some("output_text")
                        })
                        .filter_map(|item| item.get("text").and_then(Value::as_str))
                        .collect::<String>()
                })
                .filter(|text| !text.is_empty());

            text.map(|text| UnifiedResponse {
                text: Some(text),
                reasoning_content: None,
                thinking_signature: None,
                tool_call: None,
                usage: None,
                finish_reason: None,
                provider_metadata: None,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_responses_output_item, ResponsesCompleted, ResponsesStreamEvent};
    use serde_json::json;

    #[test]
    fn parses_output_text_message_item() {
        let response = parse_responses_output_item(
            json!({
                "type": "message",
                "role": "assistant",
                "content": [
                    {
                        "type": "output_text",
                        "text": "hello"
                    }
                ]
            }),
            None,
        )
        .expect("message item");

        assert_eq!(response.text.as_deref(), Some("hello"));
    }

    #[test]
    fn parses_function_call_item() {
        let response = parse_responses_output_item(
            json!({
                "type": "function_call",
                "call_id": "call_1",
                "name": "get_weather",
                "arguments": "{\"city\":\"Beijing\"}"
            }),
            Some(3),
        )
        .expect("function call item");

        let tool_call = response.tool_call.expect("tool call");
        assert_eq!(tool_call.tool_call_index, Some(3));
        assert_eq!(tool_call.id.as_deref(), Some("call_1"));
        assert_eq!(tool_call.name.as_deref(), Some("get_weather"));
    }

    #[test]
    fn parses_completed_payload_usage() {
        let event: ResponsesStreamEvent = serde_json::from_value(json!({
            "type": "response.completed",
            "response": {
                "id": "resp_1",
                "usage": {
                    "input_tokens": 10,
                    "input_tokens_details": { "cached_tokens": 2 },
                    "output_tokens": 4,
                    "total_tokens": 14
                }
            }
        }))
        .expect("event");

        let completed: ResponsesCompleted =
            serde_json::from_value(event.response.expect("response")).expect("completed");
        assert_eq!(completed.id, "resp_1");
        assert_eq!(completed.usage.expect("usage").total_tokens, 14);
    }

    #[test]
    fn parses_output_item_added_indices() {
        let event: ResponsesStreamEvent = serde_json::from_value(json!({
            "type": "response.output_item.added",
            "output_index": 3,
            "item": { "type": "function_call", "call_id": "call_1", "name": "tool", "arguments": "" }
        }))
        .expect("event");

        assert_eq!(event.output_index, Some(3));
        assert!(event.item.is_some());
    }

    #[test]
    fn parses_function_call_arguments_delta_indices() {
        let event: ResponsesStreamEvent = serde_json::from_value(json!({
            "type": "response.function_call_arguments.delta",
            "output_index": 1,
            "delta": "{\"a\":"
        }))
        .expect("event");

        assert_eq!(event.output_index, Some(1));
        assert_eq!(event.delta.as_deref(), Some("{\"a\":"));
    }
}
