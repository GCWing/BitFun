use crate::service::mcp::{
    get_global_mcp_service, ConfigLocation, MCPServerConfig, MCPServerStatus, MCPServerTransport,
    MCPServerType,
};
use async_trait::async_trait;
use bitfun_external_sources::ExternalMcpCoordinatorSnapshot;
use bitfun_product_domains::external_sources::{
    external_mcp_approval_key, external_mcp_conflict_key, ExternalMcpActivationState,
    ExternalMcpApprovalRequest, ExternalMcpCatalogEntry, ExternalMcpConflict,
    ExternalMcpConflictCandidate, ExternalMcpServerDefinition, ExternalMcpStaticStatus,
    ExternalSourceDiagnostic, PreparedExternalMcpServer, PreparedExternalMcpTransport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ExternalMcpDecision {
    pub decision_key: String,
    pub approved: bool,
}

pub(super) struct ExternalMcpDecisions<'a> {
    pub server_decisions: &'a BTreeMap<String, ExternalMcpDecision>,
    pub conflict_choices: &'a BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NativeMcpCandidate {
    pub candidate_id: String,
    pub server_id: String,
    pub name: String,
    pub display_name: String,
    pub behavior_version: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActiveExternalMcpCandidate {
    pub runtime_id: String,
    pub definition: ExternalMcpServerDefinition,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ExternalMcpProductState {
    pub entries: Vec<ExternalMcpCatalogEntry>,
    pub approval_requests: Vec<ExternalMcpApprovalRequest>,
    pub conflicts: Vec<ExternalMcpConflict>,
    pub active: Vec<ActiveExternalMcpCandidate>,
    pub suppressed_native_server_ids: std::collections::BTreeSet<String>,
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ExternalMcpRuntimeStatus {
    Active,
    Loading,
    Unavailable(String),
}

/// Narrow product-to-runtime port. Product reconciliation works only with
/// source-neutral prepared MCP data; the concrete BitFun MCP manager remains
/// behind this implementation boundary.
#[async_trait]
pub(super) trait ExternalMcpRuntimePort: Send + Sync {
    async fn install(
        &self,
        candidate: &ActiveExternalMcpCandidate,
        prepared: PreparedExternalMcpServer,
        workspace_key: &str,
    ) -> Result<(), String>;

    async fn retire(&self, runtime_id: &str) -> Result<(), String>;

    async fn status(&self, runtime_id: &str) -> Result<ExternalMcpRuntimeStatus, String>;

    async fn replace_workspace_route(
        &self,
        workspace_key: &str,
        active_external_server_ids: std::collections::BTreeSet<String>,
        suppressed_native_server_ids: std::collections::BTreeSet<String>,
    ) -> Result<(), String>;
}

pub(super) struct BitFunExternalMcpRuntime;

#[async_trait]
impl ExternalMcpRuntimePort for BitFunExternalMcpRuntime {
    async fn install(
        &self,
        candidate: &ActiveExternalMcpCandidate,
        prepared: PreparedExternalMcpServer,
        workspace_key: &str,
    ) -> Result<(), String> {
        if prepared.id != candidate.definition.id
            || prepared.behavior_version != candidate.definition.behavior_version
        {
            return Err("The external MCP configuration changed before activation".to_string());
        }
        let config = prepared_mcp_config(candidate, prepared)?;
        mcp_manager()?
            .install_external_ephemeral_server(config, workspace_key.to_string())
            .await
            // Runtime errors may contain a URL or command line. Keep the
            // product-facing error actionable without echoing sensitive data.
            .map_err(|_| "The external MCP server could not be started".to_string())
    }

    async fn retire(&self, runtime_id: &str) -> Result<(), String> {
        mcp_manager()?
            .retire_external_ephemeral_server(runtime_id)
            .await
            .map_err(|_| "The external MCP server could not be stopped cleanly".to_string())
    }

    async fn status(&self, runtime_id: &str) -> Result<ExternalMcpRuntimeStatus, String> {
        let manager = mcp_manager()?;
        if manager.external_server_readiness(runtime_id).await == Some(false) {
            return Ok(ExternalMcpRuntimeStatus::Loading);
        }
        let status = match tokio::time::timeout(
            Duration::from_millis(100),
            manager.get_server_status(runtime_id),
        )
        .await
        {
            Ok(Ok(status)) => status,
            Ok(Err(_)) => return Err("The external MCP server is no longer available".to_string()),
            Err(_) => return Ok(ExternalMcpRuntimeStatus::Loading),
        };
        Ok(match status {
            MCPServerStatus::Connected | MCPServerStatus::Healthy => {
                ExternalMcpRuntimeStatus::Active
            }
            MCPServerStatus::Uninitialized
            | MCPServerStatus::Starting
            | MCPServerStatus::Reconnecting => ExternalMcpRuntimeStatus::Loading,
            MCPServerStatus::NeedsAuth => ExternalMcpRuntimeStatus::Unavailable(
                "Authentication is required for this MCP server".to_string(),
            ),
            MCPServerStatus::Failed => ExternalMcpRuntimeStatus::Unavailable(
                "The MCP server failed to start or stopped unexpectedly".to_string(),
            ),
            MCPServerStatus::Stopping | MCPServerStatus::Stopped => {
                ExternalMcpRuntimeStatus::Unavailable(
                    "The MCP server is not currently running".to_string(),
                )
            }
        })
    }

    async fn replace_workspace_route(
        &self,
        workspace_key: &str,
        active_external_server_ids: std::collections::BTreeSet<String>,
        suppressed_native_server_ids: std::collections::BTreeSet<String>,
    ) -> Result<(), String> {
        mcp_manager()?
            .replace_external_workspace_tool_route(
                workspace_key.to_string(),
                active_external_server_ids,
                suppressed_native_server_ids,
            )
            .await;
        Ok(())
    }
}

fn mcp_manager() -> Result<std::sync::Arc<crate::service::mcp::MCPServerManager>, String> {
    get_global_mcp_service()
        .map(|service| service.server_manager())
        .ok_or_else(|| "The BitFun MCP runtime is not available in this product host".to_string())
}

pub(super) fn prepared_mcp_config(
    candidate: &ActiveExternalMcpCandidate,
    prepared: PreparedExternalMcpServer,
) -> Result<MCPServerConfig, String> {
    let (
        server_type,
        transport,
        command,
        args,
        env,
        working_directory,
        headers,
        url,
        oauth_enabled,
    ) = match prepared.transport {
        PreparedExternalMcpTransport::Local {
            command,
            args,
            environment,
            working_directory,
        } => (
            MCPServerType::Local,
            MCPServerTransport::Stdio,
            Some(command),
            args,
            environment
                .into_iter()
                .map(|(key, value)| (key, value.expose().to_string()))
                .collect(),
            working_directory.map(|path| path.to_string_lossy().to_string()),
            Default::default(),
            None,
            None,
        ),
        PreparedExternalMcpTransport::Remote {
            url,
            headers,
            oauth_enabled,
        } => (
            MCPServerType::Remote,
            MCPServerTransport::StreamableHttp,
            None,
            Vec::new(),
            Default::default(),
            None,
            headers
                .into_iter()
                .map(|(key, value)| (key, value.expose().to_string()))
                .collect(),
            Some(url),
            Some(oauth_enabled),
        ),
    };
    let config = MCPServerConfig {
        id: candidate.runtime_id.clone(),
        name: candidate.definition.name.clone(),
        server_type,
        transport: Some(transport),
        command,
        args,
        env,
        working_directory,
        inherit_parent_environment: matches!(server_type, MCPServerType::Local).then_some(false),
        headers,
        url,
        auto_start: true,
        enabled: true,
        location: ConfigLocation::BuiltIn,
        capabilities: Vec::new(),
        settings: Default::default(),
        oauth: None,
        oauth_enabled,
        xaa: None,
    };
    config.validate().map_err(|_| {
        "The external MCP configuration is not valid for the BitFun runtime".to_string()
    })?;
    Ok(config)
}

struct CandidateGroup<'a> {
    native: Vec<&'a NativeMcpCandidate>,
    external: Vec<&'a ExternalMcpServerDefinition>,
}

impl<'a> Default for CandidateGroup<'a> {
    fn default() -> Self {
        Self {
            native: Vec::new(),
            external: Vec::new(),
        }
    }
}

/// Produces the source-neutral product decision for external MCP candidates.
/// This function is pure: no provider preparation, process launch, credential
/// access, or network request can occur while a user decision is pending.
pub(super) fn reconcile_external_mcp_catalog(
    execution_domain_id: &str,
    workspace_key: &str,
    snapshot: &ExternalMcpCoordinatorSnapshot,
    native_candidates: &[NativeMcpCandidate],
    decisions: ExternalMcpDecisions<'_>,
) -> ExternalMcpProductState {
    let mut groups = BTreeMap::<String, CandidateGroup<'_>>::new();
    for native in native_candidates {
        groups
            .entry(native.name.to_ascii_lowercase())
            .or_default()
            .native
            .push(native);
    }
    for external in &snapshot.servers {
        groups
            .entry(external.name.to_ascii_lowercase())
            .or_default()
            .external
            .push(external);
    }

    let source_names = snapshot
        .sources
        .iter()
        .map(|source| {
            (
                source.record.key.clone(),
                source.record.display_name.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut state = ExternalMcpProductState::default();

    for (_, mut group) in groups {
        group
            .native
            .sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
        group
            .external
            .sort_by(|left, right| left.candidate_id().cmp(&right.candidate_id()));
        let participant_count = group.native.len() + group.external.len();
        let server_name = group
            .native
            .first()
            .map(|candidate| candidate.name.as_str())
            .or_else(|| group.external.first().map(|definition| definition.name.as_str()))
            .unwrap_or_default();
        let pending_conflict = build_conflict(
            execution_domain_id,
            workspace_key,
            &group,
            &source_names,
            decisions.conflict_choices,
        );
        let conflict_lineage = pending_conflict
            .conflict_key
            .rsplit_once(':')
            .map(|(lineage, _)| lineage)
            .unwrap_or_default();
        let prior_external_selection = decisions.conflict_choices.iter().any(|(key, selected)| {
            key.rsplit_once(':')
                .is_some_and(|(lineage, _)| lineage == conflict_lineage)
                && selected.starts_with("external_mcp:")
        });
        let conflict = if participant_count > 1
            || decisions.conflict_choices.keys().any(|key| {
                key.rsplit_once(':')
                    .is_some_and(|(lineage, _)| lineage == conflict_lineage)
            })
        {
            Some(pending_conflict)
        } else {
            None
        };
        let selected_candidate_id = conflict
            .as_ref()
            .and_then(|conflict| conflict.selected_candidate_id.as_deref());
        let selected_external = selected_candidate_id.is_some_and(|selected| {
            group
                .external
                .iter()
                .any(|definition| definition.candidate_id() == selected)
        });
        if selected_external || (selected_candidate_id.is_none() && prior_external_selection) {
            state.suppressed_native_server_ids.extend(
                group
                    .native
                    .iter()
                    .map(|candidate| candidate.server_id.clone()),
            );
        }

        for definition in group.external {
            let candidate_id = definition.candidate_id();
            let approval_key = external_mcp_approval_key(
                execution_domain_id,
                workspace_key,
                &definition.id,
                &definition.behavior_version,
            );
            let decision = current_or_previous_mcp_decision(
                decisions.server_decisions,
                &approval_key,
            );
            let static_unavailable = static_unavailable_reason(definition);
            let activation_state = if let Some(reason) = static_unavailable {
                if !definition.source_enabled
                    || matches!(
                        definition.static_status,
                        ExternalMcpStaticStatus::DisabledBySource
                    )
                {
                    ExternalMcpActivationState::SourceDisabled
                } else {
                    ExternalMcpActivationState::Unsupported { reason }
                }
            } else if conflict.is_some() && selected_candidate_id.is_none() {
                ExternalMcpActivationState::Conflict
            } else if let Some(selected) = selected_candidate_id {
                if selected != candidate_id {
                    ExternalMcpActivationState::Covered {
                        selected_candidate_id: selected.to_string(),
                    }
                } else {
                    decision_state(decision, &approval_key)
                }
            } else {
                decision_state(decision, &approval_key)
            };
            let runtime_id = matches!(activation_state, ExternalMcpActivationState::Active)
                .then(|| external_mcp_runtime_id(workspace_key, definition));
            let entry = ExternalMcpCatalogEntry {
                candidate_id: candidate_id.clone(),
                definition: definition.clone(),
                approval_key: approval_key.clone(),
                decision_key: approval_key.clone(),
                runtime_id: runtime_id.clone(),
                activation_state: activation_state.clone(),
            };
            if matches!(
                activation_state,
                ExternalMcpActivationState::ApprovalRequired
                    | ExternalMcpActivationState::ConfigurationChanged
            ) {
                state.approval_requests.push(ExternalMcpApprovalRequest {
                    candidate_id: candidate_id.clone(),
                    approval_key: approval_key.clone(),
                    decision_key: approval_key,
                    definition: definition.clone(),
                });
            }
            if let Some(runtime_id) = runtime_id {
                state.active.push(ActiveExternalMcpCandidate {
                    runtime_id,
                    definition: definition.clone(),
                });
            }
            state.entries.push(entry);
        }
        if let Some(conflict) = conflict {
            state.conflicts.push(conflict);
        }
    }

    state.entries.sort_by(|left, right| {
        left.definition
            .name
            .cmp(&right.definition.name)
            .then(left.candidate_id.cmp(&right.candidate_id))
    });
    state
        .approval_requests
        .sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    state
        .conflicts
        .sort_by(|left, right| left.server_name.cmp(&right.server_name));
    state
}

fn decision_state(
    decision: Option<&ExternalMcpDecision>,
    current_decision_key: &str,
) -> ExternalMcpActivationState {
    match decision {
        Some(decision) if decision.decision_key == current_decision_key && decision.approved => {
            ExternalMcpActivationState::Active
        }
        Some(decision) if decision.decision_key == current_decision_key => {
            ExternalMcpActivationState::Declined
        }
        Some(_) => ExternalMcpActivationState::ConfigurationChanged,
        None => ExternalMcpActivationState::ApprovalRequired,
    }
}

fn current_or_previous_mcp_decision<'a>(
    decisions: &'a BTreeMap<String, ExternalMcpDecision>,
    current_decision_key: &str,
) -> Option<&'a ExternalMcpDecision> {
    decisions.get(current_decision_key).or_else(|| {
        let (lineage, _) = current_decision_key.rsplit_once(':')?;
        decisions.iter().find_map(|(key, decision)| {
            key.rsplit_once(':')
                .is_some_and(|(candidate_lineage, _)| candidate_lineage == lineage)
                .then_some(decision)
        })
    })
}

fn build_conflict(
    execution_domain_id: &str,
    workspace_key: &str,
    group: &CandidateGroup<'_>,
    source_names: &BTreeMap<bitfun_product_domains::external_sources::SourceKey, String>,
    conflict_choices: &BTreeMap<String, String>,
) -> ExternalMcpConflict {
    let server_name = group
        .native
        .first()
        .map(|candidate| candidate.name.clone())
        .or_else(|| {
            group
                .external
                .first()
                .map(|candidate| candidate.name.clone())
        })
        .unwrap_or_default();
    let mut participants = Vec::with_capacity(group.native.len() + group.external.len());
    let mut candidates = Vec::with_capacity(participants.capacity());
    for candidate in &group.native {
        participants.push((
            candidate.candidate_id.clone(),
            candidate.behavior_version.clone(),
        ));
        candidates.push(ExternalMcpConflictCandidate {
            candidate_id: candidate.candidate_id.clone(),
            display_name: candidate.display_name.clone(),
            external: false,
            source: None,
            behavior_version: candidate.behavior_version.clone(),
            available: candidate.enabled,
            unavailable_reason: (!candidate.enabled)
                .then(|| "This BitFun MCP server is disabled".to_string()),
        });
    }
    for definition in &group.external {
        participants.push((
            definition.candidate_id(),
            definition.behavior_version.clone(),
        ));
    }
    let conflict_key = external_mcp_conflict_key(
        execution_domain_id,
        workspace_key,
        &server_name,
        participants
            .iter()
            .map(|(candidate_id, version)| (candidate_id.as_str(), version.as_str())),
    );
    for definition in &group.external {
        let reason = static_unavailable_reason(definition);
        let source_name = source_names
            .get(&definition.id.source)
            .cloned()
            .unwrap_or_else(|| "External AI application".to_string());
        candidates.push(ExternalMcpConflictCandidate {
            candidate_id: definition.candidate_id(),
            display_name: format!("{source_name}: {}", definition.name),
            external: true,
            source: Some(definition.id.source.clone()),
            behavior_version: definition.behavior_version.clone(),
            available: reason.is_none(),
            unavailable_reason: reason,
        });
    }
    let selected_candidate_id = conflict_choices.get(&conflict_key).and_then(|selected| {
        candidates
            .iter()
            .any(|candidate| candidate.candidate_id == *selected && candidate.available)
            .then(|| selected.clone())
    });
    ExternalMcpConflict {
        conflict_key,
        server_name,
        candidates,
        selected_candidate_id,
    }
}

fn static_unavailable_reason(definition: &ExternalMcpServerDefinition) -> Option<String> {
    if !definition.source_enabled {
        return Some("This MCP server is disabled in its source configuration".to_string());
    }
    match &definition.static_status {
        ExternalMcpStaticStatus::Ready => None,
        ExternalMcpStaticStatus::DisabledBySource => {
            Some("This MCP server is disabled in its source configuration".to_string())
        }
        ExternalMcpStaticStatus::Unsupported { reason }
        | ExternalMcpStaticStatus::Invalid { reason } => Some(reason.clone()),
        _ => Some("This MCP server configuration is not supported".to_string()),
    }
}

fn external_mcp_runtime_id(
    workspace_key: &str,
    definition: &ExternalMcpServerDefinition,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(workspace_key.as_bytes());
    hasher.update([0]);
    hasher.update(definition.id.stable_key().as_bytes());
    hasher.update([0]);
    hasher.update(definition.behavior_version.as_bytes());
    format!("external-mcp-{}", &hex::encode(hasher.finalize())[..32])
}
