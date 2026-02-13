//! Session Manager
//!
//! Responsible for session CRUD, lifecycle management, and resource association

use crate::agentic::core::{
    CompressionState, DialogTurn, DialogTurnState, Message, ProcessingPhase, Session,
    SessionConfig, SessionState, SessionSummary, TurnStats,
};
use crate::agentic::persistence::PersistenceManager;
use crate::agentic::session::{CompressionManager, MessageHistoryManager};
use crate::infrastructure::ai::get_global_ai_client_factory;
use crate::infrastructure::get_workspace_path;
use crate::service::conversation::ConversationPersistenceManager;
use crate::service::snapshot::get_global_snapshot_manager;
use crate::util::errors::{BitFunError, BitFunResult};
use dashmap::DashMap;
use log::{debug, error, info, warn};
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

/// Session manager
pub struct SessionManager {
    /// Active sessions in memory
    sessions: Arc<DashMap<String, Session>>,

    /// Sub-components
    history_manager: Arc<MessageHistoryManager>,
    compression_manager: Arc<CompressionManager>,
    persistence_manager: Arc<PersistenceManager>,

    /// Configuration
    config: SessionManagerConfig,
}

impl SessionManager {
    pub fn new(
        history_manager: Arc<MessageHistoryManager>,
        compression_manager: Arc<CompressionManager>,
        persistence_manager: Arc<PersistenceManager>,
        config: SessionManagerConfig,
    ) -> Self {
        let enable_persistence = config.enable_persistence;

        let manager = Self {
            sessions: Arc::new(DashMap::new()),
            history_manager,
            compression_manager,
            persistence_manager,
            config,
        };

        // Start background tasks
        if enable_persistence {
            manager.spawn_auto_save_task();
        }
        manager.spawn_cleanup_task();

        manager
    }

    // ============ Session CRUD ============

    /// Create a new session
    pub async fn create_session(
        &self,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
    ) -> BitFunResult<Session> {
        self.create_session_with_id(None, session_name, agent_type, config)
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
        // Check session count limit
        if self.sessions.len() >= self.config.max_active_sessions {
            return Err(BitFunError::Validation(format!(
                "Exceeded maximum session limit: {}",
                self.config.max_active_sessions
            )));
        }

        let session = if let Some(id) = session_id {
            Session::new_with_id(id, session_name, agent_type.clone(), config)
        } else {
            Session::new(session_name, agent_type.clone(), config)
        };
        let session_id = session.session_id.clone();

        // 1. Add to memory
        self.sessions.insert(session_id.clone(), session.clone());

        // 2. Initialize message history
        self.history_manager.create_session(&session_id).await?;

        // 3. Initialize compression manager
        self.compression_manager.create_session(&session_id);

        // 4. Persist
        if self.config.enable_persistence {
            if let Some(session) = self.sessions.get(&session_id) {
                self.persistence_manager.save_session(&session).await?;
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
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.state = new_state.clone();
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();

            // Persist state changes
            if self.config.enable_persistence {
                self.persistence_manager
                    .save_session_state(session_id, &new_state)
                    .await?;
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

    /// Update session activity time
    pub fn touch_session(&self, session_id: &str) {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.last_activity_at = SystemTime::now();
        }
    }

    /// Delete session (cascade delete all resources)
    pub async fn delete_session(&self, session_id: &str) -> BitFunResult<()> {
        // 1. Clean up snapshot system resources (including physical snapshot files)
        if let Some(snapshot_manager) = get_global_snapshot_manager() {
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

        // 2. Delete message history
        self.history_manager.delete_session(session_id).await?;

        // 3. Delete persisted data
        if self.config.enable_persistence {
            self.persistence_manager.delete_session(session_id).await?;
        }

        // 4. Delete conversation history persisted data
        if let Some(workspace_path) = get_workspace_path() {
            match ConversationPersistenceManager::new(
                self.persistence_manager.path_manager().clone(),
                workspace_path,
            )
            .await
            {
                Ok(conversation_manager) => {
                    if let Err(e) = conversation_manager.delete_session(session_id).await {
                        warn!(
                            "Failed to delete conversation history persistence data: {}",
                            e
                        );
                    } else {
                        debug!(
                            "Conversation history persistence data deleted: session_id={}",
                            session_id
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to create ConversationPersistenceManager: {}", e);
                }
            }
        }

        // 5. Clean up associated Terminal session
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

        // 6. Remove from memory
        self.sessions.remove(session_id);

        info!("Session deletion completed: session_id={}", session_id);

        Ok(())
    }

    /// Restore session (from persistent storage)
    pub async fn restore_session(&self, session_id: &str) -> BitFunResult<Session> {
        // Check if session is already in memory
        let session_already_in_memory = self.sessions.contains_key(session_id);

        // 1. Load session from storage
        let mut session = self.persistence_manager.load_session(session_id).await?;

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

        // 2. Load message history - full list by turn, may already be compressed
        let mut latest_turn_index: Option<usize> = None;
        let messages = match self
            .persistence_manager
            .load_latest_turn_context_snapshot(session_id)
            .await?
        {
            Some((turn_index, msgs)) => {
                latest_turn_index = Some(turn_index);
                msgs
            }
            None => {
                // If no turn snapshot exists, fallback to messages/compressed_messages (and try to write snapshot)
                let fallback = match self
                    .persistence_manager
                    .load_compressed_messages(session_id)
                    .await?
                {
                    Some(compressed_messages) => compressed_messages,
                    None => self.persistence_manager.load_messages(session_id).await?,
                };

                if !session.dialog_turn_ids.is_empty() && self.config.enable_persistence {
                    let snapshot_turn_index = session.dialog_turn_ids.len().saturating_sub(1);
                    latest_turn_index = Some(snapshot_turn_index);
                    let _ = self
                        .persistence_manager
                        .save_turn_context_snapshot(session_id, snapshot_turn_index, &fallback)
                        .await;
                }

                fallback
            }
        };

        if messages.is_empty() {
            debug!(
                "Session {} has empty persisted messages (may be new session)",
                session_id
            );
        }

        self.history_manager
            .restore_session(session_id, messages.clone())
            .await?;

        // 3. Restore compression manager - batch restore, don't trigger persistence
        // If session already exists, delete old one first then create (ensure clean state)
        if session_already_in_memory {
            self.compression_manager.delete_session(session_id);
        }

        // Use restore_session for batch restore, avoid triggering persistence for each add_message
        self.compression_manager
            .restore_session(session_id, messages.clone());

        // If session's recorded turn count doesn't match snapshot, truncate to snapshot's turn
        if let Some(latest_turn_index) = latest_turn_index {
            let expected_turn_count = latest_turn_index + 1;
            if session.dialog_turn_ids.len() > expected_turn_count {
                warn!(
                    "Session turn count exceeds snapshot, truncating: session_id={}, turns={} -> {}",
                    session_id,
                    session.dialog_turn_ids.len(),
                    expected_turn_count
                );
                session.dialog_turn_ids.truncate(expected_turn_count);
            }
        } else if !session.dialog_turn_ids.is_empty() && messages.is_empty() {
            warn!(
                "Session has no available context snapshot and messages are empty, clearing turns: session_id={}",
                session_id
            );
            session.dialog_turn_ids.clear();
        }

        let context_msg_count = self
            .compression_manager
            .get_context_messages(session_id)
            .len();

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

        Ok(session)
    }

    /// Rollback "model context" to before the start of specified turn (i.e., keep 0..target_turn-1)
    pub async fn rollback_context_to_turn_start(
        &self,
        session_id: &str,
        target_turn: usize,
    ) -> BitFunResult<()> {
        // Ensure session is in memory (restore from persistence if necessary)
        if !self.sessions.contains_key(session_id) && self.config.enable_persistence {
            let _ = self.restore_session(session_id).await;
        }

        // 1) Load target context (target_turn == 0 => empty context)
        let messages = if target_turn == 0 {
            Vec::new()
        } else {
            self.persistence_manager
                .load_turn_context_snapshot(session_id, target_turn - 1)
                .await?
                .ok_or_else(|| {
                    BitFunError::NotFound(format!(
                        "turn context snapshot not found: session_id={} turn={}",
                        session_id,
                        target_turn - 1
                    ))
                })?
        };

        // 2) Restore history/compression context in memory
        self.history_manager
            .restore_session(session_id, messages.clone())
            .await?;
        self.compression_manager
            .restore_session(session_id, messages);

        // 3) Truncate session turn list & persist
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            if session.dialog_turn_ids.len() > target_turn {
                session.dialog_turn_ids.truncate(target_turn);
            }
            session.state = SessionState::Idle;
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();

            if self.config.enable_persistence {
                self.persistence_manager.save_session(&session).await?;
            }
        }

        // 4) Delete snapshots from target_turn (inclusive) onwards
        if self.config.enable_persistence {
            self.persistence_manager
                .delete_turn_context_snapshots_from(session_id, target_turn)
                .await?;
        }

        Ok(())
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> BitFunResult<Vec<SessionSummary>> {
        if self.config.enable_persistence {
            self.persistence_manager.list_sessions().await
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
                        turn_count: session.dialog_turn_ids.len(),
                        created_at: session.created_at,
                        last_activity_at: session.last_activity_at,
                        state: session.state.clone(),
                    }
                })
                .collect();
            Ok(summaries)
        }
    }

    // ============ Dialog Turn Management ============

    /// Start a new dialog turn
    /// turn_id: Optional frontend-specified ID, if None then backend generates
    /// Returns: turn_id
    pub async fn start_dialog_turn(
        &self,
        session_id: &str,
        user_input: String,
        turn_id: Option<String>,
    ) -> BitFunResult<String> {
        // Check if session exists
        let session = self
            .get_session(session_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Session not found: {}", session_id)))?;

        let turn_index = session.dialog_turn_ids.len();
        // Pass frontend's turnId
        let turn = DialogTurn::new(
            session_id.to_string(),
            turn_index,
            user_input.clone(),
            turn_id,
        );
        let turn_id = turn.turn_id.clone();

        // 1. Add to session and update state to Processing (includes current_turn_id)
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.dialog_turn_ids.push(turn_id.clone());
            session.state = SessionState::Processing {
                current_turn_id: turn_id.clone(),
                phase: ProcessingPhase::Starting,
            };
            session.updated_at = SystemTime::now();
            session.last_activity_at = SystemTime::now();
        }

        // 2. Add user message to history and compression managers
        let user_message = Message::user(user_input).with_turn_id(turn_id.clone());
        self.history_manager
            .add_message(session_id, user_message.clone())
            .await?;
        self.compression_manager
            .add_message(session_id, user_message)
            .await?;

        // 3. Persist
        if self.config.enable_persistence {
            self.persistence_manager.save_dialog_turn(&turn).await?;
        }

        debug!(
            "Starting dialog turn: turn_id={}, turn_index={}",
            turn_id, turn_index
        );

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
        // Load dialog turn
        let mut turn = self
            .persistence_manager
            .load_dialog_turn(session_id, turn_id)
            .await?;

        // Update state
        turn.state = DialogTurnState::Completed {
            final_response,
            total_rounds: stats.total_rounds,
        };
        turn.stats = stats;
        turn.completed_at = Some(SystemTime::now());

        if self.config.enable_persistence {
            match self.get_context_messages(session_id).await {
                Ok(context_messages) => {
                    if let Err(err) = self
                        .persistence_manager
                        .save_turn_context_snapshot(session_id, turn.turn_index, &context_messages)
                        .await
                    {
                        warn!(
                            "failed to save turn context snapshot: session_id={}, turn_index={}, err={}",
                            session_id,
                            turn.turn_index,
                            err
                        );
                    }
                }
                Err(err) => {
                    warn!(
                        "failed to build context messages for snapshot: session_id={}, turn_index={}, err={}",
                        session_id,
                        turn.turn_index,
                        err
                    );
                }
            }
        }

        // Persist
        if self.config.enable_persistence {
            self.persistence_manager.save_dialog_turn(&turn).await?;
        }

        debug!(
            "Dialog turn completed: turn_id={}, rounds={}, tools={}",
            turn_id, turn.stats.total_rounds, turn.stats.total_tools
        );

        Ok(())
    }

    // ============ Helper Methods ============

    /// Get session's message history (complete)
    pub async fn get_messages(&self, session_id: &str) -> BitFunResult<Vec<Message>> {
        self.history_manager.get_messages(session_id).await
    }

    /// Get session's context messages (may be compressed)
    pub async fn get_context_messages(&self, session_id: &str) -> BitFunResult<Vec<Message>> {
        // Get context messages from compression manager (may be compressed)
        let context_messages = self.compression_manager.get_context_messages(session_id);

        Ok(context_messages)
    }

    /// Add message to session
    pub async fn add_message(&self, session_id: &str, message: Message) -> BitFunResult<()> {
        // Add to history manager
        self.history_manager
            .add_message(session_id, message.clone())
            .await?;
        // Also add to compression manager
        self.compression_manager
            .add_message(session_id, message)
            .await?;
        Ok(())
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

    /// Get compression manager (for ExecutionEngine use)
    pub fn get_compression_manager(&self) -> Arc<CompressionManager> {
        self.compression_manager.clone()
    }

    /// Update session's compression state
    pub async fn update_compression_state(
        &self,
        session_id: &str,
        compression_state: CompressionState,
    ) -> BitFunResult<()> {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.compression_state = compression_state;
            session.updated_at = SystemTime::now();
            Ok(())
        } else {
            Err(BitFunError::NotFound(format!(
                "Session not found: {}",
                session_id
            )))
        }
    }

    /// Generate session title
    ///
    /// Generate a concise and accurate session title based on user message content using AI
    pub async fn generate_session_title(
        &self,
        user_message: &str,
        max_length: Option<usize>,
    ) -> BitFunResult<String> {
        use crate::util::types::Message;

        let max_length = max_length.unwrap_or(20);

        // Get current user locale for language setting
        let user_language = if let Some(service) = crate::service::get_global_i18n_service().await {
            service.get_current_locale().await
        } else {
            crate::service::LocaleId::ZhCN
        };

        let language_instruction = match user_language {
            crate::service::LocaleId::ZhCN => "使用简体中文",
            crate::service::LocaleId::EnUS => "Use English",
        };

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
            },
            Message {
                role: "user".to_string(),
                content: Some(user_prompt),
                reasoning_content: None,
                thinking_signature: None,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];

        // Dynamically get Agent client to generate title
        let ai_client_factory = get_global_ai_client_factory().await.map_err(|e| {
            BitFunError::AIClient(format!("Failed to get AI client factory: {}", e))
        })?;

        let ai_client = ai_client_factory
            .get_client_resolved("fast")
            .await
            .map_err(|e| BitFunError::AIClient(format!("Failed to get AI client: {}", e)))?;

        let response = ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::ai(format!("AI call failed: {}", e)))?;

        let title = response.text.trim().to_string();

        // If title is empty, use default title
        let title = if title.is_empty() {
            "New Session".to_string()
        } else {
            title
        };

        // Truncate title
        let final_title = if title.chars().count() > max_length {
            title.chars().take(max_length).collect::<String>()
        } else {
            title
        };

        Ok(final_title)
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
                    if let Err(e) = persistence.save_session(session).await {
                        error!(
                            "Failed to auto-save session: session_id={}, error={}",
                            session.session_id, e
                        );
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
                            let _ = persistence.save_session(&session).await;
                        }
                    }

                    sessions.remove(&session_id);
                }
            }
        });

        debug!("Cleanup task started");
    }
}
