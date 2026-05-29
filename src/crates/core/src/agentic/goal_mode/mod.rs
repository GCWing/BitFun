//! Session goal mode: `/goal` command support with AI goal synthesis and
//! post-turn achievement verification.

mod types;

pub use types::*;

use crate::agentic::core::{
    Message, MessageContent, MessageRole, MessageSemanticKind, PromptEnvelope,
};
use crate::service::config::{get_app_language_code, short_model_user_language_instruction};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::extract_json_from_ai_response;
use crate::util::sanitize_plain_model_output;
use crate::util::types::Message as AIMessage;
use log::warn;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const GOAL_VERIFICATION_RETRY_BASE_DELAY_MS: u64 = 1_000;
const GOAL_VERIFICATION_RETRY_MAX_DELAY_MS: u64 = 30_000;

pub fn goal_mode_from_custom_metadata(
    custom_metadata: Option<&serde_json::Value>,
) -> Option<GoalModeState> {
    let value = custom_metadata?.get(GOAL_MODE_METADATA_KEY)?;
    let mut state: GoalModeState = serde_json::from_value(value.clone()).ok()?;
    if !state.initial_goal.is_set() && !state.goal_text.trim().is_empty() {
        state.initial_goal = GoalModeInitialGoal::new(
            state.goal_text.clone(),
            state.success_criteria.clone(),
            state.user_hint.clone(),
            state.activated_at_ms,
        );
    }
    Some(state)
}

pub fn goal_mode_patch(state: &GoalModeState) -> serde_json::Value {
    serde_json::json!({
        GOAL_MODE_METADATA_KEY: state,
    })
}

pub fn clear_goal_mode_patch() -> serde_json::Value {
    serde_json::json!({
        GOAL_MODE_METADATA_KEY: serde_json::Value::Null,
    })
}

/// In `/goal` mode, subagents start without an active timeout. The requested
/// timeout is still preserved on the handle so the user can re-enable a limit.
pub fn effective_subagent_timeout_seconds(
    timeout_seconds: Option<u64>,
    parent_goal_mode_active: bool,
) -> Option<u64> {
    if parent_goal_mode_active {
        None
    } else {
        timeout_seconds.filter(|seconds| *seconds > 0)
    }
}

pub fn message_text(message: &Message) -> Option<String> {
    match &message.content {
        MessageContent::Text(text) => Some(text.clone()),
        MessageContent::Multimodal { text, .. } => Some(text.clone()),
        MessageContent::Mixed { text, .. } if !text.trim().is_empty() => Some(text.clone()),
        _ => None,
    }
}

/// Convert the full in-memory session transcript into provider messages, using
/// the same omission rules as normal model sends for UI-only computer-use frames.
pub fn build_goal_context_ai_messages(messages: &[Message]) -> Vec<AIMessage> {
    messages
        .iter()
        .filter(|message| !should_skip_message_for_goal_context(message))
        .map(AIMessage::from)
        .collect()
}

fn should_skip_message_for_goal_context(message: &Message) -> bool {
    matches!(
        message.metadata.semantic_kind.as_ref(),
        Some(MessageSemanticKind::ComputerUseVerificationScreenshot)
            | Some(MessageSemanticKind::ComputerUsePostActionSnapshot)
    )
}

pub fn last_assistant_message_text(messages: &[Message]) -> Option<String> {
    messages
        .iter()
        .rev()
        .filter(|message| message.role == MessageRole::Assistant)
        .find_map(message_text)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

pub fn ensure_final_response_in_goal_context(
    mut context_messages: Vec<Message>,
    final_response: &str,
    turn_id: &str,
) -> Vec<Message> {
    let trimmed_final = final_response.trim();
    if trimmed_final.is_empty() {
        return context_messages;
    }

    let latest_assistant = last_assistant_message_text(&context_messages).unwrap_or_default();
    if latest_assistant.trim() != trimmed_final {
        context_messages
            .push(Message::assistant(final_response.to_string()).with_turn_id(turn_id.to_string()));
    }
    context_messages
}

pub fn user_facing_goal_mode_error(error: BitFunError) -> BitFunError {
    match error {
        BitFunError::Validation(_) | BitFunError::NotFound(_) => error,
        other => {
            warn!("Goal mode AI call failed: {other}");
            BitFunError::Validation(
                "Goal mode AI request failed. Check model configuration and try again.".to_string(),
            )
        }
    }
}

fn format_success_criteria(criteria: &[String]) -> String {
    if criteria.is_empty() {
        "- Use your best judgment to decide when the goal is fully complete.".to_string()
    } else {
        criteria
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn current_goal_note(state: &GoalModeState) -> String {
    let initial_goal = state.initial_goal_text().trim();
    let current_goal = state.goal_text.trim();
    if current_goal.is_empty() || current_goal == initial_goal {
        String::new()
    } else {
        format!(
            "\nCurrent continuation target: {current_goal}\n\
Use it only as tactical guidance; the initial session goal remains the source of truth.\n"
        )
    }
}

pub fn build_goal_system_reminder(state: &GoalModeState) -> String {
    let criteria = format_success_criteria(state.initial_success_criteria());
    let current_goal_note = current_goal_note(state);

    format!(
        "Active session goal mode is ON.\n\
Initial session goal: {}\n\
{current_goal_note}\
Success criteria:\n{}\n\
Keep working toward the initial session goal. Do not declare the task finished until every criterion is truly satisfied.",
        state.initial_goal_text().trim(),
        criteria
    )
}

pub fn wrap_user_input_with_goal_reminder(user_input: String, state: &GoalModeState) -> String {
    if has_prompt_markup(&user_input) {
        return user_input;
    }
    let mut envelope = PromptEnvelope::new();
    envelope.push_system_reminder(build_goal_system_reminder(state));
    envelope.push_user_query(user_input);
    envelope.render()
}

fn has_prompt_markup(text: &str) -> bool {
    crate::agentic::core::has_prompt_markup(text)
}

pub fn build_goal_kickoff_messages(
    generation: &GoalGenerationResult,
    user_hint: Option<&str>,
) -> GoalActivationResult {
    let goal_text = generation.goal_text.trim().to_string();
    let criteria = generation
        .success_criteria
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    let criteria_block = if criteria.is_empty() {
        String::new()
    } else {
        format!(
            "\nSuccess criteria:\n{}",
            criteria
                .iter()
                .map(|item| format!("- {item}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    let hint_line = user_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("\nUser-provided focus: {value}"))
        .unwrap_or_default();

    let display_message = format!("/goal {goal_text}");
    let kickoff_message = format!(
        "Work toward this session goal until it is fully achieved.{hint_line}\n\nGoal: {goal_text}{criteria_block}\n\nStart executing now. Verify your work before stopping."
    );

    GoalActivationResult {
        goal_text: goal_text.clone(),
        success_criteria: criteria,
        kickoff_message,
        display_message,
    }
}

pub fn build_goal_continuation_plan(
    state: &GoalModeState,
    verification: &GoalVerificationResult,
) -> GoalContinuationPlan {
    let initial_goal_text = state.initial_goal_text().trim();
    let initial_success_criteria = state.initial_success_criteria();
    let initial_user_hint = state.initial_user_hint();
    let initial_created_at_ms = state.initial_goal_created_at_ms();
    let current_goal_note = current_goal_note(state);
    let gaps = verification
        .gaps
        .iter()
        .map(|gap| format!("- {gap}"))
        .collect::<Vec<_>>()
        .join("\n");
    let guidance = verification.guidance.trim();

    let display_message = format!("Goal not yet achieved — continuing work on: {guidance}");

    let wrapped_message = {
        let mut envelope = PromptEnvelope::new();
        envelope.push_system_reminder(format!(
            "Goal verification found the initial session goal is NOT yet achieved.\n\
Initial session goal: {}\n\
{current_goal_note}\
Remaining gaps:\n{gaps}\n\
Next steps:\n{guidance}\n\
Continue working until the initial session goal is fully satisfied. Do not stop early.",
            initial_goal_text
        ));
        envelope.push_user_query(format!(
            "Continue working toward the initial session goal. Address the remaining gaps and complete the goal before stopping.\n\nInitial session goal: {}",
            initial_goal_text
        ));
        envelope.render()
    };

    GoalContinuationPlan {
        wrapped_message,
        display_message,
        user_message_metadata: serde_json::json!({
            "goalModeContinuation": true,
            "goalText": state.goal_text,
            "initialGoal": {
                "goalText": initial_goal_text,
                "successCriteria": initial_success_criteria,
                "userHint": initial_user_hint,
                "createdAtMs": initial_created_at_ms,
            },
        }),
    }
}

pub fn should_skip_goal_verification_for_turn(
    user_input: &str,
    user_message_metadata: Option<&serde_json::Value>,
) -> bool {
    let trimmed = user_input.trim();
    if trimmed.eq_ignore_ascii_case("/compact")
        || trimmed.starts_with("/usage")
        || trimmed.starts_with("/btw")
    {
        return true;
    }
    if user_message_metadata
        .and_then(|metadata| metadata.get("maintenanceTurn"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return true;
    }
    false
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

async fn call_goal_func_agent_with_context(
    system_prompt: String,
    context_messages: &[Message],
    final_user_prompt: String,
) -> BitFunResult<String> {
    let mut messages = Vec::with_capacity(context_messages.len() + 2);
    messages.push(AIMessage {
        role: "system".to_string(),
        content: Some(system_prompt),
        reasoning_content: None,
        thinking_signature: None,
        tool_calls: None,
        tool_call_id: None,
        name: None,
        is_error: None,
        tool_image_attachments: None,
    });
    messages.extend(build_goal_context_ai_messages(context_messages));
    messages.push(AIMessage {
        role: "user".to_string(),
        content: Some(final_user_prompt),
        reasoning_content: None,
        thinking_signature: None,
        tool_calls: None,
        tool_call_id: None,
        name: None,
        is_error: None,
        tool_image_attachments: None,
    });

    call_goal_func_agent_messages(messages).await
}

async fn call_goal_func_agent_with_appended_prompt(
    context_messages: &[Message],
    final_user_prompt: String,
) -> BitFunResult<String> {
    let messages = build_appended_prompt_ai_messages(context_messages, final_user_prompt);
    call_goal_func_agent_messages(messages).await
}

fn build_appended_prompt_ai_messages(
    context_messages: &[Message],
    final_user_prompt: String,
) -> Vec<AIMessage> {
    let mut messages = build_goal_context_ai_messages(context_messages);
    messages.push(AIMessage {
        role: "user".to_string(),
        content: Some(final_user_prompt),
        reasoning_content: None,
        thinking_signature: None,
        tool_calls: None,
        tool_call_id: None,
        name: None,
        is_error: None,
        tool_image_attachments: None,
    });
    messages
}

async fn call_goal_func_agent_messages(messages: Vec<AIMessage>) -> BitFunResult<String> {
    let ai_client_factory = crate::infrastructure::ai::get_global_ai_client_factory()
        .await
        .map_err(|error| {
            user_facing_goal_mode_error(BitFunError::AIClient(format!(
                "Failed to get AI client factory: {error}"
            )))
        })?;

    let ai_client = ai_client_factory
        .get_client_by_func_agent(GOAL_MODE_FUNC_AGENT)
        .await
        .map_err(|error| {
            user_facing_goal_mode_error(BitFunError::AIClient(format!(
                "Failed to get goal func agent client: {error}"
            )))
        })?;

    let response = ai_client
        .send_message(messages, None)
        .await
        .map_err(|error| {
            user_facing_goal_mode_error(BitFunError::ai(format!(
                "Goal func agent call failed: {error}"
            )))
        })?;

    Ok(sanitize_plain_model_output(&response.text))
}

pub async fn generate_goal_from_context(
    context_messages: &[Message],
    user_hint: Option<&str>,
) -> BitFunResult<GoalGenerationResult> {
    let lang_code = get_app_language_code().await;
    let language_instruction = short_model_user_language_instruction(lang_code.as_str());

    let has_user_hint = user_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    let hint_block = user_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("\nUser-provided goal focus: {value}"))
        .unwrap_or_default();
    let focus_priority = if has_user_hint {
        "- The user-provided goal focus is the highest-priority target; use the transcript only to recover constraints, current progress, and acceptance criteria\n"
    } else {
        "- Derive the goal from the latest explicit user request; use earlier conversation only as constraints and context\n"
    };

    let latest_assistant_note = last_assistant_message_text(context_messages)
        .map(|_| {
            "\nUse the full conversation above, using the latest assistant message only to understand current progress and remaining gaps."
                .to_string()
        })
        .unwrap_or_default();

    let system_prompt = format!(
        "You synthesize a single execution-focused session goal for an autonomous coding agent.\n\
The goal is a completion contract for finishing the user's concrete request, not a brainstorming prompt, roadmap, or expanded product vision.\n\
Return ONLY valid JSON with this shape:\n\
{{\"goalText\":\"...\",\"successCriteria\":[\"...\",\"...\"]}}\n\
Requirements:\n\
- {language_instruction}\n\
{focus_priority}\
- Preserve the user's actual intent and scope; do not add optional enhancements, new features, broad research, or speculative follow-up work\n\
- goalText must be imperative, concrete, and directly executable by the agent\n\
- goalText must describe the end state the agent should deliver, not the next thinking step\n\
- successCriteria must list 2-5 objective checks that prove the requested work is complete and landed\n\
- Include verification criteria when the task implies code, tests, build checks, UI behavior, files, or documentation changes\n\
- If the user asks for investigation or explanation only, make that exact deliverable the goal instead of inventing implementation work\n\
- Do not include markdown or commentary"
    );

    let final_user_prompt = format!(
        "Based on the full conversation above,{latest_assistant_note}{hint_block}\n\n\
Extract the smallest goal that would satisfy the user's request and let the agent stop when it is truly done. Return the session goal JSON:"
    );

    let raw = call_goal_func_agent_with_context(system_prompt, context_messages, final_user_prompt)
        .await?;
    parse_goal_generation(&raw)
}

pub async fn verify_goal_achievement(
    state: &GoalModeState,
    context_messages: &[Message],
) -> BitFunResult<GoalVerificationResult> {
    let mut final_user_prompt = build_goal_verification_user_prompt(state);
    let mut repair_attempt = 0_u32;
    let mut retry_count = 0_u32;

    loop {
        let raw_result =
            call_goal_func_agent_with_appended_prompt(context_messages, final_user_prompt.clone())
                .await;

        let error = match raw_result {
            Ok(raw) => match parse_goal_verification(&raw) {
                Ok(result) => return Ok(result),
                Err(BitFunError::Validation(error)) => {
                    repair_attempt = repair_attempt.saturating_add(1);
                    warn!(
                        "Goal verification returned an invalid verdict; requesting repaired verdict: repair_attempt={}, error={}",
                        repair_attempt, error
                    );
                    final_user_prompt =
                        build_goal_verification_repair_prompt(&raw, &error, repair_attempt);
                    BitFunError::Validation(error)
                }
                Err(error) => error,
            },
            Err(error) => error,
        };

        if retry_count >= MAX_GOAL_CONTINUATIONS {
            return Err(error);
        }

        retry_count = retry_count.saturating_add(1);
        let delay_ms = goal_verification_retry_delay_ms(retry_count);
        warn!(
            "Goal verification attempt failed; retrying: retry={}/{}, delay_ms={}, error={}",
            retry_count, MAX_GOAL_CONTINUATIONS, delay_ms, error
        );
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }
}

fn goal_verification_retry_delay_ms(retry_count: u32) -> u64 {
    if retry_count == 0 {
        return GOAL_VERIFICATION_RETRY_BASE_DELAY_MS;
    }
    GOAL_VERIFICATION_RETRY_BASE_DELAY_MS
        .saturating_mul(1_u64 << retry_count.saturating_sub(1).min(5))
        .min(GOAL_VERIFICATION_RETRY_MAX_DELAY_MS)
}

fn build_goal_verification_user_prompt(state: &GoalModeState) -> String {
    let criteria = if state.initial_success_criteria().is_empty() {
        "- Use the initial session goal text itself as the completion standard.".to_string()
    } else {
        state
            .initial_success_criteria()
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let current_goal_note = current_goal_note(state);

    format!(
        "Verify whether the initial session goal has been fully achieved.\n\
You are verifying a coding-agent session goal and producing focused recovery guidance if it has not been achieved.\n\
Initial session goal: {}\n\
{current_goal_note}\
Success criteria:\n{criteria}\n\
Use the initial session goal as the source of truth. If later continuation turns, summaries, or assistant messages narrow, broaden, or drift from it, judge completion against the initial session goal above.\n\
Use the full conversation transcript above, especially the latest assistant work.\n\
Return ONLY valid JSON with this shape:\n\
{{\"achieved\":true|false,\"confidence\":0.0,\"gaps\":[\"...\"],\"guidance\":\"...\"}}\n\
Rules:\n\
- achieved=true ONLY when every success criterion is objectively satisfied in the actual work done\n\
- Be strict: partial progress, plans, or explanations without completed work means achieved=false\n\
- Evaluate the transcript evidence criterion by criterion; distinguish missing proof from missing implementation\n\
- When achieved=false, gaps is REQUIRED and must list concrete unmet criteria or missing evidence, with enough detail for the agent to act without rediscovering the problem\n\
- When achieved=false, guidance is REQUIRED; it must NOT restate the goal, and must say what to do next to close the specific gaps\n\
- guidance must include three compact parts in plain text: first action, why this addresses the gap, and verification to run/check next\n\
- Name relevant files, checks, commands, UI flows, or artifacts when they are inferable from the transcript\n\
- When achieved=true, gaps and guidance must both be empty\n\
- If the previous attempt used a wrong approach, guidance must redirect the approach and explain the correction\n\
- If verification is the main gap, guidance must name the exact verification needed\n\
- Do not include markdown or commentary",
        state.initial_goal_text().trim()
    )
}

fn build_goal_verification_repair_prompt(
    invalid_output: &str,
    validation_error: &str,
    attempt: u32,
) -> String {
    format!(
        "The previous goal verification response was invalid, so you must regenerate the full verification verdict from the transcript.\n\
Repair attempt: {attempt}\n\
Validation error: {validation_error}\n\
Previous invalid response:\n{invalid_output}\n\n\
Return ONLY valid JSON with this exact shape:\n\
{{\"achieved\":true|false,\"confidence\":0.0,\"gaps\":[\"...\"],\"guidance\":\"...\"}}\n\
Contract:\n\
- confidence must be between 0.0 and 1.0\n\
- if achieved=true, gaps and guidance must both be empty\n\
- if achieved=false, gaps and guidance are mandatory\n\
- guidance must include first action, why it addresses the gap, and verification to run/check next\n\
Re-evaluate the transcript and return the corrected JSON verdict now:"
    )
}

fn parse_goal_generation(raw: &str) -> BitFunResult<GoalGenerationResult> {
    let json = extract_json_from_ai_response(raw).ok_or_else(|| {
        BitFunError::Validation(
            "Goal generation returned an unreadable model response.".to_string(),
        )
    })?;
    let mut parsed: GoalGenerationResult = serde_json::from_str(&json).map_err(|error| {
        BitFunError::Validation(format!("Failed to parse goal generation JSON: {error}"))
    })?;
    parsed.goal_text = parsed.goal_text.trim().to_string();
    parsed.success_criteria = parsed
        .success_criteria
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect();
    if parsed.goal_text.is_empty() {
        return Err(BitFunError::Validation(
            "Goal generation returned an empty goal".to_string(),
        ));
    }
    Ok(parsed)
}

fn parse_goal_verification(raw: &str) -> BitFunResult<GoalVerificationResult> {
    let json = extract_json_from_ai_response(raw).ok_or_else(|| {
        BitFunError::Validation(
            "Goal verification returned an unreadable model response.".to_string(),
        )
    })?;
    let mut parsed: GoalVerificationResult = serde_json::from_str(&json).map_err(|error| {
        BitFunError::Validation(format!("Failed to parse goal verification JSON: {error}"))
    })?;
    parsed.guidance = parsed.guidance.trim().to_string();
    parsed.gaps = parsed
        .gaps
        .into_iter()
        .map(|gap| gap.trim().to_string())
        .filter(|gap| !gap.is_empty())
        .collect();
    if !parsed.confidence.is_finite() || !(0.0..=1.0).contains(&parsed.confidence) {
        return Err(BitFunError::Validation(
            "Goal verification returned confidence outside 0.0..=1.0.".to_string(),
        ));
    }
    if parsed.achieved {
        if !parsed.gaps.is_empty() {
            return Err(BitFunError::Validation(
                "Goal verification returned achieved=true with remaining gaps.".to_string(),
            ));
        }
        if !parsed.guidance.is_empty() {
            return Err(BitFunError::Validation(
                "Goal verification returned achieved=true with recovery guidance.".to_string(),
            ));
        }
    } else {
        if parsed.gaps.is_empty() {
            return Err(BitFunError::Validation(
                "Goal verification returned achieved=false without concrete gaps.".to_string(),
            ));
        }
        if parsed.guidance.is_empty() {
            return Err(BitFunError::Validation(
                "Goal verification returned achieved=false without recovery guidance.".to_string(),
            ));
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::core::Message;

    #[test]
    fn effective_subagent_timeout_defaults_to_unlimited_in_goal_mode() {
        assert_eq!(effective_subagent_timeout_seconds(Some(1200), true), None);
        assert_eq!(
            effective_subagent_timeout_seconds(Some(1200), false),
            Some(1200)
        );
        assert_eq!(effective_subagent_timeout_seconds(None, true), None);
    }

    #[test]
    fn goal_mode_patch_round_trips() {
        let state = GoalModeState {
            active: true,
            initial_goal: GoalModeInitialGoal::new(
                "Fix login".to_string(),
                vec!["Tests pass".to_string()],
                None,
                1,
            ),
            goal_text: "Fix login".to_string(),
            success_criteria: vec!["Tests pass".to_string()],
            user_hint: None,
            activated_at_ms: 1,
            continuation_count: 0,
        };
        let patch = goal_mode_patch(&state);
        let parsed = goal_mode_from_custom_metadata(Some(&patch)).expect("goal mode");
        assert_eq!(parsed, state);
    }

    #[test]
    fn legacy_goal_mode_state_falls_back_to_goal_text_as_initial_goal() {
        let metadata = serde_json::json!({
            GOAL_MODE_METADATA_KEY: {
                "active": true,
                "goalText": "Fix legacy goal drift",
                "successCriteria": ["Legacy tests pass"],
                "activatedAtMs": 7,
                "continuationCount": 2,
            }
        });

        let parsed = goal_mode_from_custom_metadata(Some(&metadata)).expect("goal mode");
        assert!(parsed.is_active());
        assert_eq!(parsed.initial_goal_text(), "Fix legacy goal drift");
        assert_eq!(
            parsed.initial_success_criteria(),
            &["Legacy tests pass".to_string()]
        );
        assert_eq!(parsed.initial_goal_created_at_ms(), 7);
    }

    #[test]
    fn goal_verification_prompt_uses_initial_goal_as_source_of_truth() {
        let state = GoalModeState {
            active: true,
            initial_goal: GoalModeInitialGoal::new(
                "Fix the login bug without changing signup".to_string(),
                vec!["Login regression test passes".to_string()],
                Some("login only".to_string()),
                3,
            ),
            goal_text: "Only run final checks".to_string(),
            success_criteria: vec!["Current narrow check".to_string()],
            user_hint: Some("login only".to_string()),
            activated_at_ms: 3,
            continuation_count: 4,
        };

        let user_prompt = build_goal_verification_user_prompt(&state);

        assert!(
            user_prompt.contains("Initial session goal: Fix the login bug without changing signup")
        );
        assert!(user_prompt.contains("Current continuation target: Only run final checks"));
        assert!(user_prompt.contains("source of truth"));
        assert!(user_prompt.contains("Login regression test passes"));
        assert!(!user_prompt.contains("Current narrow check"));
    }

    #[test]
    fn appended_prompt_messages_preserve_session_context_prefix() {
        let context_messages = vec![
            Message::user("Original user request".to_string()),
            Message::assistant("Original assistant progress".to_string()),
        ];

        let messages =
            build_appended_prompt_ai_messages(&context_messages, "Check the goal now".to_string());

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(
            messages[0].content.as_deref(),
            Some("Original user request")
        );
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(
            messages[1].content.as_deref(),
            Some("Original assistant progress")
        );
        assert_eq!(messages[2].role, "user");
        assert_eq!(messages[2].content.as_deref(), Some("Check the goal now"));
    }

    #[test]
    fn goal_verification_retry_delay_grows_and_caps() {
        assert_eq!(goal_verification_retry_delay_ms(1), 1_000);
        assert_eq!(goal_verification_retry_delay_ms(2), 2_000);
        assert_eq!(goal_verification_retry_delay_ms(3), 4_000);
        assert_eq!(goal_verification_retry_delay_ms(6), 30_000);
        assert_eq!(
            goal_verification_retry_delay_ms(MAX_GOAL_CONTINUATIONS),
            30_000
        );
    }

    #[test]
    fn build_goal_context_ai_messages_keeps_full_user_and_assistant_messages() {
        let long_assistant = format!("{}END", "x".repeat(1200));
        let messages = vec![
            Message::user("Implement /goal".to_string()),
            Message::assistant(long_assistant.clone()),
        ];
        let converted = build_goal_context_ai_messages(&messages);
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].content.as_deref(), Some("Implement /goal"));
        assert_eq!(
            converted[1].content.as_deref(),
            Some(long_assistant.as_str())
        );
    }

    #[test]
    fn last_assistant_message_text_returns_latest_assistant() {
        let messages = vec![
            Message::assistant("older".to_string()),
            Message::user("follow up".to_string()),
            Message::assistant("latest".to_string()),
        ];
        assert_eq!(
            last_assistant_message_text(&messages).as_deref(),
            Some("latest")
        );
    }

    #[test]
    fn ensure_final_response_in_goal_context_adds_missing_final_response() {
        let messages = vec![Message::assistant("older".to_string())];
        let context = ensure_final_response_in_goal_context(messages, "final answer", "turn-1");
        assert_eq!(
            last_assistant_message_text(&context).as_deref(),
            Some("final answer")
        );
    }

    #[test]
    fn ensure_final_response_in_goal_context_does_not_duplicate_latest_response() {
        let messages = vec![Message::assistant("final answer".to_string())];
        let context = ensure_final_response_in_goal_context(messages, "final answer", "turn-1");
        assert_eq!(context.len(), 1);
    }

    #[test]
    fn skip_verification_for_maintenance_commands() {
        assert!(should_skip_goal_verification_for_turn("/compact", None));
        assert!(should_skip_goal_verification_for_turn("/usage", None));
        assert!(!should_skip_goal_verification_for_turn("fix bug", None));
    }

    #[test]
    fn user_facing_goal_mode_error_hides_ai_client_details() {
        let mapped =
            user_facing_goal_mode_error(BitFunError::AIClient("provider timeout".to_string()));
        match mapped {
            BitFunError::Validation(message) => {
                assert!(!message.contains("provider timeout"));
                assert!(message.contains("Goal mode AI request failed"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn continuation_plan_includes_goal_text() {
        let state = GoalModeState {
            active: true,
            initial_goal: GoalModeInitialGoal::new("Ship feature".to_string(), vec![], None, 0),
            goal_text: "Ship feature".to_string(),
            success_criteria: vec![],
            user_hint: None,
            activated_at_ms: 0,
            continuation_count: 1,
        };
        let verification = GoalVerificationResult {
            achieved: false,
            confidence: 0.2,
            gaps: vec!["Missing tests".to_string()],
            guidance: "Add tests".to_string(),
        };
        let plan = build_goal_continuation_plan(&state, &verification);
        assert!(plan.wrapped_message.contains("Ship feature"));
        assert!(plan.display_message.contains("Add tests"));
        assert!(!plan.display_message.contains("Ship feature"));
    }

    #[test]
    fn continuation_plan_preserves_initial_goal_when_current_goal_drifts() {
        let state = GoalModeState {
            active: true,
            initial_goal: GoalModeInitialGoal::new(
                "Ship the importer fix with tests".to_string(),
                vec!["Importer tests pass".to_string()],
                None,
                9,
            ),
            goal_text: "Summarize current progress".to_string(),
            success_criteria: vec!["Write summary".to_string()],
            user_hint: None,
            activated_at_ms: 9,
            continuation_count: 3,
        };
        let verification = GoalVerificationResult {
            achieved: false,
            confidence: 0.2,
            gaps: vec!["Importer tests were not run".to_string()],
            guidance: "Run importer tests".to_string(),
        };

        let plan = build_goal_continuation_plan(&state, &verification);
        assert!(plan
            .wrapped_message
            .contains("Initial session goal: Ship the importer fix with tests"));
        assert!(plan
            .wrapped_message
            .contains("Current continuation target: Summarize current progress"));
        assert_eq!(
            plan.user_message_metadata["initialGoal"]["goalText"],
            "Ship the importer fix with tests"
        );
        assert_eq!(
            plan.user_message_metadata["initialGoal"]["successCriteria"][0],
            "Importer tests pass"
        );
    }

    #[test]
    fn parse_goal_generation_accepts_json() {
        let parsed =
            parse_goal_generation(r#"{"goalText":"Fix bug","successCriteria":["Tests pass"]}"#)
                .expect("parsed");
        assert_eq!(parsed.goal_text, "Fix bug");
        assert_eq!(parsed.success_criteria, vec!["Tests pass".to_string()]);
    }

    #[test]
    fn parse_goal_verification_accepts_json() {
        let parsed = parse_goal_verification(
            r#"{"achieved":false,"confidence":0.4,"gaps":["Need tests"],"guidance":"Add tests"}"#,
        )
        .expect("parsed");
        assert!(!parsed.achieved);
        assert_eq!(parsed.guidance, "Add tests");
    }

    #[test]
    fn parse_goal_verification_accepts_clean_achieved_json() {
        let parsed = parse_goal_verification(
            r#"{"achieved":true,"confidence":0.9,"gaps":[],"guidance":""}"#,
        )
        .expect("parsed");
        assert!(parsed.achieved);
        assert!(parsed.gaps.is_empty());
        assert!(parsed.guidance.is_empty());
    }

    #[test]
    fn parse_goal_verification_rejects_confidence_outside_unit_range() {
        let error = parse_goal_verification(
            r#"{"achieved":false,"confidence":1.4,"gaps":["Need tests"],"guidance":"Add tests"}"#,
        )
        .expect_err("invalid confidence should fail");
        assert!(error.to_string().contains("confidence outside"));
    }

    #[test]
    fn parse_goal_verification_rejects_achieved_with_gaps() {
        let error = parse_goal_verification(
            r#"{"achieved":true,"confidence":0.9,"gaps":["Need tests"],"guidance":""}"#,
        )
        .expect_err("achieved with gaps should fail");
        assert!(error
            .to_string()
            .contains("achieved=true with remaining gaps"));
    }

    #[test]
    fn parse_goal_verification_rejects_achieved_with_guidance() {
        let error = parse_goal_verification(
            r#"{"achieved":true,"confidence":0.9,"gaps":[],"guidance":"Run tests next"}"#,
        )
        .expect_err("achieved with guidance should fail");
        assert!(error
            .to_string()
            .contains("achieved=true with recovery guidance"));
    }

    #[test]
    fn parse_goal_verification_requires_gaps_when_not_achieved() {
        let error = parse_goal_verification(
            r#"{"achieved":false,"confidence":0.4,"gaps":[],"guidance":"Add tests"}"#,
        )
        .expect_err("missing gaps should fail");
        assert!(error.to_string().contains("without concrete gaps"));
    }

    #[test]
    fn parse_goal_verification_requires_guidance_when_not_achieved() {
        let error = parse_goal_verification(
            r#"{"achieved":false,"confidence":0.4,"gaps":["Need tests"],"guidance":""}"#,
        )
        .expect_err("missing guidance should fail");
        assert!(error.to_string().contains("without recovery guidance"));
    }
}
