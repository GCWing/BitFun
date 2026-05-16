//! Persistent state store for the announcement system.
//!
//! Reads and writes `announcement-state.json` under the supplied user config
//! directory. This stays independent from core path management.

use std::fmt;
use std::path::{Path, PathBuf};

use log::{debug, warn};
use tokio::fs;

use super::types::AnnouncementState;

#[derive(Debug)]
pub enum AnnouncementStateStoreError {
    Io(std::io::Error),
    Serialization(serde_json::Error),
}

impl fmt::Display for AnnouncementStateStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "IO error: {err}"),
            Self::Serialization(err) => write!(f, "Serialization error: {err}"),
        }
    }
}

impl std::error::Error for AnnouncementStateStoreError {}

impl From<std::io::Error> for AnnouncementStateStoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for AnnouncementStateStoreError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value)
    }
}

pub type AnnouncementStateStoreResult<T> = Result<T, AnnouncementStateStoreError>;

pub struct AnnouncementStateStore {
    state_file: PathBuf,
}

impl AnnouncementStateStore {
    pub fn new(config_dir: impl AsRef<Path>) -> Self {
        Self {
            state_file: config_dir.as_ref().join("announcement-state.json"),
        }
    }

    pub fn from_state_file(state_file: impl Into<PathBuf>) -> Self {
        Self {
            state_file: state_file.into(),
        }
    }

    pub fn state_file(&self) -> &Path {
        &self.state_file
    }

    /// Load state from disk. Returns a default state if the file does not exist
    /// or cannot be parsed, preserving the legacy best-effort contract.
    pub async fn load(&self) -> AnnouncementStateStoreResult<AnnouncementState> {
        match fs::read_to_string(&self.state_file).await {
            Ok(content) => {
                let state =
                    serde_json::from_str::<AnnouncementState>(&content).unwrap_or_else(|e| {
                        warn!("Failed to parse announcement state, using default: {}", e);
                        AnnouncementState::default()
                    });
                debug!("Loaded announcement state from {:?}", self.state_file);
                Ok(state)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("Announcement state file not found, using default");
                Ok(AnnouncementState::default())
            }
            Err(e) => {
                warn!("Failed to read announcement state file: {}", e);
                Ok(AnnouncementState::default())
            }
        }
    }

    /// Persist state to disk.
    pub async fn save(&self, state: &AnnouncementState) -> AnnouncementStateStoreResult<()> {
        if let Some(parent) = self.state_file.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
            }
        }
        let content = serde_json::to_string_pretty(state)?;
        fs::write(&self.state_file, content).await?;
        debug!("Saved announcement state to {:?}", self.state_file);
        Ok(())
    }
}
