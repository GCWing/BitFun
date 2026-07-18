//! Ecosystem-neutral external source lifecycle coordination.
//!
//! The coordinator consumes capability-specific provider contracts and never
//! branches on ecosystem identity. Concrete provider selection remains in the
//! product composition root.

mod mcp;
mod subagent;
mod tool;

pub use mcp::{
    ExternalMcpCoordinator, ExternalMcpCoordinatorSnapshot, ExternalMcpDiscoveryRequest,
    ExternalMcpDiscoveryResult,
};
pub use subagent::{
    ExternalSubagentCoordinator, ExternalSubagentCoordinatorSnapshot,
    ExternalSubagentDiscoveryRequest, ExternalSubagentDiscoveryResult,
};
pub use tool::{
    ExternalToolCoordinator, ExternalToolCoordinatorSnapshot, ExternalToolDiscoveryRequest,
    ExternalToolDiscoveryResult,
};

use bitfun_product_domains::external_sources::{
    prompt_command_conflict_key, ExpandedPromptCommand, ExternalSourceCatalogEntry,
    ExternalSourceCatalogSnapshot, ExternalSourceContext, ExternalSourceDiagnostic,
    ExternalSourceHealth, ExternalSourceLifecycleState, ExternalSourceProviderError,
    ExternalSourceRecord, ExternalWatchRoot, PromptCommandAvailability, PromptCommandCatalogEntry,
    PromptCommandConflict, PromptCommandConflictCandidate, PromptCommandDefinition,
    PromptCommandProviderIdentity, PromptCommandProviderSnapshot, PromptCommandSourceProvider,
    ProviderId, SourceKey,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

struct ProviderGeneration {
    provider: Arc<dyn PromptCommandSourceProvider>,
    identity: PromptCommandProviderIdentity,
    initial_result_received: bool,
    last_success: Option<PromptCommandProviderSnapshot>,
    last_usable_sources: BTreeMap<SourceKey, SourceGeneration>,
    using_last_valid_sources: BTreeSet<SourceKey>,
    last_error: Option<ExternalSourceProviderError>,
}

#[derive(Clone)]
struct SourceGeneration {
    record: ExternalSourceRecord,
    commands: Vec<PromptCommandDefinition>,
}

/// A provider-neutral discovery unit that product assembly may schedule with
/// its own concurrency and timeout policy.
pub struct ExternalSourceDiscoveryRequest {
    provider_id: ProviderId,
    provider: Arc<dyn PromptCommandSourceProvider>,
    context: ExternalSourceContext,
}

impl ExternalSourceDiscoveryRequest {
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    pub fn execute(self) -> ExternalSourceDiscoveryResult {
        let candidate = self.provider.discover(&self.context);
        ExternalSourceDiscoveryResult {
            provider_id: self.provider_id,
            candidate,
        }
    }
}

/// Result of one independently scheduled provider discovery.
#[derive(Clone)]
pub struct ExternalSourceDiscoveryResult {
    provider_id: ProviderId,
    candidate: Result<PromptCommandProviderSnapshot, ExternalSourceProviderError>,
}

impl ExternalSourceDiscoveryResult {
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

/// Coordinates provider generations, suppression, degradation, and selection.
pub struct ExternalSourceCoordinator {
    context: ExternalSourceContext,
    providers: Vec<ProviderGeneration>,
    suppressed_sources: BTreeSet<String>,
    conflict_choices: BTreeMap<String, String>,
    conflict_lineage_current_keys: BTreeMap<String, String>,
    conflicted_candidate_ids: BTreeSet<String>,
    removed_sources: BTreeMap<String, ExternalSourceRecord>,
    generation: u64,
    snapshot: ExternalSourceCatalogSnapshot,
}

impl fmt::Debug for ExternalSourceCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalSourceCoordinator")
            .field("context", &self.context)
            .field("providers", &self.providers.len())
            .field("suppressed_sources", &self.suppressed_sources)
            .field("conflict_choices", &self.conflict_choices.len())
            .field("generation", &self.generation)
            .finish()
    }
}

impl ExternalSourceCoordinator {
    pub fn new(
        context: ExternalSourceContext,
        providers: Vec<Arc<dyn PromptCommandSourceProvider>>,
    ) -> Result<Self, String> {
        let mut provider_ids = BTreeSet::new();
        let mut generations = Vec::with_capacity(providers.len());
        for provider in providers {
            let identity = provider.identity();
            let provider_id = identity.provider_id.as_str().to_string();
            if !provider_ids.insert(provider_id.clone()) {
                return Err(format!(
                    "duplicate external source provider id: {provider_id}"
                ));
            }
            generations.push(ProviderGeneration {
                provider,
                identity,
                initial_result_received: false,
                last_success: None,
                last_usable_sources: BTreeMap::new(),
                using_last_valid_sources: BTreeSet::new(),
                last_error: None,
            });
        }
        let discovery_pending = !generations.is_empty();
        Ok(Self {
            context,
            providers: generations,
            suppressed_sources: BTreeSet::new(),
            conflict_choices: BTreeMap::new(),
            conflict_lineage_current_keys: BTreeMap::new(),
            conflicted_candidate_ids: BTreeSet::new(),
            removed_sources: BTreeMap::new(),
            generation: 0,
            snapshot: ExternalSourceCatalogSnapshot {
                generation: 0,
                discovery_pending,
                sources: Vec::new(),
                commands: Vec::new(),
                command_conflicts: Vec::new(),
                tools: Vec::new(),
                tool_approval_requests: Vec::new(),
                tool_conflicts: Vec::new(),
                mcp_generation: 0,
                mcp_servers: Vec::new(),
                mcp_approval_requests: Vec::new(),
                mcp_conflicts: Vec::new(),
                subagent_generation: 0,
                preference_revision: 0,
                subagents: Vec::new(),
                subagent_conflicts: Vec::new(),
                pending_subagent_approvals: Vec::new(),
                diagnostics: Vec::new(),
            },
        })
    }

    pub fn refresh(&mut self) -> ExternalSourceCatalogSnapshot {
        let results = self
            .discovery_requests()
            .into_iter()
            .map(ExternalSourceDiscoveryRequest::execute)
            .collect();
        self.apply_discovery_results(results)
    }

    pub fn discovery_requests(&self) -> Vec<ExternalSourceDiscoveryRequest> {
        self.providers
            .iter()
            .map(|generation| ExternalSourceDiscoveryRequest {
                provider_id: generation.identity.provider_id.clone(),
                provider: Arc::clone(&generation.provider),
                context: self.context.clone(),
            })
            .collect()
    }

    pub fn apply_discovery_results(
        &mut self,
        results: Vec<ExternalSourceDiscoveryResult>,
    ) -> ExternalSourceCatalogSnapshot {
        let mut results = results
            .into_iter()
            .map(|result| (result.provider_id, result.candidate))
            .collect::<BTreeMap<_, _>>();
        for generation in &mut self.providers {
            let candidate = results
                .remove(&generation.identity.provider_id)
                .unwrap_or_else(|| {
                    Err(ExternalSourceProviderError::new(
                        "external_source.discovery_result_missing",
                        "provider discovery did not return a result",
                        true,
                    ))
                });
            apply_provider_candidate(generation, candidate);
        }
        self.rebuild_snapshot()
    }

    pub fn apply_discovery_result(
        &mut self,
        result: ExternalSourceDiscoveryResult,
    ) -> ExternalSourceCatalogSnapshot {
        if let Some(generation) = self
            .providers
            .iter_mut()
            .find(|generation| generation.identity.provider_id == result.provider_id)
        {
            apply_provider_candidate(generation, result.candidate);
        }
        self.rebuild_snapshot()
    }

    pub fn snapshot(&self) -> ExternalSourceCatalogSnapshot {
        self.snapshot.clone()
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
            return Err(format!("unknown external source: {stable_key}"));
        }
        if enabled {
            self.suppressed_sources.remove(stable_key);
        } else {
            self.suppressed_sources.insert(stable_key.to_string());
        }
        self.rebuild_snapshot();
        Ok(())
    }

    pub fn replace_suppressed_sources(&mut self, stable_keys: BTreeSet<String>) {
        self.suppressed_sources = stable_keys;
        self.rebuild_snapshot();
    }

    pub fn suppressed_sources(&self) -> &BTreeSet<String> {
        &self.suppressed_sources
    }

    pub fn replace_conflict_choices(&mut self, choices: BTreeMap<String, String>) {
        self.conflict_choices = choices;
        self.rebuild_snapshot();
    }

    pub fn conflict_choices(&self) -> &BTreeMap<String, String> {
        &self.conflict_choices
    }

    pub fn replace_conflict_lineage_current_keys(&mut self, keys: BTreeMap<String, String>) {
        self.conflict_lineage_current_keys = keys;
        self.rebuild_snapshot();
    }

    pub fn conflict_lineage_current_keys(&self) -> &BTreeMap<String, String> {
        &self.conflict_lineage_current_keys
    }

    pub fn replace_conflicted_candidate_ids(&mut self, candidate_ids: BTreeSet<String>) {
        self.conflicted_candidate_ids = candidate_ids;
        self.rebuild_snapshot();
    }

    pub fn conflicted_candidate_ids(&self) -> &BTreeSet<String> {
        &self.conflicted_candidate_ids
    }

    /// Applies the compact, provider-neutral conflict preference lineage rule.
    /// One current fingerprint is retained per execution-domain/command family,
    /// while candidate identities that have participated in a real conflict
    /// remain marked so a later singleton update still requires confirmation.
    pub fn reconcile_conflict_preferences(
        choices: &mut BTreeMap<String, String>,
        lineage_current_keys: &mut BTreeMap<String, String>,
        conflicted_candidate_ids: &mut BTreeSet<String>,
        conflict_key: &str,
        candidate_id: &str,
        participants: &[String],
    ) {
        let lineage_key = Self::conflict_lineage_key(conflict_key);
        if let Some(previous_key) =
            lineage_current_keys.insert(lineage_key, conflict_key.to_string())
        {
            if previous_key != conflict_key {
                choices.remove(&previous_key);
            }
        }
        if participants.len() > 1 {
            conflicted_candidate_ids.extend(participants.iter().cloned());
        }
        choices.insert(conflict_key.to_string(), candidate_id.to_string());
    }

    pub fn set_conflict_choice(
        &mut self,
        conflict_key: &str,
        candidate_id: &str,
    ) -> Result<(), String> {
        let conflict = self
            .snapshot
            .command_conflicts
            .iter()
            .find(|conflict| conflict.conflict_key == conflict_key)
            .ok_or_else(|| format!("unknown external source conflict: {conflict_key}"))?;
        let candidate = conflict
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == candidate_id)
            .ok_or_else(|| format!("unknown conflict candidate: {candidate_id}"))?;
        if !matches!(candidate.availability, PromptCommandAvailability::Available) {
            return Err(format!(
                "external source conflict candidate is not available: {candidate_id}"
            ));
        }
        let participants = conflict
            .candidates
            .iter()
            .map(|candidate| candidate.candidate_id.clone())
            .collect::<Vec<_>>();
        Self::reconcile_conflict_preferences(
            &mut self.conflict_choices,
            &mut self.conflict_lineage_current_keys,
            &mut self.conflicted_candidate_ids,
            conflict_key,
            candidate_id,
            &participants,
        );
        self.rebuild_snapshot();
        Ok(())
    }

    fn conflict_lineage_key(conflict_key: &str) -> String {
        conflict_key
            .rsplit_once(':')
            .map_or(conflict_key, |(lineage, _)| lineage)
            .to_string()
    }

    pub fn watch_roots(&self) -> Vec<ExternalWatchRoot> {
        let mut roots = self
            .providers
            .iter()
            .flat_map(|provider| provider.provider.watch_roots(&self.context))
            .collect::<Vec<_>>();
        roots.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.recursive.cmp(&right.recursive))
        });
        roots.dedup_by(|left, right| left.path == right.path && left.recursive == right.recursive);
        roots
    }

    pub fn expand_command(
        &self,
        name: &str,
        arguments: &str,
    ) -> Result<ExpandedPromptCommand, ExternalSourceProviderError> {
        self.expand_command_guarded(name, arguments, None, None)
    }

    pub fn expand_command_guarded(
        &self,
        name: &str,
        arguments: &str,
        expected_candidate_id: Option<&str>,
        expected_content_version: Option<&str>,
    ) -> Result<ExpandedPromptCommand, ExternalSourceProviderError> {
        let command = self
            .snapshot
            .commands
            .iter()
            .find(|entry| entry.definition.name.eq_ignore_ascii_case(name))
            .map(|entry| &entry.definition)
            .ok_or_else(|| {
                ExternalSourceProviderError::new(
                    "external_source.command_not_found",
                    format!("external prompt command not found: {name}"),
                    false,
                )
            })?;
        if expected_candidate_id.is_some() != expected_content_version.is_some() {
            return Err(ExternalSourceProviderError::new(
                "external_source.invalid_invocation_guard",
                "external command invocation guard is incomplete",
                false,
            ));
        }
        if let (Some(expected_candidate_id), Some(expected_content_version)) =
            (expected_candidate_id, expected_content_version)
        {
            if command.id.stable_key() != expected_candidate_id
                || command.content_version != expected_content_version
            {
                return Err(ExternalSourceProviderError::new(
                    "external_source.stale_command_selection",
                    "external command changed after it was selected; review the updated command and try again",
                    true,
                ));
            }
        }
        match &command.availability {
            PromptCommandAvailability::Available => {}
            PromptCommandAvailability::Restricted { reason, .. }
            | PromptCommandAvailability::Invalid { reason } => {
                return Err(ExternalSourceProviderError::new(
                    "external_source.command_unavailable",
                    reason.clone(),
                    false,
                ));
            }
            _ => {
                return Err(ExternalSourceProviderError::new(
                    "external_source.command_unavailable",
                    "this command availability state is not supported by this runtime",
                    false,
                ));
            }
        }
        let provider = self
            .providers
            .iter()
            .find(|provider| provider.identity.provider_id == command.id.source.provider_id)
            .ok_or_else(|| {
                ExternalSourceProviderError::new(
                    "external_source.provider_not_found",
                    "provider for the selected command is no longer registered",
                    false,
                )
            })?;
        provider.provider.expand(command, arguments)
    }

    fn rebuild_snapshot(&mut self) -> ExternalSourceCatalogSnapshot {
        self.generation = self.generation.saturating_add(1);
        let mut sources = Vec::new();
        let mut diagnostics = Vec::new();
        let mut command_candidates_by_name: BTreeMap<String, Vec<PromptCommandDefinition>> =
            BTreeMap::new();

        for provider in &self.providers {
            let Some(provider_snapshot) = &provider.last_success else {
                if let Some(error) = &provider.last_error {
                    diagnostics.push(ExternalSourceDiagnostic::error(
                        error.code.clone(),
                        error.message.clone(),
                        None,
                    ));
                }
                continue;
            };

            diagnostics.extend(provider_snapshot.diagnostics.clone());
            if let Some(error) = &provider.last_error {
                diagnostics.push(ExternalSourceDiagnostic::warning(
                    error.code.clone(),
                    error.message.clone(),
                    None,
                ));
            }

            let mut enabled_source_keys = BTreeSet::new();
            for record in &provider_snapshot.sources {
                let stable_key = record.preference_key();
                let lifecycle = if self.suppressed_sources.contains(&stable_key) {
                    ExternalSourceLifecycleState::Suppressed
                } else if provider.using_last_valid_sources.contains(&record.key) {
                    ExternalSourceLifecycleState::UsingLastValidVersion
                } else {
                    match record.health {
                        ExternalSourceHealth::Available => ExternalSourceLifecycleState::Available,
                        ExternalSourceHealth::Partial => ExternalSourceLifecycleState::Restricted,
                        ExternalSourceHealth::Degraded => ExternalSourceLifecycleState::Degraded,
                        ExternalSourceHealth::Unavailable => {
                            ExternalSourceLifecycleState::Unavailable
                        }
                        _ => ExternalSourceLifecycleState::Unavailable,
                    }
                };
                if lifecycle != ExternalSourceLifecycleState::Suppressed {
                    enabled_source_keys.insert(record.key.clone());
                }
                sources.push(ExternalSourceCatalogEntry {
                    stable_key,
                    record: record.clone(),
                    lifecycle,
                });
            }

            match provider
                .provider
                .resolve_commands(&provider_snapshot.commands, &enabled_source_keys)
            {
                Ok(commands) => {
                    for command in commands {
                        if !enabled_source_keys.contains(&command.id.source)
                            || command.id.source.provider_id != provider.identity.provider_id
                            || command.validate().is_err()
                        {
                            diagnostics.push(ExternalSourceDiagnostic::error(
                                "external_source.invalid_resolved_command",
                                "provider returned an invalid resolved command",
                                Some(command.id.source),
                            ));
                            continue;
                        }
                        command_candidates_by_name
                            .entry(command.name.to_ascii_lowercase())
                            .or_default()
                            .push(command);
                    }
                }
                Err(error) => diagnostics.push(ExternalSourceDiagnostic::error(
                    error.code,
                    error.message,
                    None,
                )),
            }
        }

        sources.sort_by(|left, right| left.record.key.cmp(&right.record.key));
        let current_source_keys = sources
            .iter()
            .map(|source| source.stable_key.clone())
            .collect::<BTreeSet<_>>();
        for previous in &self.snapshot.sources {
            if previous.lifecycle != ExternalSourceLifecycleState::Removed
                && !current_source_keys.contains(&previous.stable_key)
            {
                self.removed_sources
                    .insert(previous.stable_key.clone(), previous.record.clone());
            }
        }
        for current in &current_source_keys {
            self.removed_sources.remove(current);
        }
        sources.extend(self.removed_sources.iter().map(|(stable_key, record)| {
            ExternalSourceCatalogEntry {
                stable_key: stable_key.clone(),
                record: record.clone(),
                lifecycle: ExternalSourceLifecycleState::Removed,
            }
        }));
        sources.sort_by(|left, right| left.record.key.cmp(&right.record.key));
        let mut commands = Vec::new();
        let mut command_conflicts = Vec::new();
        for (command_name, mut candidates) in command_candidates_by_name {
            candidates.sort_by(|left, right| left.id.stable_key().cmp(&right.id.stable_key()));
            let requires_reconfirmation = candidates.len() == 1
                && self
                    .conflicted_candidate_ids
                    .contains(&candidates[0].id.stable_key());
            if candidates.len() == 1 && !requires_reconfirmation {
                commands.push(PromptCommandCatalogEntry {
                    definition: candidates.remove(0),
                });
                continue;
            }

            let conflict_candidates = candidates
                .iter()
                .filter_map(|command| {
                    let source = sources
                        .iter()
                        .find(|source| source.record.key == command.id.source)?;
                    Some(PromptCommandConflictCandidate {
                        candidate_id: command.id.stable_key(),
                        source: command.id.source.clone(),
                        source_display_name: source.record.display_name.clone(),
                        ecosystem_id: source.record.ecosystem_id.clone(),
                        content_version: command.content_version.clone(),
                        command_description: command.description.clone(),
                        source_scope: source.record.scope,
                        source_location: source.record.location.clone(),
                        availability: command.availability.clone(),
                    })
                })
                .collect::<Vec<_>>();
            let conflict_key = prompt_command_conflict_key(
                self.context.execution_domain_id.as_str(),
                &command_name,
                conflict_candidates.iter().map(|candidate| {
                    (
                        candidate.candidate_id.as_str(),
                        candidate.content_version.as_str(),
                    )
                }),
            );
            let lineage_key = Self::conflict_lineage_key(&conflict_key);
            if let Some(previous_key) = self.conflict_lineage_current_keys.get(&lineage_key) {
                if previous_key != &conflict_key {
                    self.conflict_choices.remove(previous_key);
                }
            }
            let selected_candidate_id = self
                .conflict_choices
                .get(&conflict_key)
                .filter(|selected| {
                    conflict_candidates
                        .iter()
                        .any(|candidate| &candidate.candidate_id == *selected)
                })
                .cloned();
            if let Some(selected) = &selected_candidate_id {
                if let Some(definition) = candidates
                    .iter()
                    .find(|candidate| candidate.id.stable_key() == *selected)
                {
                    commands.push(PromptCommandCatalogEntry {
                        definition: definition.clone(),
                    });
                }
            }
            command_conflicts.push(PromptCommandConflict {
                conflict_key,
                command_name,
                candidates: conflict_candidates,
                selected_candidate_id,
            });
        }
        commands.sort_by(|left, right| left.definition.name.cmp(&right.definition.name));
        self.snapshot = ExternalSourceCatalogSnapshot {
            generation: self.generation,
            discovery_pending: self
                .providers
                .iter()
                .any(|provider| !provider.initial_result_received),
            sources,
            commands,
            command_conflicts,
            tools: Vec::new(),
            tool_approval_requests: Vec::new(),
            tool_conflicts: Vec::new(),
            mcp_generation: 0,
            mcp_servers: Vec::new(),
            mcp_approval_requests: Vec::new(),
            mcp_conflicts: Vec::new(),
            subagent_generation: 0,
            preference_revision: 0,
            subagents: Vec::new(),
            subagent_conflicts: Vec::new(),
            pending_subagent_approvals: Vec::new(),
            diagnostics,
        };
        self.snapshot.clone()
    }
}

fn mark_provider_last_valid(generation: &mut ProviderGeneration) {
    generation.using_last_valid_sources = generation.last_usable_sources.keys().cloned().collect();
}

fn apply_provider_candidate(
    generation: &mut ProviderGeneration,
    candidate: Result<PromptCommandProviderSnapshot, ExternalSourceProviderError>,
) {
    generation.initial_result_received = true;
    match candidate {
        Ok(mut snapshot) => match snapshot.validate() {
            Ok(()) if snapshot.provider == generation.identity => {
                reconcile_source_generations(generation, &mut snapshot);
                generation.last_success = Some(snapshot);
                generation.last_error = None;
            }
            Ok(()) => {
                mark_provider_last_valid(generation);
                generation.last_error = Some(ExternalSourceProviderError::new(
                    "external_source.provider_identity_changed",
                    "provider returned a snapshot for a different identity",
                    false,
                ));
            }
            Err(error) => {
                mark_provider_last_valid(generation);
                generation.last_error = Some(ExternalSourceProviderError::new(
                    "external_source.invalid_candidate",
                    error.to_string(),
                    false,
                ));
            }
        },
        Err(error) => {
            mark_provider_last_valid(generation);
            generation.last_error = Some(error);
        }
    }
}

fn reconcile_source_generations(
    generation: &mut ProviderGeneration,
    snapshot: &mut PromptCommandProviderSnapshot,
) {
    generation.using_last_valid_sources.clear();
    let present_sources = snapshot
        .sources
        .iter()
        .map(|source| source.key.clone())
        .collect::<BTreeSet<_>>();
    generation
        .last_usable_sources
        .retain(|source, _| present_sources.contains(source));
    let unavailable_command_ids = snapshot
        .unavailable_command_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();

    for record in &mut snapshot.sources {
        let mut current_commands = snapshot
            .commands
            .iter()
            .filter(|command| command.id.source == record.key)
            .cloned()
            .collect::<Vec<_>>();
        if matches!(
            record.health,
            ExternalSourceHealth::Available | ExternalSourceHealth::Partial
        ) {
            generation.last_usable_sources.insert(
                record.key.clone(),
                SourceGeneration {
                    record: record.clone(),
                    commands: current_commands,
                },
            );
            continue;
        }
        let Some(previous) = generation.last_usable_sources.get(&record.key).cloned() else {
            if record.health == ExternalSourceHealth::Degraded && !current_commands.is_empty() {
                generation.last_usable_sources.insert(
                    record.key.clone(),
                    SourceGeneration {
                        record: record.clone(),
                        commands: current_commands,
                    },
                );
            }
            continue;
        };
        if record.health == ExternalSourceHealth::Degraded {
            let current_ids = current_commands
                .iter()
                .map(|command| command.id.clone())
                .collect::<BTreeSet<_>>();
            let recovered = previous
                .commands
                .iter()
                .filter(|command| {
                    unavailable_command_ids.contains(&command.id)
                        && !current_ids.contains(&command.id)
                })
                .cloned()
                .collect::<Vec<_>>();
            if !recovered.is_empty() {
                current_commands.extend(recovered);
                generation
                    .using_last_valid_sources
                    .insert(record.key.clone());
            }
            snapshot
                .commands
                .retain(|command| command.id.source != record.key);
            snapshot.commands.extend(current_commands.clone());
            generation.last_usable_sources.insert(
                record.key.clone(),
                SourceGeneration {
                    record: previous.record,
                    commands: current_commands,
                },
            );
            continue;
        }
        let current_diagnostics = record.diagnostics.clone();
        *record = previous.record;
        record.diagnostics = current_diagnostics;
        snapshot
            .commands
            .retain(|command| command.id.source != record.key);
        snapshot.commands.extend(previous.commands);
        generation
            .using_last_valid_sources
            .insert(record.key.clone());
    }
}
