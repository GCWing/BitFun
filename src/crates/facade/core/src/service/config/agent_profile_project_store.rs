use crate::infrastructure::get_path_manager_arc;
use crate::service::config::types::{AgentSubagentOverrideState, ParentSubagentOverrideConfig};
use crate::util::errors::{BitFunError, BitFunResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub const PROJECT_AGENT_PROFILES_FILE_NAME: &str = "agent_profiles.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ProjectAgentProfileSkillConfig {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub disabled_project_skills: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ProjectAgentProfileSubagentConfig {
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub overrides: ParentSubagentOverrideConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ProjectAgentProfileEntry {
    #[serde(skip_serializing_if = "ProjectAgentProfileSkillConfig::is_empty")]
    pub skills: ProjectAgentProfileSkillConfig,
    #[serde(skip_serializing_if = "ProjectAgentProfileSubagentConfig::is_empty")]
    pub subagents: ProjectAgentProfileSubagentConfig,
}

pub type ProjectAgentProfilesDocument = HashMap<String, ProjectAgentProfileEntry>;

impl ProjectAgentProfileSkillConfig {
    fn is_empty(&self) -> bool {
        self.disabled_project_skills.is_empty()
    }
}

impl ProjectAgentProfileSubagentConfig {
    fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }
}

impl ProjectAgentProfileEntry {
    fn is_empty(&self) -> bool {
        self.skills.is_empty() && self.subagents.is_empty()
    }
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

pub fn normalize_project_agent_profiles_document(
    document: ProjectAgentProfilesDocument,
) -> ProjectAgentProfilesDocument {
    let mut normalized = ProjectAgentProfilesDocument::new();

    for (profile_id, mut entry) in document {
        let trimmed_profile_id = profile_id.trim();
        if trimmed_profile_id.is_empty() {
            continue;
        }

        entry.skills.disabled_project_skills =
            dedupe_preserving_order(entry.skills.disabled_project_skills);
        entry.subagents.overrides = normalize_subagent_overrides(entry.subagents.overrides);

        if !entry.is_empty() {
            normalized.insert(trimmed_profile_id.to_string(), entry);
        }
    }

    normalized
}

pub fn deserialize_project_agent_profiles_document(
    content: &str,
) -> BitFunResult<ProjectAgentProfilesDocument> {
    Ok(normalize_project_agent_profiles_document(
        serde_json::from_str(content)?,
    ))
}

pub fn serialize_project_agent_profiles_document(
    document: &ProjectAgentProfilesDocument,
) -> BitFunResult<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(
        &normalize_project_agent_profiles_document(document.clone()),
    )?)
}

pub fn project_agent_profiles_path_for_remote(remote_root: &str) -> String {
    format!(
        "{}/.bitfun/config/{}",
        remote_root.trim_end_matches('/'),
        PROJECT_AGENT_PROFILES_FILE_NAME
    )
}

pub async fn load_project_agent_profiles_document_local(
    workspace_root: &Path,
) -> BitFunResult<ProjectAgentProfilesDocument> {
    let path = get_path_manager_arc().project_agent_profiles_file(workspace_root);
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => deserialize_project_agent_profiles_document(&content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ProjectAgentProfilesDocument::new())
        }
        Err(error) => Err(BitFunError::config(format!(
            "Failed to read project agent profiles file '{}': {}",
            path.display(),
            error
        ))),
    }
}

pub async fn save_project_agent_profiles_document_local(
    workspace_root: &Path,
    document: &ProjectAgentProfilesDocument,
) -> BitFunResult<()> {
    let path = get_path_manager_arc().project_agent_profiles_file(workspace_root);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, serialize_project_agent_profiles_document(document)?).await?;
    Ok(())
}

pub fn get_disabled_project_skills(
    document: &ProjectAgentProfilesDocument,
    profile_id: &str,
) -> Vec<String> {
    document
        .get(profile_id)
        .map(|entry| entry.skills.disabled_project_skills.clone())
        .unwrap_or_default()
}

pub fn set_disabled_project_skills(
    document: &mut ProjectAgentProfilesDocument,
    profile_id: &str,
    skill_keys: Vec<String>,
) -> Vec<String> {
    let trimmed_profile_id = profile_id.trim();
    if trimmed_profile_id.is_empty() {
        return Vec::new();
    }

    let next = dedupe_preserving_order(skill_keys);
    let entry = document.entry(trimmed_profile_id.to_string()).or_default();
    entry.skills.disabled_project_skills = next.clone();

    if entry.is_empty() {
        document.remove(trimmed_profile_id);
    }

    next
}

pub fn set_project_skill_disabled(
    document: &mut ProjectAgentProfilesDocument,
    profile_id: &str,
    skill_key: &str,
    disabled: bool,
) -> Vec<String> {
    let current = get_disabled_project_skills(document, profile_id);
    let mut next = current;

    if disabled {
        next.push(skill_key.to_string());
    } else {
        next.retain(|value| value != skill_key);
    }

    set_disabled_project_skills(document, profile_id, next)
}

pub fn get_project_subagent_overrides(
    document: &ProjectAgentProfilesDocument,
    profile_id: &str,
) -> ParentSubagentOverrideConfig {
    document
        .get(profile_id)
        .map(|entry| entry.subagents.overrides.clone())
        .unwrap_or_default()
}

pub fn set_project_subagent_overrides(
    document: &mut ProjectAgentProfilesDocument,
    profile_id: &str,
    overrides: ParentSubagentOverrideConfig,
) -> ParentSubagentOverrideConfig {
    let trimmed_profile_id = profile_id.trim();
    if trimmed_profile_id.is_empty() {
        return ParentSubagentOverrideConfig::new();
    }

    let normalized = normalize_subagent_overrides(overrides);
    let entry = document.entry(trimmed_profile_id.to_string()).or_default();
    entry.subagents.overrides = normalized.clone();

    if entry.is_empty() {
        document.remove(trimmed_profile_id);
    }

    normalized
}

pub fn set_project_subagent_override_state(
    document: &mut ProjectAgentProfilesDocument,
    profile_id: &str,
    subagent_key: &str,
    state: Option<AgentSubagentOverrideState>,
) -> ParentSubagentOverrideConfig {
    let mut overrides = get_project_subagent_overrides(document, profile_id);
    match state {
        Some(state) => {
            overrides.insert(subagent_key.trim().to_string(), state);
        }
        None => {
            overrides.remove(subagent_key.trim());
        }
    }
    set_project_subagent_overrides(document, profile_id, overrides)
}
