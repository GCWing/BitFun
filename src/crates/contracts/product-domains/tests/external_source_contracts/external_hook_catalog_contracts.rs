use openbitfun_product_domains::external_hook_catalog::{
    ExternalHookCatalogEntry, ExternalHookCatalogSnapshotV1, ExternalHookHandlerKind,
    ExternalHookMapping, ExternalHookMatcherSummary, ExternalHookNativeActivation,
    ExternalHookProjectionStatus, ExternalHookProviderIdentity, ExternalHookProviderSnapshot,
    ExternalHookSource, ExternalHookSourceKind,
};
use openbitfun_product_domains::external_hook_contributions::ExternalHookPoint;
use openbitfun_product_domains::external_hook_import::{
    ExternalHookImportDependencyV1, ExternalHookImportDispositionV1, ExternalHookImportHandlerV1,
    ExternalHookImportPlanV1, ExternalHookImportSkippedV1, PreparedExternalHookAsset,
    PreparedExternalHookHandler, PreparedExternalHookImport, EXTERNAL_HOOK_IMPORT_SCHEMA_V1,
    MANAGED_HOOK_ROOT_PLACEHOLDER,
};
use openbitfun_product_domains::external_sources::{
    EcosystemId, ExternalSourceAssetKind, ExternalSourceDiagnostic, ExternalSourceHealth,
    ExternalSourceScope, ProviderId, SourceKey,
};

fn provider() -> ExternalHookProviderIdentity {
    ExternalHookProviderIdentity::new("claude-code.hooks", "claude-code", "Claude Code Hooks")
        .unwrap()
}

fn source() -> ExternalHookSource {
    ExternalHookSource {
        key: SourceKey::new("claude-code.hooks", "project-settings").unwrap(),
        ecosystem_id: EcosystemId::new("claude-code").unwrap(),
        display_name: "Claude Code project settings".to_string(),
        source_kind: ExternalHookSourceKind::Settings,
        scope: ExternalSourceScope::Project,
        location_hint: ".claude/settings.json".to_string(),
        health: ExternalSourceHealth::Available,
        content_version: "sha256:0123456789abcdef".to_string(),
        diagnostics: Vec::new(),
    }
}

fn entry(local_id: &str) -> ExternalHookCatalogEntry {
    ExternalHookCatalogEntry {
        stable_key: format!("claude-code.hooks:project-settings:{local_id}"),
        source: SourceKey::new("claude-code.hooks", "project-settings").unwrap(),
        native_event: "PreToolUse".to_string(),
        matcher: ExternalHookMatcherSummary::Pattern {
            display: "Bash|Edit".to_string(),
        },
        handler_kind: ExternalHookHandlerKind::Command,
        projection_status: ExternalHookProjectionStatus::Mapped,
        native_activation: ExternalHookNativeActivation::Unknown,
        mapping: Some(ExternalHookMapping {
            hook_point: ExternalHookPoint::ToolBefore,
        }),
        content_version: "sha256:fedcba9876543210".to_string(),
    }
}

#[test]
fn hook_diagnostics_have_a_first_class_wire_kind() {
    assert_eq!(
        serde_json::to_value(ExternalSourceAssetKind::Hook).unwrap(),
        "hook"
    );
}

#[test]
fn catalog_wire_shape_is_redacted_and_uses_stable_names() {
    let value = serde_json::to_value(entry("pre-tool-0")).unwrap();

    assert_eq!(value["nativeEvent"], "PreToolUse");
    assert_eq!(value["handlerKind"], "command");
    assert_eq!(value["projectionStatus"], "mapped");
    assert_eq!(value["nativeActivation"], "unknown");
    assert_eq!(value["mapping"]["hookPoint"], "tool_before");
    assert!(value.get("command").is_none());
    assert!(value.get("script").is_none());
    assert!(value.get("payload").is_none());
    assert!(value.get("environment").is_none());
}

#[test]
fn only_mapped_entries_may_carry_a_reviewed_openbitfun_hook_point() {
    let mut invalid = entry("native-only");
    invalid.projection_status = ExternalHookProjectionStatus::NativeOnly;
    assert!(invalid.validate().is_err());

    invalid.mapping = None;
    assert!(invalid.validate().is_ok());
}

#[test]
fn provider_snapshot_rejects_cross_provider_and_duplicate_entries() {
    let valid = entry("pre-tool-0");
    let snapshot = ExternalHookProviderSnapshot {
        provider: provider(),
        sources: vec![source()],
        entries: vec![valid.clone()],
        diagnostics: Vec::new(),
    };
    assert!(snapshot.validate().is_ok());

    let duplicate = ExternalHookProviderSnapshot {
        entries: vec![valid.clone(), valid],
        ..snapshot.clone()
    };
    assert!(duplicate.validate().is_err());

    let foreign = ExternalHookProviderSnapshot {
        sources: vec![ExternalHookSource {
            key: SourceKey::new("other-provider", "project-settings").unwrap(),
            ..source()
        }],
        ..snapshot
    };
    assert!(foreign.validate().is_err());
}

#[test]
fn provider_snapshot_rejects_non_hook_or_foreign_diagnostics() {
    let base = ExternalHookProviderSnapshot {
        provider: provider(),
        sources: vec![source()],
        entries: vec![entry("pre-tool-0")],
        diagnostics: Vec::new(),
    };
    let non_hook = ExternalHookProviderSnapshot {
        diagnostics: vec![ExternalSourceDiagnostic::warning(
            "claude.hook.partial",
            "Hook configuration is partially available",
            None,
        )],
        ..base.clone()
    };
    assert!(non_hook.validate().is_err());

    let foreign = ExternalHookProviderSnapshot {
        diagnostics: vec![ExternalSourceDiagnostic::warning(
            "claude.hook.partial",
            "Hook configuration is partially available",
            Some(SourceKey::new("other-provider", "settings").unwrap()),
        )
        .with_asset_kind(ExternalSourceAssetKind::Hook)],
        ..base
    };
    assert!(foreign.validate().is_err());
}

#[test]
fn provider_identity_keeps_ecosystems_open_without_an_ecosystem_enum() {
    let identity = ExternalHookProviderIdentity {
        provider_id: ProviderId::new("future.hooks").unwrap(),
        ecosystem_id: EcosystemId::new("future-product/v3").unwrap(),
        display_name: "Future Hooks".to_string(),
    };
    assert!(identity.validate().is_ok());
}

#[test]
fn empty_catalog_is_pending_until_the_first_discovery_finishes() {
    let snapshot = ExternalHookCatalogSnapshotV1::default();
    assert!(snapshot.discovery_pending);
    assert_eq!(snapshot.schema_version, 1);
    assert!(snapshot.providers.is_empty());
}

#[test]
fn import_plan_has_a_versioned_exact_wire_shape_and_redacted_debug() {
    let plan = ExternalHookImportPlanV1 {
        schema_version: EXTERNAL_HOOK_IMPORT_SCHEMA_V1,
        source: source(),
        disposition: ExternalHookImportDispositionV1::Import,
        behavior_version: "sha256:behavior".to_string(),
        handlers: vec![ExternalHookImportHandlerV1 {
            stable_key: "pre-tool-0".to_string(),
            event: "PreToolUse".to_string(),
            matcher: Some("Bash".to_string()),
            command: "private-command --token secret".to_string(),
            command_windows: None,
            timeout_seconds: Some(30),
            status_message: Some("secret-status".to_string()),
            dependencies: vec![ExternalHookImportDependencyV1::External {
                location: "/opt/private/tool".to_string(),
            }],
        }],
        skipped: vec![ExternalHookImportSkippedV1 {
            reason_code: "unsupported_async".to_string(),
            count: 1,
        }],
        plan_fingerprint: "sha256:plan".to_string(),
    };

    plan.validate().unwrap();
    let value = serde_json::to_value(&plan).unwrap();
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["disposition"], "import");
    assert_eq!(value["handlers"][0]["dependencies"][0]["kind"], "external");
    assert!(
        serde_json::from_value::<ExternalHookImportPlanV1>(serde_json::json!({
            "schemaVersion": 1,
            "source": value["source"],
            "disposition": "import",
            "behaviorVersion": "sha256:behavior",
            "handlers": [],
            "skipped": [],
            "planFingerprint": "sha256:plan",
            "unexpected": true
        }))
        .is_err()
    );
    let debug = format!("{plan:?}");
    assert!(!debug.contains("private-command"));
    assert!(!debug.contains("secret"));
    assert!(!debug.contains("secret-status"));
    assert!(!debug.contains("/opt/private/tool"));

    let handler_debug = format!("{:?}", plan.handlers[0]);
    assert!(!handler_debug.contains("secret-status"));
}

#[test]
fn prepared_behavior_version_tracks_commands_and_asset_bytes_without_debug_leaks() {
    let prepared = |command: &str, bytes: &[u8]| {
        PreparedExternalHookImport::new(
            source(),
            vec![PreparedExternalHookHandler {
                stable_key: "pre-tool-0".to_string(),
                event: "PreToolUse".to_string(),
                matcher: Some("Bash".to_string()),
                command: command.to_string(),
                command_windows: None,
                timeout_seconds: Some(30),
                status_message: None,
                dependencies: Vec::new(),
            }],
            vec![ExternalHookImportSkippedV1 {
                reason_code: "unsupported_async".to_string(),
                count: 1,
            }],
            vec![PreparedExternalHookAsset {
                relative_path: "hooks/check.py".into(),
                bytes: bytes.to_vec(),
            }],
        )
        .unwrap()
    };

    let first = prepared("python hooks/check.py", b"print('one')");
    let command_changed = prepared("python3 hooks/check.py", b"print('one')");
    let asset_changed = prepared("python hooks/check.py", b"print('two')");
    assert_ne!(first.behavior_version, command_changed.behavior_version);
    assert_ne!(first.behavior_version, asset_changed.behavior_version);
    let debug = format!("{first:?}");
    assert!(!debug.contains("python hooks/check.py"));
    assert!(!debug.contains("print('one')"));
}

#[test]
fn prepared_assets_reject_paths_deeper_than_the_fixed_import_budget() {
    let error = PreparedExternalHookImport::new(
        source(),
        vec![PreparedExternalHookHandler {
            stable_key: "pre-tool-0".to_string(),
            event: "PreToolUse".to_string(),
            matcher: None,
            command: "check".to_string(),
            command_windows: None,
            timeout_seconds: None,
            status_message: None,
            dependencies: Vec::new(),
        }],
        Vec::new(),
        vec![PreparedExternalHookAsset {
            relative_path: "one/two/three/four/five/six/seven/eight/nine/check.py".into(),
            bytes: vec![1],
        }],
    )
    .unwrap_err();

    assert!(error.to_string().contains("asset path"));
}

#[test]
fn prepared_import_preserves_skipped_only_preview_but_rejects_an_empty_result() {
    let skipped_only = PreparedExternalHookImport::new(
        source(),
        Vec::new(),
        vec![ExternalHookImportSkippedV1 {
            reason_code: "unsupported_handler_type".to_string(),
            count: 2,
        }],
        Vec::new(),
    )
    .unwrap();
    assert!(skipped_only.handlers.is_empty());

    assert!(PreparedExternalHookImport::new(source(), Vec::new(), Vec::new(), Vec::new()).is_err());
}

#[test]
fn prepared_handler_materializes_only_the_reserved_managed_root_placeholder() {
    let handler = PreparedExternalHookHandler {
        stable_key: "pre-tool-0".to_string(),
        event: "PreToolUse".to_string(),
        matcher: None,
        command: format!(
            "python \"{MANAGED_HOOK_ROOT_PLACEHOLDER}/hooks/check.py\" --label literal"
        ),
        command_windows: None,
        timeout_seconds: None,
        status_message: None,
        dependencies: vec![ExternalHookImportDependencyV1::Managed {
            relative_path: "hooks/check.py".to_string(),
        }],
    };

    let review = handler
        .public_review_at(std::path::Path::new("D:/managed/import"))
        .unwrap();
    assert_eq!(
        review.command,
        "python \"D:/managed/import/hooks/check.py\" --label literal"
    );
    assert!(handler
        .public_review_at(std::path::Path::new("D:/unsafe/$root"))
        .is_err());
}
