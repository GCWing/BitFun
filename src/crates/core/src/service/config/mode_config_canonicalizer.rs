//! Mode/profile tool configuration migration and resolution.
//!
//! Stored configuration keeps only user overrides. Effective tool lists are
//! derived from the current mode defaults at runtime.

use crate::agentic::agents::{
    get_agent_registry, mode_config_profile_member_mode_ids, resolve_mode_config_profile_id,
};
use crate::agentic::tools::registry::get_all_registered_tools;
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::types::{
    AgentProfileConfig, AgentProfileView, ParentSubagentOverrideConfig,
};
use crate::util::errors::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

/// Agent-profile config canonicalization report.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AgentProfileConfigCanonicalizationReport {
    pub removed_profile_configs: Vec<String>,
    pub updated_profiles: Vec<AgentProfileConfigUpdateInfo>,
}

/// Agent-profile config update information.
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentProfileConfigUpdateInfo {
    pub profile_id: String,
    pub added_tools: Vec<String>,
    pub removed_tools: Vec<String>,
}

fn dedupe_preserving_order(items: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for item in items {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }

        let owned = trimmed.to_string();
        if seen.insert(owned.clone()) {
            normalized.push(owned);
        }
    }

    normalized
}

fn normalize_tools(tools: Vec<String>, valid_tools: &HashSet<String>) -> Vec<String> {
    dedupe_preserving_order(tools)
        .into_iter()
        .filter(|tool| valid_tools.contains(tool))
        .collect()
}

fn normalize_skill_keys(keys: Vec<String>) -> Vec<String> {
    dedupe_preserving_order(keys)
}

fn normalize_skill_override_lists(
    disabled_user_skills: Vec<String>,
    enabled_user_skills: Vec<String>,
) -> (Vec<String>, Vec<String>) {
    let disabled_user_skills = normalize_skill_keys(disabled_user_skills);
    let disabled_set: HashSet<String> = disabled_user_skills.iter().cloned().collect();
    let mut enabled_user_skills = normalize_skill_keys(enabled_user_skills);
    enabled_user_skills.retain(|key| !disabled_set.contains(key));
    (disabled_user_skills, enabled_user_skills)
}

fn normalize_subagent_overrides(
    overrides: ParentSubagentOverrideConfig,
) -> ParentSubagentOverrideConfig {
    overrides
        .into_iter()
        .filter_map(|(subagent_key, state)| {
            let trimmed = subagent_key.trim();
            (!trimmed.is_empty()).then(|| (trimmed.to_string(), state))
        })
        .collect()
}

fn resolve_profile_id(mode_id: &str) -> String {
    resolve_mode_config_profile_id(mode_id).into_owned()
}

pub fn resolve_effective_tools(
    default_tools: &[String],
    mode_config: Option<&AgentProfileConfig>,
    valid_tools: &HashSet<String>,
) -> Vec<String> {
    let Some(config) = mode_config else {
        return normalize_tools(default_tools.to_vec(), valid_tools);
    };

    let default_tools = normalize_tools(default_tools.to_vec(), valid_tools);
    let removed: HashSet<String> = config.removed_tools.iter().cloned().collect();
    let added = normalize_tools(config.added_tools.clone(), valid_tools);

    let mut effective = Vec::new();
    let mut seen = HashSet::new();

    for tool in default_tools {
        if removed.contains(&tool) {
            continue;
        }
        if seen.insert(tool.clone()) {
            effective.push(tool);
        }
    }

    for tool in added {
        if seen.insert(tool.clone()) {
            effective.push(tool);
        }
    }

    effective
}

fn stored_agent_profile_from_tool_selection(
    agent_id: &str,
    enabled_tools: Vec<String>,
    disabled_user_skills: Vec<String>,
    enabled_user_skills: Vec<String>,
    subagent_overrides: ParentSubagentOverrideConfig,
    default_tools: &[String],
    valid_tools: &HashSet<String>,
) -> Option<AgentProfileConfig> {
    let default_tools = normalize_tools(default_tools.to_vec(), valid_tools);
    let enabled_tools = normalize_tools(enabled_tools, valid_tools);
    let enabled_set: HashSet<String> = enabled_tools.iter().cloned().collect();
    let default_set: HashSet<String> = default_tools.iter().cloned().collect();

    let mut added_tools = Vec::new();
    for tool in &enabled_tools {
        if !default_set.contains(tool) {
            added_tools.push(tool.clone());
        }
    }

    let mut removed_tools = Vec::new();
    for tool in &default_tools {
        if !enabled_set.contains(tool) {
            removed_tools.push(tool.clone());
        }
    }

    stored_agent_profile_from_overrides(
        agent_id,
        added_tools,
        removed_tools,
        disabled_user_skills,
        enabled_user_skills,
        subagent_overrides,
        &default_tools,
        valid_tools,
    )
}

fn stored_agent_profile_from_overrides(
    agent_id: &str,
    added_tools: Vec<String>,
    removed_tools: Vec<String>,
    disabled_user_skills: Vec<String>,
    enabled_user_skills: Vec<String>,
    subagent_overrides: ParentSubagentOverrideConfig,
    default_tools: &[String],
    valid_tools: &HashSet<String>,
) -> Option<AgentProfileConfig> {
    let profile_id = resolve_profile_id(agent_id);
    let default_set: HashSet<String> = default_tools.iter().cloned().collect();
    let mut added_tools = normalize_tools(added_tools, valid_tools);
    let mut removed_tools = normalize_tools(removed_tools, valid_tools);
    let (disabled_user_skills, enabled_user_skills) =
        normalize_skill_override_lists(disabled_user_skills, enabled_user_skills);
    let subagent_overrides = normalize_subagent_overrides(subagent_overrides);

    added_tools.retain(|tool| !default_set.contains(tool));
    removed_tools.retain(|tool| default_set.contains(tool));

    let removed_set: HashSet<String> = removed_tools.iter().cloned().collect();
    added_tools.retain(|tool| !removed_set.contains(tool));

    if added_tools.is_empty()
        && removed_tools.is_empty()
        && disabled_user_skills.is_empty()
        && enabled_user_skills.is_empty()
        && subagent_overrides.is_empty()
    {
        return None;
    }

    Some(AgentProfileConfig {
        profile_id,
        added_tools,
        removed_tools,
        disabled_user_skills,
        enabled_user_skills,
        subagent_overrides,
    })
}

fn build_agent_profile_view(
    agent_id: &str,
    default_tools: Vec<String>,
    mode_config: Option<&AgentProfileConfig>,
    valid_tools: &HashSet<String>,
) -> AgentProfileView {
    let default_tools = normalize_tools(default_tools, valid_tools);
    let enabled_tools = resolve_effective_tools(&default_tools, mode_config, valid_tools);
    let (disabled_user_skills, enabled_user_skills) = mode_config
        .map(|config| {
            normalize_skill_override_lists(
                config.disabled_user_skills.clone(),
                config.enabled_user_skills.clone(),
            )
        })
        .unwrap_or_else(|| (Vec::new(), Vec::new()));

    AgentProfileView {
        profile_id: resolve_profile_id(agent_id),
        enabled_tools,
        default_tools,
        disabled_user_skills,
        enabled_user_skills,
    }
}

fn canonicalize_agent_profile(
    profile_id: &str,
    raw_mode: Option<&Value>,
    default_tools: &[String],
    valid_tools: &HashSet<String>,
) -> BitFunResult<Option<AgentProfileConfig>> {
    let Some(raw_mode) = raw_mode else {
        return Ok(None);
    };
    if raw_mode.is_null() {
        return Ok(None);
    }

    let mut stored: AgentProfileConfig =
        serde_json::from_value(raw_mode.clone()).map_err(|error| {
            BitFunError::config(format!(
                "Failed to deserialize agent profile '{}': {}",
                profile_id, error
            ))
        })?;
    if stored.profile_id.trim().is_empty() {
        stored.profile_id = profile_id.to_string();
    }

    Ok(stored_agent_profile_from_overrides(
        profile_id,
        stored.added_tools,
        stored.removed_tools,
        stored.disabled_user_skills,
        stored.enabled_user_skills,
        stored.subagent_overrides,
        default_tools,
        valid_tools,
    ))
}

async fn get_valid_tool_names() -> HashSet<String> {
    get_all_registered_tools()
        .await
        .into_iter()
        .map(|tool| tool.name().to_string())
        .collect()
}

async fn get_mode_defaults() -> HashMap<String, Vec<String>> {
    get_agent_registry()
        .get_modes_info()
        .await
        .into_iter()
        .map(|mode| (mode.id, mode.default_tools))
        .collect()
}

async fn get_profile_defaults() -> HashMap<String, Vec<String>> {
    let mut defaults = HashMap::new();
    for (mode_id, default_tools) in get_mode_defaults().await {
        defaults
            .entry(resolve_profile_id(&mode_id))
            .or_insert(default_tools);
    }
    defaults
}

pub async fn get_agent_profile_configs() -> BitFunResult<HashMap<String, AgentProfileConfig>> {
    let config_service = GlobalConfigManager::get_service().await?;
    Ok(config_service
        .get_config(Some("ai.agent_profiles"))
        .await
        .unwrap_or_default())
}

pub async fn get_agent_profile_views() -> BitFunResult<HashMap<String, AgentProfileView>> {
    let stored_configs = get_agent_profile_configs().await?;
    let mode_defaults = get_mode_defaults().await;
    let valid_tools = get_valid_tool_names().await;

    let mut views = HashMap::new();
    for (mode_id, default_tools) in mode_defaults {
        let profile_id = resolve_profile_id(&mode_id);
        let view = build_agent_profile_view(
            &mode_id,
            default_tools,
            stored_configs.get(&profile_id),
            &valid_tools,
        );
        views.insert(mode_id, view);
    }

    Ok(views)
}

pub async fn get_agent_profile_view(agent_id: &str) -> BitFunResult<AgentProfileView> {
    let views = get_agent_profile_views().await?;
    views
        .get(agent_id)
        .cloned()
        .ok_or_else(|| BitFunError::config(format!("Agent does not exist: {}", agent_id)))
}

pub async fn persist_agent_profile_from_value(agent_id: &str, config: Value) -> BitFunResult<()> {
    let config_service = GlobalConfigManager::get_service().await?;
    let mut stored_configs = get_agent_profile_configs().await?;
    let mode_defaults = get_mode_defaults().await;
    let default_tools = mode_defaults
        .get(agent_id)
        .ok_or_else(|| BitFunError::config(format!("Agent does not exist: {}", agent_id)))?;
    let valid_tools = get_valid_tool_names().await;
    let profile_id = resolve_profile_id(agent_id);
    let current = stored_configs.get(&profile_id);

    let enabled_tools = if let Some(tools) = config.get("enabled_tools") {
        serde_json::from_value::<Vec<String>>(tools.clone()).map_err(|error| {
            BitFunError::config(format!(
                "Invalid enabled_tools for mode '{}': {}",
                agent_id, error
            ))
        })?
    } else {
        resolve_effective_tools(default_tools, current, &valid_tools)
    };

    let disabled_user_skills = if config
        .as_object()
        .map(|obj| obj.contains_key("disabled_user_skills"))
        .unwrap_or(false)
    {
        match config.get("disabled_user_skills") {
            Some(Value::Null) | None => Vec::new(),
            Some(value) => {
                serde_json::from_value::<Vec<String>>(value.clone()).map_err(|error| {
                    BitFunError::config(format!(
                        "Invalid disabled_user_skills for mode '{}': {}",
                        agent_id, error
                    ))
                })?
            }
        }
    } else {
        current
            .map(|item| item.disabled_user_skills.clone())
            .unwrap_or_default()
    };

    let enabled_user_skills = if config
        .as_object()
        .map(|obj| obj.contains_key("enabled_user_skills"))
        .unwrap_or(false)
    {
        match config.get("enabled_user_skills") {
            Some(Value::Null) | None => Vec::new(),
            Some(value) => {
                serde_json::from_value::<Vec<String>>(value.clone()).map_err(|error| {
                    BitFunError::config(format!(
                        "Invalid enabled_user_skills for mode '{}': {}",
                        agent_id, error
                    ))
                })?
            }
        }
    } else {
        current
            .map(|item| item.enabled_user_skills.clone())
            .unwrap_or_default()
    };

    let subagent_overrides = if config
        .as_object()
        .map(|obj| obj.contains_key("subagent_overrides"))
        .unwrap_or(false)
    {
        match config.get("subagent_overrides") {
            Some(Value::Null) | None => ParentSubagentOverrideConfig::new(),
            Some(value) => serde_json::from_value::<ParentSubagentOverrideConfig>(value.clone())
                .map_err(|error| {
                    BitFunError::config(format!(
                        "Invalid subagent_overrides for mode '{}': {}",
                        agent_id, error
                    ))
                })?,
        }
    } else {
        current
            .map(|item| item.subagent_overrides.clone())
            .unwrap_or_default()
    };

    if let Some(canonical) = stored_agent_profile_from_tool_selection(
        agent_id,
        enabled_tools,
        disabled_user_skills,
        enabled_user_skills,
        subagent_overrides,
        default_tools,
        &valid_tools,
    ) {
        stored_configs.insert(profile_id, canonical);
    } else {
        stored_configs.remove(&profile_id);
    }

    config_service
        .set_config("ai.agent_profiles", stored_configs)
        .await
}

pub async fn reset_agent_profile_to_default(agent_id: &str) -> BitFunResult<()> {
    let config_service = GlobalConfigManager::get_service().await?;
    let mut stored_configs = get_agent_profile_configs().await?;
    let profile_id = resolve_profile_id(agent_id);

    if let Some(current) = stored_configs.get_mut(&profile_id) {
        current.added_tools.clear();
        current.removed_tools.clear();

        if current.disabled_user_skills.is_empty()
            && current.enabled_user_skills.is_empty()
            && current.subagent_overrides.is_empty()
        {
            stored_configs.remove(&profile_id);
        }
    }

    config_service
        .set_config("ai.agent_profiles", stored_configs)
        .await
}

/// Canonicalizes stored mode profile overrides.
pub async fn canonicalize_agent_profile_configs(
) -> BitFunResult<AgentProfileConfigCanonicalizationReport> {
    let config_service = GlobalConfigManager::get_service().await?;
    let valid_tools = get_valid_tool_names().await;
    let profile_defaults = get_profile_defaults().await;
    let mut ai_value: Value = config_service.get_config(Some("ai")).await?;
    let original_ai_value = ai_value.clone();
    let ai_object = ai_value
        .as_object_mut()
        .ok_or_else(|| BitFunError::config("AI config must be a JSON object".to_string()))?;

    let raw_agent_profiles = ai_object
        .get("agent_profiles")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut rewritten_agent_profiles = Map::new();
    let mut updated_profiles = Vec::new();
    let mut removed_profile_configs = Vec::new();

    for (profile_id, default_tools) in &profile_defaults {
        let raw_profile = raw_agent_profiles.get(profile_id);
        let canonical =
            canonicalize_agent_profile(profile_id, raw_profile, default_tools, &valid_tools)?;
        if let Some(config) = canonical {
            if raw_profile.is_some() {
                updated_profiles.push(AgentProfileConfigUpdateInfo {
                    profile_id: profile_id.clone(),
                    added_tools: config.added_tools.clone(),
                    removed_tools: config.removed_tools.clone(),
                });
            }
            rewritten_agent_profiles.insert(profile_id.clone(), serde_json::to_value(config)?);
        } else if raw_profile.is_some() {
            removed_profile_configs.push(profile_id.clone());
        }
    }

    for profile_id in raw_agent_profiles.keys() {
        if !profile_defaults.contains_key(profile_id) {
            removed_profile_configs.push(profile_id.clone());
        }
    }

    ai_object.insert(
        "agent_profiles".to_string(),
        Value::Object(rewritten_agent_profiles),
    );

    if ai_value != original_ai_value {
        config_service.set_config("ai", ai_value).await?;
    }

    Ok(AgentProfileConfigCanonicalizationReport {
        removed_profile_configs,
        updated_profiles,
    })
}

pub fn agent_profile_member_mode_ids_for(agent_id: &str) -> Vec<String> {
    let profile_id = resolve_profile_id(agent_id);
    let members = mode_config_profile_member_mode_ids(&profile_id);
    if members.is_empty() {
        vec![agent_id.to_string()]
    } else {
        members
            .iter()
            .map(|mode_id| (*mode_id).to_string())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        agent_profile_member_mode_ids_for, canonicalize_agent_profile,
        normalize_skill_override_lists, stored_agent_profile_from_overrides,
    };
    use crate::service::config::types::AgentSubagentOverrideState;
    use serde_json::Value;
    use std::collections::HashSet;

    #[test]
    fn normalize_skill_override_lists_removes_duplicates_and_conflicts() {
        let (disabled, enabled) = normalize_skill_override_lists(
            vec![
                "user::bitfun-system::pdf".to_string(),
                "user::bitfun-system::pdf".to_string(),
            ],
            vec![
                "user::bitfun-system::pdf".to_string(),
                "user::bitfun-system::docx".to_string(),
                "user::bitfun-system::docx".to_string(),
            ],
        );

        assert_eq!(disabled, vec!["user::bitfun-system::pdf".to_string()]);
        assert_eq!(enabled, vec!["user::bitfun-system::docx".to_string()]);
    }

    #[test]
    fn stored_agent_profile_from_overrides_keeps_enabled_user_skills() {
        let valid_tools = HashSet::new();
        let stored = stored_agent_profile_from_overrides(
            "agentic",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec!["user::bitfun-system::pdf".to_string()],
            Default::default(),
            &[],
            &valid_tools,
        )
        .expect("mode config should be retained when skill overrides exist");

        assert_eq!(stored.profile_id, "coding_shared");
        assert_eq!(
            stored.enabled_user_skills,
            vec!["user::bitfun-system::pdf".to_string()]
        );
        assert!(stored.disabled_user_skills.is_empty());
    }

    #[test]
    fn stored_agent_profile_from_overrides_keeps_subagent_overrides() {
        let valid_tools = HashSet::new();
        let mut subagent_overrides = std::collections::HashMap::new();
        subagent_overrides.insert(
            "builtin::builtin::Explore".to_string(),
            AgentSubagentOverrideState::Disabled,
        );
        let stored = stored_agent_profile_from_overrides(
            "debug",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            subagent_overrides.clone(),
            &[],
            &valid_tools,
        )
        .expect("mode config should be retained when subagent overrides exist");

        assert_eq!(stored.profile_id, "coding_shared");
        assert_eq!(stored.subagent_overrides, subagent_overrides);
    }

    #[test]
    fn canonicalize_agent_profile_treats_null_as_missing() {
        let canonical =
            canonicalize_agent_profile("Claw", Some(&Value::Null), &[], &HashSet::new())
                .expect("null mode config should be ignored");

        assert!(canonical.is_none());
    }

    #[test]
    fn shared_modes_report_shared_profile_members() {
        assert_eq!(
            agent_profile_member_mode_ids_for("agentic"),
            vec![
                "agentic".to_string(),
                "Plan".to_string(),
                "debug".to_string(),
                "Multitask".to_string()
            ]
        );
        assert_eq!(
            agent_profile_member_mode_ids_for("Cowork"),
            vec!["Cowork".to_string()]
        );
    }
}
