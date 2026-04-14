use std::{
    collections::HashSet,
    fs::File,
    path::{Path, PathBuf},
};

use aho_corasick::{AhoCorasick, MatchKind};
use memchr::memchr;
use memmap2::Mmap;
use rayon::prelude::*;

use crate::{
    error::{AppError, Result},
    index::format::{DocMetaRef, DocsData},
    planner::PureLiteralAlternation,
    search::{FileMatch, MatchLocation, SearchHit, SearchLine, SearchMode, SearchResults},
};

use super::shared::{
    build_line_starts, line_index_for_offset, resolve_candidate_docs, trim_line_terminator_bytes,
};

const PARALLEL_DOC_THRESHOLD: usize = 16;
const PARALLEL_BYTE_THRESHOLD: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(super) struct PreparedMultiLiteral {
    pub(super) literals: Vec<String>,
    pub(super) case_insensitive: bool,
}

pub(super) fn prepare(
    alternation: &PureLiteralAlternation,
    case_insensitive: bool,
) -> Option<PreparedMultiLiteral> {
    if case_insensitive
        && !alternation
            .literals
            .iter()
            .all(|literal| literal.is_ascii())
    {
        return None;
    }
    Some(PreparedMultiLiteral {
        literals: alternation.literals.clone(),
        case_insensitive,
    })
}

pub(super) fn collect_candidates<F>(
    docs: &DocsData,
    candidate_ids: Option<&[u32]>,
    prepared: &PreparedMultiLiteral,
    resolve_path: &F,
) -> Result<Vec<u32>>
where
    F: Fn(u32) -> Result<PathBuf> + Sync,
{
    let matcher = build_fast_matcher(prepared)?;
    let candidates = if let Some(candidate_ids) = candidate_ids {
        let candidate_docs = resolve_candidate_docs(docs, candidate_ids)?;
        if should_parallel_scan(
            candidate_docs.len(),
            candidate_docs.iter().map(|doc| doc.size()).sum(),
        ) {
            candidate_docs
                .par_iter()
                .map(|doc| scan_doc_id(*doc, resolve_path, &matcher))
                .collect::<Vec<_>>()
        } else {
            candidate_docs
                .iter()
                .map(|doc| scan_doc_id(*doc, resolve_path, &matcher))
                .collect::<Vec<_>>()
        }
    } else if should_parallel_scan(docs.len(), docs.iter().map(|doc| doc.size()).sum()) {
        docs.iter()
            .collect::<Vec<_>>()
            .par_iter()
            .map(|doc| scan_doc_id(*doc, resolve_path, &matcher))
            .collect::<Vec<_>>()
    } else {
        docs.iter()
            .map(|doc| scan_doc_id(doc, resolve_path, &matcher))
            .collect::<Vec<_>>()
    };
    Ok(candidates.into_iter().flatten().collect())
}

pub(super) fn search<F>(
    docs: &DocsData,
    candidate_ids: Option<&[u32]>,
    prepared: &PreparedMultiLiteral,
    mode: SearchMode,
    global_max_results: Option<usize>,
    resolve_path: &F,
) -> Result<SearchResults>
where
    F: Fn(u32) -> Result<PathBuf> + Sync,
{
    if matches!(mode, SearchMode::CountOnly) {
        return search_count_only(docs, candidate_ids, prepared, resolve_path);
    }

    let matcher = build_semantic_matcher(prepared)?;
    let candidate_doc_count = candidate_ids.map_or(docs.len(), <[u32]>::len);
    let outcomes = if let Some(candidate_ids) = candidate_ids {
        let candidate_docs = resolve_candidate_docs(docs, candidate_ids)?;
        if global_max_results.is_none()
            && should_parallel_scan(
                candidate_docs.len(),
                candidate_docs.iter().map(|doc| doc.size()).sum(),
            )
        {
            candidate_docs
                .par_iter()
                .map(|doc| scan_doc(*doc, resolve_path, &matcher, mode))
                .collect::<Vec<_>>()
        } else {
            candidate_docs
                .iter()
                .map(|doc| scan_doc(*doc, resolve_path, &matcher, mode))
                .collect::<Vec<_>>()
        }
    } else if global_max_results.is_none()
        && should_parallel_scan(docs.len(), docs.iter().map(|doc| doc.size()).sum())
    {
        docs.iter()
            .collect::<Vec<_>>()
            .par_iter()
            .map(|doc| scan_doc(*doc, resolve_path, &matcher, mode))
            .collect::<Vec<_>>()
    } else {
        docs.iter()
            .map(|doc| scan_doc(doc, resolve_path, &matcher, mode))
            .collect::<Vec<_>>()
    };

    let mut searches_with_match = 0usize;
    let mut matched_lines = 0usize;
    let mut hits = Vec::new();
    for outcome in outcomes.into_iter().flatten() {
        searches_with_match += 1;
        matched_lines += outcome.matched_lines;
        if !outcome.matches.is_empty() {
            hits.push(SearchHit {
                path: outcome.path,
                matches: outcome.matches.clone(),
                lines: outcome.matches.into_iter().map(SearchLine::Match).collect(),
            });
        }
        let consumed_results = match mode {
            SearchMode::FirstHitOnly => searches_with_match,
            SearchMode::MaterializeMatches => matched_lines,
            SearchMode::CountOnly => matched_lines,
            SearchMode::CountMatches => hits.iter().map(|hit| hit.matches.len()).sum(),
        };
        if global_max_results.is_some_and(|limit| consumed_results >= limit) {
            break;
        }
    }

    Ok(SearchResults {
        candidate_docs: candidate_doc_count,
        searches_with_match,
        bytes_searched: 0,
        matched_lines,
        matched_occurrences: hits.iter().map(|hit| hit.matches.len()).sum(),
        file_counts: Vec::new(),
        file_match_counts: Vec::new(),
        hits,
    })
}

#[derive(Debug)]
struct Outcome {
    path: String,
    matched_lines: usize,
    matches: Vec<FileMatch>,
}

#[derive(Debug)]
struct CountOutcome {
    path: String,
    matched_lines: usize,
}

fn build_fast_matcher(prepared: &PreparedMultiLiteral) -> Result<FastMatcher> {
    build_matcher(prepared, MatchKind::Standard)
}

fn build_semantic_matcher(prepared: &PreparedMultiLiteral) -> Result<AhoCorasick> {
    build_matcher(prepared, MatchKind::LeftmostFirst)
}

fn build_matcher(prepared: &PreparedMultiLiteral, match_kind: MatchKind) -> Result<AhoCorasick> {
    AhoCorasick::builder()
        .match_kind(match_kind)
        .ascii_case_insensitive(prepared.case_insensitive)
        .build(&prepared.literals)
        .map_err(|error| AppError::InvalidPattern(error.to_string()))
}

fn search_count_only<F>(
    docs: &DocsData,
    candidate_ids: Option<&[u32]>,
    prepared: &PreparedMultiLiteral,
    resolve_path: &F,
) -> Result<SearchResults>
where
    F: Fn(u32) -> Result<PathBuf> + Sync,
{
    let matcher = build_fast_matcher(prepared)?;
    let candidate_doc_count = candidate_ids.map_or(docs.len(), <[u32]>::len);
    let candidate_docs = if let Some(candidate_ids) = candidate_ids {
        resolve_candidate_docs(docs, candidate_ids)?
    } else {
        docs.iter().collect::<Vec<_>>()
    };
    let bytes_searched = candidate_docs.iter().map(|doc| doc.size()).sum();
    let file_counts = if should_parallel_scan(candidate_docs.len(), bytes_searched) {
        let mut counts = candidate_docs
            .par_iter()
            .enumerate()
            .map(|(ordinal, doc)| {
                count_doc(*doc, resolve_path, &matcher).map(|count| (ordinal, count))
            })
            .collect::<Vec<_>>();
        counts.sort_unstable_by_key(|entry| entry.as_ref().map(|(ordinal, _)| *ordinal));
        counts
            .into_iter()
            .flatten()
            .map(|(_, count)| count)
            .map(|count| crate::search::FileCount {
                path: count.path,
                matched_lines: count.matched_lines,
            })
            .collect::<Vec<_>>()
    } else {
        candidate_docs
            .iter()
            .filter_map(|doc| count_doc(*doc, resolve_path, &matcher))
            .map(|count| crate::search::FileCount {
                path: count.path,
                matched_lines: count.matched_lines,
            })
            .collect::<Vec<_>>()
    };
    let searches_with_match = file_counts.len();
    let matched_lines = file_counts.iter().map(|count| count.matched_lines).sum();

    Ok(SearchResults {
        candidate_docs: candidate_doc_count,
        searches_with_match,
        bytes_searched,
        matched_lines,
        matched_occurrences: 0,
        file_counts,
        file_match_counts: Vec::new(),
        hits: Vec::new(),
    })
}

fn should_parallel_scan(doc_count: usize, total_bytes: u64) -> bool {
    if std::thread::available_parallelism().map_or(1, std::num::NonZero::get) <= 1 {
        return false;
    }

    doc_count >= PARALLEL_DOC_THRESHOLD || total_bytes >= PARALLEL_BYTE_THRESHOLD
}

type FastMatcher = AhoCorasick;

fn scan_doc_id<F>(doc: DocMetaRef<'_>, resolve_path: &F, matcher: &FastMatcher) -> Option<u32>
where
    F: Fn(u32) -> Result<PathBuf> + Sync,
{
    let path = resolve_path(doc.doc_id()).ok()?;
    let bytes = map_doc_bytes(&path, doc.size())?;
    matcher.find(&bytes).map(|_| doc.doc_id())
}

fn scan_doc<F>(
    doc: DocMetaRef<'_>,
    resolve_path: &F,
    matcher: &AhoCorasick,
    mode: SearchMode,
) -> Option<Outcome>
where
    F: Fn(u32) -> Result<PathBuf> + Sync,
{
    let path = resolve_path(doc.doc_id()).ok()?;
    let bytes = map_doc_bytes(&path, doc.size())?;
    let outcome = match mode {
        SearchMode::CountOnly => unreachable!("count-only uses search_count_only"),
        SearchMode::CountMatches => unreachable!("count-matches uses regex verification"),
        SearchMode::FirstHitOnly => collect_matches(&path.to_string_lossy(), &bytes, matcher, true),
        SearchMode::MaterializeMatches => {
            collect_matches(&path.to_string_lossy(), &bytes, matcher, false)
        }
    };
    if outcome.matched_lines == 0 {
        None
    } else {
        Some(outcome)
    }
}

fn count_doc<F>(
    doc: DocMetaRef<'_>,
    resolve_path: &F,
    matcher: &FastMatcher,
) -> Option<CountOutcome>
where
    F: Fn(u32) -> Result<PathBuf> + Sync,
{
    let Ok(path) = resolve_path(doc.doc_id()) else {
        return None;
    };
    let Some(bytes) = map_doc_bytes(&path, doc.size()) else {
        return None;
    };
    let matched_lines = count_matched_lines(&bytes, matcher);
    (matched_lines > 0).then(|| CountOutcome {
        path: path.to_string_lossy().into_owned(),
        matched_lines,
    })
}

fn map_doc_bytes(path: &Path, size: u64) -> Option<Mmap> {
    if size == 0 {
        return None;
    }
    let file = File::open(path).ok()?;
    unsafe { Mmap::map(&file).ok() }
}

fn count_matched_lines(bytes: &[u8], matcher: &FastMatcher) -> usize {
    let mut matched_lines = 0usize;
    let mut line_start = 0usize;

    while line_start < bytes.len() {
        let line_end =
            memchr(b'\n', &bytes[line_start..]).map_or(bytes.len(), |idx| line_start + idx + 1);
        if matcher
            .find(trim_line_terminator_bytes(&bytes[line_start..line_end]))
            .is_some()
        {
            matched_lines += 1;
        }
        line_start = line_end;
    }

    matched_lines
}

fn collect_matches(path: &str, bytes: &[u8], matcher: &AhoCorasick, first_only: bool) -> Outcome {
    let line_starts = build_line_starts(bytes);
    let mut seen_lines = HashSet::new();
    let mut matches = Vec::new();

    for matched in matcher.find_iter(bytes) {
        record_match_lines(
            &mut seen_lines,
            &line_starts,
            matched.start(),
            matched.end(),
        );
        matches.push(build_file_match(
            bytes,
            matched.start(),
            matched.end(),
            &line_starts,
        ));
        if first_only {
            break;
        }
    }

    Outcome {
        path: path.to_string(),
        matched_lines: seen_lines.len(),
        matches,
    }
}

fn record_match_lines(
    seen_lines: &mut HashSet<usize>,
    line_starts: &[usize],
    start: usize,
    end: usize,
) {
    let start_line = line_index_for_offset(start, line_starts);
    let end_offset = if start == end {
        start
    } else {
        end.saturating_sub(1)
    };
    let end_line = line_index_for_offset(end_offset, line_starts);
    for line in start_line..=end_line {
        seen_lines.insert(line);
    }
}

fn build_file_match(bytes: &[u8], start: usize, end: usize, line_starts: &[usize]) -> FileMatch {
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

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::{
        config::{CorpusMode, TokenizerMode},
        index::format::{write_docs_file, DocsData, IndexBuildSettings, IndexMetadata},
    };

    fn docs_data(root: &std::path::Path, paths: &[&std::path::Path]) -> DocsData {
        let docs_path = root.join("docs.bin");
        let docs = paths
            .iter()
            .enumerate()
            .map(|(doc_id, path)| crate::index::format::DocMeta {
                doc_id: u32::try_from(doc_id).expect("test doc id should fit"),
                path: path.to_string_lossy().into_owned(),
                size: fs::metadata(path).expect("test should succeed").len(),
                mtime_nanos: 0,
            })
            .collect::<Vec<_>>();
        write_docs_file(
            &docs_path,
            IndexMetadata {
                tokenizer: TokenizerMode::Trigram,
                min_sparse_len: 3,
                max_sparse_len: 32,
                fallback_trigram: None,
                build: Some(IndexBuildSettings {
                    repo_root: root.to_string_lossy().into_owned(),
                    corpus_mode: CorpusMode::RespectIgnore,
                    include_hidden: false,
                    max_file_size: 1024 * 1024,
                    head_commit: None,
                    config_fingerprint: None,
                }),
            },
            &docs,
        )
        .expect("test should succeed");
        DocsData::open(&docs_path).expect("test should succeed")
    }

    #[test]
    fn count_only_reports_candidate_docs_separately_from_matches() {
        let temp = tempdir().expect("test should succeed");
        let matched = temp.path().join("matched.txt");
        let missed = temp.path().join("missed.txt");
        fs::write(&matched, "ERR_SYS\n").expect("test should succeed");
        fs::write(&missed, "UNRELATED\n").expect("test should succeed");

        let docs = docs_data(temp.path(), &[matched.as_path(), missed.as_path()]);
        let paths = vec![matched, missed];
        let resolve_path =
            |doc_id| Ok(paths[usize::try_from(doc_id).expect("doc id should fit usize")].clone());
        let prepared = PreparedMultiLiteral {
            literals: vec!["ERR_SYS".into()],
            case_insensitive: false,
        };

        let results = search(
            &docs,
            Some(&[0, 1]),
            &prepared,
            SearchMode::CountOnly,
            None,
            &resolve_path,
        )
        .expect("test should succeed");

        assert_eq!(results.candidate_docs, 2);
        assert_eq!(results.searches_with_match, 1);
        assert_eq!(results.matched_lines, 1);
        assert_eq!(results.file_counts.len(), 1);
        assert_eq!(results.file_counts[0].matched_lines, 1);
        assert!(results.bytes_searched > 0);
    }

    #[test]
    fn materialized_search_reports_candidate_docs_separately_from_matches() {
        let temp = tempdir().expect("test should succeed");
        let matched = temp.path().join("matched.txt");
        let missed = temp.path().join("missed.txt");
        fs::write(&matched, "ERR_SYS\n").expect("test should succeed");
        fs::write(&missed, "UNRELATED\n").expect("test should succeed");

        let docs = docs_data(temp.path(), &[matched.as_path(), missed.as_path()]);
        let paths = vec![matched, missed];
        let resolve_path =
            |doc_id| Ok(paths[usize::try_from(doc_id).expect("doc id should fit usize")].clone());
        let prepared = PreparedMultiLiteral {
            literals: vec!["ERR_SYS".into()],
            case_insensitive: false,
        };

        let results = search(
            &docs,
            Some(&[0, 1]),
            &prepared,
            SearchMode::MaterializeMatches,
            None,
            &resolve_path,
        )
        .expect("test should succeed");

        assert_eq!(results.candidate_docs, 2);
        assert_eq!(results.searches_with_match, 1);
        assert_eq!(results.hits.len(), 1);
    }

    #[test]
    fn materialized_search_honors_global_result_limit() {
        let temp = tempdir().expect("test should succeed");
        let first = temp.path().join("a.txt");
        let second = temp.path().join("b.txt");
        let third = temp.path().join("c.txt");
        fs::write(&first, "ERR_ONE\n").expect("test should succeed");
        fs::write(&second, "ERR_TWO\n").expect("test should succeed");
        fs::write(&third, "ERR_THREE\n").expect("test should succeed");

        let docs = docs_data(
            temp.path(),
            &[first.as_path(), second.as_path(), third.as_path()],
        );
        let paths = vec![first, second, third];
        let resolve_path =
            |doc_id| Ok(paths[usize::try_from(doc_id).expect("doc id should fit usize")].clone());
        let prepared = PreparedMultiLiteral {
            literals: vec!["ERR_ONE".into(), "ERR_TWO".into(), "ERR_THREE".into()],
            case_insensitive: false,
        };

        let results = search(
            &docs,
            Some(&[0, 1, 2]),
            &prepared,
            SearchMode::MaterializeMatches,
            Some(2),
            &resolve_path,
        )
        .expect("test should succeed");

        assert_eq!(results.candidate_docs, 3);
        assert_eq!(results.matched_lines, 2);
        assert_eq!(results.searches_with_match, 2);
        assert_eq!(results.hits.len(), 2);
    }
}
