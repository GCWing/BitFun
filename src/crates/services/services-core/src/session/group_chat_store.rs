//! Group chat file and index IO owner.
//!
//! Owns the provider-neutral group chat file layout under an already-resolved
//! group-chats root (parallel to `sessions/`). `meta.json` is the authoritative
//! room record and never carries the member list; `members.json` is the single
//! source of truth for members (P1-11). `index.json` and `message-catalog.json`
//! are rebuildable derived caches.
//!
//! Contract: type-contract v1.3 §1.5 (R-GC-04) + §1.3 DTOs.

use super::group_chat_layout::{validate_room_id, GroupChatStorageLayout};
use crate::file_lock::{FileLock, FileLockError, FileLockMode};
use crate::json_store::{JsonFileStore, JsonFileStoreError};
use bitfun_runtime_ports::{GroupChatMessage, GroupChatMessageStatus, GroupChatRoom};
use log::{error, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, Weak};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::fs;
use tokio::sync::Mutex;

/// Group chat storage schema version for `meta.json`.
pub const GROUP_CHAT_STORAGE_SCHEMA_VERSION: u32 = 1;
/// Group chat index schema version for `index.json`.
pub const GROUP_CHAT_INDEX_SCHEMA_VERSION: u32 = 1;
/// Message catalog schema version for `message-catalog.json`.
pub const GROUP_CHAT_CATALOG_SCHEMA_VERSION: u32 = 1;
/// Preview length cap for message-catalog entries (contract §1.5: ≤320 chars).
pub const GROUP_CHAT_CATALOG_PREVIEW_CHAR_LIMIT: usize = 320;

/// How many times `remove_dir_all` is retried when deleting a room directory.
/// Mirrors metadata_store.rs (Windows transient handle contention).
const RETRY_REMOVE_DIR_ATTEMPTS: u32 = 5;
/// Delay between directory-removal retries.
const RETRY_REMOVE_DIR_DELAY: std::time::Duration = std::time::Duration::from_millis(50);

/// Per-room in-process lock registry (mirrors json_store.rs Weak+retain:
/// dead entries are dropped so create/delete room cycles never leak the map).
static GROUP_CHAT_ROOM_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> = OnceLock::new();

/// Rebuildable derived room index (`index.json`), mirroring StoredSessionIndexFile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredGroupChatIndexFile {
    pub schema_version: u32,
    pub updated_at: u64,
    #[serde(default)]
    pub room_count: usize,
    pub rooms: Vec<GroupChatRoom>,
}

impl StoredGroupChatIndexFile {
    pub fn new(updated_at: u64, rooms: Vec<GroupChatRoom>) -> Self {
        let room_count = rooms.len();
        Self {
            schema_version: GROUP_CHAT_INDEX_SCHEMA_VERSION,
            updated_at,
            room_count,
            rooms,
        }
    }
}

/// Message catalog entry: maps `message_id` to its message file index (P1-2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupChatCatalogEntry {
    pub message_id: String,
    pub index: usize,
    /// Preview of the message content, truncated to 320 chars (contract §1.5).
    pub preview: String,
    pub timestamp: i64,
    pub status: GroupChatMessageStatus,
}

/// Derived message preview catalog (`message-catalog.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredGroupChatMessageCatalog {
    pub schema_version: u32,
    pub updated_at: u64,
    pub entries: Vec<GroupChatCatalogEntry>,
}

impl StoredGroupChatMessageCatalog {
    pub fn new(updated_at: u64, entries: Vec<GroupChatCatalogEntry>) -> Self {
        Self {
            schema_version: GROUP_CHAT_CATALOG_SCHEMA_VERSION,
            updated_at,
            entries,
        }
    }
}

/// Message window returned by [`GroupChatStore::list_messages`].
#[derive(Debug, Clone)]
pub struct GroupChatMessagesWindow {
    pub messages: Vec<GroupChatMessage>,
    pub next_cursor: Option<usize>,
}

#[derive(Debug, Error)]
pub enum GroupChatStoreError {
    #[error(transparent)]
    Json(#[from] JsonFileStoreError),
    #[error("Invalid room ID: {0}")]
    InvalidRoomId(String),
    #[error("Room not found: {0}")]
    RoomNotFound(String),
    #[error("Failed to list indexed message paths: {source}")]
    ListIndexedMessagePaths {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to read group chats root: {source}")]
    ReadGroupChatsRoot {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to read group chats directory entry: {source}")]
    ReadGroupChatsDirectoryEntry {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to get file type: {source}")]
    GetFileType {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to create group chats directory: {source}")]
    CreateGroupChatsDir {
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to lock group chat index {path}: {source}")]
    LockGroupChatIndex {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to delete room directory: {source}")]
    DeleteRoomDir {
        #[source]
        source: std::io::Error,
    },
    #[error("Room path escapes the group chats root: path={path}, root={root}")]
    UnsafeRoomPath { path: PathBuf, root: PathBuf },
    #[error("Failed to resolve room storage path {path}: {source}")]
    ResolveRoomStoragePath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Room has no messages")]
    NoMessages,
    #[error("Message not found in catalog: {0}")]
    MessageNotFound(String),
}

impl GroupChatStoreError {
    pub fn is_deserialization(&self) -> bool {
        matches!(self, Self::Json(error) if error.is_deserialization())
    }

    pub fn is_serialization(&self) -> bool {
        matches!(self, Self::Json(error) if error.is_serialization())
    }
}

#[derive(Debug, Clone)]
pub struct GroupChatStore {
    layout: GroupChatStorageLayout,
    json_store: JsonFileStore,
}

impl GroupChatStore {
    pub fn new(group_chats_root: impl Into<PathBuf>) -> Self {
        Self {
            layout: GroupChatStorageLayout::new(group_chats_root),
            json_store: JsonFileStore,
        }
    }

    pub fn group_chats_root(&self) -> &Path {
        self.layout.group_chats_root()
    }

    fn index_path(&self) -> PathBuf {
        self.layout.index_path()
    }

    fn room_dir(&self, room_id: &str) -> PathBuf {
        self.layout.room_dir(room_id)
    }

    fn meta_path(&self, room_id: &str) -> PathBuf {
        self.layout.meta_path(room_id)
    }

    fn members_path(&self, room_id: &str) -> PathBuf {
        self.layout.members_path(room_id)
    }

    fn message_catalog_path(&self, room_id: &str) -> PathBuf {
        self.layout.message_catalog_path(room_id)
    }

    fn message_path(&self, room_id: &str, index: usize) -> PathBuf {
        self.layout.message_path(room_id, index)
    }

    async fn get_room_lock(&self, room_id: &str) -> Result<Arc<Mutex<()>>, GroupChatStoreError> {
        validate_room_id(room_id).map_err(GroupChatStoreError::InvalidRoomId)?;
        let room_dir = self.room_dir(room_id);
        let registry = GROUP_CHAT_ROOM_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut registry_guard = registry.lock().await;
        if let Some(existing) = registry_guard.get(&room_dir).and_then(Weak::upgrade) {
            return Ok(existing);
        }
        // Drop dead entries before inserting a new one (json_store.rs:398).
        registry_guard.retain(|_, weak| weak.strong_count() > 0);
        let lock = Arc::new(Mutex::new(()));
        registry_guard.insert(room_dir, Arc::downgrade(&lock));
        Ok(lock)
    }

    async fn lock_index_file(&self) -> Result<FileLock, GroupChatStoreError> {
        fs::create_dir_all(self.group_chats_root())
            .await
            .map_err(|source| GroupChatStoreError::CreateGroupChatsDir { source })?;
        let lock_path = self.group_chats_root().join(".index.lock");
        let task_path = lock_path.clone();
        tokio::task::spawn_blocking(move || FileLock::acquire(&task_path, FileLockMode::Exclusive))
            .await
            .map_err(|error| GroupChatStoreError::LockGroupChatIndex {
                path: lock_path.clone(),
                source: std::io::Error::other(error),
            })?
            .map_err(|error| GroupChatStoreError::LockGroupChatIndex {
                path: lock_path,
                source: match error {
                    FileLockError::Open(source) | FileLockError::Unavailable(source) => source,
                },
            })
    }

    async fn read_json_optional<T: serde::de::DeserializeOwned>(
        &self,
        path: &Path,
    ) -> Result<Option<T>, GroupChatStoreError> {
        self.json_store
            .read_optional(path)
            .await
            .map_err(GroupChatStoreError::from)
    }

    async fn write_json_atomic<T: serde::Serialize>(
        &self,
        path: &Path,
        value: &T,
    ) -> Result<(), GroupChatStoreError> {
        self.json_store
            .write_atomic(path, value)
            .await
            .map_err(GroupChatStoreError::from)
    }

    /// Lists all rooms by scanning room directories, reading each `meta.json`.
    /// `index.json` is a rebuildable derived cache and is not consulted here.
    ///
    /// A single damaged room must not take down the listing; it is surfaced
    /// with an `error!` log (mirrors metadata_store scan contract).
    pub async fn list_rooms(
        &self,
    ) -> Result<(Vec<GroupChatRoom>, Vec<String>), GroupChatStoreError> {
        if !self.group_chats_root().exists() {
            return Ok((Vec::new(), Vec::new()));
        }

        let mut room_ids = Vec::new();
        let mut entries = fs::read_dir(self.group_chats_root())
            .await
            .map_err(|source| GroupChatStoreError::ReadGroupChatsRoot { source })?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|source| GroupChatStoreError::ReadGroupChatsDirectoryEntry { source })?
        {
            let file_type = entry
                .file_type()
                .await
                .map_err(|source| GroupChatStoreError::GetFileType { source })?;
            if !file_type.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "index.json" || name.starts_with('.') {
                continue;
            }
            room_ids.push(name);
        }

        let mut rooms = Vec::new();
        let mut damaged_ids = Vec::new();
        for room_id in room_ids {
            match self.load_room(&room_id).await {
                Ok(room) => rooms.push(room),
                Err(error) => {
                    error!(
                        "Failed to load group chat room: room_id={}, error={}",
                        room_id, error
                    );
                    damaged_ids.push(room_id);
                }
            }
        }

        rooms.sort_by_key(|room| std::cmp::Reverse(room.last_active_at));
        Ok((rooms, damaged_ids))
    }

    /// Loads a single room: `meta.json` plus members from `members.json`
    /// (P1-11: members are never read from `meta.json`).
    pub async fn load_room(&self, room_id: &str) -> Result<GroupChatRoom, GroupChatStoreError> {
        validate_room_id(room_id).map_err(GroupChatStoreError::InvalidRoomId)?;
        let stored = self
            .read_json_optional::<StoredGroupChatMetaFile>(&self.meta_path(room_id))
            .await?
            .ok_or_else(|| GroupChatStoreError::RoomNotFound(room_id.to_string()))?;
        let mut room = stored.room;
        room.members = self.list_members(room_id).await?;
        Ok(room)
    }

    /// Reads the member list from `members.json` (P1-1/P1-11 read channel).
    /// A damaged or missing `members.json` degrades to an empty list with a
    /// warning instead of failing the room.
    pub async fn list_members(
        &self,
        room_id: &str,
    ) -> Result<Vec<bitfun_runtime_ports::GroupChatMember>, GroupChatStoreError> {
        validate_room_id(room_id).map_err(GroupChatStoreError::InvalidRoomId)?;
        match self
            .read_json_optional::<StoredGroupChatMembersFile>(&self.members_path(room_id))
            .await
        {
            Ok(Some(file)) => Ok(file.members),
            Ok(None) => Ok(Vec::new()),
            Err(error) => {
                warn!(
                    "Failed to load members for room {}, treating as empty: {}",
                    room_id, error
                );
                Ok(Vec::new())
            }
        }
    }

    /// Atomically writes `meta.json`. Members are intentionally not written
    /// here (P1-11); use [`Self::save_members`] for the member list.
    pub async fn save_room(&self, room: &GroupChatRoom) -> Result<(), GroupChatStoreError> {
        validate_room_id(&room.room_id).map_err(GroupChatStoreError::InvalidRoomId)?;
        let room_lock = self.get_room_lock(&room.room_id).await?;
        let _guard = room_lock.lock().await;
        let file = StoredGroupChatMetaFile::new(room.clone());
        self.write_json_atomic(&self.meta_path(&room.room_id), &file)
            .await
    }

    /// Atomically writes `members.json` (single source of truth, P1-11).
    pub async fn save_members(
        &self,
        room_id: &str,
        members: &[bitfun_runtime_ports::GroupChatMember],
    ) -> Result<(), GroupChatStoreError> {
        validate_room_id(room_id).map_err(GroupChatStoreError::InvalidRoomId)?;
        let room_lock = self.get_room_lock(room_id).await?;
        let _guard = room_lock.lock().await;
        let file = StoredGroupChatMembersFile {
            schema_version: GROUP_CHAT_STORAGE_SCHEMA_VERSION,
            members: members.to_vec(),
        };
        self.write_json_atomic(&self.members_path(room_id), &file)
            .await
    }

    /// Appends a message as `messages/message-{index:04}.json` and incrementally
    /// updates `message-catalog.json` (entry carries id→index mapping, P1-2).
    /// The next index is derived from the highest existing indexed file plus one.
    pub async fn append_message(
        &self,
        room_id: &str,
        message: &GroupChatMessage,
    ) -> Result<usize, GroupChatStoreError> {
        validate_room_id(room_id).map_err(GroupChatStoreError::InvalidRoomId)?;
        let room_lock = self.get_room_lock(room_id).await?;
        let _guard = room_lock.lock().await;

        let indexed = self
            .layout
            .list_indexed_message_paths(room_id)
            .await
            .map_err(|source| GroupChatStoreError::ListIndexedMessagePaths { source })?;
        let next_index = indexed.last().map(|(index, _)| index + 1).unwrap_or(0);
        self.write_json_atomic(&self.message_path(room_id, next_index), message)
            .await?;
        self.upsert_catalog_entry_locked(room_id, message, next_index)
            .await?;
        Ok(next_index)
    }

    /// Updates a message's status by resolving `message_id` through the catalog
    /// (P1-2: catalog id→index mapping locates the message file).
    pub async fn update_message_status(
        &self,
        room_id: &str,
        message_id: &str,
        status: GroupChatMessageStatus,
    ) -> Result<(), GroupChatStoreError> {
        validate_room_id(room_id).map_err(GroupChatStoreError::InvalidRoomId)?;
        let room_lock = self.get_room_lock(room_id).await?;
        let _guard = room_lock.lock().await;

        let catalog = self
            .read_json_optional::<StoredGroupChatMessageCatalog>(
                &self.message_catalog_path(room_id),
            )
            .await?;
        let catalog =
            catalog.ok_or_else(|| GroupChatStoreError::MessageNotFound(message_id.to_string()))?;
        let entry = catalog
            .entries
            .iter()
            .find(|entry| entry.message_id == message_id)
            .ok_or_else(|| GroupChatStoreError::MessageNotFound(message_id.to_string()))?;
        let index = entry.index;

        let stored = self
            .read_json_optional::<GroupChatMessage>(&self.message_path(room_id, index))
            .await?
            .ok_or_else(|| GroupChatStoreError::MessageNotFound(message_id.to_string()))?;
        let mut updated = stored;
        updated.status = status;
        self.write_json_atomic(&self.message_path(room_id, index), &updated)
            .await?;

        let mut catalog = catalog;
        if let Some(entry) = catalog
            .entries
            .iter_mut()
            .find(|entry| entry.message_id == message_id)
        {
            entry.status = status;
        }
        let updated_catalog =
            StoredGroupChatMessageCatalog::new(current_unix_ms(), catalog.entries);
        self.write_json_atomic(&self.message_catalog_path(room_id), &updated_catalog)
            .await
    }

    /// Scans the room's messages for replies that exceeded `reply_timeout_secs`
    /// while still `Pending`/`Delivered` (P1-2: timeout scan + reminder belongs
    /// to R-GC-26 consuming `group_chat.reply_timeout_secs`).
    ///
    /// Timed-out messages are updated to `Failed` (persisted) and returned so
    /// the caller can surface a timeout reminder. Healthy messages are left
    /// untouched.
    pub async fn scan_timed_out_messages(
        &self,
        room_id: &str,
        reply_timeout_secs: u64,
        now_unix_ms: i64,
    ) -> Result<Vec<GroupChatMessage>, GroupChatStoreError> {
        validate_room_id(room_id).map_err(GroupChatStoreError::InvalidRoomId)?;
        if reply_timeout_secs == 0 {
            return Ok(Vec::new());
        }
        let room_lock = self.get_room_lock(room_id).await?;
        let _guard = room_lock.lock().await;

        let indexed = self
            .layout
            .list_indexed_message_paths(room_id)
            .await
            .map_err(|source| GroupChatStoreError::ListIndexedMessagePaths { source })?;

        let timeout_ms = (reply_timeout_secs as i64).saturating_mul(1000);
        let mut timed_out = Vec::new();
        for (index, path) in indexed {
            let Ok(Some(mut message)) = self.read_json_optional::<GroupChatMessage>(&path).await
            else {
                continue;
            };
            let awaiting_reply = matches!(
                message.status,
                GroupChatMessageStatus::Pending | GroupChatMessageStatus::Delivered
            );
            if !awaiting_reply {
                continue;
            }
            if now_unix_ms.saturating_sub(message.timestamp) < timeout_ms {
                continue;
            }
            message.status = GroupChatMessageStatus::Failed;
            self.write_json_atomic(&self.message_path(room_id, index), &message)
                .await?;
            timed_out.push(message);
        }
        if !timed_out.is_empty() {
            // Keep the derived catalog consistent with the message files.
            let existing = self
                .read_json_optional::<StoredGroupChatMessageCatalog>(
                    &self.message_catalog_path(room_id),
                )
                .await?
                .unwrap_or_else(|| {
                    StoredGroupChatMessageCatalog::new(current_unix_ms(), Vec::new())
                });
            let mut entries = existing.entries;
            for timed in &timed_out {
                if let Some(entry) = entries
                    .iter_mut()
                    .find(|entry| entry.message_id == timed.message_id)
                {
                    entry.status = GroupChatMessageStatus::Failed;
                }
            }
            let updated = StoredGroupChatMessageCatalog::new(current_unix_ms(), entries);
            self.write_json_atomic(&self.message_catalog_path(room_id), &updated)
                .await?;
        }
        Ok(timed_out)
    }

    /// Reads a message window: latest `limit` messages, optionally before
    /// `cursor` (exclusive index). Returns ascending order plus `next_cursor`.
    pub async fn list_messages(
        &self,
        room_id: &str,
        limit: Option<usize>,
        cursor: Option<usize>,
    ) -> Result<GroupChatMessagesWindow, GroupChatStoreError> {
        validate_room_id(room_id).map_err(GroupChatStoreError::InvalidRoomId)?;
        let indexed = self
            .layout
            .list_indexed_message_paths(room_id)
            .await
            .map_err(|source| GroupChatStoreError::ListIndexedMessagePaths { source })?;
        if indexed.is_empty() {
            return Ok(GroupChatMessagesWindow {
                messages: Vec::new(),
                next_cursor: None,
            });
        }

        let limit = limit.unwrap_or(50).max(1);
        let end_idx = cursor.unwrap_or(indexed.len());
        let start_idx = end_idx.saturating_sub(limit);
        let mut messages = Vec::with_capacity(end_idx - start_idx);
        for (index, path) in indexed.iter().skip(start_idx).take(end_idx - start_idx) {
            if let Ok(Some(message)) = self.read_json_optional::<GroupChatMessage>(path).await {
                messages.push(message);
            } else {
                warn!(
                    "Failed to load group chat message: room_id={}, index={}",
                    room_id, index
                );
            }
        }
        let next_cursor = if start_idx > 0 { Some(start_idx) } else { None };
        Ok(GroupChatMessagesWindow {
            messages,
            next_cursor,
        })
    }

    /// Cascade deletes the room directory (meta/members/catalog/messages) and
    /// rebuilds `index.json` (R-GC-25 consumer). Path escape is rejected.
    pub async fn delete_room(&self, room_id: &str) -> Result<(), GroupChatStoreError> {
        validate_room_id(room_id).map_err(GroupChatStoreError::InvalidRoomId)?;
        let room_lock = self.get_room_lock(room_id).await?;
        let _guard = room_lock.lock().await;
        let _file_guard = self.lock_index_file().await?;

        let dir = self.room_dir(room_id);
        if dir.exists() {
            let root = fs::canonicalize(self.group_chats_root())
                .await
                .map_err(|source| GroupChatStoreError::ResolveRoomStoragePath {
                    path: self.group_chats_root().to_path_buf(),
                    source,
                })?;
            let resolved_dir = fs::canonicalize(&dir).await.map_err(|source| {
                GroupChatStoreError::ResolveRoomStoragePath {
                    path: dir.clone(),
                    source,
                }
            })?;
            if resolved_dir == root || !resolved_dir.starts_with(&root) {
                return Err(GroupChatStoreError::UnsafeRoomPath {
                    path: resolved_dir,
                    root,
                });
            }
            let mut last_error: Option<std::io::Error> = None;
            for attempt in 0..RETRY_REMOVE_DIR_ATTEMPTS {
                match fs::remove_dir_all(&dir).await {
                    Ok(()) => {
                        last_error = None;
                        break;
                    }
                    Err(source) => {
                        last_error = Some(source);
                        if attempt + 1 < RETRY_REMOVE_DIR_ATTEMPTS {
                            tokio::time::sleep(RETRY_REMOVE_DIR_DELAY).await;
                        }
                    }
                }
            }
            if let Some(source) = last_error {
                return Err(GroupChatStoreError::DeleteRoomDir { source });
            }
        }

        self.rebuild_index_locked().await?;
        Ok(())
    }

    /// Rebuilds `index.json` from authoritative `meta.json` files.
    pub async fn rebuild_index(&self) -> Result<(), GroupChatStoreError> {
        let _file_guard = self.lock_index_file().await?;
        self.rebuild_index_locked().await
    }

    async fn rebuild_index_locked(&self) -> Result<(), GroupChatStoreError> {
        let (rooms, _) = self.list_rooms().await?;
        let index = StoredGroupChatIndexFile::new(current_unix_ms(), rooms);
        self.write_json_atomic(&self.index_path(), &index).await
    }

    /// Reads the rebuildable index, rebuilding from `meta.json` when missing or
    /// corrupted (deserialization failure only; real IO errors propagate).
    pub async fn read_or_rebuild_index(
        &self,
    ) -> Result<(StoredGroupChatIndexFile, bool), GroupChatStoreError> {
        let _file_guard = self.lock_index_file().await?;
        let index_path = self.index_path();
        match self
            .read_json_optional::<StoredGroupChatIndexFile>(&index_path)
            .await
        {
            Ok(Some(index)) => Ok((index, false)),
            Ok(None) => {
                self.rebuild_index_locked().await?;
                let index = self
                    .read_json_optional::<StoredGroupChatIndexFile>(&index_path)
                    .await?
                    .ok_or_else(|| {
                        GroupChatStoreError::Json(JsonFileStoreError::Deserialize {
                            path: index_path,
                            source: serde_json::Error::io(std::io::Error::other(
                                "index rebuild produced no file",
                            )),
                        })
                    })?;
                Ok((index, true))
            }
            Err(error) if error.is_deserialization() => {
                warn!(
                    "Group chat index is unreadable; rebuilding from room metadata: path={}, error={}",
                    index_path.display(),
                    error
                );
                self.rebuild_index_locked().await?;
                let index = self
                    .read_json_optional::<StoredGroupChatIndexFile>(&index_path)
                    .await?
                    .ok_or_else(|| {
                        GroupChatStoreError::Json(JsonFileStoreError::Deserialize {
                            path: index_path,
                            source: serde_json::Error::io(std::io::Error::other(
                                "index rebuild produced no file",
                            )),
                        })
                    })?;
                Ok((index, true))
            }
            Err(error) => Err(error),
        }
    }

    async fn upsert_catalog_entry_locked(
        &self,
        room_id: &str,
        message: &GroupChatMessage,
        index: usize,
    ) -> Result<(), GroupChatStoreError> {
        let existing = self
            .read_json_optional::<StoredGroupChatMessageCatalog>(
                &self.message_catalog_path(room_id),
            )
            .await?
            .unwrap_or_else(|| StoredGroupChatMessageCatalog::new(current_unix_ms(), Vec::new()));

        let mut entries = existing.entries;
        entries.retain(|entry| entry.message_id != message.message_id);
        entries.push(GroupChatCatalogEntry {
            message_id: message.message_id.clone(),
            index,
            preview: truncate_chars(&message.content, GROUP_CHAT_CATALOG_PREVIEW_CHAR_LIMIT),
            timestamp: message.timestamp,
            status: message.status,
        });
        entries.sort_by_key(|entry| entry.index);

        let catalog = StoredGroupChatMessageCatalog::new(current_unix_ms(), entries);
        self.write_json_atomic(&self.message_catalog_path(room_id), &catalog)
            .await
    }
}

/// Meta file envelope: `meta.json` contains the room without members (P1-11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredGroupChatMetaFile {
    pub schema_version: u32,
    #[serde(flatten)]
    pub room: GroupChatRoom,
}

impl StoredGroupChatMetaFile {
    pub fn new(room: GroupChatRoom) -> Self {
        Self {
            schema_version: GROUP_CHAT_STORAGE_SCHEMA_VERSION,
            room,
        }
    }
}

/// Members file envelope: `members.json` (single source of truth, P1-11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredGroupChatMembersFile {
    pub schema_version: u32,
    pub members: Vec<bitfun_runtime_ports::GroupChatMember>,
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn truncate_chars(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}
