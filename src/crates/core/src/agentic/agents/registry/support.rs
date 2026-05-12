use crate::service::config::global::GlobalConfigManager;
use crate::service::config::types::{ModeConfig, SubAgentConfig};
use std::collections::HashMap;

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

pub(super) async fn get_subagent_configs() -> HashMap<String, SubAgentConfig> {
    if let Ok(config_service) = GlobalConfigManager::get_service().await {
        config_service
            .get_config(Some("ai.subagent_configs"))
            .await
            .unwrap_or_default()
    } else {
        HashMap::new()
    }
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
