//! Desktop Computer use host (screenshots + enigo).

mod desktop_host;
#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
mod linux_ax_ui;
#[cfg(target_os = "macos")]
mod macos_ax_ui;
#[cfg(not(target_env = "ohos"))]
mod screen_ocr;
#[cfg(not(target_env = "ohos"))]
mod ui_locate_common;
#[cfg(target_os = "windows")]
mod windows_ax_ui;

pub use desktop_host::DesktopComputerUseHost;
