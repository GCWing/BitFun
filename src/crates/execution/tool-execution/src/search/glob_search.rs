use crate::util::string::{escape_posix_single_quotes, shell_single_quote};
use globset::{GlobBuilder, GlobMatcher};
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

    match glob_start {
        Some(index) => {
            let static_prefix = &pattern[..index];
            let last_separator = static_prefix
                .char_indices()
                .rev()
                .find(|(_, ch)| *ch == '/' || *ch == '\\')
                .map(|(idx, _)| idx);

            if let Some(separator_index) = last_separator {
                (
                    static_prefix[..separator_index].to_string(),
                    pattern[separator_index + 1..].to_string(),
                )
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
    let command = Command::new(program);
    command
}

fn build_fallback_matcher(relative_pattern: &str) -> Result<GlobMatcher, String> {
    GlobBuilder::new(relative_pattern)
        .literal_separator(true)
        .build()
        .map_err(|error| error.to_string())
        .map(|glob| glob.compile_matcher())
}

fn pattern_has_path_separator(pattern: &str) -> bool {
    pattern.contains('/') || pattern.contains('\\')
}

fn match_relative_path(matcher: &GlobMatcher, relative_pattern: &str, relative_path: &str) -> bool {
    if !pattern_has_path_separator(relative_pattern)
        && Path::new(relative_path)
            .file_name()
            .is_some_and(|file_name| matcher.is_match(file_name))
    {
        return true;
    }

    matcher.is_match(relative_path)
}

fn strip_current_dir_prefix(path: &str) -> &str {
    path.strip_prefix("./")
        .or_else(|| path.strip_prefix(".\\"))
        .unwrap_or(path)
}

fn relativize_remote_stdout_path(search_dir: &str, path: &str) -> String {
    let normalized_path = strip_current_dir_prefix(path).replace('\\', "/");
    let normalized_search_dir = search_dir.replace('\\', "/");
    let search_dir_with_slash = format!("{}/", normalized_search_dir.trim_end_matches('/'));

    if let Some(relative_path) = normalized_path.strip_prefix(&search_dir_with_slash) {
        return relative_path.to_string();
    }
    if normalized_path == normalized_search_dir {
        return String::new();
    }

    normalized_path
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

    let mut best_matches = BinaryHeap::with_capacity(limit.saturating_add(1));
    let mut total_matches = 0usize;
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
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false)
        {
            continue;
        }

        let path = entry.path().to_path_buf();
        let relative_path = match path.strip_prefix(walk_root) {
            Ok(relative) => relative,
            Err(_) => continue,
        };
        let relative_path = normalize_path(relative_path);

        if match_relative_path(&matcher, relative_pattern, &relative_path) {
            total_matches += 1;
            let candidate = GlobCandidate {
                depth: relative_path.split('/').count(),
                path: relative_path,
            };

            if best_matches.len() < limit {
                best_matches.push(candidate);
            } else if let Some(worst_match) = best_matches.peek() {
                if candidate < *worst_match {
                    best_matches.pop();
                    best_matches.push(candidate);
                }
            }
        }
    }

    Ok(LocalGlobResult {
        matches: best_matches
            .into_sorted_vec()
            .into_iter()
            .map(|candidate| PathBuf::from(candidate.path))
            .collect(),
        walk_root: walk_root.to_path_buf(),
        total_matches: Some(total_matches),
        truncated: total_matches > limit,
    })
}

pub fn limit_paths(paths: &[PathBuf], limit: usize) -> Vec<PathBuf> {
    let mut depth_and_paths = paths
        .iter()
        .map(|path| {
            let normalized_path = normalize_path(path);
            let depth = normalized_path.split('/').count();
            (depth, normalized_path)
        })
        .collect::<Vec<_>>();
    depth_and_paths.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    let mut result = depth_and_paths
        .into_iter()
        .take(limit)
        .map(|(_, path)| PathBuf::from(path))
        .collect::<Vec<_>>();
    result.sort();
    result
}

pub fn collect_remote_glob_matches(search_dir: &str, stdout: &str, limit: usize) -> Vec<PathBuf> {
    collect_remote_limited_paths(search_dir, stdout, limit).0
}

fn collect_remote_limited_paths(
    search_dir: &str,
    stdout: &str,
    limit: usize,
) -> (Vec<PathBuf>, usize) {
    let mut best_matches = BinaryHeap::with_capacity(limit.saturating_add(1));
    let mut observed_matches = 0usize;

    for relative_path in stdout
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let relative_path = relativize_remote_stdout_path(search_dir, line);
            (!relative_path.is_empty()).then_some(relative_path)
        })
    {
        observed_matches += 1;
        if limit == 0 {
            continue;
        }

        let candidate = GlobCandidate {
            depth: relative_path.split('/').count(),
            path: relative_path,
        };
        if best_matches.len() < limit {
            best_matches.push(candidate);
        } else if let Some(worst_match) = best_matches.peek() {
            if candidate < *worst_match {
                best_matches.pop();
                best_matches.push(candidate);
            }
        }
    }

    let mut matches = best_matches
        .into_sorted_vec()
        .into_iter()
        .map(|candidate| PathBuf::from(candidate.path))
        .collect::<Vec<_>>();
    matches.sort();
    (matches, observed_matches)
}

pub fn collect_remote_glob_result(
    search_dir: &str,
    stdout: &str,
    limit: usize,
    exact_total: bool,
) -> LocalGlobResult {
    let (matches, observed_matches) = collect_remote_limited_paths(search_dir, stdout, limit);
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

pub fn shell_escape(value: &str) -> String {
    escape_posix_single_quotes(value)
}

pub fn build_remote_rg_command(search_dir: &str, pattern: &str) -> String {
    let search_dir_path = Path::new(search_dir);
    let (remote_walk_root, remote_pattern) = derive_walk_root(search_dir_path, pattern);
    let (apply_gitignore, ignore_hidden_files) = resolve_glob_config(pattern);

    let remote_walk_root = normalize_path(&remote_walk_root);
    let mut parts = vec![
        "cd".to_string(),
        shell_single_quote(&remote_walk_root),
        "&&".to_string(),
        "rg".to_string(),
        "--files".to_string(),
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
    parts.push("2>/dev/null".to_string());
    parts.join(" ")
}

pub fn build_remote_find_command(search_dir: &str, pattern: &str, limit: usize) -> String {
    let search_dir_path = Path::new(search_dir);
    let (remote_walk_root, remote_pattern) = derive_walk_root(search_dir_path, pattern);

    let name_pattern = if remote_pattern.contains("**/") {
        remote_pattern.replacen("**/", "", 1)
    } else if remote_pattern.contains('/') || remote_pattern.contains('\\') {
        "*".to_string()
    } else {
        remote_pattern
    };

    let escaped_dir = shell_single_quote(&normalize_path(&remote_walk_root));
    let escaped_pattern = shell_single_quote(&name_pattern);

    format!(
        "find {} -maxdepth 10 -type f -name {} -not -path '*/.git/*' -not -path '*/node_modules/*' 2>/dev/null | head -n {}",
        escaped_dir,
        escaped_pattern,
        limit.saturating_add(1)
    )
}

#[cfg(test)]
mod tests {
    use super::{collect_with_walk_fallback, normalize_path};
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
}
