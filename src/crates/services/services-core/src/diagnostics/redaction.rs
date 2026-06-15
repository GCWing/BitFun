use regex::{Captures, Regex};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedDiagnosticLog {
    /// Redacted diagnostic log text.
    pub text: String,
    /// Number of replacement operations applied while redacting the text.
    pub redaction_count: usize,
}

/// Redact sensitive values from diagnostic log text and return the text only.
///
/// This function is intentionally line-oriented so callers can use the same
/// project-level redaction rules for large local log exports without parsing
/// the entire log as one structured payload.
pub fn redact_diagnostic_log_text(input: &str) -> String {
    redact_diagnostic_log_text_with_report(input).text
}

/// Redact sensitive values from diagnostic log text and include a replacement count.
pub fn redact_diagnostic_log_text_with_report(input: &str) -> RedactedDiagnosticLog {
    let mut text = String::with_capacity(input.len());
    let mut redaction_count = 0;

    for segment in input.split_inclusive('\n') {
        let (redacted, count) = redact_diagnostic_log_segment(segment);
        text.push_str(&redacted);
        redaction_count += count;
    }

    RedactedDiagnosticLog {
        text,
        redaction_count,
    }
}

fn redact_diagnostic_log_segment(segment: &str) -> (String, usize) {
    let (segment, quoted_count) = redact_quoted_sensitive_values(segment);
    let (segment, bearer_count) = redact_bearer_tokens(&segment);
    let (segment, token_count) = redact_secret_tokens(&segment);
    let (segment, bare_count) = redact_bare_sensitive_values(&segment);
    let (segment, path_count) = redact_absolute_paths(&segment);

    (
        segment,
        quoted_count + bearer_count + token_count + bare_count + path_count,
    )
}

fn redact_quoted_sensitive_values(input: &str) -> (String, usize) {
    let mut count = 0;
    let output = sensitive_quoted_value_re()
        .replace_all(input, |captures: &Captures<'_>| {
            count += 1;
            let prefix = captures.name("prefix").map_or("", |m| m.as_str());
            let value = captures.name("value").map_or("", |m| m.as_str());
            let quote = value.chars().next().unwrap_or('"');
            let value_chars = value.chars().count().saturating_sub(2);
            format!("{prefix}{quote}<redacted chars={value_chars}>{quote}")
        })
        .into_owned();

    (output, count)
}

fn redact_bare_sensitive_values(input: &str) -> (String, usize) {
    let mut count = 0;
    let output = sensitive_bare_value_re()
        .replace_all(input, |captures: &Captures<'_>| {
            let value = captures.name("value").map_or("", |m| m.as_str());
            if value.starts_with('{') || value.starts_with('[') {
                return captures.get(0).map_or("", |m| m.as_str()).to_string();
            }

            count += 1;
            let prefix = captures.name("prefix").map_or("", |m| m.as_str());
            format!("{prefix}<redacted chars={}>", value.chars().count())
        })
        .into_owned();

    (output, count)
}

fn redact_bearer_tokens(input: &str) -> (String, usize) {
    replace_all_count(input, bearer_token_re(), "Bearer <redacted>")
}

fn redact_secret_tokens(input: &str) -> (String, usize) {
    replace_all_count(input, secret_token_re(), "<redacted token>")
}

fn redact_absolute_paths(input: &str) -> (String, usize) {
    let (input, windows_escaped_count) =
        replace_all_count(input, windows_escaped_path_re(), "<redacted path>");
    let (input, windows_count) = replace_all_count(&input, windows_path_re(), "<redacted path>");
    let (input, unix_count) = replace_all_count(&input, unix_path_re(), "<redacted path>");

    (input, windows_escaped_count + windows_count + unix_count)
}

fn replace_all_count(input: &str, regex: &Regex, replacement: &str) -> (String, usize) {
    let mut count = 0;
    let output = regex
        .replace_all(input, |_captures: &Captures<'_>| {
            count += 1;
            replacement.to_string()
        })
        .into_owned();

    (output, count)
}

fn sensitive_quoted_value_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(
            r#"(?i)(?P<prefix>(?:"(?:{keys})"|'(?:{keys})'|(?:{keys}))\s*[:=]\s*(?:Some\()?)(?P<value>"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')"#,
            keys = sensitive_key_pattern(),
        ))
        .expect("sensitive quoted value regex must compile")
    })
}

fn sensitive_bare_value_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(
            r#"(?i)(?P<prefix>\b(?:{keys})\b\s*[:=]\s*)(?P<value>[^\s,}})]+)"#,
            keys = sensitive_key_pattern(),
        ))
        .expect("sensitive bare value regex must compile")
    })
}

fn bearer_token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]{8,}"#)
            .expect("bearer token regex must compile")
    })
}

fn secret_token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"\b(?:sk|sk-ant|sk-proj|ghp|gho|github_pat)_[A-Za-z0-9_\-]{8,}|\bsk-[A-Za-z0-9_\-]{8,}"#)
            .expect("secret token regex must compile")
    })
}

fn windows_escaped_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"\b[A-Za-z]:\\\\[^"'\r\n,}\]]+"#)
            .expect("escaped Windows path regex must compile")
    })
}

fn windows_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"\b[A-Za-z]:\\[^"'\r\n,}\]]+"#).expect("Windows path regex must compile")
    })
}

fn unix_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"\b/(?:Users|home|workspace|tmp|var|private)/[^"'\s,}\]]+"#)
            .expect("Unix path regex must compile")
    })
}

fn sensitive_key_pattern() -> &'static str {
    r#"api[_-]?key|apikey|authorization|x-api-key|token|access[_-]?token|refresh[_-]?token|session[_-]?key|password|secret|prompt|system_prompt|original_prompt|suggested_prompt|copyable_prompt|content|text|partial_json|arguments|payload|raw_message|rawMessage|raw_error|outer_html|text_content|command|path|file|files|data"#
}
