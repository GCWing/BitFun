use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use std::{fs, path::PathBuf};

/// Environment variable that points at the knowledge base root directory.
///
/// The knowledge base lives outside any workspace, so workspace-bound tools
/// (Grep, Glob) cannot reach it. The root is resolved from this environment
/// variable at call time (no machine-local path is hard-coded in the binary).
const KNOWLEDGE_BASE_ROOT_ENV: &str = "BITFUN_KNOWLEDGE_BASE_ROOT";

/// Files larger than this are skipped (in bytes).
const MAX_SCAN_FILE_SIZE: u64 = 2 * 1024 * 1024;

/// Deepest directory level the recursive scan descends to.
///
/// Symlink cycles and pathological nested layouts cannot be expressed with a
/// finite depth cap: the walk stops descending past this level.
///
/// Depth accounting (d6-P2-1): the entry directory (`root` for `scope=all`,
/// or the layer directory for a scoped search) is depth 0. `search_dir`
/// guards with `depth > MAX_SCAN_DEPTH`, so the scan reaches directories at
/// depth 0..=16 — i.e. the root plus up to 16 nested subdirectory levels
/// (17 levels including the root). Files directly inside the root are
/// scanned at depth 0.
const MAX_SCAN_DEPTH: usize = 16;

/// Hard cap on the number of files scanned in one call.
///
/// A single tool call must never scan an unbounded tree; once the cap is hit
/// the walk stops and reports `file_cap_reached` so the caller can narrow the
/// scope (keyword/scope/max_results) instead of silently truncating.
const MAX_SCANNED_FILES: usize = 100_000;

/// Default result cap.
const DEFAULT_MAX_RESULTS: usize = 50;

/// Hard cap for `max_results`.
const MAX_RESULTS_CAP: usize = 200;

/// Resolve the effective result cap for one search.
///
/// Runtime clamp semantics (L6-P2-2 / PLAN-3): `max_results` defaults to
/// `DEFAULT_MAX_RESULTS` and is clamped into `1..=MAX_RESULTS_CAP` so a
/// caller that bypasses `validate_input` (or passes an out-of-range value
/// through a non-schema path) can never request 0 results (which would return
/// an empty scan) or an unbounded result set. `validate_input` rejects
/// out-of-range values as a first line of defense; this clamp is the second,
/// in the execution path itself.
/// Resolve the effective result cap with configurable default/cap
/// (阈值参数配置化：`ai.thresholds.knowledge_search.*`).
fn resolve_max_results_with_cap(
    max_results: Option<usize>,
    default_max_results: usize,
    max_results_cap: usize,
) -> usize {
    let default_max_results = default_max_results.max(1);
    let max_results_cap = max_results_cap.max(default_max_results);
    max_results
        .unwrap_or(default_max_results)
        .clamp(1, max_results_cap)
}

/// Resolved knowledge-search scan thresholds
/// (阈值参数配置化：`ai.thresholds.knowledge_search.*`).
#[derive(Debug, Clone, Copy)]
struct ResolvedKnowledgeSearchThresholds {
    max_scan_file_bytes: u64,
    max_scan_depth: usize,
    default_max_results: usize,
    max_results_cap: usize,
}

/// Load the configured knowledge-search thresholds, falling back to the legacy
/// constants when the config service is unavailable or the value is unset.
async fn resolved_knowledge_search_thresholds() -> ResolvedKnowledgeSearchThresholds {
    let Ok(config_service) = crate::service::config::get_global_config_service().await else {
        return ResolvedKnowledgeSearchThresholds {
            max_scan_file_bytes: MAX_SCAN_FILE_SIZE,
            max_scan_depth: MAX_SCAN_DEPTH,
            default_max_results: DEFAULT_MAX_RESULTS,
            max_results_cap: MAX_RESULTS_CAP,
        };
    };
    let Ok(thresholds) = config_service
        .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
        .await
    else {
        return ResolvedKnowledgeSearchThresholds {
            max_scan_file_bytes: MAX_SCAN_FILE_SIZE,
            max_scan_depth: MAX_SCAN_DEPTH,
            default_max_results: DEFAULT_MAX_RESULTS,
            max_results_cap: MAX_RESULTS_CAP,
        };
    };
    let ks = &thresholds.knowledge_search;
    ResolvedKnowledgeSearchThresholds {
        max_scan_file_bytes: ks.max_scan_file_bytes.max(1),
        max_scan_depth: ks.max_scan_depth.max(1),
        default_max_results: ks.default_max_results.max(1),
        max_results_cap: ks.max_results_cap.max(ks.default_max_results.max(1)),
    }
}

/// KnowledgeBaseSearch tool - full-text search over the configured knowledge
/// base directory.
pub struct KnowledgeBaseSearchTool;

impl Default for KnowledgeBaseSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeBaseSearchTool {
    pub fn new() -> Self {
        Self
    }
}

/// A concrete knowledge base layer (L0/L1/L3/L4, deliberately no L2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnowledgeBaseLayer {
    L0,
    L1,
    L3,
    L4,
}

impl KnowledgeBaseLayer {
    fn as_str(self) -> &'static str {
        match self {
            KnowledgeBaseLayer::L0 => "L0",
            KnowledgeBaseLayer::L1 => "L1",
            KnowledgeBaseLayer::L3 => "L3",
            KnowledgeBaseLayer::L4 => "L4",
        }
    }
}

/// Resolved search scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnowledgeBaseScope {
    All,
    Layer(KnowledgeBaseLayer),
}

impl KnowledgeBaseScope {
    fn root_dir(self, root: &Path) -> PathBuf {
        match self {
            KnowledgeBaseScope::All => PathBuf::from(root),
            KnowledgeBaseScope::Layer(layer) => PathBuf::from(root).join(layer.as_str()),
        }
    }
}

/// Parses the user-facing `scope` string into a concrete search scope.
fn parse_scope(scope: &str) -> Result<KnowledgeBaseScope, String> {
    let scope = scope.trim();
    if scope.is_empty() || scope.eq_ignore_ascii_case("all") {
        return Ok(KnowledgeBaseScope::All);
    }
    match scope.to_ascii_uppercase().as_str() {
        "L0" => Ok(KnowledgeBaseScope::Layer(KnowledgeBaseLayer::L0)),
        "L1" => Ok(KnowledgeBaseScope::Layer(KnowledgeBaseLayer::L1)),
        "L3" => Ok(KnowledgeBaseScope::Layer(KnowledgeBaseLayer::L3)),
        "L4" => Ok(KnowledgeBaseScope::Layer(KnowledgeBaseLayer::L4)),
        other => Err(format!(
            "Unsupported scope '{}'. Expected one of: all, L0, L1, L3, L4 (the knowledge base has no L2 layer)",
            other
        )),
    }
}

/// Tracks what the scan saw so the caller can tell skipped content apart.
#[derive(Debug, Default)]
struct ScanStats {
    scanned_files: usize,
    skipped_binary: usize,
    skipped_oversized: usize,
    skipped_symlinks: usize,
    /// Set when the walk stopped because it hit a hard cap (MAX_SCAN_DEPTH or
    /// MAX_SCANNED_FILES): the scan did not fully cover the requested scope.
    file_cap_reached: bool,
}

/// Recursively searches `dir` for `keyword_lower`, appending matches to `results`.
///
/// `depth` guards against unbounded descent: the entry directory is depth 0
/// and the walk stops once `depth > MAX_SCAN_DEPTH` (i.e. 16 nested
/// subdirectory levels below the entry, 17 levels including it; d6-P2-1).
/// `fs::symlink_metadata` is used so symlinks are never followed — a link
/// pointing outside the knowledge base root can never escape the scan scope.
// Recursive scan carries depth/budget state explicitly per call site.
#[allow(clippy::too_many_arguments)]
fn search_dir(
    dir: &Path,
    keyword_lower: &str,
    max_results: usize,
    results: &mut Vec<Value>,
    stats: &mut ScanStats,
    depth: usize,
    max_scan_depth: usize,
    max_scan_file_bytes: u64,
) {
    if results.len() >= max_results {
        return;
    }
    if depth > max_scan_depth || stats.scanned_files >= MAX_SCANNED_FILES {
        stats.file_cap_reached = true;
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    let mut paths = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    // Deterministic order across runs.
    paths.sort();

    for path in paths {
        if results.len() >= max_results {
            break;
        }
        if stats.scanned_files >= MAX_SCANNED_FILES {
            stats.file_cap_reached = true;
            break;
        }
        let Some(file_name) = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        // symlink_metadata does not follow links: a symlink to a directory is
        // reported as a symlink, never traversed.
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let file_type = meta.file_type();
        if file_type.is_symlink() {
            stats.skipped_symlinks += 1;
            continue;
        }
        if file_type.is_dir() {
            if file_name.starts_with('.') {
                // Skip hidden directories (e.g. .git).
                continue;
            }
            search_dir(
                &path,
                keyword_lower,
                max_results,
                results,
                stats,
                depth + 1,
                max_scan_depth,
                max_scan_file_bytes,
            );
        } else if file_type.is_file() {
            scan_file(
                &path,
                keyword_lower,
                max_results,
                results,
                stats,
                max_scan_file_bytes,
            );
        }
        // Special files are skipped.
    }
}

/// Scans one text file for `keyword_lower`, appending matches to `results`.
fn scan_file(
    path: &Path,
    keyword_lower: &str,
    max_results: usize,
    results: &mut Vec<Value>,
    stats: &mut ScanStats,
    max_scan_file_bytes: u64,
) {
    if results.len() >= max_results {
        return;
    }
    if stats.scanned_files >= MAX_SCANNED_FILES {
        stats.file_cap_reached = true;
        return;
    }
    // symlink_metadata: callers already skip symlinks, but a file that became a
    // symlink between the directory read and this call must not be followed.
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(_) => return,
    };
    if meta.file_type().is_symlink() {
        stats.skipped_symlinks += 1;
        return;
    }
    if meta.len() > max_scan_file_bytes {
        stats.skipped_oversized += 1;
        return;
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };
    // Heuristic binary detection: a NUL byte in the head of the file.
    let head_len = bytes.len().min(8192);
    if bytes[..head_len].contains(&0) {
        stats.skipped_binary += 1;
        return;
    }
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            stats.skipped_binary += 1;
            return;
        }
    };
    stats.scanned_files += 1;
    for (index, line) in text.lines().enumerate() {
        if results.len() >= max_results {
            break;
        }
        if line.to_lowercase().contains(keyword_lower) {
            results.push(json!({
                "path": path.to_string_lossy(),
                "line": index + 1,
                "line_content": line,
            }));
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct KnowledgeBaseSearchInput {
    keyword: String,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    max_results: Option<usize>,
}

#[async_trait]
impl Tool for KnowledgeBaseSearchTool {
    fn name(&self) -> &str {
        "KnowledgeBaseSearch"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Use this tool when you need to search a local knowledge base directory for skills, rules, and accumulated lessons.

The knowledge base root is resolved from the `BITFUN_KNOWLEDGE_BASE_ROOT` environment variable. When it is not configured, the tool reports a clear configuration error instead of scanning anything.

This tool is strictly read-only: it never deletes, overwrites, or modifies anything under the knowledge base root. It recursively walks the requested scope, scans UTF-8 text files for the keyword (case-insensitive), and returns every matching line.

`keyword` (required): the text to search for, matched case-insensitively against file contents.

`scope` (defaults to "all"):
- "all": the whole knowledge base root
- "L0": the top-level layer (chronicles, identities, etc.)
- "L1": skills / rules / tooling library
- "L3": refined prompts and knowledge layers
- "L4": archived or supplementary layers
Note: the knowledge base has L0/L1/L3/L4 and deliberately no L2 layer.

`max_results` (defaults to 50, capped at 200): maximum number of matching lines to return.

Non-text files, binary files, files larger than 2MB, hidden directories (e.g. .git), and symlinks are skipped; the walk starts at the scope root (depth 0) and stops after 16 nested directory levels below it (depth > 16), or after 100k scanned files. The result includes `scanned_files`, `skipped_binary`, `skipped_oversized`, `skipped_symlinks`, and `file_cap_reached` counters so you can tell what was and was not searched.

Each match has the shape {path, line, line_content}, where `line` is the 1-based line number.

Examples:
1. Search the whole knowledge base for "S-31": keyword="S-31"
2. Search only the skills layer for "from-zero": keyword="from-zero", scope="L1"
3. Search the top layer with a tight cap: keyword="search", scope="L0", max_results=20"#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Search the configured local knowledge base (L0/L1/L3/L4) by keyword. Strictly read-only."
            .to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        // Mirrors the plan tool family calibration: read-only staples stay
        // Direct so no GetToolSpec unlock round-trip is needed.
        ToolExposure::Direct
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "keyword": {
                    "type": "string",
                    "description": "Keyword to search for, matched case-insensitively against file contents. Required."
                },
                "scope": {
                    "type": "string",
                    "description": "Search scope. One of: all (default), L0, L1, L3, L4."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matching lines to return. Defaults to 50, capped at 200."
                }
            },
            "required": ["keyword"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: KnowledgeBaseSearchInput = match serde_json::from_value(input.clone()) {
            Ok(value) => value,
            Err(err) => {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid input: {}", err)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        if parsed.keyword.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("keyword must be a non-empty string".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if let Some(scope) = parsed.scope.as_deref() {
            if let Err(message) = parse_scope(scope) {
                return ValidationResult {
                    result: false,
                    message: Some(message),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        if let Some(max_results) = parsed.max_results {
            let cap = resolved_knowledge_search_thresholds().await.max_results_cap;
            if !(1..=cap).contains(&max_results) {
                return ValidationResult {
                    result: false,
                    message: Some(format!("max_results must be between 1 and {}", cap)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let keyword = input
            .get("keyword")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let scope = input
            .get("scope")
            .and_then(|value| value.as_str())
            .unwrap_or("all");
        format!(
            "Search knowledge base for '{}' (scope '{}')",
            keyword, scope
        )
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: KnowledgeBaseSearchInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;

        let keyword = params.keyword.trim();
        if keyword.is_empty() {
            return Err(BitFunError::tool("keyword must not be empty"));
        }
        let scope = params.scope.as_deref().unwrap_or("all");
        let resolved = parse_scope(scope)
            .map_err(|message| BitFunError::tool(format!("Invalid scope: {}", message)))?;
        // 阈值参数配置化：ai.thresholds.knowledge_search.*
        let search_thresholds = resolved_knowledge_search_thresholds().await;
        let max_results = resolve_max_results_with_cap(
            params.max_results,
            search_thresholds.default_max_results,
            search_thresholds.max_results_cap,
        );

        let Some(root_value) = std::env::var_os(KNOWLEDGE_BASE_ROOT_ENV) else {
            return Err(BitFunError::tool(format!(
                "{} is not configured; set it to the knowledge base root directory before using this tool",
                KNOWLEDGE_BASE_ROOT_ENV
            )));
        };
        let root = resolved.root_dir(Path::new(&root_value));
        if !root.is_dir() {
            return Err(BitFunError::tool(format!(
                "Knowledge base root does not exist: {}",
                root.to_string_lossy()
            )));
        }

        let keyword_lower = keyword.to_lowercase();
        // 阈值参数配置化：ai.thresholds.knowledge_search.max_scan_depth / max_scan_file_bytes
        let scan_depth = search_thresholds.max_scan_depth.max(1);
        let scan_file_bytes = search_thresholds.max_scan_file_bytes.max(1);
        // The recursive scan is CPU/IO-bound and unbounded in the worst case
        // (the whole knowledge base). Run it on the blocking pool so a large
        // scan never stalls the async executor, and return the capped
        // results/stats instead of mutating shared state across the await.
        let (results, stats) = tokio::task::spawn_blocking(move || {
            let mut results = Vec::new();
            let mut stats = ScanStats::default();
            search_dir(
                &root,
                &keyword_lower,
                max_results,
                &mut results,
                &mut stats,
                0,
                scan_depth,
                scan_file_bytes,
            );
            (results, stats)
        })
        .await
        .map_err(|e| BitFunError::tool(format!("Knowledge base search worker failed: {}", e)))?;

        Ok(vec![ToolResult::Result {
            data: json!({
                "success": true,
                "scope": scope,
                "keyword": keyword,
                "count": results.len(),
                "scanned_files": stats.scanned_files,
                "skipped_binary": stats.skipped_binary,
                "skipped_oversized": stats.skipped_oversized,
                "skipped_symlinks": stats.skipped_symlinks,
                "file_cap_reached": stats.file_cap_reached,
                "matches": results,
            }),
            result_for_assistant: Some(format!(
                "Searched the knowledge base with scope '{}': {} match(es) across {} scanned file(s){}.",
                scope,
                results.len(),
                stats.scanned_files,
                if stats.file_cap_reached {
                    " (file cap reached; narrow the scope or keyword to scan more)"
                } else {
                    ""
                }
            )),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scope_accepts_default_and_known_scopes() {
        assert_eq!(parse_scope(""), Ok(KnowledgeBaseScope::All));
        assert_eq!(parse_scope("all"), Ok(KnowledgeBaseScope::All));
        assert_eq!(parse_scope("ALL"), Ok(KnowledgeBaseScope::All));
        assert_eq!(
            parse_scope("L0"),
            Ok(KnowledgeBaseScope::Layer(KnowledgeBaseLayer::L0))
        );
        assert_eq!(
            parse_scope("l1"),
            Ok(KnowledgeBaseScope::Layer(KnowledgeBaseLayer::L1))
        );
        assert_eq!(
            parse_scope("L3"),
            Ok(KnowledgeBaseScope::Layer(KnowledgeBaseLayer::L3))
        );
        assert_eq!(
            parse_scope("L4"),
            Ok(KnowledgeBaseScope::Layer(KnowledgeBaseLayer::L4))
        );
    }

    #[test]
    fn parse_scope_rejects_unknown_scopes() {
        assert!(parse_scope("unknown").is_err());
        // The knowledge base has L0/L1/L3/L4 and deliberately no L2 layer.
        assert!(parse_scope("L2").is_err());
        assert!(parse_scope("l2").is_err());
        assert!(parse_scope("by_status:all").is_err());
    }

    #[test]
    fn resolve_max_results_clamps_into_1_200() {
        // 未提供 → 默认 50
        assert_eq!(
            resolve_max_results_with_cap(None, DEFAULT_MAX_RESULTS, MAX_RESULTS_CAP),
            DEFAULT_MAX_RESULTS
        );
        // 合法范围原样
        assert_eq!(
            resolve_max_results_with_cap(Some(1), DEFAULT_MAX_RESULTS, MAX_RESULTS_CAP),
            1
        );
        assert_eq!(
            resolve_max_results_with_cap(Some(200), DEFAULT_MAX_RESULTS, MAX_RESULTS_CAP),
            200
        );
        assert_eq!(
            resolve_max_results_with_cap(Some(42), DEFAULT_MAX_RESULTS, MAX_RESULTS_CAP),
            42
        );
        // 下限 clamp：0 / 越界负值（绕过 validate 的非 schema 路径）→ 1
        assert_eq!(
            resolve_max_results_with_cap(Some(0), DEFAULT_MAX_RESULTS, MAX_RESULTS_CAP),
            1
        );
        // 上限 clamp：>200 → 200（运行时护栏，防无界结果集）
        assert_eq!(
            resolve_max_results_with_cap(
                Some(MAX_RESULTS_CAP + 1),
                DEFAULT_MAX_RESULTS,
                MAX_RESULTS_CAP
            ),
            MAX_RESULTS_CAP
        );
        assert_eq!(
            resolve_max_results_with_cap(Some(10_000), DEFAULT_MAX_RESULTS, MAX_RESULTS_CAP),
            MAX_RESULTS_CAP
        );
    }

    #[tokio::test]
    async fn validate_rejects_missing_or_empty_keyword() {
        let tool = KnowledgeBaseSearchTool::new();

        let validation = tool.validate_input(&json!({}), None).await;
        assert!(!validation.result);
        assert_eq!(validation.error_code, Some(400));

        let validation = tool.validate_input(&json!({ "keyword": "  " }), None).await;
        assert!(!validation.result);
        assert_eq!(validation.error_code, Some(400));
    }

    #[tokio::test]
    async fn validate_rejects_unknown_scope() {
        let tool = KnowledgeBaseSearchTool::new();

        let validation = tool
            .validate_input(&json!({ "keyword": "search", "scope": "L2" }), None)
            .await;

        assert!(!validation.result);
        assert_eq!(validation.error_code, Some(400));
    }

    #[tokio::test]
    async fn validate_rejects_excessive_max_results() {
        let tool = KnowledgeBaseSearchTool::new();

        let validation = tool
            .validate_input(&json!({ "keyword": "search", "max_results": 201 }), None)
            .await;

        assert!(!validation.result);
        assert_eq!(validation.error_code, Some(400));
    }

    #[tokio::test]
    async fn validate_accepts_valid_input() {
        let tool = KnowledgeBaseSearchTool::new();

        let validation = tool
            .validate_input(
                &json!({ "keyword": "search", "scope": "L0", "max_results": 10 }),
                None,
            )
            .await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[cfg(unix)]
    fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[test]
    fn search_dir_skips_symlinks_outside_root() {
        // A symlink pointing outside the knowledge base root must never be
        // followed. Symlink creation needs privileges on Windows, so the
        // assertion is skipped when the OS refuses to create the link.
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(&root).expect("create root");
        std::fs::write(root.join("a.txt"), "keyword to find\n").expect("write file");

        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&outside).expect("create outside dir");
        std::fs::write(outside.join("secret.txt"), "secret content\n").expect("write secret");
        let link = root.join("link-to-outside");
        if make_symlink(&outside, &link).is_ok() {
            let mut results = Vec::new();
            let mut stats = ScanStats::default();
            search_dir(
                &root,
                "secret",
                50,
                &mut results,
                &mut stats,
                0,
                MAX_SCAN_DEPTH,
                MAX_SCAN_FILE_SIZE,
            );
            assert!(
                results
                    .iter()
                    .all(|result| !result["path"].to_string().contains("secret")),
                "files reached through a symlink must not be searched"
            );
            assert_eq!(stats.skipped_symlinks, 1);
        }
    }

    #[test]
    fn search_dir_stops_at_depth_cap() {
        // The walk must not descend past MAX_SCAN_DEPTH, so a deeply nested
        // layout cannot blow up the scan.
        let temp = tempfile::tempdir().expect("tempdir");
        let mut dir = temp.path().join("root");
        std::fs::create_dir_all(&dir).expect("create root");
        for _ in 0..MAX_SCAN_DEPTH + 1 {
            dir = dir.join("nested");
        }
        std::fs::create_dir_all(&dir).expect("create nested chain");
        std::fs::write(dir.join("deep.txt"), "deep keyword here\n").expect("write deep file");

        let mut results = Vec::new();
        let mut stats = ScanStats::default();
        search_dir(
            &temp.path().join("root"),
            "deep",
            50,
            &mut results,
            &mut stats,
            0,
            MAX_SCAN_DEPTH,
            MAX_SCAN_FILE_SIZE,
        );
        assert!(stats.file_cap_reached);
        assert!(
            results
                .iter()
                .all(|result| !result["path"].to_string().contains("deep")),
            "files deeper than MAX_SCAN_DEPTH must not be searched"
        );
    }
}
