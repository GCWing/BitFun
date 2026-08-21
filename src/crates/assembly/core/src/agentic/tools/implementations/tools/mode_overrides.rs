//! Mode-profile specific Tool override helpers.
//!
//! Mirrors `skills/mode_overrides.rs` for the tool side. The user-level global
//! switch persists under `ai.tool_settings` (symmetric to `ai.skill_settings`)
//! and mode-scoped tool selections go through the shared agent-profile
//! canonicalizer so `enabled_tools` flows into `resolve_effective_tools` and
//! the runtime RBAC gate.

use crate::agentic::agents::resolve_mode_config_profile_id;
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::types::ToolSettingsConfig;
use crate::util::errors::BitFunResult;
use std::collections::HashSet;

fn resolve_profile_id(mode_id: &str) -> String {
    resolve_mode_config_profile_id(mode_id).into_owned()
}

fn normalize_tool_names(tools: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for name in tools {
        let trimmed = name.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

/// User-level Tool names disabled for every agent profile.
pub async fn load_globally_disabled_user_tools() -> BitFunResult<Vec<String>> {
    let config_service = GlobalConfigManager::get_service().await?;
    let settings: ToolSettingsConfig = config_service
        .get_config(Some("ai.tool_settings"))
        .await
        .unwrap_or_default();
    Ok(normalize_tool_names(
        settings.globally_disabled_user_tool_names,
    ))
}

/// Persist a user-level global Tool availability change and return the
/// resulting disabled list.
pub async fn set_global_user_tool_disabled(
    tool_name: &str,
    disabled: bool,
) -> BitFunResult<Vec<String>> {
    let tool_name = tool_name.trim();
    if tool_name.is_empty() {
        return Ok(Vec::new());
    }

    let config_service = GlobalConfigManager::get_service().await?;
    let mut settings: ToolSettingsConfig = config_service
        .get_config(Some("ai.tool_settings"))
        .await
        .unwrap_or_default();

    if disabled {
        settings
            .globally_disabled_user_tool_names
            .push(tool_name.to_string());
    } else {
        settings
            .globally_disabled_user_tool_names
            .retain(|name| name != tool_name);
    }
    settings.globally_disabled_user_tool_names =
        normalize_tool_names(settings.globally_disabled_user_tool_names);

    config_service
        .set_config("ai.tool_settings", &settings)
        .await?;
    Ok(settings.globally_disabled_user_tool_names)
}

/// Profile id used by the shared agent-profile document for a mode.
pub fn mode_tool_profile_id(mode_id: &str) -> String {
    resolve_profile_id(mode_id)
}

/// Filter a tool list against the user-level global disabled set.
///
/// Used by the agent tool-policy resolver so a globally disabled tool is
/// removed from every agent's effective tool set (mirrors the skills-side
/// `filter_globally_disabled_candidates`).
pub fn filter_globally_disabled_tools(
    tools: Vec<String>,
    globally_disabled_tool_names: &HashSet<String>,
) -> Vec<String> {
    tools
        .into_iter()
        .filter(|name| !globally_disabled_tool_names.contains(name))
        .collect()
}

/// Reset user-level mode tool overrides back to the mode defaults.
///
/// Symmetric to the skills-side reset: clears `added_tools`/`removed_tools`
/// through the shared canonicalizer while preserving skill/subagent overrides.
pub async fn clear_user_mode_tool_overrides(mode_id: &str) -> BitFunResult<()> {
    crate::service::config::mode_config_canonicalizer::reset_agent_profile_to_default(mode_id).await
}

#[cfg(test)]
mod tests {
    use super::{filter_globally_disabled_tools, normalize_tool_names};
    use std::collections::HashSet;

    #[test]
    fn normalize_tool_names_dedupes_and_trims() {
        assert_eq!(
            normalize_tool_names(vec![
                "  Read ".to_string(),
                "Read".to_string(),
                "".to_string(),
                "Write".to_string(),
            ]),
            vec!["Read".to_string(), "Write".to_string()]
        );
    }

    #[test]
    fn normalize_tool_names_keeps_order() {
        assert_eq!(
            normalize_tool_names(vec![
                "B".to_string(),
                "A".to_string(),
                "A".to_string(),
                "C".to_string(),
            ]),
            vec!["B".to_string(), "A".to_string(), "C".to_string()]
        );
    }

    #[test]
    fn filter_globally_disabled_tools_removes_disabled_and_keeps_others() {
        let disabled: HashSet<String> = ["Read".to_string(), "mcp__github__search".to_string()]
            .into_iter()
            .collect();
        assert_eq!(
            filter_globally_disabled_tools(
                vec![
                    "Read".to_string(),
                    "Write".to_string(),
                    "mcp__github__search".to_string(),
                    "Grep".to_string(),
                ],
                &disabled,
            ),
            vec!["Write".to_string(), "Grep".to_string()]
        );
    }

    #[test]
    fn filter_globally_disabled_tools_empty_disabled_is_noop() {
        let disabled = HashSet::new();
        let tools = vec!["Read".to_string(), "Write".to_string()];
        assert_eq!(
            filter_globally_disabled_tools(tools.clone(), &disabled),
            tools
        );
    }
}
