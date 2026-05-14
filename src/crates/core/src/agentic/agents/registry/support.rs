use crate::service::config::global::GlobalConfigManager;
use crate::infrastructure::get_path_manager_arc;
use crate::service::config::types::{AgentSubagentOverrideConfig, ModeConfig};
use crate::util::errors::{BitFunError, BitFunResult};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::Path;

pub(super) async fn get_mode_configs() -> HashMap<String, ModeConfig> {
    if let Ok(config_service) = GlobalConfigManager::get_service().await {
        config_service
            .get_config(Some("ai.mode_configs"))
            .await
            .unwrap_or_default()
    } else {
        HashMap::new()
    }
}

pub(super) async fn get_subagent_overrides() -> AgentSubagentOverrideConfig {
    if let Ok(config_service) = GlobalConfigManager::get_service().await {
        config_service
            .get_config(Some("ai.agent_subagent_overrides"))
            .await
            .unwrap_or_default()
    } else {
        HashMap::new()
    }
}

fn normalize_project_document_value(value: Value) -> Value {
    match value {
        Value::Object(_) => value,
        _ => Value::Object(Map::new()),
    }
}

pub(super) async fn load_project_subagent_overrides_local(
    workspace_root: &Path,
) -> BitFunResult<AgentSubagentOverrideConfig> {
    let path = get_path_manager_arc().project_agent_subagents_file(workspace_root);
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => Ok(serde_json::from_value(normalize_project_document_value(
            serde_json::from_str(&content)?,
        ))?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(error) => Err(BitFunError::config(format!(
            "Failed to read project subagent overrides file '{}': {}",
            path.display(),
            error
        ))),
    }
}

pub(super) async fn save_project_subagent_overrides_local(
    workspace_root: &Path,
    overrides: &AgentSubagentOverrideConfig,
) -> BitFunResult<()> {
    let path = get_path_manager_arc().project_agent_subagents_file(workspace_root);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, serde_json::to_vec_pretty(overrides)?).await?;
    Ok(())
}

pub(super) fn merge_dynamic_mcp_tools(
    mut configured_tools: Vec<String>,
    registered_tool_names: &[String],
) -> Vec<String> {
    for tool_name in registered_tool_names {
        if !tool_name.starts_with("mcp__") {
            continue;
        }

        if configured_tools.iter().any(|existing| existing == tool_name) {
            continue;
        }

        configured_tools.push(tool_name.clone());
    }

    configured_tools
}
