//! Infrastructure module
//!
//! Provides low-level services: AI clients, storage, event system

pub mod ai;
pub mod debug_log;
pub mod events;
pub mod filesystem;
pub mod storage;

pub use ai::AIClient;
pub use events::BackendEventManager;
pub use filesystem::{
    file_watcher, get_path_manager_arc, initialize_file_watcher, try_get_path_manager_arc,
    BatchedFileSearchProgressSink, FileContentSearchOptions, FileInfo, FileNameSearchOptions,
    FileOperationOptions, FileOperationService, FileReadResult, FileSearchOutcome,
    FileSearchProgressSink, FileSearchResult, FileSearchResultGroup, FileTreeNode, FileTreeOptions,
    FileTreeService, FileTreeStatistics, FileWriteResult, PathManager, SearchMatchType,
};
// pub use storage::{};
