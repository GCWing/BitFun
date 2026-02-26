//! Theme System

use bitfun_core::infrastructure::try_get_path_manager_arc;
use bitfun_core::service::config::types::GlobalConfig;
use log::{debug, error, warn};
use tauri::WebviewUrl;

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
        Self::get_builtin_theme("bitfun-slate").unwrap_or_else(|| Self {
            id: "bitfun-slate".to_string(),
            bg_primary: "#1a1c1e".to_string(),
            bg_secondary: "#1a1c1e".to_string(),
            bg_scene: "#1d2023".to_string(),
            is_light: false,
            text_primary: "#e4e6e8".to_string(),
            text_muted: "#8a8d92".to_string(),
            accent_color: "#6b9bd5".to_string(),
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
                bg_primary: "#f4f4f4".to_string(),
                bg_secondary: "#ffffff".to_string(),
                bg_scene: "#ffffff".to_string(),
                is_light: true,
                text_primary: "#111827".to_string(),
                text_muted: "rgba(0, 0, 0, 0.5)".to_string(),
                accent_color: "#3b82f6".to_string(),
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
            .unwrap_or("bitfun-slate");

        match Self::get_builtin_theme(theme_id) {
            Some(config) => config,
            None => {
                warn!("Unknown theme ID: {}, using default theme", theme_id);
                default
            }
        }
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
        .maximized(true)
        .visible(false)
        .background_color(bg_color)
        .accept_first_mouse(true)
        .initialization_script(&init_script);

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .decorations(true)
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .traffic_light_position(tauri::LogicalPosition::new(12.0, 15.0))
            .hidden_title(true);
    }

    #[cfg(not(target_os = "macos"))]
    {
        builder = builder.decorations(false);
    }

    #[cfg(target_os = "windows")]
    {
        builder = builder.disable_drag_drop_handler();
    }

    match builder.build() {
        Ok(window) => {
            #[cfg(debug_assertions)]
            {
                if std::env::var("BITFUN_OPEN_DEVTOOLS")
                    .map(|v| v == "1")
                    .unwrap_or(false)
                {
                    window.open_devtools();
                }
            }

            #[cfg(not(debug_assertions))]
            let _ = window;
        }
        Err(e) => {
            error!("Failed to create main window: {}", e);
        }
    }
}

#[tauri::command]
pub async fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    if let Some(main_window) = app.get_webview_window("main") {
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
