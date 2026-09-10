//! Offline Session bundle writer and validator.
//!
//! The standalone data migrator cannot run the live Session runtime. It still
//! writes metadata and Turn envelopes through the same storage owners so a
//! successful import is immediately readable by Desktop, CLI, and Server.

use super::{
    DialogTurnData, SessionMetadata, SessionMetadataStore, SessionMetadataStoreError,
    SessionStorageLayout, StoredDialogTurnFile, StoredSessionMetadataFile,
    SESSION_STORAGE_SCHEMA_VERSION,
};
use crate::json_store::{JsonFileStore, JsonFileStoreError};
use openbitfun_core_types::validate_session_id;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct OfflineSessionBundle {
    pub metadata: SessionMetadata,
    pub turns: Vec<DialogTurnData>,
}

impl OfflineSessionBundle {
    pub fn validate(&self) -> Result<(), OfflineSessionImportError> {
        validate_bundle_shape(self)
    }
}

#[derive(Debug, Error)]
pub enum OfflineSessionImportError {
    #[error(transparent)]
    Metadata(#[from] SessionMetadataStoreError),
    #[error(transparent)]
    Json(#[from] JsonFileStoreError),
    #[error("Invalid Session bundle: {0}")]
    InvalidBundle(String),
}

#[derive(Debug, Clone)]
pub struct OfflineSessionImportStore {
    layout: SessionStorageLayout,
    metadata_store: SessionMetadataStore,
    json_store: JsonFileStore,
}

impl OfflineSessionImportStore {
    pub fn new(sessions_root: impl Into<PathBuf>) -> Self {
        let sessions_root = sessions_root.into();
        Self {
            layout: SessionStorageLayout::new(sessions_root.clone()),
            metadata_store: SessionMetadataStore::new(sessions_root),
            json_store: JsonFileStore,
        }
    }

    pub async fn write_bundle(
        &self,
        bundle: &OfflineSessionBundle,
    ) -> Result<(), OfflineSessionImportError> {
        validate_bundle_shape(bundle)?;
        self.metadata_store.save_metadata(&bundle.metadata).await?;
        self.layout
            .ensure_turns_dir(&bundle.metadata.session_id)
            .await
            .map_err(|error| {
                OfflineSessionImportError::InvalidBundle(format!(
                    "failed to create Turns directory: {error}"
                ))
            })?;
        for turn in &bundle.turns {
            self.json_store
                .write_atomic_strict(
                    &self.layout.turn_path(&turn.session_id, turn.turn_index),
                    &StoredDialogTurnFile::new(turn.clone()),
                )
                .await?;
        }
        Ok(())
    }

    pub async fn load_bundle(
        &self,
        session_id: &str,
    ) -> Result<Option<OfflineSessionBundle>, OfflineSessionImportError> {
        validate_session_id(session_id).map_err(OfflineSessionImportError::InvalidBundle)?;
        let Some(stored_metadata) = self
            .json_store
            .read_optional::<StoredSessionMetadataFile>(&self.layout.metadata_path(session_id))
            .await?
        else {
            return Ok(None);
        };
        if stored_metadata.schema_version > SESSION_STORAGE_SCHEMA_VERSION {
            return Err(OfflineSessionImportError::InvalidBundle(format!(
                "Session metadata schema {} is newer than supported schema {}",
                stored_metadata.schema_version, SESSION_STORAGE_SCHEMA_VERSION
            )));
        }

        let mut turns = Vec::new();
        for (file_index, path) in self
            .layout
            .list_indexed_turn_paths(session_id)
            .await
            .map_err(|error| {
                OfflineSessionImportError::InvalidBundle(format!(
                    "failed to list persisted Turns: {error}"
                ))
            })?
        {
            let stored = self
                .json_store
                .read_optional::<StoredDialogTurnFile>(&path)
                .await?
                .ok_or_else(|| {
                    OfflineSessionImportError::InvalidBundle(format!(
                        "persisted Turn disappeared while reading {}",
                        path.display()
                    ))
                })?;
            if stored.schema_version > SESSION_STORAGE_SCHEMA_VERSION {
                return Err(OfflineSessionImportError::InvalidBundle(format!(
                    "Turn schema {} is newer than supported schema {}",
                    stored.schema_version, SESSION_STORAGE_SCHEMA_VERSION
                )));
            }
            if stored.turn.turn_index != file_index {
                return Err(OfflineSessionImportError::InvalidBundle(format!(
                    "Turn file index {file_index} does not match payload index {}",
                    stored.turn.turn_index
                )));
            }
            turns.push(stored.turn);
        }
        let bundle = OfflineSessionBundle {
            metadata: stored_metadata.metadata,
            turns,
        };
        validate_bundle_shape(&bundle)?;
        Ok(Some(bundle))
    }

    pub async fn rebuild_index(&self) -> Result<(), OfflineSessionImportError> {
        self.metadata_store.rebuild_index().await?;
        Ok(())
    }

    pub fn sessions_root(&self) -> &Path {
        self.layout.sessions_root()
    }
}

fn validate_bundle_shape(bundle: &OfflineSessionBundle) -> Result<(), OfflineSessionImportError> {
    validate_session_id(&bundle.metadata.session_id)
        .map_err(OfflineSessionImportError::InvalidBundle)?;
    if bundle.metadata.turn_count != bundle.turns.len() {
        return Err(OfflineSessionImportError::InvalidBundle(format!(
            "Session {} declares {} Turns but contains {}",
            bundle.metadata.session_id,
            bundle.metadata.turn_count,
            bundle.turns.len()
        )));
    }
    let mut indices = BTreeSet::new();
    let mut turn_ids = BTreeSet::new();
    for turn in &bundle.turns {
        if turn.session_id != bundle.metadata.session_id {
            return Err(OfflineSessionImportError::InvalidBundle(format!(
                "Turn {} belongs to a different Session",
                turn.turn_id
            )));
        }
        if !indices.insert(turn.turn_index) {
            return Err(OfflineSessionImportError::InvalidBundle(format!(
                "Session {} contains duplicate Turn index {}",
                bundle.metadata.session_id, turn.turn_index
            )));
        }
        if !turn_ids.insert(turn.turn_id.as_str()) {
            return Err(OfflineSessionImportError::InvalidBundle(format!(
                "Session {} contains duplicate Turn id",
                bundle.metadata.session_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn owner_writer_round_trips_a_legacy_bundle_in_the_current_format() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../legacy-migration/tests/fixtures/v0.2.19/home/projects/c--fixture-workspace/sessions/session-1",
        );
        let stored_metadata: StoredSessionMetadataFile =
            serde_json::from_slice(&fs::read(fixture.join("metadata.json")).unwrap()).unwrap();
        let stored_turn: StoredDialogTurnFile =
            serde_json::from_slice(&fs::read(fixture.join("turns/turn-0000.json")).unwrap())
                .unwrap();
        let bundle = OfflineSessionBundle {
            metadata: stored_metadata.metadata,
            turns: vec![stored_turn.turn],
        };
        let temp_root = std::env::var_os("OPENBITFUN_TEST_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("E:/tmp"));
        fs::create_dir_all(&temp_root).unwrap();
        let temp = tempfile::Builder::new()
            .prefix("openbitfun-offline-session-")
            .tempdir_in(temp_root)
            .unwrap();
        let store = OfflineSessionImportStore::new(temp.path());

        store.write_bundle(&bundle).await.unwrap();
        let reloaded = store.load_bundle("session-1").await.unwrap().unwrap();
        reloaded.validate().unwrap();
        assert_eq!(reloaded.metadata.session_id, "session-1");
        assert_eq!(reloaded.turns.len(), 1);
        assert_eq!(reloaded.turns[0].turn_id, "turn-1");
        let stored: StoredSessionMetadataFile =
            serde_json::from_slice(&fs::read(temp.path().join("session-1/metadata.json")).unwrap())
                .unwrap();
        assert_eq!(stored.schema_version, SESSION_STORAGE_SCHEMA_VERSION);
    }
}
