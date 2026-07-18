use super::types::{AgentCategory, AgentEntry, AgentInfo, AgentSource, SubAgentSource};
use super::AgentRegistry;
use crate::agentic::agents::{Agent, SubagentVisibilityPolicy};
use bitfun_agent_runtime::prompt_cache::prompt_cache_scope_key;
use bitfun_core_types::{SessionContinuationPolicy, SessionModelBindingPolicy};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, Weak};

pub(crate) const EXTERNAL_SUBAGENT_RUNTIME_KEY_PREFIX: &str = "external_subagent_runtime:";

pub(crate) fn external_subagent_runtime_key(digest: &str) -> String {
    format!("{EXTERNAL_SUBAGENT_RUNTIME_KEY_PREFIX}{digest}")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalSubagentModelBinding {
    pub model_id: String,
    pub configuration_fingerprint: String,
}

#[derive(Clone)]
pub struct ExternalSubagentRegistration {
    pub runtime_key: String,
    pub logical_id: String,
    pub provider_label: String,
    pub model_binding: ExternalSubagentModelBinding,
    pub hidden: bool,
    pub agent: Arc<dyn Agent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalSubagentRoute {
    Local,
    External(String),
    Unavailable,
}

#[derive(Clone)]
struct ExternalSubagentGenerationEntry {
    registration: ExternalSubagentRegistration,
    agent_entry: AgentEntry,
    lease_count: usize,
}

pub(super) struct ExternalSubagentRegistryState {
    generations: RwLock<HashMap<String, ExternalSubagentGenerationEntry>>,
    workspace_routes: RwLock<HashMap<PathBuf, BTreeMap<String, ExternalSubagentRoute>>>,
}

impl ExternalSubagentRegistryState {
    pub(super) fn new() -> Self {
        Self {
            generations: RwLock::new(HashMap::new()),
            workspace_routes: RwLock::new(HashMap::new()),
        }
    }

    fn read_generations(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, HashMap<String, ExternalSubagentGenerationEntry>> {
        self.generations
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write_generations(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, HashMap<String, ExternalSubagentGenerationEntry>> {
        self.generations
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn read_routes(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, HashMap<PathBuf, BTreeMap<String, ExternalSubagentRoute>>>
    {
        self.workspace_routes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write_routes(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, HashMap<PathBuf, BTreeMap<String, ExternalSubagentRoute>>>
    {
        self.workspace_routes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(super) fn find_generation_entry(&self, runtime_key: &str) -> Option<AgentEntry> {
        self.read_generations()
            .get(runtime_key)
            .map(|entry| entry.agent_entry.clone())
    }

    pub(super) fn has_generation(&self, runtime_key: &str) -> bool {
        self.read_generations().contains_key(runtime_key)
    }

    fn prune_unrouted_generations(&self) {
        let routed = self
            .read_routes()
            .values()
            .flat_map(BTreeMap::values)
            .filter_map(|route| match route {
                ExternalSubagentRoute::External(runtime_key) => Some(runtime_key.clone()),
                ExternalSubagentRoute::Local | ExternalSubagentRoute::Unavailable => None,
            })
            .collect::<HashSet<_>>();
        self.write_generations()
            .retain(|runtime_key, entry| entry.lease_count > 0 || routed.contains(runtime_key));
    }

    fn acquire(self: &Arc<Self>, runtime_key: &str) -> Option<ExternalSubagentInvocationBinding> {
        let mut generations = self.write_generations();
        let entry = generations.get_mut(runtime_key)?;
        entry.lease_count = entry.lease_count.saturating_add(1);
        Some(ExternalSubagentInvocationBinding {
            runtime_agent_key: runtime_key.to_string(),
            logical_id: entry.registration.logical_id.clone(),
            supports_follow_up: false,
            continuation_policy: SessionContinuationPolicy::FreshOnly,
            model_binding_policy: SessionModelBindingPolicy::ApprovedImmutable,
            lease: Some(ExternalSubagentGenerationLease {
                state: Arc::downgrade(self),
                runtime_key: runtime_key.to_string(),
                model_binding: entry.registration.model_binding.clone(),
            }),
        })
    }

    fn release(&self, runtime_key: &str) {
        if let Some(entry) = self.write_generations().get_mut(runtime_key) {
            entry.lease_count = entry.lease_count.saturating_sub(1);
        }
        self.prune_unrouted_generations();
    }
}

pub struct ExternalSubagentGenerationLease {
    state: Weak<ExternalSubagentRegistryState>,
    runtime_key: String,
    model_binding: ExternalSubagentModelBinding,
}

impl std::fmt::Debug for ExternalSubagentGenerationLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExternalSubagentGenerationLease")
            .field("runtime_key", &self.runtime_key)
            .field("model_binding", &self.model_binding)
            .finish_non_exhaustive()
    }
}

impl Clone for ExternalSubagentGenerationLease {
    fn clone(&self) -> Self {
        if let Some(state) = self.state.upgrade() {
            if let Some(entry) = state.write_generations().get_mut(&self.runtime_key) {
                entry.lease_count = entry.lease_count.saturating_add(1);
            }
        }
        Self {
            state: self.state.clone(),
            runtime_key: self.runtime_key.clone(),
            model_binding: self.model_binding.clone(),
        }
    }
}

impl ExternalSubagentGenerationLease {
    pub fn model_binding(&self) -> &ExternalSubagentModelBinding {
        &self.model_binding
    }
}

impl Drop for ExternalSubagentGenerationLease {
    fn drop(&mut self) {
        if let Some(state) = self.state.upgrade() {
            state.release(&self.runtime_key);
        }
    }
}

pub struct ExternalSubagentInvocationBinding {
    pub runtime_agent_key: String,
    pub logical_id: String,
    pub supports_follow_up: bool,
    pub continuation_policy: SessionContinuationPolicy,
    pub model_binding_policy: SessionModelBindingPolicy,
    pub lease: Option<ExternalSubagentGenerationLease>,
}

impl AgentRegistry {
    /// Returns whether the logical id is owned by an external route in the
    /// requested workspace. `Unavailable` remains externally owned so a
    /// withdrawn candidate cannot expose a same-name local mutation path.
    pub fn is_external_subagent_route(
        &self,
        logical_id: &str,
        workspace_root: Option<&Path>,
    ) -> bool {
        let routes = self.external_subagents.read_routes();
        let logical_key = normalize_external_logical_id(logical_id);
        let is_external = |route: &ExternalSubagentRoute| {
            matches!(
                route,
                ExternalSubagentRoute::External(_) | ExternalSubagentRoute::Unavailable
            )
        };
        workspace_root.is_some_and(|workspace| {
            routes
                .get(workspace)
                .and_then(|workspace_routes| workspace_routes.get(&logical_key))
                .is_some_and(is_external)
        })
    }

    pub fn install_external_subagent_routes(
        &self,
        workspace_root: &Path,
        registrations: Vec<ExternalSubagentRegistration>,
        routes: BTreeMap<String, ExternalSubagentRoute>,
    ) {
        {
            let mut generations = self.external_subagents.write_generations();
            for registration in registrations {
                let runtime_key = registration.runtime_key.clone();
                let lease_count = generations
                    .get(&runtime_key)
                    .map_or(0, |entry| entry.lease_count);
                let agent_entry = AgentEntry {
                    category: AgentCategory::SubAgent,
                    source: AgentSource::External,
                    subagent_source: Some(SubAgentSource::External),
                    agent: registration.agent.clone(),
                    visibility_policy: SubagentVisibilityPolicy::public(),
                    custom_config: None,
                };
                generations.insert(
                    runtime_key,
                    ExternalSubagentGenerationEntry {
                        registration,
                        agent_entry,
                        lease_count,
                    },
                );
            }
        }
        let mut routes = routes
            .into_iter()
            .map(|(logical_id, route)| (normalize_external_logical_id(&logical_id), route))
            .collect::<BTreeMap<_, _>>();
        let previous = self
            .external_subagents
            .read_routes()
            .get(workspace_root)
            .cloned()
            .unwrap_or_default();
        // An active external implementation disappearing must never expose a
        // same-name local implementation implicitly. Keep a fail-closed route
        // until the external candidate returns or product reconciliation
        // records an explicit Local choice.
        for (logical_id, previous_route) in previous {
            if !routes.contains_key(&logical_id)
                && matches!(
                    previous_route,
                    ExternalSubagentRoute::External(_) | ExternalSubagentRoute::Unavailable
                )
            {
                routes.insert(logical_id, ExternalSubagentRoute::Unavailable);
            }
        }
        self.external_subagents
            .write_routes()
            .insert(workspace_root.to_path_buf(), routes);
        self.external_subagents.prune_unrouted_generations();
    }

    pub fn release_external_subagent_workspace(&self, workspace_root: &Path) {
        self.external_subagents
            .write_routes()
            .remove(workspace_root);
        self.external_subagents.prune_unrouted_generations();
    }

    pub fn resolve_subagent_for_fresh_invocation(
        &self,
        logical_id: &str,
        workspace_root: Option<&Path>,
        external_sources_supported: bool,
    ) -> Option<ExternalSubagentInvocationBinding> {
        let logical_key = normalize_external_logical_id(logical_id);
        if external_sources_supported {
            if let Some(workspace_root) = workspace_root {
                if let Some(route) = self
                    .external_subagents
                    .read_routes()
                    .get(workspace_root)
                    .and_then(|routes| routes.get(&logical_key))
                    .cloned()
                {
                    return match route {
                        ExternalSubagentRoute::Local => self
                            .find_agent_entry(logical_id, Some(workspace_root))
                            .map(|_| local_binding(logical_id)),
                        ExternalSubagentRoute::External(runtime_key) => {
                            self.external_subagents.acquire(&runtime_key)
                        }
                        ExternalSubagentRoute::Unavailable => None,
                    };
                }
            }
        }
        self.find_agent_entry(logical_id, workspace_root)
            .map(|_| local_binding(logical_id))
    }

    pub(super) fn apply_external_routes_to_query(
        &self,
        workspace_root: &Path,
        mut local: Vec<AgentInfo>,
    ) -> Vec<AgentInfo> {
        let routes = self
            .external_subagents
            .read_routes()
            .get(workspace_root)
            .cloned()
            .unwrap_or_default();
        let generations = self.external_subagents.read_generations();
        for (logical_id, route) in routes {
            match route {
                ExternalSubagentRoute::Local => {}
                ExternalSubagentRoute::Unavailable => {
                    local.retain(|agent| normalize_external_logical_id(&agent.id) != logical_id);
                }
                ExternalSubagentRoute::External(runtime_key) => {
                    local.retain(|agent| normalize_external_logical_id(&agent.id) != logical_id);
                    let Some(entry) = generations.get(&runtime_key) else {
                        continue;
                    };
                    if entry.registration.hidden {
                        continue;
                    }
                    local.push(external_agent_info(entry));
                }
            }
        }
        local
    }
}

fn normalize_external_logical_id(logical_id: &str) -> String {
    logical_id.to_ascii_lowercase()
}

fn local_binding(logical_id: &str) -> ExternalSubagentInvocationBinding {
    ExternalSubagentInvocationBinding {
        runtime_agent_key: logical_id.to_string(),
        logical_id: logical_id.to_string(),
        supports_follow_up: true,
        continuation_policy: SessionContinuationPolicy::Reusable,
        model_binding_policy: SessionModelBindingPolicy::Mutable,
        lease: None,
    }
}

fn external_agent_info(entry: &ExternalSubagentGenerationEntry) -> AgentInfo {
    let agent = entry.registration.agent.as_ref();
    let default_tools = agent.default_tools();
    AgentInfo {
        key: format!(
            "external::{}::{}",
            entry.registration.provider_label.to_ascii_lowercase(),
            entry.registration.logical_id
        ),
        id: entry.registration.logical_id.clone(),
        name: agent.name().to_string(),
        description: agent.description().to_string(),
        is_readonly: agent.is_readonly(),
        is_review: false,
        tool_count: default_tools.len(),
        default_tools,
        prompt_cache_scope_key: prompt_cache_scope_key(
            &agent.system_prompt_cache_identity(None),
            &agent.user_context_cache_identity(),
        ),
        config_profile_id: None,
        config_profile_label: None,
        config_profile_member_mode_ids: Vec::new(),
        default_enabled: true,
        effective_enabled: true,
        override_state: None,
        state_reason: None,
        source: AgentSource::External,
        subagent_source: Some(SubAgentSource::External),
        path: None,
        model: Some(entry.registration.model_binding.model_id.clone()),
        model_is_explicit: Some(true),
        visibility: Some(SubagentVisibilityPolicy::public().summary()),
        external_provider_label: Some(entry.registration.provider_label.clone()),
        supports_follow_up: false,
    }
}
