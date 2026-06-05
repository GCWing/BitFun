use crate::infrastructure::{
    FileInfo, FileOperationOptions, FileReadResult, FileSearchOutcome, FileSearchProgressSink,
    FileSearchResult, FileTreeNode, FileTreeStatistics, FileWriteResult,
};
use crate::util::elapsed_ms_u64;
use crate::util::errors::*;
use bitfun_services_core::filesystem::FileSystemService as BaseFileSystemService;
use log::debug;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::types::{DirectoryScanResult, DirectoryStats, FileSearchOptions, FileSystemConfig};

const SLOW_FILESYSTEM_OPERATION_LOG_MS: u64 = 500;

fn map_filesystem_error(error: impl std::fmt::Display) -> BitFunError {
    BitFunError::service(error.to_string())
}

async fn read_remote_directory_contents(
    path: &str,
    preferred_remote_connection_id: Option<&str>,
) -> Option<BitFunResult<Vec<FileTreeNode>>> {
    let entry = crate::service::remote_ssh::workspace_state::lookup_remote_connection_with_hint(
        path,
        preferred_remote_connection_id,
    )
    .await?;

    let manager = crate::service::remote_ssh::workspace_state::get_remote_workspace_manager()?;
    let file_service = manager.get_file_service().await?;

    Some(
        match file_service.read_dir(&entry.connection_id, path).await {
            Ok(entries) => Ok(entries
                .into_iter()
                .filter(|entry| entry.name != "." && entry.name != "..")
                .map(|entry| {
                    FileTreeNode::new(
                        entry.path.clone(),
                        entry.name.clone(),
                        entry.path,
                        entry.is_dir,
                    )
                })
                .collect()),
            Err(error) => Err(BitFunError::service(format!(
                "Failed to read remote directory: {}",
                error
            ))),
        },
    )
}

/// Unified file system service
pub struct FileSystemService {
    inner: BaseFileSystemService,
}

impl FileSystemService {
    /// Creates a new file system service.
    pub fn new(config: FileSystemConfig) -> Self {
        Self {
            inner: BaseFileSystemService::new(config),
        }
    }

    /// Creates the default service.
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(FileSystemConfig::default())
    }

    /// Builds a file tree.
    pub async fn build_file_tree(&self, root_path: &str) -> BitFunResult<Vec<FileTreeNode>> {
        self.build_file_tree_with_remote_hint(root_path, None).await
    }

    /// Same as [`Self::build_file_tree`], but disambiguates remote roots when `preferred_remote_connection_id` is set.
    pub async fn build_file_tree_with_remote_hint(
        &self,
        root_path: &str,
        preferred_remote_connection_id: Option<&str>,
    ) -> BitFunResult<Vec<FileTreeNode>> {
        let started_at = std::time::Instant::now();
        let tree = if crate::service::remote_ssh::workspace_state::is_remote_path(root_path).await {
            self.get_directory_contents_with_remote_hint(root_path, preferred_remote_connection_id)
                .await?
        } else {
            self.inner
                .build_file_tree_with_remote_hint(root_path, preferred_remote_connection_id)
                .await
                .map_err(map_filesystem_error)?
        };
        let duration_ms = elapsed_ms_u64(started_at);

        if duration_ms >= SLOW_FILESYSTEM_OPERATION_LOG_MS {
            debug!(
                "File tree built: root_path={}, preferred_remote_connection_id={}, duration_ms={}, root_entries={}",
                root_path,
                preferred_remote_connection_id.unwrap_or("local"),
                duration_ms,
                tree.len()
            );
        }

        Ok(tree)
    }

    /// Scans a directory and returns a detailed result.
    pub async fn scan_directory(&self, root_path: &str) -> BitFunResult<DirectoryScanResult> {
        let start_time = std::time::Instant::now();

        let (files, statistics) =
            if crate::service::remote_ssh::workspace_state::is_remote_path(root_path).await {
                let nodes = self
                    .get_directory_contents_with_remote_hint(root_path, None)
                    .await?;
                let stats = FileTreeStatistics {
                    total_files: nodes.iter().filter(|node| !node.is_directory).count(),
                    total_directories: nodes.iter().filter(|node| node.is_directory).count(),
                    total_size_bytes: 0,
                    max_depth_reached: 0,
                    file_type_counts: HashMap::new(),
                    large_files: Vec::new(),
                    symlinks_count: 0,
                    hidden_files_count: 0,
                };
                (nodes, stats)
            } else {
                let scan_result = self
                    .inner
                    .scan_directory(root_path)
                    .await
                    .map_err(map_filesystem_error)?;
                (scan_result.files, scan_result.statistics)
            };

        let scan_time_ms = elapsed_ms_u64(start_time);

        if scan_time_ms >= SLOW_FILESYSTEM_OPERATION_LOG_MS {
            debug!(
                "Directory scan completed: root_path={}, duration_ms={}, total_files={}, total_directories={}, total_size_bytes={}",
                root_path,
                scan_time_ms,
                statistics.total_files,
                statistics.total_directories,
                statistics.total_size_bytes
            );
        }

        Ok(DirectoryScanResult {
            files,
            statistics,
            scan_time_ms,
        })
    }

    /// Gets directory contents (shallow).
    pub async fn get_directory_contents(&self, path: &str) -> BitFunResult<Vec<FileTreeNode>> {
        self.get_directory_contents_with_remote_hint(path, None)
            .await
    }

    pub async fn get_directory_contents_with_remote_hint(
        &self,
        path: &str,
        preferred_remote_connection_id: Option<&str>,
    ) -> BitFunResult<Vec<FileTreeNode>> {
        if let Some(result) =
            read_remote_directory_contents(path, preferred_remote_connection_id).await
        {
            return result;
        }

        self.inner
            .get_directory_contents_with_remote_hint(path, preferred_remote_connection_id)
            .await
            .map_err(map_filesystem_error)
    }

    /// Searches files.
    pub async fn search_files(
        &self,
        root_path: &str,
        pattern: &str,
        options: FileSearchOptions,
    ) -> BitFunResult<Vec<FileSearchResult>> {
        self.inner
            .search_files(root_path, pattern, options)
            .await
            .map_err(map_filesystem_error)
    }

    pub async fn search_file_names(
        &self,
        root_path: &str,
        pattern: &str,
        options: FileSearchOptions,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> BitFunResult<FileSearchOutcome> {
        self.search_file_names_with_progress(root_path, pattern, options, cancel_flag, None)
            .await
    }

    pub async fn search_file_names_with_progress(
        &self,
        root_path: &str,
        pattern: &str,
        options: FileSearchOptions,
        cancel_flag: Option<Arc<AtomicBool>>,
        progress_sink: Option<Arc<dyn FileSearchProgressSink>>,
    ) -> BitFunResult<FileSearchOutcome> {
        self.inner
            .search_file_names_with_progress(
                root_path,
                pattern,
                options,
                cancel_flag,
                progress_sink,
            )
            .await
            .map_err(map_filesystem_error)
    }

    pub async fn search_file_contents(
        &self,
        root_path: &str,
        pattern: &str,
        options: FileSearchOptions,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> BitFunResult<FileSearchOutcome> {
        self.search_file_contents_with_progress(root_path, pattern, options, cancel_flag, None)
            .await
    }

    pub async fn search_file_contents_with_progress(
        &self,
        root_path: &str,
        pattern: &str,
        options: FileSearchOptions,
        cancel_flag: Option<Arc<AtomicBool>>,
        progress_sink: Option<Arc<dyn FileSearchProgressSink>>,
    ) -> BitFunResult<FileSearchOutcome> {
        self.inner
            .search_file_contents_with_progress(
                root_path,
                pattern,
                options,
                cancel_flag,
                progress_sink,
            )
            .await
            .map_err(map_filesystem_error)
    }

    /// Reads a file.
    pub async fn read_file(&self, file_path: &str) -> BitFunResult<FileReadResult> {
        self.inner
            .read_file(file_path)
            .await
            .map_err(map_filesystem_error)
    }

    /// Writes a file.
    pub async fn write_file(
        &self,
        file_path: &str,
        content: &str,
    ) -> BitFunResult<FileWriteResult> {
        self.inner
            .write_file(file_path, content)
            .await
            .map_err(map_filesystem_error)
    }

    /// Writes a file with options.
    pub async fn write_file_with_options(
        &self,
        file_path: &str,
        content: &str,
        options: FileOperationOptions,
    ) -> BitFunResult<FileWriteResult> {
        self.inner
            .write_file_with_options(file_path, content, options)
            .await
            .map_err(map_filesystem_error)
    }

    /// Copies a file.
    pub async fn copy_file(&self, from: &str, to: &str) -> BitFunResult<u64> {
        self.inner
            .copy_file(from, to)
            .await
            .map_err(map_filesystem_error)
    }

    /// Moves a file.
    pub async fn move_file(&self, from: &str, to: &str) -> BitFunResult<()> {
        self.inner
            .move_file(from, to)
            .await
            .map_err(map_filesystem_error)
    }

    /// Deletes a file.
    pub async fn delete_file(&self, file_path: &str) -> BitFunResult<()> {
        self.inner
            .delete_file(file_path)
            .await
            .map_err(map_filesystem_error)
    }

    /// Gets file info.
    pub async fn get_file_info(&self, file_path: &str) -> BitFunResult<FileInfo> {
        self.inner
            .get_file_info(file_path)
            .await
            .map_err(map_filesystem_error)
    }

    /// Creates a directory.
    pub async fn create_directory(&self, dir_path: &str) -> BitFunResult<()> {
        self.inner
            .create_directory(dir_path)
            .await
            .map_err(map_filesystem_error)
    }

    /// Deletes a directory.
    pub async fn delete_directory(&self, dir_path: &str, recursive: bool) -> BitFunResult<()> {
        self.inner
            .delete_directory(dir_path, recursive)
            .await
            .map_err(map_filesystem_error)
    }

    /// Checks whether the path exists.
    pub async fn exists(&self, path: &str) -> bool {
        self.inner.exists(path).await
    }

    /// Checks whether the path is a directory.
    pub async fn is_directory(&self, path: &str) -> bool {
        self.inner.is_directory(path).await
    }

    /// Checks whether the path is a file.
    pub async fn is_file(&self, path: &str) -> bool {
        self.inner.is_file(path).await
    }

    /// Gets the file size.
    pub async fn get_file_size(&self, file_path: &str) -> BitFunResult<u64> {
        self.inner
            .get_file_size(file_path)
            .await
            .map_err(map_filesystem_error)
    }

    /// Reads a text file quickly.
    pub async fn read_text_file(&self, file_path: &str) -> BitFunResult<String> {
        self.inner
            .read_text_file(file_path)
            .await
            .map_err(map_filesystem_error)
    }

    /// Writes a text file quickly.
    pub async fn write_text_file(&self, file_path: &str, content: &str) -> BitFunResult<()> {
        self.inner
            .write_text_file(file_path, content)
            .await
            .map_err(map_filesystem_error)
    }

    /// Lists all files in a directory (recursive).
    pub async fn list_all_files(&self, root_path: &str) -> BitFunResult<Vec<String>> {
        let tree = self.build_file_tree(root_path).await?;
        let mut files = Vec::new();

        fn collect_files(nodes: &[FileTreeNode], files: &mut Vec<String>) {
            for node in nodes {
                if !node.is_directory {
                    files.push(node.path.clone());
                }
                if let Some(children) = &node.children {
                    collect_files(children, files);
                }
            }
        }

        collect_files(&tree, &mut files);
        Ok(files)
    }

    /// Calculates the directory size.
    pub async fn calculate_directory_size(&self, dir_path: &str) -> BitFunResult<u64> {
        let scan_result = self.scan_directory(dir_path).await?;
        Ok(scan_result.statistics.total_size_bytes)
    }

    /// Finds files by extension.
    pub async fn find_files_by_extension(
        &self,
        root_path: &str,
        extension: &str,
    ) -> BitFunResult<Vec<String>> {
        let options = FileSearchOptions {
            include_content: false,
            file_extensions: Some(vec![extension.to_lowercase()]),
            ..Default::default()
        };

        let results = self.search_files(root_path, "", options).await?;
        Ok(results
            .into_iter()
            .filter(|r| !r.is_directory)
            .map(|r| r.path)
            .collect())
    }

    /// Gets directory statistics.
    pub async fn get_directory_stats(&self, dir_path: &str) -> BitFunResult<DirectoryStats> {
        let scan_result = self.scan_directory(dir_path).await?;
        let stats = scan_result.statistics;

        Ok(DirectoryStats {
            total_files: stats.total_files,
            total_directories: stats.total_directories,
            total_size_bytes: stats.total_size_bytes,
            total_size_mb: stats.total_size_bytes / (1024 * 1024),
            max_depth: stats.max_depth_reached,
            most_common_extensions: {
                let mut ext_vec: Vec<_> = stats.file_type_counts.into_iter().collect();
                ext_vec.sort_by(|a, b| b.1.cmp(&a.1));
                ext_vec.into_iter().take(10).collect()
            },
            large_files_count: stats.large_files.len(),
            hidden_files_count: stats.hidden_files_count,
            symlinks_count: stats.symlinks_count,
        })
    }

    /// SHA-256 hex of on-disk content after editor-sync normalization (see `FileOperationService`).
    pub async fn editor_sync_content_sha256_hex(&self, file_path: &str) -> BitFunResult<String> {
        self.inner
            .editor_sync_content_sha256_hex(file_path)
            .await
            .map_err(map_filesystem_error)
    }

    pub fn editor_sync_sha256_hex_from_raw_bytes(&self, bytes: &[u8]) -> String {
        self.inner.editor_sync_sha256_hex_from_raw_bytes(bytes)
    }
}
