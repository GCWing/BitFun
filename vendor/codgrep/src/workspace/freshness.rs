use std::{io::ErrorKind, time::Instant};

use crate::error::{AppError, Result};

use super::{CachedFreshnessProbe, WorkspaceFreshness, WorkspaceFreshnessState, WorkspaceIndex};

impl WorkspaceIndex {
    pub(super) fn compute_freshness_probe(&self) -> Result<WorkspaceFreshness> {
        let checked_at = std::time::SystemTime::now();
        let base = self.base_snapshot_info().ok();

        match self.open_searcher() {
            Ok(searcher) => {
                let diff = searcher.diff_against_worktree()?;
                let state = if diff.is_empty() {
                    WorkspaceFreshnessState::Fresh
                } else {
                    WorkspaceFreshnessState::Stale
                };
                Ok(WorkspaceFreshness {
                    checked_at,
                    state,
                    base_snapshot_key: base.map(|base| base.snapshot_key),
                    modified_files: diff.modified_files.len(),
                    deleted_files: diff.deleted_files.len(),
                    new_files: diff.new_files.len(),
                })
            }
            Err(AppError::Io(error)) if error.kind() == ErrorKind::NotFound => {
                Ok(WorkspaceFreshness {
                    checked_at,
                    state: WorkspaceFreshnessState::MissingBaseSnapshot,
                    base_snapshot_key: None,
                    modified_files: 0,
                    deleted_files: 0,
                    new_files: 0,
                })
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn cache_freshness_probe(&self, probe: WorkspaceFreshness) {
        let mut cache = match self.inner.cached_freshness.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *cache = Some(CachedFreshnessProbe {
            captured_at: Instant::now(),
            probe,
        });
    }

    pub(super) fn clear_freshness_probe_cache(&self) {
        let mut cache = match self.inner.cached_freshness.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *cache = None;
    }
}
