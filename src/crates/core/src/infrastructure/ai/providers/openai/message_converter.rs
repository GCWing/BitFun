//! OpenAI message format converter

use crate::util::types::{Message, ToolDefinition};
use log::{error, warn};
use serde_json::{json, Value};

pub struct OpenAIMessageConverter;

impl OpenAIMessageConverter {
    pub fn convert_messages(messages: Vec<Message>) -> Vec<Value> {
        messages
            .into_iter()
            .map(Self::convert_single_message)
            .collect()
    }

    fn convert_single_message(msg: Message) -> Value {
        let mut openai_msg = json!({
            "role": msg.role,
        });

        let has_tool_calls = msg.tool_calls.is_some();

        if let Some(content) = msg.content {
            if content.trim().is_empty() {
                if msg.role == "assistant" && has_tool_calls {
                    // OpenAI requires the content field; use a space for tool-call cases.
                    openai_msg["content"] = Value::String(" ".to_string());
                } else if msg.role == "tool" {
                    openai_msg["content"] = Value::String("Tool execution completed".to_string());
                    warn!(
                        "[OpenAI] Tool response content is empty: name={:?}",
                        msg.name
                    );
                } else {
                    openai_msg["content"] = Value::String(" ".to_string());
                    warn!("[OpenAI] Message content is empty: role={}", msg.role);
                }
            } else {
                if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
                    if parsed.is_array() {
                        openai_msg["content"] = parsed;
                    } else {
                        openai_msg["content"] = Value::String(content);
                    }
                } else {
                    openai_msg["content"] = Value::String(content);
                }
            }
        } else {
            if msg.role == "assistant" && has_tool_calls {
                // OpenAI requires the content field; use a space for tool-call cases.
                openai_msg["content"] = Value::String(" ".to_string());
            } else if msg.role == "tool" {
                openai_msg["content"] = Value::String("Tool execution completed".to_string());

                warn!(
                    "[OpenAI] Tool response message content is empty, set to default: name={:?}",
                    msg.name
                );
            } else {
                error!(
                    "[OpenAI] Message content is empty and violates API spec: role={}, has_tool_calls={}", 
                    msg.role, 
                    has_tool_calls
                );

                openai_msg["content"] = Value::String(" ".to_string());
            }
        }

        if let Some(reasoning) = msg.reasoning_content {
            if !reasoning.is_empty() {
                openai_msg["reasoning_content"] = Value::String(reasoning);
            }
        }

        if let Some(tool_calls) = msg.tool_calls {
            let openai_tool_calls: Vec<Value> = tool_calls
                .into_iter()
                .map(|tc| {
                    json!({
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.name,
                            "arguments": serde_json::to_string(&tc.arguments)
                                .unwrap_or_default()
                        }
                    })
                })
                .collect();
            openai_msg["tool_calls"] = Value::Array(openai_tool_calls);
        }

        if let Some(tool_call_id) = msg.tool_call_id {
            openai_msg["tool_call_id"] = Value::String(tool_call_id);
        }

        if let Some(name) = msg.name {
            openai_msg["name"] = Value::String(name);
        }

        openai_msg
    }

    pub fn convert_tools(tools: Option<Vec<ToolDefinition>>) -> Option<Vec<Value>> {
        tools.map(|tool_defs| {
            tool_defs
                .into_iter()
                .map(|tool| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.parameters
                        }
                    })
                })
                .collect()
        })
    }
}
