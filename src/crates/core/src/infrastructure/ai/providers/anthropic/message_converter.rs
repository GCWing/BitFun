//! Anthropic message format converter
//!
//! Converts the unified message format to Anthropic Claude API format

use crate::util::types::{Message, ToolDefinition};
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

        if let Some(thinking) = msg.reasoning_content.as_ref() {
            if !thinking.is_empty() {
                let mut thinking_block = json!({
                    "type": "thinking",
                    "thinking": thinking
                });

                // Append only when signature exists, to support APIs that do not require it.
                if let Some(ref sig) = msg.thinking_signature {
                    if !sig.is_empty() {
                        thinking_block["signature"] = json!(sig);
                    }
                }

                content.push(thinking_block);
            }
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
        let content = msg.content.unwrap_or_default();

        json!({
            "role": "user",
            "content": [{
                "type": "tool_result",
                "tool_use_id": tool_call_id,
                "content": content
            }]
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
