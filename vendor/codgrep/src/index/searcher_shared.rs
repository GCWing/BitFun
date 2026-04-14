use grep_regex::{RegexMatcher, RegexMatcherBuilder};
#[cfg(not(target_os = "macos"))]
use grep_searcher::MmapChoice;
use grep_searcher::{BinaryDetection, SearcherBuilder};

use crate::{
    config::QueryConfig,
    error::{AppError, Result},
    index::format::{DocMetaRef, DocsData},
};

pub(crate) fn build_regex_matcher(
    config: &QueryConfig,
    multiline_verifier: bool,
) -> Result<RegexMatcher> {
    let patterns = if config.patterns.is_empty() {
        vec![config.regex_pattern.as_str()]
    } else {
        config.patterns.iter().map(String::as_str).collect()
    };
    let mut builder = RegexMatcherBuilder::new();
    builder
        .multi_line(true)
        .unicode(true)
        .octal(false)
        .case_insensitive(config.case_insensitive)
        .fixed_strings(config.fixed_strings)
        .word(config.word_regexp && !config.line_regexp)
        .whole_line(config.line_regexp)
        .dot_matches_new_line(config.dot_matches_new_line);
    if multiline_verifier {
        builder.line_terminator(None);
    } else {
        builder.line_terminator(Some(b'\n'));
    }
    builder.ban_byte(Some(b'\0'));
    builder
        .build_many(&patterns)
        .map_err(|error| AppError::InvalidPattern(error.to_string()))
}

pub(crate) fn configure_verifier_searcher(builder: &mut SearcherBuilder, multiline_verifier: bool) {
    builder
        .binary_detection(BinaryDetection::quit(b'\0'))
        .multi_line(multiline_verifier);
    #[cfg(not(target_os = "macos"))]
    {
        // This project already uses file-backed mmaps in other hot paths. Using
        // the searcher's mmap path lets candidate verification behave more like
        // ripgrep on stable files.
        builder.memory_map(unsafe { MmapChoice::auto() });
    }
}

pub(super) fn build_line_starts(bytes: &[u8]) -> Vec<usize> {
    let mut starts = vec![0];
    for (idx, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            starts.push(idx + 1);
        }
    }
    starts
}

pub(super) fn line_index_for_offset(offset: usize, line_starts: &[usize]) -> usize {
    match line_starts.binary_search(&offset) {
        Ok(index) => index,
        Err(index) => index.saturating_sub(1),
    }
}

pub(super) fn trim_line_terminator_bytes(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

pub(super) fn doc_by_id(docs: &DocsData, doc_id: u32) -> Result<DocMetaRef<'_>> {
    docs.get(doc_id)
}

pub(super) fn validate_doc_ids(doc_count: usize, doc_ids: &[u32]) -> Result<()> {
    for &doc_id in doc_ids {
        validate_doc_id(doc_count, doc_id)?;
    }
    Ok(())
}

pub(super) fn validate_doc_id(doc_count: usize, doc_id: u32) -> Result<()> {
    let index = usize::try_from(doc_id)
        .map_err(|_| AppError::ValueOutOfRange(format!("doc id {doc_id} exceeds usize range")))?;
    if index >= doc_count {
        return Err(AppError::InvalidIndex(format!(
            "doc id {doc_id} is out of bounds for {doc_count} docs"
        )));
    }
    Ok(())
}

pub(super) fn resolve_candidate_docs<'a>(
    docs: &'a DocsData,
    doc_ids: &[u32],
) -> Result<Vec<DocMetaRef<'a>>> {
    doc_ids
        .iter()
        .map(|&doc_id| doc_by_id(docs, doc_id))
        .collect()
}
