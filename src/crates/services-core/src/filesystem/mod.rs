//! Platform-neutral filesystem owner.
//!
//! This module owns local file operations, directory listings, file-tree
//! construction, and search primitives. Product/runtime adapters in
//! `bitfun-core` may still layer remote-workspace routing or legacy error
//! mapping on top of these primitives.

mod error;
mod factory;
mod listing;
mod operations;
mod service;
mod tree;
mod types;

pub use error::{FileSystemError, FileSystemResult};
pub use factory::FileSystemServiceFactory;
pub use listing::{
    format_directory_listing, get_formatted_directory_listing, list_directory_entries,
    DirectoryListingEntry, FormattedDirectoryListing,
};
pub use operations::{
    normalize_text_for_editor_disk_sync, FileInfo, FileOperationOptions, FileOperationService,
    FileReadResult, FileWriteResult,
};
pub use service::FileSystemService;
pub use tree::{
    BatchedFileSearchProgressSink, FileContentSearchOptions, FileNameSearchOptions,
    FileSearchOutcome, FileSearchProgressSink, FileSearchResult, FileSearchResultGroup,
    FileTreeNode, FileTreeOptions, FileTreeService, FileTreeStatistics, SearchMatchType,
};
pub use types::{DirectoryScanResult, DirectoryStats, FileSearchOptions, FileSystemConfig};
