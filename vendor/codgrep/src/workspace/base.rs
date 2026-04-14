use std::{fs, sync::Arc};

use crate::{
    error::{AppError, Result},
    index::format::{read_docs_file, IndexLayout},
};

use super::{BaseSnapshotInfo, BaseSnapshotKind, CachedBaseSearcher, WorkspaceIndex};

impl WorkspaceIndex {
    pub(super) fn base_snapshot_info(&self) -> Result<BaseSnapshotInfo> {
        let layout = IndexLayout::resolve(&self.inner.options.build_config.index_path)?;
        let snapshot_key = self.current_snapshot_key_on_disk()?;
        let (metadata, docs) = read_docs_file(&layout.docs_path)?;
        let snapshot_kind = if snapshot_key.starts_with("base-git-") {
            BaseSnapshotKind::GitCommit
        } else if snapshot_key.starts_with("base-repo-") {
            BaseSnapshotKind::RepositoryFallback
        } else {
            BaseSnapshotKind::Legacy
        };
        let (head_commit, config_fingerprint) = metadata
            .build
            .as_ref()
            .map(|build| (build.head_commit.clone(), build.config_fingerprint.clone()))
            .unwrap_or((None, None));

        Ok(BaseSnapshotInfo {
            snapshot_key,
            index_path: self.inner.options.build_config.index_path.clone(),
            repo_path: self.inner.options.build_config.repo_path.clone(),
            tokenizer: metadata.tokenizer,
            doc_count: docs.len(),
            snapshot_kind,
            head_commit,
            config_fingerprint,
        })
    }

    pub(super) fn open_searcher(&self) -> Result<Arc<crate::index::IndexSearcher>> {
        let snapshot_key = self.current_snapshot_key_on_disk()?;
        let mut cached_searcher = match self.inner.cached_searcher.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(cached) = cached_searcher.as_ref() {
            if cached.snapshot_key == snapshot_key {
                return Ok(Arc::clone(&cached.searcher));
            }
        }

        let searcher = Arc::new(crate::index::IndexSearcher::open(
            self.inner.options.build_config.index_path.clone(),
        )?);
        *cached_searcher = Some(CachedBaseSearcher {
            snapshot_key,
            searcher: Arc::clone(&searcher),
        });
        Ok(searcher)
    }

    pub(super) fn current_snapshot_key_on_disk(&self) -> Result<String> {
        let layout = IndexLayout::resolve(&self.inner.options.build_config.index_path)?;
        let current_path = IndexLayout::current_path(&self.inner.options.build_config.index_path);
        if current_path.exists() {
            let generation = fs::read_to_string(current_path)?;
            let generation = generation.trim();
            if generation.is_empty() {
                return Err(AppError::InvalidIndex(
                    "base generation is missing from CURRENT".into(),
                ));
            }
            return Ok(generation.to_string());
        }
        layout
            .data_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| AppError::InvalidIndex("base generation is missing".into()))
    }
}
