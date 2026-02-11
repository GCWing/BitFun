//! Token estimation utility

use crate::util::types::{Message, ToolDefinition};

/// Heuristic: ASCII chars 0.3 token, non-ASCII chars 0.6 token
pub struct TokenCounter;

impl TokenCounter {
    pub fn estimate_tokens(text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }

        let mut token_count: f32 = 0.;

        for c in text.chars() {
            if c.is_ascii() {
                token_count += 0.3;
            } else {
                token_count += 0.6;
            }
        }

        token_count as usize
    }

    pub fn estimate_message_tokens(message: &Message) -> usize {
        let mut total = 0;

        total += 4;

        if let Some(reasoning_content) = &message.reasoning_content {
            total += Self::estimate_tokens(reasoning_content);
        }

        if let Some(content) = &message.content {
            total += Self::estimate_tokens(content);
        }

        if let Some(tool_calls) = &message.tool_calls {
            for tool_call in tool_calls {
                total += Self::estimate_tokens(&tool_call.name);
                if let Ok(json_str) = serde_json::to_string(&tool_call.arguments) {
                    total += Self::estimate_tokens(&json_str);
                }
                total += 10;
            }
        }

        if let Some(name) = &message.name {
            total += Self::estimate_tokens(name);
        }

        total
    }

    pub fn estimate_messages_tokens(messages: &[Message]) -> usize {
        let mut total: usize = messages.iter().map(Self::estimate_message_tokens).sum();

        total += 3;

        total
    }

    pub fn estimate_tool_definitions_tokens(tools: &[ToolDefinition]) -> usize {
        let mut total = 0;

        for tool in tools {
            total += Self::estimate_tokens(&tool.name);
            total += Self::estimate_tokens(&tool.description);

            if let Ok(json_str) = serde_json::to_string(&tool.parameters) {
                total += Self::estimate_tokens(&json_str);
            }

            total += 15;
        }

        if !tools.is_empty() {
            total += 10;
        }

        total
    }

    pub fn estimate_request_tokens(
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> usize {
        let mut total = Self::estimate_messages_tokens(messages);

        if let Some(tool_defs) = tools {
            total += Self::estimate_tool_definitions_tokens(tool_defs);
        }

        total
    }
}
