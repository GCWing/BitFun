use openbitfun_product_domains::external_sources::{
    EcosystemId, ExternalSourceAssetKind, ExternalSourceCatalogEntry, ExternalSourceContext,
    ExternalSourceDiagnostic, ExternalSourceDiagnosticSeverity, ExternalSourceLifecycleState,
    ExternalSourceProviderError, ExternalWatchRoot, ProviderId,
};
use openbitfun_product_domains::workspace_references::{
    ExternalWorkspaceReferenceDefinition, ExternalWorkspaceReferenceProviderIdentity,
    ExternalWorkspaceReferenceProviderSnapshot, ExternalWorkspaceReferenceSourceProvider,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

impl crate::DiscoveryRequest for ExternalWorkspaceReferenceDiscoveryRequest {
    type Result = ExternalWorkspaceReferenceDiscoveryResult;

    const DIAGNOSTIC_PREFIX: &'static str = "external_workspace_reference";
    const PROVIDER_LABEL: &'static str = "workspace reference";

    fn provider_id(&self) -> ProviderId {
        self.provider_id.clone()
    }

    fn execute(self) -> Self::Result {
        ExternalWorkspaceReferenceDiscoveryRequest::execute(self)
    }

    fn failed(provider_id: ProviderId, error: ExternalSourceProviderError) -> Self::Result {
        ExternalWorkspaceReferenceDiscoveryResult::failed(provider_id, error)
    }
}

struct ProviderGeneration {
    provider: Arc<dyn ExternalWorkspaceReferenceSourceProvider>,
    identity: ExternalWorkspaceReferenceProviderIdentity,
    initial_result_received: bool,
    last_success: Option<ExternalWorkspaceReferenceProviderSnapshot>,
    last_error: Option<ExternalSourceProviderError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWorkspaceReferenceCoordinatorSnapshot {
    pub generation: u64,
    pub discovery_pending: bool,
    pub sources: Vec<ExternalSourceCatalogEntry>,
    pub references: Vec<ExternalWorkspaceReferenceDefinition>,
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

pub struct ExternalWorkspaceReferenceDiscoveryRequest {
    provider_id: ProviderId,
    ecosystem_id: EcosystemId,
    provider: Arc<dyn ExternalWorkspaceReferenceSourceProvider>,
    context: ExternalSourceContext,
}

impl ExternalWorkspaceReferenceDiscoveryRequest {
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    pub fn ecosystem_id(&self) -> &EcosystemId {
        &self.ecosystem_id
    }

    pub fn disabled(self) -> ExternalWorkspaceReferenceDiscoveryResult {
        ExternalWorkspaceReferenceDiscoveryResult {
            provider_id: self.provider_id,
            candidate: Ok(ExternalWorkspaceReferenceProviderSnapshot {
                provider: self.provider.identity(),
                sources: Vec::new(),
                references: Vec::new(),
                diagnostics: Vec::new(),
            }),
        }
    }

    pub fn execute(self) -> ExternalWorkspaceReferenceDiscoveryResult {
        ExternalWorkspaceReferenceDiscoveryResult {
            provider_id: self.provider_id,
            candidate: self.provider.discover(&self.context),
        }
    }
}

#[derive(Clone)]
pub struct ExternalWorkspaceReferenceDiscoveryResult {
    provider_id: ProviderId,
    candidate: Result<ExternalWorkspaceReferenceProviderSnapshot, ExternalSourceProviderError>,
}

impl ExternalWorkspaceReferenceDiscoveryResult {
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    pub fn failed(provider_id: ProviderId, error: ExternalSourceProviderError) -> Self {
        Self {
            provider_id,
            candidate: Err(error),
        }
    }
}

pub struct ExternalWorkspaceReferenceCoordinator {
    context: ExternalSourceContext,
    providers: Vec<ProviderGeneration>,
    suppressed_sources: BTreeSet<String>,
    generation: u64,
    snapshot: ExternalWorkspaceReferenceCoordinatorSnapshot,
}

impl fmt::Debug for ExternalWorkspaceReferenceCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalWorkspaceReferenceCoordinator")
            .field("context", &self.context)
            .field("providers", &self.providers.len())
            .field("suppressed_sources", &self.suppressed_sources)
            .field("generation", &self.generation)
            .finish()
    }
}

impl ExternalWorkspaceReferenceCoordinator {
    pub fn new(
        context: ExternalSourceContext,
        providers: Vec<Arc<dyn ExternalWorkspaceReferenceSourceProvider>>,
    ) -> Result<Self, String> {
        let mut provider_ids = BTreeSet::new();
        let mut generations = Vec::with_capacity(providers.len());
        for provider in providers {
            let identity = provider.identity();
            if !provider_ids.insert(identity.provider_id.clone()) {
                return Err(format!(
                    "duplicate workspace reference provider id: {}",
                    identity.provider_id
                ));
            }
            generations.push(ProviderGeneration {
                provider,
                identity,
                initial_result_received: false,
                last_success: None,
                last_error: None,
            });
        }
        let discovery_pending = !generations.is_empty();
        Ok(Self {
            context,
            providers: generations,
            suppressed_sources: BTreeSet::new(),
            generation: 0,
            snapshot: ExternalWorkspaceReferenceCoordinatorSnapshot {
                generation: 0,
                discovery_pending,
                sources: Vec::new(),
                references: Vec::new(),
                diagnostics: Vec::new(),
            },
        })
    }

    pub fn refresh(&mut self) -> ExternalWorkspaceReferenceCoordinatorSnapshot {
        let results = self
            .discovery_requests()
            .into_iter()
            .map(ExternalWorkspaceReferenceDiscoveryRequest::execute)
            .collect();
        self.apply_discovery_results(results)
    }

    pub fn discovery_requests(&self) -> Vec<ExternalWorkspaceReferenceDiscoveryRequest> {
        self.providers
            .iter()
            .map(|generation| ExternalWorkspaceReferenceDiscoveryRequest {
                provider_id: generation.identity.provider_id.clone(),
                ecosystem_id: generation.identity.ecosystem_id.clone(),
                provider: Arc::clone(&generation.provider),
                context: self.context.clone(),
            })
            .collect()
    }

    pub fn apply_discovery_results(
        &mut self,
        results: Vec<ExternalWorkspaceReferenceDiscoveryResult>,
    ) -> ExternalWorkspaceReferenceCoordinatorSnapshot {
        let mut results = results
            .into_iter()
            .map(|result| (result.provider_id, result.candidate))
            .collect::<BTreeMap<_, _>>();
        for generation in &mut self.providers {
            let candidate = results
                .remove(&generation.identity.provider_id)
                .unwrap_or_else(|| {
                    Err(ExternalSourceProviderError::new(
                        "external_workspace_reference.discovery_result_missing",
                        "workspace reference provider discovery did not return a result",
                        true,
                    ))
                });
            apply_provider_candidate(generation, candidate);
        }
        self.rebuild_snapshot()
    }

    pub fn apply_discovery_result(
        &mut self,
        result: ExternalWorkspaceReferenceDiscoveryResult,
    ) -> ExternalWorkspaceReferenceCoordinatorSnapshot {
        if let Some(generation) = self
            .providers
            .iter_mut()
            .find(|generation| generation.identity.provider_id == result.provider_id)
        {
            apply_provider_candidate(generation, result.candidate);
        }
        self.rebuild_snapshot()
    }

    pub fn snapshot(&self) -> ExternalWorkspaceReferenceCoordinatorSnapshot {
        self.snapshot.clone()
    }

    pub fn ecosystem_for_provider(&self, provider_id: &ProviderId) -> Option<EcosystemId> {
        self.providers
            .iter()
            .find(|provider| &provider.identity.provider_id == provider_id)
            .map(|provider| provider.identity.ecosystem_id.clone())
    }

    pub fn set_source_enabled(&mut self, stable_key: &str, enabled: bool) -> Result<(), String> {
        let known = self.providers.iter().any(|provider| {
            provider.last_success.as_ref().is_some_and(|snapshot| {
                snapshot
                    .sources
                    .iter()
                    .any(|source| source.preference_key() == stable_key)
            })
        });
        if !known {
            return Err(format!(
                "unknown external workspace reference source: {stable_key}"
            ));
        }
        if enabled {
            self.suppressed_sources.remove(stable_key);
        } else {
            self.suppressed_sources.insert(stable_key.to_string());
        }
        self.rebuild_snapshot();
        Ok(())
    }

    pub fn replace_suppressed_sources(&mut self, sources: BTreeSet<String>) {
        self.suppressed_sources = sources;
        self.rebuild_snapshot();
    }

    pub fn suppressed_sources(&self) -> &BTreeSet<String> {
        &self.suppressed_sources
    }

    pub fn watch_roots_for_ecosystems(
        &self,
        ecosystems: &BTreeSet<EcosystemId>,
    ) -> Vec<ExternalWatchRoot> {
        let mut roots = BTreeMap::new();
        for provider in &self.providers {
            if !ecosystems.contains(&provider.identity.ecosystem_id) {
                continue;
            }
            for root in provider.provider.watch_roots(&self.context) {
                roots
                    .entry(root.path)
                    .and_modify(|recursive| *recursive |= root.recursive)
                    .or_insert(root.recursive);
            }
        }
        roots
            .into_iter()
            .map(|(path, recursive)| ExternalWatchRoot { path, recursive })
            .collect()
    }

    fn rebuild_snapshot(&mut self) -> ExternalWorkspaceReferenceCoordinatorSnapshot {
        self.generation = self.generation.saturating_add(1);
        let mut sources = Vec::new();
        let mut references = Vec::new();
        let mut diagnostics = Vec::new();
        for provider in &self.providers {
            let use_last_valid = provider
                .last_error
                .as_ref()
                .is_some_and(|error| error.transient)
                && provider.last_success.is_some();
            if let Some(snapshot) = &provider.last_success {
                diagnostics.extend(snapshot.diagnostics.clone());
                for source in &snapshot.sources {
                    let suppressed = self.suppressed_sources.contains(&source.preference_key());
                    let available = provider.last_error.is_none() || use_last_valid;
                    sources.push(ExternalSourceCatalogEntry {
                        stable_key: source.preference_key(),
                        presentation_group_id: None,
                        record: source.clone(),
                        lifecycle: if suppressed {
                            ExternalSourceLifecycleState::Suppressed
                        } else if use_last_valid {
                            ExternalSourceLifecycleState::UsingLastValidVersion
                        } else if available {
                            ExternalSourceLifecycleState::Available
                        } else {
                            ExternalSourceLifecycleState::Unavailable
                        },
                    });
                    if !suppressed && available {
                        references.extend(
                            snapshot
                                .references
                                .iter()
                                .filter(|reference| reference.source == source.key)
                                .cloned(),
                        );
                    }
                }
            }
            if let Some(error) = &provider.last_error {
                diagnostics.push(ExternalSourceDiagnostic {
                    severity: if error.transient {
                        ExternalSourceDiagnosticSeverity::Warning
                    } else {
                        ExternalSourceDiagnosticSeverity::Error
                    },
                    asset_kind: ExternalSourceAssetKind::Reference,
                    code: error.code.clone(),
                    message: error.message.clone(),
                    source: None,
                });
            }
        }
        sources.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        references.sort_by(|left, right| {
            left.alias
                .cmp(&right.alias)
                .then(left.stable_key().cmp(&right.stable_key()))
        });
        self.snapshot = ExternalWorkspaceReferenceCoordinatorSnapshot {
            generation: self.generation,
            discovery_pending: self
                .providers
                .iter()
                .any(|provider| !provider.initial_result_received),
            sources,
            references,
            diagnostics,
        };
        self.snapshot.clone()
    }
}

fn apply_provider_candidate(
    generation: &mut ProviderGeneration,
    candidate: Result<ExternalWorkspaceReferenceProviderSnapshot, ExternalSourceProviderError>,
) {
    generation.initial_result_received = true;
    match candidate {
        Ok(snapshot) => match snapshot.validate() {
            Ok(()) if snapshot.provider == generation.identity => {
                generation.last_success = Some(snapshot);
                generation.last_error = None;
            }
            Ok(()) => {
                generation.last_error = Some(ExternalSourceProviderError::new(
                    "external_workspace_reference.provider_identity_mismatch",
                    "workspace reference provider returned a mismatched identity",
                    false,
                ));
            }
            Err(error) => {
                generation.last_error = Some(ExternalSourceProviderError::new(
                    "external_workspace_reference.snapshot_invalid",
                    error.to_string(),
                    false,
                ));
            }
        },
        Err(error) => generation.last_error = Some(error),
    }
}
