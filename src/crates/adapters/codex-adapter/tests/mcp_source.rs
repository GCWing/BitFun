use bitfun_codex_adapter::{CodexMcpProvider, CodexMcpProviderOptions};
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
    codex_home: PathBuf,
    project: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let codex_home = temp.path().join("home/.codex");
        let project = temp.path().join("project");
        let workspace = project.join("packages/app");
        fs::create_dir_all(&codex_home).unwrap();
        fs::create_dir_all(project.join(".git")).unwrap();
        fs::create_dir_all(&workspace).unwrap();
        Self {
            _temp: temp,
            codex_home,
            project,
            workspace,
        }
    }

    fn provider(&self) -> CodexMcpProvider {
        CodexMcpProvider::new(CodexMcpProviderOptions {
            codex_home: self.codex_home.clone(),
            project_root_override: Some(self.project.clone()),
            project_config_enabled: true,
        })
    }

    fn input(&self) -> ExternalMcpDiscoveryInput {
        ExternalMcpDiscoveryInput {
            context: ExternalSourceContext {
                workspace_root: Some(self.workspace.clone()),
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

#[test]
fn malformed_required_user_layer_blocks_project_mcp_candidates() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("config.toml"),
        "[mcp_servers.invalid\n",
    );
    write(
        fixture.project.join(".codex/config.toml"),
        r#"[mcp_servers.project]
command = "project-server"
"#,
    );

    let error = fixture.provider().discover(&fixture.input()).unwrap_err();
    assert_eq!(error.code, "codex.mcp.overlay_invalid");
}

#[test]
fn project_layers_overlay_server_fields_in_native_order() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("config.toml"),
        r#"[mcp_servers.shared]
command = "user-server"
args = ["--user"]

[mcp_servers.shared.env]
TOKEN = "literal-secret"
"#,
    );
    write(
        fixture.project.join(".codex/config.toml"),
        r#"[mcp_servers.shared]
args = ["--project"]
"#,
    );
    write(
        fixture.workspace.join(".codex/config.toml"),
        r#"[mcp_servers.shared]
enabled = true
"#,
    );

    let snapshot = fixture.provider().discover(&fixture.input()).unwrap();
    assert!(snapshot
        .sources
        .iter()
        .all(|source| source.content_version.starts_with("hmac-sha256:")));
    assert!(snapshot
        .servers
        .iter()
        .all(|server| server.behavior_version.starts_with("hmac-sha256:")));
    let shared = &snapshot.servers[0];
    assert_eq!(shared.command_preview.as_deref(), Some("user-server"));
    assert_eq!(shared.argument_count, 1);
    assert_eq!(shared.environment_keys, vec!["TOKEN"]);
    assert_eq!(shared.provenance.len(), 3);
    assert_eq!(
        shared.id.source,
        snapshot
            .sources
            .iter()
            .find(|source| {
                source
                    .location
                    .replace('\\', "/")
                    .ends_with("packages/app/.codex/config.toml")
            })
            .unwrap()
            .key
    );
}

#[test]
fn remote_projection_redacts_secrets_and_prepare_resolves_named_environment() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("config.toml"),
        r#"[mcp_servers.docs]
url = "https://api.example.test/private/path?token=hidden"
bearer_token_env_var = "CODEX_MCP_MISSING_BEARER"

[mcp_servers.docs.http_headers]
X-Literal = "literal-secret"

[mcp_servers.docs.env_http_headers]
X-Env = "CODEX_MCP_MISSING_HEADER"
"#,
    );

    let provider = fixture.provider();
    let input = fixture.input();
    let snapshot = provider.discover(&input).unwrap();
    let docs = &snapshot.servers[0];
    assert_eq!(docs.transport, ExternalMcpTransportKind::StreamableHttp);
    assert_eq!(
        docs.remote_url_preview.as_deref(),
        Some("https://api.example.test/")
    );
    assert_eq!(
        docs.environment_reference_names,
        vec!["CODEX_MCP_MISSING_BEARER", "CODEX_MCP_MISSING_HEADER"]
    );
    assert_eq!(
        docs.header_names,
        vec!["Authorization", "X-Env", "X-Literal"]
    );
    let encoded = serde_json::to_string(&snapshot).unwrap();
    for secret in ["literal-secret", "private/path", "token=hidden"] {
        assert!(!encoded.contains(secret));
    }
    let error = provider
        .prepare_server(&input, &docs.id, &docs.behavior_version)
        .unwrap_err();
    assert_eq!(error.code, "codex.mcp.environment_missing");
}

#[test]
fn unsupported_runtime_controls_block_but_required_only_warns() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("config.toml"),
        r#"[mcp_servers.required_only]
command = "required-server"
required = true

[mcp_servers.filtered]
command = "filtered-server"
startup_timeout_sec = 15
enabled_tools = ["search"]

[mcp_servers.remote_executor]
command = "remote-server"
environment_id = "remote-machine"

[mcp_servers.disabled]
command = "disabled-server"
enabled = false
"#,
    );

    let snapshot = fixture.provider().discover(&fixture.input()).unwrap();
    let by_name = |name: &str| {
        snapshot
            .servers
            .iter()
            .find(|item| item.name == name)
            .unwrap()
    };
    assert_eq!(
        by_name("required_only").static_status,
        ExternalMcpStaticStatus::Ready
    );
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|item| item.code == "codex.mcp.required_not_imported"));
    assert!(matches!(
        by_name("filtered").static_status,
        ExternalMcpStaticStatus::Unsupported { .. }
    ));
    assert!(matches!(
        by_name("remote_executor").static_status,
        ExternalMcpStaticStatus::Unsupported { .. }
    ));
    assert_eq!(
        by_name("disabled").static_status,
        ExternalMcpStaticStatus::DisabledBySource
    );
}

#[test]
fn diagnostic_only_required_does_not_change_behavior_version() {
    let fixture = Fixture::new();
    let config = fixture.codex_home.join("config.toml");
    write(
        &config,
        r#"[mcp_servers.shared]
command = "server"
required = true
"#,
    );
    let first = fixture.provider().discover(&fixture.input()).unwrap();
    let version = first.servers[0].behavior_version.clone();

    write(
        &config,
        r#"[mcp_servers.shared]
command = "server"
required = false
"#,
    );
    let second = fixture.provider().discover(&fixture.input()).unwrap();
    assert_eq!(second.servers[0].behavior_version, version);
}

#[test]
fn fields_rejected_by_current_codex_are_not_silently_accepted() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("config.toml"),
        r#"[mcp_servers.shared]
command = "server"
name = "Invented display label"
"#,
    );

    let snapshot = fixture.provider().discover(&fixture.input()).unwrap();

    assert!(matches!(
        &snapshot.servers[0].static_status,
        ExternalMcpStaticStatus::Unsupported { reason }
            if reason.contains("field 'name' is not supported")
    ));
}

#[test]
fn suppression_recomputes_field_merge_and_stale_prepare_fails_closed() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("config.toml"),
        r#"[mcp_servers.shared]
command = "user-server"
args = ["--user"]
"#,
    );
    write(
        fixture.project.join(".codex/config.toml"),
        r#"[mcp_servers.shared]
args = ["--project"]
"#,
    );
    let provider = fixture.provider();
    let first_input = fixture.input();
    let first = provider.discover(&first_input).unwrap();
    let old = first.servers[0].clone();
    let project_source = first
        .sources
        .iter()
        .find(|source| source.scope == ExternalSourceScope::Project)
        .unwrap()
        .key
        .clone();
    let suppressed_input = ExternalMcpDiscoveryInput {
        context: first_input.context,
        suppressed_sources: [project_source].into_iter().collect(),
        revision_key: first_input.revision_key,
    };
    let suppressed = provider.discover(&suppressed_input).unwrap();
    assert_eq!(suppressed.servers[0].argument_count, 1);
    assert_ne!(suppressed.servers[0].behavior_version, old.behavior_version);
    let error = provider
        .prepare_server(
            &suppressed_input,
            &suppressed.servers[0].id,
            &old.behavior_version,
        )
        .unwrap_err();
    assert_eq!(error.code, "codex.mcp.stale_revision");
}

#[test]
fn watch_roots_cover_codex_home_and_project_codex_directories() {
    let fixture = Fixture::new();
    let roots = fixture.provider().watch_roots(&fixture.input().context);
    assert!(roots
        .iter()
        .any(|root| root.path == fixture.codex_home && root.recursive));
    assert!(roots
        .iter()
        .any(|root| root.path == fixture.project.join(".codex") && root.recursive));
    assert!(roots
        .iter()
        .any(|root| root.path == fixture.workspace.join(".codex") && root.recursive));
}

#[test]
fn public_command_preview_does_not_expose_an_absolute_home_path() {
    let fixture = Fixture::new();
    write(
        fixture.codex_home.join("config.toml"),
        r#"[mcp_servers.private]
command = "C:\\Users\\alice\\private\\mcp.exe"
"#,
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
        fixture.codex_home.join("config.toml"),
        r#"[mcp_servers.shared]
command = "user-server"
"#,
    );
    write(
        fixture.project.join(".codex/config.toml"),
        "[mcp_servers.shared\n",
    );

    let result = fixture.provider().discover(&fixture.input());

    assert!(
        result.is_err(),
        "a broken higher native layer must fail closed"
    );
}

#[cfg(unix)]
#[test]
fn project_config_symlink_cannot_escape_the_codex_directory() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let outside = fixture._temp.path().join("outside.toml");
    write(
        &outside,
        r#"[mcp_servers.escaped]
command = "outside-server"
"#,
    );
    fs::create_dir_all(fixture.project.join(".codex")).unwrap();
    symlink(&outside, fixture.project.join(".codex/config.toml")).unwrap();

    let result = fixture.provider().discover(&fixture.input());

    assert!(
        result.is_err(),
        "project provenance must not cover an outside target"
    );
}
