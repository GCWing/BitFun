//! File tree service
//!
//! Provides file tree building, directory scanning, and file search

use crate::util::errors::*;
use log::warn;

use ignore::WalkBuilder;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
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

fn cancellation_requested(cancel_flag: Option<&Arc<AtomicBool>>) -> bool {
    cancel_flag
        .map(|flag| flag.load(Ordering::Relaxed))
        .unwrap_or(false)
}

#[derive(Debug, Clone)]
pub struct FileNameSearchOptions {
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub whole_word: bool,
    pub max_results: usize,
    pub include_directories: bool,
    pub cancel_flag: Option<Arc<AtomicBool>>,
}

impl Default for FileNameSearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            use_regex: false,
            whole_word: false,
            max_results: 10_000,
            include_directories: true,
            cancel_flag: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileContentSearchOptions {
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub whole_word: bool,
    pub max_results: usize,
    pub max_file_size_bytes: u64,
    pub cancel_flag: Option<Arc<AtomicBool>>,
}

impl Default for FileContentSearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            use_regex: false,
            whole_word: false,
            max_results: 10_000,
            max_file_size_bytes: 10 * 1024 * 1024,
            cancel_flag: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchOutcome {
    pub results: Vec<FileSearchResult>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchResultGroup {
    pub path: String,
    pub name: String,
    pub is_directory: bool,
    pub file_name_match: Option<FileSearchResult>,
    pub content_matches: Vec<FileSearchResult>,
}

pub trait FileSearchProgressSink: Send + Sync {
    fn report(&self, result: FileSearchResultGroup);
    fn flush(&self);
}

pub struct BatchedFileSearchProgressSink {
    batch: Mutex<Vec<FileSearchResultGroup>>,
    batch_size: usize,
    flush_interval: Duration,
    last_flush_at: Mutex<Instant>,
    on_flush: Box<dyn Fn(Vec<FileSearchResultGroup>) + Send + Sync>,
}

impl BatchedFileSearchProgressSink {
    pub fn new<F>(batch_size: usize, flush_interval: Duration, on_flush: F) -> Self
    where
        F: Fn(Vec<FileSearchResultGroup>) + Send + Sync + 'static,
    {
        Self {
            batch: Mutex::new(Vec::new()),
            batch_size: batch_size.max(1),
            flush_interval,
            last_flush_at: Mutex::new(Instant::now() - flush_interval),
            on_flush: Box::new(on_flush),
        }
    }

    fn drain_batch(&self) -> Vec<FileSearchResultGroup> {
        match self.batch.lock() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(poisoned) => {
                warn!("File search progress batch mutex was poisoned, recovering lock");
                let mut guard = poisoned.into_inner();
                std::mem::take(&mut *guard)
            }
        }
    }

    fn elapsed_since_last_flush(&self) -> Duration {
        match self.last_flush_at.lock() {
            Ok(guard) => guard.elapsed(),
            Err(poisoned) => {
                warn!("File search progress flush timer mutex was poisoned, recovering lock");
                poisoned.into_inner().elapsed()
            }
        }
    }

    fn mark_flushed(&self) {
        match self.last_flush_at.lock() {
            Ok(mut guard) => {
                *guard = Instant::now();
            }
            Err(poisoned) => {
                warn!("File search progress flush timer mutex was poisoned, recovering lock");
                let mut guard = poisoned.into_inner();
                *guard = Instant::now();
            }
        }
    }

    fn flush_internal(&self, force: bool) {
        let should_flush = force || self.elapsed_since_last_flush() >= self.flush_interval;
        if !should_flush {
            return;
        }

        let batch = self.drain_batch();
        if batch.is_empty() {
            return;
        }

        (self.on_flush)(batch);
        self.mark_flushed();
    }
}

impl FileSearchProgressSink for BatchedFileSearchProgressSink {
    fn report(&self, result: FileSearchResultGroup) {
        let should_flush_now = match self.batch.lock() {
            Ok(mut guard) => {
                guard.push(result);
                guard.len() >= self.batch_size
            }
            Err(poisoned) => {
                warn!("File search progress batch mutex was poisoned, recovering lock");
                let mut guard = poisoned.into_inner();
                guard.push(result);
                guard.len() >= self.batch_size
            }
        };

        if should_flush_now {
            self.flush_internal(true);
            return;
        }

        self.flush_internal(false);
    }

    fn flush(&self) {
        self.flush_internal(true);
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
            let nodes = self
                .get_directory_contents_with_remote_hint(root_path, None)
                .await
                .map_err(BitFunError::service)?;
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
            .map_err(BitFunError::service)?;

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
                    m.modified().ok().map(|t| {
                        let datetime: chrono::DateTime<chrono::Utc> = t.into();
                        datetime.format("%Y-%m-%d %H:%M:%S").to_string()
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

                if is_directory && (!is_symlink || self.options.follow_symlinks) {
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
                    m.modified().ok().map(|t| {
                        let datetime: chrono::DateTime<chrono::Utc> = t.into();
                        datetime.format("%Y-%m-%d %H:%M:%S").to_string()
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

                if is_directory && (!is_symlink || self.options.follow_symlinks) {
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
        self.get_directory_contents_with_remote_hint(path, None)
            .await
    }

    /// `preferred_remote_connection_id`: when set (e.g. from workspace/session), resolves SSH file ops
    /// without relying on global `active_connection_hint` — required when multiple remotes share the same root path.
    pub async fn get_directory_contents_with_remote_hint(
        &self,
        path: &str,
        preferred_remote_connection_id: Option<&str>,
    ) -> Result<Vec<FileTreeNode>, String> {
        // Check if this path belongs to any registered remote workspace
        if let Some(entry) =
            crate::service::remote_ssh::workspace_state::lookup_remote_connection_with_hint(
                path,
                preferred_remote_connection_id,
            )
            .await
        {
            if let Some(manager) =
                crate::service::remote_ssh::workspace_state::get_remote_workspace_manager()
            {
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
        self.search_files_with_options(root_path, pattern, search_content, false, false, false)
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
        let filename_outcome = self
            .search_file_names(
                root_path,
                pattern,
                FileNameSearchOptions {
                    case_sensitive,
                    use_regex,
                    whole_word,
                    max_results: 10_000,
                    include_directories: true,
                    cancel_flag: None,
                },
            )
            .await?;
        let mut results = filename_outcome.results;

        if search_content && !filename_outcome.truncated && results.len() < 10_000 {
            let remaining = 10_000 - results.len();
            let mut content_outcome = self
                .search_file_contents(
                    root_path,
                    pattern,
                    FileContentSearchOptions {
                        case_sensitive,
                        use_regex,
                        whole_word,
                        max_results: remaining,
                        max_file_size_bytes: 10 * 1024 * 1024,
                        cancel_flag: None,
                    },
                )
                .await?;
            results.append(&mut content_outcome.results);
        }

        Ok(results)
    }

    pub async fn search_file_names(
        &self,
        root_path: &str,
        pattern: &str,
        options: FileNameSearchOptions,
    ) -> BitFunResult<FileSearchOutcome> {
        self.search_file_names_with_progress(root_path, pattern, options, None)
            .await
    }

    pub async fn search_file_names_with_progress(
        &self,
        root_path: &str,
        pattern: &str,
        options: FileNameSearchOptions,
        progress_sink: Option<Arc<dyn FileSearchProgressSink>>,
    ) -> BitFunResult<FileSearchOutcome> {
        let root_path_buf = PathBuf::from(root_path);

        if !root_path_buf.exists() {
            return Err(BitFunError::service("Directory does not exist".to_string()));
        }

        let matcher = Arc::new(Self::compile_search_regex(
            pattern,
            options.case_sensitive,
            options.use_regex,
            options.whole_word,
        )?);
        let results = Arc::new(Mutex::new(Vec::new()));
        let should_stop = Arc::new(AtomicBool::new(false));
        let limit_reached = Arc::new(AtomicBool::new(false));
        let cancel_flag = options.cancel_flag.clone();
        let include_directories = options.include_directories;
        let max_results = options.max_results.max(1);
        let progress_sink_for_walker = progress_sink.clone();

        let walker = Self::build_search_walker(&root_path_buf);

        walker.run(|| {
            let matcher = Arc::clone(&matcher);
            let results = Arc::clone(&results);
            let should_stop = Arc::clone(&should_stop);
            let limit_reached = Arc::clone(&limit_reached);
            let root_path_buf = root_path_buf.clone();
            let cancel_flag = cancel_flag.clone();
            let progress_sink = progress_sink_for_walker.clone();

            Box::new(move |entry| {
                if should_stop.load(Ordering::Relaxed)
                    || cancellation_requested(cancel_flag.as_ref())
                {
                    should_stop.store(true, Ordering::Relaxed);
                    return ignore::WalkState::Quit;
                }

                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => return ignore::WalkState::Continue,
                };

                let path = entry.path();
                if path == root_path_buf {
                    return ignore::WalkState::Continue;
                }

                let file_name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default();
                let file_type = entry.file_type();

                if file_type.map(|kind| kind.is_dir()).unwrap_or(false) {
                    if Self::should_skip_directory_static(&file_name) {
                        return ignore::WalkState::Skip;
                    }

                    if include_directories
                        && matcher.is_match(&file_name)
                        && !Self::push_search_result_group(
                            &results,
                            &should_stop,
                            &limit_reached,
                            max_results,
                            progress_sink.as_ref(),
                            vec![FileSearchResult {
                                path: path.to_string_lossy().to_string(),
                                name: file_name,
                                is_directory: true,
                                match_type: SearchMatchType::FileName,
                                line_number: None,
                                matched_content: None,
                                preview_before: None,
                                preview_inside: None,
                                preview_after: None,
                            }],
                        )
                    {
                        return ignore::WalkState::Quit;
                    }

                    return ignore::WalkState::Continue;
                }

                if !file_type.map(|kind| kind.is_file()).unwrap_or(false) {
                    return ignore::WalkState::Continue;
                }

                if Self::should_skip_file_static(&file_name)
                    || Self::is_binary_file_static(&file_name)
                {
                    return ignore::WalkState::Continue;
                }

                if matcher.is_match(&file_name)
                    && !Self::push_search_result_group(
                        &results,
                        &should_stop,
                        &limit_reached,
                        max_results,
                        progress_sink.as_ref(),
                        vec![FileSearchResult {
                            path: path.to_string_lossy().to_string(),
                            name: file_name,
                            is_directory: false,
                            match_type: SearchMatchType::FileName,
                            line_number: None,
                            matched_content: None,
                            preview_before: None,
                            preview_inside: None,
                            preview_after: None,
                        }],
                    )
                {
                    return ignore::WalkState::Quit;
                }

                ignore::WalkState::Continue
            })
        });

        if let Some(progress_sink) = progress_sink {
            progress_sink.flush();
        }

        let final_results = lock_search_results(&results).clone();
        Ok(FileSearchOutcome {
            results: final_results,
            truncated: limit_reached.load(Ordering::Relaxed),
        })
    }

    pub async fn search_file_contents(
        &self,
        root_path: &str,
        pattern: &str,
        options: FileContentSearchOptions,
    ) -> BitFunResult<FileSearchOutcome> {
        self.search_file_contents_with_progress(root_path, pattern, options, None)
            .await
    }

    pub async fn search_file_contents_with_progress(
        &self,
        root_path: &str,
        pattern: &str,
        options: FileContentSearchOptions,
        progress_sink: Option<Arc<dyn FileSearchProgressSink>>,
    ) -> BitFunResult<FileSearchOutcome> {
        let root_path_buf = PathBuf::from(root_path);

        if !root_path_buf.exists() {
            return Err(BitFunError::service("Directory does not exist".to_string()));
        }

        let matcher = Arc::new(Self::compile_search_regex(
            pattern,
            options.case_sensitive,
            options.use_regex,
            options.whole_word,
        )?);
        let results = Arc::new(Mutex::new(Vec::new()));
        let should_stop = Arc::new(AtomicBool::new(false));
        let limit_reached = Arc::new(AtomicBool::new(false));
        let cancel_flag = options.cancel_flag.clone();
        let max_results = options.max_results.max(1);
        let max_file_size_bytes = options.max_file_size_bytes;
        let progress_sink_for_walker = progress_sink.clone();

        let walker = Self::build_search_walker(&root_path_buf);

        walker.run(|| {
            let matcher = Arc::clone(&matcher);
            let results = Arc::clone(&results);
            let should_stop = Arc::clone(&should_stop);
            let limit_reached = Arc::clone(&limit_reached);
            let root_path_buf = root_path_buf.clone();
            let cancel_flag = cancel_flag.clone();
            let progress_sink = progress_sink_for_walker.clone();

            Box::new(move |entry| {
                if should_stop.load(Ordering::Relaxed)
                    || cancellation_requested(cancel_flag.as_ref())
                {
                    should_stop.store(true, Ordering::Relaxed);
                    return ignore::WalkState::Quit;
                }

                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => return ignore::WalkState::Continue,
                };

                let path = entry.path();
                if path == root_path_buf {
                    return ignore::WalkState::Continue;
                }

                let file_name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default();
                let file_type = entry.file_type();

                if file_type.map(|kind| kind.is_dir()).unwrap_or(false) {
                    return if Self::should_skip_directory_static(&file_name) {
                        ignore::WalkState::Skip
                    } else {
                        ignore::WalkState::Continue
                    };
                }

                if !file_type.map(|kind| kind.is_file()).unwrap_or(false) {
                    return ignore::WalkState::Continue;
                }

                if Self::should_skip_file_static(&file_name)
                    || Self::is_binary_file_static(&file_name)
                {
                    return ignore::WalkState::Continue;
                }

                if let Ok(metadata) = path.metadata() {
                    if metadata.len() > max_file_size_bytes {
                        return ignore::WalkState::Continue;
                    }
                }

                if let Err(error) = Self::search_file_content_lines(
                    path,
                    &file_name,
                    matcher.as_ref(),
                    &results,
                    max_results,
                    &should_stop,
                    &limit_reached,
                    cancel_flag.as_ref(),
                    progress_sink.as_ref(),
                ) {
                    warn!(
                        "Failed to search file content {}: {}",
                        path.display(),
                        error
                    );
                }

                if should_stop.load(Ordering::Relaxed) {
                    ignore::WalkState::Quit
                } else {
                    ignore::WalkState::Continue
                }
            })
        });

        if let Some(progress_sink) = progress_sink {
            progress_sink.flush();
        }

        let final_results = lock_search_results(&results).clone();
        Ok(FileSearchOutcome {
            results: final_results,
            truncated: limit_reached.load(Ordering::Relaxed),
        })
    }

    fn build_search_walker(root_path: &Path) -> ignore::WalkParallel {
        WalkBuilder::new(root_path)
            .hidden(false)
            .ignore(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .threads(
                std::thread::available_parallelism()
                    .map(|count| count.get())
                    .unwrap_or(1)
                    .min(8),
            )
            .build_parallel()
    }

    fn compile_search_regex(
        pattern: &str,
        case_sensitive: bool,
        use_regex: bool,
        whole_word: bool,
    ) -> BitFunResult<Regex> {
        let search_pattern = if use_regex {
            pattern.to_string()
        } else if whole_word {
            format!(r"\b{}\b", regex::escape(pattern))
        } else {
            regex::escape(pattern)
        };

        RegexBuilder::new(&search_pattern)
            .case_insensitive(!case_sensitive)
            .build()
            .map_err(|error| BitFunError::service(format!("Invalid regex pattern: {}", error)))
    }

    fn take_first_chars(text: &str, max_chars: usize) -> String {
        if max_chars == 0 {
            return String::new();
        }

        let mut end_index = text.len();
        let mut char_count = 0;
        for (byte_index, _) in text.char_indices() {
            if char_count == max_chars {
                end_index = byte_index;
                break;
            }
            char_count += 1;
        }

        text[..end_index].to_string()
    }

    fn left_truncate_with_ellipsis(text: &str, max_chars: usize) -> String {
        let total_chars = text.chars().count();
        if total_chars <= max_chars {
            return text.to_string();
        }

        if max_chars <= 1 {
            return "…".to_string();
        }

        let keep_chars = max_chars - 1;
        let start_index = text
            .char_indices()
            .nth(total_chars.saturating_sub(keep_chars))
            .map(|(index, _)| index)
            .unwrap_or(0);

        format!("…{}", &text[start_index..])
    }

    fn build_content_match_preview(
        line: &str,
        matcher: &Regex,
    ) -> (Option<String>, Option<String>, Option<String>) {
        const MAX_PREVIEW_CHARS: usize = 250;
        const MAX_PREVIEW_BEFORE_CHARS: usize = 26;

        let Some(found_match) = matcher.find(line) else {
            return (None, None, None);
        };

        let full_before = &line[..found_match.start()];
        let before = Self::left_truncate_with_ellipsis(full_before, MAX_PREVIEW_BEFORE_CHARS);

        let mut chars_remaining = MAX_PREVIEW_CHARS.saturating_sub(before.chars().count());
        let mut inside = Self::take_first_chars(found_match.as_str(), chars_remaining);
        chars_remaining = chars_remaining.saturating_sub(inside.chars().count());
        let after = Self::take_first_chars(&line[found_match.end()..], chars_remaining);

        if inside.is_empty() {
            inside = found_match.as_str().to_string();
        }

        (Some(before), Some(inside), Some(after))
    }

    fn build_search_result_group(results: Vec<FileSearchResult>) -> Option<FileSearchResultGroup> {
        let first = results.first()?.clone();
        let file_name_match = results
            .iter()
            .find(|result| matches!(result.match_type, SearchMatchType::FileName))
            .cloned();
        let content_matches = results
            .iter()
            .filter(|result| matches!(result.match_type, SearchMatchType::Content))
            .cloned()
            .collect();

        Some(FileSearchResultGroup {
            path: first.path,
            name: first.name,
            is_directory: first.is_directory,
            file_name_match,
            content_matches,
        })
    }

    fn push_search_result_group(
        results: &Arc<Mutex<Vec<FileSearchResult>>>,
        should_stop: &Arc<AtomicBool>,
        limit_reached: &Arc<AtomicBool>,
        max_results: usize,
        progress_sink: Option<&Arc<dyn FileSearchProgressSink>>,
        group_results: Vec<FileSearchResult>,
    ) -> bool {
        if group_results.is_empty() {
            return true;
        }

        let mut results_guard = lock_search_results(results);
        if results_guard.len() >= max_results {
            should_stop.store(true, Ordering::Relaxed);
            limit_reached.store(true, Ordering::Relaxed);
            return false;
        }

        let remaining_capacity = max_results.saturating_sub(results_guard.len());
        if remaining_capacity == 0 {
            should_stop.store(true, Ordering::Relaxed);
            limit_reached.store(true, Ordering::Relaxed);
            return false;
        }

        let accepted_results = if group_results.len() > remaining_capacity {
            limit_reached.store(true, Ordering::Relaxed);
            group_results
                .into_iter()
                .take(remaining_capacity)
                .collect::<Vec<_>>()
        } else {
            group_results
        };

        results_guard.extend(accepted_results.iter().cloned());
        if results_guard.len() >= max_results {
            should_stop.store(true, Ordering::Relaxed);
            limit_reached.store(true, Ordering::Relaxed);
        }

        drop(results_guard);
        if let (Some(progress_sink), Some(group)) = (
            progress_sink,
            Self::build_search_result_group(accepted_results),
        ) {
            progress_sink.report(group);
        }

        !should_stop.load(Ordering::Relaxed)
    }

    fn search_file_content_lines(
        path: &Path,
        file_name: &str,
        matcher: &Regex,
        results: &Arc<Mutex<Vec<FileSearchResult>>>,
        max_results: usize,
        should_stop: &Arc<AtomicBool>,
        limit_reached: &Arc<AtomicBool>,
        cancel_flag: Option<&Arc<AtomicBool>>,
        progress_sink: Option<&Arc<dyn FileSearchProgressSink>>,
    ) -> BitFunResult<()> {
        if should_stop.load(Ordering::Relaxed) || cancellation_requested(cancel_flag) {
            should_stop.store(true, Ordering::Relaxed);
            return Ok(());
        }

        let file = File::open(path)
            .map_err(|error| BitFunError::service(format!("Failed to open file: {}", error)))?;
        let reader = BufReader::new(file);
        let mut matched_results = Vec::new();

        for (index, line_result) in reader.split(b'\n').enumerate() {
            if should_stop.load(Ordering::Relaxed) || cancellation_requested(cancel_flag) {
                should_stop.store(true, Ordering::Relaxed);
                return Ok(());
            }

            let line_bytes = line_result
                .map_err(|error| BitFunError::service(format!("Failed to read file: {}", error)))?;
            let line = String::from_utf8_lossy(&line_bytes)
                .trim_end_matches('\r')
                .to_string();

            if !matcher.is_match(&line) {
                continue;
            }

            let (preview_before, preview_inside, preview_after) =
                Self::build_content_match_preview(&line, matcher);

            matched_results.push(FileSearchResult {
                path: path.to_string_lossy().to_string(),
                name: file_name.to_string(),
                is_directory: false,
                match_type: SearchMatchType::Content,
                line_number: Some(index + 1),
                matched_content: Some(line),
                preview_before,
                preview_inside,
                preview_after,
            });

            if matched_results.len() >= max_results {
                break;
            }
        }

        if !Self::push_search_result_group(
            results,
            should_stop,
            limit_reached,
            max_results,
            progress_sink,
            matched_results,
        ) {
            return Ok(());
        }

        Ok(())
    }

    fn should_skip_directory_static(file_name: &str) -> bool {
        matches!(
            file_name,
            "node_modules"
                | ".git"
                | ".svn"
                | ".hg"
                | "target"
                | "build"
                | "dist"
                | "out"
                | ".next"
                | ".nuxt"
                | ".cache"
                | "__pycache__"
                | "coverage"
                | ".idea"
                | ".vscode"
        )
    }

    fn should_skip_file_static(file_name: &str) -> bool {
        matches!(file_name, ".DS_Store" | "Thumbs.db")
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchResult {
    pub path: String,
    pub name: String,
    pub is_directory: bool,
    pub match_type: SearchMatchType,
    pub line_number: Option<usize>,
    pub matched_content: Option<String>,
    pub preview_before: Option<String>,
    pub preview_inside: Option<String>,
    pub preview_after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchMatchType {
    FileName,
    Content,
}
