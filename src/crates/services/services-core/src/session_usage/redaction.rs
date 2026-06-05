#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedLabel {
    pub value: String,
    pub redacted: bool,
}

pub fn redact_usage_label(input: &str, max_chars: usize) -> RedactedLabel {
    let mut value: String = input
        .chars()
        .filter_map(|ch| if ch.is_control() { Some(' ') } else { Some(ch) })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let mut redacted = value != input;
    if max_chars == 0 {
        return RedactedLabel {
            value: String::new(),
            redacted: true,
        };
    }

    if value.chars().count() > max_chars {
        redacted = true;
        value = truncate_chars(&value, max_chars);
    }

    RedactedLabel { value, redacted }
}

pub fn display_workspace_relative_path(
    workspace_root: Option<&str>,
    raw_path: &str,
) -> RedactedLabel {
    let normalized_raw = normalize_path(raw_path);
    if let Some(root) = workspace_root {
        let normalized_root = normalize_path(root);
        let root_with_sep = format!("{}/", normalized_root.trim_end_matches('/'));
        let raw_lower = normalized_raw.to_lowercase();
        let root_lower = root_with_sep.to_lowercase();

        if raw_lower.starts_with(&root_lower) {
            let relative = normalized_raw[root_with_sep.len()..].trim_start_matches('/');
            return redact_usage_label(relative, 120);
        }
    }

    RedactedLabel {
        value: "redacted path".to_string(),
        redacted: true,
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }

    let keep = max_chars - 3;
    let mut out: String = input.chars().take(keep).collect();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_usage_label_strips_controls_and_bounds_length() {
        let redacted = redact_usage_label("private\npath\twith-control", 12);

        assert!(redacted.redacted);
        assert_eq!(redacted.value.len(), 12);
        assert!(!redacted.value.contains('\n'));
        assert!(!redacted.value.contains('\t'));
    }

    #[test]
    fn display_workspace_relative_path_keeps_workspace_relative_label() {
        let label = display_workspace_relative_path(
            Some("D:/workspace/bitfun"),
            "D:/workspace/bitfun/src/main.rs",
        );

        assert!(!label.redacted);
        assert_eq!(label.value, "src/main.rs");
    }
}
