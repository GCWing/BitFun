use super::{SessionMetadataStore, SessionMetadataStoreError, StoredSessionMetadataFile};
use serde::{de::DeserializeOwned, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStoreMigrationRecord {
    pub source: PathBuf,
    pub target: PathBuf,
    pub strategy: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionStoreMigrationError {
    #[error("failed to create directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read directory {path}: {source}")]
    ReadDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to inspect directory entry under {path}: {source}")]
    ReadDirEntry {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to inspect file type for {path}: {source}")]
    FileType {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to copy {source_path} to {target_path}: {source}")]
    Copy {
        source_path: PathBuf,
        target_path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to remove {path}: {source}")]
    Remove {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read file {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write file {path}: {source}")]
    WriteFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to stat file {path}: {source}")]
    Stat {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("missing readable session metadata in {path}")]
    MissingMetadata { path: PathBuf },
    #[error("failed to deserialize JSON file {path}: {source}")]
    DeserializeJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to serialize JSON for {path}: {source}")]
    SerializeJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error(transparent)]
    MetadataStore(#[from] SessionMetadataStoreError),
}

impl SessionStoreMigrationError {
    pub fn is_metadata_deserialization(&self) -> bool {
        match self {
            Self::DeserializeJson { .. } => true,
            Self::MetadataStore(error) => error.is_deserialization(),
            _ => false,
        }
    }

    pub fn is_metadata_serialization(&self) -> bool {
        match self {
            Self::SerializeJson { .. } => true,
            Self::MetadataStore(error) => error.is_serialization(),
            _ => false,
        }
    }
}

pub async fn move_legacy_path(
    source: &Path,
    target: &Path,
) -> Result<SessionStoreMigrationRecord, SessionStoreMigrationError> {
    if let Some(parent) = target.parent() {
        create_dir_all(parent)?;
    }

    match tokio::fs::rename(source, target).await {
        Ok(()) => Ok(SessionStoreMigrationRecord {
            source: source.to_path_buf(),
            target: target.to_path_buf(),
            strategy: "rename".to_string(),
        }),
        Err(error) if source.is_dir() => {
            let _ = error;
            copy_dir_recursive(source, target)?;
            std::fs::remove_dir_all(source).map_err(|source_error| {
                SessionStoreMigrationError::Remove {
                    path: source.to_path_buf(),
                    source: source_error,
                }
            })?;
            Ok(SessionStoreMigrationRecord {
                source: source.to_path_buf(),
                target: target.to_path_buf(),
                strategy: "copy_dir".to_string(),
            })
        }
        Err(error) => {
            let _ = error;
            std::fs::copy(source, target).map_err(|source_error| {
                SessionStoreMigrationError::Copy {
                    source_path: source.to_path_buf(),
                    target_path: target.to_path_buf(),
                    source: source_error,
                }
            })?;
            std::fs::remove_file(source).map_err(|source_error| {
                SessionStoreMigrationError::Remove {
                    path: source.to_path_buf(),
                    source: source_error,
                }
            })?;
            Ok(SessionStoreMigrationRecord {
                source: source.to_path_buf(),
                target: target.to_path_buf(),
                strategy: "copy_file".to_string(),
            })
        }
    }
}

pub async fn merge_legacy_session_store(
    source: &Path,
    target: &Path,
) -> Result<Option<SessionStoreMigrationRecord>, SessionStoreMigrationError> {
    if !source.exists() {
        return Ok(None);
    }

    create_dir_all(target)?;

    for entry in
        std::fs::read_dir(source).map_err(|source_error| SessionStoreMigrationError::ReadDir {
            path: source.to_path_buf(),
            source: source_error,
        })?
    {
        let entry = entry.map_err(|source_error| SessionStoreMigrationError::ReadDirEntry {
            path: source.to_path_buf(),
            source: source_error,
        })?;
        let source_path = entry.path();
        let file_name = entry.file_name();
        let file_type =
            entry
                .file_type()
                .map_err(|source_error| SessionStoreMigrationError::FileType {
                    path: source_path.clone(),
                    source: source_error,
                })?;

        if file_name
            .to_string_lossy()
            .eq_ignore_ascii_case("index.json")
        {
            remove_path_if_exists(&source_path)?;
            continue;
        }

        if !file_type.is_dir() {
            let target_path = target.join(&file_name);
            if !target_path.exists() {
                move_path_best_effort(&source_path, &target_path)?;
            } else if files_are_equal(&source_path, &target_path)? {
                remove_path_if_exists(&source_path)?;
            } else {
                replace_target_if_source_newer(&source_path, &target_path)?;
            }
            continue;
        }

        let target_path = target.join(&file_name);
        if !target_path.exists() {
            move_path_best_effort(&source_path, &target_path)?;
            continue;
        }

        merge_session_directory(&source_path, &target_path)?;
        remove_path_if_exists(&source_path)?;
    }

    rebuild_session_index(target).await?;
    remove_path_if_exists(&source.join("index.json"))?;
    remove_path_if_exists(source)?;

    Ok(Some(SessionStoreMigrationRecord {
        source: source.to_path_buf(),
        target: target.to_path_buf(),
        strategy: "merge_sessions".to_string(),
    }))
}

fn remove_path_if_exists(path: &Path) -> Result<(), SessionStoreMigrationError> {
    if !path.exists() {
        return Ok(());
    }

    if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|source| SessionStoreMigrationError::Remove {
            path: path.to_path_buf(),
            source,
        })
    } else {
        std::fs::remove_file(path).map_err(|source| SessionStoreMigrationError::Remove {
            path: path.to_path_buf(),
            source,
        })
    }
}

fn merge_session_directory(source: &Path, target: &Path) -> Result<(), SessionStoreMigrationError> {
    create_dir_all(target)?;

    for entry in
        std::fs::read_dir(source).map_err(|source_error| SessionStoreMigrationError::ReadDir {
            path: source.to_path_buf(),
            source: source_error,
        })?
    {
        let entry = entry.map_err(|source_error| SessionStoreMigrationError::ReadDirEntry {
            path: source.to_path_buf(),
            source: source_error,
        })?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type =
            entry
                .file_type()
                .map_err(|source_error| SessionStoreMigrationError::FileType {
                    path: source_path.clone(),
                    source: source_error,
                })?;

        if file_type.is_dir() {
            if !target_path.exists() {
                move_path_best_effort(&source_path, &target_path)?;
            } else {
                merge_session_directory(&source_path, &target_path)?;
                remove_path_if_exists(&source_path)?;
            }
            continue;
        }

        if file_name_eq(&source_path, "metadata.json") && target_path.exists() {
            merge_session_metadata_file(&source_path, &target_path)?;
            remove_path_if_exists(&source_path)?;
            continue;
        }

        if !target_path.exists() {
            move_path_best_effort(&source_path, &target_path)?;
        } else if files_are_equal(&source_path, &target_path)? {
            remove_path_if_exists(&source_path)?;
        } else {
            replace_target_if_source_newer(&source_path, &target_path)?;
        }
    }

    Ok(())
}

fn merge_session_metadata_file(
    source: &Path,
    target: &Path,
) -> Result<(), SessionStoreMigrationError> {
    let source_file =
        read_json_optional_sync::<StoredSessionMetadataFile>(source)?.ok_or_else(|| {
            SessionStoreMigrationError::MissingMetadata {
                path: source.to_path_buf(),
            }
        })?;
    let target_file =
        read_json_optional_sync::<StoredSessionMetadataFile>(target)?.ok_or_else(|| {
            SessionStoreMigrationError::MissingMetadata {
                path: target.to_path_buf(),
            }
        })?;

    let chosen = if source_file.metadata.last_active_at > target_file.metadata.last_active_at {
        source_file
    } else {
        target_file
    };

    write_json_pretty_sync(target, &chosen)?;
    Ok(())
}

async fn rebuild_session_index(sessions_dir: &Path) -> Result<(), SessionStoreMigrationError> {
    if !sessions_dir.exists() {
        return Ok(());
    }

    SessionMetadataStore::new(sessions_dir)
        .rebuild_index()
        .await
        .map(|_| ())
        .map_err(SessionStoreMigrationError::from)
}

fn replace_target_if_source_newer(
    source: &Path,
    target: &Path,
) -> Result<(), SessionStoreMigrationError> {
    if source_is_newer(source, target)? {
        remove_path_if_exists(target)?;
        move_path_best_effort(source, target)
    } else {
        remove_path_if_exists(source)
    }
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), SessionStoreMigrationError> {
    create_dir_all(target)?;

    for entry in
        std::fs::read_dir(source).map_err(|source_error| SessionStoreMigrationError::ReadDir {
            path: source.to_path_buf(),
            source: source_error,
        })?
    {
        let entry = entry.map_err(|source_error| SessionStoreMigrationError::ReadDirEntry {
            path: source.to_path_buf(),
            source: source_error,
        })?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type =
            entry
                .file_type()
                .map_err(|source_error| SessionStoreMigrationError::FileType {
                    path: source_path.clone(),
                    source: source_error,
                })?;

        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else if file_type.is_file() {
            std::fs::copy(&source_path, &target_path).map_err(|source_error| {
                SessionStoreMigrationError::Copy {
                    source_path: source_path.clone(),
                    target_path,
                    source: source_error,
                }
            })?;
        }
    }

    Ok(())
}

fn read_json_optional_sync<T>(path: &Path) -> Result<Option<T>, SessionStoreMigrationError>
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return Ok(None);
    }

    let bytes = std::fs::read(path).map_err(|source| SessionStoreMigrationError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let value = serde_json::from_slice(&bytes).map_err(|source| {
        SessionStoreMigrationError::DeserializeJson {
            path: path.to_path_buf(),
            source,
        }
    })?;
    Ok(Some(value))
}

fn write_json_pretty_sync<T>(path: &Path, value: &T) -> Result<(), SessionStoreMigrationError>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }

    let bytes = serde_json::to_vec_pretty(value).map_err(|source| {
        SessionStoreMigrationError::SerializeJson {
            path: path.to_path_buf(),
            source,
        }
    })?;
    std::fs::write(path, bytes).map_err(|source| SessionStoreMigrationError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}

fn move_path_best_effort(source: &Path, target: &Path) -> Result<(), SessionStoreMigrationError> {
    if let Some(parent) = target.parent() {
        create_dir_all(parent)?;
    }

    match std::fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(error) if source.is_dir() => {
            let _ = error;
            copy_dir_recursive(source, target)?;
            std::fs::remove_dir_all(source).map_err(|source_error| {
                SessionStoreMigrationError::Remove {
                    path: source.to_path_buf(),
                    source: source_error,
                }
            })
        }
        Err(error) => {
            let _ = error;
            std::fs::copy(source, target).map_err(|source_error| {
                SessionStoreMigrationError::Copy {
                    source_path: source.to_path_buf(),
                    target_path: target.to_path_buf(),
                    source: source_error,
                }
            })?;
            std::fs::remove_file(source).map_err(|source_error| {
                SessionStoreMigrationError::Remove {
                    path: source.to_path_buf(),
                    source: source_error,
                }
            })
        }
    }
}

fn files_are_equal(left: &Path, right: &Path) -> Result<bool, SessionStoreMigrationError> {
    let left_bytes =
        std::fs::read(left).map_err(|source| SessionStoreMigrationError::ReadFile {
            path: left.to_path_buf(),
            source,
        })?;
    let right_bytes =
        std::fs::read(right).map_err(|source| SessionStoreMigrationError::ReadFile {
            path: right.to_path_buf(),
            source,
        })?;
    Ok(left_bytes == right_bytes)
}

fn source_is_newer(source: &Path, target: &Path) -> Result<bool, SessionStoreMigrationError> {
    let source_modified = std::fs::metadata(source)
        .map_err(|source_error| SessionStoreMigrationError::Stat {
            path: source.to_path_buf(),
            source: source_error,
        })?
        .modified()
        .ok();
    let target_modified = std::fs::metadata(target)
        .map_err(|source_error| SessionStoreMigrationError::Stat {
            path: target.to_path_buf(),
            source: source_error,
        })?
        .modified()
        .ok();

    Ok(match (source_modified, target_modified) {
        (Some(source_time), Some(target_time)) => source_time > target_time,
        (Some(_), None) => true,
        _ => false,
    })
}

fn create_dir_all(path: &Path) -> Result<(), SessionStoreMigrationError> {
    std::fs::create_dir_all(path).map_err(|source| SessionStoreMigrationError::CreateDir {
        path: path.to_path_buf(),
        source,
    })
}

fn file_name_eq(path: &Path, expected: &str) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

#[cfg(test)]
mod tests {
    use super::merge_legacy_session_store;
    use crate::session::{SessionMetadata, StoredSessionIndexFile, StoredSessionMetadataFile};
    use std::fs;
    use std::path::Path;

    #[tokio::test]
    async fn merge_legacy_session_store_preserves_newer_metadata_and_rebuilds_visible_index() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("legacy");
        let target = dir.path().join("runtime");
        fs::create_dir_all(&source).expect("legacy root");
        fs::create_dir_all(target.join("existing-session")).expect("target existing session");

        let newer_metadata = metadata("existing-session", 200);
        write_session_metadata(&target.join("existing-session"), &newer_metadata);

        let older_metadata = metadata("existing-session", 100);
        write_session_metadata(&source.join("existing-session"), &older_metadata);

        let legacy_only_metadata = metadata("legacy-session", 150);
        write_session_metadata(&source.join("legacy-session"), &legacy_only_metadata);

        let mut hidden_metadata = metadata("hidden-session", 250);
        hidden_metadata.session_kind = bitfun_core_types::SessionKind::Subagent;
        write_session_metadata(&source.join("hidden-session"), &hidden_metadata);

        write_session_index(
            &source.join("index.json"),
            vec![
                hidden_metadata.clone(),
                older_metadata.clone(),
                legacy_only_metadata.clone(),
            ],
        );
        write_session_index(&target.join("index.json"), vec![newer_metadata.clone()]);

        let record = merge_legacy_session_store(&source, &target)
            .await
            .expect("merge should succeed")
            .expect("source exists");

        assert_eq!(record.strategy, "merge_sessions");
        assert!(target.join("legacy-session").exists());
        assert!(target.join("existing-session").exists());
        assert!(!source.exists(), "legacy sessions root should be removed");

        let merged_metadata: StoredSessionMetadataFile = serde_json::from_slice(
            &fs::read(target.join("existing-session").join("metadata.json")).expect("metadata"),
        )
        .expect("merged metadata should deserialize");
        assert_eq!(merged_metadata.metadata.last_active_at, 200);

        let merged_index: StoredSessionIndexFile =
            serde_json::from_slice(&fs::read(target.join("index.json")).expect("index"))
                .expect("merged session index should deserialize");
        assert_eq!(merged_index.sessions.len(), 2);
        assert_eq!(merged_index.metadata_file_count, 3);
        assert!(merged_index
            .sessions
            .iter()
            .all(|metadata| metadata.session_id != "hidden-session"));
    }

    #[tokio::test]
    async fn merge_legacy_session_store_noops_when_source_is_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result =
            merge_legacy_session_store(&dir.path().join("missing"), &dir.path().join("target"))
                .await
                .expect("missing source should not fail");

        assert!(result.is_none());
    }

    fn metadata(session_id: &str, last_active_at: u64) -> SessionMetadata {
        let mut metadata = SessionMetadata::new(
            session_id.to_string(),
            format!("Session {session_id}"),
            "agent".to_string(),
            "model".to_string(),
        );
        metadata.last_active_at = last_active_at;
        metadata
    }

    fn write_session_metadata(session_dir: &Path, metadata: &SessionMetadata) {
        fs::create_dir_all(session_dir).expect("session dir should exist");
        let stored = StoredSessionMetadataFile::new(metadata.clone());
        fs::write(
            session_dir.join("metadata.json"),
            serde_json::to_string_pretty(&stored).expect("metadata should serialize"),
        )
        .expect("metadata should write");
    }

    fn write_session_index(path: &Path, sessions: Vec<SessionMetadata>) {
        let index = StoredSessionIndexFile::new(0, sessions);
        fs::write(
            path,
            serde_json::to_string_pretty(&index).expect("index should serialize"),
        )
        .expect("index should write");
    }
}
