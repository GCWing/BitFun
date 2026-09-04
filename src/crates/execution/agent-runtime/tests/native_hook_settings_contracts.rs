//! Native agent hook settings parsing contracts.
//!
//! These assertions pin the Codex-compatible configuration surface: document
//! shape, the fixed event list, matcher semantics, handler fields, timeout
//! defaults, and the layer/limit rules.

#![cfg(feature = "native-hook-settings")]

use openbitfun_agent_runtime::native_hooks::{
    AgentHookEvent, AgentHookScope, AgentHookSettings, AgentHookSettingsIssue,
    AgentHookSettingsLayer, MAX_HOOK_HANDLERS,
};

fn layer(scope: AgentHookScope, source: &str, json: &str) -> AgentHookSettingsLayer {
    AgentHookSettingsLayer {
        scope,
        source: source.to_string(),
        bytes: json.as_bytes().to_vec(),
    }
}

fn user_layer(json: &str) -> AgentHookSettingsLayer {
    layer(AgentHookScope::User, "user hooks.json", json)
}

#[test]
fn parses_codex_document_shape_with_matcher_and_command_handler() {
    let (settings, issues) = AgentHookSettings::from_layers(&[user_layer(
        r#"{
          "description": "example",
          "hooks": {
            "PreToolUse": [
              {
                "matcher": "Bash",
                "hooks": [
                  {
                    "type": "command",
                    "command": "python3 check.py",
                    "timeout": 30,
                    "statusMessage": "Checking command"
                  }
                ]
              }
            ]
          }
        }"#,
    )]);

    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    let rules = settings.rules_for(AgentHookEvent::PreToolUse);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].handlers.len(), 1);
    assert_eq!(rules[0].handlers[0].command, "python3 check.py");
    assert_eq!(rules[0].handlers[0].timeout_seconds, Some(30));
    assert_eq!(
        rules[0].handlers[0].status_message.as_deref(),
        Some("Checking command")
    );
    assert_eq!(rules[0].scope, AgentHookScope::User);
    assert_eq!(settings.total_handlers(), 1);
}

#[test]
fn supports_every_codex_event_name() {
    for event in AgentHookEvent::ALL {
        let json = format!(
            r#"{{"hooks":{{"{}":[{{"hooks":[{{"type":"command","command":"true"}}]}}]}}}}"#,
            event.as_str()
        );
        let (settings, issues) = AgentHookSettings::from_layers(&[user_layer(&json)]);
        assert!(issues.is_empty(), "{event} produced issues: {issues:?}");
        assert!(settings.has_rules(event), "{event} rule was not registered");
    }
}

#[test]
fn unknown_event_names_are_dropped_but_valid_events_survive() {
    let (settings, issues) = AgentHookSettings::from_layers(&[user_layer(
        r#"{
          "hooks": {
            "PreToolUes": [{"hooks":[{"type":"command","command":"typo"}]}],
            "PreToolUse": [{"hooks":[{"type":"command","command":"kept"}]}]
          }
        }"#,
    )]);

    assert!(issues.iter().any(|issue| matches!(
        issue,
        AgentHookSettingsIssue::EventNameUnsupported { event, .. } if event == "PreToolUes"
    )));
    let rules = settings.rules_for(AgentHookEvent::PreToolUse);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].handlers[0].command, "kept");
}

#[test]
fn unexpected_root_keys_reject_the_whole_document() {
    let (settings, issues) = AgentHookSettings::from_layers(&[user_layer(
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"x"}]}]},"unexpected":true}"#,
    )]);

    assert!(settings.is_empty());
    assert!(issues
        .iter()
        .any(|issue| matches!(issue, AgentHookSettingsIssue::DocumentInvalid { .. })));
}

#[test]
fn reserved_state_table_is_not_treated_as_an_event() {
    let (settings, issues) = AgentHookSettings::from_layers(&[user_layer(
        r#"{"hooks":{"state":{"anything":{"enabled":false}}}}"#,
    )]);

    assert!(settings.is_empty());
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
}

#[test]
fn matcher_semantics_follow_codex_rules() {
    let (settings, issues) = AgentHookSettings::from_layers(&[user_layer(
        r#"{
          "hooks": {
            "PreToolUse": [
              {"hooks":[{"type":"command","command":"absent"}]},
              {"matcher":"","hooks":[{"type":"command","command":"empty"}]},
              {"matcher":"*","hooks":[{"type":"command","command":"star"}]},
              {"matcher":"Bash","hooks":[{"type":"command","command":"exact"}]},
              {"matcher":"^Bash$","hooks":[{"type":"command","command":"anchored"}]},
              {"matcher":"Edit|Write","hooks":[{"type":"command","command":"alternation"}]},
              {"matcher":"mcp__filesystem__.*","hooks":[{"type":"command","command":"wildcard"}]}
            ]
          }
        }"#,
    )]);
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    let rules = settings.rules_for(AgentHookEvent::PreToolUse);
    assert_eq!(rules.len(), 7);

    // Absent, empty, and "*" match everything.
    for rule in &rules[..3] {
        assert!(rule.matcher.matches(Some("AnyTool")));
        assert!(rule.matcher.matches(None));
    }
    // "Bash" is an exact whole-value match, not a substring match.
    assert!(rules[3].matcher.matches(Some("Bash")));
    assert!(!rules[3].matcher.matches(Some("BashOutput")));
    // Anchored regex behaves the same.
    assert!(rules[4].matcher.matches(Some("Bash")));
    assert!(!rules[4].matcher.matches(Some("Bashful")));
    // Alternation matches either branch and nothing else.
    assert!(rules[5].matcher.matches(Some("Edit")));
    assert!(rules[5].matcher.matches(Some("Write")));
    assert!(!rules[5].matcher.matches(Some("Read")));
    // Regex wildcards match MCP tool families by prefix.
    assert!(rules[6].matcher.matches(Some("mcp__filesystem__read_file")));
    assert!(!rules[6].matcher.matches(Some("mcp__github__search")));
}

#[test]
fn malformed_matchers_never_match_everything() {
    let (settings, issues) = AgentHookSettings::from_layers(&[user_layer(
        r#"{"hooks":{"PreToolUse":[{"matcher":{"tool":"Bash"},"hooks":[{"type":"command","command":"x"}]}]}}"#,
    )]);

    assert!(issues
        .iter()
        .any(|issue| matches!(issue, AgentHookSettingsIssue::MatcherInvalid { .. })));
    let rules = settings.rules_for(AgentHookEvent::PreToolUse);
    assert_eq!(rules.len(), 1);
    assert!(!rules[0].matcher.matches(Some("Bash")));
    assert!(!rules[0].matcher.matches(None));
}

#[test]
fn unparsable_regex_matcher_is_reported_and_never_matches() {
    let (settings, issues) = AgentHookSettings::from_layers(&[user_layer(
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash(","hooks":[{"type":"command","command":"x"}]}]}}"#,
    )]);

    assert!(issues
        .iter()
        .any(|issue| matches!(issue, AgentHookSettingsIssue::MatcherInvalid { .. })));
    assert!(!settings.rules_for(AgentHookEvent::PreToolUse)[0]
        .matcher
        .matches(Some("Bash(")));
}

#[test]
fn prompt_and_agent_handlers_are_recognized_but_not_executable() {
    let (settings, issues) = AgentHookSettings::from_layers(&[user_layer(
        r#"{
          "hooks": {
            "SessionStart": [
              {"hooks":[
                {"type":"prompt","prompt":"remind me"},
                {"type":"agent","prompt":"delegate"},
                {"type":"command","command":"echo ok"}
              ]}
            ]
          }
        }"#,
    )]);

    assert_eq!(
        issues
            .iter()
            .filter(|issue| matches!(issue, AgentHookSettingsIssue::HandlerUnsupported { .. }))
            .count(),
        2
    );
    let rules = settings.rules_for(AgentHookEvent::SessionStart);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].handlers.len(), 1);
    assert_eq!(rules[0].handlers[0].command, "echo ok");
}

#[test]
fn invalid_handlers_are_dropped_without_losing_valid_siblings() {
    let (settings, issues) = AgentHookSettings::from_layers(&[user_layer(
        r#"{
          "hooks": {
            "PostToolUse": [
              {"hooks":[
                {"type":"http","url":"https://example.test"},
                {"type":"command"},
                {"type":"command","command":"   "},
                {"type":"command","command":"echo ok","timeout":0},
                {"type":"command","command":"kept"}
              ]}
            ]
          }
        }"#,
    )]);

    assert_eq!(
        issues
            .iter()
            .filter(|issue| matches!(issue, AgentHookSettingsIssue::HandlerInvalid { .. }))
            .count(),
        4
    );
    let rules = settings.rules_for(AgentHookEvent::PostToolUse);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].handlers.len(), 1);
    assert_eq!(rules[0].handlers[0].command, "kept");
}

#[test]
fn malformed_event_and_group_shapes_are_reported() {
    let (settings, issues) = AgentHookSettings::from_layers(&[user_layer(
        r#"{
          "hooks": {
            "PreToolUse": {"not":"an array"},
            "PostToolUse": ["not an object", {"missing":"hooks array"}]
          }
        }"#,
    )]);

    assert!(settings.is_empty());
    assert!(issues
        .iter()
        .any(|issue| matches!(issue, AgentHookSettingsIssue::EventInvalid { .. })));
    assert_eq!(
        issues
            .iter()
            .filter(|issue| matches!(issue, AgentHookSettingsIssue::GroupInvalid { .. }))
            .count(),
        2
    );
}

#[test]
fn timeout_defaults_and_caps_follow_codex() {
    let (settings, _) = AgentHookSettings::from_layers(&[user_layer(
        r#"{
          "hooks": {
            "PreToolUse": [{"hooks":[{"type":"command","command":"a"}]}],
            "SessionEnd": [{"hooks":[
              {"type":"command","command":"b"},
              {"type":"command","command":"c","timeout":30}
            ]}]
          }
        }"#,
    )]);

    let pre = &settings.rules_for(AgentHookEvent::PreToolUse)[0].handlers[0];
    assert_eq!(
        pre.effective_timeout(AgentHookEvent::PreToolUse).as_secs(),
        600
    );

    let session_end = &settings.rules_for(AgentHookEvent::SessionEnd)[0].handlers;
    assert_eq!(
        session_end[0]
            .effective_timeout(AgentHookEvent::SessionEnd)
            .as_secs(),
        1
    );
    // A configured SessionEnd timeout is capped so session teardown cannot hang.
    assert_eq!(
        session_end[1]
            .effective_timeout(AgentHookEvent::SessionEnd)
            .as_secs(),
        3
    );
}

#[test]
fn user_layers_are_ordered_before_project_layers() {
    let (settings, issues) = AgentHookSettings::from_layers(&[
        user_layer(r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"user"}]}]}}"#),
        layer(
            AgentHookScope::Project,
            "project hooks.json",
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"project"}]}]}}"#,
        ),
    ]);

    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    let rules = settings.rules_for(AgentHookEvent::Stop);
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].scope, AgentHookScope::User);
    assert_eq!(rules[0].handlers[0].command, "user");
    assert_eq!(rules[1].scope, AgentHookScope::Project);
    assert_eq!(rules[1].handlers[0].command, "project");
}

#[test]
fn handler_limit_is_enforced_across_layers() {
    let handlers = (0..MAX_HOOK_HANDLERS + 10)
        .map(|index| format!(r#"{{"type":"command","command":"echo {index}"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(r#"{{"hooks":{{"Stop":[{{"hooks":[{handlers}]}}]}}}}"#);
    let (settings, issues) = AgentHookSettings::from_layers(&[user_layer(&json)]);

    assert_eq!(settings.total_handlers(), MAX_HOOK_HANDLERS);
    assert!(issues
        .iter()
        .any(|issue| matches!(issue, AgentHookSettingsIssue::HandlerLimitExceeded { .. })));
}

#[test]
fn non_json_documents_are_reported_as_invalid() {
    let (settings, issues) = AgentHookSettings::from_layers(&[user_layer("not json at all")]);

    assert!(settings.is_empty());
    assert!(issues
        .iter()
        .any(|issue| matches!(issue, AgentHookSettingsIssue::DocumentInvalid { .. })));
}

#[test]
fn missing_hooks_key_is_accepted_without_issues() {
    let (settings, issues) =
        AgentHookSettings::from_layers(&[user_layer(r#"{"description":"nothing configured"}"#)]);

    assert!(settings.is_empty());
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
}

#[test]
fn turn_scope_and_context_flags_match_the_documented_events() {
    assert!(!AgentHookEvent::SessionStart.is_turn_scoped());
    assert!(!AgentHookEvent::SessionEnd.is_turn_scoped());
    for event in AgentHookEvent::ALL {
        if !matches!(
            event,
            AgentHookEvent::SessionStart | AgentHookEvent::SessionEnd
        ) {
            assert!(event.is_turn_scoped(), "{event} should carry turn_id");
        }
    }

    let context_events = AgentHookEvent::ALL
        .into_iter()
        .filter(|event| event.plain_stdout_is_context())
        .collect::<Vec<_>>();
    assert_eq!(
        context_events,
        vec![
            AgentHookEvent::SessionStart,
            AgentHookEvent::UserPromptSubmit,
            AgentHookEvent::SubagentStart,
        ]
    );
}
