//! LSP (Language Server Protocol) service module
//!
//! Provides full LSP support, including:
//! - Plugin management (install/uninstall/load)
//! - Server process lifecycle management
//! - LSP protocol communication
//! - Code completion, navigation, diagnostics, and more

pub mod config_watcher;
pub mod debouncer;
pub mod file_sync;
pub mod global;
pub mod manager;
pub mod plugin_loader;
pub mod process;
pub mod project_detector;
pub mod protocol;
pub mod registry;
pub mod types;
pub mod workspace_manager;

pub use global::{
    close_workspace, get_all_workspace_paths, get_global_lsp_manager, get_workspace_manager,
    initialize_global_lsp_manager, is_lsp_manager_initialized, open_workspace,
    open_workspace_with_emitter,
};
pub use manager::LspManager;
pub use project_detector::{ProjectDetector, ProjectInfo};
pub use types::{CompletionItem, LspPlugin, PluginSource};
pub use workspace_manager::{LspEvent, ServerState, ServerStatus, WorkspaceLspManager};
