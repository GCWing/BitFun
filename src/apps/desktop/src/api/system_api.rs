//! System API

use std::sync::{Arc, Mutex, OnceLock};

use crate::api::app_state::AppState;
use bitfun_core::service::system;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Position, Size, State};
use tauri_plugin_updater::UpdaterExt;

/// Emitted during `install_update` download; matches `installUpdateWithProgress` / frontend listener.
const UPDATE_PROGRESS_EVENT: &str = "bitfun-update-progress";

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgressPayload {
    downloaded: u64,
    total: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfoResponse {
    pub platform: String,
    pub arch: String,
    pub os_version: Option<String>,
}

#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfoResponse, String> {
    let info = system::get_system_info();

    Ok(SystemInfoResponse {
        platform: info.platform,
        arch: info.arch,
        os_version: info.os_version,
    })
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetAppVersionRequest {}

/// Returns the current application version (from `Cargo.toml` / bundle metadata).
#[tauri::command]
pub async fn get_app_version(
    app: AppHandle,
    request: GetAppVersionRequest,
) -> Result<String, String> {
    let _ = request;
    Ok(app.package_info().version.to_string())
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CheckForUpdatesRequest {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckForUpdatesResponse {
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_notes: Option<String>,
    pub release_date: Option<String>,
}

/// Checks the remote updater endpoint for a newer signed release (no download).
#[tauri::command]
pub async fn check_for_updates(
    app: AppHandle,
    request: CheckForUpdatesRequest,
) -> Result<CheckForUpdatesResponse, String> {
    let _ = request;
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    match update {
        Some(u) => Ok(CheckForUpdatesResponse {
            update_available: true,
            current_version: u.current_version.clone(),
            latest_version: Some(u.version.clone()),
            release_notes: u.body.clone(),
            release_date: u.date.map(|d| d.to_string()),
        }),
        None => Ok(CheckForUpdatesResponse {
            update_available: false,
            current_version: app.package_info().version.to_string(),
            latest_version: None,
            release_notes: None,
            release_date: None,
        }),
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallUpdateRequest {}

/// Downloads and installs the latest update from the updater endpoint (re-checks remote).
#[tauri::command]
pub async fn install_update(app: AppHandle, request: InstallUpdateRequest) -> Result<(), String> {
    let _ = request;
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    let Some(update) = update else {
        return Err("No update available".to_string());
    };
    let app_handle = app.clone();
    let progress = Arc::new(Mutex::new((0u64, None::<u64>)));
    let progress_chunk = Arc::clone(&progress);
    let app_chunk = app_handle.clone();
    update
        .download_and_install(
            move |chunk_len, content_len| {
                let (downloaded, total) = {
                    let mut g = progress_chunk
                        .lock()
                        .expect("update progress mutex poisoned");
                    g.0 = g.0.saturating_add(chunk_len as u64);
                    g.1 = g.1.or(content_len);
                    (g.0, g.1)
                };
                let _ = app_chunk.emit(
                    UPDATE_PROGRESS_EVENT,
                    UpdateProgressPayload { downloaded, total },
                );
            },
            {
                let app_done = app_handle.clone();
                let progress_done = Arc::clone(&progress);
                move || {
                    let (downloaded, total) = {
                        let g = progress_done
                            .lock()
                            .expect("update progress mutex poisoned");
                        (g.0, g.1)
                    };
                    let _ = app_done.emit(
                        UPDATE_PROGRESS_EVENT,
                        UpdateProgressPayload { downloaded, total },
                    );
                }
            },
        )
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RestartAppRequest {}

/// Restarts the desktop application after an update has been installed.
#[tauri::command]
#[allow(unreachable_code)]
pub async fn restart_app(app: AppHandle, request: RestartAppRequest) -> Result<(), String> {
    let _ = request;
    app.restart();
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckCommandResponse {
    pub exists: bool,
    pub path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCommandRequest {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: Option<Vec<EnvVar>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandOutputResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMacosEditMenuModeRequest {
    pub mode: crate::macos_menubar::EditMenuMode,
}

#[tauri::command]
pub async fn check_command_exists(command: String) -> Result<CheckCommandResponse, String> {
    let result = system::check_command(&command);

    Ok(CheckCommandResponse {
        exists: result.exists,
        path: result.path,
    })
}

#[tauri::command]
pub async fn check_commands_exist(
    commands: Vec<String>,
) -> Result<Vec<(String, CheckCommandResponse)>, String> {
    let cmd_refs: Vec<&str> = commands.iter().map(|s| s.as_str()).collect();
    let results = system::check_commands(&cmd_refs);

    Ok(results
        .into_iter()
        .map(|(name, result)| {
            (
                name,
                CheckCommandResponse {
                    exists: result.exists,
                    path: result.path,
                },
            )
        })
        .collect())
}

#[tauri::command]
pub async fn run_system_command(
    request: RunCommandRequest,
) -> Result<CommandOutputResponse, String> {
    let env_vars: Option<Vec<(String, String)>> = request
        .env
        .map(|vars| vars.into_iter().map(|v| (v.key, v.value)).collect());

    let env_ref: Option<&[(String, String)]> = env_vars.as_deref();

    let result = system::run_command(
        &request.command,
        &request.args,
        request.cwd.as_deref(),
        env_ref,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(CommandOutputResponse {
        exit_code: result.exit_code,
        stdout: result.stdout,
        stderr: result.stderr,
        success: result.success,
    })
}

#[tauri::command]
pub async fn set_macos_edit_menu_mode(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    request: SetMacosEditMenuModeRequest,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let current_mode = *state.macos_edit_menu_mode.read().await;
        if current_mode == request.mode {
            return Ok(());
        }

        {
            let mut edit_mode = state.macos_edit_menu_mode.write().await;
            *edit_mode = request.mode;
        }

        let language = state
            .config_service
            .get_config::<String>(Some("app.language"))
            .await
            .unwrap_or_else(|_| "zh-CN".to_string());
        let menubar_mode = if state.workspace_path.read().await.is_some() {
            crate::macos_menubar::MenubarMode::Workspace
        } else {
            crate::macos_menubar::MenubarMode::Startup
        };

        crate::macos_menubar::set_macos_menubar_with_mode(
            &app,
            &language,
            menubar_mode,
            request.mode,
        )
        .map_err(|error| error.to_string())?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (&state, &app, &request);
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendNotificationRequest {
    pub title: String,
    pub body: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ToggleMainWindowFullscreenRequest {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleMainWindowFullscreenResponse {
    pub is_fullscreen: bool,
    pub is_maximized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MainWindowFullscreenTransition {
    next_fullscreen: bool,
    should_apply_monitor_bounds_after_enter: bool,
    should_restore_maximized_after_exit: bool,
    next_restore_maximized_after_fullscreen: bool,
}

fn plan_main_window_fullscreen_transition(
    current_fullscreen: bool,
    current_maximized: bool,
    restore_maximized_after_fullscreen: bool,
    apply_maximized_fullscreen_monitor_bounds: bool,
) -> MainWindowFullscreenTransition {
    let next_fullscreen = !current_fullscreen;

    if next_fullscreen {
        MainWindowFullscreenTransition {
            next_fullscreen,
            should_apply_monitor_bounds_after_enter: current_maximized
                && apply_maximized_fullscreen_monitor_bounds,
            should_restore_maximized_after_exit: false,
            next_restore_maximized_after_fullscreen: current_maximized,
        }
    } else {
        MainWindowFullscreenTransition {
            next_fullscreen,
            should_apply_monitor_bounds_after_enter: false,
            should_restore_maximized_after_exit: restore_maximized_after_fullscreen,
            next_restore_maximized_after_fullscreen: false,
        }
    }
}

fn main_window_fullscreen_restore_maximized() -> &'static Mutex<bool> {
    static RESTORE_MAXIMIZED: OnceLock<Mutex<bool>> = OnceLock::new();
    RESTORE_MAXIMIZED.get_or_init(|| Mutex::new(false))
}

fn read_main_window_fullscreen_response(
    window: &tauri::WebviewWindow,
    fallback_fullscreen: bool,
    fallback_maximized: bool,
) -> ToggleMainWindowFullscreenResponse {
    ToggleMainWindowFullscreenResponse {
        is_fullscreen: window.is_fullscreen().unwrap_or(fallback_fullscreen),
        is_maximized: window.is_maximized().unwrap_or(fallback_maximized),
    }
}

// ─── Window / Tray behavior commands ─────────────────────────────────────────

/// Immediately exit the application (used by the "ask" dialog when the user
/// chooses to quit rather than minimize to tray).
#[tauri::command]
pub async fn quit_app(app: tauri::AppHandle) -> Result<(), String> {
    log::info!("Quit requested via quit_app command");
    crate::perform_process_exit_cleanup();
    app.exit(0);
    Ok(())
}

/// Hide the main window so it lives only in the system tray (used by the "ask"
/// dialog when the user chooses to minimize instead of quitting).
#[tauri::command]
pub async fn minimize_to_tray(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
        log::info!("Main window minimized to tray via command");
    }
    Ok(())
}

/// Toggle OS-level fullscreen for the Desktop main window.
///
/// This is intentionally not the same as maximize: maximize fills the normal
/// work area, while fullscreen asks the OS to own the whole monitor surface.
/// This is also intentionally a Desktop shell adapter command, not a remote
/// workspace/session/runtime command; remote workspaces still run inside the
/// same local Desktop window, so fullscreen must not enter transport or core
/// product logic.
/// Keeping the transition in the desktop host avoids frontend code stitching
/// together `set_fullscreen` / `maximize` with visible JS turns.
///
/// Important: do not unmaximize before entering fullscreen. On Windows this
/// briefly restores the normal window bounds, which makes the window origin and
/// size visibly jump before the OS fullscreen transition starts. Fullscreen and
/// maximize are tracked separately so we can remember whether to restore the
/// maximized state after fullscreen exits without touching window geometry on
/// entry.
///
/// Windows note: Tauri/wry fullscreen does not always expand an undecorated
/// maximized window beyond the work area if we call `set_fullscreen(true)`
/// directly. The Windows path therefore keeps the window maximized, enters
/// fullscreen, then applies the current monitor's full bounds as a geometry
/// correction. Never reintroduce `unmaximize`, `hide`, or `show` in this enter
/// path: those expose a restore transition and make repeated F11 toggles feel
/// broken.
#[tauri::command]
pub async fn toggle_main_window_fullscreen(
    app: tauri::AppHandle,
    request: ToggleMainWindowFullscreenRequest,
) -> Result<ToggleMainWindowFullscreenResponse, String> {
    let _ = request;
    let Some(window) = app.get_webview_window("main") else {
        return Err("Main window not found".to_string());
    };

    let current_fullscreen = window.is_fullscreen().map_err(|error| {
        format!("Failed to read main window fullscreen state: {}", error)
    })?;
    let current_maximized = window.is_maximized().map_err(|error| {
        format!("Failed to read main window maximize state: {}", error)
    })?;
    let restore_maximized_after_fullscreen = *main_window_fullscreen_restore_maximized()
        .lock()
        .map_err(|_| "Main window fullscreen restore state is unavailable".to_string())?;

    let transition = plan_main_window_fullscreen_transition(
        current_fullscreen,
        current_maximized,
        restore_maximized_after_fullscreen,
        should_apply_maximized_fullscreen_monitor_bounds(),
    );

    if transition.next_fullscreen {
        if let Err(error) = window.set_fullscreen(true) {
            return Err(format!("Failed to enter main window fullscreen: {}", error));
        }

        if transition.should_apply_monitor_bounds_after_enter {
            apply_main_window_fullscreen_monitor_bounds(&app, &window)?;
        }

        *main_window_fullscreen_restore_maximized()
            .lock()
            .map_err(|_| "Main window fullscreen restore state is unavailable".to_string())? =
            transition.next_restore_maximized_after_fullscreen;

        return Ok(read_main_window_fullscreen_response(
            &window,
            true,
            false,
        ));
    }

    window
        .set_fullscreen(false)
        .map_err(|error| format!("Failed to exit main window fullscreen: {}", error))?;

    let mut restored_maximized = false;
    if transition.should_restore_maximized_after_exit {
        let is_already_maximized = window.is_maximized().unwrap_or(false);
        if !is_already_maximized {
            window.maximize().map_err(|error| {
                format!("Failed to restore maximize after fullscreen: {}", error)
            })?;
        }
        restored_maximized = true;
    }

    *main_window_fullscreen_restore_maximized()
        .lock()
        .map_err(|_| "Main window fullscreen restore state is unavailable".to_string())? =
        transition.next_restore_maximized_after_fullscreen;

    Ok(read_main_window_fullscreen_response(
        &window,
        false,
        restored_maximized,
    ))
}

fn apply_main_window_fullscreen_monitor_bounds(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<(), String> {
    let monitor = window
        .current_monitor()
        .map_err(|error| format!("Failed to read current monitor for fullscreen: {}", error))?
        .or_else(|| app.primary_monitor().ok().flatten())
        .ok_or_else(|| "Failed to resolve monitor for fullscreen".to_string())?;

    window
        .set_position(Position::Physical(*monitor.position()))
        .map_err(|error| format!("Failed to align fullscreen window position: {}", error))?;
    window
        .set_size(Size::Physical(*monitor.size()))
        .map_err(|error| format!("Failed to align fullscreen window size: {}", error))?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn should_apply_maximized_fullscreen_monitor_bounds() -> bool {
    true
}

#[cfg(not(target_os = "windows"))]
fn should_apply_maximized_fullscreen_monitor_bounds() -> bool {
    false
}

/// Send an OS-level desktop notification (Windows toast / macOS notification center).
#[tauri::command]
pub async fn send_system_notification(
    app: tauri::AppHandle,
    request: SendNotificationRequest,
) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;

    let mut builder = app.notification().builder().title(&request.title);
    if let Some(body) = &request.body {
        builder = builder.body(body);
    }
    builder.show().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_window_fullscreen_transition_enters_from_maximized_without_reusing_maximize_state() {
        let transition = plan_main_window_fullscreen_transition(false, true, false, true);

        assert!(transition.next_fullscreen);
        assert!(transition.should_apply_monitor_bounds_after_enter);
        assert!(transition.next_restore_maximized_after_fullscreen);
        assert!(!transition.should_restore_maximized_after_exit);
    }

    #[test]
    fn main_window_fullscreen_transition_exits_and_restores_previous_maximize_state() {
        let transition = plan_main_window_fullscreen_transition(true, false, true, true);

        assert!(!transition.next_fullscreen);
        assert!(!transition.should_apply_monitor_bounds_after_enter);
        assert!(!transition.next_restore_maximized_after_fullscreen);
        assert!(transition.should_restore_maximized_after_exit);
    }

    #[test]
    fn main_window_fullscreen_transition_can_enter_without_masking_geometry() {
        let transition = plan_main_window_fullscreen_transition(false, true, false, false);

        assert!(transition.next_fullscreen);
        assert!(!transition.should_apply_monitor_bounds_after_enter);
        assert!(transition.next_restore_maximized_after_fullscreen);
    }
}
