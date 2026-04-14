#[path = "workspace/base.rs"]
mod base;
#[path = "workspace/freshness.rs"]
mod freshness;
mod runtime;

use std::{
    io::ErrorKind,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};

use crate::{
    config::BuildConfig,
    error::{AppError, Result},
    index::{build_index_with_options, IndexBuildOptions, IndexSearcher, IndexWorktreeDiff},
    path_filter::PathFilter,
    query_preflight::{evaluate_index_query_preflight, preflight_enabled},
    search::SearchResults,
    search_engine::{
        search_document_index, search_documents, SearchBackend, SearchDocumentIndex, SearchResponse,
    },
    TokenizerMode,
};

use self::runtime::{
    load_search_document, merge_search_results, overlay_search_scope, scan_text_repository_files,
};

/// Options for opening a [`WorkspaceIndex`].
///
/// This facade exposes the current workspace view over a stable base snapshot.
#[derive(Debug, Clone)]
pub struct WorkspaceIndexOptions {
    /// Build-time/index-location configuration for the workspace.
    pub build_config: BuildConfig,
}

impl From<BuildConfig> for WorkspaceIndexOptions {
    fn from(build_config: BuildConfig) -> Self {
        Self { build_config }
    }
}

/// Identity and metadata for the current base snapshot.
#[derive(Debug, Clone)]
pub struct BaseSnapshotInfo {
    /// Stable cache key for the base snapshot.
    pub snapshot_key: String,
    /// Root directory that stores base snapshot index data.
    pub index_path: PathBuf,
    /// Repository root for this workspace.
    pub repo_path: PathBuf,
    /// Tokenizer used by the base snapshot.
    pub tokenizer: TokenizerMode,
    /// Number of indexed documents in the base snapshot.
    pub doc_count: usize,
    /// Kind of base snapshot identity currently in use.
    pub snapshot_kind: BaseSnapshotKind,
    /// Git `HEAD` commit used for the snapshot when available.
    pub head_commit: Option<String>,
    /// Fingerprint of the build configuration used to materialize the snapshot.
    pub config_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseSnapshotKind {
    GitCommit,
    RepositoryFallback,
    Legacy,
}

/// Current observable workspace state.
#[derive(Debug, Clone)]
pub struct IndexStatus {
    /// Metadata for the current base snapshot, if one exists on disk.
    pub base: Option<BaseSnapshotInfo>,
    /// Worktree diff against the base snapshot, if the base index can be opened.
    pub dirty_files: Option<IndexWorktreeDiff>,
}

/// High-level freshness state for the currently opened workspace index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceFreshnessState {
    MissingBaseSnapshot,
    Fresh,
    Stale,
}

/// Structured freshness probe result for external schedulers/orchestrators.
#[derive(Debug, Clone)]
pub struct WorkspaceFreshness {
    /// Probe capture time.
    pub checked_at: SystemTime,
    /// High-level freshness state.
    pub state: WorkspaceFreshnessState,
    /// Active base snapshot key, when available.
    pub base_snapshot_key: Option<String>,
    /// Number of tracked files changed since base snapshot.
    pub modified_files: usize,
    /// Number of tracked files deleted since base snapshot.
    pub deleted_files: usize,
    /// Number of new files missing from base snapshot.
    pub new_files: usize,
}

impl WorkspaceFreshness {
    pub fn is_stale(&self) -> bool {
        self.state == WorkspaceFreshnessState::Stale
    }

    pub fn needs_base_snapshot(&self) -> bool {
        self.state == WorkspaceFreshnessState::MissingBaseSnapshot
    }

    pub fn dirty_file_count(&self) -> usize {
        self.modified_files + self.deleted_files + self.new_files
    }
}

/// High-level workspace facade for base snapshot management and searching
/// against the current effective repository view.
#[derive(Clone)]
pub struct WorkspaceIndex {
    inner: Arc<WorkspaceIndexInner>,
}

/// Read-only workspace query view pinned to one base snapshot and dirty path set.
///
/// The dirty path set is captured when the snapshot is created. Matching still
/// reads the current on-disk contents for those paths at query time.
#[derive(Clone)]
pub struct WorkspaceSnapshot {
    searcher: Arc<IndexSearcher>,
    dirty: IndexWorktreeDiff,
    base_snapshot_key: String,
}

struct WorkspaceIndexInner {
    options: WorkspaceIndexOptions,
    cached_searcher: Mutex<Option<CachedBaseSearcher>>,
    cached_freshness: Mutex<Option<CachedFreshnessProbe>>,
}

#[derive(Clone)]
struct CachedBaseSearcher {
    snapshot_key: String,
    searcher: Arc<IndexSearcher>,
}

#[derive(Clone)]
struct CachedFreshnessProbe {
    captured_at: Instant,
    probe: WorkspaceFreshness,
}

impl WorkspaceIndex {
    /// Opens a workspace facade over the configured repository and index root.
    pub fn open(options: WorkspaceIndexOptions) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(WorkspaceIndexInner {
                options: WorkspaceIndexOptions {
                    build_config: options.build_config.normalized()?,
                },
                cached_searcher: Mutex::new(None),
                cached_freshness: Mutex::new(None),
            }),
        })
    }

    /// Ensures that a base snapshot exists and returns its identity metadata.
    pub fn ensure_base_snapshot(&self) -> Result<BaseSnapshotInfo> {
        self.ensure_base_snapshot_with_options(IndexBuildOptions::default())
    }

    pub fn ensure_base_snapshot_with_options(
        &self,
        options: IndexBuildOptions<'_>,
    ) -> Result<BaseSnapshotInfo> {
        self.clear_freshness_probe_cache();
        build_index_with_options(&self.inner.options.build_config, options)?;
        self.clear_cached_searcher();
        self.base_snapshot_info()
    }

    fn clear_cached_searcher(&self) {
        let mut cached_searcher = match self.inner.cached_searcher.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *cached_searcher = None;
    }

    /// Reports the current base snapshot and worktree diff state.
    pub fn status(&self) -> Result<IndexStatus> {
        let base = self.base_snapshot_info().ok();
        let dirty_files = match self.open_searcher() {
            Ok(searcher) => Some(searcher.diff_against_worktree()?),
            Err(AppError::Io(error)) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        Ok(IndexStatus { base, dirty_files })
    }

    /// Performs a full freshness probe against the current worktree.
    pub fn probe_freshness(&self) -> Result<WorkspaceFreshness> {
        let probe = self.compute_freshness_probe()?;
        self.cache_freshness_probe(probe.clone());
        Ok(probe)
    }

    /// Returns a cached freshness probe when still within `ttl`; otherwise runs
    /// a new full probe.
    pub fn probe_freshness_if_due(&self, ttl: Duration) -> Result<WorkspaceFreshness> {
        if !ttl.is_zero() {
            let cached = match self.inner.cached_freshness.lock() {
                Ok(guard) => guard.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            };
            if let Some(cached) = cached {
                if cached.captured_at.elapsed() < ttl {
                    return Ok(cached.probe);
                }
            }
        }

        self.probe_freshness()
    }

    /// Searches the current effective workspace view.
    ///
    /// This resolves the current dirty diff on demand and searches against
    /// `base snapshot + current dirty files repaired`.
    pub fn search(&self, query: &crate::QueryConfig) -> Result<SearchResults> {
        self.search_with_filter(query, None)
    }

    /// Searches the current effective workspace view with an optional path filter.
    pub fn search_with_filter(
        &self,
        query: &crate::QueryConfig,
        filter: Option<&PathFilter>,
    ) -> Result<SearchResults> {
        match self.open_searcher() {
            Ok(searcher) => {
                let dirty = searcher.diff_against_worktree()?;
                search_workspace_view(&searcher, query, &dirty, filter)
            }
            Err(AppError::Io(error)) if error.kind() == ErrorKind::NotFound => {
                self.scan_search_with_filter(query, filter)
            }
            Err(error) => Err(error),
        }
    }

    /// Captures a reusable read-only workspace view pinned to the current base
    /// snapshot and dirty path set.
    pub fn snapshot(&self) -> Result<WorkspaceSnapshot> {
        let searcher = match self.open_searcher() {
            Ok(searcher) => searcher,
            Err(AppError::Io(error)) if error.kind() == ErrorKind::NotFound => {
                return Err(AppError::InvalidIndex(format!(
                    "index does not exist at {}",
                    self.inner.options.build_config.index_path.display()
                )));
            }
            Err(error) => return Err(error),
        };
        let dirty = searcher.diff_against_worktree()?;
        let base_snapshot_key = self.current_snapshot_key_on_disk()?;
        Ok(WorkspaceSnapshot {
            searcher,
            dirty,
            base_snapshot_key,
        })
    }

    /// Searches against the base snapshot plus a caller-maintained dirty overlay diff.
    pub fn search_with_dirty_diff(
        &self,
        query: &crate::QueryConfig,
        dirty: &IndexWorktreeDiff,
        filter: Option<&PathFilter>,
    ) -> Result<SearchResults> {
        match self.open_searcher() {
            Ok(searcher) => search_workspace_view(&searcher, query, dirty, filter),
            Err(AppError::Io(error)) if error.kind() == ErrorKind::NotFound => search_documents(
                query,
                &scan_text_repository_files(&self.inner.options.build_config, filter)?,
            ),
            Err(error) => Err(error),
        }
    }

    /// Searches only the current base snapshot and reuses the cached searcher when available.
    pub fn search_snapshot_with_filter(
        &self,
        query: &crate::QueryConfig,
        filter: Option<&PathFilter>,
        allow_scan_fallback: bool,
    ) -> Result<SearchResponse> {
        match self.open_searcher() {
            Ok(searcher) => {
                let query_prefers_scan = if preflight_enabled() {
                    let preflight = evaluate_index_query_preflight(&searcher, query, filter, None)?;
                    preflight.reason.requires_scan_backend()
                } else {
                    false
                };
                if allow_scan_fallback && query_prefers_scan {
                    return Ok(SearchResponse {
                        backend: SearchBackend::Scan,
                        results: self.scan_search_with_filter(query, filter)?,
                    });
                }
                Ok(SearchResponse {
                    backend: SearchBackend::Index,
                    results: searcher.search_with_filter(query, filter)?,
                })
            }
            Err(AppError::Io(error)) if error.kind() == ErrorKind::NotFound => {
                if allow_scan_fallback {
                    return Ok(SearchResponse {
                        backend: SearchBackend::Scan,
                        results: self.scan_search_with_filter(query, filter)?,
                    });
                }
                Err(AppError::InvalidIndex(format!(
                    "index does not exist at {}",
                    self.inner.options.build_config.index_path.display()
                )))
            }
            Err(error) => Err(error),
        }
    }

    pub fn scan_search_with_filter(
        &self,
        query: &crate::QueryConfig,
        filter: Option<&PathFilter>,
    ) -> Result<SearchResults> {
        search_documents(
            query,
            &scan_text_repository_files(&self.inner.options.build_config, filter)?,
        )
    }

    pub(crate) fn search_with_dirty_overlay(
        &self,
        query: &crate::QueryConfig,
        shadowed_base_paths: &std::collections::HashSet<String>,
        dirty_index: &SearchDocumentIndex,
        filter: Option<&PathFilter>,
    ) -> Result<SearchResults> {
        match self.open_searcher() {
            Ok(searcher) => {
                search_workspace_overlay(&searcher, query, shadowed_base_paths, dirty_index, filter)
            }
            Err(AppError::Io(error)) if error.kind() == ErrorKind::NotFound => {
                search_document_index(query, dirty_index, filter)
            }
            Err(error) => Err(error),
        }
    }

    /// Returns the current full worktree diff. Prefer this only for initialization
    /// or recovery; regular daemon queries should use dirty-path overlays.
    pub fn diff_against_worktree(&self) -> Result<IndexWorktreeDiff> {
        self.open_searcher()?.diff_against_worktree()
    }

    pub(crate) fn classify_dirty_paths<I>(&self, paths: I) -> Result<IndexWorktreeDiff>
    where
        I: IntoIterator<Item = String>,
    {
        match self.open_searcher() {
            Ok(searcher) => searcher.classify_dirty_paths(paths),
            Err(AppError::Io(error)) if error.kind() == ErrorKind::NotFound => {
                Ok(IndexWorktreeDiff::default())
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn has_indexed_path_under(&self, path: &str) -> Result<bool> {
        match self.open_searcher() {
            Ok(searcher) => Ok(searcher.has_indexed_path_under(path)),
            Err(AppError::Io(error)) if error.kind() == ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Reconciles a caller-maintained dirty overlay against the current base snapshot.
    pub fn reconcile_dirty_diff(&self, dirty: &IndexWorktreeDiff) -> Result<IndexWorktreeDiff> {
        if dirty.is_empty() {
            return Ok(IndexWorktreeDiff::default());
        }

        let searcher = self.open_searcher()?;
        searcher.reconcile_dirty_paths(dirty)
    }

    pub(crate) fn load_dirty_document(
        &self,
        path: &str,
    ) -> Result<Option<crate::search_engine::SearchDocument>> {
        load_search_document(path.to_string(), PathBuf::from(path))
    }
}

impl WorkspaceSnapshot {
    pub fn base_snapshot_key(&self) -> &str {
        &self.base_snapshot_key
    }

    pub fn dirty_diff(&self) -> &IndexWorktreeDiff {
        &self.dirty
    }

    pub fn search(&self, query: &crate::QueryConfig) -> Result<SearchResults> {
        self.search_with_filter(query, None)
    }

    pub fn search_with_filter(
        &self,
        query: &crate::QueryConfig,
        filter: Option<&PathFilter>,
    ) -> Result<SearchResults> {
        search_workspace_view(&self.searcher, query, &self.dirty, filter)
    }
}

fn search_workspace_view(
    searcher: &IndexSearcher,
    query: &crate::QueryConfig,
    dirty: &IndexWorktreeDiff,
    filter: Option<&PathFilter>,
) -> Result<SearchResults> {
    if dirty.is_empty() {
        return searcher.search_with_filter(query, filter);
    }
    let scope = overlay_search_scope(dirty, filter)?;
    let base_results = searcher.search_with_path_overrides(
        query,
        filter,
        None,
        Some(&scope.shadowed_base_paths),
    )?;
    if scope.dirty_documents.is_empty() {
        return Ok(base_results);
    }
    let Some(dirty_query) = query_with_remaining_global_limit(query, &base_results) else {
        return Ok(base_results);
    };
    let dirty_results = search_documents(&dirty_query, &scope.dirty_documents)?;
    Ok(merge_search_results(base_results, dirty_results))
}

fn search_workspace_overlay(
    searcher: &IndexSearcher,
    query: &crate::QueryConfig,
    shadowed_base_paths: &std::collections::HashSet<String>,
    dirty_index: &SearchDocumentIndex,
    filter: Option<&PathFilter>,
) -> Result<SearchResults> {
    let base_results =
        searcher.search_with_path_overrides(query, filter, None, Some(shadowed_base_paths))?;
    if dirty_index.is_empty() {
        return Ok(base_results);
    }
    let Some(dirty_query) = query_with_remaining_global_limit(query, &base_results) else {
        return Ok(base_results);
    };
    let dirty_results = search_document_index(&dirty_query, dirty_index, filter)?;
    Ok(merge_search_results(base_results, dirty_results))
}

fn query_with_remaining_global_limit(
    query: &crate::QueryConfig,
    current_results: &SearchResults,
) -> Option<crate::QueryConfig> {
    let Some(limit) = query.effective_global_max_results() else {
        return Some(query.clone());
    };
    let consumed = current_results.result_units(query.search_mode);
    if consumed >= limit {
        return None;
    }

    let mut next_query = query.clone();
    next_query.global_max_results = Some(limit - consumed);
    Some(next_query)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::Duration};

    use tempfile::TempDir;

    use super::{BaseSnapshotKind, WorkspaceFreshnessState, WorkspaceIndex, WorkspaceIndexOptions};
    use crate::{
        build_index,
        config::{BuildConfig, CorpusMode, TokenizerMode},
        index::IndexWorktreeDiff,
    };

    struct TestRepo {
        _temp: TempDir,
        repo: PathBuf,
        index: PathBuf,
    }

    impl TestRepo {
        fn new() -> Self {
            let temp = tempfile::tempdir().expect("temp dir should succeed");
            let repo_dir = temp.path().join("repo");
            fs::create_dir_all(&repo_dir).expect("repo dir should succeed");
            let repo = fs::canonicalize(&repo_dir).expect("repo dir should canonicalize");
            let index = repo.join(".codgrep-index");
            Self {
                _temp: temp,
                repo,
                index,
            }
        }

        fn write(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.repo.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent dir should succeed");
            }
            fs::write(&path, contents).expect("write should succeed");
            path
        }

        fn write_bytes(&self, relative: &str, bytes: &[u8]) -> PathBuf {
            let path = self.repo.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent dir should succeed");
            }
            fs::write(&path, bytes).expect("write should succeed");
            path
        }

        fn build_config(&self) -> BuildConfig {
            BuildConfig {
                repo_path: self.repo.clone(),
                index_path: self.index.clone(),
                tokenizer: TokenizerMode::Trigram,
                corpus_mode: CorpusMode::RespectIgnore,
                include_hidden: false,
                max_file_size: 1024 * 1024,
                min_sparse_len: 3,
                max_sparse_len: 32,
            }
        }

        fn build(&self) {
            build_index(&self.build_config()).expect("build should succeed");
        }
    }

    #[test]
    fn workspace_probe_freshness_reports_missing_base_snapshot() {
        let repo = TestRepo::new();
        let workspace = WorkspaceIndex::open(WorkspaceIndexOptions {
            build_config: repo.build_config(),
        })
        .expect("workspace should open");

        let probe = workspace
            .probe_freshness()
            .expect("freshness probe should succeed");
        assert_eq!(probe.state, WorkspaceFreshnessState::MissingBaseSnapshot);
        assert!(probe.base_snapshot_key.is_none());
        assert_eq!(probe.dirty_file_count(), 0);
        assert!(probe.needs_base_snapshot());
    }

    #[test]
    fn workspace_probe_freshness_reports_stale_after_new_file() {
        let repo = TestRepo::new();
        repo.write("base.rs", "const NAME: &str = \"BASE\";\n");
        repo.build();
        repo.write("added.rs", "const NAME: &str = \"DIRTY\";\n");

        let workspace = WorkspaceIndex::open(WorkspaceIndexOptions {
            build_config: repo.build_config(),
        })
        .expect("workspace should open");

        let probe = workspace
            .probe_freshness()
            .expect("freshness probe should succeed");
        assert_eq!(probe.state, WorkspaceFreshnessState::Stale);
        assert_eq!(probe.new_files, 1);
        assert_eq!(probe.dirty_file_count(), 1);
    }

    #[test]
    fn workspace_probe_freshness_if_due_reuses_cache_until_ttl_expires() {
        let repo = TestRepo::new();
        repo.write("base.rs", "const NAME: &str = \"BASE\";\n");
        repo.build();

        let workspace = WorkspaceIndex::open(WorkspaceIndexOptions {
            build_config: repo.build_config(),
        })
        .expect("workspace should open");

        let first = workspace
            .probe_freshness_if_due(Duration::from_secs(60))
            .expect("freshness probe should succeed");
        assert_eq!(first.state, WorkspaceFreshnessState::Fresh);

        repo.write("added.rs", "const NAME: &str = \"DIRTY\";\n");

        let cached = workspace
            .probe_freshness_if_due(Duration::from_secs(60))
            .expect("cached freshness probe should succeed");
        assert_eq!(cached.state, WorkspaceFreshnessState::Fresh);

        let refreshed = workspace
            .probe_freshness_if_due(Duration::ZERO)
            .expect("fresh freshness probe should succeed");
        assert_eq!(refreshed.state, WorkspaceFreshnessState::Stale);
        assert_eq!(refreshed.new_files, 1);
    }

    #[test]
    fn ensure_base_snapshot_invalidates_cached_searcher_after_rebuild() {
        let repo = TestRepo::new();
        repo.write("base.rs", "const NAME: &str = \"BASE\";\n");
        repo.build();

        let workspace = WorkspaceIndex::open(WorkspaceIndexOptions {
            build_config: repo.build_config(),
        })
        .expect("workspace should open");

        let initial_status = workspace.status().expect("status should succeed");
        let initial_dirty = initial_status.dirty_files.expect("dirty diff should exist");
        assert!(initial_dirty.modified_files.is_empty());
        assert!(initial_dirty.deleted_files.is_empty());
        assert!(initial_dirty.new_files.is_empty());

        repo.write("added.rs", "const NAME: &str = \"ADDED\";\n");

        let stale_status = workspace.status().expect("status should succeed");
        let stale_dirty = stale_status.dirty_files.expect("dirty diff should exist");
        assert_eq!(stale_dirty.new_files.len(), 1);

        workspace
            .ensure_base_snapshot()
            .expect("rebuild should succeed");

        let refreshed_status = workspace.status().expect("status should succeed");
        let refreshed_dirty = refreshed_status
            .dirty_files
            .expect("dirty diff should exist");
        assert!(refreshed_dirty.modified_files.is_empty());
        assert!(refreshed_dirty.deleted_files.is_empty());
        assert!(refreshed_dirty.new_files.is_empty());
    }

    #[test]
    fn reconcile_dirty_diff_keeps_only_paths_that_are_still_dirty() {
        let repo = TestRepo::new();
        let tracked = repo.write("tracked.rs", "const NAME: &str = \"BASE\";\n");
        let removed = repo.write("removed.rs", "const NAME: &str = \"REMOVED\";\n");
        repo.build();

        let workspace = WorkspaceIndex::open(WorkspaceIndexOptions {
            build_config: repo.build_config(),
        })
        .expect("workspace should open");

        let added = repo.write("added.rs", "const NAME: &str = \"ADDED\";\n");
        fs::remove_file(&removed).expect("remove should succeed");
        fs::remove_file(&added).expect("remove should succeed");

        let reconciled = workspace
            .reconcile_dirty_diff(&IndexWorktreeDiff {
                modified_files: vec![tracked.to_string_lossy().into_owned()],
                deleted_files: vec![removed.to_string_lossy().into_owned()],
                new_files: vec![added.to_string_lossy().into_owned()],
            })
            .expect("reconcile should succeed");

        assert!(reconciled.modified_files.is_empty());
        assert_eq!(
            reconciled.deleted_files,
            vec![removed.to_string_lossy().into_owned()]
        );
        assert!(reconciled.new_files.is_empty());
    }

    #[test]
    fn classify_dirty_paths_batches_mixed_changes() {
        let repo = TestRepo::new();
        let tracked = repo.write("tracked.rs", "const NAME: &str = \"BASE\";\n");
        let removed = repo.write("removed.rs", "const NAME: &str = \"REMOVED\";\n");
        repo.build();

        let workspace = WorkspaceIndex::open(WorkspaceIndexOptions {
            build_config: repo.build_config(),
        })
        .expect("workspace should open");

        repo.write("tracked.rs", "const NAME: &str = \"DIRTY\";\n");
        fs::remove_file(&removed).expect("remove should succeed");
        let added = repo.write("added.rs", "const NAME: &str = \"ADDED\";\n");

        let classified = workspace
            .classify_dirty_paths([
                tracked.to_string_lossy().into_owned(),
                removed.to_string_lossy().into_owned(),
                added.to_string_lossy().into_owned(),
                added.to_string_lossy().into_owned(),
            ])
            .expect("classify should succeed");

        assert_eq!(
            classified.modified_files,
            vec![tracked.to_string_lossy().into_owned()]
        );
        assert_eq!(
            classified.deleted_files,
            vec![removed.to_string_lossy().into_owned()]
        );
        assert_eq!(
            classified.new_files,
            vec![added.to_string_lossy().into_owned()]
        );
    }

    #[test]
    fn git_workspace_status_uses_filtered_dirty_paths() {
        let repo = TestRepo::new();
        let git_repo = repo.repo.clone();
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["init", "-q"])
            .status()
            .expect("git init should succeed");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["config", "user.email", "test@example.com"])
            .status()
            .expect("git config should succeed");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["config", "user.name", "Test User"])
            .status()
            .expect("git config should succeed");
        let tracked = repo.write("tracked.rs", "const NAME: &str = \"BASE\";\n");
        repo.write(".gitignore", "ignored.rs\n");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["add", "."])
            .status()
            .expect("git add should succeed");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["commit", "-qm", "initial"])
            .status()
            .expect("git commit should succeed");
        repo.build();

        repo.write("tracked.rs", "const NAME: &str = \"DIRTY\";\n");
        let added = repo.write("added.rs", "const NAME: &str = \"ADDED\";\n");
        repo.write(".hidden.rs", "const NAME: &str = \"HIDDEN\";\n");
        repo.write("ignored.rs", "const NAME: &str = \"IGNORED\";\n");

        let workspace = WorkspaceIndex::open(WorkspaceIndexOptions {
            build_config: repo.build_config(),
        })
        .expect("workspace should open");
        let status = workspace.status().expect("status should succeed");
        let dirty = status.dirty_files.expect("dirty diff should exist");

        assert_eq!(
            dirty.modified_files,
            vec![tracked.to_string_lossy().into_owned()]
        );
        assert_eq!(dirty.deleted_files, Vec::<String>::new());
        assert_eq!(dirty.new_files, vec![added.to_string_lossy().into_owned()]);
    }

    #[test]
    fn git_workspace_status_detects_files_revealed_by_gitignore_change() {
        let repo = TestRepo::new();
        let git_repo = repo.repo.clone();
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["init", "-q"])
            .status()
            .expect("git init should succeed");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["config", "user.email", "test@example.com"])
            .status()
            .expect("git config should succeed");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["config", "user.name", "Test User"])
            .status()
            .expect("git config should succeed");
        repo.write("tracked.rs", "const NAME: &str = \"BASE\";\n");
        repo.write(".gitignore", "revealed.rs\n");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["add", "."])
            .status()
            .expect("git add should succeed");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["commit", "-qm", "initial"])
            .status()
            .expect("git commit should succeed");
        repo.build();

        let revealed = repo.write("revealed.rs", "const NAME: &str = \"REVEALED\";\n");
        repo.write(".gitignore", "");

        let workspace = WorkspaceIndex::open(WorkspaceIndexOptions {
            build_config: repo.build_config(),
        })
        .expect("workspace should open");
        let status = workspace.status().expect("status should succeed");
        let dirty = status.dirty_files.expect("dirty diff should exist");

        assert_eq!(dirty.modified_files, Vec::<String>::new());
        assert_eq!(dirty.deleted_files, Vec::<String>::new());
        assert_eq!(
            dirty.new_files,
            vec![revealed.to_string_lossy().into_owned()]
        );
    }

    #[test]
    fn git_workspace_status_skips_new_files_that_cannot_enter_the_index() {
        let repo = TestRepo::new();
        let git_repo = repo.repo.clone();
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["init", "-q"])
            .status()
            .expect("git init should succeed");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["config", "user.email", "test@example.com"])
            .status()
            .expect("git config should succeed");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["config", "user.name", "Test User"])
            .status()
            .expect("git config should succeed");
        repo.write("tracked.rs", "const NAME: &str = \"BASE\";\n");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["add", "."])
            .status()
            .expect("git add should succeed");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["commit", "-qm", "initial"])
            .status()
            .expect("git commit should succeed");
        repo.build();

        let mut bytes = vec![b'a'; 8 * 1024];
        bytes.push(0xFF);
        repo.write_bytes("encoded.txt", &bytes);

        let workspace = WorkspaceIndex::open(WorkspaceIndexOptions {
            build_config: repo.build_config(),
        })
        .expect("workspace should open");
        let status = workspace.status().expect("status should succeed");
        let dirty = status.dirty_files.expect("dirty diff should exist");

        assert_eq!(dirty.modified_files, Vec::<String>::new());
        assert_eq!(dirty.deleted_files, Vec::<String>::new());
        assert_eq!(dirty.new_files, Vec::<String>::new());
    }

    #[test]
    fn workspace_exposes_git_snapshot_identity() {
        let repo = TestRepo::new();
        let git_repo = repo.repo.clone();
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["init", "-q"])
            .status()
            .expect("git init should succeed");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["config", "user.email", "test@example.com"])
            .status()
            .expect("git config should succeed");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["config", "user.name", "Test User"])
            .status()
            .expect("git config should succeed");
        repo.write("tracked.rs", "const NAME: &str = \"PM_RESUME\";\n");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["add", "."])
            .status()
            .expect("git add should succeed");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&git_repo)
            .args(["commit", "-qm", "initial"])
            .status()
            .expect("git commit should succeed");

        let workspace = WorkspaceIndex::open(WorkspaceIndexOptions {
            build_config: repo.build_config(),
        })
        .expect("workspace should open");

        let base = workspace
            .ensure_base_snapshot()
            .expect("base snapshot should build");
        assert_eq!(base.snapshot_kind, BaseSnapshotKind::GitCommit);
        assert!(base.snapshot_key.starts_with("base-git-"));
        assert!(base.head_commit.is_some());
        assert!(base.config_fingerprint.is_some());
    }
}
