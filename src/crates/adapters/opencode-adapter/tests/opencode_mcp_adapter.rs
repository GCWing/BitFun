use openbitfun_opencode_adapter::{
    OpenCodeCommandProviderOptions, OpenCodeMcpProvider, OpenCodeMcpProviderOptions,
};
use openbitfun_product_domains::external_sources::{
    ExecutionDomainId, ExternalMcpDiscoveryInput, ExternalMcpRevisionKey,
    ExternalMcpSourceProvider, ExternalMcpStaticStatus, ExternalMcpTransportKind,
    ExternalSourceContext, ExternalSourceScope, PreparedExternalMcpImportTransport,
    PreparedExternalMcpTransport,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn create_test_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        let mut command = command;
        command.creation_flags(0x0800_0000);
        command
    }
    #[cfg(not(windows))]
    command
}

fn context(workspace_root: PathBuf) -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: Some(workspace_root),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

fn revision_key() -> ExternalMcpRevisionKey {
    ExternalMcpRevisionKey::new([7; 32])
}

fn options(user_config_dir: PathBuf) -> OpenCodeMcpProviderOptions {
    OpenCodeMcpProviderOptions {
        config: OpenCodeCommandProviderOptions {
            user_config_dir,
            legacy_user_config_dir: None,
            explicit_config_file: None,
            explicit_config_dir: None,
            inline_config_content: None,
            project_config_enabled: true,
        },
        project_root_override: None,
    }
}

#[test]
fn opencode_config_dir_keeps_xdg_user_config_when_read_from_environment() {
    const CHILD_MARKER: &str = "OPENBITFUN_OPENCODE_MCP_ENV_CHILD";
    const CHILD_XDG: &str = "OPENBITFUN_OPENCODE_MCP_ENV_XDG";
    const CHILD_EXPLICIT: &str = "OPENBITFUN_OPENCODE_MCP_ENV_EXPLICIT";

    if std::env::var_os(CHILD_MARKER).is_some() {
        let xdg = PathBuf::from(std::env::var_os(CHILD_XDG).expect("child XDG path"));
        let explicit =
            PathBuf::from(std::env::var_os(CHILD_EXPLICIT).expect("child explicit path"));
        let provider_options = OpenCodeMcpProviderOptions::from_environment();
        assert_eq!(
            provider_options.config.user_config_dir,
            xdg.join("opencode")
        );
        assert_eq!(
            provider_options.config.explicit_config_dir.as_deref(),
            Some(explicit.as_path())
        );
        assert!(
            provider_options.config.legacy_user_config_dir.is_some(),
            "OPENCODE_CONFIG_DIR must not remove the normal compatibility directory"
        );
        return;
    }

    let temp = TempDir::new().unwrap();
    let xdg = temp.path().join("xdg");
    let explicit = temp.path().join("explicit");
    let output = create_test_command(std::env::current_exe().expect("current test executable"))
        .arg("--exact")
        .arg("opencode_config_dir_keeps_xdg_user_config_when_read_from_environment")
        .arg("--nocapture")
        .env(CHILD_MARKER, "1")
        .env(CHILD_XDG, &xdg)
        .env(CHILD_EXPLICIT, &explicit)
        .env("XDG_CONFIG_HOME", &xdg)
        .env("OPENCODE_CONFIG_DIR", &explicit)
        .env_remove("OPENCODE_CONFIG")
        .env_remove("OPENCODE_DISABLE_PROJECT_CONFIG")
        .output()
        .expect("run isolated environment child");

    assert!(
        output.status.success(),
        "isolated environment assertion failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn discovery_deep_merges_layers_without_exposing_or_executing_runtime_values() {
    let temp = TempDir::new().unwrap();
    let user = temp.path().join("user");
    let project = temp.path().join("project");
    fs::create_dir_all(&user).unwrap();
    fs::create_dir_all(project.join(".git")).unwrap();
    let marker = temp.path().join("must-not-exist.txt");
    fs::write(
        user.join("opencode.jsonc"),
        format!(
            r#"{{
              // Static discovery must not launch this command.
              "mcp": {{
                "local-tools": {{
                  "type": "local",
                  "command": ["powershell", "-NoProfile", "-Command", "Set-Content", "{}", "executed"],
                  "cwd": "tools",
                  "environment": {{
                    "PRIVATE_TOKEN": "literal-secret",
                    "READ_TOKEN": "{{env:OPENCODE_MCP_TEST_TOKEN}}"
                  }}
                }},
                "github": {{
                  "type": "remote",
                  "url": "https://global.example.test/private/token-path?token=hidden",
                  "headers": {{"Authorization": "Bearer secret"}}
                }}
              }}
            }}"#,
            marker.display().to_string().replace('\\', "\\\\")
        ),
    )
    .unwrap();
    fs::write(
        project.join("opencode.json"),
        r#"{
          "mcp": {
            "github": {
              "url": "https://project.example.test/mcp",
              "headers": {"X-Project": "enabled"}
            }
          }
        }"#,
    )
    .unwrap();

    let provider = OpenCodeMcpProvider::new(options(user.clone()));
    let input = ExternalMcpDiscoveryInput {
        context: context(project.clone()),
        suppressed_sources: BTreeSet::new(),
        revision_key: revision_key(),
    };
    let snapshot = provider.discover(&input).unwrap();

    assert!(!marker.exists(), "discovery must remain static");
    assert!(snapshot
        .sources
        .iter()
        .all(|source| source.content_version.starts_with("hmac-sha256:")));
    assert!(snapshot
        .servers
        .iter()
        .all(|server| server.behavior_version.starts_with("hmac-sha256:")));
    assert_eq!(snapshot.servers.len(), 2);
    let github = snapshot
        .servers
        .iter()
        .find(|server| server.name == "github")
        .unwrap();
    assert_eq!(github.transport, ExternalMcpTransportKind::StreamableHttp);
    assert_eq!(
        github.remote_url_preview.as_deref(),
        Some("https://project.example.test/")
    );
    assert_eq!(
        github.header_names,
        vec!["Authorization".to_string(), "X-Project".to_string()]
    );
    assert_eq!(github.provenance.len(), 2);
    let local = snapshot
        .servers
        .iter()
        .find(|server| server.name == "local-tools")
        .unwrap();
    assert_eq!(local.command_preview.as_deref(), Some("powershell"));
    assert_eq!(local.argument_count, 5);
    assert_eq!(
        local.environment_keys,
        vec!["PRIVATE_TOKEN".to_string(), "READ_TOKEN".to_string()]
    );
    assert_eq!(
        local.environment_reference_names,
        vec!["OPENCODE_MCP_TEST_TOKEN".to_string()]
    );
    assert_eq!(
        local.working_directory.as_deref(),
        Some(project.join("tools").to_string_lossy().as_ref())
    );

    let encoded = serde_json::to_string(&snapshot).unwrap();
    assert!(!encoded.contains("literal-secret"));
    assert!(!encoded.contains("Bearer secret"));
    assert!(!encoded.contains("token=hidden"));
    assert!(!encoded.contains("private/token-path"));

    let prepared = provider
        .prepare_server(&input, &github.id, &github.behavior_version)
        .unwrap();
    match prepared.transport {
        PreparedExternalMcpTransport::Remote { headers, url, .. } => {
            assert_eq!(url, "https://project.example.test/mcp");
            assert_eq!(headers["Authorization"].expose(), "Bearer secret");
            assert_eq!(headers["X-Project"].expose(), "enabled");
        }
        other => panic!("expected remote transport, got {other:?}"),
    }
}

#[test]
fn local_server_without_cwd_uses_the_workspace_like_opencode() {
    let temp = TempDir::new().unwrap();
    let user = temp.path().join("user");
    let project = temp.path().join("project");
    fs::create_dir_all(&user).unwrap();
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::write(
        user.join("opencode.json"),
        r#"{"mcp":{"local":{"type":"local","command":["node","server.js"]}}}"#,
    )
    .unwrap();

    let provider = OpenCodeMcpProvider::new(options(user));
    let input = ExternalMcpDiscoveryInput {
        context: context(project.clone()),
        suppressed_sources: BTreeSet::new(),
        revision_key: revision_key(),
    };
    let snapshot = provider.discover(&input).unwrap();
    let server = &snapshot.servers[0];

    assert_eq!(
        server.working_directory.as_deref(),
        Some(project.to_string_lossy().as_ref())
    );
    let prepared = provider
        .prepare_server(&input, &server.id, &server.behavior_version)
        .unwrap();
    match prepared.transport {
        PreparedExternalMcpTransport::Local {
            working_directory, ..
        } => assert_eq!(working_directory.as_deref(), Some(project.as_path())),
        other => panic!("expected local transport, got {other:?}"),
    }
}

#[test]
fn safe_user_servers_have_a_native_import_projection_without_runtime_resolution() {
    let temp = TempDir::new().unwrap();
    let user = temp.path().join("user");
    let project = temp.path().join("project");
    fs::create_dir_all(&user).unwrap();
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::write(
        user.join("opencode.json"),
        r#"{"mcp":{
          "local":{"type":"local","command":["docs-mcp","--stdio"]},
          "remote":{"type":"remote","url":"https://docs.example.test/mcp"}
        }}"#,
    )
    .unwrap();

    let provider = OpenCodeMcpProvider::new(options(user));
    let input = ExternalMcpDiscoveryInput {
        context: context(project),
        suppressed_sources: BTreeSet::new(),
        revision_key: revision_key(),
    };
    let snapshot = provider.discover(&input).unwrap();
    let local = snapshot
        .servers
        .iter()
        .find(|server| server.name == "local")
        .unwrap();
    let remote = snapshot
        .servers
        .iter()
        .find(|server| server.name == "remote")
        .unwrap();

    let prepared_local = provider
        .prepare_import(&input, &local.id, &local.behavior_version)
        .unwrap();
    match prepared_local.transport {
        PreparedExternalMcpImportTransport::Local { command, args } => {
            assert_eq!(command, "docs-mcp");
            assert_eq!(args, ["--stdio"]);
        }
        other => panic!("expected local import, got {other:?}"),
    }
    let prepared_remote = provider
        .prepare_import(&input, &remote.id, &remote.behavior_version)
        .unwrap();
    match prepared_remote.transport {
        PreparedExternalMcpImportTransport::Remote { url } => {
            assert_eq!(url, "https://docs.example.test/mcp");
        }
        other => panic!("expected remote import, got {other:?}"),
    }
}

#[test]
fn unsafe_user_servers_require_setup_instead_of_copying_opaque_fields() {
    let temp = TempDir::new().unwrap();
    let user = temp.path().join("user");
    let project = temp.path().join("project");
    fs::create_dir_all(&user).unwrap();
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::write(
        user.join("opencode.json"),
        r#"{"mcp":{
          "env":{"type":"local","command":["docs-mcp"],"environment":{"TOKEN":"secret"}},
          "cwd":{"type":"local","command":["docs-mcp"],"cwd":"tools"},
          "query":{"type":"remote","url":"https://docs.example.test/mcp?token=secret"},
          "headers":{"type":"remote","url":"https://docs.example.test/mcp","headers":{"Authorization":"secret"}},
          "no-oauth":{"type":"remote","url":"https://docs.example.test/mcp","oauth":false}
        }}"#,
    )
    .unwrap();

    let provider = OpenCodeMcpProvider::new(options(user));
    let input = ExternalMcpDiscoveryInput {
        context: context(project),
        suppressed_sources: BTreeSet::new(),
        revision_key: revision_key(),
    };
    let snapshot = provider.discover(&input).unwrap();

    for server in &snapshot.servers {
        let error = provider
            .prepare_import(&input, &server.id, &server.behavior_version)
            .unwrap_err();
        assert_eq!(error.code, "external_mcp.import_setup_required");
        assert!(!error.message.contains("secret"));
        assert!(!error.message.contains("--opaque"));
    }
}

#[test]
fn unknown_local_and_remote_fields_fail_closed_before_import() {
    let temp = TempDir::new().unwrap();
    let user = temp.path().join("user");
    let project = temp.path().join("project");
    fs::create_dir_all(&user).unwrap();
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::write(
        user.join("opencode.json"),
        r#"{"mcp":{
          "local":{"type":"local","command":["docs-mcp"],"providerMode":"opaque"},
          "remote":{"type":"remote","url":"https://docs.example.test/mcp","providerMode":"opaque"}
        }}"#,
    )
    .unwrap();

    let provider = OpenCodeMcpProvider::new(options(user));
    let input = ExternalMcpDiscoveryInput {
        context: context(project),
        suppressed_sources: BTreeSet::new(),
        revision_key: revision_key(),
    };
    let snapshot = provider.discover(&input).unwrap();

    for server in &snapshot.servers {
        assert!(matches!(
            server.static_status,
            ExternalMcpStaticStatus::Unsupported { .. }
        ));
        let error = provider
            .prepare_import(&input, &server.id, &server.behavior_version)
            .unwrap_err();
        assert_eq!(error.code, "opencode.mcp.not_activatable");
        assert!(!error.message.contains("opaque"));
    }
}

#[test]
fn suppression_recomputes_the_opencode_merge_and_stale_prepare_fails_closed() {
    let temp = TempDir::new().unwrap();
    let user = temp.path().join("user");
    let project = temp.path().join("project");
    fs::create_dir_all(&user).unwrap();
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::write(
        user.join("opencode.json"),
        r#"{"mcp":{"github":{"type":"remote","url":"https://global.example.test/mcp"}}}"#,
    )
    .unwrap();
    fs::write(
        project.join("opencode.json"),
        r#"{"mcp":{"github":{"url":"https://project.example.test/mcp"}}}"#,
    )
    .unwrap();

    let provider = OpenCodeMcpProvider::new(options(user));
    let base_input = ExternalMcpDiscoveryInput {
        context: context(project),
        suppressed_sources: BTreeSet::new(),
        revision_key: revision_key(),
    };
    let initial = provider.discover(&base_input).unwrap();
    let initial_github = initial
        .servers
        .iter()
        .find(|server| server.name == "github")
        .unwrap();
    let old_version = initial_github.behavior_version.clone();
    let project_source = initial
        .sources
        .iter()
        .find(|source| source.scope == ExternalSourceScope::Project)
        .unwrap()
        .key
        .clone();
    let suppressed_input = ExternalMcpDiscoveryInput {
        context: base_input.context.clone(),
        suppressed_sources: [project_source].into_iter().collect(),
        revision_key: base_input.revision_key.clone(),
    };
    let suppressed = provider.discover(&suppressed_input).unwrap();
    let github = suppressed
        .servers
        .iter()
        .find(|server| server.name == "github")
        .unwrap();
    assert_eq!(
        github.remote_url_preview.as_deref(),
        Some("https://global.example.test/")
    );
    assert_ne!(github.behavior_version, old_version);

    let error = provider
        .prepare_server(&suppressed_input, &github.id, &old_version)
        .unwrap_err();
    assert_eq!(error.code, "opencode.mcp.stale_revision");
}

#[test]
fn unsupported_or_source_disabled_servers_remain_visible_but_cannot_be_prepared() {
    let temp = TempDir::new().unwrap();
    let user = temp.path().join("user");
    let project = temp.path().join("project");
    fs::create_dir_all(&user).unwrap();
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::write(
        user.join("opencode.json"),
        r#"{
          "mcp": {
            "disabled": {"type":"local","command":["node","server.js"],"enabled":false},
            "insecure": {"type":"remote","url":"http://example.test/mcp"},
            "invalid-timeout": {"type":"remote","url":"https://example.test/mcp","timeout":0},
            "unsafe-timeout": {"type":"remote","url":"https://example.test/mcp","timeout":9007199254740992},
            "client-secret": {
              "type":"remote",
              "url":"https://example.test/mcp",
              "oauth":{"clientId":"id","clientSecret":"secret"}
            },
            "mutable-command": {"type":"local","command":["{env:MCP_COMMAND}"]},
            "mutable-host": {"type":"remote","url":"https://{env:MCP_HOST}/mcp"}
          }
        }"#,
    )
    .unwrap();
    let provider = OpenCodeMcpProvider::new(options(user));
    let input = ExternalMcpDiscoveryInput {
        context: context(project),
        suppressed_sources: BTreeSet::new(),
        revision_key: revision_key(),
    };
    let snapshot = provider.discover(&input).unwrap();

    assert!(matches!(
        snapshot
            .servers
            .iter()
            .find(|server| server.name == "disabled")
            .unwrap()
            .static_status,
        ExternalMcpStaticStatus::DisabledBySource
    ));
    for name in [
        "insecure",
        "invalid-timeout",
        "unsafe-timeout",
        "client-secret",
        "mutable-command",
        "mutable-host",
    ] {
        let server = snapshot
            .servers
            .iter()
            .find(|server| server.name == name)
            .unwrap();
        assert!(matches!(
            server.static_status,
            ExternalMcpStaticStatus::Unsupported { .. }
        ));
        assert!(provider
            .prepare_server(&input, &server.id, &server.behavior_version)
            .is_err());
    }
}

#[test]
fn opencode_timeout_applies_to_all_mcp_lifecycle_phases() {
    let temp = TempDir::new().unwrap();
    let user = temp.path().join("user");
    let project = temp.path().join("project");
    fs::create_dir_all(&user).unwrap();
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::write(
        user.join("opencode.json"),
        r#"{"mcp":{"docs":{"type":"remote","url":"https://example.test/mcp","timeout":1250}}}"#,
    )
    .unwrap();
    let provider = OpenCodeMcpProvider::new(options(user));
    let input = ExternalMcpDiscoveryInput {
        context: context(project),
        suppressed_sources: BTreeSet::new(),
        revision_key: revision_key(),
    };

    let snapshot = provider.discover(&input).unwrap();
    let server = snapshot
        .servers
        .iter()
        .find(|server| server.name == "docs")
        .unwrap();
    assert!(matches!(
        server.static_status,
        ExternalMcpStaticStatus::Ready
    ));
    assert_eq!(server.timeouts.startup_ms, Some(1_250));
    assert_eq!(server.timeouts.catalog_ms, Some(1_250));
    assert_eq!(server.timeouts.execution_ms, Some(1_250));

    let prepared = provider
        .prepare_server(&input, &server.id, &server.behavior_version)
        .unwrap();
    assert_eq!(prepared.timeouts, server.timeouts);
    assert_eq!(
        provider
            .prepare_import(&input, &server.id, &server.behavior_version)
            .unwrap_err()
            .code,
        "external_mcp.import_setup_required"
    );
}

#[test]
fn external_opencode_config_dir_is_a_user_scoped_late_override_and_keeps_global_sources() {
    let temp = TempDir::new().unwrap();
    let user = temp.path().join("user");
    let project = temp.path().join("project");
    let explicit = temp.path().join("explicit");
    fs::create_dir_all(&user).unwrap();
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::create_dir_all(&explicit).unwrap();
    fs::write(
        user.join("opencode.json"),
        r#"{"mcp":{"github":{"type":"remote","url":"https://global.example.test/mcp"}}}"#,
    )
    .unwrap();
    fs::write(
        project.join("opencode.json"),
        r#"{"mcp":{"github":{"url":"https://project.example.test/mcp"}}}"#,
    )
    .unwrap();
    fs::write(
        explicit.join("opencode.jsonc"),
        r#"{"mcp":{"github":{"url":"https://explicit.example.test/mcp"}}}"#,
    )
    .unwrap();
    let mut provider_options = options(user.clone());
    provider_options.config.explicit_config_dir = Some(explicit.clone());
    let provider = OpenCodeMcpProvider::new(provider_options);
    let snapshot = provider
        .discover(&ExternalMcpDiscoveryInput {
            context: context(project),
            suppressed_sources: BTreeSet::new(),
            revision_key: revision_key(),
        })
        .unwrap();
    let github = snapshot
        .servers
        .iter()
        .find(|server| server.name == "github")
        .unwrap();
    assert_eq!(
        github.remote_url_preview.as_deref(),
        Some("https://explicit.example.test/")
    );
    let explicit_source = snapshot
        .sources
        .iter()
        .find(|source| source.location == explicit.join("opencode.jsonc").to_string_lossy())
        .unwrap();
    assert_eq!(explicit_source.scope, ExternalSourceScope::UserGlobal);
    assert!(snapshot.sources.iter().any(|source| {
        source.location == user.join("opencode.json").to_string_lossy()
            && source.scope == ExternalSourceScope::UserGlobal
    }));
}
