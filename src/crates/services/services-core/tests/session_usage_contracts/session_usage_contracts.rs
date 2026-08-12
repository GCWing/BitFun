use bitfun_services_core::session_usage::{
    classify_tool_usage, display_workspace_relative_path, render_usage_report_terminal,
    SessionUsageReport, UsageToolCategory,
};

#[test]
fn usage_classifier_preserves_git_command_detection() {
    let input = serde_json::json!({ "command": "git status --short" });

    assert_eq!(
        classify_tool_usage("execute_command", Some(&input)),
        UsageToolCategory::Git
    );
}

#[test]
fn usage_path_redaction_preserves_workspace_relative_display() {
    let label = display_workspace_relative_path(
        Some("D:/workspace/bitfun"),
        "D:/workspace/bitfun/src/main.rs",
    );

    assert_eq!(label.value, "src/main.rs");
    assert!(!label.redacted);
}

#[test]
fn usage_terminal_renderer_preserves_schema_label() {
    let report = SessionUsageReport::partial_unavailable("session-1".to_string(), 42);

    let rendered = render_usage_report_terminal(&report);

    assert!(rendered.contains("session-1"));
}
