use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// Agent info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_type: String,
    pub model_name: String,
    pub description: Option<String>,
}

/// Operation type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationType {
    Create,
    Modify,
    Delete,
    Rename,
}

/// Snapshot type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotType {
    Before,
    After,
    Baseline,
}

/// Tool execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContext {
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub execution_time_ms: u64,
}

/// File operation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperation {
    pub operation_id: String,
    pub session_id: String,
    pub turn_index: usize,
    pub seq_in_turn: usize,
    pub file_path: PathBuf,
    pub operation_type: OperationType,
    pub tool_context: ToolContext,
    pub before_snapshot_id: Option<String>,
    pub after_snapshot_id: Option<String>,
    pub timestamp: SystemTime,
    pub diff_summary: DiffSummary,
    pub path_before: Option<PathBuf>,
    pub path_after: Option<PathBuf>,
}

/// Diff summary
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiffSummary {
    pub lines_added: usize,
    pub lines_removed: usize,
    pub lines_modified: usize,
}

/// Line-level diff stats for a session file (badge / toolbars), without full file contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionFileDiffStats {
    pub file_path: String,
    pub lines_added: usize,
    pub lines_removed: usize,
    /// True when stats were derived from per-operation summaries instead of a full baseline vs disk diff.
    pub approximate: bool,
    /// `create`, `modify`, or `delete` for UI mapping.
    pub change_kind: String,
}

/// File modification status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileModificationStatus {
    Modified,
    Created,
    Deleted,
    Unchanged,
}

/// Session info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub operations: Vec<FileOperation>,
}

/// File snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub snapshot_id: String,
    pub file_path: PathBuf,
    pub content_hash: String,
    pub snapshot_type: SnapshotType,
    pub compressed_content: Vec<u8>,
    pub timestamp: SystemTime,
    pub metadata: FileMetadata,
}

/// File metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub size: u64,
    pub permissions: Option<u32>,
    pub last_modified: SystemTime,
    pub encoding: String,
}

/// Optimized content storage
#[derive(Debug, Clone)]
pub enum OptimizedContent {
    Raw(Vec<u8>),
    Compressed(Vec<u8>),
    Reference(String),
}

/// File snapshot system state
#[derive(Debug)]
pub struct FileSnapshotSystem {
    pub snapshot_dir: PathBuf,
    pub hash_to_content: HashMap<String, PathBuf>,
    pub active_snapshots: HashMap<String, FileSnapshot>,
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageStats {
    pub total_snapshots: usize,
    pub total_size_bytes: u64,
    pub compressed_size_bytes: u64,
    pub compression_ratio: f32,
    pub dedup_savings_bytes: u64,
}

/// Snapshot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotConfig {
    pub auto_snapshot_enabled: bool,
    pub snapshot_interval_minutes: u64,
    pub max_session_retention_days: u64,
    pub compression_threshold_kb: usize,
    pub max_cache_size_mb: usize,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            auto_snapshot_enabled: true,
            snapshot_interval_minutes: 30,
            max_session_retention_days: 30,
            compression_threshold_kb: 100,
            max_cache_size_mb: 500,
        }
    }
}

/// Snapshot error type
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Snapshot not found: {0}")]
    SnapshotNotFound(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Operation not found: {0}")]
    OperationNotFound(String),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Git isolation verification failed: {0}")]
    GitIsolationFailure(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Tool execution error: {0}")]
    ToolExecution(#[from] crate::util::errors::BitFunError),
}

pub type SnapshotResult<T> = Result<T, SnapshotError>;

impl FileSnapshot {
    /// Creates a new file snapshot.
    pub fn new(
        file_path: PathBuf,
        content: Vec<u8>,
        metadata: FileMetadata,
        snapshot_type: SnapshotType,
    ) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let content_hash = format!("{:x}", md5::compute(&content));

        let mut hasher = DefaultHasher::new();
        SystemTime::now().hash(&mut hasher);
        content_hash.hash(&mut hasher);
        let snapshot_id = format!("snap_{:x}", hasher.finish());

        Self {
            snapshot_id,
            file_path,
            content_hash,
            snapshot_type,
            compressed_content: content,
            timestamp: SystemTime::now(),
            metadata,
        }
    }
}
