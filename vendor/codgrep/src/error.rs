use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("walkdir error: {0}")]
    Walkdir(#[from] walkdir::Error),
    #[error("ignore error: {0}")]
    Ignore(#[from] ignore::Error),
    #[error("regex error: {0}")]
    Regex(#[from] regex::Error),
    #[error("invalid regex pattern: {0}")]
    InvalidPattern(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("unsupported index format: {0}")]
    InvalidIndex(String),
    #[error("value out of range: {0}")]
    ValueOutOfRange(String),
    #[error("binary data is truncated")]
    TruncatedData,
    #[error("operation cancelled")]
    Cancelled,
    #[error("utf-8 decode failed for {path}: {source}")]
    InvalidUtf8 {
        path: PathBuf,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("rg: No files were searched, which means ripgrep probably applied a filter you didn't expect.\nRunning with --debug will show why files are being skipped.")]
    NoFilesSearched,
}

pub type Result<T> = std::result::Result<T, AppError>;
