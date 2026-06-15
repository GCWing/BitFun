use crate::infrastructure::{FileOperationOptions, FileTreeOptions};

use super::service::FileSystemService;
use super::types::FileSystemConfig;

/// File system service factory
pub struct FileSystemServiceFactory;

impl FileSystemServiceFactory {
    /// Creates the default service.
    pub fn create_default() -> FileSystemService {
        FileSystemService::default()
    }

    /// Creates a fast service (shallow scan).
    pub fn create_quick() -> FileSystemService {
        let config = FileSystemConfig {
            tree_options: FileTreeOptions {
                max_depth: Some(3),
                include_hidden: false,
                include_git_info: false,
                include_mime_types: false,
                ..Default::default()
            },
            operation_options: FileOperationOptions {
                max_file_size_mb: 10,
                backup_on_overwrite: false,
                ..Default::default()
            },
        };
        FileSystemService::new(config)
    }

    /// Creates a detailed service (deep scan).
    pub fn create_detailed() -> FileSystemService {
        let config = FileSystemConfig {
            tree_options: FileTreeOptions {
                max_depth: Some(10),
                include_hidden: true,
                include_git_info: true,
                include_mime_types: true,
                ..Default::default()
            },
            operation_options: FileOperationOptions {
                max_file_size_mb: 100,
                backup_on_overwrite: true,
                ..Default::default()
            },
        };
        FileSystemService::new(config)
    }

    /// Creates a restricted service (safe mode).
    pub fn create_restricted() -> FileSystemService {
        let config = FileSystemConfig {
            tree_options: FileTreeOptions {
                max_depth: Some(5),
                include_hidden: false,
                include_git_info: false,
                include_mime_types: false,
                ..Default::default()
            },
            operation_options: FileOperationOptions {
                max_file_size_mb: 1,
                allowed_extensions: Some(vec![
                    "txt".to_string(),
                    "md".to_string(),
                    "json".to_string(),
                    "yaml".to_string(),
                    "yml".to_string(),
                ]),
                backup_on_overwrite: true,
                ..Default::default()
            },
        };
        FileSystemService::new(config)
    }
}
