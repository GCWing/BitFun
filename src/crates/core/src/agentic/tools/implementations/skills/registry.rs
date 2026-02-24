//! Skill registry
//!
//! Manages Skill loading and enabled/disabled filtering
//! Supports multiple application paths:
//! .bitfun/skills, .claude/skills, .cursor/skills, .codex/skills, .agents/skills

use super::builtin::ensure_builtin_skills_installed;
use super::types::{SkillData, SkillInfo, SkillLocation};
use crate::infrastructure::{get_path_manager_arc, get_workspace_path};
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, error};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::fs;
use tokio::sync::RwLock;

/// Global Skill registry instance
static SKILL_REGISTRY: OnceLock<SkillRegistry> = OnceLock::new();

/// Project-level Skill directory names (relative to workspace root)
const PROJECT_SKILL_SUBDIRS: &[(&str, &str)] = &[
    (".bitfun", "skills"),
    (".claude", "skills"),
    (".cursor", "skills"),
    (".codex", "skills"),
    (".agents", "skills"),
];

/// Home-directory based user-level Skill paths.
const USER_HOME_SKILL_SUBDIRS: &[(&str, &str)] = &[
    (".claude", "skills"),
    (".cursor", "skills"),
    (".codex", "skills"),
];

/// Skill directory entry
#[derive(Debug, Clone)]
pub struct SkillDirEntry {
    pub path: PathBuf,
    pub level: SkillLocation,
}

/// Skill registry
///
/// Caches scanned skill information to avoid repeated directory scanning
pub struct SkillRegistry {
    /// Cached skill data, key is skill name
    cache: RwLock<HashMap<String, SkillInfo>>,
}

impl SkillRegistry {
    /// Create new registry instance
    fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get global instance
    pub fn global() -> &'static Self {
        SKILL_REGISTRY.get_or_init(Self::new)
    }

    /// Get all possible Skill directory paths
    ///
    /// Returns existing directories and their levels (project/user)
    /// - Project-level: .bitfun/skills, .claude/skills, .cursor/skills, .codex/skills, .agents/skills under workspace
    /// - User-level: skills under bitfun user config, ~/.claude/skills, ~/.cursor/skills, ~/.codex/skills, ~/.config/agents/skills
    pub fn get_possible_paths() -> Vec<SkillDirEntry> {
        let mut entries = Vec::new();

        // Project-level Skill paths
        if let Some(workspace_path) = get_workspace_path() {
            for (parent, sub) in PROJECT_SKILL_SUBDIRS {
                let p = workspace_path.join(parent).join(sub);
                if p.exists() && p.is_dir() {
                    entries.push(SkillDirEntry {
                        path: p,
                        level: SkillLocation::Project,
                    });
                }
            }
        }

        // User-level: skills under bitfun user config
        let pm = get_path_manager_arc();
        let bitfun_skills = pm.user_skills_dir();
        if bitfun_skills.exists() && bitfun_skills.is_dir() {
            entries.push(SkillDirEntry {
                path: bitfun_skills,
                level: SkillLocation::User,
            });
        }

        // User-level: ~/.claude/skills, ~/.cursor/skills, ~/.codex/skills
        if let Some(home) = dirs::home_dir() {
            for (parent, sub) in USER_HOME_SKILL_SUBDIRS {
                let p = home.join(parent).join(sub);
                if p.exists() && p.is_dir() {
                    entries.push(SkillDirEntry {
                        path: p,
                        level: SkillLocation::User,
                    });
                }
            }
        }

        // User-level: ~/.config/agents/skills (used by universal agent installs in skills CLI)
        if let Some(config_dir) = dirs::config_dir() {
            let p = config_dir.join("agents").join("skills");
            if p.exists() && p.is_dir() {
                entries.push(SkillDirEntry {
                    path: p,
                    level: SkillLocation::User,
                });
            }
        }

        entries
    }

    /// Scan directory to get all skill information
    /// enabled status is read from SKILL.md file
    async fn scan_skills_in_dir(dir: &Path, level: SkillLocation) -> Vec<SkillInfo> {
        let mut skills = Vec::new();

        if !dir.exists() {
            return skills;
        }

        if let Ok(mut entries) = fs::read_dir(dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    let skill_md_path = path.join("SKILL.md");
                    if skill_md_path.exists() {
                        if let Ok(content) = fs::read_to_string(&skill_md_path).await {
                            match SkillData::from_markdown(
                                path.to_string_lossy().to_string(),
                                &content,
                                level,
                                false,
                            ) {
                                Ok(skill_data) => {
                                    let info = SkillInfo {
                                        name: skill_data.name,
                                        description: skill_data.description,
                                        path: path.to_string_lossy().to_string(),
                                        level,
                                        enabled: skill_data.enabled,
                                    };
                                    skills.push(info);
                                }
                                Err(e) => {
                                    error!("Failed to parse SKILL.md in {}: {}", path.display(), e);
                                }
                            }
                        }
                    }
                }
            }
        }

        skills
    }

    /// Refresh cache, rescan all directories
    pub async fn refresh(&self) {
        if let Err(e) = ensure_builtin_skills_installed().await {
            debug!("Failed to install built-in skills: {}", e);
        }

        let mut by_name: HashMap<String, SkillInfo> = HashMap::new();

        for entry in Self::get_possible_paths() {
            let skills = Self::scan_skills_in_dir(&entry.path, entry.level).await;
            for info in skills {
                // Only keep the first skill with the same name (higher priority)
                by_name.entry(info.name.clone()).or_insert(info);
            }
        }

        let mut cache = self.cache.write().await;
        *cache = by_name;
        debug!("SkillRegistry refreshed, {} skills loaded", cache.len());
    }

    /// Ensure cache is initialized
    async fn ensure_loaded(&self) {
        let cache = self.cache.read().await;
        if cache.is_empty() {
            drop(cache);
            self.refresh().await;
        }
    }

    /// Get all skill information (including enabled status)
    ///
    /// Skills with the same name are prioritized by path order: earlier paths have higher priority, later paths won't override already loaded skills with the same name
    pub async fn get_all_skills(&self) -> Vec<SkillInfo> {
        self.ensure_loaded().await;
        let cache = self.cache.read().await;
        cache.values().cloned().collect()
    }

    /// Get all enabled skills (for tool description)
    pub async fn get_enabled_skills(&self) -> Vec<SkillInfo> {
        self.get_all_skills()
            .await
            .into_iter()
            .filter(|s| s.enabled)
            .collect()
    }

    /// Get XML description list of enabled skills
    pub async fn get_enabled_skills_xml(&self) -> Vec<String> {
        self.get_enabled_skills()
            .await
            .into_iter()
            .map(|s| s.to_xml_desc())
            .collect()
    }

    /// Find skill information by name
    pub async fn find_skill(&self, skill_name: &str) -> Option<SkillInfo> {
        self.ensure_loaded().await;
        {
            let cache = self.cache.read().await;
            if let Some(info) = cache.get(skill_name) {
                return Some(info.clone());
            }
        }

        // Skill may have been installed externally (e.g. via `npx skills add`) after cache init.
        self.refresh().await;
        let cache = self.cache.read().await;
        cache.get(skill_name).cloned()
    }

    /// Find SKILL.md path by name
    pub async fn find_skill_path(&self, skill_name: &str) -> Option<PathBuf> {
        self.find_skill(skill_name)
            .await
            .map(|info| PathBuf::from(&info.path).join("SKILL.md"))
    }

    /// Update skill enabled status in cache
    pub async fn update_skill_enabled(&self, skill_name: &str, enabled: bool) {
        let mut cache = self.cache.write().await;
        if let Some(info) = cache.get_mut(skill_name) {
            info.enabled = enabled;
        }
    }

    /// Remove skill from cache
    pub async fn remove_skill(&self, skill_name: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(skill_name);
    }

    /// Find and load skill (for execution)
    /// Only load enabled skills
    pub async fn find_and_load_skill(&self, skill_name: &str) -> BitFunResult<SkillData> {
        // First search in cache
        let skill_info = self.find_skill(skill_name).await;

        if let Some(info) = skill_info {
            // Check if enabled
            if !info.enabled {
                return Err(BitFunError::tool(format!(
                    "Skill '{}' is disabled",
                    skill_name
                )));
            }

            // Load full content from file
            let skill_md_path = PathBuf::from(&info.path).join("SKILL.md");
            let content = fs::read_to_string(&skill_md_path)
                .await
                .map_err(|e| BitFunError::tool(format!("Failed to read skill file: {}", e)))?;

            let skill_data =
                SkillData::from_markdown(info.path.clone(), &content, info.level, true)?;

            debug!(
                "SkillRegistry loaded skill '{}' from {}",
                skill_name, info.path
            );
            return Ok(skill_data);
        }

        // Skill not found
        Err(BitFunError::tool(format!(
            "Skill '{}' not found",
            skill_name
        )))
    }
}
