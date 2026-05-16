#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Repository not found: {0}")]
    RepositoryNotFound(String),

    #[error("Git command failed: {0}")]
    CommandFailed(String),

    #[error("Invalid repository path: {0}")]
    InvalidPath(String),

    #[error("Branch not found: {0}")]
    BranchNotFound(String),

    #[error("Merge conflict: {0}")]
    MergeConflict(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Git2 error: {0}")]
    Git2Error(#[from] git2::Error),
}
