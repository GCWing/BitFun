//! Theme System

use std::sync::OnceLock;

use bitfun_core::infrastructure::try_get_path_manager_arc;
use bitfun_core::service::config::types::GlobalConfig;
use dark_light::Mode;
use log::{debug, error, warn};
use tauri::{Manager, WebviewUrl};

const AGENT_COMPANION_WINDOW_LABEL: &str = "agent-companion-pet";
const AGENT_COMPANION_WINDOW_MIN_SIZE: f64 = 96.0;
const AGENT_COMPANION_WINDOW_MAX_WIDTH: f64 = 360.0;
const AGENT_COMPANION_WINDOW_MAX_HEIGHT: f64 = 240.0;
const AGENT_COMPANION_WINDOW_MARGIN: i32 = 64;

static AGENT_COMPANION_WINDOW_OPS: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

fn agent_companion_window_ops() -> &'static tokio::sync::Mutex<()> {
    AGENT_COMPANION_WINDOW_OPS.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[derive(Debug, Clone)]
pub struct ThemeConfig {
    pub id: String,
    pub bg_primary: String,
    pub bg_secondary: String,
    pub bg_scene: String,
    pub is_light: bool,
    pub text_primary: String,
    pub text_muted: String,
    pub accent_color: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self::get_builtin_theme("bitfun-light").unwrap_or_else(|| Self {
            id: "bitfun-light".to_string(),
            bg_primary: "#f3f3f5".to_string(),
            bg_secondary: "#ffffff".to_string(),
            bg_scene: "#ffffff".to_string(),
            is_light: true,
            text_primary: "#1e293b".to_string(),
            text_muted: "#64748b".to_string(),
            accent_color: "#64748b".to_string(),
        })
    }
}

impl ThemeConfig {
    pub fn get_builtin_theme(theme_id: &str) -> Option<Self> {
        match theme_id {
            "bitfun-slate" => Some(Self {
                id: theme_id.to_string(),
                bg_primary: "#1a1c1e".to_string(),
                bg_secondary: "#1a1c1e".to_string(),
                bg_scene: "#1d2023".to_string(),
                is_light: false,
                text_primary: "#e4e6e8".to_string(),
                text_muted: "#8a8d92".to_string(),
                accent_color: "#6b9bd5".to_string(),
            }),
            "bitfun-dark" => Some(Self {
                id: theme_id.to_string(),
                bg_primary: "#121214".to_string(),
                bg_secondary: "#18181a".to_string(),
                bg_scene: "#16161a".to_string(),
                is_light: false,
                text_primary: "#e8e8e8".to_string(),
                text_muted: "rgba(255, 255, 255, 0.4)".to_string(),
                accent_color: "#60a5fa".to_string(),
            }),
            "bitfun-midnight" => Some(Self {
                id: theme_id.to_string(),
                bg_primary: "#2b2d30".to_string(),
                bg_secondary: "#1e1f22".to_string(),
                bg_scene: "#27292c".to_string(),
                is_light: false,
                text_primary: "#bcbec4".to_string(),
                text_muted: "rgba(255, 255, 255, 0.4)".to_string(),
                accent_color: "#6c9eff".to_string(),
            }),
            "bitfun-cyber" => Some(Self {
                id: theme_id.to_string(),
                bg_primary: "#101010".to_string(),
                bg_secondary: "#151515".to_string(),
                bg_scene: "#141414".to_string(),
                is_light: false,
                text_primary: "#e0f2ff".to_string(),
                text_muted: "rgba(255, 255, 255, 0.4)".to_string(),
                accent_color: "#00e6ff".to_string(),
            }),
            "bitfun-china-night" => Some(Self {
                id: theme_id.to_string(),
                bg_primary: "#1a1814".to_string(),
                bg_secondary: "#141210".to_string(),
                bg_scene: "#1e1c17".to_string(),
                is_light: false,
                text_primary: "#e8e6e1".to_string(),
                text_muted: "rgba(255, 255, 255, 0.4)".to_string(),
                accent_color: "#c4a35a".to_string(),
            }),
            "bitfun-light" => Some(Self {
                id: theme_id.to_string(),
                bg_primary: "#f3f3f5".to_string(),
                bg_secondary: "#ffffff".to_string(),
                bg_scene: "#ffffff".to_string(),
                is_light: true,
                text_primary: "#1e293b".to_string(),
                text_muted: "#64748b".to_string(),
                accent_color: "#64748b".to_string(),
            }),
            "bitfun-china-style" => Some(Self {
                id: theme_id.to_string(),
                bg_primary: "#faf8f0".to_string(),
                bg_secondary: "#f5f3e8".to_string(),
                bg_scene: "#fdfcf6".to_string(),
                is_light: true,
                text_primary: "#1a1a1a".to_string(),
                text_muted: "rgba(0, 0, 0, 0.5)".to_string(),
                accent_color: "#2e5e8a".to_string(),
            }),
            _ => None,
        }
    }

    pub fn load_from_config() -> Self {
        let default = Self::default();

        let path_manager = match try_get_path_manager_arc() {
            Ok(pm) => pm,
            Err(e) => {
                debug!("Failed to create PathManager, using default theme: {}", e);
                return default;
            }
        };

        let config_file = path_manager.app_config_file();
        if !config_file.exists() {
            return default;
        }

        let config_content = match std::fs::read_to_string(&config_file) {
            Ok(content) => content,
            Err(e) => {
                debug!("Failed to read config file, using default theme: {}", e);
                return default;
            }
        };

        let global_config: GlobalConfig = match serde_json::from_str(&config_content) {
            Ok(config) => config,
            Err(e) => {
                debug!("Failed to parse config file, using default theme: {}", e);
                return default;
            }
        };

        let theme_id = global_config
            .themes
            .as_ref()
            .map(|t| t.current.as_str())
            .unwrap_or("bitfun-light");

        let resolved_id = Self::resolve_builtin_theme_id(theme_id);

        match Self::get_builtin_theme(resolved_id) {
            Some(config) => config,
            None => {
                warn!("Unknown theme ID: {}, using default theme", theme_id);
                default
            }
        }
    }

    /// Maps config `themes.current` to a built-in id for splash / window chrome.
    /// `system` follows OS light/dark (aligned with web-ui `getSystemPreferredDefaultThemeId`).
    fn resolve_builtin_theme_id(theme_id: &str) -> &str {
        if theme_id == "system" {
            return match dark_light::detect() {
                Mode::Dark => "bitfun-dark",
                Mode::Light | Mode::Default => "bitfun-light",
            };
        }
        theme_id
    }

    pub fn generate_init_script(&self) -> String {
        let theme_type = if self.is_light { "light" } else { "dark" };

        format!(
            r#"
            (function() {{
                function applyTheme() {{
                    var root = document.documentElement;
                    if (!root) return false;
                    
                    root.setAttribute('data-theme', '{id}');
                    root.setAttribute('data-theme-type', '{theme_type}');
                    
                    root.style.setProperty('--color-bg-primary', '{bg_primary}');
                    root.style.setProperty('--color-bg-secondary', '{bg_secondary}');
                    root.style.setProperty('--color-bg-tertiary', '{bg_primary}');
                    root.style.setProperty('--color-bg-workbench', '{bg_primary}');
                    root.style.setProperty('--color-bg-flowchat', '{bg_scene}');
                    root.style.setProperty('--color-bg-scene', '{bg_scene}');
                    root.style.setProperty('--color-text-primary', '{text_primary}');
                    
                    root.style.backgroundColor = '{bg_primary}';
                    
                    if (document.body) {{
                        document.body.style.backgroundColor = '{bg_primary}';
                    }}
                    
                    console.log('[Theme] Pre-injected theme: {id}');
                    return true;
                }}
                
                if (document.documentElement) {{
                    applyTheme();
                }}
                
                if (document.readyState === 'loading') {{
                    document.addEventListener('DOMContentLoaded', applyTheme);
                }} else {{
                    applyTheme();
                }}
            }})();
            "#,
            id = self.id,
            theme_type = theme_type,
            bg_primary = self.bg_primary,
            bg_secondary = self.bg_secondary,
            bg_scene = self.bg_scene,
            text_primary = self.text_primary,
        )
    }

    pub fn to_tauri_color(&self) -> tauri::window::Color {
        let hex = self.bg_primary.trim_start_matches('#');
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(18);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(18);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(20);
        tauri::window::Color(r, g, b, 255)
    }
}

pub fn create_main_window(app_handle: &tauri::AppHandle) {
    let theme = ThemeConfig::load_from_config();
    let bg_color = theme.to_tauri_color();
    let init_script = theme.generate_init_script();

    let main_url = if cfg!(debug_assertions) {
        match "http://localhost:1422".parse() {
            Ok(url) => WebviewUrl::External(url),
            Err(e) => {
                error!("Invalid dev URL, fallback to app URL: {}", e);
                WebviewUrl::App("index.html".into())
            }
        }
    } else {
        WebviewUrl::App("index.html".into())
    };

    #[allow(unused_mut)]
    let mut builder = tauri::WebviewWindowBuilder::new(app_handle, "main", main_url)
        .title("BitFun")
        .inner_size(1200.0, 800.0)
        .resizable(true)
        .fullscreen(false)
        .visible(false)
        .background_color(bg_color)
        .accept_first_mouse(true)
        .initialization_script(&init_script);

    // Keep HTML5 drag-and-drop working inside the webview for desktop UI drag targets.
    builder = builder.disable_drag_drop_handler();

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .decorations(true)
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .traffic_light_position(tauri::LogicalPosition::new(12.0, 15.0))
            .hidden_title(true);
    }

    #[cfg(target_os = "windows")]
    {
        builder = builder.decorations(false);
    }

    match builder.build() {
        Ok(window) => {
            #[cfg(any(debug_assertions, feature = "devtools"))]
            {
                if std::env::var("BITFUN_OPEN_DEVTOOLS")
                    .map(|v| v == "1")
                    .unwrap_or(false)
                {
                    let _ = window.open_devtools();
                }
            }

            #[cfg(not(any(debug_assertions, feature = "devtools")))]
            let _ = window;
        }
        Err(e) => {
            error!("Failed to create main window: {}", e);
        }
    }
}

fn app_url(path: &str) -> WebviewUrl {
    if cfg!(debug_assertions) {
        match format!("http://localhost:1422/{}", path).parse() {
            Ok(url) => WebviewUrl::External(url),
            Err(e) => {
                error!("Invalid dev URL, fallback to app URL: {}", e);
                WebviewUrl::App(path.into())
            }
        }
    } else {
        let app_path = if path.starts_with('?') {
            format!("index.html{}", path)
        } else {
            path.to_string()
        };
        WebviewUrl::App(app_path.into())
    }
}

fn agent_companion_default_position(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Option<tauri::LogicalPosition<f64>> {
    let monitor: Option<tauri::Monitor> = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());

    let monitor = monitor?;

    let scale_factor = monitor.scale_factor();
    let area = monitor.work_area();
    let area_position = area.position.to_logical::<f64>(scale_factor);
    let area_size = area.size.to_logical::<f64>(scale_factor);
    let window_size = window
        .outer_size()
        .ok()
        .map(|size| size.to_logical::<f64>(scale_factor));
    let window_width = window_size
        .as_ref()
        .map(|size| size.width)
        .unwrap_or(AGENT_COMPANION_WINDOW_MIN_SIZE);
    let window_height = window_size
        .as_ref()
        .map(|size| size.height)
        .unwrap_or(AGENT_COMPANION_WINDOW_MIN_SIZE);
    let x = area_position.x + area_size.width
        - window_width
        - f64::from(AGENT_COMPANION_WINDOW_MARGIN);
    let y = area_position.y + area_size.height
        - window_height
        - f64::from(AGENT_COMPANION_WINDOW_MARGIN);

    Some(tauri::LogicalPosition::new(
        x.max(area_position.x),
        y.max(area_position.y),
    ))
}

fn position_agent_companion_window(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let Some(position) = agent_companion_default_position(app, window) else {
        return;
    };

    if let Err(e) = window.set_position(position) {
        warn!("Failed to position Agent companion window: {}", e);
    }
}

fn resize_agent_companion_window(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    width: f64,
    height: f64,
) {
    let monitor: Option<tauri::Monitor> = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let scale_factor = monitor.as_ref().map(|monitor| monitor.scale_factor());
    let old_size = scale_factor.and_then(|scale_factor| {
        window
            .outer_size()
            .ok()
            .map(|size| size.to_logical::<f64>(scale_factor))
    });
    let old_position = scale_factor.and_then(|scale_factor| {
        window
            .outer_position()
            .ok()
            .map(|position| position.to_logical::<f64>(scale_factor))
    });

    if let Err(e) = window.set_size(tauri::LogicalSize::new(width, height)) {
        warn!("Failed to resize Agent companion window: {}", e);
        return;
    }

    // Keep the bottom-right corner fixed when bubbles change height. If we cannot
    // read the previous geometry (e.g. transient platform errors), avoid snapping
    // back to the default corner — that would feel like the pet "jumped".
    if let Some((position, size)) = old_position.zip(old_size) {
        let next_position = tauri::LogicalPosition::new(
            position.x + size.width - width,
            position.y + size.height - height,
        );
        if let Err(e) = window.set_position(next_position) {
            warn!("Failed to position Agent companion window: {}", e);
        }
    }
}

#[tauri::command]
pub async fn show_agent_companion_desktop_pet(app: tauri::AppHandle) -> Result<(), String> {
    let _guard = agent_companion_window_ops().lock().await;

    // Reuse any existing window: never destroy here. A previous implementation destroyed
    // whenever `is_visible` was false, which raced with another `show` that had built the
    // window but not called `show()` yet (or with `hide`), producing duplicate pets or
    // stuck windows.
    if let Some(window) = app.get_webview_window(AGENT_COMPANION_WINDOW_LABEL) {
        let _ = window.unminimize();
        window.show().map_err(|e| {
            error!("Failed to show Agent companion window: {}", e);
            format!("Failed to show Agent companion window: {}", e)
        })?;
        return Ok(());
    }

    let url = app_url("?bitfunWindow=agent-companion");
    let mut builder = tauri::WebviewWindowBuilder::new(&app, AGENT_COMPANION_WINDOW_LABEL, url)
        .title("BitFun Agent Companion")
        .inner_size(
            AGENT_COMPANION_WINDOW_MIN_SIZE,
            AGENT_COMPANION_WINDOW_MIN_SIZE,
        )
        .max_inner_size(
            AGENT_COMPANION_WINDOW_MAX_WIDTH,
            AGENT_COMPANION_WINDOW_MAX_HEIGHT,
        )
        .min_inner_size(1.0, 1.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .visible(false)
        .accept_first_mouse(true)
        .background_color(tauri::window::Color(0, 0, 0, 0));

    builder = builder.disable_drag_drop_handler();

    let window = builder.build().map_err(|e| {
        error!("Failed to create Agent companion window: {}", e);
        format!("Failed to create Agent companion window: {}", e)
    })?;

    position_agent_companion_window(&app, &window);

    window.show().map_err(|e| {
        error!("Failed to show Agent companion window: {}", e);
        format!("Failed to show Agent companion window: {}", e)
    })?;

    Ok(())
}

#[tauri::command]
pub async fn resize_agent_companion_desktop_pet(
    app: tauri::AppHandle,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let _guard = agent_companion_window_ops().lock().await;
    if let Some(window) = app.get_webview_window(AGENT_COMPANION_WINDOW_LABEL) {
        resize_agent_companion_window(&app, &window, width, height);
    }
    Ok(())
}

#[tauri::command]
pub async fn hide_agent_companion_desktop_pet(app: tauri::AppHandle) -> Result<(), String> {
    let _guard = agent_companion_window_ops().lock().await;
    if let Some(window) = app.get_webview_window(AGENT_COMPANION_WINDOW_LABEL) {
        window.destroy().map_err(|e| {
            error!("Failed to destroy Agent companion window: {}", e);
            format!("Failed to destroy Agent companion window: {}", e)
        })?;
    }
    Ok(())
}

#[tauri::command]
pub async fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(main_window) = app.get_webview_window("main") {
        #[cfg(target_os = "windows")]
        {
            // Work around Windows startup flicker: avoid creating the native window
            // in maximized mode, and maximize it right before showing instead.
            main_window.maximize().map_err(|e| {
                error!("Failed to maximize main window: {}", e);
                format!("Failed to maximize main window: {}", e)
            })?;

            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }

        main_window.show().map_err(|e| {
            error!("Failed to show main window: {}", e);
            format!("Failed to show main window: {}", e)
        })?;

        main_window.set_focus().map_err(|e| {
            error!("Failed to focus main window: {}", e);
            format!("Failed to focus main window: {}", e)
        })?;
    } else {
        error!("Main window not found");
        return Err("Main window not found".to_string());
    }

    Ok(())
}
