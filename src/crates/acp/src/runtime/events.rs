use std::collections::HashSet;

use agent_client_protocol::schema::{
    PermissionOption, PermissionOptionKind, RequestPermissionRequest, SessionId,
    SessionNotification, SessionUpdate, ToolCall, ToolCallContent, ToolCallStatus, ToolCallUpdate,
    ToolCallUpdateFields, ToolKind,
};
use agent_client_protocol::{Client, ConnectionTo, Result};
use bitfun_events::ToolEventData;

pub(super) const PERMISSION_ALLOW_ONCE: &str = "allow_once";
pub(super) const PERMISSION_REJECT_ONCE: &str = "reject_once";

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
                .raw_input(params.clone())
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
    let mut tool_call = ToolCall::new(tool_id, tool_title(tool_name))
        .kind(tool_kind(tool_name))
        .status(tool_status(tool_event));

    if let Some(raw_input) = tool_event.raw_input() {
        tool_call = tool_call.raw_input(raw_input);
    }

    tool_call
}

fn tool_call_update(tool_event: &ToolEventData) -> Option<ToolCallUpdate> {
    let tool_id = tool_event.tool_id().to_string();
    let fields = match tool_event {
        ToolEventData::EarlyDetected { tool_name, .. } => ToolCallUpdateFields::new()
            .title(tool_title(tool_name))
            .kind(tool_kind(tool_name))
            .status(ToolCallStatus::Pending),
        ToolEventData::ParamsPartial { params, .. } => ToolCallUpdateFields::new()
            .status(ToolCallStatus::Pending)
            .content(vec![text_content(format!("Input: {}", params))]),
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
            .raw_input(params.clone()),
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
            .raw_input(params.clone())
            .content(vec![text_content("Waiting for permission.")]),
        ToolEventData::Confirmed { .. } => ToolCallUpdateFields::new()
            .status(ToolCallStatus::InProgress)
            .content(vec![text_content("Permission granted.")]),
        ToolEventData::Rejected { .. } => ToolCallUpdateFields::new()
            .status(ToolCallStatus::Failed)
            .content(vec![text_content("Permission rejected.")]),
        ToolEventData::Completed {
            result,
            result_for_assistant,
            duration_ms,
            ..
        } => {
            let display = result_for_assistant
                .clone()
                .unwrap_or_else(|| value_to_display_text(result));
            ToolCallUpdateFields::new()
                .status(ToolCallStatus::Completed)
                .raw_output(result.clone())
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

fn tool_status(tool_event: &ToolEventData) -> ToolCallStatus {
    match tool_event {
        ToolEventData::Started { .. }
        | ToolEventData::Progress { .. }
        | ToolEventData::Streaming { .. }
        | ToolEventData::StreamChunk { .. }
        | ToolEventData::Confirmed { .. } => ToolCallStatus::InProgress,
        ToolEventData::Completed { .. } => ToolCallStatus::Completed,
        ToolEventData::Failed { .. }
        | ToolEventData::Cancelled { .. }
        | ToolEventData::Rejected { .. } => ToolCallStatus::Failed,
        _ => ToolCallStatus::Pending,
    }
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
    fn raw_input(&self) -> Option<serde_json::Value>;
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

    fn raw_input(&self) -> Option<serde_json::Value> {
        match self {
            Self::Started { params, .. } | Self::ConfirmationNeeded { params, .. } => {
                Some(params.clone())
            }
            _ => None,
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
