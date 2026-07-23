//! Diff service type definitions

use serde::{Deserialize, Serialize};

/// Diff configuration
#[derive(Debug, Clone, Default)]
pub struct DiffConfig {
    /// Default context line count
    pub default_context_lines: usize,
    /// Computation timeout (milliseconds)
    pub timeout_ms: u64,
    /// Whether to enable character-level diffs
    pub enable_char_diff: bool,
}

impl DiffConfig {
    pub fn new() -> Self {
        Self {
            default_context_lines: 3,
            timeout_ms: 5000,
            enable_char_diff: true,
        }
    }
}

/// Diff line type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffLineType {
    /// Unchanged (context)
    Context,
    /// Added
    Add,
    /// Deleted
    Delete,
}

/// Diff line
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    /// Line type
    pub line_type: DiffLineType,
    /// Content
    pub content: String,
    /// Original file line number
    pub old_line_number: Option<usize>,
    /// New file line number
    pub new_line_number: Option<usize>,
}

/// Diff hunk (change block)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    /// Original file start line
    pub old_start: usize,
    /// Original file line count
    pub old_lines: usize,
    /// New file start line
    pub new_start: usize,
    /// New file line count
    pub new_lines: usize,
    /// Changed lines
    pub lines: Vec<DiffLine>,
}

/// Diff result
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiffResult {
    /// Hunk list
    pub hunks: Vec<DiffHunk>,
    /// Added line count
    pub additions: usize,
    /// Deleted line count
    pub deletions: usize,
    /// Total change count
    pub changes: usize,
}

/// Diff computation options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiffOptions {
    /// Whether to ignore whitespace
    #[serde(default)]
    pub ignore_whitespace: bool,
    /// Context line count
    #[serde(default = "default_context_lines")]
    pub context_lines: usize,
}

fn default_context_lines() -> usize {
    3
}

/// Character-level diff segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharDiffSegment {
    /// Segment type
    pub segment_type: DiffLineType,
    /// Value
    pub value: String,
}

/// Character-level diff result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharDiffResult {
    /// Original line
    pub original_line: String,
    /// Modified line
    pub modified_line: String,
    /// Diff segments
    pub segments: Vec<CharDiffSegment>,
}
