//! Product-level activation and routing for provider-neutral external subagents.
//!
//! Ecosystem parsing stays in adapters and discovery lifecycle stays in the
//! external-sources coordinator. This module only resolves current BitFun
//! model/tool facts, applies persisted user decisions, and installs immutable
//! generation entries in the existing agent registry.

use crate::agentic::agents::{
    external_subagent_runtime_key, get_agent_registry, AgentInfo, AgentSource,
    ExternalProvidedSubagent, ExternalSubagentModelBinding, ExternalSubagentRegistration,
    ExternalSubagentRoute,
};
use crate::agentic::tools::registry::get_all_registered_tools;
use crate::external_sources::safe_external_source_location;
use crate::external_tools::resolve_external_tool_for_workspace;
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::types::{model_runtime_binding_fingerprint, AIConfig, AIModelConfig};
use crate::service::config::SubagentModelSelection;
use crate::util::BitFunError;
use bitfun_external_sources::ExternalSubagentCoordinatorSnapshot;
use bitfun_product_domains::external_sources::EcosystemId;
use bitfun_product_domains::external_sources::{ExternalSourceScope, ProviderId, SourceKey};
use bitfun_product_domains::external_subagents::{
    external_subagent_approval_key, external_subagent_conflict_key,
    ExternalSubagentActivationState, ExternalSubagentCompatibilityState, ExternalSubagentConflict,
    ExternalSubagentConflictCandidate, ExternalSubagentDefinition,
    ExternalSubagentDiagnosticSummary, ExternalSubagentModelRequest, ExternalSubagentSummary,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub(super) const DISABLED_SUBAGENT_CONFLICT_CHOICE: &str = "__bitfun_disabled__";
static MODEL_CONFIG_UNAVAILABLE_LOGGED: AtomicBool = AtomicBool::new(false);

pub(super) struct ExternalSubagentDecisions<'a> {
    pub active_ecosystems: &'a BTreeSet<EcosystemId>,
    pub approved_envelopes: &'a BTreeSet<String>,
    pub declined_decisions: &'a BTreeMap<String, String>,
    pub conflict_choices: &'a BTreeMap<String, String>,
    pub conflict_lineage_current_keys: &'a BTreeMap<String, String>,
}

#[derive(Default)]
pub(super) struct ExternalSubagentProductState {
    pub summaries: Vec<ExternalSubagentSummary>,
    pub conflicts: Vec<ExternalSubagentConflict>,
    pub pending_approvals: Vec<String>,
    pub registrations: Vec<ExternalSubagentRegistration>,
    pub routes: BTreeMap<String, ExternalSubagentRoute>,
    pub observed_conflict_lineage_current_keys: BTreeMap<String, String>,
}

#[derive(Clone)]
struct ResolvedToolFact {
    name: String,
    binding_fingerprint: String,
    readonly: bool,
}

#[derive(Clone)]
struct LocalCandidateFact {
    logical_id: String,
    candidate_id: String,
    display_name: String,
    source_label: String,
    behavior_version: String,
}

#[derive(Default)]
struct ProductFacts {
    ai_config: Option<AIConfig>,
    tools: BTreeMap<String, ResolvedToolFact>,
    locals: BTreeMap<String, LocalCandidateFact>,
}

struct ResolvedExternalCandidate {
    definition: ExternalSubagentDefinition,
    provider_label: String,
    scope: ExternalSourceScope,
    source_keys: Vec<SourceKey>,
    source_location_labels: Vec<String>,
    model_id: String,
    model_label: String,
    model_configuration_fingerprint: String,
    tools: Vec<ResolvedToolFact>,
    readonly: bool,
    activation_envelope: String,
    approval_key: String,
    conflict_behavior_version: String,
    diagnostics: Vec<ExternalSubagentDiagnosticSummary>,
    compatibility: ExternalSubagentCompatibilityState,
}

pub(super) async fn reconcile_external_subagents(
    workspace_root: Option<&Path>,
    execution_domain_id: &str,
    snapshot: &ExternalSubagentCoordinatorSnapshot,
    decisions: ExternalSubagentDecisions<'_>,
) -> ExternalSubagentProductState {
    let facts = gather_product_facts(workspace_root, &snapshot.definitions).await;
    reconcile_with_facts(
        workspace_root,
        execution_domain_id,
        snapshot,
        decisions,
        &facts,
    )
}

/// Static projection for read-only Hosts. It never reads model configuration,
/// the tool registry, or the agent registry, and it never produces routes or
/// runtime registrations.
pub(super) fn project_external_subagents_read_only(
    workspace_root: Option<&Path>,
    execution_domain_id: &str,
    snapshot: &ExternalSubagentCoordinatorSnapshot,
    decisions: ExternalSubagentDecisions<'_>,
) -> ExternalSubagentProductState {
    let source_map = snapshot
        .sources
        .iter()
        .map(|entry| (entry.record.key.clone(), &entry.record))
        .collect::<BTreeMap<_, _>>();
    let facts = ProductFacts::default();
    let mut state = ExternalSubagentProductState::default();
    for definition in &snapshot.definitions {
        let resolved = resolve_external_candidate(
            workspace_root,
            execution_domain_id,
            definition,
            &source_map,
            &snapshot.provider_labels,
            &facts,
        );
        let ecosystem_active = resolved.source_keys.iter().all(|source_key| {
            source_map
                .get(source_key)
                .is_some_and(|source| decisions.active_ecosystems.contains(&source.ecosystem_id))
        });
        let activation = if !ecosystem_active || resolved.definition.disabled {
            ExternalSubagentActivationState::Disabled
        } else if decisions
            .approved_envelopes
            .contains(&resolved.approval_key)
        {
            ExternalSubagentActivationState::Unavailable
        } else if decisions
            .declined_decisions
            .get(&resolved.approval_key)
            .is_some_and(|decision| decision == &resolved.approval_key)
        {
            ExternalSubagentActivationState::Declined
        } else {
            state.pending_approvals.push(resolved.approval_key.clone());
            ExternalSubagentActivationState::ApprovalRequired
        };
        state.summaries.push(summary_for(&resolved, activation));
    }
    state.summaries.sort_by(|left, right| {
        left.logical_id
            .cmp(&right.logical_id)
            .then(left.candidate_id.cmp(&right.candidate_id))
    });
    state.pending_approvals.sort();
    state.pending_approvals.dedup();
    state
}

async fn gather_product_facts(
    workspace_root: Option<&Path>,
    definitions: &[ExternalSubagentDefinition],
) -> ProductFacts {
    let ai_config = match GlobalConfigManager::get_service().await {
        Ok(service) => match service.get_config::<AIConfig>(Some("ai")).await {
            Ok(config) => {
                MODEL_CONFIG_UNAVAILABLE_LOGGED.store(false, Ordering::Relaxed);
                Some(config)
            }
            Err(error) => {
                log_model_config_unavailable("config_read", &error);
                None
            }
        },
        Err(error) => {
            log_model_config_unavailable("config_service", &error);
            None
        }
    };

    let requested_names = definitions
        .iter()
        .flat_map(|definition| &definition.requested_tools.selectors)
        .filter(|selector| selector.allowed)
        .map(|selector| {
            selector
                .canonical_host_name
                .as_deref()
                .unwrap_or(&selector.source_name)
                .to_string()
        })
        .collect::<BTreeSet<_>>();
    let mut tools = BTreeMap::new();
    for registered in get_all_registered_tools().await {
        let name = registered.name().to_string();
        if !requested_names.contains(&name) {
            continue;
        }
        let Some(selected) = resolve_external_tool_for_workspace(registered, workspace_root) else {
            continue;
        };
        if !selected.is_enabled().await {
            continue;
        }
        let metadata = selected
            .dynamic_tool_info()
            .and_then(|info| serde_json::to_string(&info).ok())
            .unwrap_or_default();
        let readonly = selected.is_readonly();
        tools.insert(
            name.clone(),
            ResolvedToolFact {
                name: name.clone(),
                binding_fingerprint: stable_digest([
                    name.as_str(),
                    metadata.as_str(),
                    if readonly { "readonly" } else { "writable" },
                    if readonly {
                        "unpermissioned"
                    } else {
                        "permissioned"
                    },
                ]),
                readonly,
            },
        );
    }

    let registry = get_agent_registry();
    let mut locals = BTreeMap::new();
    for info in registry
        .get_local_subagents_for_external_resolution(workspace_root)
        .await
    {
        let logical_key = normalize_logical_id(&info.id);
        if locals.contains_key(&logical_key) {
            // AgentRegistry currently resolves the first global entry before a
            // project entry. Preserve that effective local route rather than
            // offering a candidate that the Local route could not execute.
            continue;
        }
        let model = match ai_config.as_ref() {
            Some(ai_config) => {
                let model_selection = registry
                    .get_explicit_subagent_model_selection(&info.id, workspace_root)
                    .unwrap_or_else(|| {
                        ai_config
                            .agent_model_defaults
                            .builtin_subagent_selection(&info.id)
                    });
                serde_json::to_string(&model_selection)
                    .unwrap_or_else(|_| "unavailable".to_string())
            }
            None => "configuration-unavailable".to_string(),
        };
        locals.insert(logical_key, local_candidate_fact(&info, &model));
    }

    ProductFacts {
        ai_config,
        tools,
        locals,
    }
}

fn log_model_config_unavailable(stage: &str, error: &BitFunError) {
    if claim_model_config_outage_log(&MODEL_CONFIG_UNAVAILABLE_LOGGED) {
        log::warn!(
            "External subagent model configuration unavailable: stage={}, category={}",
            stage,
            model_config_error_category(error)
        );
    }
}

fn claim_model_config_outage_log(logged: &AtomicBool) -> bool {
    !logged.swap(true, Ordering::Relaxed)
}

fn model_config_error_category(error: &BitFunError) -> &'static str {
    match error {
        BitFunError::Configuration(message) if message.contains("not initialized") => {
            "service_not_initialized"
        }
        BitFunError::Configuration(message) if message.contains("service is None") => {
            "service_missing"
        }
        BitFunError::Configuration(message)
            if message.contains("Failed to deserialize config value") =>
        {
            "config_deserialization_failed"
        }
        BitFunError::Configuration(message) if message.contains("Config path") => {
            "config_path_unavailable"
        }
        BitFunError::Configuration(_) => "configuration_error",
        BitFunError::Deserialization(_) => "deserialization_error",
        BitFunError::Serialization(_) => "serialization_error",
        BitFunError::Io(_) => "io_error",
        BitFunError::Service(_) => "service_error",
        _ => "other_error",
    }
}

fn local_candidate_fact(info: &AgentInfo, model: &str) -> LocalCandidateFact {
    let mut tools = info.default_tools.clone();
    tools.sort();
    let source = match info.source {
        AgentSource::Builtin => "BitFun built-in",
        AgentSource::User => "BitFun user",
        AgentSource::Project => "BitFun project",
        AgentSource::External => "External",
    };
    let path_identity = info.path.as_deref().unwrap_or_default();
    let candidate_id = format!(
        "local_subagent:{}",
        stable_digest([
            &format!("{:?}", info.source),
            info.id.as_str(),
            path_identity,
        ])
    );
    let behavior_version = stable_digest(
        [
            info.prompt_cache_scope_key.as_str(),
            model,
            if info.is_readonly {
                "readonly"
            } else {
                "writable"
            },
            if info.is_review { "review" } else { "standard" },
            if info.effective_enabled {
                "enabled"
            } else {
                "disabled"
            },
        ]
        .into_iter()
        .chain(tools.iter().map(String::as_str)),
    );
    LocalCandidateFact {
        logical_id: info.id.clone(),
        candidate_id,
        display_name: info.name.clone(),
        source_label: source.to_string(),
        behavior_version,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResolvedModelFact {
    runtime_id: String,
    display_label: String,
    configuration_fingerprint: String,
}

fn model_display_label(model: &AIModelConfig) -> String {
    let provider = model.name.trim();
    let model_name = model.model_name.trim();
    if provider.is_empty() {
        model_name.to_string()
    } else {
        format!("{provider} · {model_name}")
    }
}

fn normalize_model_provider(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn resolve_exact_external_model(
    provider_hint: Option<&str>,
    model_name: &str,
    ai_config: &AIConfig,
) -> Option<ResolvedModelFact> {
    let model_name = model_name.trim();
    if model_name.is_empty() {
        return None;
    }

    let mut matches = ai_config
        .models
        .iter()
        .filter(|model| model.enabled)
        .filter(|model| {
            if provider_hint.is_none() && model.id == model_name {
                return true;
            }
            if model.model_name != model_name {
                return false;
            }
            provider_hint.is_none_or(|provider| {
                [model.provider.as_str(), model.name.as_str()]
                    .into_iter()
                    .map(normalize_model_provider)
                    .any(|candidate| candidate == normalize_model_provider(provider))
            })
        });
    let model = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(ResolvedModelFact {
        runtime_id: model.id.clone(),
        display_label: model_display_label(model),
        configuration_fingerprint: model_runtime_binding_fingerprint(model),
    })
}

fn resolve_bitfun_subagent_model(
    logical_id: &str,
    ai_config: &AIConfig,
) -> Option<ResolvedModelFact> {
    match ai_config
        .agent_model_defaults
        .builtin_subagent_selection(logical_id)
    {
        SubagentModelSelection::Inherit => None,
        SubagentModelSelection::Fixed { model_id } => {
            let requested = model_id.trim();
            if requested.is_empty() {
                return None;
            }
            let runtime_id = ai_config.resolve_model_selection(requested)?;
            let model = ai_config
                .models
                .iter()
                .find(|model| model.enabled && model.id == runtime_id)?;
            Some(ResolvedModelFact {
                runtime_id,
                display_label: model_display_label(model),
                configuration_fingerprint: model_runtime_binding_fingerprint(model),
            })
        }
    }
}

fn reconcile_with_facts(
    workspace_root: Option<&Path>,
    execution_domain_id: &str,
    snapshot: &ExternalSubagentCoordinatorSnapshot,
    decisions: ExternalSubagentDecisions<'_>,
    facts: &ProductFacts,
) -> ExternalSubagentProductState {
    let workspace_scope = workspace_scope_key(workspace_root);
    let source_map = snapshot
        .sources
        .iter()
        .map(|entry| (entry.record.key.clone(), &entry.record))
        .collect::<BTreeMap<_, _>>();
    let mut state = ExternalSubagentProductState::default();
    let mut by_logical = BTreeMap::<String, Vec<ResolvedExternalCandidate>>::new();

    for definition in &snapshot.definitions {
        let resolved = resolve_external_candidate(
            workspace_root,
            execution_domain_id,
            definition,
            &source_map,
            &snapshot.provider_labels,
            facts,
        );
        let ecosystem_active = resolved.source_keys.iter().all(|source_key| {
            source_map
                .get(source_key)
                .is_some_and(|source| decisions.active_ecosystems.contains(&source.ecosystem_id))
        });
        if !ecosystem_active {
            state.summaries.push(summary_for(
                &resolved,
                ExternalSubagentActivationState::Disabled,
            ));
            continue;
        }
        let summary = summary_for(&resolved, initial_activation_state(&resolved));
        if facts.ai_config.is_none() {
            if !resolved.definition.disabled {
                state.routes.insert(
                    resolved.definition.logical_id.clone(),
                    ExternalSubagentRoute::Unavailable,
                );
            }
            state.summaries.push(summary);
            continue;
        }
        if resolved.definition.disabled {
            state.summaries.push(summary);
            continue;
        }
        if matches!(
            resolved.compatibility,
            ExternalSubagentCompatibilityState::Blocked
                | ExternalSubagentCompatibilityState::Invalid
        ) {
            state.summaries.push(summary);
            continue;
        }
        if has_configuration_unavailable_diagnostic(&resolved) {
            state.summaries.push(summary);
            continue;
        }
        by_logical
            .entry(normalize_logical_id(&definition.logical_id))
            .or_default()
            .push(resolved);
    }

    if facts.ai_config.is_none() {
        for lineage in decisions.conflict_lineage_current_keys.keys() {
            if let Some((domain, scope, logical_id)) = parse_conflict_lineage(lineage) {
                if domain == execution_domain_id && scope == workspace_scope {
                    state
                        .routes
                        .insert(logical_id.to_string(), ExternalSubagentRoute::Unavailable);
                }
            }
        }
        state.summaries.sort_by(|left, right| {
            left.logical_id
                .cmp(&right.logical_id)
                .then(left.candidate_id.cmp(&right.candidate_id))
        });
        return state;
    }

    let tracked_logical_ids = decisions
        .conflict_lineage_current_keys
        .keys()
        .filter_map(|lineage| {
            let (domain, scope, logical_id) = parse_conflict_lineage(lineage)?;
            (domain == execution_domain_id && scope == workspace_scope)
                .then(|| (normalize_logical_id(logical_id), logical_id.to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    let logical_keys = by_logical
        .keys()
        .cloned()
        .chain(tracked_logical_ids.keys().cloned())
        .collect::<BTreeSet<_>>();

    for logical_key in logical_keys {
        let mut external_candidates = by_logical.remove(&logical_key).unwrap_or_default();
        external_candidates.sort_by(|left, right| {
            left.definition
                .candidate_id
                .cmp(&right.definition.candidate_id)
        });
        let local = facts.locals.get(&logical_key);
        let logical_id = external_candidates
            .first()
            .map(|candidate| candidate.definition.logical_id.clone())
            .or_else(|| local.map(|candidate| candidate.logical_id.clone()))
            .or_else(|| tracked_logical_ids.get(&logical_key).cloned())
            .unwrap_or(logical_key);
        let participant_count = external_candidates.len() + usize::from(local.is_some());
        let lineage = conflict_lineage(execution_domain_id, &workspace_scope, &logical_id);
        let previously_conflicted = decisions
            .conflict_lineage_current_keys
            .contains_key(&lineage);
        if participant_count == 0 && previously_conflicted {
            let conflict_key = external_subagent_conflict_key(
                execution_domain_id,
                &workspace_scope,
                &logical_id,
                std::iter::empty::<(&str, &str)>(),
            );
            state
                .observed_conflict_lineage_current_keys
                .insert(lineage, conflict_key);
            state
                .routes
                .insert(logical_id, ExternalSubagentRoute::Unavailable);
        } else if participant_count > 1 || previously_conflicted {
            reconcile_conflict_group(
                execution_domain_id,
                &workspace_scope,
                &logical_id,
                local,
                external_candidates,
                &decisions,
                &mut state,
            );
        } else if let Some(candidate) = external_candidates.pop() {
            reconcile_nonconflicting_candidate(candidate, &decisions, &mut state);
        }
    }

    state.summaries.sort_by(|left, right| {
        left.logical_id
            .cmp(&right.logical_id)
            .then(left.candidate_id.cmp(&right.candidate_id))
    });
    state.conflicts.sort_by(|left, right| {
        left.logical_id
            .cmp(&right.logical_id)
            .then(left.conflict_key.cmp(&right.conflict_key))
    });
    state.pending_approvals.sort();
    state.pending_approvals.dedup();
    state
}

fn resolve_external_candidate(
    workspace_root: Option<&Path>,
    execution_domain_id: &str,
    definition: &ExternalSubagentDefinition,
    sources: &BTreeMap<SourceKey, &bitfun_product_domains::external_sources::ExternalSourceRecord>,
    provider_labels: &BTreeMap<ProviderId, String>,
    facts: &ProductFacts,
) -> ResolvedExternalCandidate {
    let mut compatibility = definition.compatibility;
    let mut diagnostics = definition
        .diagnostic_codes
        .iter()
        .map(|code| ExternalSubagentDiagnosticSummary {
            code: code.clone(),
            blocks_activation: matches!(
                compatibility,
                ExternalSubagentCompatibilityState::Blocked
                    | ExternalSubagentCompatibilityState::Invalid
            ),
        })
        .collect::<Vec<_>>();
    let model = match facts.ai_config.as_ref() {
        Some(ai_config) => match &definition.requested_model {
            ExternalSubagentModelRequest::Default => {
                resolve_bitfun_subagent_model(&definition.logical_id, ai_config)
            }
            ExternalSubagentModelRequest::Exact {
                provider_hint,
                model_name,
            } => resolve_exact_external_model(provider_hint.as_deref(), model_name, ai_config),
        },
        None => {
            diagnostics.push(ExternalSubagentDiagnosticSummary {
                code: "external_subagent.configuration_unavailable".to_string(),
                blocks_activation: true,
            });
            None
        }
    };
    let model = match model {
        Some(model) => model,
        None => {
            if facts.ai_config.is_some() {
                compatibility = ExternalSubagentCompatibilityState::Blocked;
                diagnostics.push(ExternalSubagentDiagnosticSummary {
                    code: "external_subagent.model_unavailable".to_string(),
                    blocks_activation: true,
                });
            }
            ResolvedModelFact {
                runtime_id: "unavailable".to_string(),
                display_label: "unavailable".to_string(),
                configuration_fingerprint: "unavailable".to_string(),
            }
        }
    };

    let mut tools = Vec::new();
    for selector in definition
        .requested_tools
        .selectors
        .iter()
        .filter(|selector| selector.allowed)
    {
        let name = selector
            .canonical_host_name
            .as_deref()
            .unwrap_or(&selector.source_name);
        match facts.tools.get(name) {
            Some(tool) => tools.push(tool.clone()),
            None => {
                compatibility = ExternalSubagentCompatibilityState::Blocked;
                diagnostics.push(ExternalSubagentDiagnosticSummary {
                    code: "external_subagent.tool_unavailable".to_string(),
                    blocks_activation: true,
                });
            }
        }
    }
    tools.sort_by(|left, right| left.name.cmp(&right.name));
    tools.dedup_by(|left, right| left.name == right.name);
    diagnostics.sort_by(|left, right| left.code.cmp(&right.code));
    diagnostics.dedup_by(|left, right| left.code == right.code);
    let readonly = tools.iter().all(|tool| tool.readonly);
    let provenance = definition
        .provenance
        .iter()
        .map(|item| item.contribution_id.stable_key())
        .collect::<Vec<_>>();
    let activation_envelope = stable_digest(
        [
            execution_domain_id,
            definition.candidate_id.as_str(),
            &format!("{:?}", definition.mode),
            model.runtime_id.as_str(),
            model.configuration_fingerprint.as_str(),
            if definition.hidden {
                "hidden"
            } else {
                "visible"
            },
            if readonly { "readonly" } else { "writable" },
        ]
        .into_iter()
        .chain(provenance.iter().map(String::as_str))
        .chain(
            tools
                .iter()
                .flat_map(|tool| [tool.name.as_str(), tool.binding_fingerprint.as_str()]),
        )
        .chain(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.blocks_activation)
                .map(|diagnostic| diagnostic.code.as_str()),
        ),
    );
    let approval_key = external_subagent_approval_key(
        &definition.candidate_id,
        &definition.behavior_version,
        &activation_envelope,
    );
    let conflict_behavior_version = stable_digest([
        definition.behavior_version.as_str(),
        activation_envelope.as_str(),
    ]);
    let provider_id = definition
        .provenance
        .first()
        .map(|item| &item.contribution_id.source.provider_id);
    let provider_label = provider_id
        .and_then(|provider_id| provider_labels.get(provider_id))
        .cloned()
        .unwrap_or_else(|| "External AI app".to_string());
    let mut source_location_labels = Vec::new();
    let mut source_keys = Vec::new();
    let mut scope = ExternalSourceScope::UserGlobal;
    for item in &definition.provenance {
        if let Some(source) = sources.get(&item.contribution_id.source) {
            if !source_keys.contains(&source.key) {
                source_keys.push(source.key.clone());
            }
            let location_label =
                safe_external_source_location(source.scope, &source.location, workspace_root);
            if !source_location_labels.contains(&location_label) {
                source_location_labels.push(location_label);
            }
            if scope_rank(source.scope) >= scope_rank(scope) {
                scope = source.scope;
            }
        }
    }
    ResolvedExternalCandidate {
        definition: definition.clone(),
        provider_label,
        scope,
        source_keys,
        source_location_labels,
        model_id: model.runtime_id,
        model_label: model.display_label,
        model_configuration_fingerprint: model.configuration_fingerprint,
        tools,
        readonly,
        activation_envelope,
        approval_key,
        conflict_behavior_version,
        diagnostics,
        compatibility,
    }
}

fn reconcile_nonconflicting_candidate(
    candidate: ResolvedExternalCandidate,
    decisions: &ExternalSubagentDecisions<'_>,
    state: &mut ExternalSubagentProductState,
) {
    let activation = if decisions
        .approved_envelopes
        .contains(&candidate.approval_key)
    {
        install_active_candidate(&candidate, state);
        ExternalSubagentActivationState::Active
    } else if decisions
        .declined_decisions
        .contains_key(&candidate.approval_key)
    {
        ExternalSubagentActivationState::Declined
    } else {
        state
            .pending_approvals
            .push(candidate.definition.candidate_id.as_str().to_string());
        ExternalSubagentActivationState::ApprovalRequired
    };
    state.summaries.push(summary_for(&candidate, activation));
}

fn reconcile_conflict_group(
    execution_domain_id: &str,
    workspace_scope: &str,
    logical_id: &str,
    local: Option<&LocalCandidateFact>,
    external_candidates: Vec<ResolvedExternalCandidate>,
    decisions: &ExternalSubagentDecisions<'_>,
    state: &mut ExternalSubagentProductState,
) {
    let mut key_participants = external_candidates
        .iter()
        .map(|candidate| {
            (
                candidate.definition.candidate_id.as_str(),
                candidate.conflict_behavior_version.as_str(),
            )
        })
        .collect::<Vec<_>>();
    if let Some(local) = local {
        key_participants.push((&local.candidate_id, &local.behavior_version));
    }
    let conflict_key = external_subagent_conflict_key(
        execution_domain_id,
        workspace_scope,
        logical_id,
        key_participants,
    );
    let lineage = conflict_lineage(execution_domain_id, workspace_scope, logical_id);
    state
        .observed_conflict_lineage_current_keys
        .insert(lineage, conflict_key.clone());
    let selected = decisions
        .conflict_choices
        .get(&conflict_key)
        .filter(|selected| {
            *selected == DISABLED_SUBAGENT_CONFLICT_CHOICE
                || local.is_some_and(|local| &local.candidate_id == *selected)
                || external_candidates.iter().any(|candidate| {
                    candidate.definition.candidate_id.as_str() == selected.as_str()
                })
        })
        .cloned();

    let mut conflict_candidates = Vec::new();
    if let Some(local) = local {
        conflict_candidates.push(ExternalSubagentConflictCandidate {
            candidate_id: local.candidate_id.clone(),
            display_name: local.display_name.clone(),
            source_label: local.source_label.clone(),
            external: false,
        });
    }
    for candidate in &external_candidates {
        conflict_candidates.push(ExternalSubagentConflictCandidate {
            candidate_id: candidate.definition.candidate_id.as_str().to_string(),
            display_name: candidate.definition.display_name.clone(),
            source_label: candidate.provider_label.clone(),
            external: true,
        });
    }
    match selected.as_deref() {
        Some(selected_id) if selected_id == DISABLED_SUBAGENT_CONFLICT_CHOICE => {
            state
                .routes
                .insert(logical_id.to_string(), ExternalSubagentRoute::Unavailable);
        }
        Some(selected_id)
            if local.is_some_and(|candidate| candidate.candidate_id == selected_id) =>
        {
            state
                .routes
                .insert(logical_id.to_string(), ExternalSubagentRoute::Local);
        }
        Some(selected_id) => {
            if let Some(candidate) = external_candidates
                .iter()
                .find(|candidate| candidate.definition.candidate_id.as_str() == selected_id)
            {
                if decisions
                    .approved_envelopes
                    .contains(&candidate.approval_key)
                {
                    install_active_candidate(candidate, state);
                } else {
                    state
                        .routes
                        .insert(logical_id.to_string(), ExternalSubagentRoute::Unavailable);
                }
            }
        }
        None => {
            state
                .routes
                .insert(logical_id.to_string(), ExternalSubagentRoute::Unavailable);
        }
    }

    for candidate in external_candidates {
        let activation = if selected.as_deref() == Some(candidate.definition.candidate_id.as_str())
            && decisions
                .approved_envelopes
                .contains(&candidate.approval_key)
        {
            ExternalSubagentActivationState::Active
        } else if selected.is_some() {
            ExternalSubagentActivationState::Disabled
        } else {
            ExternalSubagentActivationState::Conflict
        };
        state.summaries.push(summary_for(&candidate, activation));
    }
    state.conflicts.push(ExternalSubagentConflict {
        conflict_key,
        logical_id: logical_id.to_string(),
        candidates: conflict_candidates,
        selected_candidate_id: selected,
    });
}

fn install_active_candidate(
    candidate: &ResolvedExternalCandidate,
    state: &mut ExternalSubagentProductState,
) {
    let runtime_key = external_subagent_runtime_key(&stable_digest([
        candidate.definition.candidate_id.as_str(),
        candidate.definition.behavior_version.as_str(),
        candidate.activation_envelope.as_str(),
    ]));
    let tools = candidate
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect::<Vec<_>>();
    // Source-owned catalog copy is intentionally excluded from AgentInfo. The
    // existing <available_agents> projection is model-visible, so allowing a
    // description-only update there would mutate prompt context without a new
    // behavior approval. Keep that projection host-owned and stable while the
    // review surface continues to show the source description.
    let runtime_description = format!(
        "Approved external subagent from {}. Runs as a fresh single-run task.",
        candidate.provider_label
    );
    let agent = Arc::new(ExternalProvidedSubagent::new(
        runtime_key.clone(),
        candidate.definition.display_name.clone(),
        runtime_description,
        candidate.definition.prompt.expose().to_string(),
        tools,
        candidate.readonly,
        candidate.definition.behavior_version.as_str().to_string(),
    ));
    state.registrations.push(ExternalSubagentRegistration {
        runtime_key: runtime_key.clone(),
        logical_id: candidate.definition.logical_id.clone(),
        provider_label: candidate.provider_label.clone(),
        model_binding: ExternalSubagentModelBinding {
            model_id: candidate.model_id.clone(),
            configuration_fingerprint: candidate.model_configuration_fingerprint.clone(),
        },
        hidden: candidate.definition.hidden,
        agent,
    });
    state.routes.insert(
        candidate.definition.logical_id.clone(),
        ExternalSubagentRoute::External(runtime_key),
    );
}

fn summary_for(
    candidate: &ResolvedExternalCandidate,
    activation_state: ExternalSubagentActivationState,
) -> ExternalSubagentSummary {
    ExternalSubagentSummary {
        candidate_id: candidate.definition.candidate_id.as_str().to_string(),
        logical_id: candidate.definition.logical_id.clone(),
        display_name: candidate.definition.display_name.clone(),
        description: candidate.definition.description.clone(),
        provider_label: candidate.provider_label.clone(),
        scope: candidate.scope,
        source_keys: candidate.source_keys.clone(),
        source_location_labels: candidate.source_location_labels.clone(),
        source_count: candidate.definition.provenance.len(),
        effective_model_label: (candidate.model_id != "unavailable")
            .then(|| candidate.model_label.clone()),
        effective_tool_labels: candidate
            .tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect(),
        supports_follow_up: false,
        compatibility_state: candidate.compatibility,
        diagnostics: candidate.diagnostics.clone(),
        activation_state,
        decision_key: candidate.approval_key.clone(),
    }
}

fn initial_activation_state(
    candidate: &ResolvedExternalCandidate,
) -> ExternalSubagentActivationState {
    if candidate.definition.disabled {
        ExternalSubagentActivationState::Disabled
    } else if matches!(
        candidate.compatibility,
        ExternalSubagentCompatibilityState::Blocked | ExternalSubagentCompatibilityState::Invalid
    ) {
        ExternalSubagentActivationState::Blocked
    } else if has_configuration_unavailable_diagnostic(candidate) {
        ExternalSubagentActivationState::Unavailable
    } else {
        ExternalSubagentActivationState::ApprovalRequired
    }
}

fn has_configuration_unavailable_diagnostic(candidate: &ResolvedExternalCandidate) -> bool {
    candidate.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "external_subagent.configuration_unavailable"
            && diagnostic.blocks_activation
    })
}

fn workspace_scope_key(workspace_root: Option<&Path>) -> String {
    workspace_root
        .map(|path| {
            path.to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase()
        })
        .unwrap_or_else(|| "<global>".to_string())
}

fn normalize_logical_id(value: &str) -> String {
    value.to_ascii_lowercase()
}

fn conflict_lineage(execution_domain_id: &str, workspace_scope: &str, logical_id: &str) -> String {
    [
        execution_domain_id,
        workspace_scope,
        &normalize_logical_id(logical_id),
    ]
    .into_iter()
    .map(|part| format!("{}:{part}", part.len()))
    .collect()
}

fn parse_conflict_lineage(value: &str) -> Option<(&str, &str, &str)> {
    let (domain, rest) = take_length_prefixed(value)?;
    let (scope, rest) = take_length_prefixed(rest)?;
    let (logical_id, rest) = take_length_prefixed(rest)?;
    rest.is_empty().then_some((domain, scope, logical_id))
}

fn take_length_prefixed(value: &str) -> Option<(&str, &str)> {
    let colon = value.find(':')?;
    let length = value[..colon].parse::<usize>().ok()?;
    let rest = &value[colon + 1..];
    let part = rest.get(..length)?;
    Some((part, rest.get(length..)?))
}

fn scope_rank(scope: ExternalSourceScope) -> u8 {
    match scope {
        ExternalSourceScope::UserGlobal => 0,
        ExternalSourceScope::Project => 1,
        ExternalSourceScope::WorkspaceLocal => 2,
        ExternalSourceScope::RemoteUser => 3,
        ExternalSourceScope::RemoteProject => 4,
        _ => 0,
    }
}

fn stable_digest(parts: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        let value = part.as_ref();
        hasher.update(value.len().to_le_bytes());
        hasher.update(value.as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_active_ecosystems() -> &'static BTreeSet<EcosystemId> {
        static ECOSYSTEMS: std::sync::OnceLock<BTreeSet<EcosystemId>> = std::sync::OnceLock::new();
        ECOSYSTEMS.get_or_init(|| {
            BTreeSet::from([EcosystemId::new("fake").expect("valid test ecosystem")])
        })
    }
    use bitfun_product_domains::external_sources::{
        EcosystemId, ExecutionDomainId, ExternalSourceCatalogEntry, ExternalSourceHealth,
        ExternalSourceLifecycleState, ExternalSourceRecord,
    };
    use bitfun_product_domains::external_subagents::{
        ExternalSubagentBehaviorVersion, ExternalSubagentCandidateId,
        ExternalSubagentContributionId, ExternalSubagentContributionRole, ExternalSubagentLocalId,
        ExternalSubagentMode, ExternalSubagentProvenanceRef, ExternalSubagentToolRequest,
        ExternalSubagentToolSelector, SecretText,
    };

    fn definition(behavior: &str, catalog: &str) -> (ExternalSubagentDefinition, SourceKey) {
        let source = SourceKey::new("fake-provider", "agents/reviewer.md").unwrap();
        let provenance = vec![ExternalSubagentProvenanceRef {
            contribution_id: ExternalSubagentContributionId::new(
                source.clone(),
                ExternalSubagentLocalId::new("reviewer").unwrap(),
            ),
            role: ExternalSubagentContributionRole::Definition,
        }];
        (
            ExternalSubagentDefinition {
                candidate_id: ExternalSubagentCandidateId::new(
                    "external_subagent:fake:reviewer:candidate",
                )
                .unwrap(),
                logical_id: "reviewer".to_string(),
                provenance,
                display_name: "Reviewer".to_string(),
                description: catalog.to_string(),
                prompt: SecretText::new("Review the requested changes."),
                mode: ExternalSubagentMode::Subagent,
                disabled: false,
                hidden: false,
                requested_model: ExternalSubagentModelRequest::Default,
                requested_tools: ExternalSubagentToolRequest {
                    selectors: vec![ExternalSubagentToolSelector {
                        source_name: "read".to_string(),
                        canonical_host_name: Some("Read".to_string()),
                        allowed: true,
                    }],
                    uses_conservative_default: false,
                },
                compatibility: ExternalSubagentCompatibilityState::Ready,
                diagnostic_codes: Vec::new(),
                behavior_version: ExternalSubagentBehaviorVersion::new(behavior).unwrap(),
            },
            source,
        )
    }

    fn snapshot(behavior: &str, catalog: &str) -> ExternalSubagentCoordinatorSnapshot {
        let (definition, source) = definition(behavior, catalog);
        ExternalSubagentCoordinatorSnapshot {
            generation: 1,
            discovery_pending: false,
            provider_labels: BTreeMap::from([(
                ProviderId::new("fake-provider").unwrap(),
                "Fake AI".to_string(),
            )]),
            sources: vec![ExternalSourceCatalogEntry {
                stable_key: "source".to_string(),
                presentation_group_id: None,
                record: ExternalSourceRecord {
                    key: source,
                    ecosystem_id: EcosystemId::new("fake").unwrap(),
                    display_name: "Fake agents".to_string(),
                    source_kind: "markdown".to_string(),
                    scope: ExternalSourceScope::UserGlobal,
                    location: "agents/reviewer.md".to_string(),
                    execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
                    health: ExternalSourceHealth::Available,
                    content_version: "v1".to_string(),
                    diagnostics: Vec::new(),
                },
                lifecycle: ExternalSourceLifecycleState::Available,
            }],
            definitions: vec![definition],
            using_last_valid_provider_ids: Vec::new(),
            diagnostics: Vec::new(),
            next_refresh_deadline: None,
        }
    }

    fn facts() -> ProductFacts {
        let mut ai_config = AIConfig {
            models: vec![active_model(
                "model_fast",
                "Fast provider",
                "fake",
                "fast-model",
            )],
            ..AIConfig::default()
        };
        ai_config.default_models.fast = Some("model_fast".to_string());
        ProductFacts {
            ai_config: Some(ai_config),
            tools: BTreeMap::from([(
                "Read".to_string(),
                ResolvedToolFact {
                    name: "Read".to_string(),
                    binding_fingerprint: "read-v1".to_string(),
                    readonly: true,
                },
            )]),
            locals: BTreeMap::new(),
        }
    }

    fn active_model(
        id: &str,
        provider_name: &str,
        provider: &str,
        model_name: &str,
    ) -> AIModelConfig {
        AIModelConfig {
            id: id.to_string(),
            name: provider_name.to_string(),
            provider: provider.to_string(),
            model_name: model_name.to_string(),
            enabled: true,
            ..AIModelConfig::default()
        }
    }

    #[test]
    fn exact_external_model_maps_provider_and_model_to_internal_config_without_exposing_id() {
        let config = AIConfig {
            models: vec![active_model(
                "model_internal_1",
                "OpenRouter",
                "openai",
                "anthropic/claude-sonnet-4",
            )],
            ..AIConfig::default()
        };

        let resolved =
            resolve_exact_external_model(Some("openrouter"), "anthropic/claude-sonnet-4", &config)
                .expect("provider/model identity should resolve");

        assert_eq!(resolved.runtime_id, "model_internal_1");
        assert_eq!(
            resolved.display_label,
            "OpenRouter · anthropic/claude-sonnet-4"
        );
        assert!(!resolved.display_label.contains("model_internal_1"));
    }

    #[test]
    fn exact_external_model_fails_closed_when_provider_model_identity_is_ambiguous() {
        let config = AIConfig {
            models: vec![
                active_model("model_a", "Anthropic", "anthropic", "claude-sonnet-4"),
                active_model("model_b", "Anthropic", "anthropic", "claude-sonnet-4"),
            ],
            ..AIConfig::default()
        };

        assert!(resolve_exact_external_model(None, "anthropic/claude-sonnet-4", &config).is_none());
    }

    #[test]
    fn default_external_model_materializes_an_enabled_authoritative_selection() {
        let mut config = AIConfig {
            models: vec![active_model(
                "model_review",
                "Anthropic",
                "anthropic",
                "claude-sonnet-4",
            )],
            ..AIConfig::default()
        };
        config.agent_model_defaults.subagents.default_selection =
            SubagentModelSelection::fixed("model_review");

        let resolved = resolve_bitfun_subagent_model("reviewer", &config)
            .expect("configured subagent default should resolve");
        assert_eq!(resolved.runtime_id, "model_review");
        assert_eq!(resolved.display_label, "Anthropic · claude-sonnet-4");

        config.agent_model_defaults.subagents.default_selection = SubagentModelSelection::Inherit;
        assert!(resolve_bitfun_subagent_model("reviewer", &config).is_none());

        config.agent_model_defaults.subagents.default_selection =
            SubagentModelSelection::fixed("fast");
        assert!(resolve_bitfun_subagent_model("reviewer", &config).is_none());

        config.agent_model_defaults.subagents.default_selection =
            SubagentModelSelection::fixed("model_review");
        config.models[0].enabled = false;
        assert!(resolve_bitfun_subagent_model("reviewer", &config).is_none());
    }

    #[test]
    fn source_location_labels_hide_absolute_user_and_workspace_paths() {
        assert_eq!(
            safe_external_source_location(
                ExternalSourceScope::Project,
                "C:/repo/.opencode/agents/review.md",
                Some(Path::new("C:/repo")),
            ),
            "<workspace>/.opencode/agents/review.md"
        );
        assert_eq!(
            safe_external_source_location(
                ExternalSourceScope::UserGlobal,
                "C:/Users/alice/.future-ai/agents/review.md",
                None,
            ),
            "~/.future-ai/agents/review.md"
        );
        assert_eq!(
            safe_external_source_location(
                ExternalSourceScope::UserGlobal,
                "C:/Users/alice/.config/opencode/agents/review.md",
                None,
            ),
            "~/.config/opencode/agents/review.md"
        );
        assert_eq!(
            safe_external_source_location(
                ExternalSourceScope::UserGlobal,
                "/Users/alice/.config/opencode/agents/review.md",
                None,
            ),
            "~/.config/opencode/agents/review.md"
        );
        assert_eq!(
            safe_external_source_location(
                ExternalSourceScope::Project,
                "C:\\repo\\.opencode\\agents\\review.md",
                Some(Path::new("C:\\repo")),
            ),
            "<workspace>/.opencode/agents/review.md"
        );
    }

    #[test]
    fn unavailable_bitfun_model_config_is_recoverable_without_claiming_model_mismatch() {
        let empty_set = BTreeSet::new();
        let empty_map = BTreeMap::new();
        let definition_snapshot = snapshot("behavior-v1", "catalog-v1");
        let healthy_facts = facts();
        let preview = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &definition_snapshot,
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &empty_set,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &healthy_facts,
        );
        let approved = BTreeSet::from([preview.summaries[0].decision_key.clone()]);
        let mut unavailable_facts = facts();
        unavailable_facts.ai_config = None;

        let state = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &definition_snapshot,
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &approved,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &unavailable_facts,
        );

        assert_eq!(
            state.summaries[0].activation_state,
            ExternalSubagentActivationState::Unavailable
        );
        assert_eq!(
            state.summaries[0].compatibility_state,
            ExternalSubagentCompatibilityState::Ready
        );
        assert!(state.summaries[0].diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "external_subagent.configuration_unavailable"
                && diagnostic.blocks_activation
        }));
        assert!(!state.summaries[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "external_subagent.model_unavailable"));
        assert!(state.pending_approvals.is_empty());
        assert!(state.registrations.is_empty());

        let recovered = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &definition_snapshot,
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &approved,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &healthy_facts,
        );
        assert_eq!(
            recovered.summaries[0].activation_state,
            ExternalSubagentActivationState::Active
        );
        assert_eq!(recovered.registrations.len(), 1);
    }

    #[test]
    fn model_config_outage_logging_is_deduplicated_and_does_not_expose_error_values() {
        let logged = AtomicBool::new(false);
        assert!(claim_model_config_outage_log(&logged));
        assert!(!claim_model_config_outage_log(&logged));
        logged.store(false, Ordering::Relaxed);
        assert!(claim_model_config_outage_log(&logged));

        let error = BitFunError::config(
            "Failed to deserialize config value at 'ai': invalid value 'sk-sensitive'",
        );
        let category = model_config_error_category(&error);
        assert_eq!(category, "config_deserialization_failed");
        assert!(!category.contains("sk-sensitive"));
    }

    #[test]
    fn catalog_only_update_reuses_behavior_approval() {
        let first = snapshot("behavior-v1", "catalog-v1");
        let empty_set = BTreeSet::new();
        let empty_map = BTreeMap::new();
        let preview = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &first,
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &empty_set,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &facts(),
        );
        let approval = preview.summaries[0].decision_key.clone();
        let approved = BTreeSet::from([approval]);
        let updated = snapshot("behavior-v1", "catalog-v2");
        let state = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &updated,
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &approved,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &facts(),
        );
        assert_eq!(
            state.summaries[0].activation_state,
            ExternalSubagentActivationState::Active
        );
        assert_eq!(state.registrations.len(), 1);
        assert!(!state.registrations[0]
            .agent
            .description()
            .contains("catalog-v2"));
    }

    #[test]
    fn behavior_update_requires_a_new_approval() {
        let empty_set = BTreeSet::new();
        let empty_map = BTreeMap::new();
        let first = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &snapshot("behavior-v1", "catalog-v1"),
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &empty_set,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &facts(),
        );
        let approved = BTreeSet::from([first.summaries[0].decision_key.clone()]);
        let updated = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &snapshot("behavior-v2", "catalog-v2"),
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &approved,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &facts(),
        );
        assert_eq!(
            updated.summaries[0].activation_state,
            ExternalSubagentActivationState::ApprovalRequired
        );
        assert!(updated.registrations.is_empty());
    }

    #[test]
    fn default_model_change_requires_a_new_approval_for_future_invocations() {
        let empty_set = BTreeSet::new();
        let empty_map = BTreeMap::new();
        let first_facts = facts();
        let first = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &snapshot("behavior-v1", "catalog-v1"),
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &empty_set,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &first_facts,
        );
        let approved = BTreeSet::from([first.summaries[0].decision_key.clone()]);

        let mut updated_facts = facts();
        let updated_config = updated_facts.ai_config.as_mut().unwrap();
        updated_config.models = vec![active_model(
            "model_new",
            "New provider",
            "fake",
            "new-model",
        )];
        updated_config.default_models.fast = Some("model_new".to_string());
        let updated = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &snapshot("behavior-v1", "catalog-v1"),
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &approved,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &updated_facts,
        );

        assert_ne!(
            first.summaries[0].decision_key,
            updated.summaries[0].decision_key
        );
        assert_eq!(
            updated.summaries[0].activation_state,
            ExternalSubagentActivationState::ApprovalRequired
        );
        assert!(updated.registrations.is_empty());
    }

    #[test]
    fn same_model_id_runtime_identity_change_requires_a_new_approval() {
        let empty_set = BTreeSet::new();
        let empty_map = BTreeMap::new();
        let first_facts = facts();
        let first = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &snapshot("behavior-v1", "catalog-v1"),
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &empty_set,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &first_facts,
        );
        let approved = BTreeSet::from([first.summaries[0].decision_key.clone()]);

        let mut updated_facts = facts();
        let updated_config = updated_facts.ai_config.as_mut().unwrap();
        updated_config.models[0].provider = "other-provider".to_string();
        updated_config.models[0].model_name = "replacement-model".to_string();
        updated_config.models[0].base_url = "https://models.example/v2".to_string();
        let updated = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &snapshot("behavior-v1", "catalog-v1"),
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &approved,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &updated_facts,
        );

        assert_ne!(
            first.summaries[0].decision_key,
            updated.summaries[0].decision_key
        );
        assert_eq!(
            updated.summaries[0].activation_state,
            ExternalSubagentActivationState::ApprovalRequired
        );
        assert!(updated.registrations.is_empty());
    }

    #[test]
    fn unresolved_default_model_is_blocked_without_exposing_an_internal_placeholder_label() {
        let empty_set = BTreeSet::new();
        let empty_map = BTreeMap::new();
        let mut unavailable_facts = facts();
        unavailable_facts
            .ai_config
            .as_mut()
            .unwrap()
            .agent_model_defaults
            .subagents
            .default_selection = SubagentModelSelection::Inherit;

        let state = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &snapshot("behavior-v1", "catalog-v1"),
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &empty_set,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &unavailable_facts,
        );

        assert_eq!(
            state.summaries[0].activation_state,
            ExternalSubagentActivationState::Blocked
        );
        assert_eq!(state.summaries[0].effective_model_label, None);
        assert!(state.registrations.is_empty());
    }

    #[test]
    fn unresolved_conflict_is_fail_closed_and_atomic_choice_can_activate() {
        let mut facts = facts();
        facts.locals.insert(
            "reviewer".to_string(),
            LocalCandidateFact {
                logical_id: "reviewer".to_string(),
                candidate_id: "local_subagent:reviewer".to_string(),
                display_name: "Local reviewer".to_string(),
                source_label: "BitFun project".to_string(),
                behavior_version: "local-v1".to_string(),
            },
        );
        let empty_set = BTreeSet::new();
        let empty_map = BTreeMap::new();
        let preview = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &snapshot("behavior-v1", "catalog-v1"),
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &empty_set,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &facts,
        );
        assert_eq!(
            preview.routes.get("reviewer"),
            Some(&ExternalSubagentRoute::Unavailable)
        );
        assert_eq!(preview.conflicts[0].candidates.len(), 2);
        assert!(
            !preview.conflicts[0].candidates[0].external,
            "the BitFun/local candidate must be shown before external candidates"
        );
        let external_id = preview.conflicts[0]
            .candidates
            .iter()
            .find(|candidate| candidate.external)
            .unwrap()
            .candidate_id
            .clone();
        let approval = preview.summaries[0].decision_key.clone();
        let choices = BTreeMap::from([(preview.conflicts[0].conflict_key.clone(), external_id)]);
        let approved = BTreeSet::from([approval]);
        let active = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &snapshot("behavior-v1", "catalog-v1"),
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &approved,
                declined_decisions: &empty_map,
                conflict_choices: &choices,
                conflict_lineage_current_keys: &preview.observed_conflict_lineage_current_keys,
            },
            &facts,
        );
        assert!(matches!(
            active.routes.get("reviewer"),
            Some(ExternalSubagentRoute::External(_))
        ));
    }

    #[test]
    fn model_config_outage_preserves_conflict_choice_for_recovery() {
        for select_external in [false, true] {
            let (preview, mut product_facts) = conflict_preview_with_local();
            let conflict = &preview.conflicts[0];
            let external_id = conflict
                .candidates
                .iter()
                .find(|candidate| candidate.external)
                .unwrap()
                .candidate_id
                .clone();
            let selected_id = if select_external {
                external_id
            } else {
                "local_subagent:reviewer".to_string()
            };
            let choices = BTreeMap::from([(conflict.conflict_key.clone(), selected_id.clone())]);
            let approved = BTreeSet::from([preview.summaries[0].decision_key.clone()]);
            let empty_map = BTreeMap::new();
            let healthy_ai_config = product_facts.ai_config.take();

            let unavailable = reconcile_with_facts(
                Some(Path::new("C:/repo")),
                "local-user",
                &snapshot("behavior-v1", "catalog-v1"),
                ExternalSubagentDecisions {
                    active_ecosystems: test_active_ecosystems(),
                    approved_envelopes: &approved,
                    declined_decisions: &empty_map,
                    conflict_choices: &choices,
                    conflict_lineage_current_keys: &preview.observed_conflict_lineage_current_keys,
                },
                &product_facts,
            );

            assert_eq!(
                unavailable.routes.get("reviewer"),
                Some(&ExternalSubagentRoute::Unavailable)
            );
            assert!(unavailable
                .summaries
                .iter()
                .all(|summary| summary.activation_state
                    == ExternalSubagentActivationState::Unavailable));
            assert!(unavailable.conflicts.is_empty());
            assert!(unavailable
                .observed_conflict_lineage_current_keys
                .is_empty());
            assert!(unavailable.pending_approvals.is_empty());
            assert!(unavailable.registrations.is_empty());

            product_facts.ai_config = healthy_ai_config;
            let recovered = reconcile_with_facts(
                Some(Path::new("C:/repo")),
                "local-user",
                &snapshot("behavior-v1", "catalog-v1"),
                ExternalSubagentDecisions {
                    active_ecosystems: test_active_ecosystems(),
                    approved_envelopes: &approved,
                    declined_decisions: &empty_map,
                    conflict_choices: &choices,
                    conflict_lineage_current_keys: &preview.observed_conflict_lineage_current_keys,
                },
                &product_facts,
            );

            assert_eq!(
                recovered.conflicts[0].selected_candidate_id.as_deref(),
                Some(selected_id.as_str())
            );
            assert_eq!(
                recovered.observed_conflict_lineage_current_keys,
                preview.observed_conflict_lineage_current_keys
            );
            if select_external {
                assert!(matches!(
                    recovered.routes.get("reviewer"),
                    Some(ExternalSubagentRoute::External(_))
                ));
                assert_eq!(recovered.registrations.len(), 1);
            } else {
                assert_eq!(
                    recovered.routes.get("reviewer"),
                    Some(&ExternalSubagentRoute::Local)
                );
                assert!(recovered.registrations.is_empty());
            }
        }
    }

    fn conflict_preview_with_local() -> (ExternalSubagentProductState, ProductFacts) {
        let mut product_facts = facts();
        product_facts.locals.insert(
            "reviewer".to_string(),
            LocalCandidateFact {
                logical_id: "reviewer".to_string(),
                candidate_id: "local_subagent:reviewer".to_string(),
                display_name: "Local reviewer".to_string(),
                source_label: "BitFun project".to_string(),
                behavior_version: "local-v1".to_string(),
            },
        );
        let empty_set = BTreeSet::new();
        let empty_map = BTreeMap::new();
        let preview = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &snapshot("behavior-v1", "catalog-v1"),
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &empty_set,
                declined_decisions: &empty_map,
                conflict_choices: &empty_map,
                conflict_lineage_current_keys: &empty_map,
            },
            &product_facts,
        );
        (preview, product_facts)
    }

    #[test]
    fn selected_local_does_not_switch_to_external_when_local_participant_disappears() {
        let (preview, _facts_with_local) = conflict_preview_with_local();
        let conflict = &preview.conflicts[0];
        let choices = BTreeMap::from([(
            conflict.conflict_key.clone(),
            "local_subagent:reviewer".to_string(),
        )]);
        let empty_set = BTreeSet::new();
        let empty_map = BTreeMap::new();

        let shrunk = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &snapshot("behavior-v1", "catalog-v1"),
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &empty_set,
                declined_decisions: &empty_map,
                conflict_choices: &choices,
                conflict_lineage_current_keys: &preview.observed_conflict_lineage_current_keys,
            },
            &facts(),
        );

        assert_eq!(
            shrunk.routes.get("reviewer"),
            Some(&ExternalSubagentRoute::Unavailable)
        );
        assert_eq!(shrunk.conflicts[0].candidates.len(), 1);
        assert!(shrunk.conflicts[0].selected_candidate_id.is_none());
    }

    #[test]
    fn selected_external_does_not_switch_to_local_when_external_participant_disappears() {
        let (preview, facts_with_local) = conflict_preview_with_local();
        let conflict = &preview.conflicts[0];
        let external_id = conflict
            .candidates
            .iter()
            .find(|candidate| candidate.external)
            .unwrap()
            .candidate_id
            .clone();
        let choices = BTreeMap::from([(conflict.conflict_key.clone(), external_id)]);
        let empty_set = BTreeSet::new();
        let empty_map = BTreeMap::new();
        let mut without_external = snapshot("behavior-v1", "catalog-v1");
        without_external.definitions.clear();

        let shrunk = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &without_external,
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &empty_set,
                declined_decisions: &empty_map,
                conflict_choices: &choices,
                conflict_lineage_current_keys: &preview.observed_conflict_lineage_current_keys,
            },
            &facts_with_local,
        );

        assert_eq!(
            shrunk.routes.get("reviewer"),
            Some(&ExternalSubagentRoute::Unavailable)
        );
        assert_eq!(shrunk.conflicts[0].candidates.len(), 1);
        assert!(shrunk.conflicts[0].selected_candidate_id.is_none());
    }

    #[test]
    fn keep_unavailable_survives_participant_set_shrink() {
        let (preview, facts_with_local) = conflict_preview_with_local();
        let choices = BTreeMap::from([(
            preview.conflicts[0].conflict_key.clone(),
            DISABLED_SUBAGENT_CONFLICT_CHOICE.to_string(),
        )]);
        let empty_set = BTreeSet::new();
        let empty_map = BTreeMap::new();
        let mut without_external = snapshot("behavior-v1", "catalog-v1");
        without_external.definitions.clear();

        let shrunk = reconcile_with_facts(
            Some(Path::new("C:/repo")),
            "local-user",
            &without_external,
            ExternalSubagentDecisions {
                active_ecosystems: test_active_ecosystems(),
                approved_envelopes: &empty_set,
                declined_decisions: &empty_map,
                conflict_choices: &choices,
                conflict_lineage_current_keys: &preview.observed_conflict_lineage_current_keys,
            },
            &facts_with_local,
        );

        assert_eq!(
            shrunk.routes.get("reviewer"),
            Some(&ExternalSubagentRoute::Unavailable)
        );
        assert!(shrunk.conflicts[0].selected_candidate_id.is_none());
    }
}
