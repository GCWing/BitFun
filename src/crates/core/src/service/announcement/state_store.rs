//! Persistent state store for the announcement system.
//!
//! Reads and writes `announcement-state.json` in the user config directory.

use super::types::AnnouncementState;
use crate::infrastructure::app_paths::PathManager;
use crate::util::errors::BitFunResult;
use log::{debug, warn};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;

pub struct AnnouncementStateStore {
    state_file: PathBuf,
}

impl AnnouncementStateStore {
    pub fn new(path_manager: &Arc<PathManager>) -> Self {
        let state_file = path_manager
            .user_config_dir()
            .join("announcement-state.json");
        Self { state_file }
    }

    /// Load state from disk.  Returns a default state if the file does not exist.
    pub async fn load(&self) -> BitFunResult<AnnouncementState> {
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
    pub async fn save(&self, state: &AnnouncementState) -> BitFunResult<()> {
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
