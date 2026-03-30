//! File tree service
//!
//! Provides file tree building, directory scanning, and file search

use crate::util::errors::*;
use log::warn;

use grep_regex::RegexMatcherBuilder;
use grep_searcher::{Searcher, SearcherBuilder, Sink, SinkMatch};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTreeNode {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(rename = "isDirectory")]
    pub is_directory: bool,
    pub children: Option<Vec<FileTreeNode>>,
    pub size: Option<u64>,
    #[serde(rename = "lastModified")]
    pub last_modified: Option<String>,
    pub extension: Option<String>,

    pub depth: Option<u32>,
    pub is_symlink: Option<bool>,
    pub permissions: Option<String>,
    pub mime_type: Option<String>,
    pub git_status: Option<String>,
}

impl FileTreeNode {
    pub fn new(id: String, name: String, path: String, is_directory: bool) -> Self {
        Self {
            id,
            name,
            path,
            is_directory,
            children: None,
            size: None,
            last_modified: None,
            extension: None,
            depth: None,
            is_symlink: None,
            permissions: None,
            mime_type: None,
            git_status: None,
        }
    }

    pub fn with_metadata(mut self, size: Option<u64>, last_modified: Option<String>) -> Self {
        self.size = size;
        self.last_modified = last_modified;
        self
    }

    pub fn with_extension(mut self, extension: Option<String>) -> Self {
        self.extension = extension;
        self
    }

    pub fn with_children(mut self, children: Vec<FileTreeNode>) -> Self {
        self.children = Some(children);
        self
    }

    pub fn with_depth(mut self, depth: u32) -> Self {
        self.depth = Some(depth);
        self
    }

    pub fn with_enhanced_info(
        mut self,
        is_symlink: bool,
        permissions: Option<String>,
        mime_type: Option<String>,
        git_status: Option<String>,
    ) -> Self {
        self.is_symlink = Some(is_symlink);
        self.permissions = permissions;
        self.mime_type = mime_type;
        self.git_status = git_status;
        self
    }
}

/// File tree build options
#[derive(Debug, Clone)]
pub struct FileTreeOptions {
    pub max_depth: Option<u32>,
    pub include_hidden: bool,
    pub include_git_info: bool,
    pub include_mime_types: bool,
    pub skip_patterns: Vec<String>,
    pub max_file_size_mb: Option<u64>,
    pub follow_symlinks: bool,
}

impl Default for FileTreeOptions {
    fn default() -> Self {
        Self {
            max_depth: Some(50),
            include_hidden: false,
            include_git_info: false,
            include_mime_types: false,
            skip_patterns: vec![
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
                "dist".to_string(),
                "build".to_string(),
                ".next".to_string(),
                ".nuxt".to_string(),
                ".cache".to_string(),
                "coverage".to_string(),
                "__pycache__".to_string(),
                ".vscode".to_string(),
                ".idea".to_string(),
            ],
            max_file_size_mb: Some(100),
            follow_symlinks: false,
        }
    }
}

/// File tree statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTreeStatistics {
    pub total_files: usize,
    pub total_directories: usize,
    pub total_size_bytes: u64,
    pub max_depth_reached: u32,
    pub file_type_counts: HashMap<String, usize>,
    pub large_files: Vec<(String, u64)>, // (path, size) for files > 10MB
    pub symlinks_count: usize,
    pub hidden_files_count: usize,
}

pub struct FileTreeService {
    options: FileTreeOptions,
}

fn lock_search_results(
    results: &Arc<Mutex<Vec<FileSearchResult>>>,
) -> std::sync::MutexGuard<'_, Vec<FileSearchResult>> {
    match results.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!("File search results mutex was poisoned, recovering lock");
            poisoned.into_inner()
        }
    }
}

impl Default for FileTreeService {
    fn default() -> Self {
        Self::new(FileTreeOptions::default())
    }
}

impl FileTreeService {
    pub fn new(options: FileTreeOptions) -> Self {
        Self { options }
    }

    pub async fn build_tree(&self, root_path: &str) -> Result<Vec<FileTreeNode>, String> {
        self.build_tree_with_remote_hint(root_path, None).await
    }

    pub async fn build_tree_with_remote_hint(
        &self,
        root_path: &str,
        preferred_remote_connection_id: Option<&str>,
    ) -> Result<Vec<FileTreeNode>, String> {
        // For remote workspaces, delegate to get_directory_contents which handles SSH
        if crate::service::remote_ssh::workspace_state::is_remote_path(root_path).await {
            return self
                .get_directory_contents_with_remote_hint(root_path, preferred_remote_connection_id)
                .await;
        }

        let root_path_buf = PathBuf::from(root_path);

        if !root_path_buf.exists() {
            return Err("Directory does not exist".to_string());
        }

        if !root_path_buf.is_dir() {
            return Err("Path is not a directory".to_string());
        }

        let mut visited = HashSet::new();
        self.build_tree_recursive(&root_path_buf, &root_path_buf, &mut visited, 0)
            .await
    }

    pub async fn build_tree_with_stats(
        &self,
        root_path: &str,
    ) -> BitFunResult<(Vec<FileTreeNode>, FileTreeStatistics)> {
        // For remote workspaces, return simple directory listing with empty stats
        if crate::service::remote_ssh::workspace_state::is_remote_path(root_path).await {
            let nodes = self.get_directory_contents_with_remote_hint(root_path, None).await
                .map_err(|e| BitFunError::service(e))?;
            let stats = FileTreeStatistics {
                total_files: nodes.iter().filter(|n| !n.is_directory).count(),
                total_directories: nodes.iter().filter(|n| n.is_directory).count(),
                total_size_bytes: 0,
                max_depth_reached: 0,
                file_type_counts: HashMap::new(),
                large_files: Vec::new(),
                symlinks_count: 0,
                hidden_files_count: 0,
            };
            return Ok((nodes, stats));
        }

        let root_path_buf = PathBuf::from(root_path);

        if !root_path_buf.exists() {
            return Err(BitFunError::service("Directory does not exist".to_string()));
        }

        if !root_path_buf.is_dir() {
            return Err(BitFunError::service("Path is not a directory".to_string()));
        }

        let mut visited = HashSet::new();
        let mut stats = FileTreeStatistics {
            total_files: 0,
            total_directories: 0,
            total_size_bytes: 0,
            max_depth_reached: 0,
            file_type_counts: HashMap::new(),
            large_files: Vec::new(),
            symlinks_count: 0,
            hidden_files_count: 0,
        };

        let nodes = self
            .build_tree_recursive_with_stats(
                &root_path_buf,
                &root_path_buf,
                &mut visited,
                0,
                &mut stats,
            )
            .await
            .map_err(|e| BitFunError::service(e))?;

        Ok((nodes, stats))
    }

    fn build_tree_recursive<'a>(
        &'a self,
        path: &'a PathBuf,
        root_path: &'a PathBuf,
        visited: &'a mut HashSet<PathBuf>,
        depth: u32,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<FileTreeNode>, String>> + Send + 'a>,
    > {
        Box::pin(async move {
            if let Some(max_depth) = self.options.max_depth {
                if depth > max_depth {
                    return Ok(vec![]);
                }
            }

            // Prevent cycles
            let canonical_path = match path.canonicalize() {
                Ok(p) => p,
                Err(_) => path.clone(),
            };

            if visited.contains(&canonical_path) {
                return Ok(vec![]);
            }
            visited.insert(canonical_path);

            let mut nodes = Vec::new();

            let mut read_dir = fs::read_dir(path)
                .await
                .map_err(|e| format!("Failed to read directory: {}", e))?;

            let mut entries = Vec::new();
            while let Some(entry) = read_dir
                .next_entry()
                .await
                .map_err(|e| format!("Failed to read directory entry: {}", e))?
            {
                entries.push(entry);
            }

            entries.sort_by(|a, b| {
                let a_is_dir = a.path().is_dir();
                let b_is_dir = b.path().is_dir();
                match (a_is_dir, b_is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.file_name().cmp(&b.file_name()),
                }
            });

            for entry in entries {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();

                if self.should_skip_file(&file_name_str) {
                    continue;
                }

                let entry_path = entry.path();
                let relative_path = entry_path
                    .strip_prefix(root_path)
                    .unwrap_or(&entry_path)
                    .to_string_lossy()
                    .to_string();

                let file_type = match entry.file_type().await {
                    Ok(ft) => ft,
                    Err(_) => match std::fs::symlink_metadata(&entry_path) {
                        Ok(metadata) => metadata.file_type(),
                        Err(e) => {
                            warn!(
                                "Failed to get file type, skipping: {} ({})",
                                entry_path.display(),
                                e
                            );
                            continue;
                        }
                    },
                };

                let is_directory = file_type.is_dir();
                let is_symlink = file_type.is_symlink();

                let metadata = entry.metadata().await.ok();
                let size = if is_directory {
                    None
                } else {
                    metadata.as_ref().map(|m| m.len())
                };

                if let (Some(size_bytes), Some(max_mb)) = (size, self.options.max_file_size_mb) {
                    if size_bytes > max_mb * 1024 * 1024 {
                        continue;
                    }
                }

                let last_modified = metadata.and_then(|m| {
                    m.modified().ok().and_then(|t| {
                        let datetime: chrono::DateTime<chrono::Utc> = t.into();
                        Some(datetime.format("%Y-%m-%d %H:%M:%S").to_string())
                    })
                });

                let extension = if !is_directory {
                    entry_path
                        .extension()
                        .map(|ext| ext.to_string_lossy().to_string())
                } else {
                    None
                };

                let mime_type = if self.options.include_mime_types && !is_directory {
                    self.detect_mime_type(&entry_path)
                } else {
                    None
                };

                let permissions = self.get_permissions_string(&entry_path).await;

                let mut node = FileTreeNode::new(
                    relative_path,
                    file_name_str.to_string(),
                    entry_path.to_string_lossy().to_string(),
                    is_directory,
                )
                .with_metadata(size, last_modified)
                .with_extension(extension)
                .with_depth(depth)
                .with_enhanced_info(is_symlink, permissions, mime_type, None);

                if is_directory {
                    if !is_symlink || self.options.follow_symlinks {
                        match self
                            .build_tree_recursive(&entry_path, root_path, visited, depth + 1)
                            .await
                        {
                            Ok(children) => {
                                node = node.with_children(children);
                            }
                            Err(_) => {
                                node = node.with_children(vec![]);
                            }
                        }
                    }
                }

                nodes.push(node);
            }

            Ok(nodes)
        })
    }

    fn build_tree_recursive_with_stats<'a>(
        &'a self,
        path: &'a PathBuf,
        root_path: &'a PathBuf,
        visited: &'a mut HashSet<PathBuf>,
        depth: u32,
        stats: &'a mut FileTreeStatistics,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<FileTreeNode>, String>> + Send + 'a>,
    > {
        Box::pin(async move {
            if depth > stats.max_depth_reached {
                stats.max_depth_reached = depth;
            }

            if let Some(max_depth) = self.options.max_depth {
                if depth > max_depth {
                    return Ok(vec![]);
                }
            }

            // Prevent cycles
            let canonical_path = match path.canonicalize() {
                Ok(p) => p,
                Err(_) => path.clone(),
            };

            if visited.contains(&canonical_path) {
                return Ok(vec![]);
            }
            visited.insert(canonical_path);

            let mut nodes = Vec::new();

            let mut read_dir = fs::read_dir(path)
                .await
                .map_err(|e| format!("Failed to read directory: {}", e))?;

            let mut entries = Vec::new();
            while let Some(entry) = read_dir
                .next_entry()
                .await
                .map_err(|e| format!("Failed to read directory entry: {}", e))?
            {
                entries.push(entry);
            }

            entries.sort_by(|a, b| {
                let a_is_dir = a.path().is_dir();
                let b_is_dir = b.path().is_dir();
                match (a_is_dir, b_is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.file_name().cmp(&b.file_name()),
                }
            });

            for entry in entries {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();

                if file_name_str.starts_with('.') {
                    stats.hidden_files_count += 1;
                }

                if self.should_skip_file(&file_name_str) {
                    continue;
                }

                let entry_path = entry.path();
                let relative_path = entry_path
                    .strip_prefix(root_path)
                    .unwrap_or(&entry_path)
                    .to_string_lossy()
                    .to_string();

                let file_type = match entry.file_type().await {
                    Ok(ft) => ft,
                    Err(_) => match std::fs::symlink_metadata(&entry_path) {
                        Ok(metadata) => metadata.file_type(),
                        Err(e) => {
                            warn!(
                                "Failed to get file type, skipping: {} ({})",
                                entry_path.display(),
                                e
                            );
                            continue;
                        }
                    },
                };

                let is_directory = file_type.is_dir();
                let is_symlink = file_type.is_symlink();

                if is_directory {
                    stats.total_directories += 1;
                } else {
                    stats.total_files += 1;
                }

                if is_symlink {
                    stats.symlinks_count += 1;
                }

                let metadata = entry.metadata().await.ok();
                let size = if is_directory {
                    None
                } else {
                    metadata.as_ref().map(|m| m.len())
                };

                if let Some(file_size) = size {
                    stats.total_size_bytes += file_size;

                    if file_size > 10 * 1024 * 1024 {
                        stats
                            .large_files
                            .push((entry_path.to_string_lossy().to_string(), file_size));
                    }
                }

                if let (Some(size_bytes), Some(max_mb)) = (size, self.options.max_file_size_mb) {
                    if size_bytes > max_mb * 1024 * 1024 {
                        continue;
                    }
                }

                if !is_directory {
                    if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
                        *stats.file_type_counts.entry(ext.to_string()).or_insert(0) += 1;
                    } else {
                        *stats
                            .file_type_counts
                            .entry("no_extension".to_string())
                            .or_insert(0) += 1;
                    }
                }

                let last_modified = metadata.and_then(|m| {
                    m.modified().ok().and_then(|t| {
                        let datetime: chrono::DateTime<chrono::Utc> = t.into();
                        Some(datetime.format("%Y-%m-%d %H:%M:%S").to_string())
                    })
                });

                let extension = if !is_directory {
                    entry_path
                        .extension()
                        .map(|ext| ext.to_string_lossy().to_string())
                } else {
                    None
                };

                let mime_type = if self.options.include_mime_types && !is_directory {
                    self.detect_mime_type(&entry_path)
                } else {
                    None
                };

                let permissions = self.get_permissions_string(&entry_path).await;

                let mut node = FileTreeNode::new(
                    relative_path,
                    file_name_str.to_string(),
                    entry_path.to_string_lossy().to_string(),
                    is_directory,
                )
                .with_metadata(size, last_modified)
                .with_extension(extension)
                .with_depth(depth)
                .with_enhanced_info(is_symlink, permissions, mime_type, None);

                if is_directory {
                    if !is_symlink || self.options.follow_symlinks {
                        match self
                            .build_tree_recursive_with_stats(
                                &entry_path,
                                root_path,
                                visited,
                                depth + 1,
                                stats,
                            )
                            .await
                        {
                            Ok(children) => {
                                node = node.with_children(children);
                            }
                            Err(_) => {
                                node = node.with_children(vec![]);
                            }
                        }
                    }
                }

                nodes.push(node);
            }

            Ok(nodes)
        })
    }

    fn should_skip_file(&self, file_name: &str) -> bool {
        // Skip hidden files and directories (unless explicitly included)
        // But .gitignore and .bitfun are always shown
        if !self.options.include_hidden
            && file_name.starts_with('.')
            && file_name != ".gitignore"
            && file_name != ".bitfun"
        {
            return true;
        }

        self.options.skip_patterns.iter().any(|pattern| {
            if pattern.contains('*') {
                let parts: Vec<&str> = pattern.split('*').collect();
                if parts.len() == 2 {
                    file_name.starts_with(parts[0]) && file_name.ends_with(parts[1])
                } else {
                    file_name.contains(pattern.trim_matches('*'))
                }
            } else {
                file_name == pattern
            }
        })
    }

    pub async fn get_directory_contents(&self, path: &str) -> Result<Vec<FileTreeNode>, String> {
        self.get_directory_contents_with_remote_hint(path, None).await
    }

    /// `preferred_remote_connection_id`: when set (e.g. from workspace/session), resolves SSH file ops
    /// without relying on global `active_connection_hint` — required when multiple remotes share the same root path.
    pub async fn get_directory_contents_with_remote_hint(
        &self,
        path: &str,
        preferred_remote_connection_id: Option<&str>,
    ) -> Result<Vec<FileTreeNode>, String> {
        // Check if this path belongs to any registered remote workspace
        if let Some(entry) = crate::service::remote_ssh::workspace_state::lookup_remote_connection_with_hint(
            path,
            preferred_remote_connection_id,
        )
        .await
        {
            if let Some(manager) = crate::service::remote_ssh::workspace_state::get_remote_workspace_manager() {
                if let Some(file_service) = manager.get_file_service().await {
                    match file_service.read_dir(&entry.connection_id, path).await {
                        Ok(entries) => {
                            let nodes: Vec<FileTreeNode> = entries
                                .into_iter()
                                .filter(|e| e.name != "." && e.name != "..")
                                .map(|e| {
                                    FileTreeNode::new(
                                        e.path.clone(),
                                        e.name.clone(),
                                        e.path.clone(),
                                        e.is_dir,
                                    )
                                })
                                .collect();
                            return Ok(nodes);
                        }
                        Err(e) => {
                            return Err(format!("Failed to read remote directory: {}", e));
                        }
                    }
                }
            }
        }

        // Fall back to local filesystem
        let path_buf = PathBuf::from(path);

        if !path_buf.exists() {
            return Err("Directory does not exist".to_string());
        }

        if !path_buf.is_dir() {
            return Err("Path is not a directory".to_string());
        }

        let mut nodes = Vec::new();

        let mut read_dir = fs::read_dir(&path_buf)
            .await
            .map_err(|e| format!("Failed to read directory: {}", e))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| format!("Failed to read directory entry: {}", e))?
        {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if self.should_skip_file(&file_name_str) {
                continue;
            }

            let entry_path = entry.path();
            let is_directory = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);

            let node = FileTreeNode::new(
                entry_path.to_string_lossy().to_string(),
                file_name_str.to_string(),
                entry_path.to_string_lossy().to_string(),
                is_directory,
            );

            nodes.push(node);
        }

        Ok(nodes)
    }

    fn detect_mime_type(&self, path: &Path) -> Option<String> {
        if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
            match extension.to_lowercase().as_str() {
                "txt" | "md" | "rst" => Some("text/plain".to_string()),
                "html" | "htm" => Some("text/html".to_string()),
                "css" => Some("text/css".to_string()),
                "js" => Some("application/javascript".to_string()),
                "json" => Some("application/json".to_string()),
                "xml" => Some("application/xml".to_string()),
                "yaml" | "yml" => Some("application/yaml".to_string()),

                "rs" => Some("text/rust".to_string()),
                "py" => Some("text/python".to_string()),
                "java" => Some("text/java".to_string()),
                "cpp" | "cc" | "cxx" => Some("text/cpp".to_string()),
                "c" => Some("text/c".to_string()),
                "h" | "hpp" => Some("text/c-header".to_string()),
                "go" => Some("text/go".to_string()),
                "php" => Some("text/php".to_string()),
                "rb" => Some("text/ruby".to_string()),
                "ts" => Some("application/typescript".to_string()),

                "png" => Some("image/png".to_string()),
                "jpg" | "jpeg" => Some("image/jpeg".to_string()),
                "gif" => Some("image/gif".to_string()),
                "svg" => Some("image/svg+xml".to_string()),
                "webp" => Some("image/webp".to_string()),

                "pdf" => Some("application/pdf".to_string()),
                "doc" | "docx" => Some("application/msword".to_string()),
                "xls" | "xlsx" => Some("application/excel".to_string()),
                "ppt" | "pptx" => Some("application/powerpoint".to_string()),

                "zip" => Some("application/zip".to_string()),
                "tar" => Some("application/tar".to_string()),
                "gz" => Some("application/gzip".to_string()),
                "rar" => Some("application/rar".to_string()),

                _ => None,
            }
        } else {
            None
        }
    }

    async fn get_permissions_string(&self, path: &Path) -> Option<String> {
        if let Ok(metadata) = fs::metadata(path).await {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = metadata.permissions();
                let mode = perms.mode();

                let user = format!(
                    "{}{}{}",
                    if mode & 0o400 != 0 { "r" } else { "-" },
                    if mode & 0o200 != 0 { "w" } else { "-" },
                    if mode & 0o100 != 0 { "x" } else { "-" }
                );
                let group = format!(
                    "{}{}{}",
                    if mode & 0o040 != 0 { "r" } else { "-" },
                    if mode & 0o020 != 0 { "w" } else { "-" },
                    if mode & 0o010 != 0 { "x" } else { "-" }
                );
                let other = format!(
                    "{}{}{}",
                    if mode & 0o004 != 0 { "r" } else { "-" },
                    if mode & 0o002 != 0 { "w" } else { "-" },
                    if mode & 0o001 != 0 { "x" } else { "-" }
                );

                Some(format!("{}{}{}", user, group, other))
            }

            #[cfg(windows)]
            {
                let readonly = metadata.permissions().readonly();
                Some(if readonly { "r--" } else { "rw-" }.to_string())
            }
        } else {
            None
        }
    }

    pub async fn search_files(
        &self,
        root_path: &str,
        pattern: &str,
        search_content: bool,
    ) -> BitFunResult<Vec<FileSearchResult>> {
        self.search_files_with_options(
            root_path,
            pattern,
            search_content,
            false, // case_sensitive
            false, // regex
            false, // whole_word
        )
        .await
    }

    pub async fn search_files_with_options(
        &self,
        root_path: &str,
        pattern: &str,
        search_content: bool,
        case_sensitive: bool,
        use_regex: bool,
        whole_word: bool,
    ) -> BitFunResult<Vec<FileSearchResult>> {
        let root_path_buf = PathBuf::from(root_path);

        if !root_path_buf.exists() {
            return Err(BitFunError::service("Directory does not exist".to_string()));
        }

        let max_results = 10000;

        let filename_pattern = if use_regex {
            pattern.to_string()
        } else if whole_word {
            format!(r"\b{}\b", regex::escape(pattern))
        } else {
            regex::escape(pattern)
        };

        let results = Arc::new(Mutex::new(Vec::new()));
        let should_stop = Arc::new(AtomicBool::new(false));

        let pattern = pattern.to_string();
        let filename_pattern = Arc::new(filename_pattern);

        let walker = WalkBuilder::new(&root_path_buf)
            .hidden(false)
            .ignore(true)
            .git_ignore(true)
            .git_global(false)
            .git_exclude(false)
            .threads(
                std::thread::available_parallelism()
                    .map(|count| count.get())
                    .unwrap_or(1)
                    .min(8),
            )
            .build_parallel();

        walker.run(|| {
            let results = Arc::clone(&results);
            let should_stop = Arc::clone(&should_stop);
            let pattern = pattern.clone();
            let filename_pattern = Arc::clone(&filename_pattern);
            let root_path_buf = root_path_buf.clone();

            Box::new(move |entry| {
                if should_stop.load(Ordering::Relaxed) {
                    return ignore::WalkState::Quit;
                }

                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => return ignore::WalkState::Continue,
                };

                let path = entry.path();
                let is_dir = path.is_dir();
                let is_file = path.is_file();

                if path == root_path_buf {
                    return ignore::WalkState::Continue;
                }

                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                if is_dir {
                    let dir_matches = if case_sensitive {
                        if use_regex || whole_word {
                            regex::Regex::new(&filename_pattern)
                                .map(|re| re.is_match(&file_name))
                                .unwrap_or(false)
                        } else {
                            file_name.contains(&pattern)
                        }
                    } else {
                        if use_regex || whole_word {
                            regex::RegexBuilder::new(&filename_pattern)
                                .case_insensitive(true)
                                .build()
                                .map(|re| re.is_match(&file_name))
                                .unwrap_or(false)
                        } else {
                            file_name.to_lowercase().contains(&pattern.to_lowercase())
                        }
                    };

                    if dir_matches {
                        let mut results_guard = lock_search_results(&results);
                        if results_guard.len() < max_results {
                            results_guard.push(FileSearchResult {
                                path: path.to_string_lossy().to_string(),
                                name: file_name.clone(),
                                is_directory: true,
                                match_type: SearchMatchType::FileName,
                                line_number: None,
                                matched_content: None,
                            });
                        }

                        if results_guard.len() >= max_results {
                            should_stop.store(true, Ordering::Relaxed);
                            return ignore::WalkState::Quit;
                        }
                    }

                    return ignore::WalkState::Continue;
                }

                if !is_file {
                    return ignore::WalkState::Continue;
                }

                if let Some(file_name) = path.file_name() {
                    let file_name_str = file_name.to_string_lossy();
                    if Self::should_skip_file_static(&file_name_str)
                        || Self::is_binary_file_static(&file_name_str)
                    {
                        return ignore::WalkState::Continue;
                    }
                }

                if let Ok(metadata) = path.metadata() {
                    let max_size = 10 * 1024 * 1024;
                    if metadata.len() > max_size {
                        return ignore::WalkState::Continue;
                    }
                }

                let filename_matches = if case_sensitive {
                    if use_regex || whole_word {
                        regex::Regex::new(&filename_pattern)
                            .map(|re| re.is_match(&file_name))
                            .unwrap_or(false)
                    } else {
                        file_name.contains(&pattern)
                    }
                } else {
                    if use_regex || whole_word {
                        regex::RegexBuilder::new(&filename_pattern)
                            .case_insensitive(true)
                            .build()
                            .map(|re| re.is_match(&file_name))
                            .unwrap_or(false)
                    } else {
                        file_name.to_lowercase().contains(&pattern.to_lowercase())
                    }
                };

                if filename_matches {
                    let mut results_guard = lock_search_results(&results);
                    if results_guard.len() < max_results {
                        results_guard.push(FileSearchResult {
                            path: path.to_string_lossy().to_string(),
                            name: file_name.clone(),
                            is_directory: false,
                            match_type: SearchMatchType::FileName,
                            line_number: None,
                            matched_content: None,
                        });
                    }

                    if results_guard.len() >= max_results {
                        should_stop.store(true, Ordering::Relaxed);
                        return ignore::WalkState::Quit;
                    }
                }

                if search_content {
                    let results_len = lock_search_results(&results).len();
                    if results_len < max_results {
                        if let Err(e) = Self::search_file_content_static(
                            path,
                            &file_name,
                            &pattern,
                            case_sensitive,
                            use_regex,
                            whole_word,
                            Arc::clone(&results),
                            max_results,
                            Arc::clone(&should_stop),
                        ) {
                            warn!("Failed to search file content {}: {}", path.display(), e);
                        }

                        let results_len = lock_search_results(&results).len();
                        if results_len >= max_results {
                            should_stop.store(true, Ordering::Relaxed);
                            return ignore::WalkState::Quit;
                        }
                    }
                }

                ignore::WalkState::Continue
            })
        });

        let final_results = lock_search_results(&results).clone();

        Ok(final_results)
    }

    fn search_file_content_static(
        path: &Path,
        file_name: &str,
        pattern: &str,
        case_sensitive: bool,
        use_regex: bool,
        whole_word: bool,
        results: Arc<Mutex<Vec<FileSearchResult>>>,
        max_results: usize,
        should_stop: Arc<AtomicBool>,
    ) -> BitFunResult<()> {
        if should_stop.load(Ordering::Relaxed) {
            return Ok(());
        }

        let search_pattern = if use_regex {
            pattern.to_string()
        } else if whole_word {
            format!(r"\b{}\b", regex::escape(pattern))
        } else {
            regex::escape(pattern)
        };

        let matcher = RegexMatcherBuilder::new()
            .case_insensitive(!case_sensitive)
            .build(&search_pattern)
            .map_err(|e| BitFunError::service(format!("Invalid regex pattern: {}", e)))?;

        let mut searcher = SearcherBuilder::new().line_number(true).build();

        let mut sink = FileContentSinkThreadSafe {
            path: path.to_path_buf(),
            file_name: file_name.to_string(),
            results,
            max_results,
            should_stop,
        };

        searcher
            .search_path(&matcher, path, &mut sink)
            .map_err(|e| BitFunError::service(format!("Search error: {}", e)))?;

        Ok(())
    }

    fn should_skip_file_static(file_name: &str) -> bool {
        let skip_patterns = [
            "node_modules",
            ".git",
            ".svn",
            ".hg",
            "target",
            "build",
            "dist",
            "out",
            ".DS_Store",
            "Thumbs.db",
        ];

        skip_patterns
            .iter()
            .any(|pattern| file_name.contains(pattern))
    }

    fn is_binary_file_static(file_name: &str) -> bool {
        let binary_extensions = [
            ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".ico", ".svg", ".webp", ".mp4", ".avi",
            ".mov", ".wmv", ".flv", ".mkv", ".mp3", ".wav", ".flac", ".aac", ".ogg", ".zip",
            ".tar", ".gz", ".7z", ".rar", ".bz2", ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt",
            ".pptx", ".woff", ".woff2", ".ttf", ".otf", ".eot", ".exe", ".dll", ".so", ".dylib",
            ".bin", ".pyc", ".class", ".o", ".a", ".lib",
        ];

        let lower_name = file_name.to_lowercase();
        binary_extensions
            .iter()
            .any(|ext| lower_name.ends_with(ext))
    }
}

struct FileContentSinkThreadSafe {
    path: PathBuf,
    file_name: String,
    results: Arc<Mutex<Vec<FileSearchResult>>>,
    max_results: usize,
    should_stop: Arc<AtomicBool>,
}

impl Sink for FileContentSinkThreadSafe {
    type Error = std::io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        if self.should_stop.load(Ordering::Relaxed) {
            return Ok(false);
        }

        let mut results = lock_search_results(&self.results);

        if results.len() >= self.max_results {
            self.should_stop.store(true, Ordering::Relaxed);
            return Ok(false);
        }

        let line_number = mat.line_number().unwrap_or(0) as usize;
        let matched_line = String::from_utf8_lossy(mat.bytes()).trim_end().to_string();

        results.push(FileSearchResult {
            path: self.path.to_string_lossy().to_string(),
            name: self.file_name.clone(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: Some(line_number),
            matched_content: Some(matched_line),
        });

        let should_continue = results.len() < self.max_results;
        if !should_continue {
            self.should_stop.store(true, Ordering::Relaxed);
        }

        Ok(should_continue)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchResult {
    pub path: String,
    pub name: String,
    pub is_directory: bool,
    pub match_type: SearchMatchType,
    pub line_number: Option<usize>,
    pub matched_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchMatchType {
    FileName,
    Content,
}
