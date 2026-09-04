use crate::ui::composer::ComposerDraft;
use fs2::FileExt;
use openbitfun_agent_runtime::sdk::AgentWorkspaceReference;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub(crate) const MAX_STASH_ENTRIES: usize = 50;
const MAX_STASH_ENTRY_BYTES: usize = 128 * 1024;
const MAX_STASH_FILE_BYTES: u64 = 8 * 1024 * 1024;
const LOCK_TIMEOUT: Duration = Duration::from_millis(250);
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptStashEntry {
    pub(crate) id: String,
    pub(crate) draft: ComposerDraft,
    pub(crate) timestamp_ms: u64,
    workspace_identity: Option<String>,
}

impl PromptStashEntry {
    pub(crate) fn into_draft_for_workspace(
        self,
        workspace_identity: Option<&str>,
    ) -> (ComposerDraft, bool) {
        let mut draft = self.draft;
        let references_detached = !draft.workspace_references.is_empty()
            && self.workspace_identity.as_deref() != workspace_identity;
        if references_detached {
            draft.workspace_references.clear();
        }
        (draft, references_detached)
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum PromptStashError {
    #[error("Prompt stash does not persist image attachments; remove them before stashing")]
    ImagesUnsupported,
    #[error("prompt stash is busy in another CLI process; try again")]
    Busy,
    #[error("prompt stash entry is {actual} bytes; the limit is {limit} bytes")]
    EntryTooLarge { actual: usize, limit: usize },
    #[error("prompt stash file is {actual} bytes; the limit is {limit} bytes")]
    FileTooLarge { actual: u64, limit: u64 },
    #[error("prompt stash contains invalid JSON on line {line}: {source}")]
    CorruptData {
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("prompt stash storage failed: {0}")]
    Storage(#[from] std::io::Error),
    #[error("prompt stash serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredPromptStashEntry {
    id: String,
    input: String,
    #[serde(default)]
    workspace_references: Vec<AgentWorkspaceReference>,
    timestamp_ms: u64,
    #[serde(default)]
    workspace_identity: Option<String>,
}

impl From<StoredPromptStashEntry> for PromptStashEntry {
    fn from(entry: StoredPromptStashEntry) -> Self {
        let mut draft = ComposerDraft {
            text: entry.input,
            workspace_references: entry.workspace_references,
            image_attachments: Vec::new(),
        };
        draft.retain_valid_sources();
        Self {
            id: entry.id,
            draft,
            timestamp_ms: entry.timestamp_ms,
            workspace_identity: entry.workspace_identity,
        }
    }
}

impl From<&PromptStashEntry> for StoredPromptStashEntry {
    fn from(entry: &PromptStashEntry) -> Self {
        Self {
            id: entry.id.clone(),
            input: entry.draft.text.clone(),
            workspace_references: entry.draft.workspace_references.clone(),
            timestamp_ms: entry.timestamp_ms,
            workspace_identity: entry.workspace_identity.clone(),
        }
    }
}

/// A small CLI-local JSONL store matching OpenCode's prompt-stash lifecycle.
/// Every mutation reloads under a cross-process lock so multiple TUI clients do
/// not overwrite each other's latest entries.
pub(crate) struct PromptStashStore {
    path: PathBuf,
}

impl PromptStashStore {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub(crate) fn from_config_dir() -> anyhow::Result<Self> {
        Ok(Self::new(
            crate::config::CliConfig::config_dir()?.join("prompt-stash.jsonl"),
        ))
    }

    pub(crate) fn list(&self) -> Result<Vec<PromptStashEntry>, PromptStashError> {
        self.with_lock(false, |entries| Ok(entries.iter().rev().cloned().collect()))
    }

    pub(crate) fn push(
        &self,
        draft: &ComposerDraft,
        workspace_identity: Option<&str>,
        timestamp_ms: u64,
    ) -> Result<PromptStashEntry, PromptStashError> {
        if draft.has_images() {
            return Err(PromptStashError::ImagesUnsupported);
        }
        let entry = PromptStashEntry {
            id: uuid::Uuid::new_v4().to_string(),
            draft: draft.clone(),
            timestamp_ms,
            workspace_identity: workspace_identity.map(ToOwned::to_owned),
        };
        let serialized_size = serde_json::to_vec(&StoredPromptStashEntry::from(&entry))?.len();
        if serialized_size > MAX_STASH_ENTRY_BYTES {
            return Err(PromptStashError::EntryTooLarge {
                actual: serialized_size,
                limit: MAX_STASH_ENTRY_BYTES,
            });
        }
        self.with_lock(true, |entries| {
            entries.push(entry.clone());
            if entries.len() > MAX_STASH_ENTRIES {
                entries.drain(..entries.len() - MAX_STASH_ENTRIES);
            }
            Ok(entry)
        })
    }

    pub(crate) fn pop(&self) -> Result<Option<PromptStashEntry>, PromptStashError> {
        self.with_lock(true, |entries| Ok(entries.pop()))
    }

    pub(crate) fn remove(&self, id: &str) -> Result<bool, PromptStashError> {
        self.with_lock(true, |entries| {
            let before = entries.len();
            entries.retain(|entry| entry.id != id);
            Ok(entries.len() != before)
        })
    }

    fn with_lock<T>(
        &self,
        persist: bool,
        operation: impl FnOnce(&mut Vec<PromptStashEntry>) -> Result<T, PromptStashError>,
    ) -> Result<T, PromptStashError> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| std::io::Error::other("prompt stash path has no parent"))?;
        fs::create_dir_all(parent)?;
        let lock_path = self.path.with_extension("jsonl.lock");
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(lock_path)?;
        let started = Instant::now();
        loop {
            match lock.try_lock_exclusive() {
                Ok(()) => break,
                Err(error) if lock_is_contended(&error) && started.elapsed() < LOCK_TIMEOUT => {
                    std::thread::sleep(LOCK_RETRY_INTERVAL);
                }
                Err(error) if lock_is_contended(&error) => {
                    return Err(PromptStashError::Busy);
                }
                Err(error) => return Err(error.into()),
            }
        }

        let result = self.read_entries().and_then(|mut entries| {
            let value = operation(&mut entries)?;
            if persist {
                self.write_entries(&entries)?;
            }
            Ok(value)
        });
        let unlock_result = FileExt::unlock(&lock);

        match result {
            Err(error) => Err(error),
            Ok(value) => {
                unlock_result?;
                Ok(value)
            }
        }
    }

    fn read_entries(&self) -> Result<Vec<PromptStashEntry>, PromptStashError> {
        match fs::metadata(&self.path) {
            Ok(metadata) if metadata.len() > MAX_STASH_FILE_BYTES => {
                return Err(PromptStashError::FileTooLarge {
                    actual: metadata.len(),
                    limit: MAX_STASH_FILE_BYTES,
                });
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let content = match fs::read_to_string(&self.path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        let mut entries = Vec::new();
        for (index, line) in content.lines().enumerate() {
            let entry = serde_json::from_str::<StoredPromptStashEntry>(line).map_err(|source| {
                PromptStashError::CorruptData {
                    line: index + 1,
                    source,
                }
            })?;
            entries.push(PromptStashEntry::from(entry));
        }
        if entries.len() > MAX_STASH_ENTRIES {
            entries.drain(..entries.len() - MAX_STASH_ENTRIES);
        }
        Ok(entries)
    }

    fn write_entries(&self, entries: &[PromptStashEntry]) -> Result<(), PromptStashError> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| std::io::Error::other("prompt stash path has no parent"))?;
        let mut content = String::new();
        for entry in entries {
            content.push_str(&serde_json::to_string(&StoredPromptStashEntry::from(
                entry,
            ))?);
            content.push('\n');
        }
        if content.len() as u64 > MAX_STASH_FILE_BYTES {
            return Err(PromptStashError::FileTooLarge {
                actual: content.len() as u64,
                limit: MAX_STASH_FILE_BYTES,
            });
        }
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(content.as_bytes())?;
        temporary.as_file_mut().sync_all()?;
        temporary.persist(&self.path).map_err(|error| error.error)?;
        Ok(())
    }
}

fn lock_is_contended(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock
        || matches!(error.raw_os_error(), Some(32) | Some(33))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::composer::{ComposerDraft, ComposerImage};
    use openbitfun_agent_runtime::sdk::{
        AgentWorkspaceReference, AgentWorkspaceReferenceKind, AgentWorkspaceReferenceSourceRange,
    };
    use std::io::Write;
    use std::sync::Arc;

    fn reference(value: &str) -> AgentWorkspaceReference {
        AgentWorkspaceReference {
            path: "src/main.rs".to_string(),
            kind: AgentWorkspaceReferenceKind::File,
            start_line: None,
            end_line: None,
            source: AgentWorkspaceReferenceSourceRange {
                start: 0,
                end: value.chars().count(),
                value: value.to_string(),
            },
        }
    }

    #[test]
    fn stash_is_bounded_newest_first_and_preserves_structured_references() {
        let directory = tempfile::tempdir().unwrap();
        let store = PromptStashStore::new(directory.path().join("prompt-stash.jsonl"));

        for index in 0..52 {
            let text = format!("prompt-{index}");
            store
                .push(&ComposerDraft::from_text(text), Some("workspace-a"), index)
                .unwrap();
        }
        let mut newest = ComposerDraft::from_text("@src/main.rs inspect");
        newest.workspace_references.push(reference("@src/main.rs"));
        store.push(&newest, Some("workspace-a"), 100).unwrap();

        let entries = store.list().unwrap();
        assert_eq!(entries.len(), MAX_STASH_ENTRIES);
        assert_eq!(entries.first().unwrap().draft, newest);
        assert_eq!(entries.last().unwrap().draft.text, "prompt-3");

        assert_eq!(store.pop().unwrap().unwrap().draft, newest);
        assert_eq!(
            store.list().unwrap().first().unwrap().draft.text,
            "prompt-51"
        );
    }

    #[test]
    fn corrupt_data_is_reported_without_rewriting_the_source_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("prompt-stash.jsonl");
        let store = PromptStashStore::new(path.clone());
        store
            .push(&ComposerDraft::from_text("first"), Some("workspace-a"), 1)
            .unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"not-json\n")
            .unwrap();
        let before = std::fs::read(&path).unwrap();

        assert!(matches!(
            store.list(),
            Err(PromptStashError::CorruptData { .. })
        ));
        assert!(matches!(
            store.push(&ComposerDraft::from_text("second"), Some("workspace-a"), 2),
            Err(PromptStashError::CorruptData { .. })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn mutations_reload_the_latest_file_between_store_instances() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("prompt-stash.jsonl");
        let first = PromptStashStore::new(path.clone());
        let second = PromptStashStore::new(path);
        let first_entry = first
            .push(&ComposerDraft::from_text("first"), Some("workspace-a"), 1)
            .unwrap();

        second
            .push(&ComposerDraft::from_text("second"), Some("workspace-a"), 2)
            .unwrap();
        assert!(second.remove(&first_entry.id).unwrap());

        let entries = first.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].draft.text, "second");
    }

    #[test]
    fn references_are_detached_when_restoring_in_a_different_workspace() {
        let directory = tempfile::tempdir().unwrap();
        let store = PromptStashStore::new(directory.path().join("prompt-stash.jsonl"));
        let mut draft = ComposerDraft::from_text("@src/main.rs inspect");
        draft.workspace_references.push(reference("@src/main.rs"));
        store.push(&draft, Some("workspace-a"), 1).unwrap();

        let entry = store.pop().unwrap().unwrap();
        let (restored, references_detached) = entry.into_draft_for_workspace(Some("workspace-b"));

        assert!(references_detached);
        assert_eq!(restored.text, draft.text);
        assert!(restored.workspace_references.is_empty());
    }

    #[test]
    fn contended_lock_and_oversized_entries_fail_with_bounded_errors() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("prompt-stash.jsonl");
        let store = PromptStashStore::new(path.clone());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path.with_extension("jsonl.lock"))
            .unwrap();
        lock.lock_exclusive().unwrap();
        let started = Instant::now();
        let result = store.list();
        FileExt::unlock(&lock).unwrap();

        assert!(matches!(result, Err(PromptStashError::Busy)));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(matches!(
            store.push(
                &ComposerDraft::from_text("x".repeat(MAX_STASH_ENTRY_BYTES + 1)),
                Some("workspace-a"),
                1,
            ),
            Err(PromptStashError::EntryTooLarge { .. })
        ));
    }

    #[test]
    fn image_drafts_are_rejected_without_mutating_persistent_stash() {
        let directory = tempfile::tempdir().unwrap();
        let store = PromptStashStore::new(directory.path().join("prompt-stash.jsonl"));
        let mut draft = ComposerDraft::from_text("look ");
        draft
            .insert_image(
                draft.text.chars().count(),
                ComposerImage::new(
                    "image-1",
                    "image.png",
                    "image/png",
                    Arc::<[u8]>::from([1, 2, 3]),
                ),
            )
            .unwrap();

        let error = store.push(&draft, Some("workspace-a"), 1).unwrap_err();

        assert!(matches!(error, PromptStashError::ImagesUnsupported));
        assert!(store.list().unwrap().is_empty());
    }
}
