//! Application path infrastructure.
//!
//! Centralizes path policy for user data, caches, sessions, and workspace-adjacent storage.

pub mod path_manager;

pub use path_manager::{get_path_manager_arc, try_get_path_manager_arc, PathManager, StorageLevel};
