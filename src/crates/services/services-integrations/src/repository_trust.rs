//! Feature-light Git repository ownership trust contract.
//!
//! Both the full Git service and Review Platform execute Git commands, but
//! Review Platform deliberately remains independently compilable without the
//! heavier `git` feature. Keep the ownership diagnostic classification and
//! stable boundary code here so those two capability slices cannot drift.

/// Ownership rejection wordings emitted by the Git CLI and by libgit2.
const OWNERSHIP_REJECTION_MARKERS: [&str; 5] = [
    "dubious ownership",
    "is owned by someone else",
    "is not owned by current user",
    "not owned by current user",
    "owned by a different user",
];

fn contains_any(message: &str, markers: &[&str]) -> bool {
    let lowered = message.to_lowercase();
    markers.iter().any(|marker| lowered.contains(marker))
}

/// Whether a Git/libgit2 diagnostic is an ownership rejection.
pub(crate) fn is_untrusted_repository_message(message: &str) -> bool {
    contains_any(message, &OWNERSHIP_REJECTION_MARKERS)
}

/// True when the value carries a shape only Windows produces: a `\\server\share`
/// or `\\?\...` prefix, or a `C:\` / `C:/` drive root.
fn looks_like_windows_path(value: &str) -> bool {
    if value.starts_with('\\') {
        return true;
    }
    let mut chars = value.chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(drive), Some(':'), Some('\\' | '/')) if drive.is_ascii_alphabetic()
    )
}

/// Normalizes a repository path into the shape Git compares `safe.directory`
/// entries against: forward slashes, no extended-length prefix, no trailing
/// separator.
pub(crate) fn normalize_trust_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches('\'').trim_matches('"').trim();
    if trimmed.is_empty() {
        return None;
    }

    // Only a Windows-shaped path may have its backslashes rewritten. Backslash
    // is an ordinary filename character on POSIX.
    let mut value = if looks_like_windows_path(trimmed) {
        trimmed.replace('\\', "/")
    } else {
        trimmed.to_string()
    };
    if value
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("//?/UNC/"))
    {
        value = format!("//{}", &value[8..]);
    } else if let Some(stripped) = value.strip_prefix("//?/") {
        value = stripped.to_string();
    }
    while value.len() > 1 && value.ends_with('/') && !value.ends_with(":/") {
        value.pop();
    }

    (!value.is_empty()).then_some(value)
}

/// Extracts the repository path Git named in an ownership rejection.
pub(crate) fn untrusted_repository_path_from_message(message: &str) -> Option<String> {
    if !is_untrusted_repository_message(message) {
        return None;
    }

    message
        .lines()
        .filter(|line| contains_any(line, &OWNERSHIP_REJECTION_MARKERS))
        .find_map(quoted_path_on_line)
        .or_else(|| message.lines().find_map(quoted_path_on_line))
}

fn quoted_path_on_line(line: &str) -> Option<String> {
    for quote in ['\'', '"'] {
        let Some(open) = line.find(quote) else {
            continue;
        };
        let start = open + quote.len_utf8();
        let Some(end) = line.rfind(quote) else {
            continue;
        };
        if end < start {
            continue;
        }
        if let Some(path) = normalize_trust_path(&line[start..end]) {
            return Some(path);
        }
    }
    None
}

/// Stable prefix used across string-only Desktop and JSON-RPC boundaries.
pub(crate) const REPOSITORY_UNTRUSTED_ERROR_PREFIX: &str = "git_repository_untrusted:";

/// Boundary error string for an ownership rejection.
pub(crate) fn untrusted_repository_error_message(repository_path: &str) -> String {
    format!("{REPOSITORY_UNTRUSTED_ERROR_PREFIX} {repository_path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_cli_ownership_rejection_and_preserves_the_path() {
        let message = concat!(
            "fatal: detected dubious ownership in repository at '/srv/shared/repo'\n",
            "To add an exception for this directory, call:\n",
            "git config --global --add safe.directory /srv/shared/repo",
        );

        assert!(is_untrusted_repository_message(message));
        assert_eq!(
            untrusted_repository_path_from_message(message).as_deref(),
            Some("/srv/shared/repo")
        );
    }

    #[test]
    fn normalizes_windows_paths_without_rewriting_posix_backslashes() {
        assert_eq!(
            normalize_trust_path("\\\\?\\C:\\work\\repo\\").as_deref(),
            Some("C:/work/repo")
        );
        assert_eq!(
            normalize_trust_path("/srv/we\\ird/repo").as_deref(),
            Some("/srv/we\\ird/repo")
        );
    }
}
