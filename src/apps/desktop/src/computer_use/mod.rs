//! Desktop Computer use host (screenshots + enigo).

mod desktop_host;
mod ui_locate_common;
mod screen_ocr;
#[cfg(target_os = "macos")]
mod macos_ax_ui;
#[cfg(target_os = "windows")]
mod windows_ax_ui;
#[cfg(target_os = "linux")]
mod linux_ax_ui;

pub use desktop_host::DesktopComputerUseHost;
