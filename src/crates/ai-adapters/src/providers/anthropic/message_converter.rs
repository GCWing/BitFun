//! Anthropic message format converter
//!
//! Converts the unified message format to Anthropic Claude API format

use crate::types::{Message, ToolDefinition};
use log::warn;
use serde_json::{json, Value};

pub struct AnthropicMessageConverter;

impl AnthropicMessageConverter {
    /// Convert unified message format to Anthropic format
    ///
    /// Note: Anthropic requires system messages to be handled separately, not in the messages array
    pub fn convert_messages(messages: Vec<Message>) -> (Option<String>, Vec<Value>) {
        let mut system_message = None;
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    if let Some(content) = msg.content {
                        system_message = Some(content);
                    }
                }
                "user" => {
                    anthropic_messages.push(Self::convert_user_message(msg));
                }
                "assistant" => {
                    if let Some(converted) = Self::convert_assistant_message(msg) {
                        anthropic_messages.push(converted);
                    }
                }
                "tool" => {
                    anthropic_messages.push(Self::convert_tool_result_message(msg));
                }
                _ => {
                    warn!("Unknown message role: {}", msg.role);
                }
            }
        }

        // Anthropic requires user/assistant messages to alternate
        let merged_messages = Self::merge_consecutive_messages(anthropic_messages);

        (system_message, merged_messages)
    }

    /// Merge consecutive same-role messages to keep user/assistant alternating
    fn merge_consecutive_messages(messages: Vec<Value>) -> Vec<Value> {
        let mut merged: Vec<Value> = Vec::new();

        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

            if let Some(last) = merged.last_mut() {
                let last_role = last.get("role").and_then(|r| r.as_str()).unwrap_or("");

                if last_role == role && role == "user" {
                    let current_content = msg.get("content");
                    let last_content = last.get_mut("content");

                    match (last_content, current_content) {
                        (Some(Value::Array(last_arr)), Some(Value::Array(curr_arr))) => {
                            last_arr.extend(curr_arr.clone());
                            continue;
                        }
                        (Some(Value::Array(last_arr)), Some(Value::String(curr_str))) => {
                            last_arr.push(json!({
                                "type": "text",
                                "text": curr_str
                            }));
                            continue;
                        }
                        (Some(Value::String(last_str)), Some(Value::Array(curr_arr))) => {
                            let mut new_content = vec![json!({
                                "type": "text",
                                "text": last_str
                            })];
                            new_content.extend(curr_arr.clone());
                            *last = json!({
                                "role": "user",
                                "content": new_content
                            });
                            continue;
                        }
                        (Some(Value::String(last_str)), Some(Value::String(curr_str))) => {
                            let merged_text = if last_str.is_empty() {
                                curr_str.to_string()
                            } else {
                                format!("{}\n\n{}", last_str, curr_str)
                            };
                            *last = json!({
                                "role": "user",
                                "content": merged_text
                            });
                            continue;
                        }
                        _ => {}
                    }
                }
            }

            merged.push(msg);
        }

        merged
    }

    fn convert_user_message(msg: Message) -> Value {
        let content = msg.content.unwrap_or_default();

        if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
            if parsed.is_array() {
                return json!({
                    "role": "user",
                    "content": parsed
                });
            }
        }

        json!({
            "role": "user",
            "content": content
        })
    }

    /// Convert assistant messages; return None when empty.
    fn convert_assistant_message(msg: Message) -> Option<Value> {
        let mut content = Vec::new();

        if msg.reasoning_content.is_some() || msg.thinking_signature.is_some() {
            let mut thinking_block = json!({
                "type": "thinking",
                "thinking": msg.reasoning_content.as_deref().unwrap_or("")
            });

            thinking_block["signature"] = json!(msg.thinking_signature.as_deref().unwrap_or(""));

            content.push(thinking_block);
        }

        if let Some(text) = msg.content {
            if !text.is_empty() {
                content.push(json!({
                    "type": "text",
                    "text": text
                }));
            }
        }

        if let Some(tool_calls) = msg.tool_calls {
            for tc in tool_calls {
                content.push(json!({
                    "type": "tool_use",
                    "id": tc.id,
                    "name": tc.name,
                    "input": tc.arguments
                }));
            }
        }

        if content.is_empty() {
            None
        } else {
            Some(json!({
                "role": "assistant",
                "content": content
            }))
        }
    }

    fn convert_tool_result_message(msg: Message) -> Value {
        let tool_call_id = msg.tool_call_id.unwrap_or_default();
        let text = msg.content.unwrap_or_default();

        let is_error = msg.is_error.unwrap_or(false);
        let tool_content: Value =
            if let Some(attachments) = msg.tool_image_attachments.filter(|a| !a.is_empty()) {
                let mut blocks: Vec<Value> = attachments
                    .into_iter()
                    .map(|att| {
                        json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": att.mime_type,
                                "data": att.data_base64,
                            }
                        })
                    })
                    .collect();
                blocks.push(json!({ "type": "text", "text": text }));
                json!(blocks)
            } else {
                json!(text)
            };

        let mut tool_result = json!({
            "type": "tool_result",
            "tool_use_id": tool_call_id,
            "content": tool_content,
        });
        if is_error {
            tool_result["is_error"] = json!(true);
        }

        json!({
            "role": "user",
            "content": [tool_result]
        })
    }

    /// Convert tool definitions to Anthropic format
    pub fn convert_tools(tools: Option<Vec<ToolDefinition>>) -> Option<Vec<Value>> {
        tools.map(|tool_defs| {
            tool_defs
                .into_iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "description": tool.description,
                        "input_schema": tool.parameters
                    })
                })
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::AnthropicMessageConverter;
    use crate::types::Message;
    use serde_json::json;

    #[test]
    fn preserves_empty_thinking_block_when_signature_exists() {
        let msg = Message {
            role: "assistant".to_string(),
            content: Some("Answer".to_string()),
            reasoning_content: Some(String::new()),
            thinking_signature: Some("sig_1".to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        };

        let (_, messages) = AnthropicMessageConverter::convert_messages(vec![msg]);
        let content = messages[0]["content"]
            .as_array()
            .expect("assistant content");

        assert_eq!(content[0]["type"], json!("thinking"));
        assert_eq!(content[0]["thinking"], json!(""));
        assert_eq!(content[0]["signature"], json!("sig_1"));
    }
}
