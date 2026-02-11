//! Conversation history persistence manager

use super::types::*;
use crate::infrastructure::storage::{PersistenceService, StorageOptions};
use crate::infrastructure::PathManager;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, warn};
use std::path::PathBuf;
use std::sync::Arc;

/// Conversation persistence manager
pub struct ConversationPersistenceManager {
    persistence_service: Arc<PersistenceService>,
    #[allow(dead_code)]
    workspace_path: PathBuf,
}

impl ConversationPersistenceManager {
    /// Creates a new persistence manager.
    pub async fn new(
        path_manager: Arc<PathManager>,
        workspace_path: PathBuf,
    ) -> BitFunResult<Self> {
        let mut persistence_service =
            PersistenceService::new_project_level(path_manager.clone(), workspace_path.clone())
                .await?;

        let conversations_dir = persistence_service.base_dir().join("conversations");
        persistence_service = PersistenceService::new(conversations_dir).await?;

        Ok(Self {
            persistence_service: Arc::new(persistence_service),
            workspace_path,
        })
    }

    /// Gets the full session list.
    pub async fn get_session_list(&self) -> BitFunResult<Vec<SessionMetadata>> {
        let session_list: Option<SessionList> =
            self.persistence_service.load_json("sessions").await?;

        Ok(session_list.map(|list| list.sessions).unwrap_or_default())
    }

    /// Saves the session list.
    async fn save_session_list(&self, sessions: Vec<SessionMetadata>) -> BitFunResult<()> {
        let session_list = SessionList {
            sessions,
            last_updated: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            version: "1.0".to_string(),
        };

        self.persistence_service
            .save_json("sessions", &session_list, StorageOptions::default())
            .await
    }

    /// Updates or inserts a session into the list.
    async fn upsert_session_in_list(&self, metadata: &SessionMetadata) -> BitFunResult<()> {
        let mut sessions = self.get_session_list().await?;

        if let Some(existing) = sessions
            .iter_mut()
            .find(|s| s.session_id == metadata.session_id)
        {
            *existing = metadata.clone();
        } else {
            sessions.push(metadata.clone());
        }

        sessions.sort_by(|a, b| b.last_active_at.cmp(&a.last_active_at));

        self.save_session_list(sessions).await
    }

    /// Removes a session from the list.
    async fn remove_session_from_list(&self, session_id: &str) -> BitFunResult<()> {
        let mut sessions = self.get_session_list().await?;
        sessions.retain(|s| s.session_id != session_id);
        self.save_session_list(sessions).await
    }

    /// Saves session metadata.
    pub async fn save_session_metadata(&self, metadata: &SessionMetadata) -> BitFunResult<()> {
        let key = format!("session-{}/metadata", metadata.session_id);
        self.persistence_service
            .save_json(&key, metadata, StorageOptions::default())
            .await?;

        self.upsert_session_in_list(metadata).await?;

        Ok(())
    }

    /// Loads session metadata.
    pub async fn load_session_metadata(
        &self,
        session_id: &str,
    ) -> BitFunResult<Option<SessionMetadata>> {
        let key = format!("session-{}/metadata", session_id);
        self.persistence_service.load_json(&key).await
    }

    /// Deletes a session.
    pub async fn delete_session(&self, session_id: &str) -> BitFunResult<()> {
        debug!("Deleting session: {}", session_id);

        let session_dir = self
            .persistence_service
            .base_dir()
            .join(format!("session-{}", session_id));

        if session_dir.exists() {
            tokio::fs::remove_dir_all(&session_dir).await.map_err(|e| {
                BitFunError::service(format!("Failed to delete session directory: {}", e))
            })?;
        }

        self.remove_session_from_list(session_id).await?;

        Ok(())
    }

    /// Saves a dialog turn.
    pub async fn save_dialog_turn(&self, turn: &DialogTurnData) -> BitFunResult<()> {
        debug!(
            "Saving dialog turn: session={}, turn={}",
            turn.session_id, turn.turn_index
        );

        let key = format!(
            "session-{}/turns/turn-{:04}",
            turn.session_id, turn.turn_index
        );

        let storage_options = StorageOptions {
            create_backup: false,
            backup_count: 0,
            compress: false,
        };
        self.persistence_service
            .save_json(&key, turn, storage_options)
            .await?;

        if let Some(mut metadata) = self.load_session_metadata(&turn.session_id).await? {
            metadata.touch();

            if turn.turn_index >= metadata.turn_count {
                metadata.turn_count = turn.turn_index + 1;
            }
            metadata.message_count += 1 + turn.model_rounds.len();
            metadata.tool_call_count += turn.count_tool_calls();

            self.save_session_metadata(&metadata).await?;
        }

        Ok(())
    }

    /// Loads a single dialog turn.
    pub async fn load_dialog_turn(
        &self,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<Option<DialogTurnData>> {
        let key = format!("session-{}/turns/turn-{:04}", session_id, turn_index);
        self.persistence_service.load_json(&key).await
    }

    /// Loads all dialog turns for a session.
    pub async fn load_session_turns(&self, session_id: &str) -> BitFunResult<Vec<DialogTurnData>> {
        debug!("Loading all dialog turns for session: {}", session_id);

        let metadata = self.load_session_metadata(session_id).await?;

        if let Some(metadata) = metadata {
            let mut turns = Vec::new();

            for i in 0..metadata.turn_count {
                if let Some(turn) = self.load_dialog_turn(session_id, i).await? {
                    turns.push(turn);
                } else {
                    warn!("Missing dialog turn: session={}, turn={}", session_id, i);
                }
            }

            Ok(turns)
        } else {
            Ok(Vec::new())
        }
    }

    /// Deletes all turns after the given `turn_index` (for rollback).
    pub async fn delete_turns_after(
        &self,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<usize> {
        debug!(
            "Deleting all turns after turn {} for session: {}",
            turn_index, session_id
        );

        let metadata = self.load_session_metadata(session_id).await?;

        if let Some(mut metadata) = metadata {
            let original_turn_count = metadata.turn_count;

            let mut deleted_count = 0;
            for i in (turn_index + 1)..original_turn_count {
                let key = format!("session-{}/turns/turn-{:04}", session_id, i);

                let file_path = self
                    .persistence_service
                    .base_dir()
                    .join(&key)
                    .with_extension("json");
                if file_path.exists() {
                    if let Err(e) = tokio::fs::remove_file(&file_path).await {
                        warn!("Failed to delete Turn file: {} - {}", key, e);
                    } else {
                        deleted_count += 1;
                    }
                }
            }

            metadata.turn_count = turn_index + 1;
            metadata.touch();
            self.save_session_metadata(&metadata).await?;

            debug!(
                "Deleted {} turns, new turn_count: {}",
                deleted_count, metadata.turn_count
            );
            Ok(deleted_count)
        } else {
            warn!("Session metadata not found: {}", session_id);
            Ok(0)
        }
    }

    /// Deletes all turns starting from `turn_index` (inclusive).
    pub async fn delete_turns_from(
        &self,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<usize> {
        debug!(
            "Deleting all turns from turn {} (inclusive) for session: {}",
            turn_index, session_id
        );

        let metadata = self.load_session_metadata(session_id).await?;

        if let Some(mut metadata) = metadata {
            let original_turn_count = metadata.turn_count;

            let clamped_turn_index = turn_index.min(original_turn_count);

            let mut deleted_count = 0;
            for i in clamped_turn_index..original_turn_count {
                let key = format!("session-{}/turns/turn-{:04}", session_id, i);

                let file_path = self
                    .persistence_service
                    .base_dir()
                    .join(&key)
                    .with_extension("json");
                if file_path.exists() {
                    if let Err(e) = tokio::fs::remove_file(&file_path).await {
                        warn!("Failed to delete Turn file: {} - {}", key, e);
                    } else {
                        deleted_count += 1;
                    }
                }
            }

            metadata.turn_count = clamped_turn_index;
            metadata.touch();
            self.save_session_metadata(&metadata).await?;

            debug!(
                "Deleted {} turns, new turn_count: {}",
                deleted_count, metadata.turn_count
            );
            Ok(deleted_count)
        } else {
            warn!("Session metadata not found: {}", session_id);
            Ok(0)
        }
    }

    pub async fn load_recent_turns(
        &self,
        session_id: &str,
        count: usize,
    ) -> BitFunResult<Vec<DialogTurnData>> {
        debug!(
            "Loading recent {} dialog turns for session: {}",
            count, session_id
        );

        let metadata = self.load_session_metadata(session_id).await?;

        if let Some(metadata) = metadata {
            let start_index = if metadata.turn_count > count {
                metadata.turn_count - count
            } else {
                0
            };

            let mut turns = Vec::new();

            for i in start_index..metadata.turn_count {
                if let Some(turn) = self.load_dialog_turn(session_id, i).await? {
                    turns.push(turn);
                }
            }

            Ok(turns)
        } else {
            Ok(Vec::new())
        }
    }

    /// Updates the session's last active time.
    pub async fn touch_session(&self, session_id: &str) -> BitFunResult<()> {
        if let Some(mut metadata) = self.load_session_metadata(session_id).await? {
            metadata.touch();
            self.save_session_metadata(&metadata).await?;
        }
        Ok(())
    }

    /// Returns the storage directory.
    pub fn storage_dir(&self) -> PathBuf {
        self.persistence_service.base_dir().to_path_buf()
    }
}
