//! Shared, runtime-free parser support for ecosystem Hook source adapters.

use bitfun_product_domains::external_hook_catalog::{
    ExternalHookHandlerKind, ExternalHookMatcherSummary,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

const MAX_MATCHER_BYTES: usize = 512;
const MAX_EVENT_NAME_BYTES: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedFileRead {
    Content(Vec<u8>),
    TooLarge,
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
}
