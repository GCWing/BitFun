use super::workspace_walk::{add_directory_ignores, WorkspaceFileWalker};
use crate::util::string::shell_single_quote;
use bitfun_runtime_ports::{WorkspaceFileSystem, WorkspacePathKind};
use globset::GlobBuilder;
use ignore::WalkBuilder;
use log::{info, warn};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalGlobRequest {
    pub search_path: PathBuf,
    pub pattern: String,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalGlobResult {
    pub matches: Vec<PathBuf>,
    pub walk_root: PathBuf,
    pub total_matches: Option<usize>,
    pub truncated: bool,
}

pub fn extract_glob_base_directory(pattern: &str) -> (String, String) {
    let glob_start = pattern.find(['*', '?', '[', '{']);

    #[cfg(not(windows))]
    if pattern[..glob_start.unwrap_or(pattern.len())].contains('\\') {
        return (String::new(), pattern.to_string());
    }

    match glob_start {
        Some(index) => {
            let static_prefix = &pattern[..index];
            let last_separator = static_prefix
                .char_indices()
                .rev()
                .find(|(_, ch)| *ch == '/' || *ch == '\\')
                .map(|(idx, _)| idx);

            if let Some(separator_index) = last_separator {
                let mut base_dir = static_prefix[..separator_index].to_string();

                // Preserve the root for patterns such as `/*.txt`. On Windows,
                // also preserve the separator after a drive prefix: `C:/*.txt`
                // must search from `C:/`, not from the drive-relative `C:`.
                if base_dir.is_empty() && separator_index == 0 {
                    base_dir = static_prefix[..1].to_string();
                }
                #[cfg(windows)]
                if base_dir.len() == 2
                    && base_dir.as_bytes()[1] == b':'
                    && base_dir.as_bytes()[0].is_ascii_alphabetic()
                {
                    base_dir.push(
                        static_prefix[separator_index..]
                            .chars()
                            .next()
                            .expect("separator index must point to a character"),
                    );
                }

                (base_dir, pattern[separator_index + 1..].to_string())
            } else {
                (String::new(), pattern.to_string())
            }
        }
        None => {
            let trimmed = pattern.trim_end_matches(['/', '\\']);
            let literal_path = Path::new(trimmed);
            let base_dir = literal_path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty() && *parent != Path::new("."))
                .map(|parent| parent.to_string_lossy().to_string())
                .unwrap_or_default();
            let file_name = literal_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| trimmed.to_string());

            let relative_pattern = if pattern.ends_with('/') || pattern.ends_with('\\') {
                format!("{}/", file_name)
            } else {
                file_name
            };

            (base_dir, relative_pattern)
        }
    }
}

pub fn normalize_path(path: &Path) -> String {
    dunce::simplified(path).to_string_lossy().replace('\\', "/")
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct GlobCandidate {
    depth: usize,
    path: String,
}

impl Ord for GlobCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.depth
            .cmp(&other.depth)
            .then_with(|| self.path.cmp(&other.path))
    }
}

impl PartialOrd for GlobCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The matcher consumes provider-relative paths, not host `Path` candidates.
/// This keeps a POSIX backslash filename intact on a Windows controller.
struct WorkspaceGlobMatcher {
    matcher: regex::bytes::Regex,
    match_basename: bool,
}

impl WorkspaceGlobMatcher {
    fn new(pattern: &str) -> Result<Self, String> {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .backslash_escape(true)
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            matcher: regex::bytes::Regex::new(glob.regex()).map_err(|error| error.to_string())?,
            match_basename: !pattern.contains('/'),
        })
    }

    fn is_match(&self, relative_path: &str) -> bool {
        self.matcher.is_match(relative_path.as_bytes())
            || (self.match_basename
                && relative_path
                    .rsplit('/')
                    .next()
                    .is_some_and(|name| self.matcher.is_match(name.as_bytes())))
    }
}

/// One result reducer for native walking, command output and workspace IO.
/// Only the best `limit` candidates are retained, while totals remain exact.
struct GlobCollector {
    limit: usize,
    total: usize,
    best: BinaryHeap<GlobCandidate>,
}

impl GlobCollector {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            total: 0,
            best: BinaryHeap::with_capacity(limit.saturating_add(1)),
        }
    }

    fn push(&mut self, path: String) {
        if path.is_empty() {
            return;
        }
        self.total += 1;
        if self.limit == 0 {
            return;
        }
        let candidate = GlobCandidate {
            depth: path.split('/').count(),
            path,
        };
        if self.best.len() < self.limit {
            self.best.push(candidate);
        } else if self.best.peek().is_some_and(|worst| candidate < *worst) {
            self.best.pop();
            self.best.push(candidate);
        }
    }

    fn finish(self) -> (Vec<PathBuf>, usize) {
        let mut matches = self
            .best
            .into_iter()
            .map(|candidate| PathBuf::from(candidate.path))
            .collect::<Vec<_>>();
        matches.sort();
        (matches, self.total)
    }
}

fn is_safe_relative_subpath(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

pub fn derive_walk_root(search_path_abs: &Path, pattern: &str) -> (PathBuf, String) {
    let (base_dir, relative_pattern) = extract_glob_base_directory(pattern);
    let base_path = Path::new(&base_dir);

    if base_dir.is_empty() || !is_safe_relative_subpath(base_path) {
        return (search_path_abs.to_path_buf(), pattern.to_string());
    }

    let walk_root = search_path_abs.join(base_path);
    if walk_root.starts_with(search_path_abs) {
        (walk_root, relative_pattern)
    } else {
        (search_path_abs.to_path_buf(), pattern.to_string())
    }
}

pub fn extract_remote_glob_base_directory(pattern: &str) -> (String, String) {
    let static_prefix = pattern
        .find(['*', '?', '[', '{'])
        .map_or(pattern.trim_end_matches('/'), |index| &pattern[..index]);
    // This is a glob prefix, not yet a filesystem spelling. Escaped characters
    // must be interpreted by the matcher, never copied into a directory name.
    // Keeping the original root avoids needing a second glob parser here.
    if static_prefix.contains('\\') {
        return (String::new(), pattern.to_string());
    }
    match static_prefix.rfind('/') {
        Some(index) => (
            if index == 0 { "/" } else { &pattern[..index] }.to_string(),
            pattern[index + 1..].to_string(),
        ),
        None => (String::new(), pattern.to_string()),
    }
}

/// Remote paths are POSIX strings even when the controller runs on Windows.
pub fn derive_remote_walk_root(search_dir: &str, pattern: &str) -> (String, String) {
    let (base_dir, relative_pattern) = extract_remote_glob_base_directory(pattern);
    if base_dir.is_empty()
        || base_dir.starts_with('/')
        || base_dir.split('/').any(|component| component == "..")
    {
        return (search_dir.to_string(), pattern.to_string());
    }
    let suffix = base_dir
        .split('/')
        .filter(|component| !component.is_empty() && *component != ".")
        .collect::<Vec<_>>()
        .join("/");
    let root = if suffix.is_empty() {
        search_dir.to_string()
    } else {
        format!("{}/{suffix}", search_dir.trim_end_matches('/'))
    };
    (root, relative_pattern)
}

pub fn resolve_glob_config(pattern: &str) -> (bool, bool) {
    let is_whitelisted = pattern.starts_with(".bitfun")
        || pattern.contains("/.bitfun")
        || pattern.contains("\\.bitfun");

    let apply_gitignore = !is_whitelisted;
    let ignore_hidden_files = !is_whitelisted;
    (apply_gitignore, ignore_hidden_files)
}

fn build_rg_args(
    relative_pattern: &str,
    apply_gitignore: bool,
    ignore_hidden_files: bool,
) -> Vec<String> {
    let mut args = vec![
        "--files".to_string(),
        "--glob".to_string(),
        relative_pattern.to_string(),
    ];

    if !apply_gitignore {
        args.push("--no-ignore".to_string());
    }

    if !ignore_hidden_files {
        args.push("--hidden".to_string());
    }

    args
}

#[cfg(windows)]
fn create_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(windows))]
fn create_command(program: &str) -> Command {
    Command::new(program)
}

fn build_fallback_matcher(relative_pattern: &str) -> Result<WorkspaceGlobMatcher, String> {
    WorkspaceGlobMatcher::new(relative_pattern)
}

fn match_relative_path(
    matcher: &WorkspaceGlobMatcher,
    _pattern: &str,
    relative_path: &str,
) -> bool {
    matcher.is_match(relative_path)
}

fn native_glob_relative_path(path: &Path) -> String {
    #[cfg(windows)]
    {
        normalize_path(path)
    }
    #[cfg(not(windows))]
    {
        path.to_string_lossy().into_owned()
    }
}

fn strip_current_dir_prefix(path: &str) -> &str {
    path.strip_prefix("./")
        .or_else(|| path.strip_prefix(".\\"))
        .unwrap_or(path)
}

fn relativize_remote_stdout_path(search_dir: &str, path: &str) -> String {
    let normalized_path = path.strip_prefix("./").unwrap_or(path);
    let search_dir_with_slash = format!("{}/", search_dir.trim_end_matches('/'));

    if let Some(relative_path) = normalized_path.strip_prefix(&search_dir_with_slash) {
        return relative_path.to_string();
    }
    if normalized_path == search_dir {
        return String::new();
    }

    normalized_path.to_string()
}

fn collect_with_walk_fallback(
    walk_root: &Path,
    relative_pattern: &str,
    apply_gitignore: bool,
    ignore_hidden_files: bool,
    limit: usize,
) -> Result<LocalGlobResult, String> {
    let matcher = build_fallback_matcher(relative_pattern)?;
    let walker = WalkBuilder::new(walk_root)
        .ignore(apply_gitignore)
        .git_ignore(apply_gitignore)
        .git_global(apply_gitignore)
        .git_exclude(apply_gitignore)
        .hidden(ignore_hidden_files)
        .build();

    let mut collector = GlobCollector::new(limit);
    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                warn!("Glob walker fallback entry error (skipped): {}", error);
                continue;
            }
        };
        if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_dir())
        {
            continue;
        }
        let relative_path = match entry.path().strip_prefix(walk_root) {
            Ok(relative) => native_glob_relative_path(relative),
            Err(_) => continue,
        };
        if match_relative_path(&matcher, relative_pattern, &relative_path) {
            collector.push(relative_path);
        }
    }
    let (matches, total) = collector.finish();
    Ok(LocalGlobResult {
        matches,
        walk_root: walk_root.to_path_buf(),
        total_matches: Some(total),
        truncated: total > limit,
    })
}

pub fn limit_paths(paths: &[PathBuf], limit: usize) -> Vec<PathBuf> {
    let mut collector = GlobCollector::new(limit);
    for path in paths {
        collector.push(native_glob_relative_path(path));
    }
    collector.finish().0
}

pub fn collect_remote_glob_matches(search_dir: &str, stdout: &str, limit: usize) -> Vec<PathBuf> {
    collect_remote_limited_paths(search_dir, remote_stdout_paths(stdout), limit).0
}

fn remote_stdout_paths(stdout: &str) -> Box<dyn Iterator<Item = &str> + '_> {
    if stdout.contains('\0') {
        Box::new(stdout.split_terminator('\0'))
    } else {
        // Keep accepting the legacy newline-delimited response shape.
        Box::new(stdout.lines())
    }
}

fn collect_remote_limited_paths<'a>(
    search_dir: &str,
    paths: impl Iterator<Item = &'a str>,
    limit: usize,
) -> (Vec<PathBuf>, usize) {
    let mut collector = GlobCollector::new(limit);
    for path in paths.filter(|path| !path.is_empty()) {
        collector.push(relativize_remote_stdout_path(search_dir, path));
    }
    collector.finish()
}

pub fn collect_remote_glob_result(
    search_dir: &str,
    stdout: &str,
    limit: usize,
    exact_total: bool,
) -> LocalGlobResult {
    let (matches, observed_matches) =
        collect_remote_limited_paths(search_dir, remote_stdout_paths(stdout), limit);
    let truncated = observed_matches > limit;
    let total_matches = if exact_total || !truncated {
        Some(observed_matches)
    } else {
        None
    };

    LocalGlobResult {
        matches,
        walk_root: PathBuf::from(search_dir),
        total_matches,
        truncated,
    }
}

/// A dependency-free workspace-IO fallback. It shares matching and bounded
/// collection with the native walker and never weakens the pattern to find -name.
///
/// Ignore discovery is confined to the supplied search scope. The native/rg
/// acceleration paths retain their target Git/global configuration discovery;
/// callers must not present this fallback as supporting configuration outside
/// the authorized scope. Only `limit` result paths are retained, rather than
/// buffering one shell output for the entire tree.
pub async fn collect_workspace_glob(
    fs: &dyn WorkspaceFileSystem,
    search_dir: &str,
    pattern: &str,
    limit: usize,
) -> Result<LocalGlobResult, String> {
    match fs
        .path_kind_no_follow(search_dir)
        .await
        .map_err(|error| error.to_string())?
    {
        Some(WorkspacePathKind::Directory) => {}
        None => return Err(format!("Search path '{search_dir}' does not exist")),
        Some(_) => return Err(format!("Search path '{search_dir}' is not a directory")),
    }
    let (base, relative) = extract_remote_glob_base_directory(pattern);
    let safe_prefix = !base.starts_with('/') && !base.split('/').any(|part| part == "..");
    let (prefix, relative_pattern) = if safe_prefix {
        (
            base.split('/')
                .filter(|part| !part.is_empty() && *part != ".")
                .map(str::to_string)
                .collect::<Vec<_>>(),
            relative,
        )
    } else {
        (Vec::new(), pattern.to_string())
    };
    let matcher = WorkspaceGlobMatcher::new(&relative_pattern)?;
    let (apply_ignore, hide_hidden) = resolve_glob_config(pattern);
    let mut walk_root = search_dir.to_string();
    let mut initial_rules = Vec::new();
    for (depth, component) in prefix.iter().enumerate() {
        if apply_ignore {
            let entries = fs
                .read_dir(&walk_root)
                .await
                .map_err(|error| format!("Failed to list {walk_root}: {error}"))?;
            add_directory_ignores(fs, &entries, depth, &mut initial_rules).await?;
        }
        walk_root = fs.join_path(&walk_root, &[component]);
        match fs
            .path_kind_no_follow(&walk_root)
            .await
            .map_err(|error| error.to_string())?
        {
            Some(WorkspacePathKind::Directory) => {}
            _ => {
                return Ok(LocalGlobResult {
                    matches: Vec::new(),
                    walk_root: PathBuf::from(walk_root),
                    total_matches: Some(0),
                    truncated: false,
                })
            }
        }
    }
    if limit == 0 {
        return Ok(LocalGlobResult {
            matches: Vec::new(),
            walk_root: PathBuf::from(walk_root),
            total_matches: Some(0),
            truncated: false,
        });
    }
    let mut walker = WorkspaceFileWalker::with_scope(
        fs,
        walk_root.clone(),
        prefix,
        initial_rules,
        hide_hidden,
        apply_ignore,
    );
    let mut collector = GlobCollector::new(limit);
    while let Some(file) = walker.next().await? {
        if matcher.is_match(&file.relative_path) {
            collector.push(file.relative_path);
        }
    }
    let (matches, total) = collector.finish();
    Ok(LocalGlobResult {
        matches,
        walk_root: PathBuf::from(walk_root),
        total_matches: Some(total),
        truncated: total > limit,
    })
}

pub fn validate_remote_glob_exit(exit_code: i32, stderr: &str) -> Result<(), String> {
    if exit_code == 0 || exit_code == 1 {
        return Ok(());
    }
    let details = stderr.trim();
    Err(if details.is_empty() {
        format!("Remote glob failed with exit code {exit_code}")
    } else {
        format!("Remote glob failed with exit code {exit_code}: {details}")
    })
}

pub fn execute_local_glob(request: LocalGlobRequest) -> Result<LocalGlobResult, String> {
    if !request.search_path.exists() {
        return Err(format!(
            "Search path '{}' does not exist",
            request.search_path.display()
        ));
    }
    if !request.search_path.is_dir() {
        return Err(format!(
            "Search path '{}' is not a directory",
            request.search_path.display()
        ));
    }

    let search_path_abs =
        dunce::canonicalize(&request.search_path).map_err(|error| error.to_string())?;
    let (walk_root, relative_pattern) = derive_walk_root(&search_path_abs, &request.pattern);
    let (apply_gitignore, ignore_hidden_files) = resolve_glob_config(&request.pattern);

    if !walk_root.exists() || !walk_root.is_dir() || request.limit == 0 {
        return Ok(LocalGlobResult {
            matches: Vec::new(),
            walk_root,
            total_matches: Some(0),
            truncated: false,
        });
    }

    let args = build_rg_args(&relative_pattern, apply_gitignore, ignore_hidden_files);
    let output = create_command("rg")
        .current_dir(&walk_root)
        .args(&args)
        .arg(".")
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "ripgrep (rg) is required for Glob tool execution but was not found".to_string()
            } else {
                format!("Failed to execute rg for Glob tool: {}", error)
            }
        });

    let output = match output {
        Ok(output) => {
            info!(
                "Glob backend selected: backend=rg, search_root={}, pattern={}",
                walk_root.display(),
                relative_pattern
            );
            output
        }
        Err(error) if error.contains("ripgrep (rg) is required") => {
            info!(
                "Glob backend selected: backend=fallback_walk, reason=rg_not_found, search_root={}, pattern={}",
                walk_root.display(),
                relative_pattern
            );
            return collect_with_walk_fallback(
                &walk_root,
                &relative_pattern,
                apply_gitignore,
                ignore_hidden_files,
                request.limit,
            );
        }
        Err(error) => return Err(error),
    };

    if !output.status.success() {
        if output.status.code() == Some(1) {
            return Ok(LocalGlobResult {
                matches: Vec::new(),
                walk_root,
                total_matches: Some(0),
                truncated: false,
            });
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let message = if stderr.is_empty() {
            format!("rg --files failed with status {}", output.status)
        } else {
            format!("rg --files failed: {}", stderr)
        };
        if stderr.contains("No such file or directory") || stderr.contains("not found") {
            info!(
                "Glob backend selected: backend=fallback_walk, reason=rg_execution_failed, search_root={}, pattern={}",
                walk_root.display(),
                relative_pattern
            );
            return collect_with_walk_fallback(
                &walk_root,
                &relative_pattern,
                apply_gitignore,
                ignore_hidden_files,
                request.limit,
            );
        }
        return Err(message);
    }

    let all_paths = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let relative_path = strip_current_dir_prefix(line);
            PathBuf::from(relative_path)
        })
        .collect::<Vec<_>>();

    let total_matches = all_paths.len();
    Ok(LocalGlobResult {
        matches: limit_paths(&all_paths, request.limit),
        walk_root,
        total_matches: Some(total_matches),
        truncated: total_matches > request.limit,
    })
}

pub fn build_remote_rg_command(search_dir: &str, pattern: &str) -> String {
    let (remote_walk_root, remote_pattern) = derive_remote_walk_root(search_dir, pattern);
    let (apply_gitignore, ignore_hidden_files) = resolve_glob_config(pattern);

    let mut parts = vec![
        "(cd".to_string(),
        shell_single_quote(&remote_walk_root),
        "|| exit 2;".to_string(),
        "rg".to_string(),
        "--no-config".to_string(),
        "--files".to_string(),
        "--null".to_string(),
        "--glob".to_string(),
        shell_single_quote(&remote_pattern),
    ];

    if !apply_gitignore {
        parts.push("--no-ignore".to_string());
    }

    if !ignore_hidden_files {
        parts.push("--hidden".to_string());
    }

    parts.push(".".to_string());
    parts.push(")".to_string());
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::{
        collect_with_walk_fallback, derive_remote_walk_root, extract_glob_base_directory,
        normalize_path, WorkspaceGlobMatcher,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempTree {
        root: PathBuf,
    }

    impl TempTree {
        fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn make_temp_dir(name: &str) -> TempTree {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("bitfun-glob-search-{name}-{unique}"));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        TempTree { root: dir }
    }

    fn normalized(path: &Path) -> String {
        normalize_path(path)
    }

    #[test]
    fn walk_fallback_returns_files_only_and_matches_rg_basename_globs() {
        let temp = make_temp_dir("files-only");
        let root = temp.path();
        fs::create_dir_all(root.join("src").join("nested")).expect("dirs should be created");
        fs::write(root.join("src").join("nested").join("lib.rs"), "")
            .expect("file should be written");

        let wildcard_matches = collect_with_walk_fallback(root, "*", false, false, 10)
            .expect("fallback glob should succeed")
            .matches
            .into_iter()
            .map(|path| normalized(&path))
            .collect::<Vec<_>>();

        assert!(wildcard_matches
            .iter()
            .all(|path| !path.ends_with("/src") && !path.ends_with("/src/nested")));
        assert!(wildcard_matches
            .iter()
            .any(|path| path == "src/nested/lib.rs"));

        let rust_matches = collect_with_walk_fallback(root, "*.rs", false, false, 10)
            .expect("fallback rust glob should succeed")
            .matches
            .into_iter()
            .map(|path| normalized(&path))
            .collect::<Vec<_>>();
        assert_eq!(rust_matches.len(), 1);
        assert_eq!(rust_matches[0], "src/nested/lib.rs");

        let directory_name_matches = collect_with_walk_fallback(root, "src", false, false, 10)
            .expect("fallback directory-name glob should succeed");
        assert!(directory_name_matches.matches.is_empty());
        assert_eq!(directory_name_matches.total_matches, Some(0));
        assert!(!directory_name_matches.truncated);
    }

    #[test]
    fn extract_glob_base_directory_preserves_absolute_roots() {
        assert_eq!(
            extract_glob_base_directory("/*.txt"),
            ("/".to_string(), "*.txt".to_string())
        );

        #[cfg(windows)]
        assert_eq!(
            extract_glob_base_directory("C:/*.txt"),
            ("C:/".to_string(), "*.txt".to_string())
        );
    }

    #[test]
    fn shared_glob_matcher_keeps_posix_backslashes_and_complete_patterns() {
        let matcher = WorkspaceGlobMatcher::new(r"a\\b.rs").unwrap();
        assert!(matcher.is_match(r"a\b.rs"));
        assert!(!matcher.is_match("a/b.rs"));
        let matcher = WorkspaceGlobMatcher::new("**/src/*.{rs,ts}").unwrap();
        assert!(matcher.is_match(&format!("{}src/file.rs", "level/".repeat(12))));
        assert!(!matcher.is_match("other/not-src/file.rs"));
        assert!(!matcher.is_match("other/src/file.js"));
        assert_eq!(
            derive_remote_walk_root(r"/repo\name/", r"src\cache/*.rs"),
            (r"/repo\name/".to_string(), r"src\cache/*.rs".to_string())
        );
        assert_eq!(
            derive_remote_walk_root("/", "src/*.rs"),
            ("/src".to_string(), "*.rs".to_string())
        );
    }

    #[test]
    fn escaped_static_prefix_is_matched_without_becoming_a_literal_walk_root() {
        for (pattern, file) in [
            (r"a\\b/*.rs", r"a\b/source.rs"),
            (r"a\ space/*.rs", "a space/source.rs"),
            (r"literal\?/nested/*.rs", "literal?/nested/source.rs"),
        ] {
            assert_eq!(
                derive_remote_walk_root("/repo", pattern),
                ("/repo".to_string(), pattern.to_string())
            );
            assert!(WorkspaceGlobMatcher::new(pattern).unwrap().is_match(file));
            #[cfg(not(windows))]
            assert_eq!(
                super::derive_walk_root(Path::new("/repo"), pattern),
                (PathBuf::from("/repo"), pattern.to_string())
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn native_fallback_uses_the_shared_posix_matcher_for_backslash_names() {
        let temp = make_temp_dir("posix-names");
        fs::write(temp.path().join(r"a\b.rs"), "").unwrap();
        let result = collect_with_walk_fallback(temp.path(), r"a\\b.rs", false, false, 10).unwrap();
        assert_eq!(result.matches, vec![PathBuf::from(r"a\b.rs")]);
        assert_eq!(result.total_matches, Some(1));
    }
}
