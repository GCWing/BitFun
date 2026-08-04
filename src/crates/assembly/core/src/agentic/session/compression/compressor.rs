//! Context compressor
//!
//! Responsible only for transforming a session context into a compressed one.

use super::fallback::{
    build_structured_compression_summary_with_contract, CompressionFallbackOptions,
    CompressionSummaryArtifact,
};
use crate::agentic::core::{
    render_system_reminder, CompressionContract, CompressionEntry, CompressionPayload,
    InternalReminderKind, Message, MessageContent, MessageHelper, MessageRole, MessageSemanticKind,
};
use crate::service::session::TranscriptLineRange;
use crate::util::errors::BitFunResult;
use log::{debug, trace};
use std::borrow::Cow;

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
struct TurnWithTokens {
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

#[derive(Debug, Clone)]
pub struct CompressionPlan {
    pub summary_request_messages: Vec<Message>,
    pub summary_messages: Vec<Message>,
    pub retained_user_messages: Vec<Message>,
    pub recent_tail_messages: Vec<Message>,
    pub retained_user_token_budget: usize,
    pub retained_user_tokens: usize,
    pub recent_target_tokens: usize,
    pub recent_tail_tokens: usize,
    pub cutoff_message_index: usize,
    pub next_recent_target_tokens: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct AtomicMessageUnit {
    start: usize,
    tokens: usize,
}

/// Stateless context compression service.
pub struct ContextCompressor {
    config: CompressionConfig,
}

impl ContextCompressor {
    pub const DEFAULT_RECENT_CONTEXT_TOKENS: usize = 10_000;
    pub const RECENT_CONTEXT_RETRY_STEP_TOKENS: usize = 10_000;
    const MAX_RETAINED_USER_TOKENS: usize = 20_000;
    const COMPRESSION_CONTINUATION_REMINDER: &'static str =
        "This conversation was compacted. Re-establish the working state from the retained user messages, summary, and recent context. Continue any unfinished work; otherwise use this context for the user's next request. Do not ask the user to repeat information already retained here. If a required detail is missing and a pre-compaction transcript is available, inspect the relevant part before proceeding.";

    pub fn new(config: CompressionConfig) -> Self {
        Self { config }
    }

    pub fn plan_compression(
        &self,
        session_id: &str,
        runtime_messages: &[Message],
        context_window: usize,
        recent_target_tokens: usize,
    ) -> BitFunResult<Option<CompressionPlan>> {
        let runtime_messages = if runtime_messages.iter().any(|message| {
            message
                .internal_reminder_kind()
                .is_some_and(InternalReminderKind::should_drop_during_compaction)
        }) {
            Cow::Owned(
                runtime_messages
                    .iter()
                    .filter(|message| {
                        !message
                            .internal_reminder_kind()
                            .is_some_and(InternalReminderKind::should_drop_during_compaction)
                    })
                    .cloned()
                    .collect(),
            )
        } else {
            Cow::Borrowed(runtime_messages)
        };
        let runtime_messages = runtime_messages.as_ref();
        let system_message_count = runtime_messages
            .iter()
            .take_while(|message| message.role == MessageRole::System)
            .count();
        let conversation = &runtime_messages[system_message_count..];
        if conversation.is_empty() {
            debug!(
                "No conversation messages available for automatic compression planning: session_id={}",
                session_id
            );
            return Ok(None);
        }

        let units = Self::atomic_message_units(conversation);
        if units.is_empty() {
            return Ok(None);
        }

        let minimum_cutoff = if units.len() > 1 {
            units[1].start
        } else {
            conversation.len()
        };
        let mut cutoff = conversation.len();
        let mut accumulated_tokens = 0usize;
        let mut next_recent_target_tokens = None;

        for unit in units.iter().rev() {
            if unit.start < minimum_cutoff {
                break;
            }
            let next_tokens = accumulated_tokens.saturating_add(unit.tokens);
            if next_tokens > recent_target_tokens {
                next_recent_target_tokens = Some(next_tokens);
                break;
            }
            cutoff = unit.start;
            accumulated_tokens = next_tokens;
        }

        let summary_messages = conversation[..cutoff].to_vec();
        if summary_messages.is_empty() {
            debug!(
                "Automatic compression plan has no summary prefix: session_id={}, recent_target_tokens={}",
                session_id, recent_target_tokens
            );
            return Ok(None);
        }

        let recent_tail_messages = conversation[cutoff..].to_vec();
        let recent_tail_tokens = recent_tail_messages
            .iter()
            .map(|message| message.estimate_tokens_with_reasoning(true))
            .sum();
        let mut summary_request_messages = runtime_messages[..system_message_count].to_vec();
        summary_request_messages.extend(summary_messages.clone());

        let retained_user_token_budget = (context_window / 10).min(Self::MAX_RETAINED_USER_TOKENS);
        let (retained_user_messages, retained_user_tokens) =
            Self::retain_historical_user_messages(&summary_messages, retained_user_token_budget);
        debug!(
            "Compression plan: session_id={}, retained_user_token_budget={}, retained_user_tokens={}, retained_user_messages={}, recent_target_tokens={}, recent_tail_tokens={}, cutoff_message_index={}, summary_messages={}, recent_tail_messages={}, next_recent_target_tokens={:?}",
            session_id,
            retained_user_token_budget,
            retained_user_tokens,
            retained_user_messages.len(),
            recent_target_tokens,
            recent_tail_tokens,
            cutoff,
            summary_messages.len(),
            recent_tail_messages.len(),
            next_recent_target_tokens
        );

        Ok(Some(CompressionPlan {
            summary_request_messages,
            summary_messages,
            retained_user_messages,
            recent_tail_messages,
            retained_user_token_budget,
            retained_user_tokens,
            recent_target_tokens,
            recent_tail_tokens,
            cutoff_message_index: cutoff,
            next_recent_target_tokens,
        }))
    }

    fn atomic_message_units(messages: &[Message]) -> Vec<AtomicMessageUnit> {
        let mut units = Vec::new();
        let mut index = 0usize;

        while index < messages.len() {
            let start = index;
            index += 1;
            if messages[start].role == MessageRole::Assistant {
                while index < messages.len() && messages[index].role == MessageRole::Tool {
                    index += 1;
                }
            }
            let tokens = messages[start..index]
                .iter()
                .map(|message| message.estimate_tokens_with_reasoning(true))
                .sum();
            units.push(AtomicMessageUnit { start, tokens });
        }

        units
    }

    fn retain_historical_user_messages(
        summary_messages: &[Message],
        token_budget: usize,
    ) -> (Vec<Message>, usize) {
        let mut retained = Vec::new();
        let mut retained_tokens = 0usize;

        for message in summary_messages
            .iter()
            .rev()
            .filter(|message| message.is_actual_user_message())
        {
            let message_tokens = message.estimate_tokens_with_reasoning(true);
            if retained_tokens.saturating_add(message_tokens) > token_budget {
                break;
            }
            retained_tokens += message_tokens;
            retained.push(message.clone());
        }

        retained.reverse();
        (retained, retained_tokens)
    }

    pub fn compress_plan_with_contract(
        &self,
        session_id: &str,
        context_window: usize,
        plan: CompressionPlan,
        contract: Option<CompressionContract>,
        model_summary: Option<String>,
    ) -> BitFunResult<CompressionResult> {
        let turns = MessageHelper::group_messages_by_turns(plan.summary_messages);
        let turns = turns.into_iter().map(TurnWithTokens::new).collect();
        let summary_artifact = match model_summary {
            Some(summary) => self.build_model_summary_artifact(summary, contract),
            None => self.build_fallback_summary_artifact(turns, context_window, contract),
        };
        let has_model_summary = summary_artifact.used_model_summary;
        let summary_message = self.create_summary_message(summary_artifact);
        let mut messages = plan.retained_user_messages;
        messages.push(summary_message);
        messages.extend(plan.recent_tail_messages);
        messages.push(Message::internal_reminder(
            InternalReminderKind::CompressionContinuation,
            Self::COMPRESSION_CONTINUATION_REMINDER,
        ));

        debug!(
            "Compression completed: session_id={}, compressed_messages={}",
            session_id,
            messages.len()
        );
        Ok(CompressionResult {
            messages,
            has_model_summary,
        })
    }

    pub fn append_transcript_reference(
        &self,
        result: &mut CompressionResult,
        transcript_uri: &str,
        transcript_index_range: &TranscriptLineRange,
    ) -> bool {
        let Some(summary) = result.messages.iter_mut().find(|message| {
            message.metadata.semantic_kind == Some(MessageSemanticKind::CompressionSummary)
        }) else {
            return false;
        };
        let MessageContent::Text(text) = &mut summary.content else {
            return false;
        };
        let Some(reminder_end) = text.rfind("\n</system_reminder>") else {
            return false;
        };
        let transcript_text =
            Self::render_transcript_reference_text(transcript_uri, transcript_index_range);
        text.insert_str(reminder_end, &format!("\n\n{transcript_text}"));
        summary.metadata.tokens = None;
        true
    }

    fn create_summary_message(&self, summary_artifact: CompressionSummaryArtifact) -> Message {
        let boundary_text = Self::render_boundary_marker_text(summary_artifact.used_model_summary);
        let content = render_system_reminder(&format!(
            "{}\n\n{}",
            boundary_text, summary_artifact.summary_text
        ));
        Message::user(content)
            .with_semantic_kind(MessageSemanticKind::CompressionSummary)
            .with_compression_payload(summary_artifact.payload)
    }

    fn render_boundary_marker_text(used_model_summary: bool) -> String {
        let mut msg = "Some earlier user messages were retained verbatim, and the remaining earlier conversation has been summarized below. Use both as prior context."
            .to_string();
        if !used_model_summary {
            msg.push_str(" This is a partial reconstructed record. Message text, tool arguments, task lists, and tool results may be truncated or omitted.");
        }
        msg
    }

    fn render_transcript_reference_text(
        transcript_uri: &str,
        index_range: &TranscriptLineRange,
    ) -> String {
        format!(
            "The complete conversation history from before compression is available at:\n{}\nThe transcript index is at lines {}-{}. Inspect this file if historical details are needed. The file may be large. Recommended ways to access it: 1. Read the index first, then inspect the relevant line ranges. 2. Use Grep to search for the needed content.",
            transcript_uri, index_range.start_line, index_range.end_line
        )
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
                "{}\n\nSummary of the earlier conversation:\n<summary>\n{}\n</summary>",
                contract.render_for_model(),
                summary
            )
        } else {
            format!(
                "Summary of the earlier conversation:\n<summary>\n{}\n</summary>",
                summary
            )
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
        let summary = raw.trim();
        (!summary.is_empty()).then(|| summary.to_string())
    }

    pub(crate) fn build_compact_prompt(&self) -> String {
        String::from(
            r#"You are performing a CONTEXT CHECKPOINT COMPACTION. Create a handoff summary for another LLM that will resume the task.

Include:
- Current progress and key decisions made
- Important context, constraints, or user preferences
- What remains to be done (clear next steps)
- Any critical data, examples, or references needed to continue

Be concise, structured, and focused on helping the next LLM seamlessly continue the work.

Note: Preserve durable, task-specific state, but do not reproduce information that can be obtained again from its source:
- Do not paste large file contents, long code blocks, command output, logs, tool results, or other bulky source material. Record the file path or source reference, plus a one-sentence description of its purpose or relevant contents. Include only a small exact snippet when it is essential and cannot be reliably reconstructed.
- Do not copy Skill instructions or other reloadable guidance. Record the Skill name, why it is relevant, and that the next LLM should reload it when needed.

IMPORTANT: This is a summary-only turn. Do not call tools or perform additional work. Respond with the handoff summary as plain text. Any tool call will be rejected and you will fail the task.
"#,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ContextCompressor;
    use crate::agentic::core::{
        render_system_reminder, CompressionEntry, CompressionPayload, InternalReminderKind,
        Message, MessageContent, MessageSemanticKind, ToolCall, ToolResult,
    };
    use crate::service::session::TranscriptLineRange;

    fn todo_call() -> ToolCall {
        ToolCall {
            tool_id: "todo_recent".to_string(),
            tool_name: "TodoWrite".to_string(),
            arguments: serde_json::json!({
                "todos": [
                    {"content": "Keep recent context", "status": "in_progress"},
                    {"content": "Retry compression", "status": "pending"}
                ]
            }),
            raw_arguments: None,
            is_error: false,
            parse_error: None,
            recovered_from_truncation: false,
            repair_kind: Default::default(),
        }
    }

    fn todo_result() -> Message {
        Message::tool_result(ToolResult {
            tool_id: "todo_recent".to_string(),
            tool_name: "TodoWrite".to_string(),
            effective_tool_name: None,
            result: serde_json::json!({"success": true}),
            result_for_assistant: None,
            is_error: false,
            duration_ms: None,
            image_attachments: None,
        })
    }

    #[test]
    fn recent_context_keeps_the_exact_suffix_that_fits_the_budget() {
        let compressor = ContextCompressor::new(Default::default());
        let current_user = Message::user("Current request".to_string());
        let current_assistant = Message::assistant("Current answer".to_string());
        let recent_target = current_user.estimate_tokens_with_reasoning(true)
            + current_assistant.estimate_tokens_with_reasoning(true);
        let messages = vec![
            Message::system("system".to_string()),
            Message::user("Older request".repeat(200)),
            Message::assistant("Older answer".repeat(200)),
            current_user.clone(),
            current_assistant.clone(),
        ];

        let plan = compressor
            .plan_compression("session", &messages, 128_000, recent_target)
            .expect("planning succeeds")
            .expect("plan exists");

        assert_eq!(plan.recent_tail_tokens, recent_target);
        assert_eq!(plan.recent_tail_messages.len(), 2);
        assert_eq!(plan.recent_tail_messages[0].id, current_user.id);
        assert_eq!(plan.recent_tail_messages[1].id, current_assistant.id);
    }

    #[test]
    fn cutoff_can_split_a_turn_without_adding_an_anchor_or_boundary_reminder() {
        let compressor = ContextCompressor::new(Default::default());
        let current_user = Message::user("Continue the current task".to_string())
            .with_turn_id("turn-current".to_string());
        let retained_assistant = Message::assistant("Latest evidence".to_string())
            .with_turn_id("turn-current".to_string());
        let recent_target = retained_assistant.estimate_tokens_with_reasoning(true);
        let messages = vec![
            Message::system("system".to_string()),
            Message::user("Older request".to_string()),
            Message::assistant("Older answer".to_string()),
            current_user.clone(),
            retained_assistant.clone(),
        ];

        let plan = compressor
            .plan_compression("session", &messages, 128_000, recent_target)
            .expect("planning succeeds")
            .expect("plan exists");

        assert_eq!(plan.recent_tail_messages.len(), 1);
        assert_eq!(plan.recent_tail_messages[0].id, retained_assistant.id);
        assert!(plan
            .retained_user_messages
            .iter()
            .any(|message| message.id == current_user.id));
        assert!(plan
            .summary_messages
            .iter()
            .any(|message| message.id == current_user.id));

        let result = compressor
            .compress_plan_with_contract(
                "session",
                128_000,
                plan,
                None,
                Some("Earlier work summary".to_string()),
            )
            .expect("compression succeeds");
        assert_eq!(result.messages.len(), 5);
        assert_eq!(result.messages[0].content.to_string(), "Older request");
        assert_eq!(result.messages[1].id, current_user.id);
        let MessageContent::Text(summary) = &result.messages[2].content else {
            panic!("expected summary text");
        };
        assert!(!summary.contains("Most recent user message before this summary"));
        assert!(!summary.contains("Most recent task list snapshot before this summary"));
        assert_eq!(result.messages[3].id, retained_assistant.id);
        assert_eq!(
            result.messages[4].internal_reminder_kind(),
            Some(InternalReminderKind::CompressionContinuation)
        );
        assert_eq!(
            result.messages[4].content.to_string(),
            render_system_reminder(ContextCompressor::COMPRESSION_CONTINUATION_REMINDER)
        );
    }

    #[test]
    fn recent_context_never_splits_tool_results_from_their_assistant_call() {
        let compressor = ContextCompressor::new(Default::default());
        let assistant = Message::assistant_with_tools("Planning".to_string(), vec![todo_call()]);
        let result = todo_result();
        let messages = vec![
            Message::system("system".to_string()),
            Message::user("Older request".to_string()),
            Message::assistant("Older answer".repeat(500)),
            Message::user("Current request".to_string()),
            assistant.clone(),
            result.clone(),
        ];

        let atomic_tokens = assistant.estimate_tokens_with_reasoning(true)
            + result.estimate_tokens_with_reasoning(true);
        let too_small = compressor
            .plan_compression("session", &messages, 128_000, atomic_tokens - 1)
            .expect("planning succeeds")
            .expect("plan exists");
        let exact = compressor
            .plan_compression("session", &messages, 128_000, atomic_tokens)
            .expect("planning succeeds")
            .expect("plan exists");

        assert!(too_small.recent_tail_messages.is_empty());
        assert_eq!(exact.recent_tail_messages[0].id, assistant.id);
        assert_eq!(exact.recent_tail_messages[1].id, result.id);
    }

    #[test]
    fn increasing_the_budget_to_the_next_atomic_unit_moves_the_cutoff() {
        let compressor = ContextCompressor::new(Default::default());
        let messages = vec![
            Message::system("system".to_string()),
            Message::user("request".to_string()),
            Message::assistant("first".repeat(2_000)),
            Message::assistant("second".repeat(2_000)),
            Message::assistant("third".repeat(2_000)),
        ];

        let first = compressor
            .plan_compression("session", &messages, 128_000, 1)
            .expect("planning succeeds")
            .expect("first plan exists");
        let next_target = first
            .next_recent_target_tokens
            .expect("another atomic unit can be retained");
        let second = compressor
            .plan_compression("session", &messages, 128_000, next_target)
            .expect("planning succeeds")
            .expect("second plan exists");

        assert!(first.recent_tail_messages.is_empty());
        assert!(second.cutoff_message_index < first.cutoff_message_index);
        assert!(second.summary_messages.len() < first.summary_messages.len());
        assert!(second.recent_tail_messages.len() > first.recent_tail_messages.len());
    }

    #[test]
    fn compression_orders_retained_users_before_summary_and_recent_context() {
        let compressor = ContextCompressor::new(Default::default());
        let user1 = Message::user("user1".to_string());
        let user2 = Message::user("user2".to_string());
        let user3 = Message::user("user3".to_string());
        let assistant3 = Message::assistant("assistant3".to_string());
        let recent_target = user3.estimate_tokens_with_reasoning(true)
            + assistant3.estimate_tokens_with_reasoning(true);
        let messages = vec![
            Message::system("system".to_string()),
            user1.clone(),
            Message::assistant("assistant1".to_string()),
            user2.clone(),
            Message::assistant("assistant2".to_string()),
            user3.clone(),
            assistant3.clone(),
        ];
        let plan = compressor
            .plan_compression("session", &messages, 128_000, recent_target)
            .expect("planning succeeds")
            .expect("plan exists");
        let mut result = compressor
            .compress_plan_with_contract(
                "session",
                128_000,
                plan,
                None,
                Some("Model summary".to_string()),
            )
            .expect("compression succeeds");

        assert_eq!(result.messages.len(), 6);
        assert_eq!(result.messages[0].id, user1.id);
        assert_eq!(result.messages[1].id, user2.id);
        assert_eq!(
            result.messages[2].metadata.semantic_kind,
            Some(MessageSemanticKind::CompressionSummary)
        );
        assert_eq!(result.messages[3].id, user3.id);
        assert_eq!(result.messages[4].id, assistant3.id);
        assert_eq!(
            result.messages[5].internal_reminder_kind(),
            Some(InternalReminderKind::CompressionContinuation)
        );
        let MessageContent::Text(summary_text) = &result.messages[2].content else {
            panic!("expected summary text");
        };
        assert!(summary_text.starts_with("<system_reminder>\n"));
        assert!(summary_text.ends_with("\n</system_reminder>"));
        assert_eq!(summary_text.matches("<system_reminder>").count(), 1);
        assert_eq!(summary_text.matches("</system_reminder>").count(), 1);
        assert!(summary_text.contains(
            "Summary of the earlier conversation:\n<summary>\nModel summary\n</summary>"
        ));
        assert!(summary_text.contains("Model summary"));
        assert!(!summary_text.contains("Most recent user message before this summary"));
        assert!(!summary_text.contains("Most recent task list snapshot before this summary"));
        assert!(!summary_text.contains(ContextCompressor::COMPRESSION_CONTINUATION_REMINDER));
        assert_eq!(
            result.messages[5].content.to_string(),
            render_system_reminder(ContextCompressor::COMPRESSION_CONTINUATION_REMINDER)
        );

        let uri = "bitfun://current-session/artifacts/compression-transcripts/12-a3f9.txt";
        let index_range = TranscriptLineRange {
            start_line: 1,
            end_line: 14,
        };
        assert!(compressor.append_transcript_reference(&mut result, uri, &index_range));
        let MessageContent::Text(summary_text) = &result.messages[2].content else {
            panic!("expected summary text");
        };
        assert!(summary_text.contains(uri));
        let summary_end = summary_text.find("</summary>").expect("summary tag exists");
        let transcript_start = summary_text
            .find("The complete conversation history")
            .expect("transcript reference exists");
        assert!(summary_end < transcript_start);
        assert!(summary_text.ends_with("\n</system_reminder>"));
        assert!(!summary_text.contains(ContextCompressor::COMPRESSION_CONTINUATION_REMINDER));
        assert_eq!(
            result.messages[5].content.to_string(),
            render_system_reminder(ContextCompressor::COMPRESSION_CONTINUATION_REMINDER)
        );
    }

    #[test]
    fn recompression_replaces_the_previous_continuation_reminder() {
        let compressor = ContextCompressor::new(Default::default());
        let old_reminder = Message::internal_reminder(
            InternalReminderKind::CompressionContinuation,
            "old continuation",
        );
        let messages = vec![
            Message::system("system".to_string()),
            Message::user("older request".to_string()),
            Message::assistant("older answer".to_string()),
            old_reminder.clone(),
            Message::user("new request".to_string()),
            Message::assistant("new answer".to_string()),
        ];

        let plan = compressor
            .plan_compression("session", &messages, 128_000, 1)
            .expect("planning succeeds")
            .expect("plan exists");

        assert!(!plan
            .summary_request_messages
            .iter()
            .any(|message| message.id == old_reminder.id));
        assert!(!plan
            .recent_tail_messages
            .iter()
            .any(|message| message.id == old_reminder.id));

        let result = compressor
            .compress_plan_with_contract(
                "session",
                128_000,
                plan,
                None,
                Some("updated summary".to_string()),
            )
            .expect("compression succeeds");

        assert_eq!(
            result
                .messages
                .iter()
                .filter(|message| {
                    message.internal_reminder_kind()
                        == Some(InternalReminderKind::CompressionContinuation)
                })
                .count(),
            1
        );
    }

    #[test]
    fn conditional_instruction_reminders_do_not_enter_any_compacted_context() {
        let compressor = ContextCompressor::new(Default::default());
        let summarized_reminder = Message::internal_reminder(
            InternalReminderKind::ConditionalInstructions,
            "older path rule",
        );
        let recent_reminder = Message::internal_reminder(
            InternalReminderKind::ConditionalInstructions,
            "recent path rule",
        );
        let messages = vec![
            Message::system("system".to_string()),
            Message::user("older request".repeat(200)),
            summarized_reminder.clone(),
            Message::assistant("older answer".repeat(200)),
            Message::user("current request".to_string()),
            recent_reminder.clone(),
            Message::assistant("current answer".to_string()),
        ];

        let plan = compressor
            .plan_compression("session", &messages, 128_000, 100)
            .expect("planning succeeds")
            .expect("plan exists");

        for reminder in [summarized_reminder, recent_reminder] {
            assert!(!plan
                .summary_request_messages
                .iter()
                .any(|message| message.id == reminder.id));
            assert!(!plan
                .summary_messages
                .iter()
                .any(|message| message.id == reminder.id));
            assert!(!plan
                .recent_tail_messages
                .iter()
                .any(|message| message.id == reminder.id));
        }
    }

    #[test]
    fn retained_user_messages_use_only_the_token_budget_without_a_count_limit() {
        let messages: Vec<Message> = (0..25)
            .map(|index| Message::user(format!("user-{index}")))
            .collect();
        let total_tokens = messages
            .iter()
            .map(|message| message.estimate_tokens_with_reasoning(true))
            .sum();

        let (retained, retained_tokens) =
            ContextCompressor::retain_historical_user_messages(&messages, total_tokens);

        assert_eq!(retained.len(), 25);
        assert_eq!(retained_tokens, total_tokens);
    }

    #[test]
    fn retained_user_token_budget_is_ten_percent_capped_at_twenty_thousand() {
        let compressor = ContextCompressor::new(Default::default());
        let messages = vec![
            Message::system("system".to_string()),
            Message::user("older request".to_string()),
            Message::assistant("older answer".to_string()),
            Message::user("current request".to_string()),
            Message::assistant("current answer".to_string()),
        ];

        let smaller = compressor
            .plan_compression("session", &messages, 50_000, 1)
            .expect("planning succeeds")
            .expect("plan exists");
        let larger = compressor
            .plan_compression("session", &messages, 200_000, 1)
            .expect("planning succeeds")
            .expect("plan exists");

        assert_eq!(smaller.retained_user_token_budget, 5_000);
        assert_eq!(larger.retained_user_token_budget, 20_000);
    }

    #[test]
    fn retained_user_messages_drop_the_older_prefix_when_the_next_message_does_not_fit() {
        let oldest = Message::user("oldest".to_string());
        let too_large = Message::user("large".repeat(500));
        let newest = Message::user("newest".to_string());
        let budget = newest.estimate_tokens_with_reasoning(true);

        let (retained, retained_tokens) = ContextCompressor::retain_historical_user_messages(
            &[oldest, too_large, newest.clone()],
            budget,
        );

        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].id, newest.id);
        assert_eq!(retained_tokens, budget);
    }

    #[test]
    fn retained_user_messages_are_empty_when_the_newest_candidate_exceeds_the_budget() {
        let older = Message::user("older".to_string());
        let newest = Message::user("newest".repeat(500));

        let (retained, retained_tokens) =
            ContextCompressor::retain_historical_user_messages(&[older, newest], 1);

        assert!(retained.is_empty());
        assert_eq!(retained_tokens, 0);
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
    fn merged_compression_payload_remains_atomic_on_recompression() {
        let compressor = ContextCompressor::new(Default::default());
        let messages = vec![
            Message::system("system".to_string()),
            Message::user("Continue the refactor".to_string()),
            Message::assistant("Work in progress".to_string()),
            Message::user("Keep recent context".to_string()),
            Message::assistant("Recent evidence".to_string()),
        ];
        let plan = compressor
            .plan_compression("session", &messages, 8_000, 1)
            .expect("planning succeeds")
            .expect("plan exists");
        let compressed = compressor
            .compress_plan_with_contract(
                "session",
                8_000,
                plan,
                None,
                Some("Model summary".to_string()),
            )
            .expect("compression succeeds");

        let summary_artifact =
            crate::agentic::session::compression::fallback::build_structured_compression_summary(
                vec![compressed.messages],
                &crate::agentic::session::compression::fallback::CompressionFallbackOptions {
                    max_tokens: 10_000,
                    user_chars: 120,
                    assistant_chars: 120,
                    tool_arg_chars: 80,
                    tool_command_chars: 80,
                },
            );

        assert!(summary_artifact
            .payload
            .entries
            .iter()
            .any(|entry| matches!(
                entry,
                CompressionEntry::ModelSummary { text } if text == "Model summary"
            )));
    }

    #[test]
    fn model_summary_output_trims_plain_text() {
        let normalized = ContextCompressor::normalize_model_summary_output("  Plain summary\n");

        assert_eq!(normalized.as_deref(), Some("Plain summary"));
    }

    #[test]
    fn empty_model_summary_output_is_rejected() {
        let normalized = ContextCompressor::normalize_model_summary_output(" \n\t ");

        assert_eq!(normalized, None);
    }
}
