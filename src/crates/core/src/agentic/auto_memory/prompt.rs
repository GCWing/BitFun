use crate::agentic::core::{Message, MessageRole, MessageSemanticKind};
use crate::service::memory_store::{
    build_global_memory_policy_sections, build_workspace_memory_policy_sections, MemoryScope,
    SharedMemoryPolicyProfile,
};

pub fn count_recent_model_visible_messages(
    messages: &[Message],
    since_turn_id: Option<&str>,
) -> usize {
    let boundary_index = since_turn_id.and_then(|turn_id| {
        messages
            .iter()
            .enumerate()
            .rev()
            .find(|(_, message)| message.metadata.turn_id.as_deref() == Some(turn_id))
            .map(|(index, _)| index)
    });

    let count = messages
        .iter()
        .enumerate()
        .filter(|(index, _)| boundary_index.map_or(true, |boundary| *index > boundary))
        .filter(|(_, message)| is_model_visible_message(message))
        .count();

    count.max(1)
}

fn is_model_visible_message(message: &Message) -> bool {
    if matches!(
        message.metadata.semantic_kind,
        Some(MessageSemanticKind::ComputerUseVerificationScreenshot)
            | Some(MessageSemanticKind::ComputerUsePostActionSnapshot)
    ) {
        return false;
    }

    matches!(
        message.role,
        MessageRole::User | MessageRole::Assistant | MessageRole::Tool
    )
}

pub fn build_extract_prompt(
    recent_message_count: usize,
    memory_dir: &str,
    existing_memories: Option<&str>,
    memory_scope: MemoryScope,
) -> String {
    let manifest = existing_memories
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            format!(
                "\n\n## Existing memory files\n\n{}\n\nCheck this list before writing — update an existing file rather than creating a duplicate.",
                value.trim()
            )
        })
        .unwrap_or_default();

    format!(
        "You are now acting as the memory extraction subagent. Analyze the most recent ~{recent_message_count} messages above and use them to update your persistent memory systems.\n\n\
Available tools: Read, Grep, Glob, and Write/Edit/Delete for paths inside `{memory_dir}` only. All other tools will be denied.\n\n\
You have a limited turn budget. Edit requires a prior Read of the same file, so the efficient strategy is: turn 1 — issue all Read calls in parallel for every file you might update; turn 2 — issue all Write/Edit/Delete calls in parallel. Do not interleave reads and writes across multiple turns.\n\n\
You MUST only use content from the last ~{recent_message_count} messages to update your persistent memories. Do not waste any turns attempting to investigate or verify that content further — no grepping source files, no reading code to confirm a pattern exists, no git commands.\n\n\
The conversation may not contain anything worth adding to or changing in memory. If there is nothing to update, respond with exactly `Nothing to update`.\n\n\
If you do update memory, do not include a summary of what changed. A brief confirmation that the update is complete is enough.{manifest}\n\n{}",
        match memory_scope {
            MemoryScope::WorkspaceProject => build_workspace_memory_policy_sections(
                "MEMORY.md",
                SharedMemoryPolicyProfile::Extraction,
            ),
            MemoryScope::GlobalAgenticOs => build_global_memory_policy_sections(
                "MEMORY.md",
                SharedMemoryPolicyProfile::Extraction,
            ),
        }
    )
}

#[cfg(test)]
mod tests {
    use super::{build_extract_prompt, count_recent_model_visible_messages};
    use crate::agentic::core::{Message, MessageSemanticKind, ToolCall, ToolResult};
    use serde_json::json;

    #[test]
    fn counts_runtime_message_flow_including_tool_results() {
        let messages = vec![
            Message::user("old user".to_string()).with_turn_id("turn-1".to_string()),
            Message::assistant_with_tools(
                "calling tool".to_string(),
                vec![ToolCall {
                    tool_id: "tool-1".to_string(),
                    tool_name: "Read".to_string(),
                    arguments: json!({ "file_path": "a.txt" }),
                    is_error: false,
                }],
            )
            .with_turn_id("turn-2".to_string()),
            Message::tool_result(ToolResult {
                tool_id: "tool-1".to_string(),
                tool_name: "Read".to_string(),
                result: json!({ "content": "hello" }),
                result_for_assistant: Some("hello".to_string()),
                is_error: false,
                duration_ms: None,
                image_attachments: None,
            })
            .with_turn_id("turn-2".to_string()),
            Message::assistant("final answer".to_string()).with_turn_id("turn-2".to_string()),
        ];

        assert_eq!(
            count_recent_model_visible_messages(&messages, Some("turn-1")),
            3
        );
    }

    #[test]
    fn falls_back_to_all_visible_messages_when_boundary_turn_is_missing() {
        let messages = vec![
            Message::user("user".to_string()).with_turn_id("turn-2".to_string()),
            Message::assistant("assistant".to_string()).with_turn_id("turn-2".to_string()),
        ];

        assert_eq!(
            count_recent_model_visible_messages(&messages, Some("missing-turn")),
            2
        );
    }

    #[test]
    fn uses_last_message_of_boundary_turn_before_counting() {
        let messages = vec![
            Message::user("turn one".to_string()).with_turn_id("turn-1".to_string()),
            Message::assistant("answer one".to_string()).with_turn_id("turn-1".to_string()),
            Message::user("turn two".to_string()).with_turn_id("turn-2".to_string()),
            Message::assistant("answer two".to_string()).with_turn_id("turn-2".to_string()),
        ];

        assert_eq!(
            count_recent_model_visible_messages(&messages, Some("turn-1")),
            2
        );
    }

    #[test]
    fn excludes_system_and_non_model_visible_runtime_messages() {
        let messages = vec![
            Message::system("system".to_string()),
            Message::user("user".to_string()).with_turn_id("turn-1".to_string()),
            Message::assistant("verification".to_string())
                .with_turn_id("turn-1".to_string())
                .with_semantic_kind(MessageSemanticKind::ComputerUseVerificationScreenshot),
            Message::assistant("answer".to_string()).with_turn_id("turn-1".to_string()),
        ];

        assert_eq!(count_recent_model_visible_messages(&messages, None), 2);
    }

    #[test]
    fn extract_prompt_omits_full_memory_access_guidance_sections() {
        let prompt = build_extract_prompt(
            7,
            "/workspace/memory",
            None,
            crate::service::memory_store::MemoryScope::WorkspaceProject,
        );

        assert!(
            !prompt.contains("If the user explicitly asks you to remember something"),
            "extract prompt should not include explicit request guidance"
        );
        assert!(
            !prompt.contains("## When to access memories"),
            "extract prompt should not include access-memory guidance"
        );
        assert!(
            !prompt.contains("## Before recommending from memory"),
            "extract prompt should not include post-access memory guidance"
        );
        assert!(
            !prompt.contains("## Memory and other forms of persistence"),
            "extract prompt should stop before memory persistence guidance"
        );
    }

    #[test]
    fn global_extract_prompt_uses_global_memory_policy() {
        let prompt = build_extract_prompt(
            7,
            "/global/memory",
            None,
            crate::service::memory_store::MemoryScope::GlobalAgenticOs,
        );

        assert!(prompt.contains("## Special workspace overview files"));
        assert!(!prompt.contains("<name>project</name>"));
        assert!(!prompt.contains("## When to access memories"));
    }
}
