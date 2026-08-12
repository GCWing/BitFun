//! PlanList tool implementation
//!
//! Lists plan files stored in the current workspace plans directory.

use crate::agentic::tools::framework::{Tool, ToolExposure, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::fs;
use tokio::io::AsyncReadExt;

/// PLAN-09: cap on how many plan files a single PlanList call reports, so a
/// plans directory with thousands of files cannot blow up the tool result.
const MAX_PLAN_LIST_ENTRIES: usize = 500;

/// PLAN-09: only the YAML frontmatter (always well under this) is needed for
/// todo progress. Reading a bounded prefix keeps PlanList fast and immune to
/// huge plan bodies; anything past 64KB is a body, not frontmatter.
const PLAN_FRONTMATTER_PREFIX_LIMIT: u64 = 64 * 1024;

/// PlanList tool - list plan files
pub struct PlanListTool;

impl PlanListTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlanListTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Best-effort todo progress for a plan file body: (total, completed) counts.
/// Returns None when the file is not a parseable plan (legacy plans without
/// todos, damaged frontmatter, unreadable files) - callers report 0/0/0.
///
/// d6-P2-6: a frontmatter larger than the bounded prefix is NOT reported as
/// "no todos". The second return value is `true` when the frontmatter closer
/// (`\n---`) could not be found inside the bounded prefix, i.e. the file is
/// truncated and the true counts are unknown (progress must be reported as
/// unknown, not 0/0/0). `false` means the prefix covered the whole
/// frontmatter and None is a genuine "no todos / unparseable" answer.
fn count_todo_progress(content: &str) -> (Option<(u64, u64)>, bool) {
    let trimmed = content.trim_start();
    let Some(after_open) = trimmed.strip_prefix("---") else {
        return (None, false);
    };
    let Some(end) = after_open.find("\n---") else {
        // The bounded prefix did not contain the frontmatter closer: the
        // frontmatter may continue past the prefix. Signal truncation so the
        // caller does not misreport 0/0/0 as "no todos".
        return (None, true);
    };
    let yaml_part = &after_open[..end];
    let Some(frontmatter) = serde_yaml::from_str::<Value>(yaml_part).ok() else {
        return (None, false);
    };
    let Some(todos) = frontmatter.get("todos").and_then(Value::as_array) else {
        return (None, false);
    };
    let total = todos.len() as u64;
    let completed = todos
        .iter()
        .filter(|todo| todo.get("status").and_then(|status| status.as_str()) == Some("completed"))
        .count() as u64;
    (Some((total, completed)), false)
}

#[async_trait]
impl Tool for PlanListTool {
    fn name(&self) -> &str {
        "PlanList"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r###"List plan files stored in the current workspace plans directory. Returns each plan file's name, full path and last-modified timestamp. Use this tool to discover existing plans before reading or updating them. Read-only: does not modify any files."###
            .to_string())
    }

    fn short_description(&self) -> String {
        "List plan files in the workspace plans directory.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        // 2026-08-04 user calibration: the plan tool family is a commander
        // staple; Direct so no GetToolSpec unlock round-trip is needed
        // (mirrored by `shared_coding_mode_tool_exposure_overrides()`).
        ToolExposure::Direct
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {}
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn call_impl(
        &self,
        _input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let runtime_context = context.ensure_current_workspace_runtime().await?;
        let plans_dir = runtime_context.plans_dir.clone();
        let plans_dir_str = plans_dir.to_string_lossy().to_string();

        // No plans directory yet is a valid empty listing, not an error.
        let mut entries = match fs::read_dir(&plans_dir).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let empty = json!({
                    "success": true,
                    "plans_dir": plans_dir_str,
                    "plans": [],
                    "count": 0
                });
                return Ok(vec![ToolResult::Result {
                    data: empty,
                    result_for_assistant: None,
                    image_attachments: None,
                }]);
            }
            Err(error) => {
                return Err(BitFunError::tool(format!(
                    "Failed to read plans directory: {}",
                    error
                )));
            }
        };

        let mut plans = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|error| {
            BitFunError::tool(format!("Failed to read plans directory entry: {}", error))
        })? {
            if plans.len() >= MAX_PLAN_LIST_ENTRIES {
                break;
            }
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy().to_string();
            if !name.ends_with(".plan.md") {
                continue;
            }
            let path = entry.path();
            let modified_ms = entry
                .metadata()
                .await
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| {
                    modified
                        .duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|duration| duration.as_millis() as u64)
                })
                .unwrap_or(0);

            // Best-effort todo progress from the bounded frontmatter prefix;
            // legacy plans without todos (or unreadable/damaged files) report
            // 0/0/0. PLAN-09: never read the whole plan body. d6-P2-6: when
            // the frontmatter is larger than the prefix the counts are
            // unknown — report `todo_progress_truncated: true` instead of a
            // misleading 0/0/0.
            let mut todo_total: u64 = 0;
            let mut todo_completed: u64 = 0;
            let mut completion_pct: u64 = 0;
            let mut todo_progress_truncated = false;
            let mut prefix = Vec::with_capacity(PLAN_FRONTMATTER_PREFIX_LIMIT as usize);
            if let Ok(file) = fs::File::open(&path).await {
                let read_ok = file
                    .take(PLAN_FRONTMATTER_PREFIX_LIMIT)
                    .read_to_end(&mut prefix)
                    .await
                    .is_ok();
                if read_ok {
                    let frontmatter_prefix = String::from_utf8_lossy(&prefix);
                    let (counts, truncated) = count_todo_progress(&frontmatter_prefix);
                    if let Some((total, completed)) = counts {
                        todo_total = total;
                        todo_completed = completed;
                        completion_pct = if total > 0 {
                            completed * 100 / total
                        } else {
                            0
                        };
                    } else if truncated {
                        // Frontmatter exceeds the bounded prefix: the true
                        // counts are unknown, not "no todos".
                        todo_progress_truncated = true;
                    }
                }
            }

            plans.push(json!({
                "name": name,
                "path": path.to_string_lossy().to_string(),
                "modified_ms": modified_ms,
                "todo_total": todo_total,
                "todo_completed": todo_completed,
                "completion_pct": completion_pct,
                "todo_progress_truncated": todo_progress_truncated
            }));
        }

        // Stable ordering by file name for deterministic output.
        plans.sort_by(|left, right| {
            left["name"]
                .as_str()
                .unwrap_or("")
                .cmp(right["name"].as_str().unwrap_or(""))
        });

        let result = json!({
            "success": true,
            "plans_dir": plans_dir_str,
            "plans": plans,
            "count": plans.len()
        });

        Ok(vec![ToolResult::Result {
            data: result,
            result_for_assistant: None,
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::count_todo_progress;

    #[test]
    fn count_todo_progress_counts_completed_statuses() {
        let content = "---\nname: My Plan\noverview: An overview\ntodos:\n- id: setup-auth\n  content: Set up auth\n  status: completed\n- id: implement-ui\n  content: Implement the UI\n  status: pending\n- id: deploy\n  content: Deploy\n  status: in_progress\n---\n\n# My Plan\n\nBody.\n";
        assert_eq!(count_todo_progress(content), (Some((3, 1)), false));
    }

    #[test]
    fn count_todo_progress_all_completed_rounds_pct_up() {
        let content = "---\nname: Done\ntodos:\n- id: a\n  content: A\n  status: completed\n- id: b\n  content: B\n  status: completed\n---\n\nbody";
        assert_eq!(count_todo_progress(content), (Some((2, 2)), false));
    }

    #[test]
    fn count_todo_progress_legacy_plan_without_todos_is_none() {
        // Legacy plans with no todos key: caller reports 0/0/0.
        let content = "---\nname: Legacy\n---\n\nbody";
        assert_eq!(count_todo_progress(content), (None, false));
    }

    #[test]
    fn count_todo_progress_empty_todos_is_zero_pair() {
        let content = "---\nname: Empty\ntodos: []\n---\n\nbody";
        assert_eq!(count_todo_progress(content), (Some((0, 0)), false));
    }

    #[test]
    fn count_todo_progress_damaged_file_is_none() {
        assert_eq!(count_todo_progress("no frontmatter here"), (None, false));
        assert_eq!(count_todo_progress(""), (None, false));
        // d6-P2-6: an opener with no closer inside the bounded prefix is a
        // truncation signal (frontmatter may continue past the prefix), not a
        // genuine "no todos" answer — the caller must not report 0/0/0.
        assert_eq!(count_todo_progress("---\nname: broken"), (None, true));
    }

    #[test]
    fn count_todo_progress_truncated_frontmatter_is_marked_truncated() {
        // d6-P2-6: a prefix that opens frontmatter but never reaches the
        // `\n---` closer means the frontmatter is larger than the bounded
        // prefix — the true counts are unknown, NOT "no todos".
        let content = "---\nname: Huge\noverview: never closed";
        assert_eq!(count_todo_progress(content), (None, true));
    }
}
