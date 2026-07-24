use bitfun_claude_code_adapter::{ClaudeCodeMcpProvider, ClaudeCodeMcpProviderOptions};
use bitfun_product_domains::external_sources::{
    ExecutionDomainId, ExternalMcpDiscoveryInput, ExternalMcpRevisionKey,
    ExternalMcpSourceProvider, ExternalMcpStaticStatus, ExternalMcpTransportKind,
    ExternalSourceContext, ExternalSourceScope,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    user_config: PathBuf,
    project: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let user_config = temp.path().join("home/.claude.json");
        let project = temp.path().join("project");
        fs::create_dir_all(user_config.parent().unwrap()).unwrap();
        fs::create_dir_all(project.join(".git")).unwrap();
        Self {
            _temp: temp,
            user_config,
            project,
        }
    }

    fn provider(&self) -> ClaudeCodeMcpProvider {
        ClaudeCodeMcpProvider::new(ClaudeCodeMcpProviderOptions {
            user_config_file: self.user_config.clone(),
            project_root_override: Some(self.project.clone()),
            project_config_enabled: true,
        })
    }

    fn input(&self) -> ExternalMcpDiscoveryInput {
        ExternalMcpDiscoveryInput {
            context: ExternalSourceContext {
                workspace_root: Some(self.project.clone()),
                execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
            },
            suppressed_sources: BTreeSet::new(),
            revision_key: ExternalMcpRevisionKey::new([7; 32]),
        }
    }
}

fn write(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn json_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\")
}

#[test]
fn local_then_project_then_user_precedence_replaces_whole_server_entry() {
    let fixture = Fixture::new();
    write(
        &fixture.user_config,
        &format!(
            r#"{{
              "mcpServers": {{
                "shared": {{"command":"user-bin","args":["user"],"env":{{"USER_ONLY":"secret"}}}},
                "user-only": {{"command":"user-only"}}
              }},
              "projects": {{
                "{}": {{
                  "mcpServers": {{
                    "shared": {{"command":"local-bin","args":["local"]}}
                  }}
                }}
              }}
            }}"#,
            json_path(&fixture.project)
        ),
    );
    write(
        fixture.project.join(".mcp.json"),
        r#"{"mcpServers":{"shared":{"command":"project-bin","args":["project"],"env":{"PROJECT_ONLY":"secret"}},"project-only":{"command":"project-only"}}}"#,
    );

    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.input()).unwrap();
    assert!(snapshot
        .sources
        .iter()
        .all(|source| source.content_version.starts_with("hmac-sha256:")));
    assert!(snapshot
        .servers
        .iter()
        .all(|server| server.behavior_version.starts_with("hmac-sha256:")));
    let shared = snapshot
        .servers
        .iter()
        .find(|server| server.name == "shared")
        .unwrap();

    assert_eq!(shared.command_preview.as_deref(), Some("local-bin"));
    assert_eq!(shared.argument_count, 1);
    assert!(
        shared.environment_keys.is_empty(),
        "full-entry replacement must not merge env"
    );
    assert_eq!(shared.provenance.len(), 1);
    assert!(snapshot.servers.iter().any(|item| item.name == "user-only"));
    assert!(snapshot
        .servers
        .iter()
        .any(|item| item.name == "project-only"));
    assert_eq!(
        shared.id.source,
        snapshot
            .sources
            .iter()
            .find(|source| source.scope == ExternalSourceScope::WorkspaceLocal)
            .unwrap()
            .key
    );
}

#[test]
fn discovery_is_static_and_public_projection_redacts_runtime_values() {
    let fixture = Fixture::new();
    let marker = fixture._temp.path().join("must-not-exist.txt");
    write(
        fixture.project.join(".mcp.json"),
        &format!(
            r#"{{"mcpServers":{{
              "local":{{
                "command":"powershell",
                "args":["-Command","Set-Content","{}","executed"],
                "env":{{"TOKEN":"literal-secret","READ_TOKEN":"${{CLAUDE_MCP_MISSING_TOKEN}}"}}
              }},
              "remote":{{
                "type":"http",
                "url":"https://api.example.test/private/path?token=hidden",
                "headers":{{"Authorization":"Bearer secret"}}
              }}
            }}}}"#,
            json_path(&marker)
        ),
    );

    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.input()).unwrap();
    assert!(!marker.exists());
    let local = snapshot
        .servers
        .iter()
        .find(|item| item.name == "local")
        .unwrap();
    assert_eq!(local.transport, ExternalMcpTransportKind::LocalStdio);
    assert_eq!(local.environment_keys, vec!["READ_TOKEN", "TOKEN"]);
    assert_eq!(
        local.environment_reference_names,
        vec!["CLAUDE_MCP_MISSING_TOKEN"]
    );
    let remote = snapshot
        .servers
        .iter()
        .find(|item| item.name == "remote")
        .unwrap();
    assert_eq!(remote.transport, ExternalMcpTransportKind::StreamableHttp);
    assert_eq!(
        remote.remote_url_preview.as_deref(),
        Some("https://api.example.test/")
    );
    let encoded = serde_json::to_string(&snapshot).unwrap();
    for secret in [
        "literal-secret",
        "Bearer secret",
        "private/path",
        "token=hidden",
    ] {
        assert!(!encoded.contains(secret));
    }
    let error = provider
        .prepare_server(&fixture.input(), &local.id, &local.behavior_version)
        .unwrap_err();
    assert_eq!(error.code, "claude.mcp.environment_missing");
}

#[test]
fn public_working_directory_uses_a_workspace_relative_or_redacted_label() {
    let fixture = Fixture::new();
    let outside = fixture._temp.path().join("outside/private");
    write(
        fixture.project.join(".mcp.json"),
        &format!(
            r#"{{"mcpServers":{{
              "inside":{{"command":"inside-bin","cwd":"packages/app"}},
              "outside":{{"command":"outside-bin","cwd":"{}"}}
            }}}}"#,
            json_path(&outside)
        ),
    );

    let snapshot = fixture.provider().discover(&fixture.input()).unwrap();
    let inside = snapshot
        .servers
        .iter()
        .find(|item| item.name == "inside")
        .unwrap();
    let outside_server = snapshot
        .servers
        .iter()
        .find(|item| item.name == "outside")
        .unwrap();
    assert_eq!(inside.working_directory.as_deref(), Some("./packages/app"));
    assert_eq!(
        outside_server.working_directory.as_deref(),
        Some("<configured>")
    );
    assert!(!serde_json::to_string(&snapshot)
        .unwrap()
        .contains(&outside.to_string_lossy().to_string()));
}

#[test]
fn executable_or_network_target_environment_references_are_restricted() {
    let fixture = Fixture::new();
    write(
        fixture.project.join(".mcp.json"),
        r#"{"mcpServers":{
          "command":{"command":"${MCP_COMMAND}","args":[]},
          "argument":{"command":"node","args":["${MCP_SCRIPT}"]},
          "cwd":{"command":"node","cwd":"${MCP_CWD}"},
          "url":{"type":"http","url":"https://${MCP_HOST}/mcp"},
          "sse":{"type":"sse","url":"https://example.test/sse"}
        }}"#,
    );
    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.input()).unwrap();

    assert_eq!(snapshot.servers.len(), 5);
    assert!(snapshot.servers.iter().all(|server| matches!(
        server.static_status,
        ExternalMcpStaticStatus::Unsupported { .. }
    )));
    for server in &snapshot.servers {
        assert!(provider
            .prepare_server(&fixture.input(), &server.id, &server.behavior_version)
            .is_err());
    }
}

#[test]
fn unsupported_upstream_fields_fail_closed_instead_of_changing_behavior_silently() {
    let fixture = Fixture::new();
    write(
        fixture.project.join(".mcp.json"),
        r#"{"mcpServers":{
          "always-loaded":{"command":"node","alwaysLoad":true},
          "dynamic-headers":{"type":"http","url":"https://example.test/mcp","headersHelper":"./headers.sh"},
          "invalid-type":{"type":7,"command":"node"},
          "unknown":{"command":"node","futureBehavior":"enabled"}
        }}"#,
    );

    let provider = fixture.provider();
    let snapshot = provider.discover(&fixture.input()).unwrap();

    assert_eq!(snapshot.servers.len(), 4);
    assert!(snapshot.servers.iter().all(|server| matches!(
        server.static_status,
        ExternalMcpStaticStatus::Unsupported { .. }
    )));
    for server in &snapshot.servers {
        assert!(provider
            .prepare_server(&fixture.input(), &server.id, &server.behavior_version)
            .is_err());
    }
}

#[test]
fn suppression_recomputes_native_winner_and_stale_prepare_fails_closed() {
    let fixture = Fixture::new();
    write(
        &fixture.user_config,
        r#"{"mcpServers":{"shared":{"command":"user-bin"}}}"#,
    );
    write(
        fixture.project.join(".mcp.json"),
        r#"{"mcpServers":{"shared":{"command":"project-bin"}}}"#,
    );
    let provider = fixture.provider();
    let first_input = fixture.input();
    let first = provider.discover(&first_input).unwrap();
    let first_server = &first.servers[0];
    let old_version = first_server.behavior_version.clone();
    let project_source = first
        .sources
        .iter()
        .find(|source| source.scope == ExternalSourceScope::Project)
        .unwrap()
        .key
        .clone();
    let suppressed_input = ExternalMcpDiscoveryInput {
        context: first_input.context.clone(),
        suppressed_sources: [project_source].into_iter().collect(),
        revision_key: first_input.revision_key.clone(),
    };
    let suppressed = provider.discover(&suppressed_input).unwrap();
    assert_eq!(
        suppressed.servers[0].command_preview.as_deref(),
        Some("user-bin")
    );
    assert_ne!(suppressed.servers[0].behavior_version, old_version);
    let error = provider
        .prepare_server(&suppressed_input, &suppressed.servers[0].id, &old_version)
        .unwrap_err();
    assert_eq!(error.code, "claude.mcp.stale_revision");
}

#[test]
fn watch_roots_cover_user_config_and_project_mcp_file_parent() {
    let fixture = Fixture::new();
    let roots = fixture.provider().watch_roots(&fixture.input().context);
    assert!(roots
        .iter()
        .any(|root| root.path == fixture.user_config.parent().unwrap()));
    assert!(roots.iter().any(|root| root.path == fixture.project));
}

#[test]
fn public_command_preview_does_not_expose_an_absolute_home_path() {
    let fixture = Fixture::new();
    write(
        fixture.project.join(".mcp.json"),
        r#"{"mcpServers":{"private":{"command":"C:\\Users\\alice\\private\\mcp.exe"}}}"#,
    );

    let snapshot = fixture.provider().discover(&fixture.input()).unwrap();

    assert_eq!(
        snapshot.servers[0].command_preview.as_deref(),
        Some("mcp.exe")
    );
    assert!(!serde_json::to_string(&snapshot).unwrap().contains("alice"));
}

#[test]
fn malformed_project_config_does_not_reactivate_a_user_server() {
    let fixture = Fixture::new();
    write(
        &fixture.user_config,
        r#"{"mcpServers":{"shared":{"command":"user-bin"}}}"#,
    );
    write(fixture.project.join(".mcp.json"), "{not-json");

    let result = fixture.provider().discover(&fixture.input());

    assert!(
        result.is_err(),
        "a broken higher native layer must fail closed"
    );
}

#[cfg(unix)]
#[test]
fn project_config_symlink_cannot_escape_the_project_root() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let outside = fixture._temp.path().join("outside.json");
    write(
        &outside,
        r#"{"mcpServers":{"escaped":{"command":"outside-bin"}}}"#,
    );
    symlink(&outside, fixture.project.join(".mcp.json")).unwrap();

    let result = fixture.provider().discover(&fixture.input());

    assert!(
        result.is_err(),
        "project provenance must not cover an outside target"
    );
}
