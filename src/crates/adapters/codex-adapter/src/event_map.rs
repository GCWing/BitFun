//! Codex → OpenCode hook event name translation.
//!
//! Codex plugin hooks use their own event naming convention (PreToolUse,
//! PostToolUse, etc.). This module maps them to OpenCode-compatible event
//! names so the existing lifecycle infrastructure can route them.

/// Translates a Codex hook event name to an OpenCode event name.
/// Returns `None` if the event is not recognized.
pub fn translate_codex_event(codex_event: &str) -> Option<&'static str> {
    match codex_event {
        "PreToolUse" => Some("tool.execute.before"),
        "PostToolUse" => Some("tool.execute.after"),
        "PermissionRequest" => Some("permission.asked"),
        "PreCompact" => Some("session.compacting"),
        "PostCompact" => Some("session.compacted"),
        "SessionStart" => Some("session.started"),
        "UserPromptSubmit" => Some("user.prompt_submit"),
        "SubagentStart" => Some("subagent.started"),
        "SubagentStop" => Some("subagent.stopped"),
        "Stop" => Some("session.stopping"),
        _ => None,
    }
}

/// Returns all recognized Codex event names.
pub const CODEX_EVENT_NAMES: &[&str] = &[
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PreCompact",
    "PostCompact",
    "SessionStart",
    "UserPromptSubmit",
    "SubagentStart",
    "SubagentStop",
    "Stop",
];

/// Returns the OpenCode event name for a Codex event, or the original name
/// if no translation is available.
pub fn translate_or_passthrough(codex_event: &str) -> &str {
    translate_codex_event(codex_event).unwrap_or(codex_event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_events_have_translations() {
        for event in CODEX_EVENT_NAMES {
            let translated = translate_codex_event(event);
            assert!(
                translated.is_some(),
                "Codex event '{}' should have an OpenCode translation",
                event
            );
        }
    }

    #[test]
    fn test_pre_tool_use_maps_to_tool_execute_before() {
        assert_eq!(
            translate_codex_event("PreToolUse"),
            Some("tool.execute.before")
        );
    }

    #[test]
    fn test_post_tool_use_maps_to_tool_execute_after() {
        assert_eq!(
            translate_codex_event("PostToolUse"),
            Some("tool.execute.after")
        );
    }

    #[test]
    fn test_session_start_maps_correctly() {
        assert_eq!(
            translate_codex_event("SessionStart"),
            Some("session.started")
        );
    }

    #[test]
    fn test_stop_maps_correctly() {
        assert_eq!(translate_codex_event("Stop"), Some("session.stopping"));
    }

    #[test]
    fn test_unknown_event_returns_none() {
        assert_eq!(translate_codex_event("UnknownEvent"), None);
    }

    #[test]
    fn test_passthrough_for_unknown_event() {
        assert_eq!(
            translate_or_passthrough("custom.hook"),
            "custom.hook"
        );
    }
}
