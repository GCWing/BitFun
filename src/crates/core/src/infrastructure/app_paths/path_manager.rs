//! Unified path management module
//!
//! Provides unified management for all app storage paths, supporting user, project, and temporary levels

use crate::util::errors::*;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const MAX_PROJECT_SLUG_LEN: usize = 120;

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

/// Path manager
///
/// Manages all app storage paths consistently across platforms
#[derive(Debug, Clone)]
pub struct PathManager {
    /// User config root directory
    user_root: PathBuf,
    /// Optional override for the BitFun home directory, used by tests to avoid
    /// touching the real user home.
    bitfun_home_override: Option<PathBuf>,
    /// Cache of runtime slugs keyed by the original and canonical workspace paths.
    project_runtime_slug_cache: Arc<Mutex<HashMap<PathBuf, String>>>,
}

impl PathManager {
    /// Create a new path manager
    pub fn new() -> BitFunResult<Self> {
        let user_root = Self::get_user_config_root()?;

        Ok(Self {
            user_root,
            bitfun_home_override: None,
            project_runtime_slug_cache: Arc::new(Mutex::new(HashMap::new())),
        })
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

    /// Get assistant home root directory: ~/.bitfun/
    pub fn bitfun_home_dir(&self) -> PathBuf {
        if let Some(path) = &self.bitfun_home_override {
            return path.clone();
        }
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

    /// True if `path` is this machine's BitFun **assistant** workspace directory.
    ///
    /// Used so remote-workspace registry (especially roots like `/`) does not
    /// mis-classify client paths such as `/Users/.../.bitfun/personal_assistant/workspace-*`
    /// as SSH remote paths.
    pub fn is_local_assistant_workspace_path(&self, path: &str) -> bool {
        let p = Path::new(path);
        if !p.is_absolute() {
            return false;
        }
        if p.starts_with(self.assistant_workspace_base_dir(None)) {
            return true;
        }
        if p.starts_with(self.default_assistant_workspace_dir(None)) {
            return true;
        }
        if p.starts_with(self.legacy_default_assistant_workspace_dir(None)) {
            return true;
        }
        let legacy_base = self.legacy_assistant_workspace_base_dir(None);
        if let Ok(rest) = p.strip_prefix(&legacy_base) {
            if let Some(std::path::Component::Normal(first)) = rest.components().next() {
                let name = first.to_string_lossy();
                if name == "workspace" || name.starts_with("workspace-") {
                    return true;
                }
            }
        }
        false
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

    /// Get BitFun-managed built-in skills directory under the user skills root.
    pub fn builtin_skills_dir(&self) -> PathBuf {
        self.user_skills_dir().join(".system")
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

    /// Get user data directory: ~/.config/bitfun/data/
    pub fn user_data_dir(&self) -> PathBuf {
        self.user_root.join("data")
    }

    /// Root for per-host, per-remote-path workspace mirrors: `~/.bitfun/remote_ssh/`.
    ///
    /// Session/chat persistence for SSH workspaces lives under
    /// `{this}/{sanitized_host}/{remote_path_segments}/sessions/`.
    pub fn remote_ssh_mirror_root() -> PathBuf {
        Self::new()
            .map(|pm| pm.bitfun_home_dir().join("remote_ssh"))
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".bitfun")
                    .join("remote_ssh")
            })
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

    /// Get logs directory: ~/.config/bitfun/logs/
    pub fn logs_dir(&self) -> PathBuf {
        self.user_root.join("logs")
    }

    /// Get temp directory: ~/.config/bitfun/temp/
    pub fn temp_dir(&self) -> PathBuf {
        self.user_root.join("temp")
    }

    /// Get project config root directory: {project}/.bitfun/
    pub fn project_root(&self, workspace_path: &Path) -> PathBuf {
        workspace_path.join(".bitfun")
    }

    /// Get the shared runtime projects root directory: ~/.bitfun/projects/
    pub fn projects_root(&self) -> PathBuf {
        self.bitfun_home_dir().join("projects")
    }

    /// Get the runtime root for a workspace: ~/.bitfun/projects/<workspace-slug>/
    pub fn project_runtime_root(&self, workspace_path: &Path) -> PathBuf {
        self.projects_root()
            .join(self.project_runtime_slug(workspace_path))
    }

    /// Get project internal config directory: {project}/.bitfun/config/
    pub fn project_internal_config_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("config")
    }

    /// Get project mode skills file: {project}/.bitfun/config/mode_skills.json
    pub fn project_mode_skills_file(&self, workspace_path: &Path) -> PathBuf {
        self.project_internal_config_dir(workspace_path)
            .join("mode_skills.json")
    }

    /// Get project agent directory: {project}/.bitfun/agents/
    pub fn project_agents_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("agents")
    }

    /// Get project-level rules directory: {project}/.bitfun/rules/
    pub fn project_rules_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_root(workspace_path).join("rules")
    }

    /// Get project snapshots directory: ~/.bitfun/projects/<workspace-slug>/snapshots/
    pub fn project_snapshots_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_runtime_root(workspace_path).join("snapshots")
    }

    /// Get project sessions directory: ~/.bitfun/projects/<workspace-slug>/sessions/
    pub fn project_sessions_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_runtime_root(workspace_path).join("sessions")
    }

    /// Get project plans directory: ~/.bitfun/projects/<workspace-slug>/plans/
    pub fn project_plans_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_runtime_root(workspace_path).join("plans")
    }

    /// Get project memory directory: ~/.bitfun/projects/<workspace-slug>/memory/
    pub fn project_memory_dir(&self, workspace_path: &Path) -> PathBuf {
        self.project_runtime_root(workspace_path).join("memory")
    }

    fn project_runtime_slug(&self, workspace_path: &Path) -> String {
        let requested_path = workspace_path.to_path_buf();
        if let Some(slug) = self.cached_project_runtime_slug(&requested_path) {
            return slug;
        }

        let canonical_path =
            dunce::canonicalize(workspace_path).unwrap_or_else(|_| requested_path.clone());
        if canonical_path != requested_path {
            if let Some(slug) = self.cached_project_runtime_slug(&canonical_path) {
                self.store_project_runtime_slug(&requested_path, &slug);
                return slug;
            }
        }

        let canonical = canonical_path.to_string_lossy().to_string();
        let slug = Self::build_project_runtime_slug(&canonical);

        self.store_project_runtime_slug(&canonical_path, &slug);
        if canonical_path != requested_path {
            self.store_project_runtime_slug(&requested_path, &slug);
        }

        slug
    }

    fn cached_project_runtime_slug(&self, workspace_path: &Path) -> Option<String> {
        self.project_runtime_slug_cache
            .lock()
            .expect("project runtime slug cache poisoned")
            .get(workspace_path)
            .cloned()
    }

    fn store_project_runtime_slug(&self, workspace_path: &Path, slug: &str) {
        self.project_runtime_slug_cache
            .lock()
            .expect("project runtime slug cache poisoned")
            .insert(workspace_path.to_path_buf(), slug.to_string());
    }

    fn build_project_runtime_slug(canonical: &str) -> String {
        let slug: String = canonical
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();

        let slug = slug.trim_matches('-');
        let slug = if slug.is_empty() { "workspace" } else { slug };

        if slug.len() <= MAX_PROJECT_SLUG_LEN {
            return slug.to_string();
        }

        let hash = hex::encode(Sha256::digest(canonical.as_bytes()));
        let suffix = &hash[..12];
        let max_prefix_len = MAX_PROJECT_SLUG_LEN.saturating_sub(suffix.len() + 1);
        let prefix = slug[..max_prefix_len].trim_end_matches('-');
        format!("{}-{}", prefix, suffix)
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
            self.projects_root(),
            self.assistant_workspace_base_dir(None),
            self.user_config_dir(),
            self.user_agents_dir(),
            self.cache_root(),
            self.user_data_dir(),
            self.user_cron_dir(),
            self.user_rules_dir(),
            self.miniapps_dir(),
            self.logs_dir(),
            self.temp_dir(),
        ];

        for dir in dirs {
            self.ensure_dir(&dir).await?;
        }

        debug!("User-level directories initialized");
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
                    bitfun_home_override: None,
                    project_runtime_slug_cache: Arc::new(Mutex::new(HashMap::new())),
                }
            }
        }
    }
}

#[cfg(test)]
impl PathManager {
    pub(crate) fn with_user_root_for_tests(user_root: PathBuf) -> Self {
        let base = user_root
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| user_root.clone());
        Self {
            user_root,
            bitfun_home_override: Some(base.join("home").join(".bitfun")),
            project_runtime_slug_cache: Arc::new(Mutex::new(HashMap::new())),
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
    use std::path::Path;

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

    #[test]
    fn is_local_assistant_workspace_path_detects_personal_assistant_and_legacy() {
        let pm = PathManager::default();
        let base = pm.assistant_workspace_base_dir(None);
        let named = pm.assistant_workspace_dir("abc", None);
        assert!(pm.is_local_assistant_workspace_path(&named.to_string_lossy()));
        assert!(pm.is_local_assistant_workspace_path(&base.join("workspace").to_string_lossy()));
        let legacy = pm.legacy_assistant_workspace_dir("xyz", None);
        assert!(pm.is_local_assistant_workspace_path(&legacy.to_string_lossy()));
        assert!(!pm.is_local_assistant_workspace_path("/tmp/not-bitfun"));
    }

    #[test]
    fn project_runtime_root_uses_human_readable_workspace_slug() {
        let pm = PathManager::default();
        let runtime_root = pm.project_runtime_root(Path::new(r"E:\Projects\OpenBitFun\BitFun"));
        let slug = runtime_root
            .file_name()
            .and_then(|value| value.to_str())
            .expect("runtime root should have terminal component");

        assert!(slug.starts_with("e--projects-openbitfun-bitfun"));
        assert_eq!(runtime_root.parent(), Some(pm.projects_root().as_path()));
    }
}
