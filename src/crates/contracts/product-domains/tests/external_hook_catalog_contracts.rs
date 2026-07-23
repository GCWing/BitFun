use bitfun_product_domains::external_hook_catalog::{
    ExternalHookCatalogEntry, ExternalHookCatalogSnapshotV1, ExternalHookHandlerKind,
    ExternalHookMapping, ExternalHookMatcherSummary, ExternalHookNativeActivation,
    ExternalHookProjectionStatus, ExternalHookProviderIdentity, ExternalHookProviderSnapshot,
    ExternalHookSource, ExternalHookSourceKind,
};
use bitfun_product_domains::external_hook_contributions::ExternalHookPoint;
use bitfun_product_domains::external_sources::{
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
fn only_mapped_entries_may_carry_a_reviewed_bitfun_hook_point() {
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
