//! Native agent hook process-interface contracts.
//!
//! These tests spawn real hook commands to pin the Codex process contract:
//! the payload arrives on stdin, exit code 0 interprets stdout JSON, exit
//! code 2 blocks with stderr as the reason, other codes warn without
//! blocking, and timeouts kill the handler.
//!
//! Unix-only: the fixtures are `sh` one-liners.
#![cfg(unix)]
#![cfg(feature = "native-hook-runtime")]

use openbitfun_agent_runtime::native_hooks::{
    AgentHookEngine, AgentHookEventPayload, AgentHookOutcome, AgentHookPayload,
    AgentHookPayloadCommon, AgentHookPermissionMode, AgentHookPermissionOutcome, AgentHookScope,
    AgentHookSettings, AgentHookSettingsLayer, MAX_HOOK_MODEL_OUTPUT_BYTES,
};
use serde_json::json;
use std::path::Path;

fn engine(hooks_json: &str) -> AgentHookEngine {
    let (settings, issues) = AgentHookSettings::from_layers(&[AgentHookSettingsLayer {
        scope: AgentHookScope::User,
        source: "test hooks.json".to_string(),
        bytes: hooks_json.as_bytes().to_vec(),
    }]);
    assert!(issues.is_empty(), "unexpected settings issues: {issues:?}");
    AgentHookEngine::new(settings)
}

fn pre_tool_use_payload(tool_name: &str) -> AgentHookPayload {
    AgentHookPayload {
        common: AgentHookPayloadCommon {
            session_id: "session-1".to_string(),
            transcript_path: None,
            cwd: "/".to_string(),
            model: "model-x".to_string(),
            permission_mode: AgentHookPermissionMode::Default,
            turn_id: Some("turn-1".to_string()),
        },
        event: AgentHookEventPayload::PreToolUse {
            tool_name: tool_name.to_string(),
            tool_use_id: "call-1".to_string(),
            tool_input: json!({"command": "ls"}),
        },
    }
}

fn session_start_payload() -> AgentHookPayload {
    AgentHookPayload {
        common: AgentHookPayloadCommon {
            session_id: "session-1".to_string(),
            transcript_path: None,
            cwd: "/".to_string(),
            model: "model-x".to_string(),
            permission_mode: AgentHookPermissionMode::Default,
            turn_id: None,
        },
        event: AgentHookEventPayload::SessionStart {
            source: "startup".to_string(),
        },
    }
}

async fn dispatch(engine: &AgentHookEngine, payload: &AgentHookPayload) -> AgentHookOutcome {
    engine.dispatch(payload, Path::new(".")).await
}

#[tokio::test]
async fn payload_is_delivered_on_stdin() {
    // `cat` echoes the payload JSON, which parses as a decision document with
    // no recognized fields, so nothing is blocked and no context is added.
    let echo_engine =
        engine(r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"cat"}]}]}}"#);
    let outcome = dispatch(&echo_engine, &session_start_payload()).await;

    assert_eq!(outcome.executed_handlers, 1);
    assert!(!outcome.is_blocked());
    assert!(outcome.warnings.is_empty(), "{:?}", outcome.warnings);

    // Now assert the payload content itself reached the process.
    let field_engine = engine(
        r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"python3 -c \"import json,sys; d=json.load(sys.stdin); print(d['hook_event_name'], d['session_id'], d['cwd'], d['model'], d['permission_mode'], d['source'], 'turn_id' in d)\""}]}]}}"#,
    );
    let outcome = dispatch(&field_engine, &session_start_payload()).await;
    assert!(outcome.warnings.is_empty(), "{:?}", outcome.warnings);
    assert_eq!(
        outcome.additional_context,
        vec!["SessionStart session-1 / model-x default startup False".to_string()]
    );
}

#[tokio::test]
async fn exit_code_zero_with_plain_stdout_becomes_context_for_context_events() {
    let engine = engine(
        r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"echo remember this"}]}]}}"#,
    );
    let outcome = dispatch(&engine, &session_start_payload()).await;

    assert_eq!(
        outcome.additional_context,
        vec!["remember this".to_string()]
    );
    assert!(!outcome.is_blocked());
}

#[tokio::test]
async fn plain_stdout_is_ignored_for_non_context_events() {
    let engine = engine(
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"echo chatter"}]}]}}"#,
    );
    let outcome = dispatch(&engine, &pre_tool_use_payload("Bash")).await;

    assert!(outcome.additional_context.is_empty());
    assert!(!outcome.is_blocked());
    assert!(outcome.permission.is_none());
}

#[tokio::test]
async fn exit_code_two_blocks_with_stderr_as_the_reason() {
    let engine = engine(
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"echo not allowed here >&2; exit 2"}]}]}}"#,
    );
    let outcome = dispatch(&engine, &pre_tool_use_payload("Bash")).await;

    assert_eq!(outcome.block_reason.as_deref(), Some("not allowed here"));
    assert!(outcome.warnings.is_empty(), "{:?}", outcome.warnings);
}

#[tokio::test]
async fn other_exit_codes_warn_without_blocking() {
    let engine = engine(
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"echo broken >&2; exit 7"}]}]}}"#,
    );
    let outcome = dispatch(&engine, &pre_tool_use_payload("Bash")).await;

    assert!(!outcome.is_blocked());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(
        outcome.warnings[0].contains("non-blocking code 7"),
        "{:?}",
        outcome.warnings
    );
}

#[tokio::test]
async fn permission_decision_deny_is_honored() {
    let engine = engine(
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"printf '{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"blocked by policy\"}}'"}]}]}}"#,
    );
    let outcome = dispatch(&engine, &pre_tool_use_payload("Bash")).await;

    assert_eq!(
        outcome.permission,
        Some(AgentHookPermissionOutcome::Deny {
            reason: Some("blocked by policy".to_string())
        })
    );
    assert!(outcome.permission_denied());
}

#[tokio::test]
async fn permission_decision_allow_and_updated_input_are_honored() {
    let engine = engine(
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"printf '{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"allow\",\"updatedInput\":{\"command\":\"ls -la\"}}}'"}]}]}}"#,
    );
    let outcome = dispatch(&engine, &pre_tool_use_payload("Bash")).await;

    assert_eq!(
        outcome.permission,
        Some(AgentHookPermissionOutcome::Allow { reason: None })
    );
    assert_eq!(outcome.updated_input, Some(json!({"command": "ls -la"})));
}

#[tokio::test]
async fn legacy_block_decision_is_honored() {
    let engine = engine(
        r#"{"hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":"printf '{\"decision\":\"block\",\"reason\":\"fix the lint errors\"}'"}]}]}}"#,
    );
    let payload = AgentHookPayload {
        common: AgentHookPayloadCommon {
            session_id: "session-1".to_string(),
            transcript_path: None,
            cwd: "/".to_string(),
            model: "model-x".to_string(),
            permission_mode: AgentHookPermissionMode::Default,
            turn_id: Some("turn-1".to_string()),
        },
        event: AgentHookEventPayload::PostToolUse {
            tool_name: "Edit".to_string(),
            tool_use_id: "call-1".to_string(),
            tool_input: json!({}),
            tool_response: json!({}),
        },
    };
    let outcome = dispatch(&engine, &payload).await;

    assert_eq!(outcome.block_reason.as_deref(), Some("fix the lint errors"));
}

#[tokio::test]
async fn additional_context_and_system_message_are_collected() {
    let engine = engine(
        r#"{"hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":"printf '{\"systemMessage\":\"ran the checker\",\"hookSpecificOutput\":{\"hookEventName\":\"PostToolUse\",\"additionalContext\":\"2 files changed\"}}'"}]}]}}"#,
    );
    let payload = AgentHookPayload {
        common: AgentHookPayloadCommon {
            session_id: "session-1".to_string(),
            transcript_path: None,
            cwd: "/".to_string(),
            model: "model-x".to_string(),
            permission_mode: AgentHookPermissionMode::Default,
            turn_id: Some("turn-1".to_string()),
        },
        event: AgentHookEventPayload::PostToolUse {
            tool_name: "Edit".to_string(),
            tool_use_id: "call-1".to_string(),
            tool_input: json!({}),
            tool_response: json!({}),
        },
    };
    let outcome = dispatch(&engine, &payload).await;

    assert_eq!(
        outcome.additional_context,
        vec!["2 files changed".to_string()]
    );
    assert_eq!(outcome.system_messages, vec!["ran the checker".to_string()]);
    assert!(!outcome.is_blocked());
}

#[tokio::test]
async fn continue_false_sets_a_stop_reason() {
    let engine = engine(
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"printf '{\"continue\":false,\"stopReason\":\"budget exhausted\"}'"}]}]}}"#,
    );
    let payload = AgentHookPayload {
        common: AgentHookPayloadCommon {
            session_id: "session-1".to_string(),
            transcript_path: None,
            cwd: "/".to_string(),
            model: "model-x".to_string(),
            permission_mode: AgentHookPermissionMode::Default,
            turn_id: Some("turn-1".to_string()),
        },
        event: AgentHookEventPayload::Stop {
            stop_hook_active: false,
            last_assistant_message: None,
        },
    };
    let outcome = dispatch(&engine, &payload).await;

    assert_eq!(outcome.stop_reason.as_deref(), Some("budget exhausted"));
}

#[tokio::test]
async fn permission_request_decision_behavior_is_honored() {
    let engine = engine(
        r#"{"hooks":{"PermissionRequest":[{"hooks":[{"type":"command","command":"printf '{\"hookSpecificOutput\":{\"hookEventName\":\"PermissionRequest\",\"decision\":{\"behavior\":\"allow\",\"message\":\"trusted path\"}}}'"}]}]}}"#,
    );
    let payload = AgentHookPayload {
        common: AgentHookPayloadCommon {
            session_id: "session-1".to_string(),
            transcript_path: None,
            cwd: "/".to_string(),
            model: "model-x".to_string(),
            permission_mode: AgentHookPermissionMode::Default,
            turn_id: Some("turn-1".to_string()),
        },
        event: AgentHookEventPayload::PermissionRequest {
            tool_name: "Write".to_string(),
            tool_input: json!({"file_path": "/tmp/x"}),
        },
    };
    let outcome = dispatch(&engine, &payload).await;

    assert_eq!(
        outcome.permission,
        Some(AgentHookPermissionOutcome::Allow {
            reason: Some("trusted path".to_string())
        })
    );
}

#[tokio::test]
async fn matchers_select_which_handlers_run() {
    let engine = engine(
        r#"{"hooks":{"PreToolUse":[
            {"matcher":"Bash","hooks":[{"type":"command","command":"echo bash >&2; exit 2"}]},
            {"matcher":"Write","hooks":[{"type":"command","command":"echo write >&2; exit 2"}]}
        ]}}"#,
    );

    let outcome = dispatch(&engine, &pre_tool_use_payload("Bash")).await;
    assert_eq!(outcome.block_reason.as_deref(), Some("bash"));
    assert_eq!(outcome.executed_handlers, 1);

    let outcome = dispatch(&engine, &pre_tool_use_payload("Read")).await;
    assert_eq!(outcome.executed_handlers, 0);
    assert!(!outcome.is_blocked());
}

#[tokio::test]
async fn first_blocking_handler_stops_later_handlers() {
    let engine = engine(
        r#"{"hooks":{"PreToolUse":[{"hooks":[
            {"type":"command","command":"echo first blocks >&2; exit 2"},
            {"type":"command","command":"echo second should not run >&2; exit 2"}
        ]}]}}"#,
    );
    let outcome = dispatch(&engine, &pre_tool_use_payload("Bash")).await;

    assert_eq!(outcome.block_reason.as_deref(), Some("first blocks"));
    assert_eq!(outcome.executed_handlers, 1);
}

#[tokio::test]
async fn handlers_run_in_configuration_order_and_outcomes_merge() {
    let engine = engine(
        r#"{"hooks":{"PreToolUse":[{"hooks":[
            {"type":"command","command":"printf '{\"systemMessage\":\"first\"}'"},
            {"type":"command","command":"printf '{\"systemMessage\":\"second\",\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"updatedInput\":{\"command\":\"safe\"}}}'"}
        ]}]}}"#,
    );
    let outcome = dispatch(&engine, &pre_tool_use_payload("Bash")).await;

    assert_eq!(outcome.executed_handlers, 2);
    assert_eq!(
        outcome.system_messages,
        vec!["first".to_string(), "second".to_string()]
    );
    assert_eq!(outcome.updated_input, Some(json!({"command": "safe"})));
}

#[tokio::test]
async fn a_deny_after_an_allow_wins() {
    let engine = engine(
        r#"{"hooks":{"PreToolUse":[{"hooks":[
            {"type":"command","command":"printf '{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"allow\"}}'"},
            {"type":"command","command":"printf '{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"second says no\"}}'"}
        ]}]}}"#,
    );
    let outcome = dispatch(&engine, &pre_tool_use_payload("Bash")).await;

    assert_eq!(
        outcome.permission,
        Some(AgentHookPermissionOutcome::Deny {
            reason: Some("second says no".to_string())
        })
    );
}

#[tokio::test]
async fn timeouts_kill_the_handler_and_warn() {
    let engine = engine(
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"sleep 30","timeout":1}]}]}}"#,
    );
    let started = std::time::Instant::now();
    let outcome = dispatch(&engine, &pre_tool_use_payload("Bash")).await;

    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "timeout was not enforced"
    );
    assert!(!outcome.is_blocked());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(
        outcome.warnings[0].contains("timed out"),
        "{:?}",
        outcome.warnings
    );
}

#[tokio::test]
async fn a_hook_that_never_reads_a_large_payload_still_times_out() {
    // The payload must exceed the OS pipe buffer so the stdin write blocks
    // until the handler drains it — which this handler never does.
    let engine = engine(
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"sleep 30","timeout":1}]}]}}"#,
    );
    let mut payload = pre_tool_use_payload("Bash");
    payload.event = AgentHookEventPayload::PreToolUse {
        tool_name: "Bash".to_string(),
        tool_use_id: "call-1".to_string(),
        tool_input: json!({ "command": "x".repeat(512 * 1024) }),
    };

    let started = std::time::Instant::now();
    let outcome = dispatch(&engine, &payload).await;

    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "dispatch hung on the stdin write instead of timing out"
    );
    assert!(!outcome.is_blocked());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(
        outcome.warnings[0].contains("timed out"),
        "{:?}",
        outcome.warnings
    );
}

#[tokio::test]
async fn a_hook_that_echoes_a_large_payload_does_not_deadlock() {
    // `cat` reads stdin and writes it straight back. With a payload larger
    // than the pipe buffer in both directions, a sequential write-then-wait
    // would deadlock: the parent blocks writing stdin while the child blocks
    // writing stdout. The write and the wait must be driven concurrently.
    let engine = engine(
        r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"cat","timeout":20}]}]}}"#,
    );
    let mut payload = session_start_payload();
    payload.event = AgentHookEventPayload::SessionStart {
        source: "x".repeat(512 * 1024),
    };

    let started = std::time::Instant::now();
    let outcome = dispatch(&engine, &payload).await;

    assert!(
        started.elapsed() < std::time::Duration::from_secs(15),
        "dispatch deadlocked between the stdin write and the child's stdout"
    );
    assert_eq!(outcome.executed_handlers, 1);
    assert!(outcome.warnings.is_empty(), "{:?}", outcome.warnings);
}

#[tokio::test]
async fn a_hook_that_exits_without_reading_stdin_still_succeeds() {
    // The write fails with EPIPE; that must not turn into a warning or block.
    let engine = engine(
        r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"exec echo done"}]}]}}"#,
    );
    let outcome = dispatch(&engine, &session_start_payload()).await;

    assert!(outcome.warnings.is_empty(), "{:?}", outcome.warnings);
    assert_eq!(outcome.additional_context, vec!["done".to_string()]);
}

#[tokio::test]
async fn model_visible_text_from_json_output_is_capped() {
    // The budget must apply to JSON decision fields, not just plain stdout.
    let engine = engine(
        r#"{"hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":"python3 -c \"import json;print(json.dumps({'hookSpecificOutput':{'hookEventName':'PostToolUse','additionalContext':'x'*50000}}))\""}]}]}}"#,
    );
    let payload = AgentHookPayload {
        common: AgentHookPayloadCommon {
            session_id: "session-1".to_string(),
            transcript_path: None,
            cwd: "/".to_string(),
            model: "model-x".to_string(),
            permission_mode: AgentHookPermissionMode::Default,
            turn_id: Some("turn-1".to_string()),
        },
        event: AgentHookEventPayload::PostToolUse {
            tool_name: "Edit".to_string(),
            tool_use_id: "call-1".to_string(),
            tool_input: json!({}),
            tool_response: json!({}),
        },
    };
    let outcome = dispatch(&engine, &payload).await;

    assert_eq!(outcome.additional_context.len(), 1);
    let context = &outcome.additional_context[0];
    assert!(
        context.len() <= MAX_HOOK_MODEL_OUTPUT_BYTES + 32,
        "context was not capped: {} bytes",
        context.len()
    );
    assert!(
        context.ends_with("[hook output truncated]"),
        "{context:.80}"
    );
}

#[tokio::test]
async fn missing_command_warns_without_blocking() {
    let engine = engine(
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"definitely-not-an-installed-binary-xyz"}]}]}}"#,
    );
    let outcome = dispatch(&engine, &pre_tool_use_payload("Bash")).await;

    // `sh -c` reports a missing binary as exit code 127, a non-blocking error.
    assert!(!outcome.is_blocked());
    assert_eq!(outcome.warnings.len(), 1);
}

#[tokio::test]
async fn events_without_configured_rules_execute_nothing() {
    let engine =
        engine(r#"{"hooks":{"SessionEnd":[{"hooks":[{"type":"command","command":"echo x"}]}]}}"#);
    let outcome = dispatch(&engine, &pre_tool_use_payload("Bash")).await;

    assert_eq!(outcome.executed_handlers, 0);
    assert!(outcome.additional_context.is_empty());
}
