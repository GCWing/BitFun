//! Errors from the shared on-disk format owners.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Service error: {0}")]
    Service(String),
    #[error("Tool error: {0}")]
    Tool(String),
}
pub type StorageResult<T> = Result<T, StorageError>;
impl StorageError {
    pub fn config(message: String) -> Self {
        Self::Config(message)
    }
    pub fn validation(message: String) -> Self {
        Self::Validation(message)
    }
    pub fn io(message: String) -> Self {
        Self::Io(message)
    }
    pub fn service(message: String) -> Self {
        Self::Service(message)
    }
    pub fn tool(message: String) -> Self {
        Self::Tool(message)
    }
}
