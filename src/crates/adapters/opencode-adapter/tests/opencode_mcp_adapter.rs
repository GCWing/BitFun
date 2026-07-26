use bitfun_opencode_adapter::{OpenCodeMcpProvider, OpenCodeMcpProviderOptions};
use bitfun_product_domains::external_sources::{
    ExecutionDomainId, ExternalMcpDiscoveryInput, ExternalMcpSourceProvider,
    ExternalMcpStaticStatus, ExternalMcpTransportKind, ExternalSourceContext, ExternalSourceScope,
    PreparedExternalMcpTransport,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn context(workspace_root: PathBuf) -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: Some(workspace_root),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

fn options(user_config_dir: PathBuf) -> OpenCodeMcpProviderOptions {
    OpenCodeMcpProviderOptions {
        user_config_dir,
        legacy_user_config_dir: None,
        explicit_config_file: None,
        explicit_config_dir: None,
        project_config_enabled: true,
        project_root_override: None,
    }
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
    };
    let snapshot = provider.discover(&input).unwrap();

    assert!(!marker.exists(), "discovery must remain static");
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
            "custom-timeout": {"type":"remote","url":"https://example.test/mcp","timeout":1000},
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
        "custom-timeout",
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
fn opencode_config_dir_is_a_global_late_override_like_the_source_application() {
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
    let mut provider_options = options(user);
    provider_options.explicit_config_dir = Some(explicit.clone());
    let provider = OpenCodeMcpProvider::new(provider_options);
    let snapshot = provider
        .discover(&ExternalMcpDiscoveryInput {
            context: context(project),
            suppressed_sources: BTreeSet::new(),
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
}
