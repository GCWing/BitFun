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

    /// Get user plugins directory: ~/.config/bitfun/plugins/
    pub fn user_plugins_dir(&self) -> PathBuf {
        self.user_root.join("plugins")
    }

    /// Cowork workspace root directory: ~/.config/bitfun/cowork/workspace/
    ///
    /// This is an app-managed workspace used to enable FlowChat features even when the user
    /// hasn't selected a project folder. Cowork sessions can store their conversation history and
    /// intermediate artifacts under this workspace.
    pub fn cowork_workspace_dir(&self) -> PathBuf {
        self.user_root.join("cowork").join("workspace")
    }
    /// Cowork artifacts directory (user-visible outputs): ~/.config/bitfun/cowork/workspace/artifacts/
    pub fn cowork_artifacts_dir(&self) -> PathBuf {
        self.cowork_workspace_dir().join("artifacts")
    }

    /// Cowork tmp directory (intermediate scratch): ~/.config/bitfun/cowork/workspace/tmp/
    pub fn cowork_tmp_dir(&self) -> PathBuf {
        self.cowork_workspace_dir().join("tmp")
    }

    /// Cowork per-session artifacts directory (user-visible outputs).
    pub fn cowork_session_artifacts_dir(&self, session_id: &str) -> PathBuf {
        self.cowork_artifacts_dir()
            .join(format!("session-{}", session_id))
    }

    /// Cowork per-session tmp directory (intermediate scratch).
    pub fn cowork_session_tmp_dir(&self, session_id: &str) -> PathBuf {
        self.cowork_tmp_dir().join(format!("session-{}", session_id))
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

    /// Get project plugins directory: {project}/.bitfun/plugins/
    pub fn project_plugins_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("plugins")
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
            self.user_rules_dir(),
            self.history_dir(),
            self.snippets_dir(),
            self.templates_dir(),
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

use once_cell::sync::OnceCell;

/// Global PathManager instance
static GLOBAL_PATH_MANAGER: OnceCell<Arc<PathManager>> = OnceCell::new();

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
    GLOBAL_PATH_MANAGER
        .get_or_try_init(init_global_path_manager)
        .map(Arc::clone)
}
