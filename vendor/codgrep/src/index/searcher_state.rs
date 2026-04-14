use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    config::BuildConfig,
    config::CorpusMode,
    error::{AppError, Result},
    files::{
        is_indexable_text_file, is_workspace_internal_path, resolve_repo_path, scan_repository,
    },
    index::format::{
        validate_index_layout, DocMetaRef, IndexBuildSettings, IndexLayout, LookupTable,
        PostingsData,
    },
    path_filter::PathFilter,
    tokenizer::TokenizerOptions,
};

use super::{shared::doc_by_id, IndexSearcher, IndexWorktreeDiff, IndexedDocState};

impl IndexSearcher {
    pub fn open(index_path: PathBuf) -> Result<Self> {
        let layout = IndexLayout::resolve(&index_path)?;
        let docs = crate::index::format::DocsData::open(&layout.docs_path)?;
        let metadata = docs.metadata().clone();
        validate_index_layout(&layout, &metadata, docs.len())?;
        let lookup = LookupTable::open(&layout.lookup_path)?;
        let postings = PostingsData::open(&layout.postings_path)?;

        Ok(Self {
            index_path,
            metadata: metadata.clone(),
            docs,
            lookup,
            postings,
            repo_root: metadata
                .build
                .as_ref()
                .map(|build| resolve_repo_path(Path::new(&build.repo_root), ".")),
            tokenizer_mode: metadata.tokenizer,
            tokenizer_options: TokenizerOptions {
                min_sparse_len: metadata.min_sparse_len,
                max_sparse_len: metadata.max_sparse_len,
            },
            doc_state_by_path: std::sync::OnceLock::new(),
            indexed_paths: std::sync::OnceLock::new(),
        })
    }

    pub fn build_settings(&self) -> Option<&IndexBuildSettings> {
        self.metadata.build.as_ref()
    }

    pub fn tokenizer_mode(&self) -> crate::config::TokenizerMode {
        self.tokenizer_mode
    }

    pub fn tokenizer_options(&self) -> &TokenizerOptions {
        &self.tokenizer_options
    }

    pub fn doc_count(&self) -> usize {
        self.docs.len()
    }

    pub fn ensure_query_scope_supported(
        &self,
        include_hidden: bool,
        no_ignore: bool,
        max_file_size: Option<u64>,
    ) -> Result<()> {
        let Some(build) = self.build_settings() else {
            if include_hidden || no_ignore || max_file_size.is_some() {
                return Err(AppError::InvalidIndex(
                    "index metadata is too old for search-scope overrides; rebuild the index"
                        .into(),
                ));
            }
            return Ok(());
        };

        if include_hidden && !build.include_hidden {
            return Err(AppError::InvalidIndex(
                "searching hidden files requires an index built with --hidden".into(),
            ));
        }
        if no_ignore && matches!(build.corpus_mode, crate::config::CorpusMode::RespectIgnore) {
            return Err(AppError::InvalidIndex(
                "searching ignored files requires an index built with --no-ignore".into(),
            ));
        }
        if max_file_size.is_some_and(|value| value > build.max_file_size) {
            return Err(AppError::InvalidIndex(format!(
                "search --max-filesize={} exceeds the index build limit of {} bytes",
                max_file_size.unwrap_or_default(),
                build.max_file_size,
            )));
        }
        Ok(())
    }

    pub fn diff_against_worktree(&self) -> Result<IndexWorktreeDiff> {
        let Some(build) = self.build_settings() else {
            return Ok(IndexWorktreeDiff::default());
        };

        if let Some(diff) = self.diff_against_worktree_git(build)? {
            return Ok(diff);
        }

        let current_files = scan_repository(&BuildConfig {
            repo_path: PathBuf::from(&build.repo_root),
            index_path: self.index_path.clone(),
            tokenizer: self.metadata.tokenizer,
            corpus_mode: build.corpus_mode,
            include_hidden: build.include_hidden,
            max_file_size: build.max_file_size,
            min_sparse_len: self.metadata.min_sparse_len,
            max_sparse_len: self.metadata.max_sparse_len,
        })?;
        let current = current_files
            .into_iter()
            .filter_map(|file| match is_indexable_text_file(&file.path) {
                Ok(true) => Some(Ok(file)),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .map(|file| {
                (
                    file.path.to_string_lossy().into_owned(),
                    (file.size, file.mtime_nanos),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut modified_files = Vec::new();
        let mut deleted_files = Vec::new();
        for (path, state) in self.doc_state_by_path() {
            match current.get(path.as_str()) {
                Some(&(size, mtime_nanos))
                    if size == state.size && mtime_nanos == state.mtime_nanos => {}
                Some(_) => modified_files.push(path.clone()),
                None => deleted_files.push(path.clone()),
            }
        }

        modified_files.sort_unstable();
        deleted_files.sort_unstable();
        let mut new_files = current
            .keys()
            .filter(|path| !self.doc_state_by_path().contains_key(path.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        new_files.sort_unstable();

        Ok(IndexWorktreeDiff {
            modified_files,
            deleted_files,
            new_files,
        })
    }

    fn diff_against_worktree_git(
        &self,
        build: &IndexBuildSettings,
    ) -> Result<Option<IndexWorktreeDiff>> {
        let Some(head_commit) = build.head_commit.as_deref() else {
            return Ok(None);
        };
        if !matches!(build.corpus_mode, CorpusMode::RespectIgnore) {
            return Ok(None);
        }

        let repo_root = self
            .repo_root
            .as_ref()
            .cloned()
            .unwrap_or_else(|| PathBuf::from(&build.repo_root));
        let tracked_paths = self.git_changed_paths(
            &repo_root,
            ["diff", "--name-only", "-z", head_commit, "--", "."],
        )?;
        let tracked_visible_paths = self.git_visible_paths(
            &repo_root,
            ["ls-files", "-z", "--cached", "--", "."],
            !build.include_hidden,
        )?;
        let untracked_visible_paths = self.git_visible_paths(
            &repo_root,
            [
                "ls-files",
                "-z",
                "--others",
                "--exclude-standard",
                "--",
                ".",
            ],
            !build.include_hidden,
        )?;

        let mut tracked_diff = self.classify_dirty_paths(tracked_paths)?;
        tracked_diff.new_files.clear();

        let tracked_modified = tracked_diff
            .modified_files
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let tracked_deleted = tracked_diff
            .deleted_files
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let tracked_visible_new_candidates = tracked_visible_paths
            .iter()
            .filter(|path| !self.doc_state_by_path().contains_key(path.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let untracked_visible_new_candidates = untracked_visible_paths
            .iter()
            .filter(|path| !self.doc_state_by_path().contains_key(path.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let mut new_files = self
            .classify_dirty_paths(tracked_visible_new_candidates)?
            .new_files;
        new_files.extend(
            self.classify_dirty_paths(untracked_visible_new_candidates)?
                .new_files,
        );
        new_files.sort_unstable();
        new_files.dedup();

        let mut deleted_files = tracked_deleted;
        deleted_files.extend(
            self.indexed_path_set()
                .iter()
                .filter(|path| !tracked_visible_paths.contains(path.as_str()))
                .filter(|path| !tracked_modified.contains(path.as_str()))
                .cloned(),
        );

        let mut modified_files = tracked_modified.into_iter().collect::<Vec<_>>();
        let mut deleted_files = deleted_files.into_iter().collect::<Vec<_>>();
        modified_files.sort_unstable();
        deleted_files.sort_unstable();

        Ok(Some(IndexWorktreeDiff {
            modified_files,
            deleted_files,
            new_files,
        }))
    }

    pub fn stale_reason(&self) -> Result<Option<String>> {
        let diff = self.diff_against_worktree()?;
        if let Some(path) = diff.modified_files.first() {
            return Ok(Some(format!("indexed file changed: {path}")));
        }
        if let Some(path) = diff.deleted_files.first() {
            return Ok(Some(format!("indexed file disappeared: {path}")));
        }
        if let Some(path) = diff.new_files.first() {
            return Ok(Some(format!("new file is missing from the index: {path}")));
        }
        Ok(None)
    }

    pub(crate) fn reconcile_dirty_paths(
        &self,
        dirty: &IndexWorktreeDiff,
    ) -> Result<IndexWorktreeDiff> {
        if dirty.is_empty() {
            return Ok(IndexWorktreeDiff::default());
        }

        let mut remaining = IndexWorktreeDiff::default();
        for path in &dirty.new_files {
            let path_buf = Path::new(path);
            if self.current_path_matches_corpus(path_buf)? {
                remaining.new_files.push(path.clone());
            }
        }

        for path in dirty
            .modified_files
            .iter()
            .chain(dirty.deleted_files.iter())
        {
            match self.doc_state_by_path().get(path) {
                Some(state) => match fs::metadata(path) {
                    Ok(metadata) => {
                        if !self.current_file_matches_corpus(Path::new(path), &metadata)? {
                            remaining.deleted_files.push(path.clone());
                            continue;
                        }
                        let modified = file_mtime_nanos(&metadata);
                        if metadata.len() != state.size || modified != state.mtime_nanos {
                            remaining.modified_files.push(path.clone());
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        remaining.deleted_files.push(path.clone());
                    }
                    Err(error) => return Err(error.into()),
                },
                None => {}
            }
        }

        remaining.modified_files.sort_unstable();
        remaining.deleted_files.sort_unstable();
        remaining.new_files.sort_unstable();
        Ok(remaining)
    }

    pub(crate) fn classify_dirty_paths<I>(&self, paths: I) -> Result<IndexWorktreeDiff>
    where
        I: IntoIterator<Item = String>,
    {
        let mut modified_files = Vec::new();
        let mut deleted_files = Vec::new();
        let mut new_files = Vec::new();
        let mut seen = HashSet::new();

        for path in paths {
            if !seen.insert(path.clone()) {
                continue;
            }
            match self.doc_state_by_path().get(&path) {
                Some(state) => match fs::metadata(&path) {
                    Ok(metadata) => {
                        if !self.current_file_matches_corpus(Path::new(&path), &metadata)? {
                            deleted_files.push(path);
                            continue;
                        }
                        let modified = file_mtime_nanos(&metadata);
                        if metadata.len() != state.size || modified != state.mtime_nanos {
                            modified_files.push(path);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        deleted_files.push(path);
                    }
                    Err(error) => return Err(error.into()),
                },
                None => {
                    let path_ref = Path::new(&path);
                    if self.current_path_matches_corpus(path_ref)? {
                        new_files.push(path);
                    }
                }
            }
        }

        modified_files.sort_unstable();
        deleted_files.sort_unstable();
        new_files.sort_unstable();
        Ok(IndexWorktreeDiff {
            modified_files,
            deleted_files,
            new_files,
        })
    }

    pub(crate) fn has_indexed_path_under(&self, directory: &str) -> bool {
        let prefix = format!("{directory}{}", std::path::MAIN_SEPARATOR);
        self.indexed_path_set()
            .range(directory.to_string()..)
            .next()
            .is_some_and(|path| path == directory || path.starts_with(&prefix))
    }

    pub(crate) fn indexed_paths_with_allowed_paths(
        &self,
        filter: Option<&PathFilter>,
        allowed_paths: Option<&HashSet<String>>,
    ) -> Vec<String> {
        self.docs
            .iter()
            .filter(|doc| self.doc_matches_filters(*doc, filter, allowed_paths))
            .map(|doc| self.doc_display_path_ref(doc))
            .collect()
    }

    pub(super) fn doc_display_path_ref(&self, doc: DocMetaRef<'_>) -> String {
        self.doc_resolved_path_ref(doc)
            .to_string_lossy()
            .into_owned()
    }

    pub(super) fn doc_resolved_path_ref(&self, doc: DocMetaRef<'_>) -> PathBuf {
        self.resolve_doc_path_str(doc.path())
    }

    pub(super) fn resolve_doc_path_by_id(&self, doc_id: u32) -> Result<PathBuf> {
        Ok(self.doc_resolved_path_ref(doc_by_id(&self.docs, doc_id)?))
    }

    fn doc_state_by_path(&self) -> &HashMap<String, IndexedDocState> {
        self.doc_state_by_path.get_or_init(|| {
            self.docs
                .iter()
                .map(|doc| {
                    (
                        self.doc_display_path_ref(doc),
                        IndexedDocState {
                            size: doc.size(),
                            mtime_nanos: doc.mtime_nanos(),
                        },
                    )
                })
                .collect()
        })
    }

    fn indexed_path_set(&self) -> &BTreeSet<String> {
        self.indexed_paths.get_or_init(|| {
            self.doc_state_by_path()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
    }

    fn git_changed_paths<const N: usize>(
        &self,
        repo_root: &Path,
        args: [&str; N],
    ) -> Result<Vec<String>> {
        Ok(self
            .git_path_output(repo_root, args)?
            .into_iter()
            .filter(|path| !is_workspace_internal_path(path) && !path.starts_with(&self.index_path))
            .map(|path| path.to_string_lossy().into_owned())
            .collect())
    }

    fn git_visible_paths<const N: usize>(
        &self,
        repo_root: &Path,
        args: [&str; N],
        filter_hidden: bool,
    ) -> Result<HashSet<String>> {
        Ok(self
            .git_path_output(repo_root, args)?
            .into_iter()
            .filter(|path| !is_workspace_internal_path(path) && !path.starts_with(&self.index_path))
            .filter(|path| !filter_hidden || !self.path_is_hidden_for_build(path, repo_root))
            .map(|path| path.to_string_lossy().into_owned())
            .collect())
    }

    fn git_path_output<const N: usize>(
        &self,
        repo_root: &Path,
        args: [&str; N],
    ) -> Result<Vec<PathBuf>> {
        let mut command = Command::new("git");
        command.arg("-C").arg(repo_root).args(args);
        if let Some(pathspec) = self.index_exclude_pathspec(repo_root) {
            command.arg(pathspec);
        }
        let output = command.output()?;
        if !output.status.success() {
            return Err(AppError::InvalidIndex(format!(
                "failed to inspect git worktree state for {}",
                repo_root.display()
            )));
        }

        Ok(output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| resolve_repo_path(repo_root, &String::from_utf8_lossy(path)))
            .collect())
    }

    fn current_path_matches_corpus(&self, path: &Path) -> Result<bool> {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        self.current_file_matches_corpus(path, &metadata)
    }

    fn current_file_matches_corpus(&self, path: &Path, metadata: &fs::Metadata) -> Result<bool> {
        if is_workspace_internal_path(path) || path.starts_with(&self.index_path) {
            return Ok(false);
        }
        let Some(build) = self.build_settings() else {
            return Ok(metadata.is_file() && is_indexable_text_file(path)?);
        };
        if !metadata.is_file() || metadata.len() > build.max_file_size {
            return Ok(false);
        }
        if !build.include_hidden && self.path_is_hidden_for_build(path, Path::new(&build.repo_root))
        {
            return Ok(false);
        }
        is_indexable_text_file(path)
    }

    fn path_is_hidden_for_build(&self, path: &Path, repo_root: &Path) -> bool {
        let relative = path.strip_prefix(repo_root).unwrap_or(path);
        relative.components().any(|component| match component {
            std::path::Component::Normal(value) => value.to_string_lossy().starts_with('.'),
            _ => false,
        })
    }

    fn index_exclude_pathspec(&self, repo_root: &Path) -> Option<String> {
        let relative = self.index_path.strip_prefix(repo_root).ok()?;
        if relative.as_os_str().is_empty() {
            return None;
        }
        let path = relative.to_string_lossy().replace('\\', "/");
        Some(format!(":(exclude){path}"))
    }

    pub(super) fn doc_display_path(&self, doc: &crate::index::format::DocMeta) -> String {
        self.doc_display_path_str(&doc.path)
    }

    fn doc_display_path_str(&self, path: &str) -> String {
        self.resolve_doc_path_str(path)
            .to_string_lossy()
            .into_owned()
    }

    fn resolve_doc_path_str(&self, path: &str) -> PathBuf {
        if Path::new(path).is_absolute() {
            return PathBuf::from(path);
        }

        if let Some(repo_root) = &self.repo_root {
            return resolve_repo_path(repo_root, path);
        }

        PathBuf::from(path)
    }
}

fn file_mtime_nanos(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| match u64::try_from(duration.as_nanos()) {
            Ok(value) => value,
            Err(_) => u64::MAX,
        })
        .unwrap_or_default()
}
