use bitfun_product_domains::external_sources::{
    EcosystemId, ExternalSourceAssetKind, ExternalSourceCatalogEntry, ExternalSourceDiagnostic,
    ExternalSourceDiagnosticSeverity, ExternalSourceLifecycleState, ExternalSourceProviderError,
    ExternalSourceRecord, ExternalWatchRoot, ProviderId,
};
use bitfun_product_domains::external_subagents::{
    ExternalSubagentDefinition, ExternalSubagentDiscoveryInput, ExternalSubagentProviderIdentity,
    ExternalSubagentProviderSnapshot, ExternalSubagentSourceProvider,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

impl crate::DiscoveryRequest for ExternalSubagentDiscoveryRequest {
    type Result = ExternalSubagentDiscoveryResult;

    const DIAGNOSTIC_PREFIX: &'static str = "external_subagent";
    const PROVIDER_LABEL: &'static str = "subagent";

    fn provider_id(&self) -> ProviderId {
        self.provider_id.clone()
    }

    fn execute(self) -> Self::Result {
        ExternalSubagentDiscoveryRequest::execute(self)
    }

    fn failed(provider_id: ProviderId, error: ExternalSourceProviderError) -> Self::Result {
        ExternalSubagentDiscoveryResult::failed(provider_id, error)
    }
}

const EXTERNAL_SUBAGENT_LAST_VALID_MAX_AGE: Duration = Duration::from_secs(30);
const EXTERNAL_SUBAGENT_LAST_VALID_MAX_FAILURES: u32 = 3;

struct SubagentProviderGeneration {
    provider: Arc<dyn ExternalSubagentSourceProvider>,
    identity: ExternalSubagentProviderIdentity,
    initial_result_received: bool,
    last_success: Option<ExternalSubagentProviderSnapshot>,
    transient_failure_since: Option<Instant>,
    consecutive_transient_failures: u32,
    using_last_valid: bool,
    last_error: Option<ExternalSourceProviderError>,
}

#[derive(Debug, Clone)]
pub struct ExternalSubagentCoordinatorSnapshot {
    pub generation: u64,
    pub discovery_pending: bool,
    pub provider_labels: BTreeMap<ProviderId, String>,
    pub sources: Vec<ExternalSourceCatalogEntry>,
    pub definitions: Vec<ExternalSubagentDefinition>,
    pub using_last_valid_provider_ids: Vec<ProviderId>,
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
    pub next_refresh_deadline: Option<Instant>,
}

pub struct ExternalSubagentDiscoveryRequest {
    provider_id: ProviderId,
    ecosystem_id: EcosystemId,
    provider: Arc<dyn ExternalSubagentSourceProvider>,
    input: ExternalSubagentDiscoveryInput,
}

impl ExternalSubagentDiscoveryRequest {
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    pub fn ecosystem_id(&self) -> &EcosystemId {
        &self.ecosystem_id
    }

    pub fn disabled(self) -> ExternalSubagentDiscoveryResult {
        ExternalSubagentDiscoveryResult {
            provider_id: self.provider_id,
            candidate: Ok(ExternalSubagentProviderSnapshot {
                provider: self.provider.identity(),
                sources: Vec::new(),
                definitions: Vec::new(),
                diagnostics: Vec::new(),
            }),
        }
    }

    pub fn input(&self) -> &ExternalSubagentDiscoveryInput {
        &self.input
    }

    pub fn execute(self) -> ExternalSubagentDiscoveryResult {
        ExternalSubagentDiscoveryResult {
            provider_id: self.provider_id,
            candidate: self.provider.discover(&self.input),
        }
    }
}

#[derive(Clone)]
pub struct ExternalSubagentDiscoveryResult {
    provider_id: ProviderId,
    candidate: Result<ExternalSubagentProviderSnapshot, ExternalSourceProviderError>,
}

impl ExternalSubagentDiscoveryResult {
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    pub fn succeeded(provider_id: ProviderId, snapshot: ExternalSubagentProviderSnapshot) -> Self {
        Self {
            provider_id,
            candidate: Ok(snapshot),
        }
    }

    pub fn failed(provider_id: ProviderId, error: ExternalSourceProviderError) -> Self {
        Self {
            provider_id,
            candidate: Err(error),
        }
    }
}

pub struct ExternalSubagentCoordinator {
    context: bitfun_product_domains::external_sources::ExternalSourceContext,
    providers: Vec<SubagentProviderGeneration>,
    suppressed_sources: BTreeSet<String>,
    generation: u64,
    snapshot: ExternalSubagentCoordinatorSnapshot,
}

impl fmt::Debug for ExternalSubagentCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalSubagentCoordinator")
            .field("context", &self.context)
            .field("providers", &self.providers.len())
            .field("suppressed_sources", &self.suppressed_sources)
            .field("generation", &self.generation)
            .finish()
    }
}

impl ExternalSubagentCoordinator {
    pub fn new(
        context: bitfun_product_domains::external_sources::ExternalSourceContext,
        providers: Vec<Arc<dyn ExternalSubagentSourceProvider>>,
    ) -> Result<Self, String> {
        let mut provider_ids = BTreeSet::new();
        let mut generations = Vec::with_capacity(providers.len());
        for provider in providers {
            let identity = provider.identity();
            if !provider_ids.insert(identity.provider_id.clone()) {
                return Err(format!(
                    "duplicate external subagent provider id: {}",
                    identity.provider_id
                ));
            }
            generations.push(SubagentProviderGeneration {
                provider,
                identity,
                initial_result_received: false,
                last_success: None,
                transient_failure_since: None,
                consecutive_transient_failures: 0,
                using_last_valid: false,
                last_error: None,
            });
        }
        let discovery_pending = !generations.is_empty();
        Ok(Self {
            context,
            providers: generations,
            suppressed_sources: BTreeSet::new(),
            generation: 0,
            snapshot: ExternalSubagentCoordinatorSnapshot {
                generation: 0,
                discovery_pending,
                provider_labels: BTreeMap::new(),
                sources: Vec::new(),
                definitions: Vec::new(),
                using_last_valid_provider_ids: Vec::new(),
                diagnostics: Vec::new(),
                next_refresh_deadline: None,
            },
        })
    }

    pub fn refresh(&mut self) -> ExternalSubagentCoordinatorSnapshot {
        let results = self
            .discovery_requests()
            .into_iter()
            .map(ExternalSubagentDiscoveryRequest::execute)
            .collect();
        self.apply_discovery_results_at(results, Instant::now())
    }

    pub fn discovery_requests(&self) -> Vec<ExternalSubagentDiscoveryRequest> {
        let suppressed_sources = self
            .suppressed_sources
            .iter()
            .filter_map(|key| ExternalSourceRecord::source_key_from_preference_key(key))
            .collect::<BTreeSet<_>>();
        self.providers
            .iter()
            .map(|generation| ExternalSubagentDiscoveryRequest {
                provider_id: generation.identity.provider_id.clone(),
                ecosystem_id: generation.identity.ecosystem_id.clone(),
                provider: generation.provider.clone(),
                input: ExternalSubagentDiscoveryInput {
                    context: self.context.clone(),
                    suppressed_sources: suppressed_sources.clone(),
                },
            })
            .collect()
    }

    pub fn apply_discovery_results(
        &mut self,
        results: Vec<ExternalSubagentDiscoveryResult>,
    ) -> ExternalSubagentCoordinatorSnapshot {
        self.apply_discovery_results_at(results, Instant::now())
    }

    pub fn apply_discovery_results_at(
        &mut self,
        results: Vec<ExternalSubagentDiscoveryResult>,
        now: Instant,
    ) -> ExternalSubagentCoordinatorSnapshot {
        let mut results = results
            .into_iter()
            .map(|result| (result.provider_id, result.candidate))
            .collect::<BTreeMap<_, _>>();
        for generation in &mut self.providers {
            let candidate = results
                .remove(&generation.identity.provider_id)
                .unwrap_or_else(|| {
                    Err(ExternalSourceProviderError::new(
                        "external_subagent.discovery_result_missing",
                        "subagent provider discovery did not return a result",
                        true,
                    ))
                });
            apply_provider_candidate(generation, candidate, now);
        }
        self.rebuild_snapshot_at(now)
    }

    pub fn apply_discovery_result(
        &mut self,
        result: ExternalSubagentDiscoveryResult,
    ) -> ExternalSubagentCoordinatorSnapshot {
        let now = Instant::now();
        if let Some(generation) = self
            .providers
            .iter_mut()
            .find(|generation| generation.identity.provider_id == result.provider_id)
        {
            apply_provider_candidate(generation, result.candidate, now);
        }
        self.rebuild_snapshot_at(now)
    }

    pub fn expire_last_valid(&mut self) -> ExternalSubagentCoordinatorSnapshot {
        self.rebuild_snapshot_at(Instant::now())
    }

    pub fn snapshot(&self) -> ExternalSubagentCoordinatorSnapshot {
        self.snapshot.clone()
    }

    pub fn ecosystem_for_provider(&self, provider_id: &ProviderId) -> Option<EcosystemId> {
        self.providers
            .iter()
            .find(|provider| &provider.identity.provider_id == provider_id)
            .map(|provider| provider.identity.ecosystem_id.clone())
    }

    pub fn replace_suppressed_sources(&mut self, sources: BTreeSet<String>) {
        self.suppressed_sources = sources;
        self.rebuild_snapshot_at(Instant::now());
    }

    pub fn suppressed_sources(&self) -> &BTreeSet<String> {
        &self.suppressed_sources
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
            return Err(format!("unknown external subagent source: {stable_key}"));
        }
        if enabled {
            self.suppressed_sources.remove(stable_key);
        } else {
            self.suppressed_sources.insert(stable_key.to_string());
        }
        self.rebuild_snapshot_at(Instant::now());
        Ok(())
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

    fn rebuild_snapshot_at(&mut self, now: Instant) -> ExternalSubagentCoordinatorSnapshot {
        self.generation = self.generation.saturating_add(1);
        let mut sources = Vec::new();
        let mut definitions = Vec::new();
        let mut provider_labels = BTreeMap::new();
        let mut using_last_valid_provider_ids = Vec::new();
        let mut diagnostics = Vec::new();
        let mut next_refresh_deadline: Option<Instant> = None;

        for provider in &mut self.providers {
            provider_labels.insert(
                provider.identity.provider_id.clone(),
                provider.identity.display_name.clone(),
            );
            if provider.using_last_valid
                && provider.transient_failure_since.is_some_and(|started| {
                    now.saturating_duration_since(started) >= EXTERNAL_SUBAGENT_LAST_VALID_MAX_AGE
                })
            {
                provider.using_last_valid = false;
            }
            let usable = provider.last_error.is_none() || provider.using_last_valid;
            if provider.using_last_valid {
                using_last_valid_provider_ids.push(provider.identity.provider_id.clone());
                if let Some(deadline) = provider
                    .transient_failure_since
                    .and_then(|started| started.checked_add(EXTERNAL_SUBAGENT_LAST_VALID_MAX_AGE))
                {
                    next_refresh_deadline = Some(
                        next_refresh_deadline.map_or(deadline, |current| current.min(deadline)),
                    );
                }
            }
            if let Some(snapshot) = &provider.last_success {
                let suppressed_keys = snapshot
                    .sources
                    .iter()
                    .filter(|source| self.suppressed_sources.contains(&source.preference_key()))
                    .map(|source| source.key.clone())
                    .collect::<BTreeSet<_>>();
                for source in &snapshot.sources {
                    let suppressed = suppressed_keys.contains(&source.key);
                    sources.push(ExternalSourceCatalogEntry {
                        stable_key: source.preference_key(),
                        presentation_group_id: None,
                        record: source.clone(),
                        lifecycle: if suppressed {
                            ExternalSourceLifecycleState::Suppressed
                        } else if provider.using_last_valid {
                            ExternalSourceLifecycleState::UsingLastValidVersion
                        } else if !usable {
                            ExternalSourceLifecycleState::Unavailable
                        } else {
                            ExternalSourceLifecycleState::Available
                        },
                    });
                }
                if usable {
                    definitions.extend(
                        snapshot
                            .definitions
                            .iter()
                            .filter(|definition| {
                                !definition.provenance.iter().any(|item| {
                                    suppressed_keys.contains(&item.contribution_id.source)
                                })
                            })
                            .cloned(),
                    );
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
                    asset_kind: ExternalSourceAssetKind::Subagent,
                    code: error.code.clone(),
                    message: error.message.clone(),
                    source: None,
                });
            }
        }
        sources.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        definitions.sort_by(|left, right| {
            left.logical_id
                .cmp(&right.logical_id)
                .then(left.candidate_id.cmp(&right.candidate_id))
        });
        using_last_valid_provider_ids.sort();
        self.snapshot = ExternalSubagentCoordinatorSnapshot {
            generation: self.generation,
            discovery_pending: self
                .providers
                .iter()
                .any(|provider| !provider.initial_result_received),
            provider_labels,
            sources,
            definitions,
            using_last_valid_provider_ids,
            diagnostics,
            next_refresh_deadline,
        };
        self.snapshot.clone()
    }
}

fn apply_provider_candidate(
    generation: &mut SubagentProviderGeneration,
    candidate: Result<ExternalSubagentProviderSnapshot, ExternalSourceProviderError>,
    now: Instant,
) {
    generation.initial_result_received = true;
    match candidate {
        Ok(snapshot) => match snapshot.validate() {
            Ok(()) if snapshot.provider == generation.identity => {
                generation.last_success = Some(snapshot);
                generation.transient_failure_since = None;
                generation.consecutive_transient_failures = 0;
                generation.using_last_valid = false;
                generation.last_error = None;
            }
            Ok(()) => apply_failure(
                generation,
                ExternalSourceProviderError::new(
                    "external_subagent.provider_identity_mismatch",
                    "subagent provider returned a mismatched identity",
                    false,
                ),
                now,
            ),
            Err(error) => apply_failure(
                generation,
                ExternalSourceProviderError::new(
                    "external_subagent.snapshot_invalid",
                    error.to_string(),
                    false,
                ),
                now,
            ),
        },
        Err(error) => apply_failure(generation, error, now),
    }
}

fn apply_failure(
    generation: &mut SubagentProviderGeneration,
    error: ExternalSourceProviderError,
    now: Instant,
) {
    if error.transient {
        generation.transient_failure_since.get_or_insert(now);
        generation.consecutive_transient_failures =
            generation.consecutive_transient_failures.saturating_add(1);
        generation.using_last_valid = generation.last_success.is_some()
            && generation.consecutive_transient_failures
                < EXTERNAL_SUBAGENT_LAST_VALID_MAX_FAILURES
            && generation.transient_failure_since.is_some_and(|started| {
                now.saturating_duration_since(started) < EXTERNAL_SUBAGENT_LAST_VALID_MAX_AGE
            });
    } else {
        generation.transient_failure_since = None;
        generation.consecutive_transient_failures = 0;
        generation.using_last_valid = false;
    }
    generation.last_error = Some(error);
}
