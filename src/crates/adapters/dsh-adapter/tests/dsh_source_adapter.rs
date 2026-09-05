use openbitfun_dsh_adapter::load_dsh_package_adapter;
use openbitfun_plugin_runtime_client::DefaultPluginRuntimeClient;
use openbitfun_product_domains::plugin_source::{
    PluginPackageFile, PluginPackageInput, PluginPackageManifest, PluginPackageSourceIdentity,
    PluginTrustDecision, PluginTrustStore,
};
use openbitfun_runtime_ports::{
    PluginCapabilityRef, PluginDispatchEnvelope, PluginOwnerKind, PluginOwnerRef,
    PluginRuntimeAvailability, PluginRuntimeClient, PluginRuntimeEpochs, PluginRuntimeReadRequest,
    PluginRuntimeUnavailableReason, PluginSourceKind, PluginStatusKind, PluginTrustLevel,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const BUNDLE_PACKAGE_JSON: &str = r#"{
  "name": "acme-dsh-bundle",
  "version": "1.0.0",
  "dsh": { "bundle": { "patch": "./cordis.patch.yml" } }
}"#;

const BUNDLE_PATCH: &str = r#"
- insert:
    - id: acme.tool
      disabled: !!js process.platform === 'win32'
      config:
        command: [node, tool.js]
    - id: acme.agent
      name: group
      group: true
      config:
        - id: acme.agent.inner
          name: acme-agent-inner
"#;

const PROFILE_PACKAGE_JSON: &str = r#"{
  "name": "acme-dsh-profile",
  "version": "1.0.0",
  "dsh": { "profile": { "bundles": ["@acme/dsh-base", "@acme/dsh-web"] } }
}"#;

const BUNDLE_AND_PROFILE_PACKAGE_JSON: &str = r#"{
  "name": "acme-dsh-combined",
  "version": "1.0.0",
  "dsh": {
    "bundle": { "patch": "./cordis.patch.yml" },
    "profile": { "bundles": ["@acme/dsh-base", "@acme/dsh-web"] }
  }
}"#;

fn build_input(files: &[(&str, &str)]) -> PluginPackageInput {
    let manifest = PluginPackageManifest {
        schema_version: 1,
        id: "acme.dsh".to_string(),
        version: "1.0.0".to_string(),
        adapter: "dsh_compatible".to_string(),
        files: files
            .iter()
            .map(|(path, content)| PluginPackageFile {
                path: (*path).to_string(),
                sha256: format!("sha256:{}", hex::encode(Sha256::digest(content.as_bytes()))),
            })
            .collect(),
    };
    let source = PluginPackageSourceIdentity {
        package_id: "acme.dsh".to_string(),
        version: "1.0.0".to_string(),
        adapter: "dsh_compatible".to_string(),
        source_path: "/managed/acme.dsh".to_string(),
        content_hash: manifest.content_hash().expect("manifest content hash"),
    };
    let file_map = files
        .iter()
        .map(|(path, content)| ((*path).to_string(), content.as_bytes().to_vec()))
        .collect::<BTreeMap<_, _>>();
    PluginPackageInput::new(manifest, source, file_map).expect("valid package input")
}

fn bundle_input() -> PluginPackageInput {
    build_input(&[
        ("package.json", BUNDLE_PACKAGE_JSON),
        ("cordis.patch.yml", BUNDLE_PATCH),
    ])
}

fn read_request(plugin_ids: Vec<String>) -> PluginRuntimeReadRequest {
    PluginRuntimeReadRequest {
        request_id: "dsh-read".to_string(),
        project_domain_id: "project".to_string(),
        workspace_id: "workspace".to_string(),
        plugin_ids,
        include_config_validation: true,
        epochs: PluginRuntimeEpochs {
            project_epoch: 0,
            trust_epoch: 1,
            policy_epoch: 0,
            tool_registry_epoch: None,
        },
    }
}

fn dispatch_envelope(source: openbitfun_runtime_ports::PluginSourceRef) -> PluginDispatchEnvelope {
    PluginDispatchEnvelope {
        envelope_version: 1,
        event_id: "dispatch-1".to_string(),
        event_type: "plugin.dispatch.requested".to_string(),
        event_version: "v1".to_string(),
        project_domain_id: "project".to_string(),
        workspace_id: "workspace".to_string(),
        extension_point_id: "tool".to_string(),
        declared_capability: PluginCapabilityRef {
            capability_id: "dsh.entry".to_string(),
            owner: PluginOwnerRef {
                kind: PluginOwnerKind::ExtensionContract,
                id: "dsh".to_string(),
            },
        },
        source,
        correlation_id: "correlation".to_string(),
        causation_id: None,
        idempotency_key: "idem-1".to_string(),
        deadline_ms: 30_000,
        epochs: PluginRuntimeEpochs {
            project_epoch: 0,
            trust_epoch: 1,
            policy_epoch: 0,
            tool_registry_epoch: None,
        },
        payload_ref: None,
    }
}

fn activation_authority(
    input: &PluginPackageInput,
) -> openbitfun_product_domains::plugin_source::PluginActivationAuthority {
    let source = input.clone().into_parts().1;
    let mut trust = PluginTrustStore::new(1);
    trust
        .apply_decision(
            "project",
            "workspace",
            source.clone(),
            PluginTrustDecision::ApproveSource,
            1_720_000_000,
        )
        .expect("approve dsh source");
    trust
        .activate("project", "workspace", source.clone(), 1_720_000_001)
        .expect("activate dsh source");
    trust
        .activation_authority("project", "workspace", &source)
        .expect("dsh activation authority")
}

#[tokio::test]
async fn bundle_entries_project_as_projection_only_sources() {
    let (adapter, dispatch_targets) =
        load_dsh_package_adapter(bundle_input(), None, 1_720_000_001).expect("dsh adapter");
    assert!(
        dispatch_targets.is_empty(),
        "dsh entries have no tool targets"
    );

    let client = DefaultPluginRuntimeClient::new(adapter);
    assert_eq!(
        client.availability(),
        PluginRuntimeAvailability::ProjectionOnly {
            reason: PluginRuntimeUnavailableReason::HostUnavailable
        }
    );

    let read = client
        .read_plugins(read_request(Vec::new()))
        .await
        .expect("read plugins");
    assert_eq!(read.sources.len(), 3, "two top-level + one nested entry");
    assert!(read
        .sources
        .iter()
        .all(|source| source.source_kind == PluginSourceKind::DeepSeekHarnessCompatible));
    assert!(read
        .sources
        .iter()
        .all(|source| source.trust_level == PluginTrustLevel::Unknown));
    assert!(read
        .plugin_statuses
        .iter()
        .all(|status| status.status == PluginStatusKind::TrustRequired));
    assert!(read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "dsh.bundle_entry_projection_only"));

    let source = read.sources[0].clone();
    let response = client
        .dispatch(dispatch_envelope(source))
        .await
        .expect("dispatch projection");
    assert!(
        response.effects.is_empty(),
        "static adapter produces no effects"
    );
    assert_eq!(
        response.plugin_statuses[0].status,
        PluginStatusKind::TrustRequired
    );
}

#[tokio::test]
async fn profile_bundles_project_as_projection_only_sources() {
    let input = build_input(&[("package.json", PROFILE_PACKAGE_JSON)]);
    let (adapter, _) = load_dsh_package_adapter(input, None, 1_720_000_001).expect("dsh adapter");
    let client = DefaultPluginRuntimeClient::new(adapter);

    let read = client
        .read_plugins(read_request(Vec::new()))
        .await
        .expect("read plugins");
    assert_eq!(read.sources.len(), 2);
    assert!(read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "dsh.profile_bundle_projection_only"));
}

#[tokio::test]
async fn invalid_profile_names_are_diagnosed_and_duplicate_layers_are_preserved() {
    let package_json = r#"{
      "name": "acme-dsh-profile",
      "version": "1.0.0",
      "dsh": {
        "profile": {
          "bundles": ["@acme/dsh-base", " ", "@acme/dsh-base"]
        }
      }
    }"#;
    let input = build_input(&[("package.json", package_json)]);
    let (adapter, _) = load_dsh_package_adapter(input, None, 1_720_000_001).unwrap();
    let client = DefaultPluginRuntimeClient::new(adapter);

    let read = client.read_plugins(read_request(Vec::new())).await.unwrap();
    assert_eq!(
        read.sources.len(),
        3,
        "two ordered layer references and one invalid projection"
    );
    assert!(read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "dsh.profile_bundle_invalid"));
    assert_eq!(
        read.plugin_statuses
            .iter()
            .filter(|status| status.status == PluginStatusKind::InvalidConfig)
            .count(),
        1
    );
    let valid_sources = read
        .plugin_statuses
        .iter()
        .filter(|status| status.status == PluginStatusKind::TrustRequired)
        .map(|status| &status.source)
        .collect::<Vec<_>>();
    assert_eq!(valid_sources.len(), 2);
    assert_ne!(valid_sources[0].plugin_id, valid_sources[1].plugin_id);
}

#[tokio::test]
async fn package_can_project_bundle_and_profile_roles_together() {
    let input = build_input(&[
        ("package.json", BUNDLE_AND_PROFILE_PACKAGE_JSON),
        ("cordis.patch.yml", BUNDLE_PATCH),
    ]);
    let (adapter, _) = load_dsh_package_adapter(input, None, 1_720_000_001).unwrap();
    let client = DefaultPluginRuntimeClient::new(adapter);

    let read = client
        .read_plugins(read_request(Vec::new()))
        .await
        .expect("read combined dsh package");
    assert_eq!(read.sources.len(), 5, "three patch rows plus two bundles");
    assert!(read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "dsh.bundle_entry_projection_only"));
    assert!(read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "dsh.profile_bundle_projection_only"));
}

#[tokio::test]
async fn read_plugins_filters_by_plugin_id() {
    let (adapter, _) = load_dsh_package_adapter(bundle_input(), None, 1_720_000_001).unwrap();
    let client = DefaultPluginRuntimeClient::new(adapter);
    let all = client.read_plugins(read_request(Vec::new())).await.unwrap();
    let target = all.sources[0].plugin_id.clone();

    let filtered = client
        .read_plugins(read_request(vec![target.clone()]))
        .await
        .unwrap();
    assert_eq!(filtered.sources.len(), 1);
    assert_eq!(filtered.sources[0].plugin_id, target);
}

#[tokio::test]
async fn missing_package_json_produces_invalid_projection() {
    let input = build_input(&[("cordis.patch.yml", BUNDLE_PATCH)]);
    let expected_source = input.clone().into_parts().1;
    let (adapter, _) = load_dsh_package_adapter(input, None, 1_720_000_001).unwrap();
    let client = DefaultPluginRuntimeClient::new(adapter);
    let read = client.read_plugins(read_request(Vec::new())).await.unwrap();
    assert_eq!(read.sources.len(), 1);
    assert_eq!(read.sources[0].version.as_deref(), Some("1.0.0"));
    assert_eq!(read.sources[0].content_hash, expected_source.content_hash);
    assert_eq!(
        read.plugin_statuses[0].status,
        PluginStatusKind::InvalidConfig
    );
    assert!(read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "dsh.package_json_missing"));
}

#[tokio::test]
async fn bundle_declaration_requires_an_explicit_patch_path() {
    let package_json = r#"{
      "name": "acme-dsh-bundle",
      "version": "1.0.0",
      "dsh": { "bundle": {} }
    }"#;
    let input = build_input(&[
        ("package.json", package_json),
        ("cordis.patch.yml", BUNDLE_PATCH),
    ]);
    let (adapter, _) = load_dsh_package_adapter(input, None, 1_720_000_001).unwrap();
    let client = DefaultPluginRuntimeClient::new(adapter);
    let read = client.read_plugins(read_request(Vec::new())).await.unwrap();

    assert_eq!(read.sources.len(), 1);
    assert_eq!(
        read.plugin_statuses[0].status,
        PluginStatusKind::InvalidConfig
    );
    assert!(read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "dsh.bundle_patch_missing"));
}

#[tokio::test]
async fn bundle_patch_path_uses_package_relative_normalization() {
    let package_json = r#"{
      "name": "acme-dsh-bundle",
      "version": "1.0.0",
      "dsh": { "bundle": { "patch": "./config/../cordis.patch.yml" } }
    }"#;
    let input = build_input(&[
        ("package.json", package_json),
        ("cordis.patch.yml", BUNDLE_PATCH),
    ]);
    let (adapter, _) = load_dsh_package_adapter(input, None, 1_720_000_001).unwrap();
    let client = DefaultPluginRuntimeClient::new(adapter);
    let read = client.read_plugins(read_request(Vec::new())).await.unwrap();

    assert_eq!(read.sources.len(), 3);
    assert!(read
        .plugin_statuses
        .iter()
        .all(|status| status.status == PluginStatusKind::TrustRequired));
}

#[tokio::test]
async fn package_without_dsh_field_produces_invalid_projection() {
    let input = build_input(&[("package.json", r#"{ "name": "plain" }"#)]);
    let (adapter, _) = load_dsh_package_adapter(input, None, 1_720_000_001).unwrap();
    let client = DefaultPluginRuntimeClient::new(adapter);
    let read = client.read_plugins(read_request(Vec::new())).await.unwrap();
    assert_eq!(
        read.plugin_statuses[0].status,
        PluginStatusKind::InvalidConfig
    );
    assert!(read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "dsh.package_no_dsh_field"));
}

#[tokio::test]
async fn unsupported_client_only_package_reports_its_actual_role() {
    let input = build_input(&[(
        "package.json",
        r#"{
          "name": "client-only",
          "dsh": {
            "client": { "platform": "web", "inject": [] }
          }
        }"#,
    )]);
    let (adapter, _) = load_dsh_package_adapter(input, None, 1_720_000_001).unwrap();
    let client = DefaultPluginRuntimeClient::new(adapter);
    let read = client.read_plugins(read_request(Vec::new())).await.unwrap();

    assert_eq!(
        read.plugin_statuses[0].status,
        PluginStatusKind::InvalidConfig
    );
    assert!(read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "dsh.package_no_supported_role"));
    assert!(!read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "dsh.package_no_dsh_field"));
}

#[tokio::test]
async fn bundle_with_missing_patch_produces_invalid_projection() {
    let input = build_input(&[("package.json", BUNDLE_PACKAGE_JSON)]);
    let (adapter, _) = load_dsh_package_adapter(input, None, 1_720_000_001).unwrap();
    let client = DefaultPluginRuntimeClient::new(adapter);
    let read = client.read_plugins(read_request(Vec::new())).await.unwrap();
    assert_eq!(
        read.plugin_statuses[0].status,
        PluginStatusKind::InvalidConfig
    );
    assert!(read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "dsh.bundle_patch_missing"));
}

#[tokio::test]
async fn empty_bundle_patch_produces_invalid_projection() {
    let input = build_input(&[
        ("package.json", BUNDLE_PACKAGE_JSON),
        ("cordis.patch.yml", "# comments only\n"),
    ]);
    let (adapter, _) = load_dsh_package_adapter(input, None, 1_720_000_001).unwrap();
    let client = DefaultPluginRuntimeClient::new(adapter);
    let read = client.read_plugins(read_request(Vec::new())).await.unwrap();
    assert_eq!(
        read.plugin_statuses[0].status,
        PluginStatusKind::InvalidConfig
    );
    assert!(read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "dsh.bundle_no_entries"));
}

#[tokio::test]
async fn bundle_entries_without_stable_ids_are_not_silently_dropped() {
    let patch = r#"
- insert:
    - id: valid-entry
    - id: ""
"#;
    let input = build_input(&[
        ("package.json", BUNDLE_PACKAGE_JSON),
        ("cordis.patch.yml", patch),
    ]);
    let (adapter, _) = load_dsh_package_adapter(input, None, 1_720_000_001).unwrap();
    let client = DefaultPluginRuntimeClient::new(adapter);
    let read = client.read_plugins(read_request(Vec::new())).await.unwrap();

    assert_eq!(
        read.sources.len(),
        2,
        "one valid and one invalid projection"
    );
    assert!(read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "dsh.bundle_entry_invalid"));
}

#[tokio::test]
async fn invalid_package_cannot_be_activated() {
    let input = build_input(&[("package.json", r#"{ "name": "plain" }"#)]);
    let authority = activation_authority(&input);
    let error = load_dsh_package_adapter(input, Some(authority), 1_720_000_001)
        .err()
        .expect("invalid dsh package must not activate");
    assert!(error.to_string().contains("invalid dsh package projection"));
}

#[tokio::test]
async fn wrong_adapter_is_rejected() {
    let manifest = PluginPackageManifest {
        schema_version: 1,
        id: "acme.dsh".to_string(),
        version: "1.0.0".to_string(),
        adapter: "opencode_compatible".to_string(),
        files: vec![PluginPackageFile {
            path: "package.json".to_string(),
            sha256: format!(
                "sha256:{}",
                hex::encode(Sha256::digest(BUNDLE_PACKAGE_JSON.as_bytes()))
            ),
        }],
    };
    let source = PluginPackageSourceIdentity {
        package_id: "acme.dsh".to_string(),
        version: "1.0.0".to_string(),
        adapter: "opencode_compatible".to_string(),
        source_path: "/managed/acme.dsh".to_string(),
        content_hash: manifest.content_hash().unwrap(),
    };
    let files = BTreeMap::from([(
        "package.json".to_string(),
        BUNDLE_PACKAGE_JSON.as_bytes().to_vec(),
    )]);
    let input = PluginPackageInput::new(manifest, source, files).unwrap();

    let error = load_dsh_package_adapter(input, None, 1)
        .err()
        .expect("wrong adapter must fail");
    assert!(error.to_string().contains("not dsh-compatible"));
}
