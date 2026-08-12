use super::types::{AgentCategory, AgentEntry, AgentInfo, AgentSource, SubAgentSource};
use super::AgentRegistry;
use crate::agentic::agents::{Agent, SubagentVisibilityPolicy};
use crate::agentic::deep_review_policy::{
    CODE_REVIEW_AGENT_TYPE, DEEP_REVIEW_AGENT_TYPE, REVIEW_FIXER_AGENT_TYPE,
};
use crate::agentic::workspace::canonical_local_workspace_path;
use bitfun_agent_runtime::prompt_cache::prompt_cache_scope_key;
use bitfun_core_types::{
    SessionAgentRouteOwner, SessionContinuationPolicy, SessionModelBindingPolicy,
};
use bitfun_product_domains::external_sources::EcosystemId;
use bitfun_product_domains::external_subagents::ExternalSubagentMode;
use log::{debug, warn};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;

/// Stable prefix for external subagent runtime keys within the agent registry.
/// External subagents are registered under this namespace to avoid collisions with
/// built-in agents (`builtin:`, `custom:`, etc.). The module itself is intentionally
/// minimal — routing and lifecycle logic lives in `external_subagents.rs`.
#[cfg(feature = "external-sources")]
pub(crate) const EXTERNAL_SUBAGENT_RUNTIME_KEY_PREFIX: &str = "external_subagent_runtime:";

/// Formats a stable runtime key for an external subagent given its content digest.
/// Used by `install_active_candidate` to register generation-specific agent entries
/// without re-parsing ecosystem manifests on every restart.
#[cfg(feature = "external-sources")]
pub(crate) fn external_subagent_runtime_key(digest: &str) -> String {
    format!("{EXTERNAL_SUBAGENT_RUNTIME_KEY_PREFIX}{digest}")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalSubagentModelBinding {
    Fixed {
        model_id: String,
        configuration_fingerprint: String,
    },
    InheritParent,
}

impl ExternalSubagentModelBinding {
    pub fn fixed_model_id(&self) -> Option<&str> {
        match self {
            Self::Fixed { model_id, .. } => Some(model_id),
            Self::InheritParent => None,
        }
    }

    pub fn configuration_fingerprint(&self) -> Option<&str> {
        match self {
            Self::Fixed {
                configuration_fingerprint,
                ..
            } => Some(configuration_fingerprint),
            Self::InheritParent => None,
        }
    }
}

#[derive(Clone)]
pub struct ExternalSubagentRegistration {
    pub runtime_key: String,
    pub logical_id: String,
    pub ecosystem_id: EcosystemId,
    pub provider_label: String,
    pub model_binding: ExternalSubagentModelBinding,
    pub hidden: bool,
    pub mode: ExternalSubagentMode,
    pub agent: Arc<dyn Agent>,
}

#[derive(Clone, Copy)]
enum ExternalAgentProjection {
    Primary,
    Subagent,
}

impl ExternalAgentProjection {
    fn is_supported_by(self, mode: ExternalSubagentMode) -> bool {
        match self {
            Self::Primary => mode.supports_primary(),
            Self::Subagent => mode.supports_subagent(),
        }
    }
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

    // Synchronous helper over a tokio RwLock (no await point); see
    // super::spin_read for the bounded-retry contract. Guards must never be
    // held across an await; a panic (spin cap exceeded) means a holder
    // violated that.
    fn read_generations(
        &self,
    ) -> tokio::sync::RwLockReadGuard<'_, HashMap<String, ExternalSubagentGenerationEntry>> {
        super::spin_read(
            &self.generations,
            "ExternalSubagentRegistryState generations",
        )
    }

    fn write_generations(
        &self,
    ) -> tokio::sync::RwLockWriteGuard<'_, HashMap<String, ExternalSubagentGenerationEntry>> {
        super::spin_write(
            &self.generations,
            "ExternalSubagentRegistryState generations",
        )
    }

    // Synchronous helper; see read_generations for the lock-contention contract.
    fn read_routes(
        &self,
    ) -> tokio::sync::RwLockReadGuard<'_, HashMap<PathBuf, BTreeMap<String, ExternalSubagentRoute>>>
    {
        super::spin_read(
            &self.workspace_routes,
            "ExternalSubagentRegistryState workspace_routes",
        )
    }

    fn write_routes(
        &self,
    ) -> tokio::sync::RwLockWriteGuard<'_, HashMap<PathBuf, BTreeMap<String, ExternalSubagentRoute>>>
    {
        super::spin_write(
            &self.workspace_routes,
            "ExternalSubagentRegistryState workspace_routes",
        )
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

    fn acquire_matching(
        self: &Arc<Self>,
        runtime_key: &str,
        expected_ecosystem_id: Option<&EcosystemId>,
        projection: ExternalAgentProjection,
    ) -> Option<ExternalSubagentInvocationBinding> {
        let mut generations = self.write_generations();
        let entry = generations.get_mut(runtime_key)?;
        if expected_ecosystem_id
            .is_some_and(|expected| expected != &entry.registration.ecosystem_id)
        {
            return None;
        }
        if !projection.is_supported_by(entry.registration.mode) {
            return None;
        }
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

    fn acquire(self: &Arc<Self>, runtime_key: &str) -> Option<ExternalSubagentInvocationBinding> {
        self.acquire_matching(runtime_key, None, ExternalAgentProjection::Subagent)
    }

    fn acquire_primary(
        self: &Arc<Self>,
        runtime_key: &str,
    ) -> Option<ExternalPrimaryAgentTurnBinding> {
        let mut generations = self.write_generations();
        let entry = generations.get_mut(runtime_key)?;
        if !entry.registration.mode.supports_primary() {
            return None;
        }
        entry.lease_count = entry.lease_count.saturating_add(1);
        let model_binding = entry.registration.model_binding.clone();
        Some(ExternalPrimaryAgentTurnBinding {
            runtime_agent_key: runtime_key.to_string(),
            model_binding: Some(model_binding.clone()),
            route_owner: SessionAgentRouteOwner::External,
            lease: Some(ExternalSubagentGenerationLease {
                state: Arc::downgrade(self),
                runtime_key: runtime_key.to_string(),
                model_binding,
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

pub struct ExternalPrimaryAgentTurnBinding {
    pub runtime_agent_key: String,
    pub model_binding: Option<ExternalSubagentModelBinding>,
    pub route_owner: SessionAgentRouteOwner,
    pub lease: Option<ExternalSubagentGenerationLease>,
}

/// 主代理（会话主模型）解析失败的原因分类。
///
/// 之前 `resolve_primary_agent_for_turn` 对「路由不可用」与「owner 不匹配」
/// 一律返回 `None`，调用方只能统一报 "Unknown session mode"，无法诊断。
/// 现在返回带原因的 `Err`，区分：
/// - `CandidateUnavailable`：外部候选已撤回 / generation 缺失 / 不支持主代理，
///   或本地候选不存在；
/// - `OwnerMismatch`：已解析绑定与持久化会话的期望 owner 不一致。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalPrimaryAgentResolutionError {
    /// 候选不可用：路由处于 `Unavailable`（fail-closed 撤回），或外部
    /// generation 缺失 / 不支持主代理，或本地路由下找不到注册候选。
    CandidateUnavailable {
        logical_id: String,
        reason: &'static str,
    },
    /// 已解析绑定与期望的会话 route owner 不匹配。
    OwnerMismatch {
        logical_id: String,
        expected: SessionAgentRouteOwner,
        actual: SessionAgentRouteOwner,
    },
}

impl std::fmt::Display for ExternalPrimaryAgentResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CandidateUnavailable { logical_id, reason } => {
                write!(formatter, "candidate_unavailable: {logical_id} ({reason})")
            }
            Self::OwnerMismatch {
                logical_id,
                expected,
                actual,
            } => write!(
                formatter,
                "owner_mismatch: {logical_id} expected {expected:?}, resolved {actual:?}"
            ),
        }
    }
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
            let workspace = canonical_local_workspace_path(workspace);
            routes
                .get(&workspace)
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
        let workspace_root = canonical_local_workspace_path(workspace_root);
        {
            let mut generations = self.external_subagents.write_generations();
            for registration in registrations {
                let runtime_key = registration.runtime_key.clone();
                let lease_count = generations
                    .get(&runtime_key)
                    .map_or(0, |entry| entry.lease_count);
                // 同 runtime_key 重新 install 时，若仍有在途 turn（lease_count>0），
                // 保留旧 agent_entry，避免换绑导致进行中的会话底层 agent 不一致；
                // registration 仍更新（新配置对后续 acquire 生效，已发出的 lease 持有快照）。
                let agent_entry = match generations.get(&runtime_key) {
                    Some(entry) if entry.lease_count > 0 => entry.agent_entry.clone(),
                    _ => AgentEntry {
                        category: AgentCategory::SubAgent,
                        source: AgentSource::External,
                        subagent_source: Some(SubAgentSource::External),
                        agent: registration.agent.clone(),
                        visibility_policy: SubagentVisibilityPolicy::public(),
                        custom_config: None,
                    },
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
            .get(&workspace_root)
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
            .insert(workspace_root, routes);
        self.external_subagents.prune_unrouted_generations();
    }

    pub fn release_external_subagent_workspace(&self, workspace_root: &Path) {
        let workspace_root = canonical_local_workspace_path(workspace_root);
        self.external_subagents
            .write_routes()
            .remove(&workspace_root);
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
                let workspace_key = canonical_local_workspace_path(workspace_root);
                if let Some(route) = self
                    .external_subagents
                    .read_routes()
                    .get(&workspace_key)
                    .and_then(|routes| routes.get(&logical_key))
                    .cloned()
                {
                    return match route {
                        ExternalSubagentRoute::Local => self
                            .find_agent_entry(logical_id, Some(workspace_root))
                            .map(|entry| local_binding(logical_id, entry.agent.id())),
                        ExternalSubagentRoute::External(runtime_key) => {
                            self.external_subagents.acquire(&runtime_key)
                        }
                        ExternalSubagentRoute::Unavailable => None,
                    };
                }
            }
        }
        self.find_agent_entry(logical_id, workspace_root)
            .map(|entry| local_binding(logical_id, entry.agent.id()))
    }

    /// Resolve a user-facing main-agent id to the exact generation that owns
    /// the next turn. The returned lease keeps prompt, tools, permissions, and
    /// model metadata stable until that turn settles.
    ///
    /// 失败时返回带原因的错误，而不是一律 `None`，便于调用方精确诊断：
    /// - `CandidateUnavailable`：外部候选撤回（`Unavailable` 路由）或
    ///   generation 缺失 / 不支持主代理、本地候选不存在；
    /// - `OwnerMismatch`：已解析绑定与 `expected_owner` 不一致。
    pub fn resolve_primary_agent_for_turn(
        &self,
        logical_id: &str,
        workspace_root: Option<&Path>,
        external_sources_supported: bool,
        expected_owner: Option<SessionAgentRouteOwner>,
    ) -> Result<ExternalPrimaryAgentTurnBinding, ExternalPrimaryAgentResolutionError> {
        let logical_key = normalize_external_logical_id(logical_id);
        if external_sources_supported {
            if let Some(workspace_root) = workspace_root {
                let workspace_key = canonical_local_workspace_path(workspace_root);
                if let Some(route) = self
                    .external_subagents
                    .read_routes()
                    .get(&workspace_key)
                    .and_then(|routes| routes.get(&logical_key))
                    .cloned()
                {
                    let binding = match route {
                        // 与下方 fall-through（find_agent_entry 直接映射）对齐：
                        // 移除 Mode 过滤，允许 subagent 类型代理续聊/恢复/压缩。
                        // 上游 is_local_session_primary_entry 白名单保留（融合
                        // 方案）——下方 fall-through 中
                        // 命中白名单走确认路径，未命中按本地全量放开。
                        ExternalSubagentRoute::Local => self
                            .find_agent_entry(logical_id, Some(workspace_root))
                            .map(|entry| local_primary_binding(entry.agent.id()))
                            .ok_or(ExternalPrimaryAgentResolutionError::CandidateUnavailable {
                                logical_id: logical_key.clone(),
                                reason: "local route has no registered candidate",
                            })?,
                        ExternalSubagentRoute::External(runtime_key) => self
                            .external_subagents
                            .acquire_primary(&runtime_key)
                            .ok_or(ExternalPrimaryAgentResolutionError::CandidateUnavailable {
                                logical_id: logical_key.clone(),
                                reason: "external generation missing or not primary-capable",
                            })?,
                        // 候选已撤回时保持 fail-closed：不回落同名本地实现，
                        // 并携带明确原因供调用方诊断。
                        ExternalSubagentRoute::Unavailable => {
                            return Err(
                                ExternalPrimaryAgentResolutionError::CandidateUnavailable {
                                    logical_id: logical_key,
                                    reason: "external candidate withdrawn (fail-closed route)",
                                },
                            );
                        }
                    };
                    if let Some(expected_owner) = expected_owner {
                        if binding.route_owner != expected_owner {
                            // 解析成功但 owner 与持久化会话不一致，单独归类，
                            // 避免与「候选不可用」混为一谈。
                            return Err(ExternalPrimaryAgentResolutionError::OwnerMismatch {
                                logical_id: logical_key,
                                expected: expected_owner,
                                actual: binding.route_owner,
                            });
                        }
                    }
                    return Ok(binding);
                }
            }
        }
        if expected_owner == Some(SessionAgentRouteOwner::External) {
            // 会话持久化 owner 为 External，但当前没有外部路由可解析，
            // 属于 owner 语义冲突（fail-closed），不再是「未知会话模式」。
            return Err(ExternalPrimaryAgentResolutionError::OwnerMismatch {
                logical_id: logical_key,
                expected: SessionAgentRouteOwner::External,
                actual: SessionAgentRouteOwner::Local,
            });
        }
        // Subagent types (custom `kind: subagent` agents such as legion
        // permanent posts, and builtin subagents) are valid owners of sessions
        // created through SessionControl/SessionMessage and must resolve for
        // continued dialog turns, restore, and manual compaction. The Mode
        // filter only guarded the route branch above; the fail-closed
        // `expected_owner == External` guard stays.
        // 融合（上游 review 修复 + 本地全量放开）：
        // - 命中上游 is_local_session_primary_entry 白名单（Mode 或
        //   CodeReview/DeepReview/ReviewFixer builtin）→ 白名单确认路径解析（上游功能保留）；
        // - 未命中（其他 subagent 类型）→ 本地全量放开仍允许（ACP/本地定制超集），
        //   并 warn 提示该 entry 不在上游白名单、由本地定制放开；
        // - 例外（上游 c4a301e20 语义保留）：builtin 保留 review ID
        //   （CodeReview/DeepReview/ReviewFixer）被非 Builtin entry 同名 shadow
        //   时 fail-closed——不继承 builtin primary 路径，避免自定义 agent
        //   冒用 review 会话主代理身份（安全边界）。
        match self.find_agent_entry(logical_id, workspace_root) {
            Some(entry) => {
                if is_shadowed_builtin_review_primary_id(&entry) {
                    return Err(ExternalPrimaryAgentResolutionError::CandidateUnavailable {
                        logical_id: logical_key,
                        reason:
                            "non-Builtin entry shadows a builtin review primary id (fail-closed)",
                    });
                }
                if is_local_session_primary_entry(&entry) {
                    Ok(local_primary_binding(entry.agent.id()))
                } else {
                    warn!(
                        "Session primary agent resolution allows a non-whitelisted subagent via local customization: logical_id={}, category={:?}, source={:?}, expected_owner={:?}",
                        logical_id,
                        entry.category,
                        entry.source,
                        expected_owner
                    );
                    Ok(local_primary_binding(entry.agent.id()))
                }
            }
            None => {
                debug!(
                    "Session primary agent resolution found no registered agent: logical_id={}, expected_owner={:?}",
                    logical_id, expected_owner
                );
                Err(ExternalPrimaryAgentResolutionError::CandidateUnavailable {
                    logical_id: logical_key,
                    reason: "no registered candidate for the requested session mode",
                })
            }
        }
    }

    /// Resolve only the currently approved external route for an exact
    /// ecosystem. Command delegation must never fall back to a same-name local
    /// agent or cross an ecosystem boundary after the command was expanded.
    pub fn resolve_external_subagent_for_fresh_invocation(
        &self,
        logical_id: &str,
        ecosystem_id: &EcosystemId,
        workspace_root: Option<&Path>,
    ) -> Option<ExternalSubagentInvocationBinding> {
        let workspace_root = workspace_root?;
        let workspace_key = canonical_local_workspace_path(workspace_root);
        let logical_key = normalize_external_logical_id(logical_id);
        let route = self
            .external_subagents
            .read_routes()
            .get(&workspace_key)
            .and_then(|routes| routes.get(&logical_key))
            .cloned()?;
        match route {
            ExternalSubagentRoute::External(runtime_key) => {
                self.external_subagents.acquire_matching(
                    &runtime_key,
                    Some(ecosystem_id),
                    ExternalAgentProjection::Subagent,
                )
            }
            ExternalSubagentRoute::Local | ExternalSubagentRoute::Unavailable => None,
        }
    }

    pub(super) fn apply_external_routes_to_query(
        &self,
        workspace_root: &Path,
        mut local: Vec<AgentInfo>,
    ) -> Vec<AgentInfo> {
        let workspace_root = canonical_local_workspace_path(workspace_root);
        let routes = self
            .external_subagents
            .read_routes()
            .get(&workspace_root)
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
                    if entry.registration.hidden || !entry.registration.mode.supports_subagent() {
                        continue;
                    }
                    local.push(external_agent_info(
                        entry,
                        ExternalAgentProjection::Subagent,
                    ));
                }
            }
        }
        local
    }

    pub(super) fn apply_external_routes_to_modes(
        &self,
        workspace_root: &Path,
        mut local: Vec<AgentInfo>,
    ) -> Vec<AgentInfo> {
        let workspace_root = canonical_local_workspace_path(workspace_root);
        let routes = self
            .external_subagents
            .read_routes()
            .get(&workspace_root)
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
                    if entry.registration.hidden || !entry.registration.mode.supports_primary() {
                        continue;
                    }
                    local.push(external_agent_info(entry, ExternalAgentProjection::Primary));
                }
            }
        }
        local
    }
}

fn normalize_external_logical_id(logical_id: &str) -> String {
    // 归一化更严格：折叠空白（去首尾、合并内部连续空白）后统一 Unicode 小写，
    // 避免仅 ASCII 小写时同一逻辑 id 因空白或非 ASCII 大小写变体被拆成不同键。
    logical_id
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn local_binding(logical_id: &str, runtime_agent_key: &str) -> ExternalSubagentInvocationBinding {
    ExternalSubagentInvocationBinding {
        runtime_agent_key: runtime_agent_key.to_string(),
        logical_id: logical_id.to_string(),
        supports_follow_up: true,
        continuation_policy: SessionContinuationPolicy::Reusable,
        model_binding_policy: SessionModelBindingPolicy::Mutable,
        lease: None,
    }
}

/// Builtin agents that are allowed to act as the main agent of a session even
/// though they are not registered as `Mode` (review child sessions).
///
/// Review child sessions are created by the product surfaces with
/// `agentType=CodeReview` (standard) or `agentType=DeepReview` (strict), and
/// the remediation phase of either session runs with `agentType=ReviewFixer`.
/// All three must resolve through the primary-agent path for create, turn,
/// restore, and compaction. Other subagents (e.g. `ReviewWorker`,
/// `ReviewJudge`) stay restricted.
fn is_builtin_session_primary_agent(id: &str) -> bool {
    matches!(
        id,
        CODE_REVIEW_AGENT_TYPE | DEEP_REVIEW_AGENT_TYPE | REVIEW_FIXER_AGENT_TYPE
    )
}

/// Whether a locally-resolved agent entry may act as a session primary agent.
///
/// Used by both the explicit `ExternalSubagentRoute::Local` branch and the
/// no-route fallback so review child sessions (CodeReview/DeepReview) resolve
/// identically regardless of whether a workspace route table pins them to the
/// local implementation.
fn is_local_session_primary_entry(entry: &AgentEntry) -> bool {
    entry.category == AgentCategory::Mode
        || (entry.source == AgentSource::Builtin
            && is_builtin_session_primary_agent(entry.agent.id()))
}

/// Whether a non-Builtin entry shadows a builtin review primary id.
///
/// Custom-agent loading normally filters ids that collide with builtin entries,
/// but the session-primary path must fail closed regardless: a User/Custom
/// entry occupying the builtin "ReviewFixer" (or "CodeReview"/"DeepReview") id
/// must never inherit the builtin primary path (upstream c4a301e20 semantics).
/// Non-reserved custom subagent ids (e.g. `custom-handoff`) stay full-open via
/// the local customization branch.
fn is_shadowed_builtin_review_primary_id(entry: &AgentEntry) -> bool {
    entry.source != AgentSource::Builtin && is_builtin_session_primary_agent(entry.agent.id())
}

fn local_primary_binding(runtime_agent_key: &str) -> ExternalPrimaryAgentTurnBinding {
    ExternalPrimaryAgentTurnBinding {
        runtime_agent_key: runtime_agent_key.to_string(),
        model_binding: None,
        route_owner: SessionAgentRouteOwner::Local,
        lease: None,
    }
}

fn external_agent_info(
    entry: &ExternalSubagentGenerationEntry,
    projection: ExternalAgentProjection,
) -> AgentInfo {
    let agent = entry.registration.agent.as_ref();
    let mut default_tools = agent.default_tools();
    if matches!(projection, ExternalAgentProjection::Primary) {
        bitfun_agent_runtime::thread_goal_tools::ensure_thread_goal_tools(&mut default_tools);
    }
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
        subagent_source: matches!(projection, ExternalAgentProjection::Subagent)
            .then_some(SubAgentSource::External),
        path: None,
        model: entry
            .registration
            .model_binding
            .fixed_model_id()
            .map(str::to_string),
        model_is_explicit: Some(entry.registration.model_binding.fixed_model_id().is_some()),
        visibility: matches!(projection, ExternalAgentProjection::Subagent)
            .then(|| SubagentVisibilityPolicy::public().summary()),
        external_provider_label: Some(entry.registration.provider_label.clone()),
        supports_follow_up: matches!(projection, ExternalAgentProjection::Primary),
    }
}
