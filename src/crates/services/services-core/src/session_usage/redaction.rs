use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedLabel {
    pub value: String,
    pub redacted: bool,
}

pub fn redact_usage_label(input: &str, max_chars: usize) -> RedactedLabel {
    let mut value: String = input
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
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

pub fn redact_usage_input_summary(input: &str, max_chars: usize) -> RedactedLabel {
    let sanitized = redact_sensitive_input_fragments(input);
    let mut label = redact_usage_label(&sanitized, max_chars);
    label.redacted |= sanitized != input;
    label
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

fn redact_sensitive_input_fragments(input: &str) -> String {
    sensitive_input_patterns()
        .iter()
        .fold(input.to_string(), |value, pattern| {
            pattern.replace_all(&value, "${1}[redacted]").into_owned()
        })
}

fn sensitive_input_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r#"(?i)\b(authorization\s*:\s*(?:(?:bearer|basic)\s+)?)[^\s"'`]+"#,
            r#"(?i)\b(x-api-key\s*:\s*)[^\s"'`]+"#,
            r#"(?i)\b((?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|client[_-]?secret|secret|password|passwd|pwd|token)\s*=\s*)[^&\s"'`]+"#,
            r#"(?i)(--(?:api-key|access-token|refresh-token|id-token|client-secret|secret|password|token)\s+)[^&\s"'`]+"#,
        ]
        .into_iter()
        .map(|pattern| Regex::new(pattern).expect("valid usage input redaction pattern"))
        .collect()
    })
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
    fn redact_usage_input_summary_masks_common_secret_fragments() {
        let redacted = redact_usage_input_summary(
            "curl -H 'Authorization: Bearer sk-secret' https://api.example.test?token=abc&x=1 --api-key xyz",
            240,
        );

        assert!(redacted.redacted);
        assert_eq!(
            redacted.value,
            "curl -H 'Authorization: Bearer [redacted]' https://api.example.test?token=[redacted]&x=1 --api-key [redacted]"
        );
        assert!(!redacted.value.contains("sk-secret"));
        assert!(!redacted.value.contains("token=abc"));
        assert!(!redacted.value.contains("xyz"));
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
