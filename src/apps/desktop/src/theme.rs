//! Theme System

use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};

use bitfun_core::infrastructure::try_get_path_manager_arc;
use bitfun_core::service::config::types::GlobalConfig;
use dark_light::Mode;
use log::{debug, error, warn};
use tauri::webview::PageLoadEvent;
use tauri::{Manager, WebviewUrl};

use crate::startup_trace::DesktopStartupTrace;

const AGENT_COMPANION_WINDOW_LABEL: &str = "agent-companion-pet";
const AGENT_COMPANION_WINDOW_MIN_SIZE: f64 = 96.0;
const AGENT_COMPANION_WINDOW_MAX_WIDTH: f64 = 360.0;
const AGENT_COMPANION_WINDOW_MAX_HEIGHT: f64 = 240.0;
const AGENT_COMPANION_WINDOW_MARGIN: i32 = 64;
const AGENT_COMPANION_WINDOW_EDGE_MARGIN: f64 = 8.0;
#[cfg(target_os = "windows")]
const WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_MAX: Duration = Duration::from_millis(150);
#[cfg(target_os = "windows")]
const WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_MIN: Duration = Duration::from_millis(16);
#[cfg(target_os = "windows")]
const WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_POLL: Duration = Duration::from_millis(8);

static AGENT_COMPANION_WINDOW_OPS: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
static AGENT_COMPANION_WINDOW_LAST_POSITION: OnceLock<RwLock<Option<tauri::LogicalPosition<f64>>>> =
    OnceLock::new();

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowsMaximizeShowWaitAction {
    Ready,
    Sleep(Duration),
    TimedOut,
}

#[cfg(target_os = "windows")]
fn windows_maximize_show_wait_action(
    is_maximized: Option<bool>,
    elapsed: Duration,
) -> WindowsMaximizeShowWaitAction {
    if is_maximized == Some(true) && elapsed >= WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_MIN {
        return WindowsMaximizeShowWaitAction::Ready;
    }

    if elapsed >= WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_MAX {
        return WindowsMaximizeShowWaitAction::TimedOut;
    }

    let target_wait = if is_maximized == Some(true) {
        WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_MIN
    } else {
        WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_MAX
    };
    WindowsMaximizeShowWaitAction::Sleep(
        target_wait
            .saturating_sub(elapsed)
            .min(WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_POLL),
    )
}

#[cfg(target_os = "windows")]
fn read_windows_maximize_state(
    window: &tauri::WebviewWindow,
    logged_error: &mut bool,
) -> Option<bool> {
    match window.is_maximized() {
        Ok(is_maximized) => Some(is_maximized),
        Err(error) => {
            if !*logged_error {
                warn!(
                    "Failed to read main window maximize state during startup show wait: {}",
                    error
                );
                *logged_error = true;
            }
            None
        }
    }
}

#[cfg(target_os = "windows")]
fn wait_for_windows_maximize_before_show(window: &tauri::WebviewWindow) -> &'static str {
    let started_at = Instant::now();
    let mut logged_error = false;

    loop {
        match windows_maximize_show_wait_action(
            read_windows_maximize_state(window, &mut logged_error),
            started_at.elapsed(),
        ) {
            WindowsMaximizeShowWaitAction::Ready => return "ready",
            WindowsMaximizeShowWaitAction::TimedOut => return "timeout",
            WindowsMaximizeShowWaitAction::Sleep(duration) => std::thread::sleep(duration),
        }
    }
}

#[cfg(target_os = "windows")]
async fn wait_for_windows_maximize_before_show_async(
    window: &tauri::WebviewWindow,
) -> &'static str {
    let started_at = Instant::now();
    let mut logged_error = false;

    loop {
        match windows_maximize_show_wait_action(
            read_windows_maximize_state(window, &mut logged_error),
            started_at.elapsed(),
        ) {
            WindowsMaximizeShowWaitAction::Ready => return "ready",
            WindowsMaximizeShowWaitAction::TimedOut => return "timeout",
            WindowsMaximizeShowWaitAction::Sleep(duration) => tokio::time::sleep(duration).await,
        }
    }
}

fn agent_companion_window_ops() -> &'static tokio::sync::Mutex<()> {
    AGENT_COMPANION_WINDOW_OPS.get_or_init(|| tokio::sync::Mutex::new(()))
}

fn agent_companion_window_last_position() -> &'static RwLock<Option<tauri::LogicalPosition<f64>>> {
    AGENT_COMPANION_WINDOW_LAST_POSITION.get_or_init(|| RwLock::new(None))
}

fn remember_agent_companion_window_position(position: tauri::LogicalPosition<f64>) {
    match agent_companion_window_last_position().write() {
        Ok(mut last_position) => {
            *last_position = Some(position);
        }
        Err(error) => {
            warn!(
                "Failed to remember Agent companion window position: {}",
                error
            );
        }
    }
}

fn remembered_agent_companion_window_position() -> Option<tauri::LogicalPosition<f64>> {
    agent_companion_window_last_position()
        .read()
        .ok()
        .and_then(|position| *position)
}

fn work_area_for_agent_companion_window(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Option<(tauri::LogicalPosition<f64>, tauri::LogicalSize<f64>)> {
    let monitor: Option<tauri::Monitor> = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let monitor = monitor?;
    let scale_factor = monitor.scale_factor();
    let area = monitor.work_area();
    Some((
        area.position.to_logical::<f64>(scale_factor),
        area.size.to_logical::<f64>(scale_factor),
    ))
}

fn clamp_agent_companion_window_position(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::LogicalPosition<f64>,
    size: tauri::LogicalSize<f64>,
) -> tauri::LogicalPosition<f64> {
    let Some((area_position, area_size)) = work_area_for_agent_companion_window(app, window) else {
        return position;
    };

    let min_x = area_position.x + AGENT_COMPANION_WINDOW_EDGE_MARGIN;
    let min_y = area_position.y + AGENT_COMPANION_WINDOW_EDGE_MARGIN;
    let max_x = area_position.x + area_size.width - size.width - AGENT_COMPANION_WINDOW_EDGE_MARGIN;
    let max_y =
        area_position.y + area_size.height - size.height - AGENT_COMPANION_WINDOW_EDGE_MARGIN;
    tauri::LogicalPosition::new(
        if max_x >= min_x {
            position.x.clamp(min_x, max_x)
        } else {
            area_position.x
        },
        if max_y >= min_y {
            position.y.clamp(min_y, max_y)
        } else {
            area_position.y
        },
    )
}

#[derive(Debug, Clone)]
pub struct ThemeConfig {
    pub id: String,
    pub selection_id: Option<String>,
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
        let mut theme = Self::get_builtin_theme("bitfun-light").unwrap_or_else(|| Self {
            id: "bitfun-light".to_string(),
            selection_id: None,
            bg_primary: "#f3f3f5".to_string(),
            bg_secondary: "#ffffff".to_string(),
            bg_scene: "#ffffff".to_string(),
            is_light: true,
            text_primary: "#1e293b".to_string(),
            text_muted: "#64748b".to_string(),
            accent_color: "#64748b".to_string(),
        });
        theme.selection_id = None;
        theme
    }
}

impl ThemeConfig {
    pub fn get_builtin_theme(theme_id: &str) -> Option<Self> {
        match theme_id {
            "bitfun-slate" => Some(Self {
                id: theme_id.to_string(),
                selection_id: Some(theme_id.to_string()),
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
                selection_id: Some(theme_id.to_string()),
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
                selection_id: Some(theme_id.to_string()),
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
                selection_id: Some(theme_id.to_string()),
                bg_primary: "#101010".to_string(),
                bg_secondary: "#151515".to_string(),
                bg_scene: "#141414".to_string(),
                is_light: false,
                text_primary: "#e0f2ff".to_string(),
                text_muted: "rgba(255, 255, 255, 0.4)".to_string(),
                accent_color: "#00e6ff".to_string(),
            }),
            "bitfun-tokyo-night" => Some(Self {
                id: theme_id.to_string(),
                selection_id: Some(theme_id.to_string()),
                bg_primary: "#1a1b26".to_string(),
                bg_secondary: "#16161e".to_string(),
                bg_scene: "#1a1b26".to_string(),
                is_light: false,
                text_primary: "#c0caf5".to_string(),
                text_muted: "rgba(255, 255, 255, 0.4)".to_string(),
                accent_color: "#7aa2f7".to_string(),
            }),
            "bitfun-china-night" => Some(Self {
                id: theme_id.to_string(),
                selection_id: Some(theme_id.to_string()),
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
                selection_id: Some(theme_id.to_string()),
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
                selection_id: Some(theme_id.to_string()),
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
            Some(mut config) => {
                config.selection_id = Some(theme_id.to_string());
                config
            }
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

    fn load_startup_locale_from_config() -> String {
        let path_manager = match try_get_path_manager_arc() {
            Ok(pm) => pm,
            Err(_) => return "zh-CN".to_string(),
        };
        let config_file = path_manager.app_config_file();
        let Ok(config_content) = std::fs::read_to_string(config_file) else {
            return "zh-CN".to_string();
        };
        let Ok(config_value) = serde_json::from_str::<serde_json::Value>(&config_content) else {
            return "zh-CN".to_string();
        };
        config_value
            .pointer("/app/language")
            .and_then(|value| value.as_str())
            .or_else(|| {
                config_value
                    .pointer("/i18n/currentLanguage")
                    .and_then(|value| value.as_str())
            })
            .unwrap_or("zh-CN")
            .to_string()
    }

    fn startup_messages_json(locale: &str) -> String {
        let messages = match locale {
            "en-US" | "en" => serde_json::json!({
                "loadingApp": "Starting BitFun...",
                "minimize": "Minimize",
                "maximize": "Maximize",
                "close": "Close",
                "petLoading": "Loading companion..."
            }),
            "zh-TW" | "zh-Hant-TW" => serde_json::json!({
                "loadingApp": "正在啟動 BitFun...",
                "minimize": "最小化",
                "maximize": "最大化",
                "close": "關閉",
                "petLoading": "正在載入助手..."
            }),
            _ => serde_json::json!({
                "loadingApp": "正在启动 BitFun...",
                "minimize": "最小化",
                "maximize": "最大化",
                "close": "关闭",
                "petLoading": "正在加载助手..."
            }),
        };
        messages.to_string()
    }

    pub fn generate_init_script(&self, startup_trace_id: &str) -> String {
        let theme_type = if self.is_light { "light" } else { "dark" };
        let startup_locale = Self::load_startup_locale_from_config();
        let startup_locale_json =
            serde_json::to_string(&startup_locale).unwrap_or_else(|_| "\"zh-CN\"".to_string());
        let startup_messages_json = Self::startup_messages_json(&startup_locale);
        let show_startup_window_controls = !cfg!(target_os = "macos");
        let startup_trace_id_json = serde_json::to_string(startup_trace_id)
            .unwrap_or_else(|_| "\"desktop-unknown\"".to_string());
        let bootstrap_log_level_json = serde_json::to_string(crate::logging::level_to_str(
            crate::logging::current_runtime_log_level(),
        ))
        .unwrap_or_else(|_| "\"warn\"".to_string());
        let perf_trace_enabled = cfg!(debug_assertions)
            || ((cfg!(feature = "devtools") || std::env::var_os("BITFUN_PERF_TRACE").is_some())
                && std::env::var_os("BITFUN_WEBDRIVER_PORT").is_some());
        let bootstrap_theme_id_json =
            serde_json::to_string(&self.id).unwrap_or_else(|_| "\"bitfun-light\"".to_string());
        let bootstrap_theme_selection_json = self
            .selection_id
            .as_ref()
            .and_then(|selection| serde_json::to_string(selection).ok())
            .unwrap_or_else(|| "null".to_string());

        format!(
            r#"
            (function() {{
                window.__BITFUN_STARTUP_TRACE_ID__ = {startup_trace_id_json};
                window.__BITFUN_PERF_TRACE_ENABLED__ = {perf_trace_enabled};
                window.__BITFUN_BOOTSTRAP_LOG_LEVEL__ = {bootstrap_log_level_json};
                window.__BITFUN_BOOTSTRAP_LOCALE__ = {startup_locale_json};
                window.__BITFUN_BOOTSTRAP_MESSAGES__ = {startup_messages_json};
                window.__BITFUN_SHOW_STARTUP_WINDOW_CONTROLS__ = {show_startup_window_controls};
                window.__BITFUN_BOOTSTRAP_THEME_ID__ = {bootstrap_theme_id_json};
                window.__BITFUN_BOOTSTRAP_THEME_SELECTION__ = {bootstrap_theme_selection_json};
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
                    root.style.setProperty('--bitfun-startup-bg', '{bg_primary}');
                    
                    root.style.backgroundColor = '{bg_primary}';
                    
                    if (document.body) {{
                        document.body.style.backgroundColor = '{bg_primary}';
                    }}
                    
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
            startup_trace_id_json = startup_trace_id_json,
            perf_trace_enabled = perf_trace_enabled,
            bootstrap_log_level_json = bootstrap_log_level_json,
            startup_locale_json = startup_locale_json,
            startup_messages_json = startup_messages_json,
            show_startup_window_controls = show_startup_window_controls,
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

pub fn create_main_window(
    app_handle: &tauri::AppHandle,
    startup_trace_id: &str,
    startup_trace: &DesktopStartupTrace,
) {
    let total_started_at = Instant::now();
    let theme = ThemeConfig::load_from_config();
    let bg_color = theme.to_tauri_color();
    let init_script = theme.generate_init_script(startup_trace_id);
    startup_trace.record_step(
        "native_step_end",
        "native_window",
        "prepare_theme",
        total_started_at.elapsed().as_millis(),
    );
    debug!(
        "Main window creation step completed: step=prepare_theme duration_ms={}",
        total_started_at.elapsed().as_millis()
    );

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
    let main_url_kind = match &main_url {
        WebviewUrl::External(_) => "external",
        WebviewUrl::App(_) => "app",
        _ => "other",
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
        .initialization_script(&init_script)
        .on_page_load({
            let startup_trace_id = startup_trace_id.to_string();
            let total_started_at = total_started_at;
            move |_window, payload| {
                let event = match payload.event() {
                    PageLoadEvent::Started => "started",
                    PageLoadEvent::Finished => "finished",
                };
                debug!(
                    "Main window page load event: trace_id={}, event={}, url={}, since_create_start_ms={}",
                    startup_trace_id,
                    event,
                    payload.url(),
                    total_started_at.elapsed().as_millis()
                );
            }
        });

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

    let build_started_at = Instant::now();
    match builder.build() {
        Ok(window) => {
            startup_trace.record_elapsed_step("native_window", "webview_build", build_started_at);
            debug!(
                "Main window creation step completed: step=build url_kind={} duration_ms={} total_duration_ms={}",
                main_url_kind,
                build_started_at.elapsed().as_millis(),
                total_started_at.elapsed().as_millis()
            );
            #[cfg(any(debug_assertions, feature = "devtools"))]
            {
                if std::env::var("BITFUN_OPEN_DEVTOOLS")
                    .map(|v| v == "1")
                    .unwrap_or(false)
                {
                    let _ = window.open_devtools();
                }
            }

            show_main_window_for_startup(&window, total_started_at, startup_trace);
        }
        Err(e) => {
            error!(
                "Failed to create main window: error={} duration_ms={}",
                e,
                total_started_at.elapsed().as_millis()
            );
        }
    }
}

fn show_main_window_for_startup(
    window: &tauri::WebviewWindow,
    total_started_at: Instant,
    startup_trace: &DesktopStartupTrace,
) {
    #[cfg(target_os = "windows")]
    {
        let step_started_at = Instant::now();
        if let Err(error) = window.maximize() {
            warn!("Failed to maximize main window during startup: {}", error);
        } else {
            startup_trace.record_elapsed_step("native_window", "windows_maximize", step_started_at);
            debug!(
                "Main window startup show step completed: step=maximize duration_ms={} since_create_start_ms={}",
                step_started_at.elapsed().as_millis(),
                total_started_at.elapsed().as_millis()
            );
        }
        let show_delay_started_at = Instant::now();
        let show_wait_outcome = wait_for_windows_maximize_before_show(window);
        startup_trace.record_elapsed_step(
            "native_window",
            "windows_show_after_maximize_wait",
            show_delay_started_at,
        );
        debug!(
            "Main window startup show step completed: step=wait_for_maximize_state outcome={} duration_ms={} since_create_start_ms={}",
            show_wait_outcome,
            show_delay_started_at.elapsed().as_millis(),
            total_started_at.elapsed().as_millis()
        );
    }

    let show_started_at = Instant::now();
    if let Err(error) = window.show() {
        warn!("Failed to show main window during startup: {}", error);
        return;
    }
    startup_trace.record_elapsed_step("native_window", "show_window", show_started_at);
    debug!(
        "Main window startup show step completed: step=show duration_ms={} since_create_start_ms={}",
        show_started_at.elapsed().as_millis(),
        total_started_at.elapsed().as_millis()
    );

    let focus_started_at = Instant::now();
    if let Err(error) = window.set_focus() {
        warn!("Failed to focus main window during startup: {}", error);
        return;
    }
    startup_trace.record_elapsed_step("native_window", "focus_window", focus_started_at);
    debug!(
        "Main window startup show step completed: step=focus duration_ms={} since_create_start_ms={}",
        focus_started_at.elapsed().as_millis(),
        total_started_at.elapsed().as_millis()
    );
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
    let (area_position, area_size) = work_area_for_agent_companion_window(app, window)?;

    let monitor: Option<tauri::Monitor> = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let scale_factor = monitor
        .as_ref()
        .map(|monitor| monitor.scale_factor())
        .unwrap_or(1.0);
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
    let x =
        area_position.x + area_size.width - window_width - f64::from(AGENT_COMPANION_WINDOW_MARGIN);
    let y = area_position.y + area_size.height
        - window_height
        - f64::from(AGENT_COMPANION_WINDOW_MARGIN);

    Some(clamp_agent_companion_window_position(
        app,
        window,
        tauri::LogicalPosition::new(x, y),
        tauri::LogicalSize::new(window_width, window_height),
    ))
}

fn agent_companion_window_effective_size(window: &tauri::WebviewWindow) -> tauri::LogicalSize<f64> {
    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let size = window
        .outer_size()
        .ok()
        .map(|size| size.to_logical::<f64>(scale_factor))
        .unwrap_or_else(|| {
            tauri::LogicalSize::new(
                AGENT_COMPANION_WINDOW_MIN_SIZE,
                AGENT_COMPANION_WINDOW_MIN_SIZE,
            )
        });

    tauri::LogicalSize::new(
        size.width.clamp(
            AGENT_COMPANION_WINDOW_MIN_SIZE,
            AGENT_COMPANION_WINDOW_MAX_WIDTH,
        ),
        size.height.clamp(
            AGENT_COMPANION_WINDOW_MIN_SIZE,
            AGENT_COMPANION_WINDOW_MAX_HEIGHT,
        ),
    )
}

fn position_agent_companion_window(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let Some(position) = remembered_agent_companion_window_position()
        .or_else(|| agent_companion_default_position(app, window))
    else {
        return;
    };

    let size = agent_companion_window_effective_size(window);
    let position = clamp_agent_companion_window_position(app, window, position, size);

    if let Err(e) = window.set_position(position) {
        warn!("Failed to position Agent companion window: {}", e);
    } else {
        remember_agent_companion_window_position(position);
    }
}

fn resize_agent_companion_window(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    width: f64,
    height: f64,
) {
    if !width.is_finite() || !height.is_finite() {
        warn!(
            "Ignored invalid Agent companion window size: width={}, height={}",
            width, height
        );
        return;
    }

    let width = width.clamp(
        AGENT_COMPANION_WINDOW_MIN_SIZE,
        AGENT_COMPANION_WINDOW_MAX_WIDTH,
    );
    let height = height.clamp(
        AGENT_COMPANION_WINDOW_MIN_SIZE,
        AGENT_COMPANION_WINDOW_MAX_HEIGHT,
    );
    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let size = agent_companion_window_effective_size(window);
    if (size.width - width).abs() < 0.5 && (size.height - height).abs() < 0.5 {
        return;
    }

    let old_position = window
        .outer_position()
        .ok()
        .map(|position| position.to_logical::<f64>(scale_factor));

    if let Err(e) = window.set_size(tauri::LogicalSize::new(width, height)) {
        warn!("Failed to resize Agent companion window: {}", e);
        return;
    }

    // Keep the bottom-right corner fixed when bubbles change height. If we cannot
    // read the previous geometry (e.g. transient platform errors), avoid snapping
    // back to the default corner — that would feel like the pet "jumped".
    if let Some(position) = old_position {
        let next_position = clamp_agent_companion_window_position(
            app,
            window,
            tauri::LogicalPosition::new(
                position.x + size.width - width,
                position.y + size.height - height,
            ),
            tauri::LogicalSize::new(width, height),
        );
        if let Err(e) = window.set_position(next_position) {
            warn!("Failed to position Agent companion window: {}", e);
        } else {
            remember_agent_companion_window_position(next_position);
        }
    }
}

#[tauri::command]
pub async fn show_agent_companion_desktop_pet(app: tauri::AppHandle) -> Result<(), String> {
    let started_at = Instant::now();
    let _guard = agent_companion_window_ops().lock().await;
    debug!("Agent companion window show requested");

    // Reuse any existing window: never destroy here. A previous implementation destroyed
    // whenever `is_visible` was false, which raced with another `show` that had built the
    // window but not called `show()` yet (or with `hide`), producing duplicate pets or
    // stuck windows.
    if let Some(window) = app.get_webview_window(AGENT_COMPANION_WINDOW_LABEL) {
        if let Err(e) = window.unminimize() {
            warn!("Failed to unminimize Agent companion window: {}", e);
        }
        position_agent_companion_window(&app, &window);
        window.show().map_err(|e| {
            error!("Failed to show Agent companion window: {}", e);
            format!("Failed to show Agent companion window: {}", e)
        })?;
        debug!(
            "Agent companion window reused: total_duration_ms={}",
            started_at.elapsed().as_millis()
        );
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
        .background_color(tauri::window::Color(0, 0, 0, 0))
        .on_page_load({
            let started_at = started_at;
            move |_window, payload| {
                let event = match payload.event() {
                    PageLoadEvent::Started => "started",
                    PageLoadEvent::Finished => "finished",
                };
                debug!(
                    "Agent companion window page load event: event={}, url={}, since_show_request_ms={}",
                    event,
                    payload.url(),
                    started_at.elapsed().as_millis()
                );
            }
        });

    builder = builder.disable_drag_drop_handler();

    let build_started_at = Instant::now();
    let window = builder.build().map_err(|e| {
        error!(
            "Failed to create Agent companion window: error={} duration_ms={}",
            e,
            build_started_at.elapsed().as_millis()
        );
        format!("Failed to create Agent companion window: {}", e)
    })?;
    debug!(
        "Agent companion window creation step completed: step=build duration_ms={} total_duration_ms={}",
        build_started_at.elapsed().as_millis(),
        started_at.elapsed().as_millis()
    );

    position_agent_companion_window(&app, &window);

    let show_started_at = Instant::now();
    window.show().map_err(|e| {
        error!("Failed to show Agent companion window: {}", e);
        format!("Failed to show Agent companion window: {}", e)
    })?;
    debug!(
        "Agent companion window shown: show_duration_ms={} total_duration_ms={}",
        show_started_at.elapsed().as_millis(),
        started_at.elapsed().as_millis()
    );

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
        let app_for_resize = app.clone();
        let window_for_resize = window.clone();
        window
            .run_on_main_thread(move || {
                resize_agent_companion_window(&app_for_resize, &window_for_resize, width, height);
            })
            .map_err(|e| {
                warn!("Failed to schedule Agent companion window resize: {}", e);
                format!("Failed to schedule Agent companion window resize: {}", e)
            })?;
    }
    Ok(())
}

#[tauri::command]
pub async fn hide_agent_companion_desktop_pet(app: tauri::AppHandle) -> Result<(), String> {
    let _guard = agent_companion_window_ops().lock().await;
    if let Some(window) = app.get_webview_window(AGENT_COMPANION_WINDOW_LABEL) {
        if let Ok(scale_factor) = window.scale_factor() {
            if let Ok(position) = window.outer_position() {
                remember_agent_companion_window_position(position.to_logical::<f64>(scale_factor));
            }
        }
        window.destroy().map_err(|e| {
            error!("Failed to destroy Agent companion window: {}", e);
            format!("Failed to destroy Agent companion window: {}", e)
        })?;
    }
    Ok(())
}

#[tauri::command]
pub async fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    let total_started_at = Instant::now();
    if let Some(main_window) = app.get_webview_window("main") {
        #[cfg(target_os = "windows")]
        {
            // Work around Windows startup flicker: avoid creating the native window
            // in maximized mode, and maximize it right before showing instead.
            let step_started_at = Instant::now();
            main_window.maximize().map_err(|e| {
                error!("Failed to maximize main window: {}", e);
                format!("Failed to maximize main window: {}", e)
            })?;
            debug!(
                "Main window show step completed: step=maximize duration_ms={}",
                step_started_at.elapsed().as_millis()
            );

            let wait_started_at = Instant::now();
            let show_wait_outcome = wait_for_windows_maximize_before_show_async(&main_window).await;
            debug!(
                "Main window show step completed: step=wait_for_maximize_state outcome={} duration_ms={}",
                show_wait_outcome,
                wait_started_at.elapsed().as_millis()
            );
        }

        let step_started_at = Instant::now();
        main_window.show().map_err(|e| {
            error!("Failed to show main window: {}", e);
            format!("Failed to show main window: {}", e)
        })?;
        debug!(
            "Main window show step completed: step=show duration_ms={}",
            step_started_at.elapsed().as_millis()
        );

        #[cfg(target_os = "macos")]
        {
            crate::cancel_main_window_close_request_on_macos();
            crate::mark_main_window_hidden_on_macos(false);
        }

        let step_started_at = Instant::now();
        main_window.set_focus().map_err(|e| {
            error!("Failed to focus main window: {}", e);
            format!("Failed to focus main window: {}", e)
        })?;
        debug!(
            "Main window show step completed: step=focus duration_ms={}",
            step_started_at.elapsed().as_millis()
        );
    } else {
        error!("Main window not found");
        return Err("Main window not found".to_string());
    }

    debug!(
        "Main window shown: total_duration_ms={}",
        total_started_at.elapsed().as_millis()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_maximize_show_wait_releases_when_maximized() {
        assert_eq!(
            windows_maximize_show_wait_action(Some(true), Duration::ZERO),
            WindowsMaximizeShowWaitAction::Sleep(WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_POLL)
        );
        assert_eq!(
            windows_maximize_show_wait_action(Some(true), WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_MIN),
            WindowsMaximizeShowWaitAction::Ready
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_maximize_show_wait_polls_until_max_wait() {
        assert_eq!(
            windows_maximize_show_wait_action(Some(false), Duration::from_millis(20)),
            WindowsMaximizeShowWaitAction::Sleep(WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_POLL)
        );
        assert_eq!(
            windows_maximize_show_wait_action(None, Duration::from_millis(148)),
            WindowsMaximizeShowWaitAction::Sleep(Duration::from_millis(2))
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_maximize_show_wait_times_out_at_original_bound() {
        assert_eq!(
            windows_maximize_show_wait_action(Some(false), WINDOWS_STARTUP_MAXIMIZE_SHOW_WAIT_MAX),
            WindowsMaximizeShowWaitAction::TimedOut
        );
    }
}
