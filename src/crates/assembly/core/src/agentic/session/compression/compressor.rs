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
            r#"You are compressing all available context into a compact, durable execution state for another AI agent.

Your PRIMARY goal is COMPRESSION: substantially reduce context size while preserving everything required to continue the task correctly and without regression.

The input may contain original messages, tool results, file contents, and one or more previously compressed states. A previous state may appear without wrapper tags.

## Hard Rules

* Respond with TEXT ONLY.
* Do NOT call tools.
* Output exactly one `<summary>...</summary>` block.
* Do not output anything before or after the `<summary>` block.
* Do not use Markdown code fences around the block.
* Do not invent information, present unsupported inference as fact, or silently resolve uncertainty.
* Use adaptive length: preserve critical information exactly enough for execution, but compress everything else aggressively.
* Produce a current execution state, not a transcript, chronological recap, unfiltered merge, or summary of a previous summary.
* Record each fact once in its most relevant location; refer to requirement IDs instead of repeating requirements.

## Compression Procedure

1. Identify the latest active task, intended outcome, and authoritative requirements.
2. Detect inherited structured state using section patterns, requirement IDs, file paths, task records, or semantic structure.
3. Treat inherited state as retained task information, not disposable summary prose.
4. Preserve all still-active inherited information and incorporate relevant newer evidence.
5. Consolidate duplicates and remove obsolete, superseded, completed-but-irrelevant, or safely recoverable detail.
6. Produce one compact representation of the current executable state.

Use this principle:

`compressed state = essential inherited state + essential new state - duplication - obsolete or recoverable detail`

Previous structured state is authoritative input, but must still be evaluated and compressed. Silence in newer messages does not cancel inherited requirements.

## Information That Must Survive

Preserve the smallest accurate representation of all information needed to continue safely:

* current objective and durable user intent
* active requirements, constraints, acceptance criteria, edge cases, and prohibited behavior
* requirement-bearing user corrections, rejected approaches, and direction changes
* authoritative source identities and exact file paths
* stable requirement IDs
* concrete formats, schemas, commands, values, expected outputs, and important symbols
* current implementation state, current behavior, intended behavior, and important changes
* architectural or technical decisions that still affect future work
* test expectations, validation methods, and latest validation results
* unresolved errors, blockers, uncertainties, and failed approaches that should not be repeated
* pending work, priority, completion conditions, immediate next action, and exact stopping point
* irrecoverable or conversation-only code, snippets, errors, outputs, or other exact details

Authoritative sources include user instructions, inherited structured state, `instruction.md`, requirement or specification files, task descriptions, acceptance tests, evaluation scripts, and files explicitly identified as defining expected behavior.

Never replace actual requirements with vague shorthand such as "follow the instruction file" or "pass the tests." Extract and preserve the operative requirements.

## Recursive Compression Safety

When inherited structured state exists:

* preserve stable requirement IDs and precise requirement wording
* keep active requirements even if newer messages do not mention them
* update statuses only when supported by newer evidence
* merge new constraints into the relevant existing record
* consolidate duplicate records into one authoritative record
* avoid repeated paraphrasing when retaining precise wording is safer
* remove an inherited item only when it is:

  * explicitly withdrawn or superseded
  * disproved by authoritative evidence
  * completed with no future or regression-prevention relevance
  * safely recoverable from an available source

"Preserve" does not mean copying everything. Retain only what keeps the task executable and prevents lost constraints, regressions, or repeated mistakes.

## What to Compress or Remove

Aggressively discard:

* greetings, filler, and routine conversation
* repeated requirements and explanations
* routine tool calls and action narration
* raw search queries, browsing paths, and search history
* long excerpts once their operative conclusion is captured
* ordinary file contents that can safely be reread
* completed intermediate actions with no future effect
* obsolete hypotheses, plans, diagnostics, and superseded state
* duplicate descriptions inherited across compression cycles

For research, retain only:

* operative conclusion
* authoritative reference
* relevant version or date
* unresolved uncertainty

For ordinary files, retain only:

* exact path and purpose
* important symbols or sections
* current and intended behavior
* material changes
* unresolved issues
* validation status

Preserve exact code only when it is unsaved, subtle, conversation-only, irrecoverable, or unsafe to reconstruct.

## Requirement Handling

For every authoritative requirement source:

* preserve its exact identity or path
* extract active atomic requirements
* assign or preserve stable IDs such as `REQ-001`
* preserve concrete values, paths, formats, commands, edge cases, expected outputs, and prohibitions
* record implementation and validation status when known

Use one of these statuses:

`Completed | Partial | Blocked | Unverified | Not started | Superseded`

When a requirement changes:

* unchanged: retain it compactly
* clarified: update the same record without losing earlier constraints
* completed: retain expected behavior when needed to prevent regression
* superseded: mark it and identify its replacement and supporting evidence
* conflicting: retain both requirements and explicitly record the conflict
* absent from newer messages: keep it active

## Final Verification

Before answering, verify that:

* the output is substantially smaller than the useful input state
* all active inherited and new requirements survived
* precise requirements were not weakened through paraphrasing
* duplicates and safely recoverable details were removed
* unresolved work did not disappear because it was not recently mentioned
* current behavior, intended behavior, and pending work remain distinguishable
* implementation and validation statuses are evidence-based
* blockers, failed approaches, user corrections, and conflicts remain visible
* the exact continuation point is clear
* the response contains exactly one `<summary>` block and no text outside it

## Output Format

Output exactly:

<summary>
1. OBJECTIVE
- Current: [latest active task and intended outcome]
- Durable intent: [earlier intent that remains active]
- Constraints: [active constraints, acceptance criteria, and prohibited behavior]
- Conflicts/Superseded: [conflicts or superseded intent, replacement, and evidence]

2. REQUIREMENTS

* Source: [exact user instruction, inherited state identity, or authoritative file path]

  * REQ-001 [Status]: [atomic requirement]

    * State: [implementation state, current behavior, or latest material change]
    * Files/symbols: [exact paths and important symbols]
    * Validation: [test expectation and latest result]
    * Notes: [clarification, correction, conflict, edge case, or replacement]

3. EXECUTION STATE

* Files: [path — purpose; relevant symbols; current and intended behavior; changes; issues; validation]
* Decisions: [decision and concise rationale]
* Solved: [problem → solution → result]
* Errors/failed approaches: [symptom → cause → attempts → current status → user correction → next diagnostic action]
* Technical conclusions: [concept → relevance → current conclusion]

4. CONTINUATION

* Immediate task: [what should be done now]
* Current state: [where the work stands]
* Pending:

  * P1 [priority]: [unfinished task — related REQ IDs; files; blocker; completion condition]
* Latest validation: [most recent relevant result]
* Exact stopping point: [last completed action and precise continuation location]
* Next action: [single best next step and required context]

5. CRITICAL EXACT DETAILS

* [Only exact wording, paths, commands, schemas, identifiers, values, errors, expected outputs, or irrecoverable snippets that must survive]

</summary>

Formatting rules:

* Keep the five top-level sections in the stated order.
* Keep entries compact and combine related facts when clarity is preserved.
* Omit empty optional fields or categories instead of printing many `None` values.
* If an entire top-level section has no useful information, write `None` beneath that section.
* Preserve requirement IDs across compression cycles.
* Do not duplicate information already preserved accurately elsewhere.
* Represent requirement-bearing user messages through their requirement, correction, rejection, or source identity rather than copying conversation history.
* The `<summary>` tags must remain literal and must not be replaced with Markdown headings or code fences.
{contract_instruction}
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
