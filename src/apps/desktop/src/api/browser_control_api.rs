//! Browser control API — Tauri commands for CDP-based browser control.

use bitfun_core::agentic::tools::browser_control::browser_launcher::{
    BrowserKind, BrowserLauncher, LaunchResult, DEFAULT_CDP_PORT,
};
use bitfun_core::agentic::tools::browser_control::cdp_client::CdpClient;
use bitfun_core::service::config::{get_global_config_service, GlobalConfig};
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

async fn selected_browser_kind() -> Result<BrowserKind, String> {
    let config = get_global_config_service()
        .await
        .map_err(|e| e.to_string())?
        .get_config::<GlobalConfig>(None)
        .await
        .map_err(|e| e.to_string())?;
    BrowserLauncher::resolve_browser_kind(Some(&config.ai.browser_control_preferred_browser))
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserControlBrowserOption {
    pub value: String,
    pub label: String,
    pub installed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserControlBrowsersResponse {
    pub options: Vec<BrowserControlBrowserOption>,
}

/// List selectable browsers for CDP browser control.
#[tauri::command]
pub async fn browser_control_list_browsers() -> Result<BrowserControlBrowsersResponse, String> {
    let browsers = [
        ("default", "Default browser", true),
        (
            "chrome",
            "Google Chrome",
            BrowserLauncher::is_browser_installed(&BrowserKind::Chrome),
        ),
        (
            "edge",
            "Microsoft Edge",
            BrowserLauncher::is_browser_installed(&BrowserKind::Edge),
        ),
        (
            "brave",
            "Brave Browser",
            BrowserLauncher::is_browser_installed(&BrowserKind::Brave),
        ),
        (
            "chromium",
            "Chromium",
            BrowserLauncher::is_browser_installed(&BrowserKind::Chromium),
        ),
        (
            "arc",
            "Arc",
            BrowserLauncher::is_browser_installed(&BrowserKind::Arc),
        ),
    ];

    Ok(BrowserControlBrowsersResponse {
        options: browsers
            .into_iter()
            .map(|(value, label, installed)| BrowserControlBrowserOption {
                value: value.to_string(),
                label: label.to_string(),
                installed,
            })
            .collect(),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserControlStatusResponse {
    pub cdp_available: bool,
    pub default_cdp_supported: bool,
    pub default_cdp_enabled: bool,
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
    let configured_kind = selected_browser_kind().await?;
    let default_cdp_supported = BrowserLauncher::supports_default_cdp(&configured_kind);
    let default_cdp_enabled = BrowserLauncher::is_default_cdp_enabled(&configured_kind);
    let user_profile_connection =
        CdpClient::browser_connection_for_kind(port, &configured_kind).await;
    let legacy_version =
        if user_profile_connection.is_none() && BrowserLauncher::is_cdp_available(port).await {
            CdpClient::get_version(port).await.ok()
        } else {
            None
        };
    // Chrome and Edge share the logical 9222 slot in Settings. Do not report
    // the selected browser as connected merely because the other one owns a
    // legacy fixed-port endpoint left from an earlier selection.
    let legacy_matches_selection = legacy_version.as_ref().is_some_and(|version| {
        let detected = version
            .browser
            .as_deref()
            .and_then(BrowserLauncher::browser_kind_from_cdp_version);
        match &configured_kind {
            BrowserKind::Chrome | BrowserKind::Edge => {
                detected.map(|kind| kind == configured_kind).unwrap_or(true)
            }
            _ => true,
        }
    });
    let available = user_profile_connection.is_some() || legacy_matches_selection;

    let (version, page_count, actual_kind) = if available {
        let ver_info = if let Some(connection) = &user_profile_connection {
            connection.client.browser_version().await.ok()
        } else {
            legacy_version
        };
        let ver = ver_info.as_ref().and_then(|v| v.browser.clone());
        // Identify the actual browser from CDP version response.
        let kind = ver
            .as_deref()
            .and_then(BrowserLauncher::browser_kind_from_cdp_version)
            .unwrap_or_else(|| configured_kind.clone());
        // Only count targets of type "page" (real browser tabs),
        // not service workers, browser targets, etc.
        let pages = if let Some(connection) = &user_profile_connection {
            connection.client.browser_pages().await.ok()
        } else {
            CdpClient::list_pages(port).await.ok()
        }
        .map(|p| {
            p.iter()
                .filter(|t| t.page_type.as_deref() == Some("page"))
                .count()
        })
        .unwrap_or(0);
        (ver, pages, kind)
    } else {
        (None, 0, configured_kind)
    };

    Ok(BrowserControlStatusResponse {
        cdp_available: available,
        default_cdp_supported,
        default_cdp_enabled,
        browser_kind: actual_kind.to_string(),
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
    /// Remote debugging settings URL, sent when the user has to open it
    /// themselves because the platform cannot open a `chrome://` URL for them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup_url: Option<String>,
}

fn to_launch_response(kind: &BrowserKind, result: LaunchResult) -> BrowserControlLaunchResponse {
    match result {
        LaunchResult::AlreadyConnected => BrowserControlLaunchResponse {
            success: true,
            status: "already_connected".into(),
            message: None,
            browser_kind: kind.to_string(),
            setup_url: None,
        },
        LaunchResult::Launched => BrowserControlLaunchResponse {
            success: true,
            status: "launched".into(),
            message: None,
            browser_kind: kind.to_string(),
            setup_url: None,
        },
        LaunchResult::UserProfileReady { .. } => BrowserControlLaunchResponse {
            success: false,
            status: "user_profile_ready".into(),
            message: None,
            browser_kind: kind.to_string(),
            setup_url: None,
        },
        LaunchResult::UserProfileSetupRequired {
            instructions,
            setup_url,
            opened,
            ..
        } => BrowserControlLaunchResponse {
            success: false,
            // The two cases need different guidance: one asks the user to
            // finish on a page that is already in front of them, the other
            // asks them to open that page first.
            status: if opened {
                "requires_user_profile_setup".into()
            } else {
                "requires_manual_user_profile_setup".into()
            },
            message: Some(instructions),
            browser_kind: kind.to_string(),
            setup_url: Some(setup_url),
        },
        LaunchResult::LaunchedButCdpNotReady { message, .. } => BrowserControlLaunchResponse {
            success: false,
            status: "cdp_not_ready".into(),
            message: Some(message),
            browser_kind: kind.to_string(),
            setup_url: None,
        },
        LaunchResult::BrowserRunningWithoutCdp { instructions, .. } => {
            BrowserControlLaunchResponse {
                success: false,
                status: "needs_restart".into(),
                message: Some(instructions),
                browser_kind: kind.to_string(),
                setup_url: None,
            }
        }
    }
}

async fn complete_launch(
    kind: &BrowserKind,
    logical_port: u16,
    result: LaunchResult,
) -> Result<BrowserControlLaunchResponse, String> {
    match result {
        LaunchResult::UserProfileReady { endpoint } => {
            let connection = CdpClient::connect_user_profile_browser(
                logical_port,
                endpoint.port,
                kind,
                &endpoint.web_socket_url,
            )
            .await;
            if let Err(error) = connection {
                return Ok(BrowserControlLaunchResponse {
                    success: false,
                    status: "user_profile_connection_failed".into(),
                    message: Some(error.to_string()),
                    browser_kind: kind.to_string(),
                    setup_url: None,
                });
            }
            Ok(BrowserControlLaunchResponse {
                success: true,
                status: "connected_user_profile".into(),
                message: None,
                browser_kind: kind.to_string(),
                setup_url: None,
            })
        }
        other => Ok(to_launch_response(kind, other)),
    }
}

/// Launch the user's default browser with CDP debug port.
#[tauri::command]
pub async fn browser_control_launch(
    request: BrowserControlLaunchRequest,
) -> Result<BrowserControlLaunchResponse, String> {
    let port = request.port;
    let kind = selected_browser_kind().await?;

    if CdpClient::browser_connection_for_kind(port, &kind)
        .await
        .is_some()
    {
        return Ok(to_launch_response(&kind, LaunchResult::AlreadyConnected));
    }

    // The logical port is shared across browser choices. Drop only the lookup
    // entry when the user switches browsers; any already-attached page session
    // keeps its transport alive, but new actions cannot accidentally reuse it.
    if CdpClient::browser_connection(port).await.is_some() {
        CdpClient::remove_browser_connection(port).await;
    }

    let result = BrowserLauncher::launch_with_cdp(&kind, port)
        .await
        .map_err(|e| e.to_string())?;

    complete_launch(&kind, port, result).await
}

/// Open the selected browser's persistent guarded-CDP setting and wait for the
/// user-owned consent toggle. Once enabled, immediately request and retain the
/// real-profile connection so the Settings action is one continuous flow.
#[tauri::command]
pub async fn browser_control_enable_default_cdp(
    request: BrowserControlLaunchRequest,
) -> Result<BrowserControlLaunchResponse, String> {
    let port = request.port;
    let kind = selected_browser_kind().await?;

    if !BrowserLauncher::supports_default_cdp(&kind) {
        return Ok(BrowserControlLaunchResponse {
            success: false,
            status: "default_cdp_unsupported".into(),
            message: Some(format!(
                "{} does not expose a supported persistent guarded-CDP setting",
                kind
            )),
            browser_kind: kind.to_string(),
            setup_url: None,
        });
    }

    if CdpClient::browser_connection_for_kind(port, &kind)
        .await
        .is_some()
    {
        return Ok(to_launch_response(&kind, LaunchResult::AlreadyConnected));
    }
    if CdpClient::browser_connection(port).await.is_some() {
        CdpClient::remove_browser_connection(port).await;
    }

    let result = BrowserLauncher::enable_default_cdp(&kind, port)
        .await
        .map_err(|e| e.to_string())?;
    complete_launch(&kind, port, result).await
}

/// Restart the user's default browser with CDP debug port enabled.
#[tauri::command]
pub async fn browser_control_restart_with_cdp(
    request: BrowserControlLaunchRequest,
) -> Result<BrowserControlLaunchResponse, String> {
    let port = request.port;
    let kind = selected_browser_kind().await?;

    let result = BrowserLauncher::restart_with_cdp(&kind, port)
        .await
        .map_err(|e| e.to_string())?;

    complete_launch(&kind, port, result).await
}
