use bitfun_external_sources::ExternalToolCoordinator;
use bitfun_product_domains::external_sources::{
    EcosystemId, ExecutionDomainId, ExternalSourceContext, ExternalSourceHealth,
    ExternalSourceProviderError, ExternalSourceRecord, ExternalSourceScope, ExternalToolCapability,
    ExternalToolDefinition, ExternalToolProviderIdentity, ExternalToolProviderSnapshot,
    ExternalToolRuntimeKind, ExternalToolSourceProvider, ExternalToolStaticStatus,
    ExternalWatchRoot, PreparedExternalToolExport, PreparedExternalToolTarget, SourceKey,
    SourceQualifiedToolId, SourceQualifiedToolTargetId,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct FakeToolProvider {
    identity: ExternalToolProviderIdentity,
    snapshot: Mutex<Result<ExternalToolProviderSnapshot, ExternalSourceProviderError>>,
}

impl FakeToolProvider {
    fn new() -> Self {
        let identity = ExternalToolProviderIdentity::new("fake.tools", "fake", "Fake").unwrap();
        Self {
            identity: identity.clone(),
            snapshot: Mutex::new(Ok(snapshot(identity, "v1"))),
        }
    }

    fn set_snapshot(
        &self,
        snapshot: Result<ExternalToolProviderSnapshot, ExternalSourceProviderError>,
    ) {
        *self.snapshot.lock().unwrap() = snapshot;
    }
}

impl ExternalToolSourceProvider for FakeToolProvider {
    fn identity(&self) -> ExternalToolProviderIdentity {
        self.identity.clone()
    }

    fn discover(
        &self,
        _context: &ExternalSourceContext,
    ) -> Result<ExternalToolProviderSnapshot, ExternalSourceProviderError> {
        self.snapshot.lock().unwrap().clone()
    }

    fn prepare_target(
        &self,
        _context: &ExternalSourceContext,
        target_id: &SourceQualifiedToolTargetId,
        expected_content_version: &str,
    ) -> Result<PreparedExternalToolTarget, ExternalSourceProviderError> {
        if expected_content_version != "v1" {
            return Err(ExternalSourceProviderError::new("stale", "stale", true));
        }
        Ok(PreparedExternalToolTarget {
            target_id: target_id.clone(),
            content_version: expected_content_version.to_string(),
            module_source: "export default {}".to_string(),
            module_url: "file:///fake.js".to_string(),
            working_directory: "/workspace".to_string(),
            worktree_root: Some("/workspace".to_string()),
            expected_tools: vec![PreparedExternalToolExport {
                export_name: "default".to_string(),
                tool_name: "weather".to_string(),
            }],
        })
    }

    fn watch_roots(&self, _context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        vec![ExternalWatchRoot {
            path: PathBuf::from("/workspace/.fake/tools"),
            recursive: true,
        }]
    }
}

fn snapshot(identity: ExternalToolProviderIdentity, version: &str) -> ExternalToolProviderSnapshot {
    let source_key = SourceKey::new("fake.tools", "project").unwrap();
    let target = SourceQualifiedToolTargetId::new(source_key.clone(), "weather.js").unwrap();
    ExternalToolProviderSnapshot {
        provider: identity,
        sources: vec![ExternalSourceRecord {
            key: source_key,
            ecosystem_id: EcosystemId::new("fake").unwrap(),
            display_name: "Fake project tools".to_string(),
            source_kind: "standalone_tools".to_string(),
            scope: ExternalSourceScope::Project,
            location: "/workspace/.fake/tools".to_string(),
            execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
            health: ExternalSourceHealth::Available,
            content_version: version.to_string(),
            diagnostics: Vec::new(),
        }],
        tools: vec![ExternalToolDefinition {
            id: SourceQualifiedToolId::new(target, "default").unwrap(),
            name: "weather".to_string(),
            description_preview: "Weather".to_string(),
            module_path: "/workspace/.fake/tools/weather.js".to_string(),
            working_directory: "/workspace".to_string(),
            runtime_kind: ExternalToolRuntimeKind::JavaScript,
            capabilities: vec![ExternalToolCapability::Network],
            content_version: version.to_string(),
            static_status: ExternalToolStaticStatus::Ready,
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

#[test]
fn tool_discovery_is_pending_until_each_provider_reports() {
    let provider = Arc::new(FakeToolProvider::new());
    let coordinator = ExternalToolCoordinator::new(context(), vec![provider]).unwrap();
    assert!(coordinator.snapshot().discovery_pending);
    assert!(coordinator.snapshot().tools.is_empty());
}

#[test]
fn provider_failure_withdraws_executable_candidates_instead_of_serving_stale_code() {
    let provider = Arc::new(FakeToolProvider::new());
    let mut coordinator = ExternalToolCoordinator::new(context(), vec![provider.clone()]).unwrap();
    assert_eq!(coordinator.refresh().tools.len(), 1);

    provider.set_snapshot(Err(ExternalSourceProviderError::new(
        "fake.read_failed",
        "read failed",
        true,
    )));
    let failed = coordinator.refresh();

    assert!(failed.tools.is_empty());
    assert!(failed
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "fake.read_failed"));
}

#[test]
fn suppression_and_stale_preparation_are_guarded_by_the_coordinator() {
    let provider = Arc::new(FakeToolProvider::new());
    let mut coordinator = ExternalToolCoordinator::new(context(), vec![provider]).unwrap();
    let snapshot = coordinator.refresh();
    let source_key = snapshot.sources[0].record.preference_key();
    let definition = snapshot.tools[0].clone();

    coordinator.set_source_enabled(&source_key, false).unwrap();
    assert!(coordinator.snapshot().tools.is_empty());
    coordinator.set_source_enabled(&source_key, true).unwrap();

    coordinator
        .prepare_target_guarded(&definition.id.target, "v1")
        .unwrap();
    assert!(coordinator
        .prepare_target_guarded(&definition.id.target, "v0")
        .is_err());
}
