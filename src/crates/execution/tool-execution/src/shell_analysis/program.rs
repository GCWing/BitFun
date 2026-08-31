// Program recognizers retained from Core's legacy guard. Call ONLY on a parsed
// interpreter program, never on shell source or here-doc data for other consumers.
use regex::Regex;
use std::sync::OnceLock;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShellMutationOperation {
    Write,
    Delete,
}
#[derive(Debug)]
pub(super) struct ShellMutationTarget {
    pub path: String,
    pub operation: ShellMutationOperation,
}
pub(super) fn python_segment_may_mutate(segment: &str) -> bool {
    segment.contains("write_text")
        || segment.contains("write_bytes")
        || segment.contains(".unlink(")
        || segment.contains(".rename(")
        || segment.contains(".replace(")
        || (segment.contains("open(")
            && ["'w'", "\"w\"", "'a'", "\"a\"", "'x'", "\"x\""]
                .iter()
                .any(|mode| segment.contains(mode)))
}

pub(super) fn node_segment_may_mutate(segment: &str) -> bool {
    [
        "writefile",
        "appendfile",
        "unlink",
        "rmsync",
        "rename",
        "copyfile",
    ]
    .iter()
    .any(|operation| segment.contains(operation))
}

pub(super) fn push_python_mutation_targets(targets: &mut Vec<ShellMutationTarget>, segment: &str) {
    static OPEN_FOR_WRITE: OnceLock<Regex> = OnceLock::new();
    static PATH_WRITE: OnceLock<Regex> = OnceLock::new();
    static PATH_DELETE: OnceLock<Regex> = OnceLock::new();
    static PATH_MOVE: OnceLock<Regex> = OnceLock::new();
    let open_for_write = OPEN_FOR_WRITE.get_or_init(|| {
        Regex::new(r#"(?i)\bopen\s*\(\s*["']([^"']+)["']\s*,\s*["'][wax][^"']*["']"#)
            .expect("valid Python open-for-write regex")
    });
    let path_write = PATH_WRITE.get_or_init(|| {
        Regex::new(
            r#"(?i)\bPath\s*\(\s*["']([^"']+)["']\s*\)\s*\.\s*(?:write_text|write_bytes)\s*\("#,
        )
        .expect("valid pathlib write regex")
    });
    let path_delete = PATH_DELETE.get_or_init(|| {
        Regex::new(r#"(?i)\bPath\s*\(\s*["']([^"']+)["']\s*\)\s*\.\s*unlink\s*\("#)
            .expect("valid pathlib delete regex")
    });
    let path_move = PATH_MOVE.get_or_init(|| {
        Regex::new(
            r#"(?i)\bPath\s*\(\s*["']([^"']+)["']\s*\)\s*\.\s*(?:rename|replace)\s*\(\s*["']([^"']+)["']"#,
        )
        .expect("valid pathlib move regex")
    });

    for captures in open_for_write.captures_iter(segment) {
        if let Some(path) = captures.get(1) {
            push_bash_target(targets, path.as_str(), ShellMutationOperation::Write);
        }
    }
    for captures in path_write.captures_iter(segment) {
        if let Some(path) = captures.get(1) {
            push_bash_target(targets, path.as_str(), ShellMutationOperation::Write);
        }
    }
    for captures in path_delete.captures_iter(segment) {
        if let Some(path) = captures.get(1) {
            push_bash_target(targets, path.as_str(), ShellMutationOperation::Delete);
        }
    }
    for captures in path_move.captures_iter(segment) {
        if let Some(path) = captures.get(1) {
            push_bash_target(targets, path.as_str(), ShellMutationOperation::Delete);
        }
        if let Some(path) = captures.get(2) {
            push_bash_target(targets, path.as_str(), ShellMutationOperation::Write);
        }
    }
}

pub(super) fn push_node_mutation_targets(targets: &mut Vec<ShellMutationTarget>, segment: &str) {
    static SINGLE_PATH_WRITE: OnceLock<Regex> = OnceLock::new();
    static SINGLE_PATH_DELETE: OnceLock<Regex> = OnceLock::new();
    static TWO_PATH_COPY: OnceLock<Regex> = OnceLock::new();
    static TWO_PATH_MOVE: OnceLock<Regex> = OnceLock::new();
    let single_path_write = SINGLE_PATH_WRITE.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(?:fs\s*\.\s*)?(?:writefilesync|appendfilesync)\s*\(\s*["']([^"']+)["']"#,
        )
        .expect("valid Node single-path write regex")
    });
    let single_path_delete = SINGLE_PATH_DELETE.get_or_init(|| {
        Regex::new(r#"(?i)\b(?:fs\s*\.\s*)?(?:unlinksync|rmsync)\s*\(\s*["']([^"']+)["']"#)
            .expect("valid Node single-path delete regex")
    });
    let two_path_copy = TWO_PATH_COPY.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(?:fs\s*\.\s*)?copyfilesync\s*\(\s*["']([^"']+)["']\s*,\s*["']([^"']+)["']"#,
        )
        .expect("valid Node copy regex")
    });
    let two_path_move = TWO_PATH_MOVE.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(?:fs\s*\.\s*)?renamesync\s*\(\s*["']([^"']+)["']\s*,\s*["']([^"']+)["']"#,
        )
        .expect("valid Node move regex")
    });

    for captures in single_path_write.captures_iter(segment) {
        if let Some(path) = captures.get(1) {
            push_bash_target(targets, path.as_str(), ShellMutationOperation::Write);
        }
    }
    for captures in single_path_delete.captures_iter(segment) {
        if let Some(path) = captures.get(1) {
            push_bash_target(targets, path.as_str(), ShellMutationOperation::Delete);
        }
    }
    for captures in two_path_copy.captures_iter(segment) {
        if let Some(path) = captures.get(2) {
            push_bash_target(targets, path.as_str(), ShellMutationOperation::Write);
        }
    }
    for captures in two_path_move.captures_iter(segment) {
        if let Some(path) = captures.get(1) {
            push_bash_target(targets, path.as_str(), ShellMutationOperation::Delete);
        }
        if let Some(path) = captures.get(2) {
            push_bash_target(targets, path.as_str(), ShellMutationOperation::Write);
        }
    }
}

fn push_bash_target(
    targets: &mut Vec<ShellMutationTarget>,
    raw_path: &str,
    operation: ShellMutationOperation,
) {
    let path = raw_path.trim_matches(|c: char| matches!(c, '\'' | '"' | ','));
    if !path.is_empty()
        && !targets
            .iter()
            .any(|existing| existing.path == path && existing.operation == operation)
    {
        targets.push(ShellMutationTarget {
            path: path.to_string(),
            operation,
        });
    }
}
