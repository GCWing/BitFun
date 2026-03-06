//! Session bridge: translates remote commands into local session operations.
//!
//! Mobile clients send encrypted commands via the relay (HTTP → WS bridge).
//! The desktop decrypts, dispatches, and returns encrypted responses.
//!
//! Instead of streaming events to the mobile, the desktop maintains an
//! in-memory `RemoteSessionStateTracker` per session. The mobile polls
//! for state changes using the `PollSession` command, receiving only
//! incremental updates (new messages + current active turn snapshot).

use anyhow::{anyhow, Result};
use dashmap::DashMap;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use super::encryption;

/// Image sent from mobile as a base64 data-URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageAttachment {
    pub name: String,
    pub data_url: String,
}

/// Commands that the mobile client can send to the desktop.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum RemoteCommand {
    GetWorkspaceInfo,
    ListRecentWorkspaces,
    SetWorkspace {
        path: String,
    },
    ListSessions {
        workspace_path: Option<String>,
        limit: Option<usize>,
        offset: Option<usize>,
    },
    CreateSession {
        agent_type: Option<String>,
        session_name: Option<String>,
        workspace_path: Option<String>,
    },
    GetSessionMessages {
        session_id: String,
        limit: Option<usize>,
        before_message_id: Option<String>,
    },
    SendMessage {
        session_id: String,
        content: String,
        agent_type: Option<String>,
        images: Option<Vec<ImageAttachment>>,
    },
    CancelTask {
        session_id: String,
        turn_id: Option<String>,
    },
    DeleteSession {
        session_id: String,
    },
    ConfirmTool {
        tool_id: String,
        updated_input: Option<serde_json::Value>,
    },
    RejectTool {
        tool_id: String,
        reason: Option<String>,
    },
    CancelTool {
        tool_id: String,
        reason: Option<String>,
    },
    /// Submit answers for an AskUserQuestion tool.
    AnswerQuestion {
        tool_id: String,
        answers: serde_json::Value,
    },
    /// Incremental poll — returns only what changed since `since_version`.
    PollSession {
        session_id: String,
        since_version: u64,
        known_msg_count: usize,
    },
    Ping,
}

/// Responses sent from desktop back to mobile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "resp", rename_all = "snake_case")]
pub enum RemoteResponse {
    WorkspaceInfo {
        has_workspace: bool,
        path: Option<String>,
        project_name: Option<String>,
        git_branch: Option<String>,
    },
    RecentWorkspaces {
        workspaces: Vec<RecentWorkspaceEntry>,
    },
    WorkspaceUpdated {
        success: bool,
        path: Option<String>,
        project_name: Option<String>,
        error: Option<String>,
    },
    SessionList {
        sessions: Vec<SessionInfo>,
        has_more: bool,
    },
    SessionCreated {
        session_id: String,
    },
    Messages {
        session_id: String,
        messages: Vec<ChatMessage>,
        has_more: bool,
    },
    MessageSent {
        session_id: String,
        turn_id: String,
    },
    TaskCancelled {
        session_id: String,
    },
    SessionDeleted {
        session_id: String,
    },
    /// Pushed to mobile immediately after pairing.
    InitialSync {
        has_workspace: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        project_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        git_branch: Option<String>,
        sessions: Vec<SessionInfo>,
        has_more_sessions: bool,
    },
    /// Incremental poll response.
    SessionPoll {
        version: u64,
        changed: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_state: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        new_messages: Option<Vec<ChatMessage>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        total_msg_count: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        active_turn: Option<ActiveTurnSnapshot>,
    },
    AnswerAccepted,
    InteractionAccepted {
        action: String,
        target_id: String,
    },
    Pong,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub name: String,
    pub agent_type: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<RemoteToolStatus>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    /// Ordered items preserving the interleaved display order from the desktop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<ChatMessageItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageItem {
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<RemoteToolStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentWorkspaceEntry {
    pub path: String,
    pub name: String,
    pub last_opened: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveTurnSnapshot {
    pub turn_id: String,
    pub status: String,
    pub text: String,
    pub thinking: String,
    pub tools: Vec<RemoteToolStatus>,
    pub round_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<ChatMessageItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteToolStatus {
    pub id: String,
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_preview: Option<String>,
    /// Full tool input for interactive tools (e.g. AskUserQuestion).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<serde_json::Value>,
}

pub type EncryptedPayload = (String, String);

/// Convert ConversationPersistenceManager turns into mobile ChatMessages.
/// This is the same data source the desktop frontend uses.
fn turns_to_chat_messages(
    turns: &[crate::service::conversation::DialogTurnData],
) -> Vec<ChatMessage> {
    let mut result = Vec::new();

    for turn in turns {
        result.push(ChatMessage {
            id: turn.user_message.id.clone(),
            role: "user".to_string(),
            content: strip_user_input_tags(&turn.user_message.content),
            timestamp: (turn.user_message.timestamp / 1000).to_string(),
            metadata: None,
            tools: None,
            thinking: None,
            items: None,
        });

        // Collect ordered items across all rounds, preserving interleaved order
        struct OrderedEntry {
            order_index: Option<usize>,
            timestamp: u64,
            sequence: usize,
            item: ChatMessageItem,
        }
        let mut ordered: Vec<OrderedEntry> = Vec::new();
        let mut tools_flat = Vec::new();
        let mut thinking_parts = Vec::new();
        let mut text_parts = Vec::new();
        let mut sequence = 0usize;

        for round in &turn.model_rounds {
            for t in &round.text_items {
                if t.is_subagent_item.unwrap_or(false) {
                    continue;
                }
                if !t.content.is_empty() {
                    text_parts.push(t.content.clone());
                    ordered.push(OrderedEntry {
                        order_index: t.order_index,
                        timestamp: t.timestamp,
                        sequence,
                        item: ChatMessageItem {
                            item_type: "text".to_string(),
                            content: Some(t.content.clone()),
                            tool: None,
                        },
                    });
                    sequence += 1;
                }
            }
            for t in &round.thinking_items {
                if t.is_subagent_item.unwrap_or(false) {
                    continue;
                }
                if !t.content.is_empty() {
                    thinking_parts.push(t.content.clone());
                    ordered.push(OrderedEntry {
                        order_index: t.order_index,
                        timestamp: t.timestamp,
                        sequence,
                        item: ChatMessageItem {
                            item_type: "thinking".to_string(),
                            content: Some(t.content.clone()),
                            tool: None,
                        },
                    });
                    sequence += 1;
                }
            }
            for t in &round.tool_items {
                if t.is_subagent_item.unwrap_or(false) {
                    continue;
                }
                let status_str = t.status.as_deref().unwrap_or(
                    if t.tool_result.is_some() {
                        "completed"
                    } else {
                        "running"
                    },
                );
                let tool_status = RemoteToolStatus {
                    id: t.id.clone(),
                    name: t.tool_name.clone(),
                    status: status_str.to_string(),
                    duration_ms: t.duration_ms,
                    start_ms: Some(t.start_time),
                    input_preview: None,
                    tool_input: if t.tool_name == "AskUserQuestion" {
                        Some(t.tool_call.input.clone())
                    } else {
                        None
                    },
                };
                tools_flat.push(tool_status.clone());
                ordered.push(OrderedEntry {
                    order_index: t.order_index,
                    timestamp: t.start_time,
                    sequence,
                    item: ChatMessageItem {
                        item_type: "tool".to_string(),
                        content: None,
                        tool: Some(tool_status),
                    },
                });
                sequence += 1;
            }
        }

        ordered.sort_by(|a, b| match (a.order_index, b.order_index) {
            (Some(a_idx), Some(b_idx)) => a_idx
                .cmp(&b_idx)
                .then_with(|| a.timestamp.cmp(&b.timestamp))
                .then_with(|| a.sequence.cmp(&b.sequence)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a
                .timestamp
                .cmp(&b.timestamp)
                .then_with(|| a.sequence.cmp(&b.sequence)),
        });
        let items: Vec<ChatMessageItem> = ordered.into_iter().map(|e| e.item).collect();

        let ts = turn
            .model_rounds
            .last()
            .map(|r| r.end_time.unwrap_or(r.start_time))
            .unwrap_or(turn.start_time);

        result.push(ChatMessage {
            id: format!("{}_assistant", turn.turn_id),
            role: "assistant".to_string(),
            content: text_parts.join("\n\n"),
            timestamp: (ts / 1000).to_string(),
            metadata: None,
            tools: if tools_flat.is_empty() { None } else { Some(tools_flat) },
            thinking: if thinking_parts.is_empty() {
                None
            } else {
                Some(thinking_parts.join("\n\n"))
            },
            items: if items.is_empty() { None } else { Some(items) },
        });
    }

    result
}

/// Load historical chat messages from ConversationPersistenceManager.
/// Uses the same data source as the desktop frontend.
async fn load_chat_messages_from_conversation_persistence(
    session_id: &str,
) -> (Vec<ChatMessage>, bool) {
    use crate::infrastructure::{get_workspace_path, PathManager};
    use crate::service::conversation::ConversationPersistenceManager;

    let Some(wp) = get_workspace_path() else {
        return (vec![], false);
    };
    let Ok(pm) = PathManager::new() else {
        return (vec![], false);
    };
    let pm = std::sync::Arc::new(pm);
    let Ok(conv_mgr) = ConversationPersistenceManager::new(pm, wp).await else {
        return (vec![], false);
    };
    let Ok(turns) = conv_mgr.load_session_turns(session_id).await else {
        return (vec![], false);
    };
    (turns_to_chat_messages(&turns), false)
}

fn strip_user_input_tags(content: &str) -> String {
    let s = content.trim();
    if s.starts_with("<user_query>") {
        if let Some(end) = s.find("</user_query>") {
            let inner = s["<user_query>".len()..end].trim();
            return inner.to_string();
        }
    }
    if let Some(pos) = s.find("<system_reminder>") {
        return s[..pos].trim().to_string();
    }
    s.to_string()
}

fn resolve_agent_type(mobile_type: Option<&str>) -> &'static str {
    match mobile_type {
        Some("code") | Some("agentic") | Some("Agentic") => "agentic",
        Some("cowork") | Some("Cowork") => "Cowork",
        Some("plan") | Some("Plan") => "Plan",
        Some("debug") | Some("Debug") => "debug",
        _ => "agentic",
    }
}

fn build_message_with_remote_images(content: &str, images: &[ImageAttachment]) -> String {
    use crate::agentic::tools::image_context::{
        format_image_context_reference, store_image_context, ImageContextData,
    };

    if images.is_empty() {
        return content.to_string();
    }

    let context_section = images
        .iter()
        .map(|img| {
            let mime_type = img
                .data_url
                .split_once(',')
                .and_then(|(header, _)| {
                    header
                        .strip_prefix("data:")
                        .and_then(|rest| rest.split(';').next())
                })
                .unwrap_or("image/png")
                .to_string();

            let image_context = ImageContextData {
                id: format!("remote_img_{}", uuid::Uuid::new_v4()),
                image_path: None,
                data_url: Some(img.data_url.clone()),
                mime_type,
                image_name: img.name.clone(),
                file_size: 0,
                width: None,
                height: None,
                source: "remote".to_string(),
            };

            store_image_context(image_context.clone());
            format_image_context_reference(&image_context)
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("{context_section}\n\n{content}")
}

// ── RemoteSessionStateTracker ──────────────────────────────────────

/// Mutable state snapshot updated by the event subscriber.
#[derive(Debug)]
struct TrackerState {
    session_state: String,
    title: String,
    turn_id: Option<String>,
    turn_status: String,
    accumulated_text: String,
    accumulated_thinking: String,
    active_tools: Vec<RemoteToolStatus>,
    round_index: usize,
    /// Ordered items preserving the interleaved arrival order for real-time display.
    active_items: Vec<ChatMessageItem>,
}

/// Lightweight event broadcast by the tracker for real-time consumers (e.g. bots).
#[derive(Debug, Clone)]
pub enum TrackerEvent {
    TextChunk(String),
    ThinkingChunk(String),
    ThinkingEnd,
    ToolStarted {
        tool_id: String,
        tool_name: String,
        params: Option<serde_json::Value>,
    },
    TurnCompleted,
    TurnFailed(String),
    TurnCancelled,
}

/// Tracks the real-time state of a session for polling by the mobile client.
/// Subscribes to `AgenticEvent` and updates an in-memory snapshot.
/// Also broadcasts lightweight `TrackerEvent`s for real-time consumers.
pub struct RemoteSessionStateTracker {
    target_session_id: String,
    version: AtomicU64,
    state: RwLock<TrackerState>,
    event_tx: tokio::sync::broadcast::Sender<TrackerEvent>,
}

impl RemoteSessionStateTracker {
    pub fn new(session_id: String) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(256);
        Self {
            target_session_id: session_id,
            version: AtomicU64::new(0),
            state: RwLock::new(TrackerState {
                session_state: "idle".to_string(),
                title: String::new(),
                turn_id: None,
                turn_status: String::new(),
                accumulated_text: String::new(),
                accumulated_thinking: String::new(),
                active_tools: Vec::new(),
                round_index: 0,
                active_items: Vec::new(),
            }),
            event_tx,
        }
    }

    /// Subscribe to real-time tracker events (for bot streaming).
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<TrackerEvent> {
        self.event_tx.subscribe()
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }

    fn bump_version(&self) {
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot_active_turn(&self) -> Option<ActiveTurnSnapshot> {
        let s = self.state.read().unwrap();
        s.turn_id.as_ref().map(|tid| ActiveTurnSnapshot {
            turn_id: tid.clone(),
            status: s.turn_status.clone(),
            text: s.accumulated_text.clone(),
            thinking: s.accumulated_thinking.clone(),
            tools: s.active_tools.clone(),
            round_index: s.round_index,
            items: if s.active_items.is_empty() { None } else { Some(s.active_items.clone()) },
        })
    }

    pub fn session_state(&self) -> String {
        self.state.read().unwrap().session_state.clone()
    }

    pub fn title(&self) -> String {
        self.state.read().unwrap().title.clone()
    }

    fn upsert_active_tool(
        state: &mut TrackerState,
        tool_id: &str,
        tool_name: &str,
        status: &str,
        input_preview: Option<String>,
        tool_input: Option<serde_json::Value>,
    ) {
        let resolved_id = if tool_id.is_empty() {
            format!("{}-{}", tool_name, state.active_tools.len())
        } else {
            tool_id.to_string()
        };
        let allow_name_fallback = tool_id.is_empty() && !tool_name.is_empty();

        if let Some(tool) = state
            .active_tools
            .iter_mut()
            .rev()
            .find(|t| t.id == resolved_id || (allow_name_fallback && t.name == tool_name))
        {
            tool.status = status.to_string();
            if input_preview.is_some() {
                tool.input_preview = input_preview.clone();
            }
            if tool_input.is_some() {
                tool.tool_input = tool_input.clone();
            }
        } else {
            let tool_status = RemoteToolStatus {
                id: resolved_id.clone(),
                name: tool_name.to_string(),
                status: status.to_string(),
                duration_ms: None,
                start_ms: Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                ),
                input_preview,
                tool_input,
            };
            state.active_tools.push(tool_status.clone());
            state.active_items.push(ChatMessageItem {
                item_type: "tool".to_string(),
                content: None,
                tool: Some(tool_status),
            });
            return;
        }

        if let Some(item) = state.active_items.iter_mut().rev().find(|i| {
            i.item_type == "tool"
                && i.tool
                    .as_ref()
                    .map_or(false, |t| {
                        t.id == resolved_id || (allow_name_fallback && t.name == tool_name)
                    })
        }) {
            if let Some(tool) = item.tool.as_mut() {
                tool.status = status.to_string();
                if input_preview.is_some() {
                    tool.input_preview = input_preview;
                }
                if tool_input.is_some() {
                    tool.tool_input = tool_input;
                }
            }
        }
    }

    fn handle_event(&self, event: &crate::agentic::events::AgenticEvent) {
        use bitfun_events::AgenticEvent as AE;

        let is_direct = event.session_id() == Some(self.target_session_id.as_str());
        let is_subagent = if !is_direct {
            match event {
                AE::TextChunk { subagent_parent_info, .. }
                | AE::ThinkingChunk { subagent_parent_info, .. }
                | AE::ToolEvent { subagent_parent_info, .. } => subagent_parent_info
                    .as_ref()
                    .map_or(false, |p| p.session_id == self.target_session_id),
                _ => false,
            }
        } else {
            false
        };

        if !is_direct && !is_subagent {
            return;
        }

        match event {
            AE::TextChunk { text, .. } => {
                let mut s = self.state.write().unwrap();
                s.accumulated_text.push_str(text);
                if let Some(last) = s.active_items.last_mut() {
                    if last.item_type == "text" {
                        let c = last.content.get_or_insert_with(String::new);
                        c.push_str(text);
                    } else {
                        s.active_items.push(ChatMessageItem {
                            item_type: "text".to_string(),
                            content: Some(text.clone()),
                            tool: None,
                        });
                    }
                } else {
                    s.active_items.push(ChatMessageItem {
                        item_type: "text".to_string(),
                        content: Some(text.clone()),
                        tool: None,
                    });
                }
                drop(s);
                self.bump_version();
                let _ = self.event_tx.send(TrackerEvent::TextChunk(text.clone()));
            }
            AE::ThinkingChunk { content, .. } => {
                let clean = content
                    .replace("<thinking_end>", "")
                    .replace("</thinking>", "")
                    .replace("<thinking>", "");
                let mut s = self.state.write().unwrap();
                s.accumulated_thinking.push_str(&clean);
                if let Some(last) = s.active_items.last_mut() {
                    if last.item_type == "thinking" {
                        let c = last.content.get_or_insert_with(String::new);
                        c.push_str(&clean);
                    } else {
                        s.active_items.push(ChatMessageItem {
                            item_type: "thinking".to_string(),
                            content: Some(clean),
                            tool: None,
                        });
                    }
                } else {
                    s.active_items.push(ChatMessageItem {
                        item_type: "thinking".to_string(),
                        content: Some(clean),
                        tool: None,
                    });
                }
                drop(s);
                self.bump_version();
                if content == "<thinking_end>" {
                    let _ = self.event_tx.send(TrackerEvent::ThinkingEnd);
                } else {
                    let _ = self.event_tx.send(TrackerEvent::ThinkingChunk(content.clone()));
                }
            }
            AE::ToolEvent { tool_event, .. } => {
                if let Ok(val) = serde_json::to_value(tool_event) {
                    let event_type = val
                        .get("event_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let tool_id = val
                        .get("tool_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tool_name = val
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let mut s = self.state.write().unwrap();
                    let allow_name_fallback = tool_id.is_empty() && !tool_name.is_empty();
                    match event_type {
                        "EarlyDetected" => {
                            Self::upsert_active_tool(
                                &mut s,
                                &tool_id,
                                &tool_name,
                                "preparing",
                                None,
                                None,
                            );
                        }
                        "ConfirmationNeeded" => {
                            let params = val.get("params").cloned();
                            let input_preview = params.as_ref().map(|v| {
                                let text = if v.is_string() {
                                    v.as_str().unwrap_or_default().to_string()
                                } else {
                                    serde_json::to_string(v).unwrap_or_default()
                                };
                                text.chars().take(160).collect()
                            });
                            Self::upsert_active_tool(
                                &mut s,
                                &tool_id,
                                &tool_name,
                                "pending_confirmation",
                                input_preview,
                                params,
                            );
                        }
                        "Started" => {
                            let params = val.get("params").cloned();
                            let input_preview = params.as_ref().map(|v| {
                                let text = if v.is_string() {
                                    v.as_str().unwrap_or_default().to_string()
                                } else {
                                    serde_json::to_string(v).unwrap_or_default()
                                };
                                text.chars().take(160).collect()
                            });
                            let tool_input = if tool_name == "AskUserQuestion" {
                                params.clone()
                            } else {
                                None
                            };
                            Self::upsert_active_tool(
                                &mut s,
                                &tool_id,
                                &tool_name,
                                "running",
                                input_preview,
                                tool_input,
                            );
                            let _ = self.event_tx.send(TrackerEvent::ToolStarted {
                                tool_id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                                params,
                            });
                        }
                        "Confirmed" => {
                            Self::upsert_active_tool(
                                &mut s,
                                &tool_id,
                                &tool_name,
                                "confirmed",
                                None,
                                None,
                            );
                        }
                        "Rejected" => {
                            Self::upsert_active_tool(
                                &mut s,
                                &tool_id,
                                &tool_name,
                                "rejected",
                                None,
                                None,
                            );
                        }
                        "Completed" | "Succeeded" => {
                            let duration = val
                                .get("duration_ms")
                                .and_then(|v| v.as_u64());
                            if let Some(t) = s.active_tools.iter_mut().rev().find(|t| {
                                (t.id == tool_id
                                    || (allow_name_fallback && t.name == tool_name))
                                    && t.status == "running"
                            }) {
                                t.status = "completed".to_string();
                                t.duration_ms = duration;
                            }
                            if let Some(item) = s.active_items.iter_mut().rev().find(|i| {
                                i.item_type == "tool"
                                    && i.tool.as_ref().map_or(false, |t| {
                                        (t.id == tool_id
                                            || (allow_name_fallback && t.name == tool_name))
                                            && t.status == "running"
                                    })
                            }) {
                                if let Some(t) = item.tool.as_mut() {
                                    t.status = "completed".to_string();
                                    t.duration_ms = duration;
                                }
                            }
                        }
                        "Failed" => {
                            if let Some(t) = s.active_tools.iter_mut().rev().find(|t| {
                                (t.id == tool_id
                                    || (allow_name_fallback && t.name == tool_name))
                                    && t.status == "running"
                            }) {
                                t.status = "failed".to_string();
                            }
                            if let Some(item) = s.active_items.iter_mut().rev().find(|i| {
                                i.item_type == "tool"
                                    && i.tool.as_ref().map_or(false, |t| {
                                        (t.id == tool_id
                                            || (allow_name_fallback && t.name == tool_name))
                                            && t.status == "running"
                                    })
                            }) {
                                if let Some(t) = item.tool.as_mut() {
                                    t.status = "failed".to_string();
                                }
                            }
                        }
                        "Cancelled" => {
                            if let Some(t) = s.active_tools.iter_mut().rev().find(|t| {
                                (t.id == tool_id
                                    || (allow_name_fallback && t.name == tool_name))
                                    && matches!(
                                        t.status.as_str(),
                                        "running" | "pending_confirmation" | "confirmed"
                                    )
                            }) {
                                t.status = "cancelled".to_string();
                            }
                            if let Some(item) = s.active_items.iter_mut().rev().find(|i| {
                                i.item_type == "tool"
                                    && i.tool.as_ref().map_or(false, |t| {
                                        (t.id == tool_id
                                            || (allow_name_fallback && t.name == tool_name))
                                            && matches!(
                                                t.status.as_str(),
                                                "running" | "pending_confirmation" | "confirmed"
                                            )
                                    })
                            }) {
                                if let Some(t) = item.tool.as_mut() {
                                    t.status = "cancelled".to_string();
                                }
                            }
                        }
                        _ => {}
                    }
                    drop(s);
                    self.bump_version();
                }
            }
            AE::DialogTurnStarted { turn_id, .. } if is_direct => {
                let mut s = self.state.write().unwrap();
                s.turn_id = Some(turn_id.clone());
                s.turn_status = "active".to_string();
                s.accumulated_text.clear();
                s.accumulated_thinking.clear();
                s.active_tools.clear();
                s.active_items.clear();
                s.round_index = 0;
                s.session_state = "running".to_string();
                drop(s);
                self.bump_version();
            }
            AE::DialogTurnCompleted { .. } if is_direct => {
                let mut s = self.state.write().unwrap();
                s.turn_status = "completed".to_string();
                s.turn_id = None;
                s.accumulated_text.clear();
                s.accumulated_thinking.clear();
                s.active_tools.clear();
                s.active_items.clear();
                s.session_state = "idle".to_string();
                drop(s);
                self.bump_version();
                let _ = self.event_tx.send(TrackerEvent::TurnCompleted);
            }
            AE::DialogTurnFailed { error, .. } if is_direct => {
                let mut s = self.state.write().unwrap();
                s.turn_status = "failed".to_string();
                s.turn_id = None;
                s.session_state = "idle".to_string();
                drop(s);
                self.bump_version();
                let _ = self.event_tx.send(TrackerEvent::TurnFailed(error.clone()));
            }
            AE::DialogTurnCancelled { .. } if is_direct => {
                let mut s = self.state.write().unwrap();
                s.turn_status = "cancelled".to_string();
                s.turn_id = None;
                s.session_state = "idle".to_string();
                drop(s);
                self.bump_version();
                let _ = self.event_tx.send(TrackerEvent::TurnCancelled);
            }
            AE::ModelRoundStarted { round_index, .. } if is_direct => {
                let mut s = self.state.write().unwrap();
                s.round_index = *round_index;
                drop(s);
                self.bump_version();
            }
            AE::SessionStateChanged { new_state, .. } if is_direct => {
                let mut s = self.state.write().unwrap();
                s.session_state = new_state.clone();
                drop(s);
                self.bump_version();
            }
            AE::SessionTitleGenerated { title, .. } if is_direct => {
                let mut s = self.state.write().unwrap();
                s.title = title.clone();
                drop(s);
                self.bump_version();
            }
            _ => {}
        }
    }
}

#[async_trait::async_trait]
impl crate::agentic::events::EventSubscriber for Arc<RemoteSessionStateTracker> {
    async fn on_event(
        &self,
        event: &crate::agentic::events::AgenticEvent,
    ) -> crate::util::errors::BitFunResult<()> {
        self.handle_event(event);
        Ok(())
    }
}

// ── RemoteExecutionDispatcher (global singleton) ────────────────────

/// Shared dispatch layer that owns the session state trackers.
/// Both `RemoteServer` (mobile relay) and the bot use this to
/// dispatch commands through the same path.
pub struct RemoteExecutionDispatcher {
    state_trackers: Arc<DashMap<String, Arc<RemoteSessionStateTracker>>>,
}

static GLOBAL_DISPATCHER: OnceLock<Arc<RemoteExecutionDispatcher>> = OnceLock::new();

pub fn get_or_init_global_dispatcher() -> Arc<RemoteExecutionDispatcher> {
    GLOBAL_DISPATCHER
        .get_or_init(|| {
            Arc::new(RemoteExecutionDispatcher {
                state_trackers: Arc::new(DashMap::new()),
            })
        })
        .clone()
}

pub fn get_global_dispatcher() -> Option<Arc<RemoteExecutionDispatcher>> {
    GLOBAL_DISPATCHER.get().cloned()
}

impl RemoteExecutionDispatcher {
    /// Ensure a state tracker exists for the given session and return it.
    pub fn ensure_tracker(&self, session_id: &str) -> Arc<RemoteSessionStateTracker> {
        if let Some(tracker) = self.state_trackers.get(session_id) {
            return tracker.clone();
        }

        let tracker = Arc::new(RemoteSessionStateTracker::new(session_id.to_string()));
        self.state_trackers
            .insert(session_id.to_string(), tracker.clone());

        if let Some(coordinator) = crate::agentic::coordination::get_global_coordinator() {
            let sub_id = format!("remote_tracker_{}", session_id);
            coordinator.subscribe_internal(sub_id, tracker.clone());
            info!("Registered state tracker for session {session_id}");
        }

        tracker
    }

    pub fn get_tracker(&self, session_id: &str) -> Option<Arc<RemoteSessionStateTracker>> {
        self.state_trackers.get(session_id).map(|t| t.clone())
    }

    pub fn remove_tracker(&self, session_id: &str) {
        if let Some((_, _)) = self.state_trackers.remove(session_id) {
            if let Some(coordinator) = crate::agentic::coordination::get_global_coordinator() {
                let sub_id = format!("remote_tracker_{}", session_id);
                coordinator.unsubscribe_internal(&sub_id);
            }
        }
    }

    /// Dispatch a SendMessage command: ensure tracker, restore session, start dialog turn.
    /// Returns `(session_id, turn_id)` on success.
    /// If `turn_id` is `None`, one is auto-generated.
    pub async fn send_message(
        &self,
        session_id: &str,
        content: String,
        agent_type: Option<&str>,
        images: Option<&Vec<ImageAttachment>>,
        trigger_source: crate::agentic::coordination::DialogTriggerSource,
        turn_id: Option<String>,
    ) -> std::result::Result<(String, String), String> {
        use crate::agentic::coordination::get_global_coordinator;

        let coordinator = get_global_coordinator()
            .ok_or_else(|| "Desktop session system not ready".to_string())?;

        self.ensure_tracker(session_id);

        let session_mgr = coordinator.get_session_manager();
        let _ = match session_mgr.get_session(session_id) {
            Some(session) => Some(session),
            None => coordinator.restore_session(session_id).await.ok(),
        };

        let resolved_agent_type = agent_type
            .map(|t| resolve_agent_type(Some(t)).to_string())
            .unwrap_or_else(|| "agentic".to_string());

        let full_content = images
            .map(|imgs| build_message_with_remote_images(&content, imgs))
            .unwrap_or_else(|| content.clone());

        let turn_id =
            turn_id.unwrap_or_else(|| format!("turn_{}", chrono::Utc::now().timestamp_millis()));
        coordinator
            .start_dialog_turn(
                session_id.to_string(),
                full_content,
                Some(turn_id.clone()),
                resolved_agent_type,
                trigger_source,
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok((session_id.to_string(), turn_id))
    }

    /// Cancel a running dialog turn.
    pub async fn cancel_task(
        &self,
        session_id: &str,
        requested_turn_id: Option<&str>,
    ) -> std::result::Result<(), String> {
        use crate::agentic::coordination::get_global_coordinator;

        let coordinator = get_global_coordinator()
            .ok_or_else(|| "Desktop session system not ready".to_string())?;

        let session_mgr = coordinator.get_session_manager();
        let session = match session_mgr.get_session(session_id) {
            Some(s) => s,
            None => coordinator
                .restore_session(session_id)
                .await
                .map_err(|e| format!("Session not found: {e}"))?,
        };

        let running_turn_id = match &session.state {
            crate::agentic::core::SessionState::Processing {
                current_turn_id, ..
            } => Some(current_turn_id.clone()),
            _ => None,
        };

        match (running_turn_id, requested_turn_id) {
            (Some(current_turn_id), Some(req_id)) if req_id != current_turn_id => {
                Err("This task is no longer running.".to_string())
            }
            (Some(current_turn_id), _) => coordinator
                .cancel_dialog_turn(session_id, &current_turn_id)
                .await
                .map_err(|e| e.to_string()),
            (None, Some(_)) => Err("This task is already finished.".to_string()),
            (None, None) => Err(format!(
                "No running task to cancel for session: {}",
                session_id
            )),
        }
    }
}

// ── RemoteServer ───────────────────────────────────────────────────

/// Bridges remote commands to local session operations.
/// Delegates execution and tracker management to the global `RemoteExecutionDispatcher`.
pub struct RemoteServer {
    shared_secret: [u8; 32],
}

impl RemoteServer {
    pub fn new(shared_secret: [u8; 32]) -> Self {
        get_or_init_global_dispatcher();
        Self { shared_secret }
    }

    pub fn shared_secret(&self) -> &[u8; 32] {
        &self.shared_secret
    }

    pub fn decrypt_command(
        &self,
        encrypted_data: &str,
        nonce: &str,
    ) -> Result<(RemoteCommand, Option<String>)> {
        let json = encryption::decrypt_from_base64(&self.shared_secret, encrypted_data, nonce)?;
        let value: Value = serde_json::from_str(&json).map_err(|e| anyhow!("parse json: {e}"))?;
        let request_id = value
            .get("_request_id")
            .and_then(|v| v.as_str())
            .map(String::from);
        let cmd: RemoteCommand =
            serde_json::from_value(value).map_err(|e| anyhow!("parse command: {e}"))?;
        Ok((cmd, request_id))
    }

    pub fn encrypt_response(
        &self,
        response: &RemoteResponse,
        request_id: Option<&str>,
    ) -> Result<EncryptedPayload> {
        let mut value =
            serde_json::to_value(response).map_err(|e| anyhow!("serialize response: {e}"))?;
        if let (Some(id), Some(obj)) = (request_id, value.as_object_mut()) {
            obj.insert("_request_id".to_string(), Value::String(id.to_string()));
        }
        let json = serde_json::to_string(&value).map_err(|e| anyhow!("to_string: {e}"))?;
        encryption::encrypt_to_base64(&self.shared_secret, &json)
    }

    pub async fn dispatch(&self, cmd: &RemoteCommand) -> RemoteResponse {
        match cmd {
            RemoteCommand::Ping => RemoteResponse::Pong,

            RemoteCommand::GetWorkspaceInfo
            | RemoteCommand::ListRecentWorkspaces
            | RemoteCommand::SetWorkspace { .. } => self.handle_workspace_command(cmd).await,

            RemoteCommand::ListSessions { .. }
            | RemoteCommand::CreateSession { .. }
            | RemoteCommand::GetSessionMessages { .. }
            | RemoteCommand::DeleteSession { .. } => self.handle_session_command(cmd).await,

            RemoteCommand::SendMessage { .. }
            | RemoteCommand::CancelTask { .. }
            | RemoteCommand::ConfirmTool { .. }
            | RemoteCommand::RejectTool { .. }
            | RemoteCommand::CancelTool { .. }
            | RemoteCommand::AnswerQuestion { .. } => {
                self.handle_execution_command(cmd).await
            }

            RemoteCommand::PollSession { .. } => self.handle_poll_command(cmd).await,
        }
    }

    fn ensure_tracker(&self, session_id: &str) -> Arc<RemoteSessionStateTracker> {
        get_or_init_global_dispatcher().ensure_tracker(session_id)
    }

    pub async fn generate_initial_sync(&self) -> RemoteResponse {
        use crate::infrastructure::{get_workspace_path, PathManager};
        use crate::service::conversation::ConversationPersistenceManager;

        let ws_path = get_workspace_path();
        let (has_workspace, path_str, project_name, git_branch) = if let Some(ref p) = ws_path {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string());
            let branch = git2::Repository::open(p).ok().and_then(|repo| {
                repo.head()
                    .ok()
                    .and_then(|h| h.shorthand().map(String::from))
            });
            (true, Some(p.to_string_lossy().to_string()), name, branch)
        } else {
            (false, None, None, None)
        };

        let (sessions, has_more) = if let Some(ref wp) = ws_path {
            let ws_str = wp.to_string_lossy().to_string();
            let ws_name = wp.file_name().map(|n| n.to_string_lossy().to_string());
            if let Ok(pm) = PathManager::new() {
                let pm = std::sync::Arc::new(pm);
                if let Ok(conv_mgr) = ConversationPersistenceManager::new(pm, wp.clone()).await {
                    if let Ok(all_meta) = conv_mgr.get_session_list().await {
                        let total = all_meta.len();
                        let page_size = 100usize;
                        let has_more = total > page_size;
                        let sessions: Vec<SessionInfo> = all_meta
                            .into_iter()
                            .take(page_size)
                            .map(|s| SessionInfo {
                                session_id: s.session_id,
                                name: s.session_name,
                                agent_type: s.agent_type,
                                created_at: (s.created_at / 1000).to_string(),
                                updated_at: (s.last_active_at / 1000).to_string(),
                                message_count: s.turn_count,
                                workspace_path: Some(ws_str.clone()),
                                workspace_name: ws_name.clone(),
                            })
                            .collect();
                        (sessions, has_more)
                    } else {
                        (vec![], false)
                    }
                } else {
                    (vec![], false)
                }
            } else {
                (vec![], false)
            }
        } else {
            (vec![], false)
        };

        RemoteResponse::InitialSync {
            has_workspace,
            path: path_str,
            project_name,
            git_branch,
            sessions,
            has_more_sessions: has_more,
        }
    }

    // ── Poll command handler ────────────────────────────────────────

    async fn handle_poll_command(&self, cmd: &RemoteCommand) -> RemoteResponse {
        let RemoteCommand::PollSession {
            session_id,
            since_version,
            known_msg_count,
        } = cmd
        else {
            return RemoteResponse::Error {
                message: "expected poll_session".into(),
            };
        };

        let tracker = self.ensure_tracker(session_id);
        let current_version = tracker.version();

        if *since_version == current_version && *since_version > 0 {
            return RemoteResponse::SessionPoll {
                version: current_version,
                changed: false,
                session_state: None,
                title: None,
                new_messages: None,
                total_msg_count: None,
                active_turn: None,
            };
        }

        let (all_chat_msgs, _) =
            load_chat_messages_from_conversation_persistence(session_id).await;
        let total_msg_count = all_chat_msgs.len();
        let skip = *known_msg_count;
        let new_messages: Vec<ChatMessage> =
            all_chat_msgs.into_iter().skip(skip).collect();

        let active_turn = tracker.snapshot_active_turn();
        let sess_state = tracker.session_state();
        let title = tracker.title();

        let active_turn_ask_tool_ids = active_turn
            .as_ref()
            .map(|turn| {
                turn.tools
                    .iter()
                    .filter(|tool| tool.name == "AskUserQuestion")
                    .map(|tool| tool.id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let new_message_ask_tool_ids = new_messages
            .iter()
            .flat_map(|message| message.items.iter().flatten())
            .filter_map(|item| item.tool.as_ref())
            .filter(|tool| tool.name == "AskUserQuestion")
            .map(|tool| tool.id.clone())
            .collect::<Vec<_>>();

        RemoteResponse::SessionPoll {
            version: current_version,
            changed: true,
            session_state: Some(sess_state),
            title: if title.is_empty() { None } else { Some(title) },
            new_messages: Some(new_messages),
            total_msg_count: Some(total_msg_count),
            active_turn,
        }
    }

    // ── Workspace commands ──────────────────────────────────────────

    async fn handle_workspace_command(&self, cmd: &RemoteCommand) -> RemoteResponse {
        use crate::infrastructure::get_workspace_path;
        use crate::service::workspace::get_global_workspace_service;

        match cmd {
            RemoteCommand::GetWorkspaceInfo => {
                let ws_path = get_workspace_path();
                let (project_name, git_branch) = if let Some(ref p) = ws_path {
                    let name = p.file_name().map(|n| n.to_string_lossy().to_string());
                    let branch = git2::Repository::open(p)
                        .ok()
                        .and_then(|repo| {
                            repo.head()
                                .ok()
                                .and_then(|h| h.shorthand().map(String::from))
                        });
                    (name, branch)
                } else {
                    (None, None)
                };
                RemoteResponse::WorkspaceInfo {
                    has_workspace: ws_path.is_some(),
                    path: ws_path.map(|p| p.to_string_lossy().to_string()),
                    project_name,
                    git_branch,
                }
            }
            RemoteCommand::ListRecentWorkspaces => {
                let ws_service = match get_global_workspace_service() {
                    Some(s) => s,
                    None => {
                        return RemoteResponse::RecentWorkspaces { workspaces: vec![] };
                    }
                };
                let recent = ws_service.get_recent_workspaces().await;
                let entries = recent
                    .into_iter()
                    .map(|w| RecentWorkspaceEntry {
                        path: w.root_path.to_string_lossy().to_string(),
                        name: w.name.clone(),
                        last_opened: w.last_accessed.to_rfc3339(),
                    })
                    .collect();
                RemoteResponse::RecentWorkspaces {
                    workspaces: entries,
                }
            }
            RemoteCommand::SetWorkspace { path } => {
                let ws_service = match get_global_workspace_service() {
                    Some(s) => s,
                    None => {
                        return RemoteResponse::WorkspaceUpdated {
                            success: false,
                            path: None,
                            project_name: None,
                            error: Some("Workspace service not available".into()),
                        };
                    }
                };
                let path_buf = std::path::PathBuf::from(path);
                match ws_service.open_workspace(path_buf).await {
                    Ok(info) => {
                        if let Err(e) =
                            crate::service::snapshot::initialize_global_snapshot_manager(
                                info.root_path.clone(),
                                None,
                            )
                            .await
                        {
                            error!(
                                "Failed to initialize snapshot after remote workspace set: {e}"
                            );
                        }
                        RemoteResponse::WorkspaceUpdated {
                            success: true,
                            path: Some(info.root_path.to_string_lossy().to_string()),
                            project_name: Some(info.name.clone()),
                            error: None,
                        }
                    }
                    Err(e) => RemoteResponse::WorkspaceUpdated {
                        success: false,
                        path: None,
                        project_name: None,
                        error: Some(e.to_string()),
                    },
                }
            }
            _ => RemoteResponse::Error {
                message: "Unknown workspace command".into(),
            },
        }
    }

    // ── Session commands ────────────────────────────────────────────

    async fn handle_session_command(&self, cmd: &RemoteCommand) -> RemoteResponse {
        use crate::agentic::{coordination::get_global_coordinator, core::SessionConfig};

        let coordinator = match get_global_coordinator() {
            Some(c) => c,
            None => {
                return RemoteResponse::Error {
                    message: "Desktop session system not ready".into(),
                };
            }
        };

        match cmd {
            RemoteCommand::ListSessions {
                workspace_path,
                limit,
                offset,
            } => {
                use crate::infrastructure::{get_workspace_path, PathManager};
                use crate::service::conversation::ConversationPersistenceManager;

                let page_size = limit.unwrap_or(30).min(100);
                let page_offset = offset.unwrap_or(0);

                let effective_ws: Option<std::path::PathBuf> = workspace_path
                    .as_deref()
                    .map(std::path::PathBuf::from)
                    .or_else(|| get_workspace_path());

                if let Some(ref wp) = effective_ws {
                    let ws_str = wp.to_string_lossy().to_string();
                    let workspace_name =
                        wp.file_name().map(|n| n.to_string_lossy().to_string());

                    if let Ok(pm) = PathManager::new() {
                        let pm = std::sync::Arc::new(pm);
                        match ConversationPersistenceManager::new(pm, wp.clone()).await {
                            Ok(conv_mgr) => {
                                match conv_mgr.get_session_list().await {
                                    Ok(all_meta) => {
                                        let total = all_meta.len();
                                        let has_more = page_offset + page_size < total;
                                        let sessions: Vec<SessionInfo> = all_meta
                                            .into_iter()
                                            .skip(page_offset)
                                            .take(page_size)
                                            .map(|s| {
                                                let created =
                                                    (s.created_at / 1000).to_string();
                                                let updated =
                                                    (s.last_active_at / 1000).to_string();
                                                SessionInfo {
                                                    session_id: s.session_id,
                                                    name: s.session_name,
                                                    agent_type: s.agent_type,
                                                    created_at: created,
                                                    updated_at: updated,
                                                    message_count: s.turn_count,
                                                    workspace_path: Some(ws_str.clone()),
                                                    workspace_name: workspace_name.clone(),
                                                }
                                            })
                                            .collect();
                                        return RemoteResponse::SessionList {
                                            sessions,
                                            has_more,
                                        };
                                    }
                                    Err(e) => {
                                        debug!("Session list read failed for {ws_str}: {e}")
                                    }
                                }
                            }
                            Err(e) => {
                                debug!(
                                    "ConversationPersistenceManager init failed for {ws_str}: {e}"
                                )
                            }
                        }
                    }
                }

                match coordinator.list_sessions().await {
                    Ok(summaries) => {
                        let total = summaries.len();
                        let has_more = page_offset + page_size < total;
                        let sessions = summaries
                            .into_iter()
                            .skip(page_offset)
                            .take(page_size)
                            .map(|s| {
                                let created = s
                                    .created_at
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs()
                                    .to_string();
                                let updated = s
                                    .last_activity_at
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs()
                                    .to_string();
                                SessionInfo {
                                    session_id: s.session_id,
                                    name: s.session_name,
                                    agent_type: s.agent_type,
                                    created_at: created,
                                    updated_at: updated,
                                    message_count: s.turn_count,
                                    workspace_path: None,
                                    workspace_name: None,
                                }
                            })
                            .collect();
                        RemoteResponse::SessionList { sessions, has_more }
                    }
                    Err(e) => RemoteResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            RemoteCommand::CreateSession {
                agent_type,
                session_name: custom_name,
                workspace_path: requested_ws_path,
            } => {
                use crate::infrastructure::get_workspace_path;

                let agent = resolve_agent_type(agent_type.as_deref());
                let session_name = custom_name
                    .as_deref()
                    .filter(|n| !n.is_empty())
                    .unwrap_or(match agent {
                        "Cowork" => "Remote Cowork Session",
                        _ => "Remote Code Session",
                    });
                let binding_ws_path: Option<std::path::PathBuf> = requested_ws_path
                    .as_deref()
                    .map(std::path::PathBuf::from)
                    .or_else(|| get_workspace_path());
                let binding_ws_str =
                    binding_ws_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string());

                debug!(
                    "Remote CreateSession: requested_ws={:?}, binding_ws={:?}",
                    requested_ws_path, binding_ws_str
                );
                match coordinator
                    .create_session_with_workspace(
                        None,
                        session_name.to_string(),
                        agent.to_string(),
                        SessionConfig::default(),
                        binding_ws_str.clone(),
                    )
                    .await
                {
                    Ok(session) => {
                        let session_id = session.session_id.clone();
                        RemoteResponse::SessionCreated { session_id }
                    }
                    Err(e) => RemoteResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            RemoteCommand::GetSessionMessages {
                session_id,
                limit: _,
                before_message_id: _,
            } => {
                let (chat_msgs, has_more) =
                    load_chat_messages_from_conversation_persistence(session_id).await;
                RemoteResponse::Messages {
                    session_id: session_id.clone(),
                    messages: chat_msgs,
                    has_more,
                }
            }
            RemoteCommand::DeleteSession { session_id } => {
                get_or_init_global_dispatcher().remove_tracker(session_id);
                match coordinator.delete_session(session_id).await {
                    Ok(_) => RemoteResponse::SessionDeleted {
                        session_id: session_id.clone(),
                    },
                    Err(e) => RemoteResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            _ => RemoteResponse::Error {
                message: "Unknown session command".into(),
            },
        }
    }

    // ── Execution commands ──────────────────────────────────────────

    async fn handle_execution_command(&self, cmd: &RemoteCommand) -> RemoteResponse {
        use crate::agentic::coordination::{get_global_coordinator, DialogTriggerSource};

        let dispatcher = get_or_init_global_dispatcher();

        match cmd {
            RemoteCommand::SendMessage {
                session_id,
                content,
                agent_type: requested_agent_type,
                images,
            } => {
                info!(
                    "Remote send_message: session={session_id}, agent_type={}, images={}",
                    requested_agent_type.as_deref().unwrap_or("agentic"),
                    images.as_ref().map_or(0, |v| v.len())
                );
                match dispatcher
                    .send_message(
                        session_id,
                        content.clone(),
                        requested_agent_type.as_deref(),
                        images.as_ref(),
                        DialogTriggerSource::RemoteRelay,
                        None,
                    )
                    .await
                {
                    Ok((sid, turn_id)) => RemoteResponse::MessageSent {
                        session_id: sid,
                        turn_id,
                    },
                    Err(e) => RemoteResponse::Error { message: e },
                }
            }
            RemoteCommand::CancelTask {
                session_id,
                turn_id,
            } => {
                match dispatcher
                    .cancel_task(session_id, turn_id.as_deref())
                    .await
                {
                    Ok(()) => RemoteResponse::TaskCancelled {
                        session_id: session_id.clone(),
                    },
                    Err(e) => RemoteResponse::Error { message: e },
                }
            }
            RemoteCommand::ConfirmTool {
                tool_id,
                updated_input,
            } => {
                let coordinator = match get_global_coordinator() {
                    Some(c) => c,
                    None => {
                        return RemoteResponse::Error {
                            message: "Desktop session system not ready".into(),
                        };
                    }
                };
                match coordinator.confirm_tool(tool_id, updated_input.clone()).await {
                    Ok(_) => RemoteResponse::InteractionAccepted {
                        action: "confirm_tool".to_string(),
                        target_id: tool_id.clone(),
                    },
                    Err(e) => RemoteResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            RemoteCommand::RejectTool { tool_id, reason } => {
                let coordinator = match get_global_coordinator() {
                    Some(c) => c,
                    None => {
                        return RemoteResponse::Error {
                            message: "Desktop session system not ready".into(),
                        };
                    }
                };
                let reject_reason = reason
                    .clone()
                    .unwrap_or_else(|| "User rejected".to_string());
                match coordinator.reject_tool(tool_id, reject_reason).await {
                    Ok(_) => RemoteResponse::InteractionAccepted {
                        action: "reject_tool".to_string(),
                        target_id: tool_id.clone(),
                    },
                    Err(e) => RemoteResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            RemoteCommand::CancelTool { tool_id, reason } => {
                let coordinator = match get_global_coordinator() {
                    Some(c) => c,
                    None => {
                        return RemoteResponse::Error {
                            message: "Desktop session system not ready".into(),
                        };
                    }
                };
                let cancel_reason = reason
                    .clone()
                    .unwrap_or_else(|| "User cancelled".to_string());
                match coordinator.cancel_tool(tool_id, cancel_reason).await {
                    Ok(_) => RemoteResponse::InteractionAccepted {
                        action: "cancel_tool".to_string(),
                        target_id: tool_id.clone(),
                    },
                    Err(e) => RemoteResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            RemoteCommand::AnswerQuestion { tool_id, answers } => {
                use crate::agentic::tools::user_input_manager::get_user_input_manager;
                let mgr = get_user_input_manager();
                match mgr.send_answer(tool_id, answers.clone()) {
                    Ok(()) => RemoteResponse::AnswerAccepted,
                    Err(e) => RemoteResponse::Error { message: e },
                }
            }
            _ => RemoteResponse::Error {
                message: "Unknown execution command".into(),
            },
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::remote_connect::encryption::KeyPair;

    #[test]
    fn test_command_round_trip() {
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();
        let shared = alice.derive_shared_secret(&bob.public_key_bytes());

        let bridge = RemoteServer::new(shared);

        let cmd_json = serde_json::json!({
            "cmd": "send_message",
            "session_id": "sess-123",
            "content": "Hello from mobile!",
            "_request_id": "req_abc"
        });
        let json = cmd_json.to_string();
        let (enc, nonce) = encryption::encrypt_to_base64(&shared, &json).unwrap();
        let (decoded, req_id) = bridge.decrypt_command(&enc, &nonce).unwrap();

        assert_eq!(req_id.as_deref(), Some("req_abc"));
        if let RemoteCommand::SendMessage {
            session_id,
            content,
            ..
        } = decoded
        {
            assert_eq!(session_id, "sess-123");
            assert_eq!(content, "Hello from mobile!");
        } else {
            panic!("unexpected command variant");
        }
    }

    #[test]
    fn test_response_with_request_id() {
        let alice = KeyPair::generate();
        let shared = alice.derive_shared_secret(&alice.public_key_bytes());
        let bridge = RemoteServer::new(shared);

        let resp = RemoteResponse::Pong;
        let (enc, nonce) = bridge.encrypt_response(&resp, Some("req_xyz")).unwrap();

        let json = encryption::decrypt_from_base64(&shared, &enc, &nonce).unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["resp"], "pong");
        assert_eq!(value["_request_id"], "req_xyz");
    }
}
