//! Filesystem error boundary.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSystemError {
    message: String,
}

pub type FileSystemResult<T> = Result<T, FileSystemError>;

impl FileSystemError {
    pub fn service(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for FileSystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FileSystemError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_error_display_preserves_message() {
        let error = FileSystemError::service("File does not exist: sample.txt");
        assert_eq!(error.to_string(), "File does not exist: sample.txt");
    }
}
