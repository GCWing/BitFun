use openbitfun_product_domains::external_hook_catalog::{
    ExternalHookHandlerKind, ExternalHookMatcherSummary,
};
use openbitfun_static_hook_support::{
    parse_hook_document, prepare_static_hook_command, read_bounded_file,
    redacted_parse_content_version, regular_file_exists, visit_hook_document, BoundedFileRead,
    StaticHookAssetError, StaticHookDocumentFormat, StaticHookHandlerRule, StaticHookParseIssue,
};
use std::collections::BTreeMap;
use std::fs;

const RULES: &[StaticHookHandlerRule] = &[
    StaticHookHandlerRule::new("command", ExternalHookHandlerKind::Command, &["command"]),
    StaticHookHandlerRule::new("prompt", ExternalHookHandlerKind::Prompt, &[]),
];

#[test]
fn one_document_walk_exposes_borrowed_handlers_without_retaining_bodies() {
    let mut commands = Vec::new();
    let summary = visit_hook_document(
        br#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"private-token"},{}]}]}}"#,
        StaticHookDocumentFormat::Json,
        16,
        |handler| {
            let Some(command) = handler
                .handler
                .as_object()
                .and_then(|value| value.get("command"))
                .and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            commands.push((
                handler.native_event.to_string(),
                handler.group_index,
                handler.handler_index,
                command.to_string(),
            ));
            true
        },
    );

    assert_eq!(
        commands,
        vec![("PreToolUse".to_string(), 0, 0, "private-token".to_string())]
    );
    assert_eq!(summary.inspected_handlers, 2);
    assert_eq!(summary.issues, vec![StaticHookParseIssue::HandlerInvalid]);
    assert!(!format!("{summary:?}").contains("private-token"));
}

#[test]
fn parses_json_to_redacted_handler_facts() {
    let parsed = parse_hook_document(
        br#"{"disableAllHooks":true,"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"secret --token abc"}]}]}}"#,
        StaticHookDocumentFormat::Json,
        RULES,
        16,
    );

    assert!(parsed.all_disabled);
    assert_eq!(parsed.handlers.len(), 1);
    assert_eq!(parsed.handlers[0].native_event, "PreToolUse");
    assert_eq!(
        parsed.handlers[0].handler_kind,
        ExternalHookHandlerKind::Command
    );
    assert!(!format!("{parsed:?}").contains("secret"));
    assert!(parsed.issues.is_empty());
}

#[test]
fn parses_toml_and_reports_invalid_handlers_without_losing_valid_ones() {
    let parsed = parse_hook_document(
        br#"
[[hooks.PostToolUse]]
matcher = "Edit"

[[hooks.PostToolUse.hooks]]
type = "http"
url = "https://secret.example"

[[hooks.PostToolUse.hooks]]
type = "prompt"
prompt = "private"
"#,
        StaticHookDocumentFormat::Toml,
        RULES,
        16,
    );

    assert_eq!(parsed.handlers.len(), 1);
    assert_eq!(
        parsed.handlers[0].handler_kind,
        ExternalHookHandlerKind::Prompt
    );
    assert_eq!(parsed.issues, vec![StaticHookParseIssue::HandlerInvalid]);
    assert!(!format!("{parsed:?}").contains("secret.example"));
    assert!(!format!("{parsed:?}").contains("private"));
}

#[test]
fn handler_limit_is_global_and_deterministic() {
    let parsed = parse_hook_document(
        br#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"prompt"},{"type":"prompt"}]}]}}"#,
        StaticHookDocumentFormat::Json,
        RULES,
        1,
    );
    assert_eq!(parsed.handlers.len(), 1);
    assert_eq!(parsed.issues, vec![StaticHookParseIssue::HandlerLimit]);
}

#[test]
fn invalid_handlers_also_consume_the_inspection_budget() {
    let parsed = parse_hook_document(
        br#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"unknown","value":"ignored"},{"type":"prompt"}]}]}}"#,
        StaticHookDocumentFormat::Json,
        RULES,
        1,
    );
    assert!(parsed.handlers.is_empty());
    assert_eq!(
        parsed.issues,
        vec![
            StaticHookParseIssue::HandlerInvalid,
            StaticHookParseIssue::HandlerLimit,
        ]
    );
}

#[test]
fn invalid_event_names_are_omitted_without_invalidating_other_events() {
    let long_event = "x".repeat(161);
    let input = format!(
        r#"{{"hooks":{{"{long_event}":[{{"hooks":[{{"type":"prompt"}}]}}],"PreToolUse":[{{"hooks":[{{"type":"prompt"}}]}}]}}}}"#,
    );
    let parsed = parse_hook_document(input.as_bytes(), StaticHookDocumentFormat::Json, RULES, 16);

    assert_eq!(parsed.handlers.len(), 1);
    assert_eq!(parsed.handlers[0].native_event, "PreToolUse");
    assert_eq!(parsed.issues, vec![StaticHookParseIssue::EventNameInvalid]);
}

#[test]
fn bounded_file_reads_never_return_content_past_the_limit() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("hooks.json");
    fs::write(&path, b"12345").unwrap();

    assert_eq!(
        read_bounded_file(&path, 4).unwrap(),
        BoundedFileRead::TooLarge
    );
    assert_eq!(
        read_bounded_file(&path, 5).unwrap(),
        BoundedFileRead::Content(b"12345".to_vec())
    );
}

#[test]
fn absent_files_are_distinct_from_existing_non_files() {
    let root = tempfile::tempdir().unwrap();
    assert!(!regular_file_exists(&root.path().join("missing.json")).unwrap());
    assert!(!regular_file_exists(root.path()).unwrap());
    let file = root.path().join("hooks.json");
    fs::write(&file, "{}").unwrap();
    assert!(regular_file_exists(&file).unwrap());
}

#[test]
fn redacted_versions_ignore_handler_bodies() {
    let parse = |command: &str| {
        parse_hook_document(
            format!(r#"{{"hooks":{{"PreToolUse":[{{"hooks":[{{"type":"command","command":"{command}"}}]}}]}}}}"#).as_bytes(),
            StaticHookDocumentFormat::Json,
            RULES,
            8,
        )
    };
    assert_eq!(
        redacted_parse_content_version(&parse("token-one")),
        redacted_parse_content_version(&parse("token-two"))
    );
}

#[test]
fn malformed_matchers_are_not_reported_as_matching_everything() {
    let parsed = parse_hook_document(
        br#"{"hooks":{"PreToolUse":[{"matcher":{"tool":"Bash"},"hooks":[{"type":"command","command":"safe"}]}]}}"#,
        StaticHookDocumentFormat::Json,
        &[StaticHookHandlerRule::new(
            "command",
            ExternalHookHandlerKind::Command,
            &["command"],
        )],
        8,
    );

    assert_eq!(parsed.handlers.len(), 1);
    assert_eq!(
        parsed.handlers[0].matcher,
        ExternalHookMatcherSummary::Unavailable
    );
}

#[test]
fn repeated_parse_failures_are_aggregated_by_issue_kind() {
    let parsed = parse_hook_document(
        br#"{"hooks":{"PreToolUse":[{"hooks":[{}, {}, {}]}]}}"#,
        StaticHookDocumentFormat::Json,
        RULES,
        8,
    );

    assert_eq!(
        parsed
            .issues
            .iter()
            .filter(|issue| **issue == StaticHookParseIssue::HandlerInvalid)
            .count(),
        1
    );
}

#[test]
fn static_hook_command_copies_only_referenced_assets_and_marks_external_paths() {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join(".claude");
    fs::create_dir_all(config_dir.join("hooks")).unwrap();
    fs::write(config_dir.join("hooks/check.py"), b"print('checked')").unwrap();
    let external = temp.path().join("external-tool");
    let external_text = external.to_string_lossy().replace('\\', "/");
    let mut assets = BTreeMap::new();

    let prepared = prepare_static_hook_command(
        &format!("python .claude/hooks/check.py {external_text}"),
        &config_dir,
        ".claude",
        &mut assets,
    )
    .unwrap();

    assert!(prepared
        .command
        .contains("__OPENBITFUN_MANAGED_HOOK_ROOT__/hooks/check.py"));
    assert_eq!(
        assets.get(std::path::Path::new("hooks/check.py")).unwrap(),
        b"print('checked')"
    );
    assert!(prepared.dependencies.iter().any(|dependency| matches!(
        dependency,
        openbitfun_product_domains::external_hook_import::ExternalHookImportDependencyV1::Managed { relative_path }
            if relative_path == "hooks/check.py"
    )));
    assert!(prepared.dependencies.iter().any(|dependency| matches!(
        dependency,
        openbitfun_product_domains::external_hook_import::ExternalHookImportDependencyV1::External { location }
            if location.replace('\\', "/") == external_text
    )));
}

#[test]
fn dynamic_source_hook_paths_fail_closed_without_mutating_assets() {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join(".claude");
    fs::create_dir_all(config_dir.join("hooks")).unwrap();
    let mut assets = BTreeMap::new();

    let error = prepare_static_hook_command(
        "python .claude/hooks/$SCRIPT.py",
        &config_dir,
        ".claude",
        &mut assets,
    )
    .unwrap_err();

    assert_eq!(error, StaticHookAssetError::DynamicPath);
    assert!(assets.is_empty());
}
