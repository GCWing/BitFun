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
use std::time::{SystemTime, UNIX_EPOCH};

pub fn goal_mode_from_custom_metadata(
    custom_metadata: Option<&serde_json::Value>,
) -> Option<GoalModeState> {
    let value = custom_metadata?.get(GOAL_MODE_METADATA_KEY)?;
    serde_json::from_value(value.clone()).ok()
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

pub fn build_goal_system_reminder(state: &GoalModeState) -> String {
    let criteria = if state.success_criteria.is_empty() {
        "- Use your best judgment to decide when the goal is fully complete.".to_string()
    } else {
        state
            .success_criteria
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "Active session goal mode is ON.\n\
Goal: {}\n\
Success criteria:\n{}\n\
Keep working toward this goal. Do not declare the task finished until every criterion is truly satisfied.",
        state.goal_text.trim(),
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
    let gaps = if verification.gaps.is_empty() {
        "- The goal is not fully complete yet.".to_string()
    } else {
        verification
            .gaps
            .iter()
            .map(|gap| format!("- {gap}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let guidance = verification.guidance.trim();
    let guidance_block = if guidance.is_empty() {
        "Continue working on the remaining gaps before stopping.".to_string()
    } else {
        guidance.to_string()
    };

    let display_message = format!(
        "Goal not yet achieved — continuing work on: {}",
        state.goal_text
    );

    let wrapped_message = {
        let mut envelope = PromptEnvelope::new();
        envelope.push_system_reminder(format!(
            "Goal verification found the active session goal is NOT yet achieved.\n\
Goal: {}\n\
Remaining gaps:\n{gaps}\n\
Next steps:\n{guidance_block}\n\
Continue working until the goal is fully satisfied. Do not stop early.",
            state.goal_text.trim()
        ));
        envelope.push_user_query(format!(
            "Continue working toward the session goal. Address the remaining gaps and complete the goal before stopping.\n\nGoal: {}",
            state.goal_text.trim()
        ));
        envelope.render()
    };

    GoalContinuationPlan {
        wrapped_message,
        display_message,
        user_message_metadata: serde_json::json!({
            "goalModeContinuation": true,
            "goalText": state.goal_text,
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

    let hint_block = user_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("\nUser-provided goal focus: {value}"))
        .unwrap_or_default();

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
- Derive the goal from the latest explicit user request or user-provided focus; use earlier conversation only as constraints and context\n\
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
    let criteria = if state.success_criteria.is_empty() {
        "- Use the goal text itself as the completion standard.".to_string()
    } else {
        state
            .success_criteria
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let system_prompt = format!(
        "You verify whether a coding-agent session goal has truly been achieved.\n\
Active goal: {}\n\
Success criteria:\n{criteria}\n\
Use the full conversation transcript above, especially the latest assistant work.\n\
Return ONLY valid JSON with this shape:\n\
{{\"achieved\":true|false,\"confidence\":0.0,\"gaps\":[\"...\"],\"guidance\":\"...\"}}\n\
Rules:\n\
- achieved=true ONLY when every success criterion is objectively satisfied in the actual work done\n\
- Be strict: partial progress, plans, or explanations without completed work means achieved=false\n\
- gaps must list concrete missing items when achieved=false\n\
- guidance must be actionable next steps for the agent\n\
- Do not include markdown or commentary",
        state.goal_text.trim()
    );

    let final_user_prompt =
        "Verify whether the active session goal has been fully achieved. Return the JSON verdict."
            .to_string();

    let raw = call_goal_func_agent_with_context(system_prompt, context_messages, final_user_prompt)
        .await?;
    parse_goal_verification(&raw)
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
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::core::Message;

    #[test]
    fn effective_subagent_timeout_defaults_to_unlimited_in_goal_mode() {
        assert_eq!(effective_subagent_timeout_seconds(Some(1200), true), None);
        assert_eq!(effective_subagent_timeout_seconds(Some(1200), false), Some(1200));
        assert_eq!(effective_subagent_timeout_seconds(None, true), None);
    }

    #[test]
    fn goal_mode_patch_round_trips() {
        let state = GoalModeState {
            active: true,
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
        assert!(plan.display_message.contains("Ship feature"));
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
}
