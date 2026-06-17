//! File watch service.
//!
//! Exposes filesystem watching as a product-facing service instead of a raw infrastructure detail.

pub mod service;
pub mod types;

pub use service::{
    get_global_file_watch_service, get_watched_paths, initialize_file_watch_service,
    start_file_watch, stop_file_watch, FileWatchService,
};
pub use types::{FileWatchEvent, FileWatchEventKind, FileWatcherConfig};
