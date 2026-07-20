use std::process::{Command, Output};

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
