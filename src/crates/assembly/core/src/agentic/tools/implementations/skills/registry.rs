//! Skill registry
//!
//! Manages skill discovery, mode-specific filtering, and loading.

use super::builtin::ensure_builtin_skills_installed;
use super::mode_overrides::{
    load_disabled_mode_skills_local, load_disabled_mode_skills_remote,
    load_user_mode_skill_overrides, UserModeSkillOverrides,
};
use super::types::{ModeSkillInfo, SkillData, SkillInfo, SkillLocation};
use crate::agentic::workspace::WorkspaceFileSystem;
use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_agent_runtime::skills::{
    annotate_shadowed_skills, build_mode_skill_infos, filter_candidates_for_mode,
    normalize_local_skill_dir_name, normalize_remote_skill_dir_name, normalize_skill_keys,
    resolve_default_hidden_builtin_for_explicit_invocation, resolve_user_config_skill_root,
    resolve_visible_skills, sort_skill_candidates_by_dir, sort_skills,
    ExplicitSkillInvocationResolution, SkillCandidate, BITFUN_SKILL_SOURCE_ID,
    BITFUN_SKILL_SOURCE_LABEL, BITFUN_SYSTEM_SKILL_DIR, BITFUN_SYSTEM_SKILL_SLOT,
    BITFUN_USER_SKILL_SLOT, PROJECT_SKILL_KEY_PREFIX, PROJECT_SKILL_ROOTS, USER_CONFIG_SKILL_ROOTS,
    USER_HOME_SKILL_ROOTS, USER_SKILL_KEY_PREFIX,
};
use log::{debug, error};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::fs;
use tokio::sync::RwLock;

/// Global Skill registry instance
static SKILL_REGISTRY: OnceLock<SkillRegistry> = OnceLock::new();

#[derive(Debug, Clone)]
struct SkillRootEntry {
    path: PathBuf,
    level: SkillLocation,
    slot: &'static str,
    source_id: &'static str,
    source_label: &'static str,
    priority: usize,
    is_builtin: bool,
}

#[derive(Debug, Clone)]
struct RemoteSkillRootEntry {
    path: String,
    slot: &'static str,
    source_id: &'static str,
    source_label: &'static str,
    priority: usize,
}

fn sort_remote_dir_entries(entries: &mut [crate::agentic::workspace::WorkspaceDirEntry]) {
    entries.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path.cmp(&b.path))
    });
}

/// Skill registry
pub struct SkillRegistry {
    /// Cached raw user-level skills (no workspace-specific project skills).
    cache: RwLock<Vec<SkillInfo>>,
}

impl SkillRegistry {
    fn new() -> Self {
        Self {
            cache: RwLock::new(Vec::new()),
        }
    }

    pub fn global() -> &'static Self {
        SKILL_REGISTRY.get_or_init(Self::new)
    }

    fn get_possible_paths_for_workspace(workspace_root: Option<&Path>) -> Vec<SkillRootEntry> {
        let mut entries = Vec::new();
        let mut priority = 0usize;
        let mut deferred_home_entries = Vec::new();

        if let Some(workspace_path) = workspace_root {
            for spec in PROJECT_SKILL_ROOTS {
                let path = workspace_path.join(spec.parent).join(spec.subdir);
                if path.exists() && path.is_dir() {
                    entries.push(SkillRootEntry {
                        path,
                        level: SkillLocation::Project,
                        slot: spec.slot,
                        source_id: spec.source_id,
                        source_label: spec.source_label,
                        priority,
                        is_builtin: false,
                    });
                }
                priority += 1;
            }
        }

        let home_dir = dirs::home_dir();

        if let Some(home) = home_dir.as_deref() {
            for spec in USER_HOME_SKILL_ROOTS {
                let path = home.join(spec.parent).join(spec.subdir);
                if spec.parent == ".opencode" {
                    deferred_home_entries.push((
                        path,
                        spec.slot,
                        spec.source_id,
                        spec.source_label,
                    ));
                } else if path.exists() && path.is_dir() {
                    entries.push(SkillRootEntry {
                        path,
                        level: SkillLocation::User,
                        slot: spec.slot,
                        source_id: spec.source_id,
                        source_label: spec.source_label,
                        priority,
                        is_builtin: false,
                    });
                }
                priority += 1;
            }
        }

        // BitFun's own user-defined skills sit between most home slots and config slots.
        // This lets other agent directories (e.g. ~/.claude/skills) take precedence
        // while still keeping config-level overrides after BitFun defaults.
        let path_manager = get_path_manager_arc();
        let bitfun_skills = path_manager.user_skills_dir();
        if bitfun_skills.exists() && bitfun_skills.is_dir() {
            entries.push(SkillRootEntry {
                path: bitfun_skills,
                level: SkillLocation::User,
                slot: BITFUN_USER_SKILL_SLOT,
                source_id: BITFUN_SKILL_SOURCE_ID,
                source_label: BITFUN_SKILL_SOURCE_LABEL,
                priority,
                is_builtin: false,
            });
        }
        priority += 1;

        let builtin_skills = path_manager.builtin_skills_dir();
        if builtin_skills.exists() && builtin_skills.is_dir() {
            entries.push(SkillRootEntry {
                path: builtin_skills,
                level: SkillLocation::User,
                slot: BITFUN_SYSTEM_SKILL_SLOT,
                source_id: BITFUN_SKILL_SOURCE_ID,
                source_label: BITFUN_SKILL_SOURCE_LABEL,
                priority,
                is_builtin: true,
            });
        }
        priority += 1;

        if let Some(config_dir) = dirs::config_dir() {
            for spec in USER_CONFIG_SKILL_ROOTS {
                let path = resolve_user_config_skill_root(spec, &config_dir, home_dir.as_deref());
                if path.exists() && path.is_dir() {
                    entries.push(SkillRootEntry {
                        path,
                        level: SkillLocation::User,
                        slot: spec.slot,
                        source_id: spec.source_id,
                        source_label: spec.source_label,
                        priority,
                        is_builtin: false,
                    });
                }
                priority += 1;
            }
        }

        for (path, slot, source_id, source_label) in deferred_home_entries {
            if path.exists() && path.is_dir() {
                entries.push(SkillRootEntry {
                    path,
                    level: SkillLocation::User,
                    slot,
                    source_id,
                    source_label,
                    priority,
                    is_builtin: false,
                });
            }
            priority += 1;
        }

        entries
    }

    async fn scan_skills_in_dir(entry: &SkillRootEntry) -> Vec<SkillCandidate> {
        let mut skills = Vec::new();
        if !entry.path.exists() {
            return skills;
        }

        let Ok(mut read_dir) = fs::read_dir(&entry.path).await else {
            return skills;
        };

        while let Ok(Some(item)) = read_dir.next_entry().await {
            let path = item.path();
            if !path.is_dir() {
                continue;
            }

            let Some(dir_name) = normalize_local_skill_dir_name(&path) else {
                continue;
            };

            if entry.slot == BITFUN_USER_SKILL_SLOT && dir_name == BITFUN_SYSTEM_SKILL_DIR {
                continue;
            }

            let skill_md_path = path.join("SKILL.md");
            if !skill_md_path.exists() {
                continue;
            }

            match fs::read_to_string(&skill_md_path).await {
                Ok(content) => match SkillData::from_markdown(
                    path.to_string_lossy().to_string(),
                    &content,
                    entry.level,
                    false,
                ) {
                    Ok(mut skill_data) => {
                        skill_data.dir_name = dir_name;
                        let key_prefix = match entry.level {
                            SkillLocation::User => USER_SKILL_KEY_PREFIX,
                            SkillLocation::Project => PROJECT_SKILL_KEY_PREFIX,
                        };
                        skills.push(SkillCandidate::from_data(
                            skill_data,
                            entry.slot,
                            entry.source_id,
                            entry.source_label,
                            key_prefix,
                            entry.priority,
                            entry.is_builtin,
                        ));
                    }
                    Err(error) => {
                        error!("Failed to parse SKILL.md in {}: {}", path.display(), error);
                    }
                },
                Err(error) => {
                    debug!("Failed to read {}: {}", skill_md_path.display(), error);
                }
            }
        }

        sort_skill_candidates_by_dir(skills)
    }

    async fn scan_skill_candidates_for_workspace(
        &self,
        workspace_root: Option<&Path>,
    ) -> Vec<SkillCandidate> {
        if let Err(error) = ensure_builtin_skills_installed().await {
            debug!("Failed to install built-in skills: {}", error);
        }

        let mut skills = Vec::new();
        for entry in Self::get_possible_paths_for_workspace(workspace_root) {
            let mut part = Self::scan_skills_in_dir(&entry).await;
            skills.append(&mut part);
        }
        skills
    }

    async fn scan_remote_project_skills(
        fs: &dyn WorkspaceFileSystem,
        remote_root: &str,
    ) -> Vec<SkillCandidate> {
        let mut roots = Vec::new();
        let root = remote_root.trim_end_matches('/');
        for (priority, spec) in PROJECT_SKILL_ROOTS.iter().enumerate() {
            let path = format!("{}/{}/{}", root, spec.parent, spec.subdir);
            if fs.is_dir(&path).await.unwrap_or(false) {
                roots.push(RemoteSkillRootEntry {
                    path,
                    slot: spec.slot,
                    source_id: spec.source_id,
                    source_label: spec.source_label,
                    priority,
                });
            }
        }

        let mut skills = Vec::new();
        for entry in roots {
            let mut entries = match fs.read_dir(&entry.path).await {
                Ok(value) => value,
                Err(_) => continue,
            };
            sort_remote_dir_entries(&mut entries);

            for item in entries {
                if !item.is_dir || item.is_symlink {
                    continue;
                }

                let Some(dir_name) = normalize_remote_skill_dir_name(&item.path) else {
                    continue;
                };
                let skill_md_path = format!("{}/SKILL.md", item.path.trim_end_matches('/'));
                if !fs.is_file(&skill_md_path).await.unwrap_or(false) {
                    continue;
                }

                match fs.read_file_text(&skill_md_path).await {
                    Ok(content) => match SkillData::from_markdown(
                        item.path.clone(),
                        &content,
                        SkillLocation::Project,
                        false,
                    ) {
                        Ok(mut skill_data) => {
                            skill_data.dir_name = dir_name;
                            skills.push(SkillCandidate::from_data(
                                skill_data,
                                entry.slot,
                                entry.source_id,
                                entry.source_label,
                                PROJECT_SKILL_KEY_PREFIX,
                                entry.priority,
                                false,
                            ));
                        }
                        Err(error) => {
                            error!("Failed to parse SKILL.md in {}: {}", item.path, error);
                        }
                    },
                    Err(error) => {
                        debug!("Failed to read {}: {}", skill_md_path, error);
                    }
                }
            }
        }

        skills
    }

    async fn scan_skill_candidates_for_remote_workspace(
        &self,
        fs: &dyn WorkspaceFileSystem,
        remote_root: &str,
    ) -> Vec<SkillCandidate> {
        let mut skills = self.scan_skill_candidates_for_workspace(None).await;
        skills.extend(Self::scan_remote_project_skills(fs, remote_root).await);
        skills
    }

    async fn apply_mode_filters_for_workspace(
        &self,
        candidates: Vec<SkillCandidate>,
        workspace_root: Option<&Path>,
        agent_type: Option<&str>,
    ) -> Vec<SkillCandidate> {
        let Some(mode_id) = agent_type.map(str::trim).filter(|value| !value.is_empty()) else {
            return candidates;
        };

        let user_overrides = load_user_mode_skill_overrides(mode_id)
            .await
            .unwrap_or_else(|_| UserModeSkillOverrides::default());
        let disabled_project = match workspace_root {
            Some(root) => load_disabled_mode_skills_local(root, mode_id)
                .await
                .unwrap_or_default(),
            None => Vec::new(),
        };

        let disabled_project: HashSet<String> =
            normalize_skill_keys(disabled_project).into_iter().collect();

        filter_candidates_for_mode(candidates, mode_id, &user_overrides, &disabled_project)
    }

    async fn apply_mode_filters_for_remote_workspace(
        &self,
        candidates: Vec<SkillCandidate>,
        fs: &dyn WorkspaceFileSystem,
        remote_root: &str,
        agent_type: Option<&str>,
    ) -> Vec<SkillCandidate> {
        let Some(mode_id) = agent_type.map(str::trim).filter(|value| !value.is_empty()) else {
            return candidates;
        };

        let user_overrides = load_user_mode_skill_overrides(mode_id)
            .await
            .unwrap_or_else(|_| UserModeSkillOverrides::default());
        let disabled_project = load_disabled_mode_skills_remote(fs, remote_root, mode_id)
            .await
            .unwrap_or_default();

        let disabled_project: HashSet<String> =
            normalize_skill_keys(disabled_project).into_iter().collect();

        filter_candidates_for_mode(candidates, mode_id, &user_overrides, &disabled_project)
    }

    fn find_default_hidden_builtin_for_explicit_invocation(
        skill_name: &str,
        candidates: Vec<SkillCandidate>,
        agent_type: Option<&str>,
    ) -> BitFunResult<SkillInfo> {
        match resolve_default_hidden_builtin_for_explicit_invocation(
            skill_name, candidates, agent_type,
        ) {
            ExplicitSkillInvocationResolution::Found(info) => Ok(info),
            ExplicitSkillInvocationResolution::NotFound => Err(BitFunError::tool(format!(
                "Skill '{}' not found",
                skill_name
            ))),
            ExplicitSkillInvocationResolution::DisabledForMode { mode_id } => {
                Err(BitFunError::tool(format!(
                    "Skill '{}' is disabled for mode '{}'. Enable it in mode skill settings or switch to a mode where it is enabled.",
                    skill_name, mode_id
                )))
            }
        }
    }

    async fn find_skill_info_for_explicit_invocation_workspace(
        &self,
        skill_name: &str,
        workspace_root: Option<&Path>,
        agent_type: Option<&str>,
    ) -> BitFunResult<SkillInfo> {
        let candidates = self
            .scan_skill_candidates_for_workspace(workspace_root)
            .await;
        let filtered = self
            .apply_mode_filters_for_workspace(candidates.clone(), workspace_root, agent_type)
            .await;
        if let Some(info) = resolve_visible_skills(filtered)
            .into_iter()
            .find(|skill| skill.name == skill_name)
        {
            return Ok(info);
        }

        Self::find_default_hidden_builtin_for_explicit_invocation(
            skill_name, candidates, agent_type,
        )
    }

    async fn find_skill_info_for_explicit_invocation_remote_workspace(
        &self,
        skill_name: &str,
        fs: &dyn WorkspaceFileSystem,
        remote_root: &str,
        agent_type: Option<&str>,
    ) -> BitFunResult<SkillInfo> {
        let candidates = self
            .scan_skill_candidates_for_remote_workspace(fs, remote_root)
            .await;
        let filtered = self
            .apply_mode_filters_for_remote_workspace(
                candidates.clone(),
                fs,
                remote_root,
                agent_type,
            )
            .await;
        if let Some(info) = resolve_visible_skills(filtered)
            .into_iter()
            .find(|skill| skill.name == skill_name)
        {
            return Ok(info);
        }

        Self::find_default_hidden_builtin_for_explicit_invocation(
            skill_name, candidates, agent_type,
        )
    }

    async fn ensure_loaded(&self) {
        let cache = self.cache.read().await;
        if cache.is_empty() {
            drop(cache);
            self.refresh().await;
        }
    }

    pub async fn refresh(&self) {
        let skills = sort_skills(annotate_shadowed_skills(
            self.scan_skill_candidates_for_workspace(None).await,
        ));
        let mut cache = self.cache.write().await;
        *cache = skills;
    }

    pub async fn refresh_for_workspace(&self, _workspace_root: Option<&Path>) {
        self.refresh().await;
    }

    pub async fn get_all_skills(&self) -> Vec<SkillInfo> {
        self.ensure_loaded().await;
        let cache = self.cache.read().await;
        cache.clone()
    }

    pub async fn get_all_skills_for_workspace(
        &self,
        workspace_root: Option<&Path>,
    ) -> Vec<SkillInfo> {
        sort_skills(annotate_shadowed_skills(
            self.scan_skill_candidates_for_workspace(workspace_root)
                .await,
        ))
    }

    pub async fn get_all_skills_for_remote_workspace(
        &self,
        fs: &dyn WorkspaceFileSystem,
        remote_root: &str,
    ) -> Vec<SkillInfo> {
        sort_skills(annotate_shadowed_skills(
            self.scan_skill_candidates_for_remote_workspace(fs, remote_root)
                .await,
        ))
    }

    pub async fn get_resolved_skills_for_workspace(
        &self,
        workspace_root: Option<&Path>,
        agent_type: Option<&str>,
    ) -> Vec<SkillInfo> {
        let candidates = self
            .scan_skill_candidates_for_workspace(workspace_root)
            .await;
        let filtered = self
            .apply_mode_filters_for_workspace(candidates, workspace_root, agent_type)
            .await;
        sort_skills(resolve_visible_skills(filtered))
    }

    pub async fn get_resolved_skills_for_remote_workspace(
        &self,
        fs: &dyn WorkspaceFileSystem,
        remote_root: &str,
        agent_type: Option<&str>,
    ) -> Vec<SkillInfo> {
        let candidates = self
            .scan_skill_candidates_for_remote_workspace(fs, remote_root)
            .await;
        let filtered = self
            .apply_mode_filters_for_remote_workspace(candidates, fs, remote_root, agent_type)
            .await;
        sort_skills(resolve_visible_skills(filtered))
    }

    pub async fn get_mode_skill_infos_for_workspace(
        &self,
        workspace_root: Option<&Path>,
        mode_id: &str,
    ) -> Vec<ModeSkillInfo> {
        let candidates = self
            .scan_skill_candidates_for_workspace(workspace_root)
            .await;
        let all_skills = sort_skills(annotate_shadowed_skills(candidates.clone()));
        let user_overrides = load_user_mode_skill_overrides(mode_id)
            .await
            .unwrap_or_else(|_| UserModeSkillOverrides::default());
        let disabled_project = match workspace_root {
            Some(root) => load_disabled_mode_skills_local(root, mode_id)
                .await
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let disabled_project: HashSet<String> =
            normalize_skill_keys(disabled_project).into_iter().collect();
        let filtered =
            filter_candidates_for_mode(candidates, mode_id, &user_overrides, &disabled_project);
        let resolved = resolve_visible_skills(filtered);

        build_mode_skill_infos(
            all_skills,
            resolved,
            mode_id,
            &user_overrides,
            &disabled_project,
        )
    }

    pub async fn get_mode_skill_infos_for_remote_workspace(
        &self,
        fs: &dyn WorkspaceFileSystem,
        remote_root: &str,
        mode_id: &str,
    ) -> Vec<ModeSkillInfo> {
        let candidates = self
            .scan_skill_candidates_for_remote_workspace(fs, remote_root)
            .await;
        let all_skills = sort_skills(annotate_shadowed_skills(candidates.clone()));
        let user_overrides = load_user_mode_skill_overrides(mode_id)
            .await
            .unwrap_or_else(|_| UserModeSkillOverrides::default());
        let disabled_project = load_disabled_mode_skills_remote(fs, remote_root, mode_id)
            .await
            .unwrap_or_default();
        let disabled_project: HashSet<String> =
            normalize_skill_keys(disabled_project).into_iter().collect();
        let filtered =
            filter_candidates_for_mode(candidates, mode_id, &user_overrides, &disabled_project);
        let resolved = resolve_visible_skills(filtered);

        build_mode_skill_infos(
            all_skills,
            resolved,
            mode_id,
            &user_overrides,
            &disabled_project,
        )
    }

    pub async fn find_skill_by_key_for_workspace(
        &self,
        skill_key: &str,
        workspace_root: Option<&Path>,
    ) -> Option<SkillInfo> {
        self.get_all_skills_for_workspace(workspace_root)
            .await
            .into_iter()
            .find(|skill| skill.key == skill_key)
    }

    pub async fn find_skill_by_key_for_remote_workspace(
        &self,
        fs: &dyn WorkspaceFileSystem,
        remote_root: &str,
        skill_key: &str,
    ) -> Option<SkillInfo> {
        self.get_all_skills_for_remote_workspace(fs, remote_root)
            .await
            .into_iter()
            .find(|skill| skill.key == skill_key)
    }

    pub async fn find_and_load_skill_for_workspace(
        &self,
        skill_name: &str,
        workspace_root: Option<&Path>,
        agent_type: Option<&str>,
    ) -> BitFunResult<SkillData> {
        let info = self
            .find_skill_info_for_explicit_invocation_workspace(
                skill_name,
                workspace_root,
                agent_type,
            )
            .await?;

        let skill_md_path = PathBuf::from(&info.path).join("SKILL.md");
        let content = fs::read_to_string(&skill_md_path)
            .await
            .map_err(|error| BitFunError::tool(format!("Failed to read skill file: {}", error)))?;

        let mut data = SkillData::from_markdown(info.path.clone(), &content, info.level, true)
            .map_err(|error| BitFunError::tool(error.to_string()))?;
        data.key = info.key;
        data.source_slot = info.source_slot;
        data.dir_name = info.dir_name;
        Ok(data)
    }

    pub async fn find_and_load_skill_by_key_for_workspace(
        &self,
        skill_key: &str,
        workspace_root: Option<&Path>,
        agent_type: Option<&str>,
    ) -> BitFunResult<SkillData> {
        let candidates = self
            .scan_skill_candidates_for_workspace(workspace_root)
            .await;
        let filtered = self
            .apply_mode_filters_for_workspace(candidates, workspace_root, agent_type)
            .await;
        let info = filtered
            .into_iter()
            .map(|candidate| candidate.info)
            .find(|skill| skill.key == skill_key)
            .ok_or_else(|| {
                BitFunError::tool(format!(
                    "Skill key '{}' was not found or is disabled for this mode",
                    skill_key
                ))
            })?;

        let skill_md_path = PathBuf::from(&info.path).join("SKILL.md");
        let content = fs::read_to_string(&skill_md_path)
            .await
            .map_err(|error| BitFunError::tool(format!("Failed to read skill file: {}", error)))?;

        let mut data = SkillData::from_markdown(info.path.clone(), &content, info.level, true)
            .map_err(|error| BitFunError::tool(error.to_string()))?;
        data.key = info.key;
        data.source_slot = info.source_slot;
        data.dir_name = info.dir_name;
        Ok(data)
    }

    pub async fn find_and_load_skill_for_remote_workspace(
        &self,
        skill_name: &str,
        fs: &dyn WorkspaceFileSystem,
        remote_root: &str,
        agent_type: Option<&str>,
    ) -> BitFunResult<SkillData> {
        let info = self
            .find_skill_info_for_explicit_invocation_remote_workspace(
                skill_name,
                fs,
                remote_root,
                agent_type,
            )
            .await?;

        let content = Self::read_skill_md_for_remote_merge(&info, fs).await?;
        let mut data = SkillData::from_markdown(info.path.clone(), &content, info.level, true)
            .map_err(|error| BitFunError::tool(error.to_string()))?;
        data.key = info.key;
        data.source_slot = info.source_slot;
        data.dir_name = info.dir_name;
        Ok(data)
    }

    pub async fn find_and_load_skill_by_key_for_remote_workspace(
        &self,
        skill_key: &str,
        fs: &dyn WorkspaceFileSystem,
        remote_root: &str,
        agent_type: Option<&str>,
    ) -> BitFunResult<SkillData> {
        let candidates = self
            .scan_skill_candidates_for_remote_workspace(fs, remote_root)
            .await;
        let filtered = self
            .apply_mode_filters_for_remote_workspace(candidates, fs, remote_root, agent_type)
            .await;
        let info = filtered
            .into_iter()
            .map(|candidate| candidate.info)
            .find(|skill| skill.key == skill_key)
            .ok_or_else(|| {
                BitFunError::tool(format!(
                    "Skill key '{}' was not found or is disabled for this mode",
                    skill_key
                ))
            })?;

        let content = Self::read_skill_md_for_remote_merge(&info, fs).await?;
        let mut data = SkillData::from_markdown(info.path.clone(), &content, info.level, true)
            .map_err(|error| BitFunError::tool(error.to_string()))?;
        data.key = info.key;
        data.source_slot = info.source_slot;
        data.dir_name = info.dir_name;
        Ok(data)
    }

    pub async fn get_resolved_skills_xml_for_workspace(
        &self,
        workspace_root: Option<&Path>,
        agent_type: Option<&str>,
    ) -> Vec<String> {
        self.get_resolved_skills_for_workspace(workspace_root, agent_type)
            .await
            .into_iter()
            .map(|skill| skill.to_xml_desc())
            .collect()
    }

    pub async fn get_resolved_skills_xml_for_remote_workspace(
        &self,
        fs: &dyn WorkspaceFileSystem,
        remote_root: &str,
        agent_type: Option<&str>,
    ) -> Vec<String> {
        self.get_resolved_skills_for_remote_workspace(fs, remote_root, agent_type)
            .await
            .into_iter()
            .map(|skill| skill.to_xml_desc())
            .collect()
    }

    async fn read_skill_md_for_remote_merge(
        info: &SkillInfo,
        remote_fs: &dyn WorkspaceFileSystem,
    ) -> BitFunResult<String> {
        match info.level {
            SkillLocation::User => {
                let skill_md_path = PathBuf::from(&info.path).join("SKILL.md");
                fs::read_to_string(&skill_md_path).await.map_err(|error| {
                    BitFunError::tool(format!("Failed to read skill file: {}", error))
                })
            }
            SkillLocation::Project => {
                let skill_md_path = format!("{}/SKILL.md", info.path.trim_end_matches('/'));
                remote_fs
                    .read_file_text(&skill_md_path)
                    .await
                    .map_err(|error| {
                        BitFunError::tool(format!("Failed to read skill file: {}", error))
                    })
            }
        }
    }
}
