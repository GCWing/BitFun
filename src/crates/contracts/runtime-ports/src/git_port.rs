use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceDiffFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Conflicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkspaceDiffContent {
    Text { patch: String },
    Binary,
    TooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDiffFile {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub status: WorkspaceDiffFileStatus,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub additions: usize,
    pub deletions: usize,
    pub content: WorkspaceDiffContent,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDiffSnapshot {
    pub files: Vec<WorkspaceDiffFile>,
    pub truncated: bool,
}

#[async_trait::async_trait]
pub trait GitPort: RuntimeServicePort {
    async fn workspace_diff(&self) -> PortResult<WorkspaceDiffSnapshot> {
        Err(PortError::new(
            PortErrorKind::NotAvailable,
            "workspace diff is not supported by this provider",
        ))
    }
}
