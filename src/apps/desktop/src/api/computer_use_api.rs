//! Tauri commands for Computer use (permissions + settings deep links).

use crate::api::app_state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseStatusResponse {
    pub computer_use_enabled: bool,
    pub accessibility_granted: bool,
    pub screen_capture_granted: bool,
    pub platform_note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseOpenSettingsRequest {
    /// `accessibility` | `screen_capture`
    pub pane: String,
}

#[tauri::command]
pub async fn computer_use_get_status(
    state: State<'_, AppState>,
) -> Result<ComputerUseStatusResponse, String> {
    Err("computer_use_get_status error".to_string())
}

#[tauri::command]
pub async fn computer_use_request_permissions() -> Result<(), String> {
    Err("computer_use_request_permissions error".to_string())
}

#[tauri::command]
pub async fn computer_use_open_system_settings(
    request: ComputerUseOpenSettingsRequest,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let url = match request.pane.as_str() {
            "accessibility" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            "screen_capture" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
            }
            _ => return Err(format!("Unknown settings pane: {}", request.pane)),
        };
        std::process::Command::new("open")
            .arg(url)
            .status()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        let _ = request;
        Err("Open system settings is not wired for Windows yet.".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        let _ = request;
        return Err(
            "Open system settings: use your desktop environment privacy settings.".to_string(),
        );
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = request;
        Err("Unsupported platform.".to_string())
    }
}
