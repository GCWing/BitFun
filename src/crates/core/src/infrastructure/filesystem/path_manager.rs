//! Unified path management module
//!
//! Provides unified management for all app storage paths, supporting user, project, and temporary levels

use crate::util::errors::*;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Storage level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorageLevel {
    /// User: global configuration and data
    User,
    /// Project: configuration for a specific project
    Project,
    /// Session: temporary data for the current session
    Session,
    /// Temporary: cache that can be cleaned
    Temporary,
}

/// Cache type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheType {
    /// AI model cache
    Models,
    /// Vector embedding cache
    Embeddings,
    /// Git repository metadata cache
    Git,
    /// Code index cache
    Index,
}

/// Path manager
///
/// Manages all app storage paths consistently across platforms
#[derive(Debug, Clone)]
pub struct PathManager {
    /// User config root directory
    user_root: PathBuf,
}

impl PathManager {
    /// Create a new path manager
    pub fn new() -> BitFunResult<Self> {
        let user_root = Self::get_user_config_root()?;

        Ok(Self { user_root })
    }

    /// Get user config root directory
    ///
    /// - Windows: %APPDATA%\BitFun\
    /// - macOS: ~/Library/Application Support/BitFun/
    /// - Linux: ~/.config/bitfun/
    fn get_user_config_root() -> BitFunResult<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| BitFunError::config("Failed to get config directory".to_string()))?;

        Ok(config_dir.join("bitfun"))
    }

    /// Get user config root directory
    pub fn user_root(&self) -> &Path {
        &self.user_root
    }

    /// Get assistant home root directory: ~/.bitfun/
    pub fn bitfun_home_dir(&self) -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| self.user_root.clone())
            .join(".bitfun")
    }

    /// Get the legacy assistant workspace base directory: ~/.bitfun/
    ///
    /// `override_root` is reserved for future user customization.
    pub fn legacy_assistant_workspace_base_dir(&self, override_root: Option<&Path>) -> PathBuf {
        override_root
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.bitfun_home_dir())
    }

    /// Get assistant workspace base directory: ~/.bitfun/personal_assistant/
    ///
    /// `override_root` is reserved for future user customization.
    pub fn assistant_workspace_base_dir(&self, override_root: Option<&Path>) -> PathBuf {
        self.legacy_assistant_workspace_base_dir(override_root)
            .join("personal_assistant")
    }

    /// Get the legacy default assistant workspace directory: ~/.bitfun/workspace
    pub fn legacy_default_assistant_workspace_dir(&self, override_root: Option<&Path>) -> PathBuf {
        self.legacy_assistant_workspace_base_dir(override_root)
            .join("workspace")
    }

    /// Get the default assistant workspace directory: ~/.bitfun/personal_assistant/workspace
    pub fn default_assistant_workspace_dir(&self, override_root: Option<&Path>) -> PathBuf {
        self.assistant_workspace_base_dir(override_root)
            .join("workspace")
    }

    /// Get a legacy named assistant workspace directory: ~/.bitfun/workspace-<id>
    pub fn legacy_assistant_workspace_dir(
        &self,
        assistant_id: &str,
        override_root: Option<&Path>,
    ) -> PathBuf {
        self.legacy_assistant_workspace_base_dir(override_root)
            .join(format!("workspace-{}", assistant_id))
    }

    /// Get a named assistant workspace directory: ~/.bitfun/personal_assistant/workspace-<id>
    pub fn assistant_workspace_dir(
        &self,
        assistant_id: &str,
        override_root: Option<&Path>,
    ) -> PathBuf {
        self.assistant_workspace_base_dir(override_root)
            .join(format!("workspace-{}", assistant_id))
    }

    /// Resolve assistant workspace directory for default or named assistant.
    pub fn resolve_assistant_workspace_dir(
        &self,
        assistant_id: Option<&str>,
        override_root: Option<&Path>,
    ) -> PathBuf {
        match assistant_id {
            Some(id) if !id.trim().is_empty() => self.assistant_workspace_dir(id, override_root),
            _ => self.default_assistant_workspace_dir(override_root),
        }
    }

    /// Get user config directory: ~/.config/bitfun/config/
    pub fn user_config_dir(&self) -> PathBuf {
        self.user_root.join("config")
    }

    /// Get app config file path: ~/.config/bitfun/config/app.json
    pub fn app_config_file(&self) -> PathBuf {
        self.user_config_dir().join("app.json")
    }

    /// Get user agent directory: ~/.config/bitfun/agents/
    pub fn user_agents_dir(&self) -> PathBuf {
        self.user_root.join("agents")
    }

    /// Get agent templates directory: ~/.config/bitfun/agents/templates/
    pub fn agent_templates_dir(&self) -> PathBuf {
        self.user_agents_dir().join("templates")
    }

    /// Get user skills directory:
    /// - Windows: C:\Users\xxx\AppData\Roaming\BitFun\skills\
    /// - macOS: ~/Library/Application Support/BitFun/skills/
    /// - Linux: ~/.local/share/BitFun/skills/
    pub fn user_skills_dir(&self) -> PathBuf {
        if cfg!(target_os = "windows") {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
                .join("BitFun")
                .join("skills")
        } else if cfg!(target_os = "macos") {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("Library")
                .join("Application Support")
                .join("BitFun")
                .join("skills")
        } else {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("BitFun")
                .join("skills")
        }
    }

    /// Get workspaces directory: ~/.config/bitfun/workspaces/
    pub fn workspaces_dir(&self) -> PathBuf {
        self.user_root.join("workspaces")
    }

    /// Get cache root directory: ~/.config/bitfun/cache/
    pub fn cache_root(&self) -> PathBuf {
        self.user_root.join("cache")
    }

    /// Get managed runtimes root directory: ~/.config/bitfun/runtimes/
    ///
    /// BitFun-managed runtime components (e.g. node/python/office) are stored here.
    pub fn managed_runtimes_dir(&self) -> PathBuf {
        self.user_root.join("runtimes")
    }

    /// Get cache directory for a specific type
    pub fn cache_dir(&self, cache_type: CacheType) -> PathBuf {
        let subdir = match cache_type {
            CacheType::Models => "models",
            CacheType::Embeddings => "embeddings",
            CacheType::Git => "git",
            CacheType::Index => "index",
        };
        self.cache_root().join(subdir)
    }

    /// Get user data directory: ~/.config/bitfun/data/
    pub fn user_data_dir(&self) -> PathBuf {
        self.user_root.join("data")
    }

    /// Get scheduled jobs directory: ~/.config/bitfun/data/cron/
    pub fn user_cron_dir(&self) -> PathBuf {
        self.user_data_dir().join("cron")
    }

    /// Get scheduled jobs persistence file: ~/.config/bitfun/data/cron/jobs.json
    pub fn cron_jobs_file(&self) -> PathBuf {
        self.user_cron_dir().join("jobs.json")
    }

    /// Get miniapps root directory: ~/.config/bitfun/data/miniapps/
    pub fn miniapps_dir(&self) -> PathBuf {
        self.user_data_dir().join("miniapps")
    }

    /// Get directory for a specific miniapp: ~/.config/bitfun/data/miniapps/{app_id}/
    pub fn miniapp_dir(&self, app_id: &str) -> PathBuf {
        self.miniapps_dir().join(app_id)
    }

    /// Get user-level rules directory: ~/.config/bitfun/data/rules/
    pub fn user_rules_dir(&self) -> PathBuf {
        self.user_data_dir().join("rules")
    }

    /// Get history directory: ~/.config/bitfun/data/history/
    pub fn history_dir(&self) -> PathBuf {
        self.user_data_dir().join("history")
    }

    /// Get snippets directory: ~/.config/bitfun/data/snippets/
    pub fn snippets_dir(&self) -> PathBuf {
        self.user_data_dir().join("snippets")
    }

    /// Get templates directory: ~/.config/bitfun/data/templates/
    pub fn templates_dir(&self) -> PathBuf {
        self.user_data_dir().join("templates")
    }

    /// Get logs directory: ~/.config/bitfun/logs/
    pub fn logs_dir(&self) -> PathBuf {
        self.user_root.join("logs")
    }

    /// Get backups directory: ~/.config/bitfun/backups/
    pub fn backups_dir(&self) -> PathBuf {
        self.user_root.join("backups")
    }

    /// Get temp directory: ~/.config/bitfun/temp/
    pub fn temp_dir(&self) -> PathBuf {
        self.user_root.join("temp")
    }

    /// Get project config root directory: {project}/.bitfun/
    pub fn project_root(&self, workspace_path: &Path) -> PathBuf {
        workspace_path.join(".bitfun")
    }

    /// Get project config file: {project}/.bitfun/config.json
    pub fn project_config_file(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("config.json")
    }

    /// Get project .gitignore file: {project}/.bitfun/.gitignore
    pub fn project_gitignore_file(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join(".gitignore")
    }

    /// Get project agent directory: {project}/.bitfun/agents/
    pub fn project_agents_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("agents")
    }

    /// Get project-level rules directory: {project}/.bitfun/rules/
    pub fn project_rules_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("rules")
    }

    /// Get project snapshots directory: {project}/.bitfun/snapshots/
    pub fn project_snapshots_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("snapshots")
    }

    /// Get project sessions directory: {project}/.bitfun/sessions/
    pub fn project_sessions_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("sessions")
    }

    /// Get project diffs cache directory: {project}/.bitfun/diffs/
    pub fn project_diffs_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("diffs")
    }

    /// Get project checkpoints directory: {project}/.bitfun/checkpoints/
    pub fn project_checkpoints_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("checkpoints")
    }

    /// Get project context directory: {project}/.bitfun/context/
    pub fn project_context_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("context")
    }

    /// Get project local data directory: {project}/.bitfun/local/
    pub fn project_local_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("local")
    }

    /// Get project local cache directory: {project}/.bitfun/local/cache/
    pub fn project_cache_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_local_dir(workspace_path).join("cache")
    }

    /// Get project local logs directory: {project}/.bitfun/local/logs/
    pub fn project_logs_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_local_dir(workspace_path).join("logs")
    }

    /// Get project local temp directory: {project}/.bitfun/local/temp/
    pub fn project_temp_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_local_dir(workspace_path).join("temp")
    }

    /// Get project tasks directory: {project}/.bitfun/tasks/
    pub fn project_tasks_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("tasks")
    }

    /// Get project plans directory: {project}/.bitfun/plans/
    pub fn project_plans_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("plans")
    }

    /// Compute a hash of the workspace path (used for directory names)
    pub fn workspace_hash(workspace_path: &Path) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        workspace_path.to_string_lossy().hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Ensure directory exists
    pub async fn ensure_dir(&self, path: &Path) -> BitFunResult<()> {
        if !path.exists() {
            tokio::fs::create_dir_all(path).await.map_err(|e| {
                BitFunError::service(format!("Failed to create directory {:?}: {}", path, e))
            })?;
        }
        Ok(())
    }

    /// Initialize user-level directory structure
    pub async fn initialize_user_directories(&self) -> BitFunResult<()> {
        let dirs = vec![
            self.bitfun_home_dir(),
            self.assistant_workspace_base_dir(None),
            self.user_config_dir(),
            self.user_agents_dir(),
            self.agent_templates_dir(),
            self.workspaces_dir(),
            self.cache_root(),
            self.cache_dir(CacheType::Models),
            self.cache_dir(CacheType::Embeddings),
            self.cache_dir(CacheType::Git),
            self.cache_dir(CacheType::Index),
            self.user_data_dir(),
            self.user_cron_dir(),
            self.user_rules_dir(),
            self.history_dir(),
            self.snippets_dir(),
            self.templates_dir(),
            self.miniapps_dir(),
            self.logs_dir(),
            self.backups_dir(),
            self.temp_dir(),
        ];

        for dir in dirs {
            self.ensure_dir(&dir).await?;
        }

        debug!("User-level directories initialized");
        Ok(())
    }

    /// Initialize project-level directory structure
    pub async fn initialize_project_directories(&self, workspace_path: &Path) -> BitFunResult<()> {
        let dirs = vec![
            self.project_root(workspace_path),
            self.project_agents_dir(workspace_path),
            self.project_rules_dir(workspace_path),
            self.project_snapshots_dir(workspace_path),
            self.project_sessions_dir(workspace_path),
            self.project_diffs_dir(workspace_path),
            self.project_checkpoints_dir(workspace_path),
            self.project_context_dir(workspace_path),
            self.project_local_dir(workspace_path),
            self.project_cache_dir(workspace_path),
            self.project_logs_dir(workspace_path),
            self.project_temp_dir(workspace_path),
            self.project_tasks_dir(workspace_path),
        ];

        for dir in dirs {
            self.ensure_dir(&dir).await?;
        }

        self.generate_project_gitignore(workspace_path).await?;

        debug!(
            "Project-level directories initialized for {:?}",
            workspace_path
        );
        Ok(())
    }

    /// Generate project-level .gitignore file
    async fn generate_project_gitignore(&self, workspace_path: &Path) -> BitFunResult<()> {
        let gitignore_path = self.project_gitignore_file(workspace_path);

        if gitignore_path.exists() {
            return Ok(());
        }

        let content = r#"# BitFun local data (auto-generated)

# Snapshots and cache
snapshots/
diffs/
local/

# Personal sessions and checkpoints
sessions/
checkpoints/

# Logs and temporary files
*.log
temp/

# Note: The following files SHOULD be committed to version control
# config.json
# agents/
# context/
# tasks/
"#;

        tokio::fs::write(&gitignore_path, content)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to create .gitignore: {}", e)))?;

        debug!("Generated .gitignore for project");
        Ok(())
    }
}

impl Default for PathManager {
    fn default() -> Self {
        match Self::new() {
            Ok(manager) => manager,
            Err(e) => {
                error!(
                    "Failed to create PathManager from system config directory, using temp fallback: {}",
                    e
                );
                Self {
                    user_root: std::env::temp_dir().join("bitfun"),
                }
            }
        }
    }
}

use std::sync::OnceLock;

/// Global PathManager instance
static GLOBAL_PATH_MANAGER: OnceLock<Arc<PathManager>> = OnceLock::new();

fn init_global_path_manager() -> BitFunResult<Arc<PathManager>> {
    PathManager::new().map(Arc::new)
}

/// Get the global PathManager instance (Arc)
///
/// Return a shared Arc to the global PathManager instance
pub fn get_path_manager_arc() -> Arc<PathManager> {
    GLOBAL_PATH_MANAGER
        .get_or_init(|| match init_global_path_manager() {
            Ok(manager) => manager,
            Err(e) => {
                error!(
                    "Failed to create global PathManager from config directory, using fallback: {}",
                    e
                );
                Arc::new(PathManager::default())
            }
        })
        .clone()
}

/// Try to get the global PathManager instance (Arc)
pub fn try_get_path_manager_arc() -> BitFunResult<Arc<PathManager>> {
    if let Some(manager) = GLOBAL_PATH_MANAGER.get() {
        return Ok(Arc::clone(manager));
    }

    let manager = init_global_path_manager()?;
    match GLOBAL_PATH_MANAGER.set(Arc::clone(&manager)) {
        Ok(()) => Ok(manager),
        Err(_) => Ok(Arc::clone(GLOBAL_PATH_MANAGER.get().expect(
            "GLOBAL_PATH_MANAGER should be initialized after set failure",
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::PathManager;

    #[test]
    fn assistant_workspace_paths_use_personal_assistant_subdir() {
        let path_manager = PathManager::default();
        let base_dir = path_manager.assistant_workspace_base_dir(None);

        assert_eq!(
            base_dir,
            path_manager.bitfun_home_dir().join("personal_assistant")
        );
        assert_eq!(
            path_manager.default_assistant_workspace_dir(None),
            base_dir.join("workspace")
        );
        assert_eq!(
            path_manager.assistant_workspace_dir("demo", None),
            base_dir.join("workspace-demo")
        );
        assert_eq!(
            path_manager.resolve_assistant_workspace_dir(None, None),
            base_dir.join("workspace")
        );
        assert_eq!(
            path_manager.resolve_assistant_workspace_dir(Some("demo"), None),
            base_dir.join("workspace-demo")
        );
    }

    #[test]
    fn legacy_assistant_workspace_paths_remain_at_bitfun_root() {
        let path_manager = PathManager::default();
        let legacy_base_dir = path_manager.legacy_assistant_workspace_base_dir(None);

        assert_eq!(legacy_base_dir, path_manager.bitfun_home_dir());
        assert_eq!(
            path_manager.legacy_default_assistant_workspace_dir(None),
            legacy_base_dir.join("workspace")
        );
        assert_eq!(
            path_manager.legacy_assistant_workspace_dir("demo", None),
            legacy_base_dir.join("workspace-demo")
        );
    }
}
