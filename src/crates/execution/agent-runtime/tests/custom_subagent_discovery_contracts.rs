use bitfun_agent_runtime::custom_subagent::{
    custom_subagent_possible_dirs, custom_subagent_save_markdown_file,
    load_custom_subagent_definitions, CustomSubagentDefinition, CustomSubagentDiscoveryRoots,
    CustomSubagentKind,
};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn custom_subagent_discovery_preserves_directory_priority_and_deduplication() {
    let workspace = TestTempDir::new("bitfun-runtime-subagent-workspace");
    let bitfun_user = TestTempDir::new("bitfun-runtime-subagent-user");
    let home = TestTempDir::new("bitfun-runtime-subagent-home");

    let project_bitfun = workspace.path.join(".bitfun").join("agents");
    let project_claude = workspace.path.join(".claude").join("agents");
    let user_bitfun = bitfun_user.path.join("agents");
    let home_claude = home.path.join(".claude").join("agents");
    fs::create_dir_all(&project_bitfun).expect("project bitfun agents dir should be created");
    fs::create_dir_all(&project_claude).expect("project claude agents dir should be created");
    fs::create_dir_all(&user_bitfun).expect("user bitfun agents dir should be created");
    fs::create_dir_all(&home_claude).expect("home claude agents dir should be created");

    write_agent(
        &project_bitfun.join("shared.md"),
        "Shared",
        "Project BitFun agent",
        CustomSubagentKind::Project,
    );
    write_agent(
        &project_claude.join("shared.md"),
        "Shared",
        "Project Claude duplicate",
        CustomSubagentKind::Project,
    );
    write_agent(
        &user_bitfun.join("user-only.md"),
        "UserOnly",
        "BitFun user agent",
        CustomSubagentKind::User,
    );
    write_agent(
        &home_claude.join("home-only.md"),
        "HomeOnly",
        "Claude user agent",
        CustomSubagentKind::User,
    );
    fs::write(project_bitfun.join("ignored.txt"), "ignored")
        .expect("ignored text file should be written");
    fs::create_dir_all(project_bitfun.join("nested")).expect("nested dir should be created");
    write_agent(
        &project_bitfun.join("nested").join("nested.md"),
        "Nested",
        "Nested project agent",
        CustomSubagentKind::Project,
    );

    let roots = CustomSubagentDiscoveryRoots {
        workspace_root: workspace.path.clone(),
        bitfun_user_agents_dir: Some(user_bitfun.clone()),
        home_dir: Some(home.path.clone()),
    };

    let dirs = custom_subagent_possible_dirs(&roots);
    assert_eq!(
        dirs.iter()
            .map(|entry| entry.path.as_path())
            .collect::<Vec<_>>(),
        vec![
            project_bitfun.as_path(),
            project_claude.as_path(),
            user_bitfun.as_path(),
            home_claude.as_path(),
        ]
    );
    assert_eq!(
        dirs.iter().map(|entry| entry.kind).collect::<Vec<_>>(),
        vec![
            CustomSubagentKind::Project,
            CustomSubagentKind::Project,
            CustomSubagentKind::User,
            CustomSubagentKind::User,
        ]
    );

    let report = load_custom_subagent_definitions(&roots);
    assert!(report.errors.is_empty());
    assert_eq!(
        report
            .definitions
            .iter()
            .map(|loaded| loaded.definition.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Shared", "UserOnly", "HomeOnly"]
    );
    assert_eq!(
        report.definitions[0].definition.description,
        "Project BitFun agent"
    );
    assert_eq!(report.definitions[0].path, project_bitfun.join("shared.md"));
}

#[test]
fn custom_subagent_discovery_reports_parse_errors_without_dropping_valid_files() {
    let workspace = TestTempDir::new("bitfun-runtime-subagent-invalid");
    let project_bitfun = workspace.path.join(".bitfun").join("agents");
    fs::create_dir_all(&project_bitfun).expect("project agents dir should be created");
    let broken_path = project_bitfun.join("broken.md");
    fs::write(&broken_path, "No front matter").expect("broken markdown file should be written");
    write_agent(
        &project_bitfun.join("valid.md"),
        "Valid",
        "Valid project agent",
        CustomSubagentKind::Project,
    );

    let roots = CustomSubagentDiscoveryRoots {
        workspace_root: workspace.path.clone(),
        bitfun_user_agents_dir: None,
        home_dir: None,
    };

    let report = load_custom_subagent_definitions(&roots);
    assert_eq!(report.definitions.len(), 1);
    assert_eq!(report.definitions[0].definition.name, "Valid");
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].path, broken_path);
    assert_eq!(
        report.errors[0].error,
        "Failed to parse markdown file: Failed to capture content"
    );
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
}

impl Drop for TestTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_agent(path: &Path, name: &str, description: &str, kind: CustomSubagentKind) {
    let definition = CustomSubagentDefinition::from_front_matter_fields(
        Some(name),
        Some(description),
        None,
        None,
        None,
        None,
        format!("{name} prompt."),
        kind,
    )
    .expect("custom subagent definition should be valid");
    custom_subagent_save_markdown_file(path, &definition)
        .expect("custom subagent markdown should save");
}

fn unique_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after UNIX epoch")
        .as_nanos()
        .to_string()
}
