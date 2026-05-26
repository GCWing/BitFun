//! Session Manager
//!
//! Responsible for session CRUD, lifecycle management, and resource association

use crate::agentic::core::{
    new_turn_id, CompressionContract, CompressionState, Message, MessageSemanticKind,
    ProcessingPhase, Session, SessionConfig, SessionKind, SessionState, SessionSummary, TurnStats,
};
use crate::agentic::image_analysis::ImageContextData;
use crate::agentic::persistence::PersistenceManager;
use crate::agentic::session::{
    CachedSystemPrompt, CachedUserContext, EvidenceLedgerCheckpoint, EvidenceLedgerEvent,
    EvidenceLedgerEventStatus, EvidenceLedgerSummary, EvidenceLedgerTargetKind, FileReadState,
    FileReadStateStore, PromptCacheLookup, PromptCachePolicy, PromptCacheScope,
    SessionContextStore, SessionEvidenceLedger, SessionPromptCache, SessionPromptCacheStore,
    SystemPromptCacheIdentity, UserContextCacheIdentity,
};
use crate::infrastructure::ai::get_global_ai_client_factory;
use crate::service::config::{
    get_app_language_code, get_global_config_service, short_model_user_language_instruction,
    subscribe_config_updates, ConfigUpdateEvent,
};
use crate::service::session::{
    DialogTurnData, DialogTurnKind, ModelRoundData, SessionMetadata, SessionRelationship,
    SessionRelationshipKind, TextItemData, TurnStatus, UserMessageData,
};
use crate::service::snapshot::ensure_snapshot_manager_for_workspace;
use crate::service::workspace::get_global_workspace_service;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::sanitize_plain_model_output;
use crate::util::timing::elapsed_ms_u64;
use dashmap::DashMap;
use log::{debug, error, info, warn};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use std::time::{Duration, SystemTime};
use tokio::time;

/// Session manager configuration
#[derive(Debug, Clone)]
pub struct SessionManagerConfig {
    pub max_active_sessions: usize,
    pub session_idle_timeout: Duration,
    pub auto_save_interval: Duration,
    pub enable_persistence: bool,
    pub prompt_cache_policy: PromptCachePolicy,
}

impl Default for SessionManagerConfig {
    fn default() -> Self {
        Self {
            max_active_sessions: 100,
            session_idle_timeout: Duration::from_secs(3600), // 1 hour
            auto_save_interval: Duration::from_secs(300),    // 5 minutes
            enable_persistence: true,
            prompt_cache_policy: PromptCachePolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTitleMethod {
    Ai,
    Fallback,
}

impl SessionTitleMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ai => "ai",
            Self::Fallback => "fallback",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedSessionTitle {
    pub title: String,
    pub method: SessionTitleMethod,
}
/// Session manager
pub struct SessionManager {
    /// Active sessions in memory
    sessions: Arc<DashMap<String, Session>>,

    /// Runtime cache of session_id -> effective workspace path.
    /// Populated on session create/restore and used to restore evicted sessions
    /// or resolve workspace-bound operations that only receive a session_id.
    /// This cache is intentionally retained across memory eviction, but should
    /// be cleared when a session is explicitly deleted.
    session_workspace_index: Arc<DashMap<String, PathBuf>>,

    /// Sub-components
    context_store: Arc<SessionContextStore>,
    prompt_cache_store: Arc<SessionPromptCacheStore>,
    file_read_state_store: Arc<FileReadStateStore>,
    evidence_ledger: Arc<SessionEvidenceLedger>,
    persistence_manager: Arc<PersistenceManager>,

    /// Configuration
    config: SessionManagerConfig,
}

#[derive(Clone)]
struct SessionAutoSaveSnapshot {
    session_id: String,
    updated_at: SystemTime,
    last_activity_at: SystemTime,
    session: Session,
}

#[derive(Clone)]
struct SessionCleanupCandidate {
    session_id: String,
    updated_at: SystemTime,
    last_activity_at: SystemTime,
}

impl SessionManager {
    async fn load_ai_config_for_model_resolution() -> Option<crate::service::config::types::AIConfig>
    {
        let config_service = get_global_config_service().await.ok()?;
        config_service.get_config(Some("ai")).await.ok()
    }

    fn is_auto_model_selector(model_id: &str) -> bool {
        let trimmed = model_id.trim();
        trimmed.is_empty() || trimmed == "auto" || trimmed == "default"
    }

    fn context_window_for_model_selection(
        ai_config: &crate::service::config::types::AIConfig,
        model_id: &str,
    ) -> Option<usize> {
        let trimmed = model_id.trim();
        if Self::is_auto_model_selector(trimmed) {
            return None;
        }

        let resolved_model_id = ai_config.resolve_model_selection(trimmed)?;
        ai_config
            .models
            .iter()
            .find(|model| model.id == resolved_model_id)
            .and_then(|model| model.context_window)
            .map(|tokens| tokens as usize)
    }

    fn session_context_window_from_ai_config(
        session: &Session,
        ai_config: &crate::service::config::types::AIConfig,
    ) -> Option<usize> {
        let configured_model_id = session
            .config
            .model_id
            .as_deref()
            .map(str::trim)
            .filter(|model_id| !model_id.is_empty())
            .unwrap_or("auto");

        if !Self::is_auto_model_selector(configured_model_id) {
            return Self::context_window_for_model_selection(ai_config, configured_model_id);
        }

        let agent_model_id = ai_config
            .agent_models
            .get(&session.agent_type)
            .map(String::as_str)
            .map(str::trim)
            .filter(|model_id| !Self::is_auto_model_selector(model_id));

        agent_model_id
            .and_then(|model_id| Self::context_window_for_model_selection(ai_config, model_id))
            .or_else(|| Self::context_window_for_model_selection(ai_config, "primary"))
    }

    fn sync_session_context_window_from_ai_config(
        session: &mut Session,
        ai_config: &crate::service::config::types::AIConfig,
    ) -> Option<usize> {
        let context_window = Self::session_context_window_from_ai_config(session, ai_config)?;
        session.config.max_context_tokens = context_window;
        Some(context_window)
    }

    fn normalize_session_title_input(title: &str) -> BitFunResult<String> {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return Err(BitFunError::validation(
                "Session title must not be empty".to_string(),
            ));
        }

        Ok(trimmed.to_string())
    }

    fn normalize_whitespace(value: &str) -> String {
        value.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn truncate_chars(value: &str, max_length: usize) -> String {
        value.chars().take(max_length).collect()
    }

    fn fallback_session_title(user_message: &str, max_length: usize) -> String {
        let max_length = max_length.max(1);
        let normalized = Self::normalize_whitespace(user_message);

        if normalized.is_empty() {
            return Self::truncate_chars("New Session", max_length);
        }

        let truncated_chars: Vec<char> = normalized.chars().take(max_length).collect();
        if normalized.chars().count() <= max_length {
            return truncated_chars.iter().collect();
        }

        let sentence_break_chars = ['。', '！', '？', '；', '.', '!', '?'];
        let break_chars = ['。', '！', '？', '；', '.', '!', '?', '，', ',', ' '];
        let min_break_index = max_length / 2;
        let mut best_break_index: Option<usize> = None;

        for (idx, ch) in truncated_chars.iter().enumerate() {
            if break_chars.contains(ch) && idx > min_break_index {
                best_break_index = Some(idx);
            }
        }

        if let Some(idx) = best_break_index {
            let candidate: String = truncated_chars[..=idx].iter().collect();
            if candidate
                .chars()
                .last()
                .map(|ch| sentence_break_chars.contains(&ch))
                .unwrap_or(false)
            {
                return candidate;
            }

            return format!("{}...", candidate.trim_end());
        }

        let truncated: String = truncated_chars.iter().collect();
        format!("{truncated}...")
    }

    fn paginate_messages(
        messages: &[Message],
        limit: usize,
        before_message_id: Option<&str>,
    ) -> (Vec<Message>, bool) {
        if messages.is_empty() {
            return (vec![], false);
        }

        let end_idx = if let Some(before_id) = before_message_id {
            messages.iter().position(|m| m.id == before_id).unwrap_or(0)
        } else {
            messages.len()
        };

        if end_idx == 0 {
            return (vec![], false);
        }

        let start_idx = end_idx.saturating_sub(limit);
        let has_more = start_idx > 0;

        (messages[start_idx..end_idx].to_vec(), has_more)
    }

    fn session_workspace_from_config(config: &SessionConfig) -> Option<PathBuf> {
        config.workspace_path.as_ref().map(PathBuf::from)
    }

    fn should_persist_session_kind(kind: SessionKind) -> bool {
        match kind {
            SessionKind::Standard | SessionKind::Subagent => true,
            SessionKind::EphemeralChild => false,
        }
    }

    fn should_persist_session(session: &Session) -> bool {
        Self::should_persist_session_kind(session.kind)
    }

    fn same_session_version(
        session: &Session,
        updated_at: SystemTime,
        last_activity_at: SystemTime,
    ) -> bool {
        session.updated_at == updated_at && session.last_activity_at == last_activity_at
    }

    fn collect_auto_save_snapshots(
        sessions: &DashMap<String, Session>,
    ) -> Vec<SessionAutoSaveSnapshot> {
        sessions
            .iter()
            .filter_map(|entry| {
                let session = entry.value();
                if !Self::should_persist_session(session) {
                    return None;
                }
                Some(SessionAutoSaveSnapshot {
                    session_id: session.session_id.clone(),
                    updated_at: session.updated_at,
                    last_activity_at: session.last_activity_at,
                    session: session.clone(),
                })
            })
            .collect()
    }

    fn auto_save_snapshot_is_current(
        sessions: &DashMap<String, Session>,
        snapshot: &SessionAutoSaveSnapshot,
    ) -> bool {
        sessions
            .get(&snapshot.session_id)
            .map(|session| {
                Self::same_session_version(&session, snapshot.updated_at, snapshot.last_activity_at)
            })
            .unwrap_or(false)
    }

    fn auto_save_interval(interval: Duration) -> time::Interval {
        time::interval_at(time::Instant::now() + interval, interval)
    }

    fn is_session_expired(session: &Session, now: SystemTime, timeout: Duration) -> bool {
        now.duration_since(session.last_activity_at)
            .map(|idle_duration| idle_duration > timeout)
            .unwrap_or(false)
    }

    fn collect_expired_session_candidates(
        sessions: &DashMap<String, Session>,
        now: SystemTime,
        timeout: Duration,
    ) -> Vec<SessionCleanupCandidate> {
        sessions
            .iter()
            .filter_map(|entry| {
                let session = entry.value();
                if !Self::is_session_expired(session, now, timeout) {
                    return None;
                }
                Some(SessionCleanupCandidate {
                    session_id: session.session_id.clone(),
                    updated_at: session.updated_at,
                    last_activity_at: session.last_activity_at,
                })
            })
            .collect()
    }

    fn cleanup_candidate_matches_session(
        session: &Session,
        candidate: &SessionCleanupCandidate,
        now: SystemTime,
        timeout: Duration,
    ) -> bool {
        Self::same_session_version(session, candidate.updated_at, candidate.last_activity_at)
            && Self::is_session_expired(session, now, timeout)
    }

    fn cleanup_snapshot_for_candidate(
        sessions: &DashMap<String, Session>,
        candidate: &SessionCleanupCandidate,
        now: SystemTime,
        timeout: Duration,
    ) -> Option<Session> {
        sessions.get(&candidate.session_id).and_then(|session| {
            Self::cleanup_candidate_matches_session(&session, candidate, now, timeout)
                .then(|| session.clone())
        })
    }

    pub fn should_persist_session_id(&self, session_id: &str) -> bool {
        self.config.enable_persistence
            && self
                .sessions
                .get(session_id)
                .map(|session| Self::should_persist_session(&session))
                .unwrap_or(true)
    }

    /// Resolve the effective storage path for a session's workspace.
    async fn effective_workspace_path_from_config(config: &SessionConfig) -> Option<PathBuf> {
        let workspace_path = config.workspace_path.as_ref()?;
        let identity =
            crate::service::remote_ssh::workspace_state::resolve_workspace_session_identity(
                workspace_path,
                config.remote_connection_id.as_deref(),
                config.remote_ssh_host.as_deref(),
            )
            .await?;

        if identity.hostname
            == crate::service::remote_ssh::workspace_state::LOCAL_WORKSPACE_SSH_HOST
        {
            Some(PathBuf::from(identity.logical_workspace_path()))
        } else if identity.hostname == "_unresolved" {
            Some(
                crate::service::remote_ssh::workspace_state::unresolved_remote_session_storage_dir(
                    identity.remote_connection_id.as_deref().unwrap_or_default(),
                    identity.logical_workspace_path(),
                ),
            )
        } else {
            Some(identity.session_storage_path())
        }
    }

    #[allow(dead_code)]
    fn session_workspace_path(&self, session_id: &str) -> Option<PathBuf> {
        self.sessions
            .get(session_id)
            .and_then(|session| Self::session_workspace_from_config(&session.config))
    }

    /// Resolve the effective storage path for a session by ID.
    /// For remote workspaces, maps the remote path to a local session storage path.
    async fn effective_session_workspace_path(&self, session_id: &str) -> Option<PathBuf> {
        let config = self.sessions.get(session_id)?.config.clone();
        Self::effective_workspace_path_from_config(&config).await
    }

    /// Resolve the logical workspace path bound to a session.
    ///
    /// This prefers the in-memory session config, then the persisted metadata
    /// reachable via the session workspace index, and finally scans tracked
    /// workspaces known to the global workspace service ordered by recent access.
    pub async fn resolve_session_workspace_path(&self, session_id: &str) -> Option<PathBuf> {
        if let Some(workspace_path) = self
            .get_session(session_id)
            .and_then(|session| session.config.workspace_path)
            .filter(|path| !path.is_empty())
        {
            return Some(PathBuf::from(workspace_path));
        }

        let indexed_workspace_path = self
            .session_workspace_index
            .get(session_id)
            .map(|entry| entry.clone());
        if let Some(workspace_path) = indexed_workspace_path {
            match self
                .persistence_manager
                .load_session_metadata(&workspace_path, session_id)
                .await
            {
                Ok(Some(metadata)) => {
                    if let Some(bound_workspace) =
                        metadata.workspace_path.filter(|path| !path.is_empty())
                    {
                        return Some(PathBuf::from(bound_workspace));
                    }
                    return Some(workspace_path);
                }
                Ok(None) => {}
                Err(err) => {
                    debug!(
                        "Failed to load indexed session metadata while resolving workspace: session_id={} workspace={} error={}",
                        session_id,
                        workspace_path.display(),
                        err
                    );
                }
            }
        }

        let workspace_service = get_global_workspace_service()?;
        let mut workspaces = workspace_service.list_workspace_infos().await;
        workspaces.sort_by(|left, right| right.last_accessed.cmp(&left.last_accessed));
        let candidates: Vec<PathBuf> = workspaces
            .into_iter()
            .map(|workspace| workspace.root_path)
            .collect();

        for workspace_path in candidates {
            match self
                .persistence_manager
                .load_session_metadata(&workspace_path, session_id)
                .await
            {
                Ok(Some(metadata)) => {
                    if let Some(bound_workspace) =
                        metadata.workspace_path.filter(|path| !path.is_empty())
                    {
                        return Some(PathBuf::from(bound_workspace));
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

    fn build_messages_from_turns(turns: &[DialogTurnData]) -> Vec<Message> {
        let mut messages = Vec::new();

        for turn in turns {
            if !turn.kind.is_model_visible() {
                continue;
            }

            let user_message = if let Some(metadata) = &turn.user_message.metadata {
                let images = metadata
                    .get("images")
                    .and_then(|value| value.as_array())
                    .map(|values| {
                        values
                            .iter()
                            .map(|value| ImageContextData {
                                id: value
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                image_path: value
                                    .get("image_path")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                                data_url: value
                                    .get("data_url")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                                mime_type: value
                                    .get("mime_type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("image/png")
                                    .to_string(),
                                metadata: Some(value.clone()),
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                if images.is_empty() {
                    Message::user(turn.user_message.content.clone())
                } else {
                    Message::user_multimodal(turn.user_message.content.clone(), images)
                }
            } else {
                Message::user(turn.user_message.content.clone())
            };
            messages.push(
                user_message
                    .with_turn_id(turn.turn_id.clone())
                    .with_semantic_kind(MessageSemanticKind::ActualUserInput),
            );

            let assistant_text = turn
                .model_rounds
                .iter()
                .flat_map(|round| round.text_items.iter())
                .map(|item| item.content.clone())
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");

            let assistant_thinking = turn
                .model_rounds
                .iter()
                .flat_map(|round| round.thinking_items.iter())
                .map(|item| item.content.clone())
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");

            let has_text = !assistant_text.trim().is_empty();
            let has_thinking = !assistant_thinking.trim().is_empty();

            if has_text || has_thinking {
                let reasoning_content = if has_thinking {
                    Some(assistant_thinking)
                } else {
                    None
                };
                messages.push(
                    Message::assistant_with_reasoning(
                        reasoning_content,
                        assistant_text,
                        Vec::new(),
                    )
                    .with_turn_id(turn.turn_id.clone()),
                );
            }
        }

        messages
    }

    async fn rebuild_messages_from_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Vec<Message>> {
        let turns = self
            .persistence_manager
            .load_session_turns(workspace_path, session_id)
            .await?;
        Ok(Self::build_messages_from_turns(&turns))
    }

    /// Persist the current runtime context by overwriting `snapshots/context-{turn_index}.json`.
    ///
    /// Save timing is intentionally tied to semantic context changes rather than token chunks:
    /// - after a turn starts and the user message enters runtime context
    /// - after assistant/tool messages are appended to runtime context
    /// - after compression replaces runtime context
    /// - once more when a turn completes or fails
    ///
    /// This is still a best-effort multi-file persistence flow, not a transactional commit.
    /// `session.json`, `turns/turn-*.json`, and `snapshots/context-*.json` may be briefly out of
    /// sync if the process crashes between writes, so restore logic must tolerate partial updates.
    async fn persist_context_snapshot_for_turn_best_effort(
        &self,
        session_id: &str,
        turn_index: usize,
        reason: &str,
    ) {
        if !self.should_persist_session_id(session_id) {
            return;
        }

        let Some(workspace_path) = self.effective_session_workspace_path(session_id).await else {
            debug!(
                "Skipping context snapshot persistence because workspace path is unavailable: session_id={}, turn_index={}, reason={}",
                session_id, turn_index, reason
            );
            return;
        };

        let context_messages = self.context_store.get_context_messages(session_id);
        if let Err(err) = self
            .persistence_manager
            .save_turn_context_snapshot(&workspace_path, session_id, turn_index, &context_messages)
            .await
        {
            warn!(
                "failed to persist context snapshot: session_id={}, turn_index={}, reason={}, err={}",
                session_id, turn_index, reason, err
            );
        }
    }

    async fn persist_current_turn_context_snapshot_best_effort(
        &self,
        session_id: &str,
        reason: &str,
    ) {
        let Some(turn_index) = self
            .sessions
            .get(session_id)
            .and_then(|session| session.dialog_turn_ids.len().checked_sub(1))
        else {
            debug!(
                "Skipping current-turn context snapshot because no turn is active: session_id={}, reason={}",
                session_id, reason
            );
            return;
        };

        self.persist_context_snapshot_for_turn_best_effort(session_id, turn_index, reason)
            .await;
    }

    async fn ensure_prompt_cache_loaded(&self, session_id: &str) {
        if self.prompt_cache_store.has_session(session_id) {
            return;
        }

        let cache = if self.should_persist_session_id(session_id) {
            match self.effective_session_workspace_path(session_id).await {
                Some(workspace_path) => {
                    match self
                        .load_prompt_cache_from_persistence(&workspace_path, session_id)
                        .await
                    {
                        Ok(Some(cache)) => cache,
                        Ok(None) => SessionPromptCache::default(),
                        Err(error) => {
                            warn!(
                                "Failed to load prompt cache: session_id={}, workspace_path={}, error={}",
                                session_id,
                                workspace_path.display(),
                                error
                            );
                            SessionPromptCache::default()
                        }
                    }
                }
                None => SessionPromptCache::default(),
            }
        } else {
            SessionPromptCache::default()
        };

        self.prompt_cache_store.replace_cache(session_id, cache);
    }

    async fn load_prompt_cache_from_persistence(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Option<SessionPromptCache>> {
        let mut cache = match self
            .persistence_manager
            .load_prompt_cache(workspace_path, session_id)
            .await?
        {
            Some(cache) => cache,
            None => return Ok(None),
        };

        let expired_entries_removed =
            cache.apply_persistence_ttl(self.config.prompt_cache_policy.persistence_ttl);

        if !expired_entries_removed {
            return Ok(Some(cache));
        }

        if cache.is_empty() {
            self.persistence_manager
                .delete_prompt_cache(workspace_path, session_id)
                .await?;
            Ok(None)
        } else {
            self.persistence_manager
                .save_prompt_cache(workspace_path, session_id, &cache)
                .await?;
            Ok(Some(cache))
        }
    }

    async fn persist_prompt_cache_best_effort(&self, session_id: &str, reason: &str) {
        if !self.should_persist_session_id(session_id) {
            return;
        }

        let Some(workspace_path) = self.effective_session_workspace_path(session_id).await else {
            debug!(
                "Skipping prompt cache persistence because workspace path is unavailable: session_id={}, reason={}",
                session_id, reason
            );
            return;
        };

        let cache = self
            .prompt_cache_store
            .get_cache(session_id)
            .unwrap_or_default();

        let persist_result = if cache.system_prompt.is_none() && cache.user_context.is_none() {
            self.persistence_manager
                .delete_prompt_cache(&workspace_path, session_id)
                .await
        } else {
            self.persistence_manager
                .save_prompt_cache(&workspace_path, session_id, &cache)
                .await
        };

        if let Err(error) = persist_result {
            warn!(
                "Failed to persist prompt cache: session_id={}, workspace_path={}, reason={}, error={}",
                session_id,
                workspace_path.display(),
                reason,
                error
            );
        }
    }

    pub fn new(
        context_store: Arc<SessionContextStore>,
        persistence_manager: Arc<PersistenceManager>,
        config: SessionManagerConfig,
    ) -> Self {
        let enable_persistence = config.enable_persistence;

        let manager = Self {
            sessions: Arc::new(DashMap::new()),
            session_workspace_index: Arc::new(DashMap::new()),
            context_store,
            prompt_cache_store: Arc::new(SessionPromptCacheStore::new()),
            file_read_state_store: Arc::new(FileReadStateStore::new()),
            evidence_ledger: Arc::new(SessionEvidenceLedger::new()),
            persistence_manager,
            config,
        };

        // Start background tasks
        if enable_persistence {
            manager.spawn_auto_save_task();
        }
        manager.spawn_cleanup_task();
        manager.spawn_model_reconciliation_listener();

        manager
    }

    pub fn append_evidence_event(&self, event: EvidenceLedgerEvent) -> EvidenceLedgerEvent {
        self.evidence_ledger.append(event)
    }

    pub fn record_checkpoint_created(
        &self,
        session_id: &str,
        turn_id: &str,
        tool_name: &str,
        target: &str,
        checkpoint: EvidenceLedgerCheckpoint,
    ) -> EvidenceLedgerEvent {
        self.append_evidence_event(EvidenceLedgerEvent::checkpoint_created(
            session_id, turn_id, tool_name, target, checkpoint,
        ))
    }

    pub fn evidence_events_for_turn(
        &self,
        session_id: &str,
        turn_id: &str,
    ) -> Vec<EvidenceLedgerEvent> {
        self.evidence_ledger.events_for_turn(session_id, turn_id)
    }

    pub fn evidence_summary_for_session(
        &self,
        session_id: &str,
        limit: usize,
    ) -> EvidenceLedgerSummary {
        self.evidence_ledger.summary_for_session(session_id, limit)
    }

    pub fn compression_contract_for_session(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Option<CompressionContract> {
        let contract: CompressionContract =
            self.evidence_summary_for_session(session_id, limit).into();
        (!contract.is_empty()).then_some(contract)
    }

    pub fn record_subagent_partial_timeout(
        &self,
        session_id: &str,
        turn_id: &str,
        subagent_type: &str,
        partial_output: &str,
        error_kind: Option<&str>,
    ) -> EvidenceLedgerEvent {
        let summary = format!(
            "Subagent {} timed out after producing partial output.",
            subagent_type
        );
        let event = EvidenceLedgerEvent::new(
            session_id,
            turn_id,
            "Task",
            EvidenceLedgerTargetKind::Subagent,
            subagent_type,
            EvidenceLedgerEventStatus::PartialTimeout,
            summary,
        )
        .with_error_kind(error_kind.unwrap_or("timeout"))
        .with_partial_output(partial_output);

        self.append_evidence_event(event)
    }

    /// Decide whether the given session model id is still usable.
    ///
    /// `model_id` is treated as "usable" when:
    /// - it is a special selector (`auto` / `primary` / `fast` / `default` /
    ///   empty) — these are evaluated again at request time against
    ///   `default_models`, so their long-term validity is governed elsewhere;
    /// - it resolves to a model that exists AND is enabled.
    fn is_session_model_id_usable(
        ai_config: &crate::service::config::types::AIConfig,
        model_id: &str,
    ) -> bool {
        let trimmed = model_id.trim();
        if trimmed.is_empty()
            || trimmed == "auto"
            || trimmed == "default"
            || trimmed == "primary"
            || trimmed == "fast"
        {
            return true;
        }
        ai_config.is_model_reference_active(trimmed)
    }

    /// Reset every active session whose bound model id is in
    /// `invalidated_model_ids` back to `"auto"`. Persists the change and emits
    /// `AgenticEvent::SessionModelAutoMigrated` for every migrated session so
    /// the UI can refresh its model selector and surface a notice.
    async fn migrate_sessions_off_invalidated_models(
        &self,
        invalidated_model_ids: &[String],
        reason: &'static str,
    ) {
        if invalidated_model_ids.is_empty() {
            return;
        }
        let invalid: HashSet<&str> = invalidated_model_ids.iter().map(String::as_str).collect();

        // Snapshot affected sessions first to avoid holding DashMap iterators
        // across async writes.
        let affected: Vec<(String, String)> = self
            .sessions
            .iter()
            .filter_map(|entry| {
                let session = entry.value();
                let current = session.config.model_id.as_deref()?.trim().to_string();
                if invalid.contains(current.as_str()) {
                    Some((session.session_id.clone(), current))
                } else {
                    None
                }
            })
            .collect();

        if affected.is_empty() {
            return;
        }

        for (session_id, previous_model_id) in affected {
            if let Err(e) = self.update_session_model_id(&session_id, "auto").await {
                warn!(
                    "Failed to auto-migrate session model after reconcile: session_id={}, previous={}, error={}",
                    session_id, previous_model_id, e
                );
                continue;
            }
            info!(
                "Session model auto-migrated to 'auto': session_id={}, previous_model_id={}, reason={}",
                session_id, previous_model_id, reason
            );

            if let Some(coordinator) = crate::agentic::coordination::get_global_coordinator() {
                coordinator
                    .emit_session_model_auto_migrated(
                        &session_id,
                        &previous_model_id,
                        "auto",
                        reason,
                    )
                    .await;
            }
        }
    }

    /// Best-effort: drop cached AI clients for invalidated models so the next
    /// request rebuilds against the reconciled config.
    async fn invalidate_ai_clients_for_models(invalidated_model_ids: &[String]) {
        if invalidated_model_ids.is_empty() {
            return;
        }
        if let Ok(factory) = get_global_ai_client_factory().await {
            for model_id in invalidated_model_ids {
                factory.invalidate_model(model_id);
            }
        }
    }

    fn spawn_model_reconciliation_listener(&self) {
        let sessions = self.sessions.clone();
        let session_workspace_index = self.session_workspace_index.clone();
        let context_store = self.context_store.clone();
        let prompt_cache_store = self.prompt_cache_store.clone();
        let file_read_state_store = self.file_read_state_store.clone();
        let evidence_ledger = self.evidence_ledger.clone();
        let persistence_manager = self.persistence_manager.clone();
        let manager_config = self.config.clone();

        tokio::spawn(async move {
            let Some(mut receiver) = subscribe_config_updates() else {
                debug!(
                    "SessionManager: config update subscription unavailable; skipping model reconciliation listener"
                );
                return;
            };

            // Re-build a thin handle that mirrors `self` for the listener loop.
            // We can't move `self` into a 'static task, so we recreate the
            // surface area we need from the cloned shared fields above.
            let manager = Self {
                sessions,
                session_workspace_index,
                context_store,
                prompt_cache_store,
                file_read_state_store,
                evidence_ledger,
                persistence_manager,
                config: manager_config,
            };

            loop {
                match receiver.recv().await {
                    Ok(ConfigUpdateEvent::ModelsReconciled {
                        invalidated_model_ids,
                        ..
                    }) => {
                        Self::invalidate_ai_clients_for_models(&invalidated_model_ids).await;
                        manager
                            .migrate_sessions_off_invalidated_models(
                                &invalidated_model_ids,
                                "model_reconciled",
                            )
                            .await;
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        debug!("SessionManager model reconciliation listener: channel closed");
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(
                            "SessionManager model reconciliation listener lagged by {} events; continuing",
                            n
                        );
                    }
                }
            }
        });
    }

    // ============ Session CRUD ============

    /// Create a new session
    pub async fn create_session(
        &self,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
    ) -> BitFunResult<Session> {
        self.create_session_with_id_and_details(
            None,
            session_name,
            agent_type,
            config,
            None,
            SessionKind::Standard,
        )
        .await
    }

    /// Create a new session (supports specifying session ID)
    pub async fn create_session_with_id(
        &self,
        session_id: Option<String>,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
    ) -> BitFunResult<Session> {
        self.create_session_with_id_and_details(
            session_id,
            session_name,
            agent_type,
            config,
            None,
            SessionKind::Standard,
        )
        .await
    }

    /// Create a new session (supports specifying session ID and creator identity)
    pub async fn create_session_with_id_and_creator(
        &self,
        session_id: Option<String>,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
        created_by: Option<String>,
    ) -> BitFunResult<Session> {
        self.create_session_with_id_and_details(
            session_id,
            session_name,
            agent_type,
            config,
            created_by,
            SessionKind::Standard,
        )
        .await
    }

    /// Create a new session with explicit kind.
    pub async fn create_session_with_id_and_details(
        &self,
        session_id: Option<String>,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
        created_by: Option<String>,
        kind: SessionKind,
    ) -> BitFunResult<Session> {
        let _workspace_path = Self::session_workspace_from_config(&config).ok_or_else(|| {
            BitFunError::Validation("Session workspace_path is required".to_string())
        })?;

        let session_storage_path = Self::effective_workspace_path_from_config(&config)
            .await
            .ok_or_else(|| {
                BitFunError::Validation("Session workspace_path is required".to_string())
            })?;

        // Check session count limit
        if self.sessions.len() >= self.config.max_active_sessions {
            return Err(BitFunError::Validation(format!(
                "Exceeded maximum session limit: {}",
                self.config.max_active_sessions
            )));
        }

        let mut session = if let Some(id) = session_id {
            Session::new_with_id(id, session_name, agent_type.clone(), config)
        } else {
            Session::new(session_name, agent_type.clone(), config)
        };
        session.created_by = created_by;
        session.kind = kind;
        let session_id = session.session_id.clone();

        // 1. Add to memory
        self.sessions.insert(session_id.clone(), session.clone());
        self.session_workspace_index
            .insert(session_id.clone(), session_storage_path.clone());

        // 2. Initialize the in-memory context cache.
        self.context_store.create_session(&session_id);
        self.file_read_state_store.create_session(&session_id);

        // 3. Persist to local path (handles remote workspaces correctly)
        // Use the local `session` directly -- no need to re-fetch from DashMap,
        // which would hold a Ref guard across the async save_session call.
        if self.config.enable_persistence && Self::should_persist_session(&session) {
            self.persistence_manager
                .save_session(&session_storage_path, &session)
                .await?;
        }

        info!("Session created: session_name={}", session.session_name);

        Ok(session)
    }

    /// Get session
    pub fn get_session(&self, session_id: &str) -> Option<Session> {
        self.sessions.get(session_id).map(|s| s.clone())
    }

    pub async fn cached_system_prompt(
        &self,
        session_id: &str,
        identity: &SystemPromptCacheIdentity,
    ) -> Option<String> {
        self.ensure_prompt_cache_loaded(session_id).await;
        match self.prompt_cache_store.lookup_system_prompt(
            session_id,
            identity,
            self.config.prompt_cache_policy.cache_ttl,
        ) {
            PromptCacheLookup::Hit(prompt) => Some(prompt),
            PromptCacheLookup::Miss => None,
            PromptCacheLookup::Expired => {
                self.persist_prompt_cache_best_effort(
                    session_id,
                    "system_prompt_cache_expired_cleanup",
                )
                .await;
                None
            }
        }
    }

    pub async fn remember_system_prompt(
        &self,
        session_id: &str,
        identity: SystemPromptCacheIdentity,
        prompt: String,
    ) {
        self.ensure_prompt_cache_loaded(session_id).await;
        self.prompt_cache_store
            .set_system_prompt(session_id, CachedSystemPrompt::new(identity, prompt));
        self.persist_prompt_cache_best_effort(session_id, "system_prompt_cached")
            .await;
    }

    pub async fn cached_user_context(
        &self,
        session_id: &str,
        identity: &UserContextCacheIdentity,
    ) -> Option<String> {
        self.ensure_prompt_cache_loaded(session_id).await;
        match self.prompt_cache_store.lookup_user_context(
            session_id,
            identity,
            self.config.prompt_cache_policy.cache_ttl,
        ) {
            PromptCacheLookup::Hit(user_context) => Some(user_context),
            PromptCacheLookup::Miss => None,
            PromptCacheLookup::Expired => {
                self.persist_prompt_cache_best_effort(
                    session_id,
                    "user_context_cache_expired_cleanup",
                )
                .await;
                None
            }
        }
    }

    pub async fn remember_user_context(
        &self,
        session_id: &str,
        identity: UserContextCacheIdentity,
        user_context: String,
    ) {
        self.ensure_prompt_cache_loaded(session_id).await;
        self.prompt_cache_store
            .set_user_context(session_id, CachedUserContext::new(identity, user_context));
        self.persist_prompt_cache_best_effort(session_id, "user_context_cached")
            .await;
    }

    pub async fn invalidate_prompt_cache(
        &self,
        session_id: &str,
        scope: PromptCacheScope,
        reason: &str,
    ) {
        self.ensure_prompt_cache_loaded(session_id).await;
        let changed = self.prompt_cache_store.invalidate(session_id, scope);

        if changed {
            debug!(
                "Invalidated session prompt cache: session_id={}, scope={:?}, reason={}",
                session_id, scope, reason
            );
            self.persist_prompt_cache_best_effort(session_id, reason)
                .await;
        }
    }

    /// Synchronously reset session state to Idle if it is currently Processing
    /// the expected turn.
    ///
    /// This is an in-memory-only operation intended for RAII-style cleanup in
    /// spawn tasks.  Because `Drop::drop` is synchronous we cannot do async
    /// file I/O here, but that is acceptable: the in-memory state is the
    /// source of truth at runtime, and `restore_session` already resets any
    /// non-Idle persisted state to Idle on application restart.
    pub fn reset_session_state_if_processing(&self, session_id: &str, expected_turn_id: &str) {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            if matches!(
                &session.state,
                SessionState::Processing {
                    current_turn_id,
                    ..
                } if current_turn_id == expected_turn_id
            ) {
                debug!(
                    "RAII guard resetting stuck Processing state to Idle: session_id={}, turn_id={}",
                    session_id, expected_turn_id
                );
                session.state = SessionState::Idle;
                session.updated_at = SystemTime::now();
                session.last_activity_at = SystemTime::now();
            }
        }
    }

    /// Update session state
    pub async fn update_session_state(
        &self,
        session_id: &str,
        new_state: SessionState,
    ) -> BitFunResult<()> {
        let effective_path = self.effective_session_workspace_path(session_id).await;

        // IMPORTANT: keep the DashMap guard scope short -- do NOT hold it across .await.
        // Collect the data needed for persistence, then release the guard before doing I/O.
        let should_persist = if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.state = new_state.clone();
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();

            let persist = self.config.enable_persistence && Self::should_persist_session(&session);
            persist
        } else {
            return Err(BitFunError::NotFound(format!(
                "Session not found: {}",
                session_id
            )));
        };
        // RefMut guard released here -- DashMap shard lock is free.

        // Persist state changes outside the guard scope.
        if should_persist {
            if let Some(ref workspace_path) = effective_path {
                self.persistence_manager
                    .save_session_state(workspace_path, session_id, &new_state)
                    .await?;
            }
        }

        debug!(
            "Updated session state: session_id={}, state={:?}",
            session_id, new_state
        );

        Ok(())
    }

    /// Update session state only when the session is still processing the
    /// expected turn. Returns `true` when the state was updated.
    pub async fn update_session_state_for_turn_if_processing(
        &self,
        session_id: &str,
        expected_turn_id: &str,
        new_state: SessionState,
    ) -> BitFunResult<bool> {
        let effective_path = self.effective_session_workspace_path(session_id).await;

        let should_persist = if let Some(mut session) = self.sessions.get_mut(session_id) {
            let owns_processing_turn = matches!(
                &session.state,
                SessionState::Processing {
                    current_turn_id,
                    ..
                } if current_turn_id == expected_turn_id
            );

            if !owns_processing_turn {
                debug!(
                    "Skipped session state update for stale turn: session_id={}, expected_turn_id={}, current_state={:?}",
                    session_id, expected_turn_id, session.state
                );
                return Ok(false);
            }

            session.state = new_state.clone();
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();

            self.config.enable_persistence && Self::should_persist_session(&session)
        } else {
            return Err(BitFunError::NotFound(format!(
                "Session not found: {}",
                session_id
            )));
        };

        if should_persist {
            if let Some(ref workspace_path) = effective_path {
                self.persistence_manager
                    .save_session_state(workspace_path, session_id, &new_state)
                    .await?;
            }
        }

        debug!(
            "Updated session state for turn: session_id={}, turn_id={}, state={:?}",
            session_id, expected_turn_id, new_state
        );

        Ok(true)
    }

    /// Update session title (in-memory + persistence)
    pub async fn update_session_title(&self, session_id: &str, title: &str) -> BitFunResult<()> {
        let normalized_title = Self::normalize_session_title_input(title)?;
        let workspace_path = self.effective_session_workspace_path(session_id).await;

        {
            let Some(mut session) = self.sessions.get_mut(session_id) else {
                return Err(BitFunError::NotFound(format!(
                    "Session not found: {}",
                    session_id
                )));
            };
            session.session_name = normalized_title.clone();
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();
        }

        if self.should_persist_session_id(session_id) {
            let Some(workspace_path) = workspace_path.as_ref() else {
                return Err(BitFunError::Session(format!(
                    "Workspace path is unavailable for session {}",
                    session_id
                )));
            };
            // Clone the session data out of the DashMap guard before awaiting I/O.
            let session_snapshot = {
                let Some(session) = self.sessions.get(session_id) else {
                    return Err(BitFunError::NotFound(format!(
                        "Session not found: {}",
                        session_id
                    )));
                };
                session.clone()
            };
            // Ref guard released -- DashMap shard lock is free.
            self.persistence_manager
                .save_session(workspace_path, &session_snapshot)
                .await?;
        }

        info!(
            "Session title updated: session_id={}, title={}",
            session_id, normalized_title
        );

        Ok(())
    }

    pub async fn update_session_title_if_current(
        &self,
        session_id: &str,
        expected_current_title: &str,
        title: &str,
    ) -> BitFunResult<bool> {
        let Some(session) = self.sessions.get(session_id) else {
            return Err(BitFunError::NotFound(format!(
                "Session not found: {}",
                session_id
            )));
        };

        if session.session_name != expected_current_title {
            debug!(
                "Skipping auto-generated title because current title changed: session_id={}, expected_title={}, current_title={}",
                session_id,
                expected_current_title,
                session.session_name
            );
            return Ok(false);
        }
        drop(session);

        self.update_session_title(session_id, title).await?;
        Ok(true)
    }

    /// Update session agent type (in-memory + persistence)
    pub async fn update_session_agent_type(
        &self,
        session_id: &str,
        agent_type: &str,
    ) -> BitFunResult<()> {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.agent_type = agent_type.to_string();
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();
        } else {
            return Err(BitFunError::NotFound(format!(
                "Session not found: {}",
                session_id
            )));
        }

        if self.should_persist_session_id(session_id) {
            let effective_path = self.effective_session_workspace_path(session_id).await;
            let session_snapshot = self.sessions.get(session_id).map(|s| s.clone());
            // Ref guard released -- DashMap shard lock is free.
            if let (Some(workspace_path), Some(session)) = (effective_path, session_snapshot) {
                self.persistence_manager
                    .save_session(&workspace_path, &session)
                    .await?;
            }
        }

        debug!(
            "Session agent type updated: session_id={}, agent_type={}",
            session_id, agent_type
        );

        Ok(())
    }

    fn derive_last_user_dialog_agent_type_from_turns(
        turns: &[DialogTurnData],
        fallback_agent_type: Option<&str>,
    ) -> Option<String> {
        // New turns persist their mode on the turn itself. For older persisted
        // sessions that predate this field, fall back to the session default
        // only when at least one surviving user dialog turn exists.
        turns
            .iter()
            .rev()
            .find(|turn| turn.kind == DialogTurnKind::UserDialog)
            .and_then(|turn| {
                turn.agent_type
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            })
            .or_else(|| {
                if turns
                    .iter()
                    .any(|turn| turn.kind == DialogTurnKind::UserDialog)
                {
                    fallback_agent_type
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned)
                } else {
                    None
                }
            })
    }

    /// Update session model id (in-memory + persistence)
    pub async fn update_session_model_id(
        &self,
        session_id: &str,
        model_id: &str,
    ) -> BitFunResult<()> {
        let ai_config = Self::load_ai_config_for_model_resolution().await;
        let mut resolved_context_window = None;

        // If the session was evicted from memory (idle > 1h), try to restore it
        // using the workspace path recorded when it was first created/restored.
        if !self.sessions.contains_key(session_id) && self.config.enable_persistence {
            let workspace_path = self
                .session_workspace_index
                .get(session_id)
                .map(|entry| entry.clone());
            if let Some(workspace_path) = workspace_path {
                debug!(
                    "Session evicted from memory, restoring for model update: session_id={}",
                    session_id
                );
                let _ = self.restore_session(&workspace_path, session_id).await;
            }
        }

        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.config.model_id = Some(model_id.to_string());
            if let Some(ai_config) = ai_config.as_ref() {
                resolved_context_window =
                    Self::sync_session_context_window_from_ai_config(&mut session, ai_config);
            }
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();
        } else {
            return Err(BitFunError::NotFound(format!(
                "Session not found: {}",
                session_id
            )));
        }

        if self.should_persist_session_id(session_id) {
            let effective_path = self.effective_session_workspace_path(session_id).await;
            let session_snapshot = self.sessions.get(session_id).map(|s| s.clone());
            // Ref guard released -- DashMap shard lock is free.
            if let (Some(workspace_path), Some(session)) = (effective_path, session_snapshot) {
                self.persistence_manager
                    .save_session(&workspace_path, &session)
                    .await?;
            }
        }

        debug!(
            "Session model id updated: session_id={}, model_id={}, max_context_tokens={:?}",
            session_id, model_id, resolved_context_window
        );

        Ok(())
    }

    /// Update session activity time
    pub fn touch_session(&self, session_id: &str) {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.last_activity_at = SystemTime::now();
        }
    }

    /// Delete session (cascade delete all resources)
    pub async fn delete_session(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<()> {
        let delete_started_at = Instant::now();
        debug!(
            "Session deletion started: session_id={}, workspace_path={}, persistence_enabled={}",
            session_id,
            workspace_path.display(),
            self.config.enable_persistence
        );

        // 1. Clean up snapshot system resources (including physical snapshot files)
        let snapshot_stage_started_at = Instant::now();
        debug!(
            "Session deletion stage starting: session_id={}, stage=snapshot_cleanup",
            session_id
        );
        if let Ok(snapshot_manager) = ensure_snapshot_manager_for_workspace(workspace_path) {
            let snapshot_service = snapshot_manager.get_snapshot_service();
            let snapshot_service = snapshot_service.read().await;
            if let Err(e) = snapshot_service.accept_session(session_id).await {
                warn!("Failed to cleanup snapshot system resources: {}", e);
            } else {
                debug!(
                    "Snapshot system resources cleaned up: session_id={}",
                    session_id
                );
            }
        }
        debug!(
            "Session deletion stage completed: session_id={}, stage=snapshot_cleanup, duration_ms={}",
            session_id,
            elapsed_ms_u64(snapshot_stage_started_at)
        );

        let context_stage_started_at = Instant::now();
        debug!(
            "Session deletion stage starting: session_id={}, stage=context_store_delete",
            session_id
        );
        self.context_store.delete_session(session_id);
        self.prompt_cache_store.delete_session(session_id);
        self.file_read_state_store.delete_session(session_id);
        debug!(
            "Session deletion stage completed: session_id={}, stage=context_store_delete, duration_ms={}",
            session_id,
            elapsed_ms_u64(context_stage_started_at)
        );

        // 2. Delete persisted data
        if self.config.enable_persistence {
            let persistence_stage_started_at = Instant::now();
            debug!(
                "Session deletion stage starting: session_id={}, stage=persistence_delete",
                session_id
            );
            self.persistence_manager
                .delete_session(workspace_path, session_id)
                .await?;
            debug!(
                "Session deletion stage completed: session_id={}, stage=persistence_delete, duration_ms={}",
                session_id,
                elapsed_ms_u64(persistence_stage_started_at)
            );
        }

        if let Some(cron) = crate::service::cron::get_global_cron_service() {
            let cron_stage_started_at = Instant::now();
            debug!(
                "Session deletion stage starting: session_id={}, stage=cron_cleanup",
                session_id
            );
            match cron.delete_jobs_for_session(session_id).await {
                Ok(removed) if removed > 0 => {
                    info!(
                        "Removed {} scheduled job(s) for deleted session_id={}",
                        removed, session_id
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    warn!(
                        "Failed to remove scheduled jobs for deleted session_id={}: {}",
                        session_id, e
                    );
                }
            }
            debug!(
                "Session deletion stage completed: session_id={}, stage=cron_cleanup, duration_ms={}",
                session_id,
                elapsed_ms_u64(cron_stage_started_at)
            );
        }

        // 3. Clean up associated Terminal session
        use crate::service::terminal::TerminalApi;
        if let Ok(terminal_api) = TerminalApi::from_singleton() {
            let binding = terminal_api.session_manager().binding();
            let terminal_stage_started_at = Instant::now();
            debug!(
                "Session deletion stage starting: session_id={}, stage=terminal_binding_cleanup, has_binding={}",
                session_id,
                binding.has(session_id)
            );
            if binding.has(session_id) {
                if let Err(e) = binding.remove(session_id).await {
                    warn!("Failed to cleanup associated Terminal session: {}", e);
                } else {
                    debug!(
                        "Associated Terminal session cleaned up: session_id={}",
                        session_id
                    );
                }
            }
            debug!(
                "Session deletion stage completed: session_id={}, stage=terminal_binding_cleanup, duration_ms={}",
                session_id,
                elapsed_ms_u64(terminal_stage_started_at)
            );
        }

        // 4. Remove from memory
        let memory_stage_started_at = Instant::now();
        debug!(
            "Session deletion stage starting: session_id={}, stage=in_memory_remove",
            session_id
        );
        self.sessions.remove(session_id);
        debug!(
            "Session deletion stage completed: session_id={}, stage=in_memory_remove, duration_ms={}",
            session_id,
            elapsed_ms_u64(memory_stage_started_at)
        );
        self.session_workspace_index.remove(session_id);

        info!(
            "Session deletion completed: session_id={}, workspace_path={}, duration_ms={}",
            session_id,
            workspace_path.display(),
            elapsed_ms_u64(delete_started_at)
        );

        Ok(())
    }

    /// Restore session (from persistent storage)
    pub async fn restore_session(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Session> {
        self.restore_session_internal(workspace_path, session_id, false)
            .await
    }

    pub async fn restore_internal_session(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Session> {
        self.restore_session_internal(workspace_path, session_id, true)
            .await
    }

    async fn restore_session_internal(
        &self,
        workspace_path: &Path,
        session_id: &str,
        include_internal: bool,
    ) -> BitFunResult<Session> {
        let (session, _) = self
            .restore_session_with_turns_internal(workspace_path, session_id, include_internal)
            .await?;
        Ok(session)
    }

    /// Restore the persisted session header and turns needed by the UI view
    /// without loading runtime context snapshots or inserting the session into
    /// the in-memory coordinator state.
    pub async fn restore_session_view(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<DialogTurnData>)> {
        self.restore_session_view_internal(workspace_path, session_id, false)
            .await
    }

    pub async fn restore_internal_session_view(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<DialogTurnData>)> {
        self.restore_session_view_internal(workspace_path, session_id, true)
            .await
    }

    async fn restore_session_view_internal(
        &self,
        workspace_path: &Path,
        session_id: &str,
        include_internal: bool,
    ) -> BitFunResult<(Session, Vec<DialogTurnData>)> {
        let restore_started_at = Instant::now();
        let storage_path_started_at = Instant::now();
        let session_storage_path = {
            let ws = workspace_path.to_string_lossy().to_string();
            let tmp_config = SessionConfig {
                workspace_path: Some(ws),
                ..Default::default()
            };
            Self::effective_workspace_path_from_config(&tmp_config)
                .await
                .unwrap_or_else(|| workspace_path.to_path_buf())
        };
        debug!(
            "Session view restore phase completed: session_id={}, phase=resolve_storage_path, duration_ms={}",
            session_id,
            elapsed_ms_u64(storage_path_started_at)
        );

        let metadata_started_at = Instant::now();
        if self
            .persistence_manager
            .load_session_metadata(&session_storage_path, session_id)
            .await?
            .is_some_and(|metadata| !include_internal && metadata.should_hide_from_user_lists())
        {
            return Err(BitFunError::NotFound(format!(
                "Session not found: {}",
                session_id
            )));
        }
        debug!(
            "Session view restore phase completed: session_id={}, phase=load_metadata, duration_ms={}",
            session_id,
            elapsed_ms_u64(metadata_started_at)
        );

        let session_started_at = Instant::now();
        let (mut session, persisted_turns) = self
            .persistence_manager
            .load_session_with_turns(&session_storage_path, session_id)
            .await?;
        debug!(
            "Session view restore phase completed: session_id={}, phase=load_session_with_turns, turn_count={}, duration_ms={}",
            session_id,
            persisted_turns.len(),
            elapsed_ms_u64(session_started_at)
        );

        if !matches!(session.state, SessionState::Idle) {
            let old_state = session.state.clone();
            session.state = SessionState::Idle;
            debug!(
                "Resetting session state during view restore: session_id={}, state={:?} -> Idle",
                session_id, old_state
            );
        }

        let persisted_turn_ids: Vec<String> = persisted_turns
            .iter()
            .map(|turn| turn.turn_id.clone())
            .collect();
        if session.dialog_turn_ids != persisted_turn_ids {
            debug!(
                "Session view restore normalized turn ids: session_id={}, session_turn_count={}, persisted_turn_count={}",
                session_id,
                session.dialog_turn_ids.len(),
                persisted_turn_ids.len()
            );
            session.dialog_turn_ids = persisted_turn_ids;
        }

        debug!(
            "Session view restored: session_id={}, session_name={}, turn_count={}, total_duration_ms={}",
            session_id,
            session.session_name,
            persisted_turns.len(),
            elapsed_ms_u64(restore_started_at)
        );

        Ok((session, persisted_turns))
    }

    /// Restore session and return the persisted turns read during restore.
    pub async fn restore_session_with_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<DialogTurnData>)> {
        self.restore_session_with_turns_internal(workspace_path, session_id, false)
            .await
    }

    pub async fn restore_internal_session_with_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<(Session, Vec<DialogTurnData>)> {
        self.restore_session_with_turns_internal(workspace_path, session_id, true)
            .await
    }

    async fn restore_session_with_turns_internal(
        &self,
        workspace_path: &Path,
        session_id: &str,
        include_internal: bool,
    ) -> BitFunResult<(Session, Vec<DialogTurnData>)> {
        let restore_started_at = Instant::now();
        // Check if session is already in memory
        let session_already_in_memory = self.sessions.contains_key(session_id);

        let storage_path_started_at = Instant::now();
        let session_storage_path = {
            let ws = workspace_path.to_string_lossy().to_string();
            let tmp_config = SessionConfig {
                workspace_path: Some(ws),
                ..Default::default()
            };
            Self::effective_workspace_path_from_config(&tmp_config)
                .await
                .unwrap_or_else(|| workspace_path.to_path_buf())
        };
        debug!(
            "Session restore phase completed: session_id={}, phase=resolve_storage_path, duration_ms={}",
            session_id,
            elapsed_ms_u64(storage_path_started_at)
        );

        let metadata_started_at = Instant::now();
        if self
            .persistence_manager
            .load_session_metadata(&session_storage_path, session_id)
            .await?
            .is_some_and(|metadata| !include_internal && metadata.should_hide_from_user_lists())
        {
            return Err(BitFunError::NotFound(format!(
                "Session not found: {}",
                session_id
            )));
        }
        debug!(
            "Session restore phase completed: session_id={}, phase=load_metadata, duration_ms={}",
            session_id,
            elapsed_ms_u64(metadata_started_at)
        );

        // 1. Load session and turns from storage in one pass
        let session_started_at = Instant::now();
        let (mut session, persisted_turns) = self
            .persistence_manager
            .load_session_with_turns(&session_storage_path, session_id)
            .await?;
        debug!(
            "Session restore phase completed: session_id={}, phase=load_session_with_turns, turn_count={}, duration_ms={}",
            session_id,
            persisted_turns.len(),
            elapsed_ms_u64(session_started_at)
        );

        let ai_config_for_restore = Self::load_ai_config_for_model_resolution().await;
        let mut should_persist_restored_session = false;

        // Lazy migration: if the persisted model_id is no longer usable
        // (model deleted or disabled while the session was on disk), repoint
        // it to "auto" before the session re-enters memory. The next request
        // will pick a model via the normal auto/agent/default pipeline.
        if let Some(persisted_model_id) = session.config.model_id.as_deref() {
            let trimmed = persisted_model_id.trim();
            let needs_migration = if trimmed.is_empty() {
                false
            } else if let Some(ai_config) = ai_config_for_restore.as_ref() {
                !Self::is_session_model_id_usable(ai_config, trimmed)
            } else {
                false
            };

            if needs_migration {
                warn!(
                    "Session restore detected stale model_id; migrating to auto: session_id={}, previous_model_id={}",
                    session_id, trimmed
                );
                let previous_model_id = trimmed.to_string();
                session.config.model_id = Some("auto".to_string());
                should_persist_restored_session = true;

                if let Some(coordinator) = crate::agentic::coordination::get_global_coordinator() {
                    coordinator
                        .emit_session_model_auto_migrated(
                            session_id,
                            &previous_model_id,
                            "auto",
                            "model_unavailable_on_restore",
                        )
                        .await;
                }
            }
        }

        if let Some(ai_config) = ai_config_for_restore.as_ref() {
            let previous_max_context_tokens = session.config.max_context_tokens;
            if let Some(context_window) =
                Self::sync_session_context_window_from_ai_config(&mut session, ai_config)
            {
                if context_window != previous_max_context_tokens {
                    should_persist_restored_session = true;
                    debug!(
                        "Session context window refreshed during restore: session_id={}, previous={}, resolved={}",
                        session_id, previous_max_context_tokens, context_window
                    );
                }
            }
        }

        // Reset session state to Idle
        // After application restart, previous Processing state is invalid and must be reset
        let previous_state_was_not_idle = !matches!(session.state, SessionState::Idle);
        if previous_state_was_not_idle {
            let old_state = session.state.clone();
            session.state = SessionState::Idle;
            debug!(
                "Resetting session state during restore: session_id={}, state={:?} -> Idle",
                session_id, old_state
            );
        }

        // 2. Restore runtime context with snapshot-first semantics.
        // If the latest snapshot lags behind turn persistence, append the missing turn delta
        // instead of truncating session history.
        //
        // This compensates for the fact that persistence is not transactional across
        // `session.json`, `turns/*.json`, and `snapshots/context-*.json`.
        let persisted_turn_ids: Vec<String> = persisted_turns
            .iter()
            .map(|turn| turn.turn_id.clone())
            .collect();
        session.last_user_dialog_agent_type = Self::derive_last_user_dialog_agent_type_from_turns(
            &persisted_turns,
            Some(session.agent_type.as_str()),
        );
        let mut latest_turn_index: Option<usize> = None;
        let context_snapshot_started_at = Instant::now();
        let mut messages = match self
            .persistence_manager
            .load_latest_turn_context_snapshot(&session_storage_path, session_id)
            .await?
        {
            Some((turn_index, msgs)) => {
                latest_turn_index = Some(turn_index);
                msgs
            }
            None => Self::build_messages_from_turns(&persisted_turns),
        };
        debug!(
            "Session restore phase completed: session_id={}, phase=load_context_snapshot, snapshot_turn_index={:?}, message_count={}, duration_ms={}",
            session_id,
            latest_turn_index,
            messages.len(),
            elapsed_ms_u64(context_snapshot_started_at)
        );

        if let Some(snapshot_turn_index) = latest_turn_index {
            let delta_start = snapshot_turn_index.saturating_add(1);
            if delta_start < persisted_turns.len() {
                warn!(
                    "Context snapshot is behind persisted turns, rebuilding delta: session_id={}, snapshot_turn_index={}, persisted_turn_count={}",
                    session_id,
                    snapshot_turn_index,
                    persisted_turns.len()
                );
                messages.extend(Self::build_messages_from_turns(
                    &persisted_turns[delta_start..],
                ));
            }
        };

        if messages.is_empty() {
            debug!(
                "Session {} has empty persisted messages (may be new session)",
                session_id
            );
        }

        // 3. Restore the in-memory context cache from the recovered messages.
        // If session already exists, delete old one first then create (ensure clean state)
        if session_already_in_memory {
            self.context_store.delete_session(session_id);
            self.prompt_cache_store.delete_session(session_id);
            self.file_read_state_store.delete_session(session_id);
        }

        let context_replace_started_at = Instant::now();
        self.context_store
            .replace_context(session_id, messages.clone());
        debug!(
            "Session restore phase completed: session_id={}, phase=replace_context, message_count={}, duration_ms={}",
            session_id,
            messages.len(),
            elapsed_ms_u64(context_replace_started_at)
        );

        let recoverable_turn_count = latest_turn_index
            .map(|turn_index| turn_index + 1)
            .unwrap_or(0)
            .max(persisted_turns.len());

        if session.dialog_turn_ids.len() < persisted_turns.len() {
            warn!(
                "Session metadata is behind persisted turns, rebuilding dialog_turn_ids: session_id={}, session_turn_count={}, persisted_turn_count={}",
                session_id,
                session.dialog_turn_ids.len(),
                persisted_turns.len()
            );
            session.dialog_turn_ids = persisted_turn_ids;
        } else if session.dialog_turn_ids.len() > recoverable_turn_count {
            warn!(
                "Session metadata exceeds recoverable history, truncating: session_id={}, session_turn_count={}, recoverable_turn_count={}",
                session_id,
                session.dialog_turn_ids.len(),
                recoverable_turn_count
            );
            session.dialog_turn_ids.truncate(recoverable_turn_count);
        } else if persisted_turns.len() == session.dialog_turn_ids.len()
            && session.dialog_turn_ids != persisted_turn_ids
        {
            warn!(
                "Session metadata turn ids diverge from persisted turns, normalizing order: session_id={}",
                session_id
            );
            session.dialog_turn_ids = persisted_turn_ids;
        }

        if recoverable_turn_count == 0 && !session.dialog_turn_ids.is_empty() && messages.is_empty()
        {
            warn!(
                "Session has no available context snapshot and messages are empty, clearing turns: session_id={}",
                session_id
            );
            session.dialog_turn_ids.clear();
        }

        let context_msg_count = self.context_store.get_context_messages(session_id).len();

        debug!(
            "Session restored: session_id={}, session_name={}, messages={}, context_messages={}, turn_count={}, total_duration_ms={}",
            session_id,
            session.session_name,
            messages.len(),
            context_msg_count,
            persisted_turns.len(),
            elapsed_ms_u64(restore_started_at)
        );

        // Do not infer unread completion from persisted runtime state during restore.
        // Older IDE versions could leave sessions in non-idle states on disk; treating those
        // as completed would surface misleading unread indicators after an upgrade.
        // Unread completion is now written only by runtime completion/persist paths.

        if should_persist_restored_session && self.should_persist_session_id(session_id) {
            self.persistence_manager
                .save_session(&session_storage_path, &session)
                .await?;
        }

        // 4. Add to memory (will overwrite if already exists)
        self.sessions
            .insert(session_id.to_string(), session.clone());
        self.session_workspace_index
            .insert(session_id.to_string(), session_storage_path.clone());

        Ok((session, persisted_turns))
    }

    /// Rollback "model context" to before the start of specified turn (i.e., keep 0..target_turn-1)
    pub async fn rollback_context_to_turn_start(
        &self,
        workspace_path: &Path,
        session_id: &str,
        target_turn: usize,
    ) -> BitFunResult<()> {
        // Ensure session is in memory (restore from persistence if necessary)
        if !self.sessions.contains_key(session_id) && self.config.enable_persistence {
            let _ = self.restore_session(workspace_path, session_id).await;
        }

        // 1) Load target context (target_turn == 0 => empty context)
        let messages = if target_turn == 0 {
            Vec::new()
        } else {
            self.persistence_manager
                .load_turn_context_snapshot(workspace_path, session_id, target_turn - 1)
                .await?
                .ok_or_else(|| {
                    BitFunError::NotFound(format!(
                        "turn context snapshot not found: session_id={} turn={}",
                        session_id,
                        target_turn - 1
                    ))
                })?
        };

        // 2) Restore the in-memory context cache.
        self.context_store.replace_context(session_id, messages);

        let last_user_dialog_agent_type = if target_turn == 0 {
            None
        } else {
            let surviving_turns = self
                .persistence_manager
                .load_session_turns(workspace_path, session_id)
                .await?;
            let kept_turns = surviving_turns
                .into_iter()
                .take(target_turn)
                .collect::<Vec<_>>();
            let fallback_agent_type = self
                .sessions
                .get(session_id)
                .map(|session| session.agent_type.clone());
            Self::derive_last_user_dialog_agent_type_from_turns(
                &kept_turns,
                fallback_agent_type.as_deref(),
            )
        };

        // 3) Truncate session turn list & persist
        // IMPORTANT: keep the DashMap guard scope short -- do NOT hold it across .await.
        let session_snapshot = if let Some(mut session) = self.sessions.get_mut(session_id) {
            if session.dialog_turn_ids.len() > target_turn {
                session.dialog_turn_ids.truncate(target_turn);
            }
            session.last_user_dialog_agent_type = last_user_dialog_agent_type;
            session.state = SessionState::Idle;
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();

            let should_persist =
                Self::should_persist_session(&session) && self.config.enable_persistence;
            if should_persist {
                Some(session.clone())
            } else {
                None
            }
        } else {
            None
        };
        // RefMut guard released here -- DashMap shard lock is free.

        if let Some(session) = session_snapshot {
            self.persistence_manager
                .save_session(workspace_path, &session)
                .await?;
        }

        // 4) Delete persisted turns and snapshots from target_turn (inclusive) onwards.
        // Runtime restore rebuilds history from persisted turn files, so removing only
        // context snapshots would make rolled-back prompts reappear after reload.
        if self.config.enable_persistence {
            self.persistence_manager
                .delete_dialog_turns_from(workspace_path, session_id, target_turn)
                .await?;
            self.persistence_manager
                .delete_turn_context_snapshots_from(workspace_path, session_id, target_turn)
                .await?;
        }

        Ok(())
    }

    /// List all sessions
    pub async fn list_sessions(&self, workspace_path: &Path) -> BitFunResult<Vec<SessionSummary>> {
        if self.config.enable_persistence {
            self.persistence_manager.list_sessions(workspace_path).await
        } else {
            let summaries: Vec<_> = self
                .sessions
                .iter()
                .map(|entry| {
                    let session = entry.value();
                    SessionSummary {
                        session_id: session.session_id.clone(),
                        session_name: session.session_name.clone(),
                        agent_type: session.agent_type.clone(),
                        created_by: session.created_by.clone(),
                        kind: session.kind,
                        turn_count: session.dialog_turn_ids.len(),
                        created_at: session.created_at,
                        last_activity_at: session.last_activity_at,
                        state: session.state.clone(),
                    }
                })
                .filter(|summary| {
                    !matches!(
                        summary.kind,
                        SessionKind::Subagent | SessionKind::EphemeralChild
                    )
                })
                .collect();
            Ok(summaries)
        }
    }

    pub async fn load_session_metadata(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Option<SessionMetadata>> {
        self.persistence_manager
            .load_session_metadata(workspace_path, session_id)
            .await
    }

    pub async fn save_session_metadata(
        &self,
        workspace_path: &Path,
        metadata: &SessionMetadata,
    ) -> BitFunResult<()> {
        self.persistence_manager
            .save_session_metadata(workspace_path, metadata)
            .await
    }

    pub async fn merge_session_custom_metadata(
        &self,
        session_id: &str,
        patch: serde_json::Value,
    ) -> BitFunResult<()> {
        if !self.should_persist_session_id(session_id) {
            return Ok(());
        }

        let workspace_path = self
            .effective_session_workspace_path(session_id)
            .await
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Session workspace_path is missing: {}",
                    session_id
                ))
            })?;

        let mut metadata = match self
            .persistence_manager
            .load_session_metadata(&workspace_path, session_id)
            .await?
        {
            Some(metadata) => metadata,
            None => {
                let session = self
                    .sessions
                    .get(session_id)
                    .map(|value| value.clone())
                    .ok_or_else(|| {
                        BitFunError::NotFound(format!("Session not found: {}", session_id))
                    })?;
                self.persistence_manager
                    .save_session(&workspace_path, &session)
                    .await?;
                self.persistence_manager
                    .load_session_metadata(&workspace_path, session_id)
                    .await?
                    .ok_or_else(|| {
                        BitFunError::NotFound(format!("Session not found: {}", session_id))
                    })?
            }
        };

        metadata.custom_metadata = Some(match (metadata.custom_metadata.take(), patch) {
            (
                Some(serde_json::Value::Object(mut existing)),
                serde_json::Value::Object(patch_obj),
            ) => {
                for (key, value) in patch_obj {
                    existing.insert(key, value);
                }
                serde_json::Value::Object(existing)
            }
            (_, value) => value,
        });

        self.persistence_manager
            .save_session_metadata(&workspace_path, &metadata)
            .await
    }

    pub async fn merge_session_relationship(
        &self,
        session_id: &str,
        relationship: SessionRelationship,
    ) -> BitFunResult<()> {
        if !self.should_persist_session_id(session_id) {
            return Ok(());
        }

        let workspace_path = self
            .effective_session_workspace_path(session_id)
            .await
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Session workspace_path is missing: {}",
                    session_id
                ))
            })?;

        let mut metadata = match self
            .persistence_manager
            .load_session_metadata(&workspace_path, session_id)
            .await?
        {
            Some(metadata) => metadata,
            None => {
                let session = self
                    .sessions
                    .get(session_id)
                    .map(|value| value.clone())
                    .ok_or_else(|| {
                        BitFunError::NotFound(format!("Session not found: {}", session_id))
                    })?;
                self.persistence_manager
                    .save_session(&workspace_path, &session)
                    .await?;
                self.persistence_manager
                    .load_session_metadata(&workspace_path, session_id)
                    .await?
                    .ok_or_else(|| {
                        BitFunError::NotFound(format!("Session not found: {}", session_id))
                    })?
            }
        };

        metadata.relationship = Some(relationship);
        self.persistence_manager
            .save_session_metadata(&workspace_path, &metadata)
            .await
    }

    pub async fn persist_session_lineage(
        &self,
        session_id: &str,
        relationship: SessionRelationship,
    ) -> BitFunResult<()> {
        if !self.should_persist_session_id(session_id) {
            return Ok(());
        }

        let workspace_path = self
            .effective_session_workspace_path(session_id)
            .await
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Session workspace_path is missing: {}",
                    session_id
                ))
            })?;

        let mut metadata = match self
            .persistence_manager
            .load_session_metadata(&workspace_path, session_id)
            .await?
        {
            Some(metadata) => metadata,
            None => {
                let session = self
                    .sessions
                    .get(session_id)
                    .map(|value| value.clone())
                    .ok_or_else(|| {
                        BitFunError::NotFound(format!("Session not found: {}", session_id))
                    })?;
                self.persistence_manager
                    .save_session(&workspace_path, &session)
                    .await?;
                self.persistence_manager
                    .load_session_metadata(&workspace_path, session_id)
                    .await?
                    .ok_or_else(|| {
                        BitFunError::NotFound(format!("Session not found: {}", session_id))
                    })?
            }
        };

        metadata.relationship = Some(relationship);

        if let Some(serde_json::Value::Object(mut custom_metadata)) =
            metadata.custom_metadata.take()
        {
            for key in [
                "kind",
                "parentSessionId",
                "parentRequestId",
                "parentDialogTurnId",
                "parentTurnIndex",
                "parentToolCallId",
                "subagentType",
            ] {
                custom_metadata.remove(key);
            }
            metadata.custom_metadata =
                (!custom_metadata.is_empty()).then_some(serde_json::Value::Object(custom_metadata));
        }

        self.persistence_manager
            .save_session_metadata(&workspace_path, &metadata)
            .await
    }

    pub async fn collect_hidden_subagent_cascade_for_parent_turns(
        &self,
        workspace_path: &Path,
        parent_session_id: &str,
        parent_dialog_turn_ids: &HashSet<String>,
    ) -> BitFunResult<Vec<String>> {
        if parent_session_id.trim().is_empty() || parent_dialog_turn_ids.is_empty() {
            return Ok(Vec::new());
        }

        let metadata_list = self
            .persistence_manager
            .list_session_metadata_including_internal(workspace_path)
            .await?;

        let mut child_session_ids_by_parent: HashMap<String, Vec<String>> = HashMap::new();
        let mut root_session_ids = Vec::new();

        for metadata in metadata_list {
            let Some((relationship_kind, relationship_parent_session_id, parent_dialog_turn_id)) =
                extract_subagent_relationship(&metadata)
            else {
                continue;
            };

            if relationship_kind != SessionRelationshipKind::Subagent {
                continue;
            }

            child_session_ids_by_parent
                .entry(relationship_parent_session_id.clone())
                .or_default()
                .push(metadata.session_id.clone());

            if relationship_parent_session_id == parent_session_id
                && parent_dialog_turn_ids.contains(&parent_dialog_turn_id)
            {
                root_session_ids.push(metadata.session_id);
            }
        }

        let mut visited = HashSet::new();
        let mut ordered_session_ids = Vec::new();

        fn visit(
            session_id: &str,
            child_session_ids_by_parent: &HashMap<String, Vec<String>>,
            visited: &mut HashSet<String>,
            ordered_session_ids: &mut Vec<String>,
        ) {
            if !visited.insert(session_id.to_string()) {
                return;
            }

            if let Some(child_session_ids) = child_session_ids_by_parent.get(session_id) {
                for child_session_id in child_session_ids {
                    visit(
                        child_session_id,
                        child_session_ids_by_parent,
                        visited,
                        ordered_session_ids,
                    );
                }
            }

            ordered_session_ids.push(session_id.to_string());
        }

        for root_session_id in root_session_ids {
            visit(
                &root_session_id,
                &child_session_ids_by_parent,
                &mut visited,
                &mut ordered_session_ids,
            );
        }

        Ok(ordered_session_ids)
    }

    pub async fn set_session_deep_review_run_manifest(
        &self,
        session_id: &str,
        deep_review_run_manifest: Option<serde_json::Value>,
    ) -> BitFunResult<()> {
        if !self.should_persist_session_id(session_id) {
            return Ok(());
        }

        let workspace_path = self
            .effective_session_workspace_path(session_id)
            .await
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Session workspace_path is missing: {}",
                    session_id
                ))
            })?;

        let mut metadata = match self
            .persistence_manager
            .load_session_metadata(&workspace_path, session_id)
            .await?
        {
            Some(metadata) => metadata,
            None => {
                let session = self
                    .sessions
                    .get(session_id)
                    .map(|value| value.clone())
                    .ok_or_else(|| {
                        BitFunError::NotFound(format!("Session not found: {}", session_id))
                    })?;
                self.persistence_manager
                    .save_session(&workspace_path, &session)
                    .await?;
                self.persistence_manager
                    .load_session_metadata(&workspace_path, session_id)
                    .await?
                    .ok_or_else(|| {
                        BitFunError::NotFound(format!("Session not found: {}", session_id))
                    })?
            }
        };

        metadata.deep_review_run_manifest = deep_review_run_manifest;
        self.persistence_manager
            .save_session_metadata(&workspace_path, &metadata)
            .await
    }

    // ============ Dialog Turn Management ============

    #[allow(clippy::too_many_arguments)]
    async fn start_persisted_turn(
        &self,
        session_id: &str,
        kind: DialogTurnKind,
        agent_type: Option<String>,
        user_input: String,
        turn_id: Option<String>,
        context_message: Option<Message>,
        processing_phase: ProcessingPhase,
        user_message_metadata: Option<serde_json::Value>,
    ) -> BitFunResult<String> {
        let session = self
            .get_session(session_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Session not found: {}", session_id)))?;
        let workspace_path = Self::effective_workspace_path_from_config(&session.config)
            .await
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Session workspace_path is missing: {}",
                    session_id
                ))
            })?;

        let turn_index = session.dialog_turn_ids.len();
        let turn_id = new_turn_id(turn_id);

        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.dialog_turn_ids.push(turn_id.clone());
            if kind == DialogTurnKind::UserDialog {
                session.last_user_dialog_agent_type = agent_type.clone();
            }
            session.state = SessionState::Processing {
                current_turn_id: turn_id.clone(),
                phase: processing_phase,
            };
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();
        }

        if let Some(message) = context_message {
            self.context_store
                .add_message(session_id, message.with_turn_id(turn_id.clone()));
        }

        if self.should_persist_session_id(session_id) {
            let turn_data = DialogTurnData::new_with_kind(
                kind,
                turn_id.clone(),
                turn_index,
                session_id.to_string(),
                if kind == DialogTurnKind::UserDialog {
                    agent_type.clone()
                } else {
                    None
                },
                UserMessageData {
                    id: format!("{}-user", turn_id),
                    content: user_input,
                    timestamp: SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    metadata: user_message_metadata,
                },
            );

            // Clone the session data out of the DashMap guard before awaiting I/O.
            let session_snapshot = self.sessions.get(session_id).map(|s| s.clone());
            // Ref guard released -- DashMap shard lock is free.
            if let Some(session) = session_snapshot {
                self.persistence_manager
                    .save_session(&workspace_path, &session)
                    .await?;
            }
            self.persistence_manager
                .save_dialog_turn(&workspace_path, &turn_data)
                .await?;
        }

        self.persist_context_snapshot_for_turn_best_effort(session_id, turn_index, "turn_started")
            .await;

        Ok(turn_id)
    }

    /// Start a new dialog turn
    /// turn_id: Optional frontend-specified ID, if None then backend generates
    /// Returns: turn_id
    pub async fn start_dialog_turn(
        &self,
        session_id: &str,
        agent_type: String,
        user_input: String,
        turn_id: Option<String>,
        image_contexts: Option<Vec<ImageContextData>>,
        user_message_metadata: Option<serde_json::Value>,
    ) -> BitFunResult<String> {
        let user_message =
            if let Some(images) = image_contexts.as_ref().filter(|v| !v.is_empty()).cloned() {
                Message::user_multimodal(user_input.clone(), images)
                    .with_semantic_kind(MessageSemanticKind::ActualUserInput)
            } else {
                Message::user(user_input.clone())
                    .with_semantic_kind(MessageSemanticKind::ActualUserInput)
            };

        let turn_id = self
            .start_persisted_turn(
                session_id,
                DialogTurnKind::UserDialog,
                Some(agent_type),
                user_input,
                turn_id,
                Some(user_message),
                ProcessingPhase::Starting,
                user_message_metadata,
            )
            .await?;

        debug!("Starting dialog turn: turn_id={}", turn_id);

        Ok(turn_id)
    }

    /// Start a persisted maintenance turn that should not enter model-visible context.
    pub async fn start_maintenance_turn(
        &self,
        session_id: &str,
        display_message: String,
        turn_id: Option<String>,
        user_message_metadata: Option<serde_json::Value>,
    ) -> BitFunResult<String> {
        let turn_id = self
            .start_persisted_turn(
                session_id,
                DialogTurnKind::ManualCompaction,
                None,
                display_message,
                turn_id,
                None,
                ProcessingPhase::Compacting,
                user_message_metadata,
            )
            .await?;

        debug!("Starting maintenance turn: turn_id={}", turn_id);

        Ok(turn_id)
    }

    /// Append a completed local command turn that should be persisted in user-facing
    /// history without entering model-visible runtime context.
    pub async fn append_completed_local_command_turn(
        &self,
        session_id: &str,
        content: String,
        turn_id: Option<String>,
        timestamp_ms: Option<u64>,
        user_message_metadata: Option<serde_json::Value>,
    ) -> BitFunResult<DialogTurnData> {
        let session = self
            .get_session(session_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Session not found: {}", session_id)))?;
        let workspace_path = Self::effective_workspace_path_from_config(&session.config)
            .await
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Session workspace_path is missing: {}",
                    session_id
                ))
            })?;

        let turn_id = new_turn_id(turn_id);
        let turn_index = session
            .dialog_turn_ids
            .iter()
            .position(|existing| existing == &turn_id)
            .unwrap_or(session.dialog_turn_ids.len());
        let timestamp = timestamp_ms.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
        });
        let mut turn = DialogTurnData::new_with_kind(
            DialogTurnKind::LocalCommand,
            turn_id.clone(),
            turn_index,
            session_id.to_string(),
            None,
            UserMessageData {
                id: format!("{}-user", turn_id),
                content,
                timestamp,
                metadata: user_message_metadata,
            },
        );
        turn.timestamp = timestamp;
        turn.start_time = timestamp;
        turn.end_time = Some(timestamp);
        turn.duration_ms = Some(0);
        turn.status = TurnStatus::Completed;

        if self.config.enable_persistence && Self::should_persist_session(&session) {
            self.persistence_manager
                .save_dialog_turn(&workspace_path, &turn)
                .await?;
        }

        let session_snapshot = if let Some(mut session) = self.sessions.get_mut(session_id) {
            if !session
                .dialog_turn_ids
                .iter()
                .any(|existing| existing == &turn_id)
            {
                session.dialog_turn_ids.push(turn_id);
            }
            session.state = SessionState::Idle;
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();

            if self.config.enable_persistence && Self::should_persist_session(&session) {
                Some(session.clone())
            } else {
                None
            }
        } else {
            None
        };

        if let Some(session) = session_snapshot {
            self.persistence_manager
                .save_session(&workspace_path, &session)
                .await?;
        }

        self.persist_context_snapshot_for_turn_best_effort(
            session_id,
            turn_index,
            "local_command_turn_persisted",
        )
        .await;

        Ok(turn)
    }

    /// Complete dialog turn
    pub async fn complete_dialog_turn(
        &self,
        session_id: &str,
        turn_id: &str,
        final_response: String,
        stats: TurnStats,
    ) -> BitFunResult<()> {
        if !self.should_persist_session_id(session_id) {
            debug!(
                "Skipping dialog turn persistence for transient session completion: session_id={}, turn_id={}, response_len={}, rounds={}",
                session_id,
                turn_id,
                final_response.len(),
                stats.total_rounds
            );
            return Ok(());
        }

        let workspace_path = self
            .effective_session_workspace_path(session_id)
            .await
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Session workspace_path is missing: {}",
                    session_id
                ))
            })?;
        let turn_index = self
            .sessions
            .get(session_id)
            .and_then(|session| session.dialog_turn_ids.iter().position(|id| id == turn_id))
            .ok_or_else(|| BitFunError::NotFound(format!("Dialog turn not found: {}", turn_id)))?;
        let mut turn = self
            .persistence_manager
            .load_dialog_turn(&workspace_path, session_id, turn_index)
            .await?
            .ok_or_else(|| BitFunError::NotFound(format!("Dialog turn not found: {}", turn_id)))?;

        // Update state
        let completion_timestamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let has_assistant_text = turn.model_rounds.iter().any(|round| {
            round
                .text_items
                .iter()
                .any(|item| !item.content.trim().is_empty())
        });
        if !has_assistant_text && !final_response.trim().is_empty() {
            let round_index = turn.model_rounds.len();
            turn.model_rounds.push(ModelRoundData {
                id: format!("{}-final-round", turn.turn_id),
                turn_id: turn.turn_id.clone(),
                round_index,
                timestamp: completion_timestamp,
                text_items: vec![TextItemData {
                    id: format!("{}-final-text", turn.turn_id),
                    content: final_response.clone(),
                    is_streaming: false,
                    timestamp: completion_timestamp,
                    is_markdown: true,
                    order_index: Some(0),
                    is_subagent_item: None,
                    parent_task_tool_id: None,
                    subagent_session_id: None,
                    status: Some("completed".to_string()),
                }],
                tool_items: Vec::new(),
                thinking_items: Vec::new(),
                start_time: completion_timestamp,
                end_time: Some(completion_timestamp),
                duration_ms: Some(0),
                provider_id: None,
                model_id: None,
                model_alias: None,
                first_chunk_ms: None,
                first_visible_output_ms: None,
                stream_duration_ms: None,
                attempt_count: None,
                failure_category: None,
                token_details: None,
                status: "completed".to_string(),
            });
        }
        turn.status = TurnStatus::Completed;
        turn.duration_ms = Some(stats.duration_ms);
        turn.end_time = Some(completion_timestamp);

        self.persist_context_snapshot_for_turn_best_effort(
            session_id,
            turn.turn_index,
            "turn_completed",
        )
        .await;

        // Persist
        if self.should_persist_session_id(session_id) {
            self.persistence_manager
                .save_dialog_turn(&workspace_path, &turn)
                .await?;
        }

        debug!(
            "Dialog turn completed: turn_id={}, rounds={}, tools={}",
            turn_id, stats.total_rounds, stats.total_tools
        );

        Ok(())
    }

    /// Mark a dialog turn as failed and persist it.
    /// Unlike `complete_dialog_turn`, this sets the state to `Failed` with an error message.
    pub async fn fail_dialog_turn(
        &self,
        session_id: &str,
        turn_id: &str,
        error: String,
    ) -> BitFunResult<()> {
        if !self.should_persist_session_id(session_id) {
            debug!(
                "Skipping dialog turn persistence for transient session failure: session_id={}, turn_id={}, error={}",
                session_id, turn_id, error
            );
            return Ok(());
        }

        let workspace_path = self
            .effective_session_workspace_path(session_id)
            .await
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Session workspace_path is missing: {}",
                    session_id
                ))
            })?;
        let turn_index = self
            .sessions
            .get(session_id)
            .and_then(|session| session.dialog_turn_ids.iter().position(|id| id == turn_id))
            .ok_or_else(|| BitFunError::NotFound(format!("Dialog turn not found: {}", turn_id)))?;
        let mut turn = self
            .persistence_manager
            .load_dialog_turn(&workspace_path, session_id, turn_index)
            .await?
            .ok_or_else(|| BitFunError::NotFound(format!("Dialog turn not found: {}", turn_id)))?;

        turn.status = TurnStatus::Error;
        turn.end_time = Some(
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );

        self.persist_context_snapshot_for_turn_best_effort(
            session_id,
            turn.turn_index,
            "turn_failed",
        )
        .await;
        if self.should_persist_session_id(session_id) {
            self.persistence_manager
                .save_dialog_turn(&workspace_path, &turn)
                .await?;
        }

        debug!(
            "Dialog turn marked as failed: turn_id={}, turn_index={}, error={}",
            turn_id, turn.turn_index, error
        );

        Ok(())
    }

    /// Mark a dialog turn as cancelled and persist it. Unlike
    /// `complete_dialog_turn`, this writes `TurnStatus::Cancelled` so the
    /// frontend / persistence layer can distinguish a user-cancelled turn
    /// from a fully-completed one. Any partial assistant content that was
    /// already streamed is preserved in `model_rounds`.
    pub async fn cancel_dialog_turn(&self, session_id: &str, turn_id: &str) -> BitFunResult<()> {
        if !self.should_persist_session_id(session_id) {
            debug!(
                "Skipping dialog turn persistence for transient session cancellation: session_id={}, turn_id={}",
                session_id, turn_id
            );
            return Ok(());
        }

        let workspace_path = self
            .effective_session_workspace_path(session_id)
            .await
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Session workspace_path is missing: {}",
                    session_id
                ))
            })?;
        let turn_index = self
            .sessions
            .get(session_id)
            .and_then(|session| session.dialog_turn_ids.iter().position(|id| id == turn_id))
            .ok_or_else(|| BitFunError::NotFound(format!("Dialog turn not found: {}", turn_id)))?;
        let mut turn = self
            .persistence_manager
            .load_dialog_turn(&workspace_path, session_id, turn_index)
            .await?
            .ok_or_else(|| BitFunError::NotFound(format!("Dialog turn not found: {}", turn_id)))?;

        turn.status = TurnStatus::Cancelled;
        turn.end_time = Some(
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );

        self.persist_context_snapshot_for_turn_best_effort(
            session_id,
            turn.turn_index,
            "turn_cancelled",
        )
        .await;

        self.persistence_manager
            .save_dialog_turn(&workspace_path, &turn)
            .await?;

        debug!(
            "Dialog turn marked as cancelled: turn_id={}, turn_index={}",
            turn_id, turn.turn_index
        );

        Ok(())
    }

    /// Complete a maintenance turn and persist its synthetic model round payload.
    pub async fn complete_maintenance_turn(
        &self,
        session_id: &str,
        turn_id: &str,
        model_rounds: Vec<ModelRoundData>,
        duration_ms: u64,
    ) -> BitFunResult<()> {
        if !self.should_persist_session_id(session_id) {
            debug!(
                "Skipping maintenance turn persistence for transient session completion: session_id={}, turn_id={}, rounds={}, duration_ms={}",
                session_id,
                turn_id,
                model_rounds.len(),
                duration_ms
            );
            return Ok(());
        }

        let workspace_path = self
            .effective_session_workspace_path(session_id)
            .await
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Session workspace_path is missing: {}",
                    session_id
                ))
            })?;
        let turn_index = self
            .sessions
            .get(session_id)
            .and_then(|session| session.dialog_turn_ids.iter().position(|id| id == turn_id))
            .ok_or_else(|| BitFunError::NotFound(format!("Dialog turn not found: {}", turn_id)))?;
        let mut turn = self
            .persistence_manager
            .load_dialog_turn(&workspace_path, session_id, turn_index)
            .await?
            .ok_or_else(|| BitFunError::NotFound(format!("Dialog turn not found: {}", turn_id)))?;

        let completion_timestamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        turn.model_rounds = model_rounds;
        turn.status = TurnStatus::Completed;
        turn.duration_ms = Some(duration_ms);
        turn.end_time = Some(completion_timestamp);

        self.persist_context_snapshot_for_turn_best_effort(
            session_id,
            turn.turn_index,
            "maintenance_turn_completed",
        )
        .await;

        if self.should_persist_session_id(session_id) {
            self.persistence_manager
                .save_dialog_turn(&workspace_path, &turn)
                .await?;
        }

        Ok(())
    }

    /// Mark a maintenance turn as failed while preserving its synthetic tool state.
    pub async fn fail_maintenance_turn(
        &self,
        session_id: &str,
        turn_id: &str,
        error: String,
        model_rounds: Vec<ModelRoundData>,
    ) -> BitFunResult<()> {
        if !self.should_persist_session_id(session_id) {
            debug!(
                "Skipping maintenance turn persistence for transient session failure: session_id={}, turn_id={}, rounds={}, error={}",
                session_id,
                turn_id,
                model_rounds.len(),
                error
            );
            return Ok(());
        }

        let workspace_path = self
            .effective_session_workspace_path(session_id)
            .await
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Session workspace_path is missing: {}",
                    session_id
                ))
            })?;
        let turn_index = self
            .sessions
            .get(session_id)
            .and_then(|session| session.dialog_turn_ids.iter().position(|id| id == turn_id))
            .ok_or_else(|| BitFunError::NotFound(format!("Dialog turn not found: {}", turn_id)))?;
        let mut turn = self
            .persistence_manager
            .load_dialog_turn(&workspace_path, session_id, turn_index)
            .await?
            .ok_or_else(|| BitFunError::NotFound(format!("Dialog turn not found: {}", turn_id)))?;

        let completion_timestamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        turn.model_rounds = model_rounds;
        turn.status = TurnStatus::Error;
        turn.duration_ms = Some(completion_timestamp.saturating_sub(turn.start_time));
        turn.end_time = Some(completion_timestamp);

        self.persist_context_snapshot_for_turn_best_effort(
            session_id,
            turn.turn_index,
            "maintenance_turn_failed",
        )
        .await;

        if self.should_persist_session_id(session_id) {
            self.persistence_manager
                .save_dialog_turn(&workspace_path, &turn)
                .await?;
        }

        debug!(
            "Maintenance turn marked as failed: turn_id={}, turn_index={}, error={}",
            turn_id, turn.turn_index, error
        );

        Ok(())
    }

    /// Persist a completed `/btw` side-question turn into an existing child session.
    #[allow(clippy::too_many_arguments)]
    pub async fn persist_btw_turn(
        &self,
        workspace_path: &Path,
        child_session_id: &str,
        request_id: &str,
        question: &str,
        full_text: &str,
        parent_session_id: &str,
        parent_dialog_turn_id: Option<&str>,
        parent_turn_index: Option<usize>,
    ) -> BitFunResult<()> {
        let session = self.sessions.get(child_session_id).ok_or_else(|| {
            BitFunError::NotFound(format!("Session not found: {}", child_session_id))
        })?;
        let turn_id = format!("btw-turn-{}", request_id);
        let turn_index = session
            .dialog_turn_ids
            .iter()
            .position(|existing| existing == &turn_id)
            .unwrap_or(session.dialog_turn_ids.len());

        let user_message_id = format!("btw-user-{}", request_id);
        let round_id = format!("btw-round-{}", request_id);
        let text_id = format!("btw-text-{}", request_id);
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut turn = DialogTurnData::new(
            turn_id.clone(),
            turn_index,
            child_session_id.to_string(),
            UserMessageData {
                id: user_message_id,
                content: question.to_string(),
                timestamp: now,
                metadata: Some(json!({
                    "kind": "btw",
                    "parentSessionId": parent_session_id,
                    "parentRequestId": request_id,
                    "parentDialogTurnId": parent_dialog_turn_id,
                    "parentTurnIndex": parent_turn_index,
                })),
            },
        );
        turn.timestamp = now;
        turn.start_time = now;
        turn.end_time = Some(now);
        turn.duration_ms = Some(0);
        turn.status = TurnStatus::Completed;
        turn.model_rounds = vec![ModelRoundData {
            id: round_id,
            turn_id: turn_id.clone(),
            round_index: 0,
            timestamp: now,
            text_items: vec![TextItemData {
                id: text_id,
                content: full_text.to_string(),
                is_streaming: false,
                timestamp: now,
                is_markdown: true,
                order_index: None,
                is_subagent_item: None,
                parent_task_tool_id: None,
                subagent_session_id: None,
                status: Some("completed".to_string()),
            }],
            tool_items: vec![],
            thinking_items: vec![],
            start_time: now,
            end_time: Some(now),
            duration_ms: Some(0),
            provider_id: None,
            model_id: None,
            model_alias: None,
            first_chunk_ms: None,
            first_visible_output_ms: None,
            stream_duration_ms: None,
            attempt_count: None,
            failure_category: None,
            token_details: None,
            status: "completed".to_string(),
        }];

        drop(session);

        // Persist the turn to disk
        self.persistence_manager
            .save_dialog_turn(workspace_path, &turn)
            .await?;

        // Sync messages to the in-memory caches so subsequent turns can access context.
        let user_message = Message::user(question.to_string())
            .with_turn_id(turn_id.clone())
            .with_semantic_kind(MessageSemanticKind::ActualUserInput);
        let assistant_message =
            Message::assistant(full_text.to_string()).with_turn_id(turn_id.clone());

        // Add to the in-memory runtime context cache.
        self.context_store
            .add_message(child_session_id, user_message);
        self.context_store
            .add_message(child_session_id, assistant_message);

        // IMPORTANT: keep the DashMap guard scope short -- do NOT hold it across .await.
        let session_snapshot = if let Some(mut session) = self.sessions.get_mut(child_session_id) {
            if !session
                .dialog_turn_ids
                .iter()
                .any(|existing| existing == &turn_id)
            {
                session.dialog_turn_ids.push(turn_id);
            }
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();

            if self.config.enable_persistence && Self::should_persist_session(&session) {
                Some(session.clone())
            } else {
                None
            }
        } else {
            None
        };
        // RefMut guard released here -- DashMap shard lock is free.

        if let Some(session) = session_snapshot {
            self.persistence_manager
                .save_session(workspace_path, &session)
                .await?;
        }

        self.persist_context_snapshot_for_turn_best_effort(
            child_session_id,
            turn_index,
            "btw_turn_persisted",
        )
        .await;

        Ok(())
    }

    // ============ Helper Methods ============

    /// Get a best-effort message view for the session.
    /// When persistence is enabled, rebuild from persisted turns so callers see the
    /// canonical turn history instead of the runtime context cache.
    pub async fn get_messages(&self, session_id: &str) -> BitFunResult<Vec<Message>> {
        if self.config.enable_persistence {
            if let Some(workspace_path) = self.effective_session_workspace_path(session_id).await {
                let messages = self
                    .rebuild_messages_from_turns(&workspace_path, session_id)
                    .await?;
                if !messages.is_empty() {
                    return Ok(messages);
                }
            }
        }

        Ok(self.context_store.get_context_messages(session_id))
    }

    /// Get a paginated best-effort message view for the session.
    pub async fn get_messages_paginated(
        &self,
        session_id: &str,
        limit: usize,
        before_message_id: Option<&str>,
    ) -> BitFunResult<(Vec<Message>, bool)> {
        let messages = self.get_messages(session_id).await?;
        Ok(Self::paginate_messages(&messages, limit, before_message_id))
    }

    /// Get session's runtime context messages (may already include compressed reminders).
    pub async fn get_context_messages(&self, session_id: &str) -> BitFunResult<Vec<Message>> {
        let context_messages = self.context_store.get_context_messages(session_id);

        Ok(context_messages)
    }

    /// Add a semantic message to the runtime context cache and immediately refresh the current
    /// turn snapshot so crashes do not lose the latest in-memory context change.
    pub async fn add_message(&self, session_id: &str, message: Message) -> BitFunResult<()> {
        self.context_store.add_message(session_id, message);
        self.persist_current_turn_context_snapshot_best_effort(session_id, "context_message_added")
            .await;
        Ok(())
    }

    /// Replace the runtime context cache for a session and immediately refresh the current turn
    /// snapshot. This is primarily used after compression rewrites the model-visible context.
    pub async fn replace_context_messages(&self, session_id: &str, messages: Vec<Message>) {
        self.context_store.replace_context(session_id, messages);
        self.file_read_state_store.clear_session(session_id);
        self.persist_current_turn_context_snapshot_best_effort(session_id, "context_replaced")
            .await;
    }

    pub fn set_file_read_state(&self, session_id: &str, logical_path: &str, state: FileReadState) {
        self.file_read_state_store
            .set(session_id, logical_path, state);
    }

    pub fn get_file_read_state(
        &self,
        session_id: &str,
        logical_path: &str,
    ) -> Option<FileReadState> {
        self.file_read_state_store.get(session_id, logical_path)
    }

    /// Get dialog turn count
    pub fn get_turn_count(&self, session_id: &str) -> usize {
        self.sessions
            .get(session_id)
            .map(|s| s.dialog_turn_ids.len())
            .unwrap_or(0)
    }

    /// Get session's compression state
    pub fn get_compression_state(&self, session_id: &str) -> Option<CompressionState> {
        self.sessions
            .get(session_id)
            .map(|s| s.compression_state.clone())
    }

    /// Update session's compression state
    pub async fn update_compression_state(
        &self,
        session_id: &str,
        compression_state: CompressionState,
    ) -> BitFunResult<()> {
        let effective_path = self.effective_session_workspace_path(session_id).await;

        // IMPORTANT: keep the DashMap guard scope short -- do NOT hold it across .await.
        let session_snapshot = if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.compression_state = compression_state;
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();
            if self.config.enable_persistence && Self::should_persist_session(&session) {
                Some(session.clone())
            } else {
                None
            }
        } else {
            return Err(BitFunError::NotFound(format!(
                "Session not found: {}",
                session_id
            )));
        };
        // RefMut guard released here -- DashMap shard lock is free.

        if let Some(session) = session_snapshot {
            if let Some(ref workspace_path) = effective_path {
                self.persistence_manager
                    .save_session(workspace_path, &session)
                    .await?;
            }
        }

        Ok(())
    }

    async fn try_generate_session_title_with_ai(
        &self,
        user_message: &str,
        max_length: usize,
    ) -> BitFunResult<Option<String>> {
        use crate::util::types::Message;

        // Match agent `LANGUAGE_PREFERENCE`: use `app.language`, not I18nService (see `app_language` module).
        let lang_code = get_app_language_code().await;
        let language_instruction = short_model_user_language_instruction(lang_code.as_str());

        // Construct system prompt
        let system_prompt = format!(
            "You are a professional session title generation assistant. Based on the user's message content, generate a concise and accurate session title.\n\nRequirements:\n- Title should not exceed {} characters\n- {}\n- Concise and accurate, reflecting the conversation topic\n- Do not add quotes or other decorative symbols\n- Return only the title text, no other content",
            max_length,
            language_instruction
        );

        // Truncate message to save tokens (max 200 characters)
        let truncated_message = if user_message.chars().count() > 200 {
            format!("{}...", user_message.chars().take(200).collect::<String>())
        } else {
            user_message.to_string()
        };

        let user_prompt = format!(
            "User message: {}\n\nPlease generate session title:",
            truncated_message
        );

        // Construct messages (using AIClient's Message type)
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: Some(system_prompt),
                reasoning_content: None,
                thinking_signature: None,
                tool_calls: None,
                tool_call_id: None,
                name: None,
                is_error: None,
                tool_image_attachments: None,
            },
            Message {
                role: "user".to_string(),
                content: Some(user_prompt),
                reasoning_content: None,
                thinking_signature: None,
                tool_calls: None,
                tool_call_id: None,
                name: None,
                is_error: None,
                tool_image_attachments: None,
            },
        ];

        // Dynamically get Agent client to generate title
        let ai_client_factory = get_global_ai_client_factory().await.map_err(|e| {
            BitFunError::AIClient(format!("Failed to get AI client factory: {}", e))
        })?;

        let ai_client = ai_client_factory
            .get_client_by_func_agent("session-title-func-agent")
            .await
            .map_err(|e| BitFunError::AIClient(format!("Failed to get AI client: {}", e)))?;

        let response = ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::ai(format!("AI call failed: {}", e)))?;

        let title = sanitize_plain_model_output(&response.text);
        if title.is_empty() {
            return Ok(None);
        }

        // Truncate title
        let final_title = if title.chars().count() > max_length {
            title.chars().take(max_length).collect::<String>()
        } else {
            title
        };

        Ok(Some(final_title))
    }

    /// Generate a concise session title, using AI first and falling back to a local heuristic.
    pub async fn resolve_session_title(
        &self,
        user_message: &str,
        max_length: Option<usize>,
        allow_ai: bool,
    ) -> ResolvedSessionTitle {
        let max_length = max_length.unwrap_or(20).max(1);

        if allow_ai {
            match self
                .try_generate_session_title_with_ai(user_message, max_length)
                .await
            {
                Ok(Some(title)) => {
                    return ResolvedSessionTitle {
                        title,
                        method: SessionTitleMethod::Ai,
                    };
                }
                Ok(None) => {
                    warn!("AI session title generation returned empty output; using fallback");
                }
                Err(error) => {
                    warn!("AI session title generation failed; using fallback: {error}");
                }
            }
        }

        ResolvedSessionTitle {
            title: Self::fallback_session_title(user_message, max_length),
            method: SessionTitleMethod::Fallback,
        }
    }

    /// Generate session title
    ///
    /// Generate a concise and accurate session title based on user message content.
    pub async fn generate_session_title(
        &self,
        user_message: &str,
        max_length: Option<usize>,
    ) -> BitFunResult<String> {
        Ok(self
            .resolve_session_title(user_message, max_length, true)
            .await
            .title)
    }

    // ============ Background Tasks ============

    /// Start auto-save task
    fn spawn_auto_save_task(&self) {
        let sessions = self.sessions.clone();
        let persistence = self.persistence_manager.clone();
        let interval = self.config.auto_save_interval;

        tokio::spawn(async move {
            let mut ticker = Self::auto_save_interval(interval);

            loop {
                ticker.tick().await;

                for snapshot in Self::collect_auto_save_snapshots(&sessions) {
                    if !Self::auto_save_snapshot_is_current(&sessions, &snapshot) {
                        continue;
                    }
                    if let Some(workspace_path) =
                        Self::effective_workspace_path_from_config(&snapshot.session.config).await
                    {
                        if !Self::auto_save_snapshot_is_current(&sessions, &snapshot) {
                            continue;
                        }
                        if let Err(e) = persistence
                            .save_session(&workspace_path, &snapshot.session)
                            .await
                        {
                            error!(
                                "Failed to auto-save session: session_id={}, error={}",
                                snapshot.session_id, e
                            );
                        }
                    }
                }
            }
        });

        debug!("Auto-save task started");
    }

    /// Start cleanup task for expired sessions
    fn spawn_cleanup_task(&self) {
        let sessions = self.sessions.clone();
        let timeout = self.config.session_idle_timeout;
        let persistence = self.persistence_manager.clone();
        let enable_persistence = self.config.enable_persistence;
        let context_store = self.context_store.clone();
        let prompt_cache_store = self.prompt_cache_store.clone();
        let file_read_state_store = self.file_read_state_store.clone();

        tokio::spawn(async move {
            let mut ticker = time::interval(Duration::from_secs(60));

            loop {
                ticker.tick().await;

                let now = SystemTime::now();
                let candidates = Self::collect_expired_session_candidates(&sessions, now, timeout);

                for candidate in candidates {
                    debug!(
                        "Cleaning up expired session: session_id={}",
                        candidate.session_id
                    );

                    let cleanup_now = SystemTime::now();
                    let Some(session) = Self::cleanup_snapshot_for_candidate(
                        &sessions,
                        &candidate,
                        cleanup_now,
                        timeout,
                    ) else {
                        continue;
                    };

                    if enable_persistence && Self::should_persist_session(&session) {
                        if let Some(workspace_path) =
                            Self::effective_workspace_path_from_config(&session.config).await
                        {
                            if Self::cleanup_snapshot_for_candidate(
                                &sessions,
                                &candidate,
                                SystemTime::now(),
                                timeout,
                            )
                            .is_some()
                            {
                                let _ = persistence.save_session(&workspace_path, &session).await;
                            }
                        }
                    }

                    let removal_now = SystemTime::now();
                    if sessions
                        .remove_if(&candidate.session_id, |_, session| {
                            Self::cleanup_candidate_matches_session(
                                session,
                                &candidate,
                                removal_now,
                                timeout,
                            )
                        })
                        .is_some()
                    {
                        context_store.delete_session(&candidate.session_id);
                        prompt_cache_store.delete_session(&candidate.session_id);
                        file_read_state_store.delete_session(&candidate.session_id);
                    }
                }
            }
        });

        debug!("Cleanup task started");
    }
}

fn extract_subagent_relationship(
    metadata: &SessionMetadata,
) -> Option<(SessionRelationshipKind, String, String)> {
    let relationship = metadata.relationship.as_ref();
    let custom_metadata = metadata.custom_metadata.as_ref();

    let relationship_kind = relationship
        .and_then(|value| value.kind.clone())
        .or_else(|| {
            custom_metadata
                .and_then(|value| value.get("kind"))
                .and_then(|value| value.as_str())
                .and_then(|value| match value {
                    "subagent" => Some(SessionRelationshipKind::Subagent),
                    _ => None,
                })
        })?;

    let parent_session_id = relationship
        .and_then(|value| value.parent_session_id.clone())
        .or_else(|| {
            custom_metadata
                .and_then(|value| value.get("parentSessionId"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })?;

    let parent_dialog_turn_id = relationship
        .and_then(|value| value.parent_dialog_turn_id.clone())
        .or_else(|| {
            custom_metadata
                .and_then(|value| value.get("parentDialogTurnId"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })?;

    Some((relationship_kind, parent_session_id, parent_dialog_turn_id))
}

#[cfg(test)]
mod tests {
    use super::{SessionManager, SessionManagerConfig};
    use crate::agentic::core::{Message, ProcessingPhase, Session, SessionConfig, SessionState};
    use crate::agentic::persistence::PersistenceManager;
    use crate::agentic::session::{
        PromptCachePolicy, PromptCacheScope, SessionContextStore, SystemPromptCacheIdentity,
        UserContextCacheIdentity,
    };
    use crate::infrastructure::PathManager;
    use crate::service::config::types::{
        AIConfig as ServiceAIConfig, AIModelConfig as ServiceAIModelConfig,
    };
    use crate::service::remote_ssh::workspace_state::local_workspace_roots_equal;
    use crate::service::session::{
        DialogTurnData, DialogTurnKind, ModelRoundData, SessionKind, SessionMetadata,
        SessionRelationship, SessionRelationshipKind, ToolCallData, ToolItemData, ToolResultData,
        TurnStatus, UserMessageData,
    };
    use dashmap::try_result::TryResult;
    use serde_json::json;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;
    use uuid::Uuid;

    struct TestWorkspace {
        path: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("bitfun-session-restore-test-{}", Uuid::new_v4()));
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

    fn test_manager(persistence_manager: Arc<PersistenceManager>) -> SessionManager {
        SessionManager::new(
            Arc::new(SessionContextStore::new()),
            persistence_manager,
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: true,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        )
    }

    fn test_manager_with_config(
        persistence_manager: Arc<PersistenceManager>,
        config: SessionManagerConfig,
    ) -> SessionManager {
        SessionManager::new(
            Arc::new(SessionContextStore::new()),
            persistence_manager,
            config,
        )
    }

    fn in_memory_test_manager() -> SessionManager {
        let persistence_manager = Arc::new(
            PersistenceManager::new(Arc::new(PathManager::new().expect("path manager")))
                .expect("persistence manager"),
        );
        SessionManager::new(
            Arc::new(SessionContextStore::new()),
            persistence_manager,
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        )
    }

    fn test_model(id: &str, context_window: u32) -> ServiceAIModelConfig {
        ServiceAIModelConfig {
            id: id.to_string(),
            name: id.to_string(),
            model_name: id.to_string(),
            enabled: true,
            context_window: Some(context_window),
            ..Default::default()
        }
    }

    #[test]
    fn sync_session_context_window_refreshes_stale_explicit_model_window() {
        let mut ai_config = ServiceAIConfig::default();
        ai_config.models = vec![test_model("deepseek-v4-pro", 1_000_000)];

        let mut session = Session::new_with_id(
            "session-804".to_string(),
            "DeepSeek session".to_string(),
            "agentic".to_string(),
            SessionConfig {
                model_id: Some("deepseek-v4-pro".to_string()),
                max_context_tokens: 256_000,
                ..Default::default()
            },
        );

        let resolved =
            SessionManager::sync_session_context_window_from_ai_config(&mut session, &ai_config);

        assert_eq!(resolved, Some(1_000_000));
        assert_eq!(session.config.max_context_tokens, 1_000_000);
    }

    #[test]
    fn sync_session_context_window_resolves_auto_through_agent_model_then_primary() {
        let mut ai_config = ServiceAIConfig::default();
        ai_config.models = vec![
            test_model("primary-model", 512_000),
            test_model("agent-model", 1_000_000),
        ];
        ai_config.default_models.primary = Some("primary-model".to_string());
        ai_config
            .agent_models
            .insert("agentic".to_string(), "agent-model".to_string());

        let mut session = Session::new_with_id(
            "session-auto".to_string(),
            "Auto session".to_string(),
            "agentic".to_string(),
            SessionConfig {
                model_id: Some("auto".to_string()),
                max_context_tokens: 256_000,
                ..Default::default()
            },
        );

        let resolved =
            SessionManager::sync_session_context_window_from_ai_config(&mut session, &ai_config);

        assert_eq!(resolved, Some(1_000_000));
        assert_eq!(session.config.max_context_tokens, 1_000_000);

        ai_config.agent_models.clear();
        session.config.max_context_tokens = 256_000;

        let resolved =
            SessionManager::sync_session_context_window_from_ai_config(&mut session, &ai_config);

        assert_eq!(resolved, Some(512_000));
        assert_eq!(session.config.max_context_tokens, 512_000);
    }

    #[tokio::test]
    async fn auto_save_interval_waits_before_first_tick() {
        let mut ticker = SessionManager::auto_save_interval(Duration::from_millis(40));
        let started = tokio::time::Instant::now();

        ticker.tick().await;

        assert!(started.elapsed() >= Duration::from_millis(30));
    }

    #[tokio::test]
    async fn auto_save_snapshot_collection_releases_session_map_guards() {
        let workspace = TestWorkspace::new();
        let manager = in_memory_test_manager();
        let session = manager
            .create_session(
                "Auto-save snapshot".to_string(),
                "agent".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("session should create");

        let snapshots = SessionManager::collect_auto_save_snapshots(&manager.sessions);
        assert!(snapshots
            .iter()
            .any(|snapshot| snapshot.session_id == session.session_id));

        match manager.sessions.try_get_mut(&session.session_id) {
            TryResult::Present(_) => {}
            TryResult::Absent => panic!("session should remain present"),
            TryResult::Locked => panic!("snapshot collection should not retain session map guards"),
        };
    }

    #[tokio::test]
    async fn reset_session_state_if_processing_ignores_a_newer_turn() {
        let manager = in_memory_test_manager();
        let session_id = Uuid::new_v4().to_string();
        let mut session = Session::new_with_id(
            session_id.clone(),
            "Active session".to_string(),
            "agent".to_string(),
            SessionConfig::default(),
        );
        session.state = SessionState::Processing {
            current_turn_id: "turn-2".to_string(),
            phase: ProcessingPhase::Thinking,
        };
        manager.sessions.insert(session_id.clone(), session);

        manager.reset_session_state_if_processing(&session_id, "turn-1");

        let session = manager
            .get_session(&session_id)
            .expect("session should remain available");
        assert!(matches!(
            session.state,
            SessionState::Processing {
                ref current_turn_id,
                ..
            } if current_turn_id == "turn-2"
        ));
    }

    #[tokio::test]
    async fn reset_session_state_if_processing_resets_the_matching_turn() {
        let manager = in_memory_test_manager();
        let session_id = Uuid::new_v4().to_string();
        let mut session = Session::new_with_id(
            session_id.clone(),
            "Active session".to_string(),
            "agent".to_string(),
            SessionConfig::default(),
        );
        session.state = SessionState::Processing {
            current_turn_id: "turn-1".to_string(),
            phase: ProcessingPhase::Thinking,
        };
        manager.sessions.insert(session_id.clone(), session);

        manager.reset_session_state_if_processing(&session_id, "turn-1");

        let session = manager
            .get_session(&session_id)
            .expect("session should remain available");
        assert!(matches!(session.state, SessionState::Idle));
    }

    #[tokio::test]
    async fn update_session_state_for_turn_if_processing_ignores_a_newer_turn() {
        let manager = in_memory_test_manager();
        let session_id = Uuid::new_v4().to_string();
        let mut session = Session::new_with_id(
            session_id.clone(),
            "Active session".to_string(),
            "agent".to_string(),
            SessionConfig::default(),
        );
        session.state = SessionState::Processing {
            current_turn_id: "turn-2".to_string(),
            phase: ProcessingPhase::Thinking,
        };
        manager.sessions.insert(session_id.clone(), session);

        let updated = manager
            .update_session_state_for_turn_if_processing(&session_id, "turn-1", SessionState::Idle)
            .await
            .expect("conditional state update should not fail");

        let session = manager
            .get_session(&session_id)
            .expect("session should remain available");
        assert!(!updated);
        assert!(matches!(
            session.state,
            SessionState::Processing {
                ref current_turn_id,
                ..
            } if current_turn_id == "turn-2"
        ));
    }

    #[tokio::test]
    async fn update_session_state_for_turn_if_processing_updates_matching_turn() {
        let manager = in_memory_test_manager();
        let session_id = Uuid::new_v4().to_string();
        let mut session = Session::new_with_id(
            session_id.clone(),
            "Active session".to_string(),
            "agent".to_string(),
            SessionConfig::default(),
        );
        session.state = SessionState::Processing {
            current_turn_id: "turn-1".to_string(),
            phase: ProcessingPhase::Thinking,
        };
        manager.sessions.insert(session_id.clone(), session);

        let updated = manager
            .update_session_state_for_turn_if_processing(&session_id, "turn-1", SessionState::Idle)
            .await
            .expect("conditional state update should not fail");

        let session = manager
            .get_session(&session_id)
            .expect("session should remain available");
        assert!(updated);
        assert!(matches!(session.state, SessionState::Idle));
    }

    #[tokio::test]
    async fn append_completed_local_command_turn_persists_without_model_context() {
        let workspace = TestWorkspace::new();
        let persistence_manager = Arc::new(
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager"),
        );
        let manager = test_manager(persistence_manager.clone());
        let session = manager
            .create_session(
                "Usage session".to_string(),
                "agent".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("session should create");

        let turn = manager
            .append_completed_local_command_turn(
                &session.session_id,
                "# Session Usage Report".to_string(),
                Some("local-usage-1".to_string()),
                Some(42),
                Some(json!({
                    "localCommandKind": "usage_report",
                    "modelVisible": false,
                })),
            )
            .await
            .expect("local command turn should persist");

        assert_eq!(turn.kind, DialogTurnKind::LocalCommand);
        assert_eq!(turn.status, TurnStatus::Completed);

        let active = manager
            .get_session(&session.session_id)
            .expect("session should remain active");
        assert_eq!(active.dialog_turn_ids, vec!["local-usage-1".to_string()]);
        assert!(manager
            .context_store
            .get_context_messages(&session.session_id)
            .is_empty());

        let persisted_turns = persistence_manager
            .load_session_turns(workspace.path(), &session.session_id)
            .await
            .expect("turns should load");
        assert_eq!(persisted_turns.len(), 1);
        assert_eq!(persisted_turns[0].kind, DialogTurnKind::LocalCommand);
        assert!(SessionManager::build_messages_from_turns(&persisted_turns).is_empty());

        let metadata = persistence_manager
            .load_session_metadata(workspace.path(), &session.session_id)
            .await
            .expect("metadata should load")
            .expect("metadata should exist");
        assert_eq!(metadata.turn_count, 1);
    }

    #[tokio::test]
    async fn restore_session_resets_processing_state_without_marking_unread_completion() {
        let workspace = TestWorkspace::new();
        let persistence_manager = Arc::new(
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager"),
        );
        let session_id = Uuid::new_v4().to_string();
        let mut session = Session::new_with_id(
            session_id.clone(),
            "Legacy processing session".to_string(),
            "agent".to_string(),
            SessionConfig {
                workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );
        session.state = SessionState::Processing {
            current_turn_id: "turn-1".to_string(),
            phase: ProcessingPhase::Thinking,
        };

        persistence_manager
            .save_session(workspace.path(), &session)
            .await
            .expect("session should save");
        persistence_manager
            .save_session_state(workspace.path(), &session_id, &session.state)
            .await
            .expect("processing state should save");

        let manager = test_manager(persistence_manager.clone());
        let restored = manager
            .restore_session(workspace.path(), &session_id)
            .await
            .expect("session should restore");
        let metadata = persistence_manager
            .load_session_metadata(workspace.path(), &session_id)
            .await
            .expect("metadata should load")
            .expect("metadata should exist");

        assert!(matches!(restored.state, SessionState::Idle));
        assert_eq!(metadata.unread_completion, None);
    }

    #[tokio::test]
    async fn ephemeral_child_session_is_kept_in_memory_without_persisting() {
        let workspace = TestWorkspace::new();
        let persistence_manager = Arc::new(
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager"),
        );
        let manager = test_manager(persistence_manager.clone());

        let session = manager
            .create_session_with_id_and_details(
                Some(Uuid::new_v4().to_string()),
                "Side thread".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                    ..Default::default()
                },
                Some("session-parent".to_string()),
                SessionKind::EphemeralChild,
            )
            .await
            .expect("ephemeral child session should create");

        assert!(manager.get_session(&session.session_id).is_some());
        assert!(persistence_manager
            .load_session_metadata(workspace.path(), &session.session_id)
            .await
            .expect("metadata lookup should succeed")
            .is_none());
    }

    #[tokio::test]
    async fn persist_session_lineage_updates_structured_relationship_and_clears_legacy_projection()
    {
        let workspace = TestWorkspace::new();
        let persistence_manager = Arc::new(
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager"),
        );
        let manager = test_manager(persistence_manager.clone());

        let session = manager
            .create_session_with_id_and_details(
                Some(Uuid::new_v4().to_string()),
                "Review child".to_string(),
                "CodeReview".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                    ..Default::default()
                },
                Some("session-parent".to_string()),
                SessionKind::Standard,
            )
            .await
            .expect("session should create");

        manager
            .merge_session_custom_metadata(
                &session.session_id,
                json!({
                    "kind": "review",
                    "parentSessionId": "stale-parent",
                    "parentRequestId": "stale-request",
                    "parentDialogTurnId": "stale-turn",
                    "parentTurnIndex": 1,
                    "parentToolCallId": "stale-tool",
                    "subagentType": "stale-subagent",
                    "preservedKey": "preserved-value",
                }),
            )
            .await
            .expect("legacy compatibility metadata should seed");

        manager
            .persist_session_lineage(
                &session.session_id,
                SessionRelationship {
                    kind: Some(SessionRelationshipKind::DeepReview),
                    parent_session_id: Some("parent-1".to_string()),
                    parent_request_id: Some("request-1".to_string()),
                    parent_dialog_turn_id: Some("turn-2".to_string()),
                    parent_turn_index: Some(2),
                    parent_tool_call_id: None,
                    subagent_type: None,
                },
            )
            .await
            .expect("lineage should persist");

        let metadata = persistence_manager
            .load_session_metadata(workspace.path(), &session.session_id)
            .await
            .expect("metadata lookup should succeed")
            .expect("metadata should exist");

        assert_eq!(
            metadata.relationship,
            Some(SessionRelationship {
                kind: Some(SessionRelationshipKind::DeepReview),
                parent_session_id: Some("parent-1".to_string()),
                parent_request_id: Some("request-1".to_string()),
                parent_dialog_turn_id: Some("turn-2".to_string()),
                parent_turn_index: Some(2),
                parent_tool_call_id: None,
                subagent_type: None,
            })
        );

        let custom_metadata = metadata
            .custom_metadata
            .expect("non-lineage custom metadata should remain");
        assert_eq!(custom_metadata["preservedKey"], "preserved-value");
        assert!(custom_metadata.get("kind").is_none());
        assert!(custom_metadata.get("parentSessionId").is_none());
        assert!(custom_metadata.get("parentRequestId").is_none());
        assert!(custom_metadata.get("parentDialogTurnId").is_none());
        assert!(custom_metadata.get("parentTurnIndex").is_none());
        assert!(custom_metadata.get("parentToolCallId").is_none());
        assert!(custom_metadata.get("subagentType").is_none());
    }

    #[tokio::test]
    async fn collect_hidden_subagent_cascade_for_parent_turns_returns_post_order_matches() {
        let workspace = TestWorkspace::new();
        let persistence_manager = Arc::new(
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager"),
        );
        let manager = test_manager(persistence_manager.clone());

        let mut matched_root = SessionMetadata::new(
            "child-root".to_string(),
            "Subagent: root".to_string(),
            "Explore".to_string(),
            "model".to_string(),
        );
        matched_root.session_kind = SessionKind::Subagent;
        matched_root.relationship = Some(SessionRelationship {
            kind: Some(SessionRelationshipKind::Subagent),
            parent_session_id: Some("parent-session".to_string()),
            parent_request_id: None,
            parent_dialog_turn_id: Some("turn-2".to_string()),
            parent_turn_index: Some(2),
            parent_tool_call_id: Some("tool-1".to_string()),
            subagent_type: Some("Explore".to_string()),
        });
        persistence_manager
            .save_session_metadata(workspace.path(), &matched_root)
            .await
            .expect("matched root should save");

        let mut matched_grandchild = SessionMetadata::new(
            "grandchild".to_string(),
            "Subagent: grandchild".to_string(),
            "Explore".to_string(),
            "model".to_string(),
        );
        matched_grandchild.session_kind = SessionKind::Subagent;
        matched_grandchild.relationship = Some(SessionRelationship {
            kind: Some(SessionRelationshipKind::Subagent),
            parent_session_id: Some("child-root".to_string()),
            parent_request_id: None,
            parent_dialog_turn_id: Some("child-turn".to_string()),
            parent_turn_index: None,
            parent_tool_call_id: Some("tool-child".to_string()),
            subagent_type: Some("Explore".to_string()),
        });
        persistence_manager
            .save_session_metadata(workspace.path(), &matched_grandchild)
            .await
            .expect("grandchild should save");

        let mut unmatched_root = SessionMetadata::new(
            "child-other-turn".to_string(),
            "Subagent: other turn".to_string(),
            "Explore".to_string(),
            "model".to_string(),
        );
        unmatched_root.session_kind = SessionKind::Subagent;
        unmatched_root.relationship = Some(SessionRelationship {
            kind: Some(SessionRelationshipKind::Subagent),
            parent_session_id: Some("parent-session".to_string()),
            parent_request_id: None,
            parent_dialog_turn_id: Some("turn-1".to_string()),
            parent_turn_index: Some(1),
            parent_tool_call_id: Some("tool-2".to_string()),
            subagent_type: Some("Explore".to_string()),
        });
        persistence_manager
            .save_session_metadata(workspace.path(), &unmatched_root)
            .await
            .expect("unmatched root should save");

        let mut visible_review_child = SessionMetadata::new(
            "review-child".to_string(),
            "Review child".to_string(),
            "DeepReview".to_string(),
            "model".to_string(),
        );
        visible_review_child.relationship = Some(SessionRelationship {
            kind: Some(SessionRelationshipKind::DeepReview),
            parent_session_id: Some("parent-session".to_string()),
            parent_request_id: None,
            parent_dialog_turn_id: Some("turn-2".to_string()),
            parent_turn_index: Some(2),
            parent_tool_call_id: None,
            subagent_type: None,
        });
        persistence_manager
            .save_session_metadata(workspace.path(), &visible_review_child)
            .await
            .expect("visible review child should save");

        let matched_turn_ids = HashSet::from(["turn-2".to_string()]);
        let cascade = manager
            .collect_hidden_subagent_cascade_for_parent_turns(
                workspace.path(),
                "parent-session",
                &matched_turn_ids,
            )
            .await
            .expect("cascade lookup should succeed");

        assert_eq!(
            cascade,
            vec!["grandchild".to_string(), "child-root".to_string()]
        );
    }

    #[tokio::test]
    async fn restore_session_view_loads_turns_without_restoring_runtime_context() {
        let workspace = TestWorkspace::new();
        let persistence_manager = Arc::new(
            PersistenceManager::new(Arc::new(PathManager::new().expect("path manager")))
                .expect("persistence manager"),
        );
        let manager = test_manager(persistence_manager.clone());
        let session_id = Uuid::new_v4().to_string();
        let mut session = Session::new_with_id(
            session_id.clone(),
            "Large history".to_string(),
            "agent".to_string(),
            SessionConfig {
                workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );
        session.dialog_turn_ids = vec!["turn-1".to_string()];

        persistence_manager
            .save_session(workspace.path(), &session)
            .await
            .expect("session should save");
        let turn = DialogTurnData::new(
            "turn-1".to_string(),
            0,
            session_id.clone(),
            UserMessageData {
                id: "turn-1-user".to_string(),
                content: "hello".to_string(),
                timestamp: 1,
                metadata: None,
            },
        );
        persistence_manager
            .save_dialog_turn(workspace.path(), &turn)
            .await
            .expect("turn should save");
        persistence_manager
            .save_turn_context_snapshot(
                workspace.path(),
                &session_id,
                0,
                &[Message::user("snapshot prompt".to_string())],
            )
            .await
            .expect("context snapshot should save");

        let (view_session, turns) = manager
            .restore_session_view(workspace.path(), &session_id)
            .await
            .expect("session view should restore");

        assert_eq!(view_session.dialog_turn_ids, vec!["turn-1".to_string()]);
        assert_eq!(turns.len(), 1);
        assert!(manager.get_session(&session_id).is_none());
        assert!(manager
            .context_store
            .get_context_messages(&session_id)
            .is_empty());
    }

    #[tokio::test]
    async fn restore_session_view_preserves_full_visible_tool_result_payload() {
        let workspace = TestWorkspace::new();
        let persistence_manager = Arc::new(
            PersistenceManager::new(Arc::new(PathManager::new().expect("path manager")))
                .expect("persistence manager"),
        );
        let manager = test_manager(persistence_manager.clone());
        let session_id = Uuid::new_v4().to_string();
        let mut session = Session::new_with_id(
            session_id.clone(),
            "History with tool output".to_string(),
            "agent".to_string(),
            SessionConfig {
                workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        );
        session.dialog_turn_ids = vec!["turn-1".to_string()];

        persistence_manager
            .save_session(workspace.path(), &session)
            .await
            .expect("session should save");

        let visible_output = "complete visible output ".repeat(128);
        let assistant_output = "assistant visible summary ".repeat(16);
        let mut turn = DialogTurnData::new(
            "turn-1".to_string(),
            0,
            session_id.clone(),
            UserMessageData {
                id: "turn-1-user".to_string(),
                content: "show full output".to_string(),
                timestamp: 1,
                metadata: None,
            },
        );
        turn.model_rounds.push(ModelRoundData {
            id: "round-1".to_string(),
            turn_id: "turn-1".to_string(),
            round_index: 0,
            timestamp: 1,
            text_items: vec![],
            tool_items: vec![ToolItemData {
                id: "tool-1".to_string(),
                tool_name: "Bash".to_string(),
                tool_call: ToolCallData {
                    id: "call-1".to_string(),
                    input: json!({ "command": "printf output" }),
                },
                tool_result: Some(ToolResultData {
                    result: json!({
                        "stdout": visible_output,
                        "nested": {
                            "stderr": "also visible",
                        },
                    }),
                    success: true,
                    result_for_assistant: Some(assistant_output.clone()),
                    error: None,
                    duration_ms: Some(1),
                }),
                ai_intent: None,
                start_time: 1,
                end_time: Some(2),
                duration_ms: Some(1),
                queue_wait_ms: None,
                preflight_ms: None,
                confirmation_wait_ms: None,
                execution_ms: None,
                order_index: None,
                is_subagent_item: None,
                parent_task_tool_id: None,
                subagent_session_id: None,
                subagent_model_id: None,
                subagent_model_alias: None,
                status: Some("completed".to_string()),
                interruption_reason: None,
            }],
            thinking_items: vec![],
            start_time: 1,
            end_time: Some(2),
            duration_ms: Some(1),
            provider_id: None,
            model_id: None,
            model_alias: None,
            first_chunk_ms: None,
            first_visible_output_ms: None,
            stream_duration_ms: None,
            attempt_count: None,
            failure_category: None,
            token_details: None,
            status: "completed".to_string(),
        });
        persistence_manager
            .save_dialog_turn(workspace.path(), &turn)
            .await
            .expect("turn should save");

        let (view_session, turns) = manager
            .restore_session_view(workspace.path(), &session_id)
            .await
            .expect("session view should restore");

        let restored_result = turns[0].model_rounds[0].tool_items[0]
            .tool_result
            .as_ref()
            .expect("tool result should be preserved");
        assert_eq!(view_session.dialog_turn_ids, vec!["turn-1".to_string()]);
        assert_eq!(
            restored_result.result["stdout"].as_str(),
            Some(visible_output.as_str())
        );
        assert_eq!(
            restored_result.result["nested"]["stderr"].as_str(),
            Some("also visible")
        );
        assert_eq!(
            restored_result.result_for_assistant.as_deref(),
            Some(assistant_output.as_str())
        );
        assert!(manager.get_session(&session_id).is_none());
    }

    #[tokio::test]
    async fn rollback_context_deletes_persisted_turns_from_target() {
        let workspace = TestWorkspace::new();
        let persistence_manager = Arc::new(
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager"),
        );
        let manager = test_manager(persistence_manager.clone());
        let session = manager
            .create_session(
                "Rollback session".to_string(),
                "agent".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("session should create");

        for index in 0..3 {
            let mut turn = DialogTurnData::new(
                format!("turn-{index}"),
                index,
                session.session_id.clone(),
                UserMessageData {
                    id: format!("turn-{index}-user"),
                    content: format!("prompt {index}"),
                    timestamp: index as u64,
                    metadata: None,
                },
            );
            turn.agent_type = Some(if index == 0 {
                "agentic".to_string()
            } else {
                "Plan".to_string()
            });
            persistence_manager
                .save_dialog_turn(workspace.path(), &turn)
                .await
                .expect("turn should save");
        }

        {
            let mut active = manager
                .sessions
                .get_mut(&session.session_id)
                .expect("session should be active");
            active.dialog_turn_ids = vec![
                "turn-0".to_string(),
                "turn-1".to_string(),
                "turn-2".to_string(),
            ];
            active.last_user_dialog_agent_type = Some("Plan".to_string());
        }
        persistence_manager
            .save_turn_context_snapshot(
                workspace.path(),
                &session.session_id,
                0,
                &[crate::agentic::core::Message::user("prompt 0".to_string())],
            )
            .await
            .expect("snapshot 0 should save");
        persistence_manager
            .save_turn_context_snapshot(
                workspace.path(),
                &session.session_id,
                1,
                &[
                    crate::agentic::core::Message::user("prompt 0".to_string()),
                    crate::agentic::core::Message::user("prompt 1".to_string()),
                ],
            )
            .await
            .expect("snapshot 1 should save");

        manager
            .rollback_context_to_turn_start(workspace.path(), &session.session_id, 1)
            .await
            .expect("rollback should succeed");

        let turns = persistence_manager
            .load_session_turns(workspace.path(), &session.session_id)
            .await
            .expect("turns should load");
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].user_message.content, "prompt 0");
        assert_eq!(turns[0].agent_type.as_deref(), Some("agentic"));
        assert!(persistence_manager
            .load_turn_context_snapshot(workspace.path(), &session.session_id, 1)
            .await
            .expect("snapshot load should succeed")
            .is_none());

        manager.sessions.remove(&session.session_id);
        let restored = manager
            .restore_session(workspace.path(), &session.session_id)
            .await
            .expect("session should restore");
        assert_eq!(restored.dialog_turn_ids, vec!["turn-0".to_string()]);
        assert_eq!(
            restored.last_user_dialog_agent_type.as_deref(),
            Some("agentic")
        );
        assert_eq!(
            manager
                .context_store
                .get_context_messages(&session.session_id)
                .len(),
            1
        );

        let metadata = persistence_manager
            .load_session_metadata(workspace.path(), &session.session_id)
            .await
            .expect("metadata should load")
            .expect("metadata should exist");
        assert_eq!(metadata.turn_count, 1);
    }

    #[tokio::test]
    async fn rollback_to_empty_history_clears_last_user_dialog_agent_type() {
        let workspace = TestWorkspace::new();
        let persistence_manager = Arc::new(
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager"),
        );
        let manager = test_manager(persistence_manager.clone());
        let session = manager
            .create_session(
                "Rollback empty history".to_string(),
                "Plan".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("session should create");

        let mut turn = DialogTurnData::new(
            "turn-0".to_string(),
            0,
            session.session_id.clone(),
            UserMessageData {
                id: "turn-0-user".to_string(),
                content: "plan prompt".to_string(),
                timestamp: 0,
                metadata: None,
            },
        );
        turn.agent_type = Some("Plan".to_string());
        persistence_manager
            .save_dialog_turn(workspace.path(), &turn)
            .await
            .expect("turn should save");

        {
            let mut active = manager
                .sessions
                .get_mut(&session.session_id)
                .expect("session should be active");
            active.dialog_turn_ids = vec!["turn-0".to_string()];
            active.last_user_dialog_agent_type = Some("Plan".to_string());
        }

        manager
            .rollback_context_to_turn_start(workspace.path(), &session.session_id, 0)
            .await
            .expect("rollback should succeed");

        let active = manager
            .get_session(&session.session_id)
            .expect("session should remain in memory");
        assert_eq!(active.agent_type, "Plan");
        assert_eq!(active.last_user_dialog_agent_type, None);
    }

    #[tokio::test]
    async fn delete_session_removes_workspace_cache_entry() {
        let workspace = TestWorkspace::new();
        let manager = in_memory_test_manager();
        let session = manager
            .create_session(
                "Cached session".to_string(),
                "agent".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace.path().to_string_lossy().to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("session should create");

        assert_eq!(
            manager
                .session_workspace_index
                .get(&session.session_id)
                .as_deref()
                .map(|entry| local_workspace_roots_equal(entry, workspace.path())),
            Some(true)
        );

        manager
            .delete_session(workspace.path(), &session.session_id)
            .await
            .expect("session should delete");

        assert!(manager
            .session_workspace_index
            .get(&session.session_id)
            .is_none());
    }

    #[test]
    fn build_messages_from_turns_skips_model_invisible_turns() {
        use crate::service::session::{DialogTurnData, DialogTurnKind, UserMessageData};

        let turns = vec![
            DialogTurnData::new(
                "turn-1".to_string(),
                0,
                "session-1".to_string(),
                UserMessageData {
                    id: "user-1".to_string(),
                    content: "hello".to_string(),
                    timestamp: 1,
                    metadata: None,
                },
            ),
            DialogTurnData::new_with_kind(
                DialogTurnKind::ManualCompaction,
                "turn-2".to_string(),
                1,
                "session-1".to_string(),
                None,
                UserMessageData {
                    id: "user-2".to_string(),
                    content: "/compact".to_string(),
                    timestamp: 2,
                    metadata: None,
                },
            ),
            DialogTurnData::new_with_kind(
                DialogTurnKind::LocalCommand,
                "turn-3".to_string(),
                2,
                "session-1".to_string(),
                None,
                UserMessageData {
                    id: "user-3".to_string(),
                    content: "# Session Usage Report".to_string(),
                    timestamp: 3,
                    metadata: Some(serde_json::json!({
                        "localCommandKind": "usage_report",
                        "modelVisible": false
                    })),
                },
            ),
        ];

        let messages = SessionManager::build_messages_from_turns(&turns);

        assert_eq!(messages.len(), 1);
        assert!(messages[0].is_actual_user_message());
    }

    #[test]
    fn fallback_session_title_uses_sentence_break_when_available() {
        let title = SessionManager::fallback_session_title(
            "Fix the flaky integration test. Add logging for retries.",
            20,
        );

        assert_eq!(title, "Fix the flaky...");
    }

    #[test]
    fn fallback_session_title_appends_ellipsis_when_truncated_without_sentence_break() {
        let title = SessionManager::fallback_session_title(
            "Implement session title generation fallback",
            12,
        );

        assert_eq!(title, "Implement...");
    }

    #[test]
    fn fallback_session_title_uses_default_for_blank_input() {
        let title = SessionManager::fallback_session_title("   ", 20);

        assert_eq!(title, "New Session");
    }

    #[tokio::test]
    async fn records_subagent_partial_timeout_in_evidence_ledger() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(Arc::new(PathManager::new().expect("path manager")))
                .expect("persistence manager"),
        );
        let manager = test_manager(persistence_manager);

        let event = manager.record_subagent_partial_timeout(
            "session-a",
            "turn-a",
            "ReviewSecurity",
            "Found token logging before timeout.",
            Some("timeout"),
        );

        assert!(!event.event_id.is_empty());
        let events = manager.evidence_events_for_turn("session-a", "turn-a");
        assert_eq!(events, vec![event.clone()]);
        let summary = manager.evidence_summary_for_session("session-a", 10);
        assert_eq!(summary.partial_subagent_results.len(), 1);
        assert_eq!(summary.partial_subagent_results[0].event_id, event.event_id);
    }

    #[tokio::test]
    async fn prompt_cache_persists_across_session_restore() {
        let workspace = TestWorkspace::new();
        let persistence_manager =
            Arc::new(PersistenceManager::new(workspace.path_manager()).expect("persistence"));
        let manager = test_manager(persistence_manager.clone());
        let workspace_path = workspace.path().to_string_lossy().to_string();
        let session = manager
            .create_session(
                "Prompt cache".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_path),
                    ..Default::default()
                },
            )
            .await
            .expect("session should be created");
        let identity = SystemPromptCacheIdentity::new("template:agentic_mode");
        let user_context_identity = UserContextCacheIdentity::new(
            "workspace_context|workspace_instructions|workspace_memory_files|project_layout",
        );

        manager
            .remember_system_prompt(
                &session.session_id,
                identity.clone(),
                "cached system prompt".to_string(),
            )
            .await;
        manager
            .remember_user_context(
                &session.session_id,
                user_context_identity.clone(),
                "cached user context".to_string(),
            )
            .await;

        let restored_manager = test_manager(persistence_manager);
        restored_manager
            .restore_session(workspace.path(), &session.session_id)
            .await
            .expect("session should restore");

        assert_eq!(
            restored_manager
                .cached_system_prompt(&session.session_id, &identity)
                .await,
            Some("cached system prompt".to_string())
        );
        assert_eq!(
            restored_manager
                .cached_user_context(&session.session_id, &user_context_identity)
                .await,
            Some("cached user context".to_string())
        );
    }

    #[tokio::test]
    async fn prompt_cache_invalidation_removes_persisted_entries() {
        let workspace = TestWorkspace::new();
        let persistence_manager =
            Arc::new(PersistenceManager::new(workspace.path_manager()).expect("persistence"));
        let manager = test_manager(persistence_manager.clone());
        let workspace_path = workspace.path().to_string_lossy().to_string();
        let session = manager
            .create_session(
                "Prompt cache".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_path),
                    ..Default::default()
                },
            )
            .await
            .expect("session should be created");
        let identity = SystemPromptCacheIdentity::new("template:agentic_mode");
        let user_context_identity = UserContextCacheIdentity::new(
            "workspace_context|workspace_instructions|workspace_memory_files|project_layout",
        );

        manager
            .remember_system_prompt(
                &session.session_id,
                identity.clone(),
                "cached system prompt".to_string(),
            )
            .await;
        manager
            .remember_user_context(
                &session.session_id,
                user_context_identity.clone(),
                "cached user context".to_string(),
            )
            .await;

        manager
            .invalidate_prompt_cache(&session.session_id, PromptCacheScope::All, "test")
            .await;

        let restored_manager = test_manager(persistence_manager.clone());
        restored_manager
            .restore_session(workspace.path(), &session.session_id)
            .await
            .expect("session should restore");

        assert_eq!(
            restored_manager
                .cached_system_prompt(&session.session_id, &identity)
                .await,
            None
        );
        assert_eq!(
            restored_manager
                .cached_user_context(&session.session_id, &user_context_identity)
                .await,
            None
        );
        assert_eq!(
            persistence_manager
                .load_prompt_cache(workspace.path(), &session.session_id)
                .await
                .expect("prompt cache load should succeed"),
            None
        );
    }

    #[tokio::test]
    async fn prompt_cache_persistence_ttl_only_affects_cold_start_restore() {
        let workspace = TestWorkspace::new();
        let persistence_manager =
            Arc::new(PersistenceManager::new(workspace.path_manager()).expect("persistence"));
        let manager = test_manager_with_config(
            persistence_manager.clone(),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: true,
                prompt_cache_policy: PromptCachePolicy {
                    cache_ttl: None,
                    persistence_ttl: Some(Duration::from_millis(0)),
                },
            },
        );
        let workspace_path = workspace.path().to_string_lossy().to_string();
        let session = manager
            .create_session(
                "Prompt cache".to_string(),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(workspace_path),
                    ..Default::default()
                },
            )
            .await
            .expect("session should be created");
        let identity = SystemPromptCacheIdentity::new("template:agentic_mode");
        let user_context_identity = UserContextCacheIdentity::new(
            "workspace_context|workspace_instructions|workspace_memory_files|project_layout",
        );

        manager
            .remember_system_prompt(
                &session.session_id,
                identity.clone(),
                "cached system prompt".to_string(),
            )
            .await;
        manager
            .remember_user_context(
                &session.session_id,
                user_context_identity.clone(),
                "cached user context".to_string(),
            )
            .await;

        assert_eq!(
            manager
                .cached_system_prompt(&session.session_id, &identity)
                .await,
            Some("cached system prompt".to_string())
        );
        assert_eq!(
            manager
                .cached_user_context(&session.session_id, &user_context_identity)
                .await,
            Some("cached user context".to_string())
        );

        let restored_manager = test_manager_with_config(
            persistence_manager.clone(),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: true,
                prompt_cache_policy: PromptCachePolicy {
                    cache_ttl: None,
                    persistence_ttl: Some(Duration::from_millis(0)),
                },
            },
        );
        restored_manager
            .restore_session(workspace.path(), &session.session_id)
            .await
            .expect("session should restore");

        assert_eq!(
            restored_manager
                .cached_system_prompt(&session.session_id, &identity)
                .await,
            None
        );
        assert_eq!(
            restored_manager
                .cached_user_context(&session.session_id, &user_context_identity)
                .await,
            None
        );
    }
}
