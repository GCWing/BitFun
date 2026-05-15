//! Session bridge: translates remote commands into local session operations.
//!
//! Mobile clients send encrypted commands via the relay (HTTP → WS bridge).
//! The desktop decrypts, dispatches, and returns encrypted responses.
//!
//! Instead of streaming events to the mobile, the desktop maintains an
//! in-memory `RemoteSessionStateTracker` per session. The mobile polls
//! for state changes using the `PollSession` command, receiving only
//! incremental updates (new messages + current active turn snapshot).

use anyhow::{Result, anyhow};
use dashmap::DashMap;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, OnceLock};

use super::encryption;
pub use bitfun_services_integrations::remote_connect::{
    ActiveTurnSnapshot, AssistantEntry, ChatImageAttachment, ChatMessage, ChatMessageItem,
    ImageAttachment, RecentWorkspaceEntry, RemoteSessionStateTracker, RemoteToolStatus,
    SessionInfo, TrackerEvent,
};

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

pub type EncryptedPayload = (String, String);

/// Compress a base64 data-URL image to a small thumbnail for mobile display.
/// Falls back to the original if decoding/compression fails or the image is
/// already within `max_bytes`.
fn compress_data_url_for_mobile(data_url: &str, max_bytes: usize) -> String {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64;
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
                    input_preview:
                        bitfun_services_integrations::remote_connect::make_slim_tool_params(
                            &t.tool_call.input,
                        ),
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
    bitfun_services_integrations::remote_connect::resolve_remote_agent_type(mobile_type)
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

fn resolve_remote_execution_image_contexts(
    legacy_images: Option<&Vec<ImageAttachment>>,
    image_contexts: Option<Vec<crate::agentic::image_analysis::ImageContextData>>,
) -> Vec<crate::agentic::image_analysis::ImageContextData> {
    image_contexts.unwrap_or_else(|| images_to_contexts(legacy_images))
}

fn remote_session_restore_target<'a>(
    session_exists: bool,
    binding_workspace: Option<&'a str>,
) -> Option<&'a str> {
    if session_exists {
        None
    } else {
        binding_workspace
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RemoteCancelDecision {
    CancelCurrent(String),
    StaleRequestedTurn,
    AlreadyFinished,
    NoRunningTask,
}

fn resolve_remote_cancel_decision(
    running_turn_id: Option<&str>,
    requested_turn_id: Option<&str>,
) -> RemoteCancelDecision {
    match (running_turn_id, requested_turn_id) {
        (Some(current_turn_id), Some(req_id)) if req_id != current_turn_id => {
            RemoteCancelDecision::StaleRequestedTurn
        }
        (Some(current_turn_id), _) => {
            RemoteCancelDecision::CancelCurrent(current_turn_id.to_string())
        }
        (None, Some(_)) => RemoteCancelDecision::AlreadyFinished,
        (None, None) => RemoteCancelDecision::NoRunningTask,
    }
}

// ── RemoteSessionStateTracker subscriber adapter ─────────────────

#[async_trait::async_trait]
impl crate::agentic::events::EventSubscriber for Arc<RemoteSessionStateTracker> {
    async fn on_event(
        &self,
        event: &crate::agentic::events::AgenticEvent,
    ) -> crate::util::errors::BitFunResult<()> {
        self.handle_agentic_event(event);
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

        if let Some(workspace_path) = remote_session_restore_target(
            session_mgr.get_session(session_id).is_some(),
            binding_workspace.as_deref(),
        ) {
            let _ = coordinator
                .restore_session(std::path::Path::new(workspace_path), session_id)
                .await
                .ok();
        }

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

        match resolve_remote_cancel_decision(running_turn_id.as_deref(), requested_turn_id) {
            RemoteCancelDecision::StaleRequestedTurn => {
                Err("This task is no longer running.".to_string())
            }
            RemoteCancelDecision::CancelCurrent(current_turn_id) => coordinator
                .cancel_dialog_turn(session_id, &current_turn_id)
                .await
                .map_err(|e| e.to_string()),
            RemoteCancelDecision::AlreadyFinished => {
                Err("This task is already finished.".to_string())
            }
            RemoteCancelDecision::NoRunningTask => Err(format!(
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
        use crate::service::workspace::{WorkspaceKind, get_global_workspace_service};

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
        use crate::service::remote_connect::bot::{WorkspaceFileContent, read_workspace_file};

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
                };
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
                };
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
                };
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
                };
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
                };
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
                use crate::service::workspace::{WorkspaceKind, get_global_workspace_service};

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
        use crate::agentic::coordination::get_global_coordinator;
        use bitfun_runtime_ports::AgentSubmissionPort;
        use bitfun_services_integrations::remote_connect::{
            RemoteConnectSubmissionSource, build_remote_session_create_request,
        };

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

                let request = build_remote_session_create_request(
                    session_name,
                    agent,
                    Some(binding_ws_str),
                    RemoteConnectSubmissionSource::Relay,
                );
                let submission_port: &dyn AgentSubmissionPort = coordinator.as_ref();
                match submission_port.create_session(request).await {
                    Ok(session) => RemoteResponse::SessionCreated {
                        session_id: session.session_id,
                    },
                    Err(e) => RemoteResponse::Error { message: e.message },
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
                                };
                            }
                        };
                        match ai_config.resolve_model_reference(requested_model_id) {
                            Some(resolved) => resolved,
                            None => {
                                return RemoteResponse::Error {
                                    message: format!(
                                        "Unknown model selection: {requested_model_id}"
                                    ),
                                };
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
            DialogSubmissionPolicy, DialogTriggerSource, get_global_coordinator,
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
                let resolved_contexts = resolve_remote_execution_image_contexts(
                    images.as_ref(),
                    image_contexts.clone(),
                );
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

    #[test]
    fn remote_execution_prefers_unified_image_contexts_over_legacy_images() {
        let explicit_context = crate::agentic::image_analysis::ImageContextData {
            id: "ctx-1".to_string(),
            image_path: Some("D:/workspace/project/screenshot.png".to_string()),
            data_url: None,
            mime_type: "image/png".to_string(),
            metadata: Some(serde_json::json!({ "source": "desktop" })),
        };
        let legacy_images = vec![ImageAttachment {
            name: "legacy.png".to_string(),
            data_url: "data:image/png;base64,legacy".to_string(),
        }];

        let resolved = resolve_remote_execution_image_contexts(
            Some(&legacy_images),
            Some(vec![explicit_context.clone()]),
        );

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, explicit_context.id);
        assert_eq!(resolved[0].image_path, explicit_context.image_path);
        assert!(resolved[0].data_url.is_none());
    }

    #[test]
    fn remote_execution_falls_back_to_legacy_images_as_image_contexts() {
        let legacy_images = vec![ImageAttachment {
            name: "clip.png".to_string(),
            data_url: "data:image/png;base64,abc".to_string(),
        }];

        let resolved = resolve_remote_execution_image_contexts(Some(&legacy_images), None);

        assert_eq!(resolved.len(), 1);
        assert!(resolved[0].id.starts_with("remote_img_"));
        assert_eq!(
            resolved[0].data_url.as_deref(),
            Some("data:image/png;base64,abc")
        );
        assert_eq!(resolved[0].mime_type, "image/png");
        assert_eq!(resolved[0].metadata.as_ref().unwrap()["name"], "clip.png");
    }

    #[test]
    fn remote_cancel_decision_preserves_current_turn_boundaries() {
        assert_eq!(
            resolve_remote_cancel_decision(Some("turn-current"), Some("turn-current")),
            RemoteCancelDecision::CancelCurrent("turn-current".to_string())
        );
        assert_eq!(
            resolve_remote_cancel_decision(Some("turn-current"), None),
            RemoteCancelDecision::CancelCurrent("turn-current".to_string())
        );
        assert_eq!(
            resolve_remote_cancel_decision(Some("turn-current"), Some("turn-stale")),
            RemoteCancelDecision::StaleRequestedTurn
        );
        assert_eq!(
            resolve_remote_cancel_decision(None, Some("turn-finished")),
            RemoteCancelDecision::AlreadyFinished
        );
        assert_eq!(
            resolve_remote_cancel_decision(None, None),
            RemoteCancelDecision::NoRunningTask
        );
    }

    #[test]
    fn remote_restore_target_only_restores_cold_sessions_with_workspace_binding() {
        assert_eq!(
            remote_session_restore_target(false, Some("D:/workspace/project")),
            Some("D:/workspace/project")
        );
        assert_eq!(
            remote_session_restore_target(true, Some("D:/workspace/project")),
            None
        );
        assert_eq!(remote_session_restore_target(false, None), None);
    }

    #[test]
    fn remote_command_snapshot_covers_execution_poll_and_cancel_surfaces() {
        let command = RemoteCommand::SendMessage {
            session_id: "session-1".to_string(),
            content: "hello".to_string(),
            agent_type: Some("code".to_string()),
            images: Some(vec![ImageAttachment {
                name: "clip.png".to_string(),
                data_url: "data:image/png;base64,abc".to_string(),
            }]),
            image_contexts: None,
        };
        let json = serde_json::to_value(command).expect("serialize send command");
        assert_eq!(json["cmd"], "send_message");
        assert_eq!(json["session_id"], "session-1");
        assert_eq!(json["agent_type"], "code");
        assert_eq!(json["images"][0]["name"], "clip.png");
        assert!(json["image_contexts"].is_null());
        assert!(json.get("imageContexts").is_none());

        let cancel = serde_json::to_value(RemoteCommand::CancelTask {
            session_id: "session-1".to_string(),
            turn_id: Some("turn-1".to_string()),
        })
        .expect("serialize cancel command");
        assert_eq!(cancel["cmd"], "cancel_task");
        assert_eq!(cancel["turn_id"], "turn-1");

        let poll = serde_json::to_value(RemoteCommand::PollSession {
            session_id: "session-1".to_string(),
            since_version: 7,
            known_msg_count: 3,
            known_model_catalog_version: Some(11),
        })
        .expect("serialize poll command");
        assert_eq!(poll["cmd"], "poll_session");
        assert_eq!(poll["since_version"], 7);
        assert_eq!(poll["known_msg_count"], 3);
        assert_eq!(poll["known_model_catalog_version"], 11);
    }

    #[test]
    fn remote_response_snapshot_preserves_active_turn_and_result_shapes() {
        let active_turn = ActiveTurnSnapshot {
            turn_id: "turn-1".to_string(),
            status: "active".to_string(),
            text: String::new(),
            thinking: String::new(),
            tools: vec![RemoteToolStatus {
                id: "tool-1".to_string(),
                name: "Read".to_string(),
                status: "running".to_string(),
                duration_ms: None,
                start_ms: Some(42),
                input_preview: Some("{\"path\":\"README.md\"}".to_string()),
                tool_input: None,
            }],
            round_index: 2,
            items: Some(vec![ChatMessageItem {
                item_type: "tool".to_string(),
                content: None,
                tool: None,
                is_subagent: None,
            }]),
        };

        let poll = serde_json::to_value(RemoteResponse::SessionPoll {
            version: 8,
            changed: true,
            session_state: Some("running".to_string()),
            title: Some("session title".to_string()),
            new_messages: None,
            total_msg_count: None,
            active_turn: Some(active_turn),
            model_catalog: Box::new(None),
        })
        .expect("serialize poll response");

        assert_eq!(poll["resp"], "session_poll");
        assert_eq!(poll["version"], 8);
        assert_eq!(poll["active_turn"]["turn_id"], "turn-1");
        assert_eq!(
            poll["active_turn"]["tools"][0]["input_preview"],
            "{\"path\":\"README.md\"}"
        );
        assert!(poll.get("new_messages").is_none());

        let sent = serde_json::to_value(RemoteResponse::MessageSent {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
        })
        .expect("serialize sent response");
        assert_eq!(sent["resp"], "message_sent");
        assert_eq!(sent["turn_id"], "turn-1");

        let cancelled = serde_json::to_value(RemoteResponse::TaskCancelled {
            session_id: "session-1".to_string(),
        })
        .expect("serialize cancelled response");
        assert_eq!(cancelled["resp"], "task_cancelled");
        assert_eq!(cancelled["session_id"], "session-1");
    }
}
