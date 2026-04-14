use std::{borrow::Cow, fs::File, path::Path};

use aho_corasick::{AhoCorasick, MatchKind};
use memmap2::Mmap;
use rayon::prelude::*;

use crate::{
    config::{BuildConfig, QueryConfig},
    error::{AppError, Result},
    files::{is_text_file, RepositoryFile, ScanOptions},
    index::{
        format::DocMeta,
        searcher::{
            verify::{
                build_matcher, requires_multiline_verification,
                verify_candidate_sources_with_resolver, VerifyCandidateSource,
            },
            CountVerifyKind, CountVerifyPlan, LinePrefilter,
        },
    },
    planner::plan,
    search::{FileMatch, MatchLocation, SearchHit, SearchLine, SearchResults},
};

use super::document::{SearchDocument, SearchDocumentIndex, SearchDocumentSource};

const PARALLEL_DOC_THRESHOLD: usize = 16;
const PARALLEL_BYTE_THRESHOLD: u64 = 8 * 1024 * 1024;

pub(crate) fn search_scanned_files(
    config: &QueryConfig,
    files: &[RepositoryFile],
) -> Result<SearchResults> {
    let documents = files
        .iter()
        .map(SearchDocument::from_repository_file)
        .collect::<Vec<_>>();
    search_documents(config, &documents)
}

pub(crate) fn search_documents(
    config: &QueryConfig,
    documents: &[SearchDocument],
) -> Result<SearchResults> {
    if let Some(results) = search_documents_multi_literal_fast(config, documents)? {
        return Ok(results);
    }

    let docs = repository_docs(documents)?;
    let candidate_ids = docs.iter().map(|doc| doc.doc_id).collect::<Vec<_>>();
    let multiline_verifier = requires_multiline_verification(config)?;
    let query_plan = plan(&config.regex_pattern)?;
    let line_prefilter = (!multiline_verifier)
        .then(|| LinePrefilter::compile(config, &query_plan))
        .flatten();
    match config.search_mode {
        crate::search::SearchMode::CountOnly | crate::search::SearchMode::CountMatches => {
            let verify_plan = CountVerifyPlan::compile(
                config,
                multiline_verifier,
                line_prefilter.clone(),
                if matches!(config.search_mode, crate::search::SearchMode::CountOnly) {
                    CountVerifyKind::Lines
                } else {
                    CountVerifyKind::Occurrences
                },
            )?;
            let resolve_source = |doc_id| scan_doc_source(documents, doc_id);
            let counts = verify_plan.verify_candidate_counts_by_doc_with_sources(
                &candidate_ids,
                docs.len(),
                &resolve_source,
            )?;
            let matched_docs = counts.len();
            let matched_lines = counts.iter().map(|count| count.matched_lines).sum();
            let matched_occurrences = counts.iter().map(|count| count.matched_occurrences).sum();
            let (file_counts, file_match_counts) =
                if matches!(config.search_mode, crate::search::SearchMode::CountOnly) {
                    (
                        counts
                            .into_iter()
                            .map(|count| crate::search::FileCount {
                                path: docs[usize::try_from(count.doc_id)
                                    .expect("doc id should fit usize")]
                                .path
                                .clone(),
                                matched_lines: count.matched_lines,
                            })
                            .collect(),
                        Vec::new(),
                    )
                } else {
                    (
                        Vec::new(),
                        counts
                            .into_iter()
                            .map(|count| crate::search::FileMatchCount {
                                path: docs[usize::try_from(count.doc_id)
                                    .expect("doc id should fit usize")]
                                .path
                                .clone(),
                                matched_occurrences: count.matched_occurrences,
                            })
                            .collect(),
                    )
                };
            Ok(SearchResults {
                candidate_docs: docs.len(),
                searches_with_match: matched_docs,
                bytes_searched: documents.iter().map(|document| document.size).sum(),
                matched_lines,
                matched_occurrences,
                file_counts,
                file_match_counts,
                hits: Vec::new(),
            })
        }
        _ => {
            let matcher = build_matcher(config, multiline_verifier)?;
            let resolve_source = |doc_id| scan_doc_source(documents, doc_id);
            let outcomes = verify_candidate_sources_with_resolver(
                &candidate_ids,
                docs.len(),
                &resolve_source,
                &matcher,
                line_prefilter.as_ref(),
                multiline_verifier,
                config.search_mode,
                config.max_count,
                config.effective_global_max_results(),
                config.before_context,
                config.after_context,
            )?;

            let mut hits = Vec::new();
            let mut matched_lines = 0usize;
            let mut matched_occurrences = 0usize;
            let mut searches_with_match = 0usize;
            let mut bytes_searched = 0u64;
            for task in outcomes {
                matched_lines += task.outcome.matched_lines;
                matched_occurrences += task.outcome.matched_occurrences;
                bytes_searched += task.outcome.bytes_searched;
                if !task.outcome.matches.is_empty() {
                    searches_with_match += 1;
                    let doc = &docs[usize::try_from(task.doc_id).map_err(|_| {
                        AppError::ValueOutOfRange(format!(
                            "doc id {} exceeds usize range",
                            task.doc_id
                        ))
                    })?];
                    hits.push(SearchHit {
                        path: doc.path.clone(),
                        matches: task.outcome.matches,
                        lines: task.outcome.lines,
                    });
                } else if task.outcome.matched_lines > 0 || task.outcome.matched_occurrences > 0 {
                    searches_with_match += 1;
                }
            }

            Ok(SearchResults {
                candidate_docs: docs.len(),
                searches_with_match,
                bytes_searched,
                matched_lines,
                matched_occurrences,
                file_counts: Vec::new(),
                file_match_counts: Vec::new(),
                hits,
            })
        }
    }
}

pub(crate) fn search_document_index(
    config: &QueryConfig,
    index: &SearchDocumentIndex,
    filter: Option<&crate::path_filter::PathFilter>,
) -> Result<SearchResults> {
    index.search(config, filter)
}

pub(crate) fn scan_options(config: &BuildConfig) -> ScanOptions {
    ScanOptions {
        respect_ignore: matches!(config.corpus_mode, crate::config::CorpusMode::RespectIgnore),
        include_hidden: config.include_hidden,
        max_file_size: config.max_file_size,
        max_depth: None,
        ignore_files: Vec::new(),
    }
}

pub(crate) fn scan_text_files(files: Vec<RepositoryFile>) -> Result<Vec<RepositoryFile>> {
    let mut text_files = Vec::with_capacity(files.len());
    for file in files {
        if is_text_file(Path::new(&file.path))? {
            text_files.push(file);
        }
    }
    Ok(text_files)
}

fn search_documents_multi_literal_fast(
    config: &QueryConfig,
    documents: &[SearchDocument],
) -> Result<Option<SearchResults>> {
    if config.has_context()
        || matches!(config.search_mode, crate::search::SearchMode::CountOnly)
        || matches!(config.search_mode, crate::search::SearchMode::CountMatches)
        || config.max_count.is_some()
        || config.fixed_strings
        || config.word_regexp
        || config.line_regexp
    {
        return Ok(None);
    }

    let plan = plan(&config.regex_pattern)?;
    let Some(alternation) = plan.pure_literal_alternation else {
        return Ok(None);
    };
    if config.case_insensitive
        && !alternation
            .literals
            .iter()
            .all(|literal| literal.is_ascii())
    {
        return Ok(None);
    }

    let matcher = AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostFirst)
        .ascii_case_insensitive(config.case_insensitive)
        .build(&alternation.literals)
        .map_err(|error| AppError::InvalidPattern(error.to_string()))?;

    let outcomes = if config.effective_global_max_results().is_none()
        && should_parallel_scan_documents(documents)
    {
        documents
            .par_iter()
            .map(|document| scan_document_multi_literal(document, &matcher, config.search_mode))
            .collect::<Vec<_>>()
    } else {
        documents
            .iter()
            .map(|document| scan_document_multi_literal(document, &matcher, config.search_mode))
            .collect::<Vec<_>>()
    };

    let mut matched_lines = 0usize;
    let mut matched_occurrences = 0usize;
    let mut searches_with_match = 0usize;
    let mut hits = Vec::new();
    let mut bytes_searched = 0u64;
    let global_limit = config.effective_global_max_results();

    for outcome in outcomes.into_iter().flatten() {
        matched_lines += outcome.matched_lines;
        matched_occurrences += outcome.matched_occurrences;
        bytes_searched += outcome.bytes_searched;
        if !outcome.matches.is_empty() {
            searches_with_match += 1;
            hits.push(SearchHit {
                path: outcome.path,
                matches: outcome.matches.clone(),
                lines: outcome.matches.into_iter().map(SearchLine::Match).collect(),
            });
        } else if outcome.matched_lines > 0 || outcome.matched_occurrences > 0 {
            searches_with_match += 1;
        }
        let consumed_results = match config.search_mode {
            crate::search::SearchMode::FirstHitOnly => searches_with_match,
            crate::search::SearchMode::MaterializeMatches => matched_lines,
            crate::search::SearchMode::CountOnly => matched_lines,
            crate::search::SearchMode::CountMatches => matched_occurrences,
        };
        if global_limit.is_some_and(|limit| consumed_results >= limit) {
            break;
        }
    }

    Ok(Some(SearchResults {
        candidate_docs: documents.len(),
        searches_with_match,
        bytes_searched,
        matched_lines,
        matched_occurrences,
        file_counts: Vec::new(),
        file_match_counts: Vec::new(),
        hits,
    }))
}

#[derive(Debug)]
struct MultiLiteralDocumentOutcome {
    path: String,
    matched_lines: usize,
    matched_occurrences: usize,
    bytes_searched: u64,
    matches: Vec<FileMatch>,
}

fn should_parallel_scan_documents(documents: &[SearchDocument]) -> bool {
    if std::thread::available_parallelism().map_or(1, std::num::NonZero::get) <= 1 {
        return false;
    }

    documents.len() >= PARALLEL_DOC_THRESHOLD
        || documents.iter().map(|document| document.size).sum::<u64>() >= PARALLEL_BYTE_THRESHOLD
}

fn scan_document_multi_literal(
    document: &SearchDocument,
    matcher: &AhoCorasick,
    mode: crate::search::SearchMode,
) -> Option<MultiLiteralDocumentOutcome> {
    let bytes = document_bytes(document)?;
    let bytes_searched = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    match mode {
        crate::search::SearchMode::CountOnly => {
            let matched_lines = count_matched_lines(&bytes, matcher);
            if matched_lines == 0 {
                None
            } else {
                Some(MultiLiteralDocumentOutcome {
                    path: document.logical_path.clone(),
                    matched_lines,
                    matched_occurrences: 0,
                    bytes_searched,
                    matches: Vec::new(),
                })
            }
        }
        crate::search::SearchMode::FirstHitOnly | crate::search::SearchMode::MaterializeMatches => {
            let first_only = matches!(mode, crate::search::SearchMode::FirstHitOnly);
            let matches = collect_document_matches(&bytes, matcher, first_only);
            if matches.is_empty() {
                None
            } else {
                Some(MultiLiteralDocumentOutcome {
                    path: document.logical_path.clone(),
                    matched_lines: count_unique_match_lines(&matches),
                    matched_occurrences: matches.len(),
                    bytes_searched,
                    matches,
                })
            }
        }
        crate::search::SearchMode::CountMatches => None,
    }
}

fn document_bytes(document: &SearchDocument) -> Option<Cow<'_, [u8]>> {
    match &document.source {
        SearchDocumentSource::Path(path) => {
            if document.size == 0 {
                return None;
            }
            let file = File::open(path).ok()?;
            let mmap = unsafe { Mmap::map(&file).ok()? };
            Some(Cow::Owned(mmap.to_vec()))
        }
        SearchDocumentSource::LoadedBytes(bytes) => Some(Cow::Borrowed(bytes)),
    }
}

fn count_matched_lines(bytes: &[u8], matcher: &AhoCorasick) -> usize {
    let mut matched_lines = 0usize;
    let mut last_match_end = 0usize;

    for matched in matcher.find_iter(bytes) {
        if matched_lines == 0 || bytes[last_match_end..matched.start()].contains(&b'\n') {
            matched_lines += 1;
        }
        last_match_end = matched.end();
    }

    matched_lines
}

fn collect_document_matches(
    bytes: &[u8],
    matcher: &AhoCorasick,
    first_only: bool,
) -> Vec<FileMatch> {
    let line_starts = build_line_starts(bytes);
    let mut matches = Vec::new();

    for matched in matcher.find_iter(bytes) {
        matches.push(build_document_file_match(
            bytes,
            matched.start(),
            matched.end(),
            &line_starts,
        ));
        if first_only {
            break;
        }
    }

    matches
}

fn count_unique_match_lines(matches: &[FileMatch]) -> usize {
    use std::collections::HashSet;

    matches
        .iter()
        .map(|matched| matched.location.line)
        .collect::<HashSet<_>>()
        .len()
}

fn build_document_file_match(
    bytes: &[u8],
    start: usize,
    end: usize,
    line_starts: &[usize],
) -> FileMatch {
    let line_index = line_index_for_offset(start, line_starts);
    let line_start = line_starts[line_index];
    let line_end = line_starts
        .get(line_index + 1)
        .copied()
        .unwrap_or(bytes.len());
    let snippet = trim_line_terminator_bytes(&bytes[line_start..line_end]);

    FileMatch {
        location: MatchLocation {
            line: line_index + 1,
            column: start.saturating_sub(line_start) + 1,
        },
        snippet: String::from_utf8_lossy(snippet).into_owned(),
        matched_text: String::from_utf8_lossy(&bytes[start..end]).into_owned(),
    }
}

fn build_line_starts(bytes: &[u8]) -> Vec<usize> {
    let mut starts = vec![0];
    for (idx, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            starts.push(idx + 1);
        }
    }
    starts
}

fn line_index_for_offset(offset: usize, line_starts: &[usize]) -> usize {
    match line_starts.binary_search(&offset) {
        Ok(index) => index,
        Err(index) => index.saturating_sub(1),
    }
}

fn trim_line_terminator_bytes(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

fn repository_docs(documents: &[SearchDocument]) -> Result<Vec<DocMeta>> {
    documents
        .iter()
        .enumerate()
        .map(|(index, document)| {
            let doc_id = u32::try_from(index).map_err(|_| {
                AppError::ValueOutOfRange(format!("scanned document id {index} exceeds u32 range"))
            })?;
            Ok(DocMeta {
                doc_id,
                path: document.logical_path.clone(),
                size: document.size,
                mtime_nanos: document.mtime_nanos,
            })
        })
        .collect()
}

fn scan_doc_source(documents: &[SearchDocument], doc_id: u32) -> Result<VerifyCandidateSource> {
    let index = usize::try_from(doc_id)
        .map_err(|_| AppError::ValueOutOfRange(format!("doc id {doc_id} exceeds usize range")))?;
    documents
        .get(index)
        .map(|document| match &document.source {
            SearchDocumentSource::Path(path) => VerifyCandidateSource::Path(path.clone()),
            SearchDocumentSource::LoadedBytes(bytes) => VerifyCandidateSource::Bytes(bytes.clone()),
        })
        .ok_or_else(|| {
            AppError::InvalidIndex(format!("doc id {doc_id} is missing from scan documents"))
        })
}
