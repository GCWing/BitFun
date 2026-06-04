use serde_json::Value;

pub(super) fn render_exec_response_for_assistant(
    data: &Value,
    status_lines: Vec<String>,
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

    format!(
        "<status>\n{status}\n</status>\n<wall_time>\n{wall_time}\n</wall_time>\n<output>\n{output}\n</output>"
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
}
