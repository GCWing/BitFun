//! Desktop Computer use host (screenshots + enigo).

mod desktop_host;
#[cfg(target_os = "linux")]
mod linux_ax_ui;
#[cfg(target_os = "macos")]
mod macos_ax_ui;
mod screen_ocr;
mod ui_locate_common;
#[cfg(target_os = "windows")]
mod windows_ax_ui;

pub use desktop_host::DesktopComputerUseHost;
