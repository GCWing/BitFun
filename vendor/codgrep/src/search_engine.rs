use std::io::ErrorKind;

use crate::{
    config::{BuildConfig, QueryConfig},
    error::{AppError, Result},
    files::{scan_paths, RepositoryFile},
    index::{
        build_index_with_options, rebuild_index_with_options,
        searcher::{IndexSearcher, IndexWorktreeDiff},
        IndexBuildOptions,
    },
    path_filter::PathFilter,
    query_preflight::{evaluate_index_query_preflight, preflight_enabled},
    search::SearchResults,
};

#[path = "search_engine/document.rs"]
mod document;
#[path = "search_engine/scan.rs"]
mod scan;

pub(crate) use document::{SearchDocument, SearchDocumentIndex, SearchDocumentSource};
use scan::{scan_options, scan_text_files};
pub(crate) use scan::{search_document_index, search_documents, search_scanned_files};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchPolicy {
    IndexOnly,
    FallbackToScan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchBackend {
    Index,
    Scan,
}

/// Complete response for one `SearchEngine` query invocation.
///
/// - `backend` records which execution backend was used.
/// - `results` contains the match payload for that execution.
#[derive(Debug, Clone)]
pub struct SearchResponse {
    pub backend: SearchBackend,
    pub results: SearchResults,
}

/// Simple index-or-scan search facade.
///
/// Prefer this type when you want direct search execution without workspace
/// view caching or dirty-overlay management. For workspace-aware, long-lived
/// integrations, prefer `WorkspaceIndex`.
#[derive(Debug, Clone)]
pub struct SearchEngine {
    build_config: BuildConfig,
}

impl SearchEngine {
    pub fn new(build_config: BuildConfig) -> Self {
        Self {
            build_config: build_config.normalize_lossy(),
        }
    }

    pub fn build_index(&self) -> Result<usize> {
        self.build_index_with_options(IndexBuildOptions::default())
    }

    pub fn build_index_with_options(&self, options: IndexBuildOptions<'_>) -> Result<usize> {
        build_index_with_options(&self.build_config, options)
    }

    pub fn rebuild_index(&self) -> Result<usize> {
        self.rebuild_index_with_options(IndexBuildOptions::default())
    }

    pub fn rebuild_index_with_options(&self, options: IndexBuildOptions<'_>) -> Result<usize> {
        rebuild_index_with_options(&self.build_config, options)
    }

    pub fn diff_against_worktree(&self) -> Result<IndexWorktreeDiff> {
        self.open_searcher()?.diff_against_worktree()
    }

    /// Returns a simplified human-readable stale reason, if any.
    ///
    /// For structured freshness state, prefer `WorkspaceIndex::probe_freshness`.
    pub fn stale_reason(&self) -> Result<Option<String>> {
        self.open_searcher()?.stale_reason()
    }

    pub fn search(&self, config: &QueryConfig, policy: SearchPolicy) -> Result<SearchResponse> {
        self.search_with_filter(config, None, policy)
    }

    /// Convenience search entry with an optional external path filter.
    ///
    /// This keeps path-scoping opt-in without introducing additional
    /// `search_with_*` API variants.
    pub fn search_with_filter(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
        policy: SearchPolicy,
    ) -> Result<SearchResponse> {
        match self.open_searcher() {
            Ok(searcher) => {
                let query_prefers_scan = if preflight_enabled() {
                    let preflight =
                        evaluate_index_query_preflight(&searcher, config, filter, None)?;
                    preflight.reason.requires_scan_backend()
                } else {
                    false
                };
                self.search_with_searcher(searcher, config, filter, policy, query_prefers_scan)
            }
            Err(AppError::Io(error)) if error.kind() == ErrorKind::NotFound => {
                self.search_without_index(config, filter, policy)
            }
            Err(error) => Err(error),
        }
    }

    fn search_with_searcher(
        &self,
        searcher: IndexSearcher,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
        policy: SearchPolicy,
        query_prefers_scan: bool,
    ) -> Result<SearchResponse> {
        if matches!(policy, SearchPolicy::FallbackToScan) && query_prefers_scan {
            let files = self.scan_repo_files(filter)?;
            return Ok(SearchResponse {
                backend: SearchBackend::Scan,
                results: search_scanned_files(config, &files)?,
            });
        }

        Ok(SearchResponse {
            backend: SearchBackend::Index,
            results: searcher.search_with_filter(config, filter)?,
        })
    }

    fn search_without_index(
        &self,
        config: &QueryConfig,
        filter: Option<&PathFilter>,
        policy: SearchPolicy,
    ) -> Result<SearchResponse> {
        match policy {
            SearchPolicy::IndexOnly => Err(AppError::InvalidIndex(format!(
                "index does not exist at {}",
                self.build_config.index_path.display()
            ))),
            SearchPolicy::FallbackToScan => {
                let files = self.scan_repo_files(filter)?;
                Ok(SearchResponse {
                    backend: SearchBackend::Scan,
                    results: search_scanned_files(config, &files)?,
                })
            }
        }
    }

    fn open_searcher(&self) -> Result<IndexSearcher> {
        IndexSearcher::open(self.build_config.index_path.clone())
    }

    fn scan_repo_files(&self, filter: Option<&PathFilter>) -> Result<Vec<RepositoryFile>> {
        scan_text_files(scan_paths(
            std::slice::from_ref(&self.build_config.repo_path),
            Some(&self.build_config.index_path),
            scan_options(&self.build_config),
            filter,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{search_document_index, search_documents, SearchDocument, SearchDocumentIndex};
    use crate::{
        config::{QueryConfig, TokenizerMode},
        search::SearchMode,
        tokenizer::TokenizerOptions,
    };

    fn materialize_fixed_query(pattern: &str) -> QueryConfig {
        QueryConfig {
            regex_pattern: regex::escape(pattern),
            patterns: vec![pattern.to_string()],
            fixed_strings: true,
            search_mode: SearchMode::MaterializeMatches,
            ..QueryConfig::default()
        }
    }

    fn materialize_regex_query(pattern: &str, global_max_results: Option<usize>) -> QueryConfig {
        QueryConfig {
            regex_pattern: pattern.to_string(),
            patterns: vec![pattern.to_string()],
            global_max_results,
            search_mode: SearchMode::MaterializeMatches,
            ..QueryConfig::default()
        }
    }

    fn document_index(documents: Vec<SearchDocument>) -> SearchDocumentIndex {
        SearchDocumentIndex::build(
            TokenizerMode::Trigram,
            TokenizerOptions::default(),
            documents,
        )
    }

    #[test]
    fn search_documents_matches_fixed_string_phrase_with_spaces_in_loaded_bytes() {
        let pattern = "VersionTuple watcher validation content gamma";
        let documents = vec![SearchDocument::from_loaded_bytes(
            "doc.txt".into(),
            pattern.len() as u64,
            0,
            Arc::<[u8]>::from(format!("prefix {pattern} suffix\n").into_bytes()),
        )];

        let results = search_documents(&materialize_fixed_query(pattern), &documents)
            .expect("search should succeed");

        assert_eq!(results.matched_lines, 1);
        assert_eq!(results.matched_occurrences, 1);
        assert_eq!(results.hits.len(), 1);
        assert_eq!(results.hits[0].matches.len(), 1);
        assert_eq!(results.hits[0].matches[0].matched_text, pattern);
    }

    #[test]
    fn search_documents_matches_fixed_single_token_in_loaded_bytes() {
        let pattern = "VERSIONTUPLE_WATCHER_VALIDATION_GAMMA_20260410";
        let documents = vec![SearchDocument::from_loaded_bytes(
            "doc.txt".into(),
            pattern.len() as u64,
            0,
            Arc::<[u8]>::from(format!("prefix {pattern} suffix\n").into_bytes()),
        )];

        let results = search_documents(&materialize_fixed_query(pattern), &documents)
            .expect("search should succeed");

        assert_eq!(results.matched_lines, 1);
        assert_eq!(results.matched_occurrences, 1);
        assert_eq!(results.hits.len(), 1);
        assert_eq!(results.hits[0].matches[0].matched_text, pattern);
    }

    #[test]
    fn search_document_index_matches_fixed_string_phrase_with_spaces_in_loaded_bytes() {
        let pattern = "VersionTuple watcher validation content gamma";
        let documents = vec![
            SearchDocument::from_loaded_bytes(
                "miss.txt".into(),
                4,
                0,
                Arc::<[u8]>::from(b"miss\n".to_vec()),
            ),
            SearchDocument::from_loaded_bytes(
                "hit.txt".into(),
                pattern.len() as u64,
                0,
                Arc::<[u8]>::from(format!("prefix {pattern} suffix\n").into_bytes()),
            ),
        ];
        let index = document_index(documents.clone());

        let direct = search_documents(&materialize_fixed_query(pattern), &documents)
            .expect("direct search should succeed");
        let indexed = search_document_index(&materialize_fixed_query(pattern), &index, None)
            .expect("indexed search should succeed");

        assert_eq!(indexed.matched_lines, direct.matched_lines);
        assert_eq!(indexed.matched_occurrences, direct.matched_occurrences);
        assert_eq!(indexed.hits.len(), 1);
        assert_eq!(indexed.hits[0].path, "hit.txt");
        assert_eq!(indexed.candidate_docs, 1);
    }

    #[test]
    fn search_document_index_matches_fixed_single_token_in_loaded_bytes() {
        let pattern = "VERSIONTUPLE_WATCHER_VALIDATION_GAMMA_20260410";
        let documents = vec![
            SearchDocument::from_loaded_bytes(
                "miss.txt".into(),
                4,
                0,
                Arc::<[u8]>::from(b"miss\n".to_vec()),
            ),
            SearchDocument::from_loaded_bytes(
                "hit.txt".into(),
                pattern.len() as u64,
                0,
                Arc::<[u8]>::from(format!("prefix {pattern} suffix\n").into_bytes()),
            ),
        ];
        let index = document_index(documents.clone());

        let direct = search_documents(&materialize_fixed_query(pattern), &documents)
            .expect("direct search should succeed");
        let indexed = search_document_index(&materialize_fixed_query(pattern), &index, None)
            .expect("indexed search should succeed");

        assert_eq!(indexed.matched_lines, direct.matched_lines);
        assert_eq!(indexed.matched_occurrences, direct.matched_occurrences);
        assert_eq!(indexed.hits.len(), 1);
        assert_eq!(indexed.hits[0].path, "hit.txt");
        assert_eq!(indexed.candidate_docs, 1);
    }

    #[test]
    fn search_documents_honors_global_result_limit_for_regex_verification() {
        let documents = vec![
            SearchDocument::from_loaded_bytes(
                "one.txt".into(),
                12,
                0,
                Arc::<[u8]>::from(b"foo1\nfoo2\n".to_vec()),
            ),
            SearchDocument::from_loaded_bytes(
                "two.txt".into(),
                5,
                0,
                Arc::<[u8]>::from(b"foo3\n".to_vec()),
            ),
        ];

        let results = search_documents(&materialize_regex_query(r"foo\d", Some(2)), &documents)
            .expect("search should succeed");

        assert_eq!(results.matched_lines, 2);
        assert_eq!(results.searches_with_match, 1);
        assert_eq!(results.hits.len(), 1);
    }
}
