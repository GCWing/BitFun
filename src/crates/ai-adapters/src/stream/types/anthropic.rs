use super::unified::{UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct MessageStart {
    pub message: Message,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Usage {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
    cache_read_input_tokens: Option<u32>,
    cache_creation_input_tokens: Option<u32>,
}

impl Usage {
    pub fn update(&mut self, other: &Usage) {
        if other.input_tokens.is_some() {
            self.input_tokens = other.input_tokens;
        }
        if other.output_tokens.is_some() {
            self.output_tokens = other.output_tokens;
        }
        if other.cache_read_input_tokens.is_some() {
            self.cache_read_input_tokens = other.cache_read_input_tokens;
        }
        if other.cache_creation_input_tokens.is_some() {
            self.cache_creation_input_tokens = other.cache_creation_input_tokens;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.cache_read_input_tokens.is_none()
            && self.cache_creation_input_tokens.is_none()
    }
}

impl From<Usage> for UnifiedTokenUsage {
    fn from(value: Usage) -> Self {
        let cache_read = value.cache_read_input_tokens.unwrap_or(0);
        let cache_creation = value.cache_creation_input_tokens.unwrap_or(0);
        let prompt_token_count = value.input_tokens.unwrap_or(0) + cache_read + cache_creation;
        let candidates_token_count = value.output_tokens.unwrap_or(0);
        Self {
            prompt_token_count,
            candidates_token_count,
            total_token_count: prompt_token_count + candidates_token_count,
            reasoning_token_count: None,
            cached_content_token_count: match (
                value.cache_read_input_tokens,
                value.cache_creation_input_tokens,
            ) {
                (None, None) => None,
                (read, creation) => Some(read.unwrap_or(0) + creation.unwrap_or(0)),
            },
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct MessageDelta {
    pub delta: MessageDeltaDelta,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct MessageDeltaDelta {
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
}

impl From<MessageDelta> for UnifiedResponse {
    fn from(value: MessageDelta) -> Self {
        Self {
            text: None,
            reasoning_content: None,
            thinking_signature: None,
            tool_call: None,
            usage: value.usage.map(UnifiedTokenUsage::from),
            finish_reason: value.delta.stop_reason,
            provider_metadata: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ContentBlockStart {
    pub content_block: ContentBlock,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "thinking")]
    Thinking,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
    #[serde(other)]
    Unknown,
}

impl From<ContentBlockStart> for UnifiedResponse {
    fn from(value: ContentBlockStart) -> Self {
        let mut result = UnifiedResponse::default();
        if let ContentBlock::ToolUse { id, name } = value.content_block {
            let tool_call = UnifiedToolCall {
                tool_call_index: None,
                id: Some(id),
                name: Some(name),
                arguments: None,
                arguments_is_snapshot: false,
            };
            result.tool_call = Some(tool_call);
        }
        result
    }
}

#[derive(Debug, Deserialize)]
pub struct ContentBlockDelta {
    delta: Delta,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Delta {
    #[serde(rename = "thinking_delta")]
    Thinking { thinking: String },
    #[serde(rename = "text_delta")]
    Text { text: String },
    #[serde(rename = "input_json_delta")]
    InputJson { partial_json: String },
    #[serde(rename = "signature_delta")]
    Signature { signature: String },
    #[serde(other)]
    Unknown,
}

impl TryFrom<ContentBlockDelta> for UnifiedResponse {
    type Error = String;
    fn try_from(value: ContentBlockDelta) -> Result<Self, Self::Error> {
        let mut result = UnifiedResponse::default();
        match value.delta {
            Delta::Thinking { thinking } => {
                result.reasoning_content = Some(thinking);
            }
            Delta::Text { text } => {
                result.text = Some(text);
            }
            Delta::InputJson { partial_json } => {
                let tool_call = UnifiedToolCall {
                    tool_call_index: None,
                    id: None,
                    name: None,
                    arguments: Some(partial_json),
                    arguments_is_snapshot: false,
                };
                result.tool_call = Some(tool_call);
            }
            Delta::Signature { signature } => {
                result.thinking_signature = Some(signature);
            }
            Delta::Unknown => {
                return Err("Unsupported anthropic delta type".to_string());
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Deserialize)]
pub struct AnthropicSSEError {
    pub error: AnthropicSSEErrorDetails,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicSSEErrorDetails {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

impl From<AnthropicSSEErrorDetails> for String {
    fn from(value: AnthropicSSEErrorDetails) -> Self {
        format!("{}: {}", value.error_type, value.message)
    }
}
