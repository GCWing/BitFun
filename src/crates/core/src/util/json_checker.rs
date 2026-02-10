/// JSON integrity checker - detect whether streamed JSON is complete
///
/// Primarily used to check whether tool-parameter JSON in AI streaming responses has been fully received.
/// Tolerates leading non-JSON content (e.g. spaces sent by some models) by discarding
/// everything before the first '{'.
#[derive(Debug)]
pub struct JsonChecker {
    buffer: String,
    stack: Vec<char>,
    in_string: bool,
    escape_next: bool,
    seen_left_brace: bool,
}

impl JsonChecker {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            stack: Vec::new(),
            in_string: false,
            escape_next: false,
            seen_left_brace: false,
        }
    }

    pub fn append(&mut self, s: &str) {
        let mut chars = s.chars();

        while let Some(ch) = chars.next() {
            // Discard everything before the first '{'
            if !self.seen_left_brace {
                if ch == '{' {
                    self.seen_left_brace = true;
                    self.stack.push('{');
                    self.buffer.push(ch);
                }
                continue;
            }

            self.buffer.push(ch);

            if self.escape_next {
                self.escape_next = false;
                continue;
            }

            match ch {
                '\\' if self.in_string => {
                    self.escape_next = true;
                }
                '"' => {
                    self.in_string = !self.in_string;
                }
                '{' if !self.in_string => {
                    self.stack.push('{');
                }
                '}' if !self.in_string => {
                    if !self.stack.is_empty() {
                        self.stack.pop();
                    }
                }
                _ => {}
            }
        }
    }

    pub fn get_buffer(&self) -> String {
        self.buffer.clone()
    }

    pub fn is_valid(&self) -> bool {
        self.stack.is_empty() && self.seen_left_brace
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.stack.clear();
        self.in_string = false;
        self.escape_next = false;
        self.seen_left_brace = false;
    }
}

impl Default for JsonChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper: feed string as single chunk ──

    fn check_one_shot(input: &str) -> (bool, String) {
        let mut c = JsonChecker::new();
        c.append(input);
        (c.is_valid(), c.get_buffer())
    }

    // ── Helper: feed string char-by-char (worst-case chunking) ──

    fn check_char_by_char(input: &str) -> (bool, String) {
        let mut c = JsonChecker::new();
        for ch in input.chars() {
            c.append(&ch.to_string());
        }
        (c.is_valid(), c.get_buffer())
    }

    // ── Basic validity ──

    #[test]
    fn empty_input_is_invalid() {
        let (valid, _) = check_one_shot("");
        assert!(!valid);
    }

    #[test]
    fn simple_empty_object() {
        let (valid, buf) = check_one_shot("{}");
        assert!(valid);
        assert_eq!(buf, "{}");
    }

    #[test]
    fn simple_object_with_string_value() {
        let input = r#"{"city": "Beijing"}"#;
        let (valid, buf) = check_one_shot(input);
        assert!(valid);
        assert_eq!(buf, input);
    }

    #[test]
    fn nested_object() {
        let input = r#"{"a": {"b": {"c": 1}}}"#;
        let (valid, _) = check_one_shot(input);
        assert!(valid);
    }

    #[test]
    fn incomplete_object_missing_closing_brace() {
        let (valid, _) = check_one_shot(r#"{"key": "value""#);
        assert!(!valid);
    }

    #[test]
    fn incomplete_object_open_string() {
        let (valid, _) = check_one_shot(r#"{"key": "val"#);
        assert!(!valid);
    }

    // ── Leading garbage / whitespace (ByteDance model issue) ──

    #[test]
    fn leading_space_before_brace() {
        let (valid, buf) = check_one_shot(r#" {"city": "Beijing"}"#);
        assert!(valid);
        assert_eq!(buf, r#"{"city": "Beijing"}"#);
    }

    #[test]
    fn leading_multiple_spaces_and_newlines() {
        let (valid, buf) = check_one_shot("  \n\t {\"a\": 1}");
        assert!(valid);
        assert_eq!(buf, "{\"a\": 1}");
    }

    #[test]
    fn leading_random_text_before_brace() {
        let (valid, buf) = check_one_shot("some garbage {\"ok\": true}");
        assert!(valid);
        assert_eq!(buf, "{\"ok\": true}");
    }

    #[test]
    fn only_spaces_no_brace() {
        let (valid, _) = check_one_shot("   ");
        assert!(!valid);
    }

    // ── Escape handling ──

    #[test]
    fn escaped_quote_in_string() {
        // JSON: {"msg": "say \"hello\""}
        let input = r#"{"msg": "say \"hello\""}"#;
        let (valid, _) = check_one_shot(input);
        assert!(valid);
    }

    #[test]
    fn escaped_backslash_before_quote() {
        // JSON: {"path": "C:\\"} — value is C:\, the \\ is an escaped backslash
        let input = r#"{"path": "C:\\"}"#;
        let (valid, _) = check_one_shot(input);
        assert!(valid);
    }

    #[test]
    fn escaped_backslash_followed_by_quote_char_by_char() {
        // Ensure escape state survives across single-char chunks
        let input = r#"{"path": "C:\\"}"#;
        let (valid, buf) = check_char_by_char(input);
        assert!(valid);
        assert_eq!(buf, input);
    }

    #[test]
    fn braces_inside_string_are_ignored() {
        let input = r#"{"code": "fn main() { println!(\"hi\"); }"}"#;
        let (valid, _) = check_one_shot(input);
        assert!(valid);
    }

    #[test]
    fn braces_inside_string_char_by_char() {
        let input = r#"{"code": "fn main() { println!(\"hi\"); }"}"#;
        let (valid, _) = check_char_by_char(input);
        assert!(valid);
    }

    // ── Cross-chunk escape: the exact ByteDance bug scenario ──

    #[test]
    fn escape_split_across_chunks() {
        // Simulates: {"new_string": "fn main() {\n    println!(\"Hello, World!\");\n}"}
        // The backslash and the quote land in different chunks
        let mut c = JsonChecker::new();
        c.append(r#"{"new_string": "fn main() {\n    println!(\"Hello, World!"#);
        assert!(!c.is_valid());

        // chunk ends with backslash
        c.append("\\");
        assert!(!c.is_valid());

        // next chunk starts with escaped quote — must NOT end the string
        c.append("\"");
        assert!(!c.is_valid());

        c.append(r#");\n}"}"#);
        assert!(c.is_valid());
    }

    #[test]
    fn escape_at_chunk_boundary_does_not_leak() {
        // After the escaped char is consumed, escape_next should be false
        let mut c = JsonChecker::new();
        c.append(r#"{"a": "x\"#); // ends with backslash inside string
        assert!(!c.is_valid());

        c.append("n"); // \n escape sequence complete
        assert!(!c.is_valid());

        c.append(r#""}"#); // close string and object
        assert!(c.is_valid());
    }

    // ── Realistic streaming simulation ──

    #[test]
    fn bytedance_doubao_streaming_simulation() {
        // Reproduces the exact chunking pattern from the bug report
        let mut c = JsonChecker::new();
        c.append(""); // empty first arguments chunk
        c.append(" {\""); // leading space + opening brace
        assert!(!c.is_valid());

        c.append("city");
        c.append("\":");
        c.append(" \"");
        c.append("Beijing");
        c.append("\"}");
        assert!(c.is_valid());
        assert_eq!(c.get_buffer(), r#"{"city": "Beijing"}"#);
    }

    #[test]
    fn edit_tool_streaming_simulation() {
        // Reproduces the Edit tool call from the second bug report
        let mut c = JsonChecker::new();
        c.append("{\"file_path\": \"E:/Projects/ForTest/basic-rust/src/main.rs\", \"new_string\": \"fn main() {\\n    println!(\\\"Hello,");
        c.append(" World");
        c.append("!\\"); // backslash at chunk end
        c.append("\");"); // escaped quote at chunk start — must stay in string
        assert!(!c.is_valid());

        c.append("\\"); // another backslash at chunk end
        c.append("n"); // \n escape
        c.append("}\","); // closing brace inside string, then close string, comma
        assert!(!c.is_valid()); // object not yet closed

        c.append(" \"old_string\": \"\"");
        c.append("}");
        assert!(c.is_valid());
    }

    // ── Reset ──

    #[test]
    fn reset_clears_all_state() {
        let mut c = JsonChecker::new();
        c.append(r#" {"key": "val"#); // leading space, incomplete
        assert!(!c.is_valid());

        c.reset();
        assert!(!c.is_valid());
        assert_eq!(c.get_buffer(), "");

        // Should work fresh after reset
        c.append(r#"{"ok": true}"#);
        assert!(c.is_valid());
    }

    #[test]
    fn reset_clears_escape_state() {
        let mut c = JsonChecker::new();
        c.append(r#"{"a": "\"#); // ends mid-escape
        c.reset();

        // The stale escape_next must not affect the new input
        c.append(r#"{"b": "x"}"#);
        assert!(c.is_valid());
    }

    // ── Edge cases ──

    #[test]
    fn multiple_top_level_objects_first_wins() {
        // After the first object completes, is_valid becomes true;
        // subsequent data keeps appending but re-opens the stack
        let mut c = JsonChecker::new();
        c.append("{}");
        assert!(c.is_valid());

        c.append("{}");
        // stack opens and closes again, still valid
        assert!(c.is_valid());
    }

    #[test]
    fn deeply_nested_objects() {
        let input = r#"{"a":{"b":{"c":{"d":{"e":{}}}}}}"#;
        let (valid, _) = check_one_shot(input);
        assert!(valid);
    }

    #[test]
    fn string_with_unicode_escapes() {
        let input = r#"{"emoji": "\u0048\u0065\u006C\u006C\u006F"}"#;
        let (valid, _) = check_one_shot(input);
        assert!(valid);
    }

    #[test]
    fn string_with_newlines_and_tabs() {
        let input = r#"{"text": "line1\nline2\ttab"}"#;
        let (valid, _) = check_one_shot(input);
        assert!(valid);
    }

    #[test]
    fn consecutive_escaped_backslashes() {
        // JSON value: a\\b — two backslashes, meaning literal backslash in value
        let input = r#"{"p": "a\\\\b"}"#;
        let (valid, _) = check_one_shot(input);
        assert!(valid);
    }

    #[test]
    fn consecutive_escaped_backslashes_char_by_char() {
        let input = r#"{"p": "a\\\\b"}"#;
        let (valid, _) = check_char_by_char(input);
        assert!(valid);
    }

    #[test]
    fn default_trait_works() {
        let c = JsonChecker::default();
        assert!(!c.is_valid());
        assert_eq!(c.get_buffer(), "");
    }

    // ── Streaming: no premature is_valid() ──

    #[test]
    fn never_valid_during_progressive_append() {
        // Feed a complete JSON object token-by-token, assert is_valid() is false
        // at every step except after the final '}'
        let chunks = vec![
            "{", "\"", "k", "e", "y", "\"", ":", " ", "\"", "v", "a", "l", "\"", "}",
        ];
        let mut c = JsonChecker::new();
        for (i, chunk) in chunks.iter().enumerate() {
            c.append(chunk);
            if i < chunks.len() - 1 {
                assert!(
                    !c.is_valid(),
                    "premature valid at chunk index {}: {:?}",
                    i,
                    c.get_buffer()
                );
            }
        }
        assert!(c.is_valid());
        assert_eq!(c.get_buffer(), r#"{"key": "val"}"#);
    }

    #[test]
    fn never_valid_during_nested_object_streaming() {
        // {"a": {"b": 1}} streamed in realistic chunks
        let chunks = vec!["{\"a\"", ": ", "{\"b\"", ": 1", "}", "}"];
        let mut c = JsonChecker::new();
        for (i, chunk) in chunks.iter().enumerate() {
            c.append(chunk);
            if i < chunks.len() - 1 {
                assert!(
                    !c.is_valid(),
                    "premature valid at chunk index {}: {:?}",
                    i,
                    c.get_buffer()
                );
            }
        }
        assert!(c.is_valid());
    }

    #[test]
    fn string_with_braces_never_premature_valid() {
        // {"code": "{ } { }"} — braces inside string must not close the object
        let chunks = vec!["{\"code\": \"", "{ ", "} ", "{ ", "}", "\"", "}"];
        let mut c = JsonChecker::new();
        for (i, chunk) in chunks.iter().enumerate() {
            c.append(chunk);
            if i < chunks.len() - 1 {
                assert!(
                    !c.is_valid(),
                    "premature valid at chunk index {}: {:?}",
                    i,
                    c.get_buffer()
                );
            }
        }
        assert!(c.is_valid());
    }

    // ── Streaming: empty chunks interspersed ──

    #[test]
    fn empty_chunks_between_data() {
        let mut c = JsonChecker::new();
        c.append("");
        assert!(!c.is_valid());
        c.append("{");
        assert!(!c.is_valid());
        c.append("");
        assert!(!c.is_valid());
        c.append("\"a\"");
        c.append("");
        c.append(": 1");
        c.append("");
        c.append("");
        c.append("}");
        assert!(c.is_valid());
        assert_eq!(c.get_buffer(), r#"{"a": 1}"#);
    }

    #[test]
    fn empty_chunks_before_first_brace() {
        let mut c = JsonChecker::new();
        c.append("");
        c.append("");
        c.append("");
        assert!(!c.is_valid());
        c.append("  ");
        assert!(!c.is_valid());
        c.append("{}");
        assert!(c.is_valid());
    }

    // ── Streaming: \\\" sequence split at different positions ──

    #[test]
    fn escaped_backslash_then_escaped_quote_split_1() {
        // JSON: {"a": "x\\\"y"} — value is x\"y (backslash, quote, y)
        // Split: `{"a": "x\` | `\` | `\` | `"` | `y"}`
        // Char-by-char through the \\\" sequence
        let mut c = JsonChecker::new();
        c.append(r#"{"a": "x"#);
        assert!(!c.is_valid());
        c.append("\\"); // first \ of \\, sets escape_next
        assert!(!c.is_valid());
        c.append("\\"); // consumed by escape (it's the escaped backslash), then done
        assert!(!c.is_valid());
        c.append("\\"); // first \ of \", sets escape_next
        assert!(!c.is_valid());
        c.append("\""); // consumed by escape (it's the escaped quote)
        assert!(!c.is_valid()); // still inside string!
        c.append("y");
        assert!(!c.is_valid());
        c.append("\"}");
        assert!(c.is_valid());
    }

    #[test]
    fn escaped_backslash_then_escaped_quote_split_2() {
        // Same JSON: {"a": "x\\\"y"} but split as: `...x\\` | `\"y"}`
        let mut c = JsonChecker::new();
        c.append(r#"{"a": "x\\"#); // \\ = escaped backslash, escape_next consumed
        assert!(!c.is_valid());
        c.append(r#"\"y"}"#); // \" = escaped quote, y, close string, close object
        assert!(c.is_valid());
    }

    #[test]
    fn escaped_backslash_then_escaped_quote_split_3() {
        // Same JSON but split as: `...x\` | `\\` | `"y"}`
        let mut c = JsonChecker::new();
        c.append(r#"{"a": "x\"#); // \ sets escape_next
        assert!(!c.is_valid());
        c.append("\\\\"); // first \ consumed by escape, second \ sets escape_next
        assert!(!c.is_valid());
        c.append("\"y\"}"); // " consumed by escape, y normal, " closes string, } closes object
        assert!(c.is_valid());
    }

    // ── Streaming: escaped backslash + closing quote ──

    #[test]
    fn escaped_backslash_then_closing_quote_split_at_boundary() {
        // JSON: {"a": "x\\"} — value is x\ (escaped backslash), then " closes string
        // Split as: `{"a": "x\` | `\"}` — \ crosses chunk boundary
        let mut c = JsonChecker::new();
        c.append(r#"{"a": "x\"#); // \ sets escape_next
        assert!(!c.is_valid());
        c.append("\\\"}"); // \ consumed by escape, " closes string, } closes object
        assert!(c.is_valid());
        assert_eq!(c.get_buffer(), r#"{"a": "x\\"}"#);
    }

    #[test]
    fn escaped_backslash_then_closing_quote_split_after_pair() {
        // Same JSON: {"a": "x\\"} — split as: `{"a": "x\\` | `"}`
        let mut c = JsonChecker::new();
        c.append(r#"{"a": "x\\"#); // \\ pair complete, escape_next = false
        assert!(!c.is_valid());
        c.append("\"}"); // " closes string, } closes object
        assert!(c.is_valid());
    }

    // ── Streaming: multiple tool calls with reset (full lifecycle) ──

    #[test]
    fn lifecycle_multiple_tool_calls_with_reset() {
        let mut c = JsonChecker::new();

        // --- Tool call 1: simple ---
        c.append(" "); // leading space (ByteDance)
        c.append("{\"");
        c.append("city\": \"Beijing\"}");
        assert!(c.is_valid());
        assert_eq!(c.get_buffer(), r#"{"city": "Beijing"}"#);

        // --- Reset for tool call 2 ---
        c.reset();
        assert!(!c.is_valid());
        assert_eq!(c.get_buffer(), "");

        // --- Tool call 2: with escapes ---
        c.append("{\"code\": \"");
        assert!(!c.is_valid());
        c.append("fn main() {\\n");
        assert!(!c.is_valid());
        c.append("    println!(\\\"hi\\\");");
        assert!(!c.is_valid());
        c.append("\\n}\"}");
        assert!(c.is_valid());

        // --- Reset for tool call 3 ---
        c.reset();
        assert!(!c.is_valid());

        // --- Tool call 3: empty object ---
        c.append("{}");
        assert!(c.is_valid());
    }

    #[test]
    fn lifecycle_reset_mid_escape_then_new_tool_call() {
        let mut c = JsonChecker::new();

        // Tool call 1: interrupted mid-escape
        c.append("{\"a\": \"x\\"); // ends with pending escape
        assert!(!c.is_valid());

        // Reset before completion (e.g. stream error)
        c.reset();

        // Tool call 2: must work cleanly with no stale escape state
        c.append("{\"b\": \"y\"}");
        assert!(c.is_valid());
        assert_eq!(c.get_buffer(), r#"{"b": "y"}"#);
    }

    #[test]
    fn lifecycle_reset_mid_string_then_new_tool_call() {
        let mut c = JsonChecker::new();

        // Tool call 1: interrupted inside string
        c.append("{\"a\": \"some text");
        assert!(!c.is_valid());

        c.reset();

        // Tool call 2: must not think it's still in a string
        c.append("{\"b\": \"{}\"}"); // braces inside string value
        assert!(c.is_valid());
    }
}
