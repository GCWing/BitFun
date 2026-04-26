use crate::infrastructure::{
    FileContentSearchOptions, FileInfo, FileNameSearchOptions, FileOperationOptions,
    FileOperationService, FileReadResult, FileSearchOutcome, FileSearchProgressSink,
    FileSearchResult, FileTreeNode, FileTreeService, FileWriteResult,
};
use crate::util::elapsed_ms_u64;
use crate::util::errors::*;
use log::debug;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::types::{DirectoryScanResult, DirectoryStats, FileSearchOptions, FileSystemConfig};

const SLOW_FILESYSTEM_OPERATION_LOG_MS: u64 = 500;

/// Unified file system service
pub struct FileSystemService {
    file_tree_service: Arc<FileTreeService>,
    file_operation_service: Arc<FileOperationService>,
}

impl FileSystemService {
    /// Creates a new file system service.
    pub fn new(config: FileSystemConfig) -> Self {
        let file_tree_service = Arc::new(FileTreeService::new(config.tree_options));
        let file_operation_service = Arc::new(FileOperationService::new(config.operation_options));

        Self {
            file_tree_service,
            file_operation_service,
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
        let tree = self
            .file_tree_service
            .build_tree_with_remote_hint(root_path, preferred_remote_connection_id)
            .await
            .map_err(BitFunError::service)?;
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

        let (files, statistics) = self
            .file_tree_service
            .build_tree_with_stats(root_path)
            .await?;

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
        self.file_tree_service
            .get_directory_contents_with_remote_hint(path, preferred_remote_connection_id)
            .await
            .map_err(BitFunError::service)
    }

    /// Searches files.
    pub async fn search_files(
        &self,
        root_path: &str,
        pattern: &str,
        options: FileSearchOptions,
    ) -> BitFunResult<Vec<FileSearchResult>> {
        let mut results = self
            .file_tree_service
            .search_files_with_options(
                root_path,
                pattern,
                options.include_content,
                options.case_sensitive,
                options.use_regex,
                options.whole_word,
            )
            .await?;

        if let Some(extensions) = &options.file_extensions {
            results.retain(|result| {
                if result.is_directory {
                    true
                } else {
                    let path = std::path::Path::new(&result.path);
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        extensions.contains(&ext.to_lowercase())
                    } else {
                        false
                    }
                }
            });
        }

        if let Some(max_results) = options.max_results {
            results.truncate(max_results);
        }

        Ok(results)
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
        let mut outcome = self
            .file_tree_service
            .search_file_names_with_progress(
                root_path,
                pattern,
                FileNameSearchOptions {
                    case_sensitive: options.case_sensitive,
                    use_regex: options.use_regex,
                    whole_word: options.whole_word,
                    max_results: options.max_results.unwrap_or(10_000),
                    include_directories: options.include_directories,
                    cancel_flag,
                },
                progress_sink,
            )
            .await?;

        if let Some(extensions) = &options.file_extensions {
            outcome.results.retain(|result| {
                if result.is_directory {
                    true
                } else {
                    let path = std::path::Path::new(&result.path);
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        extensions.contains(&ext.to_lowercase())
                    } else {
                        false
                    }
                }
            });
        }

        if let Some(max_results) = options.max_results {
            outcome.results.truncate(max_results);
        }

        Ok(outcome)
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
        let mut outcome = self
            .file_tree_service
            .search_file_contents_with_progress(
                root_path,
                pattern,
                FileContentSearchOptions {
                    case_sensitive: options.case_sensitive,
                    use_regex: options.use_regex,
                    whole_word: options.whole_word,
                    max_results: options.max_results.unwrap_or(10_000),
                    max_file_size_bytes: 10 * 1024 * 1024,
                    cancel_flag,
                },
                progress_sink,
            )
            .await?;

        if let Some(extensions) = &options.file_extensions {
            outcome.results.retain(|result| {
                let path = std::path::Path::new(&result.path);
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    extensions.contains(&ext.to_lowercase())
                } else {
                    false
                }
            });
        }

        if let Some(max_results) = options.max_results {
            outcome.results.truncate(max_results);
        }

        Ok(outcome)
    }

    /// Reads a file.
    pub async fn read_file(&self, file_path: &str) -> BitFunResult<FileReadResult> {
        self.file_operation_service.read_file(file_path).await
    }

    /// Writes a file.
    pub async fn write_file(
        &self,
        file_path: &str,
        content: &str,
    ) -> BitFunResult<FileWriteResult> {
        let options = FileOperationOptions::default();
        self.file_operation_service
            .write_file(file_path, content, options)
            .await
    }

    /// Writes a file with options.
    pub async fn write_file_with_options(
        &self,
        file_path: &str,
        content: &str,
        options: FileOperationOptions,
    ) -> BitFunResult<FileWriteResult> {
        self.file_operation_service
            .write_file(file_path, content, options)
            .await
    }

    /// Copies a file.
    pub async fn copy_file(&self, from: &str, to: &str) -> BitFunResult<u64> {
        self.file_operation_service.copy_file(from, to).await
    }

    /// Moves a file.
    pub async fn move_file(&self, from: &str, to: &str) -> BitFunResult<()> {
        self.file_operation_service.move_file(from, to).await
    }

    /// Deletes a file.
    pub async fn delete_file(&self, file_path: &str) -> BitFunResult<()> {
        self.file_operation_service.delete_file(file_path).await
    }

    /// Gets file info.
    pub async fn get_file_info(&self, file_path: &str) -> BitFunResult<FileInfo> {
        self.file_operation_service.get_file_info(file_path).await
    }

    /// Creates a directory.
    pub async fn create_directory(&self, dir_path: &str) -> BitFunResult<()> {
        self.file_operation_service.create_directory(dir_path).await
    }

    /// Deletes a directory.
    pub async fn delete_directory(&self, dir_path: &str, recursive: bool) -> BitFunResult<()> {
        self.file_operation_service
            .delete_directory(dir_path, recursive)
            .await
    }

    /// Checks whether the path exists.
    pub async fn exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }

    /// Checks whether the path is a directory.
    pub async fn is_directory(&self, path: &str) -> bool {
        std::path::Path::new(path).is_dir()
    }

    /// Checks whether the path is a file.
    pub async fn is_file(&self, path: &str) -> bool {
        std::path::Path::new(path).is_file()
    }

    /// Gets the file size.
    pub async fn get_file_size(&self, file_path: &str) -> BitFunResult<u64> {
        let info = self.get_file_info(file_path).await?;
        Ok(info.size)
    }

    /// Reads a text file quickly.
    pub async fn read_text_file(&self, file_path: &str) -> BitFunResult<String> {
        let result = self.read_file(file_path).await?;
        if result.is_binary {
            Err(BitFunError::service(
                "File is binary, cannot read as text".to_string(),
            ))
        } else {
            Ok(result.content)
        }
    }

    /// Writes a text file quickly.
    pub async fn write_text_file(&self, file_path: &str, content: &str) -> BitFunResult<()> {
        self.write_file(file_path, content).await.map(|_| ())
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
        self.file_operation_service
            .editor_sync_content_sha256_hex(file_path)
            .await
    }

    pub fn editor_sync_sha256_hex_from_raw_bytes(&self, bytes: &[u8]) -> String {
        self.file_operation_service
            .editor_sync_sha256_hex_from_raw_bytes(bytes)
    }
}
