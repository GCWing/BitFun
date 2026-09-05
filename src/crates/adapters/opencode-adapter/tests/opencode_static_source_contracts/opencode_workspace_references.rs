use openbitfun_opencode_adapter::{
    OpenCodeCommandProviderOptions, OpenCodeWorkspaceReferenceProvider,
    OpenCodeWorkspaceReferenceProviderOptions,
};
use openbitfun_product_domains::external_sources::{ExecutionDomainId, ExternalSourceContext};
use openbitfun_product_domains::workspace_references::ExternalWorkspaceReferenceSourceProvider;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    home: PathBuf,
    user_config: PathBuf,
    project: PathBuf,
    opened: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let home = temp.path().join("home");
        let user_config = home.join(".config/opencode");
        let project = temp.path().join("project");
        let opened = project.join("packages/app");
        fs::create_dir_all(project.join(".git")).unwrap();
        fs::create_dir_all(&opened).unwrap();
        fs::create_dir_all(&user_config).unwrap();
        Self {
            _temp: temp,
            home,
            user_config,
            project,
            opened,
        }
    }

    fn provider(&self) -> OpenCodeWorkspaceReferenceProvider {
        self.provider_with_global_config_dir(self.user_config.clone())
    }

    fn provider_with_global_config_dir(
        &self,
        global_config_dir: PathBuf,
    ) -> OpenCodeWorkspaceReferenceProvider {
        OpenCodeWorkspaceReferenceProvider::new(OpenCodeWorkspaceReferenceProviderOptions {
            config: OpenCodeCommandProviderOptions {
                user_config_dir: global_config_dir,
                legacy_user_config_dir: None,
                explicit_config_file: None,
                explicit_config_dir: None,
                inline_config_content: None,
                project_config_enabled: true,
            },
            home_dir: Some(self.home.clone()),
        })
    }

    fn context(&self) -> ExternalSourceContext {
        ExternalSourceContext {
            workspace_root: Some(self.opened.clone()),
            execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
        }
    }

    fn write_config(&self, path: impl AsRef<Path>, body: &str) {
        let path = path.as_ref();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
}

#[test]
fn nearest_project_reference_layer_wins_without_dropping_other_aliases() {
    let fixture = Fixture::new();
    let user_dir = fixture.home.join("user-docs");
    let outer_dir = fixture.project.join("outer-docs");
    let inner_dir = fixture.opened.join("inner-docs");
    let shared_dir = fixture.project.join("shared-specs");
    for path in [&user_dir, &outer_dir, &inner_dir, &shared_dir] {
        fs::create_dir_all(path).unwrap();
    }

    fixture.write_config(
        fixture.user_config.join("opencode.json"),
        r#"{"references":{"docs":{"path":"../../../user-docs","description":"user"}}}"#,
    );
    fixture.write_config(
        fixture.project.join("opencode.json"),
        r#"{"references":{"docs":{"path":"./outer-docs","description":"outer"},"specs":{"path":"./shared-specs"}}}"#,
    );
    fixture.write_config(
        fixture.opened.join(".opencode/opencode.jsonc"),
        r#"{
          // the closest .opencode layer has the highest priority
          "references": {"docs": {"path": "../inner-docs", "description": "inner", "hidden": true}},
        }"#,
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();
    snapshot.validate().unwrap();

    assert_eq!(snapshot.references.len(), 2);
    let docs = snapshot
        .references
        .iter()
        .find(|reference| reference.alias == "docs")
        .unwrap();
    assert_eq!(docs.path, dunce::canonicalize(inner_dir).unwrap());
    assert_eq!(docs.description.as_deref(), Some("inner"));
    assert!(docs.hidden);

    let specs = snapshot
        .references
        .iter()
        .find(|reference| reference.alias == "specs")
        .unwrap();
    assert_eq!(specs.path, dunce::canonicalize(shared_dir).unwrap());
}

#[test]
fn keeps_alias_identity_and_reports_invalid_or_git_entries_without_materializing_them() {
    let fixture = Fixture::new();
    let shared = fixture.project.join("shared");
    fs::create_dir_all(&shared).unwrap();
    fixture.write_config(
        fixture.opened.join("opencode.json"),
        r#"{
          "references": {
            "docs": {"path": "../../shared"},
            "specs": {"path": "../../shared"},
            "bad alias": {"path": "../../shared"},
            "remote": {"repository": "https://example.test/docs.git", "branch": "main"},
            "bare": "github.com/example/docs"
          }
        }"#,
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert_eq!(
        snapshot
            .references
            .iter()
            .map(|reference| reference.alias.as_str())
            .collect::<Vec<_>>(),
        vec!["docs", "specs"]
    );
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.reference.alias_invalid"));
    assert_eq!(
        snapshot
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "opencode.reference.git_unsupported")
            .count(),
        2
    );
}

#[test]
fn legacy_reference_field_and_home_relative_paths_follow_opencode_migration_rules() {
    let fixture = Fixture::new();
    let handbook = fixture.home.join("handbook");
    fs::create_dir_all(&handbook).unwrap();
    fixture.write_config(
        fixture.opened.join("opencode.json"),
        r#"{"reference":{"handbook":{"path":"~/handbook","description":"Team handbook"}}}"#,
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert_eq!(snapshot.references.len(), 1);
    assert_eq!(snapshot.references[0].alias, "handbook");
    assert_eq!(
        snapshot.references[0].path,
        dunce::canonicalize(handbook).unwrap()
    );
}

#[test]
fn missing_local_directories_remain_declarative_and_config_roots_are_watched() {
    let fixture = Fixture::new();
    fixture.write_config(
        fixture.opened.join("opencode.json"),
        r#"{"references":{"missing":{"path":"./missing"}}}"#,
    );
    let provider = fixture.provider();
    let context = fixture.context();

    let snapshot = provider.discover(&context).unwrap();
    let roots = provider.watch_roots(&context);

    assert_eq!(snapshot.references.len(), 1);
    assert_eq!(snapshot.references[0].path, fixture.opened.join("missing"));
    assert!(roots
        .iter()
        .any(|root| root.path == fixture.opened.join(".opencode") && root.recursive));
    assert!(roots
        .iter()
        .any(|root| root.path == fixture.user_config && root.recursive));
}

#[test]
fn explicit_global_directory_replaces_the_default_global_root() {
    let fixture = Fixture::new();
    let explicit_global = fixture.home.join("alternate-opencode");
    fixture.write_config(
        fixture.user_config.join("opencode.json"),
        r#"{"references":{"default-only":{"path":"./default"}}}"#,
    );
    fixture.write_config(
        explicit_global.join("opencode.json"),
        r#"{"references":{"explicit-only":{"path":"./explicit"}}}"#,
    );

    let snapshot = fixture
        .provider_with_global_config_dir(explicit_global)
        .discover(&fixture.context())
        .unwrap();

    assert!(snapshot
        .references
        .iter()
        .any(|reference| reference.alias == "explicit-only"));
    assert!(!snapshot
        .references
        .iter()
        .any(|reference| reference.alias == "default-only"));
}

#[test]
fn relative_global_configuration_roots_fail_closed() {
    let fixture = Fixture::new();
    let provider = fixture.provider_with_global_config_dir(PathBuf::from("relative-opencode"));

    let error = provider.discover(&fixture.context()).unwrap_err();

    assert_eq!(error.code, "opencode.reference.global_config_invalid");
    assert!(!error.transient);
    assert!(provider.watch_roots(&fixture.context()).is_empty());
}

#[test]
fn bounded_catalog_retains_a_new_alias_from_the_highest_priority_layer() {
    let fixture = Fixture::new();
    let references = (0..1024)
        .map(|index| {
            (
                format!("low-{index:04}"),
                serde_json::json!({ "path": format!("./missing-{index:04}") }),
            )
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();
    fixture.write_config(
        fixture.user_config.join("opencode.json"),
        &serde_json::json!({ "references": references }).to_string(),
    );
    fixture.write_config(
        fixture.opened.join(".opencode/opencode.json"),
        r#"{"references":{"highest":{"path":"./future"}}}"#,
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert_eq!(snapshot.references.len(), 1024);
    assert!(snapshot
        .references
        .iter()
        .any(|reference| reference.alias == "highest"));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.reference.limit"));
}

#[test]
fn inline_references_override_file_layers_and_resolve_from_the_opened_workspace() {
    let fixture = Fixture::new();
    let file_target = fixture.project.join("file-target");
    let inline_target = fixture.project.join("inline-target");
    fixture.write_config(
        fixture.project.join("opencode.json"),
        r#"{"references":{"docs":{"path":"./file-target"}}}"#,
    );
    let provider =
        OpenCodeWorkspaceReferenceProvider::new(OpenCodeWorkspaceReferenceProviderOptions {
            config: OpenCodeCommandProviderOptions {
                user_config_dir: fixture.user_config.clone(),
                legacy_user_config_dir: None,
                explicit_config_file: None,
                explicit_config_dir: None,
                inline_config_content: Some(
                    r#"{"references":{"docs":{"path":"../../inline-target"}}}"#.to_string(),
                ),
                project_config_enabled: true,
            },
            home_dir: Some(fixture.home.clone()),
        });

    let snapshot = provider.discover(&fixture.context()).unwrap();

    let docs = snapshot
        .references
        .iter()
        .find(|reference| reference.alias == "docs")
        .expect("inline reference");
    assert_eq!(docs.path, inline_target);
    assert_ne!(docs.path, file_target);
    assert!(snapshot.sources.iter().any(|source| {
        source.location == "OPENCODE_CONFIG_CONTENT"
            && source.scope
                == openbitfun_product_domains::external_sources::ExternalSourceScope::WorkspaceLocal
    }));
}

#[test]
fn unsupported_higher_priority_git_reference_does_not_reveal_a_lower_local_alias() {
    let fixture = Fixture::new();
    let local_docs = fixture.home.join("local-docs");
    fs::create_dir_all(&local_docs).unwrap();
    fixture.write_config(
        fixture.user_config.join("opencode.json"),
        r#"{"references":{"docs":{"path":"../../../local-docs"}}}"#,
    );
    fixture.write_config(
        fixture.opened.join(".opencode/opencode.json"),
        r#"{"references":{"docs":{"repository":"https://example.test/docs.git"}}}"#,
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert!(snapshot.references.is_empty());
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "opencode.reference.git_unsupported" }));
}

#[test]
fn invalid_reference_diagnostics_are_bounded_and_summarized() {
    let fixture = Fixture::new();
    let references = (0..400)
        .map(|index| {
            (
                format!("invalid alias {index:04}"),
                serde_json::json!({ "path": "./future" }),
            )
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();
    fixture.write_config(
        fixture.opened.join("opencode.json"),
        &serde_json::json!({ "references": references }).to_string(),
    );

    let snapshot = fixture.provider().discover(&fixture.context()).unwrap();

    assert_eq!(snapshot.diagnostics.len(), 256);
    assert_eq!(snapshot.sources[0].diagnostics.len(), 255);
    assert_eq!(
        snapshot.sources[0].health,
        openbitfun_product_domains::external_sources::ExternalSourceHealth::Partial
    );
    assert_eq!(
        snapshot.diagnostics.last().unwrap().code,
        "opencode.reference.diagnostic_limit"
    );
}
