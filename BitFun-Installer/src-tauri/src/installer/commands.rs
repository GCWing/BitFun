//! Tauri commands exposed to the frontend installer UI.

use super::extract::{self, ESTIMATED_INSTALL_SIZE};
use super::types::{ConnectionTestResult, DiskSpaceInfo, InstallOptions, InstallProgress, ModelConfig};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::Serialize;
use serde_json::{Map, Value};
use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{Emitter, Manager, Window};

#[cfg(target_os = "windows")]
#[derive(Default)]
struct WindowsInstallState {
    uninstall_registered: bool,
    desktop_shortcut_created: bool,
    start_menu_shortcut_created: bool,
    context_menu_registered: bool,
    added_to_path: bool,
}

const MIN_WINDOWS_APP_EXE_BYTES: u64 = 5 * 1024 * 1024;
const PAYLOAD_MANIFEST_FILE: &str = "payload-manifest.json";
const EMBEDDED_PAYLOAD_ZIP: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/embedded_payload.zip"));

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchContext {
    pub mode: String,
    pub uninstall_path: Option<String>,
    pub app_language: Option<String>,
}

/// Get the default installation path.
#[tauri::command]
pub fn get_default_install_path() -> String {
    let base = if cfg!(target_os = "windows") {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("C:\\Program Files"))
            })
    } else if cfg!(target_os = "macos") {
        dirs::home_dir()
            .map(|h| h.join("Applications"))
            .unwrap_or_else(|| PathBuf::from("/Applications"))
    } else {
        dirs::home_dir()
            .map(|h| h.join(".local/share"))
            .unwrap_or_else(|| PathBuf::from("/opt"))
    };

    base.join("BitFun").to_string_lossy().to_string()
}

/// Get available disk space for the given path.
#[tauri::command]
pub fn get_disk_space(path: String) -> Result<DiskSpaceInfo, String> {
    let path = PathBuf::from(&path);

    // Walk up to find an existing ancestor directory
    let check_path = find_existing_ancestor(&path);

    // Use std::fs metadata as a basic check. For actual disk space,
    // platform-specific APIs are needed.
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let wide_path: Vec<u16> = OsStr::new(check_path.to_str().unwrap_or("C:\\"))
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut free_bytes_available: u64 = 0;
        let mut total_bytes: u64 = 0;
        let mut total_free_bytes: u64 = 0;

        unsafe {
            let result = windows_sys_get_disk_free_space(
                wide_path.as_ptr(),
                &mut free_bytes_available,
                &mut total_bytes,
                &mut total_free_bytes,
            );
            if result != 0 {
                return Ok(DiskSpaceInfo {
                    total: total_bytes,
                    available: free_bytes_available,
                    required: ESTIMATED_INSTALL_SIZE,
                    sufficient: free_bytes_available >= ESTIMATED_INSTALL_SIZE,
                });
            }
        }
    }

    // Fallback: assume sufficient space
    Ok(DiskSpaceInfo {
        total: 0,
        available: u64::MAX,
        required: ESTIMATED_INSTALL_SIZE,
        sufficient: true,
    })
}

#[cfg(target_os = "windows")]
unsafe fn windows_sys_get_disk_free_space(
    path: *const u16,
    free_bytes_available: *mut u64,
    total_bytes: *mut u64,
    total_free_bytes: *mut u64,
) -> i32 {
    // Link to kernel32.dll GetDiskFreeSpaceExW
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            lpDirectoryName: *const u16,
            lpFreeBytesAvailableToCaller: *mut u64,
            lpTotalNumberOfBytes: *mut u64,
            lpTotalNumberOfFreeBytes: *mut u64,
        ) -> i32;
    }
    GetDiskFreeSpaceExW(path, free_bytes_available, total_bytes, total_free_bytes)
}

#[tauri::command]
pub fn get_launch_context() -> LaunchContext {
    let args: Vec<String> = std::env::args().collect();
    let app_language = read_saved_app_language();
    if let Some(idx) = args.iter().position(|arg| arg == "--uninstall") {
        let uninstall_path = args
            .get(idx + 1)
            .map(|p| p.to_string())
            .or_else(|| guess_uninstall_path_from_exe());
        return LaunchContext {
            mode: "uninstall".to_string(),
            uninstall_path,
            app_language,
        };
    }

    if is_running_as_uninstall_binary() {
        return LaunchContext {
            mode: "uninstall".to_string(),
            uninstall_path: guess_uninstall_path_from_exe(),
            app_language,
        };
    }

    LaunchContext {
        mode: "install".to_string(),
        uninstall_path: None,
        app_language,
    }
}

/// Validate the installation path.
#[tauri::command]
pub fn validate_install_path(path: String) -> Result<bool, String> {
    let path = PathBuf::from(&path);

    // Check if the path is absolute
    if !path.is_absolute() {
        return Err("Installation path must be absolute".into());
    }

    // Check if we can create the directory
    if path.exists() {
        if !path.is_dir() {
            return Err("Path exists but is not a directory".into());
        }
        // Directory exists - check if it's writable
        let test_file = path.join(".bitfun_install_test");
        match std::fs::write(&test_file, "test") {
            Ok(_) => {
                let _ = std::fs::remove_file(&test_file);
                Ok(true)
            }
            Err(_) => Err("Directory is not writable".into()),
        }
    } else {
        // Try to find the nearest existing ancestor
        let ancestor = find_existing_ancestor(&path);
        let test_file = ancestor.join(".bitfun_install_test");
        match std::fs::write(&test_file, "test") {
            Ok(_) => {
                let _ = std::fs::remove_file(&test_file);
                Ok(true)
            }
            Err(_) => Err("Cannot write to the parent directory".into()),
        }
    }
}

/// Main installation command. Emits progress events to the frontend.
#[tauri::command]
pub async fn start_installation(window: Window, options: InstallOptions) -> Result<(), String> {
    let install_path = PathBuf::from(&options.install_path);
    let install_dir_was_absent = !install_path.exists();
    #[cfg(target_os = "windows")]
    let mut windows_state = WindowsInstallState::default();

    let result: Result<(), String> = (|| {
        // Step 1: Create target directory
        emit_progress(&window, "prepare", 5, "Creating installation directory...");
        std::fs::create_dir_all(&install_path)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        // Step 2: Extract / copy application files
        emit_progress(&window, "extract", 15, "Extracting application files...");

        let mut extracted = false;
        let mut used_debug_placeholder = false;
        let mut checked_locations: Vec<String> = Vec::new();

        if embedded_payload_available() {
            checked_locations.push("embedded payload zip".to_string());
            preflight_validate_payload_zip_bytes(EMBEDDED_PAYLOAD_ZIP, "embedded payload zip")?;
            extract::extract_zip_bytes_with_filter(
                EMBEDDED_PAYLOAD_ZIP,
                &install_path,
                should_install_payload_path,
            )
            .map_err(|e| format!("Embedded payload extraction failed: {}", e))?;
            extracted = true;
            log::info!("Extracted payload from embedded installer archive");
        }

        // Fallback to external payload locations for compatibility and local debug.
        let exe_dir = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        if !extracted {
            for candidate in build_payload_candidates(&window, &exe_dir) {
                if candidate.is_zip {
                    checked_locations.push(format!("zip: {}", candidate.path.display()));
                    if !candidate.path.exists() {
                        continue;
                    }
                    preflight_validate_payload_zip_file(&candidate.path, &candidate.label)?;
                    extract::extract_zip_with_filter(
                        &candidate.path,
                        &install_path,
                        should_install_payload_path,
                    )
                    .map_err(|e| format!("Extraction failed from {}: {}", candidate.label, e))?;
                    extracted = true;
                    log::info!("Extracted payload from {}", candidate.label);
                    break;
                }

                checked_locations.push(format!("dir: {}", candidate.path.display()));
                if !candidate.path.exists() {
                    continue;
                }
                preflight_validate_payload_dir(&candidate.path, &candidate.label)?;
                extract::copy_directory_with_filter(
                    &candidate.path,
                    &install_path,
                    should_install_payload_path,
                )
                .map_err(|e| format!("File copy failed from {}: {}", candidate.label, e))?;
                extracted = true;
                log::info!("Copied payload from {}", candidate.label);
                break;
            }
        }

        if !extracted {
            if cfg!(debug_assertions) {
                // Development mode: create a placeholder to simplify local UI iteration.
                log::warn!("No payload found - running in development mode");
                let placeholder = install_path.join("BitFun.exe");
                if !placeholder.exists() {
                    std::fs::write(&placeholder, "placeholder")
                        .map_err(|e| format!("Failed to write placeholder: {}", e))?;
                }
                used_debug_placeholder = true;
            } else {
                return Err(format!(
                    "Installer payload is missing. Checked: {}",
                    checked_locations.join(" | ")
                ));
            }
        }

        if !used_debug_placeholder {
            verify_installed_payload(&install_path)?;
        }

        emit_progress(&window, "extract", 50, "Files extracted successfully");

        // Step 3: Windows-specific operations
        #[cfg(target_os = "windows")]
        {
            use super::registry;
            use super::shortcut;

            let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let uninstaller_path = install_path.join("uninstall.exe");
            std::fs::copy(&current_exe, &uninstaller_path)
                .map_err(|e| format!("Failed to create uninstaller executable: {}", e))?;
            let uninstall_command = format!(
                "\"{}\" --uninstall \"{}\"",
                uninstaller_path.display(),
                install_path.display()
            );

            emit_progress(&window, "registry", 60, "Registering application...");
            registry::register_uninstall_entry(
                &install_path,
                env!("CARGO_PKG_VERSION"),
                &uninstall_command,
            )
            .map_err(|e| format!("Registry error: {}", e))?;
            windows_state.uninstall_registered = true;

            // Desktop shortcut
            if options.desktop_shortcut {
                emit_progress(&window, "shortcuts", 70, "Creating desktop shortcut...");
                shortcut::create_desktop_shortcut(&install_path)
                    .map_err(|e| format!("Shortcut error: {}", e))?;
                windows_state.desktop_shortcut_created = true;
            }

            // Start Menu
            if options.start_menu {
                emit_progress(&window, "shortcuts", 75, "Creating Start Menu entry...");
                shortcut::create_start_menu_shortcut(&install_path)
                    .map_err(|e| format!("Start Menu error: {}", e))?;
                windows_state.start_menu_shortcut_created = true;
            }

            // Context menu
            if options.context_menu {
                emit_progress(
                    &window,
                    "context_menu",
                    80,
                    "Adding context menu integration...",
                );
                registry::register_context_menu(&install_path)
                    .map_err(|e| format!("Context menu error: {}", e))?;
                windows_state.context_menu_registered = true;
            }

            // PATH
            if options.add_to_path {
                emit_progress(&window, "path", 85, "Adding to system PATH...");
                registry::add_to_path(&install_path).map_err(|e| format!("PATH error: {}", e))?;
                windows_state.added_to_path = true;
            }
        }

        // Step 4: Save first-launch language preference for BitFun app.
        emit_progress(&window, "config", 92, "Applying startup preferences...");
        apply_first_launch_language(&options.app_language)
            .map_err(|e| format!("Failed to apply startup preferences: {}", e))?;
        // Step 5: Done
        emit_progress(&window, "complete", 100, "Installation complete!");
        Ok(())
    })();

    if let Err(err) = result {
        #[cfg(target_os = "windows")]
        rollback_installation(&install_path, install_dir_was_absent, &windows_state);
        #[cfg(not(target_os = "windows"))]
        rollback_installation(&install_path, install_dir_was_absent);
        return Err(err);
    }

    Ok(())
}

/// Uninstall BitFun (for the uninstaller companion).
#[tauri::command]
pub async fn uninstall(install_path: String) -> Result<(), String> {
    let install_path = PathBuf::from(&install_path);

    #[cfg(target_os = "windows")]
    {
        use super::registry;
        use super::shortcut;

        let _ = shortcut::remove_desktop_shortcut();
        let _ = shortcut::remove_start_menu_shortcut();
        let _ = registry::remove_context_menu();
        let _ = registry::remove_from_path(&install_path);
        let _ = registry::remove_uninstall_entry();
    }

    #[cfg(target_os = "windows")]
    {
        let current_exe = std::env::current_exe().ok();
        let running_uninstall_binary = current_exe
            .as_ref()
            .and_then(|exe| exe.file_stem().map(|s| s.to_string_lossy().to_string()))
            .map(|stem| stem.eq_ignore_ascii_case("uninstall"))
            .unwrap_or(false);

        let current_exe_parent = current_exe
            .as_ref()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf()));
        let running_from_install_dir = current_exe_parent
            .as_ref()
            .map(|parent| windows_path_eq_case_insensitive(parent, &install_path))
            .unwrap_or(false);

        append_uninstall_runtime_log(&format!(
            "uninstall called: install_path='{}', current_exe='{}', running_uninstall_binary={}, running_from_install_dir={}",
            install_path.display(),
            current_exe
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unknown>".to_string()),
            running_uninstall_binary,
            running_from_install_dir
        ));

        if running_uninstall_binary || running_from_install_dir {
            if install_path.exists() {
                schedule_windows_self_uninstall_cleanup(&install_path)?;
            } else {
                append_uninstall_runtime_log(&format!(
                    "install path does not exist, skip cleanup schedule: {}",
                    install_path.display()
                ));
            }
            return Ok(());
        }
    }

    if install_path.exists() {
        std::fs::remove_dir_all(&install_path)
            .map_err(|e| format!("Failed to remove files: {}", e))?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn schedule_windows_self_uninstall_cleanup(install_path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let temp_dir = std::env::temp_dir();
    let pid = std::process::id();
    let script_path = temp_dir.join(format!("bitfun-uninstall-{}.cmd", pid));
    let log_path = temp_dir.join(format!("bitfun-uninstall-cleanup-{}.log", pid));

    let script = format!(
        r#"@echo off
setlocal enableextensions
set "TARGET=%~1"
set "LOG=%~2"
if "%TARGET%"=="" exit /b 2
if "%LOG%"=="" set "LOG=%TEMP%\bitfun-uninstall-cleanup.log"
echo [%DATE% %TIME%] cleanup start > "%LOG%"
cd /d "%TEMP%"
taskkill /f /im BitFun.exe >> "%LOG%" 2>&1
set "DONE=0"
for /L %%i in (1,1,30) do (
  rmdir /s /q "%TARGET%" >> "%LOG%" 2>&1
  if not exist "%TARGET%" (
    echo [%DATE% %TIME%] cleanup success on try %%i >> "%LOG%"
    set "DONE=1"
    goto :cleanup_done
  )
  timeout /t 1 /nobreak >nul
)
:cleanup_done
if "%DONE%"=="1" exit /b 0
echo [%DATE% %TIME%] cleanup failed after retries >> "%LOG%"
exit /b 1
"#
    );

    std::fs::write(&script_path, script)
        .map_err(|e| format!("Failed to write cleanup script: {}", e))?;

    append_uninstall_runtime_log(&format!(
        "scheduled cleanup script='{}', target='{}', cleanup_log='{}'",
        script_path.display(),
        install_path.display(),
        log_path.display()
    ));

    let child = std::process::Command::new("cmd")
        .arg("/C")
        .arg("call")
        .arg(&script_path)
        .arg(install_path)
        .arg(&log_path)
        .current_dir(&temp_dir)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Failed to schedule uninstall cleanup: {}", e))?;

    append_uninstall_runtime_log(&format!(
        "cleanup process spawned: pid={}",
        child.id()
    ));

    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_path_eq_case_insensitive(a: &Path, b: &Path) -> bool {
    fn normalize(path: &Path) -> String {
        let mut s = path.to_string_lossy().replace('/', "\\").to_lowercase();
        while s.ends_with('\\') {
            s.pop();
        }
        s
    }
    normalize(a) == normalize(b)
}

#[cfg(target_os = "windows")]
fn append_uninstall_runtime_log(message: &str) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let log_path = std::env::temp_dir().join("bitfun-uninstall-runtime.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        use std::io::Write;
        let _ = writeln!(file, "[{}] {}", ts, message);
    }
}

/// Launch the installed application.
#[tauri::command]
pub fn launch_application(install_path: String) -> Result<(), String> {
    let exe = if cfg!(target_os = "windows") {
        PathBuf::from(&install_path).join("BitFun.exe")
    } else if cfg!(target_os = "macos") {
        PathBuf::from(&install_path).join("BitFun")
    } else {
        PathBuf::from(&install_path).join("bitfun")
    };

    std::process::Command::new(&exe)
        .current_dir(&install_path)
        .spawn()
        .map_err(|e| format!("Failed to launch BitFun: {}", e))?;

    Ok(())
}

/// Close the installer window.
#[tauri::command]
pub fn close_installer(window: Window) {
    let _ = window.close();
}

/// Save theme preference for first launch (called after installation).
#[tauri::command]
pub fn set_theme_preference(theme_preference: String) -> Result<(), String> {
    let allowed = [
        "bitfun-dark",
        "bitfun-light",
        "bitfun-midnight",
        "bitfun-china-style",
        "bitfun-china-night",
        "bitfun-cyber",
        "bitfun-slate",
    ];
    if !allowed.contains(&theme_preference.as_str()) {
        return Err("Unsupported theme preference".to_string());
    }

    let app_config_file = ensure_app_config_path()?;
    let mut root = read_or_create_root_config(&app_config_file)?;

    let root_obj = root
        .as_object_mut()
        .ok_or_else(|| "Invalid root config object".to_string())?;

    let themes_obj = root_obj
        .entry("themes".to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Invalid themes config object".to_string())?;
    themes_obj.insert("current".to_string(), Value::String(theme_preference));

    write_root_config(&app_config_file, &root)
}

/// Save default model configuration for first launch (called after installation).
#[tauri::command]
pub fn set_model_config(model_config: ModelConfig) -> Result<(), String> {
    apply_first_launch_model(&model_config)
}

/// Validate model configuration connectivity from installer.
#[tauri::command]
pub async fn test_model_config_connection(model_config: ModelConfig) -> Result<ConnectionTestResult, String> {
    let started_at = std::time::Instant::now();

    let required_fields = [
        ("baseUrl", model_config.base_url.trim()),
        ("apiKey", model_config.api_key.trim()),
        ("modelName", model_config.model_name.trim()),
    ];
    for (field, value) in required_fields {
        if value.is_empty() {
            return Ok(ConnectionTestResult {
                success: false,
                response_time_ms: started_at.elapsed().as_millis() as u64,
                model_response: None,
                error_details: Some(format!("Missing required field: {}", field)),
            });
        }
    }

    let test_result = run_model_connection_test(&model_config).await;
    let elapsed_ms = started_at.elapsed().as_millis() as u64;

    match test_result {
        Ok(model_response) => Ok(ConnectionTestResult {
            success: true,
            response_time_ms: elapsed_ms,
            model_response,
            error_details: None,
        }),
        Err(error_details) => Ok(ConnectionTestResult {
            success: false,
            response_time_ms: elapsed_ms,
            model_response: None,
            error_details: Some(error_details),
        }),
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn normalize_api_format(model: &ModelConfig) -> String {
    let normalized = model.format.trim().to_ascii_lowercase();
    if normalized == "anthropic" {
        "anthropic".to_string()
    } else {
        "openai".to_string()
    }
}

fn append_endpoint(base_url: &str, endpoint: &str) -> String {
    let base = base_url.trim();
    if base.is_empty() {
        return endpoint.to_string();
    }
    if base.ends_with(endpoint) {
        return base.to_string();
    }
    format!("{}/{}", base.trim_end_matches('/'), endpoint)
}

fn resolve_request_url(base_url: &str, format: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(stripped) = trimmed.strip_suffix('#') {
        return stripped.trim_end_matches('/').to_string();
    }

    match format {
        "anthropic" => append_endpoint(&trimmed, "v1/messages"),
        "openai" => append_endpoint(&trimmed, "chat/completions"),
        _ => trimmed,
    }
}

fn parse_custom_request_body(raw: &Option<String>) -> Result<Option<Map<String, Value>>, String> {
    let Some(raw_value) = raw else {
        return Ok(None);
    };

    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let parsed: Value =
        serde_json::from_str(trimmed).map_err(|e| format!("customRequestBody is invalid JSON: {}", e))?;
    let obj = parsed.as_object().ok_or_else(|| {
        "customRequestBody must be a JSON object (for example: {\"temperature\": 0.7})".to_string()
    })?;
    Ok(Some(obj.clone()))
}

fn merge_json_object(target: &mut Map<String, Value>, source: &Map<String, Value>) {
    for (key, value) in source {
        target.insert(key.clone(), value.clone());
    }
}

fn build_request_headers(model: &ModelConfig, format: &str) -> Result<HeaderMap, String> {
    let mode = model
        .custom_headers_mode
        .as_deref()
        .unwrap_or("merge")
        .trim()
        .to_ascii_lowercase();
    if mode != "merge" && mode != "replace" {
        return Err("customHeadersMode must be 'merge' or 'replace'".to_string());
    }

    let mut headers = HeaderMap::new();
    if mode != "replace" {
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if format == "anthropic" {
            let api_key = HeaderValue::from_str(model.api_key.trim())
                .map_err(|_| "apiKey contains unsupported header characters".to_string())?;
            headers.insert(HeaderName::from_static("x-api-key"), api_key);
            headers.insert(
                HeaderName::from_static("anthropic-version"),
                HeaderValue::from_static("2023-06-01"),
            );
        } else {
            let bearer = format!("Bearer {}", model.api_key.trim());
            let auth = HeaderValue::from_str(&bearer)
                .map_err(|_| "apiKey contains unsupported header characters".to_string())?;
            headers.insert(AUTHORIZATION, auth);
        }
    }

    if let Some(custom_headers) = &model.custom_headers {
        for (key, value) in custom_headers {
            let key_trimmed = key.trim();
            if key_trimmed.is_empty() {
                continue;
            }
            let header_name = HeaderName::from_bytes(key_trimmed.as_bytes())
                .map_err(|_| format!("Invalid custom header name: {}", key_trimmed))?;
            let header_value = HeaderValue::from_str(value.trim())
                .map_err(|_| format!("Invalid custom header value for '{}'", key_trimmed))?;
            headers.insert(header_name, header_value);
        }
    }

    Ok(headers)
}

fn truncate_error_text(raw: &str, limit: usize) -> String {
    let compact = raw.replace('\n', " ").replace('\r', " ").trim().to_string();
    if compact.chars().count() <= limit {
        return compact;
    }
    compact.chars().take(limit).collect::<String>() + "..."
}

async fn run_model_connection_test(model: &ModelConfig) -> Result<Option<String>, String> {
    let format = normalize_api_format(model);
    let endpoint = resolve_request_url(&model.base_url, &format);
    let headers = build_request_headers(model, &format)?;
    let custom_request_body = parse_custom_request_body(&model.custom_request_body)?;

    let mut payload = Map::new();
    payload.insert("model".to_string(), Value::String(model.model_name.trim().to_string()));
    if format == "anthropic" {
        payload.insert("max_tokens".to_string(), Value::Number(16_u64.into()));
        payload.insert(
            "messages".to_string(),
            serde_json::json!([{ "role": "user", "content": "hello" }]),
        );
    } else {
        payload.insert("max_tokens".to_string(), Value::Number(16_u64.into()));
        payload.insert("temperature".to_string(), serde_json::json!(0.1));
        payload.insert(
            "messages".to_string(),
            serde_json::json!([{ "role": "user", "content": "hello" }]),
        );
    }
    if let Some(extra) = custom_request_body.as_ref() {
        merge_json_object(&mut payload, extra);
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .danger_accept_invalid_certs(model.skip_ssl_verify.unwrap_or(false))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .post(endpoint)
        .headers(headers)
        .json(&Value::Object(payload))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    let response_body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;
    if !status.is_success() {
        return Err(format!(
            "HTTP {}: {}",
            status.as_u16(),
            truncate_error_text(&response_body, 260)
        ));
    }

    let parsed_json = serde_json::from_str::<Value>(&response_body).unwrap_or(Value::Null);
    let model_response = if format == "anthropic" {
        parsed_json
            .get("content")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    } else {
        parsed_json
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    Ok(model_response)
}

fn emit_progress(window: &Window, step: &str, percent: u32, message: &str) {
    let progress = InstallProgress {
        step: step.to_string(),
        percent,
        message: message.to_string(),
    };
    let _ = window.emit("install-progress", &progress);
    log::info!("[{}%] {}: {}", percent, step, message);
}

fn guess_uninstall_path_from_exe() -> Option<String> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .map(|p| p.to_string_lossy().to_string())
}

fn is_running_as_uninstall_binary() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.file_stem().map(|s| s.to_string_lossy().to_string()))
        .map(|stem| stem.eq_ignore_ascii_case("uninstall"))
        .unwrap_or(false)
}

fn embedded_payload_available() -> bool {
    option_env!("EMBEDDED_PAYLOAD_AVAILABLE")
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[derive(Debug)]
struct PayloadCandidate {
    label: String,
    path: PathBuf,
    is_zip: bool,
}

fn build_payload_candidates(window: &Window, exe_dir: &Path) -> Vec<PayloadCandidate> {
    let mut candidates = Vec::new();

    if let Ok(resource_dir) = window.app_handle().path().resource_dir() {
        candidates.push(PayloadCandidate {
            label: "resource_dir/payload.zip".to_string(),
            path: resource_dir.join("payload.zip"),
            is_zip: true,
        });
        candidates.push(PayloadCandidate {
            label: "resource_dir/payload".to_string(),
            path: resource_dir.join("payload"),
            is_zip: false,
        });
        // Some bundle layouts keep runtime resources under a nested resources directory.
        candidates.push(PayloadCandidate {
            label: "resource_dir/resources/payload.zip".to_string(),
            path: resource_dir.join("resources").join("payload.zip"),
            is_zip: true,
        });
        candidates.push(PayloadCandidate {
            label: "resource_dir/resources/payload".to_string(),
            path: resource_dir.join("resources").join("payload"),
            is_zip: false,
        });
    }

    candidates.push(PayloadCandidate {
        label: "exe_dir/payload.zip".to_string(),
        path: exe_dir.join("payload.zip"),
        is_zip: true,
    });
    candidates.push(PayloadCandidate {
        label: "exe_dir/payload".to_string(),
        path: exe_dir.join("payload"),
        is_zip: false,
    });
    candidates.push(PayloadCandidate {
        label: "exe_dir/resources/payload.zip".to_string(),
        path: exe_dir.join("resources").join("payload.zip"),
        is_zip: true,
    });
    candidates.push(PayloadCandidate {
        label: "exe_dir/resources/payload".to_string(),
        path: exe_dir.join("resources").join("payload"),
        is_zip: false,
    });

    candidates
}

fn find_existing_ancestor(path: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    while !current.exists() {
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            break;
        }
    }
    current
}

fn ensure_app_config_path() -> Result<PathBuf, String> {
    let config_root = dirs::config_dir()
        .ok_or_else(|| "Failed to get user config directory".to_string())?
        .join("bitfun")
        .join("config");
    std::fs::create_dir_all(&config_root)
        .map_err(|e| format!("Failed to create BitFun config directory: {}", e))?;
    Ok(config_root.join("app.json"))
}

fn read_saved_app_language() -> Option<String> {
    let app_config_file = ensure_app_config_path().ok()?;
    if !app_config_file.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&app_config_file).ok()?;
    let root: Value = serde_json::from_str(&content).ok()?;
    let lang = root.get("app")?.get("language")?.as_str()?;

    match lang {
        "zh-CN" => Some("zh-CN".to_string()),
        "en-US" => Some("en-US".to_string()),
        "zh" => Some("zh-CN".to_string()),
        "en" => Some("en-US".to_string()),
        _ => None,
    }
}

fn read_or_create_root_config(app_config_file: &Path) -> Result<Value, String> {
    let mut root = if app_config_file.exists() {
        let content = std::fs::read_to_string(app_config_file)
            .map_err(|e| format!("Failed to read app config: {}", e))?;
        serde_json::from_str(&content).unwrap_or_else(|_| Value::Object(Map::new()))
    } else {
        Value::Object(Map::new())
    };

    if !root.is_object() {
        root = Value::Object(Map::new());
    }
    Ok(root)
}

fn write_root_config(app_config_file: &Path, root: &Value) -> Result<(), String> {
    let formatted = serde_json::to_string_pretty(root)
        .map_err(|e| format!("Failed to serialize app config: {}", e))?;
    std::fs::write(app_config_file, formatted)
        .map_err(|e| format!("Failed to write app config: {}", e))
}

fn apply_first_launch_language(app_language: &str) -> Result<(), String> {
    let allowed = ["zh-CN", "en-US"];
    if !allowed.contains(&app_language) {
        return Err("Unsupported app language".to_string());
    }

    let app_config_file = ensure_app_config_path()?;
    let mut root = read_or_create_root_config(&app_config_file)?;

    let root_obj = root
        .as_object_mut()
        .ok_or_else(|| "Invalid root config object".to_string())?;
    let app_obj = root_obj
        .entry("app".to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Invalid app config object".to_string())?;
    app_obj.insert(
        "language".to_string(),
        Value::String(app_language.to_string()),
    );

    write_root_config(&app_config_file, &root)
}

fn apply_first_launch_model(model: &ModelConfig) -> Result<(), String> {
    if model.provider.trim().is_empty()
        || model.api_key.trim().is_empty()
        || model.base_url.trim().is_empty()
        || model.model_name.trim().is_empty()
    {
        return Ok(());
    }

    let app_config_file = ensure_app_config_path()?;
    let mut root = read_or_create_root_config(&app_config_file)?;
    let root_obj = root
        .as_object_mut()
        .ok_or_else(|| "Invalid root config object".to_string())?;

    let ai_obj = root_obj
        .entry("ai".to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Invalid ai config object".to_string())?;

    let model_id = format!("installer_{}_{}", model.provider, chrono::Utc::now().timestamp());
    let display_name = model
        .config_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .unwrap_or_else(|| format!("{} - {}", model.provider, model.model_name));

    let custom_request_body = parse_custom_request_body(&model.custom_request_body)?;
    let api_format = normalize_api_format(model);
    let request_url = resolve_request_url(model.base_url.trim(), &api_format);
    let mut model_map = Map::new();
    model_map.insert("id".to_string(), Value::String(model_id.clone()));
    model_map.insert("name".to_string(), Value::String(display_name));
    model_map.insert(
        "provider".to_string(),
        Value::String(api_format),
    );
    model_map.insert(
        "model_name".to_string(),
        Value::String(model.model_name.trim().to_string()),
    );
    model_map.insert(
        "base_url".to_string(),
        Value::String(model.base_url.trim().to_string()),
    );
    model_map.insert("request_url".to_string(), Value::String(request_url));
    model_map.insert(
        "api_key".to_string(),
        Value::String(model.api_key.trim().to_string()),
    );
    model_map.insert("enabled".to_string(), Value::Bool(true));
    model_map.insert(
        "category".to_string(),
        Value::String("general_chat".to_string()),
    );
    model_map.insert(
        "capabilities".to_string(),
        Value::Array(vec![
            Value::String("text_chat".to_string()),
            Value::String("function_calling".to_string()),
        ]),
    );
    model_map.insert("recommended_for".to_string(), Value::Array(Vec::new()));
    model_map.insert("metadata".to_string(), Value::Null);
    model_map.insert("enable_thinking_process".to_string(), Value::Bool(false));
    model_map.insert("support_preserved_thinking".to_string(), Value::Bool(false));

    if let Some(skip_ssl_verify) = model.skip_ssl_verify {
        model_map.insert("skip_ssl_verify".to_string(), Value::Bool(skip_ssl_verify));
    }
    if let Some(headers) = &model.custom_headers {
        let mut header_map = Map::new();
        for (key, value) in headers {
            let key_trimmed = key.trim();
            if key_trimmed.is_empty() {
                continue;
            }
            header_map.insert(
                key_trimmed.to_string(),
                Value::String(value.trim().to_string()),
            );
        }
        if !header_map.is_empty() {
            model_map.insert("custom_headers".to_string(), Value::Object(header_map));
            let mode = model
                .custom_headers_mode
                .as_deref()
                .unwrap_or("merge")
                .trim()
                .to_ascii_lowercase();
            if mode == "merge" || mode == "replace" {
                model_map.insert("custom_headers_mode".to_string(), Value::String(mode));
            }
        }
    }
    if let Some(extra_body) = custom_request_body {
        model_map.insert("custom_request_body".to_string(), Value::Object(extra_body));
    }

    let model_json = Value::Object(model_map);

    let models_entry = ai_obj
        .entry("models".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !models_entry.is_array() {
        *models_entry = Value::Array(Vec::new());
    }
    let models_arr = models_entry
        .as_array_mut()
        .ok_or_else(|| "Invalid ai.models type".to_string())?;
    models_arr.push(model_json);

    let default_models_entry = ai_obj
        .entry("default_models".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !default_models_entry.is_object() {
        *default_models_entry = Value::Object(Map::new());
    }
    let default_models_obj = default_models_entry
        .as_object_mut()
        .ok_or_else(|| "Invalid ai.default_models type".to_string())?;
    default_models_obj.insert("primary".to_string(), Value::String(model_id.clone()));
    default_models_obj.insert("fast".to_string(), Value::String(model_id));

    write_root_config(&app_config_file, &root)
}

fn preflight_validate_payload_zip_bytes(
    zip_bytes: &[u8],
    source_label: &str,
) -> Result<(), String> {
    let reader = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| format!("Invalid zip from {source_label}: {e}"))?;
    preflight_validate_payload_zip_archive(&mut archive, source_label)
}

fn preflight_validate_payload_zip_file(path: &Path, source_label: &str) -> Result<(), String> {
    let file = File::open(path)
        .map_err(|e| format!("Failed to open payload zip ({source_label}): {e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid payload zip ({source_label}): {e}"))?;
    preflight_validate_payload_zip_archive(&mut archive, source_label)
}

fn preflight_validate_payload_zip_archive<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    source_label: &str,
) -> Result<(), String> {
    let mut exe_size: Option<u64> = None;
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read payload entry ({source_label}): {e}"))?;
        if file.name().ends_with('/') {
            continue;
        }
        let file_name = zip_entry_file_name(file.name());
        if file_name.eq_ignore_ascii_case("BitFun.exe") {
            exe_size = Some(file.size());
            break;
        }
    }

    let size = exe_size
        .ok_or_else(|| format!("Payload from {source_label} does not contain BitFun.exe"))?;
    validate_payload_exe_size(size, source_label)
}

fn preflight_validate_payload_dir(path: &Path, source_label: &str) -> Result<(), String> {
    let app_exe = path.join("BitFun.exe");
    let meta = std::fs::metadata(&app_exe).map_err(|_| {
        format!(
            "Payload directory from {source_label} does not contain {}",
            app_exe.display()
        )
    })?;
    validate_payload_exe_size(meta.len(), source_label)
}

fn validate_payload_exe_size(size: u64, source_label: &str) -> Result<(), String> {
    if size < MIN_WINDOWS_APP_EXE_BYTES {
        return Err(format!(
            "Payload BitFun.exe from {source_label} is too small ({size} bytes)"
        ));
    }
    Ok(())
}

fn zip_entry_file_name(entry_name: &str) -> &str {
    entry_name
        .rsplit(&['/', '\\'][..])
        .next()
        .unwrap_or(entry_name)
}

fn is_payload_manifest_path(relative_path: &Path) -> bool {
    relative_path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|n| n.eq_ignore_ascii_case(PAYLOAD_MANIFEST_FILE))
        .unwrap_or(false)
}

fn should_install_payload_path(relative_path: &Path) -> bool {
    !is_payload_manifest_path(relative_path)
}

fn verify_installed_payload(install_path: &Path) -> Result<(), String> {
    let app_exe = install_path.join("BitFun.exe");
    let app_meta = std::fs::metadata(&app_exe)
        .map_err(|_| "Installed BitFun.exe is missing after extraction".to_string())?;
    if app_meta.len() < MIN_WINDOWS_APP_EXE_BYTES {
        return Err(format!(
            "Installed BitFun.exe is too small ({} bytes). Payload is likely invalid.",
            app_meta.len()
        ));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn rollback_installation(
    install_path: &Path,
    install_dir_was_absent: bool,
    windows_state: &WindowsInstallState,
) {
    use super::registry;
    use super::shortcut;

    log::warn!("Installation failed, starting rollback");

    if windows_state.added_to_path {
        let _ = registry::remove_from_path(install_path);
    }
    if windows_state.context_menu_registered {
        let _ = registry::remove_context_menu();
    }
    if windows_state.start_menu_shortcut_created {
        let _ = shortcut::remove_start_menu_shortcut();
    }
    if windows_state.desktop_shortcut_created {
        let _ = shortcut::remove_desktop_shortcut();
    }
    if windows_state.uninstall_registered {
        let _ = registry::remove_uninstall_entry();
    }

    if install_dir_was_absent && install_path.exists() {
        let _ = std::fs::remove_dir_all(install_path);
    }
}

#[cfg(not(target_os = "windows"))]
fn rollback_installation(install_path: &Path, install_dir_was_absent: bool) {
    log::warn!("Installation failed, starting rollback");
    if install_dir_was_absent && install_path.exists() {
        let _ = std::fs::remove_dir_all(install_path);
    }
}
