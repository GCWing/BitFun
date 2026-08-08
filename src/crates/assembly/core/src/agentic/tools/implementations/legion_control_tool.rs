//! LegionControl deploys a legion team topology into persisted agent sessions.
//!
//! A legion is described by a preset (stored via `team_presets`) or by inline
//! `nodes`/`edges` input. The tool validates the topology (no cycles, at most
//! one parent per node), deploys each node as a persisted session through the
//! same runtime path as SessionControl, and attaches sessions to the session
//! tree along the edges.

use super::util::normalize_path;
use crate::agentic::agents::team_presets::{get_preset, list_presets, LegionEdge, LegionNode};
use crate::agentic::coordination::{get_global_coordinator, ConversationCoordinator};
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::implementations::session_control_tool::get_available_agent_type_ids_for_creation;
use crate::agentic::tools::restrictions::{get_session_role, validate_delegation, AgentRole};
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_agent_runtime::session_control::session_control_creator_marker;
use bitfun_runtime_ports::AgentSessionCreateRequest;
use bitfun_services_core::session::types::{SessionRelationship, SessionRelationshipKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};

/// Hard upper bound on the number of legion nodes in one topology.
///
/// LEGION-03: an unbounded node count lets a single LegionControl call spawn an
/// unbounded number of persisted sessions. 20 keeps the deployment bounded
/// while leaving room for realistic team shapes (the built-in presets use at
/// most a handful of nodes).
const MAX_LEGION_NODES: usize = 20;

/// LegionControl tool - deploy a legion team topology into persisted sessions.
pub struct LegionControlTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegionControlAction {
    Load,
    List,
}

impl LegionControlAction {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "load" => Some(Self::Load),
            "list" => Some(Self::List),
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
    pub preset_id: Option<String>,
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
    /// Returns nodes in topological order (deterministic: lexicographically
    /// smallest ready node first) with depth (root = 0) and parent node id.
    pub(crate) fn resolve_legion_topology(
        nodes: Vec<LegionNode>,
        edges: Vec<LegionEdge>,
    ) -> Result<Vec<ResolvedLegionNode>, String> {
        if nodes.is_empty() {
            return Err("Legion topology must contain at least one node".to_string());
        }
        if nodes.len() > MAX_LEGION_NODES {
            return Err(format!(
                "Legion topology exceeds the maximum node count ({} > {})",
                nodes.len(),
                MAX_LEGION_NODES
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
    /// session tree. Failures are logged but do not fail the deployment.
    async fn attach_session_to_tree(
        coordinator: &ConversationCoordinator,
        created_session_id: &str,
        parent_session_id: Option<&str>,
        child_depth: u32,
    ) {
        let relationship = SessionRelationship {
            kind: Some(SessionRelationshipKind::Subagent),
            parent_session_id: parent_session_id.map(ToOwned::to_owned),
            depth: Some(child_depth),
            ..Default::default()
        };
        if let Err(e) = coordinator
            .session_manager
            .persist_session_lineage(created_session_id, relationship)
            .await
        {
            log::error!(
                "LegionControl load: failed to persist session lineage for {}: {:?}",
                created_session_id,
                e
            );
        }
        if let Some(pid) = parent_session_id {
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
    }

    /// Roll back a partially deployed legion (LEGION-01).
    ///
    /// When a later node fails its pre-create checks or its session creation,
    /// every session already persisted earlier in this deployment is deleted so
    /// a failed LegionControl load never leaks orphaned sessions. Best-effort:
    /// a deletion failure is logged and never masks the original error.
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

Arguments:
- "preset_id": Id of a saved legion preset. Mutually exclusive with "nodes".
- "overrides": Optional per-node overrides keyed by node id. Each value may set agent, role, prompt, and/or gate.
- "nodes": Inline topology nodes when preset_id is omitted: [{id, agent, role, prompt, gate}]. At most 20 nodes.
- "edges": Optional parent-child edges: [{from, to, condition}]. Each node may have at most one parent; cycles are rejected.

Notes:
- Agent types are validated against the available agent registry (same as SessionControl).
- daemon/warden agents cannot be deployed through LegionControl.
- Nodes are sorted topologically (deterministic order) and deployed root-first.
- node.prompt, node.gate, and edge.condition are reserved fields: they are persisted into the created session metadata and echoed in the result for observability, but do not yet change runtime behavior.

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
                    "enum": ["load", "list"],
                    "description": "The legion action to perform: \"load\" deploys a preset or inline topology into sessions; \"list\" lists saved presets."
                },
                "preset_id": {
                    "type": "string",
                    "description": "Id of a saved legion preset. Mutually exclusive with \"nodes\"."
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
                        "Invalid action '{}': expected one of load, list",
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

            // LEGION-03: reject inline topologies larger than MAX_LEGION_NODES at
            // validation time so an oversized request never reaches deployment.
            // resolve_legion_topology applies the same bound as a second guard.
            if let Some(nodes) = &parsed.nodes {
                if nodes.len() > MAX_LEGION_NODES {
                    return ValidationResult {
                        result: false,
                        message: Some(format!(
                            "Legion topology exceeds the maximum node count ({} > {})",
                            nodes.len(),
                            MAX_LEGION_NODES
                        )),
                        error_code: Some(400),
                        meta: None,
                    };
                }
            }
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
                if let Some(preset_id) = input.get("presetId").and_then(|v| v.as_str()) {
                    format!("Deploy legion from preset {preset_id}")
                } else {
                    "Deploy legion from inline topology".to_string()
                }
            }
            Some(LegionControlAction::List) => "List available legion presets".to_string(),
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
                "Invalid action '{}': expected one of load, list",
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
                let topology = Self::resolve_legion_topology(nodes, edges.clone())
                    .map_err(BitFunError::tool)?;

                // Validate agent types against the available agent registry
                let available_agent_ids =
                    get_available_agent_type_ids_for_creation(Some(context)).await;
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

                // LEGION-07: role-based delegation validation before any session
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
                // LEGION-02: a read failure fails fast instead of silently
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

                for resolved in &topology {
                    let node = &resolved.node;
                    let session_name = if node.role.trim().is_empty() {
                        node.id.clone()
                    } else {
                        format!("{}-{}", node.role, node.id)
                    };

                    // LEGION-01: resolve the parent and the resulting child depth
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
                    metadata.insert("legionNodeId".to_string(), json!(node.id));
                    metadata.insert("legionRole".to_string(), json!(node.role));
                    // LEGION-04: `prompt`/`gate` are reserved fields today — they
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
                            let created: Vec<String> =
                                session_by_node.values().cloned().collect();
                            Self::cleanup_deployed_sessions(
                                &coordinator,
                                &std::path::PathBuf::from(&display_workspace),
                                &created,
                            )
                            .await;
                            return Err(BitFunError::tool(
                                CoreServiceAgentRuntime::runtime_error_message(error),
                            ));
                        }
                    };

                    let created_session_id = session.session_id.clone();

                    // Attach to the session tree: the parent is the resolved
                    // parent's session; root nodes attach to the creator session.
                    Self::attach_session_to_tree(
                        &coordinator,
                        &created_session_id,
                        parent_session_id.as_deref(),
                        child_depth,
                    )
                    .await;

                    session_by_node.insert(node.id.clone(), created_session_id.clone());
                    deployed.push(json!({
                        "node_id": node.id,
                        "session_id": created_session_id,
                        "session_name": session.session_name,
                        "role": node.role,
                        "agent": node.agent,
                        "depth": child_depth,
                        // LEGION-04: 预留字段在结果中原样回显（与上方会话元数据持久化一致），
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

        let resolved = LegionControlTool::resolve_legion_topology(nodes, edges)
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

        let err = LegionControlTool::resolve_legion_topology(nodes, edges)
            .expect_err("cycle must be rejected");
        assert!(err.contains("cycle"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_rejects_multiple_parents() {
        let nodes = vec![node("a"), node("b"), node("c")];
        let edges = vec![edge("a", "c"), edge("b", "c")];

        let err = LegionControlTool::resolve_legion_topology(nodes, edges)
            .expect_err("multiple parents must be rejected");
        assert!(err.contains("multiple parents"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_rejects_unknown_endpoint() {
        let nodes = vec![node("a")];
        let edges = vec![edge("a", "z")];

        let err = LegionControlTool::resolve_legion_topology(nodes, edges)
            .expect_err("unknown endpoint must be rejected");
        assert!(err.contains("unknown node 'z'"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_rejects_duplicate_ids() {
        let mut a = node("a");
        a.agent = "Plan".to_string();
        let nodes = vec![node("a"), a];

        let err = LegionControlTool::resolve_legion_topology(nodes, Vec::new())
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

        let err = LegionControlTool::resolve_legion_topology(nodes, Vec::new())
            .expect_err("warden agent must be rejected");
        assert!(err.contains("protected agent"), "unexpected error: {err}");

        let mut daemon = node("daemon-node");
        daemon.agent = "daemon".to_string();
        let nodes = vec![daemon];

        let err = LegionControlTool::resolve_legion_topology(nodes, Vec::new())
            .expect_err("daemon agent must be rejected");
        assert!(err.contains("protected agent"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_rejects_self_loop() {
        let nodes = vec![node("a")];
        let edges = vec![edge("a", "a")];

        let err = LegionControlTool::resolve_legion_topology(nodes, edges)
            .expect_err("self-loop must be rejected");
        assert!(err.contains("self-loop"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_rejects_empty_topology() {
        let err = LegionControlTool::resolve_legion_topology(Vec::new(), Vec::new())
            .expect_err("empty topology must be rejected");
        assert!(err.contains("at least one node"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_rejects_excessive_node_count() {
        // LEGION-03: a topology larger than MAX_LEGION_NODES must be rejected so
        // a single LegionControl call cannot spawn an unbounded session fleet.
        let nodes: Vec<LegionNode> = (0..=MAX_LEGION_NODES)
            .map(|index| node(&format!("node-{index}")))
            .collect();
        let err = LegionControlTool::resolve_legion_topology(nodes, Vec::new())
            .expect_err("oversized topology must be rejected");
        assert!(
            err.contains("maximum node count"),
            "unexpected error: {err}"
        );

        // The exact maximum still resolves.
        let nodes: Vec<LegionNode> = (0..MAX_LEGION_NODES)
            .map(|index| node(&format!("node-{index}")))
            .collect();
        let resolved = LegionControlTool::resolve_legion_topology(nodes, Vec::new())
            .expect("topology at the maximum node count should resolve");
        assert_eq!(resolved.len(), MAX_LEGION_NODES);
    }

    #[test]
    fn resolve_topology_rejects_empty_node_fields() {
        let mut empty_id = node("a");
        empty_id.id = "  ".to_string();
        let err = LegionControlTool::resolve_legion_topology(vec![empty_id], Vec::new())
            .expect_err("empty id must be rejected");
        assert!(
            err.contains("id must not be empty"),
            "unexpected error: {err}"
        );

        let mut empty_agent = node("a");
        empty_agent.agent = String::new();
        let err = LegionControlTool::resolve_legion_topology(vec![empty_agent], Vec::new())
            .expect_err("empty agent must be rejected");
        assert!(err.contains("empty agent type"), "unexpected error: {err}");
    }

    #[test]
    fn resolve_topology_single_root_ok() {
        let nodes = vec![node("a"), node("b")];
        let edges = vec![edge("a", "b")];

        let resolved = LegionControlTool::resolve_legion_topology(nodes, edges)
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
        // LEGION-03: validate_input must reject an inline topology larger than
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
}
