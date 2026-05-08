use agent_client_protocol::schema::{
    ContentBlock, ContentChunk, SessionNotification, SessionUpdate, ToolCall, ToolCallContent,
    ToolCallStatus, ToolCallUpdate,
};
use agent_client_protocol::util::MatchDispatch;
use bitfun_core::util::errors::{BitFunError, BitFunResult};
use bitfun_events::ToolEventData;

use super::tool_card_bridge::{acp_tool_name, normalize_tool_params};

#[derive(Debug, Clone)]
pub enum AcpClientStreamEvent {
    ModelRoundStarted {
        round_id: String,
        round_index: usize,
        disable_explore_grouping: bool,
    },
    AgentText(String),
    AgentThought(String),
    ToolEvent(ToolEventData),
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcpStreamItemKind {
    Text,
    Tool,
}

#[derive(Debug, Default)]
pub(super) struct AcpStreamRoundTracker {
    next_round_index: usize,
    last_item_kind: Option<AcpStreamItemKind>,
}

impl AcpStreamRoundTracker {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn apply(&mut self, event: AcpClientStreamEvent) -> Vec<AcpClientStreamEvent> {
        match event {
            AcpClientStreamEvent::AgentText(_) | AcpClientStreamEvent::AgentThought(_) => {
                let mut events = Vec::new();
                if self.last_item_kind.is_none()
                    || self.last_item_kind == Some(AcpStreamItemKind::Tool)
                {
                    events.push(self.next_round_started_event());
                }
                self.last_item_kind = Some(AcpStreamItemKind::Text);
                events.push(event);
                events
            }
            AcpClientStreamEvent::ToolEvent(_) => {
                let mut events = Vec::new();
                if self.last_item_kind.is_none() {
                    events.push(self.next_round_started_event());
                }
                self.last_item_kind = Some(AcpStreamItemKind::Tool);
                events.push(event);
                events
            }
            AcpClientStreamEvent::ModelRoundStarted { .. }
            | AcpClientStreamEvent::Completed
            | AcpClientStreamEvent::Cancelled => vec![event],
        }
    }

    fn next_round_started_event(&mut self) -> AcpClientStreamEvent {
        let round_index = self.next_round_index;
        self.next_round_index += 1;
        AcpClientStreamEvent::ModelRoundStarted {
            round_id: format!(
                "round_{}_{}",
                chrono::Utc::now().timestamp_millis(),
                uuid::Uuid::new_v4()
            ),
            round_index,
            disable_explore_grouping: true,
        }
    }
}

pub async fn acp_dispatch_to_stream_events(
    dispatch: agent_client_protocol::Dispatch,
) -> BitFunResult<Vec<AcpClientStreamEvent>> {
    let mut events = Vec::new();
    MatchDispatch::new(dispatch)
        .if_notification(async |notification: SessionNotification| {
            match notification.update {
                SessionUpdate::AgentMessageChunk(chunk) => {
                    if let Some(text) = content_chunk_text(chunk) {
                        events.push(AcpClientStreamEvent::AgentText(text));
                    }
                }
                SessionUpdate::AgentThoughtChunk(chunk) => {
                    if let Some(text) = content_chunk_text(chunk) {
                        events.push(AcpClientStreamEvent::AgentThought(text));
                    }
                }
                SessionUpdate::ToolCall(tool_call) => {
                    events.extend(acp_tool_call_events(tool_call));
                }
                SessionUpdate::ToolCallUpdate(tool_call_update) => {
                    if let Some(event) = acp_tool_call_update_event(tool_call_update) {
                        events.push(event);
                    }
                }
                _ => {}
            }
            Ok(())
        })
        .await
        .otherwise_ignore()
        .map_err(protocol_error)?;
    Ok(events)
}

fn content_chunk_text(chunk: ContentChunk) -> Option<String> {
    match chunk.content {
        ContentBlock::Text(text) => Some(text.text),
        _ => None,
    }
}

fn acp_tool_call_events(tool_call: ToolCall) -> Vec<AcpClientStreamEvent> {
    let tool_id = tool_call.tool_call_id.to_string();
    let tool_name = acp_tool_name(
        &tool_call.title,
        tool_call.raw_input.as_ref(),
        Some(&tool_call.kind),
    );
    let params = normalize_tool_params(
        &tool_name,
        tool_call.raw_input.clone().unwrap_or_else(|| {
            serde_json::json!({
                "title": tool_call.title,
                "kind": format!("{:?}", tool_call.kind),
            })
        }),
    );

    let mut events = vec![AcpClientStreamEvent::ToolEvent(ToolEventData::Started {
        tool_id: tool_id.clone(),
        tool_name: tool_name.clone(),
        params,
        timeout_seconds: None,
    })];

    match tool_call.status {
        ToolCallStatus::Completed => {
            events.push(AcpClientStreamEvent::ToolEvent(ToolEventData::Completed {
                tool_id,
                tool_name,
                result: acp_tool_result_value(
                    tool_call.raw_output,
                    Some(tool_call.content),
                    Some(tool_call.locations),
                ),
                result_for_assistant: None,
                duration_ms: 0,
            }));
        }
        ToolCallStatus::Failed => {
            events.push(AcpClientStreamEvent::ToolEvent(ToolEventData::Failed {
                tool_id,
                tool_name,
                error: acp_tool_error_text(tool_call.raw_output, tool_call.content),
            }));
        }
        ToolCallStatus::Pending | ToolCallStatus::InProgress => {}
        _ => {}
    }

    events
}

fn acp_tool_call_update_event(update: ToolCallUpdate) -> Option<AcpClientStreamEvent> {
    let tool_id = update.tool_call_id.to_string();
    let title = update.fields.title.unwrap_or_else(|| tool_id.clone());
    let tool_name = acp_tool_name(
        &title,
        update.fields.raw_input.as_ref(),
        update.fields.kind.as_ref(),
    );

    match update.fields.status {
        Some(ToolCallStatus::Completed) => {
            Some(AcpClientStreamEvent::ToolEvent(ToolEventData::Completed {
                tool_id,
                tool_name,
                result: acp_tool_result_value(
                    update.fields.raw_output,
                    update.fields.content,
                    update.fields.locations,
                ),
                result_for_assistant: None,
                duration_ms: 0,
            }))
        }
        Some(ToolCallStatus::Failed) => {
            Some(AcpClientStreamEvent::ToolEvent(ToolEventData::Failed {
                tool_id,
                tool_name,
                error: acp_tool_error_text(
                    update.fields.raw_output,
                    update.fields.content.unwrap_or_default(),
                ),
            }))
        }
        Some(ToolCallStatus::InProgress) | Some(ToolCallStatus::Pending) | Some(_) => {
            let params = normalize_tool_params(
                &tool_name,
                update.fields.raw_input.unwrap_or_else(|| {
                    serde_json::json!({
                        "title": title,
                    })
                }),
            );
            Some(AcpClientStreamEvent::ToolEvent(ToolEventData::Started {
                tool_id,
                tool_name,
                params,
                timeout_seconds: None,
            }))
        }
        None => update.fields.raw_input.map(|params| {
            let params = normalize_tool_params(&tool_name, params);
            AcpClientStreamEvent::ToolEvent(ToolEventData::Started {
                tool_id,
                tool_name,
                params,
                timeout_seconds: None,
            })
        }),
    }
}

fn acp_tool_result_value(
    raw_output: Option<serde_json::Value>,
    content: Option<Vec<ToolCallContent>>,
    locations: Option<Vec<agent_client_protocol::schema::ToolCallLocation>>,
) -> serde_json::Value {
    if let Some(raw_output) = raw_output {
        return raw_output;
    }

    let content = content.unwrap_or_default();
    let locations = locations.unwrap_or_default();
    if content.is_empty() && locations.is_empty() {
        return serde_json::Value::Null;
    }

    serde_json::json!({
        "content": content,
        "locations": locations,
    })
}

fn acp_tool_error_text(
    raw_output: Option<serde_json::Value>,
    content: Vec<ToolCallContent>,
) -> String {
    if let Some(raw_output) = raw_output {
        return value_to_display_text(&raw_output);
    }
    if !content.is_empty() {
        return serde_json::to_string_pretty(&content).unwrap_or_else(|_| {
            serde_json::to_string(&content).unwrap_or_else(|_| "ACP tool failed".to_string())
        });
    }
    "ACP tool failed".to_string()
}

fn value_to_display_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        _ => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    }
}

fn protocol_error(error: impl std::fmt::Display) -> BitFunError {
    BitFunError::service(format!("ACP protocol error: {}", error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool_event(id: &str) -> AcpClientStreamEvent {
        AcpClientStreamEvent::ToolEvent(ToolEventData::Started {
            tool_id: id.to_string(),
            tool_name: "Bash".to_string(),
            params: json!({ "command": "echo ok" }),
            timeout_seconds: None,
        })
    }

    fn event_kinds(events: &[AcpClientStreamEvent]) -> Vec<&'static str> {
        events
            .iter()
            .map(|event| match event {
                AcpClientStreamEvent::ModelRoundStarted { .. } => "round",
                AcpClientStreamEvent::AgentText(_) => "text",
                AcpClientStreamEvent::AgentThought(_) => "thought",
                AcpClientStreamEvent::ToolEvent(_) => "tool",
                AcpClientStreamEvent::Completed => "completed",
                AcpClientStreamEvent::Cancelled => "cancelled",
            })
            .collect()
    }

    #[test]
    fn starts_new_round_for_text_after_tool() {
        let mut tracker = AcpStreamRoundTracker::new();
        let mut events = Vec::new();
        events.extend(tracker.apply(AcpClientStreamEvent::AgentText("before".to_string())));
        events.extend(tracker.apply(tool_event("tool-1")));
        events.extend(tracker.apply(AcpClientStreamEvent::AgentText("after".to_string())));

        assert_eq!(
            event_kinds(&events),
            vec!["round", "text", "tool", "round", "text"]
        );
        assert!(matches!(
            events[0],
            AcpClientStreamEvent::ModelRoundStarted { round_index: 0, .. }
        ));
        assert!(matches!(
            events[3],
            AcpClientStreamEvent::ModelRoundStarted { round_index: 1, .. }
        ));
    }

    #[test]
    fn keeps_consecutive_tools_in_one_round_before_text() {
        let mut tracker = AcpStreamRoundTracker::new();
        let mut events = Vec::new();
        events.extend(tracker.apply(tool_event("tool-1")));
        events.extend(tracker.apply(tool_event("tool-2")));
        events.extend(tracker.apply(AcpClientStreamEvent::AgentText("done".to_string())));

        assert_eq!(
            event_kinds(&events),
            vec!["round", "tool", "tool", "round", "text"]
        );
    }

    #[test]
    fn keeps_consecutive_text_in_one_round() {
        let mut tracker = AcpStreamRoundTracker::new();
        let mut events = Vec::new();
        events.extend(tracker.apply(AcpClientStreamEvent::AgentText("a".to_string())));
        events.extend(tracker.apply(AcpClientStreamEvent::AgentText("b".to_string())));

        assert_eq!(event_kinds(&events), vec!["round", "text", "text"]);
    }
}
