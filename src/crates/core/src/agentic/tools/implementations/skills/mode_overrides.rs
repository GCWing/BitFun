//! Mode-specific skill override helpers.

use crate::agentic::workspace::WorkspaceFileSystem;
use crate::infrastructure::get_path_manager_arc;
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::mode_config_canonicalizer::persist_mode_config_from_value;
use crate::service::config::types::ModeConfig;
use crate::util::errors::{BitFunError, BitFunResult};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const PROJECT_MODE_SKILLS_FILE_NAME: &str = "mode_skills.json";
const DISABLED_SKILLS_KEY: &str = "disabled_skills";

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

pub async fn load_user_mode_skill_overrides(mode_id: &str) -> BitFunResult<UserModeSkillOverrides> {
    let config_service = GlobalConfigManager::get_service().await?;
    let stored_configs: HashMap<String, ModeConfig> = config_service
        .get_config(Some("ai.mode_configs"))
        .await
        .unwrap_or_default();

    let config = stored_configs.get(mode_id);
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
    } else {
        if enabled {
            overrides.enabled_skills.push(skill_key.to_string());
        }
    }

    let overrides = normalize_user_overrides(overrides.disabled_skills, overrides.enabled_skills);

    persist_mode_config_from_value(
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
    persist_mode_config_from_value(
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
    format!(
        "{}/.bitfun/config/{}",
        remote_root.trim_end_matches('/'),
        PROJECT_MODE_SKILLS_FILE_NAME
    )
}

fn normalize_project_document_value(value: Value) -> Value {
    match value {
        Value::Object(_) => value,
        _ => Value::Object(Map::new()),
    }
}

fn mode_skills_object_mut(document: &mut Value) -> BitFunResult<&mut Map<String, Value>> {
    if !document.is_object() {
        *document = Value::Object(Map::new());
    }

    document
        .as_object_mut()
        .ok_or_else(|| BitFunError::config("Project mode skills must be a JSON object".to_string()))
}

fn mode_skills_object(document: &Value) -> Option<&Map<String, Value>> {
    document.as_object()
}

pub fn get_disabled_mode_skills_from_document(document: &Value, mode_id: &str) -> Vec<String> {
    let Some(mode_object) = mode_skills_object(document)
        .and_then(|map| map.get(mode_id))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };

    let keys = mode_object
        .get(DISABLED_SKILLS_KEY)
        .cloned()
        .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
        .unwrap_or_default();

    dedupe_skill_keys(keys)
}

pub fn set_mode_skill_disabled_in_document(
    document: &mut Value,
    mode_id: &str,
    skill_key: &str,
    disabled: bool,
) -> BitFunResult<Vec<String>> {
    let mode_skills = mode_skills_object_mut(document)?;
    let mode_entry = mode_skills
        .entry(mode_id.to_string())
        .or_insert_with(|| Value::Object(Map::new()));

    if !mode_entry.is_object() {
        *mode_entry = Value::Object(Map::new());
    }

    let mode_object = mode_entry.as_object_mut().ok_or_else(|| {
        BitFunError::config("Mode skills entry must be a JSON object".to_string())
    })?;

    let current = mode_object
        .get(DISABLED_SKILLS_KEY)
        .cloned()
        .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
        .unwrap_or_default();

    let mut next = dedupe_skill_keys(current);
    if disabled {
        next.push(skill_key.to_string());
        next = dedupe_skill_keys(next);
    } else {
        next.retain(|value| value != skill_key);
    }

    if next.is_empty() {
        mode_object.remove(DISABLED_SKILLS_KEY);
    } else {
        mode_object.insert(
            DISABLED_SKILLS_KEY.to_string(),
            serde_json::to_value(&next)?,
        );
    }

    if mode_object.is_empty() {
        mode_skills.remove(mode_id);
    }

    Ok(next)
}

pub fn set_disabled_mode_skills_in_document(
    document: &mut Value,
    mode_id: &str,
    skill_keys: Vec<String>,
) -> BitFunResult<Vec<String>> {
    let mode_skills = mode_skills_object_mut(document)?;
    let next = dedupe_skill_keys(skill_keys);

    if next.is_empty() {
        if let Some(mode_entry) = mode_skills.get_mut(mode_id) {
            if !mode_entry.is_object() {
                *mode_entry = Value::Object(Map::new());
            }

            if let Some(mode_object) = mode_entry.as_object_mut() {
                mode_object.remove(DISABLED_SKILLS_KEY);
                if mode_object.is_empty() {
                    mode_skills.remove(mode_id);
                }
            }
        }

        return Ok(Vec::new());
    }

    let mode_entry = mode_skills
        .entry(mode_id.to_string())
        .or_insert_with(|| Value::Object(Map::new()));

    if !mode_entry.is_object() {
        *mode_entry = Value::Object(Map::new());
    }

    let mode_object = mode_entry.as_object_mut().ok_or_else(|| {
        BitFunError::config("Mode skills entry must be a JSON object".to_string())
    })?;

    mode_object.insert(
        DISABLED_SKILLS_KEY.to_string(),
        serde_json::to_value(&next)?,
    );

    Ok(next)
}

pub async fn load_project_mode_skills_document_local(workspace_root: &Path) -> BitFunResult<Value> {
    let path = get_path_manager_arc().project_mode_skills_file(workspace_root);
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => Ok(normalize_project_document_value(serde_json::from_str(
            &content,
        )?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Value::Object(Map::new())),
        Err(error) => Err(BitFunError::config(format!(
            "Failed to read project skill overrides file '{}': {}",
            path.display(),
            error
        ))),
    }
}

pub async fn save_project_mode_skills_document_local(
    workspace_root: &Path,
    document: &Value,
) -> BitFunResult<()> {
    let path = get_path_manager_arc().project_mode_skills_file(workspace_root);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, serde_json::to_vec_pretty(document)?).await?;
    Ok(())
}

pub async fn load_disabled_mode_skills_local(
    workspace_root: &Path,
    mode_id: &str,
) -> BitFunResult<Vec<String>> {
    let document = load_project_mode_skills_document_local(workspace_root).await?;
    Ok(get_disabled_mode_skills_from_document(&document, mode_id))
}

pub async fn load_disabled_mode_skills_remote(
    fs: &dyn WorkspaceFileSystem,
    remote_root: &str,
    mode_id: &str,
) -> BitFunResult<Vec<String>> {
    let path = project_mode_skills_path_for_remote(remote_root);
    let exists = fs.exists(&path).await.unwrap_or(false);
    if !exists {
        return Ok(Vec::new());
    }

    let content = fs.read_file_text(&path).await.map_err(|error| {
        BitFunError::config(format!(
            "Failed to read remote project skill overrides: {}",
            error
        ))
    })?;
    let document = normalize_project_document_value(serde_json::from_str(&content)?);
    Ok(get_disabled_mode_skills_from_document(&document, mode_id))
}
