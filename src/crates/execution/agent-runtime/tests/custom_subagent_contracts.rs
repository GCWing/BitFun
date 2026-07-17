use bitfun_agent_runtime::custom_agent::{
    CustomAgentKind, DEFAULT_CUSTOM_SUBAGENT_MODEL, DEFAULT_CUSTOM_SUBAGENT_READONLY,
    DEFAULT_CUSTOM_SUBAGENT_REVIEW,
};
use bitfun_agent_runtime::custom_subagent::{
    custom_subagent_model_or_default, custom_subagent_model_should_save,
    custom_subagent_read_markdown_file, custom_subagent_read_markdown_str,
    custom_subagent_readonly_or_default, custom_subagent_readonly_should_save,
    custom_subagent_review_or_default, custom_subagent_review_should_save,
    custom_subagent_save_markdown_file, custom_subagent_tools_are_default,
    custom_subagent_tools_from_front_matter, custom_subagent_tools_to_front_matter,
    CustomSubagentDefinition, CustomSubagentDefinitionError, CustomSubagentKind,
};
use std::fs;
use std::path::PathBuf;

struct BuildDefinitionInput<'a>(
    Option<&'a str>,
    Option<&'a str>,
    Option<&'a str>,
    Option<Vec<String>>,
    Option<bool>,
    Option<bool>,
    Option<&'a str>,
    CustomSubagentKind,
);

fn build_definition(
    input: BuildDefinitionInput<'_>,
) -> Result<CustomSubagentDefinition, CustomSubagentDefinitionError> {
    let BuildDefinitionInput(id, name, description, tools, readonly, review, model, level) = input;
    CustomSubagentDefinition::from_front_matter_fields(
        id,
        name,
        description,
        Some(CustomAgentKind::Subagent),
        tools,
        readonly,
        review,
        model,
        None,
        "Review the selected files.".to_string(),
        level,
    )
    .map(|parsed| parsed.definition)
}

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
    let definition = build_definition(BuildDefinitionInput(
        Some("ReviewExtra"),
        Some("Additional code reviewer"),
        Some("Review agent for changed files"),
        None,
        None,
        Some(true),
        Some("deepseek-reasoner"),
        CustomSubagentKind::User,
    ))
    .expect("front matter fields should build a definition");

    assert_eq!(definition.id, "ReviewExtra");
    assert_eq!(definition.name, "Additional code reviewer");
    assert_eq!(definition.description, "Review agent for changed files");
    assert_eq!(definition.kind, CustomAgentKind::Subagent);
    assert_eq!(definition.level, CustomSubagentKind::User);
    assert_eq!(definition.tools, ["LS", "Read", "Glob", "Grep"]);
    assert!(definition.readonly);
    assert!(definition.review);
    assert_eq!(definition.model, "deepseek-reasoner");
    assert!(definition.tools_are_default());
    assert!(!definition.should_save_readonly());
    assert!(definition.should_save_review());
    assert!(definition.should_save_model());
}

#[test]
fn custom_subagent_model_presence_distinguishes_default_from_fast_override() {
    let implicit = build_definition(BuildDefinitionInput(
        Some("ImplicitModel"),
        Some("Implicit model"),
        Some("Uses the shared Subagent default"),
        None,
        None,
        None,
        None,
        CustomSubagentKind::User,
    ))
    .expect("definition without a model should parse");
    assert_eq!(implicit.model, "fast");
    assert!(!implicit.model_is_explicit);
    assert!(!implicit.should_save_model());

    let explicit_fast = build_definition(BuildDefinitionInput(
        Some("ExplicitFast"),
        Some("Explicit fast"),
        Some("Keeps a fast override"),
        None,
        None,
        None,
        Some("fast"),
        CustomSubagentKind::User,
    ))
    .expect("definition with fast should parse");
    assert!(explicit_fast.model_is_explicit);
    assert!(explicit_fast.should_save_model());

    let dir = TestTempDir::new("bitfun-agent-runtime-explicit-model");
    let implicit_path = dir.join("implicit.md");
    let explicit_path = dir.join("explicit.md");
    custom_subagent_save_markdown_file(&implicit_path, &implicit)
        .expect("implicit definition should save");
    custom_subagent_save_markdown_file(&explicit_path, &explicit_fast)
        .expect("explicit definition should save");

    let implicit_markdown =
        fs::read_to_string(&implicit_path).expect("implicit markdown should read");
    let explicit_markdown =
        fs::read_to_string(&explicit_path).expect("explicit markdown should read");
    assert!(!implicit_markdown.contains("model:"));
    assert!(explicit_markdown.contains("model: fast"));
}

#[test]
fn custom_subagent_definition_reports_legacy_missing_field_errors() {
    let missing_name = build_definition(BuildDefinitionInput(
        None,
        None,
        Some("Additional code reviewer"),
        None,
        None,
        None,
        None,
        CustomSubagentKind::Project,
    ))
    .expect_err("missing name should fail");
    assert_eq!(missing_name, CustomSubagentDefinitionError::MissingName);
    assert_eq!(missing_name.message(), "Missing name field");

    let missing_description = build_definition(BuildDefinitionInput(
        Some("ReviewExtra"),
        Some("Additional code reviewer"),
        None,
        None,
        None,
        None,
        None,
        CustomSubagentKind::Project,
    ))
    .expect_err("missing description should fail");
    assert_eq!(
        missing_description,
        CustomSubagentDefinitionError::MissingDescription
    );
    assert_eq!(missing_description.message(), "Missing description field");
}

#[test]
fn custom_subagent_markdown_io_writes_canonical_front_matter() {
    let dir = TestTempDir::new("bitfun-agent-runtime-subagent");
    let path = dir.join("reviewer.md");
    let definition = build_definition(BuildDefinitionInput(
        Some("Reviewer"),
        Some("Review changed code"),
        Some("Review changed files and report findings"),
        Some(vec!["Read".to_string(), "Grep".to_string()]),
        Some(false),
        Some(true),
        Some("deepseek-reasoner"),
        CustomSubagentKind::Project,
    ))
    .expect("definition should be valid");

    custom_subagent_save_markdown_file(&path, &definition).expect("definition should save");

    let saved = fs::read_to_string(&path).expect("saved file should be readable");
    assert!(saved.starts_with("---\n"));
    assert!(saved.contains("schema_version: 1"));
    assert!(saved.contains("kind: subagent"));
    assert!(saved.contains("id: Reviewer"));
    assert!(saved.contains("name: Review changed code"));
    assert!(saved.contains("description: Review changed files and report findings"));
    assert!(saved.contains("- Read"));
    assert!(saved.contains("- Grep"));
    assert!(saved.contains("review: true"));
    assert!(saved.contains("model: deepseek-reasoner"));
    assert!(saved.ends_with("Review the selected files."));

    let loaded = custom_subagent_read_markdown_file(&path, CustomSubagentKind::Project)
        .expect("saved definition should load");
    assert_eq!(loaded, definition);
    assert!(loaded.readonly, "review subagents must be readonly");
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
