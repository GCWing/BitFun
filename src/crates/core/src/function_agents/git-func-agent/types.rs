/**
 * Git Function Agent - type definitions
 *
 * Defines data structures for commit message generation
 */
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitMessageOptions {
    #[serde(default = "default_commit_format")]
    pub format: CommitFormat,

    #[serde(default = "default_true")]
    pub include_files: bool,

    #[serde(default = "default_max_length")]
    pub max_title_length: usize,

    #[serde(default = "default_true")]
    pub include_body: bool,

    #[serde(default = "default_language")]
    pub language: Language,
}

fn default_commit_format() -> CommitFormat {
    CommitFormat::Conventional
}

fn default_true() -> bool {
    true
}

fn default_max_length() -> usize {
    72
}

fn default_language() -> Language {
    Language::Chinese
}

impl Default for CommitMessageOptions {
    fn default() -> Self {
        Self {
            format: CommitFormat::Conventional,
            include_files: true,
            max_title_length: 72,
            include_body: true,
            language: Language::Chinese,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CommitFormat {
    /// Conventional Commits spec
    Conventional,
    /// Angular style
    Angular,
    /// Simple format
    Simple,
    /// Custom format
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Language {
    Chinese,
    English,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitMessage {
    /// Title (50-72 chars)
    pub title: String,

    pub body: Option<String>,

    /// Footer info (breaking changes, etc.)
    pub footer: Option<String>,

    pub full_message: String,

    pub commit_type: CommitType,

    pub scope: Option<String>,

    /// Confidence (0.0-1.0)
    pub confidence: f32,

    pub changes_summary: ChangesSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CommitType {
    /// New feature
    Feat,
    /// Bug fix
    Fix,
    /// Documentation update
    Docs,
    /// Code formatting
    Style,
    /// Refactoring
    Refactor,
    /// Performance optimization
    Perf,
    /// Testing
    Test,
    /// Build/tools/dependencies
    Chore,
    /// CI config
    CI,
    /// Revert
    Revert,
}

impl fmt::Display for CommitType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CommitType::Feat => "feat",
            CommitType::Fix => "fix",
            CommitType::Docs => "docs",
            CommitType::Style => "style",
            CommitType::Refactor => "refactor",
            CommitType::Perf => "perf",
            CommitType::Test => "test",
            CommitType::Chore => "chore",
            CommitType::CI => "ci",
            CommitType::Revert => "revert",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesSummary {
    pub total_additions: u32,

    pub total_deletions: u32,

    pub files_changed: u32,

    pub file_changes: Vec<FileChange>,

    pub affected_modules: Vec<String>,

    pub change_patterns: Vec<ChangePattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,

    pub change_type: FileChangeType,

    pub additions: u32,

    pub deletions: u32,

    pub file_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangePattern {
    FeatureAddition,
    BugFix,
    Refactoring,
    ConfigChange,
    DependencyUpdate,
    DocumentationUpdate,
    TestUpdate,
    StyleChange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentError {
    pub message: String,
    pub error_type: AgentErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentErrorType {
    GitError,
    AnalysisError,
    InvalidInput,
    InternalError,
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}", self.error_type, self.message)
    }
}

impl std::error::Error for AgentError {}

impl AgentError {
    pub fn git_error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            error_type: AgentErrorType::GitError,
        }
    }

    pub fn analysis_error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            error_type: AgentErrorType::AnalysisError,
        }
    }

    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            error_type: AgentErrorType::InvalidInput,
        }
    }

    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            error_type: AgentErrorType::InternalError,
        }
    }
}

pub type AgentResult<T> = Result<T, AgentError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    /// Project type (e.g., web-app, library, cli-tool, etc.)
    pub project_type: String,

    pub tech_stack: Vec<String>,

    pub project_docs: Option<String>,

    pub code_standards: Option<String>,
}

impl Default for ProjectContext {
    fn default() -> Self {
        Self {
            project_type: "unknown".to_string(),
            tech_stack: vec![],
            project_docs: None,
            code_standards: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AICommitAnalysis {
    pub commit_type: CommitType,

    pub scope: Option<String>,

    pub title: String,

    pub body: Option<String>,

    pub breaking_changes: Option<String>,

    pub reasoning: String,

    pub confidence: f32,
}
