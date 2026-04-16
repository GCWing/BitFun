//! Detect and launch the user's default browser with CDP debug port enabled.

use crate::util::errors::{BitFunError, BitFunResult};
#[allow(unused_imports)]
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::process::Command;

/// Default CDP debug port.
pub const DEFAULT_CDP_PORT: u16 = 9222;

/// Known browser identifiers and their executable paths per platform.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BrowserKind {
    Chrome,
    Edge,
    Chromium,
    Brave,
    Arc,
    Unknown(String),
}

impl std::fmt::Display for BrowserKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrowserKind::Chrome => write!(f, "Google Chrome"),
            BrowserKind::Edge => write!(f, "Microsoft Edge"),
            BrowserKind::Chromium => write!(f, "Chromium"),
            BrowserKind::Brave => write!(f, "Brave Browser"),
            BrowserKind::Arc => write!(f, "Arc"),
            BrowserKind::Unknown(name) => write!(f, "{}", name),
        }
    }
}

/// Result of browser detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserInfo {
    pub kind: BrowserKind,
    pub path: String,
    pub is_running: bool,
    pub cdp_available: bool,
}

pub struct BrowserLauncher;

impl BrowserLauncher {
    /// Check if a CDP debug port is already listening.
    pub async fn is_cdp_available(port: u16) -> bool {
        let url = format!("http://127.0.0.1:{}/json/version", port);
        reqwest::Client::new()
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Detect the user's default browser on the current platform.
    pub fn detect_default_browser() -> BitFunResult<BrowserKind> {
        #[cfg(target_os = "macos")]
        {
            Self::detect_default_browser_macos()
        }
        #[cfg(target_os = "windows")]
        {
            Self::detect_default_browser_windows()
        }
        #[cfg(target_os = "linux")]
        {
            Self::detect_default_browser_linux()
        }
    }

    #[cfg(target_os = "macos")]
    fn detect_default_browser_macos() -> BitFunResult<BrowserKind> {
        let output = Command::new("defaults")
            .args([
                "read",
                "com.apple.LaunchServices/com.apple.launchservices.secure",
                "LSHandlers",
            ])
            .output()
            .ok();

        if let Some(out) = output {
            let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if text.contains("com.google.chrome") {
                return Ok(BrowserKind::Chrome);
            } else if text.contains("com.microsoft.edgemac") {
                return Ok(BrowserKind::Edge);
            } else if text.contains("com.brave.browser") {
                return Ok(BrowserKind::Brave);
            } else if text.contains("company.thebrowser.browser") {
                return Ok(BrowserKind::Arc);
            }
        }

        // Fallback: check which browsers are installed
        let browsers = [
            ("/Applications/Google Chrome.app", BrowserKind::Chrome),
            ("/Applications/Microsoft Edge.app", BrowserKind::Edge),
            ("/Applications/Brave Browser.app", BrowserKind::Brave),
            ("/Applications/Arc.app", BrowserKind::Arc),
            ("/Applications/Chromium.app", BrowserKind::Chromium),
        ];

        for (path, kind) in &browsers {
            if std::path::Path::new(path).exists() {
                debug!("Found browser at {}", path);
                return Ok(kind.clone());
            }
        }

        Ok(BrowserKind::Chrome)
    }

    #[cfg(target_os = "windows")]
    fn detect_default_browser_windows() -> BitFunResult<BrowserKind> {
        let output = Command::new("reg")
            .args([
                "query",
                r"HKEY_CURRENT_USER\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\http\UserChoice",
                "/v",
                "ProgId",
            ])
            .output()
            .ok();

        if let Some(out) = output {
            let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if text.contains("chrome") {
                return Ok(BrowserKind::Chrome);
            } else if text.contains("edge") {
                return Ok(BrowserKind::Edge);
            } else if text.contains("brave") {
                return Ok(BrowserKind::Brave);
            }
        }

        Ok(BrowserKind::Chrome)
    }

    #[cfg(target_os = "linux")]
    fn detect_default_browser_linux() -> BitFunResult<BrowserKind> {
        let output = Command::new("xdg-settings")
            .args(["get", "default-web-browser"])
            .output()
            .ok();

        if let Some(out) = output {
            let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if text.contains("chrome") || text.contains("google") {
                return Ok(BrowserKind::Chrome);
            } else if text.contains("edge") || text.contains("microsoft") {
                return Ok(BrowserKind::Edge);
            } else if text.contains("brave") {
                return Ok(BrowserKind::Brave);
            } else if text.contains("chromium") {
                return Ok(BrowserKind::Chromium);
            }
        }

        Ok(BrowserKind::Chrome)
    }

    /// Get the executable path or launch command for a browser kind.
    pub fn browser_executable(kind: &BrowserKind) -> String {
        #[cfg(target_os = "macos")]
        {
            match kind {
                BrowserKind::Chrome => {
                    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into()
                }
                BrowserKind::Edge => {
                    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge".into()
                }
                BrowserKind::Brave => {
                    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser".into()
                }
                BrowserKind::Arc => "/Applications/Arc.app/Contents/MacOS/Arc".into(),
                BrowserKind::Chromium => {
                    "/Applications/Chromium.app/Contents/MacOS/Chromium".into()
                }
                BrowserKind::Unknown(name) => name.clone(),
            }
        }

        #[cfg(target_os = "windows")]
        {
            match kind {
                BrowserKind::Chrome => {
                    r"C:\Program Files\Google\Chrome\Application\chrome.exe".into()
                }
                BrowserKind::Edge => {
                    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe".into()
                }
                BrowserKind::Brave => {
                    r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe".into()
                }
                BrowserKind::Chromium => "chromium.exe".into(),
                BrowserKind::Arc => "arc.exe".into(),
                BrowserKind::Unknown(name) => name.clone(),
            }
        }

        #[cfg(target_os = "linux")]
        {
            match kind {
                BrowserKind::Chrome => "google-chrome".into(),
                BrowserKind::Edge => "microsoft-edge".into(),
                BrowserKind::Brave => "brave-browser".into(),
                BrowserKind::Chromium => "chromium-browser".into(),
                BrowserKind::Arc => "arc".into(),
                BrowserKind::Unknown(name) => name.clone(),
            }
        }
    }

    /// Launch the browser with the CDP debug port flag.
    /// Returns instructions if the browser is already running without CDP.
    pub async fn launch_with_cdp(kind: &BrowserKind, port: u16) -> BitFunResult<LaunchResult> {
        if Self::is_cdp_available(port).await {
            info!("CDP already available on port {} for {}", port, kind);
            return Ok(LaunchResult::AlreadyConnected);
        }

        let exe = Self::browser_executable(kind);
        let flag = format!("--remote-debugging-port={}", port);

        // Check if the browser process is already running (without CDP)
        let is_running = Self::is_browser_running(kind);

        if is_running {
            let instructions = format!(
                "Your {} is currently running without the CDP debug port. \
                 Please quit the browser completely (Cmd+Q / Ctrl+Q) and \
                 then I will relaunch it with the debug port enabled.\n\
                 Alternatively, you can restart it manually:\n  \"{}\" {}",
                kind, exe, flag
            );
            return Ok(LaunchResult::BrowserRunningWithoutCdp {
                browser: kind.to_string(),
                executable: exe,
                port,
                instructions,
            });
        }

        info!("Launching {} with CDP on port {}", kind, port);
        let result = Command::new(&exe).arg(&flag).spawn();

        match result {
            Ok(_child) => {
                // Wait a moment for the browser to start
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;

                if Self::is_cdp_available(port).await {
                    Ok(LaunchResult::Launched)
                } else {
                    Ok(LaunchResult::LaunchedButCdpNotReady {
                        port,
                        message: format!(
                            "{} was launched but CDP is not yet responding on port {}. \
                             It may need a few more seconds to initialize.",
                            kind, port
                        ),
                    })
                }
            }
            Err(e) => Err(BitFunError::tool(format!(
                "Failed to launch {}: {}",
                kind, e
            ))),
        }
    }

    /// Check if a browser process is currently running.
    fn is_browser_running(kind: &BrowserKind) -> bool {
        let process_names = match kind {
            BrowserKind::Chrome => vec!["Google Chrome", "chrome"],
            BrowserKind::Edge => vec!["Microsoft Edge", "msedge"],
            BrowserKind::Brave => vec!["Brave Browser", "brave"],
            BrowserKind::Arc => vec!["Arc"],
            BrowserKind::Chromium => vec!["Chromium", "chromium"],
            BrowserKind::Unknown(_) => return false,
        };

        #[cfg(target_os = "macos")]
        {
            for name in &process_names {
                let output = Command::new("pgrep").args(["-f", name]).output().ok();
                if let Some(out) = output {
                    if out.status.success() && !out.stdout.is_empty() {
                        return true;
                    }
                }
            }
            false
        }

        #[cfg(target_os = "windows")]
        {
            for name in &process_names {
                let output = Command::new("tasklist")
                    .args(["/FI", &format!("IMAGENAME eq {}.exe", name)])
                    .output()
                    .ok();
                if let Some(out) = output {
                    let text = String::from_utf8_lossy(&out.stdout);
                    if text.contains(name) {
                        return true;
                    }
                }
            }
            false
        }

        #[cfg(target_os = "linux")]
        {
            for name in &process_names {
                let output = Command::new("pgrep").args(["-f", name]).output().ok();
                if let Some(out) = output {
                    if out.status.success() && !out.stdout.is_empty() {
                        return true;
                    }
                }
            }
            false
        }
    }

    /// Create a macOS `.app` wrapper that launches the browser with CDP enabled.
    #[cfg(target_os = "macos")]
    pub fn create_cdp_launcher_app(kind: &BrowserKind, port: u16) -> BitFunResult<String> {
        let app_name = format!("{} Debug", kind);
        let app_dir = format!("/Applications/{}.app", app_name);
        let macos_dir = format!("{}/Contents/MacOS", app_dir);
        let script_path = format!("{}/launch", macos_dir);
        let exe = Self::browser_executable(kind);

        std::fs::create_dir_all(&macos_dir)
            .map_err(|e| BitFunError::tool(format!("Failed to create app bundle: {}", e)))?;

        let script = format!(
            "#!/bin/bash\nexec \"{}\" --remote-debugging-port={} \"$@\"\n",
            exe, port
        );
        std::fs::write(&script_path, &script)
            .map_err(|e| BitFunError::tool(format!("Failed to write launcher script: {}", e)))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| {
                    BitFunError::tool(format!("Failed to set executable permission: {}", e))
                })?;
        }

        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>{}</string>
    <key>CFBundleExecutable</key>
    <string>launch</string>
    <key>CFBundleIdentifier</key>
    <string>com.bitfun.browser-debug-launcher</string>
</dict>
</plist>"#,
            app_name
        );

        std::fs::write(format!("{}/Contents/Info.plist", app_dir), &plist)
            .map_err(|e| BitFunError::tool(format!("Failed to write Info.plist: {}", e)))?;

        info!("Created CDP launcher app at {}", app_dir);
        Ok(app_dir)
    }
}

/// Result of a browser launch attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LaunchResult {
    AlreadyConnected,
    Launched,
    LaunchedButCdpNotReady {
        port: u16,
        message: String,
    },
    BrowserRunningWithoutCdp {
        browser: String,
        executable: String,
        port: u16,
        instructions: String,
    },
}
