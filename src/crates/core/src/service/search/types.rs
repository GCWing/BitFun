use crate::infrastructure::FileSearchOutcome;
use codgrep::sdk::{FileCount, RepoStatus, SearchBackend, SearchModeConfig, TaskStatus};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentSearchOutputMode {
    Content,
    FilesWithMatches,
    Count,
}

impl ContentSearchOutputMode {
    pub fn search_mode(self) -> SearchModeConfig {
        match self {
            Self::Content => SearchModeConfig::MaterializeMatches,
            Self::Count => SearchModeConfig::CountOnly,
            Self::FilesWithMatches => SearchModeConfig::FirstHitOnly,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContentSearchRequest {
    pub repo_root: PathBuf,
    pub search_path: Option<PathBuf>,
    pub pattern: String,
    pub output_mode: ContentSearchOutputMode,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub whole_word: bool,
    pub multiline: bool,
    pub before_context: usize,
    pub after_context: usize,
    pub max_results: Option<usize>,
    pub globs: Vec<String>,
    pub file_types: Vec<String>,
    pub exclude_file_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GlobSearchRequest {
    pub repo_root: PathBuf,
    pub search_path: Option<PathBuf>,
    pub pattern: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIndexStatus {
    pub repo_status: RepoStatus,
    pub active_task: Option<TaskStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSearchResult {
    pub outcome: FileSearchOutcome,
    pub file_counts: Vec<FileCount>,
    pub backend: SearchBackend,
    pub repo_status: RepoStatus,
    pub candidate_docs: usize,
    pub matched_lines: usize,
    pub matched_occurrences: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobSearchResult {
    pub paths: Vec<String>,
    pub repo_status: RepoStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexTaskHandle {
    pub task: TaskStatus,
    pub repo_status: RepoStatus,
}
