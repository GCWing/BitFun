use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    user_config: PathBuf,
    project: PathBuf,
    opened_directory: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().expect("tempdir");
        let user_config = temp.path().join("user/opencode");
        let project = temp.path().join("workspace");
        let opened_directory = project.join("packages/app");
        fs::create_dir_all(project.join(".git")).expect("git boundary");
        fs::create_dir_all(&opened_directory).expect("opened directory");
        Self {
            _temp: temp,
            user_config,
            project,
            opened_directory,
        }
    }

    fn context(&self) -> ExternalSourceContext {
        ExternalSourceContext {
            workspace_root: Some(self.opened_directory.clone()),
            execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
        }
    }

    fn config(&self, explicit_config_dir: Option<PathBuf>) -> OpenCodeCommandProviderOptions {
        OpenCodeCommandProviderOptions {
            user_config_dir: self.user_config.clone(),
            legacy_user_config_dir: None,
            explicit_config_file: None,
            explicit_config_dir,
            inline_config_content: None,
            project_config_enabled: true,
        }
    }

    fn command_provider(&self, explicit_config_dir: Option<PathBuf>) -> OpenCodeCommandProvider {
        OpenCodeCommandProvider::new(self.config(explicit_config_dir))
    }

    fn subagent_provider(&self, explicit_config_dir: Option<PathBuf>) -> OpenCodeSubagentProvider {
        OpenCodeSubagentProvider::new(OpenCodeSubagentProviderOptions {
            config: self.config(explicit_config_dir),
            project_root_override: Some(self.project.clone()),
        })
    }

    fn mcp_provider(&self, explicit_config_dir: Option<PathBuf>) -> OpenCodeMcpProvider {
        OpenCodeMcpProvider::new(OpenCodeMcpProviderOptions {
            config: self.config(explicit_config_dir),
            project_root_override: Some(self.project.clone()),
        })
    }

    fn write_layer(&self, directory: &Path, value: &str) -> PathBuf {
        self.write_config(&directory.join(".opencode"), value)
    }

    fn write_config(&self, directory: &Path, value: &str) -> PathBuf {
        let path = directory.join("opencode.json");
        write(
            &path,
            &format!(
                r#"{{
                  "command": {{
                    "review": {{
                      "description": "{value} command",
                      "template": "{value} $ARGUMENTS"
                    }}
                  }},
                  "agent": {{
                    "review": {{
                      "description": "{value} agent",
                      "prompt": "{value} agent prompt",
                      "mode": "subagent"
                    }}
                  }},
                  "mcp": {{
                    "docs": {{
                      "type": "remote",
                      "url": "https://{value}.example.test/mcp"
                    }}
                  }}
                }}"#
            ),
        );
        path
    }

    fn control_plane(&self) -> ExternalSourceControlPlane {
        self.control_plane_with_explicit_dir(None)
    }

    fn control_plane_with_explicit_dir(
        &self,
        explicit_config_dir: Option<PathBuf>,
    ) -> ExternalSourceControlPlane {
        self.control_plane_with_config(self.config(explicit_config_dir))
    }

    fn control_plane_with_config(
        &self,
        config: OpenCodeCommandProviderOptions,
    ) -> ExternalSourceControlPlane {
        ExternalSourceControlPlane::new(
            self.context(),
            ExternalMcpRevisionKey::new([17; 32]),
            vec![Arc::new(OpenCodeCommandProvider::new(config.clone()))],
            Vec::new(),
            vec![Arc::new(OpenCodeSubagentProvider::new(
                OpenCodeSubagentProviderOptions {
                    config: config.clone(),
                    project_root_override: Some(self.project.clone()),
                },
            ))],
            vec![Arc::new(OpenCodeMcpProvider::new(
                OpenCodeMcpProviderOptions {
                    config,
                    project_root_override: Some(self.project.clone()),
                },
            ))],
            Vec::new(),
        )
        .expect("OpenCode control plane")
    }
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).expect("source parent");
    fs::write(path, contents).expect("source file");
}

fn refresh_all(plane: &ExternalSourceControlPlane) {
    plane.commands_mut(|coordinator| {
        coordinator.refresh();
    });
    plane.subagents_mut(|coordinator| {
        coordinator.refresh();
    });
    plane.mcp_mut(|coordinator| {
        coordinator.refresh();
    });
}

fn assert_winners(plane: &ExternalSourceControlPlane, expected: &str) {
    let expanded = plane
        .commands(|coordinator| coordinator.expand_command("review", "src/lib.rs"))
        .expect("expanded slash command");
    assert_eq!(expanded.content, format!("{expected} src/lib.rs"));

    plane.subagents(|coordinator| {
        let snapshot = coordinator.snapshot();
        let review = snapshot
            .definitions
            .iter()
            .find(|definition| definition.logical_id == "review")
            .expect("external review agent");
        assert_eq!(
            review.prompt.expose(),
            format!("{expected} agent prompt").as_str()
        );
    });

    plane.mcp(|coordinator| {
        let snapshot = coordinator.snapshot();
        let docs = snapshot
            .servers
            .iter()
            .find(|server| server.name == "docs")
            .expect("external docs MCP server");
        assert_eq!(
            docs.remote_url_preview.as_deref(),
            Some(format!("https://{expected}.example.test/").as_str())
        );
    });
}

fn source_stable_key_for_location(
    sources: &[ExternalSourceCatalogEntry],
    location: &Path,
) -> String {
    sources
        .iter()
        .find(|source| Path::new(&source.record.location) == location)
        .map(|source| source.stable_key.clone())
        .unwrap_or_else(|| panic!("source not found for {}", location.display()))
}

#[test]
fn nested_local_sources_flow_through_one_control_plane_without_stale_winners() {
    let fixture = Fixture::new();
    let outer_path = fixture.write_layer(&fixture.project, "outer");
    let inner_path = fixture.write_layer(&fixture.opened_directory, "inner");
    let plane = fixture.control_plane();

    refresh_all(&plane);
    assert_winners(&plane, "outer");
    for (capability, sources) in [
        (
            "command",
            plane.commands(|coordinator| coordinator.snapshot().sources),
        ),
        (
            "subagent",
            plane.subagents(|coordinator| coordinator.snapshot().sources),
        ),
        (
            "mcp",
            plane.mcp(|coordinator| coordinator.snapshot().sources),
        ),
    ] {
        let outer = sources
            .iter()
            .find(|source| Path::new(&source.record.location) == outer_path)
            .unwrap_or_else(|| panic!("{capability} outer source missing from {sources:?}"));
        assert_eq!(outer.record.scope, ExternalSourceScope::Project);
        assert!(outer.record.display_name.starts_with("OpenCode project"));
    }

    fixture.write_layer(&fixture.project, "outer-v2");
    refresh_all(&plane);
    assert_winners(&plane, "outer-v2");

    fs::remove_file(&outer_path).expect("remove outer source");
    refresh_all(&plane);
    assert_winners(&plane, "inner");

    let suppressed = [
        plane.commands(|coordinator| {
            source_stable_key_for_location(&coordinator.snapshot().sources, &inner_path)
        }),
        plane.subagents(|coordinator| {
            source_stable_key_for_location(&coordinator.snapshot().sources, &inner_path)
        }),
        plane.mcp(|coordinator| {
            source_stable_key_for_location(&coordinator.snapshot().sources, &inner_path)
        }),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    plane.replace_suppressed_sources(suppressed);
    refresh_all(&plane);

    assert!(plane.commands(|coordinator| coordinator.snapshot().commands.is_empty()));
    assert!(plane.subagents(|coordinator| coordinator.snapshot().definitions.is_empty()));
    assert!(plane.mcp(|coordinator| coordinator.snapshot().servers.is_empty()));
}

#[test]
fn command_subagent_and_mcp_share_creation_safe_watch_roots() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let as_map = |roots: Vec<openbitfun_product_domains::external_sources::ExternalWatchRoot>| {
        roots
            .into_iter()
            .map(|root| (root.path, root.recursive))
            .collect::<BTreeMap<_, _>>()
    };

    let command_roots = as_map(fixture.command_provider(None).watch_roots(&context));
    let subagent_roots = as_map(fixture.subagent_provider(None).watch_roots(&context));
    let mcp_roots = as_map(fixture.mcp_provider(None).watch_roots(&context));

    assert_eq!(subagent_roots, command_roots);
    assert_eq!(mcp_roots, command_roots);
    assert_eq!(
        command_roots.get(&fixture.user_config),
        Some(&true),
        "the desired global root remains watchable before it exists"
    );
    assert_eq!(
        command_roots.get(&fixture.project.join(".opencode")),
        Some(&true),
        "the desired project root remains watchable before it exists"
    );
    assert_eq!(
        command_roots.get(&fixture.opened_directory.join(".opencode")),
        Some(&true),
        "the desired nested root remains watchable before it exists"
    );
}

#[test]
fn config_directory_aliases_keep_their_first_upstream_position() {
    let xdg_alias = Fixture::new();
    xdg_alias.write_config(&xdg_alias.user_config, "user");
    xdg_alias.write_layer(&xdg_alias.project, "outer");
    let plane = xdg_alias.control_plane_with_explicit_dir(Some(xdg_alias.user_config.clone()));
    refresh_all(&plane);
    assert_winners(&plane, "outer");

    let nested_alias = Fixture::new();
    nested_alias.write_layer(&nested_alias.project, "outer");
    nested_alias.write_layer(&nested_alias.opened_directory, "inner");
    let plane = nested_alias
        .control_plane_with_explicit_dir(Some(nested_alias.opened_directory.join(".opencode")));
    refresh_all(&plane);
    assert_winners(&plane, "outer");
}

#[test]
fn inline_config_is_the_shared_last_override_without_becoming_a_watch_root() {
    let fixture = Fixture::new();
    fixture.write_layer(&fixture.project, "project");
    let mut config = fixture.config(None);
    config.inline_config_content = Some(
        r#"{
          "command": {"review": {"description": "inline command", "template": "inline $ARGUMENTS"}},
          "agent": {"review": {"description": "inline agent", "prompt": "inline agent prompt", "mode": "subagent"}},
          "mcp": {"docs": {"type": "remote", "url": "https://inline.example.test/mcp"}}
        }"#
        .to_string(),
    );
    let plane = fixture.control_plane_with_config(config);

    refresh_all(&plane);
    assert_winners(&plane, "inline");

    for sources in [
        plane.commands(|coordinator| coordinator.snapshot().sources),
        plane.subagents(|coordinator| coordinator.snapshot().sources),
        plane.mcp(|coordinator| coordinator.snapshot().sources),
    ] {
        let inline = sources
            .iter()
            .find(|source| source.record.location == "OPENCODE_CONFIG_CONTENT")
            .expect("inline source");
        assert_eq!(inline.record.scope, ExternalSourceScope::WorkspaceLocal);
    }
}
