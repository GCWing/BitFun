use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum AppError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("protocol error: {0}")]
    Protocol(String),
}

pub(crate) type Result<T> = std::result::Result<T, AppError>;
