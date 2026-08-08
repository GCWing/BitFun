use crate::chat_state::ModelTokenUsageSnapshot;

fn format_token_count(value: usize) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(digit);
    }
    formatted
}

fn context_window_metrics(usage: &ModelTokenUsageSnapshot) -> Option<(usize, f64)> {
    let max_context_tokens = usage.max_context_tokens.filter(|limit| *limit > 0)?;
    let percentage = usage.total_tokens as f64 / max_context_tokens as f64 * 100.0;
    Some((max_context_tokens, percentage))
}

fn context_window_text(usage: &ModelTokenUsageSnapshot) -> Option<String> {
    let (max_context_tokens, percentage) = context_window_metrics(usage)?;
    Some(format!(
        "{} / {} ({percentage:.1}%)",
        format_token_count(usage.total_tokens),
        format_token_count(max_context_tokens),
    ))
}

fn context_window_detail(usage: &ModelTokenUsageSnapshot) -> Option<String> {
    let (max_context_tokens, percentage) = context_window_metrics(usage)?;
    Some(format!(
        "{} / {} tokens ({percentage:.1}%)",
        format_token_count(usage.total_tokens),
        format_token_count(max_context_tokens),
    ))
}

pub(crate) fn context_status_text(usage: Option<&ModelTokenUsageSnapshot>) -> Option<String> {
    let usage = usage?;
    Some(match context_window_text(usage) {
        Some(context) => format!("Context: {context}"),
        None => format!(
            "Last request: {} tokens",
            format_token_count(usage.total_tokens)
        ),
    })
}

pub(crate) fn default_chat_status_text(chat_state: &ChatState) -> String {
    let mut status = format!(
        "Messages: {} | Tool calls: {}",
        chat_state.metadata.message_count, chat_state.metadata.tool_calls
    );
    if let Some(context) = context_status_text(chat_state.last_primary_model_usage.as_ref()) {
        status.push_str(" | ");
        status.push_str(&context);
    }
    status
}

fn optional_token_count(value: Option<usize>) -> String {
    value
        .map(|tokens| format!("{} tokens", format_token_count(tokens)))
        .unwrap_or_else(|| "unavailable".to_string())
}

pub(crate) fn session_status_text(chat_state: &ChatState, shared_tui: bool) -> String {
    let runtime = if shared_tui {
        "Shared TUI"
    } else {
        "Embedded TUI"
    };
    let processing = if chat_state.is_processing {
        "Processing"
    } else {
        "Idle"
    };
    let approval = if chat_state.auto_approve_ask {
        "Auto"
    } else {
        "Ask"
    };
    let workspace = chat_state
        .workspace
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .unwrap_or("unavailable");
    let model = (!chat_state.current_model_name.trim().is_empty())
        .then_some(chat_state.current_model_name.as_str())
        .or(chat_state.current_model_id.as_deref())
        .unwrap_or("unavailable");

    let mut lines = vec![
        "Status".to_string(),
        String::new(),
        "Session".to_string(),
        format!("  Session: {}", chat_state.session_name),
        format!("  ID: {}", chat_state.core_session_id),
        format!("  Runtime: {runtime}"),
        format!("  State: {processing}"),
        format!("  Agent: {}", chat_state.agent_type),
        format!("  Model: {model}"),
        format!(
            "  Reasoning: {}",
            chat_state
                .current_reasoning_preset
                .as_deref()
                .unwrap_or("Auto")
        ),
        format!("  Approval: {approval}"),
        String::new(),
        "Workspace".to_string(),
        format!("  Path: {workspace}"),
        format!("  Branch: {}", chat_state.branch_label()),
        format!("  Worktree: {}", chat_state.worktree_status_label()),
        String::new(),
        "Last primary model request".to_string(),
    ];

    if let Some(usage) = chat_state.last_primary_model_usage.as_ref() {
        lines.extend([
            format!("  Effective model: {}", usage.effective_model_name),
            format!("  Model config: {}", usage.model_config_id),
            format!("  Input: {} tokens", format_token_count(usage.input_tokens)),
            format!("  Output: {}", optional_token_count(usage.output_tokens)),
            format!(
                "  Cached input: {}",
                optional_token_count(usage.cached_tokens)
            ),
            format!("  Total: {} tokens", format_token_count(usage.total_tokens)),
            format!(
                "  Context window: {}",
                context_window_detail(usage).unwrap_or_else(|| "unavailable".to_string())
            ),
        ]);
    } else {
        lines.push("  Latest request: Not observed by this TUI".to_string());
    }

    lines.push(String::new());
    if shared_tui {
        lines.push("Cumulative session usage is unavailable in Shared TUI.".to_string());
    } else {
        lines.push("For cumulative session usage, use /usage.".to_string());
    }
    lines.join("\n")
}

#[cfg(test)]
mod status_tests {
    use super::*;
    use crate::chat_state::ModelTokenUsageSnapshot;

    fn usage(max_context_tokens: Option<usize>) -> ModelTokenUsageSnapshot {
        ModelTokenUsageSnapshot {
            model_config_id: "model-config-1".to_string(),
            effective_model_name: "example-model".to_string(),
            input_tokens: 80_000,
            output_tokens: Some(2_000),
            total_tokens: 82_000,
            max_context_tokens,
            cached_tokens: Some(10_000),
        }
    }

    #[test]
    fn compact_context_status_uses_the_latest_request_not_a_session_total() {
        assert_eq!(context_status_text(None), None);
        assert_eq!(
            context_status_text(Some(&usage(Some(128_000)))),
            Some("Context: 82,000 / 128,000 (64.1%)".to_string())
        );
        assert_eq!(
            context_status_text(Some(&usage(None))),
            Some("Last request: 82,000 tokens".to_string())
        );
    }

    #[test]
    fn default_status_bar_omits_unknown_usage_instead_of_showing_zero_tokens() {
        let mut state = ChatState::new(
            "session-1".to_string(),
            "Session".to_string(),
            "agentic".to_string(),
            None,
        );
        state.metadata.message_count = 3;
        state.metadata.tool_calls = 2;

        assert_eq!(
            default_chat_status_text(&state),
            "Messages: 3 | Tool calls: 2"
        );

        state.last_primary_model_usage = Some(usage(Some(128_000)));
        assert_eq!(
            default_chat_status_text(&state),
            "Messages: 3 | Tool calls: 2 | Context: 82,000 / 128,000 (64.1%)"
        );
    }

    #[test]
    fn embedded_status_reports_observed_facts_and_points_to_cumulative_usage() {
        let mut state = ChatState::new(
            "session-1".to_string(),
            "Session".to_string(),
            "agentic".to_string(),
            Some("/tmp/project".to_string()),
        );
        state.current_model_name = "Example Model".to_string();
        state.set_git_repository_status(true, Some("main".to_string()));
        state.last_primary_model_usage = Some(usage(Some(128_000)));

        let status = session_status_text(&state, false);

        for expected in [
            "Status",
            "Session: Session",
            "ID: session-1",
            "Runtime: Embedded TUI",
            "State: Idle",
            "Agent: agentic",
            "Model: Example Model",
            "Reasoning: Auto",
            "Approval: Ask",
            "Path: /tmp/project",
            "Branch: main",
            "Worktree: off",
            "Last primary model request",
            "Effective model: example-model",
            "Model config: model-config-1",
            "Input: 80,000 tokens",
            "Output: 2,000 tokens",
            "Cached input: 10,000 tokens",
            "Total: 82,000 tokens",
            "Context window: 82,000 / 128,000 tokens (64.1%)",
            "For cumulative session usage, use /usage.",
        ] {
            assert!(
                status.contains(expected),
                "missing {expected:?} in:\n{status}"
            );
        }
        assert!(!status.contains("Session total"));
    }

    #[test]
    fn shared_status_marks_unobserved_and_unavailable_facts() {
        let mut state = ChatState::new(
            "session-1".to_string(),
            "Session".to_string(),
            "agentic".to_string(),
            None,
        );
        state.is_processing = true;
        state.auto_approve_ask = true;

        let status = session_status_text(&state, true);

        for expected in [
            "Runtime: Shared TUI",
            "State: Processing",
            "Approval: Auto",
            "Path: unavailable",
            "Latest request: Not observed by this TUI",
            "Cumulative session usage is unavailable in Shared TUI.",
        ] {
            assert!(
                status.contains(expected),
                "missing {expected:?} in:\n{status}"
            );
        }
    }
}
