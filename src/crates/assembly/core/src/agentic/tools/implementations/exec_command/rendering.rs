use serde_json::Value;

pub(super) fn render_exec_response_for_assistant(
    data: &Value,
    status_lines: Vec<String>,
    wall_time_precision: usize,
) -> String {
    render_exec_response_for_assistant_with_notes(
        data,
        status_lines,
        Vec::new(),
        wall_time_precision,
    )
}

pub(super) fn render_exec_response_for_assistant_with_notes(
    data: &Value,
    status_lines: Vec<String>,
    note_lines: Vec<String>,
    wall_time_precision: usize,
) -> String {
    let output = data.get("output").and_then(Value::as_str).unwrap_or("");
    let status = if status_lines.is_empty() {
        "Process status unavailable.".to_string()
    } else {
        status_lines.join("\n")
    };
    let wall_time = format!(
        "{:.precision$} seconds",
        data.get("wall_time_seconds")
            .and_then(Value::as_f64)
            .unwrap_or_default(),
        precision = wall_time_precision,
    );
    let note_section = if note_lines.is_empty() {
        String::new()
    } else {
        format!("<note>\n{}\n</note>\n", note_lines.join("\n"))
    };

    format!(
        "<status>\n{status}\n</status>\n<wall_time>\n{wall_time}\n</wall_time>\n{note_section}<output>\n{output}\n</output>"
    )
}

#[cfg(test)]
mod tests {
    use super::render_exec_response_for_assistant;
    use serde_json::json;

    #[test]
    fn renders_exec_response_with_xmlish_sections() {
        let data = json!({
            "wall_time_seconds": 0.0068,
            "output": "sh: 1: node: not found\r\n",
        });

        let rendered = render_exec_response_for_assistant(
            &data,
            vec!["Process exited with code 127.".to_string()],
            3,
        );

        assert_eq!(
            rendered,
            "<status>\nProcess exited with code 127.\n</status>\n<wall_time>\n0.007 seconds\n</wall_time>\n<output>\nsh: 1: node: not found\r\n\n</output>"
        );
    }

    #[test]
    fn renders_exec_response_with_note_section() {
        let data = json!({
            "wall_time_seconds": 30.0,
            "output": "",
        });

        let rendered = super::render_exec_response_for_assistant_with_notes(
            &data,
            vec!["Process is still running. session_id: 42".to_string()],
            vec!["No output was produced during this wait window.".to_string()],
            3,
        );

        assert_eq!(
            rendered,
            "<status>\nProcess is still running. session_id: 42\n</status>\n<wall_time>\n30.000 seconds\n</wall_time>\n<note>\nNo output was produced during this wait window.\n</note>\n<output>\n\n</output>"
        );
    }
}
