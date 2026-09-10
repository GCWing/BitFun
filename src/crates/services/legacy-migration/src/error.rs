use openbitfun_product_domains::legacy_migration::MigrationDomainId;
use std::path::PathBuf;

pub type LegacyMigrationResult<T> = Result<T, LegacyMigrationError>;

#[derive(Debug, thiserror::Error)]
pub enum LegacyMigrationError {
    #[error("legacy migration path is unavailable: {0}")]
    PathUnavailable(String),
    #[error("legacy source and target resolve to the same path: {0}")]
    SourceEqualsTarget(PathBuf),
    #[error("legacy source format is unsupported: {0}")]
    UnsupportedSource(String),
    #[error("unsupported migration destination: {0}")]
    UnsupportedTarget(String),
    #[error("legacy migration request is invalid: {0}")]
    InvalidRequest(String),
    #[error("legacy migration plan is invalid: {0}")]
    InvalidPlan(String),
    #[error("legacy migration path escaped its declared root: {0}")]
    PathEscape(PathBuf),
    #[error("legacy migration refused a symbolic link or reparse point: {0}")]
    LinkedPath(PathBuf),
    #[error("legacy migration resource limit exceeded: {0}")]
    ResourceLimit(String),
    #[error("legacy migration is already running")]
    LockUnavailable,
    #[error("legacy migration was cancelled at a safe boundary")]
    Cancelled,
    #[error("legacy migration process inspection failed: {0}")]
    ProcessInspection(String),
    #[error("legacy migration executable is not trusted: {0}")]
    UntrustedExecutable(PathBuf),
    #[error("legacy migration trusted installation is unavailable: {0}")]
    TrustedInstallationUnavailable(String),
    #[error("legacy migration crash injection at {0:?}")]
    InjectedCrash(crate::CrashPoint),
    #[error("legacy migration domain {domain:?} failed: {message}")]
    Domain {
        domain: MigrationDomainId,
        message: String,
    },
    #[error("legacy migration I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("legacy migration JSON failed for {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("legacy migration SQLite failed for {path}: {source}")]
    Sqlite {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
}

impl LegacyMigrationError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn json(path: impl Into<PathBuf>, source: serde_json::Error) -> Self {
        Self::Json {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn sqlite(path: impl Into<PathBuf>, source: rusqlite::Error) -> Self {
        Self::Sqlite {
            path: path.into(),
            source,
        }
    }
}
