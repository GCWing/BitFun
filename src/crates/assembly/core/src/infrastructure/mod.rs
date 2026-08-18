//! Infrastructure module
//!
//! Provides low-level services: AI clients, storage, event system

#[cfg(feature = "ai-adapter-runtime")]
pub mod ai;
pub mod app_paths;
pub mod events;
#[cfg(feature = "filesystem")]
pub mod filesystem;
#[cfg(feature = "local-storage")]
pub mod storage;
#[cfg(all(feature = "ai-adapter-runtime", feature = "subscription-auth"))]
pub mod subscription_auth;

#[cfg(feature = "ai-adapter-runtime")]
pub use ai::AIClient;
pub use app_paths::{get_path_manager_arc, try_get_path_manager_arc, PathManager, StorageLevel};
#[cfg(feature = "runtime-services")]
pub use events::BackendEventManager;
#[cfg(feature = "filesystem")]
pub use filesystem::{
    BatchedFileSearchProgressSink, FileContentSearchOptions, FileInfo, FileNameSearchOptions,
    FileOperationOptions, FileOperationService, FileReadResult, FileSearchOutcome,
    FileSearchProgressSink, FileSearchResult, FileSearchResultGroup, FileTreeNode, FileTreeOptions,
    FileTreeService, FileTreeStatistics, FileWriteResult, SearchMatchType,
};
// pub use storage::{};
