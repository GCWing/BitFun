//! Persistence Manager
//!
//! Responsible for project-scoped session persistence.

use crate::agentic::core::{
    sanitize_persisted_session_state, CompressionState, Message, MessageContent,
    PersistedSessionStateFile as StoredSessionStateFile, Session, SessionConfig, SessionState,
    SessionSummary,
};
use crate::agentic::memories::db::{MemoryDatabase, MEMORY_PHASE2_GLOBAL_JOB_KEY};
use crate::agentic::memories::external_context::dialog_turn_uses_external_context;
use crate::agentic::session::transcript_render::{render_transcript, transcript_fingerprint};
use crate::agentic::session::{
    CoreSessionStorePort, SessionPromptCache, TokenAnchor, PROMPT_CACHE_SCHEMA_VERSION,
};
use crate::agentic::skill_agent_snapshot::TurnSkillAgentSnapshot;
use crate::infrastructure::PathManager;
use crate::service::config::get_global_config_service;
use crate::service::config::types::{GlobalConfig, MemoryExternalContextPolicy};
use crate::service::remote_ssh::workspace_state::{
    resolve_workspace_session_identity, LOCAL_WORKSPACE_SSH_HOST,
};
use crate::service::session::{
    DialogTurnData, SessionMetadata, SessionTranscriptExport, SessionTranscriptExportOptions,
    TranscriptLineRange, SESSION_STORAGE_SCHEMA_VERSION,
};
use crate::service::workspace_runtime::WorkspaceRuntimeService;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::timing::elapsed_ms_u64;
use bitfun_runtime_ports::{SessionTurnLoadRequest, SessionTurnLoadTiming};
use bitfun_services_core::{
    json_store::{JsonFileStore, JsonFileStoreError},
    session::{
        build_session_metadata as build_persisted_session_metadata, empty_session_metadata_page,
        refresh_session_metadata_from_turns, try_refresh_session_metadata_for_saved_turn,
        SessionMemoryMode, SessionMetadataBuildFacts, SessionMetadataStore,
        SessionMetadataStoreError, SessionStorageLayout,
    },
};
use futures::{stream, StreamExt};
use log::{debug, info, warn};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

pub use bitfun_services_core::session::SessionMetadataPage;

const TRANSCRIPT_SCHEMA_VERSION: u32 = 1;
const COMPRESSION_TRANSCRIPT_SCHEMA_VERSION: u32 = 1;
const COMPRESSION_TRANSCRIPT_CREATE_ATTEMPTS: usize = 32;
const TOKEN_ANCHOR_SCHEMA_VERSION: u32 = 1;
const SESSION_TURN_READ_CONCURRENCY: usize = 4;

static SESSION_PERSISTENCE_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> =
    OnceLock::new();
static SESSION_BRANCH_ALLOCATION_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> =
    OnceLock::new();

async fn memory_pollution_guard_enabled() -> bool {
    match get_global_config_service().await {
        Ok(service) => {
            let config: BitFunResult<GlobalConfig> = service.get_config(None).await;
            config
                .map(|config| {
                    config.memories.generate_memories
                        && config.memories.external_context_policy
                            == MemoryExternalContextPolicy::SkipSession
                })
                .unwrap_or(false)
        }
        Err(_) => false,
    }
}

async fn new_session_memory_mode_from_global_config() -> SessionMemoryMode {
    match get_global_config_service().await {
        Ok(service) => {
            let config: BitFunResult<GlobalConfig> = service.get_config(None).await;
            if config
                .map(|config| config.memories.generate_memories)
                .unwrap_or(true)
            {
                SessionMemoryMode::Enabled
            } else {
                SessionMemoryMode::Disabled
            }
        }
        Err(_) => SessionMemoryMode::Enabled,
    }
}

fn current_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredDialogTurnFile {
    schema_version: u32,
    #[serde(flatten)]
    turn: DialogTurnData,
}

struct ReadTurnPathsResult {
    turns: Vec<DialogTurnData>,
    missing_turn_file_count: usize,
    max_turn_read_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSessionPromptCacheFile {
    schema_version: u32,
    #[serde(flatten)]
    cache: SessionPromptCache,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredTokenAnchorsFile {
    schema_version: u32,
    session_id: String,
    anchors: Vec<TokenAnchor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredTurnContextSnapshotFile {
    schema_version: u32,
    session_id: String,
    turn_index: usize,
    messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredTurnSkillAgentSnapshotFile {
    schema_version: u32,
    session_id: String,
    turn_index: usize,
    snapshot: TurnSkillAgentSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSkillAgentBaselineOverrideFile {
    schema_version: u32,
    session_id: String,
    snapshot: TurnSkillAgentSnapshot,
}

#[derive(Debug, Default)]
struct ContextSnapshotPayloadStats {
    tool_result_count: usize,
    raw_result_string_chars: usize,
    result_for_assistant_chars: usize,
    largest_raw_result_chars: usize,
    largest_raw_result_path: String,
}

fn collect_json_string_stats(
    value: &serde_json::Value,
    path: &str,
    total: &mut usize,
    largest: &mut (usize, String),
) {
    match value {
        serde_json::Value::String(text) => {
            let char_count = text.chars().count();
            *total += char_count;
            if char_count > largest.0 {
                *largest = (char_count, path.to_string());
            }
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_json_string_stats(item, &format!("{}[{}]", path, index), total, largest);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, item) in map {
                let next_path = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{}.{}", path, key)
                };
                collect_json_string_stats(item, &next_path, total, largest);
            }
        }
        _ => {}
    }
}

fn context_snapshot_payload_stats(messages: &[Message]) -> ContextSnapshotPayloadStats {
    let mut stats = ContextSnapshotPayloadStats::default();
    for (message_index, message) in messages.iter().enumerate() {
        let MessageContent::ToolResult {
            tool_name,
            result,
            result_for_assistant,
            ..
        } = &message.content
        else {
            continue;
        };

        stats.tool_result_count += 1;
        if let Some(text) = result_for_assistant.as_deref() {
            stats.result_for_assistant_chars += text.chars().count();
        }

        let mut raw_chars = 0usize;
        let mut largest = (0usize, String::new());
        collect_json_string_stats(
            result,
            &format!("message[{}].{}", message_index, tool_name),
            &mut raw_chars,
            &mut largest,
        );
        stats.raw_result_string_chars += raw_chars;
        if largest.0 > stats.largest_raw_result_chars {
            stats.largest_raw_result_chars = largest.0;
            stats.largest_raw_result_path = largest.1;
        }
    }
    stats
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSessionTranscriptFile {
    schema_version: u32,
    #[serde(flatten)]
    transcript: SessionTranscriptExport,
}

#[derive(Debug, Clone)]
pub(crate) struct CompressionTranscriptArtifact {
    pub(crate) uri: String,
    pub(crate) index_range: TranscriptLineRange,
    pub(crate) transcript_path: PathBuf,
    pub(crate) meta_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompressionTranscriptMetadata {
    schema_version: u32,
    boundary_turn_index: usize,
    short_id: String,
    compression_id: String,
    trigger: String,
    generated_at: u64,
    origin_session_id: String,
    source_fingerprint: String,
    line_count: usize,
    byte_count: usize,
    options: CompressionTranscriptOptionsMetadata,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompressionTranscriptOptionsMetadata {
    tools: bool,
    tool_inputs: bool,
    thinking: bool,
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
    runtime_service: Arc<WorkspaceRuntimeService>,
    #[cfg(test)]
    fail_next_session_state_write: std::sync::Mutex<Option<String>>,
    #[cfg(test)]
    fail_next_session_metadata_write: std::sync::Mutex<Option<String>>,
}

impl PersistenceManager {
    pub fn new(path_manager: Arc<PathManager>) -> BitFunResult<Self> {
        Ok(Self {
            runtime_service: Arc::new(WorkspaceRuntimeService::new(path_manager.clone())),
            path_manager,
            #[cfg(test)]
            fail_next_session_state_write: std::sync::Mutex::new(None),
            #[cfg(test)]
            fail_next_session_metadata_write: std::sync::Mutex::new(None),
        })
    }

    fn validate_session_id(session_id: &str) -> BitFunResult<()> {
        bitfun_core_types::validate_session_id(session_id).map_err(BitFunError::Validation)
    }

    /// Get PathManager reference
    pub fn path_manager(&self) -> &Arc<PathManager> {
        &self.path_manager
    }

    pub fn runtime_service(&self) -> &Arc<WorkspaceRuntimeService> {
        &self.runtime_service
    }

    #[cfg(test)]
    pub(crate) fn fail_next_session_state_write_for_test(&self, session_id: &str) {
        *self
            .fail_next_session_state_write
            .lock()
            .expect("session state fault lock") = Some(session_id.to_string());
    }

    #[cfg(test)]
    pub(crate) fn fail_next_session_metadata_write_for_test(&self, session_id: &str) {
        *self
            .fail_next_session_metadata_write
            .lock()
            .expect("session metadata fault lock") = Some(session_id.to_string());
    }

    /// Resolve the on-disk sessions directory for `workspace_path`.
    ///
    /// Callers may pass either a logical workspace root or an already-resolved
    /// managed sessions directory. Local workspace roots are slugified under
    /// `~/.bitfun/projects/`; already-resolved local/remote sessions
    /// directories are used as-is.
    fn project_sessions_dir(&self, workspace_path: &Path) -> PathBuf {
        if self.is_resolved_sessions_dir(workspace_path) {
            return workspace_path.to_path_buf();
        }
        self.path_manager.project_sessions_dir(workspace_path)
    }

    pub(crate) fn is_resolved_sessions_dir(&self, path: &Path) -> bool {
        CoreSessionStorePort::resolved_sessions_dir_kind(self.path_manager.as_ref(), path).is_some()
    }

    fn state_path(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.session_layout(workspace_path).state_path(session_id)
    }

    fn prompt_cache_path(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.session_layout(workspace_path)
            .prompt_cache_path(session_id)
    }

    fn token_anchors_path(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.session_layout(workspace_path)
            .session_dir(session_id)
            .join("token-anchors.json")
    }

    fn turns_dir(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.session_layout(workspace_path).turns_dir(session_id)
    }

    fn snapshots_dir(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.session_layout(workspace_path)
            .snapshots_dir(session_id)
    }

    fn turn_path(&self, workspace_path: &Path, session_id: &str, turn_index: usize) -> PathBuf {
        self.session_layout(workspace_path)
            .turn_path(session_id, turn_index)
    }

    fn context_snapshot_path(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> PathBuf {
        self.session_layout(workspace_path)
            .context_snapshot_path(session_id, turn_index)
    }

    fn skill_agent_snapshot_path(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> PathBuf {
        self.session_layout(workspace_path)
            .skill_agent_snapshot_path(session_id, turn_index)
    }

    fn skill_agent_baseline_override_path(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> PathBuf {
        self.session_layout(workspace_path)
            .skill_agent_baseline_override_path(session_id)
    }

    fn transcript_path(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.session_layout(workspace_path)
            .transcript_path(session_id)
    }

    fn transcript_meta_path(&self, workspace_path: &Path, session_id: &str) -> PathBuf {
        self.session_layout(workspace_path)
            .transcript_meta_path(session_id)
    }

    pub(crate) fn compression_transcripts_dir(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> PathBuf {
        self.session_layout(workspace_path)
            .compression_transcripts_dir(session_id)
    }

    #[cfg(test)]
    fn index_path(&self, workspace_path: &Path) -> PathBuf {
        self.session_layout(workspace_path).index_path()
    }

    fn session_layout(&self, workspace_path: &Path) -> SessionStorageLayout {
        SessionStorageLayout::new(self.project_sessions_dir(workspace_path))
    }

    pub(crate) fn session_storage_exists(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<bool> {
        Self::validate_session_id(session_id)?;
        Ok(self
            .session_layout(workspace_path)
            .session_dir(session_id)
            .exists())
    }

    fn session_metadata_store(&self, workspace_path: &Path) -> SessionMetadataStore {
        SessionMetadataStore::new(self.project_sessions_dir(workspace_path))
    }

    fn existing_project_sessions_dir(&self, workspace_path: &Path) -> Option<PathBuf> {
        let dir = self.project_sessions_dir(workspace_path);
        dir.exists().then_some(dir)
    }

    async fn ensure_runtime_for_write(&self, workspace_path: &Path) -> BitFunResult<()> {
        if self.is_resolved_sessions_dir(workspace_path) {
            return Ok(());
        }

        self.runtime_service
            .ensure_local_workspace_runtime(workspace_path)
            .await
            .map(|_| ())
    }

    async fn ensure_session_dir(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<PathBuf> {
        self.session_layout(workspace_path)
            .ensure_session_dir(session_id)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create session directory: {}", e)))
    }

    async fn ensure_turns_dir(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<PathBuf> {
        self.session_layout(workspace_path)
            .ensure_turns_dir(session_id)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create turns directory: {}", e)))
    }

    async fn ensure_snapshots_dir(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<PathBuf> {
        self.session_layout(workspace_path)
            .ensure_snapshots_dir(session_id)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create snapshots directory: {}", e)))
    }

    async fn ensure_artifacts_dir(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<PathBuf> {
        self.session_layout(workspace_path)
            .ensure_artifacts_dir(session_id)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create artifacts directory: {}", e)))
    }

    async fn read_json_optional<T: DeserializeOwned>(
        &self,
        path: &Path,
    ) -> BitFunResult<Option<T>> {
        JsonFileStore
            .read_optional(path)
            .await
            .map_err(Self::json_store_error)
    }

    async fn write_json_atomic<T: Serialize>(&self, path: &Path, value: &T) -> BitFunResult<()> {
        JsonFileStore
            .write_atomic(path, value)
            .await
            .map_err(Self::json_store_error)
    }

    async fn get_session_persistence_lock(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> Arc<Mutex<()>> {
        let session_path = self.session_layout(workspace_path).session_dir(session_id);
        let session_path = dunce::canonicalize(&session_path).unwrap_or_else(|_| {
            session_path
                .parent()
                .and_then(|parent| dunce::canonicalize(parent).ok())
                .and_then(|parent| session_path.file_name().map(|name| parent.join(name)))
                .unwrap_or(session_path)
        });
        let registry = SESSION_PERSISTENCE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut registry_guard = registry.lock().await;
        registry_guard.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = registry_guard.get(&session_path).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(Mutex::new(()));
        registry_guard.insert(session_path, Arc::downgrade(&lock));
        lock
    }

    pub(super) async fn get_session_branch_allocation_lock(
        &self,
        workspace_path: &Path,
    ) -> Arc<Mutex<()>> {
        let registry = SESSION_BRANCH_ALLOCATION_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut registry_guard = registry.lock().await;
        registry_guard
            .entry(workspace_path.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn json_store_error(error: JsonFileStoreError) -> BitFunError {
        if error.is_deserialization() {
            BitFunError::Deserialization(error.to_string())
        } else if error.is_serialization() {
            BitFunError::serialization(error.to_string())
        } else {
            BitFunError::io(error.to_string())
        }
    }

    fn session_metadata_store_error(error: SessionMetadataStoreError) -> BitFunError {
        if error.is_deserialization() {
            BitFunError::Deserialization(error.to_string())
        } else if error.is_serialization() {
            BitFunError::serialization(error.to_string())
        } else {
            BitFunError::io(error.to_string())
        }
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
            MessageContent::ToolResult {
                result,
                image_attachments,
                ..
            } => {
                Self::redact_data_url_in_json(result);
                if image_attachments.is_some() {
                    *image_attachments = None;
                }
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

    async fn build_session_metadata(
        &self,
        workspace_path: &Path,
        session: &Session,
        existing: Option<&SessionMetadata>,
    ) -> SessionMetadata {
        let last_active_at = Self::system_time_to_unix_ms(session.last_activity_at);

        let resolved_identity =
            if let Some(workspace_root) = session.config.workspace_path.as_deref() {
                resolve_workspace_session_identity(
                    workspace_root,
                    session.config.remote_connection_id.as_deref(),
                    session.config.remote_ssh_host.as_deref(),
                )
                .await
            } else {
                None
            };

        let workspace_root = resolved_identity
            .as_ref()
            .map(|identity| identity.logical_workspace_path().to_string())
            .or_else(|| session.config.workspace_path.clone())
            .or_else(|| existing.and_then(|value| value.workspace_path.clone()))
            .unwrap_or_else(|| workspace_path.to_string_lossy().to_string());
        let workspace_hostname = resolved_identity
            .as_ref()
            .map(|identity| identity.hostname.clone())
            .or_else(|| existing.and_then(|value| value.workspace_hostname.clone()))
            .or_else(|| {
                if session.config.remote_connection_id.is_some() {
                    session.config.remote_ssh_host.clone()
                } else {
                    Some(LOCAL_WORKSPACE_SSH_HOST.to_string())
                }
            });

        build_persisted_session_metadata(SessionMetadataBuildFacts {
            session_id: &session.session_id,
            session_name: &session.session_name,
            agent_type: &session.agent_type,
            last_user_dialog_agent_type: session.last_user_dialog_agent_type.as_deref(),
            last_submitted_agent_type: session.last_submitted_agent_type.as_deref(),
            created_by: session.created_by.as_deref(),
            session_kind: session.kind,
            model_name: session.config.model_id.as_deref(),
            created_at_ms: Self::system_time_to_unix_ms(session.created_at),
            last_active_at_ms: last_active_at,
            turn_count: session.dialog_turn_ids.len(),
            snapshot_session_id: session.snapshot_session_id.as_deref(),
            workspace_path: &workspace_root,
            workspace_hostname: workspace_hostname.as_deref(),
            new_session_memory_mode: new_session_memory_mode_from_global_config().await,
            existing,
        })
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

    pub async fn list_session_metadata(
        &self,
        workspace_path: &Path,
    ) -> BitFunResult<Vec<SessionMetadata>> {
        if !workspace_path.exists() {
            return Ok(Vec::new());
        }

        if self.existing_project_sessions_dir(workspace_path).is_none() {
            return Ok(Vec::new());
        }

        self.session_metadata_store(workspace_path)
            .list_metadata()
            .await
            .map_err(Self::session_metadata_store_error)
    }

    pub async fn list_session_metadata_page(
        &self,
        workspace_path: &Path,
        cursor: Option<&str>,
        limit: usize,
    ) -> BitFunResult<SessionMetadataPage> {
        if !workspace_path.exists() {
            return Ok(empty_session_metadata_page());
        }

        if self.existing_project_sessions_dir(workspace_path).is_none() {
            return Ok(empty_session_metadata_page());
        }

        self.session_metadata_store(workspace_path)
            .list_metadata_page(cursor, limit)
            .await
            .map_err(Self::session_metadata_store_error)
    }

    pub async fn list_session_metadata_including_internal(
        &self,
        workspace_path: &Path,
    ) -> BitFunResult<Vec<SessionMetadata>> {
        if !workspace_path.exists() {
            return Ok(Vec::new());
        }

        if self.existing_project_sessions_dir(workspace_path).is_none() {
            return Ok(Vec::new());
        }

        self.session_metadata_store(workspace_path)
            .list_metadata_including_internal()
            .await
            .map_err(Self::session_metadata_store_error)
    }

    pub async fn save_session_metadata(
        &self,
        workspace_path: &Path,
        metadata: &SessionMetadata,
    ) -> BitFunResult<()> {
        let persistence_lock = self
            .get_session_persistence_lock(workspace_path, &metadata.session_id)
            .await;
        let _persistence_guard = persistence_lock.lock().await;
        self.save_session_metadata_locked(workspace_path, metadata)
            .await
    }

    async fn save_session_metadata_locked(
        &self,
        workspace_path: &Path,
        metadata: &SessionMetadata,
    ) -> BitFunResult<()> {
        Self::validate_session_id(&metadata.session_id)?;
        self.ensure_runtime_for_write(workspace_path).await?;
        #[cfg(test)]
        {
            let mut fault = self
                .fail_next_session_metadata_write
                .lock()
                .expect("session metadata fault lock");
            if fault.as_deref() == Some(metadata.session_id.as_str()) {
                *fault = None;
                return Err(BitFunError::io("Injected session metadata write failure"));
            }
        }
        self.session_metadata_store(workspace_path)
            .save_metadata(metadata)
            .await
            .map_err(Self::session_metadata_store_error)
    }

    pub async fn create_session_metadata_if_absent(
        &self,
        workspace_path: &Path,
        metadata: &SessionMetadata,
    ) -> BitFunResult<bool> {
        Self::validate_session_id(&metadata.session_id)?;
        self.ensure_runtime_for_write(workspace_path).await?;
        let persistence_lock = self
            .get_session_persistence_lock(workspace_path, &metadata.session_id)
            .await;
        let _persistence_guard = persistence_lock.lock().await;
        if self
            .load_session_metadata(workspace_path, &metadata.session_id)
            .await?
            .is_some()
        {
            return Ok(false);
        }
        self.save_session_metadata_locked(workspace_path, metadata)
            .await?;
        Ok(true)
    }

    pub async fn update_session_metadata(
        &self,
        workspace_path: &Path,
        session_id: &str,
        update: impl FnOnce(&mut SessionMetadata),
    ) -> BitFunResult<()> {
        let updated = self
            .update_session_metadata_if_present(workspace_path, session_id, |metadata| {
                update(metadata);
                Ok(())
            })
            .await?;
        if updated {
            Ok(())
        } else {
            Err(BitFunError::NotFound(format!(
                "Session metadata not found: {}",
                session_id
            )))
        }
    }

    pub async fn update_session_metadata_if_present(
        &self,
        workspace_path: &Path,
        session_id: &str,
        update: impl FnOnce(&mut SessionMetadata) -> BitFunResult<()>,
    ) -> BitFunResult<bool> {
        Self::validate_session_id(session_id)?;
        self.ensure_runtime_for_write(workspace_path).await?;
        let persistence_lock = self
            .get_session_persistence_lock(workspace_path, session_id)
            .await;
        let _persistence_guard = persistence_lock.lock().await;
        self.update_session_metadata_if_present_locked(workspace_path, session_id, update)
            .await
    }

    async fn update_session_metadata_if_present_locked(
        &self,
        workspace_path: &Path,
        session_id: &str,
        update: impl FnOnce(&mut SessionMetadata) -> BitFunResult<()>,
    ) -> BitFunResult<bool> {
        let Some(mut metadata) = self
            .load_session_metadata(workspace_path, session_id)
            .await?
        else {
            return Ok(false);
        };
        update(&mut metadata)?;
        self.save_session_metadata_locked(workspace_path, &metadata)
            .await?;
        Ok(true)
    }

    pub async fn set_session_memory_mode(
        &self,
        workspace_path: &Path,
        session_id: &str,
        mode: SessionMemoryMode,
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
        let persistence_lock = self
            .get_session_persistence_lock(workspace_path, session_id)
            .await;
        let _persistence_guard = persistence_lock.lock().await;
        let mut metadata = self
            .load_session_metadata(workspace_path, session_id)
            .await?
            .ok_or_else(|| {
                BitFunError::NotFound(format!("Session metadata not found: {}", session_id))
            })?;
        metadata.memory_mode = mode;
        self.save_session_metadata_locked(workspace_path, &metadata)
            .await
    }

    pub async fn mark_session_memory_mode_polluted(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
        let persistence_lock = self
            .get_session_persistence_lock(workspace_path, session_id)
            .await;
        let _persistence_guard = persistence_lock.lock().await;
        let mut metadata = self
            .load_session_metadata(workspace_path, session_id)
            .await?
            .ok_or_else(|| {
                BitFunError::NotFound(format!("Session metadata not found: {}", session_id))
            })?;
        let should_enqueue_phase2 = matches!(
            metadata.memory_mode,
            SessionMemoryMode::Enabled | SessionMemoryMode::Polluted
        );
        if metadata.memory_mode == SessionMemoryMode::Enabled {
            metadata.memory_mode = SessionMemoryMode::Polluted;
            self.save_session_metadata_locked(workspace_path, &metadata)
                .await?;
        }
        if should_enqueue_phase2 {
            self.enqueue_phase2_if_session_selected(session_id, current_unix_secs())
                .await?;
        }
        Ok(())
    }

    async fn enqueue_phase2_if_session_selected(
        &self,
        session_id: &str,
        input_watermark: i64,
    ) -> BitFunResult<()> {
        let db = MemoryDatabase::new(self.path_manager.clone());
        db.initialize().await?;
        if db.phase2_selected_for_session(session_id).await? {
            db.enqueue_phase2_job(MEMORY_PHASE2_GLOBAL_JOB_KEY, input_watermark)
                .await?;
        }
        Ok(())
    }

    pub async fn load_session_metadata(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Option<SessionMetadata>> {
        Self::validate_session_id(session_id)?;
        self.session_metadata_store(workspace_path)
            .load_metadata(session_id)
            .await
            .map_err(Self::session_metadata_store_error)
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

    pub async fn load_prompt_cache(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Option<SessionPromptCache>> {
        Self::validate_session_id(session_id)?;
        Ok(self
            .read_json_optional::<StoredSessionPromptCacheFile>(
                &self.prompt_cache_path(workspace_path, session_id),
            )
            .await?
            .map(|file| file.cache))
    }

    pub async fn save_prompt_cache(
        &self,
        workspace_path: &Path,
        session_id: &str,
        cache: &SessionPromptCache,
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
        self.ensure_runtime_for_write(workspace_path).await?;
        self.ensure_session_dir(workspace_path, session_id).await?;

        self.write_json_atomic(
            &self.prompt_cache_path(workspace_path, session_id),
            &StoredSessionPromptCacheFile {
                schema_version: PROMPT_CACHE_SCHEMA_VERSION,
                cache: cache.clone(),
            },
        )
        .await
    }

    pub async fn delete_prompt_cache(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
        match fs::remove_file(self.prompt_cache_path(workspace_path, session_id)).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(BitFunError::io(format!(
                "Failed to delete prompt cache for session {}: {}",
                session_id, error
            ))),
        }
    }

    pub async fn load_token_anchors(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Option<Vec<TokenAnchor>>> {
        Self::validate_session_id(session_id)?;
        Ok(self
            .read_json_optional::<StoredTokenAnchorsFile>(
                &self.token_anchors_path(workspace_path, session_id),
            )
            .await?
            .map(|file| file.anchors))
    }

    pub async fn save_token_anchors(
        &self,
        workspace_path: &Path,
        session_id: &str,
        anchors: &[TokenAnchor],
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
        self.ensure_runtime_for_write(workspace_path).await?;
        self.ensure_session_dir(workspace_path, session_id).await?;

        self.write_json_atomic(
            &self.token_anchors_path(workspace_path, session_id),
            &StoredTokenAnchorsFile {
                schema_version: TOKEN_ANCHOR_SCHEMA_VERSION,
                session_id: session_id.to_string(),
                anchors: anchors.to_vec(),
            },
        )
        .await
    }

    pub async fn delete_token_anchors(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
        match fs::remove_file(self.token_anchors_path(workspace_path, session_id)).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(BitFunError::io(format!(
                "Failed to delete token anchors for session {}: {}",
                session_id, error
            ))),
        }
    }

    // ============ Turn context snapshot (sent to model)============

    pub async fn save_turn_context_snapshot(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
        messages: &[Message],
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
        self.ensure_runtime_for_write(workspace_path).await?;
        self.ensure_snapshots_dir(workspace_path, session_id)
            .await?;

        let snapshot = StoredTurnContextSnapshotFile {
            schema_version: SESSION_STORAGE_SCHEMA_VERSION,
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
        Self::validate_session_id(session_id)?;
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
        Self::validate_session_id(session_id)?;
        let started_at = Instant::now();
        let dir = self.snapshots_dir(workspace_path, session_id);
        if !dir.exists() {
            return Ok(None);
        }

        let scan_started_at = Instant::now();
        let mut latest: Option<usize> = None;
        let mut snapshot_file_count = 0usize;
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
                snapshot_file_count += 1;
                latest = Some(latest.map(|value| value.max(index)).unwrap_or(index));
            }
        }
        let scan_duration = scan_started_at.elapsed();

        let Some(turn_index) = latest else {
            return Ok(None);
        };

        let load_started_at = Instant::now();
        let Some(messages) = self
            .load_turn_context_snapshot(workspace_path, session_id, turn_index)
            .await?
        else {
            return Ok(None);
        };
        let load_duration = load_started_at.elapsed();
        let total_duration = started_at.elapsed();

        if total_duration >= Duration::from_millis(80) || snapshot_file_count >= 10 {
            let payload_stats = context_snapshot_payload_stats(&messages);
            debug!(
                "Loaded latest context snapshot: session_id={} turn_index={} snapshot_file_count={} scan_duration_ms={} load_duration_ms={} total_duration_ms={} message_count={} tool_result_count={} raw_result_string_chars={} result_for_assistant_chars={} largest_raw_result_chars={} largest_raw_result_path={}",
                session_id,
                turn_index,
                snapshot_file_count,
                scan_duration.as_millis(),
                load_duration.as_millis(),
                total_duration.as_millis(),
                messages.len(),
                payload_stats.tool_result_count,
                payload_stats.raw_result_string_chars,
                payload_stats.result_for_assistant_chars,
                payload_stats.largest_raw_result_chars,
                payload_stats.largest_raw_result_path
            );
        }

        Ok(Some((turn_index, messages)))
    }

    pub async fn save_turn_skill_agent_snapshot(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
        snapshot: &TurnSkillAgentSnapshot,
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
        self.ensure_runtime_for_write(workspace_path).await?;
        self.ensure_snapshots_dir(workspace_path, session_id)
            .await?;

        self.write_json_atomic(
            &self.skill_agent_snapshot_path(workspace_path, session_id, turn_index),
            &StoredTurnSkillAgentSnapshotFile {
                schema_version: SESSION_STORAGE_SCHEMA_VERSION,
                session_id: session_id.to_string(),
                turn_index,
                snapshot: snapshot.clone(),
            },
        )
        .await
    }

    pub async fn load_turn_skill_agent_snapshot(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<Option<TurnSkillAgentSnapshot>> {
        Self::validate_session_id(session_id)?;
        let stored = self
            .read_json_optional::<StoredTurnSkillAgentSnapshotFile>(
                &self.skill_agent_snapshot_path(workspace_path, session_id, turn_index),
            )
            .await?;
        Ok(stored.map(|value| value.snapshot))
    }

    pub async fn delete_turn_skill_agent_snapshots_from(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
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
            let Some(index_str) = stem.strip_prefix("skill-agent-") else {
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

    pub async fn save_skill_agent_baseline_override_snapshot(
        &self,
        workspace_path: &Path,
        session_id: &str,
        snapshot: &TurnSkillAgentSnapshot,
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
        self.ensure_runtime_for_write(workspace_path).await?;
        self.ensure_snapshots_dir(workspace_path, session_id)
            .await?;

        self.write_json_atomic(
            &self.skill_agent_baseline_override_path(workspace_path, session_id),
            &StoredSkillAgentBaselineOverrideFile {
                schema_version: SESSION_STORAGE_SCHEMA_VERSION,
                session_id: session_id.to_string(),
                snapshot: snapshot.clone(),
            },
        )
        .await
    }

    pub async fn load_skill_agent_baseline_override_snapshot(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Option<TurnSkillAgentSnapshot>> {
        Self::validate_session_id(session_id)?;
        let stored = self
            .read_json_optional::<StoredSkillAgentBaselineOverrideFile>(
                &self.skill_agent_baseline_override_path(workspace_path, session_id),
            )
            .await?;
        Ok(stored.map(|value| value.snapshot))
    }

    pub async fn delete_turn_context_snapshots_from(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
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
            let index_str = if let Some(index) = stem.strip_prefix("context-") {
                index
            } else if let Some(index) = stem.strip_prefix("skill-agent-") {
                index
            } else {
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

    /// Persist a newly created session without overwriting an existing session ID.
    ///
    /// The final session directory is created exclusively so this manager owns any
    /// cleanup required by a failed first write. This also prevents a losing
    /// creator in another runtime or process from deleting the winning session.
    pub(crate) async fn create_session_if_absent(
        &self,
        workspace_path: &Path,
        session: &Session,
    ) -> BitFunResult<()> {
        Self::validate_session_id(&session.session_id)?;
        self.ensure_runtime_for_write(workspace_path).await?;

        let sessions_dir = self.project_sessions_dir(workspace_path);
        fs::create_dir_all(&sessions_dir).await.map_err(|error| {
            BitFunError::io(format!(
                "Failed to create sessions directory {}: {}",
                sessions_dir.display(),
                error
            ))
        })?;
        let persistence_lock = self
            .get_session_persistence_lock(workspace_path, &session.session_id)
            .await;
        let _persistence_guard = persistence_lock.lock().await;
        let session_dir = self
            .session_layout(workspace_path)
            .session_dir(&session.session_id);
        match fs::create_dir(&session_dir).await {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                return Err(BitFunError::Validation(format!(
                    "Persisted session ID already exists: {}",
                    session.session_id
                )));
            }
            Err(error) => {
                return Err(BitFunError::io(format!(
                    "Failed to claim session directory {}: {}",
                    session_dir.display(),
                    error
                )));
            }
        }

        if let Err(error) = self
            .save_session_files_locked(workspace_path, session)
            .await
        {
            if let Err(cleanup_error) = self
                .session_metadata_store(workspace_path)
                .delete_session_dir_and_index(&session.session_id)
                .await
            {
                warn!(
                    "Failed to clean up partial session persistence: session_id={}, error={}",
                    session.session_id, cleanup_error
                );
                return Err(BitFunError::SessionCreateCleanupRequired {
                    session_id: session.session_id.clone(),
                    error: error.to_string(),
                    cleanup_error: cleanup_error.to_string(),
                });
            }
            return Err(error);
        }

        Ok(())
    }

    /// Save session
    pub async fn save_session(&self, workspace_path: &Path, session: &Session) -> BitFunResult<()> {
        Self::validate_session_id(&session.session_id)?;
        self.ensure_runtime_for_write(workspace_path).await?;
        let persistence_lock = self
            .get_session_persistence_lock(workspace_path, &session.session_id)
            .await;
        let _persistence_guard = persistence_lock.lock().await;
        self.ensure_session_dir(workspace_path, &session.session_id)
            .await?;
        self.save_session_files_locked(workspace_path, session)
            .await
    }

    async fn save_session_files_locked(
        &self,
        workspace_path: &Path,
        session: &Session,
    ) -> BitFunResult<()> {
        let existing_metadata = self
            .load_session_metadata(workspace_path, &session.session_id)
            .await?;
        let metadata = self
            .build_session_metadata(workspace_path, session, existing_metadata.as_ref())
            .await;
        self.save_session_metadata_locked(workspace_path, &metadata)
            .await?;

        let state = StoredSessionStateFile {
            schema_version: SESSION_STORAGE_SCHEMA_VERSION,
            config: session.config.clone(),
            snapshot_session_id: session.snapshot_session_id.clone(),
            last_user_dialog_agent_type: session.last_user_dialog_agent_type.clone(),
            last_submitted_agent_type: session.last_submitted_agent_type.clone(),
            compression_state: session.compression_state.clone(),
            runtime_state: sanitize_persisted_session_state(&session.state),
        };
        #[cfg(test)]
        {
            let mut fault = self
                .fail_next_session_state_write
                .lock()
                .expect("session state fault lock");
            if fault.as_deref() == Some(session.session_id.as_str()) {
                *fault = None;
                return Err(BitFunError::io("Injected session state write failure"));
            }
        }
        self.save_stored_session_state(workspace_path, &session.session_id, &state)
            .await
    }

    /// Load session
    pub async fn load_session(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Session> {
        Self::validate_session_id(session_id)?;
        let (session, _) = self
            .load_session_with_turns(workspace_path, session_id)
            .await?;
        Ok(session)
    }

    fn build_session_from_persisted_parts(
        metadata: SessionMetadata,
        stored_state: Option<StoredSessionStateFile>,
        turns: &[DialogTurnData],
    ) -> Session {
        let mut config = stored_state
            .as_ref()
            .map(|value| value.config.clone())
            .unwrap_or_default();
        if config.workspace_path.is_none() {
            config.workspace_path = metadata.workspace_path.clone();
        }
        if config.remote_ssh_host.is_none() {
            config.remote_ssh_host = metadata
                .workspace_hostname
                .clone()
                .filter(|host| host != LOCAL_WORKSPACE_SSH_HOST && host != "_unresolved");
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
            .map(|value| sanitize_persisted_session_state(&value.runtime_state))
            .unwrap_or(SessionState::Idle);
        let created_at = Self::unix_ms_to_system_time(metadata.created_at);
        let last_activity_at = Self::unix_ms_to_system_time(metadata.last_active_at);
        let dialog_turn_ids = turns.iter().map(|turn| turn.turn_id.clone()).collect();

        Session {
            session_id: metadata.session_id.clone(),
            session_name: metadata.session_name.clone(),
            agent_type: metadata.agent_type.clone(),
            last_user_dialog_agent_type: stored_state
                .as_ref()
                .and_then(|value| value.last_user_dialog_agent_type.clone())
                .or_else(|| metadata.last_user_dialog_agent_type.clone()),
            last_submitted_agent_type: stored_state
                .as_ref()
                .and_then(|value| value.last_submitted_agent_type.clone())
                .or_else(|| metadata.last_submitted_agent_type.clone()),
            created_by: metadata.created_by.clone(),
            kind: metadata.session_kind,
            snapshot_session_id: stored_state
                .as_ref()
                .and_then(|value| value.snapshot_session_id.clone())
                .or(metadata.snapshot_session_id.clone()),
            dialog_turn_ids,
            state: runtime_state,
            config,
            compression_state,
            created_at,
            updated_at: last_activity_at,
            last_activity_at,
        }
    }

    /// Load session and return the persisted turns read while rebuilding the session header.
    pub async fn load_session_with_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<DialogTurnData>)> {
        Self::validate_session_id(session_id)?;
        self.load_session_with_turns_timed(workspace_path, session_id)
            .await
            .map(|(session, turns, _)| (session, turns))
    }

    pub async fn load_session_with_turns_timed(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<DialogTurnData>, SessionTurnLoadTiming)> {
        Self::validate_session_id(session_id)?;
        let request = SessionTurnLoadRequest {
            workspace_path: workspace_path.to_path_buf(),
            session_id: session_id.to_string(),
            tail_turn_count: None,
        };
        let started_at = Instant::now();
        let metadata_started_at = Instant::now();
        let metadata = self
            .load_session_metadata(&request.workspace_path, &request.session_id)
            .await?
            .ok_or_else(|| {
                BitFunError::NotFound(format!(
                    "Session metadata not found: {}",
                    request.session_id
                ))
            })?;
        let metadata_duration_ms = elapsed_ms_u64(metadata_started_at);

        let state_started_at = Instant::now();
        let stored_state = self
            .load_stored_session_state(&request.workspace_path, &request.session_id)
            .await?;
        let state_duration_ms = elapsed_ms_u64(state_started_at);

        let scan_started_at = Instant::now();
        let indexed_paths = self
            .list_indexed_turn_paths(&request.workspace_path, &request.session_id)
            .await?;
        let scan_duration_ms = elapsed_ms_u64(scan_started_at);

        let read_started_at = Instant::now();
        let turn_file_count = indexed_paths.len();
        let read_result = self.read_turn_paths(indexed_paths).await?;
        let read_duration_ms = elapsed_ms_u64(read_started_at);
        let missing_turn_file_count = read_result.missing_turn_file_count;
        let max_turn_read_duration_ms = read_result.max_turn_read_duration_ms;
        let turns = read_result.turns;

        let build_started_at = Instant::now();
        let session = Self::build_session_from_persisted_parts(metadata, stored_state, &turns);
        let build_session_duration_ms = elapsed_ms_u64(build_started_at);
        let total_duration_ms = elapsed_ms_u64(started_at);

        if total_duration_ms >= 80 || turn_file_count >= 50 {
            debug!(
                "Loaded session turns: session_id={} turn_count={} turn_file_count={} missing_turn_file_count={} metadata_duration_ms={} state_duration_ms={} scan_duration_ms={} read_duration_ms={} max_turn_read_duration_ms={} build_session_duration_ms={} total_duration_ms={}",
                request.session_id,
                turns.len(),
                turn_file_count,
                missing_turn_file_count,
                metadata_duration_ms,
                state_duration_ms,
                scan_duration_ms,
                read_duration_ms,
                max_turn_read_duration_ms,
                build_session_duration_ms,
                total_duration_ms
            );
        }

        let timing = SessionTurnLoadTiming {
            requested_tail_turn_count: None,
            loaded_turn_count: turns.len(),
            total_turn_count: turn_file_count,
            turn_file_count,
            missing_turn_file_count,
            fast_path: false,
            metadata_duration_ms,
            state_duration_ms,
            scan_duration_ms,
            read_duration_ms,
            max_turn_read_duration_ms,
            build_session_duration_ms,
            total_duration_ms,
        };

        Ok((session, turns, timing))
    }

    pub async fn load_session_with_tail_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
        tail_turn_count: usize,
    ) -> BitFunResult<(Session, Vec<DialogTurnData>, usize)> {
        Self::validate_session_id(session_id)?;
        self.load_session_with_tail_turns_timed(workspace_path, session_id, tail_turn_count)
            .await
            .map(|(session, turns, total_turn_count, _)| (session, turns, total_turn_count))
    }

    pub async fn load_session_with_tail_turns_timed(
        &self,
        workspace_path: &Path,
        session_id: &str,
        tail_turn_count: usize,
    ) -> BitFunResult<(Session, Vec<DialogTurnData>, usize, SessionTurnLoadTiming)> {
        Self::validate_session_id(session_id)?;
        let request = SessionTurnLoadRequest {
            workspace_path: workspace_path.to_path_buf(),
            session_id: session_id.to_string(),
            tail_turn_count: Some(tail_turn_count),
        };
        let started_at = Instant::now();
        let metadata_started_at = Instant::now();
        let metadata = self
            .load_session_metadata(&request.workspace_path, &request.session_id)
            .await?
            .ok_or_else(|| {
                BitFunError::NotFound(format!(
                    "Session metadata not found: {}",
                    request.session_id
                ))
            })?;
        let metadata_duration = metadata_started_at.elapsed();

        let state_started_at = Instant::now();
        let stored_state = self
            .load_stored_session_state(&request.workspace_path, &request.session_id)
            .await?;
        let state_duration = state_started_at.elapsed();

        let fast_path_started_at = Instant::now();
        let fast_path_turns = self
            .read_metadata_tail_turns(
                &request.workspace_path,
                &request.session_id,
                metadata.turn_count,
                tail_turn_count,
            )
            .await?;
        let fast_path_duration = fast_path_started_at.elapsed();

        let (
            turns,
            total_turn_count,
            scan_duration,
            read_duration,
            fast_path,
            missing_turn_file_count,
            max_turn_read_duration_ms,
        ) = if let Some(turns) = fast_path_turns {
            (
                turns.turns,
                metadata.turn_count,
                Duration::ZERO,
                fast_path_duration,
                true,
                turns.missing_turn_file_count,
                turns.max_turn_read_duration_ms,
            )
        } else {
            let scan_started_at = Instant::now();
            let indexed_paths = self
                .list_indexed_turn_paths(&request.workspace_path, &request.session_id)
                .await?;
            let scan_duration = scan_started_at.elapsed();
            let total_turn_count = indexed_paths.len();
            let start = indexed_paths.len().saturating_sub(tail_turn_count);
            let selected_paths = indexed_paths.into_iter().skip(start).collect::<Vec<_>>();

            let read_started_at = Instant::now();
            let read_result = self.read_turn_paths(selected_paths).await?;
            let read_duration = read_started_at.elapsed();

            (
                read_result.turns,
                total_turn_count,
                scan_duration,
                read_duration,
                false,
                read_result.missing_turn_file_count,
                read_result.max_turn_read_duration_ms,
            )
        };
        let build_started_at = Instant::now();
        let session = Self::build_session_from_persisted_parts(metadata, stored_state, &turns);
        let build_session_duration_ms = elapsed_ms_u64(build_started_at);
        let total_duration = started_at.elapsed();

        if total_duration >= Duration::from_millis(40) || total_turn_count >= 50 {
            debug!(
                "Loaded session tail view: session_id={} turn_count={} requested_count={} total_turn_count={} missing_turn_file_count={} fast_path={} metadata_duration_ms={} state_duration_ms={} scan_duration_ms={} read_duration_ms={} max_turn_read_duration_ms={} build_session_duration_ms={} total_duration_ms={}",
                request.session_id,
                turns.len(),
                request.tail_turn_count.unwrap_or(tail_turn_count),
                total_turn_count,
                missing_turn_file_count,
                fast_path,
                metadata_duration.as_millis(),
                state_duration.as_millis(),
                scan_duration.as_millis(),
                read_duration.as_millis(),
                max_turn_read_duration_ms,
                build_session_duration_ms,
                total_duration.as_millis()
            );
        }

        let timing = SessionTurnLoadTiming {
            requested_tail_turn_count: request.tail_turn_count,
            loaded_turn_count: turns.len(),
            total_turn_count,
            turn_file_count: total_turn_count,
            missing_turn_file_count,
            fast_path,
            metadata_duration_ms: metadata_duration.as_millis() as u64,
            state_duration_ms: state_duration.as_millis() as u64,
            scan_duration_ms: scan_duration.as_millis() as u64,
            read_duration_ms: read_duration.as_millis() as u64,
            max_turn_read_duration_ms,
            build_session_duration_ms,
            total_duration_ms: total_duration.as_millis() as u64,
        };

        Ok((session, turns, total_turn_count, timing))
    }

    /// Save session state
    pub async fn save_session_state(
        &self,
        workspace_path: &Path,
        session_id: &str,
        state: &SessionState,
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
        self.ensure_runtime_for_write(workspace_path).await?;
        let persistence_lock = self
            .get_session_persistence_lock(workspace_path, session_id)
            .await;
        let _persistence_guard = persistence_lock.lock().await;
        let mut stored_state = self
            .load_stored_session_state(workspace_path, session_id)
            .await?
            .unwrap_or(StoredSessionStateFile {
                schema_version: SESSION_STORAGE_SCHEMA_VERSION,
                config: SessionConfig {
                    workspace_path: None,
                    ..Default::default()
                },
                snapshot_session_id: None,
                last_user_dialog_agent_type: None,
                last_submitted_agent_type: None,
                compression_state: CompressionState::default(),
                runtime_state: SessionState::Idle,
            });
        stored_state.schema_version = SESSION_STORAGE_SCHEMA_VERSION;
        stored_state.runtime_state = sanitize_persisted_session_state(state);
        self.save_stored_session_state(workspace_path, session_id, &stored_state)
            .await
    }

    /// Delete session
    pub async fn delete_session(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
        let persistence_lock = self
            .get_session_persistence_lock(workspace_path, session_id)
            .await;
        let _persistence_guard = persistence_lock.lock().await;
        self.session_metadata_store(workspace_path)
            .delete_session_dir_and_index(session_id)
            .await
            .map_err(Self::session_metadata_store_error)?;
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
                .map(|value| sanitize_persisted_session_state(&value.runtime_state))
                .unwrap_or(SessionState::Idle);

            summaries.push(SessionSummary {
                session_id: metadata.session_id,
                session_name: metadata.session_name,
                agent_type: metadata.agent_type,
                last_user_dialog_agent_type: metadata.last_user_dialog_agent_type,
                last_submitted_agent_type: metadata.last_submitted_agent_type,
                created_by: metadata.created_by,
                kind: metadata.session_kind,
                turn_count: metadata.turn_count,
                created_at: Self::unix_ms_to_system_time(metadata.created_at),
                last_activity_at: Self::unix_ms_to_system_time(metadata.last_active_at),
                state,
            });
        }

        summaries.sort_by_key(|summary| std::cmp::Reverse(summary.last_activity_at));
        Ok(summaries)
    }

    pub async fn save_dialog_turn(
        &self,
        workspace_path: &Path,
        turn: &DialogTurnData,
    ) -> BitFunResult<()> {
        Self::validate_session_id(&turn.session_id)?;
        let save_started_at = Instant::now();
        self.ensure_runtime_for_write(workspace_path).await?;
        let persistence_lock = self
            .get_session_persistence_lock(workspace_path, &turn.session_id)
            .await;
        let _persistence_guard = persistence_lock.lock().await;
        let mut metadata = self
            .load_session_metadata(workspace_path, &turn.session_id)
            .await?
            .ok_or_else(|| {
                BitFunError::NotFound(format!("Session metadata not found: {}", turn.session_id))
            })?;
        self.ensure_turns_dir(workspace_path, &turn.session_id)
            .await?;

        let previous_turn = match self
            .load_dialog_turn(workspace_path, &turn.session_id, turn.turn_index)
            .await
        {
            Ok(turn) => turn,
            Err(error) => {
                warn!(
                    "Failed to load existing dialog turn before save; falling back to full metadata refresh: session_id={} turn_index={} error={}",
                    turn.session_id,
                    turn.turn_index,
                    error
                );
                None
            }
        };
        let previous_turn_load_failed = previous_turn.is_none()
            && self
                .turn_path(workspace_path, &turn.session_id, turn.turn_index)
                .exists();

        let file = StoredDialogTurnFile {
            schema_version: SESSION_STORAGE_SCHEMA_VERSION,
            turn: turn.clone(),
        };
        let write_started_at = Instant::now();
        self.write_json_atomic(
            &self.turn_path(workspace_path, &turn.session_id, turn.turn_index),
            &file,
        )
        .await?;
        let write_duration = write_started_at.elapsed();

        let last_active_at = turn
            .end_time
            .unwrap_or_else(|| Self::system_time_to_unix_ms(SystemTime::now()));
        let mut metadata_refresh_mode = "incremental";
        let workspace_path_text = workspace_path.to_string_lossy();
        if previous_turn_load_failed
            || !try_refresh_session_metadata_for_saved_turn(
                &mut metadata,
                workspace_path_text.as_ref(),
                previous_turn.as_ref(),
                turn,
                last_active_at,
            )
        {
            metadata_refresh_mode = "full_scan";
            let turns = self
                .load_session_turns(workspace_path, &turn.session_id)
                .await?;
            refresh_session_metadata_from_turns(
                &mut metadata,
                workspace_path_text.as_ref(),
                &turns,
                last_active_at,
            );
        }
        let uses_external_context = dialog_turn_uses_external_context(turn);
        let should_pollute_memory = memory_pollution_guard_enabled().await && uses_external_context;
        let should_enqueue_phase2_for_pollution = should_pollute_memory
            && matches!(
                metadata.memory_mode,
                SessionMemoryMode::Enabled | SessionMemoryMode::Polluted
            );
        if should_pollute_memory && metadata.memory_mode == SessionMemoryMode::Enabled {
            metadata.memory_mode = SessionMemoryMode::Polluted;
        }

        let metadata_started_at = Instant::now();
        self.save_session_metadata_locked(workspace_path, &metadata)
            .await?;
        if should_enqueue_phase2_for_pollution {
            self.enqueue_phase2_if_session_selected(&turn.session_id, current_unix_secs())
                .await?;
        }
        let metadata_duration = metadata_started_at.elapsed();
        let total_duration = save_started_at.elapsed();
        if total_duration >= Duration::from_millis(80) || metadata_refresh_mode == "full_scan" {
            debug!(
                "Saved dialog turn: session_id={} turn_index={} metadata_refresh={} write_duration_ms={} metadata_duration_ms={} total_duration_ms={}",
                turn.session_id,
                turn.turn_index,
                metadata_refresh_mode,
                write_duration.as_millis(),
                metadata_duration.as_millis(),
                total_duration.as_millis()
            );
        }

        Ok(())
    }

    pub async fn load_dialog_turn(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<Option<DialogTurnData>> {
        Self::validate_session_id(session_id)?;
        Ok(self
            .read_json_optional::<StoredDialogTurnFile>(&self.turn_path(
                workspace_path,
                session_id,
                turn_index,
            ))
            .await?
            .map(|file| file.turn))
    }

    async fn list_indexed_turn_paths(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Vec<(usize, PathBuf)>> {
        self.session_layout(workspace_path)
            .list_indexed_turn_paths(session_id)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to list dialog turn files: {}", e)))
    }

    async fn read_turn_paths(
        &self,
        indexed_paths: Vec<(usize, PathBuf)>,
    ) -> BitFunResult<ReadTurnPathsResult> {
        let mut turns = Vec::with_capacity(indexed_paths.len());
        let mut missing_turn_file_count = 0usize;
        let mut max_turn_read_duration_ms = 0u64;
        let reads = stream::iter(indexed_paths.into_iter().map(|(_, path)| {
            let manager = self;
            async move {
                let started_at = Instant::now();
                let result = manager
                    .read_json_optional::<StoredDialogTurnFile>(&path)
                    .await;
                (result, elapsed_ms_u64(started_at))
            }
        }))
        .buffered(SESSION_TURN_READ_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;

        for (result, duration_ms) in reads {
            max_turn_read_duration_ms = max_turn_read_duration_ms.max(duration_ms);
            if let Some(file) = result? {
                turns.push(file.turn);
            } else {
                missing_turn_file_count += 1;
            }
        }

        Ok(ReadTurnPathsResult {
            turns,
            missing_turn_file_count,
            max_turn_read_duration_ms,
        })
    }

    async fn read_metadata_tail_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
        total_turn_count: usize,
        requested_count: usize,
    ) -> BitFunResult<Option<ReadTurnPathsResult>> {
        if requested_count == 0 {
            return Ok(Some(ReadTurnPathsResult {
                turns: Vec::new(),
                missing_turn_file_count: 0,
                max_turn_read_duration_ms: 0,
            }));
        }
        if total_turn_count == 0 {
            return Ok(None);
        }

        let start = total_turn_count.saturating_sub(requested_count);
        let indexed_paths = (start..total_turn_count)
            .map(|index| (index, self.turn_path(workspace_path, session_id, index)))
            .collect::<Vec<_>>();
        let result = self.read_turn_paths(indexed_paths).await?;
        if result.missing_turn_file_count > 0 {
            return Ok(None);
        }

        Ok(Some(result))
    }

    pub async fn load_session_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Vec<DialogTurnData>> {
        Self::validate_session_id(session_id)?;
        let started_at = Instant::now();
        let scan_started_at = Instant::now();
        let indexed_paths = self
            .list_indexed_turn_paths(workspace_path, session_id)
            .await?;
        let scan_duration = scan_started_at.elapsed();

        let read_started_at = Instant::now();
        let turn_file_count = indexed_paths.len();
        let read_result = self.read_turn_paths(indexed_paths).await?;
        let read_duration = read_started_at.elapsed();
        let missing_turn_file_count = read_result.missing_turn_file_count;
        let max_turn_read_duration_ms = read_result.max_turn_read_duration_ms;
        let turns = read_result.turns;
        let total_duration = started_at.elapsed();
        if total_duration >= Duration::from_millis(80) || turn_file_count >= 50 {
            debug!(
                "Loaded session turns: session_id={} turn_count={} turn_file_count={} missing_turn_file_count={} scan_duration_ms={} read_duration_ms={} max_turn_read_duration_ms={} total_duration_ms={}",
                session_id,
                turns.len(),
                turn_file_count,
                missing_turn_file_count,
                scan_duration.as_millis(),
                read_duration.as_millis(),
                max_turn_read_duration_ms,
                total_duration.as_millis()
            );
        }

        Ok(turns)
    }

    pub async fn load_session_tail_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
        count: usize,
    ) -> BitFunResult<Vec<DialogTurnData>> {
        Self::validate_session_id(session_id)?;
        if count == 0 {
            return Ok(Vec::new());
        }

        let started_at = Instant::now();
        let metadata_started_at = Instant::now();
        let metadata = self
            .load_session_metadata(workspace_path, session_id)
            .await?;
        let metadata_duration = metadata_started_at.elapsed();

        let fast_path_started_at = Instant::now();
        let fast_path_turns = if let Some(metadata) = metadata.as_ref() {
            self.read_metadata_tail_turns(workspace_path, session_id, metadata.turn_count, count)
                .await?
        } else {
            None
        };
        let fast_path_duration = fast_path_started_at.elapsed();

        let (
            turns,
            turn_file_count,
            scan_duration,
            read_duration,
            fast_path,
            missing_turn_file_count,
            max_turn_read_duration_ms,
        ) = if let Some(turns) = fast_path_turns {
            let turn_file_count = metadata
                .as_ref()
                .map(|metadata| metadata.turn_count)
                .unwrap_or(turns.turns.len());
            (
                turns.turns,
                turn_file_count,
                Duration::ZERO,
                fast_path_duration,
                true,
                turns.missing_turn_file_count,
                turns.max_turn_read_duration_ms,
            )
        } else {
            let scan_started_at = Instant::now();
            let indexed_paths = self
                .list_indexed_turn_paths(workspace_path, session_id)
                .await?;
            let scan_duration = scan_started_at.elapsed();
            let turn_file_count = indexed_paths.len();
            let start = indexed_paths.len().saturating_sub(count);
            let selected_paths = indexed_paths.into_iter().skip(start).collect::<Vec<_>>();

            let read_started_at = Instant::now();
            let read_result = self.read_turn_paths(selected_paths).await?;
            let read_duration = read_started_at.elapsed();

            (
                read_result.turns,
                turn_file_count,
                scan_duration,
                read_duration,
                false,
                read_result.missing_turn_file_count,
                read_result.max_turn_read_duration_ms,
            )
        };
        let total_duration = started_at.elapsed();
        if total_duration >= Duration::from_millis(40) || turn_file_count >= 50 {
            debug!(
                "Loaded session tail turns: session_id={} turn_count={} requested_count={} turn_file_count={} missing_turn_file_count={} fast_path={} metadata_duration_ms={} scan_duration_ms={} read_duration_ms={} max_turn_read_duration_ms={} total_duration_ms={}",
                session_id,
                turns.len(),
                count,
                turn_file_count,
                missing_turn_file_count,
                fast_path,
                metadata_duration.as_millis(),
                scan_duration.as_millis(),
                read_duration.as_millis(),
                max_turn_read_duration_ms,
                total_duration.as_millis()
            );
        }

        Ok(turns)
    }

    pub async fn delete_dialog_turns_from(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<()> {
        Self::validate_session_id(session_id)?;
        let persistence_lock = self
            .get_session_persistence_lock(workspace_path, session_id)
            .await;
        let _persistence_guard = persistence_lock.lock().await;
        if !self.turns_dir(workspace_path, session_id).exists() {
            return Ok(());
        }

        self.session_layout(workspace_path)
            .delete_indexed_turn_paths_from(session_id, turn_index)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to delete dialog turn files: {}", e)))?;

        if self
            .load_session_metadata(workspace_path, session_id)
            .await?
            .is_some()
        {
            let turns = self.load_session_turns(workspace_path, session_id).await?;
            let workspace_path_text = workspace_path.to_string_lossy();
            self.update_session_metadata_if_present_locked(
                workspace_path,
                session_id,
                |metadata| {
                    refresh_session_metadata_from_turns(
                        metadata,
                        workspace_path_text.as_ref(),
                        &turns,
                        Self::system_time_to_unix_ms(SystemTime::now()),
                    );
                    Ok(())
                },
            )
            .await?;
        }

        Ok(())
    }

    pub async fn load_recent_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
        count: usize,
    ) -> BitFunResult<Vec<DialogTurnData>> {
        Self::validate_session_id(session_id)?;
        let turns = self.load_session_turns(workspace_path, session_id).await?;
        let start = turns.len().saturating_sub(count);
        Ok(turns[start..].to_vec())
    }

    fn compression_transcript_boundary_from_file_name(file_name: &str) -> Option<usize> {
        let stem = file_name
            .strip_suffix(".meta.json")
            .or_else(|| file_name.strip_suffix(".txt"))?;
        let (boundary, short_id) = stem.rsplit_once('-')?;
        if short_id.len() != 4
            || !short_id
                .bytes()
                .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
            || boundary.is_empty()
            || !boundary.bytes().all(|value| value.is_ascii_digit())
        {
            return None;
        }
        boundary.parse().ok()
    }

    pub(crate) async fn create_compression_transcript(
        &self,
        workspace_path: &Path,
        session_id: &str,
        boundary_turn_index: usize,
        compression_id: &str,
        trigger: &str,
    ) -> BitFunResult<Option<CompressionTranscriptArtifact>> {
        Self::validate_session_id(session_id)?;
        let all_turns = self.load_session_turns(workspace_path, session_id).await?;
        let selected_indices = all_turns
            .iter()
            .enumerate()
            .filter_map(|(index, turn)| (turn.turn_index <= boundary_turn_index).then_some(index))
            .collect::<Vec<_>>();
        if selected_indices.is_empty() {
            return Ok(None);
        }

        let options = SessionTranscriptExportOptions {
            tools: true,
            tool_inputs: true,
            thinking: false,
            turns: Some(vec![format!("0:{}", boundary_turn_index.saturating_add(1))]),
        };
        let selected_turns = selected_indices
            .iter()
            .map(|&index| all_turns[index].clone())
            .collect::<Vec<_>>();
        let source_fingerprint = transcript_fingerprint(session_id, &selected_turns, &options)?;
        let rendered = render_transcript(&all_turns, &selected_indices, &options);
        let transcript_content = rendered.lines.join("\n");
        let transcript_bytes = transcript_content.as_bytes();
        let generated_at = Self::system_time_to_unix_ms(SystemTime::now());

        let layout = self.session_layout(workspace_path);
        layout
            .ensure_compression_transcripts_dir(session_id)
            .await
            .map_err(|error| {
                BitFunError::io(format!(
                    "Failed to create compression transcript directory: {}",
                    error
                ))
            })?;

        for _ in 0..COMPRESSION_TRANSCRIPT_CREATE_ATTEMPTS {
            let short_id = uuid::Uuid::new_v4().simple().to_string()[..4].to_string();
            let stem = format!("{}-{}", boundary_turn_index, short_id);
            let transcript_path = layout.compression_transcript_path(session_id, &stem);
            let meta_path = layout.compression_transcript_meta_path(session_id, &stem);
            let metadata = CompressionTranscriptMetadata {
                schema_version: COMPRESSION_TRANSCRIPT_SCHEMA_VERSION,
                boundary_turn_index,
                short_id,
                compression_id: compression_id.to_string(),
                trigger: trigger.to_string(),
                generated_at,
                origin_session_id: session_id.to_string(),
                source_fingerprint: source_fingerprint.clone(),
                line_count: rendered.lines.len(),
                byte_count: transcript_bytes.len(),
                options: CompressionTranscriptOptionsMetadata {
                    tools: true,
                    tool_inputs: true,
                    thinking: false,
                },
            };
            let mut metadata_bytes = serde_json::to_vec_pretty(&metadata).map_err(|error| {
                BitFunError::serialization(format!(
                    "Failed to serialize compression transcript metadata: {}",
                    error
                ))
            })?;
            metadata_bytes.push(b'\n');

            let mut transcript_file = match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&transcript_path)
                .await
            {
                Ok(file) => file,
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(BitFunError::io(format!(
                        "Failed to reserve compression transcript {}: {}",
                        transcript_path.display(),
                        error
                    )))
                }
            };

            let mut meta_file = match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&meta_path)
                .await
            {
                Ok(file) => file,
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    let _ = fs::remove_file(&transcript_path).await;
                    continue;
                }
                Err(error) => {
                    let _ = fs::remove_file(&transcript_path).await;
                    return Err(BitFunError::io(format!(
                        "Failed to reserve compression transcript metadata {}: {}",
                        meta_path.display(),
                        error
                    )));
                }
            };

            let write_result = async {
                transcript_file.write_all(transcript_bytes).await?;
                transcript_file.flush().await?;
                meta_file.write_all(&metadata_bytes).await?;
                meta_file.flush().await
            }
            .await;
            if let Err(error) = write_result {
                drop(transcript_file);
                drop(meta_file);
                let _ = fs::remove_file(&transcript_path).await;
                let _ = fs::remove_file(&meta_path).await;
                return Err(BitFunError::io(format!(
                    "Failed to write compression transcript pair: {}",
                    error
                )));
            }

            let uri = bitfun_agent_tools::build_bitfun_current_session_uri(&format!(
                "artifacts/compression-transcripts/{}.txt",
                stem
            ))
            .map_err(|error| BitFunError::Validation(error.to_string()))?;
            return Ok(Some(CompressionTranscriptArtifact {
                uri,
                index_range: rendered.index_range.clone(),
                transcript_path,
                meta_path,
            }));
        }

        Err(BitFunError::io(format!(
            "Failed to allocate a unique compression transcript name after {} attempts",
            COMPRESSION_TRANSCRIPT_CREATE_ATTEMPTS
        )))
    }

    pub(crate) async fn delete_compression_transcripts_from(
        &self,
        workspace_path: &Path,
        session_id: &str,
        start_turn_index: usize,
    ) -> BitFunResult<usize> {
        Self::validate_session_id(session_id)?;
        let dir = self.compression_transcripts_dir(workspace_path, session_id);
        if !dir.exists() {
            return Ok(0);
        }
        let mut deleted = 0usize;
        let mut entries = fs::read_dir(&dir).await.map_err(|error| {
            BitFunError::io(format!(
                "Failed to read compression transcript directory {}: {}",
                dir.display(),
                error
            ))
        })?;
        while let Some(entry) = entries.next_entry().await.map_err(|error| {
            BitFunError::io(format!(
                "Failed to enumerate compression transcript directory {}: {}",
                dir.display(),
                error
            ))
        })? {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if Self::compression_transcript_boundary_from_file_name(&file_name)
                .is_some_and(|boundary| boundary >= start_turn_index)
            {
                fs::remove_file(entry.path()).await.map_err(|error| {
                    BitFunError::io(format!(
                        "Failed to delete compression transcript artifact {}: {}",
                        entry.path().display(),
                        error
                    ))
                })?;
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    pub(crate) async fn copy_compression_transcripts_through(
        &self,
        workspace_path: &Path,
        source_session_id: &str,
        target_session_id: &str,
        end_turn_index: usize,
    ) -> BitFunResult<usize> {
        Self::validate_session_id(source_session_id)?;
        Self::validate_session_id(target_session_id)?;
        let source_dir = self.compression_transcripts_dir(workspace_path, source_session_id);
        if !source_dir.exists() {
            return Ok(0);
        }
        let target_dir = self
            .session_layout(workspace_path)
            .ensure_compression_transcripts_dir(target_session_id)
            .await
            .map_err(|error| {
                BitFunError::io(format!(
                    "Failed to create branched compression transcript directory: {}",
                    error
                ))
            })?;
        let mut copied = 0usize;
        let mut entries = fs::read_dir(&source_dir).await.map_err(|error| {
            BitFunError::io(format!(
                "Failed to read source compression transcript directory {}: {}",
                source_dir.display(),
                error
            ))
        })?;
        while let Some(entry) = entries.next_entry().await.map_err(|error| {
            BitFunError::io(format!(
                "Failed to enumerate source compression transcripts: {}",
                error
            ))
        })? {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if Self::compression_transcript_boundary_from_file_name(&file_name)
                .is_some_and(|boundary| boundary <= end_turn_index)
            {
                fs::copy(entry.path(), target_dir.join(&file_name))
                    .await
                    .map_err(|error| {
                        BitFunError::io(format!(
                            "Failed to copy compression transcript artifact {}: {}",
                            entry.path().display(),
                            error
                        ))
                    })?;
                copied += 1;
            }
        }
        Ok(copied)
    }

    pub async fn export_session_transcript(
        &self,
        workspace_path: &Path,
        session_id: &str,
        options: &SessionTranscriptExportOptions,
    ) -> BitFunResult<SessionTranscriptExport> {
        Self::validate_session_id(session_id)?;
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

        let source_fingerprint = transcript_fingerprint(session_id, &turns, &normalized_options)?;
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
        let rendered = render_transcript(&all_turns, &selected_indices, &normalized_options);
        let lines = rendered.lines;
        let index_range = rendered.index_range;
        let index = rendered.index;

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
        Self::validate_session_id(session_id)?;
        let persistence_lock = self
            .get_session_persistence_lock(workspace_path, session_id)
            .await;
        let _persistence_guard = persistence_lock.lock().await;
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

        if self
            .load_session_metadata(workspace_path, session_id)
            .await?
            .is_some()
        {
            let remaining_turns = self.load_session_turns(workspace_path, session_id).await?;
            let workspace_path_text = workspace_path.to_string_lossy();
            self.update_session_metadata_if_present_locked(
                workspace_path,
                session_id,
                |metadata| {
                    refresh_session_metadata_from_turns(
                        metadata,
                        workspace_path_text.as_ref(),
                        &remaining_turns,
                        Self::system_time_to_unix_ms(SystemTime::now()),
                    );
                    Ok(())
                },
            )
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
        Self::validate_session_id(session_id)?;
        let persistence_lock = self
            .get_session_persistence_lock(workspace_path, session_id)
            .await;
        let _persistence_guard = persistence_lock.lock().await;
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

        if self
            .load_session_metadata(workspace_path, session_id)
            .await?
            .is_some()
        {
            let remaining_turns = self.load_session_turns(workspace_path, session_id).await?;
            let workspace_path_text = workspace_path.to_string_lossy();
            self.update_session_metadata_if_present_locked(
                workspace_path,
                session_id,
                |metadata| {
                    refresh_session_metadata_from_turns(
                        metadata,
                        workspace_path_text.as_ref(),
                        &remaining_turns,
                        Self::system_time_to_unix_ms(SystemTime::now()),
                    );
                    Ok(())
                },
            )
            .await?;
        }

        Ok(deleted)
    }

    pub async fn touch_session(&self, workspace_path: &Path, session_id: &str) -> BitFunResult<()> {
        self.update_session_metadata_if_present(workspace_path, session_id, |metadata| {
            metadata.touch();
            Ok(())
        })
        .await
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        context_snapshot_payload_stats, current_unix_secs, PersistenceManager, StoredDialogTurnFile,
    };
    use crate::agentic::core::{Message, Session, SessionConfig, SessionKind, ToolResult};
    use crate::agentic::memories::db::{MemoryDatabase, MemoryRow, MEMORY_PHASE2_GLOBAL_JOB_KEY};
    use crate::agentic::session::{TokenAnchor, TokenAnchorInput};
    use crate::agentic::skill_agent_snapshot::{
        AgentSnapshotEntry, SkillSnapshotEntry, TurnSkillAgentSnapshot,
    };
    use crate::infrastructure::PathManager;
    use crate::service::session::{
        DialogTurnData, ModelRoundData, SessionMemoryMode, SessionMetadata, SessionRelationship,
        SessionRelationshipKind, SessionTranscriptExportOptions, StoredSessionIndexFile,
        TextItemData, UserMessageData,
    };
    use crate::BitFunError;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Instant;
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

        fn path_manager(&self) -> Arc<PathManager> {
            Arc::new(PathManager::with_user_root_for_tests(
                self.path.join("user-root"),
            ))
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[tokio::test]
    async fn unsafe_session_ids_are_rejected_before_turn_path_resolution() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");

        let error = manager
            .load_session_turns(workspace.path(), "../another-project/session")
            .await
            .expect_err("path-like session id must be rejected");

        assert!(error.to_string().contains("session_id"), "{error}");
    }

    #[tokio::test]
    async fn concurrent_first_session_persistence_keeps_the_winner() {
        let workspace = TestWorkspace::new();
        let manager_a = Arc::new(
            PersistenceManager::new(workspace.path_manager()).expect("first persistence manager"),
        );
        let manager_b = Arc::new(
            PersistenceManager::new(workspace.path_manager()).expect("second persistence manager"),
        );
        let session_id = format!("concurrent-session-{}", Uuid::new_v4());
        let config = SessionConfig {
            workspace_path: Some(workspace.path().to_string_lossy().to_string()),
            ..Default::default()
        };
        let session_a = Session::new_with_id(
            session_id.clone(),
            "First contender".to_string(),
            "agent".to_string(),
            config.clone(),
        );
        let session_b = Session::new_with_id(
            session_id.clone(),
            "Second contender".to_string(),
            "agent".to_string(),
            config,
        );
        let workspace_path = workspace.path().to_path_buf();
        let barrier = Arc::new(tokio::sync::Barrier::new(3));

        let first = tokio::spawn({
            let manager = manager_a.clone();
            let barrier = barrier.clone();
            let workspace_path = workspace_path.clone();
            async move {
                barrier.wait().await;
                let result = manager
                    .create_session_if_absent(&workspace_path, &session_a)
                    .await;
                ("First contender", result)
            }
        });
        let second = tokio::spawn({
            let manager = manager_b.clone();
            let barrier = barrier.clone();
            let workspace_path = workspace_path.clone();
            async move {
                barrier.wait().await;
                let result = manager
                    .create_session_if_absent(&workspace_path, &session_b)
                    .await;
                ("Second contender", result)
            }
        });
        barrier.wait().await;

        let first = first.await.expect("first contender should finish");
        let second = second.await.expect("second contender should finish");
        let outcomes = [first, second];
        let winner = outcomes
            .iter()
            .find_map(|(name, result)| result.is_ok().then_some(*name))
            .expect("one contender must persist the session");
        let failures = outcomes
            .iter()
            .filter_map(|(_, result)| result.as_ref().err())
            .collect::<Vec<_>>();

        assert_eq!(failures.len(), 1, "exactly one contender must fail");
        assert!(matches!(failures[0], BitFunError::Validation(_)));
        let persisted = manager_a
            .load_session(workspace.path(), &session_id)
            .await
            .expect("the winning session must remain persisted");
        assert_eq!(persisted.session_name, winner);
    }

    #[tokio::test]
    async fn token_anchors_save_load_and_delete_roundtrip() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
        let session_id = format!("session-{}", Uuid::new_v4());
        let messages = vec![
            Message::system("system".to_string()),
            Message::user("hello".to_string()),
        ];
        let anchor = TokenAnchor::from_request_prefix(
            TokenAnchorInput {
                session_id: session_id.clone(),
                turn_id: "turn".to_string(),
                round_id: "round".to_string(),
                model_id: "model".to_string(),
                input_tokens: 100,
                system_tokens_at_anchor: 10,
                tool_tokens_at_anchor: 20,
                prepended_reminder_tokens_at_anchor: 0,
            },
            &messages,
        );

        manager
            .save_token_anchors(workspace.path(), &session_id, std::slice::from_ref(&anchor))
            .await
            .expect("token anchors should save");
        let loaded = manager
            .load_token_anchors(workspace.path(), &session_id)
            .await
            .expect("token anchors should load")
            .expect("token anchor file should exist");

        assert_eq!(loaded, vec![anchor]);

        manager
            .delete_token_anchors(workspace.path(), &session_id)
            .await
            .expect("token anchors should delete");
        let loaded_after_delete = manager
            .load_token_anchors(workspace.path(), &session_id)
            .await
            .expect("deleted token anchor load should succeed");

        assert!(loaded_after_delete.is_none());
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
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
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

    #[tokio::test]
    async fn load_session_tail_turns_returns_latest_turns_in_chronological_order() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
        let session_id = Uuid::new_v4().to_string();
        let metadata = SessionMetadata::new(
            session_id.clone(),
            "Tail turns test".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        manager
            .save_session_metadata(workspace.path(), &metadata)
            .await
            .expect("metadata should save");

        for index in 0..5 {
            let user_message = UserMessageData {
                id: format!("user-{index}"),
                content: format!("prompt {index}"),
                timestamp: index as u64,
                metadata: None,
            };
            let mut turn = DialogTurnData::new(
                format!("turn-{index}"),
                index,
                session_id.clone(),
                user_message,
            );
            turn.mark_completed();
            manager
                .save_dialog_turn(workspace.path(), &turn)
                .await
                .expect("turn should save");
        }

        let tail = manager
            .load_session_tail_turns(workspace.path(), &session_id, 2)
            .await
            .expect("tail turns should load");

        let turn_indices = tail.iter().map(|turn| turn.turn_index).collect::<Vec<_>>();
        let prompts = tail
            .iter()
            .map(|turn| turn.user_message.content.as_str())
            .collect::<Vec<_>>();

        assert_eq!(turn_indices, vec![3, 4]);
        assert_eq!(prompts, vec!["prompt 3", "prompt 4"]);

        let (_session, view_tail, total_turn_count) = manager
            .load_session_with_tail_turns(workspace.path(), &session_id, 2)
            .await
            .expect("tail view should load");
        let view_turn_indices = view_tail
            .iter()
            .map(|turn| turn.turn_index)
            .collect::<Vec<_>>();

        assert_eq!(view_turn_indices, vec![3, 4]);
        assert_eq!(total_turn_count, 5);
    }

    #[tokio::test]
    async fn load_session_tail_turns_uses_metadata_turn_count_as_normal_path_boundary() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
        let session_id = Uuid::new_v4().to_string();
        let metadata = SessionMetadata::new(
            session_id.clone(),
            "Tail turns boundary test".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        manager
            .save_session_metadata(workspace.path(), &metadata)
            .await
            .expect("metadata should save");

        for index in 0..5 {
            let user_message = UserMessageData {
                id: format!("user-{index}"),
                content: format!("prompt {index}"),
                timestamp: index as u64,
                metadata: None,
            };
            let mut turn = DialogTurnData::new(
                format!("turn-{index}"),
                index,
                session_id.clone(),
                user_message,
            );
            turn.mark_completed();
            manager
                .save_dialog_turn(workspace.path(), &turn)
                .await
                .expect("turn should save");
        }

        let orphan_user_message = UserMessageData {
            id: "user-99".to_string(),
            content: "orphan prompt".to_string(),
            timestamp: 99,
            metadata: None,
        };
        let mut orphan_turn = DialogTurnData::new(
            "turn-99".to_string(),
            99,
            session_id.clone(),
            orphan_user_message,
        );
        orphan_turn.mark_completed();
        let orphan_file = StoredDialogTurnFile {
            schema_version: super::SESSION_STORAGE_SCHEMA_VERSION,
            turn: orphan_turn,
        };
        let orphan_json =
            serde_json::to_string_pretty(&orphan_file).expect("orphan turn should serialize");
        std::fs::write(
            manager.turn_path(workspace.path(), &session_id, 99),
            orphan_json,
        )
        .expect("orphan turn should be written");

        let tail = manager
            .load_session_tail_turns(workspace.path(), &session_id, 2)
            .await
            .expect("tail turns should load");

        let turn_indices = tail.iter().map(|turn| turn.turn_index).collect::<Vec<_>>();
        let prompts = tail
            .iter()
            .map(|turn| turn.user_message.content.as_str())
            .collect::<Vec<_>>();

        assert_eq!(turn_indices, vec![3, 4]);
        assert_eq!(prompts, vec!["prompt 3", "prompt 4"]);

        let (_session, view_tail, total_turn_count) = manager
            .load_session_with_tail_turns(workspace.path(), &session_id, 2)
            .await
            .expect("tail view should load");
        let view_turn_indices = view_tail
            .iter()
            .map(|turn| turn.turn_index)
            .collect::<Vec<_>>();

        assert_eq!(view_turn_indices, vec![3, 4]);
        assert_eq!(total_turn_count, 5);
    }

    #[tokio::test]
    async fn load_session_with_turns_returns_session_and_persisted_turns() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
        let session_id = Uuid::new_v4().to_string();
        let session = Session::new_with_id(
            session_id.clone(),
            "Load once".to_string(),
            "agent".to_string(),
            SessionConfig {
                workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );

        manager
            .save_session(workspace.path(), &session)
            .await
            .expect("session should save");

        let user_message = UserMessageData {
            id: "user-1".to_string(),
            content: "hello once".to_string(),
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

        let (loaded_session, loaded_turns) = manager
            .load_session_with_turns(workspace.path(), &session_id)
            .await
            .expect("session and turns should load together");

        assert_eq!(loaded_session.dialog_turn_ids, vec!["turn-1".to_string()]);
        assert_eq!(loaded_turns.len(), 1);
        assert_eq!(loaded_turns[0].turn_id, "turn-1");
    }

    fn user_message(content: &str) -> UserMessageData {
        UserMessageData {
            id: format!("user-{}", content),
            content: content.to_string(),
            timestamp: 0,
            metadata: None,
        }
    }

    fn text_item(id: &str, content: &str) -> TextItemData {
        TextItemData {
            id: id.to_string(),
            content: content.to_string(),
            is_streaming: false,
            timestamp: 0,
            is_markdown: true,
            order_index: None,
            is_subagent_item: None,
            parent_task_tool_id: None,
            subagent_session_id: None,
            status: None,
            attempt_id: None,
            attempt_index: None,
        }
    }

    fn round_with_text(turn_id: &str, text_items: Vec<TextItemData>) -> ModelRoundData {
        ModelRoundData {
            id: format!("round-{}", turn_id),
            turn_id: turn_id.to_string(),
            round_index: 0,
            round_group_id: None,
            timestamp: 0,
            text_items,
            tool_items: Vec::new(),
            thinking_items: Vec::new(),
            start_time: 0,
            end_time: Some(0),
            duration_ms: Some(0),
            provider_id: None,
            model_config_id: None,
            effective_model_name: None,
            first_chunk_ms: None,
            first_visible_output_ms: None,
            stream_duration_ms: None,
            attempt_count: None,
            attempt_diagnostics: vec![],
            failure_category: None,
            token_details: None,
            status: "completed".to_string(),
        }
    }

    #[test]
    fn compression_transcript_file_name_parser_is_strict() {
        assert_eq!(
            PersistenceManager::compression_transcript_boundary_from_file_name("12-a3f9.txt"),
            Some(12)
        );
        assert_eq!(
            PersistenceManager::compression_transcript_boundary_from_file_name("12-a3f9.meta.json"),
            Some(12)
        );

        for invalid in [
            "12-A3F9.txt",
            "12-a3f.txt",
            "12-a3f90.txt",
            "12-a3f9.txt.bak",
            "-1-a3f9.txt",
            "a3f9.txt",
            "12-a3f9.json",
        ] {
            assert_eq!(
                PersistenceManager::compression_transcript_boundary_from_file_name(invalid),
                None,
                "unexpectedly accepted {invalid}"
            );
        }
    }

    #[tokio::test]
    async fn compression_transcripts_are_stable_unique_and_rollback_aware() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
        let session_id = Uuid::new_v4().to_string();
        let session = Session::new_with_id(
            session_id.clone(),
            "Compression transcripts".to_string(),
            "agent".to_string(),
            SessionConfig {
                workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );
        manager
            .save_session(workspace.path(), &session)
            .await
            .expect("session should save");

        for turn_index in 0..=2 {
            let turn_id = format!("turn-{}", turn_index);
            let mut turn = DialogTurnData::new(
                turn_id.clone(),
                turn_index,
                session_id.clone(),
                user_message(&format!("user {}", turn_index)),
            );
            let mut current_text = text_item(
                &format!("text-{}", turn_index),
                &format!("assistant {}", turn_index),
            );
            let mut text_items = Vec::new();
            if turn_index == 0 {
                let mut superseded_text = text_item("text-0-attempt-1", "superseded assistant 0");
                superseded_text.attempt_id = Some(format!("{turn_id}:attempt:1"));
                superseded_text.attempt_index = Some(1);
                text_items.push(superseded_text);

                current_text.attempt_id = Some(format!("{turn_id}:attempt:2"));
                current_text.attempt_index = Some(2);
            }
            text_items.push(current_text);

            let mut round = round_with_text(&turn_id, text_items);
            if turn_index == 0 {
                round.attempt_count = Some(2);
            }
            turn.model_rounds.push(round);
            turn.mark_completed();
            manager
                .save_dialog_turn(workspace.path(), &turn)
                .await
                .expect("turn should save");
        }

        let first = manager
            .create_compression_transcript(
                workspace.path(),
                &session_id,
                1,
                "compression-first",
                "auto",
            )
            .await
            .expect("first transcript should create")
            .expect("persisted turns should produce a transcript");
        let second = manager
            .create_compression_transcript(
                workspace.path(),
                &session_id,
                2,
                "compression-second",
                "manual",
            )
            .await
            .expect("second transcript should create")
            .expect("persisted turns should produce a transcript");

        assert_ne!(first.transcript_path, second.transcript_path);
        assert!(first
            .uri
            .starts_with("bitfun://current-session/artifacts/compression-transcripts/1-"));
        assert!(second
            .uri
            .starts_with("bitfun://current-session/artifacts/compression-transcripts/2-"));
        assert_eq!(first.index_range.start_line, 1);
        assert_eq!(first.index_range.end_line, 3);
        assert_eq!(second.index_range.start_line, 1);
        assert_eq!(second.index_range.end_line, 4);
        assert!(first.transcript_path.exists());
        assert!(first.meta_path.exists());
        assert!(second.transcript_path.exists());
        assert!(second.meta_path.exists());
        assert_ne!(
            first.transcript_path,
            manager.transcript_path(workspace.path(), &session_id)
        );

        let transcript = std::fs::read_to_string(&first.transcript_path)
            .expect("compression transcript should be readable");
        assert!(transcript.contains("## Turn 0\n[user]\nuser 0\n[/user]"));
        assert!(transcript.contains("[assistant step=0]\nassistant 0\n[/assistant]"));
        assert!(!transcript.contains("superseded assistant 0"));
        assert!(transcript.contains("## Turn 1"));
        assert!(!transcript.contains("## Turn 2"));
        assert!(!transcript.contains("[assistant_round"));
        assert!(!transcript.contains("[text]"));
        let metadata: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&first.meta_path).expect("metadata should be readable"),
        )
        .expect("metadata should be valid JSON");
        assert_eq!(metadata["boundaryTurnIndex"], 1);
        assert_eq!(metadata["compressionId"], "compression-first");
        assert_eq!(metadata["options"]["tools"], true);
        assert_eq!(metadata["options"]["toolInputs"], true);
        assert_eq!(metadata["options"]["thinking"], false);

        std::fs::write(
            manager
                .compression_transcripts_dir(workspace.path(), &session_id)
                .join("not-owned.txt"),
            "keep",
        )
        .expect("malformed artifact should save");
        let deleted = manager
            .delete_compression_transcripts_from(workspace.path(), &session_id, 2)
            .await
            .expect("rollback cleanup should succeed");
        assert_eq!(deleted, 2);
        assert!(first.transcript_path.exists());
        assert!(first.meta_path.exists());
        assert!(!second.transcript_path.exists());
        assert!(!second.meta_path.exists());
        assert!(manager
            .compression_transcripts_dir(workspace.path(), &session_id)
            .join("not-owned.txt")
            .exists());
    }

    #[tokio::test]
    async fn metadata_patch_and_turn_save_share_one_read_modify_write_lock() {
        let workspace = TestWorkspace::new();
        let manager = Arc::new(
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager"),
        );
        let session_id = Uuid::new_v4().to_string();
        let session = Session::new_with_id(
            session_id.clone(),
            "Concurrent metadata".to_string(),
            "agentic".to_string(),
            SessionConfig {
                workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );
        manager
            .save_session(workspace.path(), &session)
            .await
            .expect("session should save");

        let metadata_lock = manager
            .get_session_persistence_lock(workspace.path(), &session_id)
            .await;
        let metadata_guard = metadata_lock.lock().await;
        let workspace_path = workspace.path().to_path_buf();

        let patch_task = tokio::spawn({
            let manager = manager.clone();
            let workspace_path = workspace_path.clone();
            let session_id = session_id.clone();
            async move {
                manager
                    .update_session_metadata(&workspace_path, &session_id, |metadata| {
                        metadata.agent_type = "Plan".to_string();
                    })
                    .await
            }
        });

        let mut turn = DialogTurnData::new(
            "turn-0".to_string(),
            0,
            session_id.clone(),
            user_message("concurrent turn"),
        );
        turn.mark_completed();
        let turn_task = tokio::spawn({
            let manager = manager.clone();
            let workspace_path = workspace_path.clone();
            async move { manager.save_dialog_turn(&workspace_path, &turn).await }
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!patch_task.is_finished());
        assert!(!turn_task.is_finished());
        drop(metadata_guard);

        patch_task
            .await
            .expect("metadata patch task should join")
            .expect("metadata patch should save");
        turn_task
            .await
            .expect("turn save task should join")
            .expect("turn should save");

        let metadata = manager
            .load_session_metadata(&workspace_path, &session_id)
            .await
            .expect("metadata should load")
            .expect("metadata should exist");
        assert_eq!(metadata.agent_type, "Plan");
        assert_eq!(metadata.turn_count, 1);
    }

    #[tokio::test]
    async fn save_dialog_turn_updates_metadata_without_scanning_unrelated_turn_files() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
        let session_id = Uuid::new_v4().to_string();
        let session = Session::new_with_id(
            session_id.clone(),
            "Incremental metadata".to_string(),
            "agent".to_string(),
            SessionConfig {
                workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );

        manager
            .save_session(workspace.path(), &session)
            .await
            .expect("session should save");

        let mut turn_0 = DialogTurnData::new(
            "turn-0".to_string(),
            0,
            session_id.clone(),
            user_message("first"),
        );
        turn_0.model_rounds.push(round_with_text(
            "turn-0",
            vec![text_item("text-0", "first response")],
        ));
        turn_0.mark_completed();
        manager
            .save_dialog_turn(workspace.path(), &turn_0)
            .await
            .expect("first turn should save");

        let mut turn_1 = DialogTurnData::new(
            "turn-1".to_string(),
            1,
            session_id.clone(),
            user_message("second"),
        );
        turn_1.model_rounds.push(round_with_text(
            "turn-1",
            vec![text_item("text-1", "second response")],
        ));
        turn_1.mark_completed();
        manager
            .save_dialog_turn(workspace.path(), &turn_1)
            .await
            .expect("second turn should save");

        std::fs::write(
            manager.turn_path(workspace.path(), &session_id, 0),
            "{ not valid json",
        )
        .expect("old turn file should be replaceable for test");

        turn_1.model_rounds[0]
            .text_items
            .push(text_item("text-2", "additional response"));
        manager
            .save_dialog_turn(workspace.path(), &turn_1)
            .await
            .expect("saving current turn should not scan unrelated old turn files");

        let metadata = manager
            .load_session_metadata(workspace.path(), &session_id)
            .await
            .expect("metadata should load")
            .expect("metadata should exist");
        assert_eq!(metadata.turn_count, 2);
        assert_eq!(metadata.message_count, 5);
    }

    #[tokio::test]
    async fn turn_deletion_waits_for_the_session_metadata_transaction() {
        let workspace = TestWorkspace::new();
        let manager = Arc::new(
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager"),
        );
        let session_id = Uuid::new_v4().to_string();
        let session = Session::new_with_id(
            session_id.clone(),
            "Transactional deletion".to_string(),
            "agentic".to_string(),
            SessionConfig {
                workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );
        manager
            .save_session(workspace.path(), &session)
            .await
            .expect("session should save");
        let mut turn = DialogTurnData::new(
            "turn-0".to_string(),
            0,
            session_id.clone(),
            user_message("turn to delete"),
        );
        turn.mark_completed();
        manager
            .save_dialog_turn(workspace.path(), &turn)
            .await
            .expect("turn should save");

        let metadata_lock = manager
            .get_session_persistence_lock(workspace.path(), &session_id)
            .await;
        let metadata_guard = metadata_lock.lock().await;
        let turn_path = manager.turn_path(workspace.path(), &session_id, 0);
        let delete_task = tokio::spawn({
            let manager = manager.clone();
            let workspace_path = workspace.path().to_path_buf();
            let session_id = session_id.clone();
            async move {
                manager
                    .delete_turns_from(&workspace_path, &session_id, 0)
                    .await
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            turn_path.exists(),
            "turn files must not change before the metadata transaction is acquired"
        );
        assert!(!delete_task.is_finished());
        drop(metadata_guard);

        assert_eq!(
            delete_task
                .await
                .expect("delete task should join")
                .expect("delete should succeed"),
            1
        );
        assert!(!turn_path.exists());
        let metadata = manager
            .load_session_metadata(workspace.path(), &session_id)
            .await
            .expect("metadata should load")
            .expect("metadata should exist");
        assert_eq!(metadata.turn_count, 0);
    }

    #[tokio::test]
    async fn whole_session_deletion_waits_for_the_persistence_transaction() {
        let workspace = TestWorkspace::new();
        let manager = Arc::new(
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager"),
        );
        let session_id = Uuid::new_v4().to_string();
        let session = Session::new_with_id(
            session_id.clone(),
            "Transactional session deletion".to_string(),
            "agentic".to_string(),
            SessionConfig {
                workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );
        manager
            .save_session(workspace.path(), &session)
            .await
            .expect("session should save");

        let persistence_lock = manager
            .get_session_persistence_lock(workspace.path(), &session_id)
            .await;
        let persistence_guard = persistence_lock.lock().await;
        let session_dir = manager
            .session_layout(workspace.path())
            .session_dir(&session_id);
        let delete_task = tokio::spawn({
            let manager = manager.clone();
            let workspace_path = workspace.path().to_path_buf();
            let session_id = session_id.clone();
            async move { manager.delete_session(&workspace_path, &session_id).await }
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(session_dir.exists());
        assert!(!delete_task.is_finished());
        drop(persistence_guard);

        delete_task
            .await
            .expect("delete task should join")
            .expect("session delete should succeed");
        assert!(!session_dir.exists());
    }

    #[tokio::test]
    async fn metadata_lock_identity_normalizes_workspace_path_aliases() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
        let session_id = Uuid::new_v4().to_string();
        let session = Session::new_with_id(
            session_id.clone(),
            "Canonical metadata lock".to_string(),
            "agentic".to_string(),
            SessionConfig {
                workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );
        manager
            .save_session(workspace.path(), &session)
            .await
            .expect("session should save");

        std::fs::create_dir_all(workspace.path().join("alias-component"))
            .expect("alias component should exist");
        let alias = workspace.path().join("alias-component").join("..");
        let canonical_lock = manager
            .get_session_persistence_lock(workspace.path(), &session_id)
            .await;
        let alias_lock = manager
            .get_session_persistence_lock(&alias, &session_id)
            .await;

        assert!(Arc::ptr_eq(&canonical_lock, &alias_lock));
    }

    #[tokio::test]
    async fn save_dialog_turn_persists_last_finished_at() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
        let session_id = Uuid::new_v4().to_string();
        let session = Session::new_with_id(
            session_id.clone(),
            "Finished timestamp metadata".to_string(),
            "agent".to_string(),
            SessionConfig {
                workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );

        manager
            .save_session(workspace.path(), &session)
            .await
            .expect("session should save");

        let mut turn = DialogTurnData::new(
            "turn-0".to_string(),
            0,
            session_id.clone(),
            user_message("finished"),
        );
        turn.model_rounds.push(round_with_text(
            "turn-0",
            vec![text_item("text-0", "finished response")],
        ));
        turn.mark_completed();
        let finished_at = turn.end_time;

        manager
            .save_dialog_turn(workspace.path(), &turn)
            .await
            .expect("turn should save");

        let metadata = manager
            .load_session_metadata(workspace.path(), &session_id)
            .await
            .expect("metadata should load")
            .expect("metadata should exist");

        assert_eq!(metadata.last_finished_at, finished_at);
    }

    #[tokio::test]
    async fn concurrent_dialog_turn_saves_keep_metadata_counts_consistent() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
        let session_id = Uuid::new_v4().to_string();
        let session = Session::new_with_id(
            session_id.clone(),
            "Concurrent metadata".to_string(),
            "agent".to_string(),
            SessionConfig {
                workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );

        manager
            .save_session(workspace.path(), &session)
            .await
            .expect("session should save");

        let mut turn_0 = DialogTurnData::new(
            "turn-0".to_string(),
            0,
            session_id.clone(),
            user_message("first"),
        );
        turn_0.model_rounds.push(round_with_text(
            "turn-0",
            vec![text_item("text-0", "first response")],
        ));
        turn_0.mark_completed();
        manager
            .save_dialog_turn(workspace.path(), &turn_0)
            .await
            .expect("first turn should save");

        let mut turn_1 = DialogTurnData::new(
            "turn-1".to_string(),
            1,
            session_id.clone(),
            user_message("second"),
        );
        turn_1.model_rounds.push(round_with_text(
            "turn-1",
            vec![text_item("text-1", "second response")],
        ));
        turn_1.mark_completed();
        manager
            .save_dialog_turn(workspace.path(), &turn_1)
            .await
            .expect("second turn should save");

        let mut updated_turn_0 = turn_0.clone();
        updated_turn_0.model_rounds[0]
            .text_items
            .push(text_item("text-0b", "first follow-up"));

        let mut updated_turn_1 = turn_1.clone();
        updated_turn_1.model_rounds[0]
            .text_items
            .push(text_item("text-1b", "second follow-up"));
        updated_turn_1.model_rounds[0]
            .text_items
            .push(text_item("text-1c", "second final"));

        let (first_result, second_result) = tokio::join!(
            manager.save_dialog_turn(workspace.path(), &updated_turn_0),
            manager.save_dialog_turn(workspace.path(), &updated_turn_1)
        );
        first_result.expect("first concurrent save should succeed");
        second_result.expect("second concurrent save should succeed");

        let metadata = manager
            .load_session_metadata(workspace.path(), &session_id)
            .await
            .expect("metadata should load")
            .expect("metadata should exist");
        assert_eq!(metadata.turn_count, 2);
        assert_eq!(metadata.message_count, 7);
    }

    #[test]
    fn context_snapshot_payload_stats_counts_tool_result_payloads_without_contents() {
        let messages = vec![
            Message::assistant("hello".to_string()),
            Message::tool_result(ToolResult {
                tool_id: "tool-1".to_string(),
                tool_name: "Bash".to_string(),
                effective_tool_name: None,
                result: serde_json::json!({ "output": "x".repeat(40) }),
                result_for_assistant: Some("assistant summary".to_string()),
                is_error: false,
                duration_ms: Some(1),
                image_attachments: None,
            }),
        ];

        let stats = context_snapshot_payload_stats(&messages);

        assert_eq!(stats.tool_result_count, 1);
        assert_eq!(stats.raw_result_string_chars, 40);
        assert_eq!(stats.result_for_assistant_chars, 17);
        assert_eq!(stats.largest_raw_result_chars, 40);
        assert_eq!(stats.largest_raw_result_path, "message[1].Bash.output");
        assert!(!stats.largest_raw_result_path.contains(&"x".repeat(40)));
    }

    #[tokio::test]
    async fn subagent_session_kind_is_hidden_from_visible_session_index() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");

        let mut metadata = SessionMetadata::new(
            Uuid::new_v4().to_string(),
            "Subagent: repo sweep".to_string(),
            "Explore".to_string(),
            "model".to_string(),
        );
        metadata.session_kind = SessionKind::Subagent;

        manager
            .save_session_metadata(workspace.path(), &metadata)
            .await
            .expect("metadata should save");

        let visible = manager
            .list_session_metadata(workspace.path())
            .await
            .expect("visible metadata should load");
        let raw = manager
            .list_session_metadata_including_internal(workspace.path())
            .await
            .expect("raw metadata should load");

        assert!(visible.is_empty());
        assert_eq!(raw.len(), 1);
        assert!(raw[0].is_subagent());
    }

    #[tokio::test]
    async fn legacy_leaked_subagent_is_hidden_from_visible_session_index() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");

        let mut metadata = SessionMetadata::new(
            Uuid::new_v4().to_string(),
            "Subagent: stale task".to_string(),
            "Explore".to_string(),
            "model".to_string(),
        );
        metadata.created_by = Some("session-parent".to_string());

        manager
            .save_session_metadata(workspace.path(), &metadata)
            .await
            .expect("metadata should save");

        let visible = manager
            .list_session_metadata(workspace.path())
            .await
            .expect("visible metadata should load");
        let raw = manager
            .list_session_metadata_including_internal(workspace.path())
            .await
            .expect("raw metadata should load");

        assert!(visible.is_empty());
        assert_eq!(raw.len(), 1);
        assert!(raw[0].is_legacy_leaked_subagent_candidate());
    }

    #[tokio::test]
    async fn listing_sessions_does_not_create_sessions_dir_for_uninitialized_runtime() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");

        let visible = manager
            .list_session_metadata(workspace.path())
            .await
            .expect("visible listing should succeed");
        let raw = manager
            .list_session_metadata_including_internal(workspace.path())
            .await
            .expect("raw listing should succeed");

        assert!(visible.is_empty());
        assert!(raw.is_empty());
        assert!(
            !manager.project_sessions_dir(workspace.path()).exists(),
            "listing sessions should not create the runtime sessions directory"
        );
    }

    #[tokio::test]
    async fn list_session_metadata_page_returns_visible_top_level_page_with_children() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");

        for index in 0..12 {
            let mut metadata = SessionMetadata::new(
                format!("parent-{index}"),
                format!("Parent {index}"),
                "agent".to_string(),
                "model".to_string(),
            );
            metadata.last_active_at = 1_000 + index;
            manager
                .save_session_metadata(workspace.path(), &metadata)
                .await
                .expect("parent metadata should save");
        }

        let mut child = SessionMetadata::new(
            "child-latest".to_string(),
            "Child latest".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        child.last_active_at = 2_000;
        child.relationship = Some(SessionRelationship {
            kind: Some(SessionRelationshipKind::Btw),
            parent_session_id: Some("parent-11".to_string()),
            ..Default::default()
        });
        manager
            .save_session_metadata(workspace.path(), &child)
            .await
            .expect("child metadata should save");

        let page = manager
            .list_session_metadata_page(workspace.path(), None, 5)
            .await
            .expect("session metadata page should load");
        let session_ids = page
            .sessions
            .iter()
            .map(|metadata| metadata.session_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(page.total_top_level_count, 12);
        assert_eq!(page.loaded_top_level_count, 5);
        assert!(page.next_cursor.is_some());
        assert!(page.has_more);
        assert_eq!(
            session_ids,
            vec![
                "parent-11",
                "child-latest",
                "parent-10",
                "parent-9",
                "parent-8",
                "parent-7",
            ]
        );

        let second_page = manager
            .list_session_metadata_page(workspace.path(), page.next_cursor.as_deref(), 5)
            .await
            .expect("second session metadata page should load");
        let second_page_session_ids = second_page
            .sessions
            .iter()
            .map(|metadata| metadata.session_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(second_page.loaded_top_level_count, 5);
        assert_eq!(
            second_page_session_ids,
            vec!["parent-6", "parent-5", "parent-4", "parent-3", "parent-2"]
        );
    }

    #[tokio::test]
    async fn list_session_metadata_page_rebuilds_stale_visible_page_entry() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");

        let mut older = SessionMetadata::new(
            "older-session".to_string(),
            "Older session".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        older.last_active_at = 1_000;
        let mut newer = SessionMetadata::new(
            "newer-session".to_string(),
            "Newer session".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        newer.last_active_at = 2_000;

        manager
            .save_session_metadata(workspace.path(), &older)
            .await
            .expect("older metadata should save");
        manager
            .save_session_metadata(workspace.path(), &newer)
            .await
            .expect("newer metadata should save");

        let mut missing = SessionMetadata::new(
            "missing-session".to_string(),
            "Missing session".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        missing.last_active_at = 3_000;

        let stale_index = StoredSessionIndexFile::new(0, vec![missing, older]);
        manager
            .write_json_atomic(&manager.index_path(workspace.path()), &stale_index)
            .await
            .expect("stale index should be written");

        let page = manager
            .list_session_metadata_page(workspace.path(), None, 5)
            .await
            .expect("session metadata page should rebuild stale index");
        let session_ids = page
            .sessions
            .iter()
            .map(|metadata| metadata.session_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(page.total_top_level_count, 2);
        assert_eq!(session_ids, vec!["newer-session", "older-session"]);
    }

    #[tokio::test]
    async fn session_memory_mode_helpers_update_and_preserve_disabled_precedence() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
        let mut metadata = SessionMetadata::new(
            "session-memory-mode".to_string(),
            "Memory Mode".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        manager
            .save_session_metadata(workspace.path(), &metadata)
            .await
            .expect("metadata should save");

        manager
            .mark_session_memory_mode_polluted(workspace.path(), &metadata.session_id)
            .await
            .expect("enabled session should mark polluted");
        metadata = manager
            .load_session_metadata(workspace.path(), &metadata.session_id)
            .await
            .expect("metadata should load")
            .expect("metadata should exist");
        assert_eq!(metadata.memory_mode, SessionMemoryMode::Polluted);

        manager
            .set_session_memory_mode(
                workspace.path(),
                &metadata.session_id,
                SessionMemoryMode::Disabled,
            )
            .await
            .expect("memory mode should update");
        manager
            .mark_session_memory_mode_polluted(workspace.path(), &metadata.session_id)
            .await
            .expect("disabled session should keep disabled");
        metadata = manager
            .load_session_metadata(workspace.path(), &metadata.session_id)
            .await
            .expect("metadata should load")
            .expect("metadata should exist");
        assert_eq!(metadata.memory_mode, SessionMemoryMode::Disabled);
    }

    #[tokio::test]
    async fn polluted_selected_memory_session_enqueues_phase2() {
        let workspace = TestWorkspace::new();
        let path_manager = workspace.path_manager();
        let manager = PersistenceManager::new(path_manager.clone()).expect("persistence manager");
        let db = MemoryDatabase::new(path_manager);
        db.initialize().await.expect("memory db should initialize");

        let mut metadata = SessionMetadata::new(
            "session-memory-polluted-selected".to_string(),
            "Memory Polluted Selected".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        metadata.memory_mode = SessionMemoryMode::Polluted;
        manager
            .save_session_metadata(workspace.path(), &metadata)
            .await
            .expect("metadata should save");

        let now = current_unix_secs();
        db.upsert_memory(&MemoryRow {
            session_id: metadata.session_id.clone(),
            workspace_path: workspace.path().to_string_lossy().to_string(),
            rollout_path: workspace
                .path()
                .join("sessions")
                .join(&metadata.session_id)
                .to_string_lossy()
                .to_string(),
            source_updated_at_unix_secs: now,
            raw_memory: "memory".to_string(),
            rollout_summary: "summary".to_string(),
            rollout_slug: None,
            generated_at_unix_secs: now,
            usage_count: 1,
            last_usage_unix_secs: Some(now),
            selected_for_phase2: 1,
            selected_for_phase2_source_updated_at: Some(now),
        })
        .await
        .expect("memory row should save");

        manager
            .mark_session_memory_mode_polluted(workspace.path(), &metadata.session_id)
            .await
            .expect("already polluted selected session should enqueue phase2");

        let job = db
            .get_phase2_job(MEMORY_PHASE2_GLOBAL_JOB_KEY)
            .await
            .expect("phase2 job should load")
            .expect("phase2 job should be enqueued");
        assert!(job.input_watermark.unwrap_or_default() >= now);
        assert!(job.retry_at_unix_secs.is_none());
        assert!(job.last_error.is_none());
    }

    #[tokio::test]
    #[ignore = "local performance benchmark; prints timing data only"]
    async fn bench_session_metadata_page_vs_full_list() {
        const SESSION_COUNT: usize = 1_000;
        const ITERATIONS: usize = 10;

        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");

        for index in 0..SESSION_COUNT {
            let mut metadata = SessionMetadata::new(
                format!("bench-parent-{index}"),
                format!("Bench parent {index}"),
                "agent".to_string(),
                "model".to_string(),
            );
            metadata.last_active_at = 1_000_000 + index as u64;
            manager
                .save_session_metadata(workspace.path(), &metadata)
                .await
                .expect("benchmark metadata should save");
        }

        manager
            .list_session_metadata(workspace.path())
            .await
            .expect("warm full list should load");
        manager
            .list_session_metadata_page(workspace.path(), None, 5)
            .await
            .expect("warm page should load");

        let mut full_list_total_ms = 0.0;
        for _ in 0..ITERATIONS {
            let started = Instant::now();
            let full = manager
                .list_session_metadata(workspace.path())
                .await
                .expect("full list should load");
            assert_eq!(full.len(), SESSION_COUNT);
            full_list_total_ms += started.elapsed().as_secs_f64() * 1000.0;
        }

        let mut page_total_ms = 0.0;
        for _ in 0..ITERATIONS {
            let started = Instant::now();
            let page = manager
                .list_session_metadata_page(workspace.path(), None, 5)
                .await
                .expect("page should load");
            assert_eq!(page.loaded_top_level_count, 5);
            assert_eq!(page.total_top_level_count, SESSION_COUNT);
            page_total_ms += started.elapsed().as_secs_f64() * 1000.0;
        }

        let full_avg_ms = full_list_total_ms / ITERATIONS as f64;
        let page_avg_ms = page_total_ms / ITERATIONS as f64;
        println!(
            "session_metadata_bench sessions={} iterations={} full_list_avg_ms={:.3} page5_avg_ms={:.3} speedup={:.1}x",
            SESSION_COUNT,
            ITERATIONS,
            full_avg_ms,
            page_avg_ms,
            full_avg_ms / page_avg_ms.max(0.001)
        );
    }

    #[tokio::test]
    async fn saving_session_metadata_ensures_runtime_layout_before_writing() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");

        let metadata = SessionMetadata::new(
            Uuid::new_v4().to_string(),
            "Runtime ensure".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );

        manager
            .save_session_metadata(workspace.path(), &metadata)
            .await
            .expect("metadata should save");

        let runtime = manager
            .runtime_service()
            .context_for_local_workspace(workspace.path());
        assert!(runtime.runtime_root.exists());
        assert!(runtime.sessions_dir.exists());
        assert!(runtime.snapshot_by_hash_dir.exists());
        assert!(runtime.snapshot_metadata_dir.exists());
        assert!(runtime.snapshot_operations_dir.exists());
        assert!(runtime.plans_dir.exists());
        assert!(runtime.layout_state_file.exists());
    }

    #[tokio::test]
    async fn local_sessions_dir_input_is_used_without_reslugging() {
        let workspace = TestWorkspace::new();
        let path_manager = workspace.path_manager();
        let sessions_dir = path_manager.project_sessions_dir(workspace.path());
        let manager = PersistenceManager::new(path_manager).expect("persistence manager");

        let metadata = SessionMetadata::new(
            Uuid::new_v4().to_string(),
            "Resolved sessions root".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );

        manager
            .save_session_metadata(&sessions_dir, &metadata)
            .await
            .expect("metadata should save under resolved sessions dir");

        assert_eq!(
            manager.index_path(&sessions_dir),
            sessions_dir.join("index.json")
        );
        assert!(sessions_dir
            .join(&metadata.session_id)
            .join("metadata.json")
            .exists());
    }

    #[tokio::test]
    async fn remote_sessions_dir_input_is_used_without_accepting_runtime_root() {
        let test_root =
            std::env::temp_dir().join(format!("bitfun-persistence-test-{}", Uuid::new_v4()));
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            test_root.join("user"),
        ));
        let manager = PersistenceManager::new(path_manager.clone()).expect("persistence manager");
        let runtime_root = path_manager
            .remote_ssh_mirror_root_dir()
            .join("example-host")
            .join("root")
            .join("repo");
        let sessions_dir = runtime_root.join("sessions");

        assert_eq!(manager.project_sessions_dir(&sessions_dir), sessions_dir);
        assert_ne!(manager.project_sessions_dir(&runtime_root), runtime_root);

        let _ = std::fs::remove_dir_all(&test_root);
    }

    #[tokio::test]
    async fn skill_agent_snapshots_persist_and_truncate_with_context_snapshots() {
        let workspace = TestWorkspace::new();
        let manager =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
        let session_id = Uuid::new_v4().to_string();
        let snapshot = TurnSkillAgentSnapshot {
            skills: vec![SkillSnapshotEntry {
                name: "skill-a".to_string(),
                description: "desc-a".to_string(),
                location: "/skills/a".to_string(),
            }],
            subagents: vec![AgentSnapshotEntry {
                id: "agent-a".to_string(),
                description: "desc-a".to_string(),
                default_tools: vec!["Read".to_string()],
            }],
        };

        manager
            .save_turn_context_snapshot(
                workspace.path(),
                &session_id,
                0,
                &[Message::user("hi".to_string())],
            )
            .await
            .expect("context snapshot should save");
        manager
            .save_turn_skill_agent_snapshot(workspace.path(), &session_id, 0, &snapshot)
            .await
            .expect("skill-agent snapshot should save");

        let loaded = manager
            .load_turn_skill_agent_snapshot(workspace.path(), &session_id, 0)
            .await
            .expect("skill-agent snapshot should load")
            .expect("skill-agent snapshot should exist");
        assert_eq!(loaded, snapshot);

        manager
            .delete_turn_context_snapshots_from(workspace.path(), &session_id, 0)
            .await
            .expect("snapshot deletion should succeed");

        assert!(manager
            .load_turn_skill_agent_snapshot(workspace.path(), &session_id, 0)
            .await
            .expect("skill-agent snapshot reload should succeed")
            .is_none());
        assert!(manager
            .load_turn_context_snapshot(workspace.path(), &session_id, 0)
            .await
            .expect("context snapshot reload should succeed")
            .is_none());
    }
}
