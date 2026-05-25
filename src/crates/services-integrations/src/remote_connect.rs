//! Remote-connect integration contracts.
//!
//! This module owns stable remote-facing DTOs, runtime-port request
//! construction, and remote session tracker state. Network lifecycle and
//! product assembly stay in `bitfun-core` until their ports are explicit.

use bitfun_events::AgenticEvent;
use bitfun_runtime_ports::{
    AgentInputAttachment, AgentSessionCreateRequest, AgentSubmissionRequest, AgentSubmissionSource,
    RemoteControlStateSnapshot,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteConnectSubmissionSource {
    Relay,
    Bot,
}

impl RemoteConnectSubmissionSource {
    pub const fn agent_submission_source(self) -> AgentSubmissionSource {
        match self {
            RemoteConnectSubmissionSource::Relay => AgentSubmissionSource::RemoteRelay,
            RemoteConnectSubmissionSource::Bot => AgentSubmissionSource::Bot,
        }
    }

    pub const fn metadata_source(self) -> &'static str {
        match self {
            RemoteConnectSubmissionSource::Relay => "remote_relay",
            RemoteConnectSubmissionSource::Bot => "bot",
        }
    }
}

pub fn build_remote_session_create_request(
    session_name: impl Into<String>,
    agent_type: impl Into<String>,
    workspace_path: Option<impl Into<String>>,
    source: RemoteConnectSubmissionSource,
) -> AgentSessionCreateRequest {
    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "source".to_string(),
        serde_json::Value::String(source.metadata_source().to_string()),
    );

    AgentSessionCreateRequest {
        session_name: session_name.into(),
        agent_type: agent_type.into(),
        workspace_path: workspace_path.map(Into::into),
        metadata,
    }
}

pub fn build_remote_submission_request(
    session_id: impl Into<String>,
    message: impl Into<String>,
    turn_id: Option<String>,
    source: RemoteConnectSubmissionSource,
) -> AgentSubmissionRequest {
    AgentSubmissionRequest {
        session_id: session_id.into(),
        message: message.into(),
        turn_id,
        source: Some(source.agent_submission_source()),
        attachments: Vec::new(),
        metadata: serde_json::Map::new(),
    }
}

pub fn build_remote_image_attachment(
    index: usize,
    attachment: &ImageAttachment,
) -> AgentInputAttachment {
    AgentInputAttachment::remote_image(
        format!("remote-image-{}", index + 1),
        attachment.name.clone(),
        attachment.data_url.clone(),
    )
}

pub fn build_remote_image_submission_request(
    session_id: impl Into<String>,
    message: impl Into<String>,
    turn_id: Option<String>,
    source: RemoteConnectSubmissionSource,
    images: &[ImageAttachment],
) -> AgentSubmissionRequest {
    AgentSubmissionRequest {
        session_id: session_id.into(),
        message: message.into(),
        turn_id,
        source: Some(source.agent_submission_source()),
        attachments: images
            .iter()
            .enumerate()
            .map(|(index, image)| build_remote_image_attachment(index, image))
            .collect(),
        metadata: serde_json::Map::new(),
    }
}

/// Portable image context produced from legacy remote image payloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteImageContext {
    pub id: String,
    pub image_path: Option<String>,
    pub data_url: Option<String>,
    pub mime_type: String,
    pub metadata: Option<serde_json::Value>,
}

pub trait RemoteImageContextAdapter {
    fn from_remote_image_context(context: RemoteImageContext) -> Self;
}

pub fn build_remote_image_contexts(images: Option<&[ImageAttachment]>) -> Vec<RemoteImageContext> {
    let Some(images) = images.filter(|images| !images.is_empty()) else {
        return Vec::new();
    };

    images
        .iter()
        .map(|image| {
            let mime_type = image
                .data_url
                .split_once(',')
                .and_then(|(header, _)| {
                    header
                        .strip_prefix("data:")
                        .and_then(|rest| rest.split(';').next())
                })
                .unwrap_or("image/png")
                .to_string();

            RemoteImageContext {
                id: format!("remote_img_{}", uuid::Uuid::new_v4()),
                image_path: None,
                data_url: Some(image.data_url.clone()),
                mime_type,
                metadata: Some(serde_json::json!({
                    "name": image.name,
                    "source": "remote"
                })),
            }
        })
        .collect()
}

pub fn resolve_remote_execution_image_contexts<T>(
    legacy_images: Option<&[ImageAttachment]>,
    image_contexts: Option<Vec<T>>,
    legacy_contexts: impl FnOnce(Option<&[ImageAttachment]>) -> Vec<T>,
) -> Vec<T> {
    image_contexts.unwrap_or_else(|| legacy_contexts(legacy_images))
}

pub fn remote_session_restore_target<'a>(
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
pub enum RemoteCancelDecision {
    CancelCurrent(String),
    StaleRequestedTurn,
    AlreadyFinished,
    NoRunningTask,
}

pub fn resolve_remote_cancel_decision(
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCancelTaskRequest {
    pub session_id: String,
    pub requested_turn_id: Option<String>,
}

#[async_trait::async_trait]
pub trait RemoteCancelRuntimeHost: Send + Sync {
    async fn resolve_restore_workspace(&self, session_id: &str) -> Option<String>;

    async fn remote_control_state(
        &self,
        session_id: &str,
    ) -> Result<Option<RemoteControlStateSnapshot>, String>;

    async fn restore_remote_session(
        &self,
        session_id: &str,
        workspace_path: &str,
    ) -> Result<(), String>;

    async fn cancel_remote_turn(&self, session_id: &str, turn_id: &str) -> Result<(), String>;
}

pub async fn cancel_remote_task<H>(host: &H, request: RemoteCancelTaskRequest) -> Result<(), String>
where
    H: RemoteCancelRuntimeHost + ?Sized,
{
    let RemoteCancelTaskRequest {
        session_id,
        requested_turn_id,
    } = request;

    let mut state = host.remote_control_state(&session_id).await?;
    if state.is_none() {
        let workspace_path = host
            .resolve_restore_workspace(&session_id)
            .await
            .ok_or_else(|| format!("Workspace path not available for session: {}", session_id))?;
        host.restore_remote_session(&session_id, &workspace_path)
            .await
            .map_err(|error| format!("Session not found: {error}"))?;
        state = host.remote_control_state(&session_id).await?;
    }

    let running_turn_id = state.and_then(|state| state.active_turn_id);
    match resolve_remote_cancel_decision(running_turn_id.as_deref(), requested_turn_id.as_deref()) {
        RemoteCancelDecision::StaleRequestedTurn => {
            Err("This task is no longer running.".to_string())
        }
        RemoteCancelDecision::CancelCurrent(current_turn_id) => {
            host.cancel_remote_turn(&session_id, &current_turn_id).await
        }
        RemoteCancelDecision::AlreadyFinished => Err("This task is already finished.".to_string()),
        RemoteCancelDecision::NoRunningTask => Err(format!(
            "No running task to cancel for session: {}",
            session_id
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteDialogQueuePriority {
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteDialogSubmissionPolicy {
    pub source: RemoteConnectSubmissionSource,
    pub queue_priority: RemoteDialogQueuePriority,
    pub skip_tool_confirmation: bool,
}

impl RemoteDialogSubmissionPolicy {
    pub const fn for_source(source: RemoteConnectSubmissionSource) -> Self {
        Self {
            source,
            queue_priority: RemoteDialogQueuePriority::Normal,
            skip_tool_confirmation: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteDialogSubmissionRequest<ImageContext> {
    pub session_id: String,
    pub content: String,
    pub agent_type: Option<String>,
    pub image_contexts: Vec<ImageContext>,
    pub policy: RemoteDialogSubmissionPolicy,
    pub turn_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTerminalPrewarmRequest {
    pub session_id: String,
    pub binding_workspace: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteDialogResolvedSubmission<ImageContext> {
    pub session_id: String,
    pub content: String,
    pub resolved_agent_type: String,
    pub binding_workspace: Option<String>,
    pub image_contexts: Vec<ImageContext>,
    pub policy: RemoteDialogSubmissionPolicy,
    pub turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteDialogSubmitOutcome {
    Started { session_id: String, turn_id: String },
    Queued { session_id: String, turn_id: String },
}

/// Host callbacks required by remote-connect dialog execution.
///
/// The owner crate keeps the remote dialog orchestration order stable, while
/// concrete session restore, terminal warmup, and scheduler execution stay in
/// the product runtime adapter.
#[async_trait::async_trait]
pub trait RemoteDialogRuntimeHost: Send + Sync {
    type ImageContext: Send + Sync + 'static;

    fn ensure_tracker(&self, session_id: &str);

    async fn resolve_binding_workspace(&self, session_id: &str) -> Option<String>;

    async fn remote_session_exists(&self, session_id: &str) -> Result<bool, String>;

    async fn restore_remote_session(
        &self,
        session_id: &str,
        workspace_path: &str,
    ) -> Result<(), String>;

    fn prewarm_remote_terminal(&self, request: RemoteTerminalPrewarmRequest);

    fn generate_turn_id(&self) -> String;

    async fn submit_dialog(
        &self,
        submission: RemoteDialogResolvedSubmission<Self::ImageContext>,
    ) -> Result<RemoteDialogSubmitOutcome, String>;
}

pub async fn submit_remote_dialog<H>(
    host: &H,
    request: RemoteDialogSubmissionRequest<H::ImageContext>,
) -> Result<RemoteDialogSubmitOutcome, String>
where
    H: RemoteDialogRuntimeHost + ?Sized,
{
    let RemoteDialogSubmissionRequest {
        session_id,
        content,
        agent_type,
        image_contexts,
        policy,
        turn_id,
    } = request;

    host.ensure_tracker(&session_id);

    let binding_workspace = host.resolve_binding_workspace(&session_id).await;
    let session_exists = host.remote_session_exists(&session_id).await?;

    if let Some(workspace_path) =
        remote_session_restore_target(session_exists, binding_workspace.as_deref())
    {
        let _ = host
            .restore_remote_session(&session_id, workspace_path)
            .await;
    }

    host.prewarm_remote_terminal(RemoteTerminalPrewarmRequest {
        session_id: session_id.clone(),
        binding_workspace: binding_workspace.clone(),
    });

    let resolved_agent_type = resolve_remote_agent_type(agent_type.as_deref()).to_string();
    let turn_id = turn_id.unwrap_or_else(|| host.generate_turn_id());

    host.submit_dialog(RemoteDialogResolvedSubmission {
        session_id,
        content,
        resolved_agent_type,
        binding_workspace,
        image_contexts,
        policy,
        turn_id,
    })
    .await
}

pub const REMOTE_FILE_MAX_READ_BYTES: u64 = 30 * 1024 * 1024;
pub const REMOTE_FILE_MAX_CHUNK_BYTES: u64 = 3 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWorkspaceFileContent {
    pub name: String,
    pub bytes: Vec<u8>,
    pub mime_type: &'static str,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWorkspaceFileChunk {
    pub name: String,
    pub bytes: Vec<u8>,
    pub offset: u64,
    pub chunk_size: u64,
    pub total_size: u64,
    pub mime_type: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWorkspaceFileInfo {
    pub name: String,
    pub size: u64,
    pub mime_type: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteFileChunkRange {
    pub start: usize,
    pub end: usize,
    pub chunk_size: u64,
}

pub fn resolve_remote_file_chunk_range(
    file_len: usize,
    offset: u64,
    limit: u64,
) -> RemoteFileChunkRange {
    let actual_limit = limit.min(REMOTE_FILE_MAX_CHUNK_BYTES);
    let start = (offset as usize).min(file_len);
    let end = start.saturating_add(actual_limit as usize).min(file_len);

    RemoteFileChunkRange {
        start,
        end,
        chunk_size: (end - start) as u64,
    }
}

pub fn remote_file_display_name(name: Option<&str>) -> String {
    match name {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => "file".to_string(),
    }
}

fn strip_remote_workspace_path_prefix(raw: &str) -> &str {
    raw.strip_prefix("computer://")
        .or_else(|| raw.strip_prefix("file://"))
        .unwrap_or(raw)
}

fn is_remote_absolute_workspace_path(path: &str) -> bool {
    path.starts_with('/') || (path.len() >= 3 && path.as_bytes()[1] == b':')
}

pub fn resolve_remote_workspace_path(raw: &str, workspace_root: Option<&Path>) -> Option<PathBuf> {
    let stripped = strip_remote_workspace_path_prefix(raw);

    if is_remote_absolute_workspace_path(stripped) {
        return Some(PathBuf::from(stripped));
    }

    let workspace_root = workspace_root?;
    let canonical_root = std::fs::canonicalize(workspace_root).ok()?;
    let candidate = canonical_root.join(stripped);
    let canonical_candidate = std::fs::canonicalize(candidate).ok()?;

    if canonical_candidate.starts_with(&canonical_root) {
        Some(canonical_candidate)
    } else {
        None
    }
}

pub fn detect_remote_mime_type(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "log" => "text/plain",
        "md" => "text/markdown",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "ts" | "tsx" | "jsx" | "rs" | "py" | "go" | "java" | "c" | "cpp" | "h" | "sh" | "toml"
        | "yaml" | "yml" => "text/plain",
        "json" => "application/json",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "zip" => "application/zip",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "mp4" => "video/mp4",
        "opus" => "audio/opus",
        _ => "application/octet-stream",
    }
}

pub async fn read_remote_workspace_file(
    raw_path: &str,
    max_size: u64,
    workspace_root: Option<&Path>,
) -> Result<RemoteWorkspaceFileContent, String> {
    let abs_path = resolve_remote_workspace_path(raw_path, workspace_root)
        .ok_or_else(|| format!("Remote file path could not be resolved: {raw_path}"))?;

    if !abs_path.exists() {
        return Err(format!("File not found: {}", abs_path.display()));
    }
    if !abs_path.is_file() {
        return Err(format!(
            "Path is not a regular file: {}",
            abs_path.display()
        ));
    }

    let metadata = tokio::fs::metadata(&abs_path)
        .await
        .map_err(|e| format!("Cannot read file metadata for {}: {e}", abs_path.display()))?;

    if metadata.len() > max_size {
        return Err(format!(
            "File too large ({} bytes, limit {max_size} bytes): {}",
            metadata.len(),
            abs_path.display()
        ));
    }

    let bytes = tokio::fs::read(&abs_path)
        .await
        .map_err(|e| format!("Cannot read file {}: {e}", abs_path.display()))?;

    Ok(RemoteWorkspaceFileContent {
        name: remote_file_display_name(abs_path.file_name().and_then(|n| n.to_str())),
        bytes,
        mime_type: detect_remote_mime_type(&abs_path),
        size: metadata.len(),
    })
}

pub async fn read_remote_workspace_file_chunk(
    raw_path: &str,
    workspace_root: Option<&Path>,
    offset: u64,
    limit: u64,
) -> Result<RemoteWorkspaceFileChunk, String> {
    let abs_path = resolve_remote_workspace_path(raw_path, workspace_root)
        .ok_or_else(|| format!("Remote file path could not be resolved: {raw_path}"))?;

    if !abs_path.exists() || !abs_path.is_file() {
        return Err(format!(
            "File not found or not a regular file: {}",
            abs_path.display()
        ));
    }

    let total_size = tokio::fs::metadata(&abs_path)
        .await
        .map_err(|e| format!("Cannot read file metadata: {e}"))?
        .len();

    let bytes = tokio::fs::read(&abs_path)
        .await
        .map_err(|e| format!("Cannot read file: {e}"))?;
    let range = resolve_remote_file_chunk_range(bytes.len(), offset, limit);
    let chunk = bytes[range.start..range.end].to_vec();

    Ok(RemoteWorkspaceFileChunk {
        name: remote_file_display_name(abs_path.file_name().and_then(|n| n.to_str())),
        bytes: chunk,
        offset,
        chunk_size: range.chunk_size,
        total_size,
        mime_type: detect_remote_mime_type(&abs_path),
    })
}

pub async fn read_remote_workspace_file_info(
    raw_path: &str,
    workspace_root: Option<&Path>,
) -> Result<RemoteWorkspaceFileInfo, String> {
    let abs_path = resolve_remote_workspace_path(raw_path, workspace_root)
        .ok_or_else(|| format!("Remote file path could not be resolved: {raw_path}"))?;

    if !abs_path.exists() {
        return Err(format!("File not found: {}", abs_path.display()));
    }
    if !abs_path.is_file() {
        return Err(format!(
            "Path is not a regular file: {}",
            abs_path.display()
        ));
    }

    let size = tokio::fs::metadata(&abs_path)
        .await
        .map_err(|e| format!("Cannot read file metadata: {e}"))?
        .len();

    Ok(RemoteWorkspaceFileInfo {
        name: remote_file_display_name(abs_path.file_name().and_then(|n| n.to_str())),
        size,
        mime_type: detect_remote_mime_type(&abs_path),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RemoteDefaultModelsConfig {
    pub primary: Option<String>,
    pub fast: Option<String>,
    pub search: Option<String>,
    pub image_understanding: Option<String>,
    pub image_generation: Option<String>,
    pub speech_recognition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteModelCatalog {
    pub version: u64,
    pub models: Vec<RemoteModelConfig>,
    pub default_models: RemoteDefaultModelsConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_model_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteModelCatalogPollDelta {
    pub changed: bool,
    pub catalog: Option<RemoteModelCatalog>,
}

pub fn resolve_remote_agent_type(mobile_type: Option<&str>) -> &'static str {
    match mobile_type {
        Some("code") | Some("agentic") | Some("Agentic") => "agentic",
        Some("multitask") | Some("Multitask") => "Multitask",
        Some("cowork") | Some("Cowork") => "Cowork",
        Some("plan") | Some("Plan") => "Plan",
        Some("debug") | Some("Debug") => "debug",
        _ => "agentic",
    }
}

/// Image sent from a remote client as a base64 data URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageAttachment {
    pub name: String,
    pub data_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatImageAttachment {
    pub name: String,
    pub data_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<ChatMessageItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<ChatImageAttachment>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentWorkspaceEntry {
    pub path: String,
    pub name: String,
    pub last_opened: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistantEntry {
    pub path: String,
    pub name: String,
    pub assistant_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<serde_json::Value>,
}

/// Commands that remote clients can send to the desktop runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        image_contexts: Option<Vec<RemoteImageContext>>,
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
    AnswerQuestion {
        tool_id: String,
        answers: serde_json::Value,
    },
    PollSession {
        session_id: String,
        since_version: u64,
        known_msg_count: usize,
        known_model_catalog_version: Option<u64>,
    },
    ReadFile {
        path: String,
        session_id: Option<String>,
    },
    ReadFileChunk {
        path: String,
        session_id: Option<String>,
        offset: u64,
        limit: u64,
    },
    GetFileInfo {
        path: String,
        session_id: Option<String>,
    },
    Ping,
}

/// Responses sent from desktop back to remote clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "resp", rename_all = "snake_case")]
pub enum RemoteResponse {
    WorkspaceInfo {
        has_workspace: bool,
        path: Option<String>,
        project_name: Option<String>,
        git_branch: Option<String>,
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
    FileContent {
        name: String,
        content_base64: String,
        mime_type: String,
        size: u64,
    },
    FileChunk {
        name: String,
        chunk_base64: String,
        offset: u64,
        chunk_size: u64,
        total_size: u64,
        mime_type: String,
    },
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

/// Build a slim version of tool params for remote preview payloads.
///
/// Large string values such as file contents and diffs are omitted, while
/// short structured fields stay available for remote clients that need to
/// render tool details.
pub fn make_slim_tool_params(params: &serde_json::Value) -> Option<String> {
    match params {
        serde_json::Value::Object(obj) => {
            let slim: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .filter_map(|(key, value)| match value {
                    serde_json::Value::String(text) if text.len() > 200 => None,
                    _ => Some((key.clone(), value.clone())),
                })
                .collect();
            if slim.is_empty() {
                return None;
            }
            serde_json::to_string(&serde_json::Value::Object(slim)).ok()
        }
        serde_json::Value::String(text) => Some(text.chars().take(200).collect()),
        _ => None,
    }
}

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
    active_items: Vec<ChatMessageItem>,
    persistence_dirty: bool,
    linked_subagent_sessions: HashMap<String, String>,
}

/// Lightweight event broadcast by the tracker for real-time consumers.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackerEvent {
    TextChunk(String),
    ThinkingChunk(String),
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

/// Tracks the real-time state of a session for remote polling and bot streams.
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
                linked_subagent_sessions: HashMap::new(),
            }),
            event_tx,
        }
    }

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
        let state = self.state.read().unwrap();
        let has_items = !state.active_items.is_empty();
        state.turn_id.as_ref().map(|turn_id| ActiveTurnSnapshot {
            turn_id: turn_id.clone(),
            status: state.turn_status.clone(),
            text: if has_items {
                String::new()
            } else {
                state.accumulated_text.clone()
            },
            thinking: if has_items {
                String::new()
            } else {
                state.accumulated_thinking.clone()
            },
            tools: state.active_tools.clone(),
            round_index: state.round_index,
            items: if has_items {
                Some(state.active_items.clone())
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

    pub fn accumulated_text(&self) -> String {
        self.state.read().unwrap().accumulated_text.clone()
    }

    pub fn accumulated_thinking(&self) -> String {
        self.state.read().unwrap().accumulated_thinking.clone()
    }

    pub fn is_turn_finished(&self) -> bool {
        let state = self.state.read().unwrap();
        state.turn_id.is_some()
            && matches!(
                state.turn_status.as_str(),
                "completed" | "failed" | "cancelled"
            )
    }

    pub fn initialize_active_turn(&self, turn_id: String) {
        let mut state = self.state.write().unwrap();
        if state.turn_id.is_none() {
            state.turn_id = Some(turn_id);
            state.turn_status = "active".to_string();
            state.session_state = "running".to_string();
        }
        drop(state);
        self.bump_version();
    }

    pub fn finalize_completed_turn(&self) {
        let mut state = self.state.write().unwrap();
        if matches!(
            state.turn_status.as_str(),
            "completed" | "failed" | "cancelled"
        ) {
            state.turn_id = None;
            state.accumulated_text.clear();
            state.accumulated_thinking.clear();
            state.active_tools.clear();
            state.active_items.clear();
        }
    }

    pub fn is_persistence_dirty(&self) -> bool {
        self.state.read().unwrap().persistence_dirty
    }

    pub fn mark_persistence_clean(&self) {
        self.state.write().unwrap().persistence_dirty = false;
    }

    fn find_mergeable_item(
        items: &[ChatMessageItem],
        target_type: &str,
        subagent_marker: &Option<bool>,
    ) -> Option<usize> {
        for index in (0..items.len()).rev() {
            let item = &items[index];
            if item.item_type == "tool" {
                return None;
            }
            if item.item_type == target_type && &item.is_subagent == subagent_marker {
                return Some(index);
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

        if let Some(tool) =
            state.active_tools.iter_mut().rev().find(|tool| {
                tool.id == resolved_id || (allow_name_fallback && tool.name == tool_name)
            })
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

        if let Some(item) = state.active_items.iter_mut().rev().find(|item| {
            item.item_type == "tool"
                && item.tool.as_ref().is_some_and(|tool| {
                    tool.id == resolved_id || (allow_name_fallback && tool.name == tool_name)
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

    pub fn handle_agentic_event(&self, event: &AgenticEvent) {
        use bitfun_events::AgenticEvent as AE;

        if let AE::SubagentSessionLinked {
            session_id,
            parent_session_id,
            ..
        } = event
        {
            if parent_session_id != &self.target_session_id {
                return;
            }

            let mut state = self.state.write().unwrap();
            state
                .linked_subagent_sessions
                .insert(session_id.clone(), parent_session_id.clone());
            drop(state);
            self.bump_version();
            return;
        }

        let is_direct = event.session_id() == Some(self.target_session_id.as_str());
        let is_subagent = if !is_direct {
            match event {
                AE::TextChunk { session_id, .. }
                | AE::ThinkingChunk { session_id, .. }
                | AE::ToolEvent { session_id, .. } => self
                    .state
                    .read()
                    .unwrap()
                    .linked_subagent_sessions
                    .get(session_id)
                    .is_some_and(|parent_session_id| parent_session_id == &self.target_session_id),
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
                let mut state = self.state.write().unwrap();
                if !is_subagent {
                    state.accumulated_text.push_str(text);
                }
                if let Some(index) =
                    Self::find_mergeable_item(&state.active_items, "text", &subagent_marker)
                {
                    let item = &mut state.active_items[index];
                    item.content.get_or_insert_with(String::new).push_str(text);
                } else {
                    state.active_items.push(ChatMessageItem {
                        item_type: "text".to_string(),
                        content: Some(text.clone()),
                        tool: None,
                        is_subagent: subagent_marker,
                    });
                }
                drop(state);
                self.bump_version();
                let _ = self.event_tx.send(TrackerEvent::TextChunk(text.clone()));
            }
            AE::ThinkingChunk {
                content, is_end, ..
            } => {
                let clean = content.replace("</thinking>", "").replace("<thinking>", "");
                let subagent_marker = if is_subagent { Some(true) } else { None };
                let mut state = self.state.write().unwrap();
                if !is_subagent {
                    state.accumulated_thinking.push_str(&clean);
                }
                if let Some(index) =
                    Self::find_mergeable_item(&state.active_items, "thinking", &subagent_marker)
                {
                    let item = &mut state.active_items[index];
                    item.content
                        .get_or_insert_with(String::new)
                        .push_str(&clean);
                } else {
                    state.active_items.push(ChatMessageItem {
                        item_type: "thinking".to_string(),
                        content: Some(clean),
                        tool: None,
                        is_subagent: subagent_marker,
                    });
                }
                drop(state);
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
                if let Ok(value) = serde_json::to_value(tool_event) {
                    let event_type = value
                        .get("event_type")
                        .and_then(|value| value.as_str())
                        .unwrap_or("");
                    let tool_id = value
                        .get("tool_id")
                        .and_then(|value| value.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tool_name = value
                        .get("tool_name")
                        .and_then(|value| value.as_str())
                        .unwrap_or("")
                        .to_string();

                    let mut state = self.state.write().unwrap();
                    let allow_name_fallback = tool_id.is_empty() && !tool_name.is_empty();
                    let mut pending_tool_event: Option<TrackerEvent> = None;
                    match event_type {
                        "EarlyDetected" => {
                            Self::upsert_active_tool(
                                &mut state,
                                &tool_id,
                                &tool_name,
                                "preparing",
                                None,
                                None,
                                is_subagent,
                            );
                        }
                        "ConfirmationNeeded" => {
                            let params = value.get("params").cloned();
                            let input_preview = params.as_ref().and_then(make_slim_tool_params);
                            Self::upsert_active_tool(
                                &mut state,
                                &tool_id,
                                &tool_name,
                                "pending_confirmation",
                                input_preview,
                                params,
                                is_subagent,
                            );
                        }
                        "Started" => {
                            let params = value.get("params").cloned();
                            let input_preview = params.as_ref().and_then(make_slim_tool_params);
                            let tool_input = if tool_name == "AskUserQuestion"
                                || tool_name == "Task"
                                || tool_name == "TodoWrite"
                            {
                                params.clone()
                            } else {
                                None
                            };
                            Self::upsert_active_tool(
                                &mut state,
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
                                &mut state,
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
                                &mut state,
                                &tool_id,
                                &tool_name,
                                "rejected",
                                None,
                                None,
                                is_subagent,
                            );
                        }
                        "Completed" | "Succeeded" => {
                            let duration =
                                value.get("duration_ms").and_then(|value| value.as_u64());
                            if let Some(tool) = state.active_tools.iter_mut().rev().find(|tool| {
                                (tool.id == tool_id
                                    || (allow_name_fallback && tool.name == tool_name))
                                    && tool.status == "running"
                            }) {
                                tool.status = "completed".to_string();
                                tool.duration_ms = duration;
                            }
                            if let Some(item) = state.active_items.iter_mut().rev().find(|item| {
                                item.item_type == "tool"
                                    && item.tool.as_ref().is_some_and(|tool| {
                                        (tool.id == tool_id
                                            || (allow_name_fallback && tool.name == tool_name))
                                            && tool.status == "running"
                                    })
                            }) {
                                if let Some(tool) = item.tool.as_mut() {
                                    tool.status = "completed".to_string();
                                    tool.duration_ms = duration;
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
                            if let Some(tool) = state.active_tools.iter_mut().rev().find(|tool| {
                                (tool.id == tool_id
                                    || (allow_name_fallback && tool.name == tool_name))
                                    && tool.status == "running"
                            }) {
                                tool.status = "failed".to_string();
                            }
                            if let Some(item) = state.active_items.iter_mut().rev().find(|item| {
                                item.item_type == "tool"
                                    && item.tool.as_ref().is_some_and(|tool| {
                                        (tool.id == tool_id
                                            || (allow_name_fallback && tool.name == tool_name))
                                            && tool.status == "running"
                                    })
                            }) {
                                if let Some(tool) = item.tool.as_mut() {
                                    tool.status = "failed".to_string();
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
                            if let Some(tool) = state.active_tools.iter_mut().rev().find(|tool| {
                                (tool.id == tool_id
                                    || (allow_name_fallback && tool.name == tool_name))
                                    && matches!(
                                        tool.status.as_str(),
                                        "running" | "pending_confirmation" | "confirmed"
                                    )
                            }) {
                                tool.status = "cancelled".to_string();
                            }
                            if let Some(item) = state.active_items.iter_mut().rev().find(|item| {
                                item.item_type == "tool"
                                    && item.tool.as_ref().is_some_and(|tool| {
                                        (tool.id == tool_id
                                            || (allow_name_fallback && tool.name == tool_name))
                                            && matches!(
                                                tool.status.as_str(),
                                                "running" | "pending_confirmation" | "confirmed"
                                            )
                                    })
                            }) {
                                if let Some(tool) = item.tool.as_mut() {
                                    tool.status = "cancelled".to_string();
                                }
                            }
                        }
                        _ => {}
                    }
                    drop(state);
                    self.bump_version();
                    if let Some(event) = pending_tool_event {
                        let _ = self.event_tx.send(event);
                    }
                }
            }
            AE::DialogTurnStarted { turn_id, .. } if is_direct => {
                let mut state = self.state.write().unwrap();
                state.turn_id = Some(turn_id.clone());
                state.turn_status = "active".to_string();
                state.accumulated_text.clear();
                state.accumulated_thinking.clear();
                state.active_tools.clear();
                state.active_items.clear();
                state.round_index = 0;
                state.session_state = "running".to_string();
                state.persistence_dirty = true;
                drop(state);
                self.bump_version();
            }
            AE::DialogTurnCompleted { turn_id, .. } if is_direct => {
                let mut state = self.state.write().unwrap();
                state.turn_status = "completed".to_string();
                state.session_state = "idle".to_string();
                state.persistence_dirty = true;
                drop(state);
                self.bump_version();
                let _ = self.event_tx.send(TrackerEvent::TurnCompleted {
                    turn_id: turn_id.clone(),
                });
            }
            AE::DialogTurnFailed { turn_id, error, .. } if is_direct => {
                let mut state = self.state.write().unwrap();
                state.turn_status = "failed".to_string();
                state.session_state = "idle".to_string();
                state.persistence_dirty = true;
                drop(state);
                self.bump_version();
                let _ = self.event_tx.send(TrackerEvent::TurnFailed {
                    turn_id: turn_id.clone(),
                    error: error.clone(),
                });
            }
            AE::DialogTurnCancelled { turn_id, .. } if is_direct => {
                let mut state = self.state.write().unwrap();
                state.turn_status = "cancelled".to_string();
                state.session_state = "idle".to_string();
                state.persistence_dirty = true;
                drop(state);
                self.bump_version();
                let _ = self.event_tx.send(TrackerEvent::TurnCancelled {
                    turn_id: turn_id.clone(),
                });
            }
            AE::ModelRoundStarted { round_index, .. } if is_direct => {
                let mut state = self.state.write().unwrap();
                state.round_index = *round_index;
                drop(state);
                self.bump_version();
            }
            AE::SessionStateChanged { new_state, .. } if is_direct => {
                let mut state = self.state.write().unwrap();
                state.session_state = new_state.clone();
                drop(state);
                self.bump_version();
            }
            AE::SessionTitleGenerated { title, .. } if is_direct => {
                let mut state = self.state.write().unwrap();
                state.title = title.clone();
                drop(state);
                self.bump_version();
            }
            _ => {}
        }
    }
}

/// Host callbacks required to bind tracker lifecycle to the owning product runtime.
///
/// `services-integrations` owns tracker storage and lifecycle shape, while core
/// remains responsible for subscribing to concrete agent events and reading
/// in-memory active-turn state.
pub trait RemoteSessionTrackerHost {
    fn subscribe_tracker(&self, session_id: &str, tracker: Arc<RemoteSessionStateTracker>);
    fn unsubscribe_tracker(&self, session_id: &str);
    fn active_turn_id(&self, session_id: &str) -> Option<String>;
}

#[derive(Default)]
pub struct RemoteSessionTrackerRegistry {
    state_trackers: RwLock<HashMap<String, Arc<RemoteSessionStateTracker>>>,
}

impl RemoteSessionTrackerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ensure_tracker_with_host<H: RemoteSessionTrackerHost>(
        &self,
        session_id: &str,
        host: &H,
    ) -> Arc<RemoteSessionStateTracker> {
        if let Some(tracker) = self.get_tracker(session_id) {
            return tracker;
        }

        let tracker = {
            let mut trackers = self.state_trackers.write().unwrap();
            if let Some(tracker) = trackers.get(session_id) {
                return tracker.clone();
            }
            let tracker = Arc::new(RemoteSessionStateTracker::new(session_id.to_string()));
            trackers.insert(session_id.to_string(), tracker.clone());
            tracker
        };

        host.subscribe_tracker(session_id, tracker.clone());
        if let Some(active_turn_id) = host.active_turn_id(session_id) {
            tracker.initialize_active_turn(active_turn_id);
        }

        tracker
    }

    pub fn get_tracker(&self, session_id: &str) -> Option<Arc<RemoteSessionStateTracker>> {
        self.state_trackers.read().unwrap().get(session_id).cloned()
    }

    pub fn remove_tracker_with_host<H: RemoteSessionTrackerHost>(
        &self,
        session_id: &str,
        host: &H,
    ) -> Option<Arc<RemoteSessionStateTracker>> {
        let removed = self.state_trackers.write().unwrap().remove(session_id);
        if removed.is_some() {
            host.unsubscribe_tracker(session_id);
        }
        removed
    }
}

pub fn should_send_remote_model_catalog(
    current_model_catalog: Option<&RemoteModelCatalog>,
    known_model_catalog_version: Option<u64>,
) -> bool {
    let current_version = current_model_catalog
        .map(|catalog| catalog.version)
        .unwrap_or(0);
    known_model_catalog_version.unwrap_or(0) != current_version
}

pub fn remote_model_catalog_poll_delta(
    current_model_catalog: Option<RemoteModelCatalog>,
    known_model_catalog_version: Option<u64>,
) -> RemoteModelCatalogPollDelta {
    let changed = should_send_remote_model_catalog(
        current_model_catalog.as_ref(),
        known_model_catalog_version,
    );
    let catalog = if changed { current_model_catalog } else { None };

    RemoteModelCatalogPollDelta { changed, catalog }
}

pub fn remote_no_change_poll_response(version: u64) -> RemoteResponse {
    RemoteResponse::SessionPoll {
        version,
        changed: false,
        session_state: None,
        title: None,
        new_messages: None,
        total_msg_count: None,
        active_turn: None,
        model_catalog: Box::new(None),
    }
}

pub fn remote_snapshot_poll_response(
    tracker: &RemoteSessionStateTracker,
    version: u64,
    model_catalog: Option<RemoteModelCatalog>,
) -> RemoteResponse {
    let active_turn = tracker.snapshot_active_turn();
    let session_state = tracker.session_state();
    let title = tracker.title();
    RemoteResponse::SessionPoll {
        version,
        changed: true,
        session_state: Some(session_state),
        title: non_empty_title(title),
        new_messages: None,
        total_msg_count: None,
        active_turn,
        model_catalog: Box::new(model_catalog),
    }
}

pub fn remote_persisted_poll_response(
    tracker: &RemoteSessionStateTracker,
    version: u64,
    new_messages: Vec<ChatMessage>,
    total_msg_count: usize,
    model_catalog: Option<RemoteModelCatalog>,
) -> RemoteResponse {
    let turn_finished = tracker.is_turn_finished();
    let has_assistant_msg = new_messages
        .iter()
        .any(|message| message.role == "assistant");

    let active_turn = if turn_finished && has_assistant_msg {
        tracker.finalize_completed_turn();
        None
    } else if turn_finished {
        let status = tracker.turn_status();
        if status == "completed" {
            tracker.snapshot_active_turn()
        } else {
            tracker.finalize_completed_turn();
            tracker.mark_persistence_clean();
            None
        }
    } else {
        tracker.snapshot_active_turn()
    };

    let (send_messages, send_total) = if turn_finished && !has_assistant_msg {
        (None, None)
    } else {
        if !new_messages.is_empty() {
            tracker.mark_persistence_clean();
        }
        (Some(new_messages), Some(total_msg_count))
    };

    let session_state = tracker.session_state();
    let title = tracker.title();
    RemoteResponse::SessionPoll {
        version,
        changed: true,
        session_state: Some(session_state),
        title: non_empty_title(title),
        new_messages: send_messages,
        total_msg_count: send_total,
        active_turn,
        model_catalog: Box::new(model_catalog),
    }
}

fn non_empty_title(title: String) -> Option<String> {
    if title.is_empty() {
        None
    } else {
        Some(title)
    }
}
