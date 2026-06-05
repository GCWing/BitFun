//! Robust JSON extraction from AI model responses.
//!
//! AI models often wrap JSON in markdown code blocks (`` ```json ... ``` ``),
//! or include leading/trailing prose. This module provides a single public
//! helper that handles all common formats and falls back gracefully.
//!
//! When the extracted text is not valid JSON (e.g. the model emitted unescaped
//! quotes inside string values), a best-effort repair pass is attempted before
//! giving up.

use log::{debug, warn};

/// Extract a JSON object string from an AI response.
///
/// Tries the following strategies in order:
/// 1. Raw JSON — response starts with `{` after trimming.
/// 2. Markdown code block — `` ```json\n...\n``` `` or `` ```\n...\n``` ``.
/// 3. Zhipu AI box format — `<|begin_of_box|>...<|end_of_box|>`.
/// 4. Greedy brace match — first `{` to last `}`.
///
/// Each candidate is validated with `serde_json::from_str`.  If validation
/// fails, a repair pass ([`try_repair_json`]) is attempted before moving on.
pub fn extract_json_from_ai_response(response: &str) -> Option<String> {
    let trimmed = response.trim();

    if trimmed.is_empty() {
        return None;
    }

    // Collect candidates from the various extraction strategies.
    let mut candidates: Vec<String> = Vec::new();

    // Strategy 1: raw JSON object
    if trimmed.starts_with('{') {
        candidates.push(trimmed.to_string());
    }

    // Strategy 2: markdown code blocks (```json ... ``` or ``` ... ```)
    if let Some(extracted) = extract_from_code_block(trimmed) {
        candidates.push(extracted);
    }

    // Strategy 3: Zhipu AI box format
    if let Some(extracted) = extract_from_zhipu_box(trimmed) {
        candidates.push(extracted);
    }

    // Strategy 4: greedy first-`{` to last-`}`
    if let Some(extracted) = extract_greedy_braces(trimmed) {
        candidates.push(extracted);
    }

    // First pass: try each candidate as-is.
    for candidate in &candidates {
        if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
            return Some(candidate.clone());
        }
    }

    // Second pass: attempt repair on each candidate.
    for candidate in &candidates {
        if let Some(repaired) = try_repair_json(candidate) {
            debug!(
                "JSON repair succeeded (original length={}, repaired length={})",
                candidate.len(),
                repaired.len()
            );
            return Some(repaired);
        }
    }

    warn!(
        "Cannot extract valid JSON from AI response (length={}). Preview: {}",
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

/// Best-effort repair of malformed JSON produced by AI models.
///
/// Common breakage: the model writes unescaped `"` inside string values
/// (e.g. Chinese text like `"你到底是什么模型"` where the inner quotes are
/// plain ASCII U+0022).  This function walks the JSON character-by-character,
/// tracking brace/bracket depth and string state, and escapes interior quotes
/// that would otherwise break the parse.
fn try_repair_json(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }

    let mut out = String::with_capacity(trimmed.len() + 64);
    let chars: Vec<char> = trimmed.chars().collect();
    let len = chars.len();
    let mut i = 0;

    // We track whether we are inside a JSON string that was opened by a
    // *structural* quote (i.e. one that is part of JSON syntax).
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

        // We are inside a string.
        if ch == '\\' {
            // Escape sequence — copy verbatim.
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
            // Is this the *closing* structural quote, or a rogue interior quote?
            // Heuristic: look at what follows the quote (skipping whitespace).
            // A structural close-quote is followed by `,` `}` `]` `:` or EOF.
            let next_significant = next_non_whitespace(&chars, i + 1);
            if is_structural_follower(next_significant) {
                // Structural close.
                out.push('"');
                in_string = false;
            } else {
                // Rogue interior quote — escape it.
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

/// Characters that legitimately follow a closing `"` in JSON.
fn is_structural_follower(ch: Option<char>) -> bool {
    match ch {
        None => true, // EOF
        Some(',' | '}' | ']' | ':') => true,
        _ => false,
    }
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
    use super::*;

    #[test]
    fn raw_json_object() {
        let input = r#"{"key": "value"}"#;
        assert_eq!(
            extract_json_from_ai_response(input),
            Some(input.to_string())
        );
    }

    #[test]
    fn json_in_code_block() {
        let input = "```json\n{\"key\": \"value\"}\n```";
        assert_eq!(
            extract_json_from_ai_response(input),
            Some(r#"{"key": "value"}"#.to_string())
        );
    }

    #[test]
    fn json_in_plain_code_block() {
        let input = "```\n{\"key\": \"value\"}\n```";
        assert_eq!(
            extract_json_from_ai_response(input),
            Some(r#"{"key": "value"}"#.to_string())
        );
    }

    #[test]
    fn json_with_leading_prose() {
        let input = "Here is the result:\n{\"key\": \"value\"}";
        assert_eq!(
            extract_json_from_ai_response(input),
            Some(r#"{"key": "value"}"#.to_string())
        );
    }

    #[test]
    fn json_with_trailing_prose() {
        let input = "{\"key\": \"value\"}\nHope this helps!";
        assert_eq!(
            extract_json_from_ai_response(input),
            Some(r#"{"key": "value"}"#.to_string())
        );
    }

    #[test]
    fn zhipu_box_format() {
        let input = "<|begin_of_box|>{\"key\": \"value\"}<|end_of_box|>";
        assert_eq!(
            extract_json_from_ai_response(input),
            Some(r#"{"key": "value"}"#.to_string())
        );
    }

    #[test]
    fn nested_json_with_arrays() {
        let input = "```json\n{\"items\": [{\"name\": \"a\"}, {\"name\": \"b\"}]}\n```";
        assert_eq!(
            extract_json_from_ai_response(input),
            Some(r#"{"items": [{"name": "a"}, {"name": "b"}]}"#.to_string())
        );
    }

    #[test]
    fn multiline_json_in_code_block() {
        let input = r#"```json
{
  "narrative": "Hello **world**.\n\nSecond paragraph.",
  "key_patterns": ["pattern1", "pattern2"]
}
```"#;
        let result = extract_json_from_ai_response(input);
        assert!(result.is_some());
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed["key_patterns"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn json_with_chinese_quotes_in_values() {
        let input = "```json\n{\"text\": \"用户询问\\\"模型是什么\\\"的问题\"}\n```";
        let result = extract_json_from_ai_response(input);
        assert!(result.is_some());
    }

    #[test]
    fn empty_input_returns_none() {
        assert_eq!(extract_json_from_ai_response(""), None);
        assert_eq!(extract_json_from_ai_response("   "), None);
    }

    #[test]
    fn no_json_returns_none() {
        assert_eq!(
            extract_json_from_ai_response("This is just plain text."),
            None
        );
    }

    #[test]
    fn invalid_json_in_code_block_falls_through() {
        let input = "```json\n{\"key\": \"value\"\n```";
        // Missing closing brace — code block extraction finds it but validation fails.
        // Greedy brace match also finds `{...}` but it's still invalid.
        assert_eq!(extract_json_from_ai_response(input), None);
    }

    #[test]
    fn greedy_brace_fallback() {
        let input = "Some text before {\"ok\": true} and after";
        assert_eq!(
            extract_json_from_ai_response(input),
            Some(r#"{"ok": true}"#.to_string())
        );
    }

    #[test]
    fn code_block_with_crlf() {
        let input = "```json\r\n{\"key\": \"value\"}\r\n```";
        assert_eq!(
            extract_json_from_ai_response(input),
            Some(r#"{"key": "value"}"#.to_string())
        );
    }

    // ── Repair: unescaped interior quotes ──

    #[test]
    fn repair_unescaped_chinese_style_quotes() {
        // AI writes: "headline": "用户问AI"你是什么模型"" — inner quotes are ASCII U+0022
        let input =
            "```json\n{\"headline\": \"用户问AI\"你是什么模型\"\", \"detail\": \"ok\"}\n```";
        let result = extract_json_from_ai_response(input);
        assert!(result.is_some(), "repair should succeed");
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed["headline"]
            .as_str()
            .unwrap()
            .contains("你是什么模型"));
        assert_eq!(parsed["detail"].as_str().unwrap(), "ok");
    }

    #[test]
    fn repair_multiple_rogue_quotes_in_one_value() {
        // "text": "他说"你好"然后又说"再见""
        let input = r#"{"text": "他说"你好"然后又说"再见"", "other": "fine"}"#;
        let result = extract_json_from_ai_response(input);
        assert!(
            result.is_some(),
            "repair should handle multiple rogue quotes"
        );
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed["text"].as_str().unwrap().contains("你好"));
        assert!(parsed["text"].as_str().unwrap().contains("再见"));
    }

    #[test]
    fn repair_rogue_quotes_in_array_values() {
        let input = r#"{"items": ["他说"你好"", "正常值"]}"#;
        let result = extract_json_from_ai_response(input);
        assert!(result.is_some());
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed["items"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn repair_preserves_already_escaped_quotes() {
        let input = r#"{"text": "properly \"escaped\" quotes"}"#;
        let result = extract_json_from_ai_response(input);
        assert!(result.is_some());
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed["text"].as_str().unwrap().contains("escaped"));
    }

    #[test]
    fn repair_real_world_fun_ending() {
        // Reproduces the exact pattern from the failing log
        let input = "```json\n{\n  \"headline\": \"用户直接问AI\"你到底是什么模型\"，AI巧妙地回避了问题\",\n  \"detail\": \"AI像个守口如瓶的特工\"\n}\n```";
        let result = extract_json_from_ai_response(input);
        assert!(result.is_some(), "should repair the fun ending JSON");
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed["headline"]
            .as_str()
            .unwrap()
            .contains("你到底是什么模型"));
    }

    #[test]
    fn repair_real_world_interaction_style() {
        // Reproduces: "narrative": "...围绕着"这个项目是什么？""现在改了什么？"..."
        let input = "```json\n{\n  \"narrative\": \"会话围绕着\"这个项目是什么？\"和\"现在改了什么？\"展开\",\n  \"key_patterns\": [\"pattern1\"]\n}\n```";
        let result = extract_json_from_ai_response(input);
        assert!(result.is_some(), "should repair interaction style JSON");
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed["narrative"]
            .as_str()
            .unwrap()
            .contains("这个项目是什么"));
    }

    #[test]
    fn repair_does_not_break_valid_json() {
        let input = r#"{"key": "value", "num": 42, "arr": [1, 2]}"#;
        assert_eq!(
            extract_json_from_ai_response(input),
            Some(input.to_string())
        );
    }
}
