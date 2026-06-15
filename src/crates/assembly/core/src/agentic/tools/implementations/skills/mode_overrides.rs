//! Mode-profile specific skill override helpers.

use crate::agentic::agents::resolve_mode_config_profile_id;
use crate::agentic::workspace::WorkspaceFileSystem;
use crate::service::config::agent_profile_project_store::{
    deserialize_project_agent_profiles_document, get_disabled_project_skills,
    load_project_agent_profiles_document_local, project_agent_profiles_path_for_remote,
    save_project_agent_profiles_document_local, set_disabled_project_skills,
    set_project_skill_disabled, ProjectAgentProfilesDocument,
};
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::mode_config_canonicalizer::persist_agent_profile_from_value;
use crate::service::config::types::AgentProfileConfig;
use crate::util::errors::{BitFunError, BitFunResult};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserModeSkillOverrides {
    pub disabled_skills: Vec<String>,
    pub enabled_skills: Vec<String>,
}

fn dedupe_skill_keys(keys: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for key in keys {
        let trimmed = key.trim();
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

fn normalize_user_overrides(
    disabled_skills: Vec<String>,
    enabled_skills: Vec<String>,
) -> UserModeSkillOverrides {
    let disabled_skills = dedupe_skill_keys(disabled_skills);
    let disabled_set: HashSet<String> = disabled_skills.iter().cloned().collect();
    let mut enabled_skills = dedupe_skill_keys(enabled_skills);
    enabled_skills.retain(|key| !disabled_set.contains(key));

    UserModeSkillOverrides {
        disabled_skills,
        enabled_skills,
    }
}

fn resolve_profile_id(mode_id: &str) -> String {
    resolve_mode_config_profile_id(mode_id).into_owned()
}

pub async fn load_user_mode_skill_overrides(mode_id: &str) -> BitFunResult<UserModeSkillOverrides> {
    let config_service = GlobalConfigManager::get_service().await?;
    let stored_configs: HashMap<String, AgentProfileConfig> = config_service
        .get_config(Some("ai.agent_profiles"))
        .await
        .unwrap_or_default();
    let profile_id = resolve_profile_id(mode_id);

    let config = stored_configs.get(&profile_id);
    Ok(normalize_user_overrides(
        config
            .map(|item| item.disabled_user_skills.clone())
            .unwrap_or_default(),
        config
            .map(|item| item.enabled_user_skills.clone())
            .unwrap_or_default(),
    ))
}

pub async fn set_user_mode_skill_state(
    mode_id: &str,
    skill_key: &str,
    enabled: bool,
    default_enabled: bool,
) -> BitFunResult<UserModeSkillOverrides> {
    let mut overrides = load_user_mode_skill_overrides(mode_id).await?;
    overrides.disabled_skills.retain(|value| value != skill_key);
    overrides.enabled_skills.retain(|value| value != skill_key);

    if default_enabled {
        if !enabled {
            overrides.disabled_skills.push(skill_key.to_string());
        }
    } else if enabled {
        overrides.enabled_skills.push(skill_key.to_string());
    }

    let overrides = normalize_user_overrides(overrides.disabled_skills, overrides.enabled_skills);

    persist_agent_profile_from_value(
        mode_id,
        json!({
            "disabled_user_skills": overrides.disabled_skills,
            "enabled_user_skills": overrides.enabled_skills,
        }),
    )
    .await?;

    load_user_mode_skill_overrides(mode_id).await
}

pub async fn clear_user_mode_skill_overrides(
    mode_id: &str,
) -> BitFunResult<UserModeSkillOverrides> {
    persist_agent_profile_from_value(
        mode_id,
        json!({
            "disabled_user_skills": Vec::<String>::new(),
            "enabled_user_skills": Vec::<String>::new(),
        }),
    )
    .await?;

    load_user_mode_skill_overrides(mode_id).await
}

pub fn project_mode_skills_path_for_remote(remote_root: &str) -> String {
    project_agent_profiles_path_for_remote(remote_root)
}

pub fn get_disabled_mode_skills_from_document(
    document: &ProjectAgentProfilesDocument,
    mode_id: &str,
) -> Vec<String> {
    get_disabled_project_skills(document, &resolve_profile_id(mode_id))
}

pub fn set_mode_skill_disabled_in_document(
    document: &mut ProjectAgentProfilesDocument,
    mode_id: &str,
    skill_key: &str,
    disabled: bool,
) -> BitFunResult<Vec<String>> {
    Ok(set_project_skill_disabled(
        document,
        &resolve_profile_id(mode_id),
        skill_key,
        disabled,
    ))
}

pub fn set_disabled_mode_skills_in_document(
    document: &mut ProjectAgentProfilesDocument,
    mode_id: &str,
    skill_keys: Vec<String>,
) -> BitFunResult<Vec<String>> {
    Ok(set_disabled_project_skills(
        document,
        &resolve_profile_id(mode_id),
        skill_keys,
    ))
}

pub async fn load_project_mode_skills_document_local(
    workspace_root: &Path,
) -> BitFunResult<ProjectAgentProfilesDocument> {
    load_project_agent_profiles_document_local(workspace_root).await
}

pub async fn save_project_mode_skills_document_local(
    workspace_root: &Path,
    document: &ProjectAgentProfilesDocument,
) -> BitFunResult<()> {
    save_project_agent_profiles_document_local(workspace_root, document).await
}

pub async fn load_disabled_mode_skills_local(
    workspace_root: &Path,
    mode_id: &str,
) -> BitFunResult<Vec<String>> {
    let document = load_project_agent_profiles_document_local(workspace_root).await?;
    Ok(get_disabled_project_skills(
        &document,
        &resolve_profile_id(mode_id),
    ))
}

pub async fn load_disabled_mode_skills_remote(
    fs: &dyn WorkspaceFileSystem,
    remote_root: &str,
    mode_id: &str,
) -> BitFunResult<Vec<String>> {
    let path = project_agent_profiles_path_for_remote(remote_root);
    let exists = fs.exists(&path).await.unwrap_or(false);
    if !exists {
        return Ok(Vec::new());
    }

    let content = fs.read_file_text(&path).await.map_err(|error| {
        BitFunError::config(format!(
            "Failed to read remote project mode profiles: {}",
            error
        ))
    })?;
    let document = deserialize_project_agent_profiles_document(&content)?;
    Ok(get_disabled_project_skills(
        &document,
        &resolve_profile_id(mode_id),
    ))
}
