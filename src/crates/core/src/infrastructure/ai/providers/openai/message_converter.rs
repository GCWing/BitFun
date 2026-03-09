//! OpenAI message format converter

use log::{warn, error};
use crate::util::types::{Message, ToolDefinition};
use serde_json::{json, Value};

pub struct OpenAIMessageConverter;

impl OpenAIMessageConverter {
    pub fn convert_messages_to_responses_input(messages: Vec<Message>) -> (Option<String>, Vec<Value>) {
        let mut instructions = Vec::new();
        let mut input = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    if let Some(content) = msg.content.filter(|content| !content.trim().is_empty()) {
                        instructions.push(content);
                    }
                }
                "tool" => {
                    if let Some(tool_item) = Self::convert_tool_message_to_responses_item(msg) {
                        input.push(tool_item);
                    }
                }
                "assistant" => {
                    if let Some(content_items) = Self::convert_message_content_to_responses_items(&msg.role, msg.content.as_deref()) {
                        input.push(json!({
                            "type": "message",
                            "role": "assistant",
                            "content": content_items,
                        }));
                    }

                    if let Some(tool_calls) = msg.tool_calls {
                        for tool_call in tool_calls {
                            input.push(json!({
                                "type": "function_call",
                                "call_id": tool_call.id,
                                "name": tool_call.name,
                                "arguments": serde_json::to_string(&tool_call.arguments)
                                    .unwrap_or_else(|_| "{}".to_string()),
                            }));
                        }
                    }
                }
                role => {
                    if let Some(content_items) = Self::convert_message_content_to_responses_items(role, msg.content.as_deref()) {
                        input.push(json!({
                            "type": "message",
                            "role": role,
                            "content": content_items,
                        }));
                    }
                }
            }
        }

        let instructions = if instructions.is_empty() {
            None
        } else {
            Some(instructions.join("\n\n"))
        };

        (instructions, input)
    }

    pub fn convert_messages(messages: Vec<Message>) -> Vec<Value> {
        messages.into_iter()
            .map(Self::convert_single_message)
            .collect()
    }

    fn convert_tool_message_to_responses_item(msg: Message) -> Option<Value> {
        let call_id = msg.tool_call_id?;
        let output = msg.content.unwrap_or_else(|| "Tool execution completed".to_string());

        Some(json!({
            "type": "function_call_output",
            "call_id": call_id,
            "output": output,
        }))
    }

    fn convert_message_content_to_responses_items(role: &str, content: Option<&str>) -> Option<Vec<Value>> {
        let content = content?;
        let text_item_type = Self::responses_text_item_type(role);

        if content.trim().is_empty() {
            return Some(vec![json!({
                "type": text_item_type,
                "text": " ",
            })]);
        }

        let parsed = match serde_json::from_str::<Value>(content) {
            Ok(parsed) if parsed.is_array() => parsed,
            _ => {
                return Some(vec![json!({
                    "type": text_item_type,
                    "text": content,
                })]);
            }
        };

        let mut content_items = Vec::new();

        if let Some(items) = parsed.as_array() {
            for item in items {
                let item_type = item.get("type").and_then(Value::as_str);
                match item_type {
                    Some("text") | Some("input_text") | Some("output_text") => {
                        if let Some(text) = item.get("text").and_then(Value::as_str) {
                            content_items.push(json!({
                                "type": text_item_type,
                                "text": text,
                            }));
                        }
                    }
                    Some("image_url") if role != "assistant" => {
                        let image_url = item
                            .get("image_url")
                            .and_then(|value| {
                                value
                                    .get("url")
                                    .and_then(Value::as_str)
                                    .or_else(|| value.as_str())
                            });

                        if let Some(image_url) = image_url {
                            content_items.push(json!({
                                "type": "input_image",
                                "image_url": image_url,
                            }));
                        }
                    }
                    _ => {}
                }
            }
        }

        if content_items.is_empty() {
            Some(vec![json!({
                "type": text_item_type,
                "text": content,
            })])
        } else {
            Some(content_items)
        }
    }

    fn responses_text_item_type(role: &str) -> &'static str {
        if role == "assistant" {
            "output_text"
        } else {
            "input_text"
        }
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
                    warn!(
                        "[OpenAI] Message content is empty: role={}", 
                        msg.role
                    );
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

#[cfg(test)]
mod tests {
    use super::OpenAIMessageConverter;
    use crate::util::types::{Message, ToolCall};
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn converts_messages_to_responses_input() {
        let mut args = HashMap::new();
        args.insert("city".to_string(), json!("Beijing"));

        let messages = vec![
            Message::system("You are helpful".to_string()),
            Message::user("Hello".to_string()),
            Message::assistant_with_tools(vec![ToolCall {
                id: "call_1".to_string(),
                name: "get_weather".to_string(),
                arguments: args.clone(),
            }]),
            Message {
                role: "tool".to_string(),
                content: Some("Sunny".to_string()),
                reasoning_content: None,
                thinking_signature: None,
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
                name: Some("get_weather".to_string()),
            },
        ];

        let (instructions, input) = OpenAIMessageConverter::convert_messages_to_responses_input(messages);

        assert_eq!(instructions.as_deref(), Some("You are helpful"));
        assert_eq!(input.len(), 3);
        assert_eq!(input[0]["type"], json!("message"));
        assert_eq!(input[1]["type"], json!("function_call"));
        assert_eq!(input[2]["type"], json!("function_call_output"));
    }

    #[test]
    fn converts_openai_style_image_content_to_responses_input() {
        let messages = vec![Message {
            role: "user".to_string(),
            content: Some(json!([
                {
                    "type": "image_url",
                    "image_url": {
                        "url": "data:image/png;base64,abc"
                    }
                },
                {
                    "type": "text",
                    "text": "Describe this image"
                }
            ]).to_string()),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];

        let (_, input) = OpenAIMessageConverter::convert_messages_to_responses_input(messages);
        let content = input[0]["content"].as_array().expect("content array");

        assert_eq!(content[0]["type"], json!("input_image"));
        assert_eq!(content[1]["type"], json!("input_text"));
    }
}
