//! Remote file system operations via SFTP
//!
//! This module provides remote file system operations using the SFTP protocol

use crate::service::remote_ssh::types::{RemoteDirEntry, RemoteFileEntry, RemoteTreeNode};
use anyhow::anyhow;
use std::sync::Arc;

/// Remote file service using SFTP protocol
#[derive(Clone)]
pub struct RemoteFileService {
    manager: Arc<tokio::sync::RwLock<Option<crate::service::remote_ssh::manager::SSHConnectionManager>>>,
}

impl RemoteFileService {
    pub fn new(
        manager: Arc<tokio::sync::RwLock<Option<crate::service::remote_ssh::manager::SSHConnectionManager>>>,
    ) -> Self {
        Self { manager }
    }

    /// Get the SSH manager
    async fn get_manager(&self, _connection_id: &str) -> anyhow::Result<crate::service::remote_ssh::manager::SSHConnectionManager> {
        let guard = self.manager.read().await;
        guard.as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("SSH manager not initialized"))
    }

    /// Read a file from the remote server via SFTP
    pub async fn read_file(&self, connection_id: &str, path: &str) -> anyhow::Result<Vec<u8>> {
        let manager = self.get_manager(connection_id).await?;
        manager.sftp_read(connection_id, path).await
    }

    /// Write content to a remote file via SFTP
    pub async fn write_file(&self, connection_id: &str, path: &str, content: &[u8]) -> anyhow::Result<()> {
        let manager = self.get_manager(connection_id).await?;
        manager.sftp_write(connection_id, path, content).await
    }

    /// Check if a remote path exists
    pub async fn exists(&self, connection_id: &str, path: &str) -> anyhow::Result<bool> {
        let manager = self.get_manager(connection_id).await?;
        manager.sftp_exists(connection_id, path).await
    }

    /// Check if a remote path is a regular file
    pub async fn is_file(&self, connection_id: &str, path: &str) -> anyhow::Result<bool> {
        match self.stat(connection_id, path).await? {
            Some(entry) => Ok(entry.is_file),
            None => Ok(false),
        }
    }

    /// Check if a remote path is a directory
    pub async fn is_dir(&self, connection_id: &str, path: &str) -> anyhow::Result<bool> {
        match self.stat(connection_id, path).await? {
            Some(entry) => Ok(entry.is_dir),
            None => Ok(false),
        }
    }

    /// Read directory contents via SFTP
    pub async fn read_dir(&self, connection_id: &str, path: &str) -> anyhow::Result<Vec<RemoteDirEntry>> {
        let manager = self.get_manager(connection_id).await?;
        let mut entries = manager.sftp_read_dir(connection_id, path).await?;

        let mut result = Vec::new();

        for entry in entries.by_ref() {
            let name = entry.file_name();

            // Skip . and ..
            if name == "." || name == ".." {
                continue;
            }

            let full_path = if path.ends_with('/') {
                format!("{}{}", path, name)
            } else {
                format!("{}/{}", path, name)
            };

            let metadata = entry.metadata();
            let is_dir = entry.file_type().is_dir();
            let is_symlink = entry.file_type().is_symlink();
            let is_file = entry.file_type().is_file();

            // FileAttributes mtime is Unix timestamp in seconds; convert to milliseconds
            // for JavaScript Date compatibility.
            // Use size for any non-directory (regular files, symlinks, etc.). SFTP `is_file()`
            // is false for symlinks and some file types, which previously hid size incorrectly.
            let size = if is_dir { None } else { metadata.size };
            let modified = metadata.mtime.map(|t| (t as u64) * 1000);

            // Get permissions string
            let permissions = Some(format_permissions(metadata.permissions));

            result.push(RemoteDirEntry {
                name,
                path: full_path,
                is_dir,
                is_file,
                is_symlink,
                size,
                modified,
                permissions,
            });
        }

        Ok(result)
    }

    /// Build a tree of remote directory structure
    pub async fn build_tree(
        &self,
        connection_id: &str,
        path: &str,
        max_depth: Option<u32>,
    ) -> anyhow::Result<RemoteTreeNode> {
        let max_depth = max_depth.unwrap_or(3);
        Box::pin(self.build_tree_impl(connection_id, path, 0, max_depth)).await
    }

    async fn build_tree_impl(
        &self,
        connection_id: &str,
        path: &str,
        current_depth: u32,
        max_depth: u32,
    ) -> anyhow::Result<RemoteTreeNode> {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());

        // Check if this is a directory
        let is_dir = match self.exists(connection_id, path).await {
            Ok(exists) => exists,
            Err(_) => false,
        };

        // Check if it's a directory by trying to read it
        let is_dir = if is_dir {
            let entries = self.read_dir(connection_id, path).await;
            entries.is_ok()
        } else {
            false
        };

        if !is_dir || current_depth >= max_depth {
            return Ok(RemoteTreeNode {
                name,
                path: path.to_string(),
                is_dir,
                children: None,
            });
        }

        // Read directory contents
        let entries = match self.read_dir(connection_id, path).await {
            Ok(entries) => entries,
            Err(_) => {
                return Ok(RemoteTreeNode {
                    name,
                    path: path.to_string(),
                    is_dir: false,
                    children: None,
                });
            }
        };

        let mut children = Vec::new();

        for entry in entries {
            if entry.is_dir {
                match Box::pin(self.build_tree_impl(
                    connection_id,
                    &entry.path,
                    current_depth + 1,
                    max_depth,
                )).await {
                    Ok(child) => children.push(child),
                    Err(_) => {
                        children.push(RemoteTreeNode {
                            name: entry.name,
                            path: entry.path,
                            is_dir: true,
                            children: None,
                        });
                    }
                }
            } else {
                children.push(RemoteTreeNode {
                    name: entry.name,
                    path: entry.path,
                    is_dir: false,
                    children: None,
                });
            }
        }

        Ok(RemoteTreeNode {
            name,
            path: path.to_string(),
            is_dir: true,
            children: Some(children),
        })
    }

    /// Create a directory on the remote server via SFTP
    pub async fn create_dir(&self, connection_id: &str, path: &str) -> anyhow::Result<()> {
        let manager = self.get_manager(connection_id).await?;
        manager.sftp_mkdir(connection_id, path).await
    }

    /// Create directory and all parent directories via SFTP
    pub async fn create_dir_all(&self, connection_id: &str, path: &str) -> anyhow::Result<()> {
        let manager = self.get_manager(connection_id).await?;
        manager.sftp_mkdir_all(connection_id, path).await
    }

    /// Remove a file from the remote server via SFTP
    pub async fn remove_file(&self, connection_id: &str, path: &str) -> anyhow::Result<()> {
        let manager = self.get_manager(connection_id).await?;
        manager.sftp_remove(connection_id, path).await
    }

    /// Remove a directory and its contents recursively via SFTP
    pub async fn remove_dir_all(&self, connection_id: &str, path: &str) -> anyhow::Result<()> {
        // First, delete all contents
        match self.read_dir(connection_id, path).await {
            Ok(entries) => {
                for entry in entries {
                    let entry_path = entry.path.clone();
                    if entry.is_dir {
                        Box::pin(self.remove_dir_all(connection_id, &entry_path)).await?;
                    } else {
                        let manager = self.get_manager(connection_id).await?;
                        manager.sftp_remove(connection_id, &entry_path).await?;
                    }
                }
            }
            Err(_) => {}
        }

        // Then remove the directory itself
        let manager = self.get_manager(connection_id).await?;
        manager.sftp_rmdir(connection_id, path).await
    }

    /// Rename/move a remote file or directory via SFTP
    pub async fn rename(
        &self,
        connection_id: &str,
        old_path: &str,
        new_path: &str,
    ) -> anyhow::Result<()> {
        let manager = self.get_manager(connection_id).await?;
        manager.sftp_rename(connection_id, old_path, new_path).await
    }

    /// Get file metadata via SFTP
    pub async fn stat(&self, connection_id: &str, path: &str) -> anyhow::Result<Option<RemoteFileEntry>> {
        let manager = self.get_manager(connection_id).await?;

        match manager.sftp_stat(connection_id, path).await {
            Ok(attrs) => {
                let name = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string());

                let is_dir = attrs.is_dir();
                let is_symlink = attrs.is_symlink();
                // File is neither dir nor symlink
                let is_file = !is_dir && !is_symlink;
                let size = if is_dir { None } else { attrs.size };
                let modified = attrs.mtime.map(|t| (t as u64) * 1000);
                let permissions = Some(format_permissions(attrs.permissions));

                Ok(Some(RemoteFileEntry {
                    name,
                    path: path.to_string(),
                    is_dir,
                    is_file,
                    is_symlink,
                    size,
                    modified,
                    permissions,
                }))
            }
            Err(_) => Ok(None),
        }
    }
}

/// Format file permissions as string (e.g., "rwxr-xr-x")
fn format_permissions(mode: Option<u32>) -> String {
    let mode = match mode {
        Some(m) => m,
        None => return "---------".to_string(),
    };

    let file_type = match mode & 0o170000 {
        0o040000 => 'd',  // directory
        0o120000 => 'l',  // symbolic link
        0o060000 => 'b',  // block device
        0o020000 => 'c',  // character device
        0o010000 => 'p',  // FIFO
        0o140000 => 's',  // socket
        _ => '-',          // regular file
    };

    let perms = [
        (mode & 0o400 != 0, 'r'),
        (mode & 0o200 != 0, 'w'),
        (mode & 0o100 != 0, 'x'),
        (mode & 0o040 != 0, 'r'),
        (mode & 0o020 != 0, 'w'),
        (mode & 0o010 != 0, 'x'),
        (mode & 0o004 != 0, 'r'),
        (mode & 0o002 != 0, 'w'),
        (mode & 0o001 != 0, 'x'),
    ];

    let perm_str: String = perms.iter()
        .map(|(set, c)| if *set { *c } else { '-' })
        .collect();

    format!("{}{}", file_type, perm_str)
}
