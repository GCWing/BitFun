//! Persistence Manager
//!
//! Responsible for project-scoped session persistence and legacy
//! message/compression persistence used by in-memory managers.

use crate::agentic::core::{
    strip_prompt_markup, CompressionState, Message, MessageContent, Session, SessionConfig,
    SessionState, SessionSummary,
};
use crate::infrastructure::PathManager;
use crate::service::session::{
    DialogTurnData, SessionMetadata, SessionStatus, SessionTranscriptExport,
    SessionTranscriptExportOptions, SessionTranscriptIndexEntry, ToolItemData, TranscriptLineRange,
};
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, info, warn};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

const SESSION_SCHEMA_VERSION: u32 = 2;
const TRANSCRIPT_SCHEMA_VERSION: u32 = 1;
const JSON_WRITE_MAX_RETRIES: usize = 5;
const JSON_WRITE_RETRY_BASE_DELAY_MS: u64 = 30;
const SESSION_TRANSCRIPT_PREVIEW_CHAR_LIMIT: usize = 120;

static JSON_FILE_WRITE_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
static SESSION_INDEX_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSessionMetadataFile {
    schema_version: u32,
    #[serde(flatten)]
    metadata: SessionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredDialogTurnFile {
    schema_version: u32,
    #[serde(flatten)]
    turn: DialogTurnData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSessionStateFile {
    schema_version: u32,
    config: SessionConfig,
    snapshot_session_id: Option<String>,
    compression_state: CompressionState,
    runtime_state: SessionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredTurnContextSnapshotFile {
    schema_version: u32,
    session_id: String,
    turn_index: usize,
    messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSessionIndex {
    schema_version: u32,
    updated_at: u64,
    sessions: Vec<SessionMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSessionTranscriptFile {
    schema_version: u32,
    #[serde(flatten)]
    transcript: SessionTranscriptExport,
}

#[derive(Debug, Clone, Serialize)]
struct TranscriptFingerprintPayload {
    session_id: String,
    tools: bool,
    tool_inputs: bool,
    thinking: bool,
    turn_selectors: Option<Vec<String>>,
    turns: Vec<TranscriptFingerprintTurn>,
}

#[derive(Debug, Clone, Serialize)]
struct TranscriptFingerprintTurn {
    turn_id: String,
    turn_index: usize,
    status: String,
    user: String,
    assistant: Vec<TranscriptFingerprintTextBlock>,
    tools: Vec<TranscriptFingerprintTool>,
    thinking: Vec<TranscriptFingerprintTextBlock>,
}

#[derive(Debug, Clone, Serialize)]
struct TranscriptFingerprintTextBlock {
    round_index: usize,
    content: String,
}

#[derive(Debug, Clone, Serialize)]
struct TranscriptFingerprintTool {
    tool_name: String,
    tool_input: Option<String>,
    result: Option<String>,
}

#[derive(Debug, Clone)]
struct TranscriptTextBlock {
    round_index: usize,
    content: String,
}

#[derive(Debug, Clone)]
struct TranscriptToolBlock {
    tool_name: String,
    tool_input: Option<String>,
    result: Option<String>,
}

#[derive(Debug, Clone)]
enum TranscriptRoundBlock {
    Thinking(String),
    Assistant(String),
    Tool(TranscriptToolBlock),
}

#[derive(Debug, Clone)]
struct TranscriptRoundData {
    round_index: usize,
    blocks: Vec<TranscriptRoundBlock>,
}

#[derive(Debug, Clone)]
struct TranscriptSectionData {
    turn_index: usize,
    preview: String,
    lines: Vec<String>,
    turn_range: TranscriptLineRange,
    user_range: TranscriptLineRange,
}

#[derive(Debug, Clone, Copy)]
enum TranscriptTurnSelector {
    Index(isize),
    Slice {
        start: Option<isize>,
        end: Option<isize>,
    },
}

#[derive(Debug, Clone)]
struct ParsedTranscriptTurnSelector {
    normalized: String,
    selector: TranscriptTurnSelector,
}

pub struct PersistenceManager {
    path_manager: Arc<PathManager>,
}

impl PersistenceManager {
    pub fn new(path_manager: Arc<PathManager>) -> BitFunResult<Self> {
        Ok(Self { path_manager })
    }

    /// Get PathManager reference
    pub fn path_manager(&self) -> &Arc<PathManager> {
        &self.path_manager
    }

    fn project_sessions_dir(&self, workspace_path: &Path) -> PathBuf {
        self.path_manager.project_sessions_dir(workspace_path)
    }

    fn session_dir(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.project_sessions_dir(workspace_path).join(session_id)
    }

    fn metadata_path(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.session_dir(workspace_path, session_id)
            .join("metadata.json")
    }

    fn state_path(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.session_dir(workspace_path, session_id)
            .join("state.json")
    }

    fn turns_dir(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.session_dir(workspace_path, session_id).join("turns")
    }

    fn snapshots_dir(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.session_dir(workspace_path, session_id)
            .join("snapshots")
    }

    fn artifacts_dir(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.session_dir(workspace_path, session_id)
            .join("artifacts")
    }

    fn turn_path(&self, workspace_path: &Path, session_id: &str, turn_index: usize) -> PathBuf {
        self.turns_dir(workspace_path, session_id)
            .join(format!("turn-{:04}.json", turn_index))
    }

    fn context_snapshot_path(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> PathBuf {
        self.snapshots_dir(workspace_path, session_id)
            .join(format!("context-{:04}.json", turn_index))
    }

    fn transcript_path(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.artifacts_dir(workspace_path, session_id)
            .join("transcript.txt")
    }

    fn transcript_meta_path(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.artifacts_dir(workspace_path, session_id)
            .join("transcript.meta.json")
    }

    fn index_path(&self, workspace_path: &Path) -> PathBuf {
        self.project_sessions_dir(workspace_path).join("index.json")
    }

    async fn ensure_project_sessions_dir(&self, workspace_path: &Path) -> BitFunResult<PathBuf> {
        let dir = self.project_sessions_dir(workspace_path);
        fs::create_dir_all(&dir).await.map_err(|e| {
            BitFunError::io(format!(
                "Failed to create project sessions directory: {}",
                e
            ))
        })?;
        Ok(dir)
    }

    async fn ensure_session_dir(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<PathBuf> {
        let dir = self.session_dir(workspace_path, session_id);
        fs::create_dir_all(&dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create session directory: {}", e)))?;
        Ok(dir)
    }

    async fn ensure_turns_dir(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<PathBuf> {
        let dir = self.turns_dir(workspace_path, session_id);
        fs::create_dir_all(&dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create turns directory: {}", e)))?;
        Ok(dir)
    }

    async fn ensure_snapshots_dir(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<PathBuf> {
        let dir = self.snapshots_dir(workspace_path, session_id);
        fs::create_dir_all(&dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create snapshots directory: {}", e)))?;
        Ok(dir)
    }

    async fn ensure_artifacts_dir(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<PathBuf> {
        let dir = self.artifacts_dir(workspace_path, session_id);
        fs::create_dir_all(&dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create artifacts directory: {}", e)))?;
        Ok(dir)
    }

    async fn read_json_optional<T: DeserializeOwned>(
        &self,
        path: &Path,
    ) -> BitFunResult<Option<T>> {
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path).await.map_err(|e| {
            BitFunError::io(format!(
                "Failed to read JSON file {}: {}",
                path.display(),
                e
            ))
        })?;

        let value = serde_json::from_str::<T>(&content).map_err(|e| {
            BitFunError::Deserialization(format!(
                "Failed to deserialize JSON file {}: {}",
                path.display(),
                e
            ))
        })?;

        Ok(Some(value))
    }

    async fn write_json_atomic<T: Serialize>(&self, path: &Path, value: &T) -> BitFunResult<()> {
        let parent = path.parent().ok_or_else(|| {
            BitFunError::io(format!(
                "Target path has no parent directory: {}",
                path.display()
            ))
        })?;

        fs::create_dir_all(parent)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create parent directory: {}", e)))?;

        let json = serde_json::to_string_pretty(value)
            .map_err(|e| BitFunError::serialization(format!("Failed to serialize JSON: {}", e)))?;
        let lock = Self::get_file_write_lock(path).await;
        let _lock_guard = lock.lock().await;

        let json_bytes = json.into_bytes();
        let mut last_replace_error: Option<std::io::Error> = None;

        for attempt in 0..=JSON_WRITE_MAX_RETRIES {
            let tmp_path = Self::build_temp_json_path(path, attempt)?;
            if let Err(e) = fs::write(&tmp_path, &json_bytes).await {
                return Err(BitFunError::io(format!(
                    "Failed to write temp JSON file: {}",
                    e
                )));
            }

            match Self::replace_file_from_temp(path, &tmp_path).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    let should_retry =
                        Self::is_retryable_write_error(&e) && attempt < JSON_WRITE_MAX_RETRIES;
                    last_replace_error = Some(e);
                    let _ = fs::remove_file(&tmp_path).await;

                    if should_retry {
                        tokio::time::sleep(Self::retry_delay(attempt)).await;
                        continue;
                    }

                    break;
                }
            }
        }

        if let Some(error) = last_replace_error {
            // On Windows, external scanners/file indexers may temporarily hold a non-shareable
            // handle, making delete/rename fail with PermissionDenied. Fallback to direct write
            // to avoid losing session persistence while keeping best-effort atomic behavior.
            if error.kind() == ErrorKind::PermissionDenied {
                warn!(
                    "Atomic JSON replace permission denied for {}, fallback to direct overwrite",
                    path.display()
                );
                fs::write(path, &json_bytes).await.map_err(|e| {
                    BitFunError::io(format!(
                        "Failed fallback JSON overwrite {}: {}",
                        path.display(),
                        e
                    ))
                })?;
                return Ok(());
            }

            return Err(BitFunError::io(format!(
                "Failed to replace JSON file: {}",
                error
            )));
        }

        Err(BitFunError::io(format!(
            "Failed to replace JSON file {}: unknown error",
            path.display()
        )))
    }

    async fn get_file_write_lock(path: &Path) -> Arc<Mutex<()>> {
        let registry = JSON_FILE_WRITE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut registry_guard = registry.lock().await;
        registry_guard
            .entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    async fn get_session_index_lock(&self, workspace_path: &Path) -> Arc<Mutex<()>> {
        let index_path = self.index_path(workspace_path);
        let registry = SESSION_INDEX_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut registry_guard = registry.lock().await;
        registry_guard
            .entry(index_path)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn build_temp_json_path(path: &Path, attempt: usize) -> BitFunResult<PathBuf> {
        let parent = path.parent().ok_or_else(|| {
            BitFunError::io(format!(
                "Target path has no parent directory: {}",
                path.display()
            ))
        })?;

        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "data.json".to_string());
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp_name = format!(
            ".{}.{}.{}.{}.tmp",
            file_name,
            std::process::id(),
            nonce,
            attempt
        );
        Ok(parent.join(temp_name))
    }

    async fn replace_file_from_temp(target_path: &Path, tmp_path: &Path) -> std::io::Result<()> {
        if let Ok(()) = fs::rename(tmp_path, target_path).await {
            return Ok(());
        }

        if target_path.exists() {
            match fs::remove_file(target_path).await {
                Ok(()) => {}
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }

        fs::rename(tmp_path, target_path).await
    }

    fn is_retryable_write_error(error: &std::io::Error) -> bool {
        matches!(
            error.kind(),
            ErrorKind::PermissionDenied
                | ErrorKind::WouldBlock
                | ErrorKind::Interrupted
                | ErrorKind::TimedOut
                | ErrorKind::AlreadyExists
                | ErrorKind::Other
        )
    }

    fn retry_delay(attempt: usize) -> Duration {
        let exp = attempt.min(6) as u32;
        Duration::from_millis(JSON_WRITE_RETRY_BASE_DELAY_MS * (1u64 << exp))
    }

    fn system_time_to_unix_ms(time: SystemTime) -> u64 {
        time.duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn unix_ms_to_system_time(timestamp_ms: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(timestamp_ms)
    }

    fn sanitize_messages_for_persistence(messages: &[Message]) -> Vec<Message> {
        messages
            .iter()
            .map(Self::sanitize_message_for_persistence)
            .collect()
    }

    fn sanitize_message_for_persistence(message: &Message) -> Message {
        let mut sanitized = message.clone();

        match &mut sanitized.content {
            MessageContent::Multimodal { images, .. } => {
                for image in images.iter_mut() {
                    if image.data_url.as_ref().is_some_and(|v| !v.is_empty()) {
                        image.data_url = None;

                        let mut metadata = image
                            .metadata
                            .take()
                            .unwrap_or_else(|| serde_json::json!({}));
                        if !metadata.is_object() {
                            metadata = serde_json::json!({ "raw_metadata": metadata });
                        }
                        if let Some(obj) = metadata.as_object_mut() {
                            obj.insert("has_data_url".to_string(), serde_json::json!(true));
                        }
                        image.metadata = Some(metadata);
                    }
                }
            }
            MessageContent::ToolResult { result, .. } => {
                Self::redact_data_url_in_json(result);
            }
            _ => {}
        }

        sanitized
    }

    fn redact_data_url_in_json(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                let had_data_url = map.remove("data_url").is_some();
                if had_data_url {
                    map.insert("has_data_url".to_string(), serde_json::json!(true));
                }
                for child in map.values_mut() {
                    Self::redact_data_url_in_json(child);
                }
            }
            serde_json::Value::Array(arr) => {
                for child in arr {
                    Self::redact_data_url_in_json(child);
                }
            }
            _ => {}
        }
    }

    fn sanitize_runtime_state(state: &SessionState) -> SessionState {
        match state {
            SessionState::Processing { .. } => SessionState::Idle,
            other => other.clone(),
        }
    }

    fn build_session_metadata(
        &self,
        workspace_path: &Path,
        session: &Session,
        existing: Option<&SessionMetadata>,
    ) -> SessionMetadata {
        let created_at = existing
            .map(|value| value.created_at)
            .unwrap_or_else(|| Self::system_time_to_unix_ms(session.created_at));
        let last_active_at = Self::system_time_to_unix_ms(session.last_activity_at);
        let model_name = session
            .config
            .model_id
            .clone()
            .or_else(|| existing.map(|value| value.model_name.clone()))
            .unwrap_or_else(|| "default".to_string());

        SessionMetadata {
            session_id: session.session_id.clone(),
            session_name: session.session_name.clone(),
            agent_type: session.agent_type.clone(),
            created_by: session
                .created_by
                .clone()
                .or_else(|| existing.and_then(|value| value.created_by.clone())),
            model_name,
            created_at,
            last_active_at,
            turn_count: existing
                .map(|value| value.turn_count.max(session.dialog_turn_ids.len()))
                .unwrap_or(session.dialog_turn_ids.len()),
            message_count: existing.map(|value| value.message_count).unwrap_or(0),
            tool_call_count: existing.map(|value| value.tool_call_count).unwrap_or(0),
            status: existing
                .map(|value| value.status.clone())
                .unwrap_or(SessionStatus::Active),
            terminal_session_id: existing.and_then(|value| value.terminal_session_id.clone()),
            snapshot_session_id: session
                .snapshot_session_id
                .clone()
                .or_else(|| existing.and_then(|value| value.snapshot_session_id.clone())),
            tags: existing.map(|value| value.tags.clone()).unwrap_or_default(),
            custom_metadata: existing.and_then(|value| value.custom_metadata.clone()),
            todos: existing.and_then(|value| value.todos.clone()),
            workspace_path: Some(workspace_path.to_string_lossy().to_string()),
        }
    }

    fn turn_status_label(status: &crate::service::session::TurnStatus) -> &'static str {
        match status {
            crate::service::session::TurnStatus::InProgress => "inprogress",
            crate::service::session::TurnStatus::Completed => "completed",
            crate::service::session::TurnStatus::Error => "error",
            crate::service::session::TurnStatus::Cancelled => "cancelled",
        }
    }

    fn transcript_preview(content: &str) -> String {
        let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            return "(empty user message)".to_string();
        }

        let mut preview: String = normalized
            .chars()
            .take(SESSION_TRANSCRIPT_PREVIEW_CHAR_LIMIT)
            .collect();
        if normalized.chars().count() > SESSION_TRANSCRIPT_PREVIEW_CHAR_LIMIT {
            preview.push_str("...");
        }
        preview
    }

    fn transcript_text_lines(content: &str) -> Vec<String> {
        if content.is_empty() {
            return vec!["(empty)".to_string()];
        }

        let lines = content
            .lines()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        if lines.is_empty() {
            vec!["(empty)".to_string()]
        } else {
            lines
        }
    }

    fn transcript_value_string(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(text) => text.clone(),
            _ => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
        }
    }

    fn transcript_tool_input(item: &ToolItemData, tool_inputs: bool) -> Option<String> {
        if !tool_inputs || item.tool_call.input.is_null() {
            return None;
        }

        Some(Self::transcript_value_string(&item.tool_call.input))
    }

    fn transcript_tool_result(item: &ToolItemData) -> Option<String> {
        item.tool_result.as_ref().and_then(|result| {
            result
                .result_for_assistant
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    if result.result.is_null() {
                        None
                    } else {
                        Some(Self::transcript_value_string(&result.result))
                    }
                })
        })
    }

    fn transcript_display_user_content(turn: &DialogTurnData) -> String {
        turn.user_message
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("original_text"))
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| strip_prompt_markup(&turn.user_message.content))
    }

    fn transcript_assistant_blocks(turn: &DialogTurnData) -> Vec<TranscriptTextBlock> {
        turn.model_rounds
            .iter()
            .filter_map(|round| {
                let content = round
                    .text_items
                    .iter()
                    .filter(|item| !item.is_subagent_item.unwrap_or(false))
                    .map(|item| item.content.trim())
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                if content.is_empty() {
                    None
                } else {
                    Some(TranscriptTextBlock {
                        round_index: round.round_index,
                        content,
                    })
                }
            })
            .collect()
    }

    fn transcript_thinking_blocks(turn: &DialogTurnData) -> Vec<TranscriptTextBlock> {
        turn.model_rounds
            .iter()
            .filter_map(|round| {
                let content = round
                    .thinking_items
                    .iter()
                    .filter(|item| !item.is_subagent_item.unwrap_or(false))
                    .map(|item| item.content.trim())
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                if content.is_empty() {
                    None
                } else {
                    Some(TranscriptTextBlock {
                        round_index: round.round_index,
                        content,
                    })
                }
            })
            .collect()
    }

    fn transcript_tool_blocks(
        turn: &DialogTurnData,
        tool_inputs: bool,
    ) -> Vec<TranscriptToolBlock> {
        turn.model_rounds
            .iter()
            .flat_map(|round| round.tool_items.iter())
            .filter(|item| !item.is_subagent_item.unwrap_or(false))
            .map(|item| TranscriptToolBlock {
                tool_name: item.tool_name.clone(),
                tool_input: Self::transcript_tool_input(item, tool_inputs),
                result: Self::transcript_tool_result(item),
            })
            .collect()
    }

    fn transcript_round_blocks(
        turn: &DialogTurnData,
        options: &SessionTranscriptExportOptions,
    ) -> Vec<TranscriptRoundData> {
        turn.model_rounds
            .iter()
            .filter_map(|round| {
                let thinking_content = if options.thinking {
                    round
                        .thinking_items
                        .iter()
                        .filter(|item| !item.is_subagent_item.unwrap_or(false))
                        .map(|item| item.content.trim())
                        .filter(|value| !value.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n\n")
                } else {
                    String::new()
                };

                let assistant_content = round
                    .text_items
                    .iter()
                    .filter(|item| !item.is_subagent_item.unwrap_or(false))
                    .map(|item| item.content.trim())
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n");

                let tool_blocks = if options.tools {
                    round
                        .tool_items
                        .iter()
                        .filter(|item| !item.is_subagent_item.unwrap_or(false))
                        .map(|item| TranscriptToolBlock {
                            tool_name: item.tool_name.clone(),
                            tool_input: Self::transcript_tool_input(item, options.tool_inputs),
                            result: Self::transcript_tool_result(item),
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };

                if thinking_content.is_empty()
                    && assistant_content.is_empty()
                    && tool_blocks.is_empty()
                {
                    return None;
                }

                let mut blocks = Vec::new();
                if !thinking_content.is_empty() {
                    blocks.push(TranscriptRoundBlock::Thinking(thinking_content));
                }
                if !assistant_content.is_empty() {
                    blocks.push(TranscriptRoundBlock::Assistant(assistant_content));
                }
                for tool in tool_blocks {
                    blocks.push(TranscriptRoundBlock::Tool(tool));
                }

                Some(TranscriptRoundData {
                    round_index: round.round_index,
                    blocks,
                })
            })
            .collect()
    }

    fn transcript_fingerprint(
        session_id: &str,
        turns: &[DialogTurnData],
        options: &SessionTranscriptExportOptions,
    ) -> BitFunResult<String> {
        let payload = TranscriptFingerprintPayload {
            session_id: session_id.to_string(),
            tools: options.tools,
            tool_inputs: options.tool_inputs,
            thinking: options.thinking,
            turn_selectors: options.turns.clone(),
            turns: turns
                .iter()
                .map(|turn| TranscriptFingerprintTurn {
                    turn_id: turn.turn_id.clone(),
                    turn_index: turn.turn_index,
                    status: Self::turn_status_label(&turn.status).to_string(),
                    user: Self::transcript_display_user_content(turn),
                    assistant: Self::transcript_assistant_blocks(turn)
                        .into_iter()
                        .map(|block| TranscriptFingerprintTextBlock {
                            round_index: block.round_index,
                            content: block.content,
                        })
                        .collect(),
                    tools: if options.tools {
                        Self::transcript_tool_blocks(turn, options.tool_inputs)
                            .into_iter()
                            .map(|tool| TranscriptFingerprintTool {
                                tool_name: tool.tool_name,
                                tool_input: tool.tool_input,
                                result: tool.result,
                            })
                            .collect()
                    } else {
                        Vec::new()
                    },
                    thinking: if options.thinking {
                        Self::transcript_thinking_blocks(turn)
                            .into_iter()
                            .map(|block| TranscriptFingerprintTextBlock {
                                round_index: block.round_index,
                                content: block.content,
                            })
                            .collect()
                    } else {
                        Vec::new()
                    },
                })
                .collect(),
        };

        let bytes = serde_json::to_vec(&payload).map_err(|e| {
            BitFunError::serialization(format!("Failed to serialize transcript fingerprint: {}", e))
        })?;
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Ok(format!("{:x}", hasher.finalize()))
    }

    fn push_transcript_block(
        lines: &mut Vec<String>,
        label: &str,
        body_lines: Vec<String>,
    ) -> TranscriptLineRange {
        let start_line = lines.len() + 1;
        lines.push(format!("[{}]", label));
        lines.extend(body_lines);
        lines.push(format!("[/{}]", label));
        TranscriptLineRange {
            start_line,
            end_line: lines.len(),
        }
    }

    fn build_transcript_section(
        turn: &DialogTurnData,
        options: &SessionTranscriptExportOptions,
    ) -> TranscriptSectionData {
        let user_content = Self::transcript_display_user_content(turn);
        let round_blocks = Self::transcript_round_blocks(turn, options);

        let mut lines = Vec::new();
        lines.push(format!("## Turn {}", turn.turn_index));
        lines.push(String::new());

        let user_range = Self::push_transcript_block(
            &mut lines,
            "user",
            Self::transcript_text_lines(&user_content),
        );

        if !round_blocks.is_empty() {
            lines.push(String::new());
            for (round_index, round) in round_blocks.iter().enumerate() {
                lines.push(format!("[assistant_round {}]", round.round_index));
                for (block_index, block) in round.blocks.iter().enumerate() {
                    match block {
                        TranscriptRoundBlock::Thinking(content) => {
                            lines.push("[thinking]".to_string());
                            lines.extend(Self::transcript_text_lines(content));
                            lines.push("[/thinking]".to_string());
                        }
                        TranscriptRoundBlock::Assistant(content) => {
                            lines.push("[text]".to_string());
                            lines.extend(Self::transcript_text_lines(content));
                            lines.push("[/text]".to_string());
                        }
                        TranscriptRoundBlock::Tool(tool) => {
                            lines.push("[tool]".to_string());
                            lines.push(format!("name: {}", tool.tool_name));
                            if let Some(tool_input) = tool.tool_input.as_ref() {
                                lines.push("input:".to_string());
                                lines.extend(Self::transcript_text_lines(tool_input));
                            }
                            if let Some(result) = tool.result.as_ref() {
                                lines.push("result:".to_string());
                                lines.extend(Self::transcript_text_lines(result));
                            }
                            lines.push("[/tool]".to_string());
                        }
                    }

                    if block_index + 1 < round.blocks.len() {
                        lines.push(String::new());
                    }
                }
                lines.push(format!("[/assistant_round {}]", round.round_index));
                if round_index + 1 < round_blocks.len() {
                    lines.push(String::new());
                }
            }
        }

        TranscriptSectionData {
            turn_index: turn.turn_index,
            preview: Self::transcript_preview(&user_content),
            turn_range: TranscriptLineRange {
                start_line: 1,
                end_line: lines.len(),
            },
            user_range,
            lines,
        }
    }

    fn offset_range(range: &TranscriptLineRange, offset: usize) -> TranscriptLineRange {
        TranscriptLineRange {
            start_line: range.start_line + offset,
            end_line: range.end_line + offset,
        }
    }

    fn format_range(range: &TranscriptLineRange) -> String {
        format!("{}-{}", range.start_line, range.end_line)
    }

    fn parse_transcript_turn_selectors(
        selectors: &[String],
    ) -> BitFunResult<Vec<ParsedTranscriptTurnSelector>> {
        if selectors.is_empty() {
            return Err(BitFunError::Validation(
                "turns cannot be an empty array".to_string(),
            ));
        }

        selectors
            .iter()
            .map(|selector| Self::parse_transcript_turn_selector(selector))
            .collect()
    }

    fn parse_transcript_turn_selector(
        selector: &str,
    ) -> BitFunResult<ParsedTranscriptTurnSelector> {
        let normalized = selector.trim();
        if normalized.is_empty() {
            return Err(BitFunError::Validation(
                "turns cannot contain empty selectors".to_string(),
            ));
        }

        if normalized.matches(':').count() > 1 {
            return Err(BitFunError::Validation(format!(
                "Invalid turn selector '{}'. Use forms like ':20', '-20:', '10:30', or '15'.",
                normalized
            )));
        }

        let selector = if let Some((start, end)) = normalized.split_once(':') {
            TranscriptTurnSelector::Slice {
                start: if start.is_empty() {
                    None
                } else {
                    Some(Self::parse_transcript_turn_value(start, normalized)?)
                },
                end: if end.is_empty() {
                    None
                } else {
                    Some(Self::parse_transcript_turn_value(end, normalized)?)
                },
            }
        } else {
            TranscriptTurnSelector::Index(Self::parse_transcript_turn_value(
                normalized, normalized,
            )?)
        };

        Ok(ParsedTranscriptTurnSelector {
            normalized: normalized.to_string(),
            selector,
        })
    }

    fn parse_transcript_turn_value(value: &str, selector: &str) -> BitFunResult<isize> {
        value.parse::<isize>().map_err(|_| {
            BitFunError::Validation(format!(
                "Invalid turn selector '{}'. Use forms like ':20', '-20:', '10:30', or '15'.",
                selector
            ))
        })
    }

    fn transcript_normalize_slice_bound(
        total: usize,
        bound: Option<isize>,
        default: usize,
    ) -> usize {
        let Some(bound) = bound else {
            return default;
        };

        let total = total as isize;
        let normalized = if bound < 0 {
            total.saturating_add(bound)
        } else {
            bound
        };
        normalized.clamp(0, total) as usize
    }

    fn transcript_normalize_index(total: usize, index: isize) -> Option<usize> {
        let total = total as isize;
        let normalized = if index < 0 {
            total.saturating_add(index)
        } else {
            index
        };

        if normalized < 0 || normalized >= total {
            None
        } else {
            Some(normalized as usize)
        }
    }

    fn transcript_select_turn_indices(
        total: usize,
        selectors: &[ParsedTranscriptTurnSelector],
    ) -> Vec<usize> {
        let mut selected = vec![false; total];

        for selector in selectors {
            match selector.selector {
                TranscriptTurnSelector::Index(index) => {
                    if let Some(index) = Self::transcript_normalize_index(total, index) {
                        selected[index] = true;
                    }
                }
                TranscriptTurnSelector::Slice { start, end } => {
                    let start = Self::transcript_normalize_slice_bound(total, start, 0);
                    let end = Self::transcript_normalize_slice_bound(total, end, total);
                    if start < end {
                        selected[start..end].fill(true);
                    }
                }
            }
        }

        selected
            .into_iter()
            .enumerate()
            .filter_map(|(index, is_selected)| is_selected.then_some(index))
            .collect()
    }

    fn transcript_omitted_turns_label(
        turns: &[DialogTurnData],
        start: usize,
        end: usize,
    ) -> String {
        let start_turn = turns[start].turn_index;
        let end_turn = turns[end].turn_index;
        if start_turn == end_turn {
            format!("(omitted turn {})", start_turn)
        } else {
            format!("(omitted turns {}-{})", start_turn, end_turn)
        }
    }

    async fn rebuild_index_locked(
        &self,
        workspace_path: &Path,
    ) -> BitFunResult<Vec<SessionMetadata>> {
        let sessions_root = self.ensure_project_sessions_dir(workspace_path).await?;
        let mut metadata_list = Vec::new();
        let mut entries = fs::read_dir(&sessions_root)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read sessions root: {}", e)))?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            BitFunError::io(format!("Failed to read session directory entry: {}", e))
        })? {
            let file_type = entry
                .file_type()
                .await
                .map_err(|e| BitFunError::io(format!("Failed to get file type: {}", e)))?;
            if !file_type.is_dir() {
                continue;
            }

            let session_id = entry.file_name().to_string_lossy().to_string();
            match self
                .load_session_metadata(workspace_path, &session_id)
                .await
            {
                Ok(Some(metadata)) => metadata_list.push(metadata),
                Ok(None) => {}
                Err(e) => {
                    warn!(
                        "Failed to rebuild session index entry: session_id={}, error={}",
                        session_id, e
                    );
                }
            }
        }

        metadata_list.sort_by(|a, b| b.last_active_at.cmp(&a.last_active_at));

        let index = StoredSessionIndex {
            schema_version: SESSION_SCHEMA_VERSION,
            updated_at: Self::system_time_to_unix_ms(SystemTime::now()),
            sessions: metadata_list.clone(),
        };
        self.write_json_atomic(&self.index_path(workspace_path), &index)
            .await?;

        Ok(metadata_list)
    }

    async fn upsert_index_entry_locked(
        &self,
        workspace_path: &Path,
        metadata: &SessionMetadata,
    ) -> BitFunResult<()> {
        let index_path = self.index_path(workspace_path);
        let mut index = self
            .read_json_optional::<StoredSessionIndex>(&index_path)
            .await?
            .unwrap_or(StoredSessionIndex {
                schema_version: SESSION_SCHEMA_VERSION,
                updated_at: 0,
                sessions: Vec::new(),
            });

        if let Some(existing) = index
            .sessions
            .iter_mut()
            .find(|value| value.session_id == metadata.session_id)
        {
            *existing = metadata.clone();
        } else {
            index.sessions.push(metadata.clone());
        }

        index
            .sessions
            .sort_by(|a, b| b.last_active_at.cmp(&a.last_active_at));
        index.updated_at = Self::system_time_to_unix_ms(SystemTime::now());
        index.schema_version = SESSION_SCHEMA_VERSION;
        self.write_json_atomic(&index_path, &index).await
    }

    async fn remove_index_entry_locked(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<()> {
        let index_path = self.index_path(workspace_path);
        let Some(mut index) = self
            .read_json_optional::<StoredSessionIndex>(&index_path)
            .await?
        else {
            return Ok(());
        };

        index
            .sessions
            .retain(|value| value.session_id != session_id);
        index.updated_at = Self::system_time_to_unix_ms(SystemTime::now());
        self.write_json_atomic(&index_path, &index).await
    }

    async fn upsert_index_entry(
        &self,
        workspace_path: &Path,
        metadata: &SessionMetadata,
    ) -> BitFunResult<()> {
        let lock = self.get_session_index_lock(workspace_path).await;
        let _guard = lock.lock().await;
        self.upsert_index_entry_locked(workspace_path, metadata)
            .await
    }

    async fn remove_index_entry(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<()> {
        let lock = self.get_session_index_lock(workspace_path).await;
        let _guard = lock.lock().await;
        self.remove_index_entry_locked(workspace_path, session_id)
            .await
    }

    pub async fn list_session_metadata(
        &self,
        workspace_path: &Path,
    ) -> BitFunResult<Vec<SessionMetadata>> {
        if !workspace_path.exists() {
            return Ok(Vec::new());
        }

        let lock = self.get_session_index_lock(workspace_path).await;
        let _guard = lock.lock().await;
        let index_path = self.index_path(workspace_path);
        if let Some(index) = self
            .read_json_optional::<StoredSessionIndex>(&index_path)
            .await?
        {
            let has_stale_entry = index.sessions.iter().any(|metadata| {
                !self
                    .metadata_path(workspace_path, &metadata.session_id)
                    .exists()
            });
            if has_stale_entry {
                warn!(
                    "Session index contains stale entries, rebuilding: {}",
                    index_path.display()
                );
                return self.rebuild_index_locked(workspace_path).await;
            }
            return Ok(index.sessions);
        }

        self.rebuild_index_locked(workspace_path).await
    }

    pub async fn save_session_metadata(
        &self,
        workspace_path: &Path,
        metadata: &SessionMetadata,
    ) -> BitFunResult<()> {
        self.ensure_session_dir(workspace_path, &metadata.session_id)
            .await?;

        let file = StoredSessionMetadataFile {
            schema_version: SESSION_SCHEMA_VERSION,
            metadata: metadata.clone(),
        };

        self.write_json_atomic(
            &self.metadata_path(workspace_path, &metadata.session_id),
            &file,
        )
        .await?;
        self.upsert_index_entry(workspace_path, metadata).await
    }

    pub async fn load_session_metadata(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Option<SessionMetadata>> {
        let path = self.metadata_path(workspace_path, session_id);
        Ok(self
            .read_json_optional::<StoredSessionMetadataFile>(&path)
            .await?
            .map(|file| file.metadata))
    }

    async fn load_stored_session_state(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Option<StoredSessionStateFile>> {
        self.read_json_optional::<StoredSessionStateFile>(
            &self.state_path(workspace_path, session_id),
        )
        .await
    }

    async fn save_stored_session_state(
        &self,
        workspace_path: &Path,
        session_id: &str,
        state: &StoredSessionStateFile,
    ) -> BitFunResult<()> {
        self.write_json_atomic(&self.state_path(workspace_path, session_id), state)
            .await
    }

    // ============ Turn context snapshot (sent to model)============

    pub async fn save_turn_context_snapshot(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
        messages: &[Message],
    ) -> BitFunResult<()> {
        self.ensure_snapshots_dir(workspace_path, session_id)
            .await?;

        let snapshot = StoredTurnContextSnapshotFile {
            schema_version: SESSION_SCHEMA_VERSION,
            session_id: session_id.to_string(),
            turn_index,
            messages: Self::sanitize_messages_for_persistence(messages),
        };

        self.write_json_atomic(
            &self.context_snapshot_path(workspace_path, session_id, turn_index),
            &snapshot,
        )
        .await
    }

    pub async fn load_turn_context_snapshot(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<Option<Vec<Message>>> {
        let snapshot = self
            .read_json_optional::<StoredTurnContextSnapshotFile>(&self.context_snapshot_path(
                workspace_path,
                session_id,
                turn_index,
            ))
            .await?;
        Ok(snapshot.map(|value| value.messages))
    }

    pub async fn load_latest_turn_context_snapshot(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Option<(usize, Vec<Message>)>> {
        let dir = self.snapshots_dir(workspace_path, session_id);
        if !dir.exists() {
            return Ok(None);
        }

        let mut latest: Option<usize> = None;
        let mut rd = fs::read_dir(&dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read snapshots directory: {}", e)))?;

        while let Some(entry) = rd
            .next_entry()
            .await
            .map_err(|e| BitFunError::io(format!("Failed to iterate snapshots directory: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(index_str) = stem.strip_prefix("context-") else {
                continue;
            };
            if let Ok(index) = index_str.parse::<usize>() {
                latest = Some(latest.map(|value| value.max(index)).unwrap_or(index));
            }
        }

        let Some(turn_index) = latest else {
            return Ok(None);
        };

        let Some(messages) = self
            .load_turn_context_snapshot(workspace_path, session_id, turn_index)
            .await?
        else {
            return Ok(None);
        };

        Ok(Some((turn_index, messages)))
    }

    pub async fn delete_turn_context_snapshots_from(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<()> {
        let dir = self.snapshots_dir(workspace_path, session_id);
        if !dir.exists() {
            return Ok(());
        }

        let mut rd = fs::read_dir(&dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read snapshots directory: {}", e)))?;
        while let Some(entry) = rd
            .next_entry()
            .await
            .map_err(|e| BitFunError::io(format!("Failed to iterate snapshots directory: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(index_str) = stem.strip_prefix("context-") else {
                continue;
            };
            let Ok(index) = index_str.parse::<usize>() else {
                continue;
            };
            if index >= turn_index {
                let _ = fs::remove_file(&path).await;
            }
        }

        Ok(())
    }

    // ============ Session Persistence ============

    /// Save session
    pub async fn save_session(&self, workspace_path: &Path, session: &Session) -> BitFunResult<()> {
        self.ensure_session_dir(workspace_path, &session.session_id)
            .await?;

        let existing_metadata = self
            .load_session_metadata(workspace_path, &session.session_id)
            .await?;
        let metadata =
            self.build_session_metadata(workspace_path, session, existing_metadata.as_ref());
        self.save_session_metadata(workspace_path, &metadata)
            .await?;

        let state = StoredSessionStateFile {
            schema_version: SESSION_SCHEMA_VERSION,
            config: session.config.clone(),
            snapshot_session_id: session.snapshot_session_id.clone(),
            compression_state: session.compression_state.clone(),
            runtime_state: Self::sanitize_runtime_state(&session.state),
        };
        self.save_stored_session_state(workspace_path, &session.session_id, &state)
            .await
    }

    /// Load session
    pub async fn load_session(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Session> {
        let metadata = self
            .load_session_metadata(workspace_path, session_id)
            .await?
            .ok_or_else(|| {
                BitFunError::NotFound(format!("Session metadata not found: {}", session_id))
            })?;
        let stored_state = self
            .load_stored_session_state(workspace_path, session_id)
            .await?;
        let turns = self.load_session_turns(workspace_path, session_id).await?;

        let mut config = stored_state
            .as_ref()
            .map(|value| value.config.clone())
            .unwrap_or_default();
        if config.workspace_path.is_none() {
            config.workspace_path = Some(workspace_path.to_string_lossy().to_string());
        }
        if config.model_id.is_none() && !metadata.model_name.is_empty() {
            config.model_id = Some(metadata.model_name.clone());
        }

        let compression_state = stored_state
            .as_ref()
            .map(|value| value.compression_state.clone())
            .unwrap_or_default();
        let runtime_state = stored_state
            .as_ref()
            .map(|value| Self::sanitize_runtime_state(&value.runtime_state))
            .unwrap_or(SessionState::Idle);
        let created_at = Self::unix_ms_to_system_time(metadata.created_at);
        let last_activity_at = Self::unix_ms_to_system_time(metadata.last_active_at);

        Ok(Session {
            session_id: metadata.session_id.clone(),
            session_name: metadata.session_name.clone(),
            agent_type: metadata.agent_type.clone(),
            created_by: metadata.created_by.clone(),
            snapshot_session_id: stored_state
                .and_then(|value| value.snapshot_session_id)
                .or(metadata.snapshot_session_id.clone()),
            dialog_turn_ids: turns.into_iter().map(|turn| turn.turn_id).collect(),
            state: runtime_state,
            config,
            compression_state,
            created_at,
            updated_at: last_activity_at,
            last_activity_at,
        })
    }

    /// Save session state
    pub async fn save_session_state(
        &self,
        workspace_path: &Path,
        session_id: &str,
        state: &SessionState,
    ) -> BitFunResult<()> {
        let mut stored_state = self
            .load_stored_session_state(workspace_path, session_id)
            .await?
            .unwrap_or(StoredSessionStateFile {
                schema_version: SESSION_SCHEMA_VERSION,
                config: SessionConfig {
                    workspace_path: Some(workspace_path.to_string_lossy().to_string()),
                    ..Default::default()
                },
                snapshot_session_id: None,
                compression_state: CompressionState::default(),
                runtime_state: SessionState::Idle,
            });
        stored_state.schema_version = SESSION_SCHEMA_VERSION;
        stored_state.runtime_state = Self::sanitize_runtime_state(state);
        self.save_stored_session_state(workspace_path, session_id, &stored_state)
            .await
    }

    /// Delete session
    pub async fn delete_session(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<()> {
        let dir = self.session_dir(workspace_path, session_id);
        if dir.exists() {
            fs::remove_dir_all(&dir).await.map_err(|e| {
                BitFunError::io(format!("Failed to delete session directory: {}", e))
            })?;
        }

        self.remove_index_entry(workspace_path, session_id).await?;
        info!("Session deleted: session_id={}", session_id);
        Ok(())
    }

    /// List all sessions
    pub async fn list_sessions(&self, workspace_path: &Path) -> BitFunResult<Vec<SessionSummary>> {
        let metadata_list = self.list_session_metadata(workspace_path).await?;
        let mut summaries = Vec::with_capacity(metadata_list.len());

        for metadata in metadata_list {
            let state = self
                .load_stored_session_state(workspace_path, &metadata.session_id)
                .await?
                .map(|value| Self::sanitize_runtime_state(&value.runtime_state))
                .unwrap_or(SessionState::Idle);

            summaries.push(SessionSummary {
                session_id: metadata.session_id,
                session_name: metadata.session_name,
                agent_type: metadata.agent_type,
                created_by: metadata.created_by,
                turn_count: metadata.turn_count,
                created_at: Self::unix_ms_to_system_time(metadata.created_at),
                last_activity_at: Self::unix_ms_to_system_time(metadata.last_active_at),
                state,
            });
        }

        summaries.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
        Ok(summaries)
    }

    fn estimate_turn_message_count(turn: &DialogTurnData) -> usize {
        let assistant_text_count: usize = turn
            .model_rounds
            .iter()
            .map(|round| round.text_items.len())
            .sum();
        1 + assistant_text_count
    }

    pub async fn save_dialog_turn(
        &self,
        workspace_path: &Path,
        turn: &DialogTurnData,
    ) -> BitFunResult<()> {
        let mut metadata = self
            .load_session_metadata(workspace_path, &turn.session_id)
            .await?
            .ok_or_else(|| {
                BitFunError::NotFound(format!("Session metadata not found: {}", turn.session_id))
            })?;

        self.ensure_turns_dir(workspace_path, &turn.session_id)
            .await?;

        let file = StoredDialogTurnFile {
            schema_version: SESSION_SCHEMA_VERSION,
            turn: turn.clone(),
        };
        self.write_json_atomic(
            &self.turn_path(workspace_path, &turn.session_id, turn.turn_index),
            &file,
        )
        .await?;

        let turns = self
            .load_session_turns(workspace_path, &turn.session_id)
            .await?;
        metadata.turn_count = turns.len();
        metadata.message_count = turns.iter().map(Self::estimate_turn_message_count).sum();
        metadata.tool_call_count = turns.iter().map(DialogTurnData::count_tool_calls).sum();
        metadata.last_active_at = turn
            .end_time
            .unwrap_or_else(|| Self::system_time_to_unix_ms(SystemTime::now()));
        metadata.workspace_path = Some(workspace_path.to_string_lossy().to_string());
        self.save_session_metadata(workspace_path, &metadata).await
    }

    pub async fn load_dialog_turn(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<Option<DialogTurnData>> {
        Ok(self
            .read_json_optional::<StoredDialogTurnFile>(&self.turn_path(
                workspace_path,
                session_id,
                turn_index,
            ))
            .await?
            .map(|file| file.turn))
    }

    pub async fn load_session_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Vec<DialogTurnData>> {
        let turns_dir = self.turns_dir(workspace_path, session_id);
        if !turns_dir.exists() {
            return Ok(Vec::new());
        }

        let mut indexed_paths = Vec::new();
        let mut entries = fs::read_dir(&turns_dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read turns directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| BitFunError::io(format!("Failed to iterate turns directory: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(index_str) = stem.strip_prefix("turn-") else {
                continue;
            };
            let Ok(index) = index_str.parse::<usize>() else {
                continue;
            };
            indexed_paths.push((index, path));
        }

        indexed_paths.sort_by_key(|(index, _)| *index);

        let mut turns = Vec::with_capacity(indexed_paths.len());
        for (_, path) in indexed_paths {
            if let Some(file) = self
                .read_json_optional::<StoredDialogTurnFile>(&path)
                .await?
            {
                turns.push(file.turn);
            }
        }

        Ok(turns)
    }

    pub async fn load_recent_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
        count: usize,
    ) -> BitFunResult<Vec<DialogTurnData>> {
        let turns = self.load_session_turns(workspace_path, session_id).await?;
        let start = turns.len().saturating_sub(count);
        Ok(turns[start..].to_vec())
    }

    pub async fn export_session_transcript(
        &self,
        workspace_path: &Path,
        session_id: &str,
        options: &SessionTranscriptExportOptions,
    ) -> BitFunResult<SessionTranscriptExport> {
        if self
            .load_session_metadata(workspace_path, session_id)
            .await?
            .is_none()
        {
            return Err(BitFunError::NotFound(format!(
                "Session metadata not found: {}",
                session_id
            )));
        }

        let transcript_path = self.transcript_path(workspace_path, session_id);
        let transcript_meta_path = self.transcript_meta_path(workspace_path, session_id);

        let parsed_turn_selectors = options
            .turns
            .as_ref()
            .map(|selectors| Self::parse_transcript_turn_selectors(selectors))
            .transpose()?;
        let normalized_options = SessionTranscriptExportOptions {
            tools: options.tools,
            tool_inputs: options.tool_inputs,
            thinking: options.thinking,
            turns: parsed_turn_selectors.as_ref().map(|selectors| {
                selectors
                    .iter()
                    .map(|selector| selector.normalized.clone())
                    .collect()
            }),
        };

        let all_turns = self.load_session_turns(workspace_path, session_id).await?;
        let selected_indices = parsed_turn_selectors
            .as_ref()
            .map(|selectors| Self::transcript_select_turn_indices(all_turns.len(), selectors))
            .unwrap_or_else(|| (0..all_turns.len()).collect::<Vec<_>>());
        let turns = selected_indices
            .iter()
            .map(|&index| all_turns[index].clone())
            .collect::<Vec<_>>();

        let source_fingerprint =
            Self::transcript_fingerprint(session_id, &turns, &normalized_options)?;
        if transcript_path.exists() {
            if let Some(stored) = self
                .read_json_optional::<StoredSessionTranscriptFile>(&transcript_meta_path)
                .await?
            {
                if stored.transcript.source_fingerprint == source_fingerprint
                    && stored.transcript.index_range.start_line > 0
                    && stored.transcript.index_range.end_line > 0
                {
                    return Ok(stored.transcript);
                }
            }
        }

        self.ensure_artifacts_dir(workspace_path, session_id)
            .await?;

        let generated_at = Self::system_time_to_unix_ms(SystemTime::now());
        let sections = selected_indices
            .iter()
            .map(|&index| {
                (
                    index,
                    Self::build_transcript_section(&all_turns[index], &normalized_options),
                )
            })
            .collect::<Vec<_>>();

        let mut lines = vec!["## Index".to_string()];

        let mut index = Vec::with_capacity(sections.len());
        if sections.is_empty() {
            lines.push(if all_turns.is_empty() {
                "(no persisted turns)".to_string()
            } else {
                "(no matching turns)".to_string()
            });
        } else {
            let index_offset = lines.len() + sections.len() + 1;
            let mut body_lines = Vec::new();

            for (position, (source_index, section)) in sections.iter().enumerate() {
                let omitted_range = if position == 0 {
                    (*source_index > 0).then(|| (0, *source_index - 1))
                } else {
                    let previous_index = sections[position - 1].0;
                    (*source_index > previous_index + 1)
                        .then(|| (previous_index + 1, *source_index - 1))
                };

                if let Some((start, end)) = omitted_range {
                    if !body_lines.is_empty() {
                        body_lines.push(String::new());
                    }
                    body_lines.push(Self::transcript_omitted_turns_label(&all_turns, start, end));
                    body_lines.push(String::new());
                } else if !body_lines.is_empty() {
                    body_lines.push(String::new());
                }

                let section_offset = index_offset + body_lines.len();
                let turn_range = Self::offset_range(&section.turn_range, section_offset);
                let user_range = Self::offset_range(&section.user_range, section_offset);

                let index_line = format!(
                    "- turn={} range={} preview=\"{}\"",
                    section.turn_index,
                    Self::format_range(&turn_range),
                    section.preview.replace('"', "'")
                );
                lines.push(index_line);

                index.push(SessionTranscriptIndexEntry {
                    turn_index: section.turn_index,
                    preview: section.preview.clone(),
                    turn_range,
                    user_range,
                });

                body_lines.extend(section.lines.iter().cloned());
            }

            if let Some((last_index, _)) = sections.last() {
                if *last_index + 1 < all_turns.len() {
                    body_lines.push(String::new());
                    body_lines.push(Self::transcript_omitted_turns_label(
                        &all_turns,
                        *last_index + 1,
                        all_turns.len() - 1,
                    ));
                }
            }

            lines.push(String::new());
            lines.extend(body_lines);
        }

        let index_range = TranscriptLineRange {
            start_line: 1,
            end_line: lines
                .iter()
                .position(|line| line.is_empty())
                .unwrap_or(lines.len()),
        };

        let transcript_content = lines.join("\n");
        fs::write(&transcript_path, transcript_content)
            .await
            .map_err(|e| {
                BitFunError::io(format!(
                    "Failed to write transcript file {}: {}",
                    transcript_path.display(),
                    e
                ))
            })?;

        let transcript = SessionTranscriptExport {
            session_id: session_id.to_string(),
            transcript_path: transcript_path.to_string_lossy().to_string(),
            generated_at,
            source_fingerprint,
            includes_tools: normalized_options.tools,
            includes_tool_inputs: normalized_options.tool_inputs,
            includes_thinking: normalized_options.thinking,
            turns: normalized_options.turns,
            turn_count: turns.len(),
            line_count: lines.len(),
            index_range,
            index,
        };

        self.write_json_atomic(
            &transcript_meta_path,
            &StoredSessionTranscriptFile {
                schema_version: TRANSCRIPT_SCHEMA_VERSION,
                transcript: transcript.clone(),
            },
        )
        .await?;

        Ok(transcript)
    }

    pub async fn delete_turns_after(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<usize> {
        let turns = self.load_session_turns(workspace_path, session_id).await?;
        let mut deleted = 0usize;

        for turn in turns
            .into_iter()
            .filter(|value| value.turn_index > turn_index)
        {
            let path = self.turn_path(workspace_path, session_id, turn.turn_index);
            if path.exists() {
                fs::remove_file(&path)
                    .await
                    .map_err(|e| BitFunError::io(format!("Failed to delete turn file: {}", e)))?;
                deleted += 1;
            }
        }

        if let Some(mut metadata) = self
            .load_session_metadata(workspace_path, session_id)
            .await?
        {
            let remaining_turns = self.load_session_turns(workspace_path, session_id).await?;
            metadata.turn_count = remaining_turns.len();
            metadata.message_count = remaining_turns
                .iter()
                .map(Self::estimate_turn_message_count)
                .sum();
            metadata.tool_call_count = remaining_turns
                .iter()
                .map(DialogTurnData::count_tool_calls)
                .sum();
            metadata.last_active_at = Self::system_time_to_unix_ms(SystemTime::now());
            self.save_session_metadata(workspace_path, &metadata)
                .await?;
        }

        Ok(deleted)
    }

    pub async fn delete_turns_from(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<usize> {
        let turns = self.load_session_turns(workspace_path, session_id).await?;
        let mut deleted = 0usize;

        for turn in turns
            .into_iter()
            .filter(|value| value.turn_index >= turn_index)
        {
            let path = self.turn_path(workspace_path, session_id, turn.turn_index);
            if path.exists() {
                fs::remove_file(&path)
                    .await
                    .map_err(|e| BitFunError::io(format!("Failed to delete turn file: {}", e)))?;
                deleted += 1;
            }
        }

        if let Some(mut metadata) = self
            .load_session_metadata(workspace_path, session_id)
            .await?
        {
            let remaining_turns = self.load_session_turns(workspace_path, session_id).await?;
            metadata.turn_count = remaining_turns.len();
            metadata.message_count = remaining_turns
                .iter()
                .map(Self::estimate_turn_message_count)
                .sum();
            metadata.tool_call_count = remaining_turns
                .iter()
                .map(DialogTurnData::count_tool_calls)
                .sum();
            metadata.last_active_at = Self::system_time_to_unix_ms(SystemTime::now());
            self.save_session_metadata(workspace_path, &metadata)
                .await?;
        }

        Ok(deleted)
    }

    pub async fn touch_session(&self, workspace_path: &Path, session_id: &str) -> BitFunResult<()> {
        if let Some(mut metadata) = self
            .load_session_metadata(workspace_path, session_id)
            .await?
        {
            metadata.touch();
            self.save_session_metadata(workspace_path, &metadata)
                .await?;
        }
        Ok(())
    }

    // ============ Legacy message persistence ============

    fn legacy_sessions_dir(&self) -> PathBuf {
        self.path_manager.user_data_dir().join("legacy-sessions")
    }

    fn legacy_session_dir(&self, session_id: &str) -> PathBuf {
        self.legacy_sessions_dir().join(session_id)
    }

    async fn ensure_legacy_session_dir(&self, session_id: &str) -> BitFunResult<PathBuf> {
        let dir = self.legacy_session_dir(session_id);
        fs::create_dir_all(&dir).await.map_err(|e| {
            BitFunError::io(format!("Failed to create legacy session directory: {}", e))
        })?;
        Ok(dir)
    }

    /// Append message (JSONL format)
    pub async fn append_message(&self, session_id: &str, message: &Message) -> BitFunResult<()> {
        let dir = self.ensure_legacy_session_dir(session_id).await?;
        let messages_path = dir.join("messages.jsonl");

        let sanitized_message = Self::sanitize_message_for_persistence(message);
        let json = serde_json::to_string(&sanitized_message).map_err(|e| {
            BitFunError::serialization(format!("Failed to serialize message: {}", e))
        })?;

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&messages_path)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to open message file: {}", e)))?;

        file.write_all(json.as_bytes())
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write message: {}", e)))?;
        file.write_all(b"\n")
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write newline: {}", e)))?;

        Ok(())
    }

    /// Load all messages
    pub async fn load_messages(&self, session_id: &str) -> BitFunResult<Vec<Message>> {
        let messages_path = self.legacy_session_dir(session_id).join("messages.jsonl");
        if !messages_path.exists() {
            return Ok(vec![]);
        }

        let file = fs::File::open(&messages_path)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to open message file: {}", e)))?;

        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut messages = Vec::new();

        while let Some(line) = lines
            .next_line()
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read message line: {}", e)))?
        {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<Message>(&line) {
                Ok(message) => messages.push(message),
                Err(e) => warn!("Failed to deserialize message: {}", e),
            }
        }

        Ok(messages)
    }

    /// Clear messages
    pub async fn clear_messages(&self, session_id: &str) -> BitFunResult<()> {
        let messages_path = self.legacy_session_dir(session_id).join("messages.jsonl");
        if messages_path.exists() {
            fs::remove_file(&messages_path)
                .await
                .map_err(|e| BitFunError::io(format!("Failed to delete message file: {}", e)))?;
        }
        Ok(())
    }

    /// Delete messages
    pub async fn delete_messages(&self, session_id: &str) -> BitFunResult<()> {
        self.clear_messages(session_id).await
    }

    // ============ Legacy compressed history persistence ============

    pub async fn append_compressed_message(
        &self,
        session_id: &str,
        message: &Message,
    ) -> BitFunResult<()> {
        let dir = self.ensure_legacy_session_dir(session_id).await?;
        let compressed_path = dir.join("compressed_messages.jsonl");

        let sanitized_message = Self::sanitize_message_for_persistence(message);
        let json = serde_json::to_string(&sanitized_message).map_err(|e| {
            BitFunError::serialization(format!("Failed to serialize compressed message: {}", e))
        })?;

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&compressed_path)
            .await
            .map_err(|e| {
                BitFunError::io(format!("Failed to open compressed message file: {}", e))
            })?;

        file.write_all(json.as_bytes())
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write compressed message: {}", e)))?;
        file.write_all(b"\n")
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write newline: {}", e)))?;

        Ok(())
    }

    pub async fn save_compressed_messages(
        &self,
        session_id: &str,
        messages: &[Message],
    ) -> BitFunResult<()> {
        let dir = self.ensure_legacy_session_dir(session_id).await?;
        let compressed_path = dir.join("compressed_messages.jsonl");

        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&compressed_path)
            .await
            .map_err(|e| {
                BitFunError::io(format!("Failed to open compressed message file: {}", e))
            })?;

        let sanitized_messages = Self::sanitize_messages_for_persistence(messages);
        for message in &sanitized_messages {
            let json = serde_json::to_string(message).map_err(|e| {
                BitFunError::serialization(format!("Failed to serialize compressed message: {}", e))
            })?;

            file.write_all(json.as_bytes()).await.map_err(|e| {
                BitFunError::io(format!("Failed to write compressed message: {}", e))
            })?;
            file.write_all(b"\n")
                .await
                .map_err(|e| BitFunError::io(format!("Failed to write newline: {}", e)))?;
        }

        debug!(
            "Legacy compressed history persisted: session_id={}, message_count={}",
            session_id,
            messages.len()
        );
        Ok(())
    }

    pub async fn load_compressed_messages(
        &self,
        session_id: &str,
    ) -> BitFunResult<Option<Vec<Message>>> {
        let compressed_path = self
            .legacy_session_dir(session_id)
            .join("compressed_messages.jsonl");

        if !compressed_path.exists() {
            return Ok(None);
        }

        let file = fs::File::open(&compressed_path).await.map_err(|e| {
            BitFunError::io(format!("Failed to open compressed message file: {}", e))
        })?;

        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut messages = Vec::new();

        while let Some(line) = lines.next_line().await.map_err(|e| {
            BitFunError::io(format!("Failed to read compressed message line: {}", e))
        })? {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<Message>(&line) {
                Ok(message) => messages.push(message),
                Err(e) => warn!("Failed to deserialize compressed message: {}", e),
            }
        }

        if messages.is_empty() {
            return Ok(None);
        }

        Ok(Some(messages))
    }

    pub async fn delete_compressed_messages(&self, session_id: &str) -> BitFunResult<()> {
        let compressed_path = self
            .legacy_session_dir(session_id)
            .join("compressed_messages.jsonl");

        if compressed_path.exists() {
            fs::remove_file(&compressed_path).await.map_err(|e| {
                BitFunError::io(format!("Failed to delete compressed message file: {}", e))
            })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::PersistenceManager;
    use crate::infrastructure::PathManager;
    use crate::service::session::{
        DialogTurnData, SessionMetadata, SessionTranscriptExportOptions, UserMessageData,
    };
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use uuid::Uuid;

    struct TestWorkspace {
        path: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("bitfun-session-transcript-test-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&path).expect("test workspace should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn transcript_turn_selectors_support_head_and_tail_ranges() {
        let selectors = PersistenceManager::parse_transcript_turn_selectors(&[
            ":1".to_string(),
            "-3:".to_string(),
        ])
        .expect("selectors should parse");

        let selected = PersistenceManager::transcript_select_turn_indices(8, &selectors);

        assert_eq!(selected, vec![0, 5, 6, 7]);
    }

    #[test]
    fn transcript_turn_selectors_deduplicate_and_sort_results() {
        let selectors = PersistenceManager::parse_transcript_turn_selectors(&[
            "4".to_string(),
            "2:5".to_string(),
            "-1".to_string(),
        ])
        .expect("selectors should parse");

        let selected = PersistenceManager::transcript_select_turn_indices(6, &selectors);

        assert_eq!(selected, vec![2, 3, 4, 5]);
    }

    #[test]
    fn transcript_turn_selectors_reject_invalid_syntax() {
        let error = PersistenceManager::parse_transcript_turn_selectors(&["1:2:3".to_string()])
            .expect_err("selector should be rejected");

        assert!(
            error.to_string().contains("Invalid turn selector"),
            "unexpected error: {}",
            error
        );
    }

    #[tokio::test]
    async fn export_session_transcript_handles_first_selected_turn_without_panicking() {
        let workspace = TestWorkspace::new();
        let manager = PersistenceManager::new(Arc::new(PathManager::new().expect("path manager")))
            .expect("persistence manager");
        let session_id = Uuid::new_v4().to_string();

        let metadata = SessionMetadata::new(
            session_id.clone(),
            "Transcript test".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        manager
            .save_session_metadata(workspace.path(), &metadata)
            .await
            .expect("metadata should save");

        let user_message = UserMessageData {
            id: "user-1".to_string(),
            content: "hello transcript".to_string(),
            timestamp: 0,
            metadata: None,
        };
        let mut turn =
            DialogTurnData::new("turn-1".to_string(), 0, session_id.clone(), user_message);
        turn.mark_completed();
        manager
            .save_dialog_turn(workspace.path(), &turn)
            .await
            .expect("turn should save");

        let export = manager
            .export_session_transcript(
                workspace.path(),
                &session_id,
                &SessionTranscriptExportOptions::default(),
            )
            .await
            .expect("transcript export should succeed");

        assert_eq!(export.turn_count, 1);
        assert_eq!(export.index.len(), 1);

        let transcript = std::fs::read_to_string(&export.transcript_path)
            .expect("transcript file should be readable");
        assert!(transcript.contains("## Turn 0"));
        assert!(transcript.contains("hello transcript"));
    }
}
