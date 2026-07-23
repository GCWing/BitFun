//! Filesystem infrastructure compatibility facade.

pub use bitfun_services_core::filesystem::{
    normalize_text_for_editor_disk_sync, BatchedFileSearchProgressSink, FileContentSearchOptions,
    FileInfo, FileNameSearchOptions, FileOperationOptions, FileOperationService, FileReadResult,
    FileSearchOutcome, FileSearchProgressSink, FileSearchResult, FileSearchResultGroup,
    FileTreeNode, FileTreeOptions, FileTreeService, FileTreeStatistics, FileWriteResult,
    SearchMatchType,
};
