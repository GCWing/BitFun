//! Hydration of daemon line matches with on-disk line text.
//!
//! The flashgrep daemon reports match *positions* only — every search mode
//! returns `{path, line_number}` and never the line itself. Content output
//! therefore has to read the matched files locally; this module does that with
//! exactly one open per file by consuming the per-file grouping from
//! `search/grouped_line_matches`.
//!
//! Callers must apply their result limit *before* hydration (that is what
//! `max_results` here is for): reading files for matches that are about to be
//! truncated away is pure I/O waste on large result sets.

use openbitfun_services_core::filesystem::{
    ContentMatchPreviewBuilder, FileSearchResult, SearchMatchType,
};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Mirrors `tool_execution::search::grep_search::MAX_DISPLAY_COLUMNS` so an
/// indexed content search renders a long line the same way the ripgrep fallback
/// does.
pub(crate) const MAX_HYDRATED_LINE_COLUMNS: usize = 500;
const TRUNCATION_SUFFIX: &str = " [truncated]";

#[derive(Debug, Default)]
pub(crate) struct HydratedLineMatches {
    pub results: Vec<FileSearchResult>,
    /// Matched lines the daemon reported that `max_results` dropped before any
    /// file was opened.
    pub dropped_lines: usize,
    /// Files that could not be read, i.e. removed or made unreadable since the
    /// snapshot the daemon answered from. Their matches are still returned as
    /// path + line number, without text.
    pub unreadable_files: usize,
}

/// Applies `max_results` to the daemon's per-file grouping *before* any line is
/// read, and normalises each file's line numbers to sorted-unique order.
///
/// Split out from hydration itself because the remote path needs the same
/// budgeting decision — which files to open, and how many matches were dropped
/// on the floor — before it can build the batch it ships over SSH.
pub(crate) fn plan_wanted_lines(
    files: &[(String, Vec<usize>)],
    max_results: Option<usize>,
) -> (Vec<(String, Vec<usize>)>, usize) {
    let mut planned = Vec::with_capacity(files.len());
    let mut dropped_lines = 0usize;
    let mut remaining = max_results.unwrap_or(usize::MAX);

    for (path, line_numbers) in files {
        let mut wanted = line_numbers.clone();
        wanted.sort_unstable();
        wanted.dedup();

        if remaining == 0 {
            dropped_lines += wanted.len();
            continue;
        }
        if wanted.len() > remaining {
            dropped_lines += wanted.len() - remaining;
            wanted.truncate(remaining);
        }
        remaining -= wanted.len();
        planned.push((path.clone(), wanted));
    }

    (planned, dropped_lines)
}

/// Turns one file's wanted line numbers plus their (possibly missing) text into
/// content search results, appending them to `out`.
///
/// `texts` is positional: one entry per `wanted` line, `None` when the line
/// could not be read. Shared with the remote path so an indexed match renders
/// identically whether its text came off the local disk or off an SSH batch.
pub(crate) fn push_line_results(
    path: &str,
    wanted: &[usize],
    texts: Vec<Option<String>>,
    preview: Option<&ContentMatchPreviewBuilder>,
    out: &mut Vec<FileSearchResult>,
) {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string();

    for (line_number, text) in wanted.iter().copied().zip(texts) {
        let (preview_before, preview_inside, preview_after) = match (preview, text.as_deref()) {
            (Some(preview), Some(text)) => preview.preview(text),
            _ => (None, None, None),
        };

        out.push(FileSearchResult {
            path: path.to_string(),
            name: file_name.clone(),
            is_directory: false,
            match_type: SearchMatchType::Content,
            line_number: Some(line_number),
            matched_content: text.map(|text| clamp_display_line(&text)),
            preview_before,
            preview_inside,
            preview_after,
        });
    }
}

/// Reads the matched lines for `files` (daemon order preserved) and turns them
/// into content search results.
///
/// This performs blocking I/O; run it on a blocking thread.
pub(crate) fn hydrate_grouped_line_matches(
    files: &[(String, Vec<usize>)],
    max_results: Option<usize>,
    preview: Option<&ContentMatchPreviewBuilder>,
) -> HydratedLineMatches {
    let (planned, dropped_lines) = plan_wanted_lines(files, max_results);
    let mut outcome = HydratedLineMatches {
        dropped_lines,
        ..HydratedLineMatches::default()
    };

    for (path, wanted) in &planned {
        let texts = match read_wanted_lines(Path::new(path), wanted) {
            Some(texts) => texts,
            None => {
                outcome.unreadable_files += 1;
                vec![None; wanted.len()]
            }
        };

        push_line_results(path, wanted, texts, preview, &mut outcome.results);
    }

    outcome
}

/// Reads the requested 1-based line numbers in a single pass.
///
/// Returns `None` when the file cannot be opened at all; otherwise one entry per
/// requested line, `None` for line numbers past end of file (the snapshot can be
/// slightly ahead of the worktree).
fn read_wanted_lines(path: &Path, wanted: &[usize]) -> Option<Vec<Option<String>>> {
    let last_wanted = wanted.last().copied()?;
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut texts = vec![None; wanted.len()];
    let mut next_wanted = 0usize;

    for (index, line) in reader.split(b'\n').enumerate() {
        let line_number = index + 1;
        let Ok(line) = line else {
            // Unreadable mid-file (I/O error, not a decoding issue): keep what
            // has been hydrated so far rather than dropping the whole file.
            break;
        };

        while next_wanted < wanted.len() && wanted[next_wanted] < line_number {
            next_wanted += 1;
        }
        if next_wanted >= wanted.len() {
            break;
        }
        if wanted[next_wanted] == line_number {
            let text = String::from_utf8_lossy(&line);
            texts[next_wanted] = Some(text.trim_end_matches('\r').to_string());
            next_wanted += 1;
        }
        if line_number >= last_wanted {
            break;
        }
    }

    Some(texts)
}

pub(crate) fn clamp_display_line(line: &str) -> String {
    if line.chars().count() <= MAX_HYDRATED_LINE_COLUMNS {
        return line.to_string();
    }

    let head = line
        .chars()
        .take(MAX_HYDRATED_LINE_COLUMNS)
        .collect::<String>();
    format!("{head}{TRUNCATION_SUFFIX}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn preview_builder(pattern: &str) -> ContentMatchPreviewBuilder {
        ContentMatchPreviewBuilder::new(pattern, true, false, false).expect("preview builder")
    }

    #[test]
    fn hydrates_matched_lines_in_file_order() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("sample.txt");
        fs::write(&path, "alpha\nneedle one\ngamma\nneedle two\n").expect("write");
        let path = path.to_string_lossy().to_string();

        let outcome = hydrate_grouped_line_matches(
            &[(path.clone(), vec![4, 2])],
            None,
            Some(&preview_builder("needle")),
        );

        assert_eq!(outcome.dropped_lines, 0);
        assert_eq!(outcome.unreadable_files, 0);
        let rendered = outcome
            .results
            .iter()
            .map(|result| {
                (
                    result.line_number,
                    result.matched_content.clone(),
                    result.preview_inside.clone(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rendered,
            vec![
                (
                    Some(2),
                    Some("needle one".to_string()),
                    Some("needle".to_string())
                ),
                (
                    Some(4),
                    Some("needle two".to_string()),
                    Some("needle".to_string())
                ),
            ]
        );
    }

    #[test]
    fn strips_carriage_returns_and_keeps_multibyte_text() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("crlf.txt");
        fs::write(&path, "第一行 needle\r\n第二行\r\n").expect("write");
        let path = path.to_string_lossy().to_string();

        let outcome = hydrate_grouped_line_matches(
            &[(path, vec![1])],
            None,
            Some(&preview_builder("needle")),
        );

        let result = &outcome.results[0];
        assert_eq!(result.matched_content.as_deref(), Some("第一行 needle"));
        assert_eq!(result.preview_before.as_deref(), Some("第一行 "));
        assert_eq!(result.preview_inside.as_deref(), Some("needle"));
    }

    #[test]
    fn line_past_end_of_file_yields_no_text() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("short.txt");
        fs::write(&path, "only line\n").expect("write");
        let path = path.to_string_lossy().to_string();

        let outcome = hydrate_grouped_line_matches(&[(path, vec![1, 9])], None, None);

        assert_eq!(outcome.results.len(), 2);
        assert_eq!(
            outcome.results[0].matched_content.as_deref(),
            Some("only line")
        );
        assert_eq!(outcome.results[1].line_number, Some(9));
        assert_eq!(outcome.results[1].matched_content, None);
        assert_eq!(outcome.unreadable_files, 0);
    }

    #[test]
    fn unreadable_file_still_reports_positions() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("gone.txt").to_string_lossy().to_string();

        let outcome = hydrate_grouped_line_matches(&[(missing.clone(), vec![3])], None, None);

        assert_eq!(outcome.unreadable_files, 1);
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].path, missing);
        assert_eq!(outcome.results[0].line_number, Some(3));
        assert_eq!(outcome.results[0].matched_content, None);
    }

    #[test]
    fn max_results_is_applied_before_reading_files() {
        let dir = tempdir().expect("tempdir");
        let first = dir.path().join("first.txt");
        fs::write(&first, "needle a\nneedle b\n").expect("write");
        let missing = dir.path().join("never-opened.txt");

        let outcome = hydrate_grouped_line_matches(
            &[
                (first.to_string_lossy().to_string(), vec![1, 2]),
                (missing.to_string_lossy().to_string(), vec![1, 2, 3]),
            ],
            Some(1),
            None,
        );

        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.dropped_lines, 4);
        // The second file was never opened, so it is not counted as unreadable.
        assert_eq!(outcome.unreadable_files, 0);
    }

    #[test]
    fn very_long_lines_are_clamped_like_the_ripgrep_path() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("long.txt");
        let long_line = format!("needle{}", "x".repeat(MAX_HYDRATED_LINE_COLUMNS * 2));
        fs::write(&path, format!("{long_line}\n")).expect("write");
        let path = path.to_string_lossy().to_string();

        let outcome = hydrate_grouped_line_matches(&[(path, vec![1])], None, None);

        let content = outcome.results[0]
            .matched_content
            .as_deref()
            .expect("content");
        assert!(content.ends_with(TRUNCATION_SUFFIX));
        assert_eq!(
            content.chars().count(),
            MAX_HYDRATED_LINE_COLUMNS + TRUNCATION_SUFFIX.chars().count()
        );
    }
}
