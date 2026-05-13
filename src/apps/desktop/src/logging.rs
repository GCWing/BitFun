//! Logging Configuration

use bitfun_core::infrastructure::get_path_manager_arc;
use chrono::Local;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{
    OnceLock,
    atomic::{AtomicU8, Ordering},
};
use std::thread;
use tauri::{Runtime, plugin::TauriPlugin};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy, fern};

const SESSION_DIR_PATTERN: &str = r"^\d{8}T\d{6}$";
const MAX_LOG_SESSIONS: usize = 10;
const FLASHGREP_LOG_TARGET_PREFIX: &str = "flashgrep";
static SESSION_LOG_DIR: OnceLock<PathBuf> = OnceLock::new();
// Default to Debug in early development for easier diagnostics
static CURRENT_LOG_LEVEL: AtomicU8 = AtomicU8::new(level_filter_to_u8(log::LevelFilter::Debug));

fn get_thread_id() -> u64 {
    let thread_id = thread::current().id();
    let id_str = format!("{:?}", thread_id);
    id_str
        .trim_start_matches("ThreadId(")
        .trim_end_matches(')')
        .parse()
        .unwrap_or(0)
}

#[derive(Debug, Clone)]
pub struct LogConfig {
    pub level: log::LevelFilter,
    pub is_debug: bool,
    pub session_log_dir: PathBuf,
}

fn is_embedded_webdriver_mode() -> bool {
    cfg!(debug_assertions) && std::env::var_os("BITFUN_WEBDRIVER_PORT").is_some()
}

fn resolve_logs_root() -> PathBuf {
    if let Some(path) = std::env::var_os("BITFUN_LOG_DIR").map(PathBuf::from) {
        return path;
    }

    if let Some(path) = std::env::var_os("BITFUN_E2E_LOG_DIR").map(PathBuf::from) {
        return path;
    }

    if is_embedded_webdriver_mode() {
        return std::env::temp_dir().join("bitfun-e2e-logs");
    }

    get_path_manager_arc().logs_dir()
}

impl LogConfig {
    pub fn new(is_debug: bool) -> Self {
        let level = resolve_default_level(is_debug);

        let session_log_dir = create_session_log_dir();

        Self {
            level,
            is_debug,
            session_log_dir,
        }
    }
}

const fn level_filter_to_u8(level: log::LevelFilter) -> u8 {
    match level {
        log::LevelFilter::Off => 0,
        log::LevelFilter::Error => 1,
        log::LevelFilter::Warn => 2,
        log::LevelFilter::Info => 3,
        log::LevelFilter::Debug => 4,
        log::LevelFilter::Trace => 5,
    }
}

const fn u8_to_level_filter(value: u8) -> log::LevelFilter {
    match value {
        0 => log::LevelFilter::Off,
        1 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        3 => log::LevelFilter::Info,
        4 => log::LevelFilter::Debug,
        5 => log::LevelFilter::Trace,
        _ => log::LevelFilter::Info,
    }
}

// Default to Debug in early development for easier diagnostics
fn resolve_default_level(_is_debug: bool) -> log::LevelFilter {
    log::LevelFilter::Debug
}

pub fn parse_log_level(value: &str) -> Option<log::LevelFilter> {
    match value.trim().to_lowercase().as_str() {
        "trace" => Some(log::LevelFilter::Trace),
        "debug" => Some(log::LevelFilter::Debug),
        "info" => Some(log::LevelFilter::Info),
        "warn" => Some(log::LevelFilter::Warn),
        "error" => Some(log::LevelFilter::Error),
        "off" => Some(log::LevelFilter::Off),
        _ => None,
    }
}

pub fn level_to_str(level: log::LevelFilter) -> &'static str {
    match level {
        log::LevelFilter::Trace => "trace",
        log::LevelFilter::Debug => "debug",
        log::LevelFilter::Info => "info",
        log::LevelFilter::Warn => "warn",
        log::LevelFilter::Error => "error",
        log::LevelFilter::Off => "off",
    }
}

pub fn register_runtime_log_state(initial_level: log::LevelFilter, session_log_dir: PathBuf) {
    let _ = SESSION_LOG_DIR.set(session_log_dir);
    CURRENT_LOG_LEVEL.store(level_filter_to_u8(initial_level), Ordering::Relaxed);
    log::set_max_level(initial_level);
}

pub fn current_runtime_log_level() -> log::LevelFilter {
    u8_to_level_filter(CURRENT_LOG_LEVEL.load(Ordering::Relaxed))
}

pub fn apply_runtime_log_level(level: log::LevelFilter, source: &str) {
    let old_level = current_runtime_log_level();
    if old_level == level {
        return;
    }

    log::set_max_level(level);
    CURRENT_LOG_LEVEL.store(level_filter_to_u8(level), Ordering::Relaxed);
    log::info!(
        "Runtime log level updated: old_level={}, new_level={}, source={}",
        level_to_str(old_level),
        level_to_str(level),
        source
    );
}

pub fn session_log_dir() -> Option<PathBuf> {
    SESSION_LOG_DIR.get().cloned()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLoggingInfo {
    pub effective_level: String,
    pub session_log_dir: String,
    pub app_log_path: String,
    pub ai_log_path: String,
    pub flashgrep_log_path: String,
    pub webview_log_path: String,
}

pub fn get_runtime_logging_info() -> RuntimeLoggingInfo {
    let fallback_dir = resolve_logs_root();
    let session_dir = session_log_dir().unwrap_or(fallback_dir);

    RuntimeLoggingInfo {
        effective_level: level_to_str(current_runtime_log_level()).to_string(),
        session_log_dir: session_dir.to_string_lossy().to_string(),
        app_log_path: session_dir.join("app.log").to_string_lossy().to_string(),
        ai_log_path: session_dir.join("ai.log").to_string_lossy().to_string(),
        flashgrep_log_path: session_dir
            .join("flashgrep.log")
            .to_string_lossy()
            .to_string(),
        webview_log_path: session_dir
            .join("webview.log")
            .to_string_lossy()
            .to_string(),
    }
}

fn is_flashgrep_target(target: &str) -> bool {
    target.starts_with(FLASHGREP_LOG_TARGET_PREFIX)
}

pub fn create_session_log_dir() -> PathBuf {
    let logs_root = resolve_logs_root();

    let timestamp = Local::now().format("%Y%m%dT%H%M%S").to_string();
    let session_dir = logs_root.join(&timestamp);

    if let Err(e) = std::fs::create_dir_all(&logs_root) {
        eprintln!("Warning: Failed to create logs root directory: {}", e);
        return logs_root;
    }

    if let Err(e) = std::fs::create_dir_all(&session_dir) {
        eprintln!("Warning: Failed to create log session directory: {}", e);
        return logs_root;
    }

    session_dir
}

pub fn build_log_targets(config: &LogConfig) -> Vec<Target> {
    let mut targets = Vec::new();
    let session_dir = config.session_log_dir.clone();
    let use_stdout_only = is_embedded_webdriver_mode();

    if config.is_debug || use_stdout_only {
        targets.push(
            Target::new(TargetKind::Stdout)
                .filter(|metadata| {
                    let target = metadata.target();
                    !target.starts_with("ai")
                        && !target.starts_with("webview")
                        && !is_flashgrep_target(target)
                })
                .format(|out, message, record| {
                    let target = record.target();
                    let simplified_target = if target.starts_with("webview:") {
                        "webview"
                    } else {
                        target
                    };

                    let (level_color, reset) = match record.level() {
                        log::Level::Error => ("\x1b[31m", "\x1b[0m"), // Red
                        log::Level::Warn => ("\x1b[33m", "\x1b[0m"),  // Yellow
                        log::Level::Info => ("\x1b[32m", "\x1b[0m"),  // Green
                        log::Level::Debug => ("\x1b[36m", "\x1b[0m"), // Cyan
                        log::Level::Trace => ("\x1b[90m", "\x1b[0m"), // Gray
                    };

                    out.finish(format_args!(
                        "[{}][tid:{}][{}{}{}][{}] {}",
                        chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
                        get_thread_id(),
                        level_color,
                        record.level(),
                        reset,
                        simplified_target,
                        message
                    ))
                }),
        );
    }

    if !use_stdout_only {
        let app_log_dir = session_dir.clone();
        targets.push(
            Target::new(TargetKind::Folder {
                path: app_log_dir,
                file_name: Some("app".into()),
            })
            .filter(|metadata| {
                let target = metadata.target();
                !target.starts_with("ai")
                    && !target.starts_with("webview")
                    && !is_flashgrep_target(target)
            })
            .format(format_log_plain),
        );

        let ai_log_dir = session_dir.clone();
        targets.push(
            Target::new(TargetKind::Folder {
                path: ai_log_dir,
                file_name: Some("ai".into()),
            })
            .filter(|metadata| metadata.target().starts_with("ai"))
            .format(format_log_plain),
        );

        let flashgrep_log_dir = session_dir.clone();
        targets.push(
            Target::new(TargetKind::Folder {
                path: flashgrep_log_dir,
                file_name: Some("flashgrep".into()),
            })
            .filter(|metadata| is_flashgrep_target(metadata.target()))
            .format(format_log_plain),
        );

        let webview_log_dir = session_dir;
        targets.push(
            Target::new(TargetKind::Folder {
                path: webview_log_dir,
                file_name: Some("webview".into()),
            })
            .filter(|metadata| metadata.target().starts_with("webview"))
            .format(format_log_plain),
        );
    }

    targets
}

pub fn build_log_plugin<R: Runtime>(log_targets: Vec<Target>) -> TauriPlugin<R> {
    tauri_plugin_log::Builder::new()
        .level(log::LevelFilter::Trace)
        .level_for("ignore", log::LevelFilter::Off)
        .level_for("ignore::walk", log::LevelFilter::Off)
        .level_for("globset", log::LevelFilter::Off)
        .level_for("tracing", log::LevelFilter::Off)
        .level_for("opentelemetry_sdk", log::LevelFilter::Off)
        .level_for("opentelemetry-otlp", log::LevelFilter::Off)
        .level_for("notify", log::LevelFilter::Off)
        // These targets can emit hot-path trace diagnostics during event
        // routing. Keep debug diagnostics, warnings, and errors, but avoid
        // drowning useful app traces in mechanical noise.
        .level_for(
            "bitfun_core::agentic::events::queue",
            log::LevelFilter::Debug,
        )
        .level_for(
            "bitfun_core::agentic::events::router",
            log::LevelFilter::Debug,
        )
        .level_for("hyper_util", log::LevelFilter::Info)
        .level_for("h2", log::LevelFilter::Info)
        .level_for("portable_pty", log::LevelFilter::Info)
        .level_for("russh", log::LevelFilter::Info)
        .level_for("grep_searcher", log::LevelFilter::Warn)
        .targets(log_targets)
        .rotation_strategy(RotationStrategy::KeepSome(2)) // 1 active + 2 backups
        .max_file_size(10 * 1024 * 1024)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .clear_format()
        .build()
}

fn format_log_plain(
    out: fern::FormatCallback,
    message: &std::fmt::Arguments,
    record: &log::Record,
) {
    let target = record.target();
    let simplified_target = if target.starts_with("webview:") {
        "webview"
    } else {
        target
    };

    out.finish(format_args!(
        "[{}][tid:{}][{}][{}] {}",
        chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
        get_thread_id(),
        record.level(),
        simplified_target,
        message
    ))
}

pub async fn cleanup_old_log_sessions() {
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    let logs_root = resolve_logs_root();

    if let Err(e) = do_cleanup_log_sessions(&logs_root, MAX_LOG_SESSIONS).await {
        log::warn!("Failed to cleanup old log sessions: {}", e);
    }
}

async fn do_cleanup_log_sessions(
    logs_root: &PathBuf,
    max_sessions: usize,
) -> Result<(), std::io::Error> {
    let regex = regex::Regex::new(SESSION_DIR_PATTERN).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Invalid session dir pattern: {}", e),
        )
    })?;
    let mut entries = tokio::fs::read_dir(logs_root).await?;
    let mut session_dirs: Vec<String> = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if !metadata.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if regex.is_match(&name) {
            session_dirs.push(name);
        }
    }

    session_dirs.sort();

    if session_dirs.len() <= max_sessions {
        return Ok(());
    }

    let excess_count = session_dirs.len() - max_sessions;
    let to_delete: Vec<_> = session_dirs.into_iter().take(excess_count).collect();

    log::info!("Cleaning up {} old log session(s)", to_delete.len());

    for session_name in to_delete {
        let session_path = logs_root.join(&session_name);
        match tokio::fs::remove_dir_all(&session_path).await {
            Ok(_) => {
                log::debug!("Removed old log session: {}", session_name);
            }
            Err(e) => {
                log::warn!("Failed to remove log session {}: {}", session_name, e);
            }
        }
    }

    Ok(())
}

pub fn spawn_log_cleanup_task() {
    tokio::spawn(async {
        cleanup_old_log_sessions().await;
    });
}
