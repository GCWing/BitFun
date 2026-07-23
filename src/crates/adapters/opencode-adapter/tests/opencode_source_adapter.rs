use bitfun_opencode_adapter::load_opencode_package_adapter;
use bitfun_plugin_runtime_host::PluginRuntimeHost;
use bitfun_product_domains::plugin_source::{PluginActivationAuthority, PluginPackageInput};
use bitfun_runtime_ports::{
    PluginCapabilityRef, PluginDataClassification, PluginDispatchEnvelope, PluginOwnerKind,
    PluginOwnerRef, PluginPayloadRedaction, PluginPayloadRef, PluginPermissionGate,
    PluginRuntimeClient, PluginRuntimeEpochs, PluginRuntimeReadRequest, PluginStatusKind,
    PluginTrustLevel,
};
use bitfun_services_integrations::plugin_source::{
    ManagedPluginSourceService, ManagedPluginTrustDecision,
};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process, time::SystemTime};

const PLUGIN_SOURCE: &str =
    include_str!("fixtures/opencode-example/.opencode/plugins/workspace-tools.ts");

fn custom_tool_source(prefix: &str, count: usize) -> String {
    let declarations = (0..count)
        .map(|index| format!("  {prefix}{index}: tool({{"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("export const DemoPlugin = async () => ({{\n{declarations}\n}})")
}

struct ManagedPackageFixture {
    root: PathBuf,
    workspace: PathBuf,
    package: PathBuf,
    service: ManagedPluginSourceService,
}

impl ManagedPackageFixture {
    fn new(name: &str, source: &str) -> Self {
        Self::new_with_path(name, source, ".opencode/plugins/workspace-tools.ts")
    }

    fn new_with_path(name: &str, source: &str, relative_path: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bitfun-opencode-managed-{name}-{}-{nonce}",
            process::id()
        ));
        let workspace = root.join("workspace");
        let user_data = root.join("user");
        let package = workspace.join(".bitfun/plugins/acme.demo");
        let plugin_path = package.join(relative_path.replace('/', std::path::MAIN_SEPARATOR_STR));
        fs::create_dir_all(plugin_path.parent().expect("plugin parent")).expect("create package");
        fs::create_dir_all(user_data.join("plugins")).expect("create user package root");

        fs::write(&plugin_path, source).expect("write plugin source");
        let hash = format!("sha256:{}", hex::encode(Sha256::digest(source.as_bytes())));
        let manifest = serde_json::json!({
            "schemaVersion": 1,
            "id": "acme.demo",
            "version": "1.0.0",
            "adapter": "opencode_compatible",
            "files": [{
                "path": relative_path,
                "sha256": hash
            }]
        });
        fs::write(
            package.join("bitfun.plugin.json"),
            serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");

        let service = ManagedPluginSourceService::new(
            user_data.join("plugins"),
            user_data.clone(),
            workspace.join(".bitfun/plugins"),
            workspace.clone(),
            user_data.join("runtime/plugin-trust.json"),
        );
        Self {
            root,
            workspace,
            package,
            service,
        }
    }

    fn add_opencode_config(&self, config: &str) {
        self.add_declared_file("opencode.json", config.as_bytes());
    }

    fn add_declared_file(&self, relative_path: &str, content: &[u8]) {
        let file_path = self
            .package
            .join(relative_path.replace('/', std::path::MAIN_SEPARATOR_STR));
        fs::create_dir_all(file_path.parent().expect("declared file parent"))
            .expect("create declared file parent");
        fs::write(&file_path, content).expect("write declared file");
        let manifest_path = self.package.join("bitfun.plugin.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).expect("read plugin manifest"))
                .expect("parse plugin manifest");
        manifest["files"]
            .as_array_mut()
            .expect("manifest files")
            .push(serde_json::json!({
                "path": relative_path,
                "sha256": format!(
                    "sha256:{}",
                    hex::encode(Sha256::digest(content))
                )
            }));
        fs::write(
            manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("serialize plugin manifest"),
        )
        .expect("update plugin manifest");
    }

    async fn approved_input(&self) -> bitfun_product_domains::plugin_source::PluginPackageInput {
        self.service.refresh(&self.workspace).await;
        self.service
            .set_trust(
                &self.workspace,
                "acme.demo",
                ManagedPluginTrustDecision::ApproveSource,
            )
            .await
            .expect("approve package source");
        self.service
            .load_package(&self.workspace, "acme.demo")
            .await
            .expect("load fixed package input")
    }

    async fn activated_input(&self) -> (PluginPackageInput, PluginActivationAuthority) {
        let input = self.approved_input().await;
        let content_hash = input.clone().into_parts().1.content_hash;
        self.service
            .activate(&self.workspace, "acme.demo", Some(&content_hash))
            .await
            .expect("activate package");
        self.service
            .load_activated_package(&self.workspace, "acme.demo")
            .await
            .expect("load activated package input")
    }
}

impl Drop for ManagedPackageFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn managed_package_is_read_through_plugin_runtime_host() {
    let fixture = ManagedPackageFixture::new("read", PLUGIN_SOURCE);
    let input = fixture.approved_input().await;
    let adapter = load_opencode_package_adapter(input, None, 1_720_000_001)
        .expect("create OpenCode package adapter")
        .0;
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .read_plugins(read_request())
        .await
        .expect("read package through host");

    assert_eq!(response.sources.len(), 1);
    assert!(response.sources[0]
        .plugin_id
        .starts_with("opencode.local.workspace_tools."));
    assert_eq!(response.sources[0].trust_level, PluginTrustLevel::Unknown);
    assert_eq!(
        response.plugin_statuses[0].status,
        PluginStatusKind::TrustRequired
    );
    assert!(response
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.trust_required"));
}

#[tokio::test]
async fn tool_hook_is_statically_mapped_in_read_diagnostics_without_execution() {
    let fixture = ManagedPackageFixture::new("hook-static-mapping", PLUGIN_SOURCE);
    let input = fixture.approved_input().await;
    let adapter = load_opencode_package_adapter(input, None, 1_720_000_001)
        .expect("create OpenCode package adapter")
        .0;
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .read_plugins(read_request())
        .await
        .expect("read package through host");
    let diagnostic = response
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "opencode.hook_mapped_runtime_unavailable")
        .expect("typed Hook mapping diagnostic");

    assert!(diagnostic.message.contains("tool.execute.before"));
    assert!(diagnostic.message.contains("read tool arguments"));
    assert!(diagnostic.message.contains("modify tool arguments"));
    assert_eq!(
        response.plugin_statuses[0].status,
        PluginStatusKind::TrustRequired
    );
}

#[tokio::test]
async fn single_line_expression_body_hook_is_statically_mapped() {
    let source = r#"export const InlinePlugin: Plugin = async () => ({ "tool.execute.after": async (_input, output) => output })"#;
    let fixture = ManagedPackageFixture::new("hook-inline-expression", source);
    let input = fixture.approved_input().await;
    let adapter = load_opencode_package_adapter(input, None, 1_720_000_001)
        .expect("create OpenCode package adapter")
        .0;
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .read_plugins(read_request())
        .await
        .expect("read package through host");
    let diagnostic = response
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "opencode.hook_mapped_runtime_unavailable")
        .expect("single-line Hook mapping diagnostic");

    assert!(diagnostic.message.contains("tool.execute.after"));
    assert!(diagnostic.message.contains("read tool results"));
    assert!(diagnostic.message.contains("modify tool results"));
}

#[tokio::test]
async fn event_name_references_do_not_become_typed_hook_contributions() {
    let source = r#"
export const EventObserverPlugin: Plugin = async () => {
  return {
    event: async ({ event }) => {
      if (event.type === "tool.execute.before") {
        console.log(event.type)
      }
    },
  }
}
"#;
    let fixture = ManagedPackageFixture::new("hook-event-reference", source);
    let input = fixture.approved_input().await;
    let adapter = load_opencode_package_adapter(input, None, 1_720_000_001)
        .expect("create OpenCode package adapter")
        .0;
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .read_plugins(read_request())
        .await
        .expect("read package through host");

    assert!(!response
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.hook_mapped_runtime_unavailable"));
    let generic_event_diagnostic = response
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "opencode.hook_projection_only")
        .expect("generic event Hook diagnostic");
    assert!(generic_event_diagnostic.message.ends_with("event"));
    assert!(!generic_event_diagnostic
        .message
        .contains("tool.execute.before"));
}

#[tokio::test]
async fn real_unsupported_hooks_are_reported_without_event_type_false_positives() {
    let source = r#"
export const UnsupportedHooksPlugin: Plugin = async () => ({
  config: async () => {},
  "chat.message": async () => {},
  "permission.ask": async () => {},
  "command.execute.before": async () => {},
  "tool.definition": async () => {},
  "permission.asked": async () => {},
})
"#;
    let fixture = ManagedPackageFixture::new("unsupported-hooks", source);
    let input = fixture.approved_input().await;
    let adapter = load_opencode_package_adapter(input, None, 1_720_000_001)
        .expect("create OpenCode package adapter")
        .0;
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .read_plugins(read_request())
        .await
        .expect("read package through host");
    let messages = response
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "opencode.hook_projection_only")
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    for hook in [
        "config",
        "chat.message",
        "permission.ask",
        "command.execute.before",
        "tool.definition",
    ] {
        assert!(messages.iter().any(|message| message.ends_with(hook)));
    }
    assert!(!messages
        .iter()
        .any(|message| message.contains("permission.asked")));
}

#[tokio::test]
async fn hook_parse_failure_is_explicit_without_guessing_contributions() {
    let source = r#"
export const BrokenHookPlugin: Plugin = async () => ({
  tool: {
    summary: tool({
      description: "Summarize",
    }),
  },
  "tool.execute.before": async () => {},
  broken: [
})
"#;
    let fixture = ManagedPackageFixture::new("hook-parse-failure", source);
    let input = fixture.approved_input().await;
    let adapter = load_opencode_package_adapter(input, None, 1_720_000_001)
        .expect("preserve custom-tool projection with Hook parse diagnostic")
        .0;
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .read_plugins(read_request())
        .await
        .expect("read package through host");

    assert!(response
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.hook_projection_parse_failed"));
    assert!(!response
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.hook_mapped_runtime_unavailable"));
}

#[tokio::test]
async fn source_approval_does_not_create_custom_tool_candidate() {
    let fixture = ManagedPackageFixture::new("approval", PLUGIN_SOURCE);
    let input = fixture.approved_input().await;
    let adapter = load_opencode_package_adapter(input, None, 1_720_000_001)
        .expect("create OpenCode package adapter")
        .0;
    let host = PluginRuntimeHost::new(adapter);
    let source = host
        .read_plugins(read_request())
        .await
        .expect("read package through host")
        .sources
        .into_iter()
        .next()
        .expect("plugin source");

    let response = host
        .dispatch(dispatch_envelope(source))
        .await
        .expect("dispatch remains readable");

    assert!(response.effects.is_empty());
    assert_eq!(
        response.plugin_statuses[0].status,
        PluginStatusKind::TrustRequired
    );
}

#[tokio::test]
async fn activated_package_projects_permission_required_candidate_through_host() {
    let fixture = ManagedPackageFixture::new("activated", PLUGIN_SOURCE);
    let (input, authority) = fixture.activated_input().await;
    let (project_domain_id, workspace_id, _, activation_epoch) = authority.clone().into_parts();
    let (adapter, dispatch_targets) =
        load_opencode_package_adapter(input, Some(authority), 1_720_000_001)
            .expect("create activated OpenCode package adapter");
    assert!(!dispatch_targets.is_empty());
    let host = PluginRuntimeHost::new(adapter);
    let request = read_request_for(&project_domain_id, &workspace_id, activation_epoch);
    let source = host
        .read_plugins(request)
        .await
        .expect("read activated package")
        .sources
        .into_iter()
        .next()
        .expect("activated source");

    assert_eq!(source.trust_level, PluginTrustLevel::Trusted);
    let response = host
        .dispatch(dispatch_envelope_for(
            source,
            &project_domain_id,
            &workspace_id,
            activation_epoch,
        ))
        .await
        .expect("dispatch activated package");

    assert!(!response.effects.is_empty());
    assert!(response.effects.iter().all(|effect| matches!(
        effect.permission,
        PluginPermissionGate::PermissionRequired { .. }
    )));
    assert!(response
        .effects
        .iter()
        .all(|effect| effect.risk_level == bitfun_runtime_ports::PluginRiskLevel::High));
}

#[tokio::test]
async fn activated_adapter_rejects_wrong_scope_and_epoch() {
    let fixture = ManagedPackageFixture::new("activation-scope", PLUGIN_SOURCE);
    let (input, authority) = fixture.activated_input().await;
    let (project_domain_id, workspace_id, _, activation_epoch) = authority.clone().into_parts();
    let host = PluginRuntimeHost::new(
        load_opencode_package_adapter(input, Some(authority), 1_720_000_001)
            .expect("create activated adapter")
            .0,
    );

    assert!(host
        .read_plugins(read_request_for(
            "wrong-project",
            &workspace_id,
            activation_epoch,
        ))
        .await
        .is_err());
    assert!(host
        .read_plugins(read_request_for(
            &project_domain_id,
            &workspace_id,
            activation_epoch + 1,
        ))
        .await
        .is_err());

    let source = host
        .read_plugins(read_request_for(
            &project_domain_id,
            &workspace_id,
            activation_epoch,
        ))
        .await
        .expect("read current activation")
        .sources
        .into_iter()
        .next()
        .expect("activated source");
    let stale = host
        .dispatch(dispatch_envelope_for(
            source.clone(),
            &project_domain_id,
            &workspace_id,
            activation_epoch + 1,
        ))
        .await
        .expect("stale activation returns a typed unavailable response");
    assert!(stale.effects.is_empty());
    assert!(stale.quarantine.is_none());
    assert!(stale
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.activation_stale"));

    let current = host
        .dispatch(dispatch_envelope_for(
            source,
            &project_domain_id,
            &workspace_id,
            activation_epoch,
        ))
        .await
        .expect("stale request does not quarantine the current activation");
    assert!(!current.effects.is_empty());
}

#[tokio::test]
async fn fixed_package_input_is_not_re_read_or_executed() {
    let fixture = ManagedPackageFixture::new(
        "immutable",
        "export const Plugin = async () => { throw new Error('must not execute') }",
    );
    let input = fixture.approved_input().await;
    fs::write(
        fixture.package.join(".opencode/plugins/workspace-tools.ts"),
        "this is not the reviewed source",
    )
    .expect("replace package file after fixed input was created");

    let adapter = load_opencode_package_adapter(input, None, 1_720_000_001)
        .expect("adapter reads only fixed input")
        .0;
    let host = PluginRuntimeHost::new(adapter);
    let response = host
        .read_plugins(read_request())
        .await
        .expect("read fixed package input");

    assert_eq!(response.sources.len(), 1);
    assert!(response
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "opencode.local_plugin_unreadable"));
}

#[tokio::test]
async fn package_input_rejects_content_changed_after_source_validation() {
    let fixture = ManagedPackageFixture::new("tampered-input", PLUGIN_SOURCE);
    let (manifest, source, mut files) = fixture.approved_input().await.into_parts();
    files
        .get_mut(".opencode/plugins/workspace-tools.ts")
        .expect("declared source")
        .extend_from_slice(b"\n// changed after validation");

    let error = PluginPackageInput::new(manifest, source, files)
        .expect_err("package input rejects changed content");

    assert!(error.to_string().contains("file hash does not match"));
}

#[tokio::test]
async fn package_input_rejects_forged_manifest_and_file_set() {
    let fixture = ManagedPackageFixture::new("forged-input", PLUGIN_SOURCE);
    let (mut manifest, source, mut files) = fixture.approved_input().await.into_parts();
    manifest.files.push(manifest.files[0].clone());
    files.insert(
        ".opencode/plugins/undeclared.ts".to_string(),
        b"export const Undeclared = async () => ({})".to_vec(),
    );

    assert!(PluginPackageInput::new(manifest, source, files).is_err());

    let (manifest, mut source, files) = fixture.approved_input().await.into_parts();
    source.content_hash = format!("sha256:{}", "0".repeat(64));
    assert!(PluginPackageInput::new(manifest, source, files).is_err());
}

#[tokio::test]
async fn revoked_source_cannot_create_a_new_fixed_package_input() {
    let fixture = ManagedPackageFixture::new("revoked-source", PLUGIN_SOURCE);
    fixture.approved_input().await;
    fixture
        .service
        .set_trust(
            &fixture.workspace,
            "acme.demo",
            ManagedPluginTrustDecision::Revoked,
        )
        .await
        .expect("revoke package source");

    let error = fixture
        .service
        .load_package(&fixture.workspace, "acme.demo")
        .await
        .expect_err("revoked package cannot be loaded");

    assert!(error.to_string().contains("source is not approved"));
}

#[tokio::test]
async fn corrupted_managed_package_is_reported_as_invalid() {
    let fixture = ManagedPackageFixture::new("corrupted-package", PLUGIN_SOURCE);
    fixture.approved_input().await;
    fs::write(
        fixture.package.join(".opencode/plugins/workspace-tools.ts"),
        "changed without updating the manifest",
    )
    .expect("corrupt package source");

    let error = fixture
        .service
        .load_package(&fixture.workspace, "acme.demo")
        .await
        .expect_err("corrupted package cannot be loaded");

    assert!(error.to_string().contains("hash_mismatch"));
}

#[tokio::test]
async fn invalid_plugin_diagnostic_keeps_managed_package_identity() {
    let fixture = ManagedPackageFixture::new(
        "invalid-plugin",
        "export const BrokenPlugin = async () => ({ name: 'broken' })",
    );
    let (manifest, source, files) = fixture.approved_input().await.into_parts();
    let expected_version = source.version.clone();
    let expected_hash = source.content_hash.clone();
    let input = PluginPackageInput::new(manifest, source, files).expect("rebuild package input");
    let adapter = load_opencode_package_adapter(input, None, 1_720_000_001)
        .expect("create adapter for invalid plugin")
        .0;
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .read_plugins(read_request())
        .await
        .expect("invalid plugin remains diagnosable");

    assert_eq!(
        response.sources[0].version.as_deref(),
        Some(expected_version.as_str())
    );
    assert_eq!(response.sources[0].content_hash, expected_hash);
    assert_eq!(
        response.plugin_statuses[0].status,
        PluginStatusKind::InvalidConfig
    );
}

#[tokio::test]
async fn invalid_config_diagnostics_are_isolated_by_managed_source() {
    let first = ManagedPackageFixture::new("invalid-config-a", PLUGIN_SOURCE);
    let second = ManagedPackageFixture::new("invalid-config-b", PLUGIN_SOURCE);
    first.add_opencode_config(r#"{"$schema":"https://invalid.example/config.json"}"#);
    second.add_opencode_config(r#"{"$schema":"https://invalid.example/config.json"}"#);

    let mut ids = Vec::new();
    let mut diagnostic_ids = Vec::new();
    for fixture in [&first, &second] {
        let host = PluginRuntimeHost::new(
            load_opencode_package_adapter(fixture.approved_input().await, None, 1_720_000_001)
                .expect("create adapter")
                .0,
        );
        let response = host
            .read_plugins(read_request())
            .await
            .expect("read invalid config");
        let diagnostic = response
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "opencode.config_invalid")
            .expect("config diagnostic");
        ids.push(diagnostic.source.plugin_id.clone());
        diagnostic_ids.push(diagnostic.diagnostic_id.clone());
    }

    assert_ne!(ids[0], ids[1]);
    assert_ne!(diagnostic_ids[0], diagnostic_ids[1]);
}

#[tokio::test]
async fn host_source_identity_distinguishes_managed_package_origins() {
    let first = ManagedPackageFixture::new("origin-a", PLUGIN_SOURCE);
    let second = ManagedPackageFixture::new("origin-b", PLUGIN_SOURCE);
    let first_host = PluginRuntimeHost::new(
        load_opencode_package_adapter(first.approved_input().await, None, 1_720_000_001)
            .expect("create first adapter")
            .0,
    );
    let second_host = PluginRuntimeHost::new(
        load_opencode_package_adapter(second.approved_input().await, None, 1_720_000_001)
            .expect("create second adapter")
            .0,
    );

    let first_source = first_host
        .read_plugins(read_request())
        .await
        .expect("read first source")
        .sources
        .remove(0);
    let second_source = second_host
        .read_plugins(read_request())
        .await
        .expect("read second source")
        .sources
        .remove(0);

    assert_ne!(first_source.source, second_source.source);
    assert_ne!(first_source.plugin_id, second_source.plugin_id);
    assert_eq!(first_source.content_hash, second_source.content_hash);
}

#[tokio::test]
async fn managed_source_uri_encodes_reserved_path_characters() {
    let fixture = ManagedPackageFixture::new_with_path(
        "reserved-uri",
        PLUGIN_SOURCE,
        ".opencode/plugins/nested #dir/workspace-tools.ts",
    );
    let host = PluginRuntimeHost::new(
        load_opencode_package_adapter(fixture.approved_input().await, None, 1_720_000_001)
            .expect("create adapter")
            .0,
    );

    let source = host
        .read_plugins(read_request())
        .await
        .expect("read source")
        .sources
        .remove(0)
        .source;

    assert!(source.starts_with("bitfun://managed-plugins/"));
    assert!(source.contains("nested%20%23dir"));
    assert!(!source.starts_with("file://"));
}

#[tokio::test]
async fn npm_projection_identity_distinguishes_managed_package_origins() {
    let config = r#"{"$schema":"https://opencode.ai/config.json","plugin":["same-plugin"]}"#;
    let first = ManagedPackageFixture::new("npm-origin-a", PLUGIN_SOURCE);
    let second = ManagedPackageFixture::new("npm-origin-b", PLUGIN_SOURCE);
    first.add_opencode_config(config);
    second.add_opencode_config(config);
    let first_host = PluginRuntimeHost::new(
        load_opencode_package_adapter(first.approved_input().await, None, 1_720_000_001)
            .expect("create first adapter")
            .0,
    );
    let second_host = PluginRuntimeHost::new(
        load_opencode_package_adapter(second.approved_input().await, None, 1_720_000_001)
            .expect("create second adapter")
            .0,
    );

    let first_source = first_host
        .read_plugins(read_request())
        .await
        .expect("read first source")
        .sources
        .into_iter()
        .find(|source| source.plugin_id.starts_with("opencode.npm.same_plugin."))
        .expect("first npm projection");
    let second_source = second_host
        .read_plugins(read_request())
        .await
        .expect("read second source")
        .sources
        .into_iter()
        .find(|source| source.plugin_id.starts_with("opencode.npm.same_plugin."))
        .expect("second npm projection");

    assert_ne!(first_source.source, second_source.source);
    assert_ne!(first_source.plugin_id, second_source.plugin_id);
    assert_eq!(first_source.content_hash, second_source.content_hash);
    assert_eq!(first_source.version.as_deref(), Some("1.0.0"));
    let digest = first_source
        .plugin_id
        .rsplit('.')
        .next()
        .expect("plugin id digest");
    assert_eq!(digest.len(), 32);
    assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));

    let repeated_host = PluginRuntimeHost::new(
        load_opencode_package_adapter(first.approved_input().await, None, 1_720_000_001)
            .expect("create repeated adapter")
            .0,
    );
    let repeated_source = repeated_host
        .read_plugins(read_request())
        .await
        .expect("read repeated source")
        .sources
        .into_iter()
        .find(|source| source.plugin_id.starts_with("opencode.npm.same_plugin."))
        .expect("repeated npm projection");
    assert_eq!(first_source.plugin_id, repeated_source.plugin_id);
}

#[tokio::test]
async fn managed_local_plugin_ids_distinguish_nested_and_dotted_paths() {
    let fixture = ManagedPackageFixture::new_with_path(
        "unique-local-ids",
        PLUGIN_SOURCE,
        ".opencode/plugins/a/foo.ts",
    );
    fixture.add_declared_file(".opencode/plugins/b/foo.ts", PLUGIN_SOURCE.as_bytes());
    fixture.add_declared_file(".opencode/plugins/foo.test.ts", PLUGIN_SOURCE.as_bytes());
    let host = PluginRuntimeHost::new(
        load_opencode_package_adapter(fixture.approved_input().await, None, 1_720_000_001)
            .expect("create adapter")
            .0,
    );

    let response = host
        .read_plugins(read_request())
        .await
        .expect("read local projections");
    let ids = response
        .sources
        .iter()
        .map(|source| source.plugin_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(response.sources.len(), 3);
    assert_eq!(ids.len(), 3);
}

#[tokio::test]
async fn npm_projection_ids_distinguish_collisions_and_deduplicate_exact_entries() {
    let fixture = ManagedPackageFixture::new("unique-npm-ids", PLUGIN_SOURCE);
    let long_name = "a".repeat(256);
    let config = serde_json::json!({
        "$schema": "https://opencode.ai/config.json",
        "plugin": ["foo-bar", "foo_bar", "foo-bar", long_name]
    });
    fixture.add_opencode_config(&serde_json::to_string(&config).expect("serialize config"));
    let host = PluginRuntimeHost::new(
        load_opencode_package_adapter(fixture.approved_input().await, None, 1_720_000_001)
            .expect("create adapter")
            .0,
    );

    let response = host
        .read_plugins(read_request())
        .await
        .expect("read npm projections");
    let npm_ids = response
        .sources
        .iter()
        .filter(|source| source.plugin_id.starts_with("opencode.npm."))
        .map(|source| source.plugin_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(npm_ids.len(), 3);
    assert!(npm_ids.iter().all(|plugin_id| plugin_id.len() <= 96));
}

#[tokio::test]
async fn package_without_recognized_opencode_entries_reports_diagnostic() {
    let fixture = ManagedPackageFixture::new_with_path("unsupported-layout", "notes", "README.md");
    let host = PluginRuntimeHost::new(
        load_opencode_package_adapter(fixture.approved_input().await, None, 1_720_000_001)
            .expect("create adapter")
            .0,
    );

    let response = host
        .read_plugins(read_request())
        .await
        .expect("read unsupported package layout");

    assert_eq!(response.sources.len(), 1);
    assert!(response
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "opencode.package_no_supported_entry" }));
}

#[tokio::test]
async fn managed_package_custom_tool_limit_is_cumulative() {
    let first = custom_tool_source("first", 128);
    let fixture = ManagedPackageFixture::new("custom-tool-budget", &first);
    let second = custom_tool_source("second", 128);
    fixture.add_declared_file(".opencode/plugins/second.ts", second.as_bytes());

    let boundary = fixture.approved_input().await;
    load_opencode_package_adapter(boundary, None, 1).expect("package boundary count");

    let overflow = custom_tool_source("overflow", 1);
    fixture.add_declared_file(".opencode/plugins/overflow.ts", overflow.as_bytes());
    let input = fixture.approved_input().await;
    let error = match load_opencode_package_adapter(input, None, 1) {
        Ok(_) => panic!("package cumulative declaration overflow"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("more than 256 custom tools"));
}

fn read_request() -> PluginRuntimeReadRequest {
    read_request_for("project-1", "workspace-1", 20)
}

fn read_request_for(
    project_domain_id: &str,
    workspace_id: &str,
    trust_epoch: u64,
) -> PluginRuntimeReadRequest {
    PluginRuntimeReadRequest {
        request_id: "read-managed-package".to_string(),
        project_domain_id: project_domain_id.to_string(),
        workspace_id: workspace_id.to_string(),
        plugin_ids: Vec::new(),
        include_config_validation: true,
        epochs: PluginRuntimeEpochs {
            trust_epoch,
            ..epochs()
        },
    }
}

fn dispatch_envelope(source: bitfun_runtime_ports::PluginSourceRef) -> PluginDispatchEnvelope {
    dispatch_envelope_for(source, "project-1", "workspace-1", 20)
}

fn dispatch_envelope_for(
    source: bitfun_runtime_ports::PluginSourceRef,
    project_domain_id: &str,
    workspace_id: &str,
    trust_epoch: u64,
) -> PluginDispatchEnvelope {
    PluginDispatchEnvelope {
        envelope_version: 1,
        event_id: "event-1".to_string(),
        project_domain_id: project_domain_id.to_string(),
        workspace_id: workspace_id.to_string(),
        source,
        event_type: "agent.turn.completed".to_string(),
        event_version: "2026-07-07".to_string(),
        extension_point_id: "tool".to_string(),
        declared_capability: PluginCapabilityRef {
            capability_id: "opencode.custom_tool".to_string(),
            owner: PluginOwnerRef {
                kind: PluginOwnerKind::ExtensionContract,
                id: "opencode.custom-tools".to_string(),
            },
        },
        deadline_ms: 30_000,
        idempotency_key: "managed-package-event-1".to_string(),
        correlation_id: "correlation-1".to_string(),
        causation_id: None,
        payload_ref: Some(PluginPayloadRef {
            payload_id: "payload-1".to_string(),
            schema_version: "agent.turn.completed.v1".to_string(),
            data_classification: PluginDataClassification::Workspace,
            redaction: PluginPayloadRedaction::Partial,
            uri: Some("bitfun://payloads/payload-1".to_string()),
        }),
        epochs: PluginRuntimeEpochs {
            trust_epoch,
            ..epochs()
        },
    }
}

fn epochs() -> PluginRuntimeEpochs {
    PluginRuntimeEpochs {
        project_epoch: 7,
        trust_epoch: 3,
        policy_epoch: 5,
        tool_registry_epoch: Some(11),
    }
}
