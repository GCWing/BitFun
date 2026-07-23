//! Sanitization and validation for inline Write tool `content` arguments.

/// Returns true when the model output looks like a tool invocation instead of
/// raw file content (DSML blocks, nested tool_call XML, function_call JSON, etc.).
pub fn contains_tool_invocation_artifacts(content: &str) -> bool {
    if content.trim().is_empty() {
        return false;
    }

    let lower = content.to_ascii_lowercase();

    if content.contains("DSML")
        && (content.contains("tool_calls")
            || content.contains("invoke name")
            || content.contains("<invoke"))
    {
        return true;
    }

    const MARKERS: &[&str] = &[
        "<tool_calls",
        "</tool_calls",
        "<invoke",
        "</invoke",
        "function_call",
        "[tooluse:",
        "<function=",
        "\"type\":\"tool_use\"",
        "\"type\": \"tool_use\"",
        "toolu_",
    ];

    MARKERS.iter().any(|marker| lower.contains(marker))
}

/// Best-effort removal of tool-invocation wrappers that sometimes appear before
/// the real file body. When the entire payload is tool syntax, this returns an
/// empty string and callers should treat that as invalid file content.
pub fn strip_tool_invocation_artifacts(content: &str) -> String {
    let mut result = content.to_string();

    for (open, close) in [
        ("<tool_calls", "</tool_calls>"),
        ("<｜｜DSML｜｜tool_calls>", "</｜｜DSML｜｜tool_calls>"),
        ("<invoke", "</invoke>"),
        ("<function_call", "</function_call>"),
    ] {
        result = strip_delimited_block(&result, open, close);
    }

    // Some models emit one DSML invoke block without an outer tool_calls wrapper.
    while let Some(start) = result.find("<｜｜DSML｜｜invoke") {
        let Some(end) = result[start..].find("</｜｜DSML｜｜invoke>") else {
            result = result[..start].to_string();
            break;
        };
        let end = start + end + "</｜｜DSML｜｜invoke>".len();
        result = format!("{}{}", &result[..start], &result[end..]);
    }

    result.trim().to_string()
}

fn strip_delimited_block(content: &str, open_prefix: &str, close_tag: &str) -> String {
    let mut result = content.to_string();
    while let Some(start) = result.find(open_prefix) {
        let Some(relative_end) = result[start..].find(close_tag) else {
            result = result[..start].to_string();
            break;
        };
        let end = start + relative_end + close_tag.len();
        result = format!("{}{}", &result[..start], &result[end..]);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{contains_tool_invocation_artifacts, strip_tool_invocation_artifacts};

    #[test]
    fn detects_dsml_nested_write() {
        let content = concat!(
            "<｜｜DSML｜｜tool_calls>\n",
            "<｜｜DSML｜｜invoke name=\"Write\">\n",
            "<｜｜DSML｜｜parameter name=\"file_path\" string=\"true\">a.ts</｜｜DSML｜｜parameter>\n",
            "</｜｜DSML｜｜invoke>\n",
            "</｜｜DSML｜｜tool_calls>"
        );
        assert!(contains_tool_invocation_artifacts(content));
        assert!(strip_tool_invocation_artifacts(content).is_empty());
    }

    #[test]
    fn detects_standard_tool_calls_block() {
        let content = "<tool_calls><invoke name=\"Write\">...</invoke></tool_calls>";
        assert!(contains_tool_invocation_artifacts(content));
        assert!(strip_tool_invocation_artifacts(content).is_empty());
    }

    #[test]
    fn preserves_normal_source_code() {
        let content = "import React from 'react';\n\nexport default function App() {}\n";
        assert!(!contains_tool_invocation_artifacts(content));
        assert_eq!(
            strip_tool_invocation_artifacts(content),
            "import React from 'react';\n\nexport default function App() {}"
        );
    }

    #[test]
    fn strips_tool_block_before_real_content() {
        let content = concat!(
            "<tool_calls><invoke name=\"Write\"></invoke></tool_calls>",
            "export const value = 1;\n"
        );
        assert!(contains_tool_invocation_artifacts(content));
        assert_eq!(
            strip_tool_invocation_artifacts(content),
            "export const value = 1;"
        );
    }
}
