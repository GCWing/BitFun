//! Diff service implementation
//!
//! Uses the `similar` crate to implement an efficient Myers diff algorithm.

use similar::{DiffOp, TextDiff};
use std::time::Duration;
use tokio::time::timeout;

use super::types::*;

/// Diff service
pub struct DiffService {
    config: DiffConfig,
}

impl DiffService {
    /// Creates a new `DiffService`.
    pub fn new(config: DiffConfig) -> Self {
        Self { config }
    }

    /// Creates with default configuration.
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(DiffConfig::new())
    }

    /// Computes the diff between two texts.
    pub fn compute_diff(&self, original: &str, modified: &str) -> DiffResult {
        self.compute_diff_with_options(original, modified, &DiffOptions::default())
    }

    /// Computes the diff using options.
    pub fn compute_diff_with_options(
        &self,
        original: &str,
        modified: &str,
        options: &DiffOptions,
    ) -> DiffResult {
        let original_lines: Vec<&str> = original.lines().collect();
        let modified_lines: Vec<&str> = modified.lines().collect();

        let diff = TextDiff::from_lines(original, modified);

        let mut hunks = Vec::new();
        let mut additions = 0;
        let mut deletions = 0;

        let context_lines = if options.context_lines > 0 {
            options.context_lines
        } else {
            self.config.default_context_lines
        };

        for group in diff.grouped_ops(context_lines) {
            let mut hunk_lines = Vec::new();
            let mut old_start = 0;
            let mut new_start = 0;
            let mut old_count = 0;
            let mut new_count = 0;

            for op in &group {
                match op {
                    DiffOp::Equal {
                        old_index,
                        new_index,
                        len,
                    } => {
                        if old_start == 0 {
                            old_start = *old_index + 1;
                        }
                        if new_start == 0 {
                            new_start = *new_index + 1;
                        }

                        for i in 0..*len {
                            hunk_lines.push(DiffLine {
                                line_type: DiffLineType::Context,
                                content: original_lines
                                    .get(*old_index + i)
                                    .unwrap_or(&"")
                                    .to_string(),
                                old_line_number: Some(*old_index + i + 1),
                                new_line_number: Some(*new_index + i + 1),
                            });
                            old_count += 1;
                            new_count += 1;
                        }
                    }
                    DiffOp::Delete {
                        old_index, old_len, ..
                    } => {
                        if old_start == 0 {
                            old_start = *old_index + 1;
                        }

                        for i in 0..*old_len {
                            hunk_lines.push(DiffLine {
                                line_type: DiffLineType::Delete,
                                content: original_lines
                                    .get(*old_index + i)
                                    .unwrap_or(&"")
                                    .to_string(),
                                old_line_number: Some(*old_index + i + 1),
                                new_line_number: None,
                            });
                            old_count += 1;
                            deletions += 1;
                        }
                    }
                    DiffOp::Insert {
                        new_index, new_len, ..
                    } => {
                        if new_start == 0 {
                            new_start = *new_index + 1;
                        }

                        for i in 0..*new_len {
                            hunk_lines.push(DiffLine {
                                line_type: DiffLineType::Add,
                                content: modified_lines
                                    .get(*new_index + i)
                                    .unwrap_or(&"")
                                    .to_string(),
                                old_line_number: None,
                                new_line_number: Some(*new_index + i + 1),
                            });
                            new_count += 1;
                            additions += 1;
                        }
                    }
                    DiffOp::Replace {
                        old_index,
                        old_len,
                        new_index,
                        new_len,
                    } => {
                        if old_start == 0 {
                            old_start = *old_index + 1;
                        }
                        if new_start == 0 {
                            new_start = *new_index + 1;
                        }

                        for i in 0..*old_len {
                            hunk_lines.push(DiffLine {
                                line_type: DiffLineType::Delete,
                                content: original_lines
                                    .get(*old_index + i)
                                    .unwrap_or(&"")
                                    .to_string(),
                                old_line_number: Some(*old_index + i + 1),
                                new_line_number: None,
                            });
                            old_count += 1;
                            deletions += 1;
                        }

                        for i in 0..*new_len {
                            hunk_lines.push(DiffLine {
                                line_type: DiffLineType::Add,
                                content: modified_lines
                                    .get(*new_index + i)
                                    .unwrap_or(&"")
                                    .to_string(),
                                old_line_number: None,
                                new_line_number: Some(*new_index + i + 1),
                            });
                            new_count += 1;
                            additions += 1;
                        }
                    }
                }
            }

            if !hunk_lines.is_empty() {
                hunks.push(DiffHunk {
                    old_start,
                    old_lines: old_count,
                    new_start,
                    new_lines: new_count,
                    lines: hunk_lines,
                });
            }
        }

        DiffResult {
            hunks,
            additions,
            deletions,
            changes: additions + deletions,
        }
    }

    /// Diff calculation with timeout.
    pub async fn compute_with_timeout(
        &self,
        original: &str,
        modified: &str,
        timeout_ms: u64,
    ) -> Result<DiffResult, String> {
        let original = original.to_string();
        let modified = modified.to_string();
        let config = self.config.clone();

        let result = timeout(
            Duration::from_millis(timeout_ms),
            tokio::task::spawn_blocking(move || {
                let service = DiffService::new(config);
                service.compute_diff(&original, &modified)
            }),
        )
        .await;

        match result {
            Ok(Ok(diff_result)) => Ok(diff_result),
            Ok(Err(e)) => Err(format!("Diff computation failed: {}", e)),
            Err(_) => Err(format!("Diff computation timed out after {}ms", timeout_ms)),
        }
    }

    /// Computes a character-level diff.
    pub fn compute_char_diff(&self, original: &str, modified: &str) -> CharDiffResult {
        use similar::TextDiff;

        let diff = TextDiff::from_chars(original, modified);
        let mut segments = Vec::new();

        for change in diff.iter_all_changes() {
            let segment_type = match change.tag() {
                similar::ChangeTag::Equal => DiffLineType::Context,
                similar::ChangeTag::Insert => DiffLineType::Add,
                similar::ChangeTag::Delete => DiffLineType::Delete,
            };

            segments.push(CharDiffSegment {
                segment_type,
                value: change.value().to_string(),
            });
        }

        CharDiffResult {
            original_line: original.to_string(),
            modified_line: modified.to_string(),
            segments,
        }
    }
}
