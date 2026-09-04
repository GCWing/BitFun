use openbitfun_opencode_adapter::{
    OpenCodeCommandProviderOptions, OpenCodeSkillRootProvider, OpenCodeSkillRootProviderOptions,
};
use openbitfun_product_domains::external_sources::ExternalSourceScope;
use std::fs;
use std::path::{Path, PathBuf};

struct Fixture {
    _temp: tempfile::TempDir,
    home: PathBuf,
    user_config: PathBuf,
    project: PathBuf,
    opened_directory: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let user_config = home.join(".config/opencode");
        let project = temp.path().join("project");
        let opened_directory = project.join("packages/app");
        fs::create_dir_all(&user_config).unwrap();
        fs::create_dir_all(project.join(".git")).unwrap();
        fs::create_dir_all(&opened_directory).unwrap();
        Self {
            _temp: temp,
            home,
            user_config,
            project,
            opened_directory,
        }
    }

    fn provider(&self) -> OpenCodeSkillRootProvider {
        OpenCodeSkillRootProvider::new(OpenCodeSkillRootProviderOptions {
            config: OpenCodeCommandProviderOptions {
                user_config_dir: self.user_config.clone(),
                legacy_user_config_dir: Some(self.home.join(".opencode")),
                explicit_config_file: None,
                explicit_config_dir: None,
                inline_config_content: None,
                project_config_enabled: true,
            },
            home_dir: Some(self.home.clone()),
        })
    }
}

fn write(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
fn accumulates_v1_and_current_local_skill_paths_in_config_source_order() {
    let fixture = Fixture::new();
    let user_skill = fixture.home.join("shared-skills");
    let project_skill = fixture.project.join("project-skills");
    let nested_skill = fixture.opened_directory.join("nested-skills");
    for path in [&user_skill, &project_skill, &nested_skill] {
        fs::create_dir_all(path).unwrap();
    }
    write(
        fixture.user_config.join("opencode.json"),
        r#"{"skills":{"paths":["~/shared-skills"],"urls":["https://example.test/skills"]}}"#,
    );
    write(
        fixture.project.join("opencode.json"),
        r#"{"skills":{"paths":["../../project-skills"]}}"#,
    );
    write(
        fixture.opened_directory.join("opencode.jsonc"),
        r#"{"skills":["nested-skills", "https://example.test/ignored"]}"#,
    );

    let roots = fixture.provider().discover(Some(&fixture.opened_directory));

    assert_eq!(roots.len(), 3);
    assert_eq!(roots[0].path, dunce::canonicalize(user_skill).unwrap());
    assert_eq!(roots[0].scope, ExternalSourceScope::UserGlobal);
    assert_eq!(roots[1].path, dunce::canonicalize(project_skill).unwrap());
    assert_eq!(roots[1].scope, ExternalSourceScope::Project);
    assert_eq!(roots[2].path, dunce::canonicalize(nested_skill).unwrap());
    assert_eq!(roots[2].scope, ExternalSourceScope::Project);
    assert!(roots
        .windows(2)
        .all(|pair| pair[0].precedence < pair[1].precedence));
}

#[test]
fn rejects_roots_outside_the_config_source_boundary() {
    let fixture = Fixture::new();
    let arbitrary = fixture._temp.path().join("arbitrary");
    let home_skill = fixture.home.join("allowed-home");
    let workspace_skill = fixture.project.join("allowed-project");
    for path in [&arbitrary, &home_skill, &workspace_skill] {
        fs::create_dir_all(path).unwrap();
    }
    write(
        fixture.user_config.join("opencode.json"),
        &format!(
            r#"{{"skills":{{"paths":["{}", "~/allowed-home"]}}}}"#,
            arbitrary.to_string_lossy().replace('\\', "\\\\")
        ),
    );
    write(
        fixture.project.join("opencode.json"),
        &format!(
            r#"{{"skills":{{"paths":["{}", "../../allowed-project"]}}}}"#,
            home_skill.to_string_lossy().replace('\\', "\\\\")
        ),
    );

    let roots = fixture.provider().discover(Some(&fixture.opened_directory));
    let canonical_home_skill = dunce::canonicalize(&home_skill).unwrap();
    let canonical_workspace_skill = dunce::canonicalize(&workspace_skill).unwrap();
    let canonical_arbitrary = dunce::canonicalize(&arbitrary).unwrap();

    assert_eq!(roots.len(), 2);
    assert!(roots.iter().any(|root| root.path == canonical_home_skill));
    assert!(roots
        .iter()
        .any(|root| root.path == canonical_workspace_skill));
    assert!(roots.iter().all(|root| root.path != canonical_arbitrary));
}

#[test]
fn deduplicates_canonical_roots_while_retaining_the_first_source_position() {
    let fixture = Fixture::new();
    let root = fixture.project.join("shared");
    fs::create_dir_all(&root).unwrap();
    write(
        fixture.project.join("opencode.json"),
        r#"{"skills":{"paths":["../../shared"]}}"#,
    );
    write(
        fixture.opened_directory.join("opencode.json"),
        r#"{"skills":{"paths":["../../shared"]}}"#,
    );

    let roots = fixture.provider().discover(Some(&fixture.opened_directory));

    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].path, dunce::canonicalize(root).unwrap());
    assert_eq!(roots[0].precedence, 0);
}

#[test]
fn caps_configured_roots_by_retaining_the_latest_entries() {
    let fixture = Fixture::new();
    let paths = (0..65)
        .map(|index| {
            fs::create_dir_all(fixture.project.join(format!("skills-{index}"))).unwrap();
            format!("../../skills-{index}")
        })
        .collect::<Vec<_>>();
    write(
        fixture.project.join("opencode.json"),
        &serde_json::json!({"skills": {"paths": paths}}).to_string(),
    );

    let roots = fixture.provider().discover(Some(&fixture.opened_directory));

    assert_eq!(roots.len(), 64);
    assert!(roots.iter().all(|root| !root.path.ends_with("skills-0")));
    assert!(roots.iter().any(|root| root.path.ends_with("skills-64")));
}

#[test]
fn rejects_a_malformed_skills_list_instead_of_partially_loading_it() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.project.join("valid-skills")).unwrap();
    write(
        fixture.project.join("opencode.json"),
        r#"{"skills":["valid-skills", 42]}"#,
    );

    let roots = fixture.provider().discover(Some(&fixture.opened_directory));

    assert!(roots.is_empty());
}

#[test]
fn no_workspace_never_interprets_a_relative_configured_root_locally() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.home.join("global-skills")).unwrap();
    fs::create_dir_all(fixture.project.join("relative-skills")).unwrap();
    write(
        fixture.user_config.join("opencode.json"),
        r#"{"skills":["relative-skills", "~/global-skills"]}"#,
    );

    let roots = fixture.provider().discover(None);

    assert_eq!(roots.len(), 1);
    assert_eq!(
        roots[0].path,
        dunce::canonicalize(fixture.home.join("global-skills")).unwrap()
    );
    assert_eq!(roots[0].scope, ExternalSourceScope::UserGlobal);
}

#[test]
fn inline_config_paths_are_applied_last_and_stay_workspace_scoped() {
    let fixture = Fixture::new();
    let file_root = fixture.project.join("file-skills");
    let inline_root = fixture.project.join("inline-skills");
    fs::create_dir_all(&file_root).unwrap();
    fs::create_dir_all(&inline_root).unwrap();
    write(
        fixture.project.join("opencode.json"),
        r#"{"skills":["../../file-skills"]}"#,
    );
    let provider = OpenCodeSkillRootProvider::new(OpenCodeSkillRootProviderOptions {
        config: OpenCodeCommandProviderOptions {
            user_config_dir: fixture.user_config.clone(),
            legacy_user_config_dir: Some(fixture.home.join(".opencode")),
            explicit_config_file: None,
            explicit_config_dir: None,
            inline_config_content: Some(r#"{"skills":["../../inline-skills"]}"#.to_string()),
            project_config_enabled: true,
        },
        home_dir: Some(fixture.home.clone()),
    });

    let roots = provider.discover(Some(&fixture.opened_directory));

    assert_eq!(roots.len(), 2);
    assert_eq!(roots[1].path, dunce::canonicalize(inline_root).unwrap());
    assert_eq!(roots[1].scope, ExternalSourceScope::Project);
}
