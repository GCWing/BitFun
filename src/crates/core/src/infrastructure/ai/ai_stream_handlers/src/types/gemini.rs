use crate::types::unified::{UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiSSEData {
    #[serde(default)]
    pub candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    pub usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCandidate {
    #[serde(default)]
    pub content: Option<GeminiContent>,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiContent {
    #[serde(default)]
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiPart {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub thought: Option<bool>,
    #[serde(default)]
    pub thought_signature: Option<String>,
    #[serde(default)]
    pub function_call: Option<GeminiFunctionCall>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFunctionCall {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub args: Option<Value>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeminiUsageMetadata {
    #[serde(default)]
    pub prompt_token_count: u32,
    #[serde(default)]
    pub candidates_token_count: u32,
    #[serde(default)]
    pub total_token_count: u32,
    #[serde(default)]
    pub cached_content_token_count: Option<u32>,
}

impl From<GeminiUsageMetadata> for UnifiedTokenUsage {
    fn from(usage: GeminiUsageMetadata) -> Self {
        Self {
            prompt_token_count: usage.prompt_token_count,
            candidates_token_count: usage.candidates_token_count,
            total_token_count: usage.total_token_count,
            cached_content_token_count: usage.cached_content_token_count,
        }
    }
}

impl GeminiSSEData {
    pub fn into_unified_responses(self) -> Vec<UnifiedResponse> {
        let mut usage = self.usage_metadata.map(Into::into);
        let Some(candidate) = self.candidates.into_iter().next() else {
            return usage
                .take()
                .map(|usage| {
                    vec![UnifiedResponse {
                        usage: Some(usage),
                        ..Default::default()
                    }]
                })
                .unwrap_or_default();
        };

        let mut responses = Vec::new();
        let mut finish_reason = candidate.finish_reason;

        if let Some(content) = candidate.content {
            for part in content.parts {
                let has_function_call = part.function_call.is_some();
                let text = part.text.filter(|text| !text.is_empty());
                let is_thought = part.thought.unwrap_or(false);
                let thinking_signature = part.thought_signature.filter(|value| !value.is_empty());

                if let Some(function_call) = part.function_call {
                    let arguments = function_call.args.unwrap_or_else(|| json!({}));
                    responses.push(UnifiedResponse {
                        text: None,
                        reasoning_content: None,
                        thinking_signature,
                        tool_call: Some(UnifiedToolCall {
                            id: None,
                            name: function_call.name,
                            arguments: serde_json::to_string(&arguments).ok(),
                        }),
                        usage: usage.take(),
                        finish_reason: finish_reason.take(),
                    });
                    continue;
                }

                if let Some(text) = text {
                    responses.push(UnifiedResponse {
                        text: if is_thought { None } else { Some(text.clone()) },
                        reasoning_content: if is_thought { Some(text) } else { None },
                        thinking_signature,
                        tool_call: None,
                        usage: usage.take(),
                        finish_reason: finish_reason.take(),
                    });
                    continue;
                }

                if thinking_signature.is_some() && !has_function_call {
                    responses.push(UnifiedResponse {
                        text: None,
                        reasoning_content: None,
                        thinking_signature,
                        tool_call: None,
                        usage: usage.take(),
                        finish_reason: finish_reason.take(),
                    });
                }
            }
        }

        if responses.is_empty() {
            responses.push(UnifiedResponse {
                usage,
                finish_reason,
                ..Default::default()
            });
        }

        responses
    }
}

#[cfg(test)]
mod tests {
    use super::GeminiSSEData;

    #[test]
    fn converts_text_thought_and_usage() {
        let payload = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        { "text": "thinking", "thought": true, "thoughtSignature": "sig_1" },
                        { "text": "answer" }
                    ]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 4,
                "totalTokenCount": 14
            }
        });

        let data: GeminiSSEData = serde_json::from_value(payload).expect("gemini payload");
        let responses = data.into_unified_responses();

        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].reasoning_content.as_deref(), Some("thinking"));
        assert_eq!(responses[0].thinking_signature.as_deref(), Some("sig_1"));
        assert_eq!(
            responses[0]
                .usage
                .as_ref()
                .map(|usage| usage.total_token_count),
            Some(14)
        );
        assert_eq!(responses[1].text.as_deref(), Some("answer"));
    }

    #[test]
    fn keeps_thought_signature_on_function_call_parts() {
        let payload = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {
                            "thoughtSignature": "sig_tool",
                            "functionCall": {
                                "name": "get_weather",
                                "args": { "city": "Paris" }
                            }
                        }
                    ]
                }
            }]
        });

        let data: GeminiSSEData = serde_json::from_value(payload).expect("gemini payload");
        let responses = data.into_unified_responses();

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].thinking_signature.as_deref(), Some("sig_tool"));
        assert_eq!(
            responses[0]
                .tool_call
                .as_ref()
                .and_then(|tool_call| tool_call.name.as_deref()),
            Some("get_weather")
        );
    }

    #[test]
    fn keeps_standalone_thought_signature_parts() {
        let payload = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        { "thoughtSignature": "sig_only" }
                    ]
                }
            }]
        });

        let data: GeminiSSEData = serde_json::from_value(payload).expect("gemini payload");
        let responses = data.into_unified_responses();

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].thinking_signature.as_deref(), Some("sig_only"));
        assert!(responses[0].tool_call.is_none());
        assert!(responses[0].text.is_none());
        assert!(responses[0].reasoning_content.is_none());
    }
}
