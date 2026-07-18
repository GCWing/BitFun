mod support;

use std::process::{Command, Output};
use support::{CliTestEnvironment, MockOpenAiServer, STREAM_COMPLETED_MARKER};

fn run_cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bitfun-cli"))
        .args(args)
        .output()
        .expect("run bitfun-cli")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn jsonl_events(output: &str) -> Vec<serde_json::Value> {
    output
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .unwrap_or_else(|error| panic!("invalid JSONL line {line:?}: {error}"))
        })
        .collect()
}

fn is_terminal_event(value: &serde_json::Value) -> bool {
    matches!(
        value["event"]["type"].as_str(),
        Some("DialogTurnCompleted" | "DialogTurnCancelled" | "DialogTurnFailed" | "SystemError")
    )
}

#[test]
fn exec_help_uses_competitor_aligned_output_and_approval_flags() {
    let output = run_cli(&["exec", "--help"]);
    let stdout = stdout(&output);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout.contains("--auto"), "{stdout}");
    assert!(stdout.contains("--output-format"), "{stdout}");
    for format in ["text", "json", "stream-json"] {
        assert!(stdout.contains(format), "missing {format}: {stdout}");
    }
    assert!(!stdout.contains("--output-schema"), "{stdout}");
    assert!(
        !stdout.contains("--confirm"),
        "deprecated compatibility flag must stay out of public help: {stdout}"
    );
}

#[test]
fn exec_accepts_hidden_confirm_compatibility_flag() {
    let output = run_cli(&["exec", "--confirm", "--help"]);

    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn exec_rejects_auto_with_legacy_confirm() {
    let output = run_cli(&["exec", "task", "--auto", "--confirm"]);
    let stderr = stderr(&output);

    assert!(!output.status.success(), "{}", stdout(&output));
    assert!(stderr.contains("cannot be used with"), "{stderr}");
    assert!(stderr.contains("--auto"), "{stderr}");
    assert!(stderr.contains("--confirm"), "{stderr}");
}

#[test]
fn exec_json_clap_failure_is_one_result_document() {
    let output = run_cli(&[
        "exec",
        "task",
        "--output-format",
        "json",
        "--auto",
        "--confirm",
    ]);
    let stdout = stdout(&output);

    assert!(!output.status.success(), "{stdout}");
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(stderr(&output).is_empty(), "{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("one JSON parser error result");
    assert_eq!(value["type"], "result");
    assert_eq!(value["subtype"], "error");
    assert_eq!(value["is_error"], true);
    assert!(value["result"]
        .as_str()
        .is_some_and(|message| message.contains("--auto") && message.contains("--confirm")));
}

#[test]
fn exec_json_help_preserves_clap_success_semantics() {
    let output = run_cli(&["exec", "--output-format", "json", "--help"]);
    let stdout = stdout(&output);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout.contains("Usage:"), "{stdout}");
    assert!(stdout.contains("--output-format"), "{stdout}");
    assert!(!stdout.contains("\"subtype\": \"error\""), "{stdout}");
    assert!(stderr(&output).is_empty(), "{}", stderr(&output));
}

#[test]
fn exec_json_preflight_failure_is_one_result_document() {
    let output = run_cli(&[
        "exec",
        "task",
        "--output-format",
        "json",
        "--continue",
        "--session-id",
        "fixed-id",
    ]);
    let stdout = stdout(&output);

    assert!(!output.status.success(), "{stdout}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("one JSON result object");
    assert_eq!(value["type"], "result");
    assert_eq!(value["subtype"], "error");
    assert_eq!(value["is_error"], true);
    assert!(value.get("session_id").is_none());
    assert!(value.get("turn_id").is_none());
    assert!(value["result"]
        .as_str()
        .is_some_and(|message| message.contains("--session-id")));
}

#[test]
fn exec_json_rejects_continue_with_an_explicit_resume() {
    let output = run_cli(&[
        "exec",
        "task",
        "--output-format",
        "json",
        "--continue",
        "--resume",
        "session-1",
    ]);
    let stdout = stdout(&output);

    assert!(!output.status.success(), "{stdout}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("one JSON error result");
    assert!(value["result"]
        .as_str()
        .is_some_and(|message| message.contains("--continue") && message.contains("--resume")));
}

#[test]
fn stream_json_rejects_stdout_patch_before_starting_runtime() {
    let output = run_cli(&[
        "exec",
        "task",
        "--output-format",
        "stream-json",
        "--output-patch",
    ]);

    assert!(!output.status.success(), "{}", stdout(&output));
    assert!(
        stdout(&output).is_empty(),
        "protocol stdout must stay empty"
    );
    assert!(
        stderr(&output).contains("requires an explicit file path"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn stream_json_patch_write_failure_emits_error_without_success_terminal() {
    let server = MockOpenAiServer::immediate();
    let environment = CliTestEnvironment::new();
    environment.initialize_git_repository();
    environment.configure_mock_model(server.base_url());
    let output_target = environment.workspace().to_string_lossy().into_owned();
    let output = environment
        .std_command()
        .args([
            "exec",
            "exercise patch settlement",
            "--output-format",
            "stream-json",
            "--output-patch",
            &output_target,
        ])
        .output()
        .expect("run stream-json patch failure contract");

    let stdout = stdout(&output);
    assert!(!output.status.success(), "{stdout}");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output)
            .lines()
            .any(|line| line.starts_with("BITFUN_EXIT: patch_write_failed:")),
        "missing stable patch failure diagnostic: {}",
        stderr(&output)
    );
    assert!(!stdout.trim().is_empty(), "missing stream-json events");
    let events = jsonl_events(&stdout);
    let completed_marker_index = events
        .iter()
        .position(|value| {
            value["event"]["type"] == "TextChunk"
                && value["event"]["text"]
                    .as_str()
                    .is_some_and(|text| text.contains(STREAM_COMPLETED_MARKER))
        })
        .unwrap_or_else(|| {
            panic!("model stream did not complete before patch settlement: {stdout}")
        });
    let patch_error_index = events
        .iter()
        .position(|value| {
            value["event"]["type"] == "SystemError"
                && value["event"]["error"]
                    .as_str()
                    .is_some_and(|error| error.contains("Failed to save requested patch"))
        })
        .unwrap_or_else(|| {
            panic!("patch failure did not emit a structured system error: {stdout}")
        });
    assert!(
        completed_marker_index < patch_error_index,
        "patch failure was emitted before model stream completion: {stdout}"
    );
    assert!(
        events
            .iter()
            .all(|value| value["event"]["type"] != "DialogTurnCompleted"),
        "successful terminal event leaked before patch settlement: {stdout}"
    );
    assert_eq!(
        events
            .iter()
            .filter(|value| is_terminal_event(value))
            .count(),
        1,
        "patch failure must emit exactly one terminal envelope: {stdout}"
    );
    let terminal_event = events.last().expect("stream-json terminal event");
    assert_eq!(terminal_event["event"]["type"], "SystemError", "{stdout}");
    assert_eq!(terminal_event["event"]["recoverable"], false, "{stdout}");
    assert!(
        terminal_event["event"]["error"]
            .as_str()
            .is_some_and(|error| error.contains("Failed to save requested patch")),
        "unexpected terminal patch failure: {stdout}"
    );
}

#[test]
fn stream_json_patch_success_emits_one_success_terminal() {
    let server = MockOpenAiServer::immediate();
    let environment = CliTestEnvironment::new();
    environment.initialize_git_repository();
    environment.configure_mock_model(server.base_url());
    let output_patch = environment
        .workspace()
        .parent()
        .expect("workspace parent")
        .join("result.patch");
    let output_target = output_patch.to_string_lossy().into_owned();
    let output = environment
        .std_command()
        .args([
            "exec",
            "exercise successful patch settlement",
            "--output-format",
            "stream-json",
            "--output-patch",
            &output_target,
        ])
        .output()
        .expect("run stream-json patch success contract");

    let stdout = stdout(&output);
    assert!(output.status.success(), "{}\n{stdout}", stderr(&output));
    assert_eq!(
        std::fs::read_to_string(&output_patch).expect("read generated patch"),
        "",
        "clean workspace must produce an explicit empty patch"
    );
    let events = jsonl_events(&stdout);
    assert!(
        events.iter().any(|value| {
            value["event"]["type"] == "TextChunk"
                && value["event"]["text"]
                    .as_str()
                    .is_some_and(|text| text.contains(STREAM_COMPLETED_MARKER))
        }),
        "model stream did not complete: {stdout}"
    );
    assert_eq!(
        events
            .iter()
            .filter(|value| is_terminal_event(value))
            .count(),
        1,
        "success must emit exactly one terminal envelope: {stdout}"
    );
    let final_event = events.last().expect("stream-json success terminal event");
    assert_eq!(
        final_event["event"]["type"], "DialogTurnCompleted",
        "success terminal must be the final envelope: {stdout}"
    );
}
