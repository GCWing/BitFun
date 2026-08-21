//! rg（ripgrep 库）交叉校验与降级实现。
//!
//! 根因背景（RECON-卡搜索根因彻查-20260809）：flashgrep daemon（闭源）overlay
//! 路径匹配 bug 可能在任意相位/scope 组合下返回 `Ok(空)` 假空结果，工具层的
//! phase/scope/candidate_docs 三维判据只能枚举已见形态。本模块在 service 层
//! 用 rg 库引擎对空结果做结果实证交叉校验：
//!   - flashgrep 空 + rg 非空 = 假空 = 信任 rg 结果（本模块产出降级结果）；
//!   - flashgrep 空 + rg 空 = 真实无结果；
//!   - flashgrep 非空 = 不触发本模块。
//!
//! 与工具层 grep_tool.rs 的三维判据互不替代：工具层判据保留为第一道防线，
//! 本模块兜住「判据枚举之外的新形态假空」。

use std::path::{Path, PathBuf};

use bitfun_services_core::filesystem::{FileSearchOutcome, FileSearchResult, SearchMatchType};
use globset::{Glob, GlobSet, GlobSetBuilder};
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use ignore::types::TypesBuilder;
use ignore::WalkBuilder;

use super::types::{
    ContentSearchResult, WorkspaceSearchBackend, WorkspaceSearchFileCount,
    WorkspaceSearchRepoStatus,
};

/// rg 交叉校验所需的请求快照（在 search_content 中 pattern/globs 等被 move 前进项）。
#[derive(Debug, Clone)]
pub(crate) struct RgValidationRequest {
    /// 搜索根：子路径 scope 时为 search_path，否则为仓库根。
    pub search_root: PathBuf,
    pub pattern: String,
    pub case_insensitive: bool,
    pub multiline: bool,
    pub whole_word: bool,
    /// 等价于 `!use_regex`：字面串匹配。
    pub fixed_strings: bool,
    pub globs: Vec<String>,
    pub file_types: Vec<String>,
    pub exclude_file_types: Vec<String>,
}

/// 交叉校验/降级搜索的文件数预算：只遍历 scope 内前 N 个文件。
/// 大仓库中假空是小概率事件，限制预算避免空结果路径（真无结果）被拖慢。
pub(crate) const RG_VALIDATION_FILE_BUDGET: usize = 200;

/// 与 tool-execution grep_search 对齐的 VCS 目录排除表。
const VCS_DIRECTORIES_TO_EXCLUDE: &[&str] = &[".git", ".svn", ".hg", ".bzr", ".jj", ".sl"];

/// 判断 service 层搜索结果是否为「空」（可能为 daemon 假空的候选）。
///
/// 覆盖全部 output_mode 的空形态：转换后结果为空 + 无文件计数 +
/// daemon 自报 matched_lines/matched_occurrences 均为 0。
pub(crate) fn search_result_is_empty(result: &ContentSearchResult) -> bool {
    result.outcome.results.is_empty()
        && result.file_counts.is_empty()
        && result.matched_lines == 0
        && result.matched_occurrences == 0
}

/// rg 搜索的单条行命中。
#[derive(Debug, Clone)]
pub(crate) struct RgLineMatch {
    pub path: String,
    pub line_number: usize,
    pub line_text: String,
}

/// rg 搜索的结构化产出。
#[derive(Debug, Default)]
pub(crate) struct RgSearchOutcome {
    /// 命中行（含行号与行文本），按文件遍历顺序追加。
    pub line_matches: Vec<RgLineMatch>,
    /// 有命中的文件（去重，遍历顺序）。
    pub files: Vec<String>,
    /// 每个文件的命中行数（与 files 对齐路径）。
    pub file_counts: Vec<WorkspaceSearchFileCount>,
    /// 遍历到的文件总数（用于预算截断判断）。
    pub files_walked: usize,
}

impl RgSearchOutcome {
    pub(crate) fn total_matches(&self) -> usize {
        self.line_matches.len()
    }

    /// 转换为 service 层 ContentSearchResult（保留 daemon repo_status，backend 标 TextFallback）。
    pub(crate) fn into_content_search_result(
        self,
        repo_status: WorkspaceSearchRepoStatus,
    ) -> ContentSearchResult {
        let matched_lines = self.line_matches.len();
        let results: Vec<FileSearchResult> = self
            .line_matches
            .iter()
            .map(|matched| FileSearchResult {
                path: matched.path.clone(),
                name: Path::new(&matched.path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&matched.path)
                    .to_string(),
                is_directory: false,
                match_type: SearchMatchType::Content,
                line_number: Some(matched.line_number),
                matched_content: Some(matched.line_text.clone()),
                preview_before: None,
                preview_inside: Some(matched.line_text.clone()),
                preview_after: None,
            })
            .collect();
        let candidate_docs = self.files.len();
        ContentSearchResult {
            outcome: FileSearchOutcome {
                results,
                truncated: false,
            },
            file_counts: self.file_counts,
            hits: Vec::new(),
            backend: WorkspaceSearchBackend::TextFallback,
            repo_status,
            candidate_docs,
            matched_lines,
            matched_occurrences: matched_lines,
        }
    }
}

/// 用 rg 库引擎执行与 flashgrep 请求等价的搜索。
///
/// 返回：
/// - `Ok(Some(outcome))`：搜索完成（遍历在预算内完成，或预算耗尽前已发现命中），
///   `outcome` 为可信结果；
/// - `Ok(None)`：预算耗尽且未发现任何命中——无法区分「真无结果」与「命中在未
///   遍历到的文件中」，调用方应保守保留 daemon 原结果；
/// - `Err`：请求无法转化为 rg 搜索（无效正则/路径不存在等），调用方应保留
///   daemon 原结果。
pub(crate) fn rg_search(
    request: &RgValidationRequest,
    file_budget: usize,
) -> Result<Option<RgSearchOutcome>, String> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(request.case_insensitive)
        .multi_line(request.multiline)
        .dot_matches_new_line(request.multiline)
        .word(request.whole_word)
        .fixed_strings(request.fixed_strings)
        .build(&request.pattern)
        .map_err(|error| format!("rg cross-validation failed to build matcher: {error}"))?;

    let search_root = request.search_root.clone();
    if !search_root.exists() {
        return Err(format!(
            "rg cross-validation search root does not exist: {}",
            search_root.display()
        ));
    }

    let glob_set = build_glob_set(&request.globs)?;
    let types = build_types(&request.file_types, &request.exclude_file_types)?;

    let mut outcome = RgSearchOutcome::default();
    let mut walker = WalkBuilder::new(&search_root);
    walker
        .hidden(false)
        .ignore(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true);
    if let Some(types) = types {
        walker.types(types);
    }

    for entry in walker.build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        if is_vcs_path(path) {
            continue;
        }
        if let Some(glob_set) = &glob_set {
            if !glob_set.is_match(path) {
                continue;
            }
        }

        outcome.files_walked += 1;
        if outcome.files_walked > file_budget {
            if outcome.total_matches() > 0 {
                // 已有命中：结果足以判定假空，直接返回（truncated 语义由调用方按
                // 全量有命中处理，预算截断不影响「非空」结论）。
                return Ok(Some(outcome));
            }
            return Ok(None);
        }

        search_file(&matcher, path, &search_root, &mut outcome);
    }

    Ok(Some(outcome))
}

/// 用 grep-searcher 搜索单文件，命中行追加进 outcome。
/// 读文件/搜索错误静默跳过（二进制/编码异常文件不应中断整体校验）。
fn search_file(
    matcher: &grep_regex::RegexMatcher,
    path: &Path,
    search_root: &Path,
    outcome: &mut RgSearchOutcome,
) {
    use grep_searcher::{Sink, SinkMatch};

    struct CollectSink<'a> {
        path_display: &'a str,
        outcome: &'a mut RgSearchOutcome,
        file_matched_lines: usize,
    }

    impl Sink for CollectSink<'_> {
        type Error = std::io::Error;

        fn matched(
            &mut self,
            _searcher: &grep_searcher::Searcher,
            mat: &SinkMatch<'_>,
        ) -> Result<bool, Self::Error> {
            let line_number = mat.line_number().unwrap_or(0) as usize;
            let line_text = String::from_utf8_lossy(mat.bytes()).trim_end().to_string();
            self.outcome.line_matches.push(RgLineMatch {
                path: self.path_display.to_string(),
                line_number,
                line_text,
            });
            self.file_matched_lines += 1;
            Ok(true)
        }
    }

    let path_display = display_path(path, search_root);
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .build();
    let search_ok = {
        let mut sink = CollectSink {
            path_display: &path_display,
            outcome: &mut *outcome,
            file_matched_lines: 0,
        };
        let ok = searcher.search_path(matcher, path, &mut sink).is_ok();
        (ok, sink.file_matched_lines)
    };
    let (search_ok, file_matched_lines) = search_ok;
    if !search_ok {
        return;
    }
    if file_matched_lines > 0 {
        outcome.files.push(path_display.clone());
        outcome.file_counts.push(WorkspaceSearchFileCount {
            path: path_display,
            matched_lines: file_matched_lines,
        });
    }
}

/// 结果路径展示：相对 search_root 用正斜杠相对路径，否则用绝对路径。
/// 与 flashgrep 结果（仓库相对路径）在「仓库根 scope」下形态一致。
fn display_path(path: &Path, search_root: &Path) -> String {
    path.strip_prefix(search_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_vcs_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component,
            std::path::Component::Normal(name)
                if VCS_DIRECTORIES_TO_EXCLUDE
                    .iter()
                    .any(|excluded| name.to_string_lossy() == *excluded)
        )
    })
}

/// 将 request.globs 编译为 GlobSet；空 globs 返回 None（不过滤）。
fn build_glob_set(globs: &[String]) -> Result<Option<GlobSet>, String> {
    if globs.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in globs {
        let glob = Glob::new(pattern)
            .map_err(|error| format!("rg cross-validation invalid glob '{pattern}': {error}"))?;
        builder.add(glob);
    }
    builder
        .build()
        .map(Some)
        .map_err(|error| format!("rg cross-validation failed to build glob set: {error}"))
}

/// 将 request.file_types / exclude_file_types 编译为 ignore Types。
/// 两者皆空返回 None（walker 不按类型过滤）。
fn build_types(
    file_types: &[String],
    exclude_file_types: &[String],
) -> Result<Option<ignore::types::Types>, String> {
    if file_types.is_empty() && exclude_file_types.is_empty() {
        return Ok(None);
    }
    let mut builder = TypesBuilder::new();
    builder.add_defaults();
    for name in file_types {
        ensure_type(&mut builder, name)?;
        builder.select(name);
    }
    for name in exclude_file_types {
        ensure_type(&mut builder, name)?;
        builder.negate(name);
    }
    builder
        .build()
        .map(Some)
        .map_err(|error| format!("rg cross-validation failed to build file types: {error}"))
}

/// 未知类型名按 `*.{name}` 兜底注册（与 tool-execution grep_search 对齐）。
fn ensure_type(builder: &mut TypesBuilder, name: &str) -> Result<(), String> {
    let exists = builder.definitions().iter().any(|def| def.name() == name);
    if !exists {
        builder.add(name, &format!("*.{name}")).map_err(|error| {
            format!("rg cross-validation failed to add file type '{name}': {error}")
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_file(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, content).expect("write test file");
    }

    fn test_request(root: &Path, pattern: &str) -> RgValidationRequest {
        RgValidationRequest {
            search_root: root.to_path_buf(),
            pattern: pattern.to_string(),
            case_insensitive: false,
            multiline: false,
            whole_word: false,
            fixed_strings: false,
            globs: Vec::new(),
            file_types: Vec::new(),
            exclude_file_types: Vec::new(),
        }
    }

    #[test]
    fn rg_search_finds_matches_flashgrep_missed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        write_file(
            root,
            "src/lib.rs",
            "fn main() {\n    hello_target_symbol();\n}\n",
        );
        write_file(root, "docs/readme.md", "no match here\n");

        let request = test_request(root, "hello_target_symbol");
        let outcome = rg_search(&request, RG_VALIDATION_FILE_BUDGET)
            .expect("rg search ok")
            .expect("within budget");

        assert_eq!(outcome.total_matches(), 1);
        assert_eq!(outcome.files.len(), 1);
        assert!(outcome.line_matches[0].path.ends_with("src/lib.rs"));
        assert_eq!(outcome.line_matches[0].line_number, 2);
        assert!(outcome.line_matches[0]
            .line_text
            .contains("hello_target_symbol"));
        assert_eq!(outcome.file_counts[0].matched_lines, 1);
    }

    #[test]
    fn rg_search_confirms_true_empty() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        write_file(root, "src/lib.rs", "fn main() {}\n");
        write_file(root, "docs/readme.md", "nothing\n");

        let request = test_request(root, "definitely_absent_symbol_xyz");
        let outcome = rg_search(&request, RG_VALIDATION_FILE_BUDGET)
            .expect("rg search ok")
            .expect("within budget");

        assert_eq!(outcome.total_matches(), 0);
        assert!(outcome.files.is_empty());
    }

    #[test]
    fn rg_search_respects_search_path_scope() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        write_file(root, "inside/hit.rs", "target_symbol\n");
        write_file(root, "outside/hit.rs", "target_symbol\n");

        let mut request = test_request(root, "target_symbol");
        request.search_root = root.join("inside");
        let outcome = rg_search(&request, RG_VALIDATION_FILE_BUDGET)
            .expect("rg search ok")
            .expect("within budget");

        // 命中仅 1 处（outside 被 scope 排除），且路径相对 search_root（inside）。
        assert_eq!(outcome.total_matches(), 1);
        assert_eq!(outcome.line_matches[0].path, "hit.rs");
    }

    #[test]
    fn rg_search_respects_globs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        write_file(root, "src/lib.rs", "target_symbol\n");
        write_file(root, "src/lib.md", "target_symbol\n");

        let mut request = test_request(root, "target_symbol");
        request.globs = vec!["*.rs".to_string()];
        let outcome = rg_search(&request, RG_VALIDATION_FILE_BUDGET)
            .expect("rg search ok")
            .expect("within budget");

        assert_eq!(outcome.total_matches(), 1);
        assert!(outcome.line_matches[0].path.ends_with(".rs"));
    }

    #[test]
    fn rg_search_respects_file_types() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        write_file(root, "src/lib.rs", "target_symbol\n");
        write_file(root, "src/lib.py", "target_symbol\n");

        let mut request = test_request(root, "target_symbol");
        request.file_types = vec!["rust".to_string()];
        let outcome = rg_search(&request, RG_VALIDATION_FILE_BUDGET)
            .expect("rg search ok")
            .expect("within budget");

        assert_eq!(outcome.total_matches(), 1);
        assert!(outcome.line_matches[0].path.ends_with(".rs"));
    }

    #[test]
    fn rg_search_excludes_vcs_directories() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        write_file(root, ".git/objects/packed", "target_symbol\n");
        write_file(root, "src/lib.rs", "fn main() {}\n");

        let request = test_request(root, "target_symbol");
        let outcome = rg_search(&request, RG_VALIDATION_FILE_BUDGET)
            .expect("rg search ok")
            .expect("within budget");

        assert_eq!(outcome.total_matches(), 0);
    }

    #[test]
    fn rg_search_fixed_strings_mode() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        write_file(root, "a.txt", "literal (with) [regex] chars\n");

        let mut request = test_request(root, "(with) [regex]");
        request.fixed_strings = true;
        let outcome = rg_search(&request, RG_VALIDATION_FILE_BUDGET)
            .expect("rg search ok")
            .expect("within budget");

        assert_eq!(outcome.total_matches(), 1);
    }

    #[test]
    fn rg_search_invalid_regex_returns_err() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        write_file(root, "a.txt", "content\n");

        let request = test_request(root, "(unclosed");
        let result = rg_search(&request, RG_VALIDATION_FILE_BUDGET);
        assert!(result.is_err());
    }

    #[test]
    fn rg_search_budget_exhausted_without_match_returns_none() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        for index in 0..5 {
            write_file(root, &format!("f{index}.txt"), "nothing\n");
        }

        let request = test_request(root, "absent_symbol");
        let result = rg_search(&request, 3).expect("rg search ok");
        assert!(result.is_none(), "budget exhausted without match => None");
    }

    #[test]
    fn rg_search_budget_exhausted_with_match_returns_partial() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        // a.txt 按字典序先被遍历并命中；预算 1 保证命中后再遍历即超预算。
        write_file(root, "a.txt", "target_symbol\n");
        for index in 0..5 {
            write_file(root, &format!("z{index}.txt"), "nothing\n");
        }

        let request = test_request(root, "target_symbol");
        let outcome = rg_search(&request, 1)
            .expect("rg search ok")
            .expect("match before budget exhaustion");

        assert_eq!(outcome.total_matches(), 1);
    }

    #[test]
    fn empty_detection_covers_all_output_modes() {
        use crate::workspace_search::types::{
            WorkspaceSearchBackend, WorkspaceSearchDirtyFiles, WorkspaceSearchRepoPhase,
        };

        fn repo_status() -> WorkspaceSearchRepoStatus {
            WorkspaceSearchRepoStatus {
                repo_id: String::new(),
                repo_path: String::new(),
                storage_root: String::new(),
                base_snapshot_root: String::new(),
                workspace_overlay_root: String::new(),
                phase: WorkspaceSearchRepoPhase::Ready,
                snapshot_key: None,
                base_head_commit: None,
                workspace_head_commit: None,
                base_advance_in_progress: false,
                base_advance_target_head: None,
                base_delta_depth: 0,
                base_compaction_recommended: false,
                last_probe_unix_secs: None,
                last_rebuild_unix_secs: None,
                dirty_files: WorkspaceSearchDirtyFiles {
                    modified: 0,
                    deleted: 0,
                    new: 0,
                },
                active_task_id: None,
                probe_healthy: true,
                workspace_probe_pending: false,
                last_error: None,
                last_maintenance_error: None,
                overlay: None,
            }
        }

        fn result(
            results: Vec<FileSearchResult>,
            matched_lines: usize,
            matched_occurrences: usize,
        ) -> ContentSearchResult {
            ContentSearchResult {
                outcome: FileSearchOutcome {
                    results,
                    truncated: false,
                },
                file_counts: Vec::new(),
                hits: Vec::new(),
                backend: WorkspaceSearchBackend::Indexed,
                repo_status: repo_status(),
                candidate_docs: 10,
                matched_lines,
                matched_occurrences,
            }
        }

        // 全零 = 空（假空候选）。
        assert!(search_result_is_empty(&result(Vec::new(), 0, 0)));

        // daemon 自报计数非零 = 非空（scan fallback 计数形态）。
        assert!(!search_result_is_empty(&result(Vec::new(), 3, 3)));

        // 有结果行 = 非空。
        let hit = FileSearchResult {
            path: "a.rs".to_string(),
            name: "a.rs".to_string(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: Some(1),
            matched_content: Some("x".to_string()),
            preview_before: None,
            preview_inside: None,
            preview_after: None,
        };
        assert!(!search_result_is_empty(&result(vec![hit], 0, 0)));
    }

    #[test]
    fn rg_outcome_converts_to_content_search_result() {
        use crate::workspace_search::types::{
            WorkspaceSearchDirtyFiles, WorkspaceSearchRepoPhase, WorkspaceSearchRepoStatus,
        };

        let outcome = RgSearchOutcome {
            line_matches: vec![RgLineMatch {
                path: "src/lib.rs".to_string(),
                line_number: 7,
                line_text: "let x = target;".to_string(),
            }],
            files: vec!["src/lib.rs".to_string()],
            file_counts: vec![WorkspaceSearchFileCount {
                path: "src/lib.rs".to_string(),
                matched_lines: 1,
            }],
            files_walked: 3,
        };
        let status = WorkspaceSearchRepoStatus {
            repo_id: "r".to_string(),
            repo_path: "p".to_string(),
            storage_root: String::new(),
            base_snapshot_root: String::new(),
            workspace_overlay_root: String::new(),
            phase: WorkspaceSearchRepoPhase::Ready,
            snapshot_key: None,
            base_head_commit: None,
            workspace_head_commit: None,
            base_advance_in_progress: false,
            base_advance_target_head: None,
            base_delta_depth: 0,
            base_compaction_recommended: false,
            last_probe_unix_secs: None,
            last_rebuild_unix_secs: None,
            dirty_files: WorkspaceSearchDirtyFiles {
                modified: 0,
                deleted: 0,
                new: 0,
            },
            active_task_id: None,
            probe_healthy: true,
            workspace_probe_pending: false,
            last_error: None,
            last_maintenance_error: None,
            overlay: None,
        };

        let converted = outcome.into_content_search_result(status);

        assert_eq!(converted.backend, WorkspaceSearchBackend::TextFallback);
        assert_eq!(converted.matched_lines, 1);
        assert_eq!(converted.candidate_docs, 1);
        assert_eq!(converted.outcome.results.len(), 1);
        assert_eq!(converted.outcome.results[0].path, "src/lib.rs");
        assert_eq!(converted.outcome.results[0].line_number, Some(7));
        assert_eq!(converted.repo_status.phase, WorkspaceSearchRepoPhase::Ready);
    }
}
