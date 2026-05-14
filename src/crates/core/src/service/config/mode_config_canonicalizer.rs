//! Mode tool configuration migration and resolution.
//!
//! Stored configuration keeps only user overrides. Effective tool lists are
//! derived from the current mode defaults at runtime.

use crate::agentic::agents::get_agent_registry;
use crate::agentic::tools::registry::get_all_registered_tools;
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::types::{ModeConfig, ModeConfigView};
use crate::util::errors::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

/// Mode config canonicalization report.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ModeConfigCanonicalizationReport {
    pub removed_mode_configs: Vec<String>,
    pub updated_modes: Vec<ModeConfigUpdateInfo>,
}

/// Mode config update information.
#[derive(Debug, Serialize, Deserialize)]
pub struct ModeConfigUpdateInfo {
    pub mode_id: String,
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

pub fn resolve_effective_tools(
    default_tools: &[String],
    mode_config: Option<&ModeConfig>,
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

fn stored_mode_from_tool_selection(
    mode_id: &str,
    enabled_tools: Vec<String>,
    disabled_user_skills: Vec<String>,
    enabled_user_skills: Vec<String>,
    default_tools: &[String],
    valid_tools: &HashSet<String>,
) -> Option<ModeConfig> {
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

    stored_mode_from_overrides(
        mode_id,
        added_tools,
        removed_tools,
        disabled_user_skills,
        enabled_user_skills,
        &default_tools,
        valid_tools,
    )
}

fn stored_mode_from_overrides(
    mode_id: &str,
    added_tools: Vec<String>,
    removed_tools: Vec<String>,
    disabled_user_skills: Vec<String>,
    enabled_user_skills: Vec<String>,
    default_tools: &[String],
    valid_tools: &HashSet<String>,
) -> Option<ModeConfig> {
    let default_set: HashSet<String> = default_tools.iter().cloned().collect();
    let mut added_tools = normalize_tools(added_tools, valid_tools);
    let mut removed_tools = normalize_tools(removed_tools, valid_tools);
    let (disabled_user_skills, enabled_user_skills) =
        normalize_skill_override_lists(disabled_user_skills, enabled_user_skills);

    added_tools.retain(|tool| !default_set.contains(tool));
    removed_tools.retain(|tool| default_set.contains(tool));

    let removed_set: HashSet<String> = removed_tools.iter().cloned().collect();
    added_tools.retain(|tool| !removed_set.contains(tool));

    if added_tools.is_empty()
        && removed_tools.is_empty()
        && disabled_user_skills.is_empty()
        && enabled_user_skills.is_empty()
    {
        return None;
    }

    Some(ModeConfig {
        mode_id: mode_id.to_string(),
        added_tools,
        removed_tools,
        disabled_user_skills,
        enabled_user_skills,
    })
}

fn build_mode_view(
    mode_id: &str,
    default_tools: Vec<String>,
    mode_config: Option<&ModeConfig>,
    valid_tools: &HashSet<String>,
) -> ModeConfigView {
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

    ModeConfigView {
        mode_id: mode_id.to_string(),
        enabled_tools,
        default_tools,
        disabled_user_skills,
        enabled_user_skills,
    }
}

fn canonicalize_mode_config(
    mode_id: &str,
    raw_mode: Option<&Value>,
    default_tools: &[String],
    valid_tools: &HashSet<String>,
) -> BitFunResult<Option<ModeConfig>> {
    let Some(raw_mode) = raw_mode else {
        return Ok(None);
    };
    if raw_mode.is_null() {
        return Ok(None);
    }

    let mut stored: ModeConfig = serde_json::from_value(raw_mode.clone()).map_err(|error| {
        BitFunError::config(format!(
            "Failed to deserialize mode config '{}': {}",
            mode_id, error
        ))
    })?;
    if stored.mode_id.trim().is_empty() {
        stored.mode_id = mode_id.to_string();
    }

    Ok(stored_mode_from_overrides(
        mode_id,
        stored.added_tools,
        stored.removed_tools,
        stored.disabled_user_skills,
        stored.enabled_user_skills,
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

pub async fn get_mode_config_views() -> BitFunResult<HashMap<String, ModeConfigView>> {
    let config_service = GlobalConfigManager::get_service().await?;
    let stored_configs: HashMap<String, ModeConfig> = config_service
        .get_config(Some("ai.mode_configs"))
        .await
        .unwrap_or_default();
    let mode_defaults = get_mode_defaults().await;
    let valid_tools = get_valid_tool_names().await;

    let mut views = HashMap::new();
    for (mode_id, default_tools) in mode_defaults {
        let view = build_mode_view(
            &mode_id,
            default_tools,
            stored_configs.get(&mode_id),
            &valid_tools,
        );
        views.insert(mode_id, view);
    }

    Ok(views)
}

pub async fn get_mode_config_view(mode_id: &str) -> BitFunResult<ModeConfigView> {
    let views = get_mode_config_views().await?;
    views
        .get(mode_id)
        .cloned()
        .ok_or_else(|| BitFunError::config(format!("Mode does not exist: {}", mode_id)))
}

pub async fn persist_mode_config_from_value(mode_id: &str, config: Value) -> BitFunResult<()> {
    let config_service = GlobalConfigManager::get_service().await?;
    let mut stored_configs: HashMap<String, ModeConfig> = config_service
        .get_config(Some("ai.mode_configs"))
        .await
        .unwrap_or_default();
    let mode_defaults = get_mode_defaults().await;
    let default_tools = mode_defaults
        .get(mode_id)
        .ok_or_else(|| BitFunError::config(format!("Mode does not exist: {}", mode_id)))?;
    let valid_tools = get_valid_tool_names().await;
    let current = stored_configs.get(mode_id);

    let enabled_tools = if let Some(tools) = config.get("enabled_tools") {
        serde_json::from_value::<Vec<String>>(tools.clone()).map_err(|error| {
            BitFunError::config(format!(
                "Invalid enabled_tools for mode '{}': {}",
                mode_id, error
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
                        mode_id, error
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
                        mode_id, error
                    ))
                })?
            }
        }
    } else {
        current
            .map(|item| item.enabled_user_skills.clone())
            .unwrap_or_default()
    };

    if let Some(canonical) = stored_mode_from_tool_selection(
        mode_id,
        enabled_tools,
        disabled_user_skills,
        enabled_user_skills,
        default_tools,
        &valid_tools,
    ) {
        stored_configs.insert(mode_id.to_string(), canonical);
    } else {
        stored_configs.remove(mode_id);
    }

    config_service
        .set_config("ai.mode_configs", stored_configs)
        .await
}

pub async fn reset_mode_config_to_default(mode_id: &str) -> BitFunResult<()> {
    let config_service = GlobalConfigManager::get_service().await?;
    let mut stored_configs: HashMap<String, ModeConfig> = config_service
        .get_config(Some("ai.mode_configs"))
        .await
        .unwrap_or_default();
    stored_configs.remove(mode_id);
    config_service
        .set_config("ai.mode_configs", stored_configs)
        .await
}

/// Canonicalizes stored mode config overrides.
pub async fn canonicalize_mode_configs() -> BitFunResult<ModeConfigCanonicalizationReport> {
    let config_service = GlobalConfigManager::get_service().await?;
    let valid_tools = get_valid_tool_names().await;
    let mode_defaults = get_mode_defaults().await;
    let mut ai_value: Value = config_service.get_config(Some("ai")).await?;
    let original_ai_value = ai_value.clone();
    let ai_object = ai_value
        .as_object_mut()
        .ok_or_else(|| BitFunError::config("AI config must be a JSON object".to_string()))?;

    let raw_mode_configs = ai_object
        .get("mode_configs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut rewritten_mode_configs = Map::new();
    let mut updated_modes = Vec::new();
    let mut removed_mode_configs = Vec::new();

    for (mode_id, default_tools) in &mode_defaults {
        let raw_mode = raw_mode_configs.get(mode_id);
        let canonical = canonicalize_mode_config(mode_id, raw_mode, default_tools, &valid_tools)?;
        if let Some(config) = canonical {
            if raw_mode.is_some() {
                updated_modes.push(ModeConfigUpdateInfo {
                    mode_id: mode_id.clone(),
                    added_tools: config.added_tools.clone(),
                    removed_tools: config.removed_tools.clone(),
                });
            }
            rewritten_mode_configs.insert(mode_id.clone(), serde_json::to_value(config)?);
        } else if raw_mode.is_some() {
            removed_mode_configs.push(mode_id.clone());
        }
    }

    for mode_id in raw_mode_configs.keys() {
        if !mode_defaults.contains_key(mode_id) {
            removed_mode_configs.push(mode_id.clone());
        }
    }

    ai_object.insert(
        "mode_configs".to_string(),
        Value::Object(rewritten_mode_configs),
    );

    if ai_value != original_ai_value {
        config_service.set_config("ai", ai_value).await?;
    }

    Ok(ModeConfigCanonicalizationReport {
        removed_mode_configs,
        updated_modes,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        canonicalize_mode_config, normalize_skill_override_lists, stored_mode_from_overrides,
    };
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
    fn stored_mode_from_overrides_keeps_enabled_user_skills() {
        let valid_tools = HashSet::new();
        let stored = stored_mode_from_overrides(
            "agentic",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec!["user::bitfun-system::pdf".to_string()],
            &[],
            &valid_tools,
        )
        .expect("mode config should be retained when skill overrides exist");

        assert_eq!(
            stored.enabled_user_skills,
            vec!["user::bitfun-system::pdf".to_string()]
        );
        assert!(stored.disabled_user_skills.is_empty());
    }

    #[test]
    fn canonicalize_mode_config_treats_null_as_missing() {
        let canonical = canonicalize_mode_config(
            "Claw",
            Some(&Value::Null),
            &[],
            &HashSet::new(),
        )
        .expect("null mode config should be ignored");

        assert!(canonical.is_none());
    }
}
