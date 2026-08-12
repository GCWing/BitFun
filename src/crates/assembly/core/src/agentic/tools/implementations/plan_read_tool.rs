//! PlanRead tool implementation
//!
//! Reads a plan file from the workspace plans directory and returns its
//! structured content (YAML frontmatter: name/overview/todos + markdown body).

use crate::agentic::tools::framework::{Tool, ToolExposure, ToolResult, ToolUseContext};
use crate::agentic::tools::restrictions::is_local_path_within_root;
use crate::agentic::tools::workspace_paths::{is_bitfun_runtime_uri, parse_bitfun_runtime_uri};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::fs;

/// YAML frontmatter structure for Plan files (mirror of the CreatePlan
/// writer; fields are optional so older or hand-edited files stay readable).
#[derive(Debug, Deserialize)]
struct PlanFrontmatter {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    overview: Option<String>,
    #[serde(default)]
    todos: Vec<TodoItem>,
}

/// Todo item structure (mirror of the CreatePlan writer).
#[derive(Debug, Deserialize)]
struct TodoItem {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
}

/// PlanRead tool - read plan file
pub struct PlanReadTool;

impl PlanReadTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlanReadTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a plan file body into its YAML frontmatter and markdown body.
fn parse_plan_file(content: &str) -> BitFunResult<(PlanFrontmatter, String)> {
    let trimmed = content.trim_start();
    let after_open = trimmed.strip_prefix("---").ok_or_else(|| {
        BitFunError::tool("Plan file is missing the YAML frontmatter opener '---'")
    })?;
    let end = after_open.find("\n---").ok_or_else(|| {
        BitFunError::tool("Plan file is missing the YAML frontmatter closer '---'")
    })?;
    // PLAN-05: CRLF files keep a trailing '\r' on the last frontmatter line
    // before the closer; strip it so serde_yaml never sees a dangling CR.
    let yaml_part = after_open[..end].trim_end_matches('\r');
    let body_start = end + "\n---".len();
    let body = after_open[body_start..]
        .trim_start_matches(['\n', '\r'])
        .to_string();

    let frontmatter: PlanFrontmatter = serde_yaml::from_str(yaml_part).map_err(|error| {
        BitFunError::tool(format!("Failed to parse plan YAML frontmatter: {}", error))
    })?;
    Ok((frontmatter, body))
}

#[async_trait]
impl Tool for PlanReadTool {
    fn name(&self) -> &str {
        "PlanRead"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r###"Read a plan file from the current workspace plans directory (or an absolute plan file path). The input accepts the plan file name (for example "my_plan_1234abcd.plan.md") or a full path to a .plan.md file. Returns the parsed YAML frontmatter (name, overview, todos with id/content/status/dependencies) plus the raw markdown body. Read-only: does not modify any files."###
            .to_string())
    }

    fn short_description(&self) -> String {
        "Read and parse a plan file from the workspace plans directory.".to_string()
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
            "required": ["plan_file"],
            "properties": {
                "plan_file": {
                    "type": "string",
                    "description": "Plan file name (e.g. my_plan_1234abcd.plan.md) or an absolute path to a .plan.md file"
                }
            }
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
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let plan_file = input
            .get("plan_file")
            .and_then(|value| value.as_str())
            .ok_or(BitFunError::validation("Missing required field: plan_file"))?;
        let plan_file = plan_file.trim();
        if plan_file.is_empty() {
            return Err(BitFunError::validation("Missing required field: plan_file"));
        }

        let plan_path = resolve_plan_path(plan_file, context)?;
        let content = fs::read_to_string(&plan_path)
            .await
            .map_err(|error| BitFunError::tool(format!("Failed to read plan file: {}", error)))?;

        let (frontmatter, body) = parse_plan_file(&content)?;

        let todos = frontmatter
            .todos
            .into_iter()
            .map(|todo| {
                json!({
                    "id": todo.id.unwrap_or_default(),
                    "content": todo.content.unwrap_or_default(),
                    "status": todo.status.unwrap_or_else(|| "pending".to_string()),
                    "dependencies": todo.dependencies
                })
            })
            .collect::<Vec<_>>();

        let plan_reference = context.build_runtime_artifact_reference(&format!(
            "plans/{}",
            plan_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        ))?;

        let result = json!({
            "success": true,
            "plan_file_name": plan_path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(),
            "plan_file_path": plan_reference,
            "name": frontmatter.name,
            "overview": frontmatter.overview,
            "todos": todos,
            "body": body
        });

        Ok(vec![ToolResult::Result {
            data: result,
            result_for_assistant: None,
            image_attachments: None,
        }])
    }
}

/// Validate that the plan file argument ends with `.plan.md`. Note:
/// extension() only returns the last suffix ("md" for "xxx.plan.md"), so the
/// full file name suffix is validated instead.
fn validate_plan_file_suffix(plan_file: &str) -> BitFunResult<()> {
    let file_name = Path::new(plan_file)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if !file_name.ends_with(".plan.md") {
        return Err(BitFunError::tool(format!(
            "Plan file must end with .plan.md: {}",
            plan_file
        )));
    }
    Ok(())
}

/// PLAN-01: the canonical resolved path must live inside `plans_dir` and the
/// file must exist. Rejects `..` escapes and symlink jumps.
fn require_plan_file_exists(
    plan_path: PathBuf,
    display: &str,
    plans_dir: &Path,
) -> BitFunResult<PathBuf> {
    if !is_local_path_within_root(&plan_path, plans_dir)? {
        return Err(BitFunError::tool(format!(
            "Plan file resolves outside the plans directory: {}",
            display
        )));
    }
    // PLAN-12: `exists()` 是同步文件系统调用，在异步执行器中会造成轻微阻塞。
    // 计划路径短、存在性检查开销极小，且与仓库其他工具（file_read/file_write 等）
    // 的同步 IO 风格一致，保留现状可接受；若未来出现性能敏感场景，再改用
    // `tokio::fs::try_exists()` 或 `tokio::task::spawn_blocking` 包裹。
    if !plan_path.exists() {
        return Err(BitFunError::tool(format!(
            "Plan file not found: {}",
            display
        )));
    }
    Ok(plan_path)
}

/// Shared plan-path resolution core (PLAN-13). Every PlanRead/PlanUpdate entry
/// point (tool call, permission intents, backend scheduler) converges here so
/// suffix validation, the plans-dir containment fence and the runtime-URI
/// branch can never drift apart.
///
/// Accepted inputs:
/// - a `bitfun://runtime/<scope>/plans/<file>` URI (must point inside the
///   plans directory; scope is checked against `expected_workspace_scope`),
/// - an absolute path (kept only when the canonical path stays inside
///   `plans_dir`),
/// - a bare `.plan.md` file name or relative path (joined to `plans_dir`, so a
///   separator or `..` cannot escape the fence).
pub(crate) fn resolve_plan_path_with_plans_dir(
    plan_file: &str,
    plans_dir: &Path,
    expected_workspace_scope: Option<&str>,
) -> BitFunResult<PathBuf> {
    // PLAN-10: accept the `bitfun://runtime/...` URI that CreatePlan returns
    // on remote workspaces.
    if is_bitfun_runtime_uri(plan_file) {
        let parsed = parse_bitfun_runtime_uri(plan_file)?;
        if let Some(expected_scope) = expected_workspace_scope {
            if parsed.workspace_scope != "current" && parsed.workspace_scope != expected_scope {
                return Err(BitFunError::tool(format!(
                    "Plan runtime URI belongs to workspace '{}', expected '{}': {}",
                    parsed.workspace_scope, expected_scope, plan_file
                )));
            }
        }
        let file = parsed.relative_path.strip_prefix("plans/").ok_or_else(|| {
            BitFunError::tool(format!(
                "Plan runtime URI must point inside the plans directory: {}",
                plan_file
            ))
        })?;
        if file.is_empty() || file.contains('/') {
            return Err(BitFunError::tool(format!(
                "Plan runtime URI must reference a single plan file: {}",
                plan_file
            )));
        }
        validate_plan_file_suffix(file)?;
        return require_plan_file_exists(plans_dir.join(file), plan_file, plans_dir);
    }

    let supplied = PathBuf::from(plan_file);
    if supplied.is_absolute() {
        // PLAN-01: absolute paths are no longer trusted as-is; they must stay
        // inside the plans directory.
        validate_plan_file_suffix(plan_file)?;
        return require_plan_file_exists(supplied, plan_file, plans_dir);
    }

    // PLAN-01/07: bare names AND relative paths (separator / `..`) are always
    // resolved inside plans_dir, and the suffix check applies to both.
    validate_plan_file_suffix(plan_file)?;
    require_plan_file_exists(plans_dir.join(plan_file), plan_file, plans_dir)
}

/// Resolve the plan file argument to a concrete filesystem path inside the
/// current workspace's plans directory. See
/// [`resolve_plan_path_with_plans_dir`] for the accepted input forms.
pub(crate) fn resolve_plan_path(
    plan_file: &str,
    context: &ToolUseContext,
) -> BitFunResult<PathBuf> {
    let plans_dir = context.current_workspace_runtime_root()?.join("plans");
    resolve_plan_path_with_plans_dir(
        plan_file,
        &plans_dir,
        context.current_workspace_scope().as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::parse_plan_file;

    #[test]
    fn parse_plan_file_reads_frontmatter_and_body() {
        let content = "---\nname: My Plan\noverview: An overview\ntodos:\n- id: setup-auth\n  content: Set up auth\n  status: pending\n---\n\n# My Plan\n\nBody text here.\n";
        let (frontmatter, body) = parse_plan_file(content).expect("parse plan file");
        assert_eq!(frontmatter.name.as_deref(), Some("My Plan"));
        assert_eq!(frontmatter.overview.as_deref(), Some("An overview"));
        assert_eq!(frontmatter.todos.len(), 1);
        assert_eq!(frontmatter.todos[0].id.as_deref(), Some("setup-auth"));
        assert_eq!(frontmatter.todos[0].content.as_deref(), Some("Set up auth"));
        assert_eq!(frontmatter.todos[0].status.as_deref(), Some("pending"));
        assert!(frontmatter.todos[0].dependencies.is_empty());
        assert!(body.contains("Body text here."));
    }

    #[test]
    fn parse_plan_file_round_trips_create_plan_writer_format() {
        // Mirror the exact layout emitted by create_plan_tool.rs
        // `generate_plan_file_content` (---\n<yaml>---\n\n<body>).
        let content = "---\nname: deploy-api\noverview: Deploy the API service\ntodos:\n- id: setup-auth\n  content: Set up auth\n  status: pending\n- id: implement-ui\n  content: Implement the UI\n  status: pending\n  dependencies:\n  - setup-auth\n---\n\n# deploy-api\n\n## Steps\n\n1. Auth\n2. UI\n";
        let (frontmatter, body) = parse_plan_file(content).expect("parse plan file");
        assert_eq!(frontmatter.name.as_deref(), Some("deploy-api"));
        assert_eq!(frontmatter.todos.len(), 2);
        assert_eq!(frontmatter.todos[1].id.as_deref(), Some("implement-ui"));
        assert_eq!(
            frontmatter.todos[1].dependencies,
            vec!["setup-auth".to_string()]
        );
        assert!(body.starts_with("# deploy-api"));
        assert!(body.contains("1. Auth"));
    }

    #[test]
    fn parse_plan_file_missing_delimiters_errors() {
        assert!(parse_plan_file("no frontmatter here").is_err());
        assert!(parse_plan_file("---\nname: x").is_err());
    }

    #[test]
    fn parse_plan_file_tolerates_missing_optional_fields() {
        let content = "---\nname: Minimal\n---\n\nBody";
        let (frontmatter, body) = parse_plan_file(content).expect("parse plan file");
        assert_eq!(frontmatter.name.as_deref(), Some("Minimal"));
        assert!(frontmatter.overview.is_none());
        assert!(frontmatter.todos.is_empty());
        assert!(body.contains("Body"));
    }

    use super::{resolve_plan_path, resolve_plan_path_with_plans_dir};
    use crate::agentic::tools::framework::ToolUseContext;
    use serde_json::json;
    use std::path::Path;
    use uuid::Uuid;

    /// Context whose runtime root points at `runtime_root`, so
    /// `current_workspace_runtime_root()` resolves without real FS side effects.
    fn test_context(runtime_root: &Path) -> ToolUseContext {
        let mut context = ToolUseContext::for_tool_listing(None, None);
        context.custom_data.insert(
            "__bitfun_test_runtime_root".to_string(),
            json!(runtime_root.to_string_lossy().to_string()),
        );
        context
    }

    #[test]
    fn resolve_plan_path_absolute_plan_md_suffix_succeeds() {
        // Regression: xxx.plan.md must be accepted via absolute path
        // (extension() alone would report only "md"), as long as it stays
        // inside the plans directory.
        let dir = std::env::temp_dir().join(format!("plan-read-resolve-{}", Uuid::new_v4()));
        let plans_dir = dir.join("plans");
        std::fs::create_dir_all(&plans_dir).expect("temp plans dir should be created");
        let plan_path = plans_dir.join("my_plan_1234abcd.plan.md");
        std::fs::write(&plan_path, "---\nname: Test\n---\n\nBody").expect("write plan file");
        let result = resolve_plan_path(
            plan_path.to_str().expect("temp plan path must be UTF-8"),
            &test_context(&dir),
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            result.expect("absolute .plan.md path must resolve"),
            plan_path
        );
    }

    #[test]
    fn resolve_plan_path_rejects_wrong_suffix() {
        let error = resolve_plan_path("C:/tmp/not_a_plan.md", &test_context(Path::new("C:/tmp")))
            .expect_err("non-.plan.md absolute path must error");
        let message = error.to_string();
        assert!(
            message.contains("Plan file must end with .plan.md"),
            "unexpected error: {}",
            message
        );
    }

    #[test]
    fn resolve_plan_path_rejects_absolute_path_outside_plans_dir() {
        // PLAN-01: an absolute .plan.md path outside the plans directory must
        // be rejected by the containment fence even when the file exists.
        let dir = std::env::temp_dir().join(format!("plan-read-fence-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("plans")).expect("plans dir should be created");
        let outside = dir.join("outside.plan.md");
        std::fs::write(&outside, "---\nname: X\n---\n\nBody").expect("write outside file");
        let error = resolve_plan_path(
            outside.to_str().expect("temp plan path must be UTF-8"),
            &test_context(&dir),
        )
        .expect_err("path outside plans dir must error");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            error
                .to_string()
                .contains("resolves outside the plans directory"),
            "unexpected error: {}",
            error
        );
    }

    #[test]
    fn resolve_plan_path_rejects_parent_directory_escape() {
        // PLAN-01: `..` input must not escape the plans directory.
        let dir = std::env::temp_dir().join(format!("plan-read-dotdot-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("plans")).expect("plans dir should be created");
        let error = resolve_plan_path("../escape.plan.md", &test_context(&dir))
            .expect_err(".. escape must error");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            error
                .to_string()
                .contains("resolves outside the plans directory"),
            "unexpected error: {}",
            error
        );
    }

    #[test]
    fn resolve_plan_path_rejects_bare_name_without_plan_md_suffix() {
        // PLAN-07: the bare-name branch must validate the .plan.md suffix too.
        let dir = std::env::temp_dir().join(format!("plan-read-suffix-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("plans")).expect("plans dir should be created");
        let error = resolve_plan_path("not_a_plan.md", &test_context(&dir))
            .expect_err("bare name without .plan.md suffix must error");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            error
                .to_string()
                .contains("Plan file must end with .plan.md"),
            "unexpected error: {}",
            error
        );
    }

    #[test]
    fn resolve_plan_path_accepts_bare_name_inside_plans_dir() {
        let dir = std::env::temp_dir().join(format!("plan-read-bare-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("plans")).expect("plans dir should be created");
        std::fs::write(
            dir.join("plans/plan_abc.plan.md"),
            "---\nname: X\n---\n\nBody",
        )
        .expect("write plan file");
        let result = resolve_plan_path("plan_abc.plan.md", &test_context(&dir));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            result.expect("bare name inside plans dir must resolve"),
            dir.join("plans/plan_abc.plan.md")
        );
    }

    #[test]
    fn resolve_plan_path_resolves_runtime_uri_inside_plans_dir() {
        // PLAN-10: the bitfun://runtime/... URI returned by CreatePlan on
        // remote workspaces must resolve to the local mirror plan path.
        let dir = std::env::temp_dir().join(format!("plan-read-uri-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("plans")).expect("plans dir should be created");
        std::fs::write(
            dir.join("plans/plan_abc.plan.md"),
            "---\nname: X\n---\n\nBody",
        )
        .expect("write plan file");
        let uri = "bitfun://runtime/workspace-1/plans/plan_abc.plan.md";
        let result = resolve_plan_path_with_plans_dir(uri, &dir.join("plans"), Some("workspace-1"));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            result.expect("runtime URI inside plans dir must resolve"),
            dir.join("plans/plan_abc.plan.md")
        );
    }

    #[test]
    fn resolve_plan_path_rejects_runtime_uri_with_scope_mismatch() {
        let error = resolve_plan_path_with_plans_dir(
            "bitfun://runtime/other-workspace/plans/plan_abc.plan.md",
            Path::new("C:/plans"),
            Some("current-workspace"),
        )
        .expect_err("runtime URI scope mismatch must error");
        assert!(
            error
                .to_string()
                .contains("belongs to workspace 'other-workspace'"),
            "unexpected error: {}",
            error
        );
    }

    #[test]
    fn parse_plan_file_handles_crlf_frontmatter() {
        // PLAN-05: the trailing '\r' before the closer must not break YAML.
        let content =
            "---\r\nname: My Plan\r\noverview: An overview\r\ntodos:\r\n- id: setup-auth\r\n  content: Set up auth\r\n  status: pending\r\n---\r\n\r\nBody text here.\r\n";
        let (frontmatter, body) = parse_plan_file(content).expect("parse CRLF plan file");
        assert_eq!(frontmatter.name.as_deref(), Some("My Plan"));
        assert_eq!(frontmatter.overview.as_deref(), Some("An overview"));
        assert_eq!(frontmatter.todos.len(), 1);
        assert_eq!(frontmatter.todos[0].id.as_deref(), Some("setup-auth"));
        assert_eq!(frontmatter.todos[0].status.as_deref(), Some("pending"));
        assert!(body.contains("Body text here."));
    }
}
