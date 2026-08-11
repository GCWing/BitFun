//! LegionControl deploys a legion team topology into persisted agent sessions.
//!
//! A legion is described by a preset (stored via `team_presets`) or by inline
//! `nodes`/`edges` input. The tool validates the topology (no cycles, at most
//! one parent per node), deploys each node as a persisted session through the
//! same runtime path as SessionControl, and attaches sessions to the session
//! tree along the edges.

use super::util::normalize_path;
use crate::agentic::agents::team_presets::{
    create_preset, delete_preset, get_preset, list_presets, LegionEdge, LegionNode, LegionPreset,
};
use crate::agentic::coordination::{get_global_coordinator, ConversationCoordinator};
use crate::agentic::keyed_lock::KeyedAsyncLock;
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::restrictions::{get_session_role, validate_delegation, AgentRole};
use crate::service::config::{
    default_legion_deploy_frequency_per_hour, default_legion_max_nodes,
    default_legion_max_total_nodes, get_global_config_service,
};
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_agent_runtime::session_control::session_control_creator_marker;
use bitfun_runtime_ports::AgentSessionCreateRequest;
use bitfun_services_core::session::types::{SessionRelationship, SessionRelationshipKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Default upper bound on the number of legion nodes in one topology.
///
/// An unbounded node count lets a single LegionControl call spawn an
/// unbounded number of persisted sessions. 20 keeps the deployment bounded
/// while leaving room for realistic team shapes (the built-in presets use at
/// most a handful of nodes).
///
/// **legion 阈值参数配置化**：the effective limit now comes from
/// `ai.legion_max_nodes` (front-end configurable, default 20); production code
/// resolves it via [`resolve_legion_max_nodes`]. This constant is kept only as
/// the reference value used by unit tests.
#[cfg(test)]
const MAX_LEGION_NODES: usize = 20;

/// Custom-metadata key that records each successful LegionControl `load` on
/// the creator session (legion 阈值参数配置化：部署频率上限）。
///
/// The value is a JSON array of Unix-second timestamps (most recent last).
/// A one-hour sliding window counts entries newer than `now - 3600s`; when the
/// count reaches `ai.legion_deploy_frequency_per_hour` the next load is
/// rejected. `0` (or unset) disables the limit.
const LEGION_DEPLOY_TIMES_METADATA_KEY: &str = "legionDeployTimes";
/// Sliding window for the legion deployment frequency limit (seconds).
const LEGION_DEPLOY_WINDOW_SECS: i64 = 60 * 60;

/// Serializes the legion deployment frequency read-check-write for one
/// (workspace, creator) pair (UX-P1-5).
///
/// The frequency limit is a read-modify-write over the creator session's
/// `legionDeployTimes` custom metadata. Without serialization, two concurrent
/// loads can both read an empty history, both pass the cap check, and both
/// deploy — the limit degrades to best-effort. Keyed by the normalized
/// deployment workspace + creator session id so different creators (or
/// different workspaces) never contend, while the same creator's concurrent
/// loads are serialized. The lock covers the check *and* the reservation write
/// (below), so an in-flight deployment is already counted by the next load.
static LEGION_DEPLOY_LOCKS: OnceLock<KeyedAsyncLock> = OnceLock::new();

fn legion_deploy_locks() -> &'static KeyedAsyncLock {
    LEGION_DEPLOY_LOCKS.get_or_init(KeyedAsyncLock::default)
}

/// Resolve the effective per-topology node cap.
///
/// Reads `ai.legion_max_nodes` from the global config service; any read
/// failure or a value below 1 (meaningless for a per-topology cap) falls back
/// to the default. A config value is always clamped to a valid range so a
/// front-end misconfiguration can never accidentally disable the cap.
async fn resolve_legion_max_nodes() -> usize {
    match get_global_config_service().await {
        Ok(service) => match service
            .get_config::<usize>(Some("ai.legion_max_nodes"))
            .await
        {
            Ok(value) if value > 0 => value,
            _ => default_legion_max_nodes(),
        },
        Err(_) => default_legion_max_nodes(),
    }
}

/// Resolve the effective cross-deployment total node cap.
///
/// Reads `ai.legion_max_total_nodes` from the global config service; any read
/// failure or a value below 1 (would reject every deployment) falls back to
/// the default.
async fn resolve_legion_max_total_nodes() -> usize {
    match get_global_config_service().await {
        Ok(service) => match service
            .get_config::<usize>(Some("ai.legion_max_total_nodes"))
            .await
        {
            Ok(value) if value > 0 => value,
            _ => default_legion_max_total_nodes(),
        },
        Err(_) => default_legion_max_total_nodes(),
    }
}

/// Resolve the effective deployment frequency cap per creator per hour.
///
/// Reads `ai.legion_deploy_frequency_per_hour` from the global config service;
/// any read failure falls back to the default. `0` means unlimited (the config
/// value is passed through unchanged).
async fn resolve_legion_deploy_frequency_per_hour() -> usize {
    match get_global_config_service().await {
        Ok(service) => match service
            .get_config::<usize>(Some("ai.legion_deploy_frequency_per_hour"))
            .await
        {
            Ok(value) => value,
            Err(_) => default_legion_deploy_frequency_per_hour(),
        },
        Err(_) => default_legion_deploy_frequency_per_hour(),
    }
}

fn current_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

/// Prune a `legionDeployTimes` history to the one-hour sliding window and
/// decide whether a new deployment would exceed `frequency_per_hour` (UX-P1-5).
///
/// Pure helper extracted so the frequency-limit decision is unit-testable
/// without a coordinator: the caller holds the per-(workspace, creator)
/// [`legion_deploy_locks`] guard while running this read + the reservation
/// write, which is what makes the check-and-reserve atomic.
fn frequency_limit_reached(
    deploy_times: &mut Vec<i64>,
    now: i64,
    frequency_per_hour: usize,
) -> bool {
    deploy_times.retain(|timestamp| *timestamp >= now - LEGION_DEPLOY_WINDOW_SECS);
    deploy_times.len() >= frequency_per_hour
}

/// Remove `reserved_timestamp` from a `legionDeployTimes` history while
/// pruning stale entries (UX-P1-5 rollback; pure helper for tests).
fn rollback_deploy_timestamp_from_history(
    deploy_times: &mut Vec<i64>,
    now: i64,
    reserved_timestamp: i64,
) {
    deploy_times.retain(|timestamp| {
        *timestamp != reserved_timestamp && *timestamp >= now - LEGION_DEPLOY_WINDOW_SECS
    });
}

/// LegionControl tool - deploy a legion team topology into persisted sessions.
pub struct LegionControlTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegionControlAction {
    Load,
    List,
    Save,
    Delete,
}

impl LegionControlAction {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "load" => Some(Self::Load),
            "list" => Some(Self::List),
            "save" => Some(Self::Save),
            "delete" => Some(Self::Delete),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LegionNodeOverride {
    pub agent: Option<String>,
    pub role: Option<String>,
    pub prompt: Option<String>,
    pub gate: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LegionControlInput {
    pub action: String,
    /// Accepts `preset_id` (canonical) and the legacy alias `legion_id`
    /// (d2-P1-2: legion_mode.md historically taught the model `legion_id`;
    /// the alias keeps old prompts working while the contract is unified).
    #[serde(alias = "legion_id")]
    pub preset_id: Option<String>,
    /// Inline preset definition for the `save` action (d2-P2-1): a full
    /// `LegionPreset` (id/name/description/nodes/edges) persisted via
    /// `team_presets::create_preset`, giving LegionControl a runtime preset
    /// creation entry point. Mutually exclusive with `nodes`/`edges`.
    #[serde(default)]
    pub preset: Option<LegionPreset>,
    #[serde(default)]
    pub overrides: HashMap<String, LegionNodeOverride>,
    pub nodes: Option<Vec<LegionNode>>,
    #[serde(default)]
    pub edges: Vec<LegionEdge>,
}

/// A node resolved for deployment: topologically sorted with depth and parent.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedLegionNode {
    pub node: LegionNode,
    pub depth: u32,
    pub parent: Option<String>,
}

impl Default for LegionControlTool {
    fn default() -> Self {
        Self::new()
    }
}

impl LegionControlTool {
    pub fn new() -> Self {
        Self
    }

    /// Apply per-node overrides (keyed by node id) to a topology.
    pub(crate) fn apply_legion_node_overrides(
        mut nodes: Vec<LegionNode>,
        overrides: &HashMap<String, LegionNodeOverride>,
    ) -> Vec<LegionNode> {
        for node in nodes.iter_mut() {
            if let Some(over) = overrides.get(&node.id) {
                if let Some(agent) = &over.agent {
                    node.agent = agent.clone();
                }
                if let Some(role) = &over.role {
                    node.role = role.clone();
                }
                if let Some(prompt) = &over.prompt {
                    node.prompt = prompt.clone();
                }
                if let Some(gate) = over.gate {
                    node.gate = gate;
                }
            }
        }
        nodes
    }

    /// Validate a legion topology and resolve a deterministic deployment order.
    ///
    /// Rejects: empty topologies, empty node ids/agents, daemon/warden agents,
    /// duplicate ids, edges referencing unknown nodes, self-loops, nodes with
    /// more than one parent, and cycles.
    ///
    /// `max_nodes` is the effective per-topology node cap (from
    /// `ai.legion_max_nodes`, 前端可配置）；passing the fallback default keeps
    /// the legacy hard-coded behavior.
    ///
    /// Returns nodes in topological order (deterministic: lexicographically
    /// smallest ready node first) with depth (root = 0) and parent node id.
    pub(crate) fn resolve_legion_topology(
        nodes: Vec<LegionNode>,
        edges: Vec<LegionEdge>,
        max_nodes: usize,
    ) -> Result<Vec<ResolvedLegionNode>, String> {
        if nodes.is_empty() {
            return Err("Legion topology must contain at least one node".to_string());
        }
        if nodes.len() > max_nodes {
            return Err(format!(
                "Legion topology exceeds the maximum node count ({} > {})",
                nodes.len(),
                max_nodes
            ));
        }

        // 1. Basic node validation
        let mut ids = BTreeSet::new();
        for node in &nodes {
            if node.id.trim().is_empty() {
                return Err("Legion node id must not be empty".to_string());
            }
            if node.agent.trim().is_empty() {
                return Err(format!("Legion node '{}' has an empty agent type", node.id));
            }
            if node.agent == "daemon" || node.agent.starts_with("warden-") {
                return Err(format!(
                    "Legion node '{}' uses protected agent '{}' (daemon/warden agents cannot be controlled)",
                    node.id, node.agent
                ));
            }
            if !ids.insert(node.id.clone()) {
                return Err(format!("Duplicate legion node id '{}'", node.id));
            }
        }

        // 2. Edge validation: endpoints exist, no self-loops, at most one parent
        let mut parents: HashMap<String, String> = HashMap::new();
        for edge in &edges {
            if !ids.contains(&edge.from) {
                return Err(format!(
                    "Legion edge references unknown node '{}'",
                    edge.from
                ));
            }
            if !ids.contains(&edge.to) {
                return Err(format!("Legion edge references unknown node '{}'", edge.to));
            }
            if edge.from == edge.to {
                return Err(format!(
                    "Legion edge has a self-loop on node '{}'",
                    edge.from
                ));
            }
            if parents.insert(edge.to.clone(), edge.from.clone()).is_some() {
                return Err(format!(
                    "Legion node '{}' has multiple parents; each node may have at most one parent",
                    edge.to
                ));
            }
        }

        // 3. Kahn topological sort with deterministic (lexicographic) order
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for node in &nodes {
            adjacency.insert(node.id.clone(), Vec::new());
            in_degree.insert(node.id.clone(), 0);
        }
        for edge in &edges {
            let nexts = adjacency
                .get_mut(&edge.from)
                .ok_or_else(|| format!("Internal error: missing adjacency for '{}'", edge.from))?;
            nexts.push(edge.to.clone());
            let degree = in_degree
                .get_mut(&edge.to)
                .ok_or_else(|| format!("Internal error: missing in-degree for '{}'", edge.to))?;
            *degree += 1;
        }

        let mut ready: BTreeSet<String> = nodes
            .iter()
            .filter(|node| in_degree.get(&node.id).copied().unwrap_or(usize::MAX) == 0)
            .map(|node| node.id.clone())
            .collect();

        let mut order: Vec<String> = Vec::with_capacity(nodes.len());
        while let Some(id) = ready.iter().next().cloned() {
            ready.remove(&id);
            order.push(id.clone());
            let nexts = adjacency
                .get(&id)
                .cloned()
                .ok_or_else(|| format!("Internal error: missing adjacency for '{id}'"))?;
            for next in nexts {
                let degree = in_degree
                    .get_mut(&next)
                    .ok_or_else(|| format!("Internal error: missing in-degree for '{next}'"))?;
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(next);
                }
            }
        }
        if order.len() != nodes.len() {
            return Err("Legion topology contains a cycle".to_string());
        }

        // 4. Depth: root = 0, child = parent depth + 1 (parents precede children
        //    in topological order, so the parent depth is always known)
        let nodes_by_id: HashMap<String, LegionNode> = nodes
            .into_iter()
            .map(|node| (node.id.clone(), node))
            .collect();
        let mut depth_by_id: HashMap<String, u32> = HashMap::new();
        for id in &order {
            let depth = match parents.get(id) {
                Some(parent_id) => {
                    let parent_depth = depth_by_id.get(parent_id).copied().ok_or_else(|| {
                        format!("Internal error: missing depth for parent '{parent_id}'")
                    })?;
                    parent_depth + 1
                }
                None => 0,
            };
            depth_by_id.insert(id.clone(), depth);
        }

        let mut resolved = Vec::with_capacity(order.len());
        for id in order {
            let node = nodes_by_id
                .get(&id)
                .cloned()
                .ok_or_else(|| format!("Internal error: missing node '{id}'"))?;
            let depth = depth_by_id
                .get(&id)
                .copied()
                .ok_or_else(|| format!("Internal error: missing depth for '{id}'"))?;
            resolved.push(ResolvedLegionNode {
                node,
                depth,
                parent: parents.get(&id).cloned(),
            });
        }
        Ok(resolved)
    }

    /// Persist the session lineage and register the child in the in-memory
    /// session tree.
    ///
    /// SESSION-03-aligned (d2-P1-3): lineage persistence failure is retried
    /// once to absorb transient IO faults; if it still fails, the just-created
    /// session is rolled back (deleted) and the error is propagated so a
    /// session without a persisted parent relationship never silently becomes
    /// an orphan. A `register_child` failure (in-memory tree only, rebuilds on
    /// restart from persisted lineage) is logged and tolerated.
    ///
    /// Returns the created session id on success (unchanged), the original
    /// error when lineage persistence failed after retry.
    async fn attach_session_to_tree(
        coordinator: &ConversationCoordinator,
        workspace_path: &std::path::Path,
        created_session_id: &str,
        parent_session_id: Option<&str>,
        child_depth: u32,
    ) -> Result<(), BitFunError> {
        let relationship = SessionRelationship {
            kind: Some(SessionRelationshipKind::Subagent),
            parent_session_id: parent_session_id.map(ToOwned::to_owned),
            depth: Some(child_depth),
            ..Default::default()
        };
        let mut lineage_result = coordinator
            .session_manager
            .persist_session_lineage(created_session_id, relationship.clone())
            .await;
        if lineage_result.is_err() {
            log::warn!(
                "LegionControl load: lineage persist failed for {}, retrying once: {:?}",
                created_session_id,
                lineage_result.as_ref().err()
            );
            lineage_result = coordinator
                .session_manager
                .persist_session_lineage(created_session_id, relationship)
                .await;
        }
        if let Err(e) = lineage_result {
            // Roll back the just-created session so no orphan (created but
            // without a persisted parent relationship) survives; the node is
            // also removed from the deployment's session_by_node accounting
            // by the caller on error return.
            if let Err(rollback_error) = coordinator
                .session_manager
                .delete_session(workspace_path, created_session_id)
                .await
            {
                log::error!(
                    "LegionControl load: lineage persist failed for {} ({:?}), rollback of session also failed: {:?}",
                    created_session_id, e, rollback_error
                );
            }
            return Err(BitFunError::tool(format!(
                "LegionControl load: failed to persist session lineage for {} after retry: {}",
                created_session_id, e
            )));
        }
        if let Some(pid) = parent_session_id {
            // Depth semantics (d2-P2-5): the deployment loop validates
            // `child_depth <= session_tree().max_depth` BEFORE creating the
            // session, so every depth passed here is already within bounds.
            // `SessionTreeManager::register_child` clamps (rather than
            // rejects) an over-limit depth as a last-resort defensive guard
            // for non-LegionControl callers; it cannot silently relocate this
            // node because the tool-layer check runs first. Keep the two
            // layers in sync if the max-depth policy ever changes.
            if let Err(e) =
                coordinator
                    .session_tree()
                    .register_child(pid, created_session_id, child_depth)
            {
                log::warn!(
                    "LegionControl load: failed to register child {} under {} in tree: {:?}",
                    created_session_id,
                    pid,
                    e
                );
            }
        }
        Ok(())
    }

    /// Roll back a partially deployed legion.
    ///
    /// When a later node fails its pre-create checks or its session creation,
    /// every session already persisted earlier in this deployment is deleted so
    /// a failed LegionControl load never leaks orphaned sessions. Best-effort:
    /// a deletion failure is logged and never masks the original error.
    ///
    /// Tree cleanup (L1-P2-2): `delete_session` removes the persisted session
    /// but does not touch the in-memory `SessionTreeManager` edges, so a rolled
    /// back deployment would leave dangling parent->child entries (the tree
    /// rebuilds from persisted lineage on restart, but within the current
    /// process the stale edges would keep referencing deleted session ids).
    /// `remove_subtree` removes the node and all of its registered descendants
    /// from the in-memory tree, mirroring the deployment rollback exactly.
    async fn cleanup_deployed_sessions(
        coordinator: &ConversationCoordinator,
        workspace_path: &std::path::Path,
        session_ids: &[String],
    ) {
        for session_id in session_ids {
            if let Err(e) = coordinator
                .session_manager
                .delete_session(workspace_path, session_id)
                .await
            {
                log::warn!(
                    "LegionControl load: failed to clean up session {} after deployment failure: {:?}",
                    session_id,
                    e
                );
            }
            coordinator.session_tree().remove_subtree(session_id);
        }
    }

    /// Remove a reserved deployment-frequency timestamp from the creator
    /// session's `legionDeployTimes` metadata (UX-P1-5 rollback).
    ///
    /// The frequency reservation is written before the creation loop starts,
    /// so a failed deployment (depth cap, session creation, or lineage attach
    /// rollback) must undo it — otherwise a failed load would consume one
    /// deployment slot forever. Best-effort: a rollback failure only logs (the
    /// original deployment error is never masked) and the stale timestamp ages
    /// out of the sliding window after `LEGION_DEPLOY_WINDOW_SECS`.
    async fn rollback_deploy_timestamp(
        coordinator: &ConversationCoordinator,
        workspace_path: &std::path::Path,
        creator_session_id: &str,
        reserved_timestamp: i64,
    ) {
        let now = current_unix_secs();
        let creator_metadata = coordinator
            .session_manager
            .load_session_metadata(workspace_path, creator_session_id)
            .await
            .ok()
            .flatten();
        let mut deploy_times: Vec<i64> = creator_metadata
            .as_ref()
            .and_then(|metadata| metadata.custom_metadata.as_ref())
            .and_then(|value| value.get(LEGION_DEPLOY_TIMES_METADATA_KEY))
            .and_then(|value| value.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| entry.as_i64())
                    .collect::<Vec<i64>>()
            })
            .unwrap_or_default();
        rollback_deploy_timestamp_from_history(&mut deploy_times, now, reserved_timestamp);
        let deploy_times_json: Vec<Value> = deploy_times.into_iter().map(Value::from).collect();
        if let Err(e) = coordinator
            .session_manager
            .merge_session_custom_metadata(
                creator_session_id,
                json!({
                    LEGION_DEPLOY_TIMES_METADATA_KEY: deploy_times_json,
                }),
            )
            .await
        {
            log::warn!(
                "LegionControl load: failed to roll back reserved deploy timestamp on creator '{}': {}",
                creator_session_id,
                e
            );
        }
    }
}

#[async_trait]
impl Tool for LegionControlTool {
    fn name(&self) -> &str {
        "LegionControl"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Deploy a legion team topology into a set of persisted agent sessions.

Actions:
- "load": Materialize a legion from a saved preset (preset_id) or an inline topology (nodes/edges). Creates one persisted session per node (SessionControl semantics) and attaches sessions to the session tree along the edges. Returns the deployed topology with session ids.
- "list": List saved legion presets (id, name, description, node/edge counts).
- "save": Persist a legion preset (preset) to the user-config legions directory. Returns the saved preset. (Runtime creation entry point, d2-P2-1.)
- "delete": Remove a saved legion preset by id (preset_id). Fails if the preset does not exist.

Arguments:
- "preset_id": Id of a saved legion preset. Used by load/delete; mutually exclusive with "nodes" for load.
- "preset": Full inline preset definition for "save": {id, name, description, nodes, edges}.
- "overrides": Optional per-node overrides keyed by node id. Each value may set agent, role, prompt, and/or gate.
- "nodes": Inline topology nodes when preset_id is omitted: [{id, agent, role, prompt, gate}]. The per-topology node cap is configurable via `ai.legion_max_nodes` (default 20).
- "edges": Optional parent-child edges: [{from, to, condition}]. Each node may have at most one parent; cycles are rejected.

Notes:
- Agent types are validated against the available agent registry (same as SessionControl).
- daemon/warden agents cannot be deployed through LegionControl.
- Nodes are sorted topologically (deterministic order) and deployed root-first.
- node.role, node.prompt, node.gate, and edge.condition are reserved fields: they are persisted into the created session metadata (legionRole / legionNodePrompt / legionNodeGate) and echoed in the result for observability, but do not yet change runtime behavior. In particular node.role is metadata only — the deployed session's RBAC role is always determined by the standard subagent role resolution (Executor for subagent-marked sessions), never by legionRole (d2-P2-2).
- Saving a preset via "save" persists the same reserved fields into the preset JSON file.

Related tools:
- Use SessionControl to manage the created sessions (cancel/delete/list).
- Use SessionMessage to drive the deployed sessions.
- Use Team mode to operate inside a pre-deployed legion."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Deploy a legion team topology into persisted agent sessions.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["load", "list", "save", "delete"],
                    "description": "The legion action to perform: \"load\" deploys a preset or inline topology into sessions; \"list\" lists saved presets; \"save\" persists a full preset definition; \"delete\" removes a saved preset by id."
                },
                "preset_id": {
                    "type": "string",
                    "description": "Id of a saved legion preset. Used by load/delete; mutually exclusive with \"nodes\" for load. (Legacy alias: \"legion_id\" is also accepted.)"
                },
                "preset": {
                    "type": "object",
                    "description": "Full inline preset definition for \"save\": {id, name, description, nodes, edges}.",
                    "properties": {
                        "id": { "type": "string" },
                        "name": { "type": "string" },
                        "description": { "type": "string" },
                        "nodes": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string" },
                                    "agent": { "type": "string" },
                                    "role": { "type": "string" },
                                    "prompt": { "type": "string" },
                                    "gate": { "type": "boolean" }
                                },
                                "required": ["id", "agent"]
                            }
                        },
                        "edges": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "from": { "type": "string" },
                                    "to": { "type": "string" },
                                    "condition": { "type": "string" }
                                },
                                "required": ["from", "to"]
                            }
                        }
                    },
                    "required": ["id", "name", "nodes"]
                },
                "overrides": {
                    "type": "object",
                    "description": "Optional per-node overrides keyed by node id. Each value may set agent, role, prompt, and/or gate.",
                    "additionalProperties": {
                        "type": "object",
                        "properties": {
                            "agent": { "type": "string" },
                            "role": { "type": "string" },
                            "prompt": { "type": "string" },
                            "gate": { "type": "boolean" }
                        }
                    }
                },
                "nodes": {
                    "type": "array",
                    "description": "Inline topology nodes when preset_id is not given: [{id, agent, role, prompt, gate}].",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "agent": { "type": "string" },
                            "role": { "type": "string" },
                            "prompt": { "type": "string" },
                            "gate": { "type": "boolean" }
                        },
                        "required": ["id", "agent"]
                    }
                },
                "edges": {
                    "type": "array",
                    "description": "Optional parent-child edges between nodes: [{from, to, condition}]. Each node may have at most one parent.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "from": { "type": "string" },
                            "to": { "type": "string" },
                            "condition": { "type": "string" }
                        },
                        "required": ["from", "to"]
                    }
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: LegionControlInput = match serde_json::from_value(input.clone()) {
            Ok(value) => value,
            Err(err) => {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid input: {}", err)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        let action = match LegionControlAction::from_str(&parsed.action) {
            Some(action) => action,
            None => {
                return ValidationResult {
                    result: false,
                    message: Some(format!(
                        "Invalid action '{}': expected one of load, list, save, delete",
                        parsed.action
                    )),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        if action == LegionControlAction::Load {
            match (&parsed.preset_id, &parsed.nodes) {
                (Some(_), Some(_)) => {
                    return ValidationResult {
                        result: false,
                        message: Some("preset_id and nodes are mutually exclusive".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
                (None, None) => {
                    return ValidationResult {
                        result: false,
                        message: Some("load requires either preset_id or nodes".to_string()),
                        error_code: Some(400),
                        meta: None,
                    };
                }
                _ => {}
            }

            // Reject inline topologies larger than the effective node cap at
            // validation time so an oversized request never reaches deployment.
            // resolve_legion_topology applies the same bound as a second guard.
            // The cap is front-end configurable (`ai.legion_max_nodes`); an
            // unset config resolves to the legacy default (legion 阈值参数配置化）。
            //
            // UX-P1-4 TOCTOU note: this check is an *early-reject* hint only.
            // `validate_input` and `call_impl` are independent framework calls
            // with no shared state, so the two resolve their own `max_nodes`.
            // The authoritative bound is enforced inside `call_impl`, which
            // resolves `max_nodes` exactly once and passes it into
            // `resolve_legion_topology` (the same value guards validation and
            // deployment within a single dispatch — see the load branch
            // below). A config hot-update between validate and call therefore
            // cannot bypass the cap: execution always uses the value resolved
            // at dispatch time.
            let max_nodes = resolve_legion_max_nodes().await;
            if let Some(nodes) = &parsed.nodes {
                if nodes.len() > max_nodes {
                    return ValidationResult {
                        result: false,
                        message: Some(format!(
                            "Legion topology exceeds the maximum node count ({} > {})",
                            nodes.len(),
                            max_nodes
                        )),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            }
        } else if action == LegionControlAction::Save {
            let Some(preset) = &parsed.preset else {
                return ValidationResult {
                    result: false,
                    message: Some("save requires a full preset definition".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            };
            if preset.id.trim().is_empty() {
                return ValidationResult {
                    result: false,
                    message: Some("save requires a non-empty preset id".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
            if preset.nodes.is_empty() {
                return ValidationResult {
                    result: false,
                    message: Some("save requires at least one node in the preset".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
            let max_nodes = resolve_legion_max_nodes().await;
            if preset.nodes.len() > max_nodes {
                return ValidationResult {
                    result: false,
                    message: Some(format!(
                        "Legion preset exceeds the maximum node count ({} > {})",
                        preset.nodes.len(),
                        max_nodes
                    )),
                    error_code: Some(400),
                    meta: None,
                };
            }
            // Reuse topology resolution for structural validation (cycles,
            // duplicate ids, unknown edge endpoints, protected agents).
            if let Err(message) =
                Self::resolve_legion_topology(preset.nodes.clone(), preset.edges.clone(), max_nodes)
            {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid preset topology: {message}")),
                    error_code: Some(400),
                    meta: None,
                };
            }
        } else if action == LegionControlAction::Delete && parsed.preset_id.is_none() {
            return ValidationResult {
                result: false,
                message: Some("delete requires preset_id".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let action = input
            .get("action")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        match LegionControlAction::from_str(action) {
            Some(LegionControlAction::Load) => {
                if let Some(preset_id) = input.get("preset_id").and_then(|v| v.as_str()) {
                    format!("Deploy legion from preset {preset_id}")
                } else {
                    "Deploy legion from inline topology".to_string()
                }
            }
            Some(LegionControlAction::List) => "List available legion presets".to_string(),
            Some(LegionControlAction::Save) => "Save legion preset".to_string(),
            Some(LegionControlAction::Delete) => {
                let preset_id = input
                    .get("preset_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                format!("Delete legion preset {preset_id}")
            }
            None => "Deploy legion".to_string(),
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: LegionControlInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;
        let action = LegionControlAction::from_str(&params.action).ok_or_else(|| {
            BitFunError::tool(format!(
                "Invalid action '{}': expected one of load, list, save, delete",
                params.action
            ))
        })?;

        match action {
            LegionControlAction::List => {
                let presets = list_presets().map_err(BitFunError::tool)?;
                let preset_summaries: Vec<Value> = presets
                    .iter()
                    .map(|preset| {
                        json!({
                            "id": preset.id,
                            "name": preset.name,
                            "description": preset.description,
                            "node_count": preset.nodes.len(),
                            "edge_count": preset.edges.len(),
                        })
                    })
                    .collect();
                let result_for_assistant = format!("{} legion preset(s) available", presets.len());
                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "list",
                        "presets": preset_summaries,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            LegionControlAction::Save => {
                let preset = params.preset.ok_or_else(|| {
                    BitFunError::tool("save requires a full preset definition".to_string())
                })?;
                if preset.id.trim().is_empty() {
                    return Err(BitFunError::tool(
                        "save requires a non-empty preset id".to_string(),
                    ));
                }
                // Structural validation mirrors load: reject malformed
                // topologies (cycles/duplicate ids/unknown endpoints/protected
                // agents) before anything is persisted (d2-P2-1). The node cap
                // is front-end configurable (`ai.legion_max_nodes`).
                let max_nodes = resolve_legion_max_nodes().await;
                Self::resolve_legion_topology(
                    preset.nodes.clone(),
                    preset.edges.clone(),
                    max_nodes,
                )
                .map_err(BitFunError::tool)?;
                create_preset(&preset).map_err(BitFunError::tool)?;
                let result_for_assistant = format!("Saved legion preset '{}'", preset.id);
                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "save",
                        "preset": preset,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            LegionControlAction::Delete => {
                let preset_id = params
                    .preset_id
                    .ok_or_else(|| BitFunError::tool("delete requires preset_id".to_string()))?;
                delete_preset(&preset_id).map_err(BitFunError::tool)?;
                let result_for_assistant = format!("Deleted legion preset '{}'", preset_id);
                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "delete",
                        "preset_id": preset_id,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
            LegionControlAction::Load => {
                let workspace = context.workspace.as_ref().ok_or_else(|| {
                    BitFunError::tool("workspace is required for LegionControl load".to_string())
                })?;
                let display_workspace = normalize_path(&workspace.root_path_string());
                let project_workspace = normalize_path(&workspace.project_root_path_string());

                // Resolve source topology: saved preset or inline input
                let (preset_id, mut nodes, edges) = match (&params.preset_id, &params.nodes) {
                    (Some(preset_id), None) => {
                        let preset = get_preset(preset_id).map_err(BitFunError::tool)?;
                        (Some(preset_id.clone()), preset.nodes, preset.edges)
                    }
                    (None, Some(nodes)) => (None, nodes.clone(), params.edges.clone()),
                    (Some(_), Some(_)) => {
                        return Err(BitFunError::tool(
                            "preset_id and nodes are mutually exclusive".to_string(),
                        ));
                    }
                    (None, None) => {
                        return Err(BitFunError::tool(
                            "load requires either preset_id or nodes".to_string(),
                        ));
                    }
                };

                nodes = Self::apply_legion_node_overrides(nodes, &params.overrides);
                // Effective thresholds are front-end configurable
                // (`ai.legion_max_nodes` / `ai.legion_max_total_nodes` /
                // `ai.legion_deploy_frequency_per_hour`); unset values resolve
                // to the legacy hard-coded defaults (legion 阈值参数配置化，
                // 默认路径零回归).
                //
                // UX-P1-4: `max_nodes` is resolved exactly once per dispatch
                // and passed into `resolve_legion_topology` below — the same
                // value guards both structural validation and the deployment,
                // so a config hot-update between this resolution and the
                // creation loop cannot make validation and execution disagree.
                let max_nodes = resolve_legion_max_nodes().await;
                let max_total_nodes = resolve_legion_max_total_nodes().await;
                let frequency_per_hour = resolve_legion_deploy_frequency_per_hour().await;
                let topology = Self::resolve_legion_topology(nodes, edges.clone(), max_nodes)
                    .map_err(BitFunError::tool)?;

                // Validate agent types against the available agent registry,
                // resolved against the *deployment* workspace (display_workspace)
                // rather than the calling context's workspace (d2-P2-4).
                // Legion nodes are created in the deployment workspace, so a
                // project-scoped custom agent from that workspace must be
                // visible; validating against the caller's workspace would
                // wrongly reject cross-workspace project agents. Builtin/user
                // agents are workspace-independent and unaffected.
                let registry = crate::agentic::agents::get_agent_registry();
                registry
                    .load_custom_agents(Some(std::path::Path::new(&display_workspace)))
                    .await;
                let available_agent_ids = registry
                    .get_agent_ids_for_session_creation(Some(std::path::Path::new(
                        &display_workspace,
                    )))
                    .await;
                for resolved in &topology {
                    if !available_agent_ids.contains(&resolved.node.agent) {
                        return Err(BitFunError::tool(format!(
                            "Unknown agent type '{}' for legion node '{}'",
                            resolved.node.agent, resolved.node.id
                        )));
                    }
                }

                let coordinator = get_global_coordinator()
                    .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
                let runtime = CoreServiceAgentRuntime::agent_runtime(coordinator.clone())
                    .map_err(BitFunError::tool)?;

                let creator_session_id = context.session_id.as_ref().ok_or_else(|| {
                    BitFunError::tool("load requires a creator session in tool context".to_string())
                })?;

                // Role-based delegation validation before any session
                // is created, using the same criteria as SessionControl create
                // (R-14 B3). An executor/reviewer creator may only deploy its own
                // role; the permissive commander baseline applies when the creator
                // has no registered role.
                let creator_role = context.session_id.as_deref().and_then(get_session_role);
                let target_role = creator_role.clone().unwrap_or(AgentRole::Commander);
                validate_delegation(creator_role, target_role)?;

                // The creator session's tree depth anchors the deployed legion:
                // every root node is a direct child of the creator, and each
                // deeper node adds its resolved topology depth on top. This is
                // deterministic and avoids re-reading freshly persisted lineage
                // metadata for every node.
                //
                // A read failure fails fast instead of silently
                // degrading the depth anchor to 0, which would deploy the legion
                // at the wrong session-tree depth. A missing relationship/missing
                // metadata (fresh session) is not a failure: it degrades to 0 with
                // an explicit warning.
                let creator_depth = match coordinator
                    .session_manager
                    .load_session_metadata(
                        &std::path::PathBuf::from(&display_workspace),
                        creator_session_id,
                    )
                    .await
                {
                    Ok(Some(metadata)) => metadata
                        .relationship
                        .and_then(|relationship| relationship.depth)
                        .unwrap_or_else(|| {
                            log::warn!(
                                "LegionControl load: creator session '{}' has no persisted depth; anchoring legion at depth 0",
                                creator_session_id
                            );
                            0
                        }),
                    Ok(None) => {
                        log::warn!(
                            "LegionControl load: creator session '{}' has no persisted metadata; anchoring legion at depth 0",
                            creator_session_id
                        );
                        0
                    }
                    Err(e) => {
                        return Err(BitFunError::tool(format!(
                            "LegionControl load: failed to read creator session metadata for '{}': {}",
                            creator_session_id, e
                        )));
                    }
                };

                let mut session_by_node: HashMap<String, String> = HashMap::new();
                let mut deployed: Vec<Value> = Vec::with_capacity(topology.len());

                // Deployment frequency limit (legion 阈值参数配置化，
                // `ai.legion_deploy_frequency_per_hour`，默认 10 次/小时):
                // each successful load appends a Unix-second timestamp to the
                // creator session's `legionDeployTimes` custom metadata. A
                // one-hour sliding window counts timestamps newer than
                // `now - LEGION_DEPLOY_WINDOW_SECS`; when the count would
                // reach the cap the load is rejected BEFORE any session is
                // created. `0` disables the limit.
                //
                // UX-P1-5 atomicity: the check and the reservation write run
                // under `legion_deploy_locks()` keyed by (workspace, creator).
                // The timestamp is reserved *before* deployment begins (inside
                // the lock), so a concurrent load of the same creator cannot
                // both pass the check — the in-flight deployment is already
                // counted. A metadata read failure is treated as an empty
                // history (never blocks a first load); a reservation
                // persistence failure fails the load closed instead of
                // silently deploying without a counter. On deployment
                // rollback the reserved timestamp is removed (best-effort),
                // so a failed load never leaves a phantom count behind.
                let mut reserved_deploy_timestamp: Option<i64> = None;
                if frequency_per_hour > 0 {
                    let deploy_lock_key = format!("{display_workspace}:{creator_session_id}");
                    let _deploy_guard = legion_deploy_locks().lock(&deploy_lock_key).await;
                    let now = current_unix_secs();
                    let creator_metadata = coordinator
                        .session_manager
                        .load_session_metadata(
                            &std::path::PathBuf::from(&display_workspace),
                            creator_session_id,
                        )
                        .await
                        .ok()
                        .flatten();
                    let mut deploy_times: Vec<i64> = creator_metadata
                        .as_ref()
                        .and_then(|metadata| metadata.custom_metadata.as_ref())
                        .and_then(|value| value.get(LEGION_DEPLOY_TIMES_METADATA_KEY))
                        .and_then(|value| value.as_array())
                        .map(|entries| {
                            entries
                                .iter()
                                .filter_map(|entry| entry.as_i64())
                                .collect::<Vec<i64>>()
                        })
                        .unwrap_or_default();
                    if frequency_limit_reached(&mut deploy_times, now, frequency_per_hour) {
                        return Err(BitFunError::tool(format!(
                            "LegionControl load: deployment frequency limit reached: {} deployment(s) within the last hour, exceeding the cap {} (configured via ai.legion_deploy_frequency_per_hour)",
                            deploy_times.len(),
                            frequency_per_hour
                        )));
                    }
                    deploy_times.push(now);
                    reserved_deploy_timestamp = Some(now);
                    let deploy_times_json: Vec<Value> =
                        deploy_times.into_iter().map(Value::from).collect();
                    if let Err(e) = coordinator
                        .session_manager
                        .merge_session_custom_metadata(
                            creator_session_id,
                            json!({
                                LEGION_DEPLOY_TIMES_METADATA_KEY: deploy_times_json,
                            }),
                        )
                        .await
                    {
                        // Fail closed: without a durable reservation the next
                        // concurrent load could bypass the frequency cap.
                        return Err(BitFunError::tool(format!(
                            "LegionControl load: failed to reserve deployment timestamp on creator '{}': {}",
                            creator_session_id, e
                        )));
                    }
                }

                // Cross-deployment aggregate cap (d2-P2-3 + UX-P1-5): the
                // per-topology cap only bounds a single call; repeated loads
                // plus nested legion fission could otherwise accumulate an
                // unbounded fleet of persisted subagent sessions. The count is
                // *workspace-dimensional* (all persisted legion node sessions
                // in the deployment workspace, across every nested layer),
                // because nested legions deploy their children as independent
                // creators — a creator-subtree count would let recursive
                // fission exceed `ai.legion_max_total_nodes` layer by layer.
                // Reject the deployment before any session is created when
                // adding `topology.len()` would exceed the effective total
                // cap. The check runs before the creation loop, so a rejected
                // load never leaves a partial deployment behind.
                let existing_legion_nodes = coordinator
                    .session_manager
                    .count_workspace_legion_node_sessions(std::path::Path::new(&display_workspace))
                    .await
                    .map_err(|e| {
                        BitFunError::tool(format!(
                            "LegionControl load: failed to enumerate workspace legion nodes for aggregate session cap: {}",
                            e
                        ))
                    })?;
                if existing_legion_nodes + topology.len() > max_total_nodes {
                    if let Some(timestamp) = reserved_deploy_timestamp {
                        Self::rollback_deploy_timestamp(
                            &coordinator,
                            &std::path::PathBuf::from(&display_workspace),
                            creator_session_id,
                            timestamp,
                        )
                        .await;
                    }
                    return Err(BitFunError::tool(format!(
                        "LegionControl load: aggregate session cap reached: workspace already holds {} legion node session(s), adding {} would exceed the cap {}",
                        existing_legion_nodes,
                        topology.len(),
                        max_total_nodes
                    )));
                }

                for resolved in &topology {
                    let node = &resolved.node;
                    let session_name = if node.role.trim().is_empty() {
                        node.id.clone()
                    } else {
                        format!("{}-{}", node.role, node.id)
                    };

                    // Resolve the parent and the resulting child depth
                    // BEFORE creating the session so the depth check runs before a
                    // session is persisted. A failing node rolls back every session
                    // created earlier in this deployment.
                    let parent_session_id = match &resolved.parent {
                        Some(parent_node_id) => session_by_node.get(parent_node_id).cloned(),
                        None => Some(creator_session_id.clone()),
                    };
                    let child_depth = creator_depth + 1 + resolved.depth;
                    let max_depth = coordinator.session_tree().max_depth;
                    if child_depth > max_depth {
                        let created: Vec<String> = session_by_node.values().cloned().collect();
                        Self::cleanup_deployed_sessions(
                            &coordinator,
                            &std::path::PathBuf::from(&display_workspace),
                            &created,
                        )
                        .await;
                        if let Some(timestamp) = reserved_deploy_timestamp {
                            Self::rollback_deploy_timestamp(
                                &coordinator,
                                &std::path::PathBuf::from(&display_workspace),
                                creator_session_id,
                                timestamp,
                            )
                            .await;
                        }
                        return Err(BitFunError::tool(format!(
                            "LegionControl load: session depth limit reached for node '{}': child depth {} would exceed max allowed depth {}",
                            node.id, child_depth, max_depth
                        )));
                    }

                    let mut metadata = serde_json::Map::new();
                    metadata.insert(
                        "createdBy".to_string(),
                        json!(session_control_creator_marker(creator_session_id)),
                    );
                    // Legion 节点 = subagent 会话：必须带 subagent 标记族
                    // （subagent=true / parentSessionId / subagentType），
                    // 与 SessionControl create 对齐。缺失时 coordinator 的
                    // resolve_session_role 会因 created_by 是 marker 字符串
                    // （creator 查询恒 None）把节点注册为 Commander（全工具），
                    // 而 restore 时 is_subagent_marked_metadata 又命中
                    // （lineage Subagent）翻为 Executor——同一会话生命周期内
                    // 角色漂移，且 Executor/Reviewer 创建者可部署出高权限会话
                    // （RBAC 越权面，d2-P1-1）。补齐标记后节点创建即 Executor。
                    metadata.insert("subagent".to_string(), json!(true));
                    metadata.insert(
                        "parentSessionId".to_string(),
                        json!(parent_session_id.clone()),
                    );
                    metadata.insert("subagentType".to_string(), json!(node.agent));
                    metadata.insert("legionNodeId".to_string(), json!(node.id));
                    // legionRole 是预留元数据（d2-P2-2）：持久化进会话 metadata
                    // 供下游 SessionMessage 派发与 SessionControl 检视观察，但
                    // **不驱动 RBAC**——节点会话的角色恒由标准 subagent 角色解析
                    // 决定（subagent 标记 → Executor），绝不读取 legionRole 赋权。
                    // 三处语义一致：描述文本（description Notes）/ metadata 注释 /
                    // 本注释。如需让 legionRole 驱动 RBAC，须先改 RBAC 角色解析
                    // 并同步 12-legion军团.md。
                    metadata.insert("legionRole".to_string(), json!(node.role));
                    // `prompt`/`gate` are reserved fields today — they
                    // carry author intent but do not yet change runtime behavior.
                    // Persist them into the session metadata so the data is
                    // observable by downstream SessionMessage dispatch and
                    // SessionControl inspection instead of being silently dropped.
                    if !node.prompt.trim().is_empty() {
                        metadata.insert("legionNodePrompt".to_string(), json!(node.prompt));
                    }
                    metadata.insert("legionNodeGate".to_string(), json!(node.gate));
                    if let Some(ref pid) = preset_id {
                        metadata.insert("legionPresetId".to_string(), json!(pid));
                    }

                    let session = match runtime
                        .create_session(AgentSessionCreateRequest {
                            session_name,
                            agent_type: node.agent.clone(),
                            workspace_path: Some(display_workspace.clone()),
                            project_workspace_path: Some(project_workspace.clone()),
                            execution_target: workspace.execution_target.clone(),
                            workspace_id: workspace.workspace_id.clone(),
                            remote_connection_id: workspace.connection_id().map(ToOwned::to_owned),
                            remote_ssh_host: if workspace.is_remote() {
                                Some(workspace.session_identity.hostname.clone())
                                    .filter(|value| !value.trim().is_empty())
                            } else {
                                None
                            },
                            model_id: None,
                            metadata,
                        })
                        .await
                    {
                        Ok(session) => session,
                        Err(error) => {
                            let created: Vec<String> = session_by_node.values().cloned().collect();
                            Self::cleanup_deployed_sessions(
                                &coordinator,
                                &std::path::PathBuf::from(&display_workspace),
                                &created,
                            )
                            .await;
                            if let Some(timestamp) = reserved_deploy_timestamp {
                                Self::rollback_deploy_timestamp(
                                    &coordinator,
                                    &std::path::PathBuf::from(&display_workspace),
                                    creator_session_id,
                                    timestamp,
                                )
                                .await;
                            }
                            return Err(BitFunError::tool(
                                CoreServiceAgentRuntime::runtime_error_message(error),
                            ));
                        }
                    };

                    let created_session_id = session.session_id.clone();

                    // Attach to the session tree: the parent is the resolved
                    // parent's session; root nodes attach to the creator session.
                    // A lineage-persistence failure (after one retry) rolls back
                    // the node session inside and is propagated here: every
                    // session created earlier in this deployment is also
                    // cleaned up so a failed LegionControl load never leaks
                    // orphaned sessions (d2-P1-3, SESSION-03 semantics).
                    if let Err(error) = Self::attach_session_to_tree(
                        &coordinator,
                        &std::path::PathBuf::from(&display_workspace),
                        &created_session_id,
                        parent_session_id.as_deref(),
                        child_depth,
                    )
                    .await
                    {
                        let created: Vec<String> = session_by_node.values().cloned().collect();
                        Self::cleanup_deployed_sessions(
                            &coordinator,
                            &std::path::PathBuf::from(&display_workspace),
                            &created,
                        )
                        .await;
                        if let Some(timestamp) = reserved_deploy_timestamp {
                            Self::rollback_deploy_timestamp(
                                &coordinator,
                                &std::path::PathBuf::from(&display_workspace),
                                creator_session_id,
                                timestamp,
                            )
                            .await;
                        }
                        return Err(error);
                    }

                    session_by_node.insert(node.id.clone(), created_session_id.clone());
                    deployed.push(json!({
                        "node_id": node.id,
                        "session_id": created_session_id,
                        "session_name": session.session_name,
                        "role": node.role,
                        "agent": node.agent,
                        "depth": child_depth,
                        // 预留字段在结果中原样回显（与上方会话元数据持久化一致），
                        // 供调用方观察每个节点预期携带的 prompt/gate 语义；尚未改变运行时行为。
                        "prompt": node.prompt,
                        "gate": node.gate,
                    }));
                }

                let edge_outputs: Vec<Value> = edges
                    .iter()
                    .map(|edge| {
                        json!({
                            "from": edge.from,
                            "to": edge.to,
                            "condition": edge.condition,
                            "from_session": session_by_node.get(&edge.from),
                            "to_session": session_by_node.get(&edge.to),
                        })
                    })
                    .collect();

                // The deployment frequency timestamp was already reserved
                // (atomically, under the KeyedAsyncLock) before the creation
                // loop started (UX-P1-5). A successful deployment keeps the
                // reservation as its durable record; a failed deployment rolls
                // it back. Nothing further to write here.

                let result_for_assistant = format!(
                    "Deployed {} legion node(s){}",
                    deployed.len(),
                    preset_id
                        .as_ref()
                        .map(|id| format!(" from preset '{id}'"))
                        .unwrap_or_default()
                );

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "success": true,
                        "action": "load",
                        "preset_id": preset_id,
                        "nodes": deployed,
                        "edges": edge_outputs,
                    }),
                    result_for_assistant: Some(result_for_assistant),
                    image_attachments: None,
                }])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tools::framework::ToolUseContext;
    use std::collections::HashMap;

    fn empty_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    fn node(id: &str) -> LegionNode {
        LegionNode {
            id: id.to_string(),
            agent: "agentic".to_string(),
            role: String::new(),
            prompt: String::new(),
            gate: false,
        }
    }

    fn edge(from: &str, to: &str) -> LegionEdge {
        LegionEdge {
            from: from.to_string(),
            to: to.to_string(),
            condition: None,
        }
    }

    // ── resolve_legion_topology tests ──────────────────────────────────

    #[test]
    fn resolve_topology_sorts_and_computes_depth() {
        // Edges: a->b, a->d, b->c. Input order is intentionally shuffled.
        let nodes = vec![node("c"), node("b"), node("d"), node("a")];
        let edges = vec![edge("a", "b"), edge("a", "d"), edge("b", "c")];

        let resolved = LegionControlTool::resolve_legion_topology(nodes, edges, MAX_LEGION_NODES)
            .expect("topology should resolve");

        let order: Vec<&str> = resolved.iter().map(|r| r.node.id.as_str()).collect();
        // Lexicographic-first ready node: a -> b -> c -> d
        assert_eq!(order, vec!["a", "b", "c", "d"]);

        let by_id: HashMap<&str, &ResolvedLegionNode> =
            resolved.iter().map(|r| (r.node.id.as_str(), r)).collect();
        assert_eq!(by_id["a"].depth, 0);
        assert_eq!(by_id["b"].depth, 1);
        assert_eq!(by_id["c"].depth, 2);
        assert_eq!(by_id["d"].depth, 1);
        assert_eq!(by_id["a"].parent, None);
        assert_eq!(by_id["b"].parent.as_deref(), Some("a"));
        assert_eq!(by_id["c"].parent.as_deref(), Some("b"));
        assert_eq!(by_id["d"].parent.as_deref(), Some("a"));
    }

    #[test]
    fn resolve_topology_rejects_cycle() {
        let nodes = vec![node("a"), node("b"), node("c")];
        let edges = vec![edge("a", "b"), edge("b", "c"), edge("c", "a")];

        let err = LegionControlTool::resolve_legion_topology(nodes, edges, MAX_LEGION_NODES)
            .expect_err("cycle must be rejected");
        assert!(err.contains("cycle"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_rejects_multiple_parents() {
        let nodes = vec![node("a"), node("b"), node("c")];
        let edges = vec![edge("a", "c"), edge("b", "c")];

        let err = LegionControlTool::resolve_legion_topology(nodes, edges, MAX_LEGION_NODES)
            .expect_err("multiple parents must be rejected");
        assert!(err.contains("multiple parents"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_rejects_unknown_endpoint() {
        let nodes = vec![node("a")];
        let edges = vec![edge("a", "z")];

        let err = LegionControlTool::resolve_legion_topology(nodes, edges, MAX_LEGION_NODES)
            .expect_err("unknown endpoint must be rejected");
        assert!(err.contains("unknown node 'z'"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_rejects_duplicate_ids() {
        let mut a = node("a");
        a.agent = "Plan".to_string();
        let nodes = vec![node("a"), a];

        let err = LegionControlTool::resolve_legion_topology(nodes, Vec::new(), MAX_LEGION_NODES)
            .expect_err("duplicate ids must be rejected");
        assert!(
            err.contains("Duplicate legion node id 'a'"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolve_topology_rejects_protected_agents() {
        let mut warden = node("warden-node");
        warden.agent = "warden-auditor".to_string();
        let nodes = vec![warden];

        let err = LegionControlTool::resolve_legion_topology(nodes, Vec::new(), MAX_LEGION_NODES)
            .expect_err("warden agent must be rejected");
        assert!(err.contains("protected agent"), "unexpected error: {err}");

        let mut daemon = node("daemon-node");
        daemon.agent = "daemon".to_string();
        let nodes = vec![daemon];

        let err = LegionControlTool::resolve_legion_topology(nodes, Vec::new(), MAX_LEGION_NODES)
            .expect_err("daemon agent must be rejected");
        assert!(err.contains("protected agent"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_rejects_self_loop() {
        let nodes = vec![node("a")];
        let edges = vec![edge("a", "a")];

        let err = LegionControlTool::resolve_legion_topology(nodes, edges, MAX_LEGION_NODES)
            .expect_err("self-loop must be rejected");
        assert!(err.contains("self-loop"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_rejects_empty_topology() {
        let err =
            LegionControlTool::resolve_legion_topology(Vec::new(), Vec::new(), MAX_LEGION_NODES)
                .expect_err("empty topology must be rejected");
        assert!(err.contains("at least one node"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_rejects_excessive_node_count() {
        // A topology larger than MAX_LEGION_NODES must be rejected so
        // a single LegionControl call cannot spawn an unbounded session fleet.
        let nodes: Vec<LegionNode> = (0..=MAX_LEGION_NODES)
            .map(|index| node(&format!("node-{index}")))
            .collect();
        let err = LegionControlTool::resolve_legion_topology(nodes, Vec::new(), MAX_LEGION_NODES)
            .expect_err("oversized topology must be rejected");
        assert!(
            err.contains("maximum node count"),
            "unexpected error: {err}"
        );

        // The exact maximum still resolves.
        let nodes: Vec<LegionNode> = (0..MAX_LEGION_NODES)
            .map(|index| node(&format!("node-{index}")))
            .collect();
        let resolved =
            LegionControlTool::resolve_legion_topology(nodes, Vec::new(), MAX_LEGION_NODES)
                .expect("topology at the maximum node count should resolve");
        assert_eq!(resolved.len(), MAX_LEGION_NODES);
    }

    #[test]
    fn resolve_topology_rejects_empty_node_fields() {
        let mut empty_id = node("a");
        empty_id.id = "  ".to_string();
        let err = LegionControlTool::resolve_legion_topology(
            vec![empty_id],
            Vec::new(),
            MAX_LEGION_NODES,
        )
        .expect_err("empty id must be rejected");
        assert!(
            err.contains("id must not be empty"),
            "unexpected error: {err}"
        );

        let mut empty_agent = node("a");
        empty_agent.agent = String::new();
        let err = LegionControlTool::resolve_legion_topology(
            vec![empty_agent],
            Vec::new(),
            MAX_LEGION_NODES,
        )
        .expect_err("empty agent must be rejected");
        assert!(err.contains("empty agent type"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_single_root_ok() {
        let nodes = vec![node("a"), node("b")];
        let edges = vec![edge("a", "b")];

        let resolved = LegionControlTool::resolve_legion_topology(nodes, edges, MAX_LEGION_NODES)
            .expect("single root topology should resolve");
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].node.id, "a");
        assert_eq!(resolved[0].depth, 0);
        assert_eq!(resolved[1].node.id, "b");
        assert_eq!(resolved[1].depth, 1);
    }

    #[test]
    fn apply_overrides_per_node() {
        let nodes = vec![node("a"), node("b")];
        let mut overrides = HashMap::new();
        let over_a = LegionNodeOverride {
            agent: Some("Plan".to_string()),
            gate: Some(true),
            ..Default::default()
        };
        overrides.insert("a".to_string(), over_a);

        let applied = LegionControlTool::apply_legion_node_overrides(nodes, &overrides);

        assert_eq!(applied[0].agent, "Plan");
        assert!(applied[0].gate);
        assert_eq!(applied[1].agent, "agentic");
        assert!(!applied[1].gate);
    }

    // ── validate_input tests ───────────────────────────────────────────

    #[tokio::test]
    async fn validate_rejects_missing_action() {
        let tool = LegionControlTool::new();

        let validation = tool
            .validate_input(&json!({}), Some(&empty_context()))
            .await;

        assert!(!validation.result);
        assert_eq!(validation.error_code, Some(400));
    }

    #[tokio::test]
    async fn validate_rejects_unknown_action() {
        let tool = LegionControlTool::new();

        let validation = tool
            .validate_input(&json!({"action": "explode"}), Some(&empty_context()))
            .await;

        assert!(!validation.result);
        let message = validation.message.as_deref().unwrap_or_default();
        assert!(message.contains("explode"), "unexpected message: {message}");
    }

    #[tokio::test]
    async fn validate_load_requires_source() {
        let tool = LegionControlTool::new();

        let validation = tool
            .validate_input(&json!({"action": "load"}), Some(&empty_context()))
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("load requires either preset_id or nodes")
        );
    }

    #[tokio::test]
    async fn validate_load_rejects_dual_source() {
        let tool = LegionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "load",
                    "preset_id": "triad",
                    "nodes": [{"id": "a", "agent": "agentic"}],
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("preset_id and nodes are mutually exclusive")
        );
    }

    #[tokio::test]
    async fn validate_load_with_preset_id_ok() {
        let tool = LegionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({"action": "load", "preset_id": "triad"}),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_load_with_nodes_ok() {
        let tool = LegionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "load",
                    "nodes": [{"id": "a", "agent": "agentic", "role": "commander"}],
                    "edges": [],
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_load_with_overrides_ok() {
        let tool = LegionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "load",
                    "preset_id": "triad",
                    "overrides": {
                        "a": {"agent": "Plan", "gate": true}
                    },
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_rejects_oversized_nodes() {
        // validate_input must reject an inline topology larger than
        // MAX_LEGION_NODES before deployment is attempted.
        let tool = LegionControlTool::new();

        let nodes: Vec<Value> = (0..=MAX_LEGION_NODES)
            .map(|index| {
                json!({
                    "id": format!("node-{index}"),
                    "agent": "agentic",
                })
            })
            .collect();

        let validation = tool
            .validate_input(
                &json!({"action": "load", "nodes": nodes}),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        assert_eq!(validation.error_code, Some(400));
        let message = validation.message.as_deref().unwrap_or_default();
        assert!(
            message.contains("maximum node count"),
            "unexpected message: {message}"
        );

        // The exact maximum still validates.
        let nodes: Vec<Value> = (0..MAX_LEGION_NODES)
            .map(|index| {
                json!({
                    "id": format!("node-{index}"),
                    "agent": "agentic",
                })
            })
            .collect();
        let validation = tool
            .validate_input(
                &json!({"action": "load", "nodes": nodes}),
                Some(&empty_context()),
            )
            .await;
        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_list_ok() {
        let tool = LegionControlTool::new();

        let validation = tool
            .validate_input(&json!({"action": "list"}), Some(&empty_context()))
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_save_requires_preset() {
        let tool = LegionControlTool::new();

        let validation = tool
            .validate_input(&json!({"action": "save"}), Some(&empty_context()))
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("save requires a full preset definition")
        );
    }

    #[tokio::test]
    async fn validate_save_ok_with_valid_preset() {
        let tool = LegionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "save",
                    "preset": {
                        "id": "triad",
                        "name": "Triad",
                        "description": "test",
                        "nodes": [
                            {"id": "a", "agent": "agentic", "role": "commander"},
                            {"id": "b", "agent": "agentic", "role": "executor"}
                        ],
                        "edges": [{"from": "a", "to": "b"}]
                    }
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_save_rejects_cyclic_preset() {
        let tool = LegionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({
                    "action": "save",
                    "preset": {
                        "id": "cycle",
                        "name": "Cycle",
                        "description": "test",
                        "nodes": [
                            {"id": "a", "agent": "agentic"},
                            {"id": "b", "agent": "agentic"}
                        ],
                        "edges": [{"from": "a", "to": "b"}, {"from": "b", "to": "a"}]
                    }
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        let message = validation.message.as_deref().unwrap_or_default();
        assert!(message.contains("cycle"), "unexpected message: {message}");
    }

    #[tokio::test]
    async fn validate_save_rejects_oversized_preset() {
        let tool = LegionControlTool::new();

        let nodes: Vec<Value> = (0..=MAX_LEGION_NODES)
            .map(|index| json!({"id": format!("node-{index}"), "agent": "agentic"}))
            .collect();
        let validation = tool
            .validate_input(
                &json!({
                    "action": "save",
                    "preset": {"id": "big", "name": "Big", "description": "", "nodes": nodes, "edges": []}
                }),
                Some(&empty_context()),
            )
            .await;

        assert!(!validation.result);
        let message = validation.message.as_deref().unwrap_or_default();
        assert!(
            message.contains("maximum node count"),
            "unexpected message: {message}"
        );
    }

    #[tokio::test]
    async fn validate_delete_requires_preset_id() {
        let tool = LegionControlTool::new();

        let validation = tool
            .validate_input(&json!({"action": "delete"}), Some(&empty_context()))
            .await;

        assert!(!validation.result);
        assert_eq!(
            validation.message.as_deref(),
            Some("delete requires preset_id")
        );
    }

    #[tokio::test]
    async fn validate_delete_ok_with_preset_id() {
        let tool = LegionControlTool::new();

        let validation = tool
            .validate_input(
                &json!({"action": "delete", "preset_id": "triad"}),
                Some(&empty_context()),
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    // ── frequency-limit helpers (legion 阈值参数配置化）──────────────────

    #[test]
    fn frequency_window_prunes_stale_timestamps() {
        let now = current_unix_secs();
        let window = LEGION_DEPLOY_WINDOW_SECS;
        let mut times = vec![
            now - window - 5, // just outside the window (stale)
            now - window + 5, // inside the window
            now,              // current
        ];
        times.retain(|timestamp| *timestamp >= now - window);
        assert_eq!(times.len(), 2);
        assert_eq!(times[0], now - window + 5);
        assert_eq!(times[1], now);
    }

    // ── UX-P1-5: frequency limit atomicity helpers ─────────────────────

    #[test]
    fn frequency_limit_helper_rejects_only_at_the_cap() {
        let now = current_unix_secs();
        let window = LEGION_DEPLOY_WINDOW_SECS;
        let mut history = vec![now - window + 1, now];

        // Below the cap: allowed, no mutation besides pruning stale entries.
        assert!(!frequency_limit_reached(&mut history, now, 3));
        assert_eq!(history.len(), 2);

        // Exactly at the cap: rejected.
        history.push(now - 1);
        assert!(frequency_limit_reached(&mut history, now, 3));

        // A stale entry (outside the window) is pruned and no longer counts.
        let mut with_stale = vec![now - window - 100, now, now - 1];
        assert!(frequency_limit_reached(&mut with_stale, now, 2));
        assert_eq!(with_stale.len(), 2, "stale entry must be pruned");
    }

    #[test]
    fn rollback_helper_removes_only_the_reserved_timestamp() {
        let now = current_unix_secs();
        let window = LEGION_DEPLOY_WINDOW_SECS;
        let mut history = vec![now - 100, now, now - window - 1];

        rollback_deploy_timestamp_from_history(&mut history, now, now);

        // The reserved timestamp is removed; the older in-window entry stays;
        // the stale entry is pruned.
        assert_eq!(history, vec![now - 100]);
    }

    #[tokio::test]
    async fn concurrent_loads_of_the_same_creator_are_serialized_by_the_deploy_lock() {
        // UX-P1-5 concurrent-bypass regression: two loads racing on the same
        // (workspace, creator) key must be serialized by the KeyedAsyncLock.
        // Simulate the check-and-reserve critical section: task A acquires the
        // lock and keeps it held (with a freshly reserved timestamp); task B
        // must not be able to enter (and pass its own check) until A releases.
        let key = "workspace-a:creator-1".to_string();
        let locks = legion_deploy_locks();
        let (entered_b_tx, mut entered_b_rx) = tokio::sync::oneshot::channel();
        let (release_a_tx, release_a_rx) = tokio::sync::oneshot::channel::<()>();

        let task_a = {
            let key = key.clone();
            tokio::spawn(async move {
                let _guard = locks.lock(&key).await;
                // Simulate: read history (empty), reserve a timestamp, keep the
                // lock held until the test releases it.
                let _ = release_a_rx.await;
                // Dropping the guard releases the lock.
            })
        };
        let task_b = {
            let key = key.clone();
            tokio::spawn(async move {
                // A second concurrent load for the same creator must block
                // until A releases the lock. Assert that we are *not* able to
                // acquire it while A holds it.
                let _guard = locks.lock(&key).await;
                let _ = entered_b_tx.send(());
            })
        };

        // Give A time to acquire the lock and B time to start waiting.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            entered_b_rx.try_recv().is_err(),
            "task B must not enter the critical section while A holds the deploy lock"
        );

        release_a_tx.send(()).expect("release A");
        let _ = task_a.await.expect("task A");
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), entered_b_rx)
            .await
            .expect("task B must acquire the lock after A releases")
            .expect("B entered");
        let _ = task_b.await.expect("task B");
    }

    #[tokio::test]
    async fn sequential_check_reserve_under_lock_counts_inflight_deployments() {
        // UX-P1-5 regression at the helper level: the production critical
        // section is (lock) read-history → check cap → reserve (push now).
        // Running the same sequence twice *under the same lock* (as the
        // production code does per load) must make the second load observe the
        // first load's reservation and reject once the cap is hit.
        let key = "workspace-a:creator-2".to_string();
        let locks = legion_deploy_locks();
        let now = current_unix_secs();
        let cap = 1usize;

        let mut deploy_times: Vec<i64> = Vec::new();
        for round in 0..2 {
            let _guard = locks.lock(&key).await;
            if frequency_limit_reached(&mut deploy_times, now, cap) {
                assert_eq!(round, 1, "the second load must be rejected");
                return;
            }
            deploy_times.push(now);
            if round == 0 {
                continue;
            }
            panic!("the second load must hit the frequency cap");
        }
    }
}
