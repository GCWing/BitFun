//! LegionControl tool �?load and instantiate legion templates.
//!
//! Reads `<user-config>/legions/<id>.json`, topologically sorts nodes by
//! edges, creates each node via `SessionControl` path, and returns the
//! created session list.

use crate::agentic::coordination::{get_global_coordinator, get_global_scheduler};
use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::infrastructure::get_path_manager_arc;
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_runtime_ports::{
    AgentDialogTurnRequest, AgentSessionCreateRequest, AgentSessionDeleteRequest,
    DialogSubmissionPolicy, DialogTriggerSource,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

/// JSON format of a legion template file.
#[derive(Debug, Deserialize)]
struct LegionTemplate {
    id: String,
    name: String,
    #[serde(default)]
    nodes: Vec<LegionNode>,
    #[serde(default)]
    edges: Vec<LegionEdge>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct LegionNode {
    id: String,
    agent: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    prompt: String,
}

#[derive(Debug, Deserialize)]
struct LegionEdge {
    from: String,
    to: String,
    #[serde(default)]
    condition: String,
}

#[derive(Debug, Deserialize)]
struct LegionControlInput {
    action: String,
    #[serde(default)]
    legion_id: String,
    #[serde(default)]
    workspace: Option<String>,
    /// When true, send each first-layer node its prompt as the initial task message.
    #[serde(default)]
    send_initial_message: bool,
}

pub struct LegionControlTool;

impl LegionControlTool {
    pub fn new() -> Self {
        Self
    }

    fn config_dir(&self) -> PathBuf {
        get_path_manager_arc().user_config_dir().join("legions")
    }

    fn legion_path(&self, legion_id: &str) -> PathBuf {
        self.config_dir().join(format!("{}.json", legion_id))
    }
}

#[async_trait]
impl Tool for LegionControlTool {
    fn name(&self) -> &str {
        "LegionControl"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(concat!(
            "Load and instantiate a legion template.\n\n",
            "Actions:\n",
            "- load: Read a legion template from the user config directory, ",
            "topologically sort nodes by edges, create each node as a new agent session ",
            "via SessionControl, and return the created session list.\n",
            "- list: List available legion templates.\n\n",
            "Parameters:\n",
            "- action: \"load\" or \"list\"\n",
            "- legion_id: template id (required for load)\n",
            "- workspace: override workspace path (optional, defaults to current workspace)",
        )
        .to_string())
    }

    fn short_description(&self) -> String {
        "Load and instantiate legion templates with automatic topology-sorted node creation."
            .to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Direct
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["load", "list"]
                },
                "legion_id": {
                    "type": "string",
                    "description": "Legion template id (required for load action)"
                },
                "workspace": {
                    "type": "string",
                    "description": "Override workspace path"
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
        match serde_json::from_value::<LegionControlInput>(input.clone()) {
            Ok(params) => {
                if params.action == "load" && params.legion_id.is_empty() {
                    return ValidationResult {
                        result: false,
                        message: Some("legion_id is required for load action".to_string()),
                        error_code: None,
                        meta: None,
                    };
                }
                ValidationResult::default()
            }
            Err(e) => ValidationResult {
                result: false,
                message: Some(format!("Invalid input: {}", e)),
                error_code: None,
                meta: None,
            },
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let action = input.get("action").and_then(Value::as_str).unwrap_or("?");
        let legion_id = input
            .get("legion_id")
            .and_then(Value::as_str)
            .unwrap_or("?");
        format!("LegionControl {} {}", action, legion_id)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: LegionControlInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;

        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
        let runtime = CoreServiceAgentRuntime::agent_runtime(coordinator.clone())
            .map_err(BitFunError::tool)?;

        match params.action.as_str() {
            "list" => self.list_legions().await,
            "load" => self.load_and_instantiate(&params, context, &runtime).await,
            _ => Err(BitFunError::tool(format!(
                "Unknown action: {}. Supported: load, list",
                params.action
            ))),
        }
    }
}

impl LegionControlTool {
    async fn list_legions(&self) -> BitFunResult<Vec<ToolResult>> {
        let dir = self.config_dir();
        let mut names: Vec<String> = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }

        let result_text = if names.is_empty() {
            format!("No legion templates found in {}", dir.display())
        } else {
            let mut lines = vec!["Available legion templates:".to_string()];
            for name in &names {
                lines.push(format!("- {}", name));
            }
            lines.join("\n")
        };

        Ok(vec![ToolResult::Result {
            data: json!({ "legions": names }),
            result_for_assistant: Some(result_text),
            image_attachments: None,
        }])
    }

    async fn load_and_instantiate(
        &self,
        params: &LegionControlInput,
        context: &ToolUseContext,
        runtime: &bitfun_agent_runtime::sdk::AgentRuntime,
    ) -> BitFunResult<Vec<ToolResult>> {
        let path = self.legion_path(&params.legion_id);
        let content = std::fs::read_to_string(&path).map_err(|e| {
            BitFunError::tool(format!(
                "Failed to read legion template {}: {}",
                path.display(),
                e
            ))
        })?;

        let template: LegionTemplate = serde_json::from_str(&content)
            .map_err(|e| BitFunError::tool(format!("Invalid legion template: {}", e)))?;

        if template.nodes.is_empty() {
            return Err(BitFunError::tool(
                "Legion template has no nodes".to_string(),
            ));
        }

        // Determine workspace
        let workspace = params
            .workspace
            .clone()
            .or_else(|| {
                context
                    .workspace_root()
                    .map(|p| p.to_string_lossy().to_string())
            })
            .ok_or_else(|| BitFunError::tool("workspace is required".to_string()))?;

        // Topological sort
        let sorted_ids = topological_sort(&template.nodes, &template.edges)?;

        // Build node lookup
        let node_map: HashMap<&str, &LegionNode> =
            template.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        // Create sessions in topological order with rollback on failure
        let mut created_sessions: Vec<serde_json::Value> = Vec::new();
        let mut created_session_ids: Vec<(String, String)> = Vec::new(); // (session_id, workspace)
        let mut session_lines: Vec<String> =
            vec![format!("## Legion: {} ({})", template.name, template.id)];
        session_lines.push(format!("Workspace: {}", workspace));
        session_lines.push(String::new());

        // Batch create: group independent nodes per topological layer
        let layers = build_topological_layers(&sorted_ids, &template.edges);
        let mut creation_result: BitFunResult<()> = Ok(());
        'creation: for (layer_idx, layer) in layers.iter().enumerate() {
            if layer.len() > 1 {
                session_lines.push(format!(
                    "**Layer {} ({} parallel nodes):**",
                    layer_idx + 1,
                    layer.len()
                ));
            }

            for node_id in layer {
                let node = match node_map.get(node_id.as_str()) {
                    Some(n) => n,
                    None => continue,
                };

                let session_name = if node.role.is_empty() {
                    format!("{}-{}", template.name, node.id)
                } else {
                    format!("{}-{}", template.name, node.role)
                };

                let session = match runtime
                    .create_session(AgentSessionCreateRequest {
                        session_name,
                        agent_type: node.agent.clone(),
                        workspace_path: Some(workspace.clone()),
                        remote_connection_id: None,
                        remote_ssh_host: None,
                        model_id: None,
                        workspace_id: None,
                        metadata: {
                            let mut meta = serde_json::Map::new();
                            meta.insert("legionId".to_string(), json!(template.id));
                            meta.insert("legionNodeId".to_string(), json!(node.id));
                            meta.insert("legionRole".to_string(), json!(node.role));
                            meta
                        },
                    })
                    .await
                {
                    Ok(session) => session,
                    Err(error) => {
                        creation_result = Err(BitFunError::tool(
                            CoreServiceAgentRuntime::runtime_error_message(error),
                        ));
                        break 'creation;
                    }
                };

                created_session_ids.push((session.session_id.clone(), workspace.clone()));

                let entry = json!({
                    "node_id": node.id,
                    "role": node.role,
                    "agent_type": node.agent,
                    "session_id": session.session_id,
                    "session_name": session.session_name,
                });
                created_sessions.push(entry);

                session_lines.push(format!(
                    "- **{}** ({}) �?session `{}` (agent: {})",
                    node.role, node.id, session.session_id, node.agent
                ));
            }

            if layer.len() > 1 {
                session_lines.push(String::new());
            }
        }

        // Rollback on failure: delete all successfully created sessions
        if creation_result.is_err() {
            let error_msg = creation_result.unwrap_err();
            let mut delete_errors: Vec<String> = Vec::new();
            for (sid, ws) in &created_session_ids {
                if let Err(e) = runtime
                    .delete_session(AgentSessionDeleteRequest {
                        workspace_path: ws.clone(),
                        session_id: sid.clone(),
                        remote_connection_id: None,
                        remote_ssh_host: None,
                    })
                    .await
                {
                    delete_errors.push(format!("  {}: {}", sid, e));
                }
            }
            let mut msg = format!(
                "Failed to create all legion sessions. {} Created {} session(s) which were cleaned up.",
                error_msg,
                created_session_ids.len()
            );
            if !delete_errors.is_empty() {
                msg.push_str(&format!(
                    "\nNote: {} session(s) could not be cleaned up:\n{}",
                    delete_errors.len(),
                    delete_errors.join("\n")
                ));
            }
            return Err(BitFunError::tool(msg));
        }

        // Append edge structure
        if !template.edges.is_empty() {
            session_lines.push(String::from("### Edges"));
            for edge in &template.edges {
                let cond = if edge.condition.is_empty() {
                    String::new()
                } else {
                    format!(" [condition: {}]", edge.condition)
                };
                session_lines.push(format!("- {} �?{}{}", edge.from, edge.to, cond));
            }
        }

        // Auto-send initial task messages to first-layer nodes
        if params.send_initial_message && !layers.is_empty() {
            let coordinator = get_global_coordinator()
                .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
            let scheduler = get_global_scheduler()
                .ok_or_else(|| BitFunError::tool("scheduler not initialized".to_string()))?;
            let dialog_runtime = CoreServiceAgentRuntime::agent_runtime_with_dialog_turns(
                coordinator.clone(),
                scheduler,
            )
            .map_err(BitFunError::tool)?;

            let first_layer = &layers[0];
            session_lines.push(String::new());
            session_lines.push(format!(
                "Sent initial tasks to {} first-layer node(s)",
                first_layer.len()
            ));

            for node_id in first_layer {
                let node = match node_map.get(node_id.as_str()) {
                    Some(n) => n,
                    None => continue,
                };
                let entry = created_sessions
                    .iter()
                    .find(|e| e["node_id"].as_str() == Some(node.id.as_str()));
                let session_id = match entry.and_then(|e| e["session_id"].as_str()) {
                    Some(sid) => sid.to_string(),
                    None => continue,
                };

                let task_message = if node.prompt.is_empty() {
                    format!("Execute your role: {}", node.role)
                } else {
                    node.prompt.clone()
                };

                let _ = dialog_runtime
                    .submit_dialog_turn(AgentDialogTurnRequest {
                        session_id,
                        message: task_message.clone(),
                        original_message: None,
                        turn_id: None,
                        agent_type: node.agent.clone(),
                        workspace_path: Some(workspace.clone()),
                        remote_connection_id: None,
                        remote_ssh_host: None,
                        policy: DialogSubmissionPolicy::for_source(
                            DialogTriggerSource::AgentSession,
                        ),
                        reply_route: None,
                        prepended_reminders: vec![],
                        attachments: vec![],
                        metadata: serde_json::Map::new(),
                    })
                    .await;

                session_lines.push(format!(
                    "- Sent to **{}**: {}",
                    node.role,
                    if node.prompt.len() > 80 {
                        format!("{}...", &node.prompt[..77])
                    } else {
                        node.prompt.clone()
                    }
                ));
            }
        }

        Ok(vec![ToolResult::Result {
            data: json!({
                "legion_id": template.id,
                "legion_name": template.name,
                "nodes_created": created_sessions.len(),
                "sessions": created_sessions,
            }),
            result_for_assistant: Some(session_lines.join("\n")),
            image_attachments: None,
        }])
    }
}

/// Topological sort: nodes with no incoming edges first.
fn topological_sort(nodes: &[LegionNode], edges: &[LegionEdge]) -> BitFunResult<Vec<String>> {
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();

    for node in nodes {
        in_degree.entry(node.id.as_str()).or_insert(0);
        adjacency.entry(node.id.as_str()).or_default();
    }

    for edge in edges {
        // Skip conditional edges (fail/retry) �?they are runtime routing,
        // not compile-time dependencies for the DAG.
        if !edge.condition.is_empty() {
            continue;
        }
        adjacency
            .entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
        *in_degree.entry(edge.to.as_str()).or_insert(0) += 1;
    }

    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();

    let mut sorted: Vec<String> = Vec::new();
    while let Some(node) = queue.pop_front() {
        sorted.push(node.to_string());
        if let Some(neighbors) = adjacency.get(node) {
            for &neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(neighbor);
                    }
                }
            }
        }
    }

    if sorted.len() != nodes.len() {
        let node_ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        let sorted_set: HashSet<&str> = sorted.iter().map(|s| s.as_str()).collect();
        let missing: Vec<_> = node_ids.difference(&sorted_set).collect();
        return Err(BitFunError::tool(format!(
            "Cyclic dependency detected in legion edges. Unresolved nodes: {:?}",
            missing
        )));
    }

    Ok(sorted)
}

/// Group topologically sorted nodes into layers where each layer can execute in parallel.
fn build_topological_layers(sorted_ids: &[String], edges: &[LegionEdge]) -> Vec<Vec<String>> {
    let mut layers: Vec<Vec<String>> = Vec::new();
    let mut assigned: HashMap<&str, usize> = HashMap::new();

    for id in sorted_ids {
        // Place node one layer after all its predecessors.
        // Nodes with no predecessors land on layer 0.
        let max_pred_layer = edges
            .iter()
            .filter(|e| e.to == *id && e.condition.is_empty())
            .filter_map(|e| assigned.get(e.from.as_str()))
            .max()
            .copied();

        let layer = match max_pred_layer {
            Some(max_layer) => max_layer + 1,
            None => 0,
        };
        while layers.len() <= layer {
            layers.push(Vec::new());
        }
        layers[layer].push(id.clone());
        assigned.insert(id.as_str(), layer);
    }

    layers.retain(|l| !l.is_empty());
    layers
}
