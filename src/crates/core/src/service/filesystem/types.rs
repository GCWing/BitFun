use crate::infrastructure::{
    FileOperationOptions, FileTreeNode, FileTreeOptions, FileTreeStatistics,
};
use serde::{Deserialize, Serialize};

/// File system service configuration
#[derive(Debug, Clone, Default)]
pub struct FileSystemConfig {
    pub tree_options: FileTreeOptions,
    pub operation_options: FileOperationOptions,
}

/// Directory scan result
#[derive(Debug, Serialize, Deserialize)]
pub struct DirectoryScanResult {
    pub files: Vec<FileTreeNode>,
    pub statistics: FileTreeStatistics,
    pub scan_time_ms: u64,
}

/// File search options
#[derive(Debug, Clone)]
pub struct FileSearchOptions {
    pub include_content: bool,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub whole_word: bool,
    pub max_results: Option<usize>,
    pub file_extensions: Option<Vec<String>>,
    /// Whether to include directories in the search results
    pub include_directories: bool,
}

impl Default for FileSearchOptions {
    fn default() -> Self {
        Self {
            include_content: false,
            case_sensitive: false,
            use_regex: false,
            whole_word: false,
            max_results: None, // No limit
            file_extensions: None,
            include_directories: true, // Include directories by default
        }
    }
}

/// Directory statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct DirectoryStats {
    pub total_files: usize,
    pub total_directories: usize,
    pub total_size_bytes: u64,
    pub total_size_mb: u64,
    pub max_depth: u32,
    pub most_common_extensions: Vec<(String, usize)>,
    pub large_files_count: usize,
    pub hidden_files_count: usize,
    pub symlinks_count: usize,
}
