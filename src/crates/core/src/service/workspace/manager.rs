//! Workspace manager.

use crate::service::git::GitService;
use crate::util::{errors::*, FrontMatterMarkdown};
use log::warn;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Workspace type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WorkspaceType {
    RustProject,
    NodeProject,
    PythonProject,
    JavaProject,
    CppProject,
    WebProject,
    MobileProject,
    Other,
}

/// Workspace status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkspaceStatus {
    Active,
    Inactive,
    Loading,
    Error,
    Archived,
}

/// Workspace lifecycle kind.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceKind {
    #[default]
    Normal,
    Assistant,
    Remote,
}

pub(crate) const IDENTITY_FILE_NAME: &str = "IDENTITY.md";

/// Parsed agent identity fields from `IDENTITY.md` frontmatter.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIdentity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vibe: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
}

/// Git worktree metadata attached to a workspace.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWorktreeInfo {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub main_repo_path: String,
    pub is_main: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct WorkspaceIdentityFrontmatter {
    name: Option<String>,
    creature: Option<String>,
    vibe: Option<String>,
    emoji: Option<String>,
}

impl WorkspaceIdentity {
    pub(crate) async fn load_from_workspace_root(
        workspace_root: &Path,
    ) -> Result<Option<Self>, String> {
        let identity_path = workspace_root.join(IDENTITY_FILE_NAME);
        if !identity_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&identity_path).await.map_err(|e| {
            format!(
                "Failed to read identity file '{}': {}",
                identity_path.display(),
                e
            )
        })?;

        let identity = Self::from_markdown(&content)?;
        if identity.is_empty() {
            Ok(None)
        } else {
            Ok(Some(identity))
        }
    }

    fn from_markdown(content: &str) -> Result<Self, String> {
        let (metadata, _) = FrontMatterMarkdown::load_str(content)?;
        let frontmatter: WorkspaceIdentityFrontmatter = serde_yaml::from_value(metadata)
            .map_err(|e| format!("Failed to parse identity frontmatter: {}", e))?;

        Ok(Self {
            name: normalize_identity_field(frontmatter.name),
            creature: normalize_identity_field(frontmatter.creature),
            vibe: normalize_identity_field(frontmatter.vibe),
            emoji: normalize_identity_field(frontmatter.emoji),
        })
    }

    fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.creature.is_none()
            && self.vibe.is_none()
            && self.emoji.is_none()
    }

    pub(crate) fn collect_changed_fields(
        previous: Option<&WorkspaceIdentity>,
        current: Option<&WorkspaceIdentity>,
    ) -> Vec<String> {
        let previous_name = previous.and_then(|identity| identity.name.as_deref());
        let current_name = current.and_then(|identity| identity.name.as_deref());
        let previous_creature = previous.and_then(|identity| identity.creature.as_deref());
        let current_creature = current.and_then(|identity| identity.creature.as_deref());
        let previous_vibe = previous.and_then(|identity| identity.vibe.as_deref());
        let current_vibe = current.and_then(|identity| identity.vibe.as_deref());
        let previous_emoji = previous.and_then(|identity| identity.emoji.as_deref());
        let current_emoji = current.and_then(|identity| identity.emoji.as_deref());

        let mut changed_fields = Vec::new();
        if previous_name != current_name {
            changed_fields.push("name".to_string());
        }
        if previous_creature != current_creature {
            changed_fields.push("creature".to_string());
        }
        if previous_vibe != current_vibe {
            changed_fields.push("vibe".to_string());
        }
        if previous_emoji != current_emoji {
            changed_fields.push("emoji".to_string());
        }

        changed_fields
    }
}

fn normalize_identity_field(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

/// Workspace metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    #[serde(rename = "rootPath")]
    pub root_path: PathBuf,
    #[serde(rename = "workspaceType")]
    pub workspace_type: WorkspaceType,
    #[serde(rename = "workspaceKind", default)]
    pub workspace_kind: WorkspaceKind,
    #[serde(
        rename = "assistantId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub assistant_id: Option<String>,
    pub status: WorkspaceStatus,
    pub languages: Vec<String>,
    #[serde(rename = "openedAt")]
    pub opened_at: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "lastAccessed")]
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub statistics: Option<WorkspaceStatistics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<WorkspaceIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorkspaceWorktreeInfo>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Workspace statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStatistics {
    pub total_files: usize,
    pub total_directories: usize,
    pub total_size_bytes: u64,
    pub file_extensions: HashMap<String, usize>,
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
    pub git_info: Option<GitInfo>,
}

/// Git information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    pub is_git_repo: bool,
    pub current_branch: Option<String>,
    pub remote_url: Option<String>,
    pub has_uncommitted_changes: bool,
    pub total_commits: Option<usize>,
}

/// Options for scanning a workspace.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub include_hidden: bool,
    pub max_depth: Option<usize>,
    pub scan_git_info: bool,
    pub calculate_statistics: bool,
    pub ignore_patterns: Vec<String>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            max_depth: Some(10),
            scan_git_info: true,
            calculate_statistics: false,
            ignore_patterns: vec![
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
                "__pycache__".to_string(),
                "build".to_string(),
                "dist".to_string(),
            ],
        }
    }
}

/// Options for opening a workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceOpenOptions {
    pub scan_options: ScanOptions,
    pub auto_set_current: bool,
    pub add_to_recent: bool,
    pub workspace_kind: WorkspaceKind,
    pub assistant_id: Option<String>,
    pub display_name: Option<String>,
    /// For [`WorkspaceKind::Remote`], must match persisted `metadata["connectionId"]` so two
    /// servers opened at the same path (e.g. `/`) are separate workspace tabs.
    pub remote_connection_id: Option<String>,
}

impl Default for WorkspaceOpenOptions {
    fn default() -> Self {
        Self {
            scan_options: ScanOptions::default(),
            auto_set_current: true,
            add_to_recent: true,
            workspace_kind: WorkspaceKind::Normal,
            assistant_id: None,
            display_name: None,
            remote_connection_id: None,
        }
    }
}

impl WorkspaceInfo {
    /// SSH connection id persisted in [`WorkspaceInfo::metadata`] for remote workspaces.
    pub fn remote_ssh_connection_id(&self) -> Option<&str> {
        self.metadata
            .get("connectionId")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
    }

    /// Creates a new workspace record.
    pub async fn new(root_path: PathBuf, options: WorkspaceOpenOptions) -> BitFunResult<Self> {
        let default_name = root_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();
        let workspace_kind = options.workspace_kind.clone();
        let assistant_id = if workspace_kind == WorkspaceKind::Assistant {
            options.assistant_id.clone()
        } else {
            None
        };

        let now = chrono::Utc::now();
        let id = uuid::Uuid::new_v4().to_string();

        let is_remote = workspace_kind == WorkspaceKind::Remote;

        let mut workspace = Self {
            id,
            name: options.display_name.clone().unwrap_or(default_name),
            root_path: root_path.clone(),
            workspace_type: WorkspaceType::Other,
            workspace_kind,
            assistant_id,
            status: WorkspaceStatus::Loading,
            languages: Vec::new(),
            opened_at: now,
            last_accessed: now,
            description: None,
            tags: Vec::new(),
            statistics: None,
            identity: None,
            worktree: None,
            metadata: HashMap::new(),
        };

        if !is_remote {
            workspace.detect_workspace_type().await;
            workspace.load_identity().await;
            workspace.load_worktree().await;

            if options.scan_options.calculate_statistics {
                workspace.scan_workspace(options.scan_options).await?;
            }
        }

        workspace.status = if options.auto_set_current {
            WorkspaceStatus::Active
        } else {
            WorkspaceStatus::Inactive
        };
        Ok(workspace)
    }

    async fn load_identity(&mut self) {
        let identity = match WorkspaceIdentity::load_from_workspace_root(&self.root_path).await {
            Ok(identity) => identity,
            Err(error) => {
                warn!(
                    "Failed to load workspace identity: path={} error={}",
                    self.root_path.join(IDENTITY_FILE_NAME).display(),
                    error
                );
                self.identity = None;
                return;
            }
        };

        if self.workspace_kind == WorkspaceKind::Assistant {
            if let Some(name) = identity
                .as_ref()
                .and_then(|identity| identity.name.as_ref())
            {
                self.name = name.clone();
            }
        }

        self.identity = identity;
    }

    async fn load_worktree(&mut self) {
        self.worktree = Self::resolve_worktree_info(&self.root_path).await;
    }

    async fn resolve_worktree_info(workspace_root: &Path) -> Option<WorkspaceWorktreeInfo> {
        let normalized_workspace_path = workspace_root.to_string_lossy().replace('\\', "/");
        let worktrees = match GitService::list_worktrees(workspace_root).await {
            Ok(worktrees) => worktrees,
            Err(_) => return None,
        };

        let main_repo_path = worktrees
            .iter()
            .find(|worktree| worktree.is_main)
            .map(|worktree| worktree.path.clone())?;

        worktrees
            .into_iter()
            .find(|worktree| worktree.path == normalized_workspace_path)
            .map(|worktree| WorkspaceWorktreeInfo {
                path: worktree.path,
                branch: worktree.branch,
                main_repo_path: main_repo_path.clone(),
                is_main: worktree.is_main,
            })
    }

    /// Detects the workspace type.
    async fn detect_workspace_type(&mut self) {
        let root = &self.root_path;

        if root.join("Cargo.toml").exists() {
            self.workspace_type = WorkspaceType::RustProject;
            self.languages.push("Rust".to_string());
        } else if root.join("package.json").exists() {
            self.workspace_type = WorkspaceType::NodeProject;
            self.languages.push("JavaScript".to_string());
            self.languages.push("TypeScript".to_string());
        } else if root.join("requirements.txt").exists()
            || root.join("pyproject.toml").exists()
            || root.join("setup.py").exists()
        {
            self.workspace_type = WorkspaceType::PythonProject;
            self.languages.push("Python".to_string());
        } else if root.join("pom.xml").exists() || root.join("build.gradle").exists() {
            self.workspace_type = WorkspaceType::JavaProject;
            self.languages.push("Java".to_string());
        } else if root.join("CMakeLists.txt").exists() || root.join("Makefile").exists() {
            self.workspace_type = WorkspaceType::CppProject;
            self.languages.push("C++".to_string());
        } else if root.join("index.html").exists() || root.join("webpack.config.js").exists() {
            self.workspace_type = WorkspaceType::WebProject;
            self.languages.push("HTML".to_string());
            self.languages.push("CSS".to_string());
            self.languages.push("JavaScript".to_string());
        }

        self.detect_languages_from_files().await;
    }

    /// Detects languages from file extensions.
    async fn detect_languages_from_files(&mut self) {
        const LANGUAGE_SCAN_LIMIT: usize = 50;

        let mut language_map = HashMap::new();
        language_map.insert("rs", "Rust");
        language_map.insert("js", "JavaScript");
        language_map.insert("ts", "TypeScript");
        language_map.insert("py", "Python");
        language_map.insert("java", "Java");
        language_map.insert("cpp", "C++");
        language_map.insert("c", "C");
        language_map.insert("h", "C/C++");
        language_map.insert("html", "HTML");
        language_map.insert("css", "CSS");
        language_map.insert("go", "Go");
        language_map.insert("php", "PHP");
        language_map.insert("rb", "Ruby");
        language_map.insert("swift", "Swift");
        language_map.insert("kt", "Kotlin");

        if let Ok(mut read_dir) = fs::read_dir(&self.root_path).await {
            let mut found_languages = std::collections::HashSet::new();
            let mut count = 0;

            while let Ok(Some(entry)) = read_dir.next_entry().await {
                if count > LANGUAGE_SCAN_LIMIT {
                    break;
                }
                count += 1;

                if let Some(extension) = entry.path().extension().and_then(|s| s.to_str()) {
                    if let Some(language) = language_map.get(extension) {
                        found_languages.insert(language.to_string());
                    }
                }
            }

            for lang in found_languages {
                if !self.languages.contains(&lang) {
                    self.languages.push(lang);
                }
            }
        }
    }

    /// Scans the workspace.
    async fn scan_workspace(&mut self, options: ScanOptions) -> BitFunResult<()> {
        let mut stats = WorkspaceStatistics {
            total_files: 0,
            total_directories: 0,
            total_size_bytes: 0,
            file_extensions: HashMap::new(),
            last_modified: None,
            git_info: None,
        };

        self.scan_directory(&self.root_path.clone(), &mut stats, &options, 0)
            .await?;

        if options.scan_git_info {
            stats.git_info = self.scan_git_info().await;
        }

        self.statistics = Some(stats);
        Ok(())
    }

    /// Recursively scans a directory.
    fn scan_directory<'a>(
        &'a self,
        dir: &'a Path,
        stats: &'a mut WorkspaceStatistics,
        options: &'a ScanOptions,
        depth: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = BitFunResult<()>> + 'a + Send>> {
        Box::pin(async move {
            if let Some(max_depth) = options.max_depth {
                if depth > max_depth {
                    return Ok(());
                }
            }

            let mut read_dir = fs::read_dir(dir)
                .await
                .map_err(|e| BitFunError::service(format!("Failed to read directory: {}", e)))?;

            while let Some(entry) = read_dir.next_entry().await.map_err(|e| {
                BitFunError::service(format!("Failed to read directory entry: {}", e))
            })? {
                let path = entry.path();
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if !options.include_hidden && file_name.starts_with('.') {
                    continue;
                }

                if options
                    .ignore_patterns
                    .iter()
                    .any(|pattern| file_name.contains(pattern))
                {
                    continue;
                }

                let metadata = entry
                    .metadata()
                    .await
                    .map_err(|e| BitFunError::service(format!("Failed to read metadata: {}", e)))?;

                if metadata.is_file() {
                    stats.total_files += 1;
                    stats.total_size_bytes += metadata.len();

                    if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
                        *stats
                            .file_extensions
                            .entry(extension.to_string())
                            .or_insert(0) += 1;
                    }

                    if let Ok(modified) = metadata.modified() {
                        let modified_dt = chrono::DateTime::<chrono::Utc>::from(modified);
                        if stats
                            .last_modified
                            .as_ref()
                            .map_or(true, |last_modified| last_modified < &modified_dt)
                        {
                            stats.last_modified = Some(modified_dt);
                        }
                    }
                } else if metadata.is_dir() {
                    stats.total_directories += 1;

                    if let Err(e) = self.scan_directory(&path, stats, options, depth + 1).await {
                        warn!("Failed to scan subdirectory {:?}: {}", path, e);
                    }
                }
            }

            Ok(())
        })
    }

    /// Scans Git information.
    async fn scan_git_info(&self) -> Option<GitInfo> {
        let git_dir = self.root_path.join(".git");
        if !git_dir.exists() {
            return Some(GitInfo {
                is_git_repo: false,
                current_branch: None,
                remote_url: None,
                has_uncommitted_changes: false,
                total_commits: None,
            });
        }

        let mut git_info = GitInfo {
            is_git_repo: true,
            current_branch: None,
            remote_url: None,
            has_uncommitted_changes: false,
            total_commits: None,
        };

        if let Ok(head_content) = fs::read_to_string(git_dir.join("HEAD")).await {
            if let Some(branch) = head_content.strip_prefix("ref: refs/heads/") {
                git_info.current_branch = Some(branch.trim().to_string());
            }
        }

        if let Ok(status_output) = crate::util::process_manager::create_tokio_command("git")
            .arg("status")
            .arg("--porcelain")
            .current_dir(&self.root_path)
            .output()
            .await
        {
            git_info.has_uncommitted_changes = !status_output.stdout.is_empty();
        }

        Some(git_info)
    }

    /// Updates the last-accessed timestamp.
    pub fn touch(&mut self) {
        self.last_accessed = chrono::Utc::now();
    }

    /// Checks whether the workspace is still valid.
    pub async fn is_valid(&self) -> bool {
        if self.workspace_kind == WorkspaceKind::Remote {
            return true;
        }
        self.root_path.exists() && self.root_path.is_dir()
    }

    /// Returns a workspace summary.
    pub fn get_summary(&self) -> WorkspaceSummary {
        WorkspaceSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            root_path: self.root_path.clone(),
            workspace_type: self.workspace_type.clone(),
            workspace_kind: self.workspace_kind.clone(),
            assistant_id: self.assistant_id.clone(),
            status: self.status.clone(),
            languages: self.languages.clone(),
            last_accessed: self.last_accessed,
            file_count: self.statistics.as_ref().map(|s| s.total_files).unwrap_or(0),
            tags: self.tags.clone(),
        }
    }
}

/// Workspace summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    pub id: String,
    pub name: String,
    #[serde(rename = "rootPath")]
    pub root_path: PathBuf,
    #[serde(rename = "workspaceType")]
    pub workspace_type: WorkspaceType,
    #[serde(rename = "workspaceKind")]
    pub workspace_kind: WorkspaceKind,
    #[serde(rename = "assistantId", skip_serializing_if = "Option::is_none")]
    pub assistant_id: Option<String>,
    pub status: WorkspaceStatus,
    pub languages: Vec<String>,
    #[serde(rename = "lastAccessed")]
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "fileCount")]
    pub file_count: usize,
    pub tags: Vec<String>,
}

/// Workspace manager.
pub struct WorkspaceManager {
    workspaces: HashMap<String, WorkspaceInfo>,
    opened_workspace_ids: Vec<String>,
    current_workspace_id: Option<String>,
    recent_workspaces: Vec<String>,
    recent_assistant_workspaces: Vec<String>,
    max_recent_workspaces: usize,
}

/// Workspace manager configuration.
#[derive(Debug, Clone)]
pub struct WorkspaceManagerConfig {
    pub max_recent_workspaces: usize,
    pub auto_cleanup_invalid: bool,
    pub default_scan_options: ScanOptions,
}

impl Default for WorkspaceManagerConfig {
    fn default() -> Self {
        Self {
            max_recent_workspaces: 20,
            auto_cleanup_invalid: true,
            default_scan_options: ScanOptions::default(),
        }
    }
}

impl WorkspaceManager {
    /// Creates a new workspace manager.
    pub fn new(config: WorkspaceManagerConfig) -> Self {
        Self {
            workspaces: HashMap::new(),
            opened_workspace_ids: Vec::new(),
            current_workspace_id: None,
            recent_workspaces: Vec::new(),
            recent_assistant_workspaces: Vec::new(),
            max_recent_workspaces: config.max_recent_workspaces,
        }
    }

    /// Opens a workspace.
    pub async fn open_workspace(&mut self, path: PathBuf) -> BitFunResult<WorkspaceInfo> {
        self.open_workspace_with_options(path, WorkspaceOpenOptions::default())
            .await
    }

    /// Opens a workspace with custom options.
    pub async fn open_workspace_with_options(
        &mut self,
        path: PathBuf,
        options: WorkspaceOpenOptions,
    ) -> BitFunResult<WorkspaceInfo> {
        let is_remote = options.workspace_kind == WorkspaceKind::Remote;

        if !is_remote {
            if !path.exists() {
                return Err(BitFunError::service(format!(
                    "Workspace path does not exist: {:?}",
                    path
                )));
            }

            if !path.is_dir() {
                return Err(BitFunError::service(format!(
                    "Workspace path is not a directory: {:?}",
                    path
                )));
            }
        }

        let existing_workspace_id = if is_remote {
            let desired = options
                .remote_connection_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            self.workspaces
                .values()
                .find(|w| {
                    if w.workspace_kind != WorkspaceKind::Remote || w.root_path != path {
                        return false;
                    }
                    let existing = w.remote_ssh_connection_id();
                    match desired {
                        Some(d) => existing == Some(d),
                        None => existing.is_none(),
                    }
                })
                .map(|w| w.id.clone())
        } else {
            self.workspaces
                .values()
                .find(|w| {
                    w.root_path == path && w.workspace_kind != WorkspaceKind::Remote
                })
                .map(|w| w.id.clone())
        };

        if let Some(workspace_id) = existing_workspace_id {
            if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
                workspace.workspace_kind = options.workspace_kind.clone();
                workspace.assistant_id = if options.workspace_kind == WorkspaceKind::Assistant {
                    options.assistant_id.clone()
                } else {
                    None
                };
                if let Some(display_name) = &options.display_name {
                    workspace.name = display_name.clone();
                }
                workspace.load_identity().await;
                workspace.load_worktree().await;
            }
            self.ensure_workspace_open(&workspace_id);
            if options.auto_set_current {
                self.set_current_workspace_with_recent_policy(
                    workspace_id.clone(),
                    options.add_to_recent,
                )?;
            } else {
                self.touch_workspace_access(&workspace_id, options.add_to_recent);
            }
            return self.workspaces.get(&workspace_id).cloned().ok_or_else(|| {
                BitFunError::service(format!(
                    "Workspace '{}' disappeared after selecting it",
                    workspace_id
                ))
            });
        }

        let workspace = WorkspaceInfo::new(path, options.clone()).await?;
        let workspace_id = workspace.id.clone();

        self.workspaces
            .insert(workspace_id.clone(), workspace.clone());
        self.ensure_workspace_open(&workspace_id);
        if options.auto_set_current {
            self.set_current_workspace_with_recent_policy(
                workspace_id.clone(),
                options.add_to_recent,
            )?;
        } else {
            self.touch_workspace_access(&workspace_id, options.add_to_recent);
        }

        Ok(workspace)
    }

    /// Closes the current workspace.
    pub fn close_current_workspace(&mut self) -> BitFunResult<()> {
        let current_workspace_id = self.current_workspace_id.clone();
        match current_workspace_id {
            Some(workspace_id) => self.close_workspace(&workspace_id),
            None => Ok(()),
        }
    }

    /// Closes the specified workspace.
    pub fn close_workspace(&mut self, workspace_id: &str) -> BitFunResult<()> {
        if !self.workspaces.contains_key(workspace_id) {
            return Err(BitFunError::service(format!(
                "Workspace not found: {}",
                workspace_id
            )));
        }
        let closed_workspace_kind = self
            .workspaces
            .get(workspace_id)
            .map(|workspace| workspace.workspace_kind.clone())
            .unwrap_or_default();

        self.opened_workspace_ids.retain(|id| id != workspace_id);

        if let Some(workspace) = self.workspaces.get_mut(workspace_id) {
            workspace.status = WorkspaceStatus::Inactive;
        }

        if self.current_workspace_id.as_deref() == Some(workspace_id) {
            self.current_workspace_id = None;

            if let Some(next_workspace_id) =
                self.find_next_workspace_id_after_close(&closed_workspace_kind)
            {
                self.set_current_workspace(next_workspace_id)?;
            }
        }

        Ok(())
    }

    /// Sets the active workspace among already opened workspaces.
    pub fn set_active_workspace(&mut self, workspace_id: &str) -> BitFunResult<()> {
        if !self
            .opened_workspace_ids
            .iter()
            .any(|id| id == workspace_id)
        {
            return Err(BitFunError::service(format!(
                "Workspace is not opened: {}",
                workspace_id
            )));
        }

        self.set_current_workspace(workspace_id.to_string())
    }

    /// Sets the current workspace.
    pub fn set_current_workspace(&mut self, workspace_id: String) -> BitFunResult<()> {
        self.set_current_workspace_with_recent_policy(workspace_id, true)
    }

    fn set_current_workspace_with_recent_policy(
        &mut self,
        workspace_id: String,
        add_to_recent: bool,
    ) -> BitFunResult<()> {
        if !self.workspaces.contains_key(&workspace_id) {
            return Err(BitFunError::service(format!(
                "Workspace not found: {}",
                workspace_id
            )));
        }

        self.ensure_workspace_open(&workspace_id);

        if let Some(previous_workspace_id) = &self.current_workspace_id {
            if previous_workspace_id != &workspace_id {
                if let Some(previous_workspace) = self.workspaces.get_mut(previous_workspace_id) {
                    previous_workspace.status = WorkspaceStatus::Inactive;
                }
            }
        }

        if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
            workspace.status = WorkspaceStatus::Active;
            workspace.touch();
        }

        self.current_workspace_id = Some(workspace_id.clone());

        if add_to_recent {
            self.update_recent_workspaces(workspace_id);
        }

        Ok(())
    }

    /// Gets the current workspace.
    pub fn get_current_workspace(&self) -> Option<&WorkspaceInfo> {
        if let Some(workspace_id) = &self.current_workspace_id {
            self.workspaces.get(workspace_id)
        } else {
            None
        }
    }

    /// Gets a workspace by id.
    pub fn get_workspace(&self, workspace_id: &str) -> Option<&WorkspaceInfo> {
        self.workspaces.get(workspace_id)
    }

    /// Gets all opened workspaces.
    pub fn get_opened_workspace_infos(&self) -> Vec<&WorkspaceInfo> {
        self.opened_workspace_ids
            .iter()
            .filter_map(|id| self.workspaces.get(id))
            .collect()
    }

    /// Lists all workspaces.
    pub fn list_workspaces(&self) -> Vec<WorkspaceSummary> {
        self.workspaces.values().map(|w| w.get_summary()).collect()
    }

    /// Returns recently accessed workspace records.
    pub fn get_recent_workspace_infos(&self) -> Vec<&WorkspaceInfo> {
        self.recent_workspaces
            .iter()
            .filter_map(|id| self.workspaces.get(id))
            .collect()
    }

    /// Returns recently accessed assistant workspace records.
    pub fn get_recent_assistant_workspace_infos(&self) -> Vec<&WorkspaceInfo> {
        self.recent_assistant_workspaces
            .iter()
            .filter_map(|id| self.workspaces.get(id))
            .collect()
    }

    /// Searches workspaces.
    pub fn search_workspaces(&self, query: &str) -> Vec<WorkspaceSummary> {
        let query_lower = query.to_lowercase();

        self.workspaces
            .values()
            .filter(|workspace| {
                workspace.name.to_lowercase().contains(&query_lower)
                    || workspace
                        .root_path
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&query_lower)
                    || workspace
                        .languages
                        .iter()
                        .any(|lang| lang.to_lowercase().contains(&query_lower))
                    || workspace
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query_lower))
            })
            .map(|w| w.get_summary())
            .collect()
    }

    /// Removes a workspace.
    pub fn remove_workspace(&mut self, workspace_id: &str) -> BitFunResult<()> {
        if let Some(_) = self.workspaces.remove(workspace_id) {
            if self.current_workspace_id.as_ref() == Some(&workspace_id.to_string()) {
                self.current_workspace_id = None;
            }

            self.opened_workspace_ids.retain(|id| id != workspace_id);
            self.recent_workspaces.retain(|id| id != workspace_id);
            self.recent_assistant_workspaces
                .retain(|id| id != workspace_id);

            Ok(())
        } else {
            Err(BitFunError::service(format!(
                "Workspace not found: {}",
                workspace_id
            )))
        }
    }

    /// Cleans up invalid workspaces.
    pub async fn cleanup_invalid_workspaces(&mut self) -> BitFunResult<usize> {
        let mut invalid_workspaces = Vec::new();

        for (workspace_id, workspace) in &self.workspaces {
            if !workspace.is_valid().await {
                invalid_workspaces.push(workspace_id.clone());
            }
        }

        let count = invalid_workspaces.len();
        for workspace_id in invalid_workspaces {
            self.remove_workspace(&workspace_id)?;
        }

        Ok(count)
    }

    /// Updates the recent-workspaces list.
    fn update_recent_workspaces(&mut self, workspace_id: String) {
        self.recent_workspaces.retain(|id| id != &workspace_id);
        self.recent_assistant_workspaces
            .retain(|id| id != &workspace_id);

        let is_assistant = self
            .workspaces
            .get(&workspace_id)
            .map(|workspace| workspace.workspace_kind == WorkspaceKind::Assistant)
            .unwrap_or(false);
        let target_list = if is_assistant {
            &mut self.recent_assistant_workspaces
        } else {
            &mut self.recent_workspaces
        };
        target_list.insert(0, workspace_id);

        if target_list.len() > self.max_recent_workspaces {
            target_list.truncate(self.max_recent_workspaces);
        }
    }

    fn touch_workspace_access(&mut self, workspace_id: &str, add_to_recent: bool) {
        if let Some(workspace) = self.workspaces.get_mut(workspace_id) {
            workspace.touch();
            if self.current_workspace_id.as_deref() != Some(workspace_id) {
                workspace.status = WorkspaceStatus::Inactive;
            }
        }

        if add_to_recent {
            self.update_recent_workspaces(workspace_id.to_string());
        }
    }

    fn find_next_workspace_id_after_close(&self, preferred_kind: &WorkspaceKind) -> Option<String> {
        let same_kind = self
            .opened_workspace_ids
            .iter()
            .find(|id| {
                self.workspaces
                    .get(id.as_str())
                    .map(|workspace| &workspace.workspace_kind == preferred_kind)
                    .unwrap_or(false)
            })
            .cloned();

        if same_kind.is_some() {
            return same_kind;
        }

        // Closing the last remote workspace (e.g. SSH password session could not auto-reconnect)
        // must not activate an unrelated local project; leave current unset until the user picks
        // a workspace or reconnects.
        if *preferred_kind == WorkspaceKind::Remote {
            return None;
        }

        self.opened_workspace_ids.first().cloned()
    }

    /// Ensures a workspace stays in the opened list.
    fn ensure_workspace_open(&mut self, workspace_id: &str) {
        self.opened_workspace_ids.retain(|id| id != workspace_id);
        self.opened_workspace_ids
            .insert(0, workspace_id.to_string());
    }

    /// Returns manager statistics.
    pub fn get_statistics(&self) -> WorkspaceManagerStatistics {
        let mut stats = WorkspaceManagerStatistics::default();

        stats.total_workspaces = self.workspaces.len();

        for workspace in self.workspaces.values() {
            match workspace.status {
                WorkspaceStatus::Active => stats.active_workspaces += 1,
                WorkspaceStatus::Inactive => stats.inactive_workspaces += 1,
                WorkspaceStatus::Archived => stats.archived_workspaces += 1,
                _ => {}
            }

            *stats
                .workspaces_by_type
                .entry(workspace.workspace_type.clone())
                .or_insert(0) += 1;

            if let Some(statistics) = &workspace.statistics {
                stats.total_files += statistics.total_files;
                stats.total_size_bytes += statistics.total_size_bytes;
            }
        }

        stats
    }

    /// Returns the number of workspaces.
    pub fn get_workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    /// Returns an immutable reference to the workspace map (for export).
    pub fn get_workspaces(&self) -> &HashMap<String, WorkspaceInfo> {
        &self.workspaces
    }

    /// Returns a mutable reference to the workspace map (for import).
    pub fn get_workspaces_mut(&mut self) -> &mut HashMap<String, WorkspaceInfo> {
        &mut self.workspaces
    }

    /// Returns the opened workspace ids.
    pub fn get_opened_workspace_ids(&self) -> &Vec<String> {
        &self.opened_workspace_ids
    }

    /// Sets the opened workspace ids.
    pub fn set_opened_workspace_ids(&mut self, opened_workspace_ids: Vec<String>) {
        self.opened_workspace_ids = opened_workspace_ids
            .into_iter()
            .filter(|id| self.workspaces.contains_key(id))
            .collect();
    }

    /// Returns a reference to the recent-workspaces list.
    pub fn get_recent_workspaces(&self) -> &Vec<String> {
        &self.recent_workspaces
    }

    /// Sets the recent-workspaces list.
    pub fn set_recent_workspaces(&mut self, recent: Vec<String>) {
        self.recent_workspaces = recent
            .into_iter()
            .filter(|id| {
                self.workspaces
                    .get(id)
                    .map(|workspace| workspace.workspace_kind == WorkspaceKind::Normal)
                    .unwrap_or(false)
            })
            .collect();
    }

    /// Returns a reference to the recent assistant-workspaces list.
    pub fn get_recent_assistant_workspaces(&self) -> &Vec<String> {
        &self.recent_assistant_workspaces
    }

    /// Sets the recent assistant-workspaces list.
    pub fn set_recent_assistant_workspaces(&mut self, recent: Vec<String>) {
        self.recent_assistant_workspaces = recent
            .into_iter()
            .filter(|id| {
                self.workspaces
                    .get(id)
                    .map(|workspace| workspace.workspace_kind == WorkspaceKind::Assistant)
                    .unwrap_or(false)
            })
            .collect();
    }
}

/// Workspace manager statistics.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WorkspaceManagerStatistics {
    pub total_workspaces: usize,
    pub active_workspaces: usize,
    pub inactive_workspaces: usize,
    pub archived_workspaces: usize,
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub workspaces_by_type: HashMap<WorkspaceType, usize>,
}
