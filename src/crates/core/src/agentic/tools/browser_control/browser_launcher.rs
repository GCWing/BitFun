//! Detect and launch the user's default browser with CDP debug port enabled.

use crate::infrastructure::app_paths::get_path_manager_arc;
use crate::util::{
    errors::{BitFunError, BitFunResult},
    process_manager,
};
#[allow(unused_imports)]
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

/// Default CDP debug port.
pub const DEFAULT_CDP_PORT: u16 = 9222;

/// Build a `Command` that suppresses transient Windows console windows while
/// preserving normal process behavior on other platforms.
fn silent_command<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
    process_manager::create_command(program)
}

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

/// Cache for browser installation status to avoid repeated filesystem checks.
/// The cache is valid for the lifetime of the process since browser installations
/// don't change during a session.
static BROWSER_INSTALL_CACHE: Mutex<Option<HashMap<String, bool>>> = Mutex::new(None);

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
        let output = silent_command("defaults")
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
        let output = silent_command("reg")
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
        let output = silent_command("xdg-settings")
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

    /// Check whether a browser's executable (or app bundle) is present on disk.
    /// Results are cached for the process lifetime since browser installations
    /// don't change during a session.
    pub fn is_browser_installed(kind: &BrowserKind) -> bool {
        // Unknown browsers are never considered installed.
        if matches!(kind, BrowserKind::Unknown(_)) {
            return false;
        }

        let cache_key = format!("{:?}", kind);

        // Check cache first.
        {
            let cache = BROWSER_INSTALL_CACHE.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(ref map) = *cache {
                if let Some(&cached) = map.get(&cache_key) {
                    return cached;
                }
            }
        }

        // Compute the result.
        let result = Self::check_browser_installed_impl(kind);

        // Store in cache.
        {
            let mut cache = BROWSER_INSTALL_CACHE.lock().unwrap_or_else(|e| e.into_inner());
            let map = cache.get_or_insert_with(HashMap::new);
            map.insert(cache_key, result);
        }

        debug!("Browser {:?} installed: {}", kind, result);
        result
    }

    /// Internal implementation of browser installation check.
    fn check_browser_installed_impl(kind: &BrowserKind) -> bool {
        let exe = Self::browser_executable(kind);
        #[cfg(target_os = "macos")]
        {
            // On macOS, check the .app bundle instead of the inner executable
            let app_path = match kind {
                BrowserKind::Chrome => "/Applications/Google Chrome.app",
                BrowserKind::Edge => "/Applications/Microsoft Edge.app",
                BrowserKind::Brave => "/Applications/Brave Browser.app",
                BrowserKind::Arc => "/Applications/Arc.app",
                BrowserKind::Chromium => "/Applications/Chromium.app",
                BrowserKind::Unknown(_) => "",
            };
            if !app_path.is_empty() {
                return std::path::Path::new(app_path).exists();
            }
        }
        std::path::Path::new(&exe).exists()
    }

    /// Clear the browser installation cache. Useful for testing or when
    /// browser installations might have changed.
    #[cfg(test)]
    pub fn clear_install_cache() {
        let mut cache = BROWSER_INSTALL_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        *cache = None;
    }

    pub fn browser_kind_from_config(value: &str) -> Option<BrowserKind> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "default" => None,
            "chrome" | "google-chrome" | "google_chrome" => Some(BrowserKind::Chrome),
            "edge" | "microsoft-edge" | "microsoft_edge" => Some(BrowserKind::Edge),
            "chromium" => Some(BrowserKind::Chromium),
            "brave" | "brave-browser" | "brave_browser" => Some(BrowserKind::Brave),
            "arc" => Some(BrowserKind::Arc),
            other => Some(BrowserKind::Unknown(other.to_string())),
        }
    }

    pub fn resolve_browser_kind(preferred_browser: Option<&str>) -> BitFunResult<BrowserKind> {
        if let Some(kind) = preferred_browser.and_then(Self::browser_kind_from_config) {
            Ok(kind)
        } else {
            Self::detect_default_browser()
        }
    }

    fn browser_profile_slug(kind: &BrowserKind) -> String {
        match kind {
            BrowserKind::Chrome => "chrome".to_string(),
            BrowserKind::Edge => "edge".to_string(),
            BrowserKind::Chromium => "chromium".to_string(),
            BrowserKind::Brave => "brave".to_string(),
            BrowserKind::Arc => "arc".to_string(),
            BrowserKind::Unknown(name) => name
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
                .collect::<String>()
                .trim_matches('-')
                .to_string(),
        }
    }

    fn managed_user_data_dir(kind: &BrowserKind) -> PathBuf {
        get_path_manager_arc()
            .user_data_dir()
            .join("browser-control")
            .join(Self::browser_profile_slug(kind))
    }

    fn ensure_managed_user_data_dir(kind: &BrowserKind) -> BitFunResult<PathBuf> {
        let dir = Self::managed_user_data_dir(kind);
        std::fs::create_dir_all(&dir).map_err(|e| {
            BitFunError::tool(format!(
                "Failed to create browser control profile directory: {}",
                e
            ))
        })?;
        Ok(dir)
    }

    #[cfg(target_os = "macos")]
    fn launch_app_name(kind: &BrowserKind) -> Option<&'static str> {
        match kind {
            BrowserKind::Chrome => Some("Google Chrome"),
            BrowserKind::Edge => Some("Microsoft Edge"),
            BrowserKind::Brave => Some("Brave Browser"),
            BrowserKind::Arc => Some("Arc"),
            BrowserKind::Chromium => Some("Chromium"),
            BrowserKind::Unknown(_) => None,
        }
    }

    #[cfg(target_os = "macos")]
    fn spawn_macos_browser(kind: &BrowserKind, exe: &str, args: &[String]) -> std::io::Result<std::process::Child> {
        if let Some(app_name) = Self::launch_app_name(kind) {
            let mut command = silent_command("open");
            command.args(["-na", app_name, "--args"]);
            command.args(args);
            command.spawn()
        } else {
            silent_command(exe).args(args).spawn()
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn spawn_browser(_kind: &BrowserKind, exe: &str, args: &[String]) -> std::io::Result<std::process::Child> {
        silent_command(exe).args(args).spawn()
    }

    #[cfg(target_os = "macos")]
    fn spawn_browser(kind: &BrowserKind, exe: &str, args: &[String]) -> std::io::Result<std::process::Child> {
        Self::spawn_macos_browser(kind, exe, args)
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
            Self::windows_browser_executable(kind)
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

    /// Windows: resolve a browser's executable path by probing common install
    /// locations (Program Files / Program Files (x86) / per-user LocalAppData)
    /// and then falling back to the registry "App Paths" entry.
    #[cfg(target_os = "windows")]
    fn windows_browser_executable(kind: &BrowserKind) -> String {
        let (rel_paths, app_paths_key, fallback_cmd) = match kind {
            BrowserKind::Chrome => (
                vec![r"Google\Chrome\Application\chrome.exe"],
                Some("chrome.exe"),
                "chrome.exe",
            ),
            BrowserKind::Edge => (
                vec![r"Microsoft\Edge\Application\msedge.exe"],
                Some("msedge.exe"),
                "msedge.exe",
            ),
            BrowserKind::Brave => (
                vec![r"BraveSoftware\Brave-Browser\Application\brave.exe"],
                Some("brave.exe"),
                "brave.exe",
            ),
            BrowserKind::Chromium => (
                vec![r"Chromium\Application\chrome.exe"],
                None,
                "chromium.exe",
            ),
            BrowserKind::Arc => (vec![r"Arc\Arc.exe"], None, "arc.exe"),
            BrowserKind::Unknown(name) => return name.clone(),
        };

        let env_roots = [
            std::env::var("ProgramFiles").ok(),
            std::env::var("ProgramFiles(x86)").ok(),
            std::env::var("ProgramW6432").ok(),
            std::env::var("LOCALAPPDATA").ok(),
        ];

        for root_opt in &env_roots {
            if let Some(root) = root_opt {
                for rel in &rel_paths {
                    let candidate = format!(r"{}\{}", root.trim_end_matches('\\'), rel);
                    if std::path::Path::new(&candidate).exists() {
                        debug!("Found browser at {}", candidate);
                        return candidate;
                    }
                }
            }
        }

        // App Paths registry fallback: HKLM/HKCU \Software\Microsoft\Windows
        // \CurrentVersion\App Paths\<exe>  default value points to the .exe.
        if let Some(exe_name) = app_paths_key {
            for root in &["HKCU", "HKLM"] {
                let key = format!(
                    r"{}\Software\Microsoft\Windows\CurrentVersion\App Paths\{}",
                    root, exe_name
                );
                let output = silent_command("reg")
                    .args(["query", &key, "/ve"])
                    .output()
                    .ok();
                if let Some(out) = output {
                    let text = String::from_utf8_lossy(&out.stdout);
                    // Line looks like:  (Default)    REG_SZ    C:\Path\to\app.exe
                    for line in text.lines() {
                        let lower = line.to_ascii_lowercase();
                        if lower.contains("reg_sz") {
                            if let Some(idx) = lower.find("reg_sz") {
                                let value = line[idx + "REG_SZ".len()..].trim();
                                let unquoted = value.trim_matches('"').trim();
                                if !unquoted.is_empty() && std::path::Path::new(unquoted).exists() {
                                    debug!("Resolved {} via App Paths: {}", exe_name, unquoted);
                                    return unquoted.to_string();
                                }
                            }
                        }
                    }
                }
            }
        }

        fallback_cmd.into()
    }

    /// Launch the browser with the CDP debug port flag.
    /// Returns instructions if the browser is already running without CDP.
    pub async fn launch_with_cdp(kind: &BrowserKind, port: u16) -> BitFunResult<LaunchResult> {
        Self::launch_with_cdp_opts(kind, port, None).await
    }

    /// Same as [`launch_with_cdp`] but allows passing an isolated
    /// `--user-data-dir`. When the user is already running their main
    /// browser without CDP, an isolated profile lets us start a sibling
    /// instance with debugging enabled instead of asking them to quit.
    pub async fn launch_with_cdp_opts(
        kind: &BrowserKind,
        port: u16,
        user_data_dir: Option<&str>,
    ) -> BitFunResult<LaunchResult> {
        if Self::is_cdp_available(port).await {
            info!("CDP already available on port {} for {}", port, kind);
            return Ok(LaunchResult::AlreadyConnected);
        }

        let exe = Self::browser_executable(kind);
        let profile_dir = match user_data_dir {
            Some(dir) => Path::new(dir).to_path_buf(),
            None => Self::ensure_managed_user_data_dir(kind)?,
        };
        let flag = format!("--remote-debugging-port={}", port);
        let profile_flag = format!("--user-data-dir={}", profile_dir.display());
        let extra: Vec<String> = vec![
            flag.clone(),
            profile_flag,
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
        ];

        info!(
            "Launching {} with CDP on port {} (user_data_dir={})",
            kind,
            port,
            profile_dir.display()
        );
        let result = Self::spawn_browser(kind, &exe, &extra);

        match result {
            Ok(_child) => {
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

    pub async fn restart_with_cdp(kind: &BrowserKind, port: u16) -> BitFunResult<LaunchResult> {
        Self::launch_with_cdp_opts(kind, port, None).await
    }

    #[allow(dead_code)]
    fn terminate_browser(kind: &BrowserKind) -> BitFunResult<()> {
        #[cfg(target_os = "macos")]
        {
            let app_name = match kind {
                BrowserKind::Chrome => "Google Chrome",
                BrowserKind::Edge => "Microsoft Edge",
                BrowserKind::Brave => "Brave Browser",
                BrowserKind::Arc => "Arc",
                BrowserKind::Chromium => "Chromium",
                BrowserKind::Unknown(name) => name.as_str(),
            };
            let script = format!(
                "tell application \"{}\" to quit",
                app_name.replace('"', "\\\"")
            );
            let output = silent_command("osascript")
                .args(["-e", &script])
                .output()
                .map_err(|e| BitFunError::tool(format!("Failed to quit {}: {}", kind, e)))?;
            if output.status.success() {
                return Ok(());
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BitFunError::tool(format!(
                "Failed to quit {}: {}",
                kind,
                stderr.trim()
            )));
        }

        #[cfg(target_os = "windows")]
        {
            let process_names: &[&str] = match kind {
                BrowserKind::Chrome => &["chrome.exe"],
                BrowserKind::Edge => &["msedge.exe"],
                BrowserKind::Brave => &["brave.exe"],
                BrowserKind::Arc => &["arc.exe"],
                BrowserKind::Chromium => &["chromium.exe", "chrome.exe"],
                BrowserKind::Unknown(_) => {
                    return Err(BitFunError::tool(
                        "Unsupported browser kind for restart on Windows".to_string(),
                    ))
                }
            };
            for process_name in process_names {
                let output = silent_command("taskkill")
                    .args(["/IM", process_name, "/F"])
                    .output()
                    .map_err(|e| {
                        BitFunError::tool(format!("Failed to terminate {}: {}", process_name, e))
                    })?;
                let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
                let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
                if output.status.success()
                    || stdout.contains("no instance")
                    || stdout.contains("not found")
                    || stderr.contains("no instance")
                    || stderr.contains("not found")
                {
                    continue;
                }
                return Err(BitFunError::tool(format!(
                    "Failed to terminate {}: {}{}",
                    process_name,
                    String::from_utf8_lossy(&output.stdout).trim(),
                    String::from_utf8_lossy(&output.stderr).trim()
                )));
            }
            return Ok(());
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = kind;
            Err(BitFunError::tool(
                "Browser restart with CDP is not supported on this platform".to_string(),
            ))
        }
    }

    #[allow(dead_code)]
    async fn wait_for_browser_exit(kind: &BrowserKind, timeout: Duration) -> BitFunResult<()> {
        let started = std::time::Instant::now();
        while Self::is_browser_running(kind) {
            if started.elapsed() >= timeout {
                return Err(BitFunError::tool(format!(
                    "Timed out waiting for {} to exit before restart",
                    kind
                )));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        Ok(())
    }

    /// Check if a browser process is currently running.
    #[allow(dead_code)]
    fn is_browser_running(kind: &BrowserKind) -> bool {
        // Per-platform process names.
        // macOS / Linux match against the executable filename via `pgrep -f`.
        // Windows must use the *.exe image name as it appears in `tasklist`.
        #[cfg(target_os = "macos")]
        let process_names: Vec<&str> = match kind {
            BrowserKind::Chrome => vec!["Google Chrome"],
            BrowserKind::Edge => vec!["Microsoft Edge"],
            BrowserKind::Brave => vec!["Brave Browser"],
            BrowserKind::Arc => vec!["Arc"],
            BrowserKind::Chromium => vec!["Chromium"],
            BrowserKind::Unknown(_) => return false,
        };

        #[cfg(target_os = "linux")]
        let process_names: Vec<&str> = match kind {
            BrowserKind::Chrome => vec!["chrome", "google-chrome"],
            BrowserKind::Edge => vec!["msedge", "microsoft-edge"],
            BrowserKind::Brave => vec!["brave", "brave-browser"],
            BrowserKind::Arc => vec!["arc"],
            BrowserKind::Chromium => vec!["chromium", "chromium-browser"],
            BrowserKind::Unknown(_) => return false,
        };

        #[cfg(target_os = "windows")]
        let process_names: Vec<&str> = match kind {
            BrowserKind::Chrome => vec!["chrome.exe"],
            BrowserKind::Edge => vec!["msedge.exe"],
            BrowserKind::Brave => vec!["brave.exe"],
            BrowserKind::Arc => vec!["arc.exe"],
            BrowserKind::Chromium => vec!["chrome.exe", "chromium.exe"],
            BrowserKind::Unknown(_) => return false,
        };

        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            for name in &process_names {
                let output = silent_command("pgrep").args(["-f", name]).output().ok();
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
            for image in &process_names {
                let filter = format!("IMAGENAME eq {}", image);
                let output = silent_command("tasklist")
                    .args(["/FI", &filter, "/NH", "/FO", "CSV"])
                    .output()
                    .ok();
                if let Some(out) = output {
                    let text = String::from_utf8_lossy(&out.stdout);
                    // tasklist prints "INFO: No tasks ..." when nothing matches;
                    // otherwise the first CSV column contains the image name.
                    if text
                        .to_ascii_lowercase()
                        .contains(&image.to_ascii_lowercase())
                    {
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
