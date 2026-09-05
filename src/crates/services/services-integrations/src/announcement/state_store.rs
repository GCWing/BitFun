//! Persistent state store for the announcement system.
//!
//! Reads and writes `announcement-state.json` under the supplied user config
//! directory. This stays independent from core path management.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};

use log::{debug, warn};
use openbitfun_services_core::json_store::{JsonFileStore, JsonFileStoreError};

use super::types::AnnouncementState;

const SAVE_TO_PRIMARY: u8 = 0;
const SAVE_TO_RECOVERY: u8 = 1;
const SAVE_DISABLED: u8 = 2;

#[derive(Debug)]
pub enum AnnouncementStateStoreError {
    Json(JsonFileStoreError),
    PersistenceDisabled { state_file: PathBuf },
}

impl fmt::Display for AnnouncementStateStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "{err}"),
            Self::PersistenceDisabled { state_file } => write!(
                f,
                "Announcement state persistence is disabled because both the primary and recovery files are unreadable: {}",
                state_file.display()
            ),
        }
    }
}

impl std::error::Error for AnnouncementStateStoreError {}

impl From<JsonFileStoreError> for AnnouncementStateStoreError {
    fn from(value: JsonFileStoreError) -> Self {
        Self::Json(value)
    }
}

impl AnnouncementStateStoreError {
    pub fn is_deserialization(&self) -> bool {
        matches!(self, Self::Json(error) if error.is_deserialization())
    }

    pub fn is_serialization(&self) -> bool {
        matches!(self, Self::Json(error) if error.is_serialization())
    }
}

pub type AnnouncementStateStoreResult<T> = Result<T, AnnouncementStateStoreError>;

pub struct AnnouncementStateStore {
    state_file: PathBuf,
    recovery_file: PathBuf,
    json_store: JsonFileStore,
    save_target: AtomicU8,
}

impl AnnouncementStateStore {
    pub fn new(config_dir: impl AsRef<Path>) -> Self {
        Self::from_state_file(config_dir.as_ref().join("announcement-state.json"))
    }

    pub fn from_state_file(state_file: impl Into<PathBuf>) -> Self {
        let state_file = state_file.into();
        let recovery_file = recovery_file_for(&state_file);
        Self {
            state_file,
            recovery_file,
            json_store: JsonFileStore,
            save_target: AtomicU8::new(SAVE_TO_PRIMARY),
        }
    }

    pub fn state_file(&self) -> &Path {
        &self.state_file
    }

    /// Load state from disk. Returns a default state if the file does not exist
    /// or cannot be parsed, preserving the legacy best-effort contract.
    ///
    /// An unreadable primary file is never overwritten. Subsequent saves use a
    /// recovery sidecar so one damaged record cannot either erase the original
    /// bytes or make a first-run announcement repeat forever.
    pub async fn load(&self) -> AnnouncementStateStoreResult<AnnouncementState> {
        match self
            .json_store
            .read_optional::<AnnouncementState>(&self.state_file)
            .await
        {
            Ok(Some(state)) => {
                self.save_target.store(SAVE_TO_PRIMARY, Ordering::Release);
                debug!("Loaded announcement state from {:?}", self.state_file);
                Ok(state)
            }
            Ok(None) => self.load_recovery_after_missing_primary().await,
            Err(error) => {
                warn!(
                    "Failed to read announcement state file {}; preserving it and using recovery state: {}",
                    self.state_file.display(),
                    error
                );
                self.save_target.store(SAVE_TO_RECOVERY, Ordering::Release);
                match self
                    .json_store
                    .read_optional::<AnnouncementState>(&self.recovery_file)
                    .await
                {
                    Ok(Some(state)) => {
                        debug!(
                            "Loaded announcement recovery state from {:?}",
                            self.recovery_file
                        );
                        Ok(state)
                    }
                    Ok(None) => Ok(AnnouncementState::default()),
                    Err(recovery_error) => {
                        self.save_target.store(SAVE_DISABLED, Ordering::Release);
                        warn!(
                            "Failed to read announcement recovery state {}; persistence is disabled for this process: {}",
                            self.recovery_file.display(),
                            recovery_error
                        );
                        Ok(AnnouncementState::default())
                    }
                }
            }
        }
    }

    /// Persist state using a same-directory atomic replacement.
    pub async fn save(&self, state: &AnnouncementState) -> AnnouncementStateStoreResult<()> {
        let target = match self.save_target.load(Ordering::Acquire) {
            SAVE_TO_PRIMARY => &self.state_file,
            SAVE_TO_RECOVERY => &self.recovery_file,
            _ => {
                return Err(AnnouncementStateStoreError::PersistenceDisabled {
                    state_file: self.state_file.clone(),
                });
            }
        };

        let _cross_process_lock = self.json_store.acquire_cross_process_lock(target).await?;
        self.json_store.write_atomic_strict(target, state).await?;
        debug!("Saved announcement state to {:?}", target);
        Ok(())
    }

    async fn load_recovery_after_missing_primary(
        &self,
    ) -> AnnouncementStateStoreResult<AnnouncementState> {
        self.save_target.store(SAVE_TO_PRIMARY, Ordering::Release);
        match self
            .json_store
            .read_optional::<AnnouncementState>(&self.recovery_file)
            .await
        {
            Ok(Some(state)) => {
                debug!(
                    "Primary announcement state is missing; restoring from {:?}",
                    self.recovery_file
                );
                Ok(state)
            }
            Ok(None) => {
                debug!("Announcement state file not found, using default");
                Ok(AnnouncementState::default())
            }
            Err(error) => {
                warn!(
                    "Failed to read orphaned announcement recovery state {}; preserving it and using default: {}",
                    self.recovery_file.display(),
                    error
                );
                Ok(AnnouncementState::default())
            }
        }
    }
}

fn recovery_file_for(state_file: &Path) -> PathBuf {
    let stem = state_file
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("announcement-state");
    state_file.with_file_name(format!("{stem}.recovery.json"))
}
