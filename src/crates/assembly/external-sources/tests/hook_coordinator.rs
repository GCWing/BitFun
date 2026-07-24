use bitfun_external_sources::ExternalHookCatalogCoordinator;
use bitfun_product_domains::external_hook_catalog::{
    ExternalHookCatalogEntry, ExternalHookHandlerKind, ExternalHookMapping,
    ExternalHookMatcherSummary, ExternalHookNativeActivation, ExternalHookProjectionStatus,
    ExternalHookProviderIdentity, ExternalHookProviderSnapshot, ExternalHookSource,
    ExternalHookSourceKind, ExternalHookSourceProvider,
};
use bitfun_product_domains::external_hook_contributions::ExternalHookPoint;
use bitfun_product_domains::external_sources::{
    EcosystemId, ExecutionDomainId, ExternalSourceContext, ExternalSourceHealth,
    ExternalSourceProviderError, ExternalSourceScope, SourceKey,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct FakeProvider {
    identity: ExternalHookProviderIdentity,
    snapshot: Mutex<Result<ExternalHookProviderSnapshot, ExternalSourceProviderError>>,
    delay: Mutex<std::time::Duration>,
}

impl FakeProvider {
    fn new(provider_id: &str, ecosystem_id: &str, event: &str) -> Self {
        let identity =
            ExternalHookProviderIdentity::new(provider_id, ecosystem_id, provider_id).unwrap();
        Self {
            snapshot: Mutex::new(Ok(snapshot(identity.clone(), event, "v1"))),
            delay: Mutex::new(std::time::Duration::ZERO),
            identity,
        }
    }

    fn set_snapshot(
        &self,
        snapshot: Result<ExternalHookProviderSnapshot, ExternalSourceProviderError>,
    ) {
        *self.snapshot.lock().unwrap() = snapshot;
    }

    fn set_delay(&self, delay: std::time::Duration) {
        *self.delay.lock().unwrap() = delay;
    }
}

impl ExternalHookSourceProvider for FakeProvider {
    fn identity(&self) -> ExternalHookProviderIdentity {
        self.identity.clone()
    }

    fn discover(
        &self,
        _context: &ExternalSourceContext,
    ) -> Result<ExternalHookProviderSnapshot, ExternalSourceProviderError> {
        std::thread::sleep(*self.delay.lock().unwrap());
        self.snapshot.lock().unwrap().clone()
    }
}

fn snapshot(
    identity: ExternalHookProviderIdentity,
    event: &str,
    version: &str,
) -> ExternalHookProviderSnapshot {
    let source_key = SourceKey::new(identity.provider_id.as_str(), "project").unwrap();
    ExternalHookProviderSnapshot {
        provider: identity.clone(),
        sources: vec![ExternalHookSource {
            key: source_key.clone(),
            ecosystem_id: identity.ecosystem_id,
            display_name: "Project hooks".to_string(),
            source_kind: ExternalHookSourceKind::Settings,
            scope: ExternalSourceScope::Project,
            location_hint: ".example/settings.json".to_string(),
            health: ExternalSourceHealth::Available,
            content_version: version.to_string(),
            diagnostics: Vec::new(),
        }],
        entries: vec![ExternalHookCatalogEntry {
            stable_key: format!("{}:{event}", identity.provider_id),
            source: source_key,
            native_event: event.to_string(),
            matcher: ExternalHookMatcherSummary::Any,
            handler_kind: ExternalHookHandlerKind::Command,
            projection_status: ExternalHookProjectionStatus::Mapped,
            native_activation: ExternalHookNativeActivation::Unknown,
            mapping: Some(ExternalHookMapping {
                hook_point: ExternalHookPoint::ToolBefore,
            }),
            content_version: version.to_string(),
        }],
        diagnostics: Vec::new(),
    }
}

fn context() -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: Some(PathBuf::from("/workspace")),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

async fn refresh(
    coordinator: &ExternalHookCatalogCoordinator,
) -> bitfun_product_domains::external_hook_catalog::ExternalHookCatalogSnapshotV1 {
    let batch = coordinator
        .discover(std::time::Duration::from_secs(1))
        .await;
    assert!(batch.deferred.is_empty());
    coordinator.apply_discovery_results(batch.immediate)
}

#[tokio::test]
async fn catalog_is_pending_then_preserves_provider_registration_order() {
    let zeta = Arc::new(FakeProvider::new("zeta.hooks", "zeta", "PreToolUse"));
    let alpha = Arc::new(FakeProvider::new("alpha.hooks", "alpha", "PreToolUse"));
    let coordinator = ExternalHookCatalogCoordinator::new(context(), vec![zeta, alpha]).unwrap();

    assert!(coordinator.snapshot().discovery_pending);
    let snapshot = refresh(&coordinator).await;

    assert!(!snapshot.discovery_pending);
    assert_eq!(snapshot.entries.len(), 2);
    assert_eq!(
        snapshot.sources[0].ecosystem_id,
        EcosystemId::new("zeta").unwrap()
    );
    assert_eq!(
        snapshot.entries[0].source.provider_id.as_str(),
        "zeta.hooks"
    );
    assert_eq!(snapshot.providers[0].provider_id.as_str(), "zeta.hooks");
}

#[tokio::test]
async fn failed_refresh_retains_last_valid_static_snapshot_and_marks_it_stale() {
    let provider = Arc::new(FakeProvider::new(
        "claude.hooks",
        "claude-code",
        "PreToolUse",
    ));
    let coordinator =
        ExternalHookCatalogCoordinator::new(context(), vec![provider.clone()]).unwrap();
    assert_eq!(refresh(&coordinator).await.entries.len(), 1);

    provider.set_snapshot(Err(ExternalSourceProviderError::new(
        "claude.hook.read_failed",
        "read failed",
        true,
    )));
    let stale = refresh(&coordinator).await;

    assert_eq!(stale.entries.len(), 1);
    assert_eq!(
        stale.stale_provider_ids,
        vec![provider.identity.provider_id.clone()]
    );
    assert!(stale
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "claude.hook.read_failed"));
}

#[tokio::test]
async fn initial_provider_failure_is_distinct_from_a_valid_empty_catalog() {
    let provider = Arc::new(FakeProvider::new("failed.hooks", "failed", "PreToolUse"));
    provider.set_snapshot(Err(ExternalSourceProviderError::new(
        "failed.hook.read_failed",
        "read failed",
        true,
    )));
    let coordinator =
        ExternalHookCatalogCoordinator::new(context(), vec![provider.clone()]).unwrap();

    let failed = refresh(&coordinator).await;

    assert_eq!(
        failed.failed_provider_ids,
        vec![provider.identity.provider_id.clone()]
    );
    assert!(failed.stale_provider_ids.is_empty());
    assert!(failed.sources.is_empty());
}

#[tokio::test]
async fn timed_out_discovery_stays_pending_until_the_deferred_result_is_applied() {
    let provider = Arc::new(FakeProvider::new("slow.hooks", "slow", "PreToolUse"));
    provider.set_delay(std::time::Duration::from_millis(30));
    let coordinator =
        ExternalHookCatalogCoordinator::new(context(), vec![provider.clone()]).unwrap();

    let mut batch = coordinator
        .discover(std::time::Duration::from_millis(1))
        .await;
    assert_eq!(batch.deferred.len(), 1);
    let pending = coordinator.apply_discovery_results(batch.immediate);
    assert!(pending.discovery_pending);
    assert!(pending.entries.is_empty());
    assert!(!pending
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "external_hook.discovery_timeout"));

    let deferred = batch.deferred.pop().unwrap();
    let (completed, observer) = coordinator.complete_deferred(deferred).await.unwrap();
    assert!(observer.is_none());
    let result = coordinator.finalize_deferred(completed).await.unwrap();
    let complete = coordinator.apply_discovery_result(result);
    assert!(!complete.discovery_pending);
    assert_eq!(complete.entries.len(), 1);
}

#[test]
fn duplicate_provider_ids_are_rejected_before_discovery() {
    let first = Arc::new(FakeProvider::new("same.hooks", "first", "PreToolUse"));
    let second = Arc::new(FakeProvider::new("same.hooks", "second", "PostToolUse"));
    assert!(ExternalHookCatalogCoordinator::new(context(), vec![first, second]).is_err());
}
