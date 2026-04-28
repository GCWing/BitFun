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
use std::sync::{Arc, OnceLock, RwLock};

use super::encryption;

fn current_workspace_path() -> Option<std::path::PathBuf> {
    crate::service::workspace::get_global_workspace_service()
        .and_then(|service| service.try_get_current_workspace_path())
}

async fn resolve_session_workspace_path(session_id: &str) -> Option<std::path::PathBuf> {
    use crate::agentic::coordination::get_global_coordinator;
    use crate::agentic::persistence::PersistenceManager;
    use crate::infrastructure::PathManager;
    use crate::service::workspace::get_global_workspace_service;

    if let Some(coordinator) = get_global_coordinator() {
        if let Some(workspace_path) = coordinator
            .get_session_manager()
            .get_session(session_id)
            .and_then(|session| session.config.workspace_path.clone())
            .filter(|path| !path.is_empty())
        {
            return Some(std::path::PathBuf::from(workspace_path));
        }
    }

    let workspace_service = get_global_workspace_service()?;
    let mut candidates: Vec<std::path::PathBuf> = workspace_service
        .get_opened_workspaces()
        .await
        .into_iter()
        .map(|workspace| workspace.root_path)
        .collect();

    if let Some(current_workspace) = workspace_service.get_current_workspace().await {
        let current_root = current_workspace.root_path;
        if !candidates.iter().any(|path| path == &current_root) {
            candidates.push(current_root);
        }
    }

    let Ok(path_manager) = PathManager::new() else {
        return None;
    };
    let path_manager = Arc::new(path_manager);
    let Ok(persistence_manager) = PersistenceManager::new(path_manager) else {
        return None;
    };

    for workspace_path in candidates {
        match persistence_manager
            .load_session_metadata(&workspace_path, session_id)
            .await
        {
            Ok(Some(metadata)) => {
                if let Some(bound_workspace) =
                    metadata.workspace_path.filter(|path| !path.is_empty())
                {
                    return Some(std::path::PathBuf::from(bound_workspace));
                }
                return Some(workspace_path);
            }
            Ok(None) => {}
            Err(err) => {
                debug!(
                    "Failed to load session metadata while resolving workspace: session_id={} workspace={} error={}",
                    session_id,
                    workspace_path.display(),
                    err
                );
            }
        }
    }

    None
}

async fn resolve_file_workspace_root(session_id: Option<&str>) -> Option<std::path::PathBuf> {
    if let Some(session_id) = session_id {
        if let Some(workspace_path) = resolve_session_workspace_path(session_id).await {
            return Some(workspace_path);
        }
    }

    current_workspace_path()
}

async fn resolve_session_model_id(session_id: &str) -> Option<String> {
    use crate::agentic::coordination::get_global_coordinator;

    let coordinator = get_global_coordinator()?;
    let session_manager = coordinator.get_session_manager();

    let normalize = |model_id: Option<String>| match model_id {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() || trimmed == "default" {
                Some("auto".to_string())
            } else {
                Some(trimmed.to_string())
            }
        }
        None => Some("auto".to_string()),
    };

    if let Some(session) = session_manager.get_session(session_id) {
        return normalize(session.config.model_id.clone());
    }

    let workspace_path = resolve_session_workspace_path(session_id).await?;
    coordinator
        .restore_session(&workspace_path, session_id)
        .await
        .ok()
        .and_then(|session| normalize(session.config.model_id.clone()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteModelConfig {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub base_url: String,
    pub model_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    pub enabled: bool,
    pub capabilities: Vec<String>,
    pub enable_thinking_process: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteModelCatalog {
    pub version: u64,
    pub models: Vec<RemoteModelConfig>,
    pub default_models: crate::service::config::types::DefaultModelsConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_model_id: Option<String>,
}

async fn load_remote_model_catalog(
    session_id: Option<&str>,
) -> std::result::Result<RemoteModelCatalog, String> {
    use crate::service::config::{
        get_global_config_service,
        types::{AIConfig, GlobalConfig},
    };

    let config_service = get_global_config_service()
        .await
        .map_err(|e| format!("Config service not available: {e}"))?;
    let global_config: GlobalConfig = config_service
        .get_config(None)
        .await
        .map_err(|e| format!("Failed to load global config: {e}"))?;
    let ai_config: AIConfig = global_config.ai;

    let models: Vec<RemoteModelConfig> =
        ai_config
            .models
            .into_iter()
            .map(|model| {
                let reasoning_mode = model.effective_reasoning_mode();

                RemoteModelConfig {
                    id: model.id,
                    name: model.name,
                    provider: model.provider,
                    base_url: model.base_url,
                    model_name: model.model_name,
                    context_window: model.context_window,
                    enabled: model.enabled,
                    capabilities: model
                        .capabilities
                        .into_iter()
                        .map(|capability| {
                            match capability {
                        crate::service::config::types::ModelCapability::TextChat => "text_chat",
                        crate::service::config::types::ModelCapability::ImageUnderstanding => {
                            "image_understanding"
                        }
                        crate::service::config::types::ModelCapability::ImageGeneration => {
                            "image_generation"
                        }
                        crate::service::config::types::ModelCapability::Embedding => "embedding",
                        crate::service::config::types::ModelCapability::Search => "search",
                        crate::service::config::types::ModelCapability::CodeSpecialized => {
                            "code_specialized"
                        }
                        crate::service::config::types::ModelCapability::FunctionCalling => {
                            "function_calling"
                        }
                        crate::service::config::types::ModelCapability::SpeechRecognition => {
                            "speech_recognition"
                        }
                    }
                    .to_string()
                        })
                        .collect(),
                    enable_thinking_process: model.enable_thinking_process,
                    reasoning_mode: Some(
                        match reasoning_mode {
                            crate::service::config::types::ReasoningMode::Default => "default",
                            crate::service::config::types::ReasoningMode::Enabled => "enabled",
                            crate::service::config::types::ReasoningMode::Disabled => "disabled",
                            crate::service::config::types::ReasoningMode::Adaptive => "adaptive",
                        }
                        .to_string(),
                    ),
                    reasoning_effort: model.reasoning_effort,
                    thinking_budget_tokens: model.thinking_budget_tokens,
                }
            })
            .collect();

    let session_model_id = if let Some(session_id) = session_id {
        resolve_session_model_id(session_id).await
    } else {
        None
    };
    Ok(RemoteModelCatalog {
        version: global_config.last_modified.timestamp_millis().max(0) as u64,
        models,
        default_models: ai_config.default_models,
        session_model_id,
    })
}

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
    ListAssistants,
    SetAssistant {
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
    GetModelCatalog {
        session_id: Option<String>,
    },
    SetSessionModel {
        session_id: String,
        model_id: String,
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
        image_contexts: Option<Vec<crate::agentic::image_analysis::ImageContextData>>,
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
        known_model_catalog_version: Option<u64>,
    },
    /// Read a workspace file and return its base64-encoded content.
    ///
    /// `path` may be an absolute path or a path relative to the active
    /// workspace root. When `session_id` is present, relative paths are
    /// resolved against that session's bound workspace first.
    ReadFile {
        path: String,
        session_id: Option<String>,
    },
    /// Read a chunk of a workspace file.  `offset` is the byte offset into the
    /// raw file and `limit` is the maximum number of raw bytes to return.
    /// The response contains the base64-encoded chunk plus total file size so
    /// the client knows when it has all the data.
    ReadFileChunk {
        path: String,
        session_id: Option<String>,
        offset: u64,
        limit: u64,
    },
    /// Get metadata (name, size, mime_type) for a workspace file without
    /// transferring its content.  Used by the mobile client to display file
    /// cards before the user confirms the download.
    GetFileInfo {
        path: String,
        session_id: Option<String>,
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
        /// `"normal"` | `"assistant"` | `"remote"` — mirrors [`crate::service::workspace::WorkspaceKind`].
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_kind: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        assistant_id: Option<String>,
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
    AssistantList {
        assistants: Vec<AssistantEntry>,
    },
    AssistantUpdated {
        success: bool,
        path: Option<String>,
        name: Option<String>,
        error: Option<String>,
    },
    SessionList {
        sessions: Vec<SessionInfo>,
        has_more: bool,
    },
    SessionCreated {
        session_id: String,
    },
    ModelCatalog {
        catalog: RemoteModelCatalog,
    },
    SessionModelUpdated {
        session_id: String,
        model_id: String,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_kind: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        assistant_id: Option<String>,
        sessions: Vec<SessionInfo>,
        has_more_sessions: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        authenticated_user_id: Option<String>,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        model_catalog: Box<Option<RemoteModelCatalog>>,
    },
    AnswerAccepted,
    InteractionAccepted {
        action: String,
        target_id: String,
    },
    /// Response to `ReadFile`: the file contents encoded as a base64 data-URL.
    FileContent {
        name: String,
        content_base64: String,
        mime_type: String,
        size: u64,
    },
    /// Response to `ReadFileChunk`.
    FileChunk {
        name: String,
        chunk_base64: String,
        offset: u64,
        chunk_size: u64,
        total_size: u64,
        mime_type: String,
    },
    /// Response to `GetFileInfo`: metadata only, no file content.
    FileInfo {
        name: String,
        size: u64,
        mime_type: String,
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
pub struct ChatImageAttachment {
    pub name: String,
    pub data_url: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<ChatImageAttachment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageItem {
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<RemoteToolStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_subagent: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentWorkspaceEntry {
    pub path: String,
    pub name: String,
    pub last_opened: String,
    /// `"normal"` | `"assistant"` | `"remote"`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantEntry {
    pub path: String,
    pub name: String,
    pub assistant_id: Option<String>,
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

/// Build a slim version of tool params for mobile preview.
/// Strips large string values (file content, diffs, etc.) to keep payload small,
/// while preserving all short fields so the frontend can parse and display them.
fn make_slim_params(params: &serde_json::Value) -> Option<String> {
    match params {
        serde_json::Value::Object(obj) => {
            let slim: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .filter_map(|(k, v)| match v {
                    serde_json::Value::String(s) if s.len() > 200 => None,
                    _ => Some((k.clone(), v.clone())),
                })
                .collect();
            if slim.is_empty() {
                return None;
            }
            serde_json::to_string(&serde_json::Value::Object(slim)).ok()
        }
        serde_json::Value::String(s) => Some(s.chars().take(200).collect()),
        _ => None,
    }
}

/// Compress a base64 data-URL image to a small thumbnail for mobile display.
/// Falls back to the original if decoding/compression fails or the image is
/// already within `max_bytes`.
fn compress_data_url_for_mobile(data_url: &str, max_bytes: usize) -> String {
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;
    use image::imageops::FilterType;

    const MAX_THUMBNAIL_DIM: u32 = 400;

    let Some(comma_pos) = data_url.find(',') else {
        return data_url.to_string();
    };
    let b64_data = &data_url[comma_pos + 1..];

    if b64_data.len() * 3 / 4 <= max_bytes {
        return data_url.to_string();
    }

    let Ok(raw_bytes) = BASE64.decode(b64_data) else {
        return data_url.to_string();
    };

    let Ok(img) = image::load_from_memory(&raw_bytes) else {
        return data_url.to_string();
    };

    let resized = if img.width() > MAX_THUMBNAIL_DIM || img.height() > MAX_THUMBNAIL_DIM {
        img.resize(MAX_THUMBNAIL_DIM, MAX_THUMBNAIL_DIM, FilterType::Triangle)
    } else {
        img
    };

    fn encode_jpeg(img: &image::DynamicImage, quality: u8) -> Option<Vec<u8>> {
        let mut buf = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
        img.write_with_encoder(encoder).ok()?;
        Some(buf)
    }

    for quality in [75u8, 60, 45, 30] {
        if let Some(buf) = encode_jpeg(&resized, quality) {
            if buf.len() <= max_bytes || quality == 30 {
                let b64 = BASE64.encode(&buf);
                return format!("data:image/jpeg;base64,{b64}");
            }
        }
    }

    data_url.to_string()
}

/// Max thumbnail size per image sent to mobile (100 KB).
const MOBILE_IMAGE_MAX_BYTES: usize = 100 * 1024;

/// Convert persisted turns into mobile ChatMessages.
/// This is the same data source the desktop frontend uses.
fn turns_to_chat_messages(turns: &[crate::service::session::DialogTurnData]) -> Vec<ChatMessage> {
    use crate::service::session::TurnStatus;

    let mut result = Vec::new();

    for turn in turns {
        if !turn.kind.is_model_visible() {
            continue;
        }

        let images = turn
            .user_message
            .metadata
            .as_ref()
            .and_then(|m| m.get("images"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        let name = v.get("name")?.as_str()?.to_string();
                        let raw_url = v.get("data_url")?.as_str()?;
                        let data_url =
                            compress_data_url_for_mobile(raw_url, MOBILE_IMAGE_MAX_BYTES);
                        Some(ChatImageAttachment { name, data_url })
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty());

        // Prefer original_text from metadata (pre-enhancement) for display
        let display_content = turn
            .user_message
            .metadata
            .as_ref()
            .and_then(|m| m.get("original_text"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| strip_user_input_tags(&turn.user_message.content));

        result.push(ChatMessage {
            id: turn.user_message.id.clone(),
            role: "user".to_string(),
            content: display_content,
            timestamp: (turn.user_message.timestamp / 1000).to_string(),
            metadata: None,
            tools: None,
            thinking: None,
            items: None,
            images,
        });

        // Skip assistant message for in-progress turns.  The active turn's
        // content is delivered via the real-time overlay, not the historical
        // list.  Including an empty / partial assistant message here would
        // "consume" a slot in the count-based skip cursor and prevent the
        // final version from ever being delivered.
        if turn.status == TurnStatus::InProgress {
            continue;
        }

        // Collect ordered items across all rounds, preserving interleaved order
        struct OrderedEntry {
            order_index: Option<usize>,
            sequence: usize,
            round_idx: usize,
            item: ChatMessageItem,
        }
        let mut ordered: Vec<OrderedEntry> = Vec::new();
        let mut tools_flat = Vec::new();
        let mut thinking_parts = Vec::new();
        let mut text_parts = Vec::new();
        let mut sequence = 0usize;

        for (round_idx, round) in turn.model_rounds.iter().enumerate() {
            // Iterate in streaming order: thinking → text → tools.
            // The model first thinks, then outputs text (which may reference
            // tool calls), and finally the tools are detected and executed.
            // This matches the real-time display order on the tracker.
            for t in &round.thinking_items {
                if t.is_subagent_item.unwrap_or(false) {
                    continue;
                }
                if !t.content.is_empty() {
                    thinking_parts.push(t.content.clone());
                    ordered.push(OrderedEntry {
                        order_index: t.order_index,
                        sequence,
                        round_idx,
                        item: ChatMessageItem {
                            item_type: "thinking".to_string(),
                            content: Some(t.content.clone()),
                            tool: None,
                            is_subagent: None,
                        },
                    });
                    sequence += 1;
                }
            }
            for t in &round.text_items {
                if t.is_subagent_item.unwrap_or(false) {
                    continue;
                }
                if !t.content.is_empty() {
                    text_parts.push(t.content.clone());
                    ordered.push(OrderedEntry {
                        order_index: t.order_index,
                        sequence,
                        round_idx,
                        item: ChatMessageItem {
                            item_type: "text".to_string(),
                            content: Some(t.content.clone()),
                            tool: None,
                            is_subagent: None,
                        },
                    });
                    sequence += 1;
                }
            }
            for t in &round.tool_items {
                if t.is_subagent_item.unwrap_or(false) {
                    continue;
                }
                let status_str = t.status.as_deref().unwrap_or(if t.tool_result.is_some() {
                    "completed"
                } else {
                    "running"
                });
                let tool_status = RemoteToolStatus {
                    id: t.id.clone(),
                    name: t.tool_name.clone(),
                    status: status_str.to_string(),
                    duration_ms: t.duration_ms,
                    start_ms: Some(t.start_time),
                    input_preview: make_slim_params(&t.tool_call.input),
                    tool_input: if t.tool_name == "AskUserQuestion"
                        || t.tool_name == "Task"
                        || t.tool_name == "TodoWrite"
                    {
                        Some(t.tool_call.input.clone())
                    } else {
                        None
                    },
                };
                tools_flat.push(tool_status.clone());
                ordered.push(OrderedEntry {
                    order_index: t.order_index,
                    sequence,
                    round_idx,
                    item: ChatMessageItem {
                        item_type: "tool".to_string(),
                        content: None,
                        tool: Some(tool_status),
                        is_subagent: None,
                    },
                });
                sequence += 1;
            }
        }

        // Sort by round first (rounds are strictly sequential), then by
        // order_index within each round.  order_index is per-round (resets
        // to 0 each round), so it must NOT be compared across rounds.
        ordered.sort_by(|a, b| {
            let round_cmp = a.round_idx.cmp(&b.round_idx);
            if round_cmp != std::cmp::Ordering::Equal {
                return round_cmp;
            }
            match (a.order_index, b.order_index) {
                (Some(a_idx), Some(b_idx)) => a_idx.cmp(&b_idx),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.sequence.cmp(&b.sequence),
            }
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
            tools: if tools_flat.is_empty() {
                None
            } else {
                Some(tools_flat)
            },
            thinking: if thinking_parts.is_empty() {
                None
            } else {
                Some(thinking_parts.join("\n\n"))
            },
            items: if items.is_empty() { None } else { Some(items) },
            images: None,
        });
    }

    result
}

/// Load historical chat messages from the unified project session store.
/// Uses the same data source as the desktop frontend.
async fn load_chat_messages_from_conversation_persistence(
    workspace_path: &std::path::Path,
    session_id: &str,
) -> (Vec<ChatMessage>, bool) {
    use crate::agentic::persistence::PersistenceManager;
    use crate::infrastructure::PathManager;

    let Ok(pm) = PathManager::new() else {
        return (vec![], false);
    };
    let pm = std::sync::Arc::new(pm);
    let Ok(store) = PersistenceManager::new(pm) else {
        return (vec![], false);
    };
    let Ok(turns) = store.load_session_turns(workspace_path, session_id).await else {
        return (vec![], false);
    };
    (turns_to_chat_messages(&turns), false)
}

fn strip_user_input_tags(content: &str) -> String {
    let s = crate::agentic::core::strip_prompt_markup(content);
    // Extract original question from enhancer-wrapped content
    if s.starts_with("User uploaded") {
        if let Some(pos) = s.find("User's question:\n") {
            return s[pos + "User's question:\n".len()..].trim().to_string();
        }
    }
    s
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

/// Convert legacy `ImageAttachment` to unified `ImageContextData`.
pub fn images_to_contexts(
    images: Option<&Vec<ImageAttachment>>,
) -> Vec<crate::agentic::image_analysis::ImageContextData> {
    let Some(imgs) = images.filter(|v| !v.is_empty()) else {
        return Vec::new();
    };
    imgs.iter()
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

            crate::agentic::image_analysis::ImageContextData {
                id: format!("remote_img_{}", uuid::Uuid::new_v4()),
                image_path: None,
                data_url: Some(img.data_url.clone()),
                mime_type,
                metadata: Some(serde_json::json!({
                    "name": img.name,
                    "source": "remote"
                })),
            }
        })
        .collect()
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
    /// Set on structural events (turn start/complete) that change persisted
    /// messages.  Cleared after the poll handler loads persistence.  Allows
    /// skipping the expensive disk read during streaming.
    persistence_dirty: bool,
}

/// Lightweight event broadcast by the tracker for real-time consumers (e.g. bots).
#[derive(Debug, Clone)]
pub enum TrackerEvent {
    TextChunk(String),
    ThinkingChunk(String),
    /// All thinking content for the current round has been emitted.
    /// Carries the full accumulated thinking text so consumers can send
    /// a single summary instead of per-chunk messages.
    ThinkingEnd,
    ToolStarted {
        tool_id: String,
        tool_name: String,
        params: Option<serde_json::Value>,
    },
    ToolCompleted {
        tool_id: String,
        tool_name: String,
        duration_ms: Option<u64>,
        success: bool,
    },
    TurnCompleted {
        turn_id: String,
    },
    TurnFailed {
        turn_id: String,
        error: String,
    },
    TurnCancelled {
        turn_id: String,
    },
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
        let (event_tx, _) = tokio::sync::broadcast::channel(1024);
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
                persistence_dirty: true,
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
        let has_items = !s.active_items.is_empty();
        s.turn_id.as_ref().map(|tid| ActiveTurnSnapshot {
            turn_id: tid.clone(),
            status: s.turn_status.clone(),
            // When items exist they already contain the text/thinking content.
            // Skip the duplicate top-level fields to halve the payload.
            text: if has_items {
                String::new()
            } else {
                s.accumulated_text.clone()
            },
            thinking: if has_items {
                String::new()
            } else {
                s.accumulated_thinking.clone()
            },
            tools: s.active_tools.clone(),
            round_index: s.round_index,
            items: if has_items {
                Some(s.active_items.clone())
            } else {
                None
            },
        })
    }

    pub fn session_state(&self) -> String {
        self.state.read().unwrap().session_state.clone()
    }

    pub fn title(&self) -> String {
        self.state.read().unwrap().title.clone()
    }

    pub fn turn_status(&self) -> String {
        self.state.read().unwrap().turn_status.clone()
    }

    /// Return the full accumulated response text for the current turn.
    ///
    /// Unlike the broadcast channel (which can lag and drop chunks), this
    /// is maintained directly from the source `AgenticEvent` stream and is
    /// therefore authoritative.
    pub fn accumulated_text(&self) -> String {
        self.state.read().unwrap().accumulated_text.clone()
    }

    /// Return the full accumulated thinking text for the current turn.
    pub fn accumulated_thinking(&self) -> String {
        self.state.read().unwrap().accumulated_thinking.clone()
    }

    /// Returns true if the turn has ended (completed/failed/cancelled) but
    /// the tracker state hasn't been cleaned up yet (waiting for persistence).
    pub fn is_turn_finished(&self) -> bool {
        let s = self.state.read().unwrap();
        s.turn_id.is_some()
            && matches!(s.turn_status.as_str(), "completed" | "failed" | "cancelled")
    }

    /// Seed initial turn state when the tracker is created after the
    /// `DialogTurnStarted` event already fired (e.g. desktop-triggered turns).
    /// Subsequent streaming events will be captured normally by the subscriber.
    pub fn initialize_active_turn(&self, turn_id: String) {
        let mut s = self.state.write().unwrap();
        if s.turn_id.is_none() {
            s.turn_id = Some(turn_id);
            s.turn_status = "active".to_string();
            s.session_state = "running".to_string();
        }
        drop(s);
        self.bump_version();
    }

    /// Clear tracker state after the persisted historical message is confirmed
    /// available. Called by the poll handler to complete the atomic transition.
    pub fn finalize_completed_turn(&self) {
        let mut s = self.state.write().unwrap();
        if matches!(s.turn_status.as_str(), "completed" | "failed" | "cancelled") {
            s.turn_id = None;
            s.accumulated_text.clear();
            s.accumulated_thinking.clear();
            s.active_tools.clear();
            s.active_items.clear();
        }
    }

    /// Whether the persisted message list may have changed since the last
    /// poll.  Structural events (turn start / complete) set this flag;
    /// streaming events (text / thinking chunks) do not.
    pub fn is_persistence_dirty(&self) -> bool {
        self.state.read().unwrap().persistence_dirty
    }

    pub fn mark_persistence_clean(&self) {
        self.state.write().unwrap().persistence_dirty = false;
    }

    /// Find the last item of `target_type` with matching `subagent_marker` that
    /// can be extended, skipping over the complementary text/thinking type.
    /// Tool items act as boundaries — we never merge across tool items.
    /// This mirrors the desktop's EventBatcher behaviour where text and thinking
    /// accumulate independently within a single ModelRound.
    fn find_mergeable_item(
        items: &[ChatMessageItem],
        target_type: &str,
        subagent_marker: &Option<bool>,
    ) -> Option<usize> {
        for i in (0..items.len()).rev() {
            let item = &items[i];
            if item.item_type == "tool" {
                return None;
            }
            if item.item_type == target_type && &item.is_subagent == subagent_marker {
                return Some(i);
            }
        }
        None
    }

    fn upsert_active_tool(
        state: &mut TrackerState,
        tool_id: &str,
        tool_name: &str,
        status: &str,
        input_preview: Option<String>,
        tool_input: Option<serde_json::Value>,
        is_subagent: bool,
    ) {
        let resolved_id = if tool_id.is_empty() {
            format!("{}-{}", tool_name, state.active_tools.len())
        } else {
            tool_id.to_string()
        };
        let allow_name_fallback = tool_id.is_empty() && !tool_name.is_empty();
        let subagent_marker = if is_subagent { Some(true) } else { None };

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
                is_subagent: subagent_marker,
            });
            return;
        }

        if let Some(item) = state.active_items.iter_mut().rev().find(|i| {
            i.item_type == "tool"
                && i.tool.as_ref().is_some_and(|t| {
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
                AE::TextChunk {
                    subagent_parent_info,
                    ..
                }
                | AE::ThinkingChunk {
                    subagent_parent_info,
                    ..
                }
                | AE::ToolEvent {
                    subagent_parent_info,
                    ..
                } => subagent_parent_info
                    .as_ref()
                    .is_some_and(|p| p.session_id == self.target_session_id),
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
                let subagent_marker = if is_subagent { Some(true) } else { None };
                let mut s = self.state.write().unwrap();
                if !is_subagent {
                    s.accumulated_text.push_str(text);
                }
                let extend_idx =
                    Self::find_mergeable_item(&s.active_items, "text", &subagent_marker);
                if let Some(idx) = extend_idx {
                    let item = &mut s.active_items[idx];
                    let c = item.content.get_or_insert_with(String::new);
                    c.push_str(text);
                } else {
                    s.active_items.push(ChatMessageItem {
                        item_type: "text".to_string(),
                        content: Some(text.clone()),
                        tool: None,
                        is_subagent: subagent_marker,
                    });
                }
                drop(s);
                self.bump_version();
                let _ = self.event_tx.send(TrackerEvent::TextChunk(text.clone()));
            }
            AE::ThinkingChunk {
                content, is_end, ..
            } => {
                let clean = content.replace("</thinking>", "").replace("<thinking>", "");
                let subagent_marker = if is_subagent { Some(true) } else { None };
                let mut s = self.state.write().unwrap();
                if !is_subagent {
                    s.accumulated_thinking.push_str(&clean);
                }
                let extend_idx =
                    Self::find_mergeable_item(&s.active_items, "thinking", &subagent_marker);
                if let Some(idx) = extend_idx {
                    let item = &mut s.active_items[idx];
                    let c = item.content.get_or_insert_with(String::new);
                    c.push_str(&clean);
                } else {
                    s.active_items.push(ChatMessageItem {
                        item_type: "thinking".to_string(),
                        content: Some(clean),
                        tool: None,
                        is_subagent: subagent_marker,
                    });
                }
                drop(s);
                self.bump_version();
                if *is_end {
                    let _ = self.event_tx.send(TrackerEvent::ThinkingEnd);
                } else if !content.is_empty() {
                    let _ = self
                        .event_tx
                        .send(TrackerEvent::ThinkingChunk(content.clone()));
                }
            }
            AE::ToolEvent { tool_event, .. } => {
                if let Ok(val) = serde_json::to_value(tool_event) {
                    let event_type = val.get("event_type").and_then(|v| v.as_str()).unwrap_or("");
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
                    let mut pending_tool_event: Option<TrackerEvent> = None;
                    match event_type {
                        "EarlyDetected" => {
                            Self::upsert_active_tool(
                                &mut s,
                                &tool_id,
                                &tool_name,
                                "preparing",
                                None,
                                None,
                                is_subagent,
                            );
                        }
                        "ConfirmationNeeded" => {
                            let params = val.get("params").cloned();
                            let input_preview = params.as_ref().and_then(make_slim_params);
                            Self::upsert_active_tool(
                                &mut s,
                                &tool_id,
                                &tool_name,
                                "pending_confirmation",
                                input_preview,
                                params,
                                is_subagent,
                            );
                        }
                        "Started" => {
                            let params = val.get("params").cloned();
                            let input_preview = params.as_ref().and_then(make_slim_params);
                            let tool_input = if tool_name == "AskUserQuestion"
                                || tool_name == "Task"
                                || tool_name == "TodoWrite"
                            {
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
                                is_subagent,
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
                                is_subagent,
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
                                is_subagent,
                            );
                        }
                        "Completed" | "Succeeded" => {
                            let duration = val.get("duration_ms").and_then(|v| v.as_u64());
                            if let Some(t) = s.active_tools.iter_mut().rev().find(|t| {
                                (t.id == tool_id || (allow_name_fallback && t.name == tool_name))
                                    && t.status == "running"
                            }) {
                                t.status = "completed".to_string();
                                t.duration_ms = duration;
                            }
                            if let Some(item) = s.active_items.iter_mut().rev().find(|i| {
                                i.item_type == "tool"
                                    && i.tool.as_ref().is_some_and(|t| {
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
                            pending_tool_event = Some(TrackerEvent::ToolCompleted {
                                tool_id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                                duration_ms: duration,
                                success: true,
                            });
                        }
                        "Failed" => {
                            if let Some(t) = s.active_tools.iter_mut().rev().find(|t| {
                                (t.id == tool_id || (allow_name_fallback && t.name == tool_name))
                                    && t.status == "running"
                            }) {
                                t.status = "failed".to_string();
                            }
                            if let Some(item) = s.active_items.iter_mut().rev().find(|i| {
                                i.item_type == "tool"
                                    && i.tool.as_ref().is_some_and(|t| {
                                        (t.id == tool_id
                                            || (allow_name_fallback && t.name == tool_name))
                                            && t.status == "running"
                                    })
                            }) {
                                if let Some(t) = item.tool.as_mut() {
                                    t.status = "failed".to_string();
                                }
                            }
                            pending_tool_event = Some(TrackerEvent::ToolCompleted {
                                tool_id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                                duration_ms: None,
                                success: false,
                            });
                        }
                        "Cancelled" => {
                            if let Some(t) = s.active_tools.iter_mut().rev().find(|t| {
                                (t.id == tool_id || (allow_name_fallback && t.name == tool_name))
                                    && matches!(
                                        t.status.as_str(),
                                        "running" | "pending_confirmation" | "confirmed"
                                    )
                            }) {
                                t.status = "cancelled".to_string();
                            }
                            if let Some(item) = s.active_items.iter_mut().rev().find(|i| {
                                i.item_type == "tool"
                                    && i.tool.as_ref().is_some_and(|t| {
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
                    if let Some(evt) = pending_tool_event {
                        let _ = self.event_tx.send(evt);
                    }
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
                s.persistence_dirty = true;
                drop(s);
                self.bump_version();
            }
            AE::DialogTurnCompleted { turn_id, .. } if is_direct => {
                let mut s = self.state.write().unwrap();
                s.turn_status = "completed".to_string();
                s.session_state = "idle".to_string();
                s.persistence_dirty = true;
                drop(s);
                self.bump_version();
                let _ = self.event_tx.send(TrackerEvent::TurnCompleted {
                    turn_id: turn_id.clone(),
                });
            }
            AE::DialogTurnFailed { turn_id, error, .. } if is_direct => {
                let mut s = self.state.write().unwrap();
                s.turn_status = "failed".to_string();
                s.session_state = "idle".to_string();
                s.persistence_dirty = true;
                drop(s);
                self.bump_version();
                let _ = self.event_tx.send(TrackerEvent::TurnFailed {
                    turn_id: turn_id.clone(),
                    error: error.clone(),
                });
            }
            AE::DialogTurnCancelled { turn_id, .. } if is_direct => {
                let mut s = self.state.write().unwrap();
                s.turn_status = "cancelled".to_string();
                s.session_state = "idle".to_string();
                s.persistence_dirty = true;
                drop(s);
                self.bump_version();
                let _ = self.event_tx.send(TrackerEvent::TurnCancelled {
                    turn_id: turn_id.clone(),
                });
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
    ///
    /// When the tracker is freshly created and the session already has an active
    /// turn (e.g. a desktop-triggered dialog), the tracker is seeded with the
    /// turn id so that `snapshot_active_turn()` immediately returns a valid
    /// snapshot.  Without this, a late-created tracker would miss the
    /// `DialogTurnStarted` event and the mobile would see no active-turn
    /// overlay until the turn completes.
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

            let session_mgr = coordinator.get_session_manager();
            if let Some(session) = session_mgr.get_session(session_id) {
                if let crate::agentic::core::SessionState::Processing {
                    current_turn_id, ..
                } = &session.state
                {
                    tracker.initialize_active_turn(current_turn_id.clone());
                    info!(
                        "Seeded tracker with existing active turn {} for session {}",
                        current_turn_id, session_id
                    );
                }
            }
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

    /// Dispatch a SendMessage command: ensure tracker, restore session, submit via
    /// [`DialogScheduler`](crate::agentic::coordination::DialogScheduler) (same as desktop).
    /// When the session is already processing, the message is queued and the current turn
    /// may yield after the current model round (for interactive `submission_policy` sources).
    /// Returns whether this message started immediately or was only queued, plus ids.
    /// If `turn_id` is `None`, one is auto-generated before queueing.
    ///
    /// All platforms (desktop, mobile, bot) use the same `ImageContextData` format.
    pub async fn send_message(
        &self,
        session_id: &str,
        content: String,
        agent_type: Option<&str>,
        image_contexts: Vec<crate::agentic::image_analysis::ImageContextData>,
        submission_policy: crate::agentic::coordination::DialogSubmissionPolicy,
        turn_id: Option<String>,
    ) -> std::result::Result<crate::agentic::coordination::DialogSubmitOutcome, String> {
        use crate::agentic::coordination::{get_global_coordinator, get_global_scheduler};

        let coordinator = get_global_coordinator()
            .ok_or_else(|| "Desktop session system not ready".to_string())?;

        let scheduler = get_global_scheduler()
            .ok_or_else(|| "Dialog scheduler is not initialized".to_string())?;

        self.ensure_tracker(session_id);

        let session_mgr = coordinator.get_session_manager();
        let binding_workspace = resolve_session_workspace_path(session_id)
            .await
            .map(|path| path.to_string_lossy().into_owned());

        let _ = match session_mgr.get_session(session_id) {
            Some(session) => Some(session),
            None => {
                if let Some(workspace_path) = binding_workspace.as_deref() {
                    coordinator
                        .restore_session(std::path::Path::new(workspace_path), session_id)
                        .await
                        .ok()
                } else {
                    None
                }
            }
        };

        // Pre-warm the terminal so shell integration is ready before BashTool runs.
        // Bot/remote sessions have no Terminal panel to pre-create the session, so the
        // AI model's processing time (typically 5-15 s) gives shell integration a head
        // start.  When BashTool eventually calls get_or_create, the binding already
        // exists and the 30-second readiness wait is skipped entirely.
        {
            use terminal_core::session::SessionSource;
            use terminal_core::{TerminalApi, TerminalBindingOptions};
            let sid = session_id.to_string();
            let binding_workspace_for_terminal = binding_workspace.clone();
            tokio::spawn(async move {
                let Ok(api) = TerminalApi::from_singleton() else {
                    return;
                };
                let binding = api.session_manager().binding();
                if binding.get(&sid).is_some() {
                    return;
                }
                let workspace = binding_workspace_for_terminal.clone();
                let name = format!("Chat-{}", &sid[..8.min(sid.len())]);
                match binding
                    .get_or_create(
                        &sid,
                        TerminalBindingOptions {
                            working_directory: workspace,
                            session_id: Some(sid.clone()),
                            session_name: Some(name),
                            env: Some(
                                crate::agentic::tools::implementations::bash_tool::BashTool::noninteractive_env(),
                            ),
                            source: Some(SessionSource::Agent),
                            ..Default::default()
                        },
                    )
                    .await
                {
                    Ok(_) => info!("Terminal pre-warmed for remote session {sid}"),
                    Err(e) => debug!("Terminal pre-warm skipped for {sid}: {e}"),
                }
            });
        }

        let resolved_agent_type = agent_type
            .map(|t| resolve_agent_type(Some(t)).to_string())
            .unwrap_or_else(|| "agentic".to_string());

        let turn_id =
            turn_id.unwrap_or_else(|| format!("turn_{}", chrono::Utc::now().timestamp_millis()));

        let image_payload = if image_contexts.is_empty() {
            None
        } else {
            Some(image_contexts)
        };

        scheduler
            .submit(
                session_id.to_string(),
                content,
                None,
                Some(turn_id.clone()),
                resolved_agent_type,
                binding_workspace,
                submission_policy,
                None,
                None,
                image_payload,
            )
            .await
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
            None => {
                let workspace_path = resolve_session_workspace_path(session_id)
                    .await
                    .ok_or_else(|| {
                        format!("Workspace path not available for session: {}", session_id)
                    })?;
                coordinator
                    .restore_session(&workspace_path, session_id)
                    .await
                    .map_err(|e| format!("Session not found: {e}"))?
            }
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
            | RemoteCommand::SetWorkspace { .. }
            | RemoteCommand::ListAssistants
            | RemoteCommand::SetAssistant { .. } => self.handle_workspace_command(cmd).await,

            RemoteCommand::ListSessions { .. }
            | RemoteCommand::CreateSession { .. }
            | RemoteCommand::GetModelCatalog { .. }
            | RemoteCommand::SetSessionModel { .. }
            | RemoteCommand::GetSessionMessages { .. }
            | RemoteCommand::DeleteSession { .. } => self.handle_session_command(cmd).await,

            RemoteCommand::SendMessage { .. }
            | RemoteCommand::CancelTask { .. }
            | RemoteCommand::ConfirmTool { .. }
            | RemoteCommand::RejectTool { .. }
            | RemoteCommand::CancelTool { .. }
            | RemoteCommand::AnswerQuestion { .. } => self.handle_execution_command(cmd).await,

            RemoteCommand::PollSession { .. } => self.handle_poll_command(cmd).await,

            RemoteCommand::ReadFile { path, session_id } => {
                self.handle_read_file(path, session_id.as_deref()).await
            }
            RemoteCommand::ReadFileChunk {
                path,
                session_id,
                offset,
                limit,
            } => {
                self.handle_read_file_chunk(path, session_id.as_deref(), *offset, *limit)
                    .await
            }
            RemoteCommand::GetFileInfo { path, session_id } => {
                self.handle_get_file_info(path, session_id.as_deref()).await
            }
        }
    }

    fn ensure_tracker(&self, session_id: &str) -> Arc<RemoteSessionStateTracker> {
        get_or_init_global_dispatcher().ensure_tracker(session_id)
    }

    pub async fn generate_initial_sync(
        &self,
        authenticated_user_id: Option<String>,
    ) -> RemoteResponse {
        use crate::agentic::persistence::PersistenceManager;
        use crate::infrastructure::PathManager;
        use crate::service::workspace::{get_global_workspace_service, WorkspaceKind};

        let (
            ws_path,
            has_workspace,
            path_str,
            project_name,
            git_branch,
            workspace_kind,
            assistant_id,
        ) = if let Some(ws_service) = get_global_workspace_service() {
            if let Some(ws) = ws_service.get_current_workspace().await {
                let p = ws.root_path.clone();
                let branch = git2::Repository::open(&p).ok().and_then(|repo| {
                    repo.head()
                        .ok()
                        .and_then(|h| h.shorthand().map(String::from))
                });
                let kind_str = match ws.workspace_kind {
                    WorkspaceKind::Normal => "normal",
                    WorkspaceKind::Assistant => "assistant",
                    WorkspaceKind::Remote => "remote",
                };
                (
                    Some(p.clone()),
                    true,
                    Some(p.to_string_lossy().to_string()),
                    Some(ws.name.clone()),
                    branch,
                    Some(kind_str.to_string()),
                    ws.assistant_id.clone(),
                )
            } else {
                (None, false, None, None, None, None, None)
            }
        } else {
            (None, false, None, None, None, None, None)
        };

        let (sessions, has_more) = if let Some(ref wp) = ws_path {
            let ws_str = wp.to_string_lossy().to_string();
            let ws_name = wp.file_name().map(|n| n.to_string_lossy().to_string());
            if let Ok(pm) = PathManager::new() {
                let pm = std::sync::Arc::new(pm);
                if let Ok(store) = PersistenceManager::new(pm) {
                    if let Ok(all_meta) = store.list_session_metadata(wp).await {
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
            workspace_kind,
            assistant_id,
            sessions,
            has_more_sessions: has_more,
            authenticated_user_id,
        }
    }

    // ── Poll command handler ────────────────────────────────────────

    async fn handle_poll_command(&self, cmd: &RemoteCommand) -> RemoteResponse {
        let RemoteCommand::PollSession {
            session_id,
            since_version,
            known_msg_count,
            known_model_catalog_version,
        } = cmd
        else {
            return RemoteResponse::Error {
                message: "expected poll_session".into(),
            };
        };

        let tracker = self.ensure_tracker(session_id);
        let current_version = tracker.version();
        let current_model_catalog = load_remote_model_catalog(Some(session_id)).await.ok();
        let current_model_catalog_version = current_model_catalog
            .as_ref()
            .map(|catalog| catalog.version)
            .unwrap_or(0);
        let requested_model_catalog_version = known_model_catalog_version.unwrap_or(0);
        let should_send_model_catalog =
            requested_model_catalog_version != current_model_catalog_version;

        if *since_version == current_version && *since_version > 0 && !should_send_model_catalog {
            return RemoteResponse::SessionPoll {
                version: current_version,
                changed: false,
                session_state: None,
                title: None,
                new_messages: None,
                total_msg_count: None,
                active_turn: None,
                model_catalog: Box::new(None),
            };
        }

        // Fast path: during active streaming, only the real-time snapshot
        // changes — persisted messages stay the same.  Skip the expensive
        // disk read and return just the snapshot.
        let needs_persistence = *since_version == 0 || tracker.is_persistence_dirty();

        if !needs_persistence {
            let active_turn = tracker.snapshot_active_turn();
            let sess_state = tracker.session_state();
            let title = tracker.title();
            return RemoteResponse::SessionPoll {
                version: current_version,
                changed: true,
                session_state: Some(sess_state),
                title: if title.is_empty() { None } else { Some(title) },
                new_messages: None,
                total_msg_count: None,
                active_turn,
                model_catalog: Box::new(if should_send_model_catalog {
                    current_model_catalog
                } else {
                    None
                }),
            };
        }

        let Some(workspace_path) = resolve_session_workspace_path(session_id).await else {
            return RemoteResponse::Error {
                message: format!("Workspace path not available for session: {}", session_id),
            };
        };
        let (all_chat_msgs, _) =
            load_chat_messages_from_conversation_persistence(&workspace_path, session_id).await;
        let total_msg_count = all_chat_msgs.len();
        let skip = *known_msg_count;
        let new_messages: Vec<ChatMessage> = all_chat_msgs.into_iter().skip(skip).collect();

        let turn_finished = tracker.is_turn_finished();
        let has_assistant_msg = new_messages.iter().any(|m| m.role == "assistant");

        let active_turn = if turn_finished && has_assistant_msg {
            tracker.finalize_completed_turn();
            None
        } else if turn_finished {
            let ts = tracker.turn_status();
            if ts == "completed" {
                tracker.snapshot_active_turn()
            } else {
                tracker.finalize_completed_turn();
                tracker.mark_persistence_clean();
                None
            }
        } else {
            tracker.snapshot_active_turn()
        };

        let (send_msgs, send_total) = if turn_finished && !has_assistant_msg {
            // Turn is finished but disk doesn't have the completed assistant
            // message yet — the frontend's immediateSaveDialogTurn hasn't
            // landed.  Don't send partial data; the snapshot overlay keeps the
            // user informed.  Next poll will re-read from disk.
            (None, None)
        } else {
            if !new_messages.is_empty() {
                tracker.mark_persistence_clean();
            }
            (Some(new_messages), Some(total_msg_count))
        };

        let sess_state = tracker.session_state();
        let title = tracker.title();

        RemoteResponse::SessionPoll {
            version: current_version,
            changed: true,
            session_state: Some(sess_state),
            title: if title.is_empty() { None } else { Some(title) },
            new_messages: send_msgs,
            total_msg_count: send_total,
            active_turn,
            model_catalog: Box::new(if should_send_model_catalog {
                current_model_catalog
            } else {
                None
            }),
        }
    }

    // ── ReadFile ────────────────────────────────────────────────────

    /// Read a workspace file and return its base64-encoded content.
    ///
    /// Relative paths are resolved against the session workspace when possible,
    /// otherwise the current workspace root. Rejects files larger than 30 MB.
    async fn handle_read_file(&self, raw_path: &str, session_id: Option<&str>) -> RemoteResponse {
        use crate::service::remote_connect::bot::{read_workspace_file, WorkspaceFileContent};

        const MAX_SIZE: u64 = 30 * 1024 * 1024; // Unified 30 MB cap (Feishu API hard limit)
        let workspace_root = resolve_file_workspace_root(session_id).await;
        match read_workspace_file(raw_path, MAX_SIZE, workspace_root.as_deref()).await {
            Ok(WorkspaceFileContent {
                name,
                bytes,
                mime_type,
                size,
            }) => {
                use base64::Engine as _;
                let content_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                RemoteResponse::FileContent {
                    name,
                    content_base64,
                    mime_type: mime_type.to_string(),
                    size,
                }
            }
            Err(e) => RemoteResponse::Error {
                message: e.to_string(),
            },
        }
    }

    async fn handle_read_file_chunk(
        &self,
        raw_path: &str,
        session_id: Option<&str>,
        offset: u64,
        limit: u64,
    ) -> RemoteResponse {
        use crate::service::remote_connect::bot::{detect_mime_type, resolve_workspace_path};

        let workspace_root = resolve_file_workspace_root(session_id).await;
        let abs = match resolve_workspace_path(raw_path, workspace_root.as_deref()) {
            Some(p) => p,
            None => {
                return RemoteResponse::Error {
                    message: format!("Remote file path could not be resolved: {raw_path}"),
                }
            }
        };
        if !abs.exists() || !abs.is_file() {
            return RemoteResponse::Error {
                message: format!("File not found or not a regular file: {}", abs.display()),
            };
        }

        let total_size = match tokio::fs::metadata(&abs).await {
            Ok(m) => m.len(),
            Err(e) => {
                return RemoteResponse::Error {
                    message: format!("Cannot read file metadata: {e}"),
                }
            }
        };

        // Must be divisible by 3 so each intermediate chunk's base64 has no
        // padding; the client joins chunk base64 strings and `atob()` requires
        // padding only at the very end.
        const MAX_CHUNK: u64 = 3 * 1024 * 1024; // 3 MB raw → 4 MB base64
        let actual_limit = limit.min(MAX_CHUNK);

        let bytes = match tokio::fs::read(&abs).await {
            Ok(b) => b,
            Err(e) => {
                return RemoteResponse::Error {
                    message: format!("Cannot read file: {e}"),
                }
            }
        };

        let start = (offset as usize).min(bytes.len());
        let end = (start + actual_limit as usize).min(bytes.len());
        let chunk = &bytes[start..end];

        use base64::Engine as _;
        let chunk_base64 = base64::engine::general_purpose::STANDARD.encode(chunk);

        let name = abs
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();

        RemoteResponse::FileChunk {
            name,
            chunk_base64,
            offset,
            chunk_size: (end - start) as u64,
            total_size,
            mime_type: detect_mime_type(&abs).to_string(),
        }
    }

    async fn handle_get_file_info(
        &self,
        raw_path: &str,
        session_id: Option<&str>,
    ) -> RemoteResponse {
        use crate::service::remote_connect::bot::{detect_mime_type, resolve_workspace_path};

        let workspace_root = resolve_file_workspace_root(session_id).await;
        let abs = match resolve_workspace_path(raw_path, workspace_root.as_deref()) {
            Some(p) => p,
            None => {
                return RemoteResponse::Error {
                    message: format!("Remote file path could not be resolved: {raw_path}"),
                }
            }
        };

        if !abs.exists() {
            return RemoteResponse::Error {
                message: format!("File not found: {}", abs.display()),
            };
        }
        if !abs.is_file() {
            return RemoteResponse::Error {
                message: format!("Path is not a regular file: {}", abs.display()),
            };
        }

        let size = match std::fs::metadata(&abs) {
            Ok(m) => m.len(),
            Err(e) => {
                return RemoteResponse::Error {
                    message: format!("Cannot read file metadata: {e}"),
                }
            }
        };

        let name = abs
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();

        RemoteResponse::FileInfo {
            name,
            size,
            mime_type: detect_mime_type(&abs).to_string(),
        }
    }

    // ── Workspace commands ──────────────────────────────────────────

    async fn handle_workspace_command(&self, cmd: &RemoteCommand) -> RemoteResponse {
        use crate::service::workspace::get_global_workspace_service;

        match cmd {
            RemoteCommand::GetWorkspaceInfo => {
                use crate::service::workspace::{get_global_workspace_service, WorkspaceKind};

                if let Some(ws_service) = get_global_workspace_service() {
                    if let Some(ws) = ws_service.get_current_workspace().await {
                        let p = ws.root_path.clone();
                        let branch = git2::Repository::open(&p).ok().and_then(|repo| {
                            repo.head()
                                .ok()
                                .and_then(|h| h.shorthand().map(String::from))
                        });
                        let kind_str = match ws.workspace_kind {
                            WorkspaceKind::Normal => "normal",
                            WorkspaceKind::Assistant => "assistant",
                            WorkspaceKind::Remote => "remote",
                        };
                        return RemoteResponse::WorkspaceInfo {
                            has_workspace: true,
                            path: Some(p.to_string_lossy().to_string()),
                            project_name: Some(ws.name.clone()),
                            git_branch: branch,
                            workspace_kind: Some(kind_str.to_string()),
                            assistant_id: ws.assistant_id.clone(),
                        };
                    }
                }
                RemoteResponse::WorkspaceInfo {
                    has_workspace: false,
                    path: None,
                    project_name: None,
                    git_branch: None,
                    workspace_kind: None,
                    assistant_id: None,
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
                    .map(|w| {
                        let kind_str = match w.workspace_kind {
                            crate::service::workspace::WorkspaceKind::Normal => "normal",
                            crate::service::workspace::WorkspaceKind::Assistant => "assistant",
                            crate::service::workspace::WorkspaceKind::Remote => "remote",
                        };
                        RecentWorkspaceEntry {
                            path: w.root_path.to_string_lossy().to_string(),
                            name: w.name.clone(),
                            last_opened: w.last_accessed.to_rfc3339(),
                            workspace_kind: Some(kind_str.to_string()),
                        }
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
                            crate::service::snapshot::initialize_snapshot_manager_for_workspace(
                                info.root_path.clone(),
                                None,
                            )
                            .await
                        {
                            error!("Failed to initialize snapshot after remote workspace set: {e}");
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
            RemoteCommand::ListAssistants => {
                let ws_service = match get_global_workspace_service() {
                    Some(s) => s,
                    None => {
                        return RemoteResponse::AssistantList { assistants: vec![] };
                    }
                };
                let assistants = ws_service.get_assistant_workspaces().await;
                let entries = assistants
                    .into_iter()
                    .map(|w| AssistantEntry {
                        path: w.root_path.to_string_lossy().to_string(),
                        name: w.name.clone(),
                        assistant_id: w.assistant_id.clone(),
                    })
                    .collect();
                RemoteResponse::AssistantList {
                    assistants: entries,
                }
            }
            RemoteCommand::SetAssistant { path } => {
                let ws_service = match get_global_workspace_service() {
                    Some(s) => s,
                    None => {
                        return RemoteResponse::AssistantUpdated {
                            success: false,
                            path: None,
                            name: None,
                            error: Some("Workspace service not available".into()),
                        };
                    }
                };
                let path_buf = std::path::PathBuf::from(path);
                match ws_service.open_workspace(path_buf).await {
                    Ok(info) => {
                        if let Err(e) =
                            crate::service::snapshot::initialize_snapshot_manager_for_workspace(
                                info.root_path.clone(),
                                None,
                            )
                            .await
                        {
                            error!("Failed to initialize snapshot after remote assistant set: {e}");
                        }
                        RemoteResponse::AssistantUpdated {
                            success: true,
                            path: Some(info.root_path.to_string_lossy().to_string()),
                            name: Some(info.name.clone()),
                            error: None,
                        }
                    }
                    Err(e) => RemoteResponse::AssistantUpdated {
                        success: false,
                        path: None,
                        name: None,
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
                use crate::agentic::persistence::PersistenceManager;
                use crate::infrastructure::PathManager;

                let page_size = limit.unwrap_or(30).min(100);
                let page_offset = offset.unwrap_or(0);

                let Some(workspace_path) = workspace_path
                    .as_deref()
                    .filter(|path| !path.is_empty())
                    .map(std::path::PathBuf::from)
                else {
                    return RemoteResponse::Error {
                        message: "workspace_path is required for ListSessions".to_string(),
                    };
                };

                let ws_str = workspace_path.to_string_lossy().to_string();
                let workspace_name = workspace_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string());

                if let Ok(pm) = PathManager::new() {
                    let pm = std::sync::Arc::new(pm);
                    match PersistenceManager::new(pm) {
                        Ok(store) => match store.list_session_metadata(&workspace_path).await {
                            Ok(all_meta) => {
                                let total = all_meta.len();
                                let has_more = page_offset + page_size < total;
                                let sessions: Vec<SessionInfo> = all_meta
                                    .into_iter()
                                    .skip(page_offset)
                                    .take(page_size)
                                    .map(|s| {
                                        let created = (s.created_at / 1000).to_string();
                                        let updated = (s.last_active_at / 1000).to_string();
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
                                RemoteResponse::SessionList { sessions, has_more }
                            }
                            Err(e) => {
                                debug!("Session list read failed for {ws_str}: {e}");
                                RemoteResponse::Error {
                                    message: format!("Failed to list sessions for workspace: {e}"),
                                }
                            }
                        },
                        Err(e) => {
                            debug!("PersistenceManager init failed for {ws_str}: {e}");
                            RemoteResponse::Error {
                                message: format!("Failed to initialize session storage: {e}"),
                            }
                        }
                    }
                } else {
                    RemoteResponse::Error {
                        message: "Failed to initialize path manager".to_string(),
                    }
                }
            }
            RemoteCommand::CreateSession {
                agent_type,
                session_name: custom_name,
                workspace_path: requested_ws_path,
            } => {
                let agent = resolve_agent_type(agent_type.as_deref());
                let is_claw = agent == "Claw";

                let session_name =
                    custom_name
                        .as_deref()
                        .filter(|n| !n.is_empty())
                        .unwrap_or(match agent {
                            "Cowork" => "Remote Cowork Session",
                            "Claw" => "Remote Claw Session",
                            _ => "Remote Code Session",
                        });

                let binding_ws_str = if is_claw {
                    // For Claw sessions, get or create default assistant workspace
                    use crate::service::workspace::get_global_workspace_service;

                    let ws_service = match get_global_workspace_service() {
                        Some(s) => s,
                        None => {
                            return RemoteResponse::Error {
                                message: "Workspace service not available".to_string(),
                            };
                        }
                    };

                    let workspaces = ws_service.get_assistant_workspaces().await;
                    if let Some(default_ws) =
                        workspaces.into_iter().find(|w| w.assistant_id.is_none())
                    {
                        Some(default_ws.root_path.to_string_lossy().to_string())
                    } else {
                        match ws_service.create_assistant_workspace(None).await {
                            Ok(ws_info) => Some(ws_info.root_path.to_string_lossy().to_string()),
                            Err(e) => {
                                return RemoteResponse::Error {
                                    message: format!("Failed to create assistant workspace: {}", e),
                                };
                            }
                        }
                    }
                } else {
                    // For Code/Cowork sessions, use provided workspace
                    requested_ws_path
                        .as_deref()
                        .filter(|path| !path.is_empty())
                        .map(ToOwned::to_owned)
                };

                debug!(
                    "Remote CreateSession: agent={}, requested_ws={:?}, binding_ws={:?}",
                    agent, requested_ws_path, binding_ws_str
                );

                let Some(binding_ws_str) = binding_ws_str else {
                    return RemoteResponse::Error {
                        message: if is_claw {
                            "Failed to get or create assistant workspace".to_string()
                        } else {
                            "workspace_path is required for CreateSession".to_string()
                        },
                    };
                };

                match coordinator
                    .create_session_with_workspace(
                        None,
                        session_name.to_string(),
                        agent.to_string(),
                        SessionConfig {
                            workspace_path: Some(binding_ws_str.clone()),
                            ..Default::default()
                        },
                        binding_ws_str,
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
            RemoteCommand::GetModelCatalog { session_id } => {
                match load_remote_model_catalog(session_id.as_deref()).await {
                    Ok(catalog) => RemoteResponse::ModelCatalog { catalog },
                    Err(message) => RemoteResponse::Error { message },
                }
            }
            RemoteCommand::SetSessionModel {
                session_id,
                model_id,
            } => {
                use crate::service::config::{get_global_config_service, types::AIConfig};

                let requested_model_id = model_id.trim();
                if requested_model_id.is_empty() {
                    return RemoteResponse::Error {
                        message: "model_id is required".to_string(),
                    };
                }

                let normalized_model_id =
                    if matches!(requested_model_id, "auto" | "default" | "primary" | "fast") {
                        if requested_model_id == "default" {
                            "auto".to_string()
                        } else {
                            requested_model_id.to_string()
                        }
                    } else {
                        let Ok(config_service) = get_global_config_service().await else {
                            return RemoteResponse::Error {
                                message: "Config service not available".to_string(),
                            };
                        };
                        let ai_config: AIConfig = match config_service.get_config(Some("ai")).await
                        {
                            Ok(config) => config,
                            Err(e) => {
                                return RemoteResponse::Error {
                                    message: format!("Failed to load AI config: {e}"),
                                }
                            }
                        };
                        match ai_config.resolve_model_reference(requested_model_id) {
                            Some(resolved) => resolved,
                            None => {
                                return RemoteResponse::Error {
                                    message: format!(
                                        "Unknown model selection: {requested_model_id}"
                                    ),
                                }
                            }
                        }
                    };

                if coordinator
                    .get_session_manager()
                    .get_session(session_id)
                    .is_none()
                {
                    let Some(workspace_path) = resolve_session_workspace_path(session_id).await
                    else {
                        return RemoteResponse::Error {
                            message: format!(
                                "Workspace path not available for session: {}",
                                session_id
                            ),
                        };
                    };
                    if let Err(e) = coordinator
                        .restore_session(&workspace_path, session_id)
                        .await
                    {
                        return RemoteResponse::Error {
                            message: format!("Failed to restore session: {e}"),
                        };
                    }
                }

                match coordinator
                    .get_session_manager()
                    .update_session_model_id(session_id, &normalized_model_id)
                    .await
                {
                    Ok(()) => RemoteResponse::SessionModelUpdated {
                        session_id: session_id.clone(),
                        model_id: normalized_model_id,
                    },
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
                let Some(workspace_path) = resolve_session_workspace_path(session_id).await else {
                    return RemoteResponse::Error {
                        message: format!(
                            "Workspace path not available for session: {}",
                            session_id
                        ),
                    };
                };
                let (chat_msgs, has_more) =
                    load_chat_messages_from_conversation_persistence(&workspace_path, session_id)
                        .await;
                RemoteResponse::Messages {
                    session_id: session_id.clone(),
                    messages: chat_msgs,
                    has_more,
                }
            }
            RemoteCommand::DeleteSession { session_id } => {
                let Some(workspace_path) = resolve_session_workspace_path(session_id).await else {
                    return RemoteResponse::Error {
                        message: format!(
                            "Workspace path not available for session: {}",
                            session_id
                        ),
                    };
                };

                match coordinator
                    .delete_session(&workspace_path, session_id)
                    .await
                {
                    Ok(_) => {
                        get_or_init_global_dispatcher().remove_tracker(session_id);
                        RemoteResponse::SessionDeleted {
                            session_id: session_id.clone(),
                        }
                    }
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
        use crate::agentic::coordination::{
            get_global_coordinator, DialogSubmissionPolicy, DialogTriggerSource,
        };

        let dispatcher = get_or_init_global_dispatcher();

        match cmd {
            RemoteCommand::SendMessage {
                session_id,
                content,
                agent_type: requested_agent_type,
                images,
                image_contexts,
            } => {
                // Unified: prefer image_contexts (new format), fall back to legacy images
                let resolved_contexts = image_contexts
                    .clone()
                    .unwrap_or_else(|| images_to_contexts(images.as_ref()));
                info!(
                    "Remote send_message: session={session_id}, agent_type={}, image_contexts={}",
                    requested_agent_type.as_deref().unwrap_or("agentic"),
                    resolved_contexts.len()
                );
                match dispatcher
                    .send_message(
                        session_id,
                        content.clone(),
                        requested_agent_type.as_deref(),
                        resolved_contexts,
                        DialogSubmissionPolicy::for_source(DialogTriggerSource::RemoteRelay),
                        None,
                    )
                    .await
                {
                    Ok(outcome) => {
                        let (sid, turn_id) = match outcome {
                            crate::agentic::coordination::DialogSubmitOutcome::Started {
                                session_id,
                                turn_id,
                            }
                            | crate::agentic::coordination::DialogSubmitOutcome::Queued {
                                session_id,
                                turn_id,
                            } => (session_id, turn_id),
                        };
                        RemoteResponse::MessageSent {
                            session_id: sid,
                            turn_id,
                        }
                    }
                    Err(e) => RemoteResponse::Error { message: e },
                }
            }
            RemoteCommand::CancelTask {
                session_id,
                turn_id,
            } => match dispatcher.cancel_task(session_id, turn_id.as_deref()).await {
                Ok(()) => RemoteResponse::TaskCancelled {
                    session_id: session_id.clone(),
                },
                Err(e) => RemoteResponse::Error { message: e },
            },
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
                match coordinator
                    .confirm_tool(tool_id, updated_input.clone())
                    .await
                {
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
