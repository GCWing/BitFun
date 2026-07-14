//! Edit constraint guard.
//!
//! Extracts "don't modify X" constraints from the task's first user message
//! (once per session, best-effort, fail-open) and exposes a deterministic
//! matcher so Edit/Write/Delete tools can reject edits that violate them via
//! their existing `validate_input()` path. See docs/plans/edit-constraint-guard-plan.md.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::tools::framework::{ToolUseContext, ValidationResult};
use crate::infrastructure::ai::get_global_ai_client_factory;
use crate::util::json_extract::extract_json_from_ai_response;
use crate::util::types::Message;

/// Cap on how much of the first user message we send to the extraction model.
/// PR-style task descriptions can run long; constraints are almost always
/// stated near the top ("I've already taken care of...", "do not modify...").
const MAX_PROMPT_CHARS: usize = 8_000;

const EXTRACTION_SYSTEM_PROMPT: &str = r#"You extract explicit prohibitions from a software task description.

Find sentences that explicitly forbid modifying certain files, file types, or
categories of files (e.g. "don't modify the test files", "you don't need to
change the testing logic", "do not touch the migration files"). Ignore
constraints about anything other than *which files may be edited* (e.g. style
preferences, scope limits on behavior) — this tool only enforces file-edit
prohibitions.

For each prohibition found, classify it into exactly ONE of these matcher kinds:
- "test_files": the prohibition is about test files / testing logic in general
- "path_contains": the prohibition names specific files or keywords (give the literal substrings)
- "path_under_dir": the prohibition names a specific directory (give the directory names)
- "extension": the prohibition is about a specific file type (give the extensions, including the dot)
- "unmatched": you found a prohibition but it doesn't fit any of the above (still report it, just tag as unmatched)

Respond with ONLY a fenced ```json code block containing this exact shape (no other text):
```json
{
  "constraints": [
    {"description": "<short paraphrase of the prohibition>", "matcher": {"kind": "test_files"}},
    {"description": "<short paraphrase>", "matcher": {"kind": "path_contains", "substrings": ["..."]}},
    {"description": "<short paraphrase>", "matcher": {"kind": "path_under_dir", "dirs": ["..."]}},
    {"description": "<short paraphrase>", "matcher": {"kind": "extension", "exts": [".ext"]}},
    {"description": "<short paraphrase>", "matcher": {"kind": "unmatched"}}
  ]
}
```
If there are no such prohibitions, respond with `{"constraints": []}`."#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedConstraint {
    /// Human-readable paraphrase, shown back to the agent in the rejection message.
    pub description: String,
    pub matcher: ConstraintMatcher,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConstraintMatcher {
    /// Reuses the same file-naming heuristic previously validated by the
    /// (reverted) test-selection-gate: `_test.go`, `test_*.py`/`*_test.py`,
    /// `.test.`/`.spec.`, or files under a `tests`/`test`/`__tests__` directory.
    TestFiles,
    PathContains { substrings: Vec<String> },
    PathUnderDir { dirs: Vec<String> },
    Extension { exts: Vec<String> },
    /// Extraction recognized a prohibition but could not map it to a
    /// machine-checkable pattern. Never enforced; kept for future analysis.
    Unmatched,
}

impl ConstraintMatcher {
    pub fn matches(&self, file_path: &str) -> bool {
        match self {
            ConstraintMatcher::TestFiles => is_test_file(file_path),
            ConstraintMatcher::PathContains { substrings } => {
                substrings.iter().any(|s| !s.is_empty() && file_path.contains(s.as_str()))
            }
            ConstraintMatcher::PathUnderDir { dirs } => dirs.iter().any(|d| {
                let d = d.trim_matches('/');
                if d.is_empty() {
                    return false;
                }
                file_path.starts_with(&format!("{d}/")) || file_path.contains(&format!("/{d}/"))
            }),
            ConstraintMatcher::Extension { exts } => {
                exts.iter().any(|e| !e.is_empty() && file_path.ends_with(e.as_str()))
            }
            ConstraintMatcher::Unmatched => false,
        }
    }
}

/// Language-aware test-file heuristic. Re-derived from the (reverted)
/// `test_selection_gate.rs::Lang::is_test_file` rules rather than imported,
/// since that module no longer exists on this branch.
fn is_test_file(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    let name = Path::new(&normalized)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    if name.ends_with("_test.go") {
        return true;
    }
    if name.starts_with("test_") || name.ends_with("_test.py") {
        return true;
    }
    if name.contains(".test.") || name.contains(".spec.") {
        return true;
    }
    if normalized.split('/').any(|seg| seg == "tests" || seg == "test" || seg == "__tests__") {
        return true;
    }
    false
}

#[derive(Debug, Default, Deserialize)]
struct ExtractionResponse {
    #[serde(default)]
    constraints: Vec<ExtractedConstraint>,
}

/// One-shot, best-effort extraction against the task's first user message.
/// Never propagates an error to the caller: any failure (no AI client
/// configured, network error, malformed response) resolves to `Vec::new()`
/// (fail open — no constraints enforced, identical to today's behavior).
pub async fn extract_constraints(user_message: &str) -> Vec<ExtractedConstraint> {
    if user_message.trim().is_empty() {
        return Vec::new();
    }
    let truncated: String = user_message.chars().take(MAX_PROMPT_CHARS).collect();

    let Ok(factory) = get_global_ai_client_factory().await else {
        return Vec::new();
    };
    let Ok(client) = factory.get_client_resolved("fast").await else {
        return Vec::new();
    };

    let prompt = format!(
        "{EXTRACTION_SYSTEM_PROMPT}\n\n<task_description>\n{truncated}\n</task_description>"
    );

    let Ok(response) = client.send_message(vec![Message::user(prompt)], None).await else {
        return Vec::new();
    };
    if response.text.is_empty() {
        return Vec::new();
    }
    let Some(json_str) = extract_json_from_ai_response(&response.text) else {
        return Vec::new();
    };
    serde_json::from_str::<ExtractionResponse>(&json_str)
        .map(|r| r.constraints)
        .unwrap_or_default()
}

/// Returns the first constraint (if any) that `file_path` violates.
pub fn find_violation<'a>(
    constraints: &'a [ExtractedConstraint],
    file_path: &str,
) -> Option<&'a ExtractedConstraint> {
    constraints.iter().find(|c| c.matcher.matches(file_path))
}

/// Rejection message threaded back to the model as the tool's error result.
/// Designed to redirect the agent toward reconsidering its implementation
/// rather than just reporting "action denied".
pub fn violation_message(file_path: &str, constraint: &ExtractedConstraint) -> String {
    format!(
        "This file (`{file_path}`) matches a constraint stated in the task: \"{}\". \
         This edit was not applied.\n\n\
         Editing a file you were told not to touch usually means your own implementation \
         doesn't match what's expected — not that the file is wrong. Reconsider your \
         source-code approach instead of adjusting this file.\n\n\
         If you're certain this file must change for a legitimate reason unrelated to \
         making your own code compile or pass tests, explain why before retrying with \
         `force: true`.",
        constraint.description
    )
}

/// Shared `validate_input()` check for Edit/Write/Delete. Returns `Some(rejection)`
/// if `file_path` violates a cached constraint for this session and the caller
/// didn't pass `force: true`; `None` means the edit may proceed (no session,
/// no cached constraints yet, no violation, or the escape hatch was used).
///
/// `force: true` is intentionally silent here — it only suppresses the block,
/// it does not validate the agent's stated justification (the input schema
/// requires the tool description to ask for one; enforcing its *content* would
/// need another LLM round-trip, which is out of scope for v1). The bypass is
/// still visible in the tool call args for later trajectory review.
pub fn check(
    context: Option<&ToolUseContext>,
    file_path: &str,
    force: bool,
) -> Option<ValidationResult> {
    if force {
        return None;
    }
    let session_id = context.and_then(|c| c.session_id.as_deref())?;
    let coordinator = get_global_coordinator()?;
    let constraints = coordinator.get_session_manager().edit_constraints(session_id)?;
    let violation = find_violation(&constraints, file_path)?;

    Some(ValidationResult {
        result: false,
        message: Some(violation_message(file_path, violation)),
        error_code: Some(403),
        meta: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_files_matcher_covers_common_conventions() {
        let m = ConstraintMatcher::TestFiles;
        assert!(m.matches("report/util_test.go"));
        assert!(m.matches("pkg/foo/test_bar.py"));
        assert!(m.matches("pkg/foo/bar_test.py"));
        assert!(m.matches("src/foo.test.tsx"));
        assert!(m.matches("src/foo.spec.ts"));
        assert!(m.matches("test/components/Foo-test.tsx"));
        assert!(m.matches("__tests__/foo.js"));
        assert!(!m.matches("src/foo.ts"));
        assert!(!m.matches("report/util.go"));
    }

    #[test]
    fn path_contains_matcher() {
        let m = ConstraintMatcher::PathContains {
            substrings: vec!["package-lock.json".to_string()],
        };
        assert!(m.matches("frontend/package-lock.json"));
        assert!(!m.matches("frontend/package.json"));
    }

    #[test]
    fn path_under_dir_matcher() {
        let m = ConstraintMatcher::PathUnderDir {
            dirs: vec!["migrations".to_string()],
        };
        assert!(m.matches("migrations/0001_init.sql"));
        assert!(m.matches("db/migrations/0002_add_column.sql"));
        assert!(!m.matches("db/migration_helpers.go"));
    }

    #[test]
    fn extension_matcher() {
        let m = ConstraintMatcher::Extension {
            exts: vec![".lock".to_string()],
        };
        assert!(m.matches("Cargo.lock"));
        assert!(!m.matches("Cargo.toml"));
    }

    #[test]
    fn unmatched_never_matches() {
        let m = ConstraintMatcher::Unmatched;
        assert!(!m.matches("anything.go"));
    }

    #[test]
    fn find_violation_returns_first_match() {
        let constraints = vec![
            ExtractedConstraint {
                description: "don't touch tests".to_string(),
                matcher: ConstraintMatcher::TestFiles,
            },
            ExtractedConstraint {
                description: "don't touch lockfiles".to_string(),
                matcher: ConstraintMatcher::Extension {
                    exts: vec![".lock".to_string()],
                },
            },
        ];
        assert_eq!(
            find_violation(&constraints, "report/util_test.go").map(|c| c.description.as_str()),
            Some("don't touch tests")
        );
        assert_eq!(
            find_violation(&constraints, "Cargo.lock").map(|c| c.description.as_str()),
            Some("don't touch lockfiles")
        );
        assert!(find_violation(&constraints, "src/main.rs").is_none());
    }

    #[tokio::test]
    async fn extract_constraints_returns_empty_for_blank_input() {
        assert!(extract_constraints("").await.is_empty());
        assert!(extract_constraints("   \n  ").await.is_empty());
    }
}
