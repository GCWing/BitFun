//! Session goal mode: `/goal` command support with AI goal synthesis and
//! post-turn achievement verification.

mod types;

pub use types::*;

use crate::agentic::core::{Message, MessageContent, MessageRole, PromptEnvelope};
use crate::service::config::{get_app_language_code, short_model_user_language_instruction};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::extract_json_from_ai_response;
use crate::util::sanitize_plain_model_output;
use crate::util::types::Message as AIMessage;
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

pub fn message_text(message: &Message) -> Option<String> {
    match &message.content {
        MessageContent::Text(text) => Some(text.clone()),
        MessageContent::Multimodal { text, .. } => Some(text.clone()),
        MessageContent::Mixed { text, .. } if !text.trim().is_empty() => Some(text.clone()),
        _ => None,
    }
}

pub fn build_recent_context_summary(messages: &[Message], max_chars: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    for message in messages.iter().rev() {
        let role = match message.role {
            MessageRole::User => "User",
            MessageRole::Assistant => "Assistant",
            _ => continue,
        };
        let Some(text) = message_text(message) else {
            continue;
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        let snippet = if trimmed.chars().count() > 800 {
            format!(
                "{}...",
                trimmed.chars().take(800).collect::<String>()
            )
        } else {
            trimmed.to_string()
        };
        lines.push(format!("{role}: {snippet}"));
        if lines.iter().map(|line| line.len()).sum::<usize>() >= max_chars {
            break;
        }
    }
    lines.reverse();
    let mut summary = lines.join("\n\n");
    if summary.chars().count() > max_chars {
        summary = summary.chars().take(max_chars).collect();
        summary.push_str("...");
    }
    summary
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

async fn call_goal_func_agent(system_prompt: String, user_prompt: String) -> BitFunResult<String> {
    let messages = vec![
        AIMessage {
            role: "system".to_string(),
            content: Some(system_prompt),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        },
        AIMessage {
            role: "user".to_string(),
            content: Some(user_prompt),
            reasoning_content: None,
            thinking_signature: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: None,
            tool_image_attachments: None,
        },
    ];

    let ai_client_factory = crate::infrastructure::ai::get_global_ai_client_factory()
        .await
        .map_err(|error| BitFunError::AIClient(format!("Failed to get AI client factory: {error}")))?;

    let ai_client = ai_client_factory
        .get_client_by_func_agent(GOAL_MODE_FUNC_AGENT)
        .await
        .map_err(|error| BitFunError::AIClient(format!("Failed to get goal func agent client: {error}")))?;

    let response = ai_client
        .send_message(messages, None)
        .await
        .map_err(|error| BitFunError::ai(format!("Goal func agent call failed: {error}")))?;

    Ok(sanitize_plain_model_output(&response.text))
}

pub async fn generate_goal_from_context(
    context_summary: &str,
    user_hint: Option<&str>,
    final_response: Option<&str>,
) -> BitFunResult<GoalGenerationResult> {
    let lang_code = get_app_language_code().await;
    let language_instruction = short_model_user_language_instruction(lang_code.as_str());

    let hint_block = user_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("\nUser-provided goal focus: {value}"))
        .unwrap_or_default();

    let response_block = final_response
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("\nLatest assistant response:\n{value}"))
        .unwrap_or_default();

    let system_prompt = format!(
        "You synthesize a single actionable session goal from conversation context.\n\
Return ONLY valid JSON with this shape:\n\
{{\"goalText\":\"...\",\"successCriteria\":[\"...\",\"...\"]}}\n\
Requirements:\n\
- {language_instruction}\n\
- goalText must be concrete and verifiable\n\
- successCriteria must list 2-5 objective completion checks\n\
- Do not include markdown or commentary"
    );

    let user_prompt = format!(
        "Conversation context:\n{context_summary}{hint_block}{response_block}\n\n\
Synthesize the session goal JSON:"
    );

    let raw = call_goal_func_agent(system_prompt, user_prompt).await?;
    parse_goal_generation(&raw)
}

pub async fn verify_goal_achievement(
    state: &GoalModeState,
    context_summary: &str,
    final_response: &str,
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

    let system_prompt = "You verify whether a coding-agent session goal has truly been achieved.\n\
Return ONLY valid JSON with this shape:\n\
{\"achieved\":true|false,\"confidence\":0.0,\"gaps\":[\"...\"],\"guidance\":\"...\"}\n\
Rules:\n\
- achieved=true ONLY when every success criterion is objectively satisfied in the actual work done\n\
- Be strict: partial progress, plans, or explanations without completed work means achieved=false\n\
- gaps must list concrete missing items when achieved=false\n\
- guidance must be actionable next steps for the agent\n\
- Do not include markdown or commentary"
        .to_string();

    let user_prompt = format!(
        "Goal: {}\n\
Success criteria:\n{criteria}\n\
Conversation context:\n{context_summary}\n\
Latest assistant response:\n{final_response}\n\n\
Verify goal completion JSON:",
        state.goal_text.trim(),
        criteria = criteria,
        context_summary = context_summary,
        final_response = final_response,
    );

    let raw = call_goal_func_agent(system_prompt, user_prompt).await?;
    parse_goal_verification(&raw)
}

fn parse_goal_generation(raw: &str) -> BitFunResult<GoalGenerationResult> {
    let json = extract_json_from_ai_response(raw).ok_or_else(|| {
        BitFunError::Validation(format!("Goal generation returned non-JSON output: {raw}"))
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
        BitFunError::Validation(format!("Goal verification returned non-JSON output: {raw}"))
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
    fn build_recent_context_summary_keeps_user_and_assistant_messages() {
        let messages = vec![
            Message::user("Implement /goal".to_string()),
            Message::assistant("Working on it".to_string()),
        ];
        let summary = build_recent_context_summary(&messages, 1000);
        assert!(summary.contains("Implement /goal"));
        assert!(summary.contains("Working on it"));
    }

    #[test]
    fn skip_verification_for_maintenance_commands() {
        assert!(should_skip_goal_verification_for_turn("/compact", None));
        assert!(should_skip_goal_verification_for_turn("/usage", None));
        assert!(!should_skip_goal_verification_for_turn("fix bug", None));
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
        let parsed = parse_goal_generation(
            r#"{"goalText":"Fix bug","successCriteria":["Tests pass"]}"#,
        )
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
