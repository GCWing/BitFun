use std::{collections::HashSet, fs, path::PathBuf, sync::Arc, time::UNIX_EPOCH};

#[cfg(test)]
use std::collections::BTreeMap;

use crate::{
    config::BuildConfig,
    error::Result,
    files::{is_indexable_text_file, is_text_file, scan_paths, scan_repository, ScanOptions},
    index::IndexWorktreeDiff,
    path_filter::PathFilter,
    search::SearchResults,
    search_engine::{SearchDocument, SearchDocumentSource},
};

#[cfg(test)]
use crate::{index::IndexSearcher, planner::QueryPlan, QueryConfig};

#[cfg(test)]
pub(super) struct CandidateRuntime<'a> {
    searcher: &'a IndexSearcher,
    diff: &'a IndexWorktreeDiff,
    query: &'a QueryConfig,
    filter: Option<&'a PathFilter>,
}

#[cfg(test)]
pub(super) struct ResolvedDocuments {
    documents: Vec<SearchDocument>,
}

pub(super) struct OverlaySearchScope {
    pub(super) shadowed_base_paths: HashSet<String>,
    pub(super) dirty_documents: Vec<SearchDocument>,
}

#[derive(Debug, Clone)]
#[cfg(test)]
struct EffectiveCandidate {
    path: String,
}

#[cfg(test)]
struct EffectiveCandidateSet {
    by_path: BTreeMap<String, EffectiveCandidate>,
}

#[cfg(test)]
struct PlannedWorkspaceQuery {
    plan: QueryPlan,
}

pub(super) fn scan_text_repository_files(
    build_config: &BuildConfig,
    filter: Option<&PathFilter>,
) -> Result<Vec<SearchDocument>> {
    let mut files = Vec::new();
    let scanned = if filter.is_some() {
        scan_paths(
            std::slice::from_ref(&build_config.repo_path),
            Some(&build_config.index_path),
            ScanOptions {
                respect_ignore: matches!(
                    build_config.corpus_mode,
                    crate::config::CorpusMode::RespectIgnore
                ),
                include_hidden: build_config.include_hidden,
                max_file_size: build_config.max_file_size,
                max_depth: None,
                ignore_files: Vec::new(),
            },
            filter,
        )?
    } else {
        scan_repository(build_config)?
    };
    for file in scanned {
        if !is_text_file(&file.path)? {
            continue;
        }
        files.push(SearchDocument::from_repository_file(&file));
    }
    Ok(files)
}

#[cfg(test)]
impl<'a> CandidateRuntime<'a> {
    pub(super) fn new(
        searcher: &'a IndexSearcher,
        diff: &'a IndexWorktreeDiff,
        query: &'a QueryConfig,
        filter: Option<&'a PathFilter>,
    ) -> Self {
        Self {
            searcher,
            diff,
            query,
            filter,
        }
    }

    pub(super) fn candidate_documents(&self) -> Result<ResolvedDocuments> {
        let planned_query = PlannedWorkspaceQuery::new(self.query)?;
        let base_branches = self
            .searcher
            .candidate_doc_ids_by_branch_for_plan_with_allowed_paths(
                self.query,
                &planned_query.plan,
                self.filter,
                None,
            )?;
        let shadowed_paths = shadowed_paths(self.diff);
        let mut candidates = EffectiveCandidateSet::new();
        for branch_doc_ids in base_branches {
            candidates.extend_doc_ids(self.searcher, branch_doc_ids, Some(&shadowed_paths));
        }
        candidates.extend_paths(existing_dirty_paths(self.diff));
        resolve_documents(candidates, self.filter)
    }
}

#[cfg(test)]
impl ResolvedDocuments {
    pub(super) fn into_documents(self) -> Vec<SearchDocument> {
        self.documents
    }
}

#[cfg(test)]
impl EffectiveCandidateSet {
    fn new() -> Self {
        Self {
            by_path: BTreeMap::new(),
        }
    }

    fn extend_doc_ids(
        &mut self,
        searcher: &IndexSearcher,
        doc_ids: Vec<u32>,
        excluded_paths: Option<&HashSet<String>>,
    ) {
        for doc_id in doc_ids {
            let Ok(path) = searcher.doc_display_path_by_id(doc_id) else {
                continue;
            };
            if excluded_paths.is_some_and(|paths| paths.contains(&path)) {
                continue;
            }
            self.by_path
                .insert(path.clone(), EffectiveCandidate { path });
        }
    }

    fn extend_paths<I>(&mut self, paths: I)
    where
        I: IntoIterator<Item = String>,
    {
        for path in paths {
            self.by_path
                .insert(path.clone(), EffectiveCandidate { path });
        }
    }

    fn into_ordered(self) -> Vec<EffectiveCandidate> {
        self.by_path.into_values().collect()
    }
}

#[cfg(test)]
pub(super) fn candidate_documents(
    searcher: &IndexSearcher,
    diff: &IndexWorktreeDiff,
    query: &QueryConfig,
    filter: Option<&PathFilter>,
) -> Result<ResolvedDocuments> {
    CandidateRuntime::new(searcher, diff, query, filter).candidate_documents()
}

pub(super) fn overlay_search_scope(
    diff: &IndexWorktreeDiff,
    filter: Option<&PathFilter>,
) -> Result<OverlaySearchScope> {
    let shadowed_base_paths = diff
        .modified_files
        .iter()
        .chain(diff.deleted_files.iter())
        .cloned()
        .collect::<HashSet<_>>();
    let dirty_documents = resolve_dirty_documents(existing_dirty_paths(diff), filter)?;
    Ok(OverlaySearchScope {
        shadowed_base_paths,
        dirty_documents,
    })
}

pub(super) fn merge_search_results(
    mut base: SearchResults,
    mut dirty: SearchResults,
) -> SearchResults {
    base.candidate_docs += dirty.candidate_docs;
    base.searches_with_match += dirty.searches_with_match;
    base.bytes_searched += dirty.bytes_searched;
    base.matched_lines += dirty.matched_lines;
    base.matched_occurrences += dirty.matched_occurrences;
    base.file_counts.append(&mut dirty.file_counts);
    base.file_match_counts.append(&mut dirty.file_match_counts);
    base.hits.append(&mut dirty.hits);
    base.hits
        .sort_unstable_by(|left, right| left.path.cmp(&right.path));
    base.file_counts
        .sort_unstable_by(|left, right| left.path.cmp(&right.path));
    base.file_match_counts
        .sort_unstable_by(|left, right| left.path.cmp(&right.path));
    base
}

#[cfg(test)]
fn shadowed_paths(diff: &IndexWorktreeDiff) -> HashSet<String> {
    diff.modified_files
        .iter()
        .chain(diff.deleted_files.iter())
        .chain(diff.new_files.iter())
        .cloned()
        .collect()
}

fn existing_dirty_paths(diff: &IndexWorktreeDiff) -> impl Iterator<Item = String> + '_ {
    diff.modified_files
        .iter()
        .chain(diff.new_files.iter())
        .cloned()
}

fn search_document_from_path(logical_path: String, path: PathBuf) -> Result<SearchDocument> {
    let metadata = fs::metadata(&path)?;
    let mtime_nanos = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| match u64::try_from(duration.as_nanos()) {
            Ok(value) => value,
            Err(_) => u64::MAX,
        })
        .unwrap_or_default();

    Ok(SearchDocument {
        logical_path,
        size: metadata.len(),
        mtime_nanos,
        source: SearchDocumentSource::Path(path),
    })
}

pub(super) fn load_search_document(
    logical_path: String,
    path: PathBuf,
) -> Result<Option<SearchDocument>> {
    if !path.exists() || !is_indexable_text_file(&path)? {
        return Ok(None);
    }
    let metadata = fs::metadata(&path)?;
    let mtime_nanos = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| match u64::try_from(duration.as_nanos()) {
            Ok(value) => value,
            Err(_) => u64::MAX,
        })
        .unwrap_or_default();
    let bytes = fs::read(&path)?;
    Ok(Some(SearchDocument::from_loaded_bytes(
        logical_path,
        metadata.len(),
        mtime_nanos,
        Arc::<[u8]>::from(bytes),
    )))
}

#[cfg(test)]
impl PlannedWorkspaceQuery {
    fn new(query: &QueryConfig) -> Result<Self> {
        Ok(Self {
            plan: crate::planner::plan(&query.regex_pattern)?,
        })
    }
}

#[cfg(test)]
fn resolve_documents(
    candidates: EffectiveCandidateSet,
    filter: Option<&PathFilter>,
) -> Result<ResolvedDocuments> {
    let mut documents = Vec::new();
    for candidate in candidates.into_ordered() {
        let path = PathBuf::from(&candidate.path);
        if filter.is_some_and(|active| !active.matches_file(&path)) {
            continue;
        }
        if !path.exists() || !is_indexable_text_file(&path)? {
            continue;
        }
        documents.push(search_document_from_path(candidate.path, path)?);
    }
    Ok(ResolvedDocuments { documents })
}

fn resolve_dirty_documents<I>(paths: I, filter: Option<&PathFilter>) -> Result<Vec<SearchDocument>>
where
    I: IntoIterator<Item = String>,
{
    let mut documents = Vec::new();
    for logical_path in paths {
        let path = PathBuf::from(&logical_path);
        if filter.is_some_and(|active| !active.matches_file(&path)) {
            continue;
        }
        if !path.exists() || !is_indexable_text_file(&path)? {
            continue;
        }
        documents.push(search_document_from_path(logical_path, path)?);
    }
    Ok(documents)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::TempDir;

    use super::candidate_documents;
    use crate::{
        build_index,
        config::{BuildConfig, CorpusMode, TokenizerMode},
        index::IndexSearcher,
        QueryConfig,
    };

    struct RuntimeRepo {
        _temp: TempDir,
        repo: PathBuf,
        index: PathBuf,
    }

    impl RuntimeRepo {
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

        fn path(&self, relative: &str) -> PathBuf {
            self.repo.join(relative)
        }

        fn write(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent dir should succeed");
            }
            fs::write(&path, contents).expect("write should succeed");
            path
        }

        fn remove(&self, relative: &str) {
            fs::remove_file(self.path(relative)).expect("remove should succeed");
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

    fn query(pattern: &str) -> QueryConfig {
        QueryConfig {
            regex_pattern: pattern.into(),
            patterns: vec![pattern.into()],
            ..QueryConfig::default()
        }
    }

    fn candidate_paths(searcher: &IndexSearcher, pattern: &str) -> Vec<String> {
        let diff = searcher
            .diff_against_worktree()
            .expect("diff should succeed");
        candidate_documents(searcher, &diff, &query(pattern), None)
            .expect("candidate resolution should succeed")
            .into_documents()
            .into_iter()
            .map(|document| document.logical_path)
            .collect()
    }

    #[test]
    fn candidate_documents_include_dirty_new_files() {
        let repo = RuntimeRepo::new();
        repo.write("base.rs", "const NAME: &str = \"BASE\";\n");
        repo.build();
        let added = repo.write("added.rs", "const NAME: &str = \"DIRTY\";\n");

        let searcher = IndexSearcher::open(repo.index.clone()).expect("searcher should open");
        let paths = candidate_paths(&searcher, "DIRTY");

        assert_eq!(paths, vec![added.to_string_lossy().into_owned()]);
    }

    #[test]
    fn candidate_documents_keep_dirty_modified_paths_for_query_time_repair() {
        let repo = RuntimeRepo::new();
        let keep = repo.write("keep.rs", "const NAME: &str = \"BASE\";\n");
        repo.write("tracked.rs", "const NAME: &str = \"BASE\";\n");
        repo.build();

        repo.write("tracked.rs", "const NAME: &str = \"DIRTY\";\n");

        let searcher = IndexSearcher::open(repo.index.clone()).expect("searcher should open");
        let paths = candidate_paths(&searcher, "BASE");

        assert!(paths.contains(&keep.to_string_lossy().into_owned()));
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn candidate_documents_skip_deleted_dirty_paths() {
        let repo = RuntimeRepo::new();
        repo.write("keep.rs", "const NAME: &str = \"BASE\";\n");
        repo.write("deleted.rs", "const NAME: &str = \"BASE\";\n");
        repo.build();

        repo.remove("deleted.rs");

        let searcher = IndexSearcher::open(repo.index.clone()).expect("searcher should open");
        let paths = candidate_paths(&searcher, "BASE");

        assert_eq!(paths.len(), 1);
    }
}
