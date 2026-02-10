//! Logging Configuration

use bitfun_core::infrastructure::get_path_manager_arc;
use chrono::Local;
use std::path::PathBuf;
use std::thread;
use tauri_plugin_log::{fern, Target, TargetKind};

const SESSION_DIR_PATTERN: &str = r"^\d{8}T\d{6}$";
const MAX_LOG_SESSIONS: usize = 50;
const LOG_RETENTION_DAYS: i64 = 7;

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

impl LogConfig {
    pub fn new(is_debug: bool) -> Self {
        let level = match std::env::var("BITFUN_LOG_LEVEL") {
            Ok(val) => match val.to_lowercase().as_str() {
                "trace" => log::LevelFilter::Trace,
                "debug" => log::LevelFilter::Debug,
                "info" => log::LevelFilter::Info,
                "warn" => log::LevelFilter::Warn,
                "error" => log::LevelFilter::Error,
                "off" => log::LevelFilter::Off,
                other => {
                    eprintln!(
                        "Warning: Invalid BITFUN_LOG_LEVEL '{}', falling back to default",
                        other
                    );
                    if is_debug {
                        log::LevelFilter::Debug
                    } else {
                        log::LevelFilter::Info
                    }
                }
            },
            Err(_) => {
                if is_debug {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                }
            }
        };

        let session_log_dir = create_session_log_dir();

        Self {
            level,
            is_debug,
            session_log_dir,
        }
    }
}

pub fn create_session_log_dir() -> PathBuf {
    let pm = get_path_manager_arc();
    let logs_root = pm.logs_dir();

    let timestamp = Local::now().format("%Y%m%dT%H%M%S").to_string();
    let session_dir = logs_root.join(&timestamp);

    if let Err(e) = std::fs::create_dir_all(&session_dir) {
        eprintln!("Warning: Failed to create log session directory: {}", e);
        return logs_root;
    }

    session_dir
}

pub fn build_log_targets(config: &LogConfig) -> Vec<Target> {
    let mut targets = Vec::new();
    let session_dir = config.session_log_dir.clone();

    if config.is_debug {
        targets.push(
            Target::new(TargetKind::Stdout)
                .filter(|metadata| {
                    let target = metadata.target();
                    !target.starts_with("ai") && !target.starts_with("webview")
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

    let app_log_dir = session_dir.clone();
    targets.push(
        Target::new(TargetKind::Folder {
            path: app_log_dir,
            file_name: Some("app".into()),
        })
        .filter(|metadata| {
            let target = metadata.target();
            !target.starts_with("ai") && !target.starts_with("webview")
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

    let webview_log_dir = session_dir;
    targets.push(
        Target::new(TargetKind::Folder {
            path: webview_log_dir,
            file_name: Some("webview".into()),
        })
        .filter(|metadata| metadata.target().starts_with("webview"))
        .format(format_log_plain),
    );

    targets
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

fn parse_session_timestamp(name: &str) -> Option<chrono::NaiveDateTime> {
    chrono::NaiveDateTime::parse_from_str(name, "%Y%m%dT%H%M%S").ok()
}

pub async fn cleanup_old_log_sessions() {
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    let pm = get_path_manager_arc();
    let logs_root = pm.logs_dir();

    if let Err(e) = do_cleanup_log_sessions(&logs_root, MAX_LOG_SESSIONS).await {
        log::warn!("Failed to cleanup old log sessions: {}", e);
    }
}

async fn do_cleanup_log_sessions(
    logs_root: &PathBuf,
    max_sessions: usize,
) -> Result<(), std::io::Error> {
    let regex = regex::Regex::new(SESSION_DIR_PATTERN).expect("Invalid session dir pattern");
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

    let now = Local::now().naive_local();
    let retention_threshold = now - chrono::Duration::days(LOG_RETENTION_DAYS);

    let excess_count = session_dirs.len() - max_sessions;
    let to_delete: Vec<_> = session_dirs
        .into_iter()
        .take(excess_count)
        .filter(|name| {
            parse_session_timestamp(name)
                .map(|ts| ts < retention_threshold)
                .unwrap_or(false)
        })
        .collect();

    if to_delete.is_empty() {
        return Ok(());
    }

    log::info!(
        "Cleaning up {} old log session(s) older than {} days",
        to_delete.len(),
        LOG_RETENTION_DAYS
    );

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
