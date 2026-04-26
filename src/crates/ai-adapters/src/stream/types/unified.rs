use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::borrow::Cow;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedToolCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_index: Option<usize>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<String>,
    #[serde(default)]
    pub arguments_is_snapshot: bool,
}

/// Unified AI response format
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct UnifiedResponse {
    pub text: Option<String>,
    pub reasoning_content: Option<String>,
    /// Signature for Anthropic extended thinking (returned in multi-turn conversations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_signature: Option<String>,
    pub tool_call: Option<UnifiedToolCall>,
    pub usage: Option<UnifiedTokenUsage>,
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<Value>,
}

impl fmt::Debug for UnifiedResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reasoning_summary = self.reasoning_content.as_ref().map(|s| {
            if s.len() > 100 {
                let end = s
                    .char_indices()
                    .take_while(|(i, _)| *i < 100)
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(0);
                // Guard against multi-byte chars pushing end past the string length
                let end = end.min(s.len());
                Cow::Owned(format!("{}... ({} bytes)", &s[..end], s.len()))
            } else {
                Cow::Borrowed(s.as_str())
            }
        });
        f.debug_struct("UnifiedResponse")
            .field("text", &self.text)
            .field("reasoning_content", &reasoning_summary)
            .field("thinking_signature", &"<omitted>")
            .field("tool_call", &self.tool_call)
            .field("usage", &self.usage)
            .field("finish_reason", &self.finish_reason)
            .field("provider_metadata", &"<omitted>")
            .finish()
    }
}

/// Unified token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedTokenUsage {
    pub prompt_token_count: u32,
    pub candidates_token_count: u32,
    pub total_token_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_token_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_content_token_count: Option<u32>,
}
