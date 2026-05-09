//! Context compressor
//!
//! Responsible only for transforming a session context into a compressed one.

use super::fallback::{
    build_structured_compression_summary, CompressionFallbackOptions, CompressionSummaryArtifact,
};
use crate::agentic::core::{
    render_system_reminder, CompressedTodoSnapshot, CompressionEntry, CompressionPayload, Message,
    MessageHelper, MessageRole, MessageSemanticKind,
};
use crate::infrastructure::ai::{get_global_ai_client_factory, AIClient};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::Message as AIMessage;
use anyhow;
use log::{debug, trace, warn};
use std::sync::Arc;

/// Context compressor configuration
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    pub keep_turns_ratio: f32,
    pub keep_last_turn_ratio: f32,
    pub single_request_max_tokens_ratio: f32,
    pub fallback_max_tokens_ratio: f32,
    pub fallback_user_chars: usize,
    pub fallback_assistant_chars: usize,
    pub fallback_tool_arg_chars: usize,
    pub fallback_tool_command_chars: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            keep_turns_ratio: 0.3,
            keep_last_turn_ratio: 0.4,
            single_request_max_tokens_ratio: 0.7,
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
    tokens: usize,
}

impl TurnWithTokens {
    fn new(messages: Vec<Message>, tokens: usize) -> Self {
        Self { messages, tokens }
    }
}

#[derive(Debug, Clone)]
pub struct CompressionResult {
    pub messages: Vec<Message>,
    pub has_model_summary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionTailPolicy {
    CollapseAll,
    PreserveLiveFrontier,
}

/// Stateless context compression service.
pub struct ContextCompressor {
    config: CompressionConfig,
}

impl ContextCompressor {
    pub fn new(config: CompressionConfig) -> Self {
        Self { config }
    }

    fn get_turn_index_to_keep(&self, turns_tokens: &[usize], token_limit: usize) -> usize {
        let mut sum = 0;
        let mut result = turns_tokens.len();
        for (idx, turn_token) in turns_tokens.iter().enumerate().rev() {
            sum += turn_token;
            if sum <= token_limit {
                result = idx;
            } else {
                break;
            }
        }
        result
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
            .zip(turns_tokens)
            .map(|(msgs, tokens)| TurnWithTokens::new(msgs, tokens))
            .collect())
    }

    /// Returns `(turn_index_to_keep, turns)`.
    /// If `turn_index_to_keep` is 0, no compression is needed.
    pub async fn preprocess_turns(
        &self,
        session_id: &str,
        context_window: usize,
        messages: Vec<Message>,
    ) -> BitFunResult<(usize, Vec<TurnWithTokens>)> {
        debug!(
            "Starting session context compression analysis: session_id={}",
            session_id
        );

        let turns = self.collect_conversation_turns(session_id, messages)?;
        if turns.is_empty() {
            return Ok((0, Vec::new()));
        }
        let turns_count = turns.len();
        let turns_tokens: Vec<usize> = turns.iter().map(|turn| turn.tokens).collect();

        let token_limit_keep_turns =
            (context_window as f32 * self.config.keep_turns_ratio) as usize;
        let mut turn_index_to_keep =
            self.get_turn_index_to_keep(&turns_tokens, token_limit_keep_turns);
        if turn_index_to_keep == turns_count {
            let token_limit_last_turn =
                (context_window as f32 * self.config.keep_last_turn_ratio) as usize;
            if let Some(last_turn_tokens) = turns_tokens.last() {
                if *last_turn_tokens <= token_limit_last_turn {
                    turn_index_to_keep = turns_count - 1;
                }
            }
        }
        debug!(
            "Turn index to keep after compression analysis: session_id={}, keep_from_turn={}",
            session_id, turn_index_to_keep
        );

        Ok((turn_index_to_keep, turns))
    }

    /// Collect all non-system conversation turns for a full manual compaction pass.
    pub fn collect_all_turns_for_manual_compaction(
        &self,
        session_id: &str,
        messages: Vec<Message>,
    ) -> BitFunResult<Vec<TurnWithTokens>> {
        self.collect_conversation_turns(session_id, messages)
    }

    pub async fn compress_turns(
        &self,
        session_id: &str,
        context_window: usize,
        turn_index_to_keep: usize,
        mut turns: Vec<TurnWithTokens>,
        tail_policy: CompressionTailPolicy,
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
                "No turns available after split, skipping compression: session_id={}",
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
        let turns_to_keep = turns.split_off(turn_index_to_keep);

        let mut compressed_messages = Vec::new();
        let mut has_model_summary = false;
        if !turns.is_empty() {
            let mut summary_artifact = self
                .execute_compression_with_fallback(turns, context_window)
                .await?;
            if turns_to_keep.is_empty() {
                self.append_todo_snapshot(&mut summary_artifact, last_todo.clone());
            }
            trace!("Compression summary artifact generated");
            has_model_summary = summary_artifact.used_model_summary;
            let (boundary_message, summary_message) = self.create_summary_turn(summary_artifact);
            compressed_messages.push(boundary_message);
            compressed_messages.push(summary_message);
        }

        if !turns_to_keep.is_empty() {
            for turn in turns_to_keep {
                compressed_messages.extend(turn.messages);
            }
        } else if matches!(tail_policy, CompressionTailPolicy::PreserveLiveFrontier) {
            if let Some(last_user_message) = last_user_message {
                compressed_messages.push(last_user_message);
            }
        }

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

    fn append_todo_snapshot(
        &self,
        summary_artifact: &mut CompressionSummaryArtifact,
        todo_snapshot: Option<CompressedTodoSnapshot>,
    ) {
        let Some(todo_snapshot) = todo_snapshot else {
            return;
        };

        let todo_text = Self::render_todo_snapshot(&todo_snapshot);
        if !todo_text.is_empty() {
            summary_artifact.summary_text = format!(
                "{}\n\nLatest task list snapshot at the compression boundary:\n{}",
                summary_artifact.summary_text.trim_end(),
                todo_text
            );
        }

        summary_artifact
            .payload
            .entries
            .push(CompressionEntry::Turn {
                turn_id: None,
                messages: Vec::new(),
                todo: Some(todo_snapshot),
            });
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
        let mut msg = "Earlier conversation was compressed for context management. Use the summary in the next assistant message as historical context.".to_string();
        if !used_model_summary {
            msg.push_str(" This compressed context is a partial reconstructed record. Message text, tool arguments, task lists, and tool results may be truncated or omitted.");
        }
        msg
    }

    async fn execute_compression_with_fallback(
        &self,
        turns_to_compress: Vec<TurnWithTokens>,
        context_window: usize,
    ) -> BitFunResult<CompressionSummaryArtifact> {
        let summary_result = match get_global_ai_client_factory().await {
            Ok(ai_client_factory) => match ai_client_factory
                .get_client_by_func_agent("compression")
                .await
            {
                Ok(ai_client) => {
                    self.execute_compression(ai_client, turns_to_compress.clone(), context_window)
                        .await
                }
                Err(err) => Err(BitFunError::AIClient(format!(
                    "Failed to get AI client: {}",
                    err
                ))),
            },
            Err(err) => Err(BitFunError::AIClient(format!(
                "Failed to get AI client factory: {}",
                err
            ))),
        };

        match summary_result {
            Ok(summary) => {
                trace!("Compression summary: {}", summary);
                Ok(CompressionSummaryArtifact {
                    summary_text: format!(
                        "Previous conversation is summarized below:\n{}",
                        summary
                    ),
                    payload: CompressionPayload::from_summary(summary),
                    used_model_summary: true,
                })
            }
            Err(err) => {
                warn!(
                    "Model-based compression failed, falling back to structured local compression: {}",
                    err
                );
                let summary_artifact = build_structured_compression_summary(
                    turns_to_compress
                        .into_iter()
                        .map(|turn| turn.messages)
                        .collect(),
                    &self.build_fallback_options(context_window),
                );
                Ok(summary_artifact)
            }
        }
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

    fn normalize_model_summary_output(raw: &str) -> Option<String> {
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

    async fn execute_compression(
        &self,
        ai_client: Arc<AIClient>,
        turns_to_compress: Vec<TurnWithTokens>,
        context_window: usize,
    ) -> BitFunResult<String> {
        debug!("Compressing {} turn(s)", turns_to_compress.len());

        fn gen_system_message_for_summary(prev_summary: &str) -> Message {
            if prev_summary.is_empty() {
                Message::system(
                    "You are a helpful AI assistant tasked with summarizing conversations."
                        .to_string(),
                )
            } else {
                Message::system(format!(
                    r#"You are a conversation summarization assistant performing an INCREMENTAL summary update.

## Previous Summary
The conversation has already been partially summarized. Here is the existing summary:

<previous_summary>
{}
</previous_summary>

## Your Task
You will be given the CONTINUATION of this conversation. Your job is to:
1. Read and understand the new conversation segment
2. MERGE the new information into the existing summary
3. Output a single, unified summary that combines both the previous summary and the new conversation

## Important Guidelines
- Preserve all important information from the previous summary
- Add new details from the current conversation segment
- If new information contradicts or updates previous information, use the newer information
- Maintain the same summary structure/format as specified in the user instructions
- The final output should be ONE cohesive summary, not two separate summaries
- Do not mention "previous summary" or "new conversation" in your output - write as if summarizing the entire conversation from the start

Be thorough and precise. Do not lose important technical details from either the previous summary or the new conversation."#,
                    prev_summary
                ))
            }
        }

        let max_tokens_in_one_request =
            (context_window as f32 * self.config.single_request_max_tokens_ratio) as usize;
        let mut current_tokens = 0;
        let mut cur_messages = Vec::new();
        let mut summary = String::new();
        let mut request_cnt = 0;
        for (idx, turn) in turns_to_compress.into_iter().enumerate() {
            if current_tokens + turn.tokens <= max_tokens_in_one_request {
                cur_messages.extend(turn.messages);
                current_tokens += turn.tokens;
            } else {
                if !cur_messages.is_empty() {
                    summary = self
                        .generate_summary(
                            ai_client.clone(),
                            gen_system_message_for_summary(&summary),
                            cur_messages,
                        )
                        .await?;
                    cur_messages = Vec::new();
                    current_tokens = 0;
                    request_cnt += 1;
                    trace!(
                        "Compression request {} completed: turn_idx={}",
                        request_cnt,
                        idx
                    );
                }

                if turn.tokens <= max_tokens_in_one_request {
                    cur_messages.extend(turn.messages);
                    current_tokens = turn.tokens;
                } else if let Some((messages_part1, messages_part2)) =
                    MessageHelper::split_messages_in_middle(turn.messages)
                {
                    summary = self
                        .generate_summary(
                            ai_client.clone(),
                            gen_system_message_for_summary(&summary),
                            messages_part1,
                        )
                        .await?;
                    request_cnt += 1;
                    debug!(
                        "[execute_compression] request_cnt={}, turn_idx={}, summary: \n{}",
                        request_cnt, idx, summary
                    );
                    summary = self
                        .generate_summary(
                            ai_client.clone(),
                            gen_system_message_for_summary(&summary),
                            messages_part2,
                        )
                        .await?;
                    request_cnt += 1;
                    debug!(
                        "[execute_compression] request_cnt={}, turn_idx={}, summary: \n{}",
                        request_cnt, idx, summary
                    );
                } else {
                    return Err(BitFunError::Service(format!(
                        "Compression Failed, turn {} cannot be split in middle",
                        idx
                    )));
                }
            }
        }

        if !cur_messages.is_empty() {
            summary = self
                .generate_summary(
                    ai_client.clone(),
                    gen_system_message_for_summary(&summary),
                    cur_messages,
                )
                .await?;
            request_cnt += 1;
            trace!("Compression request {} completed", request_cnt);
        }
        Ok(summary)
    }

    async fn generate_summary(
        &self,
        ai_client: Arc<AIClient>,
        system_message_for_summary: Message,
        messages: Vec<Message>,
    ) -> BitFunResult<String> {
        let raw_summary = self
            .generate_summary_with_retry(ai_client, system_message_for_summary, messages, 2)
            .await?;
        Self::normalize_model_summary_output(&raw_summary).ok_or_else(|| {
            BitFunError::AIClient(
                "Model-based compression returned <analysis> without a usable <summary>"
                    .to_string(),
            )
        })
    }

    async fn generate_summary_with_retry(
        &self,
        ai_client: Arc<AIClient>,
        system_message_for_summary: Message,
        messages: Vec<Message>,
        max_tries: usize,
    ) -> BitFunResult<String> {
        let mut summary_messages = vec![AIMessage::from(system_message_for_summary)];
        summary_messages.extend(messages.iter().map(|m| {
            let mut ai_msg = AIMessage::from(m);
            ai_msg.reasoning_content = None;
            ai_msg
        }));
        summary_messages.push(AIMessage::user(self.get_compact_prompt()));

        let mut last_error = None;
        let base_wait_time_ms = 500;

        for attempt in 0..max_tries {
            let result = ai_client.send_message(summary_messages.clone(), None).await;

            match result {
                Ok(response) => {
                    if attempt > 0 {
                        debug!(
                            "Summary generation succeeded (attempt {}/{})",
                            attempt + 1,
                            max_tries
                        );
                    }
                    return Ok(response.text);
                }
                Err(e) => {
                    warn!(
                        "Summary generation failed (attempt {}/{}): {}",
                        attempt + 1,
                        max_tries,
                        e
                    );
                    last_error = Some(e);

                    if attempt < max_tries - 1 {
                        let delay_ms = base_wait_time_ms * (1 << attempt.min(3));
                        debug!("Waiting {}ms before retry {}...", delay_ms, attempt + 2);
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        let error_msg = format!(
            "Summary generation failed after {} attempts: {}",
            max_tries,
            last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error"))
        );
        warn!("{}", error_msg);
        Err(BitFunError::AIClient(error_msg))
    }

    fn get_compact_prompt(&self) -> String {
        r#"Your task is to create a detailed summary of the conversation so far, paying close attention to the user's explicit requests and your previous actions.
This summary should be thorough in capturing technical details, code patterns, and architectural decisions that would be essential for continuing development work without losing context.

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
9. Optional Next Step: List the next step that you will take that is related to the most recent work you were doing. IMPORTANT: ensure that this step is DIRECTLY in line with the user's most recent explicit requests, and the task you were working on immediately before this summary request. If your last task was concluded, then only list next steps if they are explicitly in line with the users request. Do not start on tangential requests or really old requests that were already completed without confirming with the user first.
If there is a next step, include direct quotes from the most recent conversation showing exactly what task you were working on and where you left off. This should be verbatim to ensure there's no drift in task interpretation.

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
"#
        .to_string()
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
    use super::{CompressionTailPolicy, ContextCompressor, TurnWithTokens};
    use crate::agentic::core::{
        render_system_reminder, CompressionEntry, CompressionPayload, Message, MessageSemanticKind,
    };

    fn make_turn(messages: Vec<Message>) -> TurnWithTokens {
        let mut messages_with_tokens = messages;
        let tokens = messages_with_tokens
            .iter_mut()
            .map(|message| message.get_tokens())
            .sum();
        TurnWithTokens::new(messages_with_tokens, tokens)
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
                }],
            ),
        ])
    }

    #[tokio::test]
    async fn collapse_all_creates_closed_compression_turn() {
        let compressor = ContextCompressor::new(Default::default());
        let result = compressor
            .compress_turns(
                "session",
                8000,
                1,
                vec![todo_turn()],
                CompressionTailPolicy::CollapseAll,
            )
            .await
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
        assert!(summary_text.contains("Latest task list snapshot at the compression boundary"));
        assert!(summary_text.contains("Update compressor"));
    }

    #[tokio::test]
    async fn preserve_live_frontier_keeps_last_user_after_summary_turn() {
        let compressor = ContextCompressor::new(Default::default());
        let result = compressor
            .compress_turns(
                "session",
                8000,
                1,
                vec![todo_turn()],
                CompressionTailPolicy::PreserveLiveFrontier,
            )
            .await
            .expect("compression succeeds");

        assert_eq!(result.messages.len(), 3);
        assert_eq!(
            result.messages[2].role,
            crate::agentic::core::MessageRole::User
        );
        assert!(result.messages[2].is_actual_user_message());
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
    fn model_summary_boundary_marker_omits_partial_record_notice() {
        let marker = ContextCompressor::render_boundary_marker_text(true);
        assert!(!marker.contains("partial reconstructed record"));
        assert!(marker.contains("historical context"));
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

    #[tokio::test]
    async fn manual_compaction_turn_collection_includes_all_non_system_turns() {
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
        let (_, passive_turns) = compressor
            .preprocess_turns("session", 8_000, messages)
            .await
            .expect("passive preprocessing succeeds");

        assert_eq!(manual_turns.len(), 2);
        assert_eq!(manual_turns.len(), passive_turns.len());
    }
}
