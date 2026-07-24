//! Shared, runtime-free bounded file and parser support for ecosystem source adapters.

use bitfun_product_domains::external_hook_catalog::{
    ExternalHookHandlerKind, ExternalHookMatcherSummary,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_MATCHER_BYTES: usize = 512;
const MAX_EVENT_NAME_BYTES: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedFileRead {
    Content(Vec<u8>),
    TooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedTextRead {
    Content(String),
    TooLarge,
    InvalidUtf8,
}

/// Reads at most `max_bytes + 1` bytes so a file changed between metadata and
/// read cannot cause an unbounded allocation.
pub fn read_bounded_file(path: &Path, max_bytes: usize) -> std::io::Result<BoundedFileRead> {
    let file = std::fs::File::open(path)?;
    let read_limit = max_bytes.saturating_add(1) as u64;
    let mut bytes = Vec::with_capacity(max_bytes.min(64 * 1024).saturating_add(1));
    file.take(read_limit).read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        Ok(BoundedFileRead::TooLarge)
    } else {
        Ok(BoundedFileRead::Content(bytes))
    }
}

/// Reads a UTF-8 text file without allocating more than `max_bytes + 1`.
pub fn read_bounded_text(path: &Path, max_bytes: usize) -> std::io::Result<BoundedTextRead> {
    match read_bounded_file(path, max_bytes)? {
        BoundedFileRead::Content(bytes) => Ok(match String::from_utf8(bytes) {
            Ok(content) => BoundedTextRead::Content(content),
            Err(_) => BoundedTextRead::InvalidUtf8,
        }),
        BoundedFileRead::TooLarge => Ok(BoundedTextRead::TooLarge),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedDirectoryWalkLimits {
    pub max_depth: usize,
    pub max_entries: usize,
    pub max_directories: usize,
    pub max_files: usize,
}

impl BoundedDirectoryWalkLimits {
    pub fn for_file_limit(max_files: usize) -> Self {
        Self {
            max_depth: 32,
            max_entries: max_files.saturating_mul(4).max(1),
            max_directories: max_files.max(1),
            max_files,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundedDirectoryWalkLimit {
    Depth,
    Entries,
    Directories,
    Files,
}

#[derive(Debug)]
pub enum BoundedDirectoryWalkError {
    Io(std::io::Error),
    LimitExceeded(BoundedDirectoryWalkLimit),
}

impl std::fmt::Display for BoundedDirectoryWalkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::LimitExceeded(limit) => write!(formatter, "{limit:?} limit exceeded"),
        }
    }
}

impl std::error::Error for BoundedDirectoryWalkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::LimitExceeded(_) => None,
        }
    }
}

/// Iteratively collects matching regular files without following symlinks.
/// Limits apply to the actual traversal cost, not only to matching files.
pub fn collect_bounded_regular_files(
    root: &Path,
    limits: BoundedDirectoryWalkLimits,
    mut matches: impl FnMut(&Path) -> bool,
) -> Result<Vec<PathBuf>, BoundedDirectoryWalkError> {
    let metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(BoundedDirectoryWalkError::Io(error)),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    let mut visited_entries = 0usize;
    let mut visited_directories = 1usize;
    while let Some((directory, depth)) = stack.pop() {
        let entries = std::fs::read_dir(&directory).map_err(BoundedDirectoryWalkError::Io)?;
        for entry in entries {
            let entry = entry.map_err(BoundedDirectoryWalkError::Io)?;
            visited_entries = visited_entries.saturating_add(1);
            if visited_entries > limits.max_entries {
                return Err(BoundedDirectoryWalkError::LimitExceeded(
                    BoundedDirectoryWalkLimit::Entries,
                ));
            }
            let file_type = entry.file_type().map_err(BoundedDirectoryWalkError::Io)?;
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                let next_depth = depth.saturating_add(1);
                if next_depth > limits.max_depth {
                    return Err(BoundedDirectoryWalkError::LimitExceeded(
                        BoundedDirectoryWalkLimit::Depth,
                    ));
                }
                visited_directories = visited_directories.saturating_add(1);
                if visited_directories > limits.max_directories {
                    return Err(BoundedDirectoryWalkError::LimitExceeded(
                        BoundedDirectoryWalkLimit::Directories,
                    ));
                }
                stack.push((path, next_depth));
            } else if file_type.is_file() && matches(&path) {
                if files.len() >= limits.max_files {
                    return Err(BoundedDirectoryWalkError::LimitExceeded(
                        BoundedDirectoryWalkLimit::Files,
                    ));
                }
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Distinguishes an absent path from metadata failures. Static adapters may
/// ignore `NotFound`, but permission and transient filesystem failures must be
/// surfaced so the coordinator can retain the last valid snapshot as stale.
pub fn regular_file_exists(path: &Path) -> std::io::Result<bool> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[derive(Debug)]
pub enum BoundedFileResolveError {
    OutsideRoot,
    NotRegular,
    Io(std::io::Error),
}

/// Resolves a configured file once and verifies that its canonical target is a
/// regular file inside the canonical source root. The source root itself may
/// be a user-managed symlink, but indirection below it cannot escape that
/// canonical root. Callers should read the returned canonical path; concurrent
/// same-user filesystem replacement remains outside this static boundary.
pub fn resolve_bounded_regular_file(
    path: &Path,
    allowed_root: &Path,
) -> Result<PathBuf, BoundedFileResolveError> {
    let canonical_root =
        std::fs::canonicalize(allowed_root).map_err(BoundedFileResolveError::Io)?;
    let canonical_path = std::fs::canonicalize(path).map_err(BoundedFileResolveError::Io)?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(BoundedFileResolveError::OutsideRoot);
    }
    let metadata = std::fs::metadata(&canonical_path).map_err(BoundedFileResolveError::Io)?;
    if !metadata.is_file() {
        return Err(BoundedFileResolveError::NotRegular);
    }
    Ok(canonical_path)
}

/// Produces a useful executable label without exposing an absolute path or a
/// shell-like command string. Runtime preparation retains the original value.
pub fn redacted_executable_preview(command: &str) -> String {
    let command = command.trim();
    if command.is_empty() {
        return "unsupported".to_string();
    }
    if command.chars().any(char::is_whitespace)
        || command.chars().any(char::is_control)
        || command.contains('=')
    {
        return "<configured-command>".to_string();
    }
    let normalized = command.replace('\\', "/");
    normalized
        .rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or("<configured-command>")
        .to_string()
}

/// Returns the bounded project path chain from the outer project boundary to
/// the selected workspace directory. An invalid boundary fails closed to the
/// workspace itself so adapters never walk arbitrary filesystem ancestors.
pub fn bounded_project_ancestors(
    workspace_root: &Path,
    project_boundary: &Path,
    max_depth: usize,
) -> Vec<std::path::PathBuf> {
    if max_depth == 0 || !workspace_root.starts_with(project_boundary) {
        return vec![workspace_root.to_path_buf()];
    }
    let mut roots = Vec::new();
    let mut current = Some(workspace_root);
    while let Some(path) = current {
        if !path.starts_with(project_boundary) || roots.len() == max_depth {
            break;
        }
        roots.push(path.to_path_buf());
        if path == project_boundary {
            roots.reverse();
            return roots;
        }
        current = path.parent();
    }
    vec![workspace_root.to_path_buf()]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticHookDocumentFormat {
    Json,
    Toml,
}

#[derive(Debug, Clone, Copy)]
pub struct StaticHookHandlerRule {
    pub native_type: &'static str,
    pub handler_kind: ExternalHookHandlerKind,
    pub required_string_fields: &'static [&'static str],
}

impl StaticHookHandlerRule {
    pub const fn new(
        native_type: &'static str,
        handler_kind: ExternalHookHandlerKind,
        required_string_fields: &'static [&'static str],
    ) -> Self {
        Self {
            native_type,
            handler_kind,
            required_string_fields,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StaticHookParseIssue {
    DocumentInvalid,
    EventNameInvalid,
    EventInvalid,
    GroupInvalid,
    HandlerInvalid,
    HandlerLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticHookHandlerFact {
    pub native_event: String,
    pub matcher: ExternalHookMatcherSummary,
    pub handler_kind: ExternalHookHandlerKind,
    pub group_index: usize,
    pub handler_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StaticHookParseResult {
    pub handlers: Vec<StaticHookHandlerFact>,
    pub issues: Vec<StaticHookParseIssue>,
    pub all_disabled: bool,
    pub inspected_handlers: usize,
}

/// Fingerprints only facts that the catalog already exposes. Handler bodies,
/// command arguments, request data, environment variables, and credentials
/// never contribute to the externally visible version.
pub fn redacted_parse_content_version(result: &StaticHookParseResult) -> String {
    let mut hasher = Sha256::new();
    hasher.update(if result.all_disabled {
        b"disabled".as_slice()
    } else {
        b"unknown".as_slice()
    });
    for handler in &result.handlers {
        hasher.update([0]);
        hasher.update(handler.native_event.as_bytes());
        hasher.update([0]);
        match &handler.matcher {
            ExternalHookMatcherSummary::Any => hasher.update(b"any"),
            ExternalHookMatcherSummary::Pattern { display } => {
                hasher.update(b"pattern:");
                hasher.update(display.as_bytes());
            }
            ExternalHookMatcherSummary::Dynamic => hasher.update(b"dynamic"),
            ExternalHookMatcherSummary::Unavailable => hasher.update(b"unavailable"),
            _ => hasher.update(b"unknown_matcher"),
        }
        hasher.update(format!(
            ":{:?}:{}:{}",
            handler.handler_kind, handler.group_index, handler.handler_index
        ));
    }
    for issue in &result.issues {
        hasher.update(format!(":issue:{issue:?}"));
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Parses only Hook structure and returns redacted facts. Handler-specific
/// values are checked for presence but never copied into the result.
pub fn parse_hook_document(
    bytes: &[u8],
    format: StaticHookDocumentFormat,
    rules: &[StaticHookHandlerRule],
    max_handlers: usize,
) -> StaticHookParseResult {
    let parsed = match format {
        StaticHookDocumentFormat::Json => serde_json::from_slice::<Value>(bytes).ok(),
        StaticHookDocumentFormat::Toml => std::str::from_utf8(bytes)
            .ok()
            .and_then(|source| toml::from_str::<toml::Value>(source).ok())
            .and_then(|value| serde_json::to_value(value).ok()),
    };
    let Some(Value::Object(root)) = parsed else {
        return StaticHookParseResult {
            issues: vec![StaticHookParseIssue::DocumentInvalid],
            ..StaticHookParseResult::default()
        };
    };

    // This is only the Claude-compatible document flag. Other ecosystem
    // adapters ignore it; static discovery does not evaluate Codex activation.
    let all_disabled = root
        .get("disableAllHooks")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut result = StaticHookParseResult {
        all_disabled,
        ..StaticHookParseResult::default()
    };
    let Some(Value::Object(events)) = root.get("hooks") else {
        return result;
    };

    let mut event_names = events
        .keys()
        .filter(|name| name.as_str() != "state")
        .cloned()
        .collect::<Vec<_>>();
    event_names.sort();
    'events: for native_event in event_names {
        if native_event.is_empty()
            || native_event.len() > MAX_EVENT_NAME_BYTES
            || native_event.chars().any(char::is_control)
        {
            record_issue(&mut result, StaticHookParseIssue::EventNameInvalid);
            continue;
        }
        let Some(groups) = events.get(&native_event).and_then(Value::as_array) else {
            record_issue(&mut result, StaticHookParseIssue::EventInvalid);
            continue;
        };
        for (group_index, group) in groups.iter().enumerate() {
            let Some(group) = group.as_object() else {
                record_issue(&mut result, StaticHookParseIssue::GroupInvalid);
                continue;
            };
            let matcher = matcher_summary(group.get("matcher"));
            let Some(handlers) = group.get("hooks").and_then(Value::as_array) else {
                record_issue(&mut result, StaticHookParseIssue::GroupInvalid);
                continue;
            };
            for (handler_index, handler) in handlers.iter().enumerate() {
                if result.inspected_handlers >= max_handlers {
                    record_issue(&mut result, StaticHookParseIssue::HandlerLimit);
                    break 'events;
                }
                result.inspected_handlers += 1;
                let Some(handler_kind) = parse_handler_kind(handler, rules) else {
                    record_issue(&mut result, StaticHookParseIssue::HandlerInvalid);
                    continue;
                };
                result.handlers.push(StaticHookHandlerFact {
                    native_event: native_event.clone(),
                    matcher: matcher.clone(),
                    handler_kind,
                    group_index,
                    handler_index,
                });
            }
        }
    }
    result
}

fn record_issue(result: &mut StaticHookParseResult, issue: StaticHookParseIssue) {
    if !result.issues.contains(&issue) {
        result.issues.push(issue);
    }
}

fn parse_handler_kind(
    value: &Value,
    rules: &[StaticHookHandlerRule],
) -> Option<ExternalHookHandlerKind> {
    let object = value.as_object()?;
    let native_type = object.get("type")?.as_str()?;
    let rule = rules.iter().find(|rule| rule.native_type == native_type)?;
    rule.required_string_fields
        .iter()
        .all(|field| {
            object
                .get(*field)
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty())
        })
        .then_some(rule.handler_kind)
}

fn matcher_summary(value: Option<&Value>) -> ExternalHookMatcherSummary {
    match value {
        None => ExternalHookMatcherSummary::Any,
        Some(Value::String(value)) if value.is_empty() => ExternalHookMatcherSummary::Any,
        Some(Value::String(value))
            if value.len() <= MAX_MATCHER_BYTES && !value.chars().any(char::is_control) =>
        {
            ExternalHookMatcherSummary::Pattern {
                display: value.to_string(),
            }
        }
        Some(_) => ExternalHookMatcherSummary::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_ancestors_are_bounded_and_returned_outer_to_inner() {
        let boundary = Path::new("/repo");
        let workspace = Path::new("/repo/packages/app");
        assert_eq!(
            bounded_project_ancestors(workspace, boundary, 8),
            vec![
                std::path::PathBuf::from("/repo"),
                std::path::PathBuf::from("/repo/packages"),
                std::path::PathBuf::from("/repo/packages/app"),
            ]
        );
        assert_eq!(
            bounded_project_ancestors(workspace, Path::new("/other"), 8),
            vec![workspace.to_path_buf()]
        );
    }

    #[test]
    fn shared_parser_does_not_interpret_codex_feature_flags() {
        let result = parse_hook_document(
            br#"{"features":{"hooks":false}}"#,
            StaticHookDocumentFormat::Json,
            &[],
            1,
        );
        assert!(!result.all_disabled);
    }

    #[test]
    fn executable_preview_keeps_only_a_safe_basename() {
        assert_eq!(
            redacted_executable_preview(r"C:\Users\alice\private\mcp.exe"),
            "mcp.exe",
        );
        assert_eq!(redacted_executable_preview("/home/alice/bin/mcp"), "mcp");
        assert_eq!(redacted_executable_preview("npx"), "npx");
        assert_eq!(
            redacted_executable_preview("TOKEN=secret npx"),
            "<configured-command>",
        );
    }

    #[test]
    fn bounded_file_accepts_regular_files_only_inside_the_declared_root() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("declared-root");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let inside_file = root.join("config.toml");
        let outside_file = outside.join("config.toml");
        std::fs::write(&inside_file, "enabled = true").unwrap();
        std::fs::write(&outside_file, "enabled = true").unwrap();

        assert_eq!(
            resolve_bounded_regular_file(&inside_file, &root).unwrap(),
            std::fs::canonicalize(inside_file).unwrap(),
        );
        assert!(matches!(
            resolve_bounded_regular_file(&outside_file, &root),
            Err(BoundedFileResolveError::OutsideRoot)
        ));
    }

    #[test]
    fn bounded_text_read_checks_the_bytes_actually_read() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.json");
        std::fs::write(&path, "12345").unwrap();

        assert_eq!(
            read_bounded_text(&path, 4).unwrap(),
            BoundedTextRead::TooLarge
        );
        assert_eq!(
            read_bounded_text(&path, 5).unwrap(),
            BoundedTextRead::Content("12345".to_string())
        );
    }

    #[test]
    fn bounded_walk_limits_actual_entries_and_depth() {
        let temp = tempfile::tempdir().unwrap();
        for name in ["a.txt", "b.txt", "c.txt"] {
            std::fs::write(temp.path().join(name), "ignored").unwrap();
        }
        let error = collect_bounded_regular_files(
            temp.path(),
            BoundedDirectoryWalkLimits {
                max_depth: 8,
                max_entries: 2,
                max_directories: 8,
                max_files: 8,
            },
            |_| false,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            BoundedDirectoryWalkError::LimitExceeded(BoundedDirectoryWalkLimit::Entries)
        ));

        let nested = temp.path().join("one/two");
        std::fs::create_dir_all(&nested).unwrap();
        let error = collect_bounded_regular_files(
            temp.path(),
            BoundedDirectoryWalkLimits {
                max_depth: 1,
                max_entries: 16,
                max_directories: 16,
                max_files: 16,
            },
            |_| false,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            BoundedDirectoryWalkError::LimitExceeded(BoundedDirectoryWalkLimit::Depth)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_file_rejects_an_intermediate_symlink_escape() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("declared-root");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("config.toml"), "enabled = true").unwrap();
        symlink(&outside, root.join("linked-directory")).unwrap();

        assert!(matches!(
            resolve_bounded_regular_file(&root.join("linked-directory/config.toml"), &root),
            Err(BoundedFileResolveError::OutsideRoot)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_file_accepts_a_user_managed_symlink_root() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let actual = temp.path().join("dotfiles/claude");
        std::fs::create_dir_all(&actual).unwrap();
        let config = actual.join("config.json");
        std::fs::write(&config, "{}").unwrap();
        let linked = temp.path().join(".claude");
        symlink(&actual, &linked).unwrap();

        assert_eq!(
            resolve_bounded_regular_file(&linked.join("config.json"), &linked).unwrap(),
            std::fs::canonicalize(config).unwrap(),
        );
    }
}
