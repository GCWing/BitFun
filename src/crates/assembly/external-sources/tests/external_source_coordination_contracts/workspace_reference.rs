use openbitfun_external_sources::ExternalWorkspaceReferenceCoordinator;
use openbitfun_product_domains::external_sources::{
    EcosystemId, ExecutionDomainId, ExternalSourceContext, ExternalSourceHealth,
    ExternalSourceLifecycleState, ExternalSourceProviderError, ExternalSourceRecord,
    ExternalSourceScope, ExternalWatchRoot, SourceKey,
};
use openbitfun_product_domains::workspace_references::{
    ExternalWorkspaceReferenceDefinition, ExternalWorkspaceReferenceProviderIdentity,
    ExternalWorkspaceReferenceProviderSnapshot, ExternalWorkspaceReferenceSourceProvider,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct ToggleProvider {
    fail: Arc<AtomicBool>,
    path: PathBuf,
}

impl ExternalWorkspaceReferenceSourceProvider for ToggleProvider {
    fn identity(&self) -> ExternalWorkspaceReferenceProviderIdentity {
        ExternalWorkspaceReferenceProviderIdentity::new(
            "opencode.references",
            "opencode",
            "OpenCode",
        )
        .unwrap()
    }

    fn discover(
        &self,
        context: &ExternalSourceContext,
    ) -> Result<ExternalWorkspaceReferenceProviderSnapshot, ExternalSourceProviderError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(ExternalSourceProviderError::new(
                "opencode.reference.refresh_failed",
                "refresh failed",
                true,
            ));
        }
        let source = ExternalSourceRecord {
            key: SourceKey::new("opencode.references", "project").unwrap(),
            ecosystem_id: EcosystemId::new("opencode").unwrap(),
            display_name: "OpenCode project references".to_string(),
            source_kind: "opencode_config".to_string(),
            scope: ExternalSourceScope::Project,
            location: self.path.join("opencode.json").display().to_string(),
            execution_domain_id: context.execution_domain_id.clone(),
            health: ExternalSourceHealth::Available,
            content_version: "source-v1".to_string(),
            diagnostics: Vec::new(),
        };
        Ok(ExternalWorkspaceReferenceProviderSnapshot {
            provider: self.identity(),
            sources: vec![source.clone()],
            references: vec![ExternalWorkspaceReferenceDefinition {
                source: source.key,
                alias: "docs".to_string(),
                path: self.path.clone(),
                description: Some("Documentation".to_string()),
                hidden: false,
                content_version: "docs-v1".to_string(),
            }],
            diagnostics: Vec::new(),
        })
    }

    fn watch_roots(&self, _context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        vec![ExternalWatchRoot {
            path: self.path.clone(),
            recursive: true,
        }]
    }
}

fn context() -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: Some(std::env::temp_dir()),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

#[test]
fn transient_failure_keeps_the_last_valid_reference_generation() {
    let fail = Arc::new(AtomicBool::new(false));
    let path = std::env::temp_dir();
    let provider = Arc::new(ToggleProvider {
        fail: Arc::clone(&fail),
        path,
    });
    let mut coordinator =
        ExternalWorkspaceReferenceCoordinator::new(context(), vec![provider]).unwrap();

    let current = coordinator.refresh();
    assert_eq!(current.references.len(), 1);
    assert_eq!(
        current.sources[0].lifecycle,
        ExternalSourceLifecycleState::Available
    );

    fail.store(true, Ordering::SeqCst);
    let degraded = coordinator.refresh();
    assert_eq!(degraded.references, current.references);
    assert_eq!(
        degraded.sources[0].lifecycle,
        ExternalSourceLifecycleState::UsingLastValidVersion
    );
    assert!(degraded
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.reference.refresh_failed"));
}

#[test]
fn suppression_and_ecosystem_watch_filter_use_the_shared_source_identity() {
    let path = std::env::temp_dir();
    let provider = Arc::new(ToggleProvider {
        fail: Arc::new(AtomicBool::new(false)),
        path: path.clone(),
    });
    let mut coordinator =
        ExternalWorkspaceReferenceCoordinator::new(context(), vec![provider]).unwrap();
    let snapshot = coordinator.refresh();
    let stable_key = snapshot.sources[0].stable_key.clone();

    coordinator.set_source_enabled(&stable_key, false).unwrap();

    assert!(coordinator.snapshot().references.is_empty());
    assert_eq!(
        coordinator.snapshot().sources[0].lifecycle,
        ExternalSourceLifecycleState::Suppressed
    );
    coordinator.set_source_enabled(&stable_key, true).unwrap();
    assert_eq!(coordinator.snapshot().references.len(), 1);
    assert_eq!(
        coordinator.watch_roots_for_ecosystems(
            &[EcosystemId::new("opencode").unwrap()]
                .into_iter()
                .collect()
        ),
        vec![ExternalWatchRoot {
            path,
            recursive: true,
        }]
    );
}
