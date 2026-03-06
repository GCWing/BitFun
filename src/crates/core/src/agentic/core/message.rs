use crate::agentic::image_analysis::ImageContextData;
use crate::util::types::{Message as AIMessage, ToolCall as AIToolCall};
use crate::util::TokenCounter;
use log::warn;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

// ============ Message ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: MessageContent,
    pub timestamp: SystemTime,
    pub metadata: MessageMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    Tool,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    Text(String),
    Multimodal {
        text: String,
        images: Vec<ImageContextData>,
    },
    ToolResult {
        tool_id: String,
        tool_name: String,
        result: serde_json::Value,
        result_for_assistant: Option<String>,
        is_error: bool,
    },
    Mixed {
        /// Reasoning content (for interleaved thinking mode)
        reasoning_content: Option<String>,
        text: String,
        tool_calls: Vec<ToolCall>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub turn_id: Option<String>,
    pub round_id: Option<String>,
    pub tokens: Option<usize>,
    #[serde(skip)] // Not serialized, auxiliary field for runtime use only
    pub keep_thinking: bool,
    /// Anthropic extended thinking signature (for passing back in multi-turn conversations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_signature: Option<String>,
}

impl From<Message> for AIMessage {
    fn from(msg: Message) -> Self {
        let role = match msg.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
            MessageRole::System => "system",
        };
        let keep_thinking = msg.metadata.keep_thinking;
        let thinking_signature = msg.metadata.thinking_signature.clone();

        match msg.content {
            MessageContent::Text(text) => {
                // Check if text is empty to avoid sending empty content to API
                let content = if text.trim().is_empty() {
                    // Should not have empty text messages, but provide default value for defensive programming
                    warn!("Empty text message detected: role={}", role);
                    if role == "user" {
                        Some("(empty message)".to_string())
                    } else if role == "system" {
                        Some("You are a helpful assistant.".to_string())
                    } else {
                        Some(" ".to_string()) // Minimum valid value
                    }
                } else {
                    Some(text)
                };

                Self {
                    role: role.to_string(),
                    content,
                    reasoning_content: None,
                    thinking_signature: None,
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                }
            }
            MessageContent::Multimodal { text, images } => {
                let mut content = text;
                if !images.is_empty() {
                    content.push_str("\n\n[Attached image(s):\n");
                    for image in images {
                        let name = image
                            .metadata
                            .as_ref()
                            .and_then(|m| m.get("name"))
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .map(str::to_string)
                            .or_else(|| {
                                image
                                    .image_path
                                    .as_ref()
                                    .filter(|s| !s.is_empty())
                                    .cloned()
                            })
                            .unwrap_or_else(|| image.id.clone());

                        content.push_str(&format!("- {} ({})\n", name, image.mime_type));
                    }
                    content.push(']');
                }

                Self {
                    role: "user".to_string(),
                    content: Some(content),
                    reasoning_content: None,
                    thinking_signature: None,
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                }
            }
            MessageContent::Mixed {
                reasoning_content,
                text,
                tool_calls,
            } => {
                let converted_tool_calls = if tool_calls.is_empty() {
                    // Set to None when tool_call is empty to avoid deepseek model errors
                    None
                } else {
                    Some(
                        tool_calls
                            .into_iter()
                            .map(|tc| {
                                // Convert serde_json::Value to HashMap
                                let arguments = if let serde_json::Value::Object(map) = tc.arguments
                                {
                                    map.into_iter().map(|(k, v)| (k, v)).collect()
                                } else {
                                    std::collections::HashMap::new()
                                };

                                AIToolCall {
                                    id: tc.tool_id,
                                    name: tc.tool_name,
                                    arguments,
                                }
                            })
                            .collect(),
                    )
                };

                // When there are tool_calls, empty text should use None
                let content = if text.trim().is_empty() {
                    None // OpenAI API allows content to be null when assistant + tool_calls
                } else {
                    Some(text)
                };

                // Reasoning content (interleaved thinking mode)
                let reasoning = if keep_thinking {
                    reasoning_content.filter(|r| !r.is_empty())
                } else {
                    None
                };

                Self {
                    role: "assistant".to_string(),
                    content,
                    reasoning_content: reasoning,
                    thinking_signature: thinking_signature.clone(),
                    tool_calls: converted_tool_calls,
                    tool_call_id: None,
                    name: None,
                }
            }
            MessageContent::ToolResult {
                tool_id,
                tool_name,
                result,
                result_for_assistant,
                ..
            } => {
                // Tool messages must include tool_call_id
                // Prefer result_for_assistant (text specifically for AI), if None or empty then use result (data field)
                let content_for_ai = if let Some(assistant_text) = result_for_assistant {
                    // Check if empty string
                    if assistant_text.trim().is_empty() {
                        // If empty, use serialized result
                        serde_json::to_string(&result)
                            .unwrap_or(format!("Tool {} execution completed", tool_name))
                    } else {
                        assistant_text
                    }
                } else {
                    // If no result_for_assistant, use serialized result
                    serde_json::to_string(&result)
                        .unwrap_or(format!("Tool {} execution completed", tool_name))
                };

                Self {
                    role: "tool".to_string(),
                    content: Some(content_for_ai),
                    reasoning_content: None,
                    thinking_signature: None,
                    tool_calls: None,
                    tool_call_id: Some(tool_id),
                    name: Some(tool_name),
                }
            }
        }
    }
}

impl From<&Message> for AIMessage {
    fn from(msg: &Message) -> Self {
        // Reference version calls owned version after clone to avoid duplicate logic
        AIMessage::from(msg.clone())
    }
}

impl Message {
    pub fn system(text: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::System,
            content: MessageContent::Text(text),
            timestamp: SystemTime::now(),
            metadata: MessageMetadata::default(),
        }
    }

    pub fn user(text: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content: MessageContent::Text(text),
            timestamp: SystemTime::now(),
            metadata: MessageMetadata::default(),
        }
    }

    pub fn user_multimodal(text: String, images: Vec<ImageContextData>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content: MessageContent::Multimodal { text, images },
            timestamp: SystemTime::now(),
            metadata: MessageMetadata::default(),
        }
    }

    pub fn assistant(text: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content: MessageContent::Text(text),
            timestamp: SystemTime::now(),
            metadata: MessageMetadata::default(),
        }
    }

    pub fn assistant_with_tools(text: String, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content: MessageContent::Mixed {
                reasoning_content: None,
                text,
                tool_calls,
            },
            timestamp: SystemTime::now(),
            metadata: MessageMetadata::default(),
        }
    }

    /// Create assistant message with reasoning content (supports interleaved thinking mode)
    pub fn assistant_with_reasoning(
        reasoning_content: Option<String>,
        text: String,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content: MessageContent::Mixed {
                reasoning_content,
                text,
                tool_calls,
            },
            timestamp: SystemTime::now(),
            metadata: MessageMetadata::default(),
        }
    }

    pub fn tool_result(result: ToolResult) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::Tool,
            content: MessageContent::ToolResult {
                tool_id: result.tool_id.clone(),
                tool_name: result.tool_name.clone(),
                result: result.result.clone(),
                result_for_assistant: result.result_for_assistant.clone(),
                is_error: result.is_error,
            },
            timestamp: SystemTime::now(),
            metadata: MessageMetadata::default(),
        }
    }

    /// Check if message is a user message (role is user and not <system-reminder>)
    pub fn is_actual_user_message(&self) -> bool {
        if self.role != MessageRole::User {
            return false;
        }
        let text = match &self.content {
            MessageContent::Text(text) => Some(text.as_str()),
            MessageContent::Multimodal { text, .. } => Some(text.as_str()),
            _ => None,
        };
        if text.is_some_and(|t| t.starts_with("<system-reminder>")) {
            return false;
        }
        true
    }

    /// Set message's turn_id (to identify which dialog turn the message belongs to)
    pub fn with_turn_id(mut self, turn_id: String) -> Self {
        self.metadata.turn_id = Some(turn_id);
        self
    }

    /// Set message's round_id (to identify which model round the message belongs to)
    pub fn with_round_id(mut self, round_id: String) -> Self {
        self.metadata.round_id = Some(round_id);
        self
    }

    /// Set message's thinking_signature (for Anthropic extended thinking multi-turn conversations)
    pub fn with_thinking_signature(mut self, signature: Option<String>) -> Self {
        self.metadata.thinking_signature = signature;
        self
    }

    /// Get message's token count
    pub fn get_tokens(&mut self) -> usize {
        if let Some(tokens) = self.metadata.tokens {
            return tokens;
        }
        let tokens = self.estimate_tokens();
        self.metadata.tokens = Some(tokens);
        tokens
    }

    fn estimate_image_tokens(metadata: Option<&serde_json::Value>) -> usize {
        let (width, height) = metadata
            .and_then(|m| {
                let w = m.get("width").and_then(|v| v.as_u64());
                let h = m.get("height").and_then(|v| v.as_u64());
                match (w, h) {
                    (Some(w), Some(h)) if w > 0 && h > 0 => Some((w as u32, h as u32)),
                    _ => None,
                }
            })
            .unwrap_or((1024, 1024));

        let tiles_w = (width + 511) / 512;
        let tiles_h = (height + 511) / 512;
        let tiles = (tiles_w.max(1) * tiles_h.max(1)) as usize;
        50 + tiles * 200
    }

    fn estimate_tokens(&self) -> usize {
        let mut total = 0usize;
        total += 4;

        match &self.content {
            MessageContent::Text(text) => {
                total += TokenCounter::estimate_tokens(text);
            }
            MessageContent::Multimodal { text, images } => {
                total += TokenCounter::estimate_tokens(text);
                for image in images {
                    total += Self::estimate_image_tokens(image.metadata.as_ref());
                }
            }
            MessageContent::Mixed {
                reasoning_content,
                text,
                tool_calls,
            } => {
                if self.metadata.keep_thinking {
                    if let Some(reasoning) = reasoning_content.as_ref() {
                        total += TokenCounter::estimate_tokens(reasoning);
                    }
                }
                total += TokenCounter::estimate_tokens(text);

                for tool_call in tool_calls {
                    total += TokenCounter::estimate_tokens(&tool_call.tool_name);
                    if let Ok(json_str) = serde_json::to_string(&tool_call.arguments) {
                        total += TokenCounter::estimate_tokens(&json_str);
                    }
                    total += 10;
                }
            }
            MessageContent::ToolResult {
                tool_name,
                result,
                result_for_assistant,
                ..
            } => {
                if let Some(text) = result_for_assistant.as_ref().filter(|s| !s.is_empty()) {
                    total += TokenCounter::estimate_tokens(text);
                } else if let Ok(json_str) = serde_json::to_string(result) {
                    total += TokenCounter::estimate_tokens(&json_str);
                } else {
                    total += TokenCounter::estimate_tokens(tool_name);
                }
            }
        }

        total
    }
}

impl ToString for MessageContent {
    fn to_string(&self) -> String {
        match self {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Multimodal { text, images } => format!(
                "Multimodal: text_length={}, images={}",
                text.len(),
                images.len()
            ),
            MessageContent::ToolResult {
                tool_id,
                tool_name,
                result,
                result_for_assistant,
                is_error,
            } => {
                format!(
                    "ToolResult: tool_id={}, tool_name={}, result={}, result_for_assistant={:?}, is_error={}",
                    tool_id, tool_name, result, result_for_assistant, is_error
                )
            }
            MessageContent::Mixed {
                reasoning_content,
                text,
                tool_calls,
            } => {
                format!(
                    "Mixed: reasoning_content={:?}, text={}, tool_calls={}",
                    reasoning_content,
                    text,
                    tool_calls
                        .iter()
                        .map(|tc| format!(
                            "ToolCall: tool_id={}, tool_name={}, arguments={}",
                            tc.tool_id, tc.tool_name, tc.arguments
                        ))
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
        }
    }
}

// ============ Tool Calls and Results ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    /// Record whether tool parameters are valid
    pub is_error: bool,
    /// Record whether tool is a should_end_turn tool, to avoid frequent tool_registry calls
    #[serde(skip)]
    pub should_end_turn: bool,
}

impl ToolCall {
    pub fn is_valid(&self) -> bool {
        !self.tool_id.is_empty() && !self.tool_name.is_empty() && !self.is_error
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_id: String,
    pub tool_name: String,
    pub result: serde_json::Value,
    /// Result text specifically for passing to AI assistant (if None, then use result)
    pub result_for_assistant: Option<String>,
    pub is_error: bool,
    pub duration_ms: Option<u64>,
}

impl From<ToolCall> for AIToolCall {
    fn from(tc: ToolCall) -> Self {
        // Convert serde_json::Value to HashMap
        let arguments = if let serde_json::Value::Object(map) = &tc.arguments {
            map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        } else {
            std::collections::HashMap::new()
        };

        Self {
            id: tc.tool_id.clone(),
            name: tc.tool_name.clone(),
            arguments,
        }
    }
}
