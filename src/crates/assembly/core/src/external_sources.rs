//! Product composition and lifecycle service for external AI application sources.
//!
//! Concrete ecosystem providers are selected only in this assembly module. The
//! catalog and product surfaces remain provider- and ecosystem-neutral.

pub use bitfun_product_domains::external_integration_policy::{
    EffectiveExternalIntegrationPolicy, ExternalIntegrationAccess, ExternalIntegrationMode,
    ExternalIntegrationPolicyMutation, ExternalIntegrationPolicyOperation,
    ExternalIntegrationPolicyScope, ExternalIntegrationPolicySnapshot,
    ExternalIntegrationPolicyStatus,
};
pub use bitfun_product_domains::external_source_control::{
    ExternalCapabilityKindV1, ExternalSourceControlActionV1, ExternalSourceControlRequestV1,
    ExternalSourceControlSnapshotV1, ExternalSourceRuntimeState, ExternalSourceSurfaceSnapshotV1,
    EXTERNAL_SOURCE_CONTROL_SCHEMA_V1,
};
pub use bitfun_product_domains::external_sources::{
    prompt_command_conflict_key, EcosystemId, ExpandedPromptCommand,
    ExternalIntegrationCapabilityId, ExternalMcpActivationState, ExternalMcpApprovalRequest,
    ExternalMcpCatalogEntry, ExternalMcpConflict, ExternalMcpTransportKind,
    ExternalSourceAssetKind, ExternalSourceCatalogEntry, ExternalSourceCatalogSnapshot,
    ExternalSourceDiagnostic, ExternalSourceDiagnosticSeverity, ExternalSourceHostCapabilities,
    ExternalSourceLifecycleState, ExternalSourceOperationError, ExternalSourceOperationErrorCode,
    ExternalSourceOperationResult, ExternalSourcePublicSnapshot, ExternalToolActivationState,
    ExternalToolApprovalRequest, ExternalToolCapability, ExternalToolCatalogEntry,
    ExternalToolConflict, ExternalToolConflictCandidateKind, ExternalToolRuntimeKind,
    PromptCommandAvailability, PromptCommandCatalogEntry, PromptCommandDefinition, SourceKey,
};
pub use bitfun_product_domains::external_subagents::{
    ExternalSubagentActivationState, ExternalSubagentCompatibilityState, ExternalSubagentConflict,
    ExternalSubagentConflictCandidate, ExternalSubagentSummary,
};

use crate::external_mcp::{
    reconcile_external_mcp_catalog, BitFunExternalMcpRuntime, ExternalMcpDecision,
    ExternalMcpDecisions, ExternalMcpProductState, ExternalMcpRuntimePort,
    ExternalMcpRuntimeStatus, NativeMcpCandidate,
};
use crate::external_subagents::{
    project_external_subagents_read_only, reconcile_external_subagents, ExternalSubagentDecisions,
    ExternalSubagentProductState, DISABLED_SUBAGENT_CONFLICT_CHOICE,
};
use crate::external_tools::{
    begin_external_tool_workspace_recovery, external_tool_workspace_requires_recovery,
    merge_tool_state, project_external_tools_read_only, reconcile_external_tools,
    release_external_tool_workspace, reset_external_tool_workspace_recovery_budget,
    workspace_route_key, ExternalToolDecisions, ExternalToolProductState,
    TOOL_CONFLICT_RESELECTION_REQUIRED, UNRESOLVED_TOOL_CONFLICT_CHOICE,
};
use crate::service::config::{subscribe_config_updates, ConfigUpdateEvent};
use bitfun_external_sources::{
    DeferredDiscovery, ExternalMcpDiscoveryResult, ExternalSourceControlPlane,
    ExternalSourceCoordinator, ExternalSourceDiscoveryResult, ExternalSubagentDiscoveryResult,
    ExternalToolDiscoveryResult,
};
use bitfun_opencode_adapter::{
    OpenCodeCommandProvider, OpenCodeMcpProvider, OpenCodeSubagentProvider, OpenCodeToolProvider,
};
use bitfun_product_domains::external_integration_policy::{
    external_integration_policy_snapshot, incompatible_external_integration_policy_snapshot,
    ExternalIntegrationCapabilityDescriptor, ExternalIntegrationEcosystemDescriptor,
    ExternalIntegrationPolicyDocument, ExternalIntegrationPolicySettings,
    EXTERNAL_INTEGRATION_POLICY_SCHEMA_MAJOR,
};
use bitfun_product_domains::external_sources::{
    ExecutionDomainId, ExternalMcpSourceProvider, ExternalSourceContext, ExternalSourceScope,
    ExternalToolSourceProvider, PromptCommandSourceProvider,
};
use bitfun_product_domains::external_subagents::ExternalSubagentSourceProvider;
use bitfun_services_core::json_store::JsonFileStore;
use bitfun_services_integrations::file_watch::{FileWatchService, FileWatcherConfig};
use dashmap::{mapref::entry::Entry, DashMap};
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard, OnceLock, Weak};
use tokio::sync::broadcast;

const PROVIDER_DISCOVERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const EXTERNAL_SOURCE_PREFERENCES_FILE: &str = "external-sources.json";
const SUBAGENT_CONFLICT_RESELECTION_REQUIRED: &str = "__bitfun_reselection_required__";
const OPENCODE_ECOSYSTEM_ID: &str = "opencode";
pub const EXTERNAL_CAPABILITY_COMMAND: &str = "command";
pub const EXTERNAL_CAPABILITY_TOOL: &str = "tool";
pub const EXTERNAL_CAPABILITY_SUBAGENT: &str = "subagent";
pub const EXTERNAL_CAPABILITY_MCP: &str = "mcp";
const EXTERNAL_ADAPTER_CONTRACT_MAJOR: u32 = 1;

fn external_capability_descriptor(
    capability_id: &str,
    recommended_access: ExternalIntegrationAccess,
    safety_ceiling: ExternalIntegrationAccess,
) -> ExternalIntegrationCapabilityDescriptor {
    ExternalIntegrationCapabilityDescriptor {
        capability_id: ExternalIntegrationCapabilityId::new(capability_id)
            .expect("built-in external integration capability id is valid"),
        recommended_access,
        safety_ceiling,
    }
}

/// Internal SDK-ready registration seam. Adapters only contribute discovery
/// providers and metadata; execution remains owned by BitFun policy/runtime.
#[derive(Clone)]
struct ExternalEcosystemRegistration {
    descriptor: ExternalIntegrationEcosystemDescriptor,
    contract_major: u32,
    upstream_format_revision: &'static str,
    command_provider: Option<Arc<dyn PromptCommandSourceProvider>>,
    tool_provider: Option<Arc<dyn ExternalToolSourceProvider>>,
    subagent_provider: Option<Arc<dyn ExternalSubagentSourceProvider>>,
    mcp_provider: Option<Arc<dyn ExternalMcpSourceProvider>>,
}

impl ExternalEcosystemRegistration {
    fn validate(&self) -> Result<(), String> {
        if self.contract_major != EXTERNAL_ADAPTER_CONTRACT_MAJOR {
            return Err(format!(
                "adapter contract major {} is not supported",
                self.contract_major
            ));
        }
        let ecosystem_id = &self.descriptor.ecosystem_id;
        let capabilities = self
            .descriptor
            .capabilities
            .iter()
            .map(|capability| capability.capability_id.as_str())
            .collect::<BTreeSet<_>>();
        let providers = [
            (
                EXTERNAL_CAPABILITY_COMMAND,
                self.command_provider
                    .as_ref()
                    .map(|provider| provider.identity().ecosystem_id),
            ),
            (
                EXTERNAL_CAPABILITY_TOOL,
                self.tool_provider
                    .as_ref()
                    .map(|provider| provider.identity().ecosystem_id),
            ),
            (
                EXTERNAL_CAPABILITY_SUBAGENT,
                self.subagent_provider
                    .as_ref()
                    .map(|provider| provider.identity().ecosystem_id),
            ),
            (
                EXTERNAL_CAPABILITY_MCP,
                self.mcp_provider
                    .as_ref()
                    .map(|provider| provider.identity().ecosystem_id),
            ),
        ];
        for (capability_id, provider_ecosystem) in providers {
            if capabilities.contains(capability_id) != provider_ecosystem.is_some() {
                return Err(format!(
                    "capability '{capability_id}' and provider registration do not match"
                ));
            }
            if provider_ecosystem
                .as_ref()
                .is_some_and(|provider_ecosystem| provider_ecosystem != ecosystem_id)
            {
                return Err(format!(
                    "capability '{capability_id}' provider belongs to a different ecosystem"
                ));
            }
        }
        Ok(())
    }
}

fn default_external_integration_registry() -> Vec<ExternalEcosystemRegistration> {
    vec![ExternalEcosystemRegistration {
        descriptor: ExternalIntegrationEcosystemDescriptor {
            ecosystem_id: EcosystemId::new(OPENCODE_ECOSYSTEM_ID)
                .expect("OpenCode ecosystem id is valid"),
            display_name: "OpenCode".to_string(),
            adapter_revision: "1".to_string(),
            capabilities: vec![
                external_capability_descriptor(
                    EXTERNAL_CAPABILITY_COMMAND,
                    ExternalIntegrationAccess::Auto,
                    ExternalIntegrationAccess::Auto,
                ),
                external_capability_descriptor(
                    EXTERNAL_CAPABILITY_TOOL,
                    ExternalIntegrationAccess::AskBeforeUse,
                    ExternalIntegrationAccess::AskBeforeUse,
                ),
                external_capability_descriptor(
                    EXTERNAL_CAPABILITY_SUBAGENT,
                    ExternalIntegrationAccess::AskBeforeUse,
                    ExternalIntegrationAccess::AskBeforeUse,
                ),
                external_capability_descriptor(
                    EXTERNAL_CAPABILITY_MCP,
                    ExternalIntegrationAccess::AskBeforeUse,
                    ExternalIntegrationAccess::AskBeforeUse,
                ),
            ],
        },
        contract_major: EXTERNAL_ADAPTER_CONTRACT_MAJOR,
        upstream_format_revision: "opencode-config-v1",
        command_provider: Some(Arc::new(OpenCodeCommandProvider::default())),
        tool_provider: Some(Arc::new(OpenCodeToolProvider::default())),
        subagent_provider: Some(Arc::new(OpenCodeSubagentProvider::default())),
        mcp_provider: Some(Arc::new(OpenCodeMcpProvider::default())),
    }]
}

fn default_external_integration_ecosystems() -> Vec<ExternalIntegrationEcosystemDescriptor> {
    default_external_integration_registry()
        .into_iter()
        .filter(|registration| {
            let compatible = registration.validate();
            if let Err(error) = &compatible {
                log::warn!(
                    "External ecosystem adapter skipped ecosystem={} contract_major={} host_contract_major={} upstream_format={} reason={}",
                    safe_external_log_token(registration.descriptor.ecosystem_id.as_str()),
                    registration.contract_major,
                    EXTERNAL_ADAPTER_CONTRACT_MAJOR,
                    safe_external_log_token(registration.upstream_format_revision),
                    safe_external_log_token(error),
                );
            }
            compatible.is_ok()
        })
        .map(|registration| registration.descriptor)
        .collect()
}
/// Kept stable so existing approval fingerprints remain valid. Product hosts
/// resolve this identity once; capability owners never hard-code it.
const LEGACY_LOCAL_EXECUTION_DOMAIN_ID: &str = "local-user";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ExternalSourcesConfig {
    #[serde(default)]
    integration_policy: StoredExternalIntegrationPolicy,
    /// Bounded recovery history for a policy document written by an
    /// incompatible host. This remains persistence-only and is never projected
    /// through public Host APIs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    integration_policy_backups: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    suppressed_source_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    conflict_choices: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    conflict_lineage_current_keys: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    conflicted_candidate_ids: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    approved_tool_targets: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    declined_tool_decisions: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    tool_conflict_choices: BTreeMap<String, String>,
    #[serde(default)]
    preference_revision: u64,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    approved_subagent_envelopes: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    declined_subagent_decisions: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    subagent_conflict_choices: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    subagent_conflict_lineage_current_keys: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    mcp_server_decisions: BTreeMap<String, ExternalMcpDecision>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    mcp_conflict_choices: BTreeMap<String, String>,
    /// Preserves fields written by a newer preferences schema.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    extensions: BTreeMap<String, serde_json::Value>,
}

/// Persistence-only version gate. Unknown major versions remain opaque until
/// the user explicitly backs them up and resets; this prevents current structs
/// from partially decoding a future policy shape before compatibility is known.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StoredExternalIntegrationPolicy {
    Known(ExternalIntegrationPolicyDocument),
    Unknown {
        schema_major: u32,
        raw: serde_json::Value,
    },
}

impl StoredExternalIntegrationPolicy {
    fn schema_major(&self) -> u32 {
        match self {
            Self::Known(document) => document.schema_major,
            Self::Unknown { schema_major, .. } => *schema_major,
        }
    }

    fn known(&self) -> Option<&ExternalIntegrationPolicyDocument> {
        match self {
            Self::Known(document) => Some(document),
            Self::Unknown { .. } => None,
        }
    }

    fn known_mut(&mut self) -> Option<&mut ExternalIntegrationPolicyDocument> {
        match self {
            Self::Known(document) => Some(document),
            Self::Unknown { .. } => None,
        }
    }

    fn raw_value(&self) -> serde_json::Value {
        match self {
            Self::Known(document) => serde_json::to_value(document).unwrap_or_else(|_| {
                serde_json::json!({
                    "schemaMajor": EXTERNAL_INTEGRATION_POLICY_SCHEMA_MAJOR,
                    "userDefaults": { "enabled": false }
                })
            }),
            Self::Unknown { raw, .. } => raw.clone(),
        }
    }
}

impl Default for StoredExternalIntegrationPolicy {
    fn default() -> Self {
        Self::Known(ExternalIntegrationPolicyDocument::default())
    }
}

impl Serialize for StoredExternalIntegrationPolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.raw_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StoredExternalIntegrationPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;

        let raw = serde_json::Value::deserialize(deserializer)?;
        let schema_major = raw
            .get("schemaMajor")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(EXTERNAL_INTEGRATION_POLICY_SCHEMA_MAJOR);
        if schema_major != EXTERNAL_INTEGRATION_POLICY_SCHEMA_MAJOR {
            return Ok(Self::Unknown { schema_major, raw });
        }
        serde_json::from_value(raw)
            .map(Self::Known)
            .map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone)]
struct ExternalSourcePreferenceStore {
    path: PathBuf,
}

impl ExternalSourcePreferenceStore {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn global() -> Result<Self, String> {
        let path_manager =
            crate::infrastructure::try_get_path_manager_arc().map_err(|error| error.to_string())?;
        Ok(Self::new(
            path_manager
                .user_config_dir()
                .join(EXTERNAL_SOURCE_PREFERENCES_FILE),
        ))
    }

    async fn read(&self) -> Result<ExternalSourcesConfig, String> {
        JsonFileStore
            .read_locked_optional(&self.path)
            .await
            .map(|config| config.unwrap_or_default())
            .map_err(|error| error.to_string())
    }

    async fn update<R>(
        &self,
        update: impl FnOnce(&mut ExternalSourcesConfig) -> R,
    ) -> Result<(R, ExternalSourcesConfig), String> {
        JsonFileStore
            .update_locked(&self.path, ExternalSourcesConfig::default(), update)
            .await
            .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Copy)]
enum WorkerRecoveryPolicy {
    Preserve,
    PendingOnce,
    ResetAndAttempt,
}

fn config_update_refreshes_external_model_bindings(event: &ConfigUpdateEvent) -> bool {
    matches!(event, ConfigUpdateEvent::ModelConfigurationUpdated)
}

pub(crate) fn host_execution_domain_id() -> Result<ExecutionDomainId, String> {
    ExecutionDomainId::new(LEGACY_LOCAL_EXECUTION_DOMAIN_ID).map_err(|error| error.to_string())
}

fn workspace_policy_key(workspace_root: Option<&Path>) -> Option<String> {
    let route = workspace_route_key(workspace_root);
    workspace_policy_key_from_route(&route)
}

fn workspace_policy_key_from_route(route: &str) -> Option<String> {
    if route == "<global>" {
        return None;
    }
    let normalized = route.replace('\\', "/");
    #[cfg(windows)]
    let normalized = normalized.to_ascii_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    Some(format!(
        "workspace:{}",
        hex::encode(&hasher.finalize()[..16])
    ))
}

fn external_source_safe_mode_key(execution_domain_id: &str, workspace_route: &str) -> String {
    format!("{execution_domain_id}\u{1f}{workspace_route}")
}

fn external_source_safe_mode_enabled_for(execution_domain_id: &str, workspace_route: &str) -> bool {
    safe_mode_workspaces().contains_key(&external_source_safe_mode_key(
        execution_domain_id,
        workspace_route,
    ))
}

fn set_external_source_safe_mode_for(
    execution_domain_id: &str,
    workspace_route: &str,
    enabled: bool,
) {
    let key = external_source_safe_mode_key(execution_domain_id, workspace_route);
    if enabled {
        safe_mode_workspaces().insert(key, ());
    } else {
        safe_mode_workspaces().remove(&key);
    }
}

fn integration_policy_snapshot(
    preferences: &ExternalSourcesConfig,
    workspace_root: Option<&Path>,
) -> Result<ExternalIntegrationPolicySnapshot, String> {
    let ecosystems = default_external_integration_ecosystems();
    match preferences.integration_policy.known() {
        Some(document) => external_integration_policy_snapshot(
            document,
            workspace_policy_key(workspace_root).as_deref(),
            ecosystems,
        ),
        None => incompatible_external_integration_policy_snapshot(
            preferences.integration_policy.schema_major(),
            ecosystems,
        ),
    }
    .map_err(|error| format!("policy_unavailable: {error}"))
}

fn integration_access(
    policy: &ExternalIntegrationPolicySnapshot,
    ecosystem_id: &str,
    capability_id: &str,
) -> ExternalIntegrationAccess {
    let Some(ecosystem) = policy
        .effective
        .ecosystems
        .iter()
        .find_map(|(id, policy)| (id.as_str() == ecosystem_id).then_some(policy))
    else {
        return ExternalIntegrationAccess::Disabled;
    };
    ecosystem
        .capabilities
        .iter()
        .find_map(|(id, access)| (id.as_str() == capability_id).then_some(access.clone()))
        .unwrap_or(ExternalIntegrationAccess::Disabled)
}

fn integration_capability_is_discoverable(
    policy: &ExternalIntegrationPolicySnapshot,
    ecosystem_id: &str,
    capability_id: &str,
) -> bool {
    !matches!(
        integration_access(policy, ecosystem_id, capability_id),
        ExternalIntegrationAccess::Disabled | ExternalIntegrationAccess::Unknown(_)
    )
}

fn integration_capability_is_active(
    policy: &ExternalIntegrationPolicySnapshot,
    ecosystem_id: &str,
    capability_id: &str,
) -> bool {
    matches!(
        integration_access(policy, ecosystem_id, capability_id),
        ExternalIntegrationAccess::Auto | ExternalIntegrationAccess::AskBeforeUse
    )
}

fn ecosystems_with_discoverable_capability(
    policy: &ExternalIntegrationPolicySnapshot,
    capability_id: &str,
) -> BTreeSet<EcosystemId> {
    policy
        .registered_ecosystems
        .iter()
        .filter(|descriptor| {
            integration_capability_is_discoverable(
                policy,
                descriptor.ecosystem_id.as_str(),
                capability_id,
            )
        })
        .map(|descriptor| descriptor.ecosystem_id.clone())
        .collect()
}

fn ecosystems_with_active_capability(
    policy: &ExternalIntegrationPolicySnapshot,
    capability_id: &str,
) -> BTreeSet<EcosystemId> {
    policy
        .registered_ecosystems
        .iter()
        .filter(|descriptor| {
            integration_capability_is_active(
                policy,
                descriptor.ecosystem_id.as_str(),
                capability_id,
            )
        })
        .map(|descriptor| descriptor.ecosystem_id.clone())
        .collect()
}

fn source_ecosystem_id(
    snapshot: &ExternalSourceCatalogSnapshot,
    source_key: &SourceKey,
) -> Result<EcosystemId, String> {
    snapshot
        .sources
        .iter()
        .find(|source| source.record.key == *source_key)
        .map(|source| source.record.ecosystem_id.clone())
        .ok_or_else(|| {
            encoded_operation_error(
                ExternalSourceOperationErrorCode::NotFound,
                format!(
                    "External source '{}' is no longer available",
                    source_key.stable_key()
                ),
                false,
            )
        })
}

fn ensure_source_capability_active(
    snapshot: &ExternalSourceCatalogSnapshot,
    source_key: &SourceKey,
    capability_id: &str,
) -> Result<(), String> {
    let ecosystem_id = source_ecosystem_id(snapshot, source_key)?;
    integration_capability_is_active(
        &snapshot.integration_policy,
        ecosystem_id.as_str(),
        capability_id,
    )
    .then_some(())
    .ok_or_else(|| encoded_operation_error(
        ExternalSourceOperationErrorCode::PolicyLimited,
        format!(
            "External capability '{capability_id}' is not enabled for ecosystem '{}' in this workspace",
            ecosystem_id.as_str()
        ),
        false,
    ))
}

fn ensure_source_set_capability_active(
    snapshot: &ExternalSourceCatalogSnapshot,
    source_keys: &[SourceKey],
    capability_id: &str,
) -> Result<(), String> {
    if source_keys.is_empty() {
        return Err(encoded_operation_error(
            ExternalSourceOperationErrorCode::NotFound,
            "External source provenance is missing",
            false,
        ));
    }
    for source_key in source_keys {
        ensure_source_capability_active(snapshot, source_key, capability_id)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalSourceServiceProfile {
    LocalExecution,
    ReadOnlyProjection,
}

struct WorkspaceExternalSourceService {
    profile: ExternalSourceServiceProfile,
    workspace_root: Option<PathBuf>,
    execution_domain_id: ExecutionDomainId,
    control_plane: Arc<ExternalSourceControlPlane>,
    snapshot: StdMutex<ExternalSourceCatalogSnapshot>,
    updates: broadcast::Sender<ExternalSourceCatalogSnapshot>,
    watch_states: tokio::sync::Mutex<BTreeMap<(PathBuf, bool), bool>>,
    refresh_gate: tokio::sync::Mutex<()>,
    product_rebuild_gate: tokio::sync::Mutex<()>,
    mcp_runtime: Arc<dyn ExternalMcpRuntimePort>,
    active_mcp_runtime_ids: tokio::sync::Mutex<BTreeSet<String>>,
    initial_refresh_completed: AtomicBool,
    background_refresh_scheduled: AtomicBool,
    initial_refresh_gate: tokio::sync::Mutex<()>,
    keepalive_started: AtomicBool,
    last_access_epoch_seconds: AtomicU64,
    subagent_expiry_schedule: AtomicU64,
    watcher: Arc<FileWatchService>,
    #[cfg(test)]
    tool_decision_gate_waiting: tokio::sync::Notify,
    #[cfg(test)]
    tool_decision_gate_acquired: tokio::sync::Notify,
}

impl WorkspaceExternalSourceService {
    async fn create(
        workspace_root: Option<PathBuf>,
        profile: ExternalSourceServiceProfile,
    ) -> Result<Arc<Self>, String> {
        let execution_domain_id = host_execution_domain_id()?;
        let context = ExternalSourceContext {
            workspace_root: workspace_root.clone(),
            execution_domain_id: execution_domain_id.clone(),
        };
        let registrations = default_external_integration_registry()
            .into_iter()
            .filter_map(|registration| match registration.validate() {
                Ok(()) => Some(registration),
                Err(error) => {
                    log::warn!(
                        "External ecosystem registration rejected ecosystem={} reason={}",
                        safe_external_log_token(registration.descriptor.ecosystem_id.as_str()),
                        safe_external_log_token(&error),
                    );
                    None
                }
            })
            .collect::<Vec<_>>();
        let providers: Vec<Arc<dyn PromptCommandSourceProvider>> = registrations
            .iter()
            .filter_map(|registration| registration.command_provider.as_ref().map(Arc::clone))
            .collect();
        let tool_providers: Vec<Arc<dyn ExternalToolSourceProvider>> = registrations
            .iter()
            .filter_map(|registration| registration.tool_provider.as_ref().map(Arc::clone))
            .collect();
        let subagent_providers: Vec<Arc<dyn ExternalSubagentSourceProvider>> = registrations
            .iter()
            .filter_map(|registration| registration.subagent_provider.as_ref().map(Arc::clone))
            .collect();
        let mcp_providers: Vec<Arc<dyn ExternalMcpSourceProvider>> = registrations
            .iter()
            .filter_map(|registration| registration.mcp_provider.as_ref().map(Arc::clone))
            .collect();
        let control_plane = Arc::new(ExternalSourceControlPlane::new(
            context,
            providers,
            tool_providers,
            subagent_providers,
            mcp_providers,
        )?);
        let preferences = read_external_sources_config().await?;
        let suppressed_sources = preferences
            .suppressed_source_keys
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        control_plane.replace_suppressed_sources(suppressed_sources);
        control_plane.commands_mut(|coordinator| {
            coordinator.replace_conflict_choices(preferences.conflict_choices.clone());
            coordinator.replace_conflict_lineage_current_keys(
                preferences.conflict_lineage_current_keys.clone(),
            );
            coordinator
                .replace_conflicted_candidate_ids(preferences.conflicted_candidate_ids.clone());
        });
        let mut initial_snapshot = merge_tool_state(
            control_plane.commands(|coordinator| coordinator.snapshot()),
            &control_plane.tools(|coordinator| coordinator.snapshot()),
            ExternalToolProductState::default(),
        );
        initial_snapshot.subagent_generation =
            control_plane.subagents(|coordinator| coordinator.snapshot().generation);
        initial_snapshot.preference_revision = preferences.preference_revision;
        initial_snapshot.integration_policy =
            integration_policy_snapshot(&preferences, workspace_root.as_deref())?;
        let (updates, _) = broadcast::channel(32);
        let service = Arc::new(Self {
            profile,
            workspace_root,
            execution_domain_id,
            control_plane,
            snapshot: StdMutex::new(initial_snapshot),
            updates,
            watch_states: tokio::sync::Mutex::new(BTreeMap::new()),
            refresh_gate: tokio::sync::Mutex::new(()),
            product_rebuild_gate: tokio::sync::Mutex::new(()),
            mcp_runtime: Arc::new(BitFunExternalMcpRuntime),
            active_mcp_runtime_ids: tokio::sync::Mutex::new(BTreeSet::new()),
            initial_refresh_completed: AtomicBool::new(false),
            background_refresh_scheduled: AtomicBool::new(false),
            initial_refresh_gate: tokio::sync::Mutex::new(()),
            keepalive_started: AtomicBool::new(false),
            last_access_epoch_seconds: AtomicU64::new(epoch_seconds()),
            subagent_expiry_schedule: AtomicU64::new(0),
            watcher: Arc::new(FileWatchService::new(FileWatcherConfig::default())),
            #[cfg(test)]
            tool_decision_gate_waiting: tokio::sync::Notify::new(),
            #[cfg(test)]
            tool_decision_gate_acquired: tokio::sync::Notify::new(),
        });
        service.start_watching().await;
        if profile == ExternalSourceServiceProfile::LocalExecution {
            service.start_model_config_watching();
        }
        Ok(service)
    }

    async fn refresh(self: &Arc<Self>) -> Result<ExternalSourceCatalogSnapshot, String> {
        self.refresh_with_worker_recovery(WorkerRecoveryPolicy::ResetAndAttempt)
            .await
    }

    async fn refresh_preserving_worker_recovery(
        self: &Arc<Self>,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        self.refresh_with_worker_recovery(WorkerRecoveryPolicy::Preserve)
            .await
    }

    async fn refresh_worker_loss_once(
        self: &Arc<Self>,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        self.refresh_with_worker_recovery(WorkerRecoveryPolicy::PendingOnce)
            .await
    }

    async fn refresh_with_worker_recovery(
        self: &Arc<Self>,
        recovery_policy: WorkerRecoveryPolicy,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        // Preferences are global to the local execution domain and may be
        // changed by another BitFun process. Synchronize before every refresh
        // so a cached CLI/Desktop service cannot keep an externally disabled
        // source active.
        sync_service_preferences(self).await?;
        let _refresh_guard = self.refresh_gate.lock().await;
        let preferences = read_external_sources_config().await?;
        let policy = integration_policy_snapshot(&preferences, self.workspace_root.as_deref())?;
        if self.profile == ExternalSourceServiceProfile::LocalExecution
            && matches!(recovery_policy, WorkerRecoveryPolicy::ResetAndAttempt)
        {
            reset_external_tool_workspace_recovery_budget(self.workspace_root.as_deref()).await;
        }
        let recovery_targets = if self.profile == ExternalSourceServiceProfile::LocalExecution
            && matches!(
                recovery_policy,
                WorkerRecoveryPolicy::PendingOnce | WorkerRecoveryPolicy::ResetAndAttempt
            ) {
            begin_external_tool_workspace_recovery(self.workspace_root.as_deref()).await
        } else {
            BTreeSet::new()
        };
        let mut requests = Vec::new();
        let mut disabled_command_results = Vec::new();
        for request in lock_coordinator(&self.control_plane).discovery_requests() {
            if integration_capability_is_discoverable(
                &policy,
                request.ecosystem_id().as_str(),
                EXTERNAL_CAPABILITY_COMMAND,
            ) {
                requests.push(request);
            } else {
                disabled_command_results.push(request.disabled());
            }
        }
        let mut tool_requests = Vec::new();
        let mut disabled_tool_results = Vec::new();
        for request in lock_tool_coordinator(&self.control_plane).discovery_requests() {
            if integration_capability_is_discoverable(
                &policy,
                request.ecosystem_id().as_str(),
                EXTERNAL_CAPABILITY_TOOL,
            ) {
                tool_requests.push(request);
            } else {
                disabled_tool_results.push(request.disabled());
            }
        }
        let mut subagent_requests = Vec::new();
        let mut disabled_subagent_results = Vec::new();
        for request in lock_subagent_coordinator(&self.control_plane).discovery_requests() {
            if integration_capability_is_discoverable(
                &policy,
                request.ecosystem_id().as_str(),
                EXTERNAL_CAPABILITY_SUBAGENT,
            ) {
                subagent_requests.push(request);
            } else {
                disabled_subagent_results.push(request.disabled());
            }
        }
        let mut mcp_requests = Vec::new();
        let mut disabled_mcp_results = Vec::new();
        for request in lock_mcp_coordinator(&self.control_plane).discovery_requests() {
            if integration_capability_is_discoverable(
                &policy,
                request.ecosystem_id().as_str(),
                EXTERNAL_CAPABILITY_MCP,
            ) {
                mcp_requests.push(request);
            } else {
                disabled_mcp_results.push(request.disabled());
            }
        }
        let (command_batch, tool_batch, subagent_batch, mcp_batch) = tokio::join!(
            self.control_plane
                .discover_commands(requests, PROVIDER_DISCOVERY_TIMEOUT),
            self.control_plane
                .discover_tools(tool_requests, PROVIDER_DISCOVERY_TIMEOUT),
            self.control_plane
                .discover_subagents(subagent_requests, PROVIDER_DISCOVERY_TIMEOUT),
            self.control_plane
                .discover_mcp(mcp_requests, PROVIDER_DISCOVERY_TIMEOUT),
        );
        let mut results = command_batch.immediate;
        results.append(&mut disabled_command_results);
        let mut tool_results = tool_batch.immediate;
        tool_results.append(&mut disabled_tool_results);
        let mut subagent_results = subagent_batch.immediate;
        subagent_results.append(&mut disabled_subagent_results);
        let mut mcp_results = mcp_batch.immediate;
        mcp_results.append(&mut disabled_mcp_results);
        let command_snapshot =
            lock_coordinator(&self.control_plane).apply_discovery_results(results);
        lock_tool_coordinator(&self.control_plane).apply_discovery_results(tool_results);
        let subagent_snapshot = lock_subagent_coordinator(&self.control_plane)
            .apply_discovery_results(subagent_results);
        lock_mcp_coordinator(&self.control_plane).apply_discovery_results(mcp_results);
        for deferred in command_batch.deferred {
            self.schedule_deferred_command_discovery(deferred);
        }
        for deferred in tool_batch.deferred {
            self.schedule_deferred_tool_discovery(deferred);
        }
        for deferred in subagent_batch.deferred {
            self.schedule_deferred_subagent_discovery(deferred);
        }
        for deferred in mcp_batch.deferred {
            self.schedule_deferred_mcp_discovery(deferred);
        }
        self.schedule_subagent_last_valid_expiry(&subagent_snapshot);
        self.ensure_watch_roots(&policy).await;
        let snapshot = self
            .rebuild_product_snapshot_with_worker_recovery(command_snapshot, &recovery_targets)
            .await;
        let snapshot = snapshot?;
        let _ = self.updates.send(snapshot.clone());
        self.initial_refresh_completed
            .store(true, Ordering::Release);
        Ok(snapshot)
    }

    async fn ensure_initial_refresh_with<F, Fut>(
        &self,
        refresh: F,
    ) -> Result<ExternalSourceCatalogSnapshot, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<ExternalSourceCatalogSnapshot, String>>,
    {
        if self.initial_refresh_completed.load(Ordering::Acquire) {
            return Ok(self.snapshot());
        }
        let _initial_refresh_guard = self.initial_refresh_gate.lock().await;
        if self.initial_refresh_completed.load(Ordering::Acquire) {
            return Ok(self.snapshot());
        }
        let snapshot = refresh().await?;
        self.initial_refresh_completed
            .store(true, Ordering::Release);
        Ok(snapshot)
    }

    async fn ensure_initial_refresh(
        self: &Arc<Self>,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        self.ensure_initial_refresh_with(|| self.refresh()).await
    }

    async fn rebuild_product_snapshot(
        &self,
        command_snapshot: ExternalSourceCatalogSnapshot,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        self.rebuild_product_snapshot_with_worker_recovery(command_snapshot, &BTreeSet::new())
            .await
    }

    async fn rebuild_product_snapshot_with_worker_recovery(
        &self,
        _command_snapshot: ExternalSourceCatalogSnapshot,
        worker_recovery_targets: &BTreeSet<String>,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let _rebuild_guard = self.product_rebuild_gate.lock().await;
        let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
        let mut preferences = read_external_sources_config().await?;
        let mut policy = integration_policy_snapshot(&preferences, self.workspace_root.as_deref())?;
        if self.profile == ExternalSourceServiceProfile::ReadOnlyProjection {
            return self
                .rebuild_read_only_projection(command_snapshot, preferences, policy)
                .await;
        }
        let command_discoverable =
            !ecosystems_with_discoverable_capability(&policy, EXTERNAL_CAPABILITY_COMMAND)
                .is_empty();
        let command_active_ecosystems =
            ecosystems_with_active_capability(&policy, EXTERNAL_CAPABILITY_COMMAND);
        let safe_mode = self.safe_mode_enabled();
        let tool_discoverable =
            !ecosystems_with_discoverable_capability(&policy, EXTERNAL_CAPABILITY_TOOL).is_empty();
        let tool_active_ecosystems = if safe_mode {
            BTreeSet::new()
        } else {
            ecosystems_with_active_capability(&policy, EXTERNAL_CAPABILITY_TOOL)
        };
        let subagent_discoverable =
            !ecosystems_with_discoverable_capability(&policy, EXTERNAL_CAPABILITY_SUBAGENT)
                .is_empty();
        let subagent_active_ecosystems = if safe_mode {
            BTreeSet::new()
        } else {
            ecosystems_with_active_capability(&policy, EXTERNAL_CAPABILITY_SUBAGENT)
        };
        let mcp_discoverable =
            !ecosystems_with_discoverable_capability(&policy, EXTERNAL_CAPABILITY_MCP).is_empty();
        let mcp_active_ecosystems = if safe_mode {
            BTreeSet::new()
        } else {
            ecosystems_with_active_capability(&policy, EXTERNAL_CAPABILITY_MCP)
        };
        let mut state = reconcile_external_tools(
            self.workspace_root.as_deref(),
            self.execution_domain_id.as_str(),
            &self.control_plane,
            ExternalToolDecisions {
                active_ecosystems: &tool_active_ecosystems,
                approved_targets: &preferences.approved_tool_targets,
                declined_decisions_by_approval: &preferences.declined_tool_decisions,
                conflict_choices: &preferences.tool_conflict_choices,
            },
            worker_recovery_targets,
        )
        .await;
        if let Err(error) = persist_observed_tool_conflicts(&state.conflicts).await {
            state.diagnostics.push(ExternalSourceDiagnostic {
                severity: bitfun_product_domains::external_sources::ExternalSourceDiagnosticSeverity::Warning,
                asset_kind: bitfun_product_domains::external_sources::ExternalSourceAssetKind::Tool,
                code: "external_tool.conflict_history_write_failed".to_string(),
                message: format!(
                    "Could not persist external tool conflict history; the current catalog remains fail-closed: {error}"
                ),
                source: None,
            });
        }
        let tool_snapshot = lock_tool_coordinator(&self.control_plane).snapshot();
        let mut snapshot = merge_tool_state(command_snapshot, &tool_snapshot, state);
        let mcp_snapshot = lock_mcp_coordinator(&self.control_plane).snapshot();
        let mcp_workspace_key = workspace_route_key(self.workspace_root.as_deref());
        let native_mcp_candidates = if !mcp_active_ecosystems.is_empty() {
            load_native_mcp_candidates().await
        } else {
            Ok(Vec::new())
        };
        let mut mcp_state = match native_mcp_candidates {
            Ok(native_candidates) => reconcile_external_mcp_catalog(
                self.execution_domain_id.as_str(),
                &mcp_workspace_key,
                &mcp_snapshot,
                &native_candidates,
                ExternalMcpDecisions {
                    active_ecosystems: &mcp_active_ecosystems,
                    server_decisions: &preferences.mcp_server_decisions,
                    conflict_choices: &preferences.mcp_conflict_choices,
                },
            ),
            Err(error) => {
                let mut state = reconcile_external_mcp_catalog(
                    self.execution_domain_id.as_str(),
                    &mcp_workspace_key,
                    &mcp_snapshot,
                    &[],
                    ExternalMcpDecisions {
                        active_ecosystems: &mcp_active_ecosystems,
                        server_decisions: &preferences.mcp_server_decisions,
                        conflict_choices: &preferences.mcp_conflict_choices,
                    },
                );
                for entry in &mut state.entries {
                    entry.runtime_id = None;
                    entry.activation_state = ExternalMcpActivationState::RuntimeUnavailable {
                        reason: error.clone(),
                    };
                }
                state.active.clear();
                state
            }
        };
        self.reconcile_mcp_runtime(&mut mcp_state).await;
        merge_mcp_state(&mut snapshot, &mcp_snapshot, mcp_state);
        let subagent_snapshot = lock_subagent_coordinator(&self.control_plane).snapshot();
        let mut subagent_state = reconcile_external_subagents(
            self.workspace_root.as_deref(),
            self.execution_domain_id.as_str(),
            &subagent_snapshot,
            ExternalSubagentDecisions {
                active_ecosystems: &subagent_active_ecosystems,
                approved_envelopes: &preferences.approved_subagent_envelopes,
                declined_decisions: &preferences.declined_subagent_decisions,
                conflict_choices: &preferences.subagent_conflict_choices,
                conflict_lineage_current_keys: &preferences.subagent_conflict_lineage_current_keys,
            },
        )
        .await;
        {
            match persist_observed_subagent_conflicts(
                &subagent_state.observed_conflict_lineage_current_keys,
            )
            .await
            {
                Ok((_history_changed, authoritative)) => {
                    let decisions_changed = authoritative.preference_revision
                        != preferences.preference_revision
                        || authoritative.approved_subagent_envelopes
                            != preferences.approved_subagent_envelopes
                        || authoritative.declined_subagent_decisions
                            != preferences.declined_subagent_decisions
                        || authoritative.subagent_conflict_choices
                            != preferences.subagent_conflict_choices
                        || authoritative.subagent_conflict_lineage_current_keys
                            != preferences.subagent_conflict_lineage_current_keys;
                    preferences = authoritative;
                    if decisions_changed {
                        subagent_state = reconcile_external_subagents(
                            self.workspace_root.as_deref(),
                            self.execution_domain_id.as_str(),
                            &subagent_snapshot,
                            ExternalSubagentDecisions {
                                active_ecosystems: &subagent_active_ecosystems,
                                approved_envelopes: &preferences.approved_subagent_envelopes,
                                declined_decisions: &preferences.declined_subagent_decisions,
                                conflict_choices: &preferences.subagent_conflict_choices,
                                conflict_lineage_current_keys: &preferences
                                    .subagent_conflict_lineage_current_keys,
                            },
                        )
                        .await;
                    }
                }
                Err(error) => {
                    snapshot.diagnostics.push(ExternalSourceDiagnostic::warning(
                    "external_subagent.conflict_history_write_failed",
                    format!(
                        "Could not persist external subagent conflict history; routes remain unavailable: {error}"
                    ),
                    None,
                ).with_asset_kind(ExternalSourceAssetKind::Subagent));
                }
            }
        }
        merge_subagent_state(
            &mut snapshot,
            &subagent_snapshot,
            &subagent_state,
            preferences.preference_revision,
        );
        if let Some(workspace_root) = self.workspace_root.as_deref() {
            crate::agentic::agents::get_agent_registry().install_external_subagent_routes(
                workspace_root,
                subagent_state.registrations,
                subagent_state.routes,
            );
        }
        let source_ecosystems = snapshot
            .sources
            .iter()
            .map(|source| {
                (
                    source.record.key.clone(),
                    source.record.ecosystem_id.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let restricted = PromptCommandAvailability::Restricted {
            reason: "External command execution is disabled by integration policy".to_string(),
            required_capabilities: vec![EXTERNAL_CAPABILITY_COMMAND.to_string()],
        };
        for command in &mut snapshot.commands {
            if !source_ecosystems
                .get(&command.definition.id.source)
                .is_some_and(|ecosystem| command_active_ecosystems.contains(ecosystem))
            {
                command.definition.availability = restricted.clone();
            }
        }
        for conflict in &mut snapshot.command_conflicts {
            for candidate in &mut conflict.candidates {
                if !command_active_ecosystems.contains(&candidate.ecosystem_id) {
                    candidate.availability = restricted.clone();
                    if conflict.selected_candidate_id.as_deref()
                        == Some(candidate.candidate_id.as_str())
                    {
                        conflict.selected_candidate_id = None;
                    }
                }
            }
        }
        if !tool_discoverable {
            snapshot.tools.clear();
            snapshot.tool_approval_requests.clear();
            snapshot.tool_conflicts.clear();
        }
        if !subagent_discoverable {
            snapshot.subagents.clear();
            snapshot.subagent_conflicts.clear();
            snapshot.pending_subagent_approvals.clear();
        }
        if !mcp_discoverable {
            snapshot.mcp_servers.clear();
            snapshot.mcp_approval_requests.clear();
            snapshot.mcp_conflicts.clear();
        }
        if !command_discoverable
            && !tool_discoverable
            && !subagent_discoverable
            && !mcp_discoverable
        {
            snapshot.sources.clear();
            snapshot.diagnostics.clear();
        }
        policy = integration_policy_snapshot(&preferences, self.workspace_root.as_deref())?;
        snapshot.integration_policy = policy;
        assign_external_source_presentation_groups(&mut snapshot);
        sanitize_external_snapshot_locations(&mut snapshot, self.workspace_root.as_deref());
        let mut current = lock_snapshot(&self.snapshot);
        let mcp_changed = snapshot.mcp_servers != current.mcp_servers
            || snapshot.mcp_conflicts != current.mcp_conflicts
            || snapshot.mcp_approval_requests != current.mcp_approval_requests;
        snapshot.mcp_generation = if mcp_changed {
            snapshot
                .mcp_generation
                .max(current.mcp_generation.saturating_add(1))
        } else {
            snapshot.mcp_generation.max(current.mcp_generation)
        };
        let subagent_changed = snapshot.subagents != current.subagents
            || snapshot.subagent_conflicts != current.subagent_conflicts
            || snapshot.pending_subagent_approvals != current.pending_subagent_approvals
            || snapshot.preference_revision != current.preference_revision;
        snapshot.subagent_generation = if subagent_changed {
            snapshot
                .subagent_generation
                .max(current.subagent_generation.saturating_add(1))
        } else {
            snapshot
                .subagent_generation
                .max(current.subagent_generation)
        };
        snapshot.generation = snapshot
            .generation
            .max(current.generation.saturating_add(1));
        *current = snapshot.clone();
        Ok(snapshot)
    }

    async fn rebuild_read_only_projection(
        &self,
        command_snapshot: ExternalSourceCatalogSnapshot,
        preferences: ExternalSourcesConfig,
        policy: ExternalIntegrationPolicySnapshot,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let tool_active_ecosystems =
            ecosystems_with_active_capability(&policy, EXTERNAL_CAPABILITY_TOOL);
        let subagent_active_ecosystems =
            ecosystems_with_active_capability(&policy, EXTERNAL_CAPABILITY_SUBAGENT);
        let mcp_active_ecosystems =
            ecosystems_with_active_capability(&policy, EXTERNAL_CAPABILITY_MCP);

        let tool_snapshot = lock_tool_coordinator(&self.control_plane).snapshot();
        let tool_state = project_external_tools_read_only(
            self.execution_domain_id.as_str(),
            &tool_snapshot,
            ExternalToolDecisions {
                active_ecosystems: &tool_active_ecosystems,
                approved_targets: &preferences.approved_tool_targets,
                declined_decisions_by_approval: &preferences.declined_tool_decisions,
                conflict_choices: &preferences.tool_conflict_choices,
            },
        );
        let mut snapshot = merge_tool_state(command_snapshot, &tool_snapshot, tool_state);

        let mcp_snapshot = lock_mcp_coordinator(&self.control_plane).snapshot();
        let mcp_workspace_key = workspace_route_key(self.workspace_root.as_deref());
        let mut mcp_state = reconcile_external_mcp_catalog(
            self.execution_domain_id.as_str(),
            &mcp_workspace_key,
            &mcp_snapshot,
            &[],
            ExternalMcpDecisions {
                active_ecosystems: &mcp_active_ecosystems,
                server_decisions: &preferences.mcp_server_decisions,
                conflict_choices: &preferences.mcp_conflict_choices,
            },
        );
        mcp_state.active.clear();
        mcp_state.suppressed_native_server_ids.clear();
        for entry in &mut mcp_state.entries {
            entry.runtime_id = None;
            if matches!(
                entry.activation_state,
                ExternalMcpActivationState::Active | ExternalMcpActivationState::Starting
            ) {
                entry.activation_state = ExternalMcpActivationState::RuntimeUnavailable {
                    reason: "This Host exposes discovery only; use Desktop or an authenticated Peer Host to run external MCP servers".to_string(),
                };
            }
        }
        merge_mcp_state(&mut snapshot, &mcp_snapshot, mcp_state);

        let subagent_snapshot = lock_subagent_coordinator(&self.control_plane).snapshot();
        let subagent_state = project_external_subagents_read_only(
            self.workspace_root.as_deref(),
            self.execution_domain_id.as_str(),
            &subagent_snapshot,
            ExternalSubagentDecisions {
                active_ecosystems: &subagent_active_ecosystems,
                approved_envelopes: &preferences.approved_subagent_envelopes,
                declined_decisions: &preferences.declined_subagent_decisions,
                conflict_choices: &preferences.subagent_conflict_choices,
                conflict_lineage_current_keys: &preferences.subagent_conflict_lineage_current_keys,
            },
        );
        merge_subagent_state(
            &mut snapshot,
            &subagent_snapshot,
            &subagent_state,
            preferences.preference_revision,
        );

        let restricted = PromptCommandAvailability::Restricted {
            reason: "This Host exposes discovery only; run external commands from Desktop or an authenticated Peer Host".to_string(),
            required_capabilities: vec![EXTERNAL_CAPABILITY_COMMAND.to_string()],
        };
        for command in &mut snapshot.commands {
            command.definition.availability = restricted.clone();
        }
        for conflict in &mut snapshot.command_conflicts {
            conflict.selected_candidate_id = None;
            for candidate in &mut conflict.candidates {
                candidate.availability = restricted.clone();
            }
        }
        snapshot.integration_policy = policy;
        assign_external_source_presentation_groups(&mut snapshot);
        sanitize_external_snapshot_locations(&mut snapshot, self.workspace_root.as_deref());
        let mut current = lock_snapshot(&self.snapshot);
        let mcp_changed = snapshot.mcp_servers != current.mcp_servers
            || snapshot.mcp_conflicts != current.mcp_conflicts
            || snapshot.mcp_approval_requests != current.mcp_approval_requests;
        snapshot.mcp_generation = if mcp_changed {
            snapshot
                .mcp_generation
                .max(current.mcp_generation.saturating_add(1))
        } else {
            snapshot.mcp_generation.max(current.mcp_generation)
        };
        let subagent_changed = snapshot.subagents != current.subagents
            || snapshot.subagent_conflicts != current.subagent_conflicts
            || snapshot.pending_subagent_approvals != current.pending_subagent_approvals
            || snapshot.preference_revision != current.preference_revision;
        snapshot.subagent_generation = if subagent_changed {
            snapshot
                .subagent_generation
                .max(current.subagent_generation.saturating_add(1))
        } else {
            snapshot
                .subagent_generation
                .max(current.subagent_generation)
        };
        snapshot.generation = snapshot
            .generation
            .max(current.generation.saturating_add(1));
        *current = snapshot.clone();
        Ok(snapshot)
    }

    async fn reconcile_mcp_runtime(&self, state: &mut ExternalMcpProductState) {
        let desired = state
            .active
            .iter()
            .map(|candidate| (candidate.runtime_id.clone(), candidate.clone()))
            .collect::<BTreeMap<_, _>>();
        let desired_ids = desired.keys().cloned().collect::<BTreeSet<_>>();
        let mut managed = self.active_mcp_runtime_ids.lock().await.clone();
        let workspace_key = workspace_route_key(self.workspace_root.as_deref());
        if let Err(reason) = self
            .mcp_runtime
            .replace_workspace_route(
                &workspace_key,
                desired_ids.clone(),
                state.suppressed_native_server_ids.clone(),
            )
            .await
        {
            state.diagnostics.push(
                ExternalSourceDiagnostic::warning("external_mcp.route_update_failed", reason, None)
                    .with_asset_kind(ExternalSourceAssetKind::Mcp),
            );
        }
        let managed_statuses = join_all(
            desired
                .values()
                .filter(|candidate| managed.contains(&candidate.runtime_id))
                .map(|candidate| async {
                    (
                        candidate.runtime_id.clone(),
                        self.mcp_runtime.status(&candidate.runtime_id).await,
                    )
                }),
        )
        .await
        .into_iter()
        .collect::<BTreeMap<_, _>>();

        for runtime_id in managed
            .difference(&desired_ids)
            .cloned()
            .collect::<Vec<_>>()
        {
            match self.mcp_runtime.retire(&runtime_id).await {
                Ok(()) => {
                    managed.remove(&runtime_id);
                }
                Err(reason) => state.diagnostics.push(
                    ExternalSourceDiagnostic::warning(
                        "external_mcp.retirement_failed",
                        reason,
                        None,
                    )
                    .with_asset_kind(ExternalSourceAssetKind::Mcp),
                ),
            }
        }

        for candidate in desired.values() {
            if managed.contains(&candidate.runtime_id) {
                let status = managed_statuses
                    .get(&candidate.runtime_id)
                    .cloned()
                    .unwrap_or_else(|| {
                        Err("The external MCP server status is unavailable".to_string())
                    });
                // Keep failed registrations managed until the user disables
                // them. Re-installing from a status error would turn a
                // persistent startup failure into an unbounded retry loop.
                apply_external_mcp_runtime_status(state, candidate, status);
                continue;
            }

            let coordinator = Arc::clone(&self.control_plane);
            let server_id = candidate.definition.id.clone();
            let behavior_version = candidate.definition.behavior_version.clone();
            let prepared = tokio::task::spawn_blocking(move || {
                lock_mcp_coordinator(&coordinator)
                    .prepare_server_guarded(&server_id, &behavior_version)
            })
            .await
            .map_err(|_| "The external MCP configuration could not be prepared".to_string())
            .and_then(|result| result.map_err(|error| error.message));

            let activation = match prepared {
                Ok(prepared) => {
                    self.mcp_runtime
                        .install(candidate, prepared, &workspace_key)
                        .await
                }
                Err(reason) => Err(reason),
            };
            match activation {
                Ok(()) => {
                    managed.insert(candidate.runtime_id.clone());
                    apply_external_mcp_runtime_status(
                        state,
                        candidate,
                        self.mcp_runtime.status(&candidate.runtime_id).await,
                    );
                }
                Err(reason) => {
                    // A process may have reached a failed-but-registered state.
                    // Track it so later source changes retire it safely instead
                    // of attempting duplicate installs on every refresh.
                    if self.mcp_runtime.status(&candidate.runtime_id).await.is_ok() {
                        managed.insert(candidate.runtime_id.clone());
                    }
                    mark_external_mcp_runtime_unavailable(state, candidate, reason);
                }
            }
        }

        *self.active_mcp_runtime_ids.lock().await = managed;
    }

    fn schedule_deferred_command_discovery(
        self: &Arc<Self>,
        deferred: DeferredDiscovery<ExternalSourceDiscoveryResult>,
    ) {
        let weak = Arc::downgrade(self);
        let control_plane = Arc::clone(&self.control_plane);
        tokio::spawn(async move {
            let mut deferred = deferred;
            loop {
                let Some((completed, observer)) = control_plane.complete_command(deferred).await
                else {
                    return;
                };
                {
                    let Some(service) = weak.upgrade() else {
                        return;
                    };
                    let _refresh_guard = service.refresh_gate.lock().await;
                    let Some(result) = control_plane.finalize_command(completed).await else {
                        return;
                    };
                    service.complete_deferred_discovery(result).await;
                }
                let Some(observer) = observer else {
                    return;
                };
                let Some(next) = control_plane.resume_abandoned_command(observer).await else {
                    return;
                };
                deferred = next;
            }
        });
    }

    fn schedule_deferred_tool_discovery(
        self: &Arc<Self>,
        deferred: DeferredDiscovery<ExternalToolDiscoveryResult>,
    ) {
        let weak = Arc::downgrade(self);
        let control_plane = Arc::clone(&self.control_plane);
        tokio::spawn(async move {
            let mut deferred = deferred;
            loop {
                let Some((completed, observer)) = control_plane.complete_tool(deferred).await
                else {
                    return;
                };
                {
                    let Some(service) = weak.upgrade() else {
                        return;
                    };
                    let _refresh_guard = service.refresh_gate.lock().await;
                    let Some(result) = control_plane.finalize_tool(completed).await else {
                        return;
                    };
                    service.complete_deferred_tool_discovery(result).await;
                }
                let Some(observer) = observer else {
                    return;
                };
                let Some(next) = control_plane.resume_abandoned_tool(observer).await else {
                    return;
                };
                deferred = next;
            }
        });
    }

    fn schedule_deferred_subagent_discovery(
        self: &Arc<Self>,
        deferred: DeferredDiscovery<ExternalSubagentDiscoveryResult>,
    ) {
        let weak = Arc::downgrade(self);
        let control_plane = Arc::clone(&self.control_plane);
        tokio::spawn(async move {
            let mut deferred = deferred;
            loop {
                let Some((completed, observer)) = control_plane.complete_subagent(deferred).await
                else {
                    return;
                };
                {
                    let Some(service) = weak.upgrade() else {
                        return;
                    };
                    let _refresh_guard = service.refresh_gate.lock().await;
                    let Some(result) = control_plane.finalize_subagent(completed).await else {
                        return;
                    };
                    service.complete_deferred_subagent_discovery(result).await;
                }
                let Some(observer) = observer else {
                    return;
                };
                let Some(next) = control_plane.resume_abandoned_subagent(observer).await else {
                    return;
                };
                deferred = next;
            }
        });
    }

    fn schedule_deferred_mcp_discovery(
        self: &Arc<Self>,
        deferred: DeferredDiscovery<ExternalMcpDiscoveryResult>,
    ) {
        let weak = Arc::downgrade(self);
        let control_plane = Arc::clone(&self.control_plane);
        tokio::spawn(async move {
            let mut deferred = deferred;
            loop {
                let Some((completed, observer)) = control_plane.complete_mcp(deferred).await else {
                    return;
                };
                {
                    let Some(service) = weak.upgrade() else {
                        return;
                    };
                    let _refresh_guard = service.refresh_gate.lock().await;
                    let Some(result) = control_plane.finalize_mcp(completed).await else {
                        return;
                    };
                    service.complete_deferred_mcp_discovery(result).await;
                }
                let Some(observer) = observer else {
                    return;
                };
                let Some(next) = control_plane.resume_abandoned_mcp(observer).await else {
                    return;
                };
                deferred = next;
            }
        });
    }

    async fn complete_deferred_discovery(&self, result: ExternalSourceDiscoveryResult) {
        let provider_id = result.provider_id().clone();
        let Ok(preferences) = read_external_sources_config().await else {
            return;
        };
        let Ok(policy) = integration_policy_snapshot(&preferences, self.workspace_root.as_deref())
        else {
            return;
        };
        let ecosystem_id =
            lock_coordinator(&self.control_plane).ecosystem_for_provider(&provider_id);
        if ecosystem_id.is_none_or(|ecosystem_id| {
            !integration_capability_is_discoverable(
                &policy,
                ecosystem_id.as_str(),
                EXTERNAL_CAPABILITY_COMMAND,
            )
        }) {
            self.ensure_watch_roots(&policy).await;
            return;
        }
        let command_snapshot = lock_coordinator(&self.control_plane).apply_discovery_result(result);
        self.ensure_watch_roots(&policy).await;
        if let Ok(snapshot) = self.rebuild_product_snapshot(command_snapshot).await {
            let _ = self.updates.send(snapshot);
        }
    }

    async fn complete_deferred_tool_discovery(&self, result: ExternalToolDiscoveryResult) {
        let provider_id = result.provider_id().clone();
        let Ok(preferences) = read_external_sources_config().await else {
            return;
        };
        let Ok(policy) = integration_policy_snapshot(&preferences, self.workspace_root.as_deref())
        else {
            return;
        };
        let ecosystem_id =
            lock_tool_coordinator(&self.control_plane).ecosystem_for_provider(&provider_id);
        if ecosystem_id.is_none_or(|ecosystem_id| {
            !integration_capability_is_discoverable(
                &policy,
                ecosystem_id.as_str(),
                EXTERNAL_CAPABILITY_TOOL,
            )
        }) {
            self.ensure_watch_roots(&policy).await;
            return;
        }
        lock_tool_coordinator(&self.control_plane).apply_discovery_result(result);
        self.ensure_watch_roots(&policy).await;
        let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
        if let Ok(snapshot) = self.rebuild_product_snapshot(command_snapshot).await {
            let _ = self.updates.send(snapshot);
        }
    }

    async fn complete_deferred_subagent_discovery(
        self: &Arc<Self>,
        result: ExternalSubagentDiscoveryResult,
    ) {
        let provider_id = result.provider_id().clone();
        let Ok(preferences) = read_external_sources_config().await else {
            return;
        };
        let Ok(policy) = integration_policy_snapshot(&preferences, self.workspace_root.as_deref())
        else {
            return;
        };
        let ecosystem_id =
            lock_subagent_coordinator(&self.control_plane).ecosystem_for_provider(&provider_id);
        if ecosystem_id.is_none_or(|ecosystem_id| {
            !integration_capability_is_discoverable(
                &policy,
                ecosystem_id.as_str(),
                EXTERNAL_CAPABILITY_SUBAGENT,
            )
        }) {
            self.ensure_watch_roots(&policy).await;
            return;
        }
        let subagent_snapshot =
            lock_subagent_coordinator(&self.control_plane).apply_discovery_result(result);
        self.schedule_subagent_last_valid_expiry(&subagent_snapshot);
        self.ensure_watch_roots(&policy).await;
        let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
        if let Ok(snapshot) = self.rebuild_product_snapshot(command_snapshot).await {
            let _ = self.updates.send(snapshot);
        }
    }

    async fn complete_deferred_mcp_discovery(&self, result: ExternalMcpDiscoveryResult) {
        let provider_id = result.provider_id().clone();
        let Ok(preferences) = read_external_sources_config().await else {
            return;
        };
        let Ok(policy) = integration_policy_snapshot(&preferences, self.workspace_root.as_deref())
        else {
            return;
        };
        let ecosystem_id =
            lock_mcp_coordinator(&self.control_plane).ecosystem_for_provider(&provider_id);
        if ecosystem_id.is_none_or(|ecosystem_id| {
            !integration_capability_is_discoverable(
                &policy,
                ecosystem_id.as_str(),
                EXTERNAL_CAPABILITY_MCP,
            )
        }) {
            self.ensure_watch_roots(&policy).await;
            return;
        }
        lock_mcp_coordinator(&self.control_plane).apply_discovery_result(result);
        self.ensure_watch_roots(&policy).await;
        let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
        if let Ok(snapshot) = self.rebuild_product_snapshot(command_snapshot).await {
            let _ = self.updates.send(snapshot);
        }
    }

    fn schedule_subagent_last_valid_expiry(
        self: &Arc<Self>,
        snapshot: &bitfun_external_sources::ExternalSubagentCoordinatorSnapshot,
    ) {
        let schedule = self
            .subagent_expiry_schedule
            .fetch_add(1, Ordering::AcqRel)
            .saturating_add(1);
        let Some(deadline) = snapshot.next_refresh_deadline else {
            return;
        };
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
            let Some(service) = weak.upgrade() else {
                return;
            };
            if service.subagent_expiry_schedule.load(Ordering::Acquire) != schedule {
                return;
            }
            let _refresh_guard = service.refresh_gate.lock().await;
            lock_subagent_coordinator(&service.control_plane).expire_last_valid();
            let command_snapshot = lock_coordinator(&service.control_plane).snapshot();
            if let Ok(snapshot) = service.rebuild_product_snapshot(command_snapshot).await {
                let _ = service.updates.send(snapshot);
            }
        });
    }

    fn ensure_background_refresh(self: &Arc<Self>) {
        if self.initial_refresh_completed.load(Ordering::Acquire)
            || self
                .background_refresh_scheduled
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return;
        }
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            let Some(service) = weak.upgrade() else {
                return;
            };
            if let Err(error) = service.ensure_initial_refresh().await {
                log::warn!(
                    "Initial external source refresh failed scope={} error_category={}",
                    external_log_scope(service.workspace_root.as_deref()),
                    external_log_error_category(&error),
                );
            }
            service
                .background_refresh_scheduled
                .store(false, Ordering::Release);
        });
    }

    fn touch(&self) {
        self.last_access_epoch_seconds
            .store(epoch_seconds(), Ordering::Release);
    }

    fn ensure_idle_keepalive(self: &Arc<Self>) {
        if self
            .keepalive_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let service = Arc::clone(self);
        tokio::spawn(async move {
            const IDLE_SECONDS: u64 = 300;
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                let idle_for = epoch_seconds()
                    .saturating_sub(service.last_access_epoch_seconds.load(Ordering::Acquire));
                // The keepalive itself and this task account for one strong
                // service reference. A subscription or in-flight operation
                // keeps the service alive independently of idle time.
                if idle_for < IDLE_SECONDS || Arc::strong_count(&service) > 1 {
                    continue;
                }
                let _service_gate = workspace_service_gate().lock().await;
                let idle_for = epoch_seconds()
                    .saturating_sub(service.last_access_epoch_seconds.load(Ordering::Acquire));
                if idle_for < IDLE_SECONDS || Arc::strong_count(&service) > 1 {
                    continue;
                }
                let _rebuild_guard = service.product_rebuild_gate.lock().await;
                if Arc::strong_count(&service) > 1 {
                    continue;
                }
                let key = service.workspace_root.clone();
                let services = workspace_services_for_profile(service.profile);
                if let Some(entry) = services.get(&key) {
                    let should_remove = entry
                        .value()
                        .upgrade()
                        .is_some_and(|cached| Arc::ptr_eq(&cached, &service));
                    drop(entry);
                    if should_remove {
                        if service.profile == ExternalSourceServiceProfile::LocalExecution {
                            let runtime_ids =
                                std::mem::take(&mut *service.active_mcp_runtime_ids.lock().await);
                            let workspace_key = workspace_route_key(key.as_deref());
                            let _ = service
                                .mcp_runtime
                                .replace_workspace_route(
                                    &workspace_key,
                                    BTreeSet::new(),
                                    BTreeSet::new(),
                                )
                                .await;
                            for runtime_id in runtime_ids {
                                if let Err(error) = service.mcp_runtime.retire(&runtime_id).await {
                                    log::warn!(
                                        "Could not retire idle external MCP runtime runtime_id={} error_category={}",
                                        safe_external_log_token(&runtime_id),
                                        external_log_error_category(&error.to_string()),
                                    );
                                }
                            }
                        }
                        services.remove(&key);
                        if service.profile == ExternalSourceServiceProfile::LocalExecution {
                            release_external_tool_workspace(key.as_deref()).await;
                            if let Some(workspace_root) = key.as_deref() {
                                crate::agentic::agents::get_agent_registry()
                                    .release_external_subagent_workspace(workspace_root);
                            }
                        }
                    }
                }
                break;
            }
        });
    }

    fn snapshot(&self) -> ExternalSourceCatalogSnapshot {
        lock_snapshot(&self.snapshot).clone()
    }

    fn source_location(&self, stable_key: &str) -> Result<PathBuf, String> {
        let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
        let tool_snapshot = lock_tool_coordinator(&self.control_plane).snapshot();
        let subagent_snapshot = lock_subagent_coordinator(&self.control_plane).snapshot();
        let mcp_snapshot = lock_mcp_coordinator(&self.control_plane).snapshot();
        resolve_external_source_location(
            stable_key,
            command_snapshot
                .sources
                .iter()
                .chain(tool_snapshot.sources.iter())
                .chain(subagent_snapshot.sources.iter())
                .chain(mcp_snapshot.sources.iter()),
        )
    }

    fn surface_snapshot(
        &self,
        host_capabilities: ExternalSourceHostCapabilities,
    ) -> ExternalSourceSurfaceSnapshotV1 {
        let catalog = self.snapshot();
        let safe_mode = self.safe_mode_enabled();
        let control = ExternalSourceControlSnapshotV1::from_catalog(
            &catalog,
            self.execution_domain_id.clone(),
            safe_mode,
            host_capabilities.clone(),
        );
        let mut public = ExternalSourcePublicSnapshot::from(catalog);
        public.host_capabilities = host_capabilities;
        ExternalSourceSurfaceSnapshotV1 {
            control,
            catalog: public,
        }
    }

    fn safe_mode_enabled(&self) -> bool {
        external_source_safe_mode_enabled_for(
            self.execution_domain_id.as_str(),
            &workspace_route_key(self.workspace_root.as_deref()),
        )
    }

    fn write_safe_mode(&self, enabled: bool) {
        set_external_source_safe_mode_for(
            self.execution_domain_id.as_str(),
            &workspace_route_key(self.workspace_root.as_deref()),
            enabled,
        );
    }

    async fn set_safe_mode(
        &self,
        enabled: bool,
        expected_preference_revision: Option<u64>,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let _refresh_guard = self.refresh_gate.lock().await;
        let previous = self.safe_mode_enabled();
        if let Some(expected_revision) = expected_preference_revision {
            let (accepted, _) = ExternalSourcePreferenceStore::global()?
                .update(|config| {
                    if config.preference_revision != expected_revision {
                        return false;
                    }
                    // Safe Mode is process-local, but its CAS linearization
                    // point must share the authoritative preference file lock
                    // with every persisted source/review mutation.
                    self.write_safe_mode(enabled);
                    true
                })
                .await?;
            if !accepted {
                return Err(stale_operation_error(
                    "External source preferences changed; refresh before changing Safe Mode",
                ));
            }
            if let Err(error) = sync_service_preferences(self).await {
                self.write_safe_mode(previous);
                return Err(error);
            }
        } else {
            self.write_safe_mode(enabled);
        }
        if previous == enabled {
            return Ok(self.snapshot());
        }

        let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
        match self.rebuild_product_snapshot(command_snapshot).await {
            Ok(snapshot) => {
                let _ = self.updates.send(snapshot.clone());
                Ok(snapshot)
            }
            Err(error) => {
                self.write_safe_mode(previous);
                let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
                if let Err(rollback_error) = self.rebuild_product_snapshot(command_snapshot).await {
                    log::error!(
                        "External source Safe Mode rollback failed execution_domain={} reason={}",
                        safe_external_log_token(self.execution_domain_id.as_str()),
                        safe_external_log_token(&rollback_error),
                    );
                }
                Err(error)
            }
        }
    }

    async fn apply_control_action(
        self: &Arc<Self>,
        request: ExternalSourceControlRequestV1,
    ) -> ExternalSourceOperationResult<ExternalSourceSurfaceSnapshotV1> {
        use bitfun_product_domains::external_source_control::ExternalSourceOperationStage;

        if let Err(detail) = request.validate() {
            return Err(ExternalSourceOperationError::invalid_request(detail)
                .with_correlation_id(request.operation_id)
                .with_stage(ExternalSourceOperationStage::ValidateRequest));
        }

        let started = std::time::Instant::now();
        let operation_id = request.operation_id;
        let expected_revision = request.expected_preference_revision;
        let (action, stage, result) = match request.action {
            ExternalSourceControlActionV1::Refresh => (
                "refresh",
                ExternalSourceOperationStage::Discover,
                self.refresh().await,
            ),
            ExternalSourceControlActionV1::SetSourceEnabled {
                source_key,
                enabled,
            } => {
                let stage = ExternalSourceOperationStage::ApplyPreference;
                let result = match expected_revision {
                    Some(revision) => {
                        self.set_source_enabled(&source_key, enabled, revision)
                            .await
                    }
                    None => Err(invalid_operation_error(
                        "External source enablement requires an expected preference revision",
                    )),
                };
                ("set_source_enabled", stage, result)
            }
            ExternalSourceControlActionV1::SetSafeMode { enabled } => {
                let stage = ExternalSourceOperationStage::ApplyPreference;
                let result = match expected_revision {
                    Some(revision) => self.set_safe_mode(enabled, Some(revision)).await,
                    None => Err(invalid_operation_error(
                        "Safe Mode requires an expected preference revision",
                    )),
                };
                ("set_safe_mode", stage, result)
            }
        };

        match result {
            Ok(snapshot) => {
                log::info!(
                    "External source control action outcome=success action={} correlation_id={} duration_ms={} generation={}",
                    action,
                    safe_external_log_token(&operation_id),
                    started.elapsed().as_millis(),
                    snapshot.generation,
                );
                Ok(self.surface_snapshot(ExternalSourceHostCapabilities::read_write()))
            }
            Err(error) => {
                let typed = typed_control_operation_error(error, &operation_id, stage);
                log::warn!(
                    "External source control action outcome=failure action={} correlation_id={} duration_ms={} error_code={}",
                    action,
                    safe_external_log_token(&operation_id),
                    started.elapsed().as_millis(),
                    typed.code.as_str(),
                );
                Err(typed)
            }
        }
    }

    async fn set_source_enabled(
        self: &Arc<Self>,
        stable_key: &str,
        enabled: bool,
        expected_preference_revision: u64,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let refresh_guard = self.refresh_gate.lock().await;
        if self.snapshot().preference_revision != expected_preference_revision {
            return Err(stale_operation_error(
                "External source preferences changed; refresh before retrying",
            ));
        }
        let (previous_commands, command_known) = {
            let mut coordinator = lock_coordinator(&self.control_plane);
            let previous = coordinator.suppressed_sources().clone();
            let known = coordinator.set_source_enabled(stable_key, enabled).is_ok();
            (previous, known)
        };
        let (previous_tools, tool_known) = {
            let mut coordinator = lock_tool_coordinator(&self.control_plane);
            let previous = coordinator.suppressed_sources().clone();
            let known = coordinator.set_source_enabled(stable_key, enabled).is_ok();
            (previous, known)
        };
        let (previous_subagents, subagent_known) = {
            let mut coordinator = lock_subagent_coordinator(&self.control_plane);
            let previous = coordinator.suppressed_sources().clone();
            let known = coordinator.set_source_enabled(stable_key, enabled).is_ok();
            (previous, known)
        };
        let (previous_mcps, mcp_known) = {
            let mut coordinator = lock_mcp_coordinator(&self.control_plane);
            let previous = coordinator.suppressed_sources().clone();
            let known = coordinator.set_source_enabled(stable_key, enabled).is_ok();
            (previous, known)
        };
        if !command_known && !tool_known && !subagent_known && !mcp_known {
            return Err(missing_candidate_error(format!(
                "External source '{stable_key}' is no longer available"
            )));
        }
        let authoritative =
            match persist_source_enabled_change(stable_key, enabled, expected_preference_revision)
                .await
            {
                Ok(authoritative) => authoritative,
                Err(error) => {
                    lock_coordinator(&self.control_plane)
                        .replace_suppressed_sources(previous_commands);
                    lock_tool_coordinator(&self.control_plane)
                        .replace_suppressed_sources(previous_tools);
                    lock_subagent_coordinator(&self.control_plane)
                        .replace_suppressed_sources(previous_subagents);
                    lock_mcp_coordinator(&self.control_plane)
                        .replace_suppressed_sources(previous_mcps);
                    return Err(error);
                }
            };
        lock_coordinator(&self.control_plane).replace_suppressed_sources(authoritative.clone());
        lock_tool_coordinator(&self.control_plane)
            .replace_suppressed_sources(authoritative.clone());
        lock_subagent_coordinator(&self.control_plane)
            .replace_suppressed_sources(authoritative.clone());
        lock_mcp_coordinator(&self.control_plane).replace_suppressed_sources(authoritative.clone());
        propagate_suppressed_sources(&authoritative, self);
        // Refresh acquires the same gate. Release the mutation critical section
        // after the preference and in-memory coordinators agree, then refresh
        // from the authoritative store to avoid self-deadlocking the request.
        drop(refresh_guard);
        self.refresh_preserving_worker_recovery().await
    }

    async fn update_integration_policy(
        self: &Arc<Self>,
        mutation: ExternalIntegrationPolicyMutation,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let preferences =
            persist_integration_policy_mutation(self.workspace_root.as_deref(), mutation).await?;
        propagate_integration_policy_preferences(&preferences, self);
        self.refresh_preserving_worker_recovery().await
    }

    async fn set_conflict_choice(
        &self,
        conflict_key: &str,
        candidate_id: &str,
        expected_preference_revision: u64,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let _refresh_guard = self.refresh_gate.lock().await;
        let product_snapshot = self.snapshot();
        if product_snapshot.preference_revision != expected_preference_revision {
            return Err(stale_operation_error(
                "External command preferences changed; refresh before retrying",
            ));
        }
        let selected_candidate = product_snapshot
            .command_conflicts
            .iter()
            .find(|conflict| conflict.conflict_key == conflict_key)
            .and_then(|conflict| {
                conflict
                    .candidates
                    .iter()
                    .find(|candidate| candidate.candidate_id == candidate_id)
            })
            .ok_or_else(|| {
                missing_candidate_error(format!(
                    "External source conflict '{conflict_key}' is no longer available"
                ))
            })?;
        if !integration_capability_is_active(
            &product_snapshot.integration_policy,
            selected_candidate.ecosystem_id.as_str(),
            EXTERNAL_CAPABILITY_COMMAND,
        ) {
            return Err(policy_limited_error(
                "The selected external command ecosystem is not enabled for this workspace",
            ));
        }
        let (previous_choices, previous_lineage_keys, previous_conflicted_ids, participants) = {
            let mut coordinator = lock_coordinator(&self.control_plane);
            let participants = coordinator
                .snapshot()
                .command_conflicts
                .into_iter()
                .find(|conflict| conflict.conflict_key == conflict_key)
                .map(|conflict| {
                    conflict
                        .candidates
                        .into_iter()
                        .map(|candidate| candidate.candidate_id)
                        .collect::<Vec<_>>()
                })
                .ok_or_else(|| {
                    missing_candidate_error(format!(
                        "External source conflict '{conflict_key}' is no longer available"
                    ))
                })?;
            let previous_choices = coordinator.conflict_choices().clone();
            let previous_lineage_keys = coordinator.conflict_lineage_current_keys().clone();
            let previous_conflicted_ids = coordinator.conflicted_candidate_ids().clone();
            coordinator.set_conflict_choice(conflict_key, candidate_id)?;
            (
                previous_choices,
                previous_lineage_keys,
                previous_conflicted_ids,
                participants,
            )
        };
        let (updated_choices, updated_lineage_keys, updated_conflicted_ids) = {
            let coordinator = lock_coordinator(&self.control_plane);
            (
                coordinator.conflict_choices().clone(),
                coordinator.conflict_lineage_current_keys().clone(),
                coordinator.conflicted_candidate_ids().clone(),
            )
        };
        let authoritative = match persist_conflict_choice(
            conflict_key,
            candidate_id,
            participants,
            expected_preference_revision,
        )
        .await
        {
            Ok(authoritative) => authoritative,
            Err(error) => {
                let mut coordinator = lock_coordinator(&self.control_plane);
                coordinator.replace_conflict_choices(previous_choices);
                coordinator.replace_conflict_lineage_current_keys(previous_lineage_keys);
                coordinator.replace_conflicted_candidate_ids(previous_conflicted_ids);
                return Err(error);
            }
        };
        if authoritative.conflict_choices != updated_choices
            || authoritative.conflict_lineage_current_keys != updated_lineage_keys
            || authoritative.conflicted_candidate_ids != updated_conflicted_ids
        {
            log::debug!("External source conflict preferences changed in another workspace");
        }
        propagate_conflict_preferences(&authoritative);
        let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
        self.rebuild_product_snapshot(command_snapshot).await
    }

    async fn set_tool_target_decision(
        &self,
        approval_key: &str,
        decision_key: &str,
        approved: bool,
        expected_preference_revision: u64,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        // Keep preview validation, preference persistence and the resulting
        // product rebuild in the same ordering domain as watcher refreshes.
        // Otherwise an approval for content v1 could be persisted after a
        // refresh installs v2 with the same capability-based approval key.
        #[cfg(test)]
        self.tool_decision_gate_waiting.notify_one();
        let _refresh_guard = self.refresh_gate.lock().await;
        #[cfg(test)]
        self.tool_decision_gate_acquired.notify_one();
        let snapshot = self.snapshot();
        if snapshot.preference_revision != expected_preference_revision {
            return Err(stale_operation_error(
                "External tool preferences changed; refresh before retrying",
            ));
        }
        let source_key = snapshot
            .tool_approval_requests
            .iter()
            .find(|request| {
                request.approval_key == approval_key && request.decision_key == decision_key
            })
            .map(|request| request.target_id.source.clone())
            .or_else(|| {
                snapshot
                    .tools
                    .iter()
                    .find(|tool| {
                        tool.approval_key == approval_key && tool.decision_key == decision_key
                    })
                    .map(|tool| tool.definition.id.target.source.clone())
            })
            .ok_or_else(|| {
                missing_candidate_error("External tool decision is stale or no longer available")
            })?;
        if approved {
            ensure_source_capability_active(&snapshot, &source_key, EXTERNAL_CAPABILITY_TOOL)?;
        }
        validate_conflict_preference(approval_key, decision_key)?;
        let preferences = persist_tool_target_decision(
            approval_key,
            decision_key,
            approved,
            expected_preference_revision,
        )
        .await?;
        propagate_tool_preferences(&preferences);
        let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
        self.rebuild_product_snapshot(command_snapshot).await
    }

    async fn set_tool_conflict_choice(
        &self,
        conflict_key: &str,
        candidate_id: &str,
        expected_preference_revision: u64,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let _refresh_guard = self.refresh_gate.lock().await;
        let snapshot = self.snapshot();
        if snapshot.preference_revision != expected_preference_revision {
            return Err(stale_operation_error(
                "External tool preferences changed; refresh before retrying",
            ));
        }
        let candidate = snapshot
            .tool_conflicts
            .iter()
            .find(|conflict| conflict.conflict_key == conflict_key)
            .and_then(|conflict| {
                conflict
                    .candidates
                    .iter()
                    .find(|candidate| candidate.candidate_id == candidate_id)
            })
            .ok_or_else(|| {
                missing_candidate_error(
                    "External tool conflict choice is stale or no longer available",
                )
            })?;
        if matches!(candidate.kind, ExternalToolConflictCandidateKind::External) {
            let source_key = candidate.source.as_ref().ok_or_else(|| {
                missing_candidate_error("External tool conflict source is missing")
            })?;
            ensure_source_capability_active(&snapshot, source_key, EXTERNAL_CAPABILITY_TOOL)?;
        }
        validate_conflict_preference(conflict_key, candidate_id)?;
        let preferences =
            persist_tool_conflict_choice(conflict_key, candidate_id, expected_preference_revision)
                .await?;
        propagate_tool_preferences(&preferences);
        let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
        self.rebuild_product_snapshot(command_snapshot).await
    }

    async fn set_mcp_server_decision(
        &self,
        candidate_id: &str,
        decision_key: &str,
        approved: bool,
        expected_mcp_generation: u64,
        expected_preference_revision: u64,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let _refresh_guard = self.refresh_gate.lock().await;
        let snapshot = self.snapshot();
        if snapshot.mcp_generation != expected_mcp_generation
            || snapshot.preference_revision != expected_preference_revision
        {
            return Err(stale_operation_error(
                "External MCP catalog changed; refresh before retrying",
            ));
        }
        let entry = snapshot
            .mcp_servers
            .iter()
            .find(|entry| entry.candidate_id == candidate_id && entry.decision_key == decision_key)
            .ok_or_else(|| {
                missing_candidate_error("External MCP candidate is no longer available")
            })?;
        if approved {
            ensure_source_capability_active(
                &snapshot,
                &entry.definition.id.source,
                EXTERNAL_CAPABILITY_MCP,
            )?;
        }
        if !external_mcp_decision_allowed(&entry.activation_state, approved) {
            return Err(unavailable_operation_error(
                "External MCP candidate cannot be changed in its current state",
            ));
        }
        validate_mcp_decision_value(candidate_id, "candidate id")?;
        validate_mcp_decision_value(decision_key, "decision key")?;
        let preferences =
            persist_mcp_server_decision(decision_key, approved, expected_preference_revision)
                .await?;
        propagate_mcp_preferences(&preferences);
        let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
        self.rebuild_product_snapshot(command_snapshot).await
    }

    async fn choose_mcp_conflict(
        &self,
        conflict_key: &str,
        candidate_id: &str,
        approve_external: bool,
        expected_mcp_generation: u64,
        expected_preference_revision: u64,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let _refresh_guard = self.refresh_gate.lock().await;
        let snapshot = self.snapshot();
        if snapshot.mcp_generation != expected_mcp_generation
            || snapshot.preference_revision != expected_preference_revision
        {
            return Err(stale_operation_error(
                "External MCP catalog changed; refresh before retrying",
            ));
        }
        let conflict = snapshot
            .mcp_conflicts
            .iter()
            .find(|conflict| conflict.conflict_key == conflict_key)
            .ok_or_else(|| {
                conflict_operation_error("External MCP conflict is stale or no longer available")
            })?;
        let candidate = conflict
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == candidate_id && candidate.available)
            .ok_or_else(|| {
                missing_candidate_error("External MCP conflict candidate is unavailable")
            })?;
        let external_decision = if candidate.external {
            let source_key = candidate.source.as_ref().ok_or_else(|| {
                missing_candidate_error("External MCP conflict source is missing")
            })?;
            ensure_source_capability_active(&snapshot, source_key, EXTERNAL_CAPABILITY_MCP)?;
            if !approve_external {
                return Err(policy_limited_error(
                    "Selecting an external MCP server requires approval of its current behavior",
                ));
            }
            let entry = snapshot
                .mcp_servers
                .iter()
                .find(|entry| entry.candidate_id == candidate_id)
                .ok_or_else(|| {
                    missing_candidate_error("External MCP candidate is no longer available")
                })?;
            Some(entry.decision_key.as_str())
        } else {
            None
        };
        validate_mcp_decision_value(conflict_key, "conflict key")?;
        validate_mcp_decision_value(candidate_id, "candidate id")?;
        let preferences = persist_mcp_conflict_choice(
            conflict_key,
            candidate_id,
            external_decision,
            expected_preference_revision,
        )
        .await?;
        propagate_mcp_preferences(&preferences);
        let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
        self.rebuild_product_snapshot(command_snapshot).await
    }

    async fn set_subagent_activation(
        &self,
        candidate_id: &str,
        approved: bool,
        expected_subagent_generation: u64,
        expected_preference_revision: u64,
        decision_key: &str,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let _refresh_guard = self.refresh_gate.lock().await;
        let snapshot = self.snapshot();
        if snapshot.subagent_generation != expected_subagent_generation
            || snapshot.preference_revision != expected_preference_revision
        {
            return Err(stale_operation_error(
                "External subagent catalog changed; refresh before retrying",
            ));
        }
        let summary = snapshot
            .subagents
            .iter()
            .find(|summary| {
                summary.candidate_id == candidate_id && summary.decision_key == decision_key
            })
            .ok_or_else(|| {
                missing_candidate_error("External subagent candidate is no longer available")
            })?;
        if approved {
            ensure_source_set_capability_active(
                &snapshot,
                &summary.source_keys,
                EXTERNAL_CAPABILITY_SUBAGENT,
            )?;
        }
        if matches!(
            summary.activation_state,
            ExternalSubagentActivationState::Blocked
                | ExternalSubagentActivationState::Unavailable
                | ExternalSubagentActivationState::Conflict
        ) {
            return Err(unavailable_operation_error(
                "External subagent cannot be activated in its current state",
            ));
        }
        validate_subagent_decision_value(candidate_id, "candidate id")?;
        validate_subagent_decision_value(decision_key, "decision key")?;
        let preferences =
            persist_subagent_activation(decision_key, approved, expected_preference_revision)
                .await?;
        propagate_subagent_preferences(&preferences);
        let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
        self.rebuild_product_snapshot(command_snapshot).await
    }

    async fn choose_subagent_conflict(
        &self,
        conflict_key: &str,
        candidate_id: &str,
        approve_external: bool,
        expected_subagent_generation: u64,
        expected_preference_revision: u64,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let _refresh_guard = self.refresh_gate.lock().await;
        let snapshot = self.snapshot();
        if snapshot.subagent_generation != expected_subagent_generation
            || snapshot.preference_revision != expected_preference_revision
        {
            return Err(stale_operation_error(
                "External subagent catalog changed; refresh before retrying",
            ));
        }
        let conflict = snapshot
            .subagent_conflicts
            .iter()
            .find(|conflict| conflict.conflict_key == conflict_key)
            .ok_or_else(|| {
                conflict_operation_error(
                    "External subagent conflict is stale or no longer available",
                )
            })?;
        let external = if candidate_id == DISABLED_SUBAGENT_CONFLICT_CHOICE {
            false
        } else {
            conflict
                .candidates
                .iter()
                .find(|candidate| candidate.candidate_id == candidate_id)
                .map(|candidate| candidate.external)
                .ok_or_else(|| {
                    missing_candidate_error("Conflict candidate is no longer available")
                })?
        };
        let approval_key = if external {
            let summary = snapshot
                .subagents
                .iter()
                .find(|summary| summary.candidate_id == candidate_id)
                .ok_or_else(|| {
                    missing_candidate_error("External subagent candidate is no longer available")
                })?;
            ensure_source_set_capability_active(
                &snapshot,
                &summary.source_keys,
                EXTERNAL_CAPABILITY_SUBAGENT,
            )?;
            if !approve_external {
                return Err(policy_limited_error(
                    "Selecting an external subagent requires approval of its current capability envelope",
                ));
            }
            Some(summary.decision_key.clone())
        } else {
            None
        };
        validate_subagent_decision_value(conflict_key, "conflict key")?;
        validate_subagent_decision_value(candidate_id, "candidate id")?;
        let preferences = persist_subagent_conflict_choice(
            conflict_key,
            candidate_id,
            approval_key.as_deref(),
            expected_preference_revision,
        )
        .await?;
        propagate_subagent_preferences(&preferences);
        let command_snapshot = lock_coordinator(&self.control_plane).snapshot();
        self.rebuild_product_snapshot(command_snapshot).await
    }

    async fn expand_command(
        self: &Arc<Self>,
        name: &str,
        arguments: &str,
        expected_candidate_id: Option<&str>,
        expected_content_version: Option<&str>,
    ) -> Result<ExpandedPromptCommand, String> {
        // Explicit invocation refreshes first, so a stable deletion cannot be
        // bypassed by an old menu projection.
        let snapshot = self.refresh_preserving_worker_recovery().await?;
        let source_key = snapshot
            .commands
            .iter()
            .find(|entry| entry.definition.name.eq_ignore_ascii_case(name))
            .map(|entry| entry.definition.id.source.clone())
            .ok_or_else(|| {
                missing_candidate_error(format!("External prompt command '{name}' was not found"))
            })?;
        ensure_source_capability_active(&snapshot, &source_key, EXTERNAL_CAPABILITY_COMMAND)?;
        let coordinator = Arc::clone(&self.control_plane);
        let name = name.to_string();
        let arguments = arguments.to_string();
        let expected_candidate_id = expected_candidate_id.map(str::to_string);
        let expected_content_version = expected_content_version.map(str::to_string);
        tokio::task::spawn_blocking(move || {
            lock_coordinator(&coordinator)
                .expand_command_guarded(
                    &name,
                    &arguments,
                    expected_candidate_id.as_deref(),
                    expected_content_version.as_deref(),
                )
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| format!("external command expansion task failed: {error}"))?
    }

    async fn start_watching(self: &Arc<Self>) {
        let policy = self.snapshot().integration_policy;
        let watch_roots = self.watch_roots(&policy);
        if watch_roots.is_empty() {
            return;
        }
        self.ensure_watch_roots(&policy).await;
        let mut receiver = self.watcher.subscribe();
        let weak: Weak<Self> = Arc::downgrade(self);
        tokio::spawn(async move {
            loop {
                let events = match receiver.recv().await {
                    Ok(events) => events,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        if let Some(service) = weak.upgrade() {
                            let _ = service.refresh().await;
                            continue;
                        }
                        break;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                let Some(service) = weak.upgrade() else {
                    break;
                };
                let policy = service.snapshot().integration_policy;
                let watch_roots = service.watch_roots(&policy);
                let relevant = events.iter().any(|event| {
                    let path = Path::new(&event.path);
                    watch_roots.iter().any(|root| path.starts_with(&root.path))
                });
                if !relevant {
                    continue;
                }
                if let Err(error) = service.refresh().await {
                    log::warn!(
                        "External source background refresh failed scope={} error_category={}",
                        external_log_scope(service.workspace_root.as_deref()),
                        external_log_error_category(&error),
                    );
                }
            }
        });
    }

    fn start_model_config_watching(self: &Arc<Self>) {
        let Some(mut receiver) = subscribe_config_updates() else {
            return;
        };
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            loop {
                let should_refresh = match receiver.recv().await {
                    Ok(event) => config_update_refreshes_external_model_bindings(&event),
                    Err(broadcast::error::RecvError::Lagged(_)) => true,
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if !should_refresh {
                    continue;
                }
                let Some(service) = weak.upgrade() else {
                    break;
                };
                let command_snapshot = lock_coordinator(&service.control_plane).snapshot();
                match service.rebuild_product_snapshot(command_snapshot).await {
                    Ok(snapshot) => {
                        let _ = service.updates.send(snapshot);
                    }
                    Err(error) => log::warn!(
                        "External source model-binding refresh failed scope={} error_category={}",
                        external_log_scope(service.workspace_root.as_deref()),
                        external_log_error_category(&error),
                    ),
                }
            }
        });
    }

    async fn ensure_watch_roots(&self, policy: &ExternalIntegrationPolicySnapshot) {
        let watch_roots = self.watch_roots(policy);
        let watcher = Arc::clone(&self.watcher);
        let mut states = self.watch_states.lock().await;
        let desired = watch_roots
            .iter()
            .map(|root| (root.path.clone(), root.recursive))
            .collect::<BTreeSet<_>>();
        let obsolete = states
            .keys()
            .filter(|key| !desired.contains(*key))
            .cloned()
            .collect::<Vec<_>>();
        for key in obsolete {
            if states.get(&key).copied().unwrap_or(false) {
                let path = key.0.to_string_lossy().to_string();
                if let Err(error) = watcher.unwatch_path(&path).await {
                    log::warn!(
                        "Failed to stop watching external source root scope={} recursive={} error_category={}",
                        external_log_scope(self.workspace_root.as_deref()),
                        key.1,
                        external_log_error_category(&error.to_string()),
                    );
                }
            }
            states.remove(&key);
        }
        for root in watch_roots {
            let key = (root.path.clone(), root.recursive);
            let exists = root.path.exists();
            let was_available = states.get(&key).copied().unwrap_or(false);
            if !exists {
                states.insert(key, false);
                continue;
            }
            if was_available {
                continue;
            }
            let mut config = FileWatcherConfig::default();
            config.watch_recursively = root.recursive;
            config.ignore_hidden_files = false;
            config.debounce_interval_ms = 350;
            let path = root.path.to_string_lossy().to_string();
            match watcher.watch_path(&path, Some(config)).await {
                Ok(()) => {
                    states.insert(key, true);
                }
                Err(error) => {
                    states.insert(key, false);
                    log::warn!(
                        "Failed to watch external source root scope={} recursive={} error_category={}",
                        external_log_scope(self.workspace_root.as_deref()),
                        root.recursive,
                        external_log_error_category(&error.to_string()),
                    );
                }
            }
        }
    }

    fn watch_roots(
        &self,
        policy: &ExternalIntegrationPolicySnapshot,
    ) -> Vec<bitfun_product_domains::external_sources::ExternalWatchRoot> {
        let mut roots = BTreeMap::new();
        let mut provider_roots = Vec::new();
        let command_ecosystems =
            ecosystems_with_discoverable_capability(policy, EXTERNAL_CAPABILITY_COMMAND);
        provider_roots.extend(
            lock_coordinator(&self.control_plane).watch_roots_for_ecosystems(&command_ecosystems),
        );
        let tool_ecosystems =
            ecosystems_with_discoverable_capability(policy, EXTERNAL_CAPABILITY_TOOL);
        provider_roots.extend(
            lock_tool_coordinator(&self.control_plane).watch_roots_for_ecosystems(&tool_ecosystems),
        );
        let subagent_ecosystems =
            ecosystems_with_discoverable_capability(policy, EXTERNAL_CAPABILITY_SUBAGENT);
        provider_roots.extend(
            lock_subagent_coordinator(&self.control_plane)
                .watch_roots_for_ecosystems(&subagent_ecosystems),
        );
        let mcp_ecosystems =
            ecosystems_with_discoverable_capability(policy, EXTERNAL_CAPABILITY_MCP);
        provider_roots.extend(
            lock_mcp_coordinator(&self.control_plane).watch_roots_for_ecosystems(&mcp_ecosystems),
        );
        for root in provider_roots {
            roots
                .entry(root.path)
                .and_modify(|recursive| *recursive |= root.recursive)
                .or_insert(root.recursive);
        }
        if let Ok(store) = ExternalSourcePreferenceStore::global() {
            if let Some(parent) = store.path.parent() {
                roots.entry(parent.to_path_buf()).or_insert(false);
            }
        }
        roots
            .into_iter()
            .map(
                |(path, recursive)| bitfun_product_domains::external_sources::ExternalWatchRoot {
                    path,
                    recursive,
                },
            )
            .collect()
    }
}

fn lock_coordinator(
    control_plane: &ExternalSourceControlPlane,
) -> MutexGuard<'_, bitfun_external_sources::ExternalSourceCoordinator> {
    control_plane.lock_commands()
}

fn lock_tool_coordinator(
    control_plane: &ExternalSourceControlPlane,
) -> MutexGuard<'_, bitfun_external_sources::ExternalToolCoordinator> {
    control_plane.lock_tools()
}

fn lock_subagent_coordinator(
    control_plane: &ExternalSourceControlPlane,
) -> MutexGuard<'_, bitfun_external_sources::ExternalSubagentCoordinator> {
    control_plane.lock_subagents()
}

fn lock_mcp_coordinator(
    control_plane: &ExternalSourceControlPlane,
) -> MutexGuard<'_, bitfun_external_sources::ExternalMcpCoordinator> {
    control_plane.lock_mcp()
}

fn lock_snapshot(
    snapshot: &StdMutex<ExternalSourceCatalogSnapshot>,
) -> MutexGuard<'_, ExternalSourceCatalogSnapshot> {
    match snapshot.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

static WORKSPACE_SERVICES: OnceLock<
    DashMap<Option<PathBuf>, Weak<WorkspaceExternalSourceService>>,
> = OnceLock::new();
static READ_ONLY_WORKSPACE_SERVICES: OnceLock<
    DashMap<Option<PathBuf>, Weak<WorkspaceExternalSourceService>>,
> = OnceLock::new();
static SAFE_MODE_WORKSPACES: OnceLock<DashMap<String, ()>> = OnceLock::new();

fn safe_mode_workspaces() -> &'static DashMap<String, ()> {
    SAFE_MODE_WORKSPACES.get_or_init(DashMap::new)
}
static TOOL_REGISTRY_CHANGE_EPOCH: AtomicU64 = AtomicU64::new(0);
static TOOL_REGISTRY_REBUILD_SCHEDULED: AtomicBool = AtomicBool::new(false);

fn workspace_services() -> &'static DashMap<Option<PathBuf>, Weak<WorkspaceExternalSourceService>> {
    WORKSPACE_SERVICES.get_or_init(DashMap::new)
}

fn read_only_workspace_services(
) -> &'static DashMap<Option<PathBuf>, Weak<WorkspaceExternalSourceService>> {
    READ_ONLY_WORKSPACE_SERVICES.get_or_init(DashMap::new)
}

fn workspace_services_for_profile(
    profile: ExternalSourceServiceProfile,
) -> &'static DashMap<Option<PathBuf>, Weak<WorkspaceExternalSourceService>> {
    match profile {
        ExternalSourceServiceProfile::LocalExecution => workspace_services(),
        ExternalSourceServiceProfile::ReadOnlyProjection => read_only_workspace_services(),
    }
}

fn workspace_service_gate() -> &'static tokio::sync::Mutex<()> {
    static GATE: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    GATE.get_or_init(|| tokio::sync::Mutex::new(()))
}

pub(crate) fn normalize_workspace_root(
    workspace_root: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    let Some(workspace_root) = workspace_root else {
        return Ok(None);
    };
    if !workspace_root.is_absolute() {
        return Err("external source workspace root must be absolute".to_string());
    }
    Ok(Some(
        dunce::canonicalize(workspace_root).unwrap_or_else(|_| workspace_root.to_path_buf()),
    ))
}

fn relative_display_path(location: &str, root: &Path) -> Option<String> {
    let normalized_location = location.replace('\\', "/");
    let normalized_root = root.to_string_lossy().replace('\\', "/");
    let root = normalized_root.trim_end_matches('/');
    let prefix = normalized_location.get(..root.len())?;
    let relative = normalized_location.get(root.len()..)?;
    (prefix.eq_ignore_ascii_case(root) && relative.starts_with('/'))
        .then(|| relative.trim_start_matches('/').to_string())
        .or_else(|| (prefix.eq_ignore_ascii_case(root) && relative.is_empty()).then(String::new))
}

fn resolve_external_source_location<'a>(
    stable_key: &str,
    sources: impl IntoIterator<Item = &'a ExternalSourceCatalogEntry>,
) -> Result<PathBuf, String> {
    let location = sources
        .into_iter()
        .find(|source| source.stable_key == stable_key)
        .map(|source| source.record.location.trim())
        .filter(|location| !location.is_empty())
        .ok_or_else(|| {
            missing_candidate_error(format!(
                "External source '{stable_key}' is no longer available"
            ))
        })?;
    let path = PathBuf::from(location);
    if !path.is_absolute() {
        return Err(invalid_operation_error(
            "External source location is not an absolute host path",
        ));
    }
    Ok(path)
}

pub(super) fn safe_external_source_location(
    scope: ExternalSourceScope,
    location: &str,
    workspace_root: Option<&Path>,
) -> String {
    let normalized = location.replace('\\', "/");
    let components = normalized
        .split('/')
        .filter(|component| {
            !component.is_empty()
                && *component != "."
                && *component != ".."
                && !component.ends_with(':')
        })
        .collect::<Vec<_>>();
    let generic_tail = || {
        components
            .iter()
            .position(|component| *component == ".config")
            .map(|index| components[index..].join("/"))
            .unwrap_or_else(|| components[components.len().saturating_sub(3)..].join("/"))
    };

    match scope {
        ExternalSourceScope::Project | ExternalSourceScope::WorkspaceLocal => {
            let relative = workspace_root
                .and_then(|root| relative_display_path(&normalized, root))
                .unwrap_or_else(generic_tail);
            format!("<workspace>/{}", relative.trim_start_matches('/'))
        }
        ExternalSourceScope::UserGlobal => {
            let relative = dirs::home_dir()
                .as_deref()
                .and_then(|home| relative_display_path(&normalized, home))
                .unwrap_or_else(generic_tail);
            format!("~/{}", relative.trim_start_matches('/'))
        }
        ExternalSourceScope::RemoteUser | ExternalSourceScope::RemoteProject => {
            format!("<remote>/{}", generic_tail().trim_start_matches('/'))
        }
        _ => format!(
            "<external-source>/{}",
            generic_tail().trim_start_matches('/')
        ),
    }
}

fn assign_external_source_presentation_groups(snapshot: &mut ExternalSourceCatalogSnapshot) {
    let mut groups = BTreeMap::<(String, String, String), Vec<usize>>::new();
    for (index, source) in snapshot.sources.iter().enumerate() {
        let normalized_location = source
            .record
            .location
            .trim()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string();
        let location_key = if normalized_location.is_empty() {
            format!("<source:{}>", source.stable_key)
        } else {
            normalized_location
        };
        groups
            .entry((
                source.record.ecosystem_id.as_str().to_string(),
                source.record.execution_domain_id.as_str().to_string(),
                location_key,
            ))
            .or_default()
            .push(index);
    }

    for indices in groups.into_values() {
        let mut stable_keys = indices
            .iter()
            .map(|index| snapshot.sources[*index].stable_key.as_str())
            .collect::<Vec<_>>();
        stable_keys.sort_unstable();
        let group_id = format!(
            "external-source:{}",
            serde_json::to_string(&stable_keys).unwrap_or_default()
        );
        for index in indices {
            snapshot.sources[index].presentation_group_id = Some(group_id.clone());
        }
    }
}

fn sanitize_external_snapshot_locations(
    snapshot: &mut ExternalSourceCatalogSnapshot,
    workspace_root: Option<&Path>,
) {
    let source_scopes = snapshot
        .sources
        .iter()
        .map(|source| (source.record.key.clone(), source.record.scope))
        .collect::<BTreeMap<_, _>>();
    let mut replacements = Vec::new();
    let mut remember_location = |scope: ExternalSourceScope, location: &str| {
        if location.is_empty() {
            return;
        }
        let safe = safe_external_source_location(scope, location, workspace_root);
        let safe_prefix = format!("{}/", safe.trim_end_matches('/'));
        for raw in [
            location.to_string(),
            location.replace('\\', "/"),
            location.replace('/', "\\"),
        ] {
            for (raw, safe) in [
                (format!("{raw}/"), safe_prefix.clone()),
                (format!("{raw}\\"), safe_prefix.clone()),
                (raw, safe.clone()),
            ] {
                if raw != safe && !replacements.iter().any(|(known, _)| known == &raw) {
                    replacements.push((raw, safe));
                }
            }
        }
    };
    for source in &snapshot.sources {
        remember_location(source.record.scope, &source.record.location);
    }
    for conflict in &snapshot.command_conflicts {
        for candidate in &conflict.candidates {
            remember_location(candidate.source_scope, &candidate.source_location);
        }
    }
    for request in &snapshot.tool_approval_requests {
        remember_location(request.source_scope, &request.source_location);
        remember_location(request.source_scope, &request.working_directory);
    }
    for tool in &snapshot.tools {
        let scope = source_scopes
            .get(&tool.definition.id.target.source)
            .copied()
            .unwrap_or(ExternalSourceScope::WorkspaceLocal);
        remember_location(scope, &tool.definition.module_path);
        remember_location(scope, &tool.definition.working_directory);
    }
    for conflict in &snapshot.tool_conflicts {
        for candidate in &conflict.candidates {
            let Some(location) = candidate.source_location.as_deref() else {
                continue;
            };
            let scope = candidate
                .source
                .as_ref()
                .and_then(|source| source_scopes.get(source))
                .copied()
                .unwrap_or(ExternalSourceScope::WorkspaceLocal);
            remember_location(scope, location);
        }
    }
    for server in &snapshot.mcp_servers {
        let Some(directory) = server.definition.working_directory.as_deref() else {
            continue;
        };
        let scope = source_scopes
            .get(&server.definition.id.source)
            .copied()
            .unwrap_or(ExternalSourceScope::WorkspaceLocal);
        remember_location(scope, directory);
    }
    for request in &snapshot.mcp_approval_requests {
        let Some(directory) = request.definition.working_directory.as_deref() else {
            continue;
        };
        let scope = source_scopes
            .get(&request.definition.id.source)
            .copied()
            .unwrap_or(ExternalSourceScope::WorkspaceLocal);
        remember_location(scope, directory);
    }
    drop(remember_location);
    replacements.sort_by(|left, right| right.0.len().cmp(&left.0.len()));
    let sanitize_message = |message: &mut String| {
        for (raw, safe) in &replacements {
            if message.contains(raw) {
                *message = message.replace(raw, safe);
            }
        }
    };
    for diagnostic in &mut snapshot.diagnostics {
        sanitize_message(&mut diagnostic.message);
    }
    for source in &mut snapshot.sources {
        for diagnostic in &mut source.record.diagnostics {
            sanitize_message(&mut diagnostic.message);
        }
    }

    for source in &mut snapshot.sources {
        source.record.location = safe_external_source_location(
            source.record.scope,
            &source.record.location,
            workspace_root,
        );
    }
    for conflict in &mut snapshot.command_conflicts {
        for candidate in &mut conflict.candidates {
            candidate.source_location = safe_external_source_location(
                candidate.source_scope,
                &candidate.source_location,
                workspace_root,
            );
        }
    }
    for request in &mut snapshot.tool_approval_requests {
        request.source_location = safe_external_source_location(
            request.source_scope,
            &request.source_location,
            workspace_root,
        );
        request.working_directory = safe_external_source_location(
            request.source_scope,
            &request.working_directory,
            workspace_root,
        );
    }
    for tool in &mut snapshot.tools {
        let scope = source_scopes
            .get(&tool.definition.id.target.source)
            .copied()
            .unwrap_or(ExternalSourceScope::WorkspaceLocal);
        tool.definition.module_path =
            safe_external_source_location(scope, &tool.definition.module_path, workspace_root);
        tool.definition.working_directory = safe_external_source_location(
            scope,
            &tool.definition.working_directory,
            workspace_root,
        );
    }
    for conflict in &mut snapshot.tool_conflicts {
        for candidate in &mut conflict.candidates {
            let Some(location) = candidate.source_location.as_mut() else {
                continue;
            };
            let scope = candidate
                .source
                .as_ref()
                .and_then(|source| source_scopes.get(source))
                .copied()
                .unwrap_or(ExternalSourceScope::WorkspaceLocal);
            *location = safe_external_source_location(scope, location, workspace_root);
        }
    }
    for server in &mut snapshot.mcp_servers {
        let Some(directory) = server.definition.working_directory.as_mut() else {
            continue;
        };
        let scope = source_scopes
            .get(&server.definition.id.source)
            .copied()
            .unwrap_or(ExternalSourceScope::WorkspaceLocal);
        *directory = safe_external_source_location(scope, directory, workspace_root);
    }
    for request in &mut snapshot.mcp_approval_requests {
        let Some(directory) = request.definition.working_directory.as_mut() else {
            continue;
        };
        let scope = source_scopes
            .get(&request.definition.id.source)
            .copied()
            .unwrap_or(ExternalSourceScope::WorkspaceLocal);
        *directory = safe_external_source_location(scope, directory, workspace_root);
    }
}

async fn load_native_mcp_candidates() -> Result<Vec<NativeMcpCandidate>, String> {
    let service = crate::service::mcp::get_global_mcp_service()
        .ok_or_else(|| "MCP service is not initialized".to_string())?;
    let configs = service
        .config_service()
        .load_all_configs()
        .await
        .map_err(|error| format!("Could not read BitFun MCP configuration: {error}"))?;
    let mut candidates = Vec::with_capacity(configs.len());
    for config in configs {
        let encoded = serde_json::to_vec(&config)
            .map_err(|error| format!("Could not fingerprint BitFun MCP configuration: {error}"))?;
        let mut behavior_hasher = Sha256::new();
        behavior_hasher.update(&encoded);
        let behavior_version = format!("sha256:{}", hex::encode(behavior_hasher.finalize()));
        let candidate_id = native_mcp_candidate_id(&config.id);
        candidates.push(NativeMcpCandidate {
            candidate_id,
            server_id: config.id,
            display_name: format!("BitFun: {}", config.name),
            name: config.name,
            behavior_version,
            enabled: config.enabled,
        });
    }
    candidates.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.server_id.cmp(&right.server_id))
    });
    Ok(candidates)
}

/// Stable product identifier for a BitFun-owned MCP configuration. Surfaces
/// use this only to correlate native list rows with conflict candidates; the
/// underlying configuration id remains private to the MCP owner.
pub fn native_mcp_candidate_id(server_id: &str) -> String {
    let mut id_hasher = Sha256::new();
    id_hasher.update(server_id.as_bytes());
    format!("native_mcp:{}", &hex::encode(id_hasher.finalize())[..24])
}

fn apply_external_mcp_runtime_status(
    state: &mut ExternalMcpProductState,
    candidate: &crate::external_mcp::ActiveExternalMcpCandidate,
    status: Result<ExternalMcpRuntimeStatus, String>,
) {
    match status {
        Ok(ExternalMcpRuntimeStatus::Active) => {}
        Ok(ExternalMcpRuntimeStatus::Loading) => {
            if let Some(entry) = state
                .entries
                .iter_mut()
                .find(|entry| entry.candidate_id == candidate.definition.candidate_id())
            {
                entry.activation_state = ExternalMcpActivationState::Starting;
            }
        }
        Ok(ExternalMcpRuntimeStatus::Unavailable(reason)) | Err(reason) => {
            mark_external_mcp_runtime_unavailable(state, candidate, reason);
        }
    }
}

fn mark_external_mcp_runtime_unavailable(
    state: &mut ExternalMcpProductState,
    candidate: &crate::external_mcp::ActiveExternalMcpCandidate,
    reason: String,
) {
    if let Some(entry) = state
        .entries
        .iter_mut()
        .find(|entry| entry.candidate_id == candidate.definition.candidate_id())
    {
        entry.activation_state = ExternalMcpActivationState::RuntimeUnavailable {
            reason: reason.clone(),
        };
    }
    state.diagnostics.push(
        ExternalSourceDiagnostic::warning(
            "external_mcp.runtime_unavailable",
            reason,
            Some(candidate.definition.id.source.clone()),
        )
        .with_asset_kind(ExternalSourceAssetKind::Mcp),
    );
}

fn merge_mcp_state(
    snapshot: &mut ExternalSourceCatalogSnapshot,
    coordinator_snapshot: &bitfun_external_sources::ExternalMcpCoordinatorSnapshot,
    state: ExternalMcpProductState,
) {
    let known_sources = snapshot
        .sources
        .iter()
        .map(|source| source.stable_key.clone())
        .collect::<BTreeSet<_>>();
    snapshot.sources.extend(
        coordinator_snapshot
            .sources
            .iter()
            .filter(|source| !known_sources.contains(&source.stable_key))
            .cloned(),
    );
    snapshot.sources.sort_by(|left, right| {
        left.record
            .ecosystem_id
            .cmp(&right.record.ecosystem_id)
            .then(left.stable_key.cmp(&right.stable_key))
    });
    snapshot
        .diagnostics
        .extend(coordinator_snapshot.diagnostics.clone());
    snapshot.diagnostics.extend(state.diagnostics.clone());
    snapshot.discovery_pending |= coordinator_snapshot.discovery_pending;
    snapshot.mcp_generation = coordinator_snapshot.generation;
    snapshot.mcp_servers = state.entries;
    snapshot.mcp_approval_requests = state.approval_requests;
    snapshot.mcp_conflicts = state.conflicts;
}

async fn service_for(
    workspace_root: Option<&Path>,
) -> Result<Arc<WorkspaceExternalSourceService>, String> {
    service_for_profile(workspace_root, ExternalSourceServiceProfile::LocalExecution).await
}

async fn read_only_service_for(
    workspace_root: Option<&Path>,
) -> Result<Arc<WorkspaceExternalSourceService>, String> {
    service_for_profile(
        workspace_root,
        ExternalSourceServiceProfile::ReadOnlyProjection,
    )
    .await
}

async fn service_for_profile(
    workspace_root: Option<&Path>,
    profile: ExternalSourceServiceProfile,
) -> Result<Arc<WorkspaceExternalSourceService>, String> {
    let workspace_root = normalize_workspace_root(workspace_root)?;
    // Serialize cache acquisition with idle retirement. Without this lease
    // gate, a caller could upgrade the weak entry after the retirement count
    // check and have its newly acquired routes removed underneath it.
    let _service_gate = workspace_service_gate().lock().await;
    let services = workspace_services_for_profile(profile);
    if let Some(service) = services
        .get(&workspace_root)
        .and_then(|service| service.value().upgrade())
    {
        service.touch();
        sync_service_preferences(&service).await?;
        return Ok(service);
    }
    let created = WorkspaceExternalSourceService::create(workspace_root.clone(), profile).await?;
    let service = match services.entry(workspace_root) {
        Entry::Occupied(mut entry) => match entry.get().upgrade() {
            Some(existing) => existing,
            None => {
                entry.insert(Arc::downgrade(&created));
                created
            }
        },
        Entry::Vacant(entry) => {
            entry.insert(Arc::downgrade(&created));
            created
        }
    };
    service.touch();
    service.ensure_idle_keepalive();
    sync_service_preferences(&service).await?;
    Ok(service)
}

fn epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn read_external_sources_config() -> Result<ExternalSourcesConfig, String> {
    ExternalSourcePreferenceStore::global()?.read().await
}

pub(crate) async fn external_tool_invocation_is_authorized(
    ecosystem_id: &str,
    approval_key: &str,
    source_key: &str,
    workspace_route: &str,
) -> Result<bool, String> {
    if external_source_safe_mode_enabled_for(LEGACY_LOCAL_EXECUTION_DOMAIN_ID, workspace_route) {
        return Ok(false);
    }
    let preferences = read_external_sources_config().await?;
    Ok(external_tool_invocation_is_authorized_by(
        &preferences,
        ecosystem_id,
        approval_key,
        source_key,
        workspace_route,
    ))
}

fn external_tool_invocation_is_authorized_by(
    preferences: &ExternalSourcesConfig,
    ecosystem_id: &str,
    approval_key: &str,
    source_preference_key: &str,
    workspace_route: &str,
) -> bool {
    let policy = preferences.integration_policy.known().map(|document| {
        external_integration_policy_snapshot(
            document,
            workspace_policy_key_from_route(workspace_route).as_deref(),
            default_external_integration_ecosystems(),
        )
    });
    policy.is_some_and(|policy| {
        policy.is_ok_and(|policy| {
            integration_capability_is_active(&policy, ecosystem_id, EXTERNAL_CAPABILITY_TOOL)
        })
    }) && preferences.approved_tool_targets.contains(approval_key)
        && !preferences
            .suppressed_source_keys
            .iter()
            .any(|suppressed| suppressed == source_preference_key)
}

pub(crate) async fn external_tool_conflict_selection_is_current(
    conflict_key: &str,
    candidate_id: Option<&str>,
) -> Result<bool, String> {
    let preferences = read_external_sources_config().await?;
    let persisted = preferences
        .tool_conflict_choices
        .get(conflict_key)
        .map(String::as_str)
        .filter(|choice| {
            *choice != UNRESOLVED_TOOL_CONFLICT_CHOICE
                && *choice != TOOL_CONFLICT_RESELECTION_REQUIRED
        });
    Ok(persisted == candidate_id)
}

async fn persist_observed_tool_conflicts(conflicts: &[ExternalToolConflict]) -> Result<(), String> {
    if conflicts.is_empty() {
        return Ok(());
    }
    let conflicts = conflicts.to_vec();
    ExternalSourcePreferenceStore::global()?
        .update(move |config| {
            let previous = config.tool_conflict_choices.clone();
            for conflict in conflicts {
                reconcile_observed_tool_conflict(
                    &mut config.tool_conflict_choices,
                    &conflict.conflict_key,
                );
            }
            if config.tool_conflict_choices != previous {
                config.preference_revision = config.preference_revision.saturating_add(1);
            }
        })
        .await
        .map(|_| ())
}

async fn persist_observed_subagent_conflicts(
    observed: &BTreeMap<String, String>,
) -> Result<(bool, ExternalSourcesConfig), String> {
    let store = ExternalSourcePreferenceStore::global()?;
    persist_observed_subagent_conflicts_with_store(&store, observed).await
}

async fn persist_observed_subagent_conflicts_with_store(
    store: &ExternalSourcePreferenceStore,
    observed: &BTreeMap<String, String>,
) -> Result<(bool, ExternalSourcesConfig), String> {
    if observed.is_empty() {
        return store.read().await.map(|config| (false, config));
    }
    let observed = observed.clone();
    store
        .update(move |config| {
            let mut changed = false;
            for (lineage, current_key) in observed {
                let previous_key = config
                    .subagent_conflict_lineage_current_keys
                    .get(&lineage)
                    .cloned();
                if previous_key.as_deref() == Some(current_key.as_str()) {
                    continue;
                }
                config
                    .subagent_conflict_lineage_current_keys
                    .insert(lineage, current_key.clone());
                changed = true;
                let previous_choice = previous_key
                    .as_ref()
                    .and_then(|previous_key| config.subagent_conflict_choices.remove(previous_key));
                if previous_choice.is_some() {
                    let replaced = config.subagent_conflict_choices.insert(
                        current_key,
                        SUBAGENT_CONFLICT_RESELECTION_REQUIRED.to_string(),
                    );
                    changed |= replaced.as_deref() != Some(SUBAGENT_CONFLICT_RESELECTION_REQUIRED);
                }
            }
            if changed {
                config.preference_revision = config.preference_revision.saturating_add(1);
            }
            changed
        })
        .await
}

fn merge_subagent_state(
    snapshot: &mut ExternalSourceCatalogSnapshot,
    coordinator_snapshot: &bitfun_external_sources::ExternalSubagentCoordinatorSnapshot,
    state: &ExternalSubagentProductState,
    preference_revision: u64,
) {
    snapshot.generation = snapshot.generation.max(coordinator_snapshot.generation);
    snapshot.discovery_pending |= coordinator_snapshot.discovery_pending;
    let known_sources = snapshot
        .sources
        .iter()
        .map(|entry| entry.stable_key.clone())
        .collect::<BTreeSet<_>>();
    snapshot.sources.extend(
        coordinator_snapshot
            .sources
            .iter()
            .filter(|source| !known_sources.contains(&source.stable_key))
            .cloned(),
    );
    snapshot
        .sources
        .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    snapshot.subagent_generation = coordinator_snapshot.generation;
    snapshot.preference_revision = preference_revision;
    snapshot.subagents = state.summaries.clone();
    snapshot.subagent_conflicts = state.conflicts.clone();
    snapshot.pending_subagent_approvals = state.pending_approvals.clone();
    snapshot
        .diagnostics
        .extend(coordinator_snapshot.diagnostics.clone());
}

fn reconcile_observed_tool_conflict(choices: &mut BTreeMap<String, String>, conflict_key: &str) {
    if choices.contains_key(conflict_key) {
        return;
    }
    let Some((lineage, _)) = conflict_key.rsplit_once(':') else {
        choices.insert(
            conflict_key.to_string(),
            UNRESOLVED_TOOL_CONFLICT_CHOICE.to_string(),
        );
        return;
    };
    let requires_fail_closed_reselection = choices.iter().any(|(existing_key, choice)| {
        existing_key
            .rsplit_once(':')
            .is_some_and(|(existing_lineage, _)| existing_lineage == lineage)
            && (choice.starts_with("external:") || choice == TOOL_CONFLICT_RESELECTION_REQUIRED)
    });
    choices.retain(|existing_key, _| {
        existing_key
            .rsplit_once(':')
            .is_none_or(|(existing_lineage, _)| existing_lineage != lineage)
    });
    choices.insert(
        conflict_key.to_string(),
        if requires_fail_closed_reselection {
            TOOL_CONFLICT_RESELECTION_REQUIRED.to_string()
        } else {
            UNRESOLVED_TOOL_CONFLICT_CHOICE.to_string()
        },
    );
}

async fn persist_source_enabled_change(
    stable_key: &str,
    enabled: bool,
    expected_preference_revision: u64,
) -> Result<BTreeSet<String>, String> {
    let stable_key = stable_key.to_string();
    ExternalSourcePreferenceStore::global()?
        .update(move |config| {
            if config.preference_revision != expected_preference_revision {
                return None;
            }
            let mut sources = config
                .suppressed_source_keys
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            if enabled {
                sources.remove(&stable_key);
            } else {
                sources.insert(stable_key);
            }
            let next = sources.iter().cloned().collect::<Vec<_>>();
            if config.suppressed_source_keys != next {
                config.suppressed_source_keys = next;
                config.preference_revision = config.preference_revision.saturating_add(1);
            }
            Some(sources)
        })
        .await
        .and_then(|(sources, _)| {
            sources.ok_or_else(|| {
                stale_operation_error(
                    "External source preferences changed; refresh before retrying",
                )
            })
        })
}

async fn persist_conflict_choice(
    conflict_key: &str,
    candidate_id: &str,
    participants: Vec<String>,
    expected_preference_revision: u64,
) -> Result<ExternalSourcesConfig, String> {
    let conflict_key = conflict_key.to_string();
    let candidate_id = candidate_id.to_string();
    ExternalSourcePreferenceStore::global()?
        .update(move |config| {
            if config.preference_revision != expected_preference_revision {
                return false;
            }
            let previous_choices = config.conflict_choices.clone();
            let previous_lineage = config.conflict_lineage_current_keys.clone();
            let previous_candidates = config.conflicted_candidate_ids.clone();
            ExternalSourceCoordinator::reconcile_conflict_preferences(
                &mut config.conflict_choices,
                &mut config.conflict_lineage_current_keys,
                &mut config.conflicted_candidate_ids,
                &conflict_key,
                &candidate_id,
                &participants,
            );
            if config.conflict_choices != previous_choices
                || config.conflict_lineage_current_keys != previous_lineage
                || config.conflicted_candidate_ids != previous_candidates
            {
                config.preference_revision = config.preference_revision.saturating_add(1);
            }
            true
        })
        .await
        .and_then(|(applied, config)| {
            applied.then_some(config).ok_or_else(|| {
                stale_operation_error(
                    "External command preferences changed; refresh before retrying",
                )
            })
        })
}

async fn persist_tool_target_decision(
    approval_key: &str,
    decision_key: &str,
    approved: bool,
    expected_preference_revision: u64,
) -> Result<ExternalSourcesConfig, String> {
    let approval_key = approval_key.to_string();
    let decision_key = decision_key.to_string();
    ExternalSourcePreferenceStore::global()?
        .update(move |config| {
            if config.preference_revision != expected_preference_revision {
                return false;
            }
            let previous_approved = config.approved_tool_targets.clone();
            let previous_declined = config.declined_tool_decisions.clone();
            reconcile_tool_target_decision(config, approval_key, decision_key, approved);
            if config.approved_tool_targets != previous_approved
                || config.declined_tool_decisions != previous_declined
            {
                config.preference_revision = config.preference_revision.saturating_add(1);
            }
            true
        })
        .await
        .and_then(|(applied, config)| {
            applied.then_some(config).ok_or_else(|| {
                stale_operation_error("External tool preferences changed; refresh before retrying")
            })
        })
}

fn reconcile_tool_target_decision(
    config: &mut ExternalSourcesConfig,
    approval_key: String,
    decision_key: String,
    approved: bool,
) {
    if approved {
        config.approved_tool_targets.insert(approval_key.clone());
        config.declined_tool_decisions.remove(&approval_key);
    } else {
        config.approved_tool_targets.remove(&approval_key);
        config
            .declined_tool_decisions
            .insert(approval_key, decision_key);
    }
}

async fn persist_tool_conflict_choice(
    conflict_key: &str,
    candidate_id: &str,
    expected_preference_revision: u64,
) -> Result<ExternalSourcesConfig, String> {
    let conflict_key = conflict_key.to_string();
    let candidate_id = candidate_id.to_string();
    ExternalSourcePreferenceStore::global()?
        .update(move |config| {
            if config.preference_revision != expected_preference_revision {
                return false;
            }
            let previous = config.tool_conflict_choices.clone();
            reconcile_versioned_tool_conflict_choice(
                &mut config.tool_conflict_choices,
                conflict_key,
                candidate_id,
            );
            if config.tool_conflict_choices != previous {
                config.preference_revision = config.preference_revision.saturating_add(1);
            }
            true
        })
        .await
        .and_then(|(applied, config)| {
            applied.then_some(config).ok_or_else(|| {
                stale_operation_error("External tool preferences changed; refresh before retrying")
            })
        })
}

async fn persist_subagent_activation(
    approval_key: &str,
    approved: bool,
    expected_preference_revision: u64,
) -> Result<ExternalSourcesConfig, String> {
    let approval_key = approval_key.to_string();
    ExternalSourcePreferenceStore::global()?
        .update(move |config| {
            if config.preference_revision != expected_preference_revision {
                return false;
            }
            if approved {
                config
                    .approved_subagent_envelopes
                    .insert(approval_key.clone());
                config.declined_subagent_decisions.remove(&approval_key);
            } else {
                config.approved_subagent_envelopes.remove(&approval_key);
                config
                    .declined_subagent_decisions
                    .insert(approval_key.clone(), approval_key);
            }
            config.preference_revision = config.preference_revision.saturating_add(1);
            true
        })
        .await
        .and_then(|(applied, config)| {
            applied.then_some(config).ok_or_else(|| {
                stale_operation_error(
                    "External subagent preferences changed; refresh before retrying",
                )
            })
        })
}

async fn persist_subagent_conflict_choice(
    conflict_key: &str,
    candidate_id: &str,
    approval_key: Option<&str>,
    expected_preference_revision: u64,
) -> Result<ExternalSourcesConfig, String> {
    let store = ExternalSourcePreferenceStore::global()?;
    persist_subagent_conflict_choice_with_store(
        &store,
        conflict_key,
        candidate_id,
        approval_key,
        expected_preference_revision,
    )
    .await
}

async fn persist_subagent_conflict_choice_with_store(
    store: &ExternalSourcePreferenceStore,
    conflict_key: &str,
    candidate_id: &str,
    approval_key: Option<&str>,
    expected_preference_revision: u64,
) -> Result<ExternalSourcesConfig, String> {
    let conflict_key = conflict_key.to_string();
    let candidate_id = candidate_id.to_string();
    let approval_key = approval_key.map(str::to_string);
    store
        .update(move |config| {
            if config.preference_revision != expected_preference_revision {
                return false;
            }
            config
                .subagent_conflict_choices
                .insert(conflict_key, candidate_id);
            if let Some(approval_key) = approval_key {
                config
                    .approved_subagent_envelopes
                    .insert(approval_key.clone());
                config.declined_subagent_decisions.remove(&approval_key);
            }
            config.preference_revision = config.preference_revision.saturating_add(1);
            true
        })
        .await
        .and_then(|(applied, config)| {
            applied.then_some(config).ok_or_else(|| {
                stale_operation_error(
                    "External subagent preferences changed; refresh before retrying",
                )
            })
        })
}

async fn persist_mcp_server_decision(
    decision_key: &str,
    approved: bool,
    expected_preference_revision: u64,
) -> Result<ExternalSourcesConfig, String> {
    let store = ExternalSourcePreferenceStore::global()?;
    let decision_key = decision_key.to_string();
    store
        .update(move |config| {
            if config.preference_revision != expected_preference_revision {
                return false;
            }
            reconcile_versioned_mcp_server_decision(
                &mut config.mcp_server_decisions,
                decision_key,
                approved,
            );
            config.preference_revision = config.preference_revision.saturating_add(1);
            true
        })
        .await
        .and_then(|(applied, config)| {
            applied.then_some(config).ok_or_else(|| {
                stale_operation_error("External MCP preferences changed; refresh before retrying")
            })
        })
}

async fn persist_mcp_conflict_choice(
    conflict_key: &str,
    candidate_id: &str,
    external_decision: Option<&str>,
    expected_preference_revision: u64,
) -> Result<ExternalSourcesConfig, String> {
    let store = ExternalSourcePreferenceStore::global()?;
    let conflict_key = conflict_key.to_string();
    let candidate_id = candidate_id.to_string();
    let external_decision = external_decision.map(str::to_string);
    store
        .update(move |config| {
            if config.preference_revision != expected_preference_revision {
                return false;
            }
            reconcile_versioned_mcp_conflict_choice(
                &mut config.mcp_conflict_choices,
                conflict_key,
                candidate_id,
            );
            if let Some(decision_key) = external_decision {
                reconcile_versioned_mcp_server_decision(
                    &mut config.mcp_server_decisions,
                    decision_key,
                    true,
                );
            }
            config.preference_revision = config.preference_revision.saturating_add(1);
            true
        })
        .await
        .and_then(|(applied, config)| {
            applied.then_some(config).ok_or_else(|| {
                stale_operation_error("External MCP preferences changed; refresh before retrying")
            })
        })
}

fn validate_integration_policy_operation(
    scope: ExternalIntegrationPolicyScope,
    operation: &ExternalIntegrationPolicyOperation,
) -> Result<(), String> {
    let descriptors = default_external_integration_ecosystems();
    let validate_ecosystem =
        |ecosystem_id: &bitfun_product_domains::external_sources::EcosystemId| {
            descriptors
                .iter()
                .find(|descriptor| descriptor.ecosystem_id == *ecosystem_id)
                .ok_or_else(|| {
                    invalid_operation_error(format!(
                        "External ecosystem '{}' is not registered",
                        ecosystem_id
                    ))
                })
        };
    match operation {
        ExternalIntegrationPolicyOperation::SetEnabled { .. } => Ok(()),
        ExternalIntegrationPolicyOperation::SetEcosystemMode { ecosystem_id, mode } => {
            validate_ecosystem(ecosystem_id)?;
            mode.is_known().then_some(()).ok_or_else(|| {
                invalid_operation_error("External integration mode is not supported")
            })
        }
        ExternalIntegrationPolicyOperation::SetCapabilityAccess {
            ecosystem_id,
            capability_id,
            access,
        } => {
            let descriptor = validate_ecosystem(ecosystem_id)?;
            if !descriptor
                .capabilities
                .iter()
                .any(|capability| capability.capability_id == *capability_id)
                || !access.is_known()
            {
                return Err(invalid_operation_error(format!(
                    "External capability '{}' is not registered",
                    capability_id
                )));
            }
            Ok(())
        }
        ExternalIntegrationPolicyOperation::ResetWorkspace
            if scope == ExternalIntegrationPolicyScope::Workspace =>
        {
            Ok(())
        }
        ExternalIntegrationPolicyOperation::ResetWorkspace => Err(invalid_operation_error(
            "reset_workspace requires workspace policy scope",
        )),
        ExternalIntegrationPolicyOperation::ResetIncompatiblePolicy
            if scope == ExternalIntegrationPolicyScope::User =>
        {
            Ok(())
        }
        ExternalIntegrationPolicyOperation::ResetIncompatiblePolicy => Err(
            invalid_operation_error("reset_incompatible_policy requires user policy scope"),
        ),
        _ => Err(invalid_operation_error("Policy operation is not supported")),
    }
}

fn apply_user_policy_operation(
    settings: &mut ExternalIntegrationPolicySettings,
    operation: &ExternalIntegrationPolicyOperation,
) -> Result<bool, String> {
    match operation {
        ExternalIntegrationPolicyOperation::SetEnabled { enabled } => {
            let changed = settings.enabled != *enabled;
            settings.enabled = *enabled;
            Ok(changed)
        }
        ExternalIntegrationPolicyOperation::SetEcosystemMode { ecosystem_id, mode } => {
            let policy = settings.ecosystems.entry(ecosystem_id.clone()).or_default();
            let changed = policy.mode != *mode;
            policy.mode = mode.clone();
            Ok(changed)
        }
        ExternalIntegrationPolicyOperation::SetCapabilityAccess {
            ecosystem_id,
            capability_id,
            access,
        } => {
            let policy = settings.ecosystems.entry(ecosystem_id.clone()).or_default();
            let changed = policy.mode != ExternalIntegrationMode::Custom
                || policy.capability_overrides.get(capability_id) != Some(access);
            policy.mode = ExternalIntegrationMode::Custom;
            policy
                .capability_overrides
                .insert(capability_id.clone(), access.clone());
            Ok(changed)
        }
        ExternalIntegrationPolicyOperation::ResetWorkspace => Err(invalid_operation_error(
            "reset_workspace cannot update user defaults",
        )),
        _ => Err(invalid_operation_error("Policy operation is not supported")),
    }
}

fn apply_workspace_policy_operation(
    document: &mut ExternalIntegrationPolicyDocument,
    workspace_key: &str,
    operation: &ExternalIntegrationPolicyOperation,
) -> Result<bool, String> {
    if matches!(
        operation,
        ExternalIntegrationPolicyOperation::ResetWorkspace
    ) {
        return Ok(document.workspace_overrides.remove(workspace_key).is_some());
    }
    let policy = document
        .workspace_overrides
        .entry(workspace_key.to_string())
        .or_default();
    match operation {
        ExternalIntegrationPolicyOperation::SetEnabled { enabled } => {
            let changed = policy.enabled != Some(*enabled);
            policy.enabled = Some(*enabled);
            Ok(changed)
        }
        ExternalIntegrationPolicyOperation::SetEcosystemMode { ecosystem_id, mode } => {
            let ecosystem = policy.ecosystems.entry(ecosystem_id.clone()).or_default();
            let changed = ecosystem.mode.as_ref() != Some(mode);
            ecosystem.mode = Some(mode.clone());
            Ok(changed)
        }
        ExternalIntegrationPolicyOperation::SetCapabilityAccess {
            ecosystem_id,
            capability_id,
            access,
        } => {
            let ecosystem = policy.ecosystems.entry(ecosystem_id.clone()).or_default();
            let changed = ecosystem.mode != Some(ExternalIntegrationMode::Custom)
                || ecosystem.capability_overrides.get(capability_id) != Some(access);
            ecosystem.mode = Some(ExternalIntegrationMode::Custom);
            ecosystem
                .capability_overrides
                .insert(capability_id.clone(), access.clone());
            Ok(changed)
        }
        ExternalIntegrationPolicyOperation::ResetWorkspace => Ok(false),
        _ => Err(invalid_operation_error("Policy operation is not supported")),
    }
}

async fn persist_integration_policy_mutation(
    workspace_root: Option<&Path>,
    mutation: ExternalIntegrationPolicyMutation,
) -> Result<ExternalSourcesConfig, String> {
    validate_integration_policy_operation(mutation.scope, &mutation.change)?;
    let workspace_key = match mutation.scope {
        ExternalIntegrationPolicyScope::User => None,
        ExternalIntegrationPolicyScope::Workspace => {
            Some(workspace_policy_key(workspace_root).ok_or_else(|| {
                invalid_operation_error("Workspace policy scope requires a workspace")
            })?)
        }
        _ => {
            return Err(invalid_operation_error("Policy scope is not supported"));
        }
    };
    ExternalSourcePreferenceStore::global()?
        .update(move |config| {
            apply_integration_policy_mutation_to_config(config, workspace_key.as_deref(), &mutation)
                .map(|_| ())
        })
        .await
        .and_then(|(result, config)| result.map(|()| config))
}

fn apply_integration_policy_mutation_to_config(
    config: &mut ExternalSourcesConfig,
    workspace_key: Option<&str>,
    mutation: &ExternalIntegrationPolicyMutation,
) -> Result<bool, String> {
    if config.preference_revision != mutation.expected_preference_revision {
        return Err(stale_operation_error(
            "External integration policy changed; refresh before retrying",
        ));
    }
    let incompatible = config.integration_policy.known().is_none();
    if incompatible {
        if mutation.scope == ExternalIntegrationPolicyScope::User
            && matches!(
                &mutation.change,
                ExternalIntegrationPolicyOperation::ResetIncompatiblePolicy
            )
        {
            config
                .integration_policy_backups
                .push(config.integration_policy.raw_value());
            const MAX_POLICY_BACKUPS: usize = 3;
            if config.integration_policy_backups.len() > MAX_POLICY_BACKUPS {
                let remove_count = config.integration_policy_backups.len() - MAX_POLICY_BACKUPS;
                config.integration_policy_backups.drain(0..remove_count);
            }
            let mut reset_policy = StoredExternalIntegrationPolicy::default();
            reset_policy
                .known_mut()
                .expect("the host-owned default policy schema must be compatible")
                .user_defaults
                .enabled = false;
            config.integration_policy = reset_policy;
            config.preference_revision = config.preference_revision.saturating_add(1);
            return Ok(true);
        }
        return Err(incompatible_policy_error(format!(
            "External integration policy schema {} is not supported; back up and reset it before making changes",
            config.integration_policy.schema_major()
        )));
    }
    if matches!(
        &mutation.change,
        ExternalIntegrationPolicyOperation::ResetIncompatiblePolicy
    ) {
        return Err(invalid_operation_error(
            "External integration policy is already compatible",
        ));
    }
    let document = config.integration_policy.known_mut().ok_or_else(|| {
        incompatible_policy_error("External integration policy requires a backup and reset")
    })?;
    let changed = match mutation.scope {
        ExternalIntegrationPolicyScope::User => {
            apply_user_policy_operation(&mut document.user_defaults, &mutation.change)?
        }
        ExternalIntegrationPolicyScope::Workspace => apply_workspace_policy_operation(
            document,
            workspace_key.ok_or_else(|| {
                invalid_operation_error("Workspace policy scope requires a workspace")
            })?,
            &mutation.change,
        )?,
        _ => return Err(invalid_operation_error("Policy scope is not supported")),
    };
    if changed {
        config.preference_revision = config.preference_revision.saturating_add(1);
    }
    Ok(changed)
}

fn reconcile_versioned_mcp_conflict_choice(
    choices: &mut BTreeMap<String, String>,
    conflict_key: String,
    candidate_id: String,
) {
    if let Some((lineage, _)) = conflict_key.rsplit_once(':') {
        choices.retain(|existing_key, _| {
            existing_key
                .rsplit_once(':')
                .is_none_or(|(existing_lineage, _)| existing_lineage != lineage)
        });
    }
    choices.insert(conflict_key, candidate_id);
}

fn reconcile_versioned_mcp_server_decision(
    decisions: &mut BTreeMap<String, ExternalMcpDecision>,
    decision_key: String,
    approved: bool,
) {
    if let Some((lineage, _)) = decision_key.rsplit_once(':') {
        decisions.retain(|existing_key, _| {
            existing_key
                .rsplit_once(':')
                .is_none_or(|(existing_lineage, _)| existing_lineage != lineage)
        });
    }
    decisions.insert(
        decision_key.clone(),
        ExternalMcpDecision {
            decision_key,
            approved,
        },
    );
}

fn reconcile_versioned_tool_conflict_choice(
    choices: &mut BTreeMap<String, String>,
    conflict_key: String,
    candidate_id: String,
) {
    if let Some((lineage, _)) = conflict_key.rsplit_once(':') {
        choices.retain(|existing_key, _| {
            existing_key
                .rsplit_once(':')
                .is_none_or(|(existing_lineage, _)| existing_lineage != lineage)
        });
    }
    choices.insert(conflict_key, candidate_id);
}

fn propagate_suppressed_sources(
    sources: &BTreeSet<String>,
    current: &Arc<WorkspaceExternalSourceService>,
) {
    for service in workspace_services().iter() {
        let Some(service) = service.value().upgrade() else {
            continue;
        };
        if Arc::ptr_eq(&service, current) {
            continue;
        }
        lock_coordinator(&service.control_plane).replace_suppressed_sources(sources.clone());
        lock_tool_coordinator(&service.control_plane).replace_suppressed_sources(sources.clone());
        lock_subagent_coordinator(&service.control_plane)
            .replace_suppressed_sources(sources.clone());
        lock_mcp_coordinator(&service.control_plane).replace_suppressed_sources(sources.clone());
        tokio::spawn(async move {
            if let Err(error) = service.refresh_preserving_worker_recovery().await {
                log::warn!(
                    "Could not refresh external sources after source preference change scope={} error_category={}",
                    external_log_scope(service.workspace_root.as_deref()),
                    external_log_error_category(&error),
                );
            }
        });
    }
}

fn propagate_conflict_preferences(preferences: &ExternalSourcesConfig) {
    for service in workspace_services().iter() {
        let Some(service) = service.value().upgrade() else {
            continue;
        };
        {
            let mut coordinator = lock_coordinator(&service.control_plane);
            coordinator.replace_conflict_choices(preferences.conflict_choices.clone());
            coordinator.replace_conflict_lineage_current_keys(
                preferences.conflict_lineage_current_keys.clone(),
            );
            coordinator
                .replace_conflicted_candidate_ids(preferences.conflicted_candidate_ids.clone());
        }
        tokio::spawn(async move {
            let command_snapshot = lock_coordinator(&service.control_plane).snapshot();
            if let Ok(snapshot) = service.rebuild_product_snapshot(command_snapshot).await {
                let _ = service.updates.send(snapshot);
            }
        });
    }
}

fn propagate_tool_preferences(_preferences: &ExternalSourcesConfig) {
    for service in workspace_services().iter() {
        let Some(service) = service.value().upgrade() else {
            continue;
        };
        tokio::spawn(async move {
            let command_snapshot = lock_coordinator(&service.control_plane).snapshot();
            if let Ok(snapshot) = service.rebuild_product_snapshot(command_snapshot).await {
                let _ = service.updates.send(snapshot);
            }
        });
    }
}

fn propagate_subagent_preferences(_preferences: &ExternalSourcesConfig) {
    for service in workspace_services().iter() {
        let Some(service) = service.value().upgrade() else {
            continue;
        };
        tokio::spawn(async move {
            let command_snapshot = lock_coordinator(&service.control_plane).snapshot();
            if let Ok(snapshot) = service.rebuild_product_snapshot(command_snapshot).await {
                let _ = service.updates.send(snapshot);
            }
        });
    }
}

fn propagate_mcp_preferences(_preferences: &ExternalSourcesConfig) {
    for service in workspace_services().iter() {
        let Some(service) = service.value().upgrade() else {
            continue;
        };
        tokio::spawn(async move {
            let command_snapshot = lock_coordinator(&service.control_plane).snapshot();
            if let Ok(snapshot) = service.rebuild_product_snapshot(command_snapshot).await {
                let _ = service.updates.send(snapshot);
            }
        });
    }
}

fn propagate_integration_policy_preferences(
    _preferences: &ExternalSourcesConfig,
    current: &Arc<WorkspaceExternalSourceService>,
) {
    for service in workspace_services().iter() {
        let Some(service) = service.value().upgrade() else {
            continue;
        };
        if Arc::ptr_eq(&service, current) {
            continue;
        }
        tokio::spawn(async move {
            if let Err(error) = service.refresh_preserving_worker_recovery().await {
                log::warn!(
                    "Could not apply external integration policy update scope={} error_category={}",
                    external_log_scope(service.workspace_root.as_deref()),
                    external_log_error_category(&error),
                );
            }
        });
    }
}

pub(crate) fn notify_external_tool_registry_changed() {
    TOOL_REGISTRY_CHANGE_EPOCH.fetch_add(1, Ordering::AcqRel);
    if TOOL_REGISTRY_REBUILD_SCHEDULED.swap(true, Ordering::AcqRel) {
        return;
    }
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        TOOL_REGISTRY_REBUILD_SCHEDULED.store(false, Ordering::Release);
        return;
    };
    runtime.spawn(async move {
        loop {
            let observed_epoch = TOOL_REGISTRY_CHANGE_EPOCH.load(Ordering::Acquire);
            let services = workspace_services()
                .iter()
                .filter_map(|entry| entry.value().upgrade())
                .collect::<Vec<_>>();
            for service in services {
                let command_snapshot = lock_coordinator(&service.control_plane).snapshot();
                if let Ok(snapshot) = service.rebuild_product_snapshot(command_snapshot).await {
                    let _ = service.updates.send(snapshot);
                }
            }
            if TOOL_REGISTRY_CHANGE_EPOCH.load(Ordering::Acquire) != observed_epoch {
                continue;
            }
            TOOL_REGISTRY_REBUILD_SCHEDULED.store(false, Ordering::Release);
            if TOOL_REGISTRY_CHANGE_EPOCH.load(Ordering::Acquire) == observed_epoch {
                break;
            }
            if TOOL_REGISTRY_REBUILD_SCHEDULED.swap(true, Ordering::AcqRel) {
                break;
            }
        }
    });
}

async fn sync_service_preferences(service: &WorkspaceExternalSourceService) -> Result<(), String> {
    let preferences = read_external_sources_config().await?;
    let policy = integration_policy_snapshot(&preferences, service.workspace_root.as_deref())?;
    let suppressed_sources = preferences
        .suppressed_source_keys
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let command_changed = {
        let mut coordinator = lock_coordinator(&service.control_plane);
        let mut changed = false;
        if coordinator.suppressed_sources() != &suppressed_sources {
            coordinator.replace_suppressed_sources(suppressed_sources.clone());
            changed = true;
        }
        if coordinator.conflict_choices() != &preferences.conflict_choices {
            coordinator.replace_conflict_choices(preferences.conflict_choices.clone());
            changed = true;
        }
        if coordinator.conflict_lineage_current_keys() != &preferences.conflict_lineage_current_keys
        {
            coordinator.replace_conflict_lineage_current_keys(
                preferences.conflict_lineage_current_keys.clone(),
            );
            changed = true;
        }
        if coordinator.conflicted_candidate_ids() != &preferences.conflicted_candidate_ids {
            coordinator
                .replace_conflicted_candidate_ids(preferences.conflicted_candidate_ids.clone());
            changed = true;
        }
        changed
    };
    let tool_changed = {
        let mut coordinator = lock_tool_coordinator(&service.control_plane);
        if coordinator.suppressed_sources() != &suppressed_sources {
            coordinator.replace_suppressed_sources(suppressed_sources.clone());
            true
        } else {
            false
        }
    };
    let subagent_changed = {
        let mut coordinator = lock_subagent_coordinator(&service.control_plane);
        if coordinator.suppressed_sources() != &suppressed_sources {
            coordinator.replace_suppressed_sources(suppressed_sources.clone());
            true
        } else {
            false
        }
    };
    let mcp_changed = {
        let mut coordinator = lock_mcp_coordinator(&service.control_plane);
        if coordinator.suppressed_sources() != &suppressed_sources {
            coordinator.replace_suppressed_sources(suppressed_sources.clone());
            true
        } else {
            false
        }
    };
    let subagent_preferences_changed =
        service.snapshot().preference_revision != preferences.preference_revision;
    let policy_changed = service.snapshot().integration_policy != policy;
    if command_changed
        || tool_changed
        || subagent_changed
        || mcp_changed
        || subagent_preferences_changed
        || policy_changed
    {
        let command_snapshot = lock_coordinator(&service.control_plane).snapshot();
        let snapshot = service.rebuild_product_snapshot(command_snapshot).await?;
        let _ = service.updates.send(snapshot);
    }
    service.ensure_watch_roots(&policy).await;
    Ok(())
}

fn validate_conflict_preference(conflict_key: &str, candidate_id: &str) -> Result<(), String> {
    if conflict_key.is_empty() || conflict_key.len() > 512 {
        return Err(invalid_operation_error(
            "External source conflict key is invalid",
        ));
    }
    if candidate_id.is_empty() || candidate_id.len() > 512 {
        return Err(invalid_operation_error(
            "External source conflict candidate is invalid",
        ));
    }
    Ok(())
}

fn validate_subagent_decision_value(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(invalid_operation_error(format!(
            "External subagent {label} is invalid"
        )));
    }
    Ok(())
}

fn validate_mcp_decision_value(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(invalid_operation_error(format!(
            "External MCP {label} is invalid"
        )));
    }
    Ok(())
}

pub(super) fn external_mcp_decision_allowed(
    state: &ExternalMcpActivationState,
    approved: bool,
) -> bool {
    if matches!(
        state,
        ExternalMcpActivationState::Conflict
            | ExternalMcpActivationState::Covered { .. }
            | ExternalMcpActivationState::SourceDisabled
            | ExternalMcpActivationState::Unsupported { .. }
            | ExternalMcpActivationState::Removed
    ) {
        return false;
    }
    !approved
        || !matches!(
            state,
            ExternalMcpActivationState::Starting
                | ExternalMcpActivationState::RuntimeUnavailable { .. }
        )
}

pub async fn external_source_conflict_choices() -> Result<
    (
        BTreeMap<String, String>,
        BTreeMap<String, String>,
        BTreeSet<String>,
    ),
    String,
> {
    let preferences = read_external_sources_config().await?;
    Ok((
        preferences.conflict_choices,
        preferences.conflict_lineage_current_keys,
        preferences.conflicted_candidate_ids,
    ))
}

pub async fn remember_external_source_conflict_choice(
    conflict_key: &str,
    candidate_id: &str,
    participants: Vec<String>,
    expected_preference_revision: u64,
) -> Result<
    (
        BTreeMap<String, String>,
        BTreeMap<String, String>,
        BTreeSet<String>,
        u64,
    ),
    String,
> {
    validate_conflict_preference(conflict_key, candidate_id)?;
    if participants.is_empty()
        || !participants
            .iter()
            .any(|candidate| candidate == candidate_id)
        || participants
            .iter()
            .any(|candidate| validate_conflict_preference(conflict_key, candidate).is_err())
    {
        return Err(invalid_operation_error(
            "External source conflict participants are invalid",
        ));
    }
    let preferences = persist_conflict_choice(
        conflict_key,
        candidate_id,
        participants,
        expected_preference_revision,
    )
    .await?;
    propagate_conflict_preferences(&preferences);
    Ok((
        preferences.conflict_choices,
        preferences.conflict_lineage_current_keys,
        preferences.conflicted_candidate_ids,
        preferences.preference_revision,
    ))
}

pub async fn set_external_prompt_command_conflict_choice(
    workspace_root: Option<&Path>,
    conflict_key: &str,
    candidate_id: &str,
    expected_preference_revision: u64,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    validate_conflict_preference(conflict_key, candidate_id)?;
    service_for(workspace_root)
        .await?
        .set_conflict_choice(conflict_key, candidate_id, expected_preference_revision)
        .await
}

pub async fn set_external_tool_target_decision(
    workspace_root: Option<&Path>,
    approval_key: &str,
    decision_key: &str,
    approved: bool,
    expected_preference_revision: u64,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    service_for(workspace_root)
        .await?
        .set_tool_target_decision(
            approval_key,
            decision_key,
            approved,
            expected_preference_revision,
        )
        .await
}

pub async fn set_external_tool_conflict_choice(
    workspace_root: Option<&Path>,
    conflict_key: &str,
    candidate_id: &str,
    expected_preference_revision: u64,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    service_for(workspace_root)
        .await?
        .set_tool_conflict_choice(conflict_key, candidate_id, expected_preference_revision)
        .await
}

pub async fn set_external_mcp_server_decision(
    workspace_root: Option<&Path>,
    candidate_id: &str,
    decision_key: &str,
    approved: bool,
    expected_mcp_generation: u64,
    expected_preference_revision: u64,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    service_for(workspace_root)
        .await?
        .set_mcp_server_decision(
            candidate_id,
            decision_key,
            approved,
            expected_mcp_generation,
            expected_preference_revision,
        )
        .await
}

pub async fn choose_external_mcp_conflict(
    workspace_root: Option<&Path>,
    conflict_key: &str,
    candidate_id: &str,
    approve_external: bool,
    expected_mcp_generation: u64,
    expected_preference_revision: u64,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    service_for(workspace_root)
        .await?
        .choose_mcp_conflict(
            conflict_key,
            candidate_id,
            approve_external,
            expected_mcp_generation,
            expected_preference_revision,
        )
        .await
}

pub async fn set_external_subagent_activation(
    workspace_root: Option<&Path>,
    candidate_id: &str,
    approved: bool,
    expected_subagent_generation: u64,
    expected_preference_revision: u64,
    decision_key: &str,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    service_for(workspace_root)
        .await?
        .set_subagent_activation(
            candidate_id,
            approved,
            expected_subagent_generation,
            expected_preference_revision,
            decision_key,
        )
        .await
}

pub async fn choose_external_subagent_conflict(
    workspace_root: Option<&Path>,
    conflict_key: &str,
    candidate_id: &str,
    approve_external: bool,
    expected_subagent_generation: u64,
    expected_preference_revision: u64,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    service_for(workspace_root)
        .await?
        .choose_subagent_conflict(
            conflict_key,
            candidate_id,
            approve_external,
            expected_subagent_generation,
            expected_preference_revision,
        )
        .await
}

pub async fn external_source_snapshot(
    workspace_root: Option<&Path>,
    force_refresh: bool,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    let service = service_for(workspace_root).await?;
    if force_refresh {
        service.refresh().await
    } else {
        service.ensure_background_refresh();
        Ok(service.snapshot())
    }
}

/// Resolves an opaque source identity to its host-local location for a host
/// action. The raw path must not be serialized into the public snapshot.
pub async fn external_source_location_for_host_action(
    workspace_root: Option<&Path>,
    stable_key: &str,
) -> Result<PathBuf, String> {
    service_for(workspace_root)
        .await?
        .source_location(stable_key)
}

pub async fn get_external_source_control_snapshot(
    workspace_root: Option<&Path>,
    force_refresh: bool,
    host_capabilities: ExternalSourceHostCapabilities,
) -> ExternalSourceOperationResult<ExternalSourceSurfaceSnapshotV1> {
    use bitfun_product_domains::external_source_control::ExternalSourceOperationStage;

    let service = if host_capabilities.can_execute_external_assets {
        service_for(workspace_root).await
    } else {
        read_only_service_for(workspace_root).await
    }
    .map_err(|error| {
        sanitize_external_source_operation_error(error)
            .with_stage(ExternalSourceOperationStage::ProjectResponse)
    })?;
    if force_refresh {
        service.refresh().await.map_err(|error| {
            sanitize_external_source_operation_error(error)
                .with_stage(ExternalSourceOperationStage::Discover)
        })?;
    } else {
        service.ensure_background_refresh();
    }
    Ok(service.surface_snapshot(host_capabilities))
}

pub async fn apply_external_source_control_action(
    workspace_root: Option<&Path>,
    request: ExternalSourceControlRequestV1,
) -> ExternalSourceOperationResult<ExternalSourceSurfaceSnapshotV1> {
    use bitfun_product_domains::external_source_control::ExternalSourceOperationStage;

    let operation_id = request.operation_id.clone();
    let service = service_for(workspace_root).await.map_err(|error| {
        typed_control_operation_error(
            error,
            &operation_id,
            ExternalSourceOperationStage::ValidateRequest,
        )
    })?;
    service.apply_control_action(request).await
}

/// Returns a static, sanitized projection for Hosts that may inspect external
/// configuration but must never load external code or alter runtime routes.
pub async fn external_source_read_only_snapshot(
    workspace_root: Option<&Path>,
    force_refresh: bool,
) -> Result<ExternalSourcePublicSnapshot, String> {
    let service = read_only_service_for(workspace_root).await?;
    let snapshot = if force_refresh {
        service.refresh().await?
    } else {
        service.ensure_background_refresh();
        service.snapshot()
    };
    let mut public = ExternalSourcePublicSnapshot::from(snapshot);
    public.host_capabilities = ExternalSourceHostCapabilities::read_only_projection();
    Ok(public)
}

pub async fn update_external_integration_policy(
    workspace_root: Option<&Path>,
    mutation: ExternalIntegrationPolicyMutation,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    let expected_revision = mutation.expected_preference_revision;
    let (scope, operation, ecosystem, capability) = integration_policy_log_context(&mutation);
    let result = match service_for(workspace_root).await {
        Ok(service) => service.update_integration_policy(mutation).await,
        Err(error) => Err(error),
    };
    match &result {
        Ok(snapshot) => log::info!(
            "External integration policy mutation outcome=success scope={} operation={} ecosystem={} capability={} revision={} changed={}",
            scope,
            operation,
            ecosystem,
            capability,
            snapshot.preference_revision,
            snapshot.preference_revision != expected_revision,
        ),
        Err(error) => log::warn!(
            "External integration policy mutation outcome=failure scope={} operation={} ecosystem={} capability={} expected_revision={} error_code={}",
            scope,
            operation,
            ecosystem,
            capability,
            expected_revision,
            external_integration_error_code(error),
        ),
    }
    result
}

fn integration_policy_log_context(
    mutation: &ExternalIntegrationPolicyMutation,
) -> (&'static str, &'static str, String, String) {
    let scope = match mutation.scope {
        ExternalIntegrationPolicyScope::User => "user",
        ExternalIntegrationPolicyScope::Workspace => "workspace",
        _ => "unknown",
    };
    let (operation, ecosystem, capability) = match &mutation.change {
        ExternalIntegrationPolicyOperation::SetEnabled { .. } => {
            ("set_enabled", "all".to_string(), "all".to_string())
        }
        ExternalIntegrationPolicyOperation::SetEcosystemMode { ecosystem_id, .. } => (
            "set_ecosystem_mode",
            safe_external_log_token(ecosystem_id.as_str()),
            "all".to_string(),
        ),
        ExternalIntegrationPolicyOperation::SetCapabilityAccess {
            ecosystem_id,
            capability_id,
            ..
        } => (
            "set_capability_access",
            safe_external_log_token(ecosystem_id.as_str()),
            safe_external_log_token(capability_id.as_str()),
        ),
        ExternalIntegrationPolicyOperation::ResetWorkspace => {
            ("reset_workspace", "all".to_string(), "all".to_string())
        }
        ExternalIntegrationPolicyOperation::ResetIncompatiblePolicy => (
            "reset_incompatible_policy",
            "all".to_string(),
            "all".to_string(),
        ),
        _ => ("unknown", "unknown".to_string(), "unknown".to_string()),
    };
    (scope, operation, ecosystem, capability)
}

fn safe_external_log_token(value: &str) -> String {
    value
        .chars()
        .take(64)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn external_log_scope(workspace_root: Option<&Path>) -> &'static str {
    if workspace_root.is_some() {
        "workspace"
    } else {
        "user-global"
    }
}

fn external_log_error_category(error: &str) -> String {
    ExternalSourceOperationError::decode(error)
        .map(|typed| typed.code.as_str().to_string())
        .unwrap_or_else(|| "internal".to_string())
}

/// Converts legacy internal failures at the product boundary without deriving
/// control flow from prose. Callers may pass an exactly encoded shared error;
/// every other failure becomes a sanitized internal error with a correlation
/// id, while the local log retains only a bounded category token.
pub fn sanitize_external_source_operation_error(error: String) -> ExternalSourceOperationError {
    if let Some(typed) = ExternalSourceOperationError::decode(&error) {
        return typed.with_default_recovery_actions();
    }
    static NEXT_CORRELATION_ID: AtomicU64 = AtomicU64::new(1);
    let correlation_id = format!(
        "external-source-{}-{}",
        epoch_seconds(),
        NEXT_CORRELATION_ID.fetch_add(1, Ordering::Relaxed)
    );
    log::error!(
        "External source operation failed correlation_id={} category={}",
        correlation_id,
        external_log_error_category(&error),
    );
    ExternalSourceOperationError::new(
        ExternalSourceOperationErrorCode::Internal,
        "External source operation failed. Retry, then use the reference id if the problem continues.",
        true,
    )
    .with_correlation_id(correlation_id)
    .with_default_recovery_actions()
}

fn typed_control_operation_error(
    error: String,
    operation_id: &str,
    stage: bitfun_product_domains::external_source_control::ExternalSourceOperationStage,
) -> ExternalSourceOperationError {
    let mut typed = sanitize_external_source_operation_error(error);
    if typed.correlation_id.is_none() {
        typed = typed.with_correlation_id(operation_id);
    } else if typed.causation_id.is_none() {
        typed = typed.with_causation_id(operation_id);
    }
    if typed.stage.is_none() {
        typed = typed.with_stage(stage);
    }
    typed
}

fn encoded_operation_error(
    code: ExternalSourceOperationErrorCode,
    detail: impl Into<String>,
    retryable: bool,
) -> String {
    ExternalSourceOperationError::new(code, detail, retryable).encode()
}

fn stale_operation_error(detail: impl Into<String>) -> String {
    encoded_operation_error(
        ExternalSourceOperationErrorCode::StaleRevision,
        detail,
        true,
    )
}

fn missing_candidate_error(detail: impl Into<String>) -> String {
    encoded_operation_error(ExternalSourceOperationErrorCode::NotFound, detail, false)
}

fn policy_limited_error(detail: impl Into<String>) -> String {
    encoded_operation_error(
        ExternalSourceOperationErrorCode::PolicyLimited,
        detail,
        false,
    )
}

fn conflict_operation_error(detail: impl Into<String>) -> String {
    encoded_operation_error(ExternalSourceOperationErrorCode::Conflict, detail, true)
}

fn unavailable_operation_error(detail: impl Into<String>) -> String {
    encoded_operation_error(ExternalSourceOperationErrorCode::Unavailable, detail, true)
}

fn invalid_operation_error(detail: impl Into<String>) -> String {
    encoded_operation_error(
        ExternalSourceOperationErrorCode::InvalidRequest,
        detail,
        false,
    )
}

fn incompatible_policy_error(detail: impl Into<String>) -> String {
    encoded_operation_error(
        ExternalSourceOperationErrorCode::PolicyIncompatible,
        detail,
        false,
    )
}

fn external_integration_error_code(error: &str) -> String {
    ExternalSourceOperationError::decode(error)
        .map(|error| error.code.as_str().to_string())
        .unwrap_or_else(|| "internal".to_string())
}

/// Keep the external-source runtime aligned with an actively assembled product
/// tool catalog. A newly created service performs one synchronous refresh so an
/// idle-retired workspace can restore approved routes before the catalog is
/// exposed to the model. Existing services are only touched; file watchers and
/// explicit refreshes remain responsible for later source changes.
pub(crate) async fn ensure_external_source_workspace_runtime(workspace_root: Option<&Path>) {
    let service = match service_for(workspace_root).await {
        Ok(service) => service,
        Err(error) => {
            log::warn!(
                "Could not retain external source workspace runtime scope={} error_category={}",
                external_log_scope(workspace_root),
                external_log_error_category(&error),
            );
            return;
        }
    };
    if let Err(error) = service.ensure_initial_refresh().await {
        log::warn!(
            "Could not initialize external source workspace runtime scope={} error_category={}",
            external_log_scope(workspace_root),
            external_log_error_category(&error),
        );
        return;
    }
    if external_tool_workspace_requires_recovery(workspace_root).await {
        if let Err(error) = service.refresh_worker_loss_once().await {
            log::warn!(
                "Could not recover external source tool runtime scope={} error_category={}",
                external_log_scope(workspace_root),
                external_log_error_category(&error),
            );
        }
    }
}

pub async fn set_external_source_enabled(
    workspace_root: Option<&Path>,
    source_key: &str,
    enabled: bool,
    expected_preference_revision: u64,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    service_for(workspace_root)
        .await?
        .set_source_enabled(source_key, enabled, expected_preference_revision)
        .await
}

pub async fn expand_external_prompt_command(
    workspace_root: Option<&Path>,
    name: &str,
    arguments: &str,
    expected_candidate_id: Option<&str>,
    expected_content_version: Option<&str>,
) -> Result<ExpandedPromptCommand, String> {
    service_for(workspace_root)
        .await?
        .expand_command(
            name,
            arguments,
            expected_candidate_id,
            expected_content_version,
        )
        .await
}

pub async fn subscribe_external_source_updates(
    workspace_root: Option<&Path>,
) -> Result<ExternalSourceSubscription, String> {
    let service = service_for(workspace_root).await?;
    let receiver = service.updates.subscribe();
    service.ensure_background_refresh();
    Ok(ExternalSourceSubscription {
        _service: service,
        receiver,
    })
}

pub struct ExternalSourceSubscription {
    _service: Arc<WorkspaceExternalSourceService>,
    receiver: broadcast::Receiver<ExternalSourceCatalogSnapshot>,
}

impl ExternalSourceSubscription {
    pub fn try_recv(
        &mut self,
    ) -> Result<ExternalSourceCatalogSnapshot, broadcast::error::TryRecvError> {
        self.receiver.try_recv()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_product_domains::external_sources::{
        EcosystemId, ExternalSourceProviderError, ExternalSourceRecord, ExternalSourceScope,
        PromptCommandAvailability, PromptCommandDefinition, PromptCommandProviderIdentity,
        PromptCommandProviderSnapshot, SourceQualifiedCommandId,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn only_model_configuration_events_refresh_external_model_bindings() {
        assert!(config_update_refreshes_external_model_bindings(
            &ConfigUpdateEvent::ModelConfigurationUpdated
        ));
        assert!(!config_update_refreshes_external_model_bindings(
            &ConfigUpdateEvent::ThemeUpdated {
                theme_id: "dark".to_string(),
            }
        ));
    }

    #[test]
    fn integration_error_metrics_decode_typed_codes_without_parsing_prose() {
        let stale = stale_operation_error("preferences changed");
        assert_eq!(external_integration_error_code(&stale), "stale_revision");
        assert_eq!(
            external_integration_error_code("legacy internal failure: private detail"),
            "internal"
        );
    }

    #[test]
    fn background_log_categories_never_include_error_details_or_paths() {
        let stale = stale_operation_error("private workspace path changed");
        assert_eq!(external_log_error_category(&stale), "stale_revision");

        for raw in [
            r"directory_read_failed: C:\Users\alice\.config\opencode",
            "Failed to watch path /home/alice/.config/opencode: permission denied",
        ] {
            let category = external_log_error_category(raw);
            assert_eq!(category, "internal");
            assert!(!category.contains("alice"));
            assert!(!category.contains("opencode"));
        }
        assert_eq!(external_log_scope(Some(Path::new("C:/repo"))), "workspace");
        assert_eq!(external_log_scope(None), "user-global");
    }

    #[test]
    fn final_catalog_redacts_known_absolute_paths_from_diagnostics() {
        let source_key = SourceKey::new("future.tools", "project").unwrap();
        let raw_root = r"C:\Users\alice\repo\.future-ai\tools";
        let raw_file = format!(r"{raw_root}\review.js");
        let mut snapshot = ExternalSourceCatalogSnapshot {
            generation: 1,
            discovery_pending: false,
            sources: vec![ExternalSourceCatalogEntry {
                stable_key: source_key.stable_key(),
                presentation_group_id: None,
                record: ExternalSourceRecord {
                    key: source_key.clone(),
                    ecosystem_id: EcosystemId::new("future-ai").unwrap(),
                    display_name: "Future AI project tools".to_string(),
                    source_kind: "standalone_tools".to_string(),
                    scope: ExternalSourceScope::Project,
                    location: raw_root.to_string(),
                    execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
                    health: bitfun_product_domains::external_sources::ExternalSourceHealth::Partial,
                    content_version: "source-v1".to_string(),
                    diagnostics: vec![ExternalSourceDiagnostic::warning(
                        "future.tool.directory_read_failed",
                        format!("Failed to read '{raw_root}'"),
                        Some(source_key.clone()),
                    )
                    .with_asset_kind(ExternalSourceAssetKind::Tool)],
                },
                lifecycle: ExternalSourceLifecycleState::Degraded,
            }],
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
            integration_policy: Default::default(),
            diagnostics: vec![ExternalSourceDiagnostic::warning(
                "future.tool.file_read_failed",
                format!("Failed to read '{raw_file}'"),
                Some(source_key),
            )
            .with_asset_kind(ExternalSourceAssetKind::Tool)],
        };

        sanitize_external_snapshot_locations(
            &mut snapshot,
            Some(Path::new(r"C:\Users\alice\repo")),
        );

        assert_eq!(
            snapshot.sources[0].record.location,
            "<workspace>/.future-ai/tools"
        );
        assert_eq!(
            snapshot.sources[0].record.diagnostics[0].message,
            "Failed to read '<workspace>/.future-ai/tools'"
        );
        assert_eq!(
            snapshot.diagnostics[0].message,
            "Failed to read '<workspace>/.future-ai/tools/review.js'"
        );
        let serialized = serde_json::to_string(&snapshot).unwrap();
        assert!(!serialized.contains("C:\\\\Users\\\\alice"));
        assert!(!serialized.contains("C:/Users/alice"));
    }

    #[test]
    fn presentation_groups_are_assigned_before_location_redaction() {
        let make_source = |provider_id: &str, stable_key: &str, location: &str| {
            let source_key = SourceKey::new(provider_id, "user-configuration").unwrap();
            ExternalSourceCatalogEntry {
                stable_key: stable_key.to_string(),
                presentation_group_id: None,
                record: ExternalSourceRecord {
                    key: source_key,
                    ecosystem_id: EcosystemId::new("opencode").unwrap(),
                    display_name: "OpenCode user configuration".to_string(),
                    source_kind: "configuration".to_string(),
                    scope: ExternalSourceScope::RemoteUser,
                    location: location.to_string(),
                    execution_domain_id: ExecutionDomainId::new("peer-a").unwrap(),
                    health:
                        bitfun_product_domains::external_sources::ExternalSourceHealth::Available,
                    content_version: "source-v1".to_string(),
                    diagnostics: Vec::new(),
                },
                lifecycle: ExternalSourceLifecycleState::Available,
            }
        };
        let mut snapshot = ExternalSourceCatalogSnapshot {
            generation: 0,
            discovery_pending: false,
            sources: vec![
                make_source(
                    "opencode.commands",
                    "command-source",
                    "/remote/alice/.config/opencode/opencode.json",
                ),
                make_source(
                    "opencode.subagents",
                    "agent-source",
                    "/remote/alice/.config/opencode/opencode.json",
                ),
                make_source(
                    "opencode.mcp",
                    "other-user-source",
                    "/remote/bob/.config/opencode/opencode.json",
                ),
            ],
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
            integration_policy: Default::default(),
            diagnostics: Vec::new(),
        };

        assign_external_source_presentation_groups(&mut snapshot);
        sanitize_external_snapshot_locations(&mut snapshot, None);

        assert_eq!(
            snapshot.sources[0].presentation_group_id,
            snapshot.sources[1].presentation_group_id,
        );
        assert_ne!(
            snapshot.sources[0].presentation_group_id,
            snapshot.sources[2].presentation_group_id,
        );
        assert!(snapshot
            .sources
            .iter()
            .all(|source| source.record.location == "<remote>/.config/opencode/opencode.json"));
    }

    #[test]
    fn reveal_location_uses_stable_identity_and_keeps_the_raw_path_private() {
        let raw_location = std::env::current_dir()
            .unwrap()
            .join(".opencode")
            .join("opencode.json");
        let source = ExternalSourceCatalogEntry {
            stable_key: "opencode-project".to_string(),
            presentation_group_id: Some("opencode-project".to_string()),
            record: ExternalSourceRecord {
                key: SourceKey::new("opencode.commands", "project").unwrap(),
                ecosystem_id: EcosystemId::new("opencode").unwrap(),
                display_name: "OpenCode project configuration".to_string(),
                source_kind: "configuration".to_string(),
                scope: ExternalSourceScope::Project,
                location: raw_location.to_string_lossy().into_owned(),
                execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
                health: bitfun_product_domains::external_sources::ExternalSourceHealth::Available,
                content_version: "source-v1".to_string(),
                diagnostics: Vec::new(),
            },
            lifecycle: ExternalSourceLifecycleState::Available,
        };

        let resolved =
            resolve_external_source_location("opencode-project", std::iter::once(&source)).unwrap();

        assert_eq!(resolved, raw_location);
        assert!(resolve_external_source_location(
            "<workspace>/.opencode",
            std::iter::once(&source)
        )
        .is_err());
    }

    struct DelayedProvider {
        identity: PromptCommandProviderIdentity,
        source: SourceKey,
        command_name: String,
        delay: std::time::Duration,
        calls: Arc<AtomicUsize>,
    }

    impl PromptCommandSourceProvider for DelayedProvider {
        fn identity(&self) -> PromptCommandProviderIdentity {
            self.identity.clone()
        }

        fn discover(
            &self,
            context: &ExternalSourceContext,
        ) -> Result<PromptCommandProviderSnapshot, ExternalSourceProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(self.delay);
            let record = ExternalSourceRecord {
                key: self.source.clone(),
                ecosystem_id: self.identity.ecosystem_id.clone(),
                display_name: self.identity.display_name.clone(),
                source_kind: "prompt_commands".to_string(),
                scope: ExternalSourceScope::UserGlobal,
                location: format!("/{}", self.command_name),
                execution_domain_id: context.execution_domain_id.clone(),
                health: bitfun_product_domains::external_sources::ExternalSourceHealth::Available,
                content_version: "source-v1".to_string(),
                diagnostics: Vec::new(),
            };
            Ok(PromptCommandProviderSnapshot {
                provider: self.identity.clone(),
                sources: vec![record],
                commands: vec![PromptCommandDefinition {
                    id: SourceQualifiedCommandId::new(
                        self.source.clone(),
                        self.command_name.clone(),
                    )
                    .unwrap(),
                    name: self.command_name.clone(),
                    description: self.command_name.clone(),
                    template: self.command_name.clone(),
                    availability: PromptCommandAvailability::Available,
                    content_version: "command-v1".to_string(),
                }],
                unavailable_command_ids: Vec::new(),
                diagnostics: Vec::new(),
            })
        }

        fn expand(
            &self,
            command: &PromptCommandDefinition,
            _arguments: &str,
        ) -> Result<ExpandedPromptCommand, ExternalSourceProviderError> {
            Ok(ExpandedPromptCommand {
                content: command.template.clone(),
            })
        }

        fn watch_roots(
            &self,
            _context: &ExternalSourceContext,
        ) -> Vec<bitfun_product_domains::external_sources::ExternalWatchRoot> {
            Vec::new()
        }
    }

    fn delayed_provider(
        id: &str,
        delay: std::time::Duration,
        calls: Arc<AtomicUsize>,
    ) -> Arc<dyn PromptCommandSourceProvider> {
        Arc::new(DelayedProvider {
            identity: PromptCommandProviderIdentity::new(id, id, id).unwrap(),
            source: SourceKey::new(id, "global").unwrap(),
            command_name: id.to_string(),
            delay,
            calls,
        })
    }

    fn test_service(
        providers: Vec<Arc<dyn PromptCommandSourceProvider>>,
    ) -> Arc<WorkspaceExternalSourceService> {
        let context = ExternalSourceContext {
            workspace_root: None,
            execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
        };
        let (updates, _) = broadcast::channel(8);
        let control_plane = Arc::new(
            ExternalSourceControlPlane::new(context, providers, Vec::new(), Vec::new(), Vec::new())
                .unwrap(),
        );
        let mut snapshot = merge_tool_state(
            control_plane.commands(|coordinator| coordinator.snapshot()),
            &control_plane.tools(|coordinator| coordinator.snapshot()),
            ExternalToolProductState::default(),
        );
        snapshot.integration_policy =
            integration_policy_snapshot(&ExternalSourcesConfig::default(), None)
                .expect("built-in integration policy is valid");
        Arc::new(WorkspaceExternalSourceService {
            profile: ExternalSourceServiceProfile::LocalExecution,
            workspace_root: None,
            execution_domain_id: ExecutionDomainId::new(LEGACY_LOCAL_EXECUTION_DOMAIN_ID).unwrap(),
            control_plane,
            snapshot: StdMutex::new(snapshot),
            updates,
            watch_states: tokio::sync::Mutex::new(BTreeMap::new()),
            refresh_gate: tokio::sync::Mutex::new(()),
            product_rebuild_gate: tokio::sync::Mutex::new(()),
            mcp_runtime: Arc::new(BitFunExternalMcpRuntime),
            active_mcp_runtime_ids: tokio::sync::Mutex::new(BTreeSet::new()),
            initial_refresh_completed: AtomicBool::new(false),
            background_refresh_scheduled: AtomicBool::new(false),
            initial_refresh_gate: tokio::sync::Mutex::new(()),
            keepalive_started: AtomicBool::new(false),
            last_access_epoch_seconds: AtomicU64::new(epoch_seconds()),
            watcher: Arc::new(FileWatchService::new(FileWatcherConfig::default())),
            tool_decision_gate_waiting: tokio::sync::Notify::new(),
            tool_decision_gate_acquired: tokio::sync::Notify::new(),
            subagent_expiry_schedule: AtomicU64::new(0),
        })
    }

    #[test]
    fn surface_snapshot_projects_control_and_legacy_catalog_from_one_generation() {
        let service = test_service(Vec::new());

        let surface = service.surface_snapshot(ExternalSourceHostCapabilities::read_write());

        assert_eq!(
            surface.control.refresh_generation,
            surface.catalog.generation
        );
        assert_eq!(surface.control.capabilities.len(), 4);
        assert!(!surface.control.safe_mode);
    }

    #[tokio::test]
    async fn safe_mode_keeps_commands_visible_and_retires_managed_executables() {
        let discovery_calls = Arc::new(AtomicUsize::new(0));
        let runtime = Arc::new(CountingExternalMcpRuntime::default());
        let workspace = tempfile::tempdir().unwrap();
        let mut service = test_service(vec![delayed_provider(
            "opencode",
            std::time::Duration::ZERO,
            discovery_calls,
        )]);
        let service_inner = Arc::get_mut(&mut service).expect("test owns the service");
        service_inner.workspace_root = Some(workspace.path().to_path_buf());
        service_inner.mcp_runtime = runtime.clone();

        let requests = lock_coordinator(&service.control_plane).discovery_requests();
        let batch = service
            .control_plane
            .discover_commands(requests, std::time::Duration::from_secs(1))
            .await;
        assert!(batch.deferred.is_empty());
        let mut catalog =
            lock_coordinator(&service.control_plane).apply_discovery_results(batch.immediate);
        catalog.integration_policy = service.snapshot().integration_policy;
        *lock_snapshot(&service.snapshot) = catalog;
        service
            .active_mcp_runtime_ids
            .lock()
            .await
            .insert("managed-mcp".to_string());

        let snapshot = service.set_safe_mode(true, None).await.unwrap();
        let surface = service.surface_snapshot(ExternalSourceHostCapabilities::read_write());

        assert!(surface.control.safe_mode);
        assert!(!surface.catalog.commands.is_empty());
        assert!(snapshot
            .tools
            .iter()
            .all(|tool| !matches!(tool.activation, ExternalToolActivationState::Active)));
        assert!(snapshot
            .mcp_servers
            .iter()
            .all(|server| server.runtime_id.is_none()));
        assert!(snapshot.subagents.iter().all(|agent| !matches!(
            agent.activation_state,
            ExternalSubagentActivationState::Active
        )));
        assert!(service.active_mcp_runtime_ids.lock().await.is_empty());
        assert!(runtime.calls.load(Ordering::SeqCst) >= 2);

        service.set_safe_mode(false, None).await.unwrap();
        assert!(
            !service
                .surface_snapshot(ExternalSourceHostCapabilities::read_write())
                .control
                .safe_mode
        );
    }

    #[tokio::test]
    async fn control_action_returns_typed_stale_revision_and_shared_surface() {
        let workspace = tempfile::tempdir().unwrap();
        let mut service = test_service(Vec::new());
        Arc::get_mut(&mut service)
            .expect("test owns the service")
            .workspace_root = Some(workspace.path().to_path_buf());
        let revision = read_external_sources_config()
            .await
            .unwrap()
            .preference_revision;
        lock_snapshot(&service.snapshot).preference_revision = revision;
        let stale_request = ExternalSourceControlRequestV1 {
            schema_version: EXTERNAL_SOURCE_CONTROL_SCHEMA_V1,
            operation_id: "safe-mode-stale".to_string(),
            expected_preference_revision: Some(revision.saturating_add(1)),
            action: ExternalSourceControlActionV1::SetSafeMode { enabled: true },
        };

        let error = service
            .apply_control_action(stale_request)
            .await
            .unwrap_err();

        assert_eq!(error.code, ExternalSourceOperationErrorCode::StaleRevision);
        assert_eq!(error.correlation_id.as_deref(), Some("safe-mode-stale"));
        assert_eq!(
            error.recovery_actions,
            vec![
                bitfun_product_domains::external_source_control::ExternalSourceRecoveryActionV1::Refresh,
            ]
        );
        assert_eq!(
            error.stage,
            Some(
                bitfun_product_domains::external_source_control::ExternalSourceOperationStage::ApplyPreference
            )
        );

        let response = service
            .apply_control_action(ExternalSourceControlRequestV1 {
                schema_version: EXTERNAL_SOURCE_CONTROL_SCHEMA_V1,
                operation_id: "safe-mode-on".to_string(),
                expected_preference_revision: Some(revision),
                action: ExternalSourceControlActionV1::SetSafeMode { enabled: true },
            })
            .await
            .unwrap();
        assert!(response.control.safe_mode);
        assert_eq!(
            response.control.refresh_generation,
            response.catalog.generation
        );

        service.set_safe_mode(false, None).await.unwrap();
    }

    #[derive(Default)]
    struct CountingExternalMcpRuntime {
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl ExternalMcpRuntimePort for CountingExternalMcpRuntime {
        async fn install(
            &self,
            _candidate: &crate::external_mcp::ActiveExternalMcpCandidate,
            _prepared: bitfun_product_domains::external_sources::PreparedExternalMcpServer,
            _workspace_key: &str,
        ) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn retire(&self, _runtime_id: &str) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn status(&self, _runtime_id: &str) -> Result<ExternalMcpRuntimeStatus, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ExternalMcpRuntimeStatus::Active)
        }

        async fn replace_workspace_route(
            &self,
            _workspace_key: &str,
            _active_external_server_ids: BTreeSet<String>,
            _suppressed_native_server_ids: BTreeSet<String>,
        ) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn read_only_projection_never_calls_the_mcp_runtime() {
        let runtime = Arc::new(CountingExternalMcpRuntime::default());
        let mut service = test_service(Vec::new());
        let service_inner = Arc::get_mut(&mut service).expect("test owns the service");
        service_inner.profile = ExternalSourceServiceProfile::ReadOnlyProjection;
        service_inner.mcp_runtime = runtime.clone();

        let preferences = ExternalSourcesConfig::default();
        let policy = integration_policy_snapshot(&preferences, None).unwrap();
        let command_snapshot = lock_coordinator(&service.control_plane).snapshot();
        let snapshot = service
            .rebuild_read_only_projection(command_snapshot, preferences, policy)
            .await
            .unwrap();

        assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
        assert!(snapshot
            .mcp_servers
            .iter()
            .all(|entry| entry.runtime_id.is_none()));
        assert!(snapshot
            .tools
            .iter()
            .all(|entry| { !matches!(entry.activation, ExternalToolActivationState::Active) }));
        assert!(snapshot.subagents.iter().all(|entry| {
            !matches!(
                entry.activation_state,
                ExternalSubagentActivationState::Active
            )
        }));
    }

    #[tokio::test]
    async fn preference_store_merges_updates_from_independent_instances() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("external-sources.json");
        let first = ExternalSourcePreferenceStore::new(path.clone());
        let second = ExternalSourcePreferenceStore::new(path);

        let disable = first.update(|config| {
            config
                .suppressed_source_keys
                .push("opencode:global".to_string());
        });
        let choose = second.update(|config| {
            ExternalSourceCoordinator::reconcile_conflict_preferences(
                &mut config.conflict_choices,
                &mut config.conflict_lineage_current_keys,
                &mut config.conflicted_candidate_ids,
                "prompt_command:local-user:review:v1",
                "candidate-a",
                &["candidate-a".to_string(), "candidate-b".to_string()],
            );
        });
        let (disabled, chosen) = tokio::join!(disable, choose);
        disabled.unwrap();
        chosen.unwrap();

        let persisted = first.read().await.unwrap();
        assert_eq!(persisted.suppressed_source_keys, ["opencode:global"]);
        assert_eq!(
            persisted
                .conflict_choices
                .get("prompt_command:local-user:review:v1")
                .map(String::as_str),
            Some("candidate-a")
        );
        assert_eq!(
            persisted.conflict_lineage_current_keys["prompt_command:local-user:review"],
            "prompt_command:local-user:review:v1"
        );
        assert_eq!(
            persisted.conflicted_candidate_ids,
            BTreeSet::from(["candidate-a".to_string(), "candidate-b".to_string()])
        );
    }

    #[test]
    fn opencode_registry_owns_low_friction_defaults_and_safety_ceilings() {
        let policy = integration_policy_snapshot(&ExternalSourcesConfig::default(), None)
            .expect("built-in policy is valid");
        let descriptor = policy
            .registered_ecosystems
            .iter()
            .find(|descriptor| descriptor.ecosystem_id.as_str() == OPENCODE_ECOSYSTEM_ID)
            .expect("OpenCode is registered by product assembly");

        for (capability_id, recommended, ceiling) in [
            (
                EXTERNAL_CAPABILITY_COMMAND,
                ExternalIntegrationAccess::Auto,
                ExternalIntegrationAccess::Auto,
            ),
            (
                EXTERNAL_CAPABILITY_TOOL,
                ExternalIntegrationAccess::AskBeforeUse,
                ExternalIntegrationAccess::AskBeforeUse,
            ),
            (
                EXTERNAL_CAPABILITY_SUBAGENT,
                ExternalIntegrationAccess::AskBeforeUse,
                ExternalIntegrationAccess::AskBeforeUse,
            ),
            (
                EXTERNAL_CAPABILITY_MCP,
                ExternalIntegrationAccess::AskBeforeUse,
                ExternalIntegrationAccess::AskBeforeUse,
            ),
        ] {
            let capability = descriptor
                .capabilities
                .iter()
                .find(|capability| capability.capability_id.as_str() == capability_id)
                .expect("built-in capability is registered");
            assert_eq!(capability.recommended_access, recommended);
            assert_eq!(capability.safety_ceiling, ceiling);
            assert_eq!(
                integration_access(&policy, OPENCODE_ECOSYSTEM_ID, capability_id),
                recommended
            );
        }
    }

    #[test]
    fn active_capability_sets_are_scoped_per_ecosystem_for_every_asset_kind() {
        let mut policy = integration_policy_snapshot(&ExternalSourcesConfig::default(), None)
            .expect("built-in policy is valid");
        let template_descriptor = policy.registered_ecosystems[0].clone();
        let template_effective = policy
            .effective
            .ecosystems
            .get(&template_descriptor.ecosystem_id)
            .cloned()
            .expect("built-in effective ecosystem exists");
        let discover_id = EcosystemId::new("discover.ecosystem").unwrap();
        let active_id = EcosystemId::new("active.ecosystem").unwrap();
        let mut discover_descriptor = template_descriptor.clone();
        discover_descriptor.ecosystem_id = discover_id.clone();
        discover_descriptor.display_name = "Discover ecosystem".to_string();
        let mut active_descriptor = template_descriptor;
        active_descriptor.ecosystem_id = active_id.clone();
        active_descriptor.display_name = "Active ecosystem".to_string();
        policy.registered_ecosystems = vec![discover_descriptor, active_descriptor];

        let mut discover_effective = template_effective.clone();
        discover_effective.ecosystem_id = discover_id.clone();
        for access in discover_effective.capabilities.values_mut() {
            *access = ExternalIntegrationAccess::DiscoverOnly;
        }
        let mut active_effective = template_effective;
        active_effective.ecosystem_id = active_id.clone();
        policy.effective.ecosystems = BTreeMap::from([
            (discover_id.clone(), discover_effective),
            (active_id.clone(), active_effective),
        ]);

        for capability in [
            EXTERNAL_CAPABILITY_COMMAND,
            EXTERNAL_CAPABILITY_TOOL,
            EXTERNAL_CAPABILITY_SUBAGENT,
            EXTERNAL_CAPABILITY_MCP,
        ] {
            assert_eq!(
                ecosystems_with_discoverable_capability(&policy, capability),
                BTreeSet::from([discover_id.clone(), active_id.clone()])
            );
            assert_eq!(
                ecosystems_with_active_capability(&policy, capability),
                BTreeSet::from([active_id.clone()])
            );
        }
    }

    #[test]
    fn ecosystem_registration_rejects_provider_and_capability_mismatches() {
        let mut incompatible_contract = default_external_integration_registry()
            .into_iter()
            .next()
            .expect("built-in registration exists");
        incompatible_contract.contract_major = EXTERNAL_ADAPTER_CONTRACT_MAJOR + 1;
        assert!(incompatible_contract
            .validate()
            .unwrap_err()
            .contains("contract major"));

        let mut wrong_ecosystem = default_external_integration_registry()
            .into_iter()
            .next()
            .expect("built-in registration exists");
        wrong_ecosystem.command_provider = Some(delayed_provider(
            "different.ecosystem",
            std::time::Duration::ZERO,
            Arc::new(AtomicUsize::new(0)),
        ));
        assert!(wrong_ecosystem
            .validate()
            .unwrap_err()
            .contains("different ecosystem"));

        let mut missing_provider = default_external_integration_registry()
            .into_iter()
            .next()
            .expect("built-in registration exists");
        missing_provider.command_provider = None;
        assert!(missing_provider
            .validate()
            .unwrap_err()
            .contains("provider registration do not match"));
    }

    #[test]
    fn integration_policy_mutations_share_revision_and_keep_workspace_paths_private() {
        let temp = tempfile::tempdir().unwrap();
        let workspace_key = workspace_policy_key(Some(temp.path())).expect("workspace has a key");
        assert!(workspace_key.starts_with("workspace:"));
        assert!(!workspace_key.contains(&temp.path().to_string_lossy().to_string()));

        let ecosystem_id =
            bitfun_product_domains::external_sources::EcosystemId::new(OPENCODE_ECOSYSTEM_ID)
                .unwrap();
        let mut config = ExternalSourcesConfig::default();
        let user_mutation = ExternalIntegrationPolicyMutation {
            expected_preference_revision: 0,
            scope: ExternalIntegrationPolicyScope::User,
            change: ExternalIntegrationPolicyOperation::SetEcosystemMode {
                ecosystem_id: ecosystem_id.clone(),
                mode: ExternalIntegrationMode::DiscoverOnly,
            },
        };
        assert!(
            apply_integration_policy_mutation_to_config(&mut config, None, &user_mutation,)
                .unwrap()
        );
        assert_eq!(config.preference_revision, 1);

        let stale = apply_integration_policy_mutation_to_config(&mut config, None, &user_mutation)
            .expect_err("old revisions cannot overwrite a newer policy");
        assert_eq!(
            ExternalSourceOperationError::decode(&stale)
                .expect("stale policy revisions use the typed error contract")
                .code,
            ExternalSourceOperationErrorCode::StaleRevision
        );

        let workspace_mutation = ExternalIntegrationPolicyMutation {
            expected_preference_revision: 1,
            scope: ExternalIntegrationPolicyScope::Workspace,
            change: ExternalIntegrationPolicyOperation::SetEcosystemMode {
                ecosystem_id: ecosystem_id.clone(),
                mode: ExternalIntegrationMode::Disabled,
            },
        };
        assert!(apply_integration_policy_mutation_to_config(
            &mut config,
            Some(&workspace_key),
            &workspace_mutation,
        )
        .unwrap());
        assert_eq!(config.preference_revision, 2);
        assert_eq!(
            config
                .integration_policy
                .known()
                .expect("the built-in policy schema is known")
                .workspace_overrides[&workspace_key]
                .ecosystems[&ecosystem_id]
                .mode,
            Some(ExternalIntegrationMode::Disabled)
        );
    }

    #[test]
    fn preference_document_preserves_future_minor_fields() {
        let raw = serde_json::json!({
            "integrationPolicy": {
                "schemaMajor": 1,
                "userDefaults": {
                    "enabled": true,
                    "futureSetting": "keep"
                },
                "futurePolicyField": { "revision": 2 }
            },
            "preferenceRevision": 4,
            "futurePreferenceField": ["keep"]
        });
        let mut config: ExternalSourcesConfig = serde_json::from_value(raw).unwrap();
        config.preference_revision += 1;
        let encoded = serde_json::to_value(config).unwrap();

        assert_eq!(
            encoded["integrationPolicy"]["userDefaults"]["futureSetting"],
            "keep"
        );
        assert_eq!(
            encoded["integrationPolicy"]["futurePolicyField"]["revision"],
            2
        );
        assert_eq!(encoded["futurePreferenceField"][0], "keep");
    }

    #[test]
    fn incompatible_policy_requires_explicit_reset_and_keeps_a_bounded_backup() {
        let future_policy = serde_json::json!({
            "schemaMajor": 13,
            "userDefaults": "future-policy-shape",
            "workspaceOverrides": ["also", "structurally", "different"],
            "futurePolicyField": { "schema": 13 }
        });
        let stored_future_policy: StoredExternalIntegrationPolicy =
            serde_json::from_value(future_policy.clone()).unwrap();
        let mut config = ExternalSourcesConfig {
            integration_policy: stored_future_policy,
            integration_policy_backups: vec![
                serde_json::json!({ "schemaMajor": 10, "opaque": "first" }),
                serde_json::json!({ "schemaMajor": 11, "opaque": "second" }),
                serde_json::json!({ "schemaMajor": 12, "opaque": "third" }),
            ],
            preference_revision: 7,
            ..ExternalSourcesConfig::default()
        };
        let public_snapshot = integration_policy_snapshot(&config, None).unwrap();
        assert_eq!(
            public_snapshot.status,
            ExternalIntegrationPolicyStatus::IncompatibleSchema
        );
        assert!(!public_snapshot.global_effective.enabled);
        assert!(!public_snapshot.effective.enabled);
        let serialized_snapshot = serde_json::to_value(&public_snapshot).unwrap();
        assert!(!serialized_snapshot
            .to_string()
            .contains("future-policy-shape"));
        assert!(!serialized_snapshot
            .to_string()
            .contains("futurePolicyField"));

        config
            .suppressed_source_keys
            .push("opencode:project".to_string());
        let persisted = serde_json::to_value(&config).unwrap();
        config = serde_json::from_value(persisted).unwrap();
        assert_eq!(config.integration_policy.raw_value(), future_policy);
        assert_eq!(config.suppressed_source_keys, ["opencode:project"]);

        let ordinary_mutation = ExternalIntegrationPolicyMutation {
            expected_preference_revision: 7,
            scope: ExternalIntegrationPolicyScope::User,
            change: ExternalIntegrationPolicyOperation::SetEnabled { enabled: false },
        };
        let error =
            apply_integration_policy_mutation_to_config(&mut config, None, &ordinary_mutation)
                .expect_err("future schemas cannot be edited by an older host");
        assert_eq!(
            ExternalSourceOperationError::decode(&error)
                .expect("incompatible schemas use the typed error contract")
                .code,
            ExternalSourceOperationErrorCode::PolicyIncompatible
        );
        assert_eq!(config.preference_revision, 7);
        assert_eq!(config.integration_policy.schema_major(), 13);

        let reset = ExternalIntegrationPolicyMutation {
            expected_preference_revision: 7,
            scope: ExternalIntegrationPolicyScope::User,
            change: ExternalIntegrationPolicyOperation::ResetIncompatiblePolicy,
        };
        assert!(apply_integration_policy_mutation_to_config(&mut config, None, &reset).unwrap());
        assert_eq!(config.preference_revision, 8);
        assert_eq!(
            config.integration_policy.schema_major(),
            EXTERNAL_INTEGRATION_POLICY_SCHEMA_MAJOR
        );
        assert!(
            !integration_policy_snapshot(&config, None)
                .unwrap()
                .effective
                .enabled
        );
        assert_eq!(
            config
                .integration_policy_backups
                .iter()
                .map(|document| document["schemaMajor"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            vec![11, 12, 13]
        );
        assert_eq!(config.integration_policy_backups[2], future_policy);
    }

    #[tokio::test]
    async fn subagent_conflict_history_advances_revision_and_rejects_stale_process_actions() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("external-sources.json");
        let process_a = ExternalSourcePreferenceStore::new(path.clone());
        let process_b = ExternalSourcePreferenceStore::new(path);
        let lineage = "external_subagent_lineage:local-user:workspace:review";
        let conflict_v1 = "external_subagent:local-user:workspace:review:v1";
        let conflict_v2 = "external_subagent:local-user:workspace:review:v2";

        let (changed, observed_v1) = persist_observed_subagent_conflicts_with_store(
            &process_a,
            &BTreeMap::from([(lineage.to_string(), conflict_v1.to_string())]),
        )
        .await
        .unwrap();
        assert!(changed);
        assert_eq!(observed_v1.preference_revision, 1);

        let selected_v1 = persist_subagent_conflict_choice_with_store(
            &process_a,
            conflict_v1,
            "external_subagent:candidate-v1",
            None,
            observed_v1.preference_revision,
        )
        .await
        .unwrap();
        assert_eq!(selected_v1.preference_revision, 2);

        let (changed, observed_v2) = persist_observed_subagent_conflicts_with_store(
            &process_b,
            &BTreeMap::from([(lineage.to_string(), conflict_v2.to_string())]),
        )
        .await
        .unwrap();
        assert!(changed);
        assert_eq!(observed_v2.preference_revision, 3);
        assert!(!observed_v2
            .subagent_conflict_choices
            .contains_key(conflict_v1));
        assert_eq!(
            observed_v2.subagent_conflict_choices[conflict_v2],
            SUBAGENT_CONFLICT_RESELECTION_REQUIRED
        );

        let error = persist_subagent_conflict_choice_with_store(
            &process_a,
            conflict_v1,
            "external_subagent:candidate-v1",
            None,
            selected_v1.preference_revision,
        )
        .await
        .expect_err("the stale process must not overwrite the new conflict generation");
        assert_eq!(
            ExternalSourceOperationError::decode(&error)
                .expect("stale conflict actions use the typed error contract")
                .code,
            ExternalSourceOperationErrorCode::StaleRevision
        );
    }

    #[tokio::test]
    async fn invalid_preference_file_is_an_error_instead_of_resetting_choices() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("external-sources.json");
        tokio::fs::write(&path, "{ invalid json").await.unwrap();

        let error = ExternalSourcePreferenceStore::new(path)
            .read()
            .await
            .expect_err("invalid preferences must fail closed");

        assert!(error.contains("deserialize"));
    }

    #[test]
    fn invocation_authorization_uses_the_execution_domain_preference_key() {
        let source = ExternalSourceRecord {
            key: SourceKey::new("opencode", "global-tools").unwrap(),
            ecosystem_id: bitfun_product_domains::external_sources::EcosystemId::new("opencode")
                .unwrap(),
            display_name: "OpenCode tools".to_string(),
            source_kind: "standalone_tools".to_string(),
            scope: ExternalSourceScope::UserGlobal,
            location: "/tools".to_string(),
            execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
            health: bitfun_product_domains::external_sources::ExternalSourceHealth::Available,
            content_version: "v1".to_string(),
            diagnostics: Vec::new(),
        };
        let approval_key = "approval";
        let mut config = ExternalSourcesConfig {
            approved_tool_targets: BTreeSet::from([approval_key.to_string()]),
            ..ExternalSourcesConfig::default()
        };

        config.suppressed_source_keys.push(source.preference_key());
        assert!(!external_tool_invocation_is_authorized_by(
            &config,
            source.ecosystem_id.as_str(),
            approval_key,
            &source.preference_key(),
            "<global>",
        ));
        assert!(external_tool_invocation_is_authorized_by(
            &config,
            source.ecosystem_id.as_str(),
            approval_key,
            &source.key.stable_key(),
            "<global>",
        ));
        config
            .integration_policy
            .known_mut()
            .expect("the built-in policy schema is known")
            .user_defaults
            .enabled = false;
        assert!(!external_tool_invocation_is_authorized_by(
            &config,
            source.ecosystem_id.as_str(),
            approval_key,
            &source.key.stable_key(),
            "<global>",
        ));
    }

    #[test]
    fn observed_tool_conflict_requires_reselection_after_external_lineage_changes() {
        let old = "external_tool:domain:read:old";
        let current = "external_tool:domain:read:new";
        let mut choices = BTreeMap::from([(old.to_string(), "external:source-a".to_string())]);

        reconcile_observed_tool_conflict(&mut choices, current);

        assert!(!choices.contains_key(old));
        assert_eq!(
            choices.get(current).map(String::as_str),
            Some(TOOL_CONFLICT_RESELECTION_REQUIRED)
        );
    }

    #[test]
    fn first_observed_tool_conflict_persists_an_unresolved_lineage() {
        let conflict_key = "external_tool:domain:read:first";
        let mut choices = BTreeMap::new();

        reconcile_observed_tool_conflict(&mut choices, conflict_key);

        assert_eq!(
            choices.get(conflict_key).map(String::as_str),
            Some(UNRESOLVED_TOOL_CONFLICT_CHOICE)
        );
    }

    #[test]
    fn conflict_lineages_are_compact_and_independent() {
        let mut choices = BTreeMap::from([
            (
                "prompt_command:local-user:review:old".to_string(),
                "external-a".to_string(),
            ),
            (
                "native:prompt_command:local-user:help:old".to_string(),
                "bitfun.cli:help".to_string(),
            ),
        ]);
        let mut lineage_keys = BTreeMap::from([
            (
                "prompt_command:local-user:review".to_string(),
                "prompt_command:local-user:review:old".to_string(),
            ),
            (
                "native:prompt_command:local-user:help".to_string(),
                "native:prompt_command:local-user:help:old".to_string(),
            ),
        ]);
        let mut conflicted_ids = BTreeSet::from([
            "external-a".to_string(),
            "external-b".to_string(),
            "bitfun.cli:help".to_string(),
        ]);

        ExternalSourceCoordinator::reconcile_conflict_preferences(
            &mut choices,
            &mut lineage_keys,
            &mut conflicted_ids,
            "native:prompt_command:local-user:help:new",
            "bitfun.cli:help",
            &["bitfun.cli:help".to_string()],
        );

        assert!(choices.contains_key("prompt_command:local-user:review:old"));
        assert!(!choices.contains_key("native:prompt_command:local-user:help:old"));
        assert_eq!(choices.len(), 2);
        assert_eq!(lineage_keys.len(), 2);
    }

    #[test]
    fn tool_decisions_keep_only_the_current_decline_per_approval() {
        let mut config = ExternalSourcesConfig::default();

        reconcile_tool_target_decision(
            &mut config,
            "approval-a".to_string(),
            "decision-v1".to_string(),
            false,
        );
        reconcile_tool_target_decision(
            &mut config,
            "approval-a".to_string(),
            "decision-v2".to_string(),
            false,
        );

        assert_eq!(
            config.declined_tool_decisions,
            BTreeMap::from([("approval-a".to_string(), "decision-v2".to_string())])
        );
        reconcile_tool_target_decision(
            &mut config,
            "approval-a".to_string(),
            "decision-v2".to_string(),
            true,
        );
        assert!(config.declined_tool_decisions.is_empty());
        assert_eq!(
            config.approved_tool_targets,
            BTreeSet::from(["approval-a".to_string()])
        );
    }

    #[tokio::test]
    async fn tool_approval_waits_for_refresh_and_rejects_a_changed_decision() {
        let service = test_service(Vec::new());
        let request = |decision_key: &str, content_version: &str| {
            serde_json::from_value::<ExternalToolApprovalRequest>(serde_json::json!({
                "approvalKey": "approval-a",
                "decisionKey": decision_key,
                "targetId": {
                    "source": { "providerId": "opencode.tools", "sourceId": "project" },
                    "localId": "review.js"
                },
                "sourceDisplayName": "OpenCode project tools",
                "sourceScope": "project",
                "sourceLocation": "/repo/.opencode/tools/review.js",
                "workingDirectory": "/repo",
                "runtimeKind": "java_script",
                "capabilities": ["file_system"],
                "contentVersion": content_version,
                "toolNames": ["review"]
            }))
            .unwrap()
        };
        lock_snapshot(&service.snapshot).tool_approval_requests =
            vec![request("decision-v1", "v1")];
        let expected_preference_revision = lock_snapshot(&service.snapshot).preference_revision;

        let refresh_guard = service.refresh_gate.lock().await;
        let decision_service = Arc::clone(&service);
        let decision = tokio::spawn(async move {
            decision_service
                .set_tool_target_decision(
                    "approval-a",
                    "decision-v1",
                    true,
                    expected_preference_revision,
                )
                .await
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            service.tool_decision_gate_waiting.notified(),
        )
        .await
        .expect("approval task must reach the refresh gate");
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(50),
                service.tool_decision_gate_acquired.notified(),
            )
            .await
            .is_err(),
            "approval must not enter the decision critical section while refresh owns the gate"
        );

        lock_snapshot(&service.snapshot).tool_approval_requests =
            vec![request("decision-v2", "v2")];
        drop(refresh_guard);
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            service.tool_decision_gate_acquired.notified(),
        )
        .await
        .expect("approval task must enter after the refresh releases the gate");

        let error = decision
            .await
            .unwrap()
            .expect_err("the approval must not apply to the changed content");
        assert_eq!(
            ExternalSourceOperationError::decode(&error)
                .expect("changed tool decisions use the typed error contract")
                .code,
            ExternalSourceOperationErrorCode::NotFound
        );
    }

    #[test]
    fn tool_conflict_choices_keep_only_the_current_version_per_lineage() {
        let mut choices = BTreeMap::from([
            (
                "external_tool:local-user:review:old".to_string(),
                "external-a".to_string(),
            ),
            (
                "external_tool:local-user:help:old".to_string(),
                "builtin-help".to_string(),
            ),
        ]);

        reconcile_versioned_tool_conflict_choice(
            &mut choices,
            "external_tool:local-user:review:new".to_string(),
            "external-b".to_string(),
        );

        assert!(!choices.contains_key("external_tool:local-user:review:old"));
        assert_eq!(choices["external_tool:local-user:review:new"], "external-b");
        assert_eq!(choices["external_tool:local-user:help:old"], "builtin-help");
        assert_eq!(choices.len(), 2);
    }

    #[test]
    fn mcp_server_decisions_keep_one_version_per_workspace_lineage() {
        let mut decisions = BTreeMap::from([
            (
                "external_mcp_approval:local-user:workspace-a:server:old".to_string(),
                ExternalMcpDecision {
                    decision_key: "external_mcp_approval:local-user:workspace-a:server:old"
                        .to_string(),
                    approved: true,
                },
            ),
            (
                "external_mcp_approval:local-user:workspace-b:server:current".to_string(),
                ExternalMcpDecision {
                    decision_key: "external_mcp_approval:local-user:workspace-b:server:current"
                        .to_string(),
                    approved: true,
                },
            ),
        ]);

        reconcile_versioned_mcp_server_decision(
            &mut decisions,
            "external_mcp_approval:local-user:workspace-a:server:new".to_string(),
            false,
        );

        assert!(!decisions.contains_key("external_mcp_approval:local-user:workspace-a:server:old"));
        assert!(!decisions["external_mcp_approval:local-user:workspace-a:server:new"].approved);
        assert!(
            decisions.contains_key("external_mcp_approval:local-user:workspace-b:server:current")
        );
        assert_eq!(decisions.len(), 2);
    }

    #[tokio::test]
    async fn slow_provider_is_not_respawned_while_healthy_sibling_updates() {
        let slow_calls = Arc::new(AtomicUsize::new(0));
        let healthy_calls = Arc::new(AtomicUsize::new(0));
        let service = test_service(vec![
            delayed_provider(
                "slow",
                std::time::Duration::from_millis(250),
                Arc::clone(&slow_calls),
            ),
            delayed_provider(
                "healthy",
                std::time::Duration::ZERO,
                Arc::clone(&healthy_calls),
            ),
        ]);

        let requests = lock_coordinator(&service.control_plane).discovery_requests();
        let batch = service
            .control_plane
            .discover_commands(requests, std::time::Duration::from_millis(25))
            .await;
        let snapshot =
            lock_coordinator(&service.control_plane).apply_discovery_results(batch.immediate);
        for deferred in batch.deferred {
            service.schedule_deferred_command_discovery(deferred);
        }
        assert!(snapshot
            .commands
            .iter()
            .any(|command| command.definition.name == "healthy"));

        let requests = lock_coordinator(&service.control_plane).discovery_requests();
        let batch = service
            .control_plane
            .discover_commands(requests, std::time::Duration::from_millis(25))
            .await;
        let snapshot =
            lock_coordinator(&service.control_plane).apply_discovery_results(batch.immediate);

        assert_eq!(slow_calls.load(Ordering::SeqCst), 1);
        assert!(healthy_calls.load(Ordering::SeqCst) >= 2);
        assert!(snapshot
            .commands
            .iter()
            .any(|command| command.definition.name == "healthy"));
    }

    #[tokio::test]
    async fn initial_refresh_waiters_reuse_the_in_flight_result() {
        let service = test_service(Vec::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());

        let background = {
            let service = Arc::clone(&service);
            let snapshot_service = Arc::clone(&service);
            let calls = Arc::clone(&calls);
            let started = Arc::clone(&started);
            let release = Arc::clone(&release);
            tokio::spawn(async move {
                service
                    .ensure_initial_refresh_with(|| async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        started.notify_one();
                        release.notified().await;
                        Ok(snapshot_service.snapshot())
                    })
                    .await
            })
        };

        started.notified().await;
        let catalog_waiter = {
            let service = Arc::clone(&service);
            let snapshot_service = Arc::clone(&service);
            let calls = Arc::clone(&calls);
            tokio::spawn(async move {
                service
                    .ensure_initial_refresh_with(|| async move {
                        calls.fetch_add(100, Ordering::SeqCst);
                        Ok(snapshot_service.snapshot())
                    })
                    .await
            })
        };
        tokio::task::yield_now().await;
        assert!(!catalog_waiter.is_finished());

        release.notify_one();
        background.await.unwrap().unwrap();
        catalog_waiter.await.unwrap().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn failed_initial_refresh_can_be_retried() {
        let service = test_service(Vec::new());
        let first = service
            .ensure_initial_refresh_with(|| async { Err("temporary failure".to_string()) })
            .await;
        assert_eq!(first.unwrap_err(), "temporary failure");

        let calls = Arc::new(AtomicUsize::new(0));
        let snapshot_service = Arc::clone(&service);
        service
            .ensure_initial_refresh_with(|| {
                let calls = Arc::clone(&calls);
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(snapshot_service.snapshot())
                }
            })
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
