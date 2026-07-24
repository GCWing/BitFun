use bitfun_product_domains::external_sources::{
    EcosystemId, ExternalMcpDiscoveryInput, ExternalMcpProviderIdentity,
    ExternalMcpProviderSnapshot, ExternalMcpRevisionKey, ExternalMcpServerDefinition,
    ExternalMcpSourceProvider, ExternalMcpStaticStatus, ExternalSourceAssetKind,
    ExternalSourceCatalogEntry, ExternalSourceContext, ExternalSourceDiagnostic,
    ExternalSourceHealth, ExternalSourceLifecycleState, ExternalSourceProviderError,
    ExternalWatchRoot, PreparedExternalMcpServer, ProviderId, SourceKey,
    SourceQualifiedMcpServerId,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

impl crate::DiscoveryRequest for ExternalMcpDiscoveryRequest {
    type Result = ExternalMcpDiscoveryResult;

    const DIAGNOSTIC_PREFIX: &'static str = "external_mcp";
    const PROVIDER_LABEL: &'static str = "MCP";

    fn provider_id(&self) -> ProviderId {
        self.provider_id.clone()
    }

    fn execute(self) -> Self::Result {
        ExternalMcpDiscoveryRequest::execute(self)
    }

    fn failed(provider_id: ProviderId, error: ExternalSourceProviderError) -> Self::Result {
        ExternalMcpDiscoveryResult::failed(provider_id, error)
    }
}

struct McpProviderGeneration {
    provider: Arc<dyn ExternalMcpSourceProvider>,
    identity: ExternalMcpProviderIdentity,
    initial_result_received: bool,
    last_success: Option<ExternalMcpProviderSnapshot>,
    last_error: Option<ExternalSourceProviderError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMcpCoordinatorSnapshot {
    pub generation: u64,
    pub discovery_pending: bool,
    pub sources: Vec<ExternalSourceCatalogEntry>,
    pub servers: Vec<ExternalMcpServerDefinition>,
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

/// One provider-neutral discovery unit that product assembly may schedule in
/// parallel without teaching the coordinator about a concrete ecosystem.
pub struct ExternalMcpDiscoveryRequest {
    provider_id: ProviderId,
    ecosystem_id: EcosystemId,
    provider: Arc<dyn ExternalMcpSourceProvider>,
    input: ExternalMcpDiscoveryInput,
}

impl ExternalMcpDiscoveryRequest {
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    pub fn ecosystem_id(&self) -> &EcosystemId {
        &self.ecosystem_id
    }

    pub fn disabled(self) -> ExternalMcpDiscoveryResult {
        ExternalMcpDiscoveryResult {
            provider_id: self.provider_id,
            candidate: Ok(ExternalMcpProviderSnapshot {
                provider: self.provider.identity(),
                sources: Vec::new(),
                servers: Vec::new(),
                diagnostics: Vec::new(),
            }),
        }
    }

    pub fn execute(self) -> ExternalMcpDiscoveryResult {
        ExternalMcpDiscoveryResult {
            provider_id: self.provider_id,
            candidate: self.provider.discover(&self.input),
        }
    }
}

#[derive(Clone)]
pub struct ExternalMcpDiscoveryResult {
    provider_id: ProviderId,
    candidate: Result<ExternalMcpProviderSnapshot, ExternalSourceProviderError>,
}

impl ExternalMcpDiscoveryResult {
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

/// Coordinates lifecycle and stale-revision guards for executable MCP
/// configuration while remaining independent of the source ecosystem.
pub struct ExternalMcpCoordinator {
    context: ExternalSourceContext,
    revision_key: ExternalMcpRevisionKey,
    providers: Vec<McpProviderGeneration>,
    suppressed_sources: BTreeSet<String>,
    generation: u64,
    snapshot: ExternalMcpCoordinatorSnapshot,
}

impl fmt::Debug for ExternalMcpCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalMcpCoordinator")
            .field("context", &self.context)
            .field("providers", &self.providers.len())
            .field("suppressed_sources", &self.suppressed_sources)
            .field("generation", &self.generation)
            .finish()
    }
}

impl ExternalMcpCoordinator {
    pub fn new(
        context: ExternalSourceContext,
        revision_key: ExternalMcpRevisionKey,
        providers: Vec<Arc<dyn ExternalMcpSourceProvider>>,
    ) -> Result<Self, String> {
        let mut provider_ids = BTreeSet::new();
        let mut generations = Vec::with_capacity(providers.len());
        for provider in providers {
            let identity = provider.identity();
            if !provider_ids.insert(identity.provider_id.clone()) {
                return Err(format!(
                    "duplicate external MCP provider id: {}",
                    identity.provider_id
                ));
            }
            generations.push(McpProviderGeneration {
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
            revision_key,
            providers: generations,
            suppressed_sources: BTreeSet::new(),
            generation: 0,
            snapshot: ExternalMcpCoordinatorSnapshot {
                generation: 0,
                discovery_pending,
                sources: Vec::new(),
                servers: Vec::new(),
                diagnostics: Vec::new(),
            },
        })
    }

    pub fn refresh(&mut self) -> ExternalMcpCoordinatorSnapshot {
        let results = self
            .discovery_requests()
            .into_iter()
            .map(ExternalMcpDiscoveryRequest::execute)
            .collect();
        self.apply_discovery_results(results)
    }

    pub fn discovery_requests(&self) -> Vec<ExternalMcpDiscoveryRequest> {
        let suppressed_sources = self.suppressed_source_keys();
        self.providers
            .iter()
            .map(|generation| ExternalMcpDiscoveryRequest {
                provider_id: generation.identity.provider_id.clone(),
                ecosystem_id: generation.identity.ecosystem_id.clone(),
                provider: Arc::clone(&generation.provider),
                input: ExternalMcpDiscoveryInput {
                    context: self.context.clone(),
                    suppressed_sources: suppressed_sources.clone(),
                    revision_key: self.revision_key.clone(),
                },
            })
            .collect()
    }

    pub fn apply_discovery_results(
        &mut self,
        results: Vec<ExternalMcpDiscoveryResult>,
    ) -> ExternalMcpCoordinatorSnapshot {
        let mut results = results
            .into_iter()
            .map(|result| (result.provider_id, result.candidate))
            .collect::<BTreeMap<_, _>>();
        for generation in &mut self.providers {
            let candidate = results
                .remove(&generation.identity.provider_id)
                .unwrap_or_else(|| {
                    Err(ExternalSourceProviderError::new(
                        "external_mcp.discovery_result_missing",
                        "MCP provider discovery did not return a result",
                        true,
                    ))
                });
            apply_provider_candidate(generation, candidate);
        }
        self.rebuild_snapshot()
    }

    pub fn apply_discovery_result(
        &mut self,
        result: ExternalMcpDiscoveryResult,
    ) -> ExternalMcpCoordinatorSnapshot {
        if let Some(generation) = self
            .providers
            .iter_mut()
            .find(|generation| generation.identity.provider_id == result.provider_id)
        {
            apply_provider_candidate(generation, result.candidate);
        }
        self.rebuild_snapshot()
    }

    pub fn snapshot(&self) -> ExternalMcpCoordinatorSnapshot {
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
            return Err(format!("unknown external MCP source: {stable_key}"));
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

    pub fn prepare_server_guarded(
        &self,
        server_id: &SourceQualifiedMcpServerId,
        expected_behavior_version: &str,
    ) -> Result<PreparedExternalMcpServer, ExternalSourceProviderError> {
        let current = self.snapshot.servers.iter().find(|server| {
            &server.id == server_id && server.behavior_version == expected_behavior_version
        });
        let Some(server) = current else {
            return Err(ExternalSourceProviderError::new(
                "external_mcp.stale_revision",
                "MCP server is not available at the requested revision",
                true,
            ));
        };
        if !server.source_enabled || !matches!(server.static_status, ExternalMcpStaticStatus::Ready)
        {
            return Err(ExternalSourceProviderError::new(
                "external_mcp.server_unavailable",
                "MCP server configuration is not available for activation",
                false,
            ));
        }
        let provider = self
            .providers
            .iter()
            .find(|provider| provider.identity.provider_id == server_id.source.provider_id)
            .ok_or_else(|| {
                ExternalSourceProviderError::new(
                    "external_mcp.provider_missing",
                    "MCP provider is not registered",
                    false,
                )
            })?;
        let prepared = provider.provider.prepare_server(
            &ExternalMcpDiscoveryInput {
                context: self.context.clone(),
                suppressed_sources: self.suppressed_source_keys(),
                revision_key: self.revision_key.clone(),
            },
            server_id,
            expected_behavior_version,
        )?;
        if prepared.id != *server_id || prepared.behavior_version != expected_behavior_version {
            return Err(ExternalSourceProviderError::new(
                "external_mcp.prepared_server_mismatch",
                "MCP provider prepared a different server revision",
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

    fn suppressed_source_keys(&self) -> BTreeSet<SourceKey> {
        self.providers
            .iter()
            .filter_map(|provider| provider.last_success.as_ref())
            .flat_map(|snapshot| snapshot.sources.iter())
            .filter(|source| self.suppressed_sources.contains(&source.preference_key()))
            .map(|source| source.key.clone())
            .collect()
    }

    fn rebuild_snapshot(&mut self) -> ExternalMcpCoordinatorSnapshot {
        self.generation = self.generation.saturating_add(1);
        let mut sources = Vec::new();
        let mut servers = Vec::new();
        let mut diagnostics = Vec::new();
        let suppressed_source_keys = self.suppressed_source_keys();
        for provider in &self.providers {
            let failed = provider.last_error.is_some();
            if let Some(snapshot) = &provider.last_success {
                for source in &snapshot.sources {
                    let suppressed = self.suppressed_sources.contains(&source.preference_key());
                    sources.push(ExternalSourceCatalogEntry {
                        stable_key: source.preference_key(),
                        presentation_group_id: None,
                        record: source.clone(),
                        lifecycle: source_lifecycle(source.health, suppressed, failed),
                    });
                    if !suppressed {
                        servers.extend(
                            snapshot
                                .servers
                                .iter()
                                .filter(|server| {
                                    server.id.source == source.key
                                        && server.provenance.iter().all(|source_key| {
                                            !suppressed_source_keys.contains(source_key)
                                        })
                                })
                                .cloned(),
                        );
                    }
                }
                diagnostics.extend(snapshot.diagnostics.clone());
            }
            if let Some(error) = &provider.last_error {
                let diagnostic = if error.transient {
                    ExternalSourceDiagnostic::warning(
                        error.code.clone(),
                        error.message.clone(),
                        None,
                    )
                } else {
                    ExternalSourceDiagnostic::error(error.code.clone(), error.message.clone(), None)
                };
                diagnostics.push(diagnostic.with_asset_kind(ExternalSourceAssetKind::Mcp));
            }
        }
        sources.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        servers.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        self.snapshot = ExternalMcpCoordinatorSnapshot {
            generation: self.generation,
            discovery_pending: self
                .providers
                .iter()
                .any(|provider| !provider.initial_result_received),
            sources,
            servers,
            diagnostics,
        };
        self.snapshot.clone()
    }
}

fn source_lifecycle(
    health: ExternalSourceHealth,
    suppressed: bool,
    using_last_valid: bool,
) -> ExternalSourceLifecycleState {
    if suppressed {
        return ExternalSourceLifecycleState::Suppressed;
    }
    if using_last_valid {
        return ExternalSourceLifecycleState::UsingLastValidVersion;
    }
    match health {
        ExternalSourceHealth::Available => ExternalSourceLifecycleState::Available,
        ExternalSourceHealth::Partial => ExternalSourceLifecycleState::Restricted,
        ExternalSourceHealth::Degraded => ExternalSourceLifecycleState::Degraded,
        ExternalSourceHealth::Unavailable => ExternalSourceLifecycleState::Unavailable,
        _ => ExternalSourceLifecycleState::Unavailable,
    }
}

fn apply_provider_candidate(
    generation: &mut McpProviderGeneration,
    candidate: Result<ExternalMcpProviderSnapshot, ExternalSourceProviderError>,
) {
    generation.initial_result_received = true;
    match candidate {
        Ok(snapshot) => match snapshot.validate() {
            Ok(()) if snapshot.provider == generation.identity => {
                let previously_known_sources = generation
                    .last_success
                    .as_ref()
                    .map(|previous| {
                        previous
                            .sources
                            .iter()
                            .map(|source| source.key.clone())
                            .collect::<BTreeSet<_>>()
                    })
                    .unwrap_or_default();
                let previously_known_locations = generation
                    .last_success
                    .as_ref()
                    .map(|previous| {
                        previous
                            .sources
                            .iter()
                            .map(|source| source.location.clone())
                            .collect::<BTreeSet<_>>()
                    })
                    .unwrap_or_default();
                let known_source_refresh_failed = snapshot.sources.iter().any(|source| {
                    source.health == ExternalSourceHealth::Unavailable
                        && (previously_known_sources.contains(&source.key)
                            || previously_known_locations.contains(&source.location))
                });
                if known_source_refresh_failed {
                    generation.last_error = Some(ExternalSourceProviderError::new(
                        "external_mcp.source_refresh_failed",
                        "An MCP source could not be refreshed; using its last valid version",
                        true,
                    ));
                } else {
                    generation.last_success = Some(snapshot);
                    generation.last_error = None;
                }
            }
            Ok(()) => {
                generation.last_error = Some(ExternalSourceProviderError::new(
                    "external_mcp.provider_identity_mismatch",
                    "MCP provider returned a mismatched identity",
                    false,
                ));
            }
            Err(error) => {
                generation.last_error = Some(ExternalSourceProviderError::new(
                    "external_mcp.snapshot_invalid",
                    error.to_string(),
                    false,
                ));
            }
        },
        Err(error) => generation.last_error = Some(error),
    }
}
