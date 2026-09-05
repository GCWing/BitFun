use crate::infrastructure::{
    FileInfo, FileOperationOptions, FileReadResult, FileSearchOutcome, FileSearchProgressSink,
    FileSearchResult, FileTreeNode, FileTreeStatistics, FileWriteResult,
};
use crate::util::elapsed_ms_u64;
use crate::util::errors::*;
use log::debug;
use openbitfun_services_core::filesystem::FileSystemService as BaseFileSystemService;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::types::{DirectoryScanResult, DirectoryStats, FileSearchOptions, FileSystemConfig};

const SLOW_FILESYSTEM_OPERATION_LOG_MS: u64 = 500;

fn map_filesystem_error(error: impl std::fmt::Display) -> OpenBitFunError {
    OpenBitFunError::service(error.to_string())
}

#[cfg(feature = "remote-workspace")]
async fn read_remote_directory_contents(
    path: &str,
    preferred_remote_connection_id: Option<&str>,
) -> Option<OpenBitFunResult<Vec<FileTreeNode>>> {
    let explicit_connection_id = preferred_remote_connection_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let remote_entry = if let Some(connection_id) = explicit_connection_id {
        crate::service::remote_ssh::workspace_state::lookup_remote_connection_scoped(
            path,
            connection_id,
        )
        .await
    } else {
        crate::service::remote_ssh::workspace_state::lookup_remote_connection_with_hint(path, None)
            .await
    };
    let entry = match remote_entry {
        Some(entry) => entry,
        None if explicit_connection_id.is_some() => {
            return Some(Err(OpenBitFunError::service(format!(
                "Remote workspace connection '{}' is unavailable or does not own path '{}'; local filesystem fallback was not attempted",
                explicit_connection_id.unwrap_or_default(),
                path
            ))));
        }
        None => return None,
    };

    let Some(manager) = crate::service::remote_ssh::workspace_state::get_remote_workspace_manager()
    else {
        return Some(Err(OpenBitFunError::service(
            "Remote workspace manager is unavailable",
        )));
    };
    let Some(file_service) = manager.get_file_service().await else {
        return Some(Err(OpenBitFunError::service(
            "Remote file service is unavailable",
        )));
    };

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
            Err(error) => Err(OpenBitFunError::service(format!(
                "Failed to read remote directory: {}",
                error
            ))),
        },
    )
}

/// Without the `remote-workspace` feature there is no SSH provider to route
/// to. A request that carries a remote marker (an explicit connection id or a
/// path owned by an opened workspace of kind `Remote`) is refused here so it
/// never reaches the controller filesystem scanner below.
#[cfg(not(feature = "remote-workspace"))]
async fn read_remote_directory_contents(
    path: &str,
    preferred_remote_connection_id: Option<&str>,
) -> Option<OpenBitFunResult<Vec<FileTreeNode>>> {
    use crate::service::remote_ssh::workspace_state::{
        is_remote_path, remote_workspace_not_compiled_message,
    };

    let explicit_connection_id = preferred_remote_connection_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if explicit_connection_id.is_some() || is_remote_path(path).await {
        return Some(Err(OpenBitFunError::NotImplemented(
            remote_workspace_not_compiled_message(path),
        )));
    }
    None
}

async fn is_remote_path(path: &str) -> bool {
    crate::service::remote_ssh::workspace_state::is_remote_path(path).await
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
    pub async fn build_file_tree(&self, root_path: &str) -> OpenBitFunResult<Vec<FileTreeNode>> {
        self.build_file_tree_with_remote_hint(root_path, None).await
    }

    /// Same as [`Self::build_file_tree`], but disambiguates remote roots when `preferred_remote_connection_id` is set.
    pub async fn build_file_tree_with_remote_hint(
        &self,
        root_path: &str,
        preferred_remote_connection_id: Option<&str>,
    ) -> OpenBitFunResult<Vec<FileTreeNode>> {
        let started_at = std::time::Instant::now();
        // An explicit remote connection id is exact target identity, never a
        // hint the local scanner may ignore: route it through the remote
        // reader, which fails closed when that connection does not own the
        // path or when this build has no remote provider at all.
        let explicit_remote_scope = preferred_remote_connection_id
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        let tree = if explicit_remote_scope || is_remote_path(root_path).await {
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
    pub async fn scan_directory(&self, root_path: &str) -> OpenBitFunResult<DirectoryScanResult> {
        let start_time = std::time::Instant::now();

        let (files, statistics) = if is_remote_path(root_path).await {
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
    pub async fn get_directory_contents(&self, path: &str) -> OpenBitFunResult<Vec<FileTreeNode>> {
        self.get_directory_contents_with_remote_hint(path, None)
            .await
    }

    pub async fn get_directory_contents_with_remote_hint(
        &self,
        path: &str,
        preferred_remote_connection_id: Option<&str>,
    ) -> OpenBitFunResult<Vec<FileTreeNode>> {
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
    ) -> OpenBitFunResult<Vec<FileSearchResult>> {
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
    ) -> OpenBitFunResult<FileSearchOutcome> {
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
    ) -> OpenBitFunResult<FileSearchOutcome> {
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
    ) -> OpenBitFunResult<FileSearchOutcome> {
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
    ) -> OpenBitFunResult<FileSearchOutcome> {
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
    pub async fn read_file(&self, file_path: &str) -> OpenBitFunResult<FileReadResult> {
        self.inner
            .read_file(file_path)
            .await
            .map_err(map_filesystem_error)
    }

    /// Reads the exact file bytes without text or binary-content inference.
    pub async fn read_file_bytes(&self, file_path: &str) -> OpenBitFunResult<Vec<u8>> {
        self.inner
            .read_file_bytes(file_path)
            .await
            .map_err(map_filesystem_error)
    }

    /// Writes a file.
    pub async fn write_file(
        &self,
        file_path: &str,
        content: &str,
    ) -> OpenBitFunResult<FileWriteResult> {
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
    ) -> OpenBitFunResult<FileWriteResult> {
        self.inner
            .write_file_with_options(file_path, content, options)
            .await
            .map_err(map_filesystem_error)
    }

    /// Copies a file.
    pub async fn copy_file(&self, from: &str, to: &str) -> OpenBitFunResult<u64> {
        self.inner
            .copy_file(from, to)
            .await
            .map_err(map_filesystem_error)
    }

    /// Moves a file.
    pub async fn move_file(&self, from: &str, to: &str) -> OpenBitFunResult<()> {
        self.inner
            .move_file(from, to)
            .await
            .map_err(map_filesystem_error)
    }

    /// Deletes a file.
    pub async fn delete_file(&self, file_path: &str) -> OpenBitFunResult<()> {
        self.inner
            .delete_file(file_path)
            .await
            .map_err(map_filesystem_error)
    }

    /// Gets file info.
    pub async fn get_file_info(&self, file_path: &str) -> OpenBitFunResult<FileInfo> {
        self.inner
            .get_file_info(file_path)
            .await
            .map_err(map_filesystem_error)
    }

    /// Creates a directory.
    pub async fn create_directory(&self, dir_path: &str) -> OpenBitFunResult<()> {
        self.inner
            .create_directory(dir_path)
            .await
            .map_err(map_filesystem_error)
    }

    /// Deletes a directory.
    pub async fn delete_directory(&self, dir_path: &str, recursive: bool) -> OpenBitFunResult<()> {
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
    pub async fn get_file_size(&self, file_path: &str) -> OpenBitFunResult<u64> {
        self.inner
            .get_file_size(file_path)
            .await
            .map_err(map_filesystem_error)
    }

    /// Reads a text file quickly.
    pub async fn read_text_file(&self, file_path: &str) -> OpenBitFunResult<String> {
        self.inner
            .read_text_file(file_path)
            .await
            .map_err(map_filesystem_error)
    }

    /// Writes a text file quickly.
    pub async fn write_text_file(&self, file_path: &str, content: &str) -> OpenBitFunResult<()> {
        self.inner
            .write_text_file(file_path, content)
            .await
            .map_err(map_filesystem_error)
    }

    /// Lists all files in a directory (recursive).
    pub async fn list_all_files(&self, root_path: &str) -> OpenBitFunResult<Vec<String>> {
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
    pub async fn calculate_directory_size(&self, dir_path: &str) -> OpenBitFunResult<u64> {
        let scan_result = self.scan_directory(dir_path).await?;
        Ok(scan_result.statistics.total_size_bytes)
    }

    /// Finds files by extension.
    pub async fn find_files_by_extension(
        &self,
        root_path: &str,
        extension: &str,
    ) -> OpenBitFunResult<Vec<String>> {
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
    pub async fn get_directory_stats(&self, dir_path: &str) -> OpenBitFunResult<DirectoryStats> {
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
                ext_vec.sort_by_key(|entry| std::cmp::Reverse(entry.1));
                ext_vec.into_iter().take(10).collect()
            },
            large_files_count: stats.large_files.len(),
            hidden_files_count: stats.hidden_files_count,
            symlinks_count: stats.symlinks_count,
        })
    }

    /// SHA-256 hex of on-disk content after editor-sync normalization (see `FileOperationService`).
    pub async fn editor_sync_content_sha256_hex(
        &self,
        file_path: &str,
    ) -> OpenBitFunResult<String> {
        self.inner
            .editor_sync_content_sha256_hex(file_path)
            .await
            .map_err(map_filesystem_error)
    }

    pub fn editor_sync_sha256_hex_from_raw_bytes(&self, bytes: &[u8]) -> String {
        self.inner.editor_sync_sha256_hex_from_raw_bytes(bytes)
    }
}

#[cfg(all(test, not(feature = "remote-workspace")))]
mod remote_marker_tests {
    use super::FileSystemService;

    /// Without the `remote-workspace` feature there is no provider, so a
    /// request that names a remote connection must be refused instead of
    /// reading the controller filesystem. The sentinel proves no local read
    /// happened: the directory exists locally and would list fine.
    #[tokio::test]
    async fn explicit_remote_scope_is_refused_without_remote_workspace_support() {
        let temp = tempfile::tempdir().expect("tempdir");
        // The sentinel only needs to exist; the directory itself is the local
        // content a silent fallback would have listed.
        tempfile::Builder::new()
            .prefix("controller-sentinel")
            .suffix(".txt")
            .tempfile_in(temp.path())
            .expect("sentinel")
            .keep()
            .expect("keep sentinel");
        let root = temp.path().to_string_lossy().to_string();
        let service = FileSystemService::default();

        for result in [
            service
                .get_directory_contents_with_remote_hint(&root, Some("ssh-remote-1"))
                .await,
            service
                .build_file_tree_with_remote_hint(&root, Some("ssh-remote-1"))
                .await,
        ] {
            let error = result.expect_err("remote-marked requests must fail closed");
            assert!(
                matches!(
                    error,
                    crate::util::errors::OpenBitFunError::NotImplemented(_)
                ),
                "unexpected error: {error}"
            );
            let message = error.to_string();
            assert!(
                message.contains("not compiled into this OpenBitFun host"),
                "unexpected error: {message}"
            );
            assert!(
                !message.contains("controller-sentinel"),
                "the controller directory must not have been listed: {message}"
            );
        }
    }

    #[tokio::test]
    async fn local_requests_keep_working_without_remote_workspace_support() {
        let temp = tempfile::tempdir().expect("tempdir");
        let (_, local_path) = tempfile::Builder::new()
            .prefix("local-")
            .suffix(".txt")
            .tempfile_in(temp.path())
            .expect("file")
            .keep()
            .expect("keep file");
        let local_name = local_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("file name")
            .to_string();
        let root = temp.path().to_string_lossy().to_string();

        let entries = FileSystemService::default()
            .get_directory_contents_with_remote_hint(&root, None)
            .await
            .expect("local directories stay readable");
        assert!(entries.iter().any(|entry| entry.name == local_name));
    }
}

#[cfg(all(test, feature = "remote-workspace", not(feature = "ssh-remote")))]
mod tests {
    use super::FileSystemService;
    use crate::service::remote_ssh::workspace_state::init_remote_workspace_manager;

    #[tokio::test]
    async fn registered_remote_path_without_file_provider_fails_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let remote_root = temp.path().to_string_lossy().to_string();
        let connection_id = "filesystem-no-provider";
        let manager = init_remote_workspace_manager();
        manager
            .register_remote_workspace(
                remote_root.clone(),
                connection_id.to_string(),
                "No provider".to_string(),
                "no-provider-host".to_string(),
            )
            .await;

        let error = FileSystemService::default()
            .get_directory_contents_with_remote_hint(&remote_root, Some(connection_id))
            .await
            .expect_err("registered remote paths must not fall back to the local filesystem");

        manager
            .unregister_remote_workspace(connection_id, &remote_root)
            .await;
        assert!(
            error
                .to_string()
                .contains("Remote file service is unavailable"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn explicit_remote_scope_never_falls_back_to_a_local_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let remote_root = temp.path().to_string_lossy().to_string();
        let registered_connection_id = "filesystem-exact-scope-registered";
        let requested_connection_id = "filesystem-exact-scope-requested";
        let manager = init_remote_workspace_manager();
        manager
            .register_remote_workspace(
                remote_root.clone(),
                registered_connection_id.to_string(),
                "Other remote".to_string(),
                "other-remote-host".to_string(),
            )
            .await;

        let error = FileSystemService::default()
            .get_directory_contents_with_remote_hint(&remote_root, Some(requested_connection_id))
            .await
            .expect_err("an explicit remote scope must not read the controller filesystem");

        manager
            .unregister_remote_workspace(registered_connection_id, &remote_root)
            .await;
        let message = error.to_string();
        assert!(
            message.contains(requested_connection_id),
            "unexpected error: {message}"
        );
        assert!(
            message.contains("local filesystem fallback was not attempted"),
            "unexpected error: {message}"
        );
    }
}
