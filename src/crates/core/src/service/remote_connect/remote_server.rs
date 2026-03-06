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
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

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
    },
    DeleteSession {
        session_id: String,
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
            order_index: usize,
            item: ChatMessageItem,
        }
        let mut ordered: Vec<OrderedEntry> = Vec::new();
        let mut tools_flat = Vec::new();
        let mut thinking_parts = Vec::new();
        let mut text_parts = Vec::new();

        for round in &turn.model_rounds {
            for t in &round.text_items {
                if t.is_subagent_item.unwrap_or(false) {
                    continue;
                }
                if !t.content.is_empty() {
                    text_parts.push(t.content.clone());
                    ordered.push(OrderedEntry {
                        order_index: t.order_index.unwrap_or(usize::MAX),
                        item: ChatMessageItem {
                            item_type: "text".to_string(),
                            content: Some(t.content.clone()),
                            tool: None,
                        },
                    });
                }
            }
            for t in &round.thinking_items {
                if t.is_subagent_item.unwrap_or(false) {
                    continue;
                }
                if !t.content.is_empty() {
                    thinking_parts.push(t.content.clone());
                    ordered.push(OrderedEntry {
                        order_index: t.order_index.unwrap_or(usize::MAX),
                        item: ChatMessageItem {
                            item_type: "thinking".to_string(),
                            content: Some(t.content.clone()),
                            tool: None,
                        },
                    });
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
                    tool_input: None,
                };
                tools_flat.push(tool_status.clone());
                ordered.push(OrderedEntry {
                    order_index: t.order_index.unwrap_or(usize::MAX),
                    item: ChatMessageItem {
                        item_type: "tool".to_string(),
                        content: None,
                        tool: Some(tool_status),
                    },
                });
            }
        }

        ordered.sort_by_key(|e| e.order_index);
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

fn save_data_url_image(
    dir: &std::path::Path,
    name: &str,
    data_url: &str,
) -> Option<std::path::PathBuf> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    let (header, b64_data) = data_url.split_once(",")?;
    let ext = if header.contains("png") {
        "png"
    } else if header.contains("gif") {
        "gif"
    } else if header.contains("webp") {
        "webp"
    } else {
        "jpg"
    };

    let decoded = B64.decode(b64_data.trim()).ok()?;

    let stem = std::path::Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let ts = chrono::Utc::now().timestamp_millis();
    let filename = format!("{stem}_{ts}.{ext}");
    let path = dir.join(&filename);

    std::fs::write(&path, &decoded).ok()?;
    Some(path)
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

/// Tracks the real-time state of a session for polling by the mobile client.
/// Subscribes to `AgenticEvent` and updates an in-memory snapshot.
pub struct RemoteSessionStateTracker {
    target_session_id: String,
    version: AtomicU64,
    state: RwLock<TrackerState>,
}

impl RemoteSessionStateTracker {
    pub fn new(session_id: String) -> Self {
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
        }
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
                    match event_type {
                        "Started" => {
                            let input_preview = val
                                .get("input")
                                .and_then(|v| v.as_str())
                                .map(|s| s.chars().take(100).collect());
                            let tool_input = if tool_name == "AskUserQuestion" {
                                val.get("params").cloned()
                            } else {
                                None
                            };
                            let tool_count = s.active_tools.len();
                            let resolved_id = if tool_id.is_empty() {
                                format!("{}-{}", tool_name, tool_count)
                            } else {
                                tool_id
                            };
                            let tool_status = RemoteToolStatus {
                                id: resolved_id,
                                name: tool_name,
                                status: "running".to_string(),
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
                            s.active_items.push(ChatMessageItem {
                                item_type: "tool".to_string(),
                                content: None,
                                tool: Some(tool_status.clone()),
                            });
                            s.active_tools.push(tool_status);
                        }
                        "Completed" | "Succeeded" => {
                            let duration = val
                                .get("duration_ms")
                                .and_then(|v| v.as_u64());
                            if let Some(t) = s.active_tools.iter_mut().rev().find(|t| {
                                (t.id == tool_id || t.name == tool_name) && t.status == "running"
                            }) {
                                t.status = "completed".to_string();
                                t.duration_ms = duration;
                            }
                            if let Some(item) = s.active_items.iter_mut().rev().find(|i| {
                                i.item_type == "tool" && i.tool.as_ref().map_or(false, |t| (t.id == tool_id || t.name == tool_name) && t.status == "running")
                            }) {
                                if let Some(t) = item.tool.as_mut() {
                                    t.status = "completed".to_string();
                                    t.duration_ms = duration;
                                }
                            }
                        }
                        "Failed" => {
                            if let Some(t) = s.active_tools.iter_mut().rev().find(|t| {
                                (t.id == tool_id || t.name == tool_name) && t.status == "running"
                            }) {
                                t.status = "failed".to_string();
                            }
                            if let Some(item) = s.active_items.iter_mut().rev().find(|i| {
                                i.item_type == "tool" && i.tool.as_ref().map_or(false, |t| (t.id == tool_id || t.name == tool_name) && t.status == "running")
                            }) {
                                if let Some(t) = item.tool.as_mut() {
                                    t.status = "failed".to_string();
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
            }
            AE::DialogTurnFailed { .. } if is_direct => {
                let mut s = self.state.write().unwrap();
                s.turn_status = "failed".to_string();
                s.turn_id = None;
                s.session_state = "idle".to_string();
                drop(s);
                self.bump_version();
            }
            AE::DialogTurnCancelled { .. } if is_direct => {
                let mut s = self.state.write().unwrap();
                s.turn_status = "cancelled".to_string();
                s.turn_id = None;
                s.session_state = "idle".to_string();
                drop(s);
                self.bump_version();
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

// ── RemoteServer ───────────────────────────────────────────────────

/// Bridges remote commands to local session operations.
pub struct RemoteServer {
    shared_secret: [u8; 32],
    state_trackers: Arc<DashMap<String, Arc<RemoteSessionStateTracker>>>,
}

impl Drop for RemoteServer {
    fn drop(&mut self) {
        use crate::agentic::coordination::get_global_coordinator;
        if let Some(coordinator) = get_global_coordinator() {
            for entry in self.state_trackers.iter() {
                let sub_id = format!("remote_tracker_{}", entry.key());
                coordinator.unsubscribe_internal(&sub_id);
            }
        }
    }
}

impl RemoteServer {
    pub fn new(shared_secret: [u8; 32]) -> Self {
        Self {
            shared_secret,
            state_trackers: Arc::new(DashMap::new()),
        }
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
            | RemoteCommand::AnswerQuestion { .. } => {
                self.handle_execution_command(cmd).await
            }

            RemoteCommand::PollSession { .. } => self.handle_poll_command(cmd).await,
        }
    }

    /// Ensure a state tracker exists for the given session and return it.
    fn ensure_tracker(&self, session_id: &str) -> Arc<RemoteSessionStateTracker> {
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

    pub async fn generate_initial_sync(&self) -> RemoteResponse {
        use crate::infrastructure::{get_workspace_path, PathManager};
        use crate::service::conversation::ConversationPersistenceManager;

        let ws_path = get_workspace_path();
        let (has_workspace, path_str, project_name, git_branch) = if let Some(ref p) = ws_path {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string());
            let branch = git2::Repository::open(p)
                .ok()
                .and_then(|repo| repo.head().ok().and_then(|h| h.shorthand().map(String::from)));
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
                        return RemoteResponse::RecentWorkspaces {
                            workspaces: vec![],
                        };
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
                RemoteResponse::RecentWorkspaces { workspaces: entries }
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
                use crate::infrastructure::{get_workspace_path, PathManager};
                use crate::service::conversation::{
                    ConversationPersistenceManager, SessionMetadata, SessionStatus,
                };

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

                        if let Some(wp) = binding_ws_path {
                            if let Ok(pm) = PathManager::new() {
                                let pm = std::sync::Arc::new(pm);
                                if let Ok(conv_mgr) =
                                    ConversationPersistenceManager::new(pm, wp.clone()).await
                                {
                                    let now_ms = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_millis()
                                        as u64;
                                    let meta = SessionMetadata {
                                        session_id: session_id.clone(),
                                        session_name: session_name.to_string(),
                                        agent_type: agent.to_string(),
                                        model_name: "default".to_string(),
                                        created_at: now_ms,
                                        last_active_at: now_ms,
                                        turn_count: 0,
                                        message_count: 0,
                                        tool_call_count: 0,
                                        status: SessionStatus::Active,
                                        terminal_session_id: None,
                                        snapshot_session_id: None,
                                        tags: vec![],
                                        custom_metadata: None,
                                        todos: None,
                                        workspace_path: binding_ws_str,
                                    };
                                    if let Err(e) =
                                        conv_mgr.save_session_metadata(&meta).await
                                    {
                                        error!(
                                            "Failed to sync remote session to workspace: {e}"
                                        );
                                    } else {
                                        info!(
                                            "Remote session synced to workspace: {session_id}"
                                        );
                                    }
                                }
                            }
                        }

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
                self.state_trackers.remove(session_id);
                if let Some(coordinator) =
                    crate::agentic::coordination::get_global_coordinator()
                {
                    let sub_id = format!("remote_tracker_{}", session_id);
                    coordinator.unsubscribe_internal(&sub_id);
                }
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
        use crate::agentic::coordination::get_global_coordinator;

        let coordinator = match get_global_coordinator() {
            Some(c) => c,
            None => {
                return RemoteResponse::Error {
                    message: "Desktop session system not ready".into(),
                };
            }
        };

        match cmd {
            RemoteCommand::SendMessage {
                session_id,
                content,
                agent_type: requested_agent_type,
                images,
            } => {
                self.ensure_tracker(session_id);

                let session_mgr = coordinator.get_session_manager();
                let (session_agent_type, session_ws) = session_mgr
                    .get_session(session_id)
                    .map(|s| (s.agent_type.clone(), s.config.workspace_path.clone()))
                    .unwrap_or_else(|| ("default".to_string(), None));

                let agent_type = requested_agent_type
                    .as_deref()
                    .map(|t| resolve_agent_type(Some(t)).to_string())
                    .unwrap_or(session_agent_type);

                if let Some(ws_path_str) = &session_ws {
                    use crate::infrastructure::{get_workspace_path, set_workspace_path};
                    let current = get_workspace_path();
                    let current_str =
                        current.as_ref().map(|p| p.to_string_lossy().to_string());
                    if current_str.as_deref() != Some(ws_path_str.as_str()) {
                        info!("Remote send_message: temporarily setting workspace for session={session_id} to {ws_path_str}");
                        set_workspace_path(Some(std::path::PathBuf::from(ws_path_str)));
                    }
                }

                let full_content = if let Some(imgs) = &images {
                    if imgs.is_empty() {
                        content.clone()
                    } else {
                        let save_dir = if let Some(ws) = &session_ws {
                            let d = std::path::PathBuf::from(ws)
                                .join(".bitfun")
                                .join("remote-images");
                            let _ = std::fs::create_dir_all(&d);
                            Some(d)
                        } else {
                            None
                        };

                        let mut extra = String::new();
                        for (i, img) in imgs.iter().enumerate() {
                            if let Some(ref dir) = save_dir {
                                if let Some(saved) =
                                    save_data_url_image(dir, &img.name, &img.data_url)
                                {
                                    let path_str = saved.to_string_lossy();
                                    extra.push_str(&format!(
                                        "\n\n[Image: {}]\nPath: {}\nTip: You can use the AnalyzeImage tool with the image_path parameter.",
                                        img.name, path_str
                                    ));
                                    info!("Remote image {i} saved: {path_str}");
                                    continue;
                                }
                            }
                            extra.push_str(&format!(
                                "\n\n[Image: {} (inline)]\nData URL provided inline.\nTip: You can use the AnalyzeImage tool with the data_url parameter.",
                                img.name
                            ));
                        }
                        format!("{content}{extra}")
                    }
                } else {
                    content.clone()
                };

                let is_first_message = session_mgr
                    .get_session(session_id)
                    .map(|s| s.dialog_turn_ids.is_empty())
                    .unwrap_or(true);

                info!(
                    "Remote send_message: session={session_id}, agent_type={agent_type}, images={}",
                    images.as_ref().map_or(0, |v| v.len())
                );
                let turn_id = format!("turn_{}", chrono::Utc::now().timestamp_millis());
                match coordinator
                    .start_dialog_turn(
                        session_id.clone(),
                        full_content,
                        Some(turn_id.clone()),
                        agent_type,
                        true,
                    )
                    .await
                {
                    Ok(()) => {
                        if is_first_message {
                            let sid = session_id.clone();
                            let msg = content.clone();
                            let ws = session_ws.clone();
                            tokio::spawn(async move {
                                if let Some(coord) = get_global_coordinator() {
                                    match coord
                                        .generate_session_title(&sid, &msg, Some(20))
                                        .await
                                    {
                                        Ok(title) => {
                                            Self::persist_session_title(&sid, &title, ws.as_ref())
                                                .await;
                                        }
                                        Err(e) => {
                                            debug!(
                                                "Remote session title generation failed: {e}"
                                            );
                                        }
                                    }
                                }
                            });
                        }
                        RemoteResponse::MessageSent {
                            session_id: session_id.clone(),
                            turn_id,
                        }
                    }
                    Err(e) => RemoteResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            RemoteCommand::CancelTask { session_id } => {
                let session_mgr = coordinator.get_session_manager();
                if let Some(session) = session_mgr.get_session(session_id) {
                    use crate::agentic::core::SessionState;
                    let _ = session_mgr
                        .update_session_state(session_id, SessionState::Idle)
                        .await;
                    if let Some(last_turn_id) = session.dialog_turn_ids.last() {
                        let _ =
                            coordinator.cancel_dialog_turn(session_id, last_turn_id).await;
                    }
                }
                RemoteResponse::TaskCancelled {
                    session_id: session_id.clone(),
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

    async fn persist_session_title(
        session_id: &str,
        title: &str,
        workspace_path: Option<&String>,
    ) {
        use crate::infrastructure::{get_workspace_path, PathManager};
        use crate::service::conversation::ConversationPersistenceManager;

        let ws = workspace_path
            .map(std::path::PathBuf::from)
            .or_else(get_workspace_path);
        let Some(wp) = ws else { return };

        let pm = match PathManager::new() {
            Ok(pm) => std::sync::Arc::new(pm),
            Err(_) => return,
        };
        let conv_mgr = match ConversationPersistenceManager::new(pm, wp).await {
            Ok(m) => m,
            Err(_) => return,
        };
        if let Ok(Some(mut meta)) = conv_mgr.load_session_metadata(session_id).await {
            meta.session_name = title.to_string();
            meta.last_active_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            if let Err(e) = conv_mgr.save_session_metadata(&meta).await {
                error!("Failed to persist remote session title: {e}");
            } else {
                info!("Remote session title persisted: session_id={session_id}, title={title}");
            }
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
