//! Diff API - Tauri commands for diff comparison

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeDiffRequest {
    #[serde(rename = "oldContent")]
    pub old_content: String,
    #[serde(rename = "newContent")]
    pub new_content: String,
    pub options: Option<DiffOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffOptions {
    pub ignore_whitespace: Option<bool>,
    pub context_lines: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub hunks: Vec<DiffHunk>,
    pub additions: usize,
    pub deletions: usize,
    pub changes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub line_type: String, // "context" | "add" | "delete"
    pub content: String,
    pub old_line_number: Option<usize>,
    pub new_line_number: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyPatchRequest {
    pub content: String,
    pub patch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveMergedContentRequest {
    #[serde(rename = "filePath")]
    pub file_path: String,
    pub content: String,
}

#[tauri::command]
pub async fn compute_diff(request: ComputeDiffRequest) -> Result<DiffResult, String> {
    let old_lines: Vec<&str> = request.old_content.lines().collect();
    let new_lines: Vec<&str> = request.new_content.lines().collect();
    let diff = similar::TextDiff::from_lines(&request.old_content, &request.new_content);

    let mut hunks = Vec::new();
    let mut additions = 0;
    let mut deletions = 0;

    for group in diff.grouped_ops(
        request
            .options
            .as_ref()
            .and_then(|o| o.context_lines)
            .unwrap_or(3),
    ) {
        let mut hunk_lines = Vec::new();
        let mut old_start = 0;
        let mut new_start = 0;
        let mut old_count = 0;
        let mut new_count = 0;

        for op in &group {
            match op {
                similar::DiffOp::Equal {
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
                            line_type: "context".to_string(),
                            content: old_lines.get(*old_index + i).unwrap_or(&"").to_string(),
                            old_line_number: Some(*old_index + i + 1),
                            new_line_number: Some(*new_index + i + 1),
                        });
                        old_count += 1;
                        new_count += 1;
                    }
                }
                similar::DiffOp::Delete {
                    old_index, old_len, ..
                } => {
                    if old_start == 0 {
                        old_start = *old_index + 1;
                    }
                    for i in 0..*old_len {
                        hunk_lines.push(DiffLine {
                            line_type: "delete".to_string(),
                            content: old_lines.get(*old_index + i).unwrap_or(&"").to_string(),
                            old_line_number: Some(*old_index + i + 1),
                            new_line_number: None,
                        });
                        old_count += 1;
                        deletions += 1;
                    }
                }
                similar::DiffOp::Insert {
                    new_index, new_len, ..
                } => {
                    if new_start == 0 {
                        new_start = *new_index + 1;
                    }
                    for i in 0..*new_len {
                        hunk_lines.push(DiffLine {
                            line_type: "add".to_string(),
                            content: new_lines.get(*new_index + i).unwrap_or(&"").to_string(),
                            old_line_number: None,
                            new_line_number: Some(*new_index + i + 1),
                        });
                        new_count += 1;
                        additions += 1;
                    }
                }
                similar::DiffOp::Replace {
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
                            line_type: "delete".to_string(),
                            content: old_lines.get(*old_index + i).unwrap_or(&"").to_string(),
                            old_line_number: Some(*old_index + i + 1),
                            new_line_number: None,
                        });
                        old_count += 1;
                        deletions += 1;
                    }
                    for i in 0..*new_len {
                        hunk_lines.push(DiffLine {
                            line_type: "add".to_string(),
                            content: new_lines.get(*new_index + i).unwrap_or(&"").to_string(),
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

    Ok(DiffResult {
        hunks,
        additions,
        deletions,
        changes: additions + deletions,
    })
}

#[tauri::command]
pub async fn apply_patch(request: ApplyPatchRequest) -> Result<String, String> {
    Ok(request.content)
}

#[tauri::command]
pub async fn save_merged_diff_content(request: SaveMergedContentRequest) -> Result<(), String> {
    let path = PathBuf::from(&request.file_path);

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    tokio::fs::write(&path, &request.content)
        .await
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}
