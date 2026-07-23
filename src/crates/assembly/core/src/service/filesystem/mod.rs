//! File system service module
//!
//! Integrates file operations, file tree building, search, and related functionality.

pub mod factory;
pub mod listing;
pub mod service;
pub mod types;

pub use factory::FileSystemServiceFactory;
pub use listing::{
    format_directory_listing, get_formatted_directory_listing, list_directory_entries,
    DirectoryListingEntry, FormattedDirectoryListing,
};
pub use service::FileSystemService;
pub use types::{DirectoryScanResult, DirectoryStats, FileSearchOptions, FileSystemConfig};
