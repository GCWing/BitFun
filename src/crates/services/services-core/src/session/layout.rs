//! Session storage path layout.
//!
//! This module owns stable file and directory names under an already-resolved
//! sessions root. Workspace-to-root resolution remains outside services-core.

use std::io;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStorageLayout {
    sessions_root: PathBuf,
}

impl SessionStorageLayout {
    pub fn new(sessions_root: impl Into<PathBuf>) -> Self {
        Self {
            sessions_root: sessions_root.into(),
        }
    }

    pub fn sessions_root(&self) -> &Path {
        &self.sessions_root
    }

    pub fn index_path(&self) -> PathBuf {
        self.sessions_root.join("index.json")
    }

    pub fn session_dir(&self, session_id: &str) -> PathBuf {
        self.sessions_root.join(session_id)
    }

    pub fn metadata_path(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("metadata.json")
    }

    pub fn state_path(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("state.json")
    }

    pub fn prompt_cache_path(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("prompt_cache.json")
    }

    pub fn turns_dir(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("turns")
    }

    pub fn snapshots_dir(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("snapshots")
    }

    pub fn artifacts_dir(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("artifacts")
    }

    pub fn turn_path(&self, session_id: &str, turn_index: usize) -> PathBuf {
        self.turns_dir(session_id)
            .join(format!("turn-{:04}.json", turn_index))
    }

    pub fn context_snapshot_path(&self, session_id: &str, turn_index: usize) -> PathBuf {
        self.snapshots_dir(session_id)
            .join(format!("context-{:04}.json", turn_index))
    }

    pub fn skill_agent_snapshot_path(&self, session_id: &str, turn_index: usize) -> PathBuf {
        self.snapshots_dir(session_id)
            .join(format!("skill-agent-{:04}.json", turn_index))
    }

    /// Forked subagents keep this override separate from the child's own turn-0
    /// skill-agent snapshot so listing reminders can reuse the parent baseline.
    pub fn skill_agent_baseline_override_path(&self, session_id: &str) -> PathBuf {
        self.snapshots_dir(session_id)
            .join("skill-agent-baseline-override.json")
    }

    pub fn transcript_path(&self, session_id: &str) -> PathBuf {
        self.artifacts_dir(session_id).join("transcript.txt")
    }

    pub fn transcript_meta_path(&self, session_id: &str) -> PathBuf {
        self.artifacts_dir(session_id).join("transcript.meta.json")
    }

    pub async fn ensure_session_dir(&self, session_id: &str) -> io::Result<PathBuf> {
        self.ensure_dir(self.session_dir(session_id)).await
    }

    pub async fn ensure_turns_dir(&self, session_id: &str) -> io::Result<PathBuf> {
        self.ensure_dir(self.turns_dir(session_id)).await
    }

    pub async fn ensure_snapshots_dir(&self, session_id: &str) -> io::Result<PathBuf> {
        self.ensure_dir(self.snapshots_dir(session_id)).await
    }

    pub async fn ensure_artifacts_dir(&self, session_id: &str) -> io::Result<PathBuf> {
        self.ensure_dir(self.artifacts_dir(session_id)).await
    }

    pub async fn list_indexed_turn_paths(
        &self,
        session_id: &str,
    ) -> io::Result<Vec<(usize, PathBuf)>> {
        let turns_dir = self.turns_dir(session_id);
        if !turns_dir.exists() {
            return Ok(Vec::new());
        }

        let mut indexed_paths = Vec::new();
        let mut entries = fs::read_dir(&turns_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(index_str) = stem.strip_prefix("turn-") else {
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

    pub async fn delete_indexed_turn_paths_from(
        &self,
        session_id: &str,
        start_index: usize,
    ) -> io::Result<usize> {
        let mut deleted = 0usize;
        for (index, path) in self.list_indexed_turn_paths(session_id).await? {
            if index >= start_index {
                fs::remove_file(&path).await?;
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    async fn ensure_dir(&self, dir: PathBuf) -> io::Result<PathBuf> {
        fs::create_dir_all(&dir).await?;
        Ok(dir)
    }
}
