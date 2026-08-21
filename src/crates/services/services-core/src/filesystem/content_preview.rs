//! Content-match preview primitives shared by every content search backend.
//!
//! The local walker (`tree.rs`) and the flashgrep-backed workspace search must
//! render the same preview shape, otherwise the same match looks different
//! depending on whether the workspace happens to be indexed. Both the pattern →
//! regex translation and the truncation budget therefore live here instead of
//! being duplicated per backend.

use super::error::{FileSystemError, FileSystemResult};
use regex::{Regex, RegexBuilder};

/// Total preview budget across the before/inside/after segments.
const MAX_PREVIEW_CHARS: usize = 250;
/// Budget for the text preceding the match; the rest is spent on the match and
/// its trailing context so that the match itself stays visible on screen.
const MAX_PREVIEW_BEFORE_CHARS: usize = 26;

/// Compiles a user-facing search pattern into the matcher used for previews.
///
/// The literal/whole-word translation is part of the contract: callers pass the
/// raw pattern plus the same flags they hand to their search backend, so the
/// preview highlights exactly what the backend matched.
pub fn compile_content_search_regex(
    pattern: &str,
    case_sensitive: bool,
    use_regex: bool,
    whole_word: bool,
) -> Result<Regex, regex::Error> {
    let search_pattern = if use_regex {
        pattern.to_string()
    } else if whole_word {
        format!(r"\b{}\b", regex::escape(pattern))
    } else {
        regex::escape(pattern)
    };

    RegexBuilder::new(&search_pattern)
        .case_insensitive(!case_sensitive)
        .build()
}

/// Splits a matched line into `(before, inside, after)` preview segments.
///
/// Returns `(None, None, None)` when the matcher does not match the line, which
/// happens whenever the caller's matcher is not the one that produced the line
/// (for example a daemon-side regex dialect the client cannot reproduce).
pub fn build_content_match_preview(
    line: &str,
    matcher: &Regex,
) -> (Option<String>, Option<String>, Option<String>) {
    let Some(found_match) = matcher.find(line) else {
        return (None, None, None);
    };

    let full_before = &line[..found_match.start()];
    let before = left_truncate_with_ellipsis(full_before, MAX_PREVIEW_BEFORE_CHARS);

    let mut chars_remaining = MAX_PREVIEW_CHARS.saturating_sub(before.chars().count());
    let mut inside = take_first_chars(found_match.as_str(), chars_remaining);
    chars_remaining = chars_remaining.saturating_sub(inside.chars().count());
    let after = take_first_chars(&line[found_match.end()..], chars_remaining);

    if inside.is_empty() {
        inside = found_match.as_str().to_string();
    }

    (Some(before), Some(inside), Some(after))
}

/// A compiled preview matcher.
///
/// Backends that receive matched lines from somewhere else (a search daemon, a
/// remote host) get the line text without any match offsets, so they recompute
/// the highlight locally. This wrapper keeps `regex` out of their dependency
/// surface: they hand over the same pattern and flags they searched with, then
/// ask for a preview per line.
pub struct ContentMatchPreviewBuilder {
    matcher: Regex,
}

impl ContentMatchPreviewBuilder {
    pub fn new(
        pattern: &str,
        case_sensitive: bool,
        use_regex: bool,
        whole_word: bool,
    ) -> FileSystemResult<Self> {
        let matcher = compile_content_search_regex(pattern, case_sensitive, use_regex, whole_word)
            .map_err(|error| {
                FileSystemError::service(format!("Invalid regex pattern: {}", error))
            })?;
        Ok(Self { matcher })
    }

    /// Splits `line` into `(before, inside, after)` preview segments.
    pub fn preview(&self, line: &str) -> (Option<String>, Option<String>, Option<String>) {
        build_content_match_preview(line, &self.matcher)
    }
}

fn take_first_chars(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let mut end_index = text.len();
    for (char_count, (byte_index, _)) in text.char_indices().enumerate() {
        if char_count == max_chars {
            end_index = byte_index;
            break;
        }
    }

    text[..end_index].to_string()
}

fn left_truncate_with_ellipsis(text: &str, max_chars: usize) -> String {
    let total_chars = text.chars().count();
    if total_chars <= max_chars {
        return text.to_string();
    }

    if max_chars <= 1 {
        return "\u{2026}".to_string();
    }

    let keep_chars = max_chars - 1;
    let start_index = text
        .char_indices()
        .nth(total_chars.saturating_sub(keep_chars))
        .map(|(index, _)| index)
        .unwrap_or(0);

    format!("\u{2026}{}", &text[start_index..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_pattern_is_escaped() {
        let matcher = compile_content_search_regex("a.c", true, false, false).expect("regex");
        assert!(matcher.is_match("a.c"));
        assert!(!matcher.is_match("abc"));
    }

    #[test]
    fn whole_word_literal_requires_boundaries() {
        let matcher = compile_content_search_regex("cat", true, false, true).expect("regex");
        assert!(matcher.is_match("a cat here"));
        assert!(!matcher.is_match("concatenate"));
    }

    #[test]
    fn case_insensitive_is_the_default_when_not_case_sensitive() {
        let matcher = compile_content_search_regex("Cat", false, false, false).expect("regex");
        assert!(matcher.is_match("CAT"));
    }

    #[test]
    fn preview_splits_line_around_the_match() {
        let matcher = compile_content_search_regex("needle", true, false, false).expect("regex");
        let (before, inside, after) = build_content_match_preview("a needle here", &matcher);
        assert_eq!(before.as_deref(), Some("a "));
        assert_eq!(inside.as_deref(), Some("needle"));
        assert_eq!(after.as_deref(), Some(" here"));
    }

    #[test]
    fn preview_is_empty_when_the_matcher_does_not_match() {
        let matcher = compile_content_search_regex("needle", true, false, false).expect("regex");
        assert_eq!(
            build_content_match_preview("nothing here", &matcher),
            (None, None, None)
        );
    }

    #[test]
    fn long_prefix_is_left_truncated_with_an_ellipsis() {
        let matcher = compile_content_search_regex("needle", true, false, false).expect("regex");
        let line = format!("{}needle", "x".repeat(200));
        let (before, inside, _) = build_content_match_preview(&line, &matcher);
        let before = before.expect("before segment");
        assert!(before.starts_with('\u{2026}'));
        assert_eq!(before.chars().count(), MAX_PREVIEW_BEFORE_CHARS);
        assert_eq!(inside.as_deref(), Some("needle"));
    }

    #[test]
    fn trailing_context_is_capped_by_the_total_budget() {
        let matcher = compile_content_search_regex("needle", true, false, false).expect("regex");
        let line = format!("needle{}", "y".repeat(1000));
        let (before, inside, after) = build_content_match_preview(&line, &matcher);
        let total = before.unwrap().chars().count()
            + inside.unwrap().chars().count()
            + after.unwrap().chars().count();
        assert_eq!(total, MAX_PREVIEW_CHARS);
    }

    #[test]
    fn multibyte_prefix_truncation_keeps_char_boundaries() {
        let matcher = compile_content_search_regex("needle", true, false, false).expect("regex");
        let line = format!("{}needle", "中".repeat(100));
        let (before, inside, _) = build_content_match_preview(&line, &matcher);
        let before = before.expect("before segment");
        assert_eq!(before.chars().count(), MAX_PREVIEW_BEFORE_CHARS);
        assert!(before.chars().skip(1).all(|character| character == '中'));
        assert_eq!(inside.as_deref(), Some("needle"));
    }

    #[test]
    fn zero_width_match_falls_back_to_the_matched_text() {
        let matcher = compile_content_search_regex("x*", true, true, false).expect("regex");
        let (_, inside, _) = build_content_match_preview("abc", &matcher);
        assert_eq!(inside.as_deref(), Some(""));
    }
}
