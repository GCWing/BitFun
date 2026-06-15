//! Context compressor
//!
//! Responsible only for transforming a session context into a compressed one.

use super::fallback::{
    build_structured_compression_summary_with_contract, CompressionFallbackOptions,
    CompressionSummaryArtifact,
};
use crate::agentic::core::{
    render_system_reminder, CompressedMessage, CompressedMessageRole, CompressedTodoSnapshot,
    CompressionContract, CompressionEntry, CompressionPayload, Message, MessageContent,
    MessageHelper, MessageRole, MessageSemanticKind,
};
use crate::util::errors::BitFunResult;
use log::{debug, trace};

/// Context compressor configuration
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    pub fallback_max_tokens_ratio: f32,
    pub fallback_user_chars: usize,
    pub fallback_assistant_chars: usize,
    pub fallback_tool_arg_chars: usize,
    pub fallback_tool_command_chars: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            fallback_max_tokens_ratio: 0.25,
            fallback_user_chars: 1000,
            fallback_assistant_chars: 1000,
            fallback_tool_arg_chars: 100,
            fallback_tool_command_chars: 100,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TurnWithTokens {
    messages: Vec<Message>,
}

impl TurnWithTokens {
    fn new(messages: Vec<Message>) -> Self {
        Self { messages }
    }
}

#[derive(Debug, Clone)]
pub struct CompressionResult {
    pub messages: Vec<Message>,
    pub has_model_summary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMode {
    Auto,
    Manual,
}

/// Stateless context compression service.
pub struct ContextCompressor {
    config: CompressionConfig,
}

impl ContextCompressor {
    pub fn new(config: CompressionConfig) -> Self {
        Self { config }
    }

    fn collect_conversation_turns(
        &self,
        session_id: &str,
        mut messages: Vec<Message>,
    ) -> BitFunResult<Vec<TurnWithTokens>> {
        debug!(
            "Collecting conversation turns for compression: session_id={}",
            session_id
        );

        let message_start = {
            let mut start_idx = messages.len();
            for (idx, msg) in messages.iter().enumerate() {
                if msg.role != MessageRole::System {
                    start_idx = idx;
                    break;
                }
            }
            start_idx
        };
        let all_messages = messages.split_off(message_start);

        if all_messages.is_empty() {
            debug!(
                "Session context is empty, no compression candidates: session_id={}",
                session_id
            );
            return Ok(Vec::new());
        }

        let mut turns_messages = MessageHelper::group_messages_by_turns(all_messages);
        let turns_count = turns_messages.len();
        let turns_tokens: Vec<usize> = turns_messages
            .iter_mut()
            .map(|turn| turn.iter_mut().map(|m| m.get_tokens()).sum::<usize>())
            .collect();
        let turns_msg_num: Vec<usize> = turns_messages.iter().map(|turn| turn.len()).collect();
        debug!(
            "Session has {} turn(s), messages per turn: {:?}, tokens per turn: {:?}",
            turns_count, turns_msg_num, turns_tokens
        );

        Ok(turns_messages
            .into_iter()
            .map(TurnWithTokens::new)
            .collect())
    }

    /// Collect all non-system conversation turns for an automatic compression pass.
    pub fn collect_turns_for_auto_compression(
        &self,
        session_id: &str,
        messages: Vec<Message>,
    ) -> BitFunResult<Vec<TurnWithTokens>> {
        debug!(
            "Starting session context compression analysis: session_id={}",
            session_id
        );

        let turns = self.collect_conversation_turns(session_id, messages)?;
        if turns.is_empty() {
            return Ok(Vec::new());
        }

        Ok(turns)
    }

    /// Collect all non-system conversation turns for a full manual compaction pass.
    pub fn collect_all_turns_for_manual_compaction(
        &self,
        session_id: &str,
        messages: Vec<Message>,
    ) -> BitFunResult<Vec<TurnWithTokens>> {
        self.collect_conversation_turns(session_id, messages)
    }

    pub fn compress_turns(
        &self,
        session_id: &str,
        context_window: usize,
        turns: Vec<TurnWithTokens>,
        mode: CompressionMode,
        model_summary: Option<String>,
    ) -> BitFunResult<CompressionResult> {
        self.compress_turns_with_contract(
            session_id,
            context_window,
            turns,
            mode,
            None,
            model_summary,
        )
    }

    pub fn compress_turns_with_contract(
        &self,
        session_id: &str,
        context_window: usize,
        turns: Vec<TurnWithTokens>,
        mode: CompressionMode,
        contract: Option<CompressionContract>,
        model_summary: Option<String>,
    ) -> BitFunResult<CompressionResult> {
        if turns.is_empty() {
            debug!("No turns need compression: session_id={}", session_id);
            return Ok(CompressionResult {
                messages: Vec::new(),
                has_model_summary: false,
            });
        }

        let Some(last_turn_messages) = turns.last().map(|turn| &turn.messages) else {
            debug!(
                "No turns available after collection, skipping compression: session_id={}",
                session_id
            );
            return Ok(CompressionResult {
                messages: Vec::new(),
                has_model_summary: false,
            });
        };
        let last_user_message = last_turn_messages
            .iter()
            .find(|message| message.is_actual_user_message())
            .cloned();
        let last_todo = MessageHelper::get_last_todo_snapshot(last_turn_messages);
        trace!("Last user message: {:?}", last_user_message);
        trace!("Last todo: {:?}", last_todo);
        let mut summary_artifact = match model_summary {
            Some(summary) => self.build_model_summary_artifact(summary, contract),
            None => self.build_fallback_summary_artifact(turns, context_window, contract),
        };
        if matches!(mode, CompressionMode::Auto) {
            self.append_live_boundary_context(
                &mut summary_artifact,
                last_user_message.as_ref(),
                last_todo.as_ref(),
            );
        }
        trace!("Compression summary artifact generated");
        let has_model_summary = summary_artifact.used_model_summary;
        let (boundary_message, summary_message) = self.create_summary_turn(summary_artifact);
        let compressed_messages = vec![boundary_message, summary_message];

        debug!(
            "Compression completed: session_id={}, compressed_messages={}",
            session_id,
            compressed_messages.len()
        );

        Ok(CompressionResult {
            messages: compressed_messages,
            has_model_summary,
        })
    }

    fn create_summary_turn(
        &self,
        summary_artifact: CompressionSummaryArtifact,
    ) -> (Message, Message) {
        let boundary = Message::user(render_system_reminder(&Self::render_boundary_marker_text(
            summary_artifact.used_model_summary,
        )))
        .with_semantic_kind(MessageSemanticKind::CompressionBoundaryMarker);

        let summary = Message::assistant(summary_artifact.summary_text)
            .with_semantic_kind(MessageSemanticKind::CompressionSummary)
            .with_compression_payload(summary_artifact.payload);

        (boundary, summary)
    }

    fn append_live_boundary_context(
        &self,
        summary_artifact: &mut CompressionSummaryArtifact,
        last_user_message: Option<&Message>,
        todo_snapshot: Option<&CompressedTodoSnapshot>,
    ) {
        let mut additions = Vec::new();
        let mut payload_messages = Vec::new();

        if let Some(last_user_text) =
            last_user_message.and_then(Self::render_boundary_user_message_text)
        {
            additions.push(format!(
                "Most recent user message before this summary:\n{}",
                last_user_text
            ));
            payload_messages.push(CompressedMessage {
                role: CompressedMessageRole::User,
                text: Some(last_user_text),
                tool_calls: Vec::new(),
            });
        }

        let todo_text = todo_snapshot
            .map(Self::render_todo_snapshot)
            .unwrap_or_default();
        if !todo_text.is_empty() {
            additions.push(format!(
                "Most recent task list snapshot before this summary:\n{}",
                todo_text
            ));
        }

        if additions.is_empty() {
            return;
        }

        summary_artifact.summary_text = format!(
            "{}\n\n{}",
            summary_artifact.summary_text.trim_end(),
            additions.join("\n\n")
        );
        summary_artifact
            .payload
            .entries
            .push(CompressionEntry::Turn {
                turn_id: None,
                messages: payload_messages,
                todo: todo_snapshot.cloned(),
            });
    }

    fn render_boundary_user_message_text(message: &Message) -> Option<String> {
        let text = match &message.content {
            MessageContent::Text(text) => text.trim(),
            MessageContent::Multimodal { text, .. } => text.trim(),
            _ => return None,
        };

        (!text.is_empty()).then(|| text.to_string())
    }

    fn render_todo_snapshot(todo_snapshot: &CompressedTodoSnapshot) -> String {
        if todo_snapshot.todos.is_empty() {
            return todo_snapshot.summary.clone().unwrap_or_default();
        }

        let mut lines: Vec<String> = todo_snapshot
            .todos
            .iter()
            .map(|todo| format!("- [{}] {}", todo.status, todo.content))
            .collect();

        if let Some(summary) = &todo_snapshot.summary {
            if !summary.trim().is_empty() {
                lines.push(format!("Task list note: {}", summary.trim()));
            }
        }

        lines.join("\n")
    }

    fn render_boundary_marker_text(used_model_summary: bool) -> String {
        let mut msg = "The earlier conversation is summarized in the next assistant message. Use it as prior context.".to_string();
        if !used_model_summary {
            msg.push_str(" This is a partial reconstructed record. Message text, tool arguments, task lists, and tool results may be truncated or omitted.");
        }
        msg
    }

    fn build_model_summary_artifact(
        &self,
        summary: String,
        contract: Option<CompressionContract>,
    ) -> CompressionSummaryArtifact {
        trace!("Compression summary: {}", summary);
        let mut payload = CompressionPayload::from_summary(summary.clone());
        let summary_text = if let Some(contract) = contract.filter(|contract| !contract.is_empty())
        {
            payload.entries.insert(
                0,
                CompressionEntry::Contract {
                    contract: contract.clone(),
                },
            );
            format!(
                "{}\n\nSummary of the earlier conversation:\n{}",
                contract.render_for_model(),
                summary
            )
        } else {
            format!("Summary of the earlier conversation:\n{}", summary)
        };

        CompressionSummaryArtifact {
            summary_text,
            payload,
            used_model_summary: true,
        }
    }

    fn build_fallback_summary_artifact(
        &self,
        turns_to_compress: Vec<TurnWithTokens>,
        context_window: usize,
        contract: Option<CompressionContract>,
    ) -> CompressionSummaryArtifact {
        build_structured_compression_summary_with_contract(
            turns_to_compress
                .into_iter()
                .map(|turn| turn.messages)
                .collect(),
            &self.build_fallback_options(context_window),
            contract,
        )
    }

    fn build_fallback_options(&self, context_window: usize) -> CompressionFallbackOptions {
        CompressionFallbackOptions {
            max_tokens: ((context_window as f32 * self.config.fallback_max_tokens_ratio) as usize)
                .max(256),
            user_chars: self.config.fallback_user_chars,
            assistant_chars: self.config.fallback_assistant_chars,
            tool_arg_chars: self.config.fallback_tool_arg_chars,
            tool_command_chars: self.config.fallback_tool_command_chars,
        }
    }

    pub(crate) fn normalize_model_summary_output(raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        if let Some(summary) = extract_tag_content(trimmed, "summary") {
            let summary = summary.trim();
            if !summary.is_empty() {
                return Some(summary.to_string());
            }
        }

        if trimmed.contains("<analysis>") {
            return None;
        }

        Some(trimmed.to_string())
    }

    pub(crate) fn build_compact_prompt(&self, contract: Option<&CompressionContract>) -> String {
        let contract_instruction = contract
            .filter(|contract| !contract.is_empty())
            .map(|contract| {
                format!(
                    "\n\nThe following compaction contract is authoritative factual context from tool observations. Preserve every field from it in the final <summary>:\n{}\n",
                    contract.render_for_model()
                )
            })
            .unwrap_or_default();

        format!(
            r#"Your current task is to create a detailed summary of the conversation so far, paying close attention to the user's explicit requests and your previous actions.
This summary should be thorough in capturing technical details, code patterns, and architectural decisions that would be essential for continuing development work without losing context.
{contract_instruction}

CRITICAL: Respond with TEXT ONLY. Do NOT call any tools.

- Do NOT use Read, Bash, Grep, Glob, Edit, Write, or ANY other tool.
- You already have all the context you need in the conversation above.
- Tool calls will be REJECTED and will waste your only turn — you will fail the task.
- Your entire response must be plain text: an <analysis> block followed by a <summary> block.

Before providing your final summary, wrap your analysis in <analysis> tags to organize your thoughts and ensure you've covered all necessary points. Then output the final retained summary in <summary> tags.
Important: only the content inside <summary> will be kept as compressed history. The <analysis> section is transient and will be discarded, so do not put any required final information only in <analysis>.
In your analysis process:

1. Chronologically analyze each message and section of the conversation. For each section thoroughly identify:
   - The user's explicit requests and intents
   - Your approach to addressing the user's requests
   - Key decisions, technical concepts and code patterns
   - Specific details like:
     - file names
     - full code snippets
     - function signatures
     - file edits
   - Errors that you ran into and how you fixed them
   - Pay special attention to specific user feedback that you received, especially if the user told you to do something differently.
2. Double-check for technical accuracy and completeness, addressing each required element thoroughly.

Your summary should include the following sections:

1. Primary Request and Intent: Capture all of the user's explicit requests and intents in detail
2. Key Technical Concepts: List all important technical concepts, technologies, and frameworks discussed.
3. Files and Code Sections: Enumerate specific files and code sections examined, modified, or created. Pay special attention to the most recent messages and include full code snippets where applicable and include a summary of why this file read or edit is important.
4. Errors and fixes: List all errors that you ran into, and how you fixed them. Pay special attention to specific user feedback that you received, especially if the user told you to do something differently.
5. Problem Solving: Document problems solved and any ongoing troubleshooting efforts.
6. All user messages: List ALL user messages that are not tool results. These are critical for understanding the users' feedback and changing intent.
7. Pending Tasks: Outline any pending tasks that you have explicitly been asked to work on.
8. Current Work: Describe in detail precisely what was being worked on immediately before this summary request, paying special attention to the most recent messages from both user and assistant. Include file names and code snippets where applicable.
9. Optional Next Step: List the next step that you will take that is related to the most recent work you were doing. IMPORTANT: ensure that this step is DIRECTLY in line with the user's most recent explicit requests, and the task you were working on immediately before this summary request. If your last task was concluded, then only list next steps if they are explicitly in line with the users request. Do not start on tangential requests or really old requests that were already completed without confirming with the user first. If there is a next step, include direct quotes from the most recent conversation showing exactly what task you were working on and where you left off. This should be verbatim to ensure there's no drift in task interpretation.

Here's an example of how your output should be structured:

<example>
<analysis>
[Your thought process, ensuring all points are covered thoroughly and accurately]
</analysis>

<summary>
1. Primary Request and Intent:
   [Detailed description]

2. Key Technical Concepts:
   - [Concept 1]
   - [Concept 2]
   - [...]

3. Files and Code Sections:
   - [File Name 1]
      - [Summary of why this file is important]
      - [Summary of the changes made to this file, if any]
      - [Important Code Snippet]
   - [File Name 2]
      - [Important Code Snippet]
   - [...]

4. Errors and fixes:
    - [Detailed description of error 1]:
      - [How you fixed the error]
      - [User feedback on the error if any]
    - [...]

5. Problem Solving:
   [Description of solved problems and ongoing troubleshooting]

6. All user messages:
    - [Detailed non tool use user message]
    - [...]

7. Pending Tasks:
   - [Task 1]
   - [Task 2]
   - [...]

8. Current Work:
   [Precise description of current work]

9. Optional Next Step:
   [Optional Next step to take]

</summary>
</example>

Please provide your summary based on the conversation so far, following this structure and ensuring precision and thoroughness in your response.
REMINDER: Do NOT call any tools. Respond with plain text only — an <analysis> block followed by a <summary> block. Tool calls will be rejected and you will fail the task.
"#
        )
    }
}

fn extract_tag_content<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)?;
    let after_open = &text[start + open.len()..];
    let end = after_open.find(&close)?;
    Some(&after_open[..end])
}

#[cfg(test)]
mod tests {
    use super::{CompressionMode, ContextCompressor, TurnWithTokens};
    use crate::agentic::core::{
        render_system_reminder, CompressionContract, CompressionContractItem, CompressionEntry,
        CompressionPayload, Message, MessageSemanticKind,
    };

    fn make_turn(messages: Vec<Message>) -> TurnWithTokens {
        TurnWithTokens::new(messages)
    }

    fn todo_turn() -> TurnWithTokens {
        make_turn(vec![
            Message::user("Continue the refactor".to_string()),
            Message::assistant_with_tools(
                "Planning next steps".to_string(),
                vec![crate::agentic::core::ToolCall {
                    tool_id: "todo_1".to_string(),
                    tool_name: "TodoWrite".to_string(),
                    arguments: serde_json::json!({
                        "todos": [
                            {"content": "Update compressor", "status": "in_progress"},
                            {"content": "Add regression tests", "status": "pending"}
                        ]
                    }),
                    raw_arguments: None,
                    is_error: false,
                    recovered_from_truncation: false,
                }],
            ),
        ])
    }

    #[test]
    fn manual_compression_creates_closed_compression_turn() {
        let compressor = ContextCompressor::new(Default::default());
        let result = compressor
            .compress_turns(
                "session",
                8000,
                vec![todo_turn()],
                CompressionMode::Manual,
                None,
            )
            .expect("compression succeeds");

        assert_eq!(result.messages.len(), 2);
        assert_eq!(
            result.messages[0].metadata.semantic_kind,
            Some(MessageSemanticKind::CompressionBoundaryMarker)
        );
        assert_eq!(
            result.messages[1].metadata.semantic_kind,
            Some(MessageSemanticKind::CompressionSummary)
        );

        let boundary_text = match &result.messages[0].content {
            crate::agentic::core::MessageContent::Text(text) => text,
            _ => panic!("expected boundary marker text"),
        };
        assert!(boundary_text.contains("partial reconstructed record"));

        let summary_text = match &result.messages[1].content {
            crate::agentic::core::MessageContent::Text(text) => text,
            _ => panic!("expected assistant text summary"),
        };
        assert!(summary_text.contains("Continue the refactor"));
    }

    #[test]
    fn auto_compression_appends_latest_user_and_todo_into_summary_turn() {
        let compressor = ContextCompressor::new(Default::default());
        let result = compressor
            .compress_turns(
                "session",
                8000,
                vec![todo_turn()],
                CompressionMode::Auto,
                Some("Model summary".to_string()),
            )
            .expect("compression succeeds");

        assert_eq!(result.messages.len(), 2);
        let summary_text = match &result.messages[1].content {
            crate::agentic::core::MessageContent::Text(text) => text,
            _ => panic!("expected assistant text summary"),
        };
        assert!(summary_text.contains("Model summary"));
        assert!(summary_text.contains("Most recent user message before this summary"));
        assert!(summary_text.contains("Continue the refactor"));
        assert!(summary_text.contains("Most recent task list snapshot before this summary"));
    }

    #[test]
    fn synthetic_summary_turn_payload_remains_atomic_on_recompression() {
        let marker = Message::user(render_system_reminder(
            "Earlier conversation was compressed.",
        ))
        .with_semantic_kind(MessageSemanticKind::CompressionBoundaryMarker);
        let summary = Message::assistant("Summary text".to_string())
            .with_semantic_kind(MessageSemanticKind::CompressionSummary)
            .with_compression_payload(CompressionPayload::from_summary("Summary text".to_string()));

        let summary_artifact =
            crate::agentic::session::compression::fallback::build_structured_compression_summary(
                vec![vec![marker, summary]],
                &crate::agentic::session::compression::fallback::CompressionFallbackOptions {
                    max_tokens: 10_000,
                    user_chars: 120,
                    assistant_chars: 120,
                    tool_arg_chars: 80,
                    tool_command_chars: 80,
                },
            );

        assert!(matches!(
            &summary_artifact.payload.entries[0],
            CompressionEntry::ModelSummary { text } if text == "Summary text"
        ));
    }

    #[test]
    fn model_summary_prompt_includes_compaction_contract() {
        let compressor = ContextCompressor::new(Default::default());
        let contract = CompressionContract {
            touched_files: vec!["src/lib.rs".to_string()],
            verification_commands: vec![CompressionContractItem {
                target: "cargo test".to_string(),
                status: "succeeded".to_string(),
                summary: "Tests passed.".to_string(),
                error_kind: None,
            }],
            blocking_failures: Vec::new(),
            subagent_statuses: Vec::new(),
        };

        let prompt = compressor.build_compact_prompt(Some(&contract));

        assert!(prompt.contains("authoritative factual context"));
        assert!(prompt.contains("src/lib.rs"));
        assert!(prompt.contains("cargo test"));
    }

    #[test]
    fn model_summary_output_uses_summary_tag_body_only() {
        let normalized = ContextCompressor::normalize_model_summary_output(
            "<analysis>\ninternal reasoning\n</analysis>\n<summary>\nFinal summary\n</summary>",
        );

        assert_eq!(normalized.as_deref(), Some("Final summary"));
    }

    #[test]
    fn model_summary_output_without_tags_keeps_plain_text() {
        let normalized =
            ContextCompressor::normalize_model_summary_output("Plain summary without tags");

        assert_eq!(normalized.as_deref(), Some("Plain summary without tags"));
    }

    #[test]
    fn model_summary_output_with_analysis_but_no_summary_is_rejected() {
        let normalized = ContextCompressor::normalize_model_summary_output(
            "<analysis>\ninternal reasoning\n</analysis>",
        );

        assert_eq!(normalized, None);
    }

    #[test]
    fn auto_turn_collection_keeps_single_active_turn() {
        let compressor = ContextCompressor::new(Default::default());
        let messages = vec![
            Message::system("system".to_string()),
            Message::user("First request".to_string()),
            Message::assistant("First reply".to_string()),
        ];

        let turns = compressor
            .collect_turns_for_auto_compression("session", messages)
            .expect("collection succeeds");

        assert_eq!(turns.len(), 1);
    }

    #[test]
    fn manual_compaction_turn_collection_includes_all_non_system_turns() {
        let compressor = ContextCompressor::new(Default::default());
        let messages = vec![
            Message::system("system".to_string()),
            Message::user("First request".to_string()),
            Message::assistant("First reply".to_string()),
            Message::user("Second request".to_string()),
            Message::assistant("Second reply".to_string()),
        ];

        let manual_turns = compressor
            .collect_all_turns_for_manual_compaction("session", messages.clone())
            .expect("manual collection succeeds");
        let passive_turns = compressor
            .collect_turns_for_auto_compression("session", messages)
            .expect("passive collection succeeds");

        assert_eq!(manual_turns.len(), 2);
        assert_eq!(manual_turns.len(), passive_turns.len());
    }
}
