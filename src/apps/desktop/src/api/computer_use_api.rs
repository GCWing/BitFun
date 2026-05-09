//! Tauri commands for Computer use (permissions + settings deep links).

use crate::api::app_state::AppState;
use crate::computer_use::DesktopComputerUseHost;
use bitfun_core::agentic::tools::computer_use_host::ComputerUseHost;
use bitfun_core::service::config::types::AIConfig;
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
    let ai: AIConfig = state
        .config_service
        .get_config(Some("ai"))
        .await
        .map_err(|e| e.to_string())?;

    let host = DesktopComputerUseHost::new();
    let snap = host
        .permission_snapshot()
        .await
        .map_err(|e| e.to_string())?;

    Ok(ComputerUseStatusResponse {
        computer_use_enabled: ai.computer_use_enabled,
        accessibility_granted: snap.accessibility_granted,
        screen_capture_granted: snap.screen_capture_granted,
        platform_note: snap.platform_note,
    })
}

#[tauri::command]
pub async fn computer_use_request_permissions() -> Result<(), String> {
    let host = DesktopComputerUseHost::new();
    host.prompt_for_missing_permissions();
    Ok(())
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
