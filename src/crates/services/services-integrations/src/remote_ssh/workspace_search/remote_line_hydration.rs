//! Batched remote hydration of daemon line matches.
//!
//! The flashgrep daemon reports match *positions* only, so content output has to
//! read the matched lines from wherever the files live. Locally that is one
//! `File::open` per file (see `workspace_search::line_hydration`); over SSH the
//! same shape would be one round trip per file, which at a 250-match head limit
//! is 250 sequential round trips — unusable on any real link.
//!
//! So the remote path ships a *manifest* instead: the files and the exact line
//! numbers wanted in each, read by one `awk` pass on the far side. That is a
//! constant handful of round trips regardless of how many files matched, and the
//! text comes back already clamped so a pathological line cannot flood the link.
//!
//! Results are built through the same `push_line_results` the local path uses,
//! so an indexed match renders identically no matter which side read it.

use crate::workspace_search::line_hydration::{
    plan_wanted_lines, push_line_results, MAX_HYDRATED_LINE_COLUMNS,
};
use openbitfun_services_core::filesystem::{ContentMatchPreviewBuilder, FileSearchResult};

/// Ceiling on lines hydrated in one remote content search, applied on top of any
/// caller limit.
///
/// An unbounded `max_results` is a local-only luxury: there the cost of an extra
/// match is a disk read, here it is bytes on a link the user is waiting on. Lines
/// past this are dropped and reported as truncation, never silently.
pub(crate) const MAX_REMOTE_HYDRATED_LINES: usize = 1_000;

/// Byte ceiling applied to each line on the remote side, before it is sent.
///
/// Sized so it can never change what the user sees: the shared renderer clamps to
/// [`MAX_HYDRATED_LINE_COLUMNS`] *characters*, which is at most 4× that in UTF-8
/// bytes, so anything this cuts was already past the display cut.
const MAX_REMOTE_LINE_BYTES: usize = MAX_HYDRATED_LINE_COLUMNS * 4 + 96;

// Enforced at compile time rather than in a test: if the byte clamp could ever
// cut inside the first MAX_HYDRATED_LINE_COLUMNS characters, remote and local
// renderings would diverge on the part the user actually reads.
const _: () = assert!(MAX_REMOTE_LINE_BYTES > MAX_HYDRATED_LINE_COLUMNS * 4);

/// Manifest budget for a single command.
///
/// An SSH `exec` request carries its command in one protocol packet, and
/// implementations cap that in the tens of kilobytes; a repo-wide match list can
/// exceed it. Splitting keeps every command comfortably inside the limit while
/// staying at a handful of round trips in the worst case, and exactly one in the
/// common one.
const MAX_MANIFEST_COMMAND_BYTES: usize = 8 * 1024;

const MANIFEST_HEREDOC_DELIMITER: &str = "__OPENBITFUN_LINE_MANIFEST_EOF__";

/// Reads the matched lines for one file, positionally aligned with its wanted
/// line numbers.
#[derive(Debug, Default, Clone)]
pub(crate) struct RemoteFileLines {
    pub texts: Vec<Option<String>>,
    /// The file could not be opened at all — removed or made unreadable since the
    /// snapshot the daemon answered from. Distinct from a line past end of file,
    /// which is a readable file that simply got shorter.
    pub unreadable: bool,
}

#[derive(Debug, Default)]
pub(crate) struct RemoteHydratedLines {
    pub results: Vec<FileSearchResult>,
    pub dropped_lines: usize,
    pub unreadable_files: usize,
}

/// The files and line numbers a remote content search will actually read, after
/// both the caller's limit and [`MAX_REMOTE_HYDRATED_LINES`] have been applied.
pub(crate) fn plan_remote_hydration(
    files: &[(String, Vec<usize>)],
    max_results: Option<usize>,
) -> (Vec<(String, Vec<usize>)>, usize) {
    let budget = max_results
        .unwrap_or(MAX_REMOTE_HYDRATED_LINES)
        .min(MAX_REMOTE_HYDRATED_LINES);
    plan_wanted_lines(files, Some(budget))
}

/// Splits the planned files into manifest chunks, each small enough to travel as
/// one command. Returns file index ranges, not paths, so callers keep the
/// daemon's ordering.
pub(crate) fn chunk_manifest(files: &[(String, Vec<usize>)]) -> Vec<Vec<usize>> {
    let mut chunks: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut current_bytes = 0usize;

    for (index, (path, wanted)) in files.iter().enumerate() {
        if wanted.is_empty() || path.contains('\n') || path.contains('\t') {
            // Tab and newline are the manifest's own framing. A path containing
            // either cannot be expressed, and is left out of every chunk so it
            // surfaces as unreadable rather than corrupting its neighbours.
            continue;
        }
        let entry_bytes = manifest_line(index, path, wanted).len();
        if !current.is_empty() && current_bytes + entry_bytes > MAX_MANIFEST_COMMAND_BYTES {
            chunks.push(std::mem::take(&mut current));
            current_bytes = 0;
        }
        current.push(index);
        current_bytes += entry_bytes;
    }

    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn manifest_line(index: usize, path: &str, wanted: &[usize]) -> String {
    let lines = wanted
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("{index}\t{lines}\t{path}\n")
}

/// Builds the one command that reads every line the chunk asks for.
///
/// `awk` rather than a shell loop because the read has to be a single pass per
/// file and stop at the last wanted line — a `sed`/`head` composition would
/// re-open, and a shell `while read` would be a fork per line. `LC_ALL=C` keeps
/// `length`/`substr` byte-oriented so the clamp is predictable on non-UTF-8 input.
pub(crate) fn build_read_command(files: &[(String, Vec<usize>)], chunk: &[usize]) -> String {
    let mut manifest = String::new();
    for index in chunk {
        let (path, wanted) = &files[*index];
        manifest.push_str(&manifest_line(*index, path, wanted));
    }

    format!(
        "LC_ALL=C awk -v MAXB={max_bytes} '{program}' <<'{delimiter}'\n{manifest}{delimiter}\n",
        max_bytes = MAX_REMOTE_LINE_BYTES,
        program = AWK_READ_PROGRAM,
        delimiter = MANIFEST_HEREDOC_DELIMITER,
    )
}

/// Reads `idx<TAB>lines<TAB>path` records and emits `idx<TAB>L<TAB>line<TAB>text`
/// per hydrated line, or a single `idx<TAB>E<TAB>0<TAB>` when the file could not
/// be opened. The path is last in the record so it is the only field that may
/// contain a tab, and the text is last in the output for the same reason.
///
/// The inner `want[k] < cur` skip is a guard, not a hot path: for the
/// sorted-unique 1-based lines `plan_wanted_lines` produces it never advances.
/// It is what keeps a line number the walker could never reach — a 0, say — from
/// stalling the cursor against a `want` entry that will never equal `cur`.
const AWK_READ_PROGRAM: &str = concat!(
    "{",
    "t1 = index($0, \"\\t\"); if (t1 < 2) next;",
    "rest = substr($0, t1 + 1);",
    "t2 = index(rest, \"\\t\"); if (t2 < 2) next;",
    "idx = substr($0, 1, t1 - 1);",
    "n = split(substr(rest, 1, t2 - 1), want, \",\"); if (n < 1) next;",
    "path = substr(rest, t2 + 1);",
    "cur = 0; k = 1; rc = 1;",
    "while (k <= n) {",
    "rc = (getline line < path); if (rc <= 0) break;",
    "cur++;",
    "while (k <= n && want[k] + 0 < cur) k++;",
    "if (k <= n && want[k] + 0 == cur) {",
    "if (length(line) > MAXB) line = substr(line, 1, MAXB);",
    "print idx \"\\tL\\t\" cur \"\\t\" line; k++;",
    "}",
    "}",
    "close(path);",
    "if (rc < 0) print idx \"\\tE\\t0\\t\";",
    "}",
);

/// Folds one command's stdout back onto the planned files.
///
/// `into` is indexed by the planned file index, so a chunk only ever fills its
/// own slots and a failed chunk leaves the rest untouched.
pub(crate) fn absorb_read_output(
    stdout: &str,
    files: &[(String, Vec<usize>)],
    into: &mut [RemoteFileLines],
) {
    for record in stdout.split('\n') {
        if record.is_empty() {
            continue;
        }
        let Some((index, kind, line_number, text)) = split_record(record) else {
            continue;
        };
        let Some(slot) = into.get_mut(index) else {
            continue;
        };
        if kind == "E" {
            slot.unreadable = true;
            continue;
        }
        let Some((_, wanted)) = files.get(index) else {
            continue;
        };
        let Ok(position) = wanted.binary_search(&line_number) else {
            // A line the manifest never asked for: ignore rather than trust it.
            continue;
        };
        if slot.texts.len() != wanted.len() {
            slot.texts = vec![None; wanted.len()];
        }
        slot.texts[position] = Some(text.trim_end_matches('\r').to_string());
    }
}

fn split_record(record: &str) -> Option<(usize, &str, usize, &str)> {
    let (index, rest) = record.split_once('\t')?;
    let (kind, rest) = rest.split_once('\t')?;
    let (line_number, text) = rest.split_once('\t')?;
    Some((
        index.parse().ok()?,
        kind,
        line_number.parse().unwrap_or(0),
        text,
    ))
}

/// Turns the per-file reads into content results, in the daemon's file order.
pub(crate) fn build_remote_results(
    files: &[(String, Vec<usize>)],
    reads: Vec<RemoteFileLines>,
    dropped_lines: usize,
    preview: Option<&ContentMatchPreviewBuilder>,
) -> RemoteHydratedLines {
    let mut outcome = RemoteHydratedLines {
        dropped_lines,
        ..RemoteHydratedLines::default()
    };

    for ((path, wanted), read) in files.iter().zip(reads) {
        if read.unreadable {
            outcome.unreadable_files += 1;
        }
        let mut texts = read.texts;
        texts.resize(wanted.len(), None);
        push_line_results(path, wanted, texts, preview, &mut outcome.results);
    }

    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(entries: &[(&str, &[usize])]) -> Vec<(String, Vec<usize>)> {
        entries
            .iter()
            .map(|(path, lines)| ((*path).to_string(), lines.to_vec()))
            .collect()
    }

    #[test]
    fn the_remote_ceiling_applies_even_without_a_caller_limit() {
        let wanted = (1..=MAX_REMOTE_HYDRATED_LINES + 25).collect::<Vec<_>>();
        let (planned, dropped) = plan_remote_hydration(&files(&[("/repo/a.rs", &wanted)]), None);

        assert_eq!(planned[0].1.len(), MAX_REMOTE_HYDRATED_LINES);
        assert_eq!(dropped, 25);
    }

    #[test]
    fn a_caller_limit_below_the_ceiling_still_wins() {
        let (planned, dropped) =
            plan_remote_hydration(&files(&[("/repo/a.rs", &[1, 2, 3, 4])]), Some(2));

        assert_eq!(planned[0].1, vec![1, 2]);
        assert_eq!(dropped, 2);
    }

    #[test]
    fn one_command_covers_a_head_limit_sized_result() {
        // 250 matches spread one per file is the shape a default `Grep` produces.
        let paths = (0..250)
            .map(|index| format!("/workspace/project/src/module/file_{index}.rs"))
            .collect::<Vec<_>>();
        let planned = paths
            .iter()
            .map(|path| (path.clone(), vec![42]))
            .collect::<Vec<_>>();

        let chunks = chunk_manifest(&planned);

        assert!(
            chunks.len() <= 4,
            "a 250-match search must stay at a handful of round trips, got {}",
            chunks.len()
        );
        let covered = chunks.iter().map(Vec::len).sum::<usize>();
        assert_eq!(covered, 250);
        for chunk in &chunks {
            assert!(build_read_command(&planned, chunk).len() < 32 * 1024);
        }
    }

    #[test]
    fn chunks_partition_the_files_in_order() {
        let planned = (0..400)
            .map(|index| {
                (
                    format!("/very/long/remote/path/segment/file_{index}.rs"),
                    vec![1, 2, 3],
                )
            })
            .collect::<Vec<_>>();

        let chunks = chunk_manifest(&planned);

        assert!(chunks.len() > 1, "the fixture is meant to need splitting");
        let flattened = chunks.concat();
        assert_eq!(flattened, (0..400).collect::<Vec<_>>());
    }

    #[test]
    fn a_path_that_cannot_be_framed_is_left_out_of_every_chunk() {
        let planned = files(&[
            ("/repo/ok.rs", &[1]),
            ("/repo/we\tird.rs", &[1]),
            ("/repo/also\nbad.rs", &[1]),
            ("/repo/fine.rs", &[2]),
        ]);

        assert_eq!(chunk_manifest(&planned).concat(), vec![0, 3]);
    }

    #[test]
    fn the_command_carries_every_wanted_line_of_its_chunk() {
        let planned = files(&[("/repo/a.rs", &[3, 9]), ("/repo/b.rs", &[1])]);

        let command = build_read_command(&planned, &[0, 1]);

        assert!(command.contains("0\t3,9\t/repo/a.rs\n"));
        assert!(command.contains("1\t1\t/repo/b.rs\n"));
        assert!(command.ends_with(&format!("{MANIFEST_HEREDOC_DELIMITER}\n")));
        // Quoted heredoc: the manifest must not be expanded by the remote shell.
        assert!(command.contains(&format!("<<'{MANIFEST_HEREDOC_DELIMITER}'")));
    }

    #[test]
    fn output_lands_in_the_slot_its_line_number_asked_for() {
        let planned = files(&[("/repo/a.rs", &[3, 9]), ("/repo/b.rs", &[1])]);
        let mut reads = vec![RemoteFileLines::default(); planned.len()];

        absorb_read_output(
            "0\tL\t9\tnine\n0\tL\t3\tthree\n1\tE\t0\t\n",
            &planned,
            &mut reads,
        );

        assert_eq!(
            reads[0].texts,
            vec![Some("three".to_string()), Some("nine".to_string())]
        );
        assert!(!reads[0].unreadable);
        assert!(reads[1].unreadable);
    }

    #[test]
    fn text_containing_tabs_survives_the_framing() {
        let planned = files(&[("/repo/a.rs", &[1])]);
        let mut reads = vec![RemoteFileLines::default(); 1];

        absorb_read_output("0\tL\t1\tlet\tx\t= 1;\r\n", &planned, &mut reads);

        assert_eq!(reads[0].texts[0].as_deref(), Some("let\tx\t= 1;"));
    }

    #[test]
    fn a_line_the_manifest_never_asked_for_is_ignored() {
        let planned = files(&[("/repo/a.rs", &[1])]);
        let mut reads = vec![RemoteFileLines::default(); 1];

        absorb_read_output(
            "0\tL\t7\tsurprise\n99\tL\t1\tout of range\n",
            &planned,
            &mut reads,
        );

        // Nothing was absorbed, so the file renders as a bare position.
        assert!(reads[0].texts.iter().all(Option::is_none));
        let outcome = build_remote_results(&planned, reads, 0, None);
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].matched_content, None);
    }

    #[test]
    fn a_chunk_that_never_answered_keeps_its_positions() {
        let planned = files(&[("/repo/a.rs", &[4, 8])]);
        let reads = vec![RemoteFileLines::default()];

        let outcome = build_remote_results(&planned, reads, 0, None);

        let rendered = outcome
            .results
            .iter()
            .map(|result| (result.line_number, result.matched_content.clone()))
            .collect::<Vec<_>>();
        assert_eq!(rendered, vec![(Some(4), None), (Some(8), None)]);
        assert_eq!(outcome.unreadable_files, 0);
    }

    #[test]
    fn long_lines_are_clamped_the_same_way_the_local_path_clamps_them() {
        let planned = files(&[("/repo/a.rs", &[1])]);
        let long = "x".repeat(MAX_HYDRATED_LINE_COLUMNS * 3);
        let reads = vec![RemoteFileLines {
            texts: vec![Some(long)],
            unreadable: false,
        }];

        let outcome = build_remote_results(&planned, reads, 0, None);

        let content = outcome.results[0]
            .matched_content
            .as_deref()
            .expect("content");
        assert!(content.ends_with(" [truncated]"));
        assert!(content.starts_with(&"x".repeat(MAX_HYDRATED_LINE_COLUMNS)));
    }
}

/// Executes the command this module generates against a real POSIX shell.
///
/// The Rust half of the protocol is covered above; this is the other half. An
/// `awk` slip or a heredoc framing mistake only ever shows up when the thing
/// actually runs, and on the remote path the only place it would show up is a
/// user's SSH session.
#[cfg(all(test, unix))]
mod shell_tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn run(files: &[(String, Vec<usize>)]) -> Vec<RemoteFileLines> {
        let mut reads = vec![RemoteFileLines::default(); files.len()];
        for chunk in chunk_manifest(files) {
            let output = Command::new("sh")
                .arg("-c")
                .arg(build_read_command(files, &chunk))
                .output()
                .expect("run the generated line-read command");
            assert!(
                output.status.success(),
                "command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            absorb_read_output(&String::from_utf8_lossy(&output.stdout), files, &mut reads);
        }
        reads
    }

    #[test]
    fn the_generated_command_reads_exactly_the_wanted_lines() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = |name: &str| dir.path().join(name).to_string_lossy().to_string();

        fs::write(
            dir.path().join("a.txt"),
            "alpha
needle one
gamma
needle two
",
        )
        .expect("write");
        fs::write(
            dir.path().join("crlf.txt"),
            "第一行 needle
第二行
",
        )
        .expect("write");
        fs::write(
            dir.path().join("tabs.txt"),
            "let	x	= 1;
only line
",
        )
        .expect("write");
        fs::write(
            dir.path().join("short.txt"),
            "short
",
        )
        .expect("write");
        fs::write(dir.path().join("empty.txt"), "").expect("write");
        fs::write(
            dir.path().join("long.txt"),
            format!("needle{}\n", "x".repeat(MAX_REMOTE_LINE_BYTES * 2)),
        )
        .expect("write");

        let files = vec![
            (path("a.txt"), vec![2, 4]),
            (path("crlf.txt"), vec![1]),
            (path("tabs.txt"), vec![1, 2]),
            // Line 9 is past end of file: the snapshot can be ahead of the worktree.
            (path("short.txt"), vec![1, 9]),
            (path("empty.txt"), vec![1]),
            (path("long.txt"), vec![1]),
            (path("gone.txt"), vec![1]),
        ];

        let reads = run(&files);

        assert_eq!(
            reads[0].texts,
            vec![
                Some("needle one".to_string()),
                Some("needle two".to_string())
            ]
        );
        // The carriage return is stripped on the way in, exactly as the local
        // reader strips it, and multi-byte text survives the byte-oriented awk.
        assert_eq!(reads[1].texts, vec![Some("第一行 needle".to_string())]);
        assert_eq!(
            reads[2].texts,
            vec![
                Some("let	x	= 1;".to_string()),
                Some("only line".to_string())
            ]
        );
        assert_eq!(reads[3].texts, vec![Some("short".to_string()), None]);
        assert!(
            !reads[3].unreadable,
            "a short file is readable, just shorter"
        );
        assert!(!reads[4].unreadable, "an empty file is readable");
        assert!(reads[4].texts.iter().all(Option::is_none));
        assert_eq!(
            reads[5].texts[0].as_ref().map(String::len),
            Some(MAX_REMOTE_LINE_BYTES),
            "the remote clamp bounds what crosses the link"
        );
        assert!(
            reads[6].unreadable,
            "a missing file must be reported as such"
        );
    }

    #[test]
    fn the_generated_command_renders_the_same_text_the_local_reader_would() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("sample.rs");
        fs::write(
            &file,
            "fn main() {
    let needle = 1;
}
",
        )
        .expect("write");
        let path = file.to_string_lossy().to_string();

        let files = vec![(path.clone(), vec![2])];
        let preview =
            ContentMatchPreviewBuilder::new("needle", true, false, false).expect("preview builder");

        let remote = build_remote_results(&files, run(&files), 0, Some(&preview));
        let local = crate::workspace_search::line_hydration::hydrate_grouped_line_matches(
            &files,
            None,
            Some(&preview),
        );

        let render = |result: &FileSearchResult| {
            (
                result.path.clone(),
                result.name.clone(),
                result.line_number,
                result.matched_content.clone(),
                result.preview_before.clone(),
                result.preview_inside.clone(),
                result.preview_after.clone(),
            )
        };
        assert_eq!(
            remote.results.iter().map(render).collect::<Vec<_>>(),
            local.results.iter().map(render).collect::<Vec<_>>()
        );
    }
}
