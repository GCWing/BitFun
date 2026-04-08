//! Workspace context generator

use crate::infrastructure::FileTreeService;
use crate::service::workspace::WorkspaceManager;
use crate::util::errors::*;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Workspace context generator.
pub struct WorkspaceContextGenerator {
    workspace_manager: Option<Arc<RwLock<WorkspaceManager>>>,
    file_tree_service: Arc<FileTreeService>,
}

/// Generated workspace context information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedWorkspaceContext {
    pub app_name: String,
    pub current_date: String,
    pub operating_system: String,
    pub working_directory: String,
    pub directory_structure: String,
    pub project_summary: Option<String>,
    pub git_info: Option<GitInfo>,
    pub statistics: Option<WorkspaceStatistics>,
}

impl Default for GeneratedWorkspaceContext {
    fn default() -> Self {
        Self {
            app_name: "BitFun".to_string(),
            current_date: chrono::Utc::now()
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
            operating_system: std::env::consts::OS.to_string(),
            working_directory: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "Unknown".to_string()),
            directory_structure: "Unable to analyze directory structure".to_string(),
            project_summary: None,
            git_info: None,
            statistics: None,
        }
    }
}

/// Git information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    pub branch: Option<String>,
    pub commit_hash: Option<String>,
    pub status: Option<String>,
    pub remote_url: Option<String>,
}

/// Workspace statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStatistics {
    pub total_files: usize,
    pub total_directories: usize,
    pub total_size_mb: u64,
    pub code_files_count: usize,
    pub most_common_extensions: Vec<(String, usize)>,
}

/// Options for generating context.
#[derive(Debug, Clone)]
pub struct ContextGenerationOptions {
    pub include_git_info: bool,
    pub include_statistics: bool,
    pub include_project_summary: bool,
    pub max_directory_depth: Option<u32>,
    pub max_files_shown: usize,
    pub language: ContextLanguage,
}

/// Context language.
#[derive(Debug, Clone)]
pub enum ContextLanguage {
    Chinese,
    English,
}

impl Default for ContextGenerationOptions {
    fn default() -> Self {
        Self {
            include_git_info: false,
            include_statistics: true,
            include_project_summary: true,
            max_directory_depth: Some(3),
            max_files_shown: 20,
            language: ContextLanguage::Chinese,
        }
    }
}

impl WorkspaceContextGenerator {
    /// Creates a new workspace context generator.
    pub fn new(
        workspace_manager: Option<Arc<RwLock<WorkspaceManager>>>,
        file_tree_service: Arc<FileTreeService>,
    ) -> Self {
        Self {
            workspace_manager,
            file_tree_service,
        }
    }

    /// Creates a default generator.
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(None, Arc::new(FileTreeService::default()))
    }

    /// Generates the full workspace context.
    pub async fn generate_context(
        &self,
        workspace_path: Option<&str>,
        options: ContextGenerationOptions,
    ) -> BitFunResult<GeneratedWorkspaceContext> {
        let app_name = "BitFun".to_string();
        let current_date = self.get_current_date(&options.language);
        let operating_system = self.get_operating_system();

        let (working_directory, directory_structure, project_summary, git_info, statistics) =
            if let Some(ws_path) = workspace_path {
                self.generate_context_for_path(ws_path, &options).await?
            } else if let Some(ref workspace_manager) = self.workspace_manager {
                let manager = workspace_manager.read().await;
                if let Some(current_workspace) = manager.get_current_workspace() {
                    let working_dir = current_workspace.root_path.to_string_lossy();
                    self.generate_context_for_path(&working_dir, &options)
                        .await?
                } else {
                    let current_dir = std::env::current_dir()
                        .map_err(|e| {
                            BitFunError::service(format!("Failed to get current directory: {}", e))
                        })?
                        .to_string_lossy()
                        .to_string();
                    self.generate_context_for_path(&current_dir, &options)
                        .await?
                }
            } else {
                let current_dir = std::env::current_dir()
                    .map_err(|e| {
                        BitFunError::service(format!("Failed to get current directory: {}", e))
                    })?
                    .to_string_lossy()
                    .to_string();
                self.generate_context_for_path(&current_dir, &options)
                    .await?
            };

        Ok(GeneratedWorkspaceContext {
            app_name,
            current_date,
            operating_system,
            working_directory,
            directory_structure,
            project_summary,
            git_info,
            statistics,
        })
    }

    /// Generates context for the given path.
    async fn generate_context_for_path(
        &self,
        path: &str,
        options: &ContextGenerationOptions,
    ) -> BitFunResult<(
        String,
        String,
        Option<String>,
        Option<GitInfo>,
        Option<WorkspaceStatistics>,
    )> {
        let working_dir = path.to_string();

        let dir_structure = self.generate_directory_structure(path, options).await?;

        let proj_summary = if options.include_project_summary {
            self.get_project_summary(path).await
        } else {
            None
        };

        let git_info = if options.include_git_info {
            self.get_git_info(path).await
        } else {
            None
        };

        let statistics = if options.include_statistics {
            self.get_workspace_statistics(path).await.ok()
        } else {
            None
        };

        Ok((
            working_dir,
            dir_structure,
            proj_summary,
            git_info,
            statistics,
        ))
    }

    /// Generates the workspace context prompt.
    pub async fn generate_context_prompt(
        &self,
        workspace_path: Option<&str>,
        options: ContextGenerationOptions,
    ) -> BitFunResult<String> {
        let context = self
            .generate_context(workspace_path, options.clone())
            .await?;

        let mut prompt = match options.language {
            ContextLanguage::Chinese => {
                format!(
                    "这是{}。我们正在设置聊天上下文。\n\
                    今天的日期是：{}\n\
                    我的操作系统是：{}\n\
                    我当前正在工作的目录：{}\n\
                    以下是当前工作目录的文件夹结构：\n\n\
                    显示最多{}个项目（文件+文件夹）。用...表示的文件夹或文件包含更多未显示的项目，或者已达到显示限制。\n\n\
                    {}\n",
                    context.app_name,
                    context.current_date,
                    context.operating_system,
                    context.working_directory,
                    options.max_files_shown,
                    context.directory_structure
                )
            }
            ContextLanguage::English => {
                format!(
                    "This is the {}. We are setting up the context for our chat.\n\
                    Today's date is {}.\n\
                    My operating system is: {}\n\
                    I'm currently working in the directory: {}\n\
                    Here is the folder structure of the current working directories:\n\n\
                    Showing up to {} items (files + folders). Folders or files indicated with ... contain more items not shown, were ignored, or the display limit was reached.\n\n\
                    {}\n",
                    context.app_name,
                    context.current_date,
                    context.operating_system,
                    context.working_directory,
                    options.max_files_shown,
                    context.directory_structure
                )
            }
        };

        if let Some(summary) = context.project_summary {
            match options.language {
                ContextLanguage::Chinese => {
                    prompt.push_str(&format!("\n\n项目总结：\n{}\n", summary));
                }
                ContextLanguage::English => {
                    prompt.push_str(&format!("\n\nProject Summary:\n{}\n", summary));
                }
            }
        }

        if let Some(git_info) = context.git_info {
            match options.language {
                ContextLanguage::Chinese => {
                    prompt.push_str("\n\nGit信息：\n");
                    if let Some(branch) = git_info.branch {
                        prompt.push_str(&format!("- 当前分支：{}\n", branch));
                    }
                    if let Some(commit) = git_info.commit_hash {
                        prompt.push_str(&format!("- 最新提交：{}\n", commit));
                    }
                    if let Some(status) = git_info.status {
                        prompt.push_str(&format!("- 工作区状态：{}\n", status));
                    }
                }
                ContextLanguage::English => {
                    prompt.push_str("\n\nGit Information:\n");
                    if let Some(branch) = git_info.branch {
                        prompt.push_str(&format!("- Current branch: {}\n", branch));
                    }
                    if let Some(commit) = git_info.commit_hash {
                        prompt.push_str(&format!("- Latest commit: {}\n", commit));
                    }
                    if let Some(status) = git_info.status {
                        prompt.push_str(&format!("- Working tree status: {}\n", status));
                    }
                }
            }
        }

        if let Some(stats) = context.statistics {
            match options.language {
                ContextLanguage::Chinese => {
                    prompt.push_str(&format!(
                        "\n\n工作区统计：\n\
                        - 总文件数：{}\n\
                        - 总目录数：{}\n\
                        - 总大小：{}MB\n\
                        - 代码文件数：{}\n",
                        stats.total_files,
                        stats.total_directories,
                        stats.total_size_mb,
                        stats.code_files_count
                    ));

                    if !stats.most_common_extensions.is_empty() {
                        prompt.push_str("- 常见文件类型：");
                        for (ext, count) in stats.most_common_extensions.iter().take(5) {
                            prompt.push_str(&format!(" .{}({})", ext, count));
                        }
                        prompt.push('\n');
                    }
                }
                ContextLanguage::English => {
                    prompt.push_str(&format!(
                        "\n\nWorkspace Statistics:\n\
                        - Total files: {}\n\
                        - Total directories: {}\n\
                        - Total size: {}MB\n\
                        - Code files: {}\n",
                        stats.total_files,
                        stats.total_directories,
                        stats.total_size_mb,
                        stats.code_files_count
                    ));

                    if !stats.most_common_extensions.is_empty() {
                        prompt.push_str("- Common file types:");
                        for (ext, count) in stats.most_common_extensions.iter().take(5) {
                            prompt.push_str(&format!(" .{}({})", ext, count));
                        }
                        prompt.push('\n');
                    }
                }
            }
        }

        Ok(prompt)
    }

    /// Legacy-compatible API: generate context.
    pub async fn generate_context_legacy(
        &self,
        workspace_path: Option<&str>,
    ) -> BitFunResult<GeneratedWorkspaceContext> {
        let options = ContextGenerationOptions::default();
        self.generate_context(workspace_path, options).await
    }

    /// Legacy-compatible API: generate context prompt.
    pub async fn generate_context_prompt_legacy(
        &self,
        workspace_path: Option<&str>,
    ) -> BitFunResult<String> {
        let options = ContextGenerationOptions::default();
        self.generate_context_prompt(workspace_path, options).await
    }

    /// Returns the current date.
    fn get_current_date(&self, language: &ContextLanguage) -> String {
        let now = Utc::now();
        match language {
            ContextLanguage::Chinese => now
                .format("%Y年%m月%d日星期%u")
                .to_string()
                .replace("星期1", "星期一")
                .replace("星期2", "星期二")
                .replace("星期3", "星期三")
                .replace("星期4", "星期四")
                .replace("星期5", "星期五")
                .replace("星期6", "星期六")
                .replace("星期7", "星期日"),
            ContextLanguage::English => now.format("%A, %B %d, %Y").to_string(),
        }
    }

    /// Returns operating system information.
    fn get_operating_system(&self) -> String {
        match std::env::consts::OS {
            "windows" => {
                let arch = if cfg!(target_arch = "x86_64") {
                    "x86_64"
                } else if cfg!(target_arch = "x86") {
                    "x86"
                } else if cfg!(target_arch = "aarch64") {
                    "aarch64"
                } else {
                    "unknown"
                };
                format!("Windows ({})", arch)
            }
            "macos" => {
                let arch = if cfg!(target_arch = "aarch64") {
                    "Apple Silicon"
                } else {
                    "Intel"
                };
                format!("macOS ({})", arch)
            }
            "linux" => {
                let arch = std::env::consts::ARCH;
                format!("Linux ({})", arch)
            }
            other => other.to_string(),
        }
    }

    /// Generates the directory structure (using the new file tree service).
    async fn generate_directory_structure(
        &self,
        path: &str,
        options: &ContextGenerationOptions,
    ) -> BitFunResult<String> {
        let path_buf = PathBuf::from(path);
        if !path_buf.exists() {
            return Err(BitFunError::service(format!(
                "Directory does not exist: {}",
                path
            )));
        }

        let contents = self
            .file_tree_service
            .get_directory_contents(path)
            .await
            .map_err(BitFunError::service)?;

        let mut structure = String::new();
        let dir_name = path_buf
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");

        structure.push_str(&format!("{}/\n", dir_name));

        let mut directories = Vec::new();
        let mut files = Vec::new();

        for item in contents {
            let display_name = if item.is_directory {
                format!("{}/", item.name)
            } else {
                item.name.clone()
            };

            if item.is_directory {
                directories.push(display_name);
            } else {
                files.push(display_name);
            }
        }

        directories.sort();
        files.sort();

        let mut all_entries = Vec::new();
        all_entries.extend(directories);
        all_entries.extend(files);

        if all_entries.len() > options.max_files_shown {
            all_entries.truncate(options.max_files_shown);
            all_entries.push("... (more items not shown)".to_string());
        }

        for (i, entry) in all_entries.iter().enumerate() {
            let prefix = if i == all_entries.len() - 1 {
                "└── "
            } else {
                "├── "
            };
            structure.push_str(&format!("{}{}\n", prefix, entry));
        }

        Ok(structure)
    }

    /// Retrieves a project summary.
    async fn get_project_summary(&self, workspace_path: &str) -> Option<String> {
        let readme_files = ["README.md", "README.txt", "README.rst", "readme.md"];
        let package_files = ["package.json", "Cargo.toml", "pyproject.toml", "pom.xml"];

        for readme_file in &readme_files {
            let readme_path = PathBuf::from(workspace_path).join(readme_file);
            if let Ok(content) = tokio::fs::read_to_string(&readme_path).await {
                let lines: Vec<&str> = content.lines().take(10).collect();
                let summary = lines.join("\n");
                if !summary.trim().is_empty() {
                    return Some(format!("From {}:\n{}", readme_file, summary));
                }
            }
        }

        for package_file in &package_files {
            let package_path = PathBuf::from(workspace_path).join(package_file);
            if package_path.exists() {
                if let Some(summary) = self.extract_project_info_from_config(&package_path).await {
                    return Some(summary);
                }
            }
        }

        None
    }

    /// Extracts project information from a config file.
    async fn extract_project_info_from_config(&self, config_path: &PathBuf) -> Option<String> {
        let file_name = config_path.file_name()?.to_str()?;

        match file_name {
            "package.json" => {
                if let Ok(content) = tokio::fs::read_to_string(config_path).await {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        let mut info = String::new();
                        if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                            info.push_str(&format!("Package: {}\n", name));
                        }
                        if let Some(desc) = json.get("description").and_then(|v| v.as_str()) {
                            info.push_str(&format!("Description: {}\n", desc));
                        }
                        if let Some(version) = json.get("version").and_then(|v| v.as_str()) {
                            info.push_str(&format!("Version: {}\n", version));
                        }
                        if !info.is_empty() {
                            return Some(format!("From package.json:\n{}", info));
                        }
                    }
                }
            }
            "Cargo.toml" => {
                if let Ok(content) = tokio::fs::read_to_string(config_path).await {
                    let mut info = String::new();
                    let lines: Vec<&str> = content.lines().collect();
                    let mut in_package = false;

                    for line in lines {
                        let line = line.trim();
                        if line == "[package]" {
                            in_package = true;
                            continue;
                        }
                        if line.starts_with('[') && line != "[package]" {
                            in_package = false;
                        }

                        if in_package
                            && line.contains('=')
                            && (line.starts_with("name")
                                || line.starts_with("description")
                                || line.starts_with("version"))
                        {
                            info.push_str(&format!("{}\n", line));
                        }
                    }

                    if !info.is_empty() {
                        return Some(format!("From Cargo.toml:\n{}", info));
                    }
                }
            }
            _ => {}
        }

        None
    }

    /// Retrieves Git information.
    async fn get_git_info(&self, workspace_path: &str) -> Option<GitInfo> {
        let git_dir = PathBuf::from(workspace_path).join(".git");
        if !git_dir.exists() {
            return None;
        }

        let mut git_info = GitInfo {
            branch: None,
            commit_hash: None,
            status: None,
            remote_url: None,
        };

        if let Ok(head_content) = tokio::fs::read_to_string(git_dir.join("HEAD")).await {
            if let Some(branch) = head_content.strip_prefix("ref: refs/heads/") {
                git_info.branch = Some(branch.trim().to_string());
            }
        }

        if let Some(branch) = &git_info.branch {
            let commit_file = git_dir.join("refs").join("heads").join(branch);
            if let Ok(commit_content) = tokio::fs::read_to_string(commit_file).await {
                git_info.commit_hash = Some(commit_content.trim().chars().take(8).collect());
            }
        }

        git_info.status = Some("Clean".to_string());

        Some(git_info)
    }

    /// Retrieves workspace statistics.
    async fn get_workspace_statistics(
        &self,
        workspace_path: &str,
    ) -> BitFunResult<WorkspaceStatistics> {
        if let Ok((_, file_stats)) = self
            .file_tree_service
            .build_tree_with_stats(workspace_path)
            .await
        {
            let code_extensions = [
                "rs", "js", "ts", "py", "java", "cpp", "c", "h", "hpp", "go", "php", "rb", "swift",
                "kt", "scala", "sh", "bash", "html", "css", "scss", "less", "vue", "jsx", "tsx",
            ];

            let code_files_count = file_stats
                .file_type_counts
                .iter()
                .filter(|(ext, _)| code_extensions.contains(&ext.as_str()))
                .map(|(_, count)| count)
                .sum();

            let mut most_common: Vec<_> = file_stats.file_type_counts.into_iter().collect();
            most_common.sort_by(|a, b| b.1.cmp(&a.1));

            Ok(WorkspaceStatistics {
                total_files: file_stats.total_files,
                total_directories: file_stats.total_directories,
                total_size_mb: file_stats.total_size_bytes / (1024 * 1024),
                code_files_count,
                most_common_extensions: most_common.into_iter().take(10).collect(),
            })
        } else {
            Err(BitFunError::service(
                "Failed to get workspace statistics".to_string(),
            ))
        }
    }
}
