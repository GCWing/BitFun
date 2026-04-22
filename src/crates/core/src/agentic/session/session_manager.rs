//! Session Manager
//!
//! Responsible for session CRUD, lifecycle management, and resource association

use crate::agentic::core::{
    CompressionState, DialogTurn, Message, MessageSemanticKind, ProcessingPhase, Session,
    SessionConfig, SessionKind, SessionState, SessionSummary, TurnStats,
};
use crate::agentic::image_analysis::ImageContextData;
use crate::agentic::persistence::PersistenceManager;
use crate::agentic::session::SessionContextStore;
use crate::infrastructure::ai::get_global_ai_client_factory;
use crate::service::config::{
    get_app_language_code, get_global_config_service, short_model_user_language_instruction,
    subscribe_config_updates, ConfigUpdateEvent,
};
use crate::service::session::{
    DialogTurnData, DialogTurnKind, ModelRoundData, TextItemData, TurnStatus, UserMessageData,
};
use crate::service::snapshot::ensure_snapshot_manager_for_workspace;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::sanitize_plain_model_output;
use dashmap::DashMap;
use log::{debug, error, info, warn};
use serde_json::json;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::time;

/// Session manager configuration
#[derive(Debug, Clone)]
pub struct SessionManagerConfig {
    pub max_active_sessions: usize,
    pub session_idle_timeout: Duration,
    pub auto_save_interval: Duration,
    pub enable_persistence: bool,
}

impl Default for SessionManagerConfig {
    fn default() -> Self {
        Self {
            max_active_sessions: 100,
            session_idle_timeout: Duration::from_secs(3600), // 1 hour
            auto_save_interval: Duration::from_secs(300),    // 5 minutes
            enable_persistence: true,
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

    /// Persistent index of session_id -> effective workspace path.
    /// Populated on session create/restore; NOT cleared on memory eviction.
    /// Allows commands that only receive a session_id (e.g. update_session_model_id)
    /// to restore an evicted session without requiring the caller to supply a path.
    session_workspace_index: Arc<DashMap<String, PathBuf>>,

    /// Sub-components
    context_store: Arc<SessionContextStore>,
    persistence_manager: Arc<PersistenceManager>,

    /// Configuration
    config: SessionManagerConfig,
}

impl SessionManager {
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

            if !assistant_text.trim().is_empty() {
                messages
                    .push(Message::assistant(assistant_text).with_turn_id(turn.turn_id.clone()));
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
        if !self.config.enable_persistence {
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

            if let Some(coordinator) =
                crate::agentic::coordination::get_global_coordinator()
            {
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

        // 3. Persist to local path (handles remote workspaces correctly)
        if self.config.enable_persistence {
            if let Some(session) = self.sessions.get(&session_id) {
                self.persistence_manager
                    .save_session(&session_storage_path, &session)
                    .await?;
            }
        }

        info!("Session created: session_name={}", session.session_name);

        Ok(session)
    }

    /// Get session
    pub fn get_session(&self, session_id: &str) -> Option<Session> {
        self.sessions.get(session_id).map(|s| s.clone())
    }

    /// Update session state
    pub async fn update_session_state(
        &self,
        session_id: &str,
        new_state: SessionState,
    ) -> BitFunResult<()> {
        let effective_path = self.effective_session_workspace_path(session_id).await;

        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.state = new_state.clone();
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();

            // Persist state changes
            if self.config.enable_persistence {
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
        } else {
            return Err(BitFunError::NotFound(format!(
                "Session not found: {}",
                session_id
            )));
        }

        Ok(())
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

        if self.config.enable_persistence {
            let Some(workspace_path) = workspace_path.as_ref() else {
                return Err(BitFunError::Session(format!(
                    "Workspace path is unavailable for session {}",
                    session_id
                )));
            };
            let Some(session) = self.sessions.get(session_id) else {
                return Err(BitFunError::NotFound(format!(
                    "Session not found: {}",
                    session_id
                )));
            };
            self.persistence_manager
                .save_session(workspace_path, &session)
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

        if self.config.enable_persistence {
            let effective_path = self.effective_session_workspace_path(session_id).await;
            if let (Some(workspace_path), Some(session)) =
                (effective_path, self.sessions.get(session_id))
            {
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

    /// Update session model id (in-memory + persistence)
    pub async fn update_session_model_id(
        &self,
        session_id: &str,
        model_id: &str,
    ) -> BitFunResult<()> {
        // If the session was evicted from memory (idle > 1h), try to restore it
        // using the workspace path recorded when it was first created/restored.
        if !self.sessions.contains_key(session_id) && self.config.enable_persistence {
            if let Some(workspace_path) = self.session_workspace_index.get(session_id) {
                debug!(
                    "Session evicted from memory, restoring for model update: session_id={}",
                    session_id
                );
                let _ = self
                    .restore_session(&workspace_path.clone(), session_id)
                    .await;
            }
        }

        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.config.model_id = Some(model_id.to_string());
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();
        } else {
            return Err(BitFunError::NotFound(format!(
                "Session not found: {}",
                session_id
            )));
        }

        if self.config.enable_persistence {
            let effective_path = self.effective_session_workspace_path(session_id).await;
            if let (Some(workspace_path), Some(session)) =
                (effective_path, self.sessions.get(session_id))
            {
                self.persistence_manager
                    .save_session(&workspace_path, &session)
                    .await?;
            }
        }

        debug!(
            "Session model id updated: session_id={}, model_id={}",
            session_id, model_id
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
        // 1. Clean up snapshot system resources (including physical snapshot files)
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

        self.context_store.delete_session(session_id);

        // 2. Delete persisted data
        if self.config.enable_persistence {
            self.persistence_manager
                .delete_session(workspace_path, session_id)
                .await?;
        }

        if let Some(cron) = crate::service::cron::get_global_cron_service() {
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
        }

        // 3. Clean up associated Terminal session
        use crate::service::terminal::TerminalApi;
        if let Ok(terminal_api) = TerminalApi::from_singleton() {
            let binding = terminal_api.session_manager().binding();
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
        }

        // 4. Remove from memory
        self.sessions.remove(session_id);

        info!("Session deletion completed: session_id={}", session_id);

        Ok(())
    }

    /// Restore session (from persistent storage)
    pub async fn restore_session(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Session> {
        // Check if session is already in memory
        let session_already_in_memory = self.sessions.contains_key(session_id);

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

        if self
            .persistence_manager
            .load_session_metadata(&session_storage_path, session_id)
            .await?
            .is_some_and(|metadata| metadata.should_hide_from_user_lists())
        {
            return Err(BitFunError::NotFound(format!(
                "Session not found: {}",
                session_id
            )));
        }

        // 1. Load session from storage
        let mut session = self
            .persistence_manager
            .load_session(&session_storage_path, session_id)
            .await?;

        // Lazy migration: if the persisted model_id is no longer usable
        // (model deleted or disabled while the session was on disk), repoint
        // it to "auto" before the session re-enters memory. The next request
        // will pick a model via the normal auto/agent/default pipeline.
        if let Some(persisted_model_id) = session.config.model_id.as_deref() {
            let trimmed = persisted_model_id.trim();
            let needs_migration = if trimmed.is_empty() {
                false
            } else if let Ok(ai_config) = get_global_config_service()
                .await
                .map_err(|e| BitFunError::config(e.to_string()))?
                .get_config::<crate::service::config::types::AIConfig>(Some("ai"))
                .await
            {
                !Self::is_session_model_id_usable(&ai_config, trimmed)
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

                if let Some(coordinator) =
                    crate::agentic::coordination::get_global_coordinator()
                {
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

        // Reset session state to Idle
        // After application restart, previous Processing state is invalid and must be reset
        if !matches!(session.state, SessionState::Idle) {
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
        let persisted_turns = self
            .persistence_manager
            .load_session_turns(&session_storage_path, session_id)
            .await?;
        let persisted_turn_ids: Vec<String> = persisted_turns
            .iter()
            .map(|turn| turn.turn_id.clone())
            .collect();
        let mut latest_turn_index: Option<usize> = None;
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
        }

        self.context_store
            .replace_context(session_id, messages.clone());

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

        info!(
            "Session restored: session_id={}, session_name={}, messages={}, context_messages={}",
            session_id,
            session.session_name,
            messages.len(),
            context_msg_count
        );

        // 4. Add to memory (will overwrite if already exists)
        self.sessions
            .insert(session_id.to_string(), session.clone());
        self.session_workspace_index
            .insert(session_id.to_string(), session_storage_path.clone());

        Ok(session)
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

        // 3) Truncate session turn list & persist
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            if session.dialog_turn_ids.len() > target_turn {
                session.dialog_turn_ids.truncate(target_turn);
            }
            session.state = SessionState::Idle;
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();

            if self.config.enable_persistence {
                self.persistence_manager
                    .save_session(workspace_path, &session)
                    .await?;
            }
        }

        // 4) Delete snapshots from target_turn (inclusive) onwards
        if self.config.enable_persistence {
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
                .filter(|summary| !matches!(summary.kind, SessionKind::Subagent))
                .collect();
            Ok(summaries)
        }
    }

    // ============ Dialog Turn Management ============

    #[allow(clippy::too_many_arguments)]
    async fn start_persisted_turn(
        &self,
        session_id: &str,
        kind: DialogTurnKind,
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
        let turn = DialogTurn::new(
            session_id.to_string(),
            turn_index,
            user_input.clone(),
            turn_id,
        );
        let turn_id = turn.turn_id.clone();

        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.dialog_turn_ids.push(turn_id.clone());
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

        if self.config.enable_persistence {
            let turn_data = DialogTurnData::new_with_kind(
                kind,
                turn_id.clone(),
                turn_index,
                session_id.to_string(),
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

            if let Some(session) = self.sessions.get(session_id) {
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

    /// Complete dialog turn
    pub async fn complete_dialog_turn(
        &self,
        session_id: &str,
        turn_id: &str,
        final_response: String,
        stats: TurnStats,
    ) -> BitFunResult<()> {
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
        if self.config.enable_persistence {
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
        if self.config.enable_persistence {
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

    /// Complete a maintenance turn and persist its synthetic model round payload.
    pub async fn complete_maintenance_turn(
        &self,
        session_id: &str,
        turn_id: &str,
        model_rounds: Vec<ModelRoundData>,
        duration_ms: u64,
    ) -> BitFunResult<()> {
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

        if self.config.enable_persistence {
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

        if self.config.enable_persistence {
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

        if let Some(mut session) = self.sessions.get_mut(child_session_id) {
            if !session
                .dialog_turn_ids
                .iter()
                .any(|existing| existing == &turn_id)
            {
                session.dialog_turn_ids.push(turn_id);
            }
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();

            if self.config.enable_persistence {
                self.persistence_manager
                    .save_session(workspace_path, &session)
                    .await?;
            }
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
        self.persist_current_turn_context_snapshot_best_effort(session_id, "context_replaced")
            .await;
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

        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.compression_state = compression_state;
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();
            if self.config.enable_persistence {
                if let Some(ref workspace_path) = effective_path {
                    self.persistence_manager
                        .save_session(workspace_path, &session)
                        .await?;
                }
            }
            Ok(())
        } else {
            Err(BitFunError::NotFound(format!(
                "Session not found: {}",
                session_id
            )))
        }
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
            let mut ticker = time::interval(interval);

            loop {
                ticker.tick().await;

                for entry in sessions.iter() {
                    let session = entry.value();
                    if let Some(workspace_path) =
                        Self::effective_workspace_path_from_config(&session.config).await
                    {
                        if let Err(e) = persistence.save_session(&workspace_path, session).await {
                            error!(
                                "Failed to auto-save session: session_id={}, error={}",
                                session.session_id, e
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

        tokio::spawn(async move {
            let mut ticker = time::interval(Duration::from_secs(60));

            loop {
                ticker.tick().await;

                let now = SystemTime::now();
                let mut expired_sessions = Vec::new();

                for entry in sessions.iter() {
                    let session = entry.value();
                    if let Ok(idle_duration) = now.duration_since(session.last_activity_at) {
                        if idle_duration > timeout {
                            expired_sessions.push(session.session_id.clone());
                        }
                    }
                }

                for session_id in expired_sessions {
                    debug!("Cleaning up expired session: session_id={}", session_id);

                    // Save before deleting
                    if enable_persistence {
                        if let Some(session) = sessions.get(&session_id) {
                            if let Some(workspace_path) =
                                Self::effective_workspace_path_from_config(&session.config).await
                            {
                                let _ = persistence.save_session(&workspace_path, &session).await;
                            }
                        }
                    }

                    context_store.delete_session(&session_id);
                    sessions.remove(&session_id);
                }
            }
        });

        debug!("Cleanup task started");
    }
}

#[cfg(test)]
mod tests {
    use super::SessionManager;
    use crate::service::session::{DialogTurnData, DialogTurnKind, UserMessageData};

    #[test]
    fn build_messages_from_turns_skips_manual_compaction_turns() {
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
                UserMessageData {
                    id: "user-2".to_string(),
                    content: "/compact".to_string(),
                    timestamp: 2,
                    metadata: None,
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
}
