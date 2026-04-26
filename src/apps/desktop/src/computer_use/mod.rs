//! Desktop Computer use host (screenshots + enigo).

mod desktop_host;
mod interactive_filter;
#[cfg(target_os = "linux")]
mod linux_ax_ui;
#[cfg(target_os = "macos")]
mod macos_ax_dump;
#[cfg(target_os = "macos")]
mod macos_ax_ui;
#[cfg(target_os = "macos")]
mod macos_ax_write;
#[cfg(target_os = "macos")]
mod macos_bg_input;
#[cfg(target_os = "macos")]
mod macos_list_apps;
mod screen_ocr;
mod som_overlay;
mod ui_locate_common;
#[cfg(target_os = "windows")]
mod windows_ax_ui;

pub use desktop_host::DesktopComputerUseHost;
