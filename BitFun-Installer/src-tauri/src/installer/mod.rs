pub mod commands;
pub mod extract;
pub mod types;

#[cfg(target_os = "windows")]
pub mod registry;
#[cfg(target_os = "windows")]
pub mod shortcut;
