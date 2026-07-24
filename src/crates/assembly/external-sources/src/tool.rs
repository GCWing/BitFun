use bitfun_product_domains::external_sources::{
    EcosystemId, ExternalSourceAssetKind, ExternalSourceCatalogEntry, ExternalSourceContext,
    ExternalSourceDiagnostic, ExternalSourceDiagnosticSeverity, ExternalSourceLifecycleState,
    ExternalSourceProviderError, ExternalToolDefinition, ExternalToolProviderIdentity,
    ExternalToolProviderSnapshot, ExternalToolSourceProvider, ExternalWatchRoot,
    PreparedExternalToolTarget, ProviderId, SourceQualifiedToolTargetId,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

impl crate::DiscoveryRequest for ExternalToolDiscoveryRequest {
    type Result = ExternalToolDiscoveryResult;

    const DIAGNOSTIC_PREFIX: &'static str = "external_tool";
    const PROVIDER_LABEL: &'static str = "tool";

    fn provider_id(&self) -> ProviderId {
        self.provider_id.clone()
    }

    fn execute(self) -> Self::Result {
        ExternalToolDiscoveryRequest::execute(self)
    }

    fn failed(provider_id: ProviderId, error: ExternalSourceProviderError) -> Self::Result {
        ExternalToolDiscoveryResult::failed(provider_id, error)
    }
}

struct ToolProviderGeneration {
    provider: Arc<dyn ExternalToolSourceProvider>,
    identity: ExternalToolProviderIdentity,
    initial_result_received: bool,
    last_success: Option<ExternalToolProviderSnapshot>,
    last_error: Option<ExternalSourceProviderError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalToolCoordinatorSnapshot {
    pub generation: u64,
    pub discovery_pending: bool,
    pub sources: Vec<ExternalSourceCatalogEntry>,
    pub tools: Vec<ExternalToolDefinition>,
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

pub struct ExternalToolDiscoveryRequest {
    provider_id: ProviderId,
    ecosystem_id: EcosystemId,
    provider: Arc<dyn ExternalToolSourceProvider>,
    context: ExternalSourceContext,
}

impl ExternalToolDiscoveryRequest {
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    pub fn ecosystem_id(&self) -> &EcosystemId {
        &self.ecosystem_id
    }

    pub fn disabled(self) -> ExternalToolDiscoveryResult {
        ExternalToolDiscoveryResult {
            provider_id: self.provider_id,
            candidate: Ok(ExternalToolProviderSnapshot {
                provider: self.provider.identity(),
                sources: Vec::new(),
                tools: Vec::new(),
                diagnostics: Vec::new(),
            }),
        }
    }

    pub fn execute(self) -> ExternalToolDiscoveryResult {
        ExternalToolDiscoveryResult {
            provider_id: self.provider_id,
            candidate: self.provider.discover(&self.context),
        }
    }
}

#[derive(Clone)]
pub struct ExternalToolDiscoveryResult {
    provider_id: ProviderId,
    candidate: Result<ExternalToolProviderSnapshot, ExternalSourceProviderError>,
}

impl ExternalToolDiscoveryResult {
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

pub struct ExternalToolCoordinator {
    context: ExternalSourceContext,
    providers: Vec<ToolProviderGeneration>,
    suppressed_sources: BTreeSet<String>,
    generation: u64,
    snapshot: ExternalToolCoordinatorSnapshot,
}

impl fmt::Debug for ExternalToolCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalToolCoordinator")
            .field("context", &self.context)
            .field("providers", &self.providers.len())
            .field("suppressed_sources", &self.suppressed_sources)
            .field("generation", &self.generation)
            .finish()
    }
}

impl ExternalToolCoordinator {
    pub fn new(
        context: ExternalSourceContext,
        providers: Vec<Arc<dyn ExternalToolSourceProvider>>,
    ) -> Result<Self, String> {
        let mut provider_ids = BTreeSet::new();
        let mut generations = Vec::with_capacity(providers.len());
        for provider in providers {
            let identity = provider.identity();
            if !provider_ids.insert(identity.provider_id.clone()) {
                return Err(format!(
                    "duplicate external tool provider id: {}",
                    identity.provider_id
                ));
            }
            generations.push(ToolProviderGeneration {
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
            snapshot: ExternalToolCoordinatorSnapshot {
                generation: 0,
                discovery_pending,
                sources: Vec::new(),
                tools: Vec::new(),
                diagnostics: Vec::new(),
            },
        })
    }

    pub fn refresh(&mut self) -> ExternalToolCoordinatorSnapshot {
        let results = self
            .discovery_requests()
            .into_iter()
            .map(ExternalToolDiscoveryRequest::execute)
            .collect();
        self.apply_discovery_results(results)
    }

    pub fn discovery_requests(&self) -> Vec<ExternalToolDiscoveryRequest> {
        self.providers
            .iter()
            .map(|generation| ExternalToolDiscoveryRequest {
                provider_id: generation.identity.provider_id.clone(),
                ecosystem_id: generation.identity.ecosystem_id.clone(),
                provider: generation.provider.clone(),
                context: self.context.clone(),
            })
            .collect()
    }

    pub fn apply_discovery_results(
        &mut self,
        results: Vec<ExternalToolDiscoveryResult>,
    ) -> ExternalToolCoordinatorSnapshot {
        let mut results = results
            .into_iter()
            .map(|result| (result.provider_id, result.candidate))
            .collect::<BTreeMap<_, _>>();
        for generation in &mut self.providers {
            let candidate = results
                .remove(&generation.identity.provider_id)
                .unwrap_or_else(|| {
                    Err(ExternalSourceProviderError::new(
                        "external_tool.discovery_result_missing",
                        "tool provider discovery did not return a result",
                        true,
                    ))
                });
            apply_tool_provider_candidate(generation, candidate);
        }
        self.rebuild_snapshot()
    }

    pub fn apply_discovery_result(
        &mut self,
        result: ExternalToolDiscoveryResult,
    ) -> ExternalToolCoordinatorSnapshot {
        if let Some(generation) = self
            .providers
            .iter_mut()
            .find(|generation| generation.identity.provider_id == result.provider_id)
        {
            apply_tool_provider_candidate(generation, result.candidate);
        }
        self.rebuild_snapshot()
    }

    pub fn snapshot(&self) -> ExternalToolCoordinatorSnapshot {
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
            return Err(format!("unknown external tool source: {stable_key}"));
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

    pub fn prepare_target_guarded(
        &self,
        target_id: &SourceQualifiedToolTargetId,
        expected_content_version: &str,
    ) -> Result<PreparedExternalToolTarget, ExternalSourceProviderError> {
        let current = self.snapshot.tools.iter().any(|tool| {
            &tool.id.target == target_id && tool.content_version == expected_content_version
        });
        if !current {
            return Err(ExternalSourceProviderError::new(
                "external_tool.stale_revision",
                "tool target is not available at the requested revision",
                true,
            ));
        }
        let provider = self
            .providers
            .iter()
            .find(|provider| provider.identity.provider_id == target_id.source.provider_id)
            .ok_or_else(|| {
                ExternalSourceProviderError::new(
                    "external_tool.provider_missing",
                    "tool provider is not registered",
                    false,
                )
            })?;
        let prepared =
            provider
                .provider
                .prepare_target(&self.context, target_id, expected_content_version)?;
        if prepared.target_id != *target_id || prepared.content_version != expected_content_version
        {
            return Err(ExternalSourceProviderError::new(
                "external_tool.prepared_target_mismatch",
                "tool provider prepared a different target revision",
                false,
            ));
        }
        Ok(prepared)
    }

    pub fn watch_roots(&self) -> Vec<ExternalWatchRoot> {
        let mut roots = BTreeMap::new();
        for provider in &self.providers {
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

    fn rebuild_snapshot(&mut self) -> ExternalToolCoordinatorSnapshot {
        self.generation = self.generation.saturating_add(1);
        let mut sources = Vec::new();
        let mut tools = Vec::new();
        let mut diagnostics = Vec::new();
        for provider in &self.providers {
            let failed = provider.last_error.is_some();
            if let Some(snapshot) = &provider.last_success {
                for source in &snapshot.sources {
                    let suppressed = self.suppressed_sources.contains(&source.preference_key());
                    sources.push(ExternalSourceCatalogEntry {
                        stable_key: source.preference_key(),
                        presentation_group_id: None,
                        record: source.clone(),
                        lifecycle: if suppressed {
                            ExternalSourceLifecycleState::Suppressed
                        } else if failed {
                            ExternalSourceLifecycleState::Unavailable
                        } else {
                            ExternalSourceLifecycleState::Available
                        },
                    });
                    if !suppressed && !failed {
                        tools.extend(
                            snapshot
                                .tools
                                .iter()
                                .filter(|tool| tool.id.target.source == source.key)
                                .cloned(),
                        );
                    }
                }
                diagnostics.extend(snapshot.diagnostics.clone());
            }
            if let Some(error) = &provider.last_error {
                diagnostics.push(ExternalSourceDiagnostic {
                    severity: if error.transient {
                        ExternalSourceDiagnosticSeverity::Warning
                    } else {
                        ExternalSourceDiagnosticSeverity::Error
                    },
                    asset_kind: ExternalSourceAssetKind::Tool,
                    code: error.code.clone(),
                    message: error.message.clone(),
                    source: None,
                });
            }
        }
        sources.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        tools.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        self.snapshot = ExternalToolCoordinatorSnapshot {
            generation: self.generation,
            discovery_pending: self
                .providers
                .iter()
                .any(|provider| !provider.initial_result_received),
            sources,
            tools,
            diagnostics,
        };
        self.snapshot.clone()
    }
}

fn apply_tool_provider_candidate(
    generation: &mut ToolProviderGeneration,
    candidate: Result<ExternalToolProviderSnapshot, ExternalSourceProviderError>,
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
                    "external_tool.provider_identity_mismatch",
                    "tool provider returned a mismatched identity",
                    false,
                ));
            }
            Err(error) => {
                generation.last_error = Some(ExternalSourceProviderError::new(
                    "external_tool.snapshot_invalid",
                    error.to_string(),
                    false,
                ));
            }
        },
        Err(error) => generation.last_error = Some(error),
    }
}
