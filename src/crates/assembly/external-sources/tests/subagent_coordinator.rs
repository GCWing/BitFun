use bitfun_external_sources::{ExternalSubagentCoordinator, ExternalSubagentDiscoveryResult};
use bitfun_product_domains::external_sources::{
    EcosystemId, ExecutionDomainId, ExternalSourceContext, ExternalSourceHealth,
    ExternalSourceProviderError, ExternalSourceRecord, ExternalSourceScope, ExternalWatchRoot,
    SourceKey,
};
use bitfun_product_domains::external_subagents::{
    external_subagent_candidate_id, ExternalSubagentBehaviorVersion,
    ExternalSubagentCompatibilityState, ExternalSubagentContributionId,
    ExternalSubagentContributionRole, ExternalSubagentDefinition, ExternalSubagentDiscoveryInput,
    ExternalSubagentLocalId, ExternalSubagentMode, ExternalSubagentModelRequest,
    ExternalSubagentProvenanceRef, ExternalSubagentProviderIdentity,
    ExternalSubagentProviderSnapshot, ExternalSubagentSourceProvider, ExternalSubagentToolRequest,
    SecretText,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const EXPECTED_LAST_VALID_MAX_AGE: Duration = Duration::from_secs(30);
const EXPECTED_LAST_VALID_MAX_FAILURES: u32 = 3;

struct FakeProvider {
    identity: ExternalSubagentProviderIdentity,
}

impl FakeProvider {
    fn new(id: &str) -> Self {
        Self {
            identity: ExternalSubagentProviderIdentity::new(id, "fake.ecosystem", "Fake").unwrap(),
        }
    }
}

impl ExternalSubagentSourceProvider for FakeProvider {
    fn identity(&self) -> ExternalSubagentProviderIdentity {
        self.identity.clone()
    }

    fn discover(
        &self,
        _input: &ExternalSubagentDiscoveryInput,
    ) -> Result<ExternalSubagentProviderSnapshot, ExternalSourceProviderError> {
        Ok(snapshot(&self.identity, "v1", "catalog-v1"))
    }

    fn watch_roots(&self, context: &ExternalSourceContext) -> Vec<ExternalWatchRoot> {
        vec![ExternalWatchRoot {
            path: context.workspace_root.clone().unwrap(),
            recursive: true,
        }]
    }
}

fn context() -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: Some(PathBuf::from("C:/workspace")),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

fn snapshot(
    provider: &ExternalSubagentProviderIdentity,
    behavior: &str,
    catalog: &str,
) -> ExternalSubagentProviderSnapshot {
    let source_key = SourceKey::new(provider.provider_id.as_str(), "project-agents").unwrap();
    let provenance = vec![ExternalSubagentProvenanceRef {
        contribution_id: ExternalSubagentContributionId::new(
            source_key.clone(),
            ExternalSubagentLocalId::new("review").unwrap(),
        ),
        role: ExternalSubagentContributionRole::Definition,
    }];
    ExternalSubagentProviderSnapshot {
        provider: provider.clone(),
        sources: vec![ExternalSourceRecord {
            key: source_key,
            ecosystem_id: EcosystemId::new("fake.ecosystem").unwrap(),
            display_name: "Fake project agents".to_string(),
            source_kind: "subagents".to_string(),
            scope: ExternalSourceScope::Project,
            location: "C:/workspace/.fake/agents".to_string(),
            execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
            health: ExternalSourceHealth::Available,
            content_version: catalog.to_string(),
            diagnostics: Vec::new(),
        }],
        definitions: vec![ExternalSubagentDefinition {
            candidate_id: external_subagent_candidate_id(
                &provider.provider_id,
                "review",
                &provenance,
            ),
            logical_id: "review".to_string(),
            provenance,
            display_name: "Review".to_string(),
            description: catalog.to_string(),
            prompt: SecretText::new(format!("prompt-{behavior}")),
            mode: ExternalSubagentMode::Subagent,
            disabled: false,
            hidden: false,
            requested_model: ExternalSubagentModelRequest::Default,
            requested_tools: ExternalSubagentToolRequest {
                selectors: Vec::new(),
                uses_conservative_default: true,
            },
            compatibility: ExternalSubagentCompatibilityState::ReadyWithDegradation,
            diagnostic_codes: vec!["fake.default_tools".to_string()],
            behavior_version: ExternalSubagentBehaviorVersion::new(behavior).unwrap(),
        }],
        diagnostics: Vec::new(),
    }
}

#[test]
fn provider_failures_are_isolated_and_transient_last_valid_is_bounded() {
    let first = Arc::new(FakeProvider::new("fake.first"));
    let second = Arc::new(FakeProvider::new("fake.second"));
    let first_id = first.identity().provider_id;
    let second_id = second.identity().provider_id;
    let mut coordinator = ExternalSubagentCoordinator::new(context(), vec![first, second]).unwrap();
    let started = Instant::now();
    let initial = coordinator.apply_discovery_results_at(
        vec![
            ExternalSubagentDiscoveryResult::succeeded(
                first_id.clone(),
                snapshot(&FakeProvider::new("fake.first").identity(), "v1", "c1"),
            ),
            ExternalSubagentDiscoveryResult::succeeded(
                second_id.clone(),
                snapshot(&FakeProvider::new("fake.second").identity(), "v1", "c1"),
            ),
        ],
        started,
    );
    assert_eq!(initial.definitions.len(), 2);

    for failure in 1..EXPECTED_LAST_VALID_MAX_FAILURES {
        let current = coordinator.apply_discovery_results_at(
            vec![
                ExternalSubagentDiscoveryResult::failed(
                    first_id.clone(),
                    ExternalSourceProviderError::new("fake.transient", "temporary", true),
                ),
                ExternalSubagentDiscoveryResult::succeeded(
                    second_id.clone(),
                    snapshot(
                        &FakeProvider::new("fake.second").identity(),
                        &format!("v{failure}"),
                        "c1",
                    ),
                ),
            ],
            started + Duration::from_secs(u64::from(failure)),
        );
        assert_eq!(current.definitions.len(), 2);
        assert_eq!(
            current.using_last_valid_provider_ids,
            vec![first_id.clone()]
        );
    }

    let expired = coordinator.apply_discovery_results_at(
        vec![
            ExternalSubagentDiscoveryResult::failed(
                first_id.clone(),
                ExternalSourceProviderError::new("fake.transient", "temporary", true),
            ),
            ExternalSubagentDiscoveryResult::succeeded(
                second_id,
                snapshot(&FakeProvider::new("fake.second").identity(), "v3", "c1"),
            ),
        ],
        started + Duration::from_secs(EXPECTED_LAST_VALID_MAX_AGE.as_secs() - 1),
    );
    assert_eq!(
        expired.definitions.len(),
        1,
        "failure bound wins before age bound"
    );
    assert!(expired.using_last_valid_provider_ids.is_empty());
}

#[test]
fn stable_deletion_and_non_transient_failure_never_reuse_last_valid() {
    let provider = Arc::new(FakeProvider::new("fake.agents"));
    let provider_id = provider.identity().provider_id;
    let mut coordinator = ExternalSubagentCoordinator::new(context(), vec![provider]).unwrap();
    let started = Instant::now();
    coordinator.apply_discovery_results_at(
        vec![ExternalSubagentDiscoveryResult::succeeded(
            provider_id.clone(),
            snapshot(&FakeProvider::new("fake.agents").identity(), "v1", "c1"),
        )],
        started,
    );

    let deleted = coordinator.apply_discovery_results_at(
        vec![ExternalSubagentDiscoveryResult::succeeded(
            provider_id.clone(),
            ExternalSubagentProviderSnapshot {
                provider: FakeProvider::new("fake.agents").identity(),
                sources: Vec::new(),
                definitions: Vec::new(),
                diagnostics: Vec::new(),
            },
        )],
        started + Duration::from_secs(1),
    );
    assert!(deleted.definitions.is_empty());

    coordinator.apply_discovery_results_at(
        vec![ExternalSubagentDiscoveryResult::succeeded(
            provider_id.clone(),
            snapshot(&FakeProvider::new("fake.agents").identity(), "v2", "c2"),
        )],
        started + Duration::from_secs(2),
    );
    let unsafe_failure = coordinator.apply_discovery_results_at(
        vec![ExternalSubagentDiscoveryResult::failed(
            provider_id,
            ExternalSourceProviderError::new("fake.unsafe", "unsafe update", false),
        )],
        started + Duration::from_secs(3),
    );
    assert!(unsafe_failure.definitions.is_empty());
}

#[test]
fn suppression_is_forwarded_to_the_provider_and_removes_current_routes() {
    let provider = Arc::new(FakeProvider::new("fake.agents"));
    let provider_id = provider.identity().provider_id;
    let mut coordinator = ExternalSubagentCoordinator::new(context(), vec![provider]).unwrap();
    let snapshot = coordinator.apply_discovery_results_at(
        vec![ExternalSubagentDiscoveryResult::succeeded(
            provider_id,
            snapshot(&FakeProvider::new("fake.agents").identity(), "v1", "c1"),
        )],
        Instant::now(),
    );
    let preference_key = snapshot.sources[0].stable_key.clone();
    coordinator
        .set_source_enabled(&preference_key, false)
        .unwrap();
    assert!(coordinator.snapshot().definitions.is_empty());
    let request = coordinator.discovery_requests().pop().unwrap();
    assert_eq!(request.input().suppressed_sources.len(), 1);
}
