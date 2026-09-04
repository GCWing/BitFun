#![cfg(feature = "markdown")]

use openbitfun_services_core::markdown::{
    expand_prompt_template_arguments, expand_prompt_template_arguments_with_names,
    parse_prompt_shell_directives, prompt_template_expansion_upper_bound, FrontMatterMarkdown,
};
use std::fs;

#[test]
fn front_matter_markdown_preserves_metadata_and_trimmed_body_contract() {
    let content = "---\ntitle: Demo\ntags:\n  - one\n---\n\n# Body\n";

    let (metadata, body) = FrontMatterMarkdown::load_str(content).expect("front matter");
    assert_eq!(metadata["title"].as_str(), Some("Demo"));
    assert_eq!(body, "# Body\n");

    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("doc.md");
    FrontMatterMarkdown::save(path.to_str().expect("utf8 path"), &metadata, "  # Saved\n")
        .expect("save");
    let saved = fs::read_to_string(path).expect("saved");
    assert!(saved.starts_with("---\n"));
    assert!(saved.contains("title: Demo\n"));
    assert!(saved.contains("tags:\n- one\n"));
    assert!(saved.ends_with("---\n\n# Saved\n"));
}

#[test]
fn prompt_arguments_expand_full_and_zero_based_quoted_positions() {
    let expanded = expand_prompt_template_arguments(
        "Full: $ARGUMENTS\nFirst: $0\nSecond: $ARGUMENTS[1]\nThird: $2",
        "alpha \"two words\" 'three words'",
    );

    assert_eq!(
        expanded,
        "Full: alpha \"two words\" 'three words'\nFirst: alpha\nSecond: two words\nThird: three words"
    );
}

#[test]
fn prompt_expansion_upper_bound_is_checked_without_rendering_the_result() {
    let template = "$ARGUMENTS".repeat(1024);
    let bound = prompt_template_expansion_upper_bound(&template, &"x".repeat(2048)).unwrap();

    assert!(bound > 1024 * 1024);
}

#[test]
fn prompt_arguments_preserve_missing_and_escaped_placeholders() {
    let expanded = expand_prompt_template_arguments(
        r"Use $0, keep $ARGUMENTS[3], and show \$ARGUMENTS plus \$1",
        "alpha beta",
    );

    assert_eq!(
        expanded,
        "Use alpha, keep $ARGUMENTS[3], and show $ARGUMENTS plus $1"
    );

    assert_eq!(
        expand_prompt_template_arguments(r"Keep \\$0 expandable", "alpha"),
        r"Keep \\alpha expandable"
    );
    assert_eq!(
        expand_prompt_template_arguments(
            "Keep $ARGUMENTS[999999999999999999999999999999999999]",
            "alpha",
        ),
        "Keep $ARGUMENTS[999999999999999999999999999999999999]"
    );
}

#[test]
fn prompt_arguments_append_a_fallback_section_only_without_placeholders() {
    assert_eq!(
        expand_prompt_template_arguments("Review this change", "focus on auth"),
        "Review this change\n\nARGUMENTS: focus on auth"
    );
    assert_eq!(
        expand_prompt_template_arguments(r"Show \$ARGUMENTS", "literally"),
        "Show $ARGUMENTS\n\nARGUMENTS: literally"
    );
    assert_eq!(
        expand_prompt_template_arguments("Review this change", "   "),
        "Review this change"
    );
}

#[test]
fn prompt_arguments_preserve_backslashes_before_non_placeholders() {
    assert_eq!(
        expand_prompt_template_arguments(
            r"Keep \$HOME, \$ARGUMENTS[foo], and \${CLAUDE_SESSION_ID}",
            "alpha",
        ),
        "Keep \\$HOME, \\$ARGUMENTS[foo], and \\${CLAUDE_SESSION_ID}\n\nARGUMENTS: alpha"
    );
}

#[test]
fn prompt_arguments_expand_declared_names_from_the_existing_argument_list() {
    assert_eq!(
        expand_prompt_template_arguments_with_names(
            "Deploy $service to $environment; positional $0 / $1.",
            "api \"staging west\"",
            &["service".to_string(), "environment".to_string()],
        ),
        "Deploy api to staging west; positional api / staging west."
    );
}

#[test]
fn prompt_arguments_expand_missing_names_to_empty_and_preserve_unknown_names() {
    assert_eq!(
        expand_prompt_template_arguments_with_names(
            "Known: <$target>; missing: <$focus>; unknown: <$owner>.",
            "src/lib.rs",
            &["target".to_string(), "focus".to_string()],
        ),
        "Known: <src/lib.rs>; missing: <>; unknown: <$owner>."
    );
}

#[test]
fn prompt_arguments_preserve_escaped_declared_names() {
    assert_eq!(
        expand_prompt_template_arguments_with_names(
            r"Use $target and show \$target; keep \\$target expandable.",
            "src/lib.rs",
            &["target".to_string()],
        ),
        r"Use src/lib.rs and show $target; keep \\src/lib.rs expandable."
    );
}

#[test]
fn existing_prompt_argument_api_remains_compatible_without_declared_names() {
    assert_eq!(
        expand_prompt_template_arguments("Keep $target and use $0", "alpha"),
        "Keep $target and use alpha"
    );
}

#[test]
fn shell_directives_keep_expanded_ranges_and_static_review_scope() {
    let template = "Before !`git diff -- $1` after !`git status`";
    let expanded = "Before !`git diff -- src/lib.rs` after !`git status`";

    let parsed = parse_prompt_shell_directives(template, expanded).unwrap();

    assert_eq!(parsed.directives.len(), 2);
    assert_eq!(parsed.directives[0].command, "git diff -- src/lib.rs");
    assert!(!parsed.directives[0].can_remember);
    assert!(parsed.directives[1].can_remember);
    assert_eq!(
        &parsed.content[parsed.directives[1].range.clone()],
        "!`git status`"
    );
    assert!(!parsed.template_without_directives.contains("git status"));
}

#[test]
fn shell_directives_fail_closed_when_argument_expansion_changes_structure() {
    let error = parse_prompt_shell_directives(
        "Inspect !`printf $ARGUMENTS`",
        "Inspect !`printf injected`backtick`",
    )
    .unwrap_err();

    assert!(error.contains("structure changed"));
}

#[test]
fn prompt_only_commands_accept_backticks_from_arguments() {
    let parsed = parse_prompt_shell_directives(
        "Explain $ARGUMENTS",
        "Explain `inline code` without running it",
    )
    .unwrap();

    assert!(parsed.directives.is_empty());
    assert_eq!(parsed.content, "Explain `inline code` without running it");
}

#[test]
fn empty_shell_markers_are_not_executable_directives() {
    let parsed = parse_prompt_shell_directives("Keep !`` literal", "Keep !`` literal").unwrap();

    assert!(parsed.directives.is_empty());
}
