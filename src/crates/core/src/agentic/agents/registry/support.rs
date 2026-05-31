use crate::service::config::agent_profile_project_store::{
    get_project_subagent_overrides, load_project_agent_profiles_document_local,
    save_project_agent_profiles_document_local, set_project_subagent_overrides,
};
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::types::{AgentProfileConfig, AgentSubagentOverrideConfig};
use crate::util::errors::BitFunResult;
use std::collections::HashMap;
use std::path::Path;

pub(super) async fn get_mode_configs() -> HashMap<String, AgentProfileConfig> {
    if let Ok(config_service) = GlobalConfigManager::get_service().await {
        config_service
            .get_config(Some("ai.agent_profiles"))
            .await
            .unwrap_or_default()
    } else {
        HashMap::new()
    }
}

pub(super) async fn get_subagent_overrides() -> AgentSubagentOverrideConfig {
    get_mode_configs()
        .await
        .into_iter()
        .filter_map(|(profile_id, config)| {
            (!config.subagent_overrides.is_empty()).then(|| (profile_id, config.subagent_overrides))
        })
        .collect()
}

pub(super) async fn load_project_subagent_overrides_local(
    workspace_root: &Path,
) -> BitFunResult<AgentSubagentOverrideConfig> {
    let document = load_project_agent_profiles_document_local(workspace_root).await?;
    Ok(document
        .keys()
        .map(|profile_id| {
            (
                profile_id.clone(),
                get_project_subagent_overrides(&document, profile_id),
            )
        })
        .filter(|(_, overrides)| !overrides.is_empty())
        .collect())
}

pub(super) async fn save_project_subagent_overrides_local(
    workspace_root: &Path,
    overrides: &AgentSubagentOverrideConfig,
) -> BitFunResult<()> {
    let mut document = load_project_agent_profiles_document_local(workspace_root).await?;

    let existing_profile_ids: Vec<String> = document.keys().cloned().collect();
    for profile_id in existing_profile_ids {
        let next = overrides.get(&profile_id).cloned().unwrap_or_default();
        set_project_subagent_overrides(&mut document, &profile_id, next);
    }

    for (profile_id, profile_overrides) in overrides {
        set_project_subagent_overrides(&mut document, profile_id, profile_overrides.clone());
    }

    save_project_agent_profiles_document_local(workspace_root, &document).await
}

pub(super) fn merge_dynamic_mcp_tools(
    mut configured_tools: Vec<String>,
    registered_tool_names: &[String],
) -> Vec<String> {
    for tool_name in registered_tool_names {
        if !tool_name.starts_with("mcp__") {
            continue;
        }

        if configured_tools
            .iter()
            .any(|existing| existing == tool_name)
        {
            continue;
        }

        configured_tools.push(tool_name.clone());
    }

    configured_tools
}
