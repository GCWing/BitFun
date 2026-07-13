use bitfun_product_domains::plugin_source::{
    PluginPackageInput, PluginPackageManifest, PluginPackageSourceIdentity,
    PluginPackageTrustLevel, PluginTrustDecision, PluginTrustStore,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const HASH_A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const PROJECT: &str = "project-1";
const WORKSPACE: &str = "workspace-1";
const SOURCE_PATH: &str = "file:///workspace/.bitfun/plugins/acme.demo";

fn source(content_hash: &str, source_path: &str) -> PluginPackageSourceIdentity {
    PluginPackageSourceIdentity {
        package_id: "acme.demo".to_string(),
        version: "1.0.0".to_string(),
        adapter: "test_adapter".to_string(),
        source_path: source_path.to_string(),
        content_hash: content_hash.to_string(),
    }
}

fn approve_source(store: &mut PluginTrustStore, package: &PluginPackageSourceIdentity) {
    store
        .apply_decision(
            PROJECT,
            WORKSPACE,
            package.clone(),
            PluginTrustDecision::ApproveSource,
            100,
        )
        .expect("approve source");
}

fn activate_source(store: &mut PluginTrustStore, package: &PluginPackageSourceIdentity) {
    store
        .activate(PROJECT, WORKSPACE, package.clone(), 101)
        .expect("activate source");
}

fn fixed_package_input(source_path: &str) -> (PluginPackageInput, PluginPackageSourceIdentity) {
    let bytes = b"export const Demo = async () => ({})".to_vec();
    let file_hash = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    let manifest = PluginPackageManifest::parse_json(&format!(
        r#"{{"schemaVersion":1,"id":"acme.demo","version":"1.0.0","adapter":"test_adapter","files":[{{"path":"plugin/main.ts","sha256":"{file_hash}"}}]}}"#
    ))
    .expect("valid manifest");
    let package = source(
        &manifest.content_hash().expect("manifest hash"),
        source_path,
    );
    let input = PluginPackageInput::new(
        manifest,
        package.clone(),
        BTreeMap::from([("plugin/main.ts".to_string(), bytes)]),
    )
    .expect("valid fixed package input");
    (input, package)
}

fn trust_record(
    package: &PluginPackageSourceIdentity,
    trust_level: PluginPackageTrustLevel,
) -> serde_json::Value {
    serde_json::json!({
        "projectDomainId": PROJECT,
        "workspaceId": WORKSPACE,
        "source": package,
        "trustLevel": trust_level,
        "updatedAtMs": 100
    })
}

fn activation_record(package: &PluginPackageSourceIdentity) -> serde_json::Value {
    serde_json::json!({
        "projectDomainId": PROJECT,
        "workspaceId": WORKSPACE,
        "source": package,
        "activationEpoch": 2,
        "updatedAtMs": 101
    })
}

fn schema_v2_store(
    records: Vec<serde_json::Value>,
    activation_records: Vec<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 2,
        "epoch": 2,
        "records": records,
        "activationEpoch": 2,
        "activationRecords": activation_records
    })
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
fn fixed_package_input_enforces_file_set_hash_and_size_limits() {
    let bytes = b"export const Demo = async () => ({})".to_vec();
    let file_hash = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    let manifest = PluginPackageManifest::parse_json(&format!(
        r#"{{"schemaVersion":1,"id":"acme.demo","version":"1.0.0","adapter":"test_adapter","files":[{{"path":"plugin/main.ts","sha256":"{file_hash}"}}]}}"#
    ))
    .expect("valid manifest");
    let identity = source(
        &manifest.content_hash().expect("manifest hash"),
        "native:workspace-source",
    );
    let files = BTreeMap::from([("plugin/main.ts".to_string(), bytes)]);

    PluginPackageInput::new(manifest.clone(), identity.clone(), files.clone())
        .expect("valid fixed input");

    let mut extra = files.clone();
    extra.insert("plugin/extra.ts".to_string(), Vec::new());
    assert!(PluginPackageInput::new(manifest.clone(), identity.clone(), extra).is_err());

    let oversized = BTreeMap::from([("plugin/main.ts".to_string(), vec![0; 1024 * 1024 + 1])]);
    assert!(PluginPackageInput::new(manifest, identity, oversized).is_err());
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
fn absent_sources_preserve_review_history_until_a_replacement_is_discovered() {
    let original = source(HASH_A, SOURCE_PATH);
    let replacement = source(HASH_B, SOURCE_PATH);
    let mut store = PluginTrustStore::new(1);
    approve_source(&mut store, &original);
    activate_source(&mut store, &original);
    let epochs = (store.epoch(), store.activation_epoch());

    assert!(!store
        .reconcile_sources(PROJECT, WORKSPACE, &[])
        .expect("reconcile absent source"));
    assert_eq!((store.epoch(), store.activation_epoch()), epochs);
    assert_eq!(
        store.trust_level_for(PROJECT, WORKSPACE, &original),
        PluginPackageTrustLevel::SourceApproved
    );
    assert!(store.is_activated(PROJECT, WORKSPACE, &original));
    assert_eq!(
        store.activation_sources(PROJECT, WORKSPACE),
        vec![original.clone()]
    );

    assert!(store
        .reconcile_sources(PROJECT, WORKSPACE, &[replacement])
        .expect("reconcile replacement source"));
    assert_eq!(
        store.trust_level_for(PROJECT, WORKSPACE, &original),
        PluginPackageTrustLevel::Unknown
    );
    assert!(!store.is_activated(PROJECT, WORKSPACE, &original));
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
      "schemaVersion": 3,
      "epoch": 1,
      "records": [],
      "activationEpoch": 1,
      "activationRecords": []
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

#[test]
fn activation_lifecycle_is_exact_independent_and_idempotent() {
    let package = source(HASH_A, SOURCE_PATH);
    let mut store = PluginTrustStore::new(7);
    assert_eq!(store.activation_epoch(), 7);
    assert_eq!(
        store
            .activate(PROJECT, WORKSPACE, package.clone(), 100)
            .expect_err("unapproved source must not activate")
            .to_string(),
        "only a source-approved plugin package can be activated"
    );

    approve_source(&mut store, &package);
    let trust_epoch = store.epoch();
    activate_source(&mut store, &package);
    assert_eq!((store.epoch(), store.activation_epoch()), (trust_epoch, 8));
    assert!(store.is_activated(PROJECT, WORKSPACE, &package));
    assert!(!store.is_activated(PROJECT, "workspace-2", &package));
    assert!(!store.is_activated(PROJECT, WORKSPACE, &source(HASH_B, SOURCE_PATH)));
    assert!(!store
        .activate(PROJECT, WORKSPACE, package.clone(), 102)
        .expect("repeat activation"));

    assert!(store
        .clear_activation_record(PROJECT, WORKSPACE, &package.package_id, None)
        .expect("deactivate source")
        .is_some());
    assert_eq!((store.epoch(), store.activation_epoch()), (trust_epoch, 9));
    assert!(!store
        .clear_activation_record(PROJECT, WORKSPACE, &package.package_id, None)
        .expect("repeat deactivation")
        .is_some());
    assert_eq!((store.epoch(), store.activation_epoch()), (trust_epoch, 9));
}

#[test]
fn residual_activation_cleanup_preserves_source_approval_and_is_idempotent() {
    let package = source(HASH_A, SOURCE_PATH);
    let mut store = PluginTrustStore::new(7);
    approve_source(&mut store, &package);
    activate_source(&mut store, &package);
    let trust_epoch = store.epoch();
    let activated_epoch = store
        .activation_authority(PROJECT, WORKSPACE, &package)
        .expect("read activation authority")
        .activation_epoch();

    assert_eq!(
        store
            .clear_activation_record(
                PROJECT,
                WORKSPACE,
                &package.package_id,
                Some(activated_epoch),
            )
            .expect("clear residual activation"),
        Some(package.clone())
    );
    assert_eq!(store.epoch(), trust_epoch);
    assert_eq!(store.activation_epoch(), activated_epoch + 1);
    assert_eq!(
        store.trust_level_for(PROJECT, WORKSPACE, &package),
        PluginPackageTrustLevel::SourceApproved
    );
    assert!(!store.is_activated(PROJECT, WORKSPACE, &package));

    let cleanup_epoch = store.activation_epoch();
    assert_eq!(
        store
            .clear_activation_record(PROJECT, WORKSPACE, &package.package_id, None)
            .expect("repeat residual cleanup"),
        None
    );
    assert_eq!(store.activation_epoch(), cleanup_epoch);
}

#[test]
fn stale_residual_cleanup_cannot_clear_a_newer_activation() {
    let package = source(HASH_A, SOURCE_PATH);
    let mut store = PluginTrustStore::new(1);
    approve_source(&mut store, &package);
    activate_source(&mut store, &package);
    let stale_epoch = store
        .activation_authority(PROJECT, WORKSPACE, &package)
        .expect("read first activation authority")
        .activation_epoch();
    store
        .clear_activation_record(PROJECT, WORKSPACE, &package.package_id, None)
        .expect("deactivate source");
    store
        .activate(PROJECT, WORKSPACE, package.clone(), 103)
        .expect("reactivate source");
    let current_epoch = store
        .activation_authority(PROJECT, WORKSPACE, &package)
        .expect("read current activation authority")
        .activation_epoch();

    assert!(!store
        .clear_activation_record(PROJECT, WORKSPACE, &package.package_id, Some(stale_epoch),)
        .expect("stale cleanup is a no-op")
        .is_some());
    assert!(store.is_activated(PROJECT, WORKSPACE, &package));
    assert_eq!(store.activation_epoch(), current_epoch);
}

#[test]
fn source_changes_deny_and_revoke_invalidate_activation_atomically() {
    let original = source(HASH_A, SOURCE_PATH);
    let changed = source(HASH_B, SOURCE_PATH);
    let mut store = PluginTrustStore::new(1);
    approve_source(&mut store, &original);
    activate_source(&mut store, &original);
    let epochs = (store.epoch(), store.activation_epoch());
    assert!(store
        .reconcile_sources(PROJECT, WORKSPACE, std::slice::from_ref(&changed))
        .expect("reconcile changed source"));
    assert_eq!(
        (store.epoch(), store.activation_epoch()),
        (epochs.0 + 1, epochs.1 + 1)
    );
    assert!(!store.is_activated(PROJECT, WORKSPACE, &original));
    assert!(!store
        .reconcile_sources(PROJECT, WORKSPACE, &[changed])
        .expect("repeat reconciliation"));

    for decision in [PluginTrustDecision::Denied, PluginTrustDecision::Revoked] {
        let mut store = PluginTrustStore::new(1);
        approve_source(&mut store, &original);
        activate_source(&mut store, &original);
        let epochs = (store.epoch(), store.activation_epoch());
        store
            .apply_decision(PROJECT, WORKSPACE, original.clone(), decision, 102)
            .expect("replace source approval");
        assert_eq!(
            (store.epoch(), store.activation_epoch()),
            (epochs.0 + 1, epochs.1 + 1)
        );
        assert!(!store.is_activated(PROJECT, WORKSPACE, &original));
    }
}

#[test]
fn schema_v1_migrates_inactive_and_recreated_stores_do_not_reuse_generation() {
    let package = source(HASH_A, SOURCE_PATH);
    let schema_v1 = serde_json::json!({
        "schemaVersion": 1,
        "epoch": 7,
        "records": [trust_record(&package, PluginPackageTrustLevel::SourceApproved)]
    });
    let store: PluginTrustStore =
        serde_json::from_value(schema_v1).expect("deserialize schema-v1 store");
    store.validate().expect("validate migrated schema-v1 store");
    assert_eq!(store.activation_epoch(), 7);
    assert!(!store.is_activated(PROJECT, WORKSPACE, &package));
    let migrated = serde_json::to_value(&store).expect("serialize migrated store");
    assert_eq!(migrated["schemaVersion"], 2);
    assert_eq!(migrated["activationRecords"], serde_json::json!([]));

    let mut recreated = PluginTrustStore::new(store.activation_epoch());
    approve_source(&mut recreated, &package);
    activate_source(&mut recreated, &package);
    assert!(recreated.activation_epoch() > store.activation_epoch());
}

#[test]
fn invalid_persisted_activation_records_fail_closed() {
    let package = source(HASH_A, SOURCE_PATH);
    let activation = activation_record(&package);
    let approved = trust_record(&package, PluginPackageTrustLevel::SourceApproved);

    let duplicate: PluginTrustStore = serde_json::from_value(schema_v2_store(
        vec![approved],
        vec![activation.clone(), activation.clone()],
    ))
    .expect("deserialize duplicate activations");
    assert_eq!(
        duplicate
            .validate()
            .expect_err("reject duplicate")
            .to_string(),
        "duplicate plugin activation record"
    );

    let unknown: PluginTrustStore =
        serde_json::from_value(schema_v2_store(vec![], vec![activation.clone()]))
            .expect("deserialize unknown activation");
    assert_eq!(
        unknown.validate().expect_err("reject unknown").to_string(),
        "persisted plugin activation record has no trust record"
    );

    let stale: PluginTrustStore = serde_json::from_value(schema_v2_store(
        vec![trust_record(&package, PluginPackageTrustLevel::Denied)],
        vec![activation.clone()],
    ))
    .expect("deserialize stale activation");
    assert_eq!(
        stale.validate().expect_err("reject stale").to_string(),
        "persisted plugin activation record is not source-approved"
    );

    let excessive: PluginTrustStore =
        serde_json::from_value(schema_v2_store(vec![], vec![activation; 1025]))
            .expect("deserialize excessive activations");
    assert_eq!(
        excessive
            .validate()
            .expect_err("reject excessive")
            .to_string(),
        "too many plugin activation records"
    );
}

#[test]
fn activation_authority_requires_the_exact_activated_package() {
    let (_, package) = fixed_package_input(SOURCE_PATH);
    let mut store = PluginTrustStore::new(1);
    assert_eq!(
        store
            .activation_authority(PROJECT, WORKSPACE, &package)
            .expect_err("inactive package must not create authority")
            .to_string(),
        "plugin package input is not activated in this scope"
    );

    approve_source(&mut store, &package);
    activate_source(&mut store, &package);
    let (_, mismatched) = fixed_package_input("file:///workspace/other/acme.demo");
    assert!(store
        .activation_authority(PROJECT, WORKSPACE, &mismatched)
        .is_err());

    let authority = store
        .activation_authority(PROJECT, WORKSPACE, &package)
        .expect("create exact activation authority");
    let issued_epoch = authority.activation_epoch();
    assert_eq!(issued_epoch, store.activation_epoch());
    assert!(store.is_activation_current(&authority));
    let mut other = source(HASH_B, "file:///workspace/.bitfun/plugins/acme.other");
    other.package_id = "acme.other".to_string();
    approve_source(&mut store, &other);
    activate_source(&mut store, &other);
    assert!(store.activation_epoch() > issued_epoch);
    assert!(store.is_activation_current(&authority));
    let cross_scope = store
        .activation_authority(PROJECT, "workspace-2", &package)
        .expect_err("activation authority must stay in its exact scope");
    assert_eq!(
        cross_scope.to_string(),
        "plugin package input is not activated in this scope"
    );

    let retained = authority.clone();
    store
        .clear_activation_record(PROJECT, WORKSPACE, &package.package_id, None)
        .expect("deactivate source");
    assert!(!store.is_activation_current(&retained));

    let (project, workspace, authority_source, activation_epoch) = authority.into_parts();
    assert_eq!(activation_epoch, issued_epoch);
    assert_ne!(activation_epoch, store.activation_epoch());
    assert_eq!((project.as_str(), workspace.as_str()), (PROJECT, WORKSPACE));
    assert_eq!(authority_source, package);
}

#[test]
fn trust_store_schema_versions_reject_missing_null_and_cross_version_fields() {
    let missing_v2 = r#"{
      "schemaVersion": 2,
      "epoch": 1,
      "records": [],
      "activationEpoch": 1
    }"#;
    assert!(serde_json::from_str::<PluginTrustStore>(missing_v2).is_err());

    for invalid_v1 in [
        r#"{"schemaVersion":1,"epoch":1,"records":[],"activationEpoch":null}"#,
        r#"{"schemaVersion":1,"epoch":1,"records":[],"activationRecords":null}"#,
    ] {
        assert!(serde_json::from_str::<PluginTrustStore>(invalid_v1).is_err());
    }

    let null_v2 = r#"{
      "schemaVersion": 2,
      "epoch": 1,
      "records": [],
      "activationEpoch": 1,
      "activationRecords": null
    }"#;
    assert!(serde_json::from_str::<PluginTrustStore>(null_v2).is_err());
}
