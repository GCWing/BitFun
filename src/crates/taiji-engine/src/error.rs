use crate::types::state::StateKey;
use crate::types::NodeId;
use thiserror::Error;

/// Error kind categorization aligned with BitFun `PortErrorKind`.
///
/// Each [`TaijiError`] variant maps to one of these kinds, enabling
/// consistent triage at the engine boundary regardless of internal
/// variant shape.
///
/// ## Mapping to BitFun `PortErrorKind`
///
/// | `TaijiErrorKind`    | `PortErrorKind`      | Typical source in taiji-engine       |
/// |---------------------|----------------------|--------------------------------------|
/// | `NotAvailable`      | `NotAvailable`       | `AllSourcesDown` — all feeds offline |
/// | `NotFound`          | `NotFound`           | `KeyNotFound` — missing state key    |
/// | `InvalidRequest`    | `InvalidRequest`     | `Config`, `CycleDetected`, `Serde`   |
/// | `Backend`           | `Backend`            | `Io`, `DataSource`, `NodeFailed`, `Fusion` |
/// | `Timeout`           | `Timeout`            | (future) operation deadline exceeded |
/// | `Cancelled`         | `Cancelled`          | (future) operation cancelled         |
/// | `PermissionDenied`  | `PermissionDenied`   | (future) access control rejection    |
/// | `CleanupRequired`   | `CleanupRequired`    | (future) resource teardown needed    |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaijiErrorKind {
    NotAvailable,
    NotFound,
    InvalidRequest,
    PermissionDenied,
    Cancelled,
    Timeout,
    CleanupRequired,
    Backend,
}

#[derive(Debug, Error)]
pub enum TaijiError {
    #[error("config error: {0}")]
    Config(String),
    #[error("data source error: {0}")]
    DataSource(String),
    #[error("node '{node}' failed: {reason}")]
    NodeFailed { node: NodeId, reason: String },
    #[error("required key '{0}' not found")]
    KeyNotFound(StateKey),
    #[error("circular dependency: {0:?}")]
    CycleDetected(Vec<NodeId>),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("all sources down for '{0}'")]
    AllSourcesDown(String),
    #[error("fusion error: {0}")]
    Fusion(String),
    /// Operation timed out.
    #[error("timeout: {0}")]
    Timeout(String),
    /// Operation was cancelled.
    #[error("cancelled: {0}")]
    Cancelled(String),
    /// Permission denied for the requested operation.
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    /// Resource cleanup required before retry.
    #[error("cleanup required: {0}")]
    CleanupRequired(String),
}

impl TaijiError {
    /// Return the error kind, matching BitFun `PortErrorKind` semantics.
    ///
    /// This enables callers at the engine boundary to triage errors without
    /// matching on every internal variant.
    pub fn kind(&self) -> TaijiErrorKind {
        match self {
            Self::Config(_) => TaijiErrorKind::InvalidRequest,
            Self::CycleDetected(_) => TaijiErrorKind::InvalidRequest,
            Self::Serde(_) => TaijiErrorKind::InvalidRequest,
            Self::KeyNotFound(_) => TaijiErrorKind::NotFound,
            Self::AllSourcesDown(_) => TaijiErrorKind::NotAvailable,
            Self::DataSource(_) => TaijiErrorKind::Backend,
            Self::NodeFailed { .. } => TaijiErrorKind::Backend,
            Self::Io(_) => TaijiErrorKind::Backend,
            Self::Fusion(_) => TaijiErrorKind::Backend,
            Self::Timeout(_) => TaijiErrorKind::Timeout,
            Self::Cancelled(_) => TaijiErrorKind::Cancelled,
            Self::PermissionDenied(_) => TaijiErrorKind::PermissionDenied,
            Self::CleanupRequired(_) => TaijiErrorKind::CleanupRequired,
        }
    }

    /// Human-readable error message, mirroring `PortError::message`.
    pub fn message(&self) -> String {
        self.to_string()
    }

    /// Construct a `TaijiError` from a kind and message, aligned with
    /// `PortError::new(kind, message)`.
    ///
    /// When the kind maps to multiple possible variants (e.g. `Backend` →
    /// `DataSource` vs `Fusion` vs `NodeFailed`), this constructor picks a
    /// reasonable default. Callers that need a specific variant should
    /// construct it directly.
    pub fn new(kind: TaijiErrorKind, message: impl Into<String>) -> Self {
        let msg = message.into();
        match kind {
            TaijiErrorKind::InvalidRequest => Self::Config(msg),
            TaijiErrorKind::NotFound => Self::KeyNotFound(msg),
            TaijiErrorKind::NotAvailable => Self::AllSourcesDown(msg),
            TaijiErrorKind::Backend => Self::Fusion(msg),
            TaijiErrorKind::Timeout => Self::Timeout(msg),
            TaijiErrorKind::Cancelled => Self::Cancelled(msg),
            TaijiErrorKind::PermissionDenied => Self::PermissionDenied(msg),
            TaijiErrorKind::CleanupRequired => Self::CleanupRequired(msg),
        }
    }
}

pub type Result<T> = std::result::Result<T, TaijiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_maps_config_to_invalid_request() {
        let err = TaijiError::Config("bad".into());
        assert_eq!(err.kind(), TaijiErrorKind::InvalidRequest);
    }

    #[test]
    fn kind_maps_key_not_found_to_not_found() {
        let err = TaijiError::KeyNotFound("ma_fast".into());
        assert_eq!(err.kind(), TaijiErrorKind::NotFound);
    }

    #[test]
    fn kind_maps_all_sources_down_to_not_available() {
        let err = TaijiError::AllSourcesDown("AG2506".into());
        assert_eq!(err.kind(), TaijiErrorKind::NotAvailable);
    }

    #[test]
    fn kind_maps_io_to_backend() {
        let err = TaijiError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file missing",
        ));
        assert_eq!(err.kind(), TaijiErrorKind::Backend);
    }

    #[test]
    fn kind_maps_new_variants() {
        assert_eq!(
            TaijiError::Timeout("deadline".into()).kind(),
            TaijiErrorKind::Timeout
        );
        assert_eq!(
            TaijiError::Cancelled("user abort".into()).kind(),
            TaijiErrorKind::Cancelled
        );
        assert_eq!(
            TaijiError::PermissionDenied("no access".into()).kind(),
            TaijiErrorKind::PermissionDenied
        );
        assert_eq!(
            TaijiError::CleanupRequired("stale lock".into()).kind(),
            TaijiErrorKind::CleanupRequired
        );
    }

    #[test]
    fn new_constructs_correct_variant() {
        let err = TaijiError::new(TaijiErrorKind::Timeout, "too slow");
        assert!(matches!(err, TaijiError::Timeout(_)));
        assert_eq!(err.kind(), TaijiErrorKind::Timeout);
    }

    #[test]
    fn new_falls_back_for_backend() {
        let err = TaijiError::new(TaijiErrorKind::Backend, "disk full");
        assert!(matches!(err, TaijiError::Fusion(_)));
        assert_eq!(err.kind(), TaijiErrorKind::Backend);
    }

    #[test]
    fn message_returns_display_string() {
        let err = TaijiError::Config("invalid pipeline".into());
        assert_eq!(err.message(), "config error: invalid pipeline");
    }

    #[test]
    fn io_from_works() {
        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "oh no");
        let err: TaijiError = io_err.into();
        assert_eq!(err.kind(), TaijiErrorKind::Backend);
        assert!(err.message().contains("oh no"));
    }

    #[test]
    fn serde_from_works() {
        let serde_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err: TaijiError = serde_err.into();
        assert_eq!(err.kind(), TaijiErrorKind::InvalidRequest);
    }

    #[test]
    fn cycle_detected_display_includes_nodes() {
        let err = TaijiError::CycleDetected(vec!["a".into(), "b".into()]);
        let msg = err.message();
        assert!(msg.contains("a"));
        assert!(msg.contains("b"));
    }
}
