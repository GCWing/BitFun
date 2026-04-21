//! Browser control API — Tauri commands for CDP-based browser control.

use bitfun_core::agentic::tools::browser_control::browser_launcher::{
    BrowserKind, BrowserLauncher, LaunchResult, DEFAULT_CDP_PORT,
};
use bitfun_core::agentic::tools::browser_control::cdp_client::CdpClient;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserControlStatusRequest {
    #[serde(default = "default_cdp_port")]
    pub port: u16,
}

fn default_cdp_port() -> u16 {
    DEFAULT_CDP_PORT
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserControlStatusResponse {
    pub cdp_available: bool,
    pub browser_kind: String,
    pub browser_version: Option<String>,
    pub port: u16,
    pub page_count: usize,
}

/// Check CDP browser control status.
#[tauri::command]
pub async fn browser_control_get_status(
    request: BrowserControlStatusRequest,
) -> Result<BrowserControlStatusResponse, String> {
    let port = request.port;
    let available = BrowserLauncher::is_cdp_available(port).await;
    let kind = BrowserLauncher::detect_default_browser().unwrap_or(BrowserKind::Chrome);

    let (version, page_count) = if available {
        let ver = CdpClient::get_version(port)
            .await
            .ok()
            .and_then(|v| v.browser);
        let pages = CdpClient::list_pages(port)
            .await
            .ok()
            .map(|p| p.len())
            .unwrap_or(0);
        (ver, pages)
    } else {
        (None, 0)
    };

    Ok(BrowserControlStatusResponse {
        cdp_available: available,
        browser_kind: kind.to_string(),
        browser_version: version,
        port,
        page_count,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserControlLaunchRequest {
    #[serde(default = "default_cdp_port")]
    pub port: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserControlLaunchResponse {
    pub success: bool,
    pub status: String,
    pub message: Option<String>,
    pub browser_kind: String,
}

fn to_launch_response(kind: &BrowserKind, result: LaunchResult) -> BrowserControlLaunchResponse {
    match result {
        LaunchResult::AlreadyConnected => BrowserControlLaunchResponse {
            success: true,
            status: "already_connected".into(),
            message: None,
            browser_kind: kind.to_string(),
        },
        LaunchResult::Launched => BrowserControlLaunchResponse {
            success: true,
            status: "launched".into(),
            message: None,
            browser_kind: kind.to_string(),
        },
        LaunchResult::LaunchedButCdpNotReady { message, .. } => BrowserControlLaunchResponse {
            success: false,
            status: "cdp_not_ready".into(),
            message: Some(message),
            browser_kind: kind.to_string(),
        },
        LaunchResult::BrowserRunningWithoutCdp { instructions, .. } => BrowserControlLaunchResponse {
            success: false,
            status: "needs_restart".into(),
            message: Some(instructions),
            browser_kind: kind.to_string(),
        },
    }
}

/// Launch the user's default browser with CDP debug port.
#[tauri::command]
pub async fn browser_control_launch(
    request: BrowserControlLaunchRequest,
) -> Result<BrowserControlLaunchResponse, String> {
    let port = request.port;
    let kind = BrowserLauncher::detect_default_browser().map_err(|e| e.to_string())?;

    let result = BrowserLauncher::launch_with_cdp(&kind, port)
        .await
        .map_err(|e| e.to_string())?;

    Ok(to_launch_response(&kind, result))
}

/// Restart the user's default browser with CDP debug port enabled.
#[tauri::command]
pub async fn browser_control_restart_with_cdp(
    request: BrowserControlLaunchRequest,
) -> Result<BrowserControlLaunchResponse, String> {
    let port = request.port;
    let kind = BrowserLauncher::detect_default_browser().map_err(|e| e.to_string())?;

    let result = BrowserLauncher::restart_with_cdp(&kind, port)
        .await
        .map_err(|e| e.to_string())?;

    Ok(to_launch_response(&kind, result))
}

/// Create a macOS .app wrapper for the browser with CDP enabled.
#[tauri::command]
pub async fn browser_control_create_launcher() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let kind = BrowserLauncher::detect_default_browser().map_err(|e| e.to_string())?;
        BrowserLauncher::create_cdp_launcher_app(&kind, DEFAULT_CDP_PORT).map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("CDP launcher app creation is only supported on macOS".into())
    }
}
