//! Search results arrive over ACP as the model-facing *text* the harness wrote,
//! while OpenBitFun's Grep/Glob cards read counts and a file list off a structured
//! object. Without a translation the cards render every search as "0 matches /
//! 0 files" even though the search itself succeeded.
//!
//! Parsing belongs here rather than in the cards: the wire format is a property
//! of the agent we speak to, and the cards should keep reading one shape.
//! Anything that does not parse is returned untouched — an unrecognized result
//! is still shown raw, which is better than an invented one.

use serde_json::{json, Map, Value};

/// How deep to walk a result value looking for its text blocks. Real payloads
/// nest three or four levels; the bound only exists so a cyclic-looking shape
/// cannot spin.
const MAX_TEXT_DEPTH: usize = 8;

pub(super) fn normalize_tool_result(tool_name: &str, result: Value) -> Value {
    match tool_name {
        "Grep" => normalize_grep_result(result),
        "Glob" => normalize_glob_result(result),
        "ExecCommand" | "RunCode" => normalize_output_result(result),
        _ => result,
    }
}

/// The terminal and run-code cards show `output` (or `stdout`), while an ACP
/// result carries its text in content blocks. Without this the card renders the
/// command it ran and nothing underneath it.
fn normalize_output_result(result: Value) -> Value {
    if result.get("output").is_some() || result.get("stdout").is_some() {
        return result;
    }
    let Some(text) = result_text(&result) else {
        return result;
    };

    let mut object = match result {
        Value::Object(object) => object,
        _ => Map::new(),
    };
    object.insert("output".to_string(), Value::String(text));
    Value::Object(object)
}

/// The grep card reads `total_matches` / `file_count`, and shows `result` as the
/// expanded body; `matches` is the same rows in structured form.
fn normalize_grep_result(result: Value) -> Value {
    if grep_already_structured(&result) {
        return result;
    }
    let Some(text) = result_text(&result) else {
        return result;
    };
    let Some(summary) = parse_grep_text(&text) else {
        return result;
    };

    let mut object = object_base(result, &text);
    object.insert("total_matches".to_string(), json!(summary.total));
    object.insert("file_count".to_string(), json!(summary.file_count));
    object.insert("matches".to_string(), Value::Array(summary.matches));
    Value::Object(object)
}

/// The glob card reads `files`; the text stays as `result` for the raw view.
fn normalize_glob_result(result: Value) -> Value {
    if glob_already_structured(&result) {
        return result;
    }
    let Some(text) = result_text(&result) else {
        return result;
    };
    let Some(paths) = parse_glob_text(&text) else {
        return result;
    };

    let mut object = object_base(result, &text);
    object.insert(
        "files".to_string(),
        Value::Array(paths.into_iter().map(Value::String).collect()),
    );
    Value::Object(object)
}

fn grep_already_structured(result: &Value) -> bool {
    result.get("total_matches").is_some() || result.get("matches").is_some_and(Value::is_array)
}

fn glob_already_structured(result: &Value) -> bool {
    result.is_array()
        || result.get("files").is_some_and(Value::is_array)
        || result.get("matches").is_some_and(Value::is_array)
}

/// Keep whatever the agent sent and add the card's fields to it, so the raw view
/// still shows the original payload. A non-object result becomes the `result`
/// text the cards already know how to display.
fn object_base(result: Value, text: &str) -> Map<String, Value> {
    let mut object = match result {
        Value::Object(object) => object,
        _ => Map::new(),
    };
    object.insert("result".to_string(), Value::String(text.to_string()));
    object
}

/// Collect the text blocks out of a tool result, whichever envelope it came in:
/// a bare string, the ACP `{content: [...]}` fallback, or a harness result
/// message whose `content` nests one more level.
fn result_text(value: &Value) -> Option<String> {
    let mut chunks = Vec::new();
    collect_text(value, 0, &mut chunks);
    if chunks.is_empty() {
        return None;
    }
    Some(chunks.join("\n"))
}

fn collect_text(value: &Value, depth: usize, out: &mut Vec<String>) {
    if depth > MAX_TEXT_DEPTH {
        return;
    }
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => {
            for item in items {
                collect_text(item, depth + 1, out);
            }
        }
        Value::Object(object) => {
            if let Some(Value::String(text)) = object.get("text") {
                out.push(text.clone());
            } else if let Some(content) = object.get("content") {
                collect_text(content, depth + 1, out);
            }
        }
        _ => {}
    }
}

struct GrepSummary {
    /// Matches the search found — the header's count, which stays the truth even
    /// when the body below it was capped.
    total: u64,
    /// Distinct files among the rows actually listed.
    file_count: usize,
    matches: Vec<Value>,
}

/// Parse the harness grep body: a `Found N matches` header, then per file its
/// path followed by one `Line N: <text>` row per match, with a parenthesized
/// footer when the list was capped.
fn parse_grep_text(text: &str) -> Option<GrepSummary> {
    let trimmed = text.trim();
    if trimmed == "No matches found" {
        return Some(GrepSummary {
            total: 0,
            file_count: 0,
            matches: Vec::new(),
        });
    }

    let mut lines = trimmed.lines();
    let total = parse_grep_header(lines.next()?)?;

    let mut current_file: Option<&str> = None;
    let mut files: Vec<&str> = Vec::new();
    let mut matches = Vec::new();
    for line in lines {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        if let Some((number, matched)) = parse_grep_match_line(line) {
            if let Some(file) = current_file {
                matches.push(json!({ "file": file, "line": number, "text": matched }));
            }
        } else if line.starts_with('(') && line.ends_with(')') {
            // The truncation / spill footer, not a path.
            break;
        } else {
            current_file = Some(line);
            if !files.contains(&line) {
                files.push(line);
            }
        }
    }

    Some(GrepSummary {
        total,
        file_count: files.len(),
        matches,
    })
}

/// `Found 12 matches`, `Found 1 match`, or `Found 30 of 159 matches` — the count
/// reported is always the number the search saw.
fn parse_grep_header(header: &str) -> Option<u64> {
    let rest = header.trim().strip_prefix("Found ")?;
    let counts = rest
        .strip_suffix(" matches")
        .or_else(|| rest.strip_suffix(" match"))?;
    counts.rsplit(" of ").next()?.trim().parse().ok()
}

fn parse_grep_match_line(line: &str) -> Option<(u64, &str)> {
    let (number, matched) = line.strip_prefix("Line ")?.split_once(": ")?;
    Some((number.parse().ok()?, matched))
}

/// Parse the harness glob body: one path per line, with a parenthesized paging
/// footer when the list was capped. Prose is rejected rather than turned into
/// paths — a path list has no blank or indented lines.
fn parse_glob_text(text: &str) -> Option<Vec<String>> {
    let trimmed = text.trim();
    if trimmed == "No files found" {
        return Some(Vec::new());
    }

    let body = match trimmed.rsplit_once("\n\n") {
        Some((head, footer)) if footer.starts_with('(') && footer.ends_with(')') => head,
        _ => trimmed,
    };

    let mut paths = Vec::new();
    for line in body.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with(char::is_whitespace) || line.contains('\t') {
            return None;
        }
        paths.push(line.to_string());
    }
    if paths.is_empty() {
        return None;
    }
    Some(paths)
}

#[cfg(test)]
mod tests {
    use super::normalize_tool_result;
    use serde_json::json;

    /// The envelope DeepSeek Harness sends: a result message whose content
    /// nests the text one level deeper than the ACP fallback shape.
    fn harness_result(text: &str) -> serde_json::Value {
        json!({
            "source": { "kind": "tool", "callId": "call_00" },
            "role": "user",
            "content": [{
                "type": "tool-result",
                "toolCallId": "call_00",
                "isError": false,
                "content": [{ "type": "text", "text": text }],
            }],
        })
    }

    #[test]
    fn counts_grep_matches_and_files_from_the_harness_text() {
        let result = normalize_tool_result(
            "Grep",
            harness_result(
                "Found 3 matches\n\ndocs/a.md\nLine 12: alpha\nLine 40: beta\n\nsrc/b.rs\nLine 7: gamma",
            ),
        );

        assert_eq!(result["total_matches"], 3);
        assert_eq!(result["file_count"], 2);
        assert_eq!(result["matches"][0]["file"], "docs/a.md");
        assert_eq!(result["matches"][0]["line"], 12);
        assert_eq!(result["matches"][0]["text"], "alpha");
        assert_eq!(result["matches"][2]["file"], "src/b.rs");
        // The original text stays available to the expanded view.
        assert!(result["result"]
            .as_str()
            .unwrap()
            .starts_with("Found 3 matches"));
        assert_eq!(result["source"]["callId"], "call_00");
    }

    #[test]
    fn reports_the_searched_total_when_the_listing_was_capped() {
        // The body is capped but the search saw 159; showing the capped number
        // would understate the result the user asked about.
        let result = normalize_tool_result(
            "Grep",
            json!("Found 2 of 159 matches\n\ndocs/a.md\nLine 1: x\nLine 2: y\n\n(Full grep result stored at: /tmp/grep.txt. Read it.)"),
        );

        assert_eq!(result["total_matches"], 159);
        assert_eq!(result["file_count"], 1);
        assert_eq!(result["matches"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn keeps_an_empty_search_empty() {
        let grep = normalize_tool_result("Grep", harness_result("No matches found"));
        assert_eq!(grep["total_matches"], 0);
        assert_eq!(grep["file_count"], 0);

        let glob = normalize_tool_result("Glob", harness_result("No files found"));
        assert_eq!(glob["files"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn lists_glob_paths_and_drops_the_paging_footer() {
        let result = normalize_tool_result(
            "Glob",
            harness_result(
                "docs/a.md\ndocs/b.md\n\n(Showing 2 of 40 paths. Narrow path to see more.)",
            ),
        );

        assert_eq!(result["files"], json!(["docs/a.md", "docs/b.md"]));
    }

    #[test]
    fn leaves_a_result_it_cannot_parse_untouched() {
        // Prose from some other agent must not become a fake file list.
        let prose = harness_result("I looked around the repository.\n\n  Nothing matched.");
        assert_eq!(normalize_tool_result("Glob", prose.clone()), prose);
        assert_eq!(normalize_tool_result("Grep", prose.clone()), prose);
        // A tool whose card reads the result as-is is passed straight through.
        assert_eq!(normalize_tool_result("Read", prose.clone()), prose);
    }

    #[test]
    fn lifts_command_and_program_output_to_the_field_those_cards_read() {
        let command =
            normalize_tool_result("ExecCommand", harness_result("total 0\ndrwxr-xr-x  4 user"));
        assert_eq!(command["output"], "total 0\ndrwxr-xr-x  4 user");
        // The envelope stays intact for the raw view.
        assert_eq!(command["role"], "user");

        let code = normalize_tool_result("RunCode", harness_result("42"));
        assert_eq!(code["output"], "42");

        // An agent that already speaks the card's shape is left alone.
        let native = json!({ "output": "done", "exit_code": 0 });
        assert_eq!(normalize_tool_result("ExecCommand", native.clone()), native);
        let piped = json!({ "stdout": "done", "stderr": "" });
        assert_eq!(normalize_tool_result("ExecCommand", piped.clone()), piped);

        // Nothing to lift: a result with no text is untouched rather than
        // given an empty output that reads as "the command printed nothing".
        let empty = json!({ "locations": [] });
        assert_eq!(normalize_tool_result("RunCode", empty.clone()), empty);
    }

    #[test]
    fn leaves_an_already_structured_result_alone() {
        let grep = json!({ "total_matches": 4, "file_count": 2, "result": "Found 4 matches" });
        assert_eq!(normalize_tool_result("Grep", grep.clone()), grep);

        let glob = json!({ "files": ["a.rs"] });
        assert_eq!(normalize_tool_result("Glob", glob.clone()), glob);
    }
}
