use std::{
    collections::HashSet,
    env, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use grep_matcher::Matcher;
use grep_regex::RegexMatcher;
use grep_searcher::{Searcher, SearcherBuilder, Sink, SinkContext, SinkFinish, SinkMatch};
use rayon::prelude::*;
use regex_syntax::{
    hir::{Class, Hir, HirKind},
    ParserBuilder as HirParserBuilder,
};

use crate::{
    config::QueryConfig,
    error::Result,
    search::{FileContext, FileMatch, MatchLocation, SearchLine, SearchMode},
};

use super::query::LinePrefilter;
pub(crate) use super::shared::build_regex_matcher as build_matcher;
use super::shared::{
    build_line_starts, configure_verifier_searcher, line_index_for_offset,
    trim_line_terminator_bytes, validate_doc_id, validate_doc_ids,
};

const MAX_LOOK_AHEAD: usize = 128;
const VERIFY_PARALLEL_THRESHOLD: usize = 512;

#[derive(Debug, Clone)]
pub(crate) enum VerifyCandidateSource {
    Path(PathBuf),
    Bytes(Arc<[u8]>),
}

pub(crate) fn requires_multiline_verification(config: &QueryConfig) -> Result<bool> {
    if !config.multiline && !config.dot_matches_new_line {
        return Ok(false);
    }
    let mut parser = HirParserBuilder::new();
    parser
        .case_insensitive(config.case_insensitive)
        .multi_line(config.multiline)
        .dot_matches_new_line(config.dot_matches_new_line);
    let hir = parser
        .build()
        .parse(&config.regex_pattern)
        .map_err(|error| crate::error::AppError::InvalidPattern(error.to_string()))?;
    Ok(hir_can_match_line_terminator(&hir))
}

pub(crate) fn verify_candidates<F>(
    candidate_ids: &[u32],
    doc_count: usize,
    resolve_path: &F,
    matcher: &RegexMatcher,
    line_prefilter: Option<&LinePrefilter>,
    multiline_verifier: bool,
    mode: SearchMode,
    max_count: Option<usize>,
    global_max_results: Option<usize>,
    before_context: usize,
    after_context: usize,
) -> Result<Vec<SearchTaskOutcome>>
where
    F: Fn(u32) -> Result<PathBuf> + Sync,
{
    verify_candidate_sources(
        candidate_ids,
        doc_count,
        resolve_path,
        matcher,
        line_prefilter,
        multiline_verifier,
        mode,
        max_count,
        global_max_results,
        before_context,
        after_context,
    )
}

pub(crate) fn verify_candidate_sources<F>(
    candidate_ids: &[u32],
    doc_count: usize,
    resolve_path: &F,
    matcher: &RegexMatcher,
    line_prefilter: Option<&LinePrefilter>,
    multiline_verifier: bool,
    mode: SearchMode,
    max_count: Option<usize>,
    global_max_results: Option<usize>,
    before_context: usize,
    after_context: usize,
) -> Result<Vec<SearchTaskOutcome>>
where
    F: Fn(u32) -> Result<PathBuf> + Sync,
{
    verify_candidate_sources_with_resolver(
        candidate_ids,
        doc_count,
        &|doc_id| resolve_path(doc_id).map(VerifyCandidateSource::Path),
        matcher,
        line_prefilter,
        multiline_verifier,
        mode,
        max_count,
        global_max_results,
        before_context,
        after_context,
    )
}

pub(crate) fn verify_candidate_sources_with_resolver<F>(
    candidate_ids: &[u32],
    doc_count: usize,
    resolve_source: &F,
    matcher: &RegexMatcher,
    line_prefilter: Option<&LinePrefilter>,
    multiline_verifier: bool,
    mode: SearchMode,
    max_count: Option<usize>,
    global_max_results: Option<usize>,
    before_context: usize,
    after_context: usize,
) -> Result<Vec<SearchTaskOutcome>>
where
    F: Fn(u32) -> Result<VerifyCandidateSource> + Sync,
{
    validate_doc_ids(doc_count, candidate_ids)?;
    let mut outcomes = if global_max_results.is_none()
        && should_parallel_verify(candidate_ids.len())
    {
        let results = candidate_ids
            .par_iter()
            .enumerate()
            .map_init(
                || {
                    (
                        matcher.clone(),
                        build_verifier(
                            multiline_verifier,
                            mode,
                            max_count,
                            before_context,
                            after_context,
                        ),
                    )
                },
                |(matcher, verifier), (ordinal, &doc_id)| {
                    validate_doc_id(doc_count, doc_id)?;
                    let source = resolve_source(doc_id)?;
                    let outcome = verify_source(&source, matcher, line_prefilter, verifier, mode)?;
                    Ok(SearchTaskOutcome {
                        ordinal,
                        doc_id,
                        outcome,
                    })
                },
            )
            .collect::<Vec<Result<SearchTaskOutcome>>>();

        let mut outcomes = Vec::with_capacity(results.len());
        for result in results {
            outcomes.push(result?);
        }
        outcomes
    } else {
        let mut verifier = build_verifier(
            multiline_verifier,
            mode,
            max_count,
            before_context,
            after_context,
        );
        let mut outcomes = Vec::with_capacity(candidate_ids.len());
        let mut consumed_results = 0usize;
        for (ordinal, &doc_id) in candidate_ids.iter().enumerate() {
            validate_doc_id(doc_count, doc_id)?;
            let source = resolve_source(doc_id)?;
            let outcome = verify_source(&source, matcher, line_prefilter, &mut verifier, mode)?;
            consumed_results += outcome_result_units(mode, &outcome);
            outcomes.push(SearchTaskOutcome {
                ordinal,
                doc_id,
                outcome,
            });
            if global_max_results.is_some_and(|limit| consumed_results >= limit) {
                break;
            }
        }
        return Ok(outcomes);
    };
    outcomes.sort_unstable_by_key(|task| task.ordinal);
    Ok(outcomes)
}

fn outcome_result_units(mode: SearchMode, outcome: &VerifyOutcome) -> usize {
    match mode {
        SearchMode::CountOnly => outcome.matched_lines,
        SearchMode::CountMatches => outcome.matched_occurrences,
        SearchMode::FirstHitOnly => usize::from(!outcome.matches.is_empty()),
        SearchMode::MaterializeMatches => outcome.matched_lines,
    }
}

pub(crate) struct VerifyOutcome {
    pub(crate) matched_lines: usize,
    pub(crate) matched_occurrences: usize,
    pub(crate) bytes_searched: u64,
    pub(crate) matches: Vec<FileMatch>,
    pub(crate) lines: Vec<SearchLine>,
}

pub(crate) struct SearchTaskOutcome {
    pub(crate) ordinal: usize,
    pub(crate) doc_id: u32,
    pub(crate) outcome: VerifyOutcome,
}

#[derive(Debug, Default)]
pub(super) struct SearchProfile {
    enabled: bool,
}

impl SearchProfile {
    pub(super) fn enabled() -> Self {
        Self {
            enabled: env::var_os("BITFUN_PROFILE_SEARCH").is_some(),
        }
    }

    pub(super) fn record_plan(&self, duration: Duration) {
        if self.enabled {
            eprintln!("profile plan_secs={:.6}", duration.as_secs_f64());
        }
    }

    pub(super) fn record_candidates(&self, duration: Duration) {
        if self.enabled {
            eprintln!("profile candidate_secs={:.6}", duration.as_secs_f64());
        }
    }

    pub(super) fn record_regex_compile(&self, duration: Duration) {
        if self.enabled {
            eprintln!("profile regex_compile_secs={:.6}", duration.as_secs_f64());
        }
    }

    pub(super) fn record_verify(&self, match_duration: Duration) {
        if self.enabled {
            eprintln!(
                "profile verify_read_secs={:.6} verify_match_secs={:.6}",
                0.0,
                match_duration.as_secs_f64()
            );
        }
    }

    pub(super) fn finish(
        &self,
        total_duration: Duration,
        candidate_docs: usize,
        match_count: usize,
    ) {
        if self.enabled {
            eprintln!(
                "profile total_secs={:.6} candidate_docs={} match_count={}",
                total_duration.as_secs_f64(),
                candidate_docs,
                match_count
            );
        }
    }
}

fn build_verifier(
    multiline_verifier: bool,
    mode: SearchMode,
    max_count: Option<usize>,
    before_context: usize,
    after_context: usize,
) -> Searcher {
    let mut builder = SearcherBuilder::new();
    builder.line_number(mode.materializes_matches());
    configure_verifier_searcher(&mut builder, multiline_verifier);
    if mode.materializes_matches() {
        builder
            .before_context(before_context)
            .after_context(after_context);
    }
    let limit = if matches!(mode, SearchMode::FirstHitOnly) {
        Some(1)
    } else {
        max_count.and_then(|count| u64::try_from(count).ok())
    };
    if limit.is_some() {
        builder.max_matches(limit);
    }
    builder.build()
}

fn verify_path(
    path: &Path,
    matcher: &RegexMatcher,
    line_prefilter: Option<&LinePrefilter>,
    verifier: &mut Searcher,
    mode: SearchMode,
) -> Result<VerifyOutcome> {
    let mut sink = VerifySink::new(matcher, mode);
    if let Some(prefilter) = line_prefilter {
        let wrapped = LinePrefilterMatcher::new(matcher, prefilter);
        verifier.search_path(&wrapped, path, &mut sink)?;
    } else {
        verifier.search_path(matcher, path, &mut sink)?;
    }
    Ok(sink.finish())
}

fn verify_slice(
    bytes: &[u8],
    matcher: &RegexMatcher,
    line_prefilter: Option<&LinePrefilter>,
    verifier: &mut Searcher,
    mode: SearchMode,
) -> Result<VerifyOutcome> {
    let mut sink = VerifySink::new(matcher, mode);
    if let Some(prefilter) = line_prefilter {
        let wrapped = LinePrefilterMatcher::new(matcher, prefilter);
        verifier.search_slice(&wrapped, bytes, &mut sink)?;
    } else {
        verifier.search_slice(matcher, bytes, &mut sink)?;
    }
    Ok(sink.finish())
}

fn verify_source(
    source: &VerifyCandidateSource,
    matcher: &RegexMatcher,
    line_prefilter: Option<&LinePrefilter>,
    verifier: &mut Searcher,
    mode: SearchMode,
) -> Result<VerifyOutcome> {
    match source {
        VerifyCandidateSource::Path(path) => {
            verify_path(path, matcher, line_prefilter, verifier, mode)
        }
        VerifyCandidateSource::Bytes(bytes) => {
            verify_slice(bytes, matcher, line_prefilter, verifier, mode)
        }
    }
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
    type Captures = grep_regex::RegexCaptures;
    type Error = grep_matcher::NoError;

    fn find_at(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> std::result::Result<Option<grep_matcher::Match>, Self::Error> {
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
        F: FnMut(grep_matcher::Match) -> std::result::Result<bool, E>,
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
    ) -> std::result::Result<Option<grep_matcher::LineMatchKind>, Self::Error> {
        Ok(self
            .prefilter
            .find_candidate_line(haystack, b'\n')
            .map(grep_matcher::LineMatchKind::Candidate))
    }
}

fn should_parallel_verify(candidate_count: usize) -> bool {
    candidate_count >= VERIFY_PARALLEL_THRESHOLD
        && std::thread::available_parallelism().map_or(1, std::num::NonZero::get) > 1
}

fn hir_can_match_line_terminator(hir: &Hir) -> bool {
    match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => false,
        HirKind::Literal(literal) => literal.0.contains(&b'\n'),
        HirKind::Class(class) => class_can_match_line_terminator(class),
        HirKind::Capture(capture) => hir_can_match_line_terminator(&capture.sub),
        HirKind::Repetition(repetition) => hir_can_match_line_terminator(&repetition.sub),
        HirKind::Concat(hirs) | HirKind::Alternation(hirs) => {
            hirs.iter().any(hir_can_match_line_terminator)
        }
    }
}

fn class_can_match_line_terminator(class: &Class) -> bool {
    match class {
        Class::Unicode(class) => class
            .ranges()
            .iter()
            .any(|range| range.start() <= '\n' && '\n' <= range.end()),
        Class::Bytes(class) => class
            .ranges()
            .iter()
            .any(|range| range.start() <= b'\n' && b'\n' <= range.end()),
    }
}

struct OffsetLocation {
    line: usize,
    column: usize,
    line_start_offset: usize,
    line_end_offset: usize,
}

fn offset_to_location(offset: usize, line_starts: &[usize], text_len: usize) -> OffsetLocation {
    let line_index = line_index_for_offset(offset, line_starts);
    let line_start = line_starts[line_index];
    let line_end = line_starts.get(line_index + 1).copied().unwrap_or(text_len);

    OffsetLocation {
        line: line_index + 1,
        column: offset.saturating_sub(line_start) + 1,
        line_start_offset: line_start,
        line_end_offset: line_end,
    }
}

struct VerifySink<'m> {
    matcher: &'m RegexMatcher,
    mode: SearchMode,
    matched_lines: usize,
    matched_occurrences: usize,
    bytes_searched: u64,
    matches: Vec<FileMatch>,
    lines: Vec<SearchLine>,
}

impl<'m> VerifySink<'m> {
    fn new(matcher: &'m RegexMatcher, mode: SearchMode) -> Self {
        Self {
            matcher,
            mode,
            matched_lines: 0,
            matched_occurrences: 0,
            bytes_searched: 0,
            matches: Vec::new(),
            lines: Vec::new(),
        }
    }

    fn finish(self) -> VerifyOutcome {
        VerifyOutcome {
            matched_lines: self.matched_lines,
            matched_occurrences: self.matched_occurrences,
            bytes_searched: self.bytes_searched,
            matches: self.matches,
            lines: self.lines,
        }
    }

    fn record_match_group(&mut self, searcher: &Searcher, mat: &SinkMatch<'_>) -> io::Result<bool> {
        match self.mode {
            SearchMode::CountOnly => {
                if searcher.multi_line() {
                    self.matched_lines += mat.lines().count().max(1);
                } else {
                    self.matched_lines += 1;
                }
                Ok(true)
            }
            SearchMode::CountMatches => {
                let buffer = mat.buffer();
                let range = mat.bytes_range_in_buffer();
                let line_starts = build_line_starts(buffer);
                let mut seen_lines = HashSet::new();
                let mut matched_occurrences = 0usize;
                find_iter_at_in_context(searcher, self.matcher, buffer, range, |matched| {
                    record_line_span(&mut seen_lines, &line_starts, matched);
                    matched_occurrences += 1;
                    true
                })
                .map_err(io_error_from_matcher)?;
                if !seen_lines.is_empty() {
                    self.matched_lines += seen_lines.len();
                }
                self.matched_occurrences += matched_occurrences;
                Ok(true)
            }
            SearchMode::FirstHitOnly => {
                let buffer = mat.buffer();
                let range = mat.bytes_range_in_buffer();
                let line_starts = build_line_starts(buffer);
                let base_line = buffer_base_line_number(mat, &line_starts);
                let mut seen_lines = HashSet::new();
                let mut first_match = None;
                find_iter_at_in_context(searcher, self.matcher, buffer, range, |matched| {
                    record_line_span(&mut seen_lines, &line_starts, matched);
                    first_match = Some(matched);
                    false
                })?;
                let Some(matched) = first_match else {
                    return Ok(true);
                };
                self.matched_lines += seen_lines.len().max(1);
                self.matched_occurrences += 1;
                let file_match = build_file_match(buffer, matched, &line_starts, base_line);
                self.matches.push(file_match.clone());
                self.lines.push(SearchLine::Match(file_match));
                Ok(false)
            }
            SearchMode::MaterializeMatches => {
                let buffer = mat.buffer();
                let range = mat.bytes_range_in_buffer();
                let line_starts = build_line_starts(buffer);
                let base_line = buffer_base_line_number(mat, &line_starts);
                let mut seen_lines = HashSet::new();
                find_iter_at_in_context(searcher, self.matcher, buffer, range, |matched| {
                    record_line_span(&mut seen_lines, &line_starts, matched);
                    self.matched_occurrences += 1;
                    let file_match = build_file_match(buffer, matched, &line_starts, base_line);
                    self.matches.push(file_match.clone());
                    self.lines.push(SearchLine::Match(file_match));
                    true
                })
                .map_err(io_error_from_matcher)?;
                if !seen_lines.is_empty() {
                    self.matched_lines += seen_lines.len();
                }
                Ok(true)
            }
        }
    }

    fn record_context(&mut self, context: &SinkContext<'_>) -> io::Result<bool> {
        if !self.mode.materializes_matches() {
            return Ok(true);
        }
        let line_number = context
            .line_number()
            .and_then(|line| usize::try_from(line).ok())
            .unwrap_or(1);
        self.lines.push(SearchLine::Context(FileContext {
            line_number,
            snippet: String::from_utf8_lossy(trim_line_terminator_bytes(context.bytes()))
                .into_owned(),
        }));
        Ok(true)
    }
}

impl Sink for VerifySink<'_> {
    type Error = io::Error;

    fn matched(
        &mut self,
        searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> std::result::Result<bool, Self::Error> {
        self.record_match_group(searcher, mat)
    }

    fn context(
        &mut self,
        _searcher: &Searcher,
        context: &SinkContext<'_>,
    ) -> std::result::Result<bool, Self::Error> {
        self.record_context(context)
    }

    fn context_break(&mut self, _searcher: &Searcher) -> std::result::Result<bool, Self::Error> {
        if self.mode.materializes_matches() {
            self.lines.push(SearchLine::ContextBreak);
        }
        Ok(true)
    }

    fn finish(
        &mut self,
        _searcher: &Searcher,
        finish: &SinkFinish,
    ) -> std::result::Result<(), Self::Error> {
        self.bytes_searched = finish.byte_count();
        Ok(())
    }
}

fn buffer_base_line_number(mat: &SinkMatch<'_>, line_starts: &[usize]) -> usize {
    let range = mat.bytes_range_in_buffer();
    let range_start_line = line_index_for_offset(range.start, line_starts);
    mat.line_number().map_or(1, |line| {
        usize::try_from(line)
            .unwrap_or(usize::MAX)
            .saturating_sub(range_start_line)
    })
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
) -> io::Result<()>
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

fn build_file_match(
    bytes: &[u8],
    matched: grep_matcher::Match,
    line_starts: &[usize],
    base_line: usize,
) -> FileMatch {
    let location = offset_to_location(matched.start(), line_starts, bytes.len());
    let line =
        trim_line_terminator_bytes(&bytes[location.line_start_offset..location.line_end_offset]);

    FileMatch {
        location: MatchLocation {
            line: base_line + location.line - 1,
            column: location.column,
        },
        snippet: String::from_utf8_lossy(line).into_owned(),
        matched_text: String::from_utf8_lossy(&bytes[matched]).into_owned(),
    }
}

fn io_error_from_matcher<E: std::fmt::Display>(error: E) -> io::Error {
    io::Error::other(error.to_string())
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
