//! Shared, runtime-free bounded file and parser support for ecosystem source adapters.

use openbitfun_product_domains::external_hook_catalog::{
    ExternalHookHandlerKind, ExternalHookMatcherSummary,
};
use openbitfun_product_domains::external_hook_import::{
    ExternalHookImportDependencyV1, MANAGED_HOOK_ROOT_PLACEHOLDER, MAX_EXTERNAL_HOOK_IMPORT_ASSETS,
    MAX_EXTERNAL_HOOK_IMPORT_ASSET_BYTES, MAX_EXTERNAL_HOOK_IMPORT_ASSET_DEPTH,
    MAX_EXTERNAL_HOOK_IMPORT_TOTAL_ASSET_BYTES,
};
use openbitfun_product_domains::external_subagents::ExternalSubagentToolCapability;
pub use openbitfun_services_core::bounded_fs::{
    collect_bounded_regular_files, read_bounded_file, read_bounded_text, BoundedDirectoryWalkError,
    BoundedDirectoryWalkLimit, BoundedDirectoryWalkLimits, BoundedFileRead, BoundedTextRead,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

const MAX_MATCHER_BYTES: usize = 512;
const MAX_EVENT_NAME_BYTES: usize = 160;

const COMMON_EXTERNAL_SUBAGENT_TOOL_CAPABILITIES: &[(&str, ExternalSubagentToolCapability)] = &[
    ("ls", ExternalSubagentToolCapability::DirectoryList),
    ("list", ExternalSubagentToolCapability::DirectoryList),
    ("read", ExternalSubagentToolCapability::ReadFile),
    ("glob", ExternalSubagentToolCapability::GlobFiles),
    ("grep", ExternalSubagentToolCapability::SearchText),
    ("bash", ExternalSubagentToolCapability::ExecuteCommand),
    ("edit", ExternalSubagentToolCapability::EditFile),
    ("write", ExternalSubagentToolCapability::WriteFile),
];

/// Normalizes widely shared external Agent tool labels at the adapter boundary.
/// Provider-specific aliases remain owned by their ecosystem adapter.
pub fn common_external_subagent_tool_capability(
    name: &str,
) -> Option<ExternalSubagentToolCapability> {
    COMMON_EXTERNAL_SUBAGENT_TOOL_CAPABILITIES
        .iter()
        .find_map(|(candidate, capability)| {
            candidate.eq_ignore_ascii_case(name).then_some(*capability)
        })
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedStaticHookCommand {
    pub command: String,
    pub dependencies: Vec<ExternalHookImportDependencyV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticHookAssetError {
    DynamicPath,
    InvalidPath,
    MissingOrLinked,
    Unreadable,
    BudgetExceeded,
}

impl StaticHookAssetError {
    pub const fn skip_reason(self) -> &'static str {
        match self {
            Self::DynamicPath => "dynamic_source_path",
            Self::InvalidPath => "invalid_asset_path",
            Self::MissingOrLinked => "asset_missing_or_linked",
            Self::Unreadable => "asset_unreadable",
            Self::BudgetExceeded => "asset_budget_exceeded",
        }
    }
}

pub fn importable_hook_matcher(value: Option<&Value>) -> Result<Option<String>, &'static str> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        Some(Value::String(value))
            if value.len() <= MAX_MATCHER_BYTES && !value.chars().any(char::is_control) =>
        {
            Ok(Some(value.clone()))
        }
        Some(_) => Err("invalid_matcher"),
    }
}

pub fn required_hook_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<String, &'static str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or("invalid_handler")
}

pub fn optional_hook_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<String>, &'static str> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err("invalid_handler"),
    }
}

pub fn optional_positive_hook_u64(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<u64>, &'static str> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .filter(|value| *value > 0)
            .map(Some)
            .ok_or("invalid_handler"),
    }
}

/// Rewrites only simple shell tokens that point below `<source-dir>/hooks`.
/// Arbitrary shell syntax is intentionally left untouched; source-root tokens
/// with expansion or glob syntax fail closed instead of being guessed.
pub fn prepare_static_hook_command(
    command: &str,
    source_config_dir: &Path,
    source_dir_name: &str,
    assets: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<PreparedStaticHookCommand, StaticHookAssetError> {
    let normalized_source = source_dir_name.replace('\\', "/");
    let prefix = format!("{normalized_source}/hooks/");
    let dot_prefix = format!("./{prefix}");
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut dependencies = Vec::new();
    let mut dependency_keys = BTreeSet::new();
    let mut pending_assets = BTreeMap::<PathBuf, Vec<u8>>::new();
    let mut recognized_managed_path = false;

    for token in simple_command_tokens(command) {
        let normalized = token.value.replace('\\', "/");
        let managed_suffix = normalized
            .strip_prefix(&dot_prefix)
            .or_else(|| normalized.strip_prefix(&prefix));
        if let Some(suffix) = managed_suffix {
            recognized_managed_path = true;
            if suffix.is_empty()
                || suffix
                    .chars()
                    .any(|value| matches!(value, '$' | '`' | '*' | '?' | '[' | ']' | '{' | '}'))
            {
                return Err(StaticHookAssetError::DynamicPath);
            }
            let suffix_path = PathBuf::from(suffix);
            if suffix_path.is_absolute()
                || suffix_path
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err(StaticHookAssetError::InvalidPath);
            }
            let relative_path = PathBuf::from("hooks").join(&suffix_path);
            if relative_path.components().count() > MAX_EXTERNAL_HOOK_IMPORT_ASSET_DEPTH {
                return Err(StaticHookAssetError::BudgetExceeded);
            }
            if !assets.contains_key(&relative_path) && !pending_assets.contains_key(&relative_path)
            {
                let source_path = source_config_dir.join(&relative_path);
                let resolved = resolve_regular_file_without_links(&source_path, source_config_dir)?;
                let bytes = match read_bounded_file(&resolved, MAX_EXTERNAL_HOOK_IMPORT_ASSET_BYTES)
                {
                    Ok(BoundedFileRead::Content(bytes)) => bytes,
                    Ok(BoundedFileRead::TooLarge) => {
                        return Err(StaticHookAssetError::BudgetExceeded)
                    }
                    Err(_) => return Err(StaticHookAssetError::Unreadable),
                };
                pending_assets.insert(relative_path.clone(), bytes);
            }
            let relative_text = relative_path.to_string_lossy().replace('\\', "/");
            let dependency = ExternalHookImportDependencyV1::Managed {
                relative_path: relative_text.clone(),
            };
            if dependency_keys.insert(format!("managed:{relative_text}")) {
                dependencies.push(dependency);
            }
            replacements.push((
                token.start,
                token.end,
                format!("\"{MANAGED_HOOK_ROOT_PLACEHOLDER}/{relative_text}\""),
            ));
        } else if Path::new(&token.value).is_absolute() {
            let location = token.value.to_string();
            if dependency_keys.insert(format!("external:{location}")) {
                dependencies.push(ExternalHookImportDependencyV1::External { location });
            }
        }
    }
    if !recognized_managed_path && command.replace('\\', "/").contains(&prefix) {
        return Err(StaticHookAssetError::DynamicPath);
    }

    let file_count = assets.len().saturating_add(pending_assets.len());
    let byte_count = assets
        .values()
        .chain(pending_assets.values())
        .try_fold(0usize, |total, bytes| total.checked_add(bytes.len()))
        .ok_or(StaticHookAssetError::BudgetExceeded)?;
    if file_count > MAX_EXTERNAL_HOOK_IMPORT_ASSETS
        || byte_count > MAX_EXTERNAL_HOOK_IMPORT_TOTAL_ASSET_BYTES
    {
        return Err(StaticHookAssetError::BudgetExceeded);
    }

    let mut rewritten = command.to_string();
    for (start, end, replacement) in replacements.into_iter().rev() {
        rewritten.replace_range(start..end, &replacement);
    }
    assets.extend(pending_assets);
    Ok(PreparedStaticHookCommand {
        command: rewritten,
        dependencies,
    })
}

struct CommandToken<'a> {
    start: usize,
    end: usize,
    value: &'a str,
}

fn simple_command_tokens(command: &str) -> Vec<CommandToken<'_>> {
    let mut tokens = Vec::new();
    let bytes = command.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor == bytes.len() {
            break;
        }
        let start = cursor;
        let quote = matches!(bytes[cursor], b'\'' | b'\"').then_some(bytes[cursor]);
        if quote.is_some() {
            cursor += 1;
        }
        let value_start = cursor;
        while cursor < bytes.len()
            && match quote {
                Some(quote) => bytes[cursor] != quote,
                None => !bytes[cursor].is_ascii_whitespace(),
            }
        {
            cursor += 1;
        }
        let value_end = cursor;
        if quote.is_some() && cursor < bytes.len() {
            cursor += 1;
        }
        tokens.push(CommandToken {
            start,
            end: cursor,
            value: &command[value_start..value_end],
        });
    }
    tokens
}

fn resolve_regular_file_without_links(
    path: &Path,
    allowed_root: &Path,
) -> Result<PathBuf, StaticHookAssetError> {
    let relative = path
        .strip_prefix(allowed_root)
        .map_err(|_| StaticHookAssetError::InvalidPath)?;
    let mut current = allowed_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(StaticHookAssetError::InvalidPath);
        };
        current.push(component);
        let metadata = std::fs::symlink_metadata(&current)
            .map_err(|_| StaticHookAssetError::MissingOrLinked)?;
        if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
            return Err(StaticHookAssetError::MissingOrLinked);
        }
    }
    let metadata =
        std::fs::metadata(&current).map_err(|_| StaticHookAssetError::MissingOrLinked)?;
    if !metadata.is_file() {
        return Err(StaticHookAssetError::MissingOrLinked);
    }
    Ok(current)
}

#[cfg(windows)]
fn metadata_is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StaticHookVisitSummary {
    pub issues: Vec<StaticHookParseIssue>,
    pub all_disabled: bool,
    pub inspected_handlers: usize,
}

/// Borrowed structural facts for one handler during a bounded document walk.
///
/// This type intentionally has no `Debug` implementation: `group` and
/// `handler` may contain commands, environment values, or credentials.
pub struct StaticHookHandlerRef<'a> {
    pub native_event: &'a str,
    pub group: &'a serde_json::Map<String, Value>,
    pub handler: &'a Value,
    pub group_index: usize,
    pub handler_index: usize,
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

/// Walks Hook structure once and exposes handler bodies only to the supplied
/// callback. Returning `false` marks that handler invalid; no borrowed value is
/// retained in the returned summary.
pub fn visit_hook_document(
    bytes: &[u8],
    format: StaticHookDocumentFormat,
    max_handlers: usize,
    mut visitor: impl FnMut(StaticHookHandlerRef<'_>) -> bool,
) -> StaticHookVisitSummary {
    let parsed = match format {
        StaticHookDocumentFormat::Json => serde_json::from_slice::<Value>(bytes).ok(),
        StaticHookDocumentFormat::Toml => std::str::from_utf8(bytes)
            .ok()
            .and_then(|source| toml::from_str::<toml::Value>(source).ok())
            .and_then(|value| serde_json::to_value(value).ok()),
    };
    let Some(Value::Object(root)) = parsed else {
        return StaticHookVisitSummary {
            issues: vec![StaticHookParseIssue::DocumentInvalid],
            ..StaticHookVisitSummary::default()
        };
    };

    // This is only the Claude-compatible document flag. Other ecosystem
    // adapters ignore it; static discovery does not evaluate Codex activation.
    let all_disabled = root
        .get("disableAllHooks")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut result = StaticHookVisitSummary {
        all_disabled,
        ..StaticHookVisitSummary::default()
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
            record_visit_issue(&mut result, StaticHookParseIssue::EventNameInvalid);
            continue;
        }
        let Some(groups) = events.get(&native_event).and_then(Value::as_array) else {
            record_visit_issue(&mut result, StaticHookParseIssue::EventInvalid);
            continue;
        };
        for (group_index, group) in groups.iter().enumerate() {
            let Some(group) = group.as_object() else {
                record_visit_issue(&mut result, StaticHookParseIssue::GroupInvalid);
                continue;
            };
            let Some(handlers) = group.get("hooks").and_then(Value::as_array) else {
                record_visit_issue(&mut result, StaticHookParseIssue::GroupInvalid);
                continue;
            };
            for (handler_index, handler) in handlers.iter().enumerate() {
                if result.inspected_handlers >= max_handlers {
                    record_visit_issue(&mut result, StaticHookParseIssue::HandlerLimit);
                    break 'events;
                }
                result.inspected_handlers += 1;
                if !visitor(StaticHookHandlerRef {
                    native_event: &native_event,
                    group,
                    handler,
                    group_index,
                    handler_index,
                }) {
                    record_visit_issue(&mut result, StaticHookParseIssue::HandlerInvalid);
                }
            }
        }
    }
    result
}

/// Parses only Hook structure and returns redacted facts. Handler-specific
/// values are checked for presence but never copied into the result.
pub fn parse_hook_document(
    bytes: &[u8],
    format: StaticHookDocumentFormat,
    rules: &[StaticHookHandlerRule],
    max_handlers: usize,
) -> StaticHookParseResult {
    let mut handlers = Vec::new();
    let summary = visit_hook_document(bytes, format, max_handlers, |candidate| {
        let Some(fact) = static_hook_handler_fact(&candidate, rules) else {
            return false;
        };
        handlers.push(fact);
        true
    });
    StaticHookParseResult {
        handlers,
        issues: summary.issues,
        all_disabled: summary.all_disabled,
        inspected_handlers: summary.inspected_handlers,
    }
}

/// Converts one borrowed visit item to the same redacted fact used by the
/// compatibility parser. Import adapters use this to guard the public catalog
/// version without walking the document a second time.
pub fn static_hook_handler_fact(
    candidate: &StaticHookHandlerRef<'_>,
    rules: &[StaticHookHandlerRule],
) -> Option<StaticHookHandlerFact> {
    let handler_kind = parse_handler_kind(candidate.handler, rules)?;
    Some(StaticHookHandlerFact {
        native_event: candidate.native_event.to_string(),
        matcher: matcher_summary(candidate.group.get("matcher")),
        handler_kind,
        group_index: candidate.group_index,
        handler_index: candidate.handler_index,
    })
}

fn record_visit_issue(result: &mut StaticHookVisitSummary, issue: StaticHookParseIssue) {
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
    use openbitfun_product_domains::external_subagents::ExternalSubagentToolCapability;

    #[test]
    fn common_external_tool_names_map_without_source_specific_contract_behavior() {
        use ExternalSubagentToolCapability::{EditFile, ExecuteCommand, WriteFile};

        assert_eq!(
            common_external_subagent_tool_capability("bash"),
            Some(ExecuteCommand)
        );
        assert_eq!(
            common_external_subagent_tool_capability("EDIT"),
            Some(EditFile)
        );
        assert_eq!(
            common_external_subagent_tool_capability("write"),
            Some(WriteFile)
        );
        assert_eq!(
            common_external_subagent_tool_capability("provider-specific-tool"),
            None
        );
    }

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
