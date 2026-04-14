use std::{
    collections::HashSet,
    io::{self, Result as IoResult},
    path::{Path, PathBuf},
};

use grep_matcher::{LineMatchKind, Match, Matcher, NoError};
use grep_regex::{RegexCaptures, RegexMatcher};
use grep_searcher::{Searcher, SearcherBuilder, Sink, SinkMatch};
use rayon::prelude::*;

use crate::{config::QueryConfig, error::Result};

use super::{
    query::LinePrefilter,
    shared::{
        build_line_starts, build_regex_matcher, configure_verifier_searcher, line_index_for_offset,
        validate_doc_id, validate_doc_ids,
    },
    verify::VerifyCandidateSource,
};

const MAX_LOOK_AHEAD: usize = 128;
const VERIFY_PARALLEL_THRESHOLD: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CountKind {
    Lines,
    Occurrences,
}

#[derive(Debug)]
pub(crate) struct VerifyPlan {
    matcher: RegexMatcher,
    line_prefilter: Option<LinePrefilter>,
    count_kind: CountKind,
    multiline_verifier: bool,
    max_count: Option<usize>,
}

impl VerifyPlan {
    pub(crate) fn compile(
        config: &QueryConfig,
        multiline_verifier: bool,
        line_prefilter: Option<LinePrefilter>,
        count_kind: CountKind,
    ) -> Result<Self> {
        Ok(Self {
            matcher: build_regex_matcher(config, multiline_verifier)?,
            line_prefilter: (!multiline_verifier).then_some(line_prefilter).flatten(),
            count_kind,
            multiline_verifier,
            max_count: config.max_count,
        })
    }

    pub(crate) fn verify_candidate_count_stats<F>(
        &self,
        candidate_ids: &[u32],
        doc_count: usize,
        resolve_path: &F,
    ) -> Result<CountStats>
    where
        F: Fn(u32) -> Result<PathBuf> + Sync,
    {
        self.verify_candidate_count_stats_with_sources(candidate_ids, doc_count, &|doc_id| {
            resolve_path(doc_id).map(VerifyCandidateSource::Path)
        })
    }

    pub(crate) fn verify_candidate_count_stats_with_sources<F>(
        &self,
        candidate_ids: &[u32],
        doc_count: usize,
        resolve_source: &F,
    ) -> Result<CountStats>
    where
        F: Fn(u32) -> Result<VerifyCandidateSource> + Sync,
    {
        validate_doc_ids(doc_count, candidate_ids)?;
        if should_parallel_verify(candidate_ids.len()) {
            candidate_ids
                .par_iter()
                .map_init(
                    || self.create_runtime(),
                    |runtime, &doc_id| {
                        validate_doc_id(doc_count, doc_id)?;
                        let source = resolve_source(doc_id)?;
                        let doc_stats = runtime.verify_source(&source)?;
                        Ok(CountStats {
                            matched_docs: usize::from(doc_stats.has_match()),
                            matched_lines: doc_stats.matched_lines,
                            matched_occurrences: doc_stats.matched_occurrences,
                        })
                    },
                )
                .try_reduce(CountStats::default, |left, right| Ok(left + right))
        } else {
            let mut runtime = self.create_runtime();
            runtime.verify_chunk(candidate_ids, doc_count, resolve_source)
        }
    }

    pub(crate) fn verify_candidate_counts_by_doc<F>(
        &self,
        candidate_ids: &[u32],
        doc_count: usize,
        resolve_path: &F,
    ) -> Result<Vec<DocMatchCount>>
    where
        F: Fn(u32) -> Result<PathBuf> + Sync,
    {
        validate_doc_ids(doc_count, candidate_ids)?;
        if should_parallel_verify(candidate_ids.len()) {
            let results = candidate_ids
                .par_iter()
                .enumerate()
                .map_init(
                    || self.create_runtime(),
                    |runtime, (ordinal, &doc_id)| {
                        validate_doc_id(doc_count, doc_id)?;
                        let path = resolve_path(doc_id)?;
                        let stats = runtime.verify_path(&path)?;
                        Ok(stats.has_match().then_some(OrdinalDocMatchCount {
                            ordinal,
                            count: DocMatchCount {
                                doc_id,
                                matched_lines: stats.matched_lines,
                                matched_occurrences: stats.matched_occurrences,
                            },
                        }))
                    },
                )
                .collect::<Vec<Result<Option<OrdinalDocMatchCount>>>>();

            let mut counts = Vec::new();
            for result in results {
                if let Some(count) = result? {
                    counts.push(count);
                }
            }
            counts.sort_unstable_by_key(|count| count.ordinal);
            Ok(counts.into_iter().map(|count| count.count).collect())
        } else {
            let mut runtime = self.create_runtime();
            let mut counts = Vec::new();
            for &doc_id in candidate_ids {
                validate_doc_id(doc_count, doc_id)?;
                let path = resolve_path(doc_id)?;
                let stats = runtime.verify_path(&path)?;
                if stats.has_match() {
                    counts.push(DocMatchCount {
                        doc_id,
                        matched_lines: stats.matched_lines,
                        matched_occurrences: stats.matched_occurrences,
                    });
                }
            }
            Ok(counts)
        }
    }

    pub(crate) fn verify_candidate_counts_by_doc_with_sources<F>(
        &self,
        candidate_ids: &[u32],
        doc_count: usize,
        resolve_source: &F,
    ) -> Result<Vec<DocMatchCount>>
    where
        F: Fn(u32) -> Result<VerifyCandidateSource> + Sync,
    {
        validate_doc_ids(doc_count, candidate_ids)?;
        if should_parallel_verify(candidate_ids.len()) {
            let results = candidate_ids
                .par_iter()
                .enumerate()
                .map_init(
                    || self.create_runtime(),
                    |runtime, (ordinal, &doc_id)| {
                        validate_doc_id(doc_count, doc_id)?;
                        let source = resolve_source(doc_id)?;
                        let stats = runtime.verify_source(&source)?;
                        Ok(stats.has_match().then_some(OrdinalDocMatchCount {
                            ordinal,
                            count: DocMatchCount {
                                doc_id,
                                matched_lines: stats.matched_lines,
                                matched_occurrences: stats.matched_occurrences,
                            },
                        }))
                    },
                )
                .collect::<Vec<Result<Option<OrdinalDocMatchCount>>>>();

            let mut counts = Vec::new();
            for result in results {
                if let Some(count) = result? {
                    counts.push(count);
                }
            }
            counts.sort_unstable_by_key(|count| count.ordinal);
            Ok(counts.into_iter().map(|count| count.count).collect())
        } else {
            let mut runtime = self.create_runtime();
            let mut counts = Vec::new();
            for &doc_id in candidate_ids {
                validate_doc_id(doc_count, doc_id)?;
                let source = resolve_source(doc_id)?;
                let stats = runtime.verify_source(&source)?;
                if stats.has_match() {
                    counts.push(DocMatchCount {
                        doc_id,
                        matched_lines: stats.matched_lines,
                        matched_occurrences: stats.matched_occurrences,
                    });
                }
            }
            Ok(counts)
        }
    }

    fn create_runtime(&self) -> VerifyRuntime<'_> {
        VerifyRuntime {
            plan: self,
            searcher: build_count_searcher(self.multiline_verifier, self.max_count),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct CountStats {
    pub(crate) matched_docs: usize,
    pub(crate) matched_lines: usize,
    pub(crate) matched_occurrences: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DocMatchCount {
    pub(crate) doc_id: u32,
    pub(crate) matched_lines: usize,
    pub(crate) matched_occurrences: usize,
}

#[derive(Debug, Clone)]
struct OrdinalDocMatchCount {
    ordinal: usize,
    count: DocMatchCount,
}

#[derive(Debug, Default, Clone, Copy)]
struct DocCountStats {
    matched_lines: usize,
    matched_occurrences: usize,
}

impl DocCountStats {
    fn has_match(self) -> bool {
        self.matched_lines > 0 || self.matched_occurrences > 0
    }
}

impl std::ops::Add for CountStats {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            matched_docs: self.matched_docs + rhs.matched_docs,
            matched_lines: self.matched_lines + rhs.matched_lines,
            matched_occurrences: self.matched_occurrences + rhs.matched_occurrences,
        }
    }
}

struct VerifyRuntime<'a> {
    plan: &'a VerifyPlan,
    searcher: Searcher,
}

impl VerifyRuntime<'_> {
    fn verify_chunk<F>(
        &mut self,
        candidate_ids: &[u32],
        doc_count: usize,
        resolve_source: &F,
    ) -> Result<CountStats>
    where
        F: Fn(u32) -> Result<VerifyCandidateSource> + Sync,
    {
        let mut stats = CountStats::default();
        for &doc_id in candidate_ids {
            validate_doc_id(doc_count, doc_id)?;
            let source = resolve_source(doc_id)?;
            let doc_stats = self.verify_source(&source)?;
            if doc_stats.has_match() {
                stats.matched_docs += 1;
                stats.matched_lines += doc_stats.matched_lines;
                stats.matched_occurrences += doc_stats.matched_occurrences;
            }
        }
        Ok(stats)
    }

    fn verify_path(&mut self, path: &Path) -> Result<DocCountStats> {
        let mut sink = CountSink::new(&self.plan.matcher, self.plan.count_kind);
        if let Some(prefilter) = self.plan.line_prefilter.as_ref() {
            let matcher = LinePrefilterMatcher::new(&self.plan.matcher, prefilter);
            self.searcher.search_path(&matcher, path, &mut sink)?;
        } else {
            self.searcher
                .search_path(&self.plan.matcher, path, &mut sink)?;
        }
        Ok(sink.stats)
    }

    fn verify_slice(&mut self, bytes: &[u8]) -> Result<DocCountStats> {
        let mut sink = CountSink::new(&self.plan.matcher, self.plan.count_kind);
        if let Some(prefilter) = self.plan.line_prefilter.as_ref() {
            let matcher = LinePrefilterMatcher::new(&self.plan.matcher, prefilter);
            self.searcher.search_slice(&matcher, bytes, &mut sink)?;
        } else {
            self.searcher
                .search_slice(&self.plan.matcher, bytes, &mut sink)?;
        }
        Ok(sink.stats)
    }

    fn verify_source(&mut self, source: &VerifyCandidateSource) -> Result<DocCountStats> {
        match source {
            VerifyCandidateSource::Path(path) => self.verify_path(path),
            VerifyCandidateSource::Bytes(bytes) => self.verify_slice(bytes),
        }
    }
}

fn build_count_searcher(multiline_verifier: bool, max_count: Option<usize>) -> Searcher {
    let mut builder = SearcherBuilder::new();
    builder.line_number(false);
    configure_verifier_searcher(&mut builder, multiline_verifier);
    if let Some(limit) = max_count.and_then(|count| u64::try_from(count).ok()) {
        builder.max_matches(Some(limit));
    }
    builder.build()
}

fn should_parallel_verify(candidate_count: usize) -> bool {
    candidate_count >= VERIFY_PARALLEL_THRESHOLD
        && std::thread::available_parallelism().map_or(1, std::num::NonZero::get) > 1
}

struct CountSink<'m> {
    matcher: &'m RegexMatcher,
    count_kind: CountKind,
    stats: DocCountStats,
}

impl<'m> CountSink<'m> {
    fn new(matcher: &'m RegexMatcher, count_kind: CountKind) -> Self {
        Self {
            matcher,
            count_kind,
            stats: DocCountStats::default(),
        }
    }
}

impl Sink for CountSink<'_> {
    type Error = io::Error;

    fn matched(&mut self, searcher: &Searcher, mat: &SinkMatch<'_>) -> IoResult<bool> {
        match self.count_kind {
            CountKind::Lines => {
                if searcher.multi_line() {
                    self.stats.matched_lines += mat.lines().count().max(1);
                } else {
                    self.stats.matched_lines += 1;
                }
            }
            CountKind::Occurrences => {
                let buffer = mat.buffer();
                let range = mat.bytes_range_in_buffer();
                let line_starts = build_line_starts(buffer);
                let mut seen_lines = HashSet::new();
                let mut matched_occurrences = 0usize;
                find_iter_at_in_context(searcher, self.matcher, buffer, range, |matched| {
                    record_line_span(&mut seen_lines, &line_starts, matched);
                    matched_occurrences += 1;
                    true
                })?;
                self.stats.matched_lines += seen_lines.len();
                self.stats.matched_occurrences += matched_occurrences;
            }
        }
        Ok(true)
    }
}

fn trim_line_terminator<'b>(
    searcher: &Searcher,
    buf: &'b [u8],
    line: &mut grep_matcher::Match,
) -> &'b [u8] {
    let line_term = searcher.line_terminator();
    if line_term.is_suffix(&buf[*line]) {
        let mut end = line.end() - 1;
        if line_term.is_crlf() && end > 0 && buf.get(end - 1) == Some(&b'\r') {
            end -= 1;
        }
        let orig_end = line.end();
        *line = line.with_end(end);
        &buf[end..orig_end]
    } else {
        &[]
    }
}

fn find_iter_at_in_context<F>(
    searcher: &Searcher,
    matcher: &RegexMatcher,
    mut bytes: &[u8],
    range: std::ops::Range<usize>,
    mut matched: F,
) -> IoResult<()>
where
    F: FnMut(grep_matcher::Match) -> bool,
{
    if searcher.multi_line_with_matcher(matcher) {
        if bytes[range.end..].len() >= MAX_LOOK_AHEAD {
            bytes = &bytes[..range.end + MAX_LOOK_AHEAD];
        }
    } else {
        let mut line = grep_matcher::Match::new(0, range.end);
        trim_line_terminator(searcher, bytes, &mut line);
        bytes = &bytes[..line.end()];
    }
    matcher
        .find_iter_at(bytes, range.start, |matched_range| {
            if matched_range.start() >= range.end {
                return false;
            }
            matched(matched_range)
        })
        .map_err(io_error_from_matcher)
}

fn record_line_span(
    seen_lines: &mut HashSet<usize>,
    line_starts: &[usize],
    matched: grep_matcher::Match,
) {
    let start = line_index_for_offset(matched.start(), line_starts);
    let end_offset = if matched.is_empty() {
        matched.start()
    } else {
        matched.end().saturating_sub(1)
    };
    let end = line_index_for_offset(end_offset, line_starts);
    for line in start..=end {
        seen_lines.insert(line);
    }
}

fn io_error_from_matcher<E: std::fmt::Display>(error: E) -> io::Error {
    io::Error::other(error.to_string())
}

struct LinePrefilterMatcher<'a> {
    inner: &'a RegexMatcher,
    prefilter: &'a LinePrefilter,
}

impl<'a> LinePrefilterMatcher<'a> {
    fn new(inner: &'a RegexMatcher, prefilter: &'a LinePrefilter) -> Self {
        Self { inner, prefilter }
    }
}

impl Matcher for LinePrefilterMatcher<'_> {
    type Captures = RegexCaptures;
    type Error = NoError;

    fn find_at(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> std::result::Result<Option<Match>, Self::Error> {
        self.inner.find_at(haystack, at)
    }

    fn new_captures(&self) -> std::result::Result<Self::Captures, Self::Error> {
        self.inner.new_captures()
    }

    fn capture_count(&self) -> usize {
        self.inner.capture_count()
    }

    fn capture_index(&self, name: &str) -> Option<usize> {
        self.inner.capture_index(name)
    }

    fn try_find_iter<F, E>(
        &self,
        haystack: &[u8],
        matched: F,
    ) -> std::result::Result<std::result::Result<(), E>, Self::Error>
    where
        F: FnMut(Match) -> std::result::Result<bool, E>,
    {
        self.inner.try_find_iter(haystack, matched)
    }

    fn captures_at(
        &self,
        haystack: &[u8],
        at: usize,
        caps: &mut Self::Captures,
    ) -> std::result::Result<bool, Self::Error> {
        self.inner.captures_at(haystack, at, caps)
    }

    fn shortest_match_at(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> std::result::Result<Option<usize>, Self::Error> {
        self.inner.shortest_match_at(haystack, at)
    }

    fn non_matching_bytes(&self) -> Option<&grep_matcher::ByteSet> {
        self.inner.non_matching_bytes()
    }

    fn line_terminator(&self) -> Option<grep_matcher::LineTerminator> {
        self.inner.line_terminator()
    }

    fn find_candidate_line(
        &self,
        haystack: &[u8],
    ) -> std::result::Result<Option<LineMatchKind>, Self::Error> {
        Ok(self
            .prefilter
            .find_candidate_line(haystack, b'\n')
            .map(LineMatchKind::Candidate))
    }
}
