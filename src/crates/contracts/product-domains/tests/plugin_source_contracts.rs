use bitfun_product_domains::plugin_source::{
    PluginPackageManifest, PluginPackageSourceIdentity, PluginPackageTrustLevel,
    PluginTrustDecision, PluginTrustStore,
};

const HASH_A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn source(content_hash: &str, source_path: &str) -> PluginPackageSourceIdentity {
    PluginPackageSourceIdentity {
        package_id: "acme.demo".to_string(),
        version: "1.0.0".to_string(),
        adapter: "test_adapter".to_string(),
        source_path: source_path.to_string(),
        content_hash: content_hash.to_string(),
    }
}

#[test]
fn manifest_v1_accepts_only_normalized_declared_package_files() {
    let manifest = PluginPackageManifest::parse_json(&format!(
        r#"{{
          "schemaVersion": 1,
          "id": "acme.demo",
          "version": "1.0.0",
          "adapter": "test_adapter",
          "files": [
            {{"path": "plugin/main.ts", "sha256": "{HASH_A}"}}
          ]
        }}"#
    ))
    .expect("valid v1 manifest");

    assert_eq!(manifest.id, "acme.demo");
    assert_eq!(manifest.files.len(), 1);

    for invalid in [
        format!(
            r#"{{"schemaVersion":1,"id":"acme.demo","version":"1.0.0","adapter":"test_adapter","files":[{{"path":"../demo.ts","sha256":"{HASH_A}"}}]}}"#
        ),
        r#"{"schemaVersion":1,"id":"acme.demo","version":"1.0.0","adapter":"test_adapter","files":[{"path":"plugin/main.ts","sha256":"sha256:short"}]}"#.to_string(),
        format!(
            r#"{{"schemaVersion":1,"id":"acme.demo","version":"1.0.0","adapter":"OpenCode","files":[{{"path":"plugin/main.ts","sha256":"{HASH_A}"}}]}}"#
        ),
        format!(
            r#"{{"schemaVersion":1,"id":"acme.demo","version":"1.0.0","adapter":"test_adapter","files":[{{"path":"plugin/main.ts","sha256":"{HASH_A}"}}],"futureField":true}}"#
        ),
    ] {
        assert!(
            PluginPackageManifest::parse_json(&invalid).is_err(),
            "invalid manifest must fail closed: {invalid}"
        );
    }
}

#[test]
fn manifest_and_trust_identity_reject_terminal_spoofing_characters() {
    let manifest = format!(
        r#"{{"schemaVersion":1,"id":"acme.demo","version":"1.0\nforged","adapter":"test_adapter","files":[{{"path":"plugin/main.ts","sha256":"{HASH_A}"}}]}}"#
    );
    assert!(PluginPackageManifest::parse_json(&manifest).is_err());
    let bidi_manifest = serde_json::json!({
        "schemaVersion": 1,
        "id": "acme.demo",
        "version": "1.0.0\u{202e}source-approved",
        "adapter": "test_adapter",
        "files": [{"path": "plugin/main.ts", "sha256": HASH_A}],
    });
    assert!(PluginPackageManifest::parse_json(&bidi_manifest.to_string()).is_err());

    let trust_store = serde_json::json!({
        "schemaVersion": 1,
        "epoch": 2,
        "records": [{
            "projectDomainId": "project-1",
            "workspaceId": "workspace-1",
            "source": {
                "packageId": "acme.demo",
                "version": "1.0.0",
                "adapter": "test_adapter",
                "sourcePath": "path:unix:\u{1b}]8;;forged",
                "contentHash": HASH_A
            },
            "trustLevel": "source_approved",
            "updatedAtMs": 100
        }]
    });
    let store: PluginTrustStore =
        serde_json::from_value(trust_store).expect("deserialize trust fixture");
    assert!(store.validate().is_err());
}

#[test]
fn trust_store_invalidates_changed_package_identity_and_advances_epoch_once() {
    let mut store = PluginTrustStore::new(1);
    let original = source(HASH_A, "file:///workspace/.bitfun/plugins/acme.demo");

    assert_eq!(store.epoch(), 1);
    assert_eq!(
        store.trust_level_for("project-1", "workspace-1", &original),
        PluginPackageTrustLevel::Unknown
    );

    assert!(store
        .apply_decision(
            "project-1",
            "workspace-1",
            original.clone(),
            PluginTrustDecision::ApproveSource,
            100,
        )
        .expect("trust decision"));
    assert_eq!(store.epoch(), 2);
    assert_eq!(
        store.trust_level_for("project-1", "workspace-1", &original),
        PluginPackageTrustLevel::SourceApproved
    );

    assert!(!store
        .apply_decision(
            "project-1",
            "workspace-1",
            original.clone(),
            PluginTrustDecision::ApproveSource,
            101,
        )
        .expect("idempotent trust decision"));
    assert_eq!(store.epoch(), 2);

    let changed = source(HASH_B, "file:///workspace/.bitfun/plugins/acme.demo");
    assert!(store
        .reconcile_sources("project-1", "workspace-1", std::slice::from_ref(&changed))
        .expect("reconcile changed source"));
    assert_eq!(store.epoch(), 3);
    assert_eq!(
        store.trust_level_for("project-1", "workspace-1", &changed),
        PluginPackageTrustLevel::Unknown
    );
    assert_eq!(
        store.trust_level_for("project-1", "workspace-1", &original),
        PluginPackageTrustLevel::Unknown
    );

    assert!(!store
        .reconcile_sources("project-1", "workspace-1", &[changed])
        .expect("repeated reconcile"));
    assert_eq!(store.epoch(), 3);
}

#[test]
fn trust_decisions_are_scoped_to_project_and_workspace() {
    let mut store = PluginTrustStore::new(1);
    let package = source(HASH_A, "file:///workspace/.bitfun/plugins/acme.demo");

    store
        .apply_decision(
            "project-1",
            "workspace-1",
            package.clone(),
            PluginTrustDecision::Denied,
            100,
        )
        .expect("deny source");

    assert_eq!(
        store.trust_level_for("project-1", "workspace-1", &package),
        PluginPackageTrustLevel::Denied
    );
    assert_eq!(
        store.trust_level_for("project-1", "workspace-2", &package),
        PluginPackageTrustLevel::Unknown
    );
    assert_eq!(
        store.trust_level_for("project-2", "workspace-1", &package),
        PluginPackageTrustLevel::Unknown
    );
}

#[test]
fn revoke_requires_an_existing_source_approval() {
    let mut store = PluginTrustStore::new(1);
    let package = source(HASH_A, "file:///workspace/.bitfun/plugins/acme.demo");

    assert!(store
        .apply_decision(
            "project-1",
            "workspace-1",
            package.clone(),
            PluginTrustDecision::Revoked,
            100,
        )
        .is_err());
    store
        .apply_decision(
            "project-1",
            "workspace-1",
            package.clone(),
            PluginTrustDecision::ApproveSource,
            101,
        )
        .expect("trust source");
    assert!(store
        .apply_decision(
            "project-1",
            "workspace-1",
            package,
            PluginTrustDecision::Revoked,
            102,
        )
        .expect("revoke source-approved package"));
}

#[test]
fn trust_store_rejects_unknown_schema_and_duplicate_identity_records() {
    let unknown_schema = r#"{
      "schemaVersion": 2,
      "epoch": 1,
      "records": []
    }"#;
    let unknown_schema: PluginTrustStore =
        serde_json::from_str(unknown_schema).expect("deserialize unknown schema");
    assert!(unknown_schema.validate().is_err());

    let identity = serde_json::to_value(source(
        HASH_A,
        "file:///workspace/.bitfun/plugins/acme.demo",
    ))
    .expect("serialize source identity");
    let duplicate_records = serde_json::json!({
        "schemaVersion": 1,
        "epoch": 2,
        "records": [
            {
                "projectDomainId": "project-1",
                "workspaceId": "workspace-1",
                "source": identity.clone(),
                "trustLevel": "source_approved",
                "updatedAtMs": 100
            },
            {
                "projectDomainId": "project-1",
                "workspaceId": "workspace-1",
                "source": {
                    "packageId": "acme.demo",
                    "version": "2.0.0",
                    "adapter": "test_adapter",
                    "sourcePath": "file:///workspace/.bitfun/plugins/acme.demo",
                    "contentHash": HASH_B
                },
                "trustLevel": "denied",
                "updatedAtMs": 101
            }
        ]
    });

    let duplicate_records: PluginTrustStore =
        serde_json::from_value(duplicate_records).expect("deserialize duplicate records");
    assert!(duplicate_records.validate().is_err());
}
