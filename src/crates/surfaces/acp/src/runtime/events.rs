use std::collections::HashSet;
use std::path::PathBuf;

use agent_client_protocol::schema::{
    PermissionOption, PermissionOptionKind, RequestPermissionRequest, SessionId,
    SessionNotification, SessionUpdate, ToolCall, ToolCallContent, ToolCallLocation,
    ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields, ToolKind,
};
use agent_client_protocol::{Client, ConnectionTo, Result};
use bitfun_events::ToolEventData;

pub(super) const PERMISSION_ALLOW_ONCE: &str = "allow_once";
pub(super) const PERMISSION_REJECT_ONCE: &str = "reject_once";
const ACP_LARGE_TEXT_PREVIEW_CHARS: usize = 2_000;

pub(super) fn send_update(
    connection: &ConnectionTo<Client>,
    session_id: &str,
    update: SessionUpdate,
) -> Result<()> {
    connection.send_notification(SessionNotification::new(
        SessionId::new(session_id.to_string()),
        update,
    ))
}

pub(super) fn tool_event_updates(
    tool_event: &ToolEventData,
    seen_tool_calls: &mut HashSet<String>,
) -> Vec<SessionUpdate> {
    let tool_id = tool_event.tool_id();
    let mut updates = Vec::new();

    if !seen_tool_calls.contains(tool_id) {
        seen_tool_calls.insert(tool_id.to_string());
        updates.push(SessionUpdate::ToolCall(initial_tool_call(tool_event)));
    }

    if let Some(update) = tool_call_update(tool_event) {
        updates.push(SessionUpdate::ToolCallUpdate(update));
    }

    updates
}

pub(super) fn permission_request(
    session_id: &str,
    tool_id: &str,
    tool_name: &str,
    params: &serde_json::Value,
) -> RequestPermissionRequest {
    RequestPermissionRequest::new(
        SessionId::new(session_id.to_string()),
        ToolCallUpdate::new(
            tool_id.to_string(),
            ToolCallUpdateFields::new()
                .title(format!("Allow {}?", tool_name))
                .status(ToolCallStatus::Pending)
                .kind(tool_kind(tool_name))
                .locations(tool_locations(params))
                .raw_input(sanitize_tool_input(tool_name, params.clone()))
                .content(vec![text_content(format!(
                    "Permission required to run {}.",
                    tool_name
                ))]),
        ),
        vec![
            PermissionOption::new(
                PERMISSION_ALLOW_ONCE,
                "Allow once",
                PermissionOptionKind::AllowOnce,
            ),
            PermissionOption::new(
                PERMISSION_REJECT_ONCE,
                "Reject once",
                PermissionOptionKind::RejectOnce,
            ),
        ],
    )
}

fn initial_tool_call(tool_event: &ToolEventData) -> ToolCall {
    let tool_id = tool_event.tool_id().to_string();
    let tool_name = tool_event.tool_name();
    ToolCall::new(tool_id, tool_title(tool_name))
        .kind(tool_kind(tool_name))
        .status(ToolCallStatus::Pending)
        .raw_input(serde_json::json!({}))
}

fn tool_call_update(tool_event: &ToolEventData) -> Option<ToolCallUpdate> {
    let tool_id = tool_event.tool_id().to_string();
    let fields = match tool_event {
        ToolEventData::EarlyDetected { tool_name, .. } => ToolCallUpdateFields::new()
            .title(tool_title(tool_name))
            .kind(tool_kind(tool_name))
            .status(ToolCallStatus::Pending),
        ToolEventData::ParamsPartial {
            tool_name, params, ..
        } => {
            let fields = ToolCallUpdateFields::new().status(ToolCallStatus::Pending);
            if is_write_like_tool(tool_name) {
                match serde_json::from_str::<serde_json::Value>(params) {
                    Ok(value) => fields
                        .raw_input(sanitize_tool_input(tool_name, value.clone()))
                        .content(vec![text_content(write_input_status_text(&value))]),
                    Err(_) => fields.content(vec![text_content(format!(
                        "Writing file ({} bytes received so far).",
                        params.len()
                    ))]),
                }
            } else {
                fields.content(vec![text_content(format!("Input: {}", params))])
            }
        }
        ToolEventData::Queued { position, .. } => ToolCallUpdateFields::new()
            .status(ToolCallStatus::Pending)
            .content(vec![text_content(format!(
                "Queued at position {}.",
                position
            ))]),
        ToolEventData::Waiting { dependencies, .. } => ToolCallUpdateFields::new()
            .status(ToolCallStatus::Pending)
            .content(vec![text_content(format!(
                "Waiting for dependencies: {}.",
                dependencies.join(", ")
            ))]),
        ToolEventData::Started {
            tool_name, params, ..
        } => ToolCallUpdateFields::new()
            .title(tool_title(tool_name))
            .kind(tool_kind(tool_name))
            .status(ToolCallStatus::InProgress)
            .locations(tool_locations(params))
            .raw_input(sanitize_tool_input(tool_name, params.clone())),
        ToolEventData::Progress {
            message,
            percentage,
            ..
        } => ToolCallUpdateFields::new()
            .status(ToolCallStatus::InProgress)
            .content(vec![text_content(format!(
                "{} ({:.0}%)",
                message, percentage
            ))]),
        ToolEventData::Streaming {
            chunks_received, ..
        } => ToolCallUpdateFields::new()
            .status(ToolCallStatus::InProgress)
            .content(vec![text_content(format!(
                "Received {} streaming chunks.",
                chunks_received
            ))]),
        ToolEventData::StreamChunk { data, .. } => ToolCallUpdateFields::new()
            .status(ToolCallStatus::InProgress)
            .content(vec![text_content(value_to_display_text(data))]),
        ToolEventData::ConfirmationNeeded {
            tool_name, params, ..
        } => ToolCallUpdateFields::new()
            .title(format!("Allow {}?", tool_name))
            .status(ToolCallStatus::Pending)
            .locations(tool_locations(params))
            .raw_input(sanitize_tool_input(tool_name, params.clone()))
            .content(vec![text_content("Waiting for permission.")]),
        ToolEventData::Confirmed { .. } => ToolCallUpdateFields::new()
            .status(ToolCallStatus::InProgress)
            .content(vec![text_content("Permission granted.")]),
        ToolEventData::Rejected { .. } => ToolCallUpdateFields::new()
            .status(ToolCallStatus::Failed)
            .content(vec![text_content("Permission rejected.")]),
        ToolEventData::Completed {
            tool_name,
            result,
            result_for_assistant,
            duration_ms,
            ..
        } => {
            let raw_output = sanitize_tool_payload(tool_name, result.clone());
            let display = result_for_assistant
                .clone()
                .unwrap_or_else(|| value_to_display_text(&raw_output));
            ToolCallUpdateFields::new()
                .status(ToolCallStatus::Completed)
                .locations(tool_locations(&raw_output))
                .raw_output(raw_output)
                .content(vec![text_content(format!(
                    "{}\nCompleted in {} ms.",
                    display, duration_ms
                ))])
        }
        ToolEventData::Failed { error, .. } => ToolCallUpdateFields::new()
            .status(ToolCallStatus::Failed)
            .raw_output(serde_json::json!({ "error": error }))
            .content(vec![text_content(format!("Error: {}", error))]),
        ToolEventData::Cancelled { reason, .. } => ToolCallUpdateFields::new()
            .status(ToolCallStatus::Failed)
            .raw_output(serde_json::json!({ "reason": reason }))
            .content(vec![text_content(format!("Cancelled: {}", reason))]),
    };

    Some(ToolCallUpdate::new(tool_id, fields))
}

fn tool_title(tool_name: &str) -> String {
    format!("Run {}", tool_name)
}

fn tool_kind(tool_name: &str) -> ToolKind {
    let name = tool_name.to_ascii_lowercase();
    if name.contains("delete") || name.contains("remove") {
        ToolKind::Delete
    } else if name.contains("write")
        || name.contains("edit")
        || name.contains("patch")
        || name.contains("replace")
    {
        ToolKind::Edit
    } else if name.contains("move") || name.contains("rename") {
        ToolKind::Move
    } else if name.contains("grep")
        || name.contains("glob")
        || name.contains("search")
        || name.contains("find")
    {
        ToolKind::Search
    } else if name.contains("bash")
        || name.contains("terminal")
        || name.contains("command")
        || name.contains("execute")
    {
        ToolKind::Execute
    } else if name.contains("web") || name.contains("fetch") || name.contains("http") {
        ToolKind::Fetch
    } else if name.contains("think") || name.contains("plan") {
        ToolKind::Think
    } else if name.contains("read") || name == "ls" {
        ToolKind::Read
    } else {
        ToolKind::Other
    }
}

fn tool_locations(input: &serde_json::Value) -> Vec<ToolCallLocation> {
    input
        .get("file_path")
        .or_else(|| input.get("path"))
        .and_then(|value| value.as_str())
        .filter(|path| !path.trim().is_empty())
        .map(|path| vec![ToolCallLocation::new(PathBuf::from(path))])
        .unwrap_or_default()
}

fn is_write_like_tool(tool_name: &str) -> bool {
    matches!(
        tool_name.to_ascii_lowercase().as_str(),
        "write" | "file_write" | "write_file" | "write_notebook"
    )
}

fn sanitize_tool_input(tool_name: &str, mut input: serde_json::Value) -> serde_json::Value {
    if !is_large_text_payload_tool(tool_name) {
        return input;
    }

    let Some(object) = input.as_object_mut() else {
        return input;
    };

    sanitize_large_text_fields(object);

    input
}

fn sanitize_tool_payload(tool_name: &str, mut payload: serde_json::Value) -> serde_json::Value {
    if !is_large_text_payload_tool(tool_name) {
        return payload;
    }

    let Some(object) = payload.as_object_mut() else {
        return payload;
    };

    sanitize_large_text_fields(object);

    payload
}

fn is_large_text_payload_tool(tool_name: &str) -> bool {
    matches!(
        tool_name.to_ascii_lowercase().as_str(),
        "write"
            | "file_write"
            | "write_file"
            | "write_notebook"
            | "edit"
            | "file_edit"
            | "search_replace"
    )
}

fn sanitize_large_text_fields(object: &mut serde_json::Map<String, serde_json::Value>) {
    for key in ["content", "contents", "old_string", "new_string"] {
        let Some(value) = object.get_mut(key) else {
            continue;
        };
        let Some(content) = value.as_str() else {
            continue;
        };

        let content_len = content.len();
        let Some(preview) = large_text_preview(content) else {
            continue;
        };
        *value = serde_json::Value::String(preview);
        object.insert(format!("{}_bytes", key), serde_json::json!(content_len));
        object.insert(format!("{}_truncated", key), serde_json::json!(true));
    }
}

fn large_text_preview(content: &str) -> Option<String> {
    if content.chars().count() <= ACP_LARGE_TEXT_PREVIEW_CHARS {
        return None;
    }

    Some(truncate_chars(content, ACP_LARGE_TEXT_PREVIEW_CHARS))
}

fn write_input_status_text(input: &serde_json::Value) -> String {
    let path = input
        .get("file_path")
        .or_else(|| input.get("path"))
        .and_then(|value| value.as_str())
        .unwrap_or("file");
    let content_len = input
        .get("content")
        // Legacy alias kept for replaying older Write tool-call transcripts.
        .or_else(|| input.get("contents"))
        .and_then(|value| value.as_str())
        .map(str::len)
        .unwrap_or(0);

    format!("Writing {} ({} bytes).", path, content_len)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    value.chars().take(max_chars).collect()
}

fn text_content(text: impl Into<String>) -> ToolCallContent {
    ToolCallContent::from(text.into())
}

fn value_to_display_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        _ => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    }
}

trait ToolEventExt {
    fn tool_id(&self) -> &str;
    fn tool_name(&self) -> &str;
}

impl ToolEventExt for ToolEventData {
    fn tool_id(&self) -> &str {
        match self {
            Self::EarlyDetected { tool_id, .. }
            | Self::ParamsPartial { tool_id, .. }
            | Self::Queued { tool_id, .. }
            | Self::Waiting { tool_id, .. }
            | Self::Started { tool_id, .. }
            | Self::Progress { tool_id, .. }
            | Self::Streaming { tool_id, .. }
            | Self::StreamChunk { tool_id, .. }
            | Self::ConfirmationNeeded { tool_id, .. }
            | Self::Confirmed { tool_id, .. }
            | Self::Rejected { tool_id, .. }
            | Self::Completed { tool_id, .. }
            | Self::Failed { tool_id, .. }
            | Self::Cancelled { tool_id, .. } => tool_id,
        }
    }

    fn tool_name(&self) -> &str {
        match self {
            Self::EarlyDetected { tool_name, .. }
            | Self::ParamsPartial { tool_name, .. }
            | Self::Queued { tool_name, .. }
            | Self::Waiting { tool_name, .. }
            | Self::Started { tool_name, .. }
            | Self::Progress { tool_name, .. }
            | Self::Streaming { tool_name, .. }
            | Self::StreamChunk { tool_name, .. }
            | Self::ConfirmationNeeded { tool_name, .. }
            | Self::Confirmed { tool_name, .. }
            | Self::Rejected { tool_name, .. }
            | Self::Completed { tool_name, .. }
            | Self::Failed { tool_name, .. }
            | Self::Cancelled { tool_name, .. } => tool_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_detected_creates_tool_call_once() {
        let mut seen = HashSet::new();
        let event = ToolEventData::EarlyDetected {
            tool_id: "tool-1".to_string(),
            tool_name: "Read".to_string(),
        };

        let first = tool_event_updates(&event, &mut seen);
        assert_eq!(first.len(), 2);
        assert!(matches!(first[0], SessionUpdate::ToolCall(_)));
        assert!(matches!(first[1], SessionUpdate::ToolCallUpdate(_)));

        let second = tool_event_updates(&event, &mut seen);
        assert_eq!(second.len(), 1);
        assert!(matches!(second[0], SessionUpdate::ToolCallUpdate(_)));
    }

    #[test]
    fn completed_event_maps_to_completed_update_with_output() {
        let mut seen = HashSet::new();
        let event = ToolEventData::Completed {
            tool_id: "tool-1".to_string(),
            tool_name: "Bash".to_string(),
            result: serde_json::json!({ "stdout": "ok" }),
            result_for_assistant: Some("done".to_string()),
            duration_ms: 42,
            queue_wait_ms: None,
            preflight_ms: None,
            confirmation_wait_ms: None,
            execution_ms: None,
        };

        let updates = tool_event_updates(&event, &mut seen);
        let SessionUpdate::ToolCallUpdate(update) = &updates[1] else {
            panic!("expected tool call update");
        };

        assert_eq!(update.fields.status, Some(ToolCallStatus::Completed));
        assert_eq!(
            update.fields.raw_output,
            Some(serde_json::json!({ "stdout": "ok" }))
        );
    }

    #[test]
    fn write_started_redacts_large_content_from_raw_input() {
        let mut seen = HashSet::new();
        let content = "x".repeat(ACP_LARGE_TEXT_PREVIEW_CHARS + 10);
        let event = ToolEventData::Started {
            tool_id: "tool-1".to_string(),
            tool_name: "Write".to_string(),
            params: serde_json::json!({
                "file_path": "src/lib.rs",
                "content": content,
            }),
            timeout_seconds: None,
        };

        let updates = tool_event_updates(&event, &mut seen);
        let SessionUpdate::ToolCall(tool_call) = &updates[0] else {
            panic!("expected initial tool call");
        };
        assert_eq!(tool_call.status, ToolCallStatus::Pending);
        assert_eq!(tool_call.raw_input, Some(serde_json::json!({})));

        let SessionUpdate::ToolCallUpdate(update) = &updates[1] else {
            panic!("expected tool call update");
        };
        assert_eq!(update.fields.status, Some(ToolCallStatus::InProgress));
        assert_eq!(
            update.fields.locations.as_ref().unwrap()[0].path,
            PathBuf::from("src/lib.rs")
        );

        let raw_input = update
            .fields
            .raw_input
            .as_ref()
            .expect("raw input should be present");

        assert_eq!(raw_input["file_path"], "src/lib.rs");
        assert_eq!(
            raw_input["content_bytes"],
            ACP_LARGE_TEXT_PREVIEW_CHARS + 10
        );
        assert_eq!(
            raw_input["content"].as_str().unwrap().len(),
            ACP_LARGE_TEXT_PREVIEW_CHARS
        );
        assert_eq!(raw_input["content_truncated"], true);
    }

    #[test]
    fn write_params_partial_sends_bounded_raw_input() {
        let mut seen = HashSet::new();
        let content = "a".repeat(ACP_LARGE_TEXT_PREVIEW_CHARS + 25);
        let event = ToolEventData::ParamsPartial {
            tool_id: "tool-1".to_string(),
            tool_name: "Write".to_string(),
            params: serde_json::json!({
                "file_path": "src/main.rs",
                "content": content,
            })
            .to_string(),
        };

        let updates = tool_event_updates(&event, &mut seen);
        let SessionUpdate::ToolCallUpdate(update) = &updates[1] else {
            panic!("expected tool call update");
        };
        let raw_input = update
            .fields
            .raw_input
            .as_ref()
            .expect("raw input should be present");

        assert_eq!(raw_input["file_path"], "src/main.rs");
        assert_eq!(
            raw_input["content_bytes"],
            ACP_LARGE_TEXT_PREVIEW_CHARS + 25
        );
        assert_eq!(
            raw_input["content"].as_str().unwrap().len(),
            ACP_LARGE_TEXT_PREVIEW_CHARS
        );
        assert_eq!(raw_input["content_truncated"], true);
    }

    #[test]
    fn write_started_sends_small_content_on_in_progress_update() {
        let mut seen = HashSet::new();
        let event = ToolEventData::Started {
            tool_id: "tool-1".to_string(),
            tool_name: "Write".to_string(),
            params: serde_json::json!({
                "file_path": "tiny.txt",
                "content": "hello\n",
            }),
            timeout_seconds: None,
        };

        let updates = tool_event_updates(&event, &mut seen);
        let SessionUpdate::ToolCall(tool_call) = &updates[0] else {
            panic!("expected initial tool call");
        };
        assert_eq!(tool_call.status, ToolCallStatus::Pending);
        assert_eq!(tool_call.raw_input, Some(serde_json::json!({})));

        let SessionUpdate::ToolCallUpdate(update) = &updates[1] else {
            panic!("expected tool call update");
        };
        let raw_input = update
            .fields
            .raw_input
            .as_ref()
            .expect("raw input should be present");

        assert_eq!(update.fields.status, Some(ToolCallStatus::InProgress));
        assert_eq!(
            update.fields.locations.as_ref().unwrap()[0].path,
            PathBuf::from("tiny.txt")
        );
        assert_eq!(raw_input["file_path"], "tiny.txt");
        assert_eq!(raw_input["content"], "hello\n");
        assert!(raw_input.get("content_bytes").is_none());
        assert!(raw_input.get("content_truncated").is_none());
    }

    #[test]
    fn edit_started_redacts_large_strings_from_raw_input() {
        let mut seen = HashSet::new();
        let old_string = "old".repeat(ACP_LARGE_TEXT_PREVIEW_CHARS);
        let new_string = "new".repeat(ACP_LARGE_TEXT_PREVIEW_CHARS);
        let event = ToolEventData::Started {
            tool_id: "tool-1".to_string(),
            tool_name: "Edit".to_string(),
            params: serde_json::json!({
                "file_path": "src/lib.rs",
                "old_string": old_string,
                "new_string": new_string,
            }),
            timeout_seconds: None,
        };

        let updates = tool_event_updates(&event, &mut seen);
        let SessionUpdate::ToolCallUpdate(update) = &updates[1] else {
            panic!("expected tool call update");
        };
        let raw_input = update
            .fields
            .raw_input
            .as_ref()
            .expect("raw input should be present");

        assert_eq!(update.fields.status, Some(ToolCallStatus::InProgress));
        assert_eq!(
            update.fields.locations.as_ref().unwrap()[0].path,
            PathBuf::from("src/lib.rs")
        );
        assert_eq!(raw_input["file_path"], "src/lib.rs");
        assert_eq!(
            raw_input["old_string_bytes"],
            ACP_LARGE_TEXT_PREVIEW_CHARS * 3
        );
        assert_eq!(
            raw_input["new_string_bytes"],
            ACP_LARGE_TEXT_PREVIEW_CHARS * 3
        );
        assert_eq!(
            raw_input["old_string"].as_str().unwrap().len(),
            ACP_LARGE_TEXT_PREVIEW_CHARS
        );
        assert_eq!(
            raw_input["new_string"].as_str().unwrap().len(),
            ACP_LARGE_TEXT_PREVIEW_CHARS
        );
        assert_eq!(raw_input["old_string_truncated"], true);
        assert_eq!(raw_input["new_string_truncated"], true);
    }

    #[test]
    fn edit_completed_redacts_large_strings_from_raw_output() {
        let mut seen = HashSet::new();
        let old_string = "old".repeat(ACP_LARGE_TEXT_PREVIEW_CHARS);
        let new_string = "new".repeat(ACP_LARGE_TEXT_PREVIEW_CHARS);
        let event = ToolEventData::Completed {
            tool_id: "tool-1".to_string(),
            tool_name: "Edit".to_string(),
            result: serde_json::json!({
                "file_path": "src/lib.rs",
                "old_string": old_string,
                "new_string": new_string,
                "success": true,
            }),
            result_for_assistant: None,
            duration_ms: 15,
            queue_wait_ms: None,
            preflight_ms: None,
            confirmation_wait_ms: None,
            execution_ms: None,
        };

        let updates = tool_event_updates(&event, &mut seen);
        let SessionUpdate::ToolCallUpdate(update) = &updates[1] else {
            panic!("expected tool call update");
        };
        let raw_output = update
            .fields
            .raw_output
            .as_ref()
            .expect("raw output should be present");

        assert_eq!(raw_output["file_path"], "src/lib.rs");
        assert_eq!(
            raw_output["old_string_bytes"],
            ACP_LARGE_TEXT_PREVIEW_CHARS * 3
        );
        assert_eq!(
            raw_output["new_string_bytes"],
            ACP_LARGE_TEXT_PREVIEW_CHARS * 3
        );
        assert_eq!(
            raw_output["old_string"].as_str().unwrap().len(),
            ACP_LARGE_TEXT_PREVIEW_CHARS
        );
        assert_eq!(
            raw_output["new_string"].as_str().unwrap().len(),
            ACP_LARGE_TEXT_PREVIEW_CHARS
        );
        assert_eq!(raw_output["old_string_truncated"], true);
        assert_eq!(raw_output["new_string_truncated"], true);
    }

    #[test]
    fn permission_request_exposes_allow_and_reject_once() {
        let request = permission_request(
            "session-1",
            "tool-1",
            "FileWrite",
            &serde_json::json!({ "path": "a.txt" }),
        );

        assert_eq!(request.options.len(), 2);
        assert_eq!(
            request.options[0].option_id.to_string(),
            PERMISSION_ALLOW_ONCE
        );
        assert_eq!(request.options[0].kind, PermissionOptionKind::AllowOnce);
        assert_eq!(
            request.options[1].option_id.to_string(),
            PERMISSION_REJECT_ONCE
        );
        assert_eq!(request.options[1].kind, PermissionOptionKind::RejectOnce);
    }
}
