//! Group chat storage path layout.
//!
//! Owns stable file and directory names under an already-resolved group-chats
//! root (parallel to `sessions/` inside `~/.bitfun/projects/<slug>/`).
//!
//! Contract: type-contract v1.3 §1.5 (R-GC-03).
//! Members are stored in `members.json` as the single source of truth (P1-11);
//! `meta.json` never contains the member list. `index.json` is a rebuildable
//! derived cache; `message-catalog.json` is a derived preview cache.

use std::io;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Reuses `validate_session_id` semantics (core-types/session.rs:44-62):
/// a room id must be a single safe path component.
pub fn validate_room_id(room_id: &str) -> Result<(), String> {
    if room_id.is_empty() {
        return Err("room_id cannot be empty".to_string());
    }
    if room_id == "." || room_id == ".." {
        return Err("room_id cannot be '.' or '..'".to_string());
    }
    if room_id.contains('/') || room_id.contains('\\') {
        return Err("room_id cannot contain path separators".to_string());
    }
    if room_id.chars().any(char::is_control) {
        return Err("room_id cannot contain control characters".to_string());
    }
    let bytes = room_id.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err("room_id cannot use a drive-relative path prefix".to_string());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupChatStorageLayout {
    group_chats_root: PathBuf,
}

impl GroupChatStorageLayout {
    pub fn new(group_chats_root: impl Into<PathBuf>) -> Self {
        Self {
            group_chats_root: group_chats_root.into(),
        }
    }

    /// Root of the `group-chats/` tree (sibling of `sessions/`).
    pub fn group_chats_root(&self) -> &Path {
        &self.group_chats_root
    }

    /// Derived room index cache: `group-chats/index.json`.
    pub fn index_path(&self) -> PathBuf {
        self.group_chats_root.join("index.json")
    }

    /// Per-room directory: `group-chats/<room_id>/`.
    pub fn room_dir(&self, room_id: &str) -> PathBuf {
        self.assert_valid_room_id(room_id);
        self.group_chats_root.join(room_id)
    }

    /// Room metadata: `group-chats/<room_id>/meta.json` (never contains members).
    pub fn meta_path(&self, room_id: &str) -> PathBuf {
        self.room_dir(room_id).join("meta.json")
    }

    /// Member list (single source of truth, P1-11): `members.json`.
    pub fn members_path(&self, room_id: &str) -> PathBuf {
        self.room_dir(room_id).join("members.json")
    }

    /// Derived message preview cache: `message-catalog.json`.
    pub fn message_catalog_path(&self, room_id: &str) -> PathBuf {
        self.room_dir(room_id).join("message-catalog.json")
    }

    /// Message files directory: `messages/`.
    pub fn messages_dir(&self, room_id: &str) -> PathBuf {
        self.room_dir(room_id).join("messages")
    }

    /// Single message file with a globally increasing sequence index:
    /// `messages/message-{index:04}.json`.
    pub fn message_path(&self, room_id: &str, index: usize) -> PathBuf {
        self.messages_dir(room_id)
            .join(format!("message-{index:04}.json"))
    }

    /// Lists `message-<number>.json` files under `messages/`, ignoring any
    /// non-matching files, sorted by index ascending.
    pub async fn list_indexed_message_paths(
        &self,
        room_id: &str,
    ) -> io::Result<Vec<(usize, PathBuf)>> {
        let messages_dir = self.messages_dir(room_id);
        if !messages_dir.exists() {
            return Ok(Vec::new());
        }

        let mut indexed_paths = Vec::new();
        let mut entries = fs::read_dir(&messages_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(index_str) = stem.strip_prefix("message-") else {
                continue;
            };
            let Ok(index) = index_str.parse::<usize>() else {
                continue;
            };
            indexed_paths.push((index, path));
        }

        indexed_paths.sort_by_key(|(index, _)| *index);
        Ok(indexed_paths)
    }

    /// Complete set of paths to remove for a cascade delete (R-GC-25).
    /// Deleting the room directory recursively covers every message file.
    pub fn room_delete_paths(&self, room_id: &str) -> Vec<PathBuf> {
        self.assert_valid_room_id(room_id);
        vec![
            self.meta_path(room_id),
            self.members_path(room_id),
            self.message_catalog_path(room_id),
            self.messages_dir(room_id),
            self.room_dir(room_id),
        ]
    }

    fn assert_valid_room_id(&self, room_id: &str) {
        validate_room_id(room_id).unwrap_or_else(|error| panic!("invalid room_id: {error}"));
    }
}
