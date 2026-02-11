//! Filesystem infrastructure
//!
//! File operations, file tree building, file watching, and path management.

pub mod file_operations;
pub mod file_tree;
pub mod file_watcher;
pub mod path_manager;

pub use file_operations::{
    FileInfo, FileOperationOptions, FileOperationService, FileReadResult, FileWriteResult,
};
pub use file_tree::{
    FileSearchResult, FileTreeNode, FileTreeOptions, FileTreeService, FileTreeStatistics,
    SearchMatchType,
};
pub use file_watcher::initialize_file_watcher;
#[cfg(feature = "tauri-support")]
pub use file_watcher::{get_watched_paths, start_file_watch, stop_file_watch};
pub use path_manager::{
    get_path_manager_arc, try_get_path_manager_arc, CacheType, PathManager, StorageLevel,
};
