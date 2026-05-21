/*!
 * Function Agents Common Module
 *
 * Shared types, errors, and utilities for function agents
 */

use serde::{Deserialize, Serialize};
use std::fmt;

// ==================== Shared Types ====================

/// Language selection for agent outputs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Language {
    Chinese,
    English,
}

impl Language {
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Chinese => "Chinese",
            Language::English => "English",
        }
    }
}

// ==================== Shared Error Types ====================

/// Error types for function agents
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentErrorType {
    GitError,
    AnalysisError,
    InvalidInput,
    InternalError,
}

/// Error struct for function agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentError {
    pub message: String,
    pub error_type: AgentErrorType,
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}", self.error_type, self.message)
    }
}

impl std::error::Error for AgentError {}

impl AgentError {
    pub fn git_error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            error_type: AgentErrorType::GitError,
        }
    }

    pub fn analysis_error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            error_type: AgentErrorType::AnalysisError,
        }
    }

    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            error_type: AgentErrorType::InvalidInput,
        }
    }

    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            error_type: AgentErrorType::InternalError,
        }
    }
}

/// Result type for function agents
pub type AgentResult<T> = Result<T, AgentError>;

pub(crate) fn extract_json_from_ai_response(response: &str) -> Option<String> {
    let trimmed = response.trim();

    if trimmed.is_empty() {
        return None;
    }

    let mut candidates = Vec::new();

    if trimmed.starts_with('{') {
        candidates.push(trimmed.to_string());
    }

    if let Some(extracted) = extract_from_code_block(trimmed) {
        candidates.push(extracted);
    }

    if let Some(extracted) = extract_from_zhipu_box(trimmed) {
        candidates.push(extracted);
    }

    if let Some(extracted) = extract_greedy_braces(trimmed) {
        candidates.push(extracted);
    }

    for candidate in &candidates {
        if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
            return Some(candidate.clone());
        }
    }

    for candidate in &candidates {
        if let Some(repaired) = try_repair_json(candidate) {
            log::debug!(
                "Function-agent JSON repair succeeded (original length={}, repaired length={})",
                candidate.len(),
                repaired.len()
            );
            return Some(repaired);
        }
    }

    log::warn!(
        "Cannot extract valid function-agent JSON from AI response (length={}). Preview: {}",
        response.len(),
        safe_preview(trimmed, 300),
    );
    None
}

fn extract_from_code_block(text: &str) -> Option<String> {
    let start_markers = ["```json\n", "```json\r\n", "```\n", "```\r\n"];

    for marker in &start_markers {
        if let Some(start_idx) = text.find(marker) {
            let content_start = start_idx + marker.len();
            if let Some(end_offset) = text[content_start..].find("```") {
                let json_str = text[content_start..content_start + end_offset].trim();
                if !json_str.is_empty() {
                    return Some(json_str.to_string());
                }
            }
        }
    }
    None
}

fn extract_from_zhipu_box(text: &str) -> Option<String> {
    let begin_tag = "<|begin_of_box|>";
    let end_tag = "<|end_of_box|>";
    if let Some(start_idx) = text.find(begin_tag) {
        let content_start = start_idx + begin_tag.len();
        if let Some(end_offset) = text[content_start..].find(end_tag) {
            let json_str = text[content_start..content_start + end_offset].trim();
            if !json_str.is_empty() {
                return Some(json_str.to_string());
            }
        }
    }
    None
}

fn extract_greedy_braces(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end > start {
        Some(text[start..=end].to_string())
    } else {
        None
    }
}

fn try_repair_json(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }

    let mut out = String::with_capacity(trimmed.len() + 64);
    let chars: Vec<char> = trimmed.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        let ch = chars[i];

        if !in_string {
            out.push(ch);
            if ch == '"' {
                in_string = true;
            }
            i += 1;
            continue;
        }

        if ch == '\\' {
            out.push(ch);
            if i + 1 < len {
                out.push(chars[i + 1]);
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }

        if ch == '"' {
            let next_significant = next_non_whitespace(&chars, i + 1);
            if is_structural_follower(next_significant) {
                out.push('"');
                in_string = false;
            } else {
                out.push('\\');
                out.push('"');
            }
            i += 1;
            continue;
        }

        out.push(ch);
        i += 1;
    }

    if serde_json::from_str::<serde_json::Value>(&out).is_ok() {
        Some(out)
    } else {
        None
    }
}

fn next_non_whitespace(chars: &[char], start: usize) -> Option<char> {
    chars[start..]
        .iter()
        .find(|c| !c.is_ascii_whitespace())
        .copied()
}

fn is_structural_follower(ch: Option<char>) -> bool {
    matches!(ch, None | Some(',' | '}' | ']' | ':'))
}

fn safe_preview(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::extract_json_from_ai_response;

    #[test]
    fn extracts_json_from_common_ai_response_wrappers() {
        assert_eq!(
            extract_json_from_ai_response(r#"{"key":"value"}"#),
            Some(r#"{"key":"value"}"#.to_string())
        );
        assert_eq!(
            extract_json_from_ai_response("```json\n{\"key\":\"value\"}\n```"),
            Some(r#"{"key":"value"}"#.to_string())
        );
        assert_eq!(
            extract_json_from_ai_response("<|begin_of_box|>{\"key\":\"value\"}<|end_of_box|>"),
            Some(r#"{"key":"value"}"#.to_string())
        );
        assert_eq!(
            extract_json_from_ai_response("prefix {\"key\":\"value\"} suffix"),
            Some(r#"{"key":"value"}"#.to_string())
        );
    }

    #[test]
    fn repairs_unescaped_quotes_inside_json_strings() {
        let repaired =
            extract_json_from_ai_response(r#"{"text":"before "inner" after","ok":true}"#)
                .expect("repair should produce valid JSON");
        let parsed: serde_json::Value = serde_json::from_str(&repaired).unwrap();

        assert_eq!(parsed["text"].as_str(), Some("before \"inner\" after"));
        assert_eq!(parsed["ok"].as_bool(), Some(true));
    }

    #[test]
    fn rejects_missing_or_invalid_json() {
        assert_eq!(extract_json_from_ai_response(""), None);
        assert_eq!(extract_json_from_ai_response("plain text"), None);
        assert_eq!(
            extract_json_from_ai_response("```json\n{\"key\":\"value\"\n```"),
            None
        );
    }
}
