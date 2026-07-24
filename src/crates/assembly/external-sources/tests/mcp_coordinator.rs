use bitfun_external_sources::ExternalMcpCoordinator;
use bitfun_product_domains::external_sources::{
    EcosystemId, ExecutionDomainId, ExternalMcpDiscoveryInput, ExternalMcpProviderIdentity,
    ExternalMcpProviderSnapshot, ExternalMcpRevisionKey, ExternalMcpServerDefinition,
    ExternalMcpSourceProvider, ExternalMcpStaticStatus, ExternalMcpTransportKind,
    ExternalSourceContext, ExternalSourceHealth, ExternalSourceLifecycleState,
    ExternalSourceProviderError, ExternalSourceRecord, ExternalSourceScope, ExternalWatchRoot,
    PreparedExternalMcpServer, PreparedExternalMcpTransport, ProviderId, SourceKey,
    SourceQualifiedMcpServerId,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn context() -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: Some(PathBuf::from("/workspace")),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

fn revision_key() -> ExternalMcpRevisionKey {
    ExternalMcpRevisionKey::new([7; 32])
}

fn source(source_id: &str) -> ExternalSourceRecord {
    ExternalSourceRecord {
        key: SourceKey::new("fake.mcp", source_id).unwrap(),
        ecosystem_id: EcosystemId::new("fake").unwrap(),
        display_name: format!("Fake {source_id}"),
        source_kind: "fake_mcp_config".to_string(),
        scope: ExternalSourceScope::Project,
        location: format!("/workspace/{source_id}.json"),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
        health: ExternalSourceHealth::Available,
        content_version: "source-v1".to_string(),
        diagnostics: Vec::new(),
    }
}

fn snapshot(source_id: &str, version: &str) -> ExternalMcpProviderSnapshot {
    let source = source(source_id);
    ExternalMcpProviderSnapshot {
        provider: ExternalMcpProviderIdentity::new("fake.mcp", "fake", "Fake MCP").unwrap(),
        sources: vec![source.clone()],
        servers: vec![ExternalMcpServerDefinition {
            id: SourceQualifiedMcpServerId::new(source.key.clone(), "github").unwrap(),
            provenance: vec![source.key],
            name: "github".to_string(),
            transport: ExternalMcpTransportKind::StreamableHttp,
            command_preview: None,
            argument_count: 0,
            working_directory: None,
            environment_keys: Vec::new(),
            environment_reference_names: Vec::new(),
            remote_url_preview: Some("https://example.test/mcp".to_string()),
            header_names: Vec::new(),
            source_enabled: true,
            behavior_version: version.to_string(),
            static_status: ExternalMcpStaticStatus::Ready,
        }],
        diagnostics: Vec::new(),
    }
}

fn merged_snapshot() -> ExternalMcpProviderSnapshot {
    let global = source("global");
    let project = source("project");
    ExternalMcpProviderSnapshot {
        provider: ExternalMcpProviderIdentity::new("fake.mcp", "fake", "Fake MCP").unwrap(),
        sources: vec![global.clone(), project.clone()],
        servers: vec![ExternalMcpServerDefinition {
            id: SourceQualifiedMcpServerId::new(project.key.clone(), "github").unwrap(),
            provenance: vec![global.key, project.key],
            name: "github".to_string(),
            transport: ExternalMcpTransportKind::StreamableHttp,
            command_preview: None,
            argument_count: 0,
            working_directory: None,
            environment_keys: Vec::new(),
            environment_reference_names: Vec::new(),
            remote_url_preview: Some("https://example.test/mcp".to_string()),
            header_names: Vec::new(),
            source_enabled: true,
            behavior_version: "behavior-v1".to_string(),
            static_status: ExternalMcpStaticStatus::Ready,
        }],
        diagnostics: Vec::new(),
    }
}

struct FakeProvider {
    result: Mutex<Result<ExternalMcpProviderSnapshot, ExternalSourceProviderError>>,
    observed_suppression: Mutex<Vec<BTreeSet<SourceKey>>>,
}

impl FakeProvider {
    fn new(snapshot: ExternalMcpProviderSnapshot) -> Self {
        Self {
            result: Mutex::new(Ok(snapshot)),
            observed_suppression: Mutex::new(Vec::new()),
        }
    }

    fn replace(&self, result: Result<ExternalMcpProviderSnapshot, ExternalSourceProviderError>) {
        *self.result.lock().unwrap() = result;
    }
}

impl ExternalMcpSourceProvider for FakeProvider {
    fn identity(&self) -> ExternalMcpProviderIdentity {
        ExternalMcpProviderIdentity::new("fake.mcp", "fake", "Fake MCP").unwrap()
    }

    fn discover(
        &self,
        input: &ExternalMcpDiscoveryInput,
    ) -> Result<ExternalMcpProviderSnapshot, ExternalSourceProviderError> {
        self.observed_suppression
            .lock()
            .unwrap()
            .push(input.suppressed_sources.clone());
        self.result.lock().unwrap().clone()
    }

    fn prepare_server(
        &self,
        _input: &ExternalMcpDiscoveryInput,
        server_id: &SourceQualifiedMcpServerId,
        expected_behavior_version: &str,
    ) -> Result<PreparedExternalMcpServer, ExternalSourceProviderError> {
        Ok(PreparedExternalMcpServer {
            id: server_id.clone(),
            behavior_version: expected_behavior_version.to_string(),
            transport: PreparedExternalMcpTransport::Remote {
                url: "https://example.test/mcp".to_string(),
                headers: BTreeMap::new(),
                oauth_enabled: true,
            },
        })
    }

    fn watch_roots(&self, _context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        vec![
            ExternalWatchRoot {
                path: PathBuf::from("/workspace/.opencode"),
                recursive: false,
            },
            ExternalWatchRoot {
                path: PathBuf::from("/workspace/.opencode"),
                recursive: true,
            },
        ]
    }
}

#[test]
fn coordinator_isolates_failures_keeps_last_success_and_withdraws_stable_deletions() {
    let provider = Arc::new(FakeProvider::new(snapshot("project", "behavior-v1")));
    let provider_trait: Arc<dyn ExternalMcpSourceProvider> = provider.clone();
    let mut coordinator =
        ExternalMcpCoordinator::new(context(), revision_key(), vec![provider_trait]).unwrap();

    let first = coordinator.refresh();
    assert!(!first.discovery_pending);
    assert_eq!(first.servers.len(), 1);
    assert_eq!(
        first.sources[0].lifecycle,
        ExternalSourceLifecycleState::Available
    );

    provider.replace(Err(ExternalSourceProviderError::new(
        "fake.mcp.temporarily_unreadable",
        "temporary failure",
        true,
    )));
    let degraded = coordinator.refresh();
    assert_eq!(degraded.servers.len(), 1);
    assert_eq!(
        degraded.sources[0].lifecycle,
        ExternalSourceLifecycleState::UsingLastValidVersion
    );
    assert!(degraded
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "fake.mcp.temporarily_unreadable"));

    provider.replace(Ok(ExternalMcpProviderSnapshot {
        provider: provider.identity(),
        sources: Vec::new(),
        servers: Vec::new(),
        diagnostics: Vec::new(),
    }));
    let removed = coordinator.refresh();
    assert!(removed.sources.is_empty());
    assert!(removed.servers.is_empty());
}

#[test]
fn coordinator_passes_suppression_to_the_provider_and_guards_preparation_revision() {
    let provider = Arc::new(FakeProvider::new(snapshot("project", "behavior-v1")));
    let provider_trait: Arc<dyn ExternalMcpSourceProvider> = provider.clone();
    let mut coordinator =
        ExternalMcpCoordinator::new(context(), revision_key(), vec![provider_trait]).unwrap();
    let first = coordinator.refresh();
    let server = first.servers[0].clone();
    let source_key = first.sources[0].stable_key.clone();

    coordinator.set_source_enabled(&source_key, false).unwrap();
    coordinator.refresh();
    assert!(provider
        .observed_suppression
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .contains(&server.id.source));

    assert!(coordinator
        .prepare_server_guarded(&server.id, "behavior-v0")
        .is_err());
    coordinator.set_source_enabled(&source_key, true).unwrap();
    coordinator.refresh();
    coordinator
        .prepare_server_guarded(&server.id, "behavior-v1")
        .expect("current approved revision can be prepared");
}

#[test]
fn coordinator_keeps_last_success_for_a_known_unavailable_source() {
    let provider = Arc::new(FakeProvider::new(snapshot("project", "behavior-v1")));
    let provider_trait: Arc<dyn ExternalMcpSourceProvider> = provider.clone();
    let mut coordinator =
        ExternalMcpCoordinator::new(context(), revision_key(), vec![provider_trait]).unwrap();
    let first = coordinator.refresh();
    assert_eq!(first.servers.len(), 1);

    let mut unavailable = source("project");
    unavailable.health = ExternalSourceHealth::Unavailable;
    provider.replace(Ok(ExternalMcpProviderSnapshot {
        provider: provider.identity(),
        sources: vec![unavailable],
        servers: Vec::new(),
        diagnostics: Vec::new(),
    }));

    let degraded = coordinator.refresh();
    assert_eq!(degraded.servers.len(), 1);
    assert_eq!(
        degraded.sources[0].lifecycle,
        ExternalSourceLifecycleState::UsingLastValidVersion
    );
    assert!(degraded
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "external_mcp.source_refresh_failed"));
}

#[test]
fn coordinator_withdraws_merged_server_when_any_provenance_source_is_suppressed() {
    let provider = Arc::new(FakeProvider::new(merged_snapshot()));
    let provider_trait: Arc<dyn ExternalMcpSourceProvider> = provider.clone();
    let mut coordinator =
        ExternalMcpCoordinator::new(context(), revision_key(), vec![provider_trait]).unwrap();
    let first = coordinator.refresh();
    assert_eq!(first.servers.len(), 1);

    let global_source = first
        .sources
        .iter()
        .find(|source| source.record.key.source_id.as_str() == "global")
        .unwrap();
    coordinator
        .set_source_enabled(&global_source.stable_key, false)
        .unwrap();
    provider.replace(Err(ExternalSourceProviderError::new(
        "fake.mcp.temporarily_unreadable",
        "temporary failure",
        true,
    )));

    let degraded = coordinator.refresh();
    assert!(degraded.servers.is_empty());
    assert_eq!(
        degraded
            .sources
            .iter()
            .find(|source| source.record.key.source_id.as_str() == "global")
            .unwrap()
            .lifecycle,
        ExternalSourceLifecycleState::Suppressed
    );
}

#[test]
fn coordinator_deduplicates_watch_roots_and_rejects_duplicate_providers() {
    let provider = Arc::new(FakeProvider::new(snapshot("project", "behavior-v1")));
    let provider_trait: Arc<dyn ExternalMcpSourceProvider> = provider.clone();
    let coordinator =
        ExternalMcpCoordinator::new(context(), revision_key(), vec![provider_trait.clone()])
            .unwrap();
    assert_eq!(
        coordinator.watch_roots(),
        vec![ExternalWatchRoot {
            path: PathBuf::from("/workspace/.opencode"),
            recursive: true,
        }]
    );
    let duplicate = ExternalMcpCoordinator::new(
        context(),
        revision_key(),
        vec![provider_trait.clone(), provider_trait],
    );
    assert!(duplicate.is_err());
    assert_eq!(
        provider.identity().provider_id,
        ProviderId::new("fake.mcp").unwrap()
    );
}
