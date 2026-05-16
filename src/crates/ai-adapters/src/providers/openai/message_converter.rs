//! OpenAI message format converter

use crate::types::{Message, ToolDefinition};
use log::{error, warn};
use serde_json::{json, Value};

pub struct OpenAIMessageConverter;

impl OpenAIMessageConverter {
    pub fn convert_messages_to_responses_input(
        messages: Vec<Message>,
    ) -> (Option<String>, Vec<Value>) {
        let mut instructions = Vec::new();
        let mut input = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    if let Some(content) = msg.content.filter(|content| !content.trim().is_empty())
                    {
                        instructions.push(content);
                    }
                }
                "tool" => {
                    if let Some(tool_item) = Self::convert_tool_message_to_responses_item(msg) {
                        input.push(tool_item);
                    }
                }
                "assistant" => {
                    if let Some(content_items) = Self::convert_message_content_to_responses_items(
                        &msg.role,
                        msg.content.as_deref(),
                    ) {
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
                                "arguments": tool_call.serialized_arguments(),
                            }));
                        }
                    }
                }
                role => {
                    if let Some(content_items) = Self::convert_message_content_to_responses_items(
                        role,
                        msg.content.as_deref(),
                    ) {
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
        messages
            .into_iter()
            .map(Self::convert_single_message)
            .collect()
    }

    fn convert_tool_message_to_responses_item(msg: Message) -> Option<Value> {
        let call_id = msg.tool_call_id?;
        let is_error = msg.is_error.unwrap_or(false);
        let text = msg.content.unwrap_or_default();
        let text = if is_error && !text.starts_with("[TOOL ERROR]") {
            format!("[TOOL ERROR] {}", text)
        } else {
            text
        };

        // Responses API: `output` may be a string or a list of input_text / input_image / input_file
        // (see OpenAI FunctionCallOutput schema).
        let output: Value =
            if let Some(attachments) = msg.tool_image_attachments.filter(|a| !a.is_empty()) {
                let mut parts: Vec<Value> = attachments
                    .into_iter()
                    .map(|att| {
                        let data_url = format!("data:{};base64,{}", att.mime_type, att.data_base64);
                        json!({
                            "type": "input_image",
                            "image_url": data_url
                        })
                    })
                    .collect();
                parts.push(json!({
                    "type": "input_text",
                    "text": if text.is_empty() {
                        "Tool execution completed".to_string()
                    } else {
                        text
                    }
                }));
                json!(parts)
            } else {
                json!(if text.is_empty() {
                    "Tool execution completed".to_string()
                } else {
                    text
                })
            };

        Some(json!({
            "type": "function_call_output",
            "call_id": call_id,
            "output": output,
        }))
    }

    fn convert_message_content_to_responses_items(
        role: &str,
        content: Option<&str>,
    ) -> Option<Vec<Value>> {
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
                        let image_url = item.get("image_url").and_then(|value| {
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

    fn convert_single_message(mut msg: Message) -> Value {
        // Prefix tool error content so the model can distinguish failures from normal results.
        if msg.role == "tool" && msg.is_error.unwrap_or(false) {
            if let Some(ref content) = msg.content {
                if !content.starts_with("[TOOL ERROR]") {
                    msg.content = Some(format!("[TOOL ERROR] {}", content));
                }
            }
        }

        // Chat Completions: multimodal tool message (e.g. GPT-4o vision + tools) — image parts + text.
        if msg.role == "tool" {
            if let Some(ref attachments) = msg.tool_image_attachments {
                if !attachments.is_empty() {
                    let mut parts: Vec<Value> = attachments
                        .iter()
                        .map(|att| {
                            let url = format!("data:{};base64,{}", att.mime_type, att.data_base64);
                            json!({
                                "type": "image_url",
                                "image_url": { "url": url, "detail": "auto" }
                            })
                        })
                        .collect();
                    let text = msg.content.clone().unwrap_or_default();
                    if text.trim().is_empty() {
                        parts.push(json!({
                            "type": "text",
                            "text": "Tool execution completed"
                        }));
                    } else {
                        parts.push(json!({ "type": "text", "text": text }));
                    }
                    let mut openai_msg = json!({
                        "role": "tool",
                        "content": Value::Array(parts),
                    });
                    if let Some(id) = msg.tool_call_id {
                        openai_msg["tool_call_id"] = Value::String(id);
                    }
                    if let Some(name) = msg.name {
                        openai_msg["name"] = Value::String(name);
                    }
                    return openai_msg;
                }
            }
        }

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
            // Official OpenAI Chat Completions may ignore replayed reasoning_content, but
            // many OpenAI-compatible providers require it to continue interleaved thinking.
            // Preserve even the empty-string case so providers like DeepSeek can validate the
            // original assistant turn shape on follow-up requests.
            openai_msg["reasoning_content"] = Value::String(reasoning);
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
                            "arguments": tc.serialized_arguments()
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
    use crate::types::{Message, ToolCall, ToolImageAttachment};
    use serde_json::json;

    #[test]
    fn converts_messages_to_responses_input() {
        let messages = vec![
            Message::system("You are helpful".to_string()),
            Message::user("Hello".to_string()),
            Message::assistant_with_tools(vec![ToolCall {
                id: "call_1".to_string(),
                name: "get_weather".to_string(),
                arguments: json!({"city": "Beijing"}),
                raw_arguments: None,
            }]),
            Message {
                role: "tool".to_string(),
                content: Some("Sunny".to_string()),
                reasoning_content: None,
                thinking_signature: None,
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
                name: Some("get_weather".to_string()),
                is_error: None,
                tool_image_attachments: None,
            },
        ];

        let (instructions, input) =
            OpenAIMessageConverter::convert_messages_to_responses_input(messages);

        assert_eq!(instructions.as_deref(), Some("You are helpful"));
        assert_eq!(input.len(), 3);
        assert_eq!(input[0]["type"], json!("message"));
        assert_eq!(input[1]["type"], json!("function_call"));
        assert_eq!(input[1]["arguments"], json!("{\"city\":\"Beijing\"}"));
        assert_eq!(input[2]["type"], json!("function_call_output"));
    }

    #[test]
    fn preserves_raw_tool_arguments_for_openai_replay() {
        let openai =
            OpenAIMessageConverter::convert_messages(vec![Message::assistant_with_tools(vec![
                ToolCall {
                    id: "call_1".to_string(),
                    name: "get_weather".to_string(),
                    arguments: json!({"city": "Beijing", "unit": "celsius"}),
                    raw_arguments: Some("{\"unit\":\"celsius\",\"city\":\"Beijing\"}".to_string()),
                },
            ])]);

        assert_eq!(
            openai[0]["tool_calls"][0]["function"]["arguments"],
            json!("{\"unit\":\"celsius\",\"city\":\"Beijing\"}")
        );
    }

    #[test]
    fn falls_back_to_stable_serialization_when_raw_arguments_are_invalid() {
        let openai =
            OpenAIMessageConverter::convert_messages(vec![Message::assistant_with_tools(vec![
                ToolCall {
                    id: "call_1".to_string(),
                    name: "get_weather".to_string(),
                    arguments: json!({"city": "Beijing", "unit": "celsius"}),
                    raw_arguments: Some("{\"city\":\"Beijing\"".to_string()),
                },
            ])]);

        assert_eq!(
            openai[0]["tool_calls"][0]["function"]["arguments"],
            json!("{\"city\":\"Beijing\",\"unit\":\"celsius\"}")
        );
    }

    #[test]
    fn converts_openai_style_image_content_to_responses_input() {
        let messages = vec![Message {
            role: "user".to_string(),
            content: Some(
                json!([
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
                ])
                .to_string(),
            ),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        }];

        let (_, input) = OpenAIMessageConverter::convert_messages_to_responses_input(messages);
        let content = input[0]["content"].as_array().expect("content array");

        assert_eq!(content[0]["type"], json!("input_image"));
        assert_eq!(content[1]["type"], json!("input_text"));
    }

    #[test]
    fn converts_tool_message_with_images_to_responses_function_call_output() {
        let messages = vec![Message {
            role: "tool".to_string(),
            content: Some("Screen captured".to_string()),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: Some("call_cu_1".to_string()),
            name: Some("computer_use".to_string()),
            is_error: None,
            tool_image_attachments: Some(vec![ToolImageAttachment {
                mime_type: "image/jpeg".to_string(),
                data_base64: "AAA".to_string(),
            }]),
        }];

        let (_, input) = OpenAIMessageConverter::convert_messages_to_responses_input(messages);
        let out = &input[0];
        assert_eq!(out["type"], json!("function_call_output"));
        assert_eq!(out["call_id"], json!("call_cu_1"));
        let output = out["output"].as_array().expect("multimodal output");
        assert_eq!(output[0]["type"], json!("input_image"));
        assert!(output[0]["image_url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
        assert_eq!(output[1]["type"], json!("input_text"));
        assert_eq!(output[1]["text"], json!("Screen captured"));
    }

    #[test]
    fn converts_tool_message_with_images_to_chat_completions_content_parts() {
        let msg = Message {
            role: "tool".to_string(),
            content: Some("ok".to_string()),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: Some("call_1".to_string()),
            name: Some("computer_use".to_string()),
            is_error: None,
            tool_image_attachments: Some(vec![ToolImageAttachment {
                mime_type: "image/jpeg".to_string(),
                data_base64: "YmFi".to_string(),
            }]),
        };

        let openai = OpenAIMessageConverter::convert_messages(vec![msg]);
        let content = openai[0]["content"].as_array().expect("content parts");
        assert_eq!(content[0]["type"], json!("image_url"));
        assert_eq!(content[1]["type"], json!("text"));
        assert_eq!(content[1]["text"], json!("ok"));
    }

    #[test]
    fn preserves_empty_reasoning_content_for_chat_completions() {
        let msg = Message {
            role: "assistant".to_string(),
            content: Some("Answer".to_string()),
            reasoning_content: Some(String::new()),
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        };

        let openai = OpenAIMessageConverter::convert_messages(vec![msg]);

        assert_eq!(openai[0]["reasoning_content"], json!(""));
    }
}
