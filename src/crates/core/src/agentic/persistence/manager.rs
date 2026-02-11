//! Persistence Manager
//!
//! Responsible for persistent storage of sessions, messages, and tool states

use crate::agentic::core::{DialogTurn, Message, Session, SessionState, SessionSummary};
use crate::infrastructure::PathManager;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, info, warn};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct PersistenceManager {
    path_manager: Arc<PathManager>,
    base_path: PathBuf,
}

impl PersistenceManager {
    pub fn new(path_manager: Arc<PathManager>) -> BitFunResult<Self> {
        let base_path = path_manager.user_data_dir().join("sessions");

        Ok(Self {
            path_manager,
            base_path,
        })
    }

    /// Get PathManager reference
    pub fn path_manager(&self) -> &Arc<PathManager> {
        &self.path_manager
    }

    /// Get session directory
    fn get_session_dir(&self, session_id: &str) -> PathBuf {
        self.base_path.join(session_id)
    }

    /// Ensure session directory exists
    async fn ensure_session_dir(&self, session_id: &str) -> BitFunResult<PathBuf> {
        let dir = self.get_session_dir(session_id);
        fs::create_dir_all(&dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create session directory: {}", e)))?;
        Ok(dir)
    }

    // ============ Turn context snapshot (sent to model)============

    fn context_snapshots_dir(&self, session_id: &str) -> PathBuf {
        self.get_session_dir(session_id).join("context_snapshots")
    }

    fn context_snapshot_path(&self, session_id: &str, turn_index: usize) -> PathBuf {
        self.context_snapshots_dir(session_id)
            .join(format!("turn-{:04}.json", turn_index))
    }

    pub async fn save_turn_context_snapshot(
        &self,
        session_id: &str,
        turn_index: usize,
        messages: &[Message],
    ) -> BitFunResult<()> {
        let dir = self.ensure_session_dir(session_id).await?;
        let snapshots_dir = dir.join("context_snapshots");
        fs::create_dir_all(&snapshots_dir).await.map_err(|e| {
            BitFunError::io(format!(
                "Failed to create context_snapshots directory: {}",
                e
            ))
        })?;

        let snapshot_path = self.context_snapshot_path(session_id, turn_index);
        let json = serde_json::to_string(messages).map_err(|e| {
            BitFunError::serialization(format!("Failed to serialize turn context snapshot: {}", e))
        })?;
        fs::write(&snapshot_path, json).await.map_err(|e| {
            BitFunError::io(format!("Failed to write turn context snapshot: {}", e))
        })?;
        Ok(())
    }

    pub async fn load_turn_context_snapshot(
        &self,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<Option<Vec<Message>>> {
        let snapshot_path = self.context_snapshot_path(session_id, turn_index);
        if !snapshot_path.exists() {
            return Ok(None);
        }

        let mut file = fs::File::open(&snapshot_path)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to open turn context snapshot: {}", e)))?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read turn context snapshot: {}", e)))?;

        let messages: Vec<Message> = serde_json::from_str(&content).map_err(|e| {
            BitFunError::Deserialization(format!(
                "Failed to deserialize turn context snapshot: {}",
                e
            ))
        })?;
        Ok(Some(messages))
    }

    pub async fn load_latest_turn_context_snapshot(
        &self,
        session_id: &str,
    ) -> BitFunResult<Option<(usize, Vec<Message>)>> {
        let dir = self.context_snapshots_dir(session_id);
        if !dir.exists() {
            return Ok(None);
        }

        let mut rd = fs::read_dir(&dir).await.map_err(|e| {
            BitFunError::io(format!("Failed to read context_snapshots directory: {}", e))
        })?;

        let mut latest: Option<usize> = None;
        while let Some(entry) = rd
            .next_entry()
            .await
            .map_err(|e| BitFunError::io(format!("Failed to iterate context_snapshots: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(idx_str) = stem.strip_prefix("turn-") else {
                continue;
            };
            if let Ok(idx) = idx_str.parse::<usize>() {
                latest = Some(latest.map(|v| v.max(idx)).unwrap_or(idx));
            }
        }

        let Some(turn_index) = latest else {
            return Ok(None);
        };
        let Some(messages) = self
            .load_turn_context_snapshot(session_id, turn_index)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some((turn_index, messages)))
    }

    pub async fn delete_turn_context_snapshots_from(
        &self,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<()> {
        let dir = self.context_snapshots_dir(session_id);
        if !dir.exists() {
            return Ok(());
        }

        let mut rd = fs::read_dir(&dir).await.map_err(|e| {
            BitFunError::io(format!("Failed to read context_snapshots directory: {}", e))
        })?;
        while let Some(entry) = rd
            .next_entry()
            .await
            .map_err(|e| BitFunError::io(format!("Failed to iterate context_snapshots: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(idx_str) = stem.strip_prefix("turn-") else {
                continue;
            };
            let Ok(idx) = idx_str.parse::<usize>() else {
                continue;
            };
            if idx >= turn_index {
                let _ = fs::remove_file(&path).await;
            }
        }

        Ok(())
    }

    // ============ Session Persistence ============

    /// Save session
    pub async fn save_session(&self, session: &Session) -> BitFunResult<()> {
        let dir = self.ensure_session_dir(&session.session_id).await?;
        let metadata_path = dir.join("metadata.json");

        let json = serde_json::to_string_pretty(session).map_err(|e| {
            BitFunError::serialization(format!("Failed to serialize session: {}", e))
        })?;

        fs::write(&metadata_path, json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write session metadata: {}", e)))?;

        Ok(())
    }

    /// Load session
    pub async fn load_session(&self, session_id: &str) -> BitFunResult<Session> {
        let metadata_path = self.get_session_dir(session_id).join("metadata.json");

        let json = fs::read_to_string(&metadata_path)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read session metadata: {}", e)))?;

        let session: Session = serde_json::from_str(&json).map_err(|e| {
            BitFunError::Deserialization(format!("Failed to deserialize session: {}", e))
        })?;

        Ok(session)
    }

    /// Save session state
    pub async fn save_session_state(
        &self,
        session_id: &str,
        state: &SessionState,
    ) -> BitFunResult<()> {
        let dir = self.ensure_session_dir(session_id).await?;
        let state_path = dir.join("state.json");

        let json = serde_json::to_string(state)
            .map_err(|e| BitFunError::serialization(format!("Failed to serialize state: {}", e)))?;

        fs::write(&state_path, json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write state: {}", e)))?;

        Ok(())
    }

    /// Delete session
    pub async fn delete_session(&self, session_id: &str) -> BitFunResult<()> {
        let dir = self.get_session_dir(session_id);

        if dir.exists() {
            fs::remove_dir_all(&dir).await.map_err(|e| {
                BitFunError::io(format!("Failed to delete session directory: {}", e))
            })?;
        }

        info!("Session deleted: session_id={}", session_id);
        Ok(())
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> BitFunResult<Vec<SessionSummary>> {
        let mut summaries = Vec::new();

        if !self.base_path.exists() {
            return Ok(summaries);
        }

        let mut entries = fs::read_dir(&self.base_path)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read session directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read directory entry: {}", e)))?
        {
            if entry
                .file_type()
                .await
                .map_err(|e| BitFunError::io(format!("Failed to get file type: {}", e)))?
                .is_dir()
            {
                let session_id = entry.file_name().to_string_lossy().to_string();

                match self.load_session(&session_id).await {
                    Ok(session) => {
                        summaries.push(SessionSummary {
                            session_id: session.session_id.clone(),
                            session_name: session.session_name.clone(),
                            agent_type: session.agent_type.clone(),
                            turn_count: session.dialog_turn_ids.len(),
                            created_at: session.created_at,
                            last_activity_at: session.last_activity_at,
                            state: session.state.clone(),
                        });
                    }
                    Err(e) => {
                        warn!(
                            "Failed to load session: session_id={}, error={}",
                            session_id, e
                        );
                    }
                }
            }
        }

        // Sort by last activity time in descending order
        summaries.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));

        Ok(summaries)
    }

    // ============ Message Persistence ============

    /// Append message (JSONL format)
    pub async fn append_message(&self, session_id: &str, message: &Message) -> BitFunResult<()> {
        let dir = self.ensure_session_dir(session_id).await?;
        let messages_path = dir.join("messages.jsonl");

        let json = serde_json::to_string(message).map_err(|e| {
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
        let messages_path = self.get_session_dir(session_id).join("messages.jsonl");

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
                Err(e) => {
                    warn!("Failed to deserialize message: {}", e);
                }
            }
        }

        Ok(messages)
    }

    /// Clear messages
    pub async fn clear_messages(&self, session_id: &str) -> BitFunResult<()> {
        let messages_path = self.get_session_dir(session_id).join("messages.jsonl");

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

    // ============ Compressed history persistence ============

    /// Append single compressed message (similar to append_message)
    pub async fn append_compressed_message(
        &self,
        session_id: &str,
        message: &Message,
    ) -> BitFunResult<()> {
        let dir = self.ensure_session_dir(session_id).await?;
        let compressed_path = dir.join("compressed_messages.jsonl");

        let json = serde_json::to_string(message).map_err(|e| {
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

    /// Save compressed message history (overwrite entire file, for full replacement after compression)
    pub async fn save_compressed_messages(
        &self,
        session_id: &str,
        messages: &[Message],
    ) -> BitFunResult<()> {
        let dir = self.ensure_session_dir(session_id).await?;
        let compressed_path = dir.join("compressed_messages.jsonl");

        // Create or overwrite file
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&compressed_path)
            .await
            .map_err(|e| {
                BitFunError::io(format!("Failed to open compressed message file: {}", e))
            })?;

        // Write all messages
        for message in messages {
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
            "Compressed history persisted: session_id={}, message_count={}",
            session_id,
            messages.len()
        );
        Ok(())
    }

    /// Load compressed message history
    pub async fn load_compressed_messages(
        &self,
        session_id: &str,
    ) -> BitFunResult<Option<Vec<Message>>> {
        let compressed_path = self
            .get_session_dir(session_id)
            .join("compressed_messages.jsonl");

        // If compressed file does not exist, return None
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
                Err(e) => {
                    warn!("Failed to deserialize compressed message: {}", e);
                }
            }
        }

        if messages.is_empty() {
            return Ok(None);
        }

        debug!(
            "Compressed history loaded: session_id={}, message_count={}",
            session_id,
            messages.len()
        );
        Ok(Some(messages))
    }

    /// Delete compressed message history
    pub async fn delete_compressed_messages(&self, session_id: &str) -> BitFunResult<()> {
        let compressed_path = self
            .get_session_dir(session_id)
            .join("compressed_messages.jsonl");

        if compressed_path.exists() {
            fs::remove_file(&compressed_path).await.map_err(|e| {
                BitFunError::io(format!("Failed to delete compressed message file: {}", e))
            })?;
            debug!("Compressed history file deleted: session_id={}", session_id);
        }

        Ok(())
    }

    // ============ Dialog turn persistence ============

    /// Save dialog turn
    pub async fn save_dialog_turn(&self, turn: &DialogTurn) -> BitFunResult<()> {
        let dir = self.ensure_session_dir(&turn.session_id).await?;
        let turns_dir = dir.join("turns");
        fs::create_dir_all(&turns_dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create turns directory: {}", e)))?;

        let turn_path = turns_dir.join(format!("{}.json", turn.turn_id));

        let json = serde_json::to_string_pretty(turn).map_err(|e| {
            BitFunError::serialization(format!("Failed to serialize dialog turn: {}", e))
        })?;

        fs::write(&turn_path, json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write dialog turn: {}", e)))?;

        Ok(())
    }

    /// Load dialog turn
    pub async fn load_dialog_turn(
        &self,
        session_id: &str,
        turn_id: &str,
    ) -> BitFunResult<DialogTurn> {
        let turn_path = self
            .get_session_dir(session_id)
            .join("turns")
            .join(format!("{}.json", turn_id));

        let json = fs::read_to_string(&turn_path)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read dialog turn: {}", e)))?;

        let turn: DialogTurn = serde_json::from_str(&json).map_err(|e| {
            BitFunError::Deserialization(format!("Failed to deserialize dialog turn: {}", e))
        })?;

        Ok(turn)
    }
}
