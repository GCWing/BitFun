//! Tauri commands exposed to the frontend installer UI.

use super::extract::{self, ESTIMATED_INSTALL_SIZE};
use super::types::{DiskSpaceInfo, InstallOptions, InstallProgress, ModelConfig};
use serde::Serialize;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use tauri::{Emitter, Window};

#[cfg(target_os = "windows")]
#[derive(Default)]
struct WindowsInstallState {
    uninstall_registered: bool,
    desktop_shortcut_created: bool,
    start_menu_shortcut_created: bool,
    context_menu_registered: bool,
    added_to_path: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchContext {
    pub mode: String,
    pub uninstall_path: Option<String>,
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
    if let Some(idx) = args.iter().position(|arg| arg == "--uninstall") {
        let uninstall_path = args
            .get(idx + 1)
            .map(|p| p.to_string())
            .or_else(|| guess_uninstall_path_from_exe());
        return LaunchContext {
            mode: "uninstall".to_string(),
            uninstall_path,
        };
    }

    LaunchContext {
        mode: "install".to_string(),
        uninstall_path: None,
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
pub async fn start_installation(
    window: Window,
    options: InstallOptions,
) -> Result<(), String> {
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

        // In production, this would extract from an embedded archive.
        // For development, we look for a payload directory next to the installer.
        let exe_dir = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        let payload_zip = exe_dir.join("payload.zip");
        let payload_dir = exe_dir.join("payload");

        if payload_zip.exists() {
            extract::extract_zip(&payload_zip, &install_path)
                .map_err(|e| format!("Extraction failed: {}", e))?;
        } else if payload_dir.exists() {
            extract::copy_directory(&payload_dir, &install_path)
                .map_err(|e| format!("File copy failed: {}", e))?;
        } else {
            // Development mode: create a placeholder
            log::warn!("No payload found - running in development mode");
            let placeholder = install_path.join("BitFun.exe");
            if !placeholder.exists() {
                std::fs::write(&placeholder, "placeholder")
                    .map_err(|e| format!("Failed to write placeholder: {}", e))?;
            }
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
            registry::register_uninstall_entry(&install_path, "0.1.0", &uninstall_command)
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
                emit_progress(&window, "context_menu", 80, "Adding context menu integration...");
                registry::register_context_menu(&install_path)
                    .map_err(|e| format!("Context menu error: {}", e))?;
                windows_state.context_menu_registered = true;
            }

            // PATH
            if options.add_to_path {
                emit_progress(&window, "path", 85, "Adding to system PATH...");
                registry::add_to_path(&install_path)
                    .map_err(|e| format!("PATH error: {}", e))?;
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
        let running_from_install_dir = current_exe
            .as_ref()
            .map(|exe| exe.starts_with(&install_path))
            .unwrap_or(false);

        if running_from_install_dir {
            if install_path.exists() {
                let cmd = format!(
                    "ping 127.0.0.1 -n 3 > nul && rmdir /s /q \"{}\"",
                    install_path.display()
                );
                std::process::Command::new("cmd")
                    .args(["/C", &cmd])
                    .spawn()
                    .map_err(|e| format!("Failed to schedule uninstall cleanup: {}", e))?;
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
        "bitfun-starry-night",
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

// ── Helpers ────────────────────────────────────────────────────────────────

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
    app_obj.insert("language".to_string(), Value::String(app_language.to_string()));

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
    let model_json = serde_json::json!({
        "id": model_id,
        "name": format!("{} - {}", model.provider, model.model_name),
        "provider": model.format,
        "model_name": model.model_name,
        "base_url": model.base_url,
        "api_key": model.api_key,
        "enabled": true,
        "category": "general_chat",
        "capabilities": ["text_chat", "function_calling"],
        "recommended_for": [],
        "metadata": null,
        "enable_thinking_process": false,
        "support_preserved_thinking": false,
        "skip_ssl_verify": false
    });

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
