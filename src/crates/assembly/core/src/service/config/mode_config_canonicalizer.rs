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
use bitfun_agent_runtime::skills::normalize_user_mode_skill_overrides;
use bitfun_agent_runtime::thread_goal_tools::THREAD_GOAL_TOOL_NAMES;
use bitfun_runtime_ports::PermissionRule;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

const DYNAMIC_MCP_TOOL_PREFIX: &str = "mcp__";

/// Agent-profile config canonicalization report.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AgentProfileConfigCanonicalizationReport {
    pub removed_profile_configs: Vec<String>,
    pub updated_profiles: Vec<AgentProfileConfigUpdateInfo>,
}

struct AgentProfileDefaults {
    tools: Vec<String>,
    include_implicit_thread_goal_tools: bool,
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

fn normalize_added_tool_overrides(
    tools: Vec<String>,
    valid_tools: &HashSet<String>,
) -> Vec<String> {
    dedupe_preserving_order(tools)
        .into_iter()
        .filter(|tool| valid_tools.contains(tool) || tool.starts_with(DYNAMIC_MCP_TOOL_PREFIX))
        .collect()
}

fn normalize_skill_override_lists(
    disabled_user_skills: Vec<String>,
    enabled_user_skills: Vec<String>,
) -> (Vec<String>, Vec<String>) {
    let overrides = normalize_user_mode_skill_overrides(disabled_user_skills, enabled_user_skills);
    (overrides.disabled_skills, overrides.enabled_skills)
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
    let default_tools = normalize_tools(default_tools.to_vec(), valid_tools);
    let removed: HashSet<String> = mode_config
        .map(|config| config.removed_tools.iter().cloned().collect())
        .unwrap_or_default();
    let added = mode_config
        .map(|config| normalize_tools(config.added_tools.clone(), valid_tools))
        .unwrap_or_default();

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

    // Thread goals are a main-session lifecycle capability, not an optional
    // mode specialization. The UI and backend can activate a goal without a
    // model tool call, so allowing a profile override to remove update_goal
    // would strand the active goal in the automatic continuation loop.
    for tool_name in THREAD_GOAL_TOOL_NAMES {
        if valid_tools.contains(tool_name) && seen.insert(tool_name.to_string()) {
            effective.push(tool_name.to_string());
        }
    }

    effective
}

pub(crate) fn apply_implicit_thread_goal_policy(
    mut tools: Vec<String>,
    include_implicit_thread_goal_tools: bool,
) -> Vec<String> {
    if !include_implicit_thread_goal_tools {
        tools.retain(|tool| !THREAD_GOAL_TOOL_NAMES.contains(&tool.as_str()));
    }
    tools
}

fn stored_agent_profile_from_tool_selection(
    agent_id: &str,
    enabled_tools: Vec<String>,
    disabled_user_skills: Vec<String>,
    enabled_user_skills: Vec<String>,
    subagent_overrides: ParentSubagentOverrideConfig,
    tool_permission_rules: Vec<PermissionRule>,
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

    stored_agent_profile_from_overrides(StoredAgentProfileOverrides {
        agent_id,
        added_tools,
        removed_tools,
        disabled_user_skills,
        enabled_user_skills,
        subagent_overrides,
        tool_permission_rules,
        default_tools: &default_tools,
        valid_tools,
    })
}

struct StoredAgentProfileOverrides<'a> {
    agent_id: &'a str,
    added_tools: Vec<String>,
    removed_tools: Vec<String>,
    disabled_user_skills: Vec<String>,
    enabled_user_skills: Vec<String>,
    subagent_overrides: ParentSubagentOverrideConfig,
    tool_permission_rules: Vec<PermissionRule>,
    default_tools: &'a [String],
    valid_tools: &'a HashSet<String>,
}

fn stored_agent_profile_from_overrides(
    overrides: StoredAgentProfileOverrides<'_>,
) -> Option<AgentProfileConfig> {
    let StoredAgentProfileOverrides {
        agent_id,
        added_tools,
        removed_tools,
        disabled_user_skills,
        enabled_user_skills,
        subagent_overrides,
        tool_permission_rules,
        default_tools,
        valid_tools,
    } = overrides;
    let profile_id = resolve_profile_id(agent_id);
    let default_set: HashSet<String> = default_tools.iter().cloned().collect();
    // MCP tools are registered only after deferred server initialization. Keep
    // their stored overrides during startup canonicalization even when the
    // current registry snapshot cannot validate them yet.
    let mut added_tools = normalize_added_tool_overrides(added_tools, valid_tools);
    let mut removed_tools = normalize_tools(removed_tools, valid_tools);
    let (disabled_user_skills, enabled_user_skills) =
        normalize_skill_override_lists(disabled_user_skills, enabled_user_skills);
    let subagent_overrides = normalize_subagent_overrides(subagent_overrides);

    added_tools.retain(|tool| !default_set.contains(tool));
    removed_tools.retain(|tool| default_set.contains(tool));
    removed_tools.retain(|tool| !THREAD_GOAL_TOOL_NAMES.contains(&tool.as_str()));

    let removed_set: HashSet<String> = removed_tools.iter().cloned().collect();
    added_tools.retain(|tool| !removed_set.contains(tool));

    if added_tools.is_empty()
        && removed_tools.is_empty()
        && disabled_user_skills.is_empty()
        && enabled_user_skills.is_empty()
        && subagent_overrides.is_empty()
        && tool_permission_rules.is_empty()
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
        tool_permission_rules,
    })
}

fn build_agent_profile_view(
    agent_id: &str,
    default_tools: Vec<String>,
    include_implicit_thread_goal_tools: bool,
    mode_config: Option<&AgentProfileConfig>,
    valid_tools: &HashSet<String>,
) -> AgentProfileView {
    let default_tools = normalize_tools(default_tools, valid_tools);
    let enabled_tools = apply_implicit_thread_goal_policy(
        resolve_effective_tools(&default_tools, mode_config, valid_tools),
        include_implicit_thread_goal_tools,
    );
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
    include_implicit_thread_goal_tools: bool,
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
    stored.added_tools =
        apply_implicit_thread_goal_policy(stored.added_tools, include_implicit_thread_goal_tools);

    Ok(stored_agent_profile_from_overrides(
        StoredAgentProfileOverrides {
            agent_id: profile_id,
            added_tools: stored.added_tools,
            removed_tools: stored.removed_tools,
            disabled_user_skills: stored.disabled_user_skills,
            enabled_user_skills: stored.enabled_user_skills,
            subagent_overrides: stored.subagent_overrides,
            tool_permission_rules: stored.tool_permission_rules,
            default_tools,
            valid_tools,
        },
    ))
}

async fn get_valid_tool_names() -> HashSet<String> {
    get_all_registered_tools()
        .await
        .into_iter()
        .map(|tool| tool.name().to_string())
        .collect()
}

async fn get_mode_defaults() -> HashMap<String, AgentProfileDefaults> {
    let registry = get_agent_registry();
    registry
        .get_modes_info()
        .await
        .into_iter()
        .map(|mode| {
            let include_implicit_thread_goal_tools = registry
                .get_agent(&mode.id, None)
                .is_none_or(|agent| agent.include_implicit_thread_goal_tools());
            (
                mode.id,
                AgentProfileDefaults {
                    tools: mode.default_tools,
                    include_implicit_thread_goal_tools,
                },
            )
        })
        .collect()
}

/// Default tool lists for every agent that can own a stored profile.
///
/// Profiles are not a mode-only concept: sub-agents carry their own skill
/// selection, so a mode-only lookup rejects writes for agents the UI already
/// lets users configure (`ComputerUse` being the built-in case).
async fn get_agent_defaults() -> HashMap<String, AgentProfileDefaults> {
    let registry = get_agent_registry();
    let mut defaults = get_mode_defaults().await;
    for subagent in registry.get_subagents_info(None).await {
        let include_implicit_thread_goal_tools = registry
            .get_agent(&subagent.id, None)
            .is_none_or(|agent| agent.include_implicit_thread_goal_tools());
        defaults.entry(subagent.id).or_insert(AgentProfileDefaults {
            tools: subagent.default_tools,
            include_implicit_thread_goal_tools,
        });
    }
    defaults
}

async fn get_profile_defaults() -> HashMap<String, AgentProfileDefaults> {
    let mut defaults = HashMap::new();
    for (agent_id, profile_defaults) in get_agent_defaults().await {
        defaults
            .entry(resolve_profile_id(&agent_id))
            .or_insert(profile_defaults);
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
    for (mode_id, defaults) in mode_defaults {
        let profile_id = resolve_profile_id(&mode_id);
        let view = build_agent_profile_view(
            &mode_id,
            defaults.tools,
            defaults.include_implicit_thread_goal_tools,
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
    let agent_defaults = get_agent_defaults().await;
    let defaults = agent_defaults
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
        resolve_effective_tools(&defaults.tools, current, &valid_tools)
    };
    let enabled_tools = apply_implicit_thread_goal_policy(
        enabled_tools,
        defaults.include_implicit_thread_goal_tools,
    );

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

    let tool_permission_rules = if config
        .as_object()
        .map(|obj| obj.contains_key("tool_permission_rules"))
        .unwrap_or(false)
    {
        match config.get("tool_permission_rules") {
            Some(Value::Null) | None => Vec::new(),
            Some(value) => {
                serde_json::from_value::<Vec<PermissionRule>>(value.clone()).map_err(|error| {
                    BitFunError::config(format!(
                        "Invalid tool_permission_rules for mode '{}': {}",
                        agent_id, error
                    ))
                })?
            }
        }
    } else {
        current
            .map(|item| item.tool_permission_rules.clone())
            .unwrap_or_default()
    };

    if let Some(canonical) = stored_agent_profile_from_tool_selection(
        agent_id,
        enabled_tools,
        disabled_user_skills,
        enabled_user_skills,
        subagent_overrides,
        tool_permission_rules,
        &defaults.tools,
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
            && current.tool_permission_rules.is_empty()
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

    for (profile_id, defaults) in &profile_defaults {
        let raw_profile = raw_agent_profiles.get(profile_id);
        let canonical = canonicalize_agent_profile(
            profile_id,
            raw_profile,
            &defaults.tools,
            defaults.include_implicit_thread_goal_tools,
            &valid_tools,
        )?;
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

    // Profiles we cannot resolve defaults for are kept, not dropped. Canonicalization
    // runs at startup with no workspace, so project-scoped sub-agents are invisible
    // here; pruning them would silently discard the user's stored selection every
    // launch. Records that no longer deserialize are still removed, so one bad entry
    // cannot take the whole map down when it is read back.
    for (profile_id, raw_profile) in &raw_agent_profiles {
        if profile_defaults.contains_key(profile_id) {
            continue;
        }
        match serde_json::from_value::<AgentProfileConfig>(raw_profile.clone()) {
            Ok(config) => {
                rewritten_agent_profiles.insert(profile_id.clone(), serde_json::to_value(config)?);
            }
            Err(_) => removed_profile_configs.push(profile_id.clone()),
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
        agent_profile_member_mode_ids_for, canonicalize_agent_profile, get_agent_defaults,
        normalize_skill_override_lists, resolve_effective_tools,
        stored_agent_profile_from_overrides, StoredAgentProfileOverrides,
    };
    use crate::agentic::agents::get_agent_registry;
    use crate::service::config::types::{AgentProfileConfig, AgentSubagentOverrideState};
    use bitfun_agent_runtime::thread_goal_tools::THREAD_GOAL_TOOL_NAMES;
    use bitfun_runtime_ports::{PermissionEffect, PermissionRule};
    use serde_json::Value;
    use std::collections::HashSet;

    /// Skill selection is stored per agent profile, so every agent whose default
    /// tools include `Skill` must resolve to a profile default. Otherwise saving
    /// its selection fails with "Agent does not exist" — which is what happened
    /// to `ComputerUse`, a sub-agent the agents scene renders as a core card.
    #[tokio::test]
    async fn agents_shipping_the_skill_tool_can_own_a_stored_profile() {
        let defaults = get_agent_defaults().await;

        assert!(
            defaults.contains_key("ComputerUse"),
            "ComputerUse ships the Skill tool but cannot own a stored profile"
        );

        for subagent in get_agent_registry().get_subagents_info(None).await {
            if subagent.default_tools.iter().any(|tool| tool == "Skill") {
                assert!(
                    defaults.contains_key(&subagent.id),
                    "sub-agent '{}' ships the Skill tool but cannot own a stored profile",
                    subagent.id
                );
            }
        }
    }

    #[test]
    fn mode_profiles_cannot_remove_required_thread_goal_tools() {
        let default_tools = vec![
            "Read".to_string(),
            "get_goal".to_string(),
            "create_goal".to_string(),
            "update_goal".to_string(),
        ];
        let valid_tools = default_tools.iter().cloned().collect();
        let stored = stored_agent_profile_from_overrides(StoredAgentProfileOverrides {
            agent_id: "Claw",
            added_tools: Vec::new(),
            removed_tools: default_tools.clone(),
            disabled_user_skills: Vec::new(),
            enabled_user_skills: Vec::new(),
            subagent_overrides: Default::default(),
            tool_permission_rules: Vec::new(),
            default_tools: &default_tools,
            valid_tools: &valid_tools,
        })
        .expect("the ordinary Read removal should keep the profile");

        assert_eq!(stored.removed_tools, vec!["Read".to_string()]);
        for tool_name in THREAD_GOAL_TOOL_NAMES {
            assert!(!stored.removed_tools.iter().any(|tool| tool == tool_name));
        }

        let legacy_config = AgentProfileConfig {
            profile_id: "Claw".to_string(),
            removed_tools: default_tools.clone(),
            ..AgentProfileConfig::default()
        };
        let effective_tools =
            resolve_effective_tools(&default_tools, Some(&legacy_config), &valid_tools);
        assert!(!effective_tools.contains(&"Read".to_string()));
        for tool_name in THREAD_GOAL_TOOL_NAMES {
            assert!(effective_tools.iter().any(|tool| tool == tool_name));
        }
    }

    #[test]
    fn normalize_skill_override_lists_removes_duplicates_and_conflicts() {
        let (disabled, enabled) = normalize_skill_override_lists(
            vec![
                "user::bitfun-system::ppt-design".to_string(),
                "user::bitfun-system::ppt-design".to_string(),
            ],
            vec![
                "user::bitfun-system::ppt-design".to_string(),
                "user::bitfun-system::agent-browser".to_string(),
                "user::bitfun-system::agent-browser".to_string(),
            ],
        );

        assert_eq!(
            disabled,
            vec!["user::bitfun-system::ppt-design".to_string()]
        );
        assert_eq!(
            enabled,
            vec!["user::bitfun-system::agent-browser".to_string()]
        );
    }

    #[test]
    fn stored_agent_profile_from_overrides_keeps_enabled_user_skills() {
        let valid_tools = HashSet::new();
        let stored = stored_agent_profile_from_overrides(StoredAgentProfileOverrides {
            agent_id: "agentic",
            added_tools: Vec::new(),
            removed_tools: Vec::new(),
            disabled_user_skills: Vec::new(),
            enabled_user_skills: vec!["user::bitfun-system::ppt-design".to_string()],
            subagent_overrides: Default::default(),
            tool_permission_rules: Vec::new(),
            default_tools: &[],
            valid_tools: &valid_tools,
        })
        .expect("mode config should be retained when skill overrides exist");

        assert_eq!(stored.profile_id, "coding_shared");
        assert_eq!(
            stored.enabled_user_skills,
            vec!["user::bitfun-system::ppt-design".to_string()]
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
        let stored = stored_agent_profile_from_overrides(StoredAgentProfileOverrides {
            agent_id: "debug",
            added_tools: Vec::new(),
            removed_tools: Vec::new(),
            disabled_user_skills: Vec::new(),
            enabled_user_skills: Vec::new(),
            subagent_overrides: subagent_overrides.clone(),
            tool_permission_rules: Vec::new(),
            default_tools: &[],
            valid_tools: &valid_tools,
        })
        .expect("mode config should be retained when subagent overrides exist");

        assert_eq!(stored.profile_id, "coding_shared");
        assert_eq!(stored.subagent_overrides, subagent_overrides);
    }

    #[test]
    fn stored_agent_profile_from_overrides_keeps_permission_rules() {
        let valid_tools = HashSet::new();
        let rules = vec![PermissionRule::new(
            "read",
            "secrets/*",
            PermissionEffect::Deny,
        )];
        let stored = stored_agent_profile_from_overrides(StoredAgentProfileOverrides {
            agent_id: "agentic",
            added_tools: Vec::new(),
            removed_tools: Vec::new(),
            disabled_user_skills: Vec::new(),
            enabled_user_skills: Vec::new(),
            subagent_overrides: Default::default(),
            tool_permission_rules: rules.clone(),
            default_tools: &[],
            valid_tools: &valid_tools,
        })
        .expect("permission-only profile should be retained");

        assert_eq!(stored.tool_permission_rules, rules);
    }

    #[test]
    fn canonicalize_agent_profile_preserves_permission_rules() {
        let raw = serde_json::json!({
            "tool_permission_rules": [{
                "action": "edit",
                "resource": "generated/*",
                "effect": "allow"
            }]
        });
        let canonical =
            canonicalize_agent_profile("agentic", Some(&raw), &[], true, &HashSet::new())
                .expect("profile should canonicalize")
                .expect("permission-only profile should be present");

        assert_eq!(canonical.tool_permission_rules.len(), 1);
        assert_eq!(canonical.tool_permission_rules[0].action, "edit");
    }

    #[test]
    fn canonicalize_agent_profile_treats_null_as_missing() {
        let canonical =
            canonicalize_agent_profile("Claw", Some(&Value::Null), &[], true, &HashSet::new())
                .expect("null mode config should be ignored");

        assert!(canonical.is_none());
    }

    #[test]
    fn canonicalize_agent_profile_preserves_mcp_override_before_registration() {
        let raw = serde_json::json!({
            "profile_id": "coding_shared",
            "added_tools": ["mcp__github__list_issues", "missing_static_tool"]
        });
        let canonical = canonicalize_agent_profile(
            "coding_shared",
            Some(&raw),
            &["Read".to_string()],
            true,
            &HashSet::from(["Read".to_string()]),
        )
        .expect("profile should canonicalize")
        .expect("the MCP override should keep the profile");

        assert_eq!(
            canonical.added_tools,
            vec!["mcp__github__list_issues".to_string()]
        );
    }

    #[test]
    fn minimal_profile_drops_implicit_goal_tools_from_views_and_stored_overrides() {
        let default_tools = vec!["Read".to_string(), "ExecCommand".to_string()];
        let valid_tools = HashSet::from([
            "Read".to_string(),
            "ExecCommand".to_string(),
            "get_goal".to_string(),
            "create_goal".to_string(),
            "update_goal".to_string(),
        ]);
        let view = super::build_agent_profile_view(
            "minimal",
            default_tools.clone(),
            false,
            None,
            &valid_tools,
        );
        assert_eq!(view.enabled_tools, default_tools);

        let raw = serde_json::json!({
            "profile_id": "minimal",
            "added_tools": ["get_goal", "update_goal"]
        });
        let canonical = canonicalize_agent_profile(
            "minimal",
            Some(&raw),
            &view.default_tools,
            false,
            &valid_tools,
        )
        .expect("minimal profile should canonicalize");
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
