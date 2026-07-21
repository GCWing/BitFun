//! Local account sync cursors and upload content hashes.
//!
//! Persists per-user state under `~/.bitfun/account_sync/` so incremental
//! `?since=` pulls and upload dedupe survive app restarts. Not secret —
//! hashes are of plaintext session bundles; cursors are relay version ints.
//!
//! Session sync state (`<user>.json`) and the settings sync cursor
//! (`<user>.settings.json`) live in separate files on purpose: the session
//! backup loop and the settings sync engine are independent writers, so a
//! shared read-modify-write file could drop one writer's update.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// On-disk sync progress for one account.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountSyncState {
    /// Highest relay session `version` successfully processed by a pull.
    #[serde(default)]
    pub last_session_since: i64,
    /// Last successfully uploaded content hash per session_id.
    #[serde(default)]
    pub uploaded_hashes: HashMap<String, String>,
}

/// Settings sync progress for one account: the cloud settings blob version
/// this device last uploaded or applied, plus the content hash of that blob.
/// Lets the periodic pull skip unchanged blobs across restarts and the push
/// path skip unchanged content.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettingsCursor {
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub hash: String,
}

/// SHA-256 hex digest of session bundle plaintext (stable skip key).
pub fn content_hash(plaintext: &str) -> String {
    let digest = Sha256::digest(plaintext.as_bytes());
    hex::encode(digest)
}

fn sync_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot determine home directory"))?;
    Ok(home.join(".bitfun").join("account_sync"))
}

fn safe_user_id(user_id: &str) -> String {
    user_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn sync_state_path(user_id: &str) -> Result<PathBuf> {
    Ok(sync_dir()?.join(format!("{}.json", safe_user_id(user_id))))
}

fn settings_cursor_path(user_id: &str) -> Result<PathBuf> {
    Ok(sync_dir()?.join(format!("{}.settings.json", safe_user_id(user_id))))
}

/// Load sync state for `user_id`, or a default empty state if missing/corrupt.
pub fn load(user_id: &str) -> AccountSyncState {
    let path = match sync_state_path(user_id) {
        Ok(p) => p,
        Err(_) => return AccountSyncState::default(),
    };
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => AccountSyncState::default(),
    }
}

/// Persist sync state for `user_id`.
pub fn save(user_id: &str, state: &AccountSyncState) -> Result<()> {
    let dir = sync_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = sync_state_path(user_id)?;
    let raw = serde_json::to_string_pretty(state)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Load the settings cursor for `user_id`, defaulting when missing/corrupt.
pub fn load_settings_cursor(user_id: &str) -> SettingsCursor {
    let path = match settings_cursor_path(user_id) {
        Ok(p) => p,
        Err(_) => return SettingsCursor::default(),
    };
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => SettingsCursor::default(),
    }
}

/// Persist the settings cursor for `user_id`.
pub fn save_settings_cursor(user_id: &str, cursor: &SettingsCursor) -> Result<()> {
    let dir = sync_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = settings_cursor_path(user_id)?;
    let raw = serde_json::to_string_pretty(cursor)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

impl AccountSyncState {
    pub fn uploaded_hash(&self, session_id: &str) -> Option<&str> {
        self.uploaded_hashes.get(session_id).map(String::as_str)
    }

    pub fn set_uploaded_hash(&mut self, session_id: &str, hash: String) {
        self.uploaded_hashes.insert(session_id.to_string(), hash);
    }

    pub fn clear_uploaded_hash(&mut self, session_id: &str) {
        self.uploaded_hashes.remove(session_id);
    }

    /// Advance pull cursor to the max version seen in this batch (if any).
    pub fn advance_session_since(&mut self, versions: impl IntoIterator<Item = i64>) {
        let mut max_v = self.last_session_since;
        for v in versions {
            if v > max_v {
                max_v = v;
            }
        }
        self.last_session_since = max_v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_state_deserializes() {
        let legacy = r#"{"last_session_since":7,"uploaded_hashes":{"s1":"abc"}}"#;
        let state: AccountSyncState = serde_json::from_str(legacy).unwrap();
        assert_eq!(state.last_session_since, 7);
        assert_eq!(state.uploaded_hash("s1"), Some("abc"));
    }

    #[test]
    fn settings_cursor_round_trips() {
        let cursor = SettingsCursor {
            version: 42,
            hash: "deadbeef".to_string(),
        };
        let raw = serde_json::to_string(&cursor).unwrap();
        let back: SettingsCursor = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.version, 42);
        assert_eq!(back.hash, "deadbeef");
    }

    #[test]
    fn settings_cursor_defaults_when_missing_fields() {
        let cursor: SettingsCursor = serde_json::from_str("{}").unwrap();
        assert_eq!(cursor.version, 0);
        assert!(cursor.hash.is_empty());
    }
}
