//! Pure policy helpers for oversized tool-result storage.
//!
//! This module intentionally does not write files or know where session runtime
//! artifacts live. Product runtimes provide the actual storage binding.

pub const DEFAULT_MAX_TOOL_RESULT_CHARS: usize = 50_000;
pub const MAX_TOOL_RESULTS_PER_ROUND_CHARS: usize = 200_000;
pub const TOOL_RESULT_PREVIEW_CHARS: usize = 2_000;
pub const PERSISTED_OUTPUT_TAG: &str = "<persisted-output>";
pub const PERSISTED_OUTPUT_CLOSING_TAG: &str = "</persisted-output>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolResultStoragePolicy {
    pub per_tool_limit_chars: usize,
    pub per_round_limit_chars: usize,
    pub preview_chars: usize,
}

impl Default for ToolResultStoragePolicy {
    fn default() -> Self {
        Self {
            per_tool_limit_chars: DEFAULT_MAX_TOOL_RESULT_CHARS,
            per_round_limit_chars: MAX_TOOL_RESULTS_PER_ROUND_CHARS,
            preview_chars: TOOL_RESULT_PREVIEW_CHARS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedToolOutput {
    pub reference: String,
    pub original_chars: usize,
    pub line_count: usize,
    pub preview: String,
    pub has_more: bool,
    pub metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolResultPersistenceCandidate {
    pub index: usize,
    pub visible_chars: usize,
}

pub fn select_tool_result_indices_for_persistence(
    candidates: &[ToolResultPersistenceCandidate],
    total_visible_chars: usize,
    limit: usize,
) -> Vec<usize> {
    let mut sorted = candidates.to_vec();
    sorted.sort_by(|a, b| b.visible_chars.cmp(&a.visible_chars));

    let mut selected = Vec::new();
    let mut remaining = total_visible_chars;
    for candidate in sorted {
        if remaining <= limit {
            break;
        }
        selected.push(candidate.index);
        remaining = remaining.saturating_sub(candidate.visible_chars);
    }
    selected
}

pub fn sanitize_tool_result_file_component(value: &str, empty_fallback: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();

    if sanitized.is_empty() {
        empty_fallback.to_string()
    } else {
        sanitized
    }
}

pub fn generate_tool_result_preview(content: &str, max_chars: usize) -> (String, bool) {
    if content.chars().count() <= max_chars {
        return (content.to_string(), false);
    }

    let prefix = content.chars().take(max_chars).collect::<String>();
    let cut_point = prefix
        .char_indices()
        .filter_map(|(idx, ch)| (ch == '\n').then_some(idx))
        .next_back()
        .filter(|idx| *idx > prefix.len() / 2)
        .unwrap_or(prefix.len());

    (prefix[..cut_point].to_string(), true)
}

pub fn count_tool_result_lines(content: &str) -> usize {
    if content.is_empty() {
        0
    } else {
        content.lines().count()
    }
}

pub fn tool_result_is_persisted_output(text: &str) -> bool {
    text.starts_with(PERSISTED_OUTPUT_TAG)
}

pub fn build_persisted_tool_output_message(
    result: &PersistedToolOutput,
    preview_chars: usize,
) -> String {
    let mut message = format!(
        "{PERSISTED_OUTPUT_TAG}\nOutput too large ({} chars). Full output saved to: {}\nLine count: {}\n\nPreview (first {} chars):\n{}",
        result.original_chars, result.reference, result.line_count, preview_chars, result.preview
    );
    if result.has_more {
        message.push_str("\n...\n");
    } else {
        message.push('\n');
    }
    if !result.metadata.is_empty() {
        message.push_str("\nMetadata:\n");
        for (key, value) in &result.metadata {
            message.push_str(&format!("- {}: {}\n", key, value));
        }
    }
    message.push_str(PERSISTED_OUTPUT_CLOSING_TAG);
    message
}
