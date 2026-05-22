//! Shared file logging initialization for CLI modes.

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use crate::config::CliConfig;

pub fn resolve_log_dir() -> PathBuf {
    CliConfig::config_dir()
        .ok()
        .map(|d| d.join("logs"))
        .unwrap_or_else(|| std::env::temp_dir().join("bitfun-cli"))
}

pub fn resolve_log_file_path() -> PathBuf {
    resolve_log_dir().join("bitfun-cli.log")
}

pub fn init_file_logging_at(log_dir: &Path, log_level: tracing::Level) -> PathBuf {
    fs::create_dir_all(log_dir).ok();
    let log_file = log_dir.join("bitfun-cli.log");

    if let Ok(file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
    {
        tracing_subscriber::fmt()
            .with_max_level(log_level)
            .with_writer(move || file.try_clone().expect("log file clone"))
            .with_ansi(false)
            .with_target(false)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(log_level)
            .with_target(false)
            .init();
    }

    log_file
}

pub fn init_file_logging(log_level: tracing::Level) -> PathBuf {
    let log_dir = resolve_log_dir();
    init_file_logging_at(&log_dir, log_level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_file_logging_at_creates_log_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let log_file = init_file_logging_at(temp.path(), tracing::Level::INFO);
        assert!(log_file.exists());
        assert_eq!(log_file, temp.path().join("bitfun-cli.log"));
    }
}
