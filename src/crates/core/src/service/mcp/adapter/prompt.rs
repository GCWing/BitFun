//! MCP prompt adapter
//!
//! Integrates MCP prompts into the agent system prompt.

use crate::service::mcp::protocol::{MCPPrompt, MCPPromptContent, MCPPromptMessage};

/// Prompt adapter.
pub struct PromptAdapter;

impl PromptAdapter {
    /// Converts MCP prompt content into system prompt text.
    pub fn to_system_prompt(content: &MCPPromptContent) -> String {
        let mut prompt_parts = Vec::new();

        for message in &content.messages {
            let text = message.content.text_or_placeholder();
            match message.role.as_str() {
                "system" => prompt_parts.push(text),
                "user" => prompt_parts.push(format!("User: {}", text)),
                "assistant" => prompt_parts.push(format!("Assistant: {}", text)),
                _ => prompt_parts.push(format!("{}: {}", message.role, text)),
            }
        }

        prompt_parts.join("\n\n")
    }

    /// Returns whether a prompt is applicable to the current context.
    pub fn is_applicable(
        prompt: &MCPPrompt,
        context: &std::collections::HashMap<String, String>,
    ) -> bool {
        if let Some(arguments) = &prompt.arguments {
            for arg in arguments {
                if arg.required && !context.contains_key(&arg.name) {
                    return false;
                }
            }
        }
        true
    }

    /// Substitutes arguments in prompt messages.
    pub fn substitute_arguments(
        mut messages: Vec<MCPPromptMessage>,
        arguments: &std::collections::HashMap<String, String>,
    ) -> Vec<MCPPromptMessage> {
        for msg in &mut messages {
            msg.content.substitute_placeholders(arguments);
        }
        messages
    }
}
