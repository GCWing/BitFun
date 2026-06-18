use crate::background_command_output::BackgroundCommandOutputStatus;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecCommandControlAction {
    Interrupt,
    Kill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecCommandControlOrigin {
    ModelTool,
    OutOfBand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecCommandCompletionStatus {
    Exited,
    Interrupted,
    Killed,
    Pruned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecCommandCompletionSource {
    Process,
    OutOfBandControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecCommandCompletion {
    pub status: ExecCommandCompletionStatus,
    pub source: ExecCommandCompletionSource,
}

#[derive(Debug, Clone)]
pub struct ExecCommandControlRequest {
    pub session_id: i32,
    pub action: ExecCommandControlAction,
    pub origin: ExecCommandControlOrigin,
    pub remote: bool,
    pub yield_time_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ExecCommandControlResponse {
    pub chunk_id: String,
    pub wall_time_seconds: f64,
    pub output: String,
    pub session_id: Option<i32>,
    pub exit_code: Option<i32>,
    pub original_output_chars: usize,
    pub action: ExecCommandControlAction,
    pub remote: bool,
    pub completion: Option<ExecCommandCompletion>,
}

#[derive(Debug, Clone)]
pub struct ExecCommandSessionNotFoundResult {
    pub data: Value,
    pub assistant_message: String,
}

pub fn exec_command_session_id_from_input(input: &Value) -> Option<i32> {
    input.get("session_id").and_then(|value| {
        value
            .as_i64()
            .and_then(|id| i32::try_from(id).ok())
            .or_else(|| value.as_u64().and_then(|id| i32::try_from(id).ok()))
    })
}

pub fn exec_command_control_action_from_input(input: &Value) -> Option<ExecCommandControlAction> {
    match input.get("action").and_then(Value::as_str)?.trim() {
        "interrupt" => Some(ExecCommandControlAction::Interrupt),
        "kill" => Some(ExecCommandControlAction::Kill),
        _ => None,
    }
}

pub fn exec_command_control_action_name(action: ExecCommandControlAction) -> &'static str {
    match action {
        ExecCommandControlAction::Interrupt => "interrupt",
        ExecCommandControlAction::Kill => "kill",
    }
}

pub fn exec_command_completion_value(completion: ExecCommandCompletion) -> Value {
    json!({
        "status": exec_command_completion_status_name(completion.status),
        "source": exec_command_completion_source_name(completion.source),
    })
}

pub fn exec_command_background_output_status(
    completion: Option<ExecCommandCompletion>,
) -> BackgroundCommandOutputStatus {
    match completion.map(|completion| completion.status) {
        Some(ExecCommandCompletionStatus::Interrupted) => {
            BackgroundCommandOutputStatus::Interrupted
        }
        Some(ExecCommandCompletionStatus::Killed) => BackgroundCommandOutputStatus::Killed,
        Some(ExecCommandCompletionStatus::Pruned) => BackgroundCommandOutputStatus::Pruned,
        Some(ExecCommandCompletionStatus::Exited) | None => BackgroundCommandOutputStatus::Exited,
    }
}

pub fn render_exec_response_for_assistant(
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

pub fn render_exec_response_for_assistant_with_notes(
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

pub fn render_exec_command_response_for_assistant(data: &Value) -> String {
    let status_lines = completion_status_lines(data);
    let mut note_lines = Vec::new();

    if data.get("tty").and_then(Value::as_bool) == Some(false)
        && data
            .get("output")
            .and_then(Value::as_str)
            .map(str::is_empty)
            .unwrap_or(true)
    {
        note_lines.push(
            "No output was produced. In non-TTY mode, programs may block-buffer pipe output; use unbuffered flags/env vars or TTY mode if progressive output matters."
                .to_string(),
        );
    }

    render_exec_response_for_assistant_with_notes(data, status_lines, note_lines, 3)
}

pub fn render_write_stdin_response_for_assistant(data: &Value) -> String {
    render_exec_response_for_assistant(data, completion_status_lines(data), 4)
}

pub fn render_exec_control_response_for_assistant(
    data: &Value,
    action: ExecCommandControlAction,
) -> String {
    let mut status_lines = Vec::new();
    match action {
        ExecCommandControlAction::Interrupt => {
            status_lines.push("Sent interrupt to process.".to_string())
        }
        ExecCommandControlAction::Kill => status_lines.push("Sent kill to process.".to_string()),
    }
    if let Some(exit_code) = data.get("exit_code").and_then(Value::as_i64) {
        status_lines.push(format!("Process exited with code {exit_code}."));
    } else if let Some(session_id) = data.get("session_id").and_then(Value::as_i64) {
        status_lines.push(format!(
            "Process is still running. session_id: {session_id}"
        ));
    }
    render_exec_response_for_assistant(data, status_lines, 4)
}

pub fn write_stdin_session_not_found_result(
    session_id: i32,
    remote: bool,
) -> ExecCommandSessionNotFoundResult {
    let message = format!(
        "ExecCommand session {session_id} was not found. It may have already exited, been collected, or been pruned."
    );
    let mut data = json!({
        "status": "session_not_found",
        "message": message,
        "requested_session_id": session_id,
        "session_id": null,
        "exit_code": null,
        "output": "",
        "original_output_chars": 0,
    });
    if remote {
        data["remote"] = json!(true);
    }

    ExecCommandSessionNotFoundResult {
        data,
        assistant_message: message,
    }
}

pub fn exec_control_session_not_found_result(
    session_id: i32,
    action: ExecCommandControlAction,
    remote: bool,
) -> ExecCommandSessionNotFoundResult {
    let action_name = exec_command_control_action_name(action);
    let message = format!(
        "No {action_name} was sent because ExecCommand session {session_id} was not found. It may have already exited, been collected, or been pruned."
    );
    let mut data = json!({
        "status": "session_not_found",
        "message": message,
        "requested_session_id": session_id,
        "session_id": null,
        "exit_code": null,
        "output": "",
        "original_output_chars": 0,
        "action": action_name,
    });
    if remote {
        data["remote"] = json!(true);
    }

    ExecCommandSessionNotFoundResult {
        data,
        assistant_message: message,
    }
}

fn completion_status_lines(data: &Value) -> Vec<String> {
    let mut status_lines = Vec::new();
    let completion = data.get("completion");
    let completion_source = completion
        .and_then(|value| value.get("source"))
        .and_then(Value::as_str);
    let completion_status = completion
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str);

    if completion_source == Some("out_of_band_control") {
        match completion_status {
            Some("interrupted") => {
                status_lines.push("Process was interrupted externally.".to_string())
            }
            Some("killed") => status_lines.push("Process was terminated externally.".to_string()),
            Some(status) => {
                status_lines.push(format!("Process ended externally with status {status}."))
            }
            None => status_lines.push("Process ended externally.".to_string()),
        }
        if let Some(exit_code) = data.get("exit_code").and_then(Value::as_i64) {
            status_lines.push(format!("Process exited with code {exit_code}."));
        }
    } else if let Some(exit_code) = data.get("exit_code").and_then(Value::as_i64) {
        status_lines.push(format!("Process exited with code {exit_code}."));
    } else if let Some(session_id) = data.get("session_id").and_then(Value::as_i64) {
        status_lines.push(format!(
            "Process is still running. session_id: {session_id}"
        ));
    }

    status_lines
}

fn exec_command_completion_status_name(status: ExecCommandCompletionStatus) -> &'static str {
    match status {
        ExecCommandCompletionStatus::Exited => "exited",
        ExecCommandCompletionStatus::Interrupted => "interrupted",
        ExecCommandCompletionStatus::Killed => "killed",
        ExecCommandCompletionStatus::Pruned => "pruned",
    }
}

fn exec_command_completion_source_name(source: ExecCommandCompletionSource) -> &'static str {
    match source {
        ExecCommandCompletionSource::Process => "process",
        ExecCommandCompletionSource::OutOfBandControl => "out_of_band_control",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        let rendered = render_exec_response_for_assistant_with_notes(
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

    #[test]
    fn command_response_keeps_non_tty_empty_output_note() {
        let data = json!({
            "wall_time_seconds": 1.0,
            "output": "",
            "tty": false,
            "session_id": 7,
        });

        let rendered = render_exec_command_response_for_assistant(&data);

        assert!(rendered.contains("Process is still running. session_id: 7"));
        assert!(rendered.contains("programs may block-buffer pipe output"));
        assert!(rendered.contains("<output>\n\n</output>"));
    }

    #[test]
    fn write_stdin_response_reports_external_interrupt() {
        let data = json!({
            "wall_time_seconds": 1.25,
            "output": "partial",
            "exit_code": 130,
            "completion": {
                "status": "interrupted",
                "source": "out_of_band_control"
            }
        });

        let rendered = render_write_stdin_response_for_assistant(&data);

        assert!(rendered.contains("Process was interrupted externally."));
        assert!(rendered.contains("Process exited with code 130."));
        assert!(rendered.contains("<wall_time>\n1.2500 seconds\n</wall_time>"));
    }

    #[test]
    fn control_session_not_found_result_is_plain_assistant_text() {
        let result =
            exec_control_session_not_found_result(456, ExecCommandControlAction::Interrupt, true);

        assert_eq!(
            result.data.get("status").and_then(Value::as_str),
            Some("session_not_found")
        );
        assert_eq!(
            result
                .data
                .get("requested_session_id")
                .and_then(Value::as_i64),
            Some(456)
        );
        assert_eq!(
            result.data.get("remote").and_then(Value::as_bool),
            Some(true)
        );
        assert!(result.assistant_message.contains("No interrupt was sent"));
        assert!(!result.assistant_message.contains("<wall_time>"));
        assert!(!result.assistant_message.contains("<output>"));
    }

    #[test]
    fn completion_value_uses_stable_snake_case_shape() {
        let value = exec_command_completion_value(ExecCommandCompletion {
            status: ExecCommandCompletionStatus::Killed,
            source: ExecCommandCompletionSource::OutOfBandControl,
        });

        assert_eq!(
            value,
            json!({
                "status": "killed",
                "source": "out_of_band_control",
            })
        );
    }

    #[test]
    fn background_output_status_maps_terminal_completion_without_core_types() {
        assert_eq!(
            exec_command_background_output_status(Some(ExecCommandCompletion {
                status: ExecCommandCompletionStatus::Interrupted,
                source: ExecCommandCompletionSource::Process,
            })),
            BackgroundCommandOutputStatus::Interrupted
        );
        assert_eq!(
            exec_command_background_output_status(Some(ExecCommandCompletion {
                status: ExecCommandCompletionStatus::Pruned,
                source: ExecCommandCompletionSource::OutOfBandControl,
            })),
            BackgroundCommandOutputStatus::Pruned
        );
        assert_eq!(
            exec_command_background_output_status(None),
            BackgroundCommandOutputStatus::Exited
        );
    }
}
