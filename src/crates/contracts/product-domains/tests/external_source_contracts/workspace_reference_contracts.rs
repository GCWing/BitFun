use openbitfun_product_domains::external_sources::{
    EcosystemId, ExecutionDomainId, ExternalSourceAssetKind, ExternalSourceDiagnostic,
    ExternalSourceHealth, ExternalSourceRecord, ExternalSourceScope, ProviderId, SourceKey,
};
use openbitfun_product_domains::workspace_references::{
    ExternalWorkspaceReferenceDefinition, ExternalWorkspaceReferenceProviderIdentity,
    ExternalWorkspaceReferenceProviderSnapshot,
};
use std::path::PathBuf;

fn source() -> ExternalSourceRecord {
    ExternalSourceRecord {
        key: SourceKey::new("opencode.references", "project-config").unwrap(),
        ecosystem_id: EcosystemId::new("opencode").unwrap(),
        display_name: "OpenCode project configuration".to_string(),
        source_kind: "opencode_config".to_string(),
        scope: ExternalSourceScope::Project,
        location: "<workspace>/opencode.json".to_string(),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
        health: ExternalSourceHealth::Available,
        content_version: "source-v1".to_string(),
        diagnostics: Vec::new(),
    }
}

#[test]
fn provider_snapshot_accepts_distinct_aliases_for_the_same_directory() {
    let source = source();
    let shared_path = std::env::temp_dir().join("workspace-reference-shared");
    let snapshot = ExternalWorkspaceReferenceProviderSnapshot {
        provider: ExternalWorkspaceReferenceProviderIdentity::new(
            "opencode.references",
            "opencode",
            "OpenCode",
        )
        .unwrap(),
        sources: vec![source.clone()],
        references: vec![
            ExternalWorkspaceReferenceDefinition {
                source: source.key.clone(),
                alias: "docs".to_string(),
                path: shared_path.clone(),
                description: Some("Product documentation".to_string()),
                hidden: false,
                content_version: "docs-v1".to_string(),
            },
            ExternalWorkspaceReferenceDefinition {
                source: source.key,
                alias: "specs".to_string(),
                path: shared_path,
                description: None,
                hidden: true,
                content_version: "specs-v1".to_string(),
            },
        ],
        diagnostics: Vec::new(),
    };

    snapshot.validate().unwrap();
}

#[test]
fn provider_snapshot_rejects_duplicate_aliases_and_relative_paths() {
    let source = source();
    let identity = ExternalWorkspaceReferenceProviderIdentity {
        provider_id: ProviderId::new("opencode.references").unwrap(),
        ecosystem_id: EcosystemId::new("opencode").unwrap(),
        display_name: "OpenCode".to_string(),
    };
    let definition = ExternalWorkspaceReferenceDefinition {
        source: source.key.clone(),
        alias: "docs".to_string(),
        path: PathBuf::from("relative/docs"),
        description: None,
        hidden: false,
        content_version: "docs-v1".to_string(),
    };
    let snapshot = ExternalWorkspaceReferenceProviderSnapshot {
        provider: identity,
        sources: vec![source],
        references: vec![definition.clone(), definition],
        diagnostics: Vec::new(),
    };

    assert!(snapshot.validate().is_err());
}

#[test]
fn provider_snapshot_rejects_unbounded_diagnostics() {
    let source = source();
    let diagnostics = (0..257)
        .map(|index| {
            ExternalSourceDiagnostic::warning(
                format!("opencode.reference.invalid_{index}"),
                "invalid reference",
                Some(source.key.clone()),
            )
            .with_asset_kind(ExternalSourceAssetKind::Reference)
        })
        .collect();
    let snapshot = ExternalWorkspaceReferenceProviderSnapshot {
        provider: ExternalWorkspaceReferenceProviderIdentity::new(
            "opencode.references",
            "opencode",
            "OpenCode",
        )
        .unwrap(),
        sources: vec![source],
        references: Vec::new(),
        diagnostics,
    };

    assert!(snapshot.validate().is_err());
}
