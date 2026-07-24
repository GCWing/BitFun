use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpeechError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Cancelled(String),
    #[error("{0}")]
    Http(String),
    #[error("{0}")]
    Service(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

impl SpeechError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn service(message: impl Into<String>) -> Self {
        Self::Service(message.into())
    }
}

pub type SpeechResult<T> = Result<T, SpeechError>;
