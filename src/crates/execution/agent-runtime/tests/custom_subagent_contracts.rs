use bitfun_agent_runtime::custom_subagent::{
    custom_subagent_model_or_default, custom_subagent_model_should_save,
    custom_subagent_read_markdown_file, custom_subagent_read_markdown_str,
    custom_subagent_readonly_or_default, custom_subagent_readonly_should_save,
    custom_subagent_review_or_default, custom_subagent_review_should_save,
    custom_subagent_save_markdown_file, custom_subagent_tools_are_default,
    custom_subagent_tools_from_front_matter, custom_subagent_tools_to_front_matter,
    CustomSubagentDefinition, CustomSubagentDefinitionError, CustomSubagentKind,
    DEFAULT_CUSTOM_SUBAGENT_MODEL, DEFAULT_CUSTOM_SUBAGENT_READONLY,
    DEFAULT_CUSTOM_SUBAGENT_REVIEW,
};
use std::fs;
use std::path::PathBuf;

#[test]
fn custom_subagent_defaults_match_existing_front_matter_contract() {
    assert_eq!(
        custom_subagent_tools_from_front_matter(None),
        ["LS", "Read", "Glob", "Grep"]
    );
    assert_eq!(
        custom_subagent_readonly_or_default(None),
        DEFAULT_CUSTOM_SUBAGENT_READONLY
    );
    assert_eq!(
        custom_subagent_review_or_default(None),
        DEFAULT_CUSTOM_SUBAGENT_REVIEW
    );
    assert_eq!(
        custom_subagent_model_or_default(None),
        DEFAULT_CUSTOM_SUBAGENT_MODEL
    );
}

#[test]
fn custom_subagent_tool_front_matter_keeps_existing_comma_format() {
    let tools = custom_subagent_tools_from_front_matter(Some("Read, Grep, Bash"));
    assert_eq!(tools, ["Read", "Grep", "Bash"]);
    assert_eq!(
        custom_subagent_tools_to_front_matter(&tools),
        Some("Read, Grep, Bash".to_string())
    );
}

#[test]
fn custom_subagent_default_fields_are_omitted_when_saved() {
    let default_tools = custom_subagent_tools_from_front_matter(None);
    assert!(custom_subagent_tools_are_default(&default_tools));
    assert_eq!(custom_subagent_tools_to_front_matter(&default_tools), None);
    assert!(!custom_subagent_readonly_should_save(
        DEFAULT_CUSTOM_SUBAGENT_READONLY
    ));
    assert!(custom_subagent_readonly_should_save(false));
    assert!(!custom_subagent_review_should_save(
        DEFAULT_CUSTOM_SUBAGENT_REVIEW
    ));
    assert!(custom_subagent_review_should_save(true));
    assert!(!custom_subagent_model_should_save(
        DEFAULT_CUSTOM_SUBAGENT_MODEL
    ));
    assert!(custom_subagent_model_should_save("deepseek-reasoner"));
}

#[test]
fn custom_subagent_kind_remains_project_or_user() {
    assert_eq!(CustomSubagentKind::Project, CustomSubagentKind::Project);
    assert_eq!(CustomSubagentKind::User, CustomSubagentKind::User);
}

#[test]
fn custom_subagent_definition_from_front_matter_preserves_schema_and_defaults() {
    let definition = CustomSubagentDefinition::from_front_matter_fields(
        Some("ReviewExtra"),
        Some("Additional code reviewer"),
        None,
        None,
        Some(true),
        Some("deepseek-reasoner"),
        "Review the selected files.".to_string(),
        CustomSubagentKind::User,
    )
    .expect("front matter fields should build a definition");

    assert_eq!(definition.name, "ReviewExtra");
    assert_eq!(definition.description, "Additional code reviewer");
    assert_eq!(definition.tools, ["LS", "Read", "Glob", "Grep"]);
    assert!(definition.readonly);
    assert!(definition.review);
    assert_eq!(definition.kind, CustomSubagentKind::User);
    assert_eq!(definition.model, "deepseek-reasoner");
    assert_eq!(definition.tools_front_matter(), None);
    assert!(!definition.should_save_readonly());
    assert!(definition.should_save_review());
    assert!(definition.should_save_model());
}

#[test]
fn custom_subagent_definition_reports_legacy_missing_field_errors() {
    let missing_name = CustomSubagentDefinition::from_front_matter_fields(
        None,
        Some("Additional code reviewer"),
        None,
        None,
        None,
        None,
        "Review the selected files.".to_string(),
        CustomSubagentKind::Project,
    )
    .expect_err("missing name should fail");
    assert_eq!(missing_name, CustomSubagentDefinitionError::MissingName);
    assert_eq!(missing_name.message(), "Missing name field");

    let missing_description = CustomSubagentDefinition::from_front_matter_fields(
        Some("ReviewExtra"),
        None,
        None,
        None,
        None,
        None,
        "Review the selected files.".to_string(),
        CustomSubagentKind::Project,
    )
    .expect_err("missing description should fail");
    assert_eq!(
        missing_description,
        CustomSubagentDefinitionError::MissingDescription
    );
    assert_eq!(missing_description.message(), "Missing description field");
}

#[test]
fn custom_subagent_markdown_io_preserves_legacy_front_matter_shape() {
    let dir = TestTempDir::new("bitfun-agent-runtime-subagent");
    let path = dir.join("reviewer.md");
    let definition = CustomSubagentDefinition::from_front_matter_fields(
        Some("Reviewer"),
        Some("Review changed code"),
        Some("Read, Grep"),
        Some(false),
        Some(true),
        Some("deepseek-reasoner"),
        "Review the selected files.".to_string(),
        CustomSubagentKind::Project,
    )
    .expect("definition should be valid");

    custom_subagent_save_markdown_file(&path, &definition).expect("definition should save");

    let saved = fs::read_to_string(&path).expect("saved file should be readable");
    assert!(saved.starts_with("---\n"));
    assert!(saved.contains("name: Reviewer"));
    assert!(saved.contains("description: Review changed code"));
    assert!(saved.contains("tools: Read, Grep"));
    assert!(saved.contains("readonly: false"));
    assert!(saved.contains("review: true"));
    assert!(saved.contains("model: deepseek-reasoner"));
    assert!(saved.ends_with("Review the selected files."));

    let loaded = custom_subagent_read_markdown_file(&path, CustomSubagentKind::Project)
        .expect("saved definition should load");
    assert_eq!(loaded, definition);
}

#[test]
fn custom_subagent_markdown_parse_errors_match_legacy_prefixes() {
    let missing_front_matter =
        custom_subagent_read_markdown_str("No front matter", CustomSubagentKind::User)
            .expect_err("missing front matter should fail");
    assert_eq!(missing_front_matter, "Failed to capture content");
}

struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{prefix}-{}", unique_suffix()));
        fs::create_dir_all(&path).expect("temp dir should be created");
        Self { path }
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn unique_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after UNIX epoch")
        .as_nanos()
        .to_string()
}
