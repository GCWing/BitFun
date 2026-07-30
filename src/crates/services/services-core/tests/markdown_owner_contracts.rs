use bitfun_services_core::markdown::{
    expand_prompt_template_arguments, expand_prompt_template_arguments_with_names,
    prompt_template_expansion_upper_bound, FrontMatterMarkdown,
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
