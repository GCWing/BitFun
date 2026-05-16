pub mod ai_config;
pub mod commands;
pub mod extract;
pub mod types;

/// Windows main binary file name — must match `src/apps/desktop` `[[bin]]` and Tauri NSIS output.
pub const MAIN_APP_EXE: &str = "bitfun-desktop.exe";

#[cfg(target_os = "windows")]
pub mod registry;
#[cfg(target_os = "windows")]
pub mod shortcut;
