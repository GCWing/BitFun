use crate::util::string::truncate_string_by_chars;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};

#[derive(Debug, PartialEq, Eq)]
pub struct ReadFileResult {
    pub start_line: usize,
    pub end_line: usize,
    pub total_lines: usize,
    pub content: String,
    pub hit_total_char_limit: bool,
    /// True when the returned view omits characters from selected lines.
    pub content_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadFilePresentation {
    pub result_for_assistant: String,
    pub lines_read: usize,
}

fn read_file_lines_read(result: &ReadFileResult) -> usize {
    if result.total_lines == 0 || result.end_line < result.start_line {
        0
    } else {
        result.end_line - result.start_line + 1
    }
}

pub fn build_read_file_presentation(
    logical_path: &str,
    result: &ReadFileResult,
) -> ReadFilePresentation {
    let mut result_for_assistant = format!(
        "Read lines {}-{} from {} ({} total lines)\n<file_content>\n{}\n</file_content>",
        result.start_line, result.end_line, logical_path, result.total_lines, result.content
    );

    let has_more = result.end_line < result.total_lines;
    let next_start_line = has_more.then_some(result.end_line + 1);
    if let Some(next_start) = next_start_line {
        if result.hit_total_char_limit {
            result_for_assistant.push_str(&format!(
                "\n\n[Output truncated after reaching the Read tool size limit. Use offset={} and limit to continue reading.]",
                next_start
            ));
        } else {
            result_for_assistant.push_str(&format!(
                "\n\n[Showing lines {}-{} of {} total. Use offset={} and limit to continue reading.]",
                result.start_line, result.end_line, result.total_lines, next_start
            ));
        }
    }

    ReadFilePresentation {
        result_for_assistant,
        lines_read: read_file_lines_read(result),
    }
}

/// Read a local file only when it fits within `max_bytes`.
///
/// The limit is enforced from handle metadata and again while reading so a growing file cannot
/// make the caller retain an unbounded buffer.
pub fn read_file_bytes_bounded(
    file_path: &str,
    max_bytes: usize,
) -> Result<Option<Vec<u8>>, String> {
    let file = File::open(file_path)
        .map_err(|error| format!("Failed to read file {file_path}: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("Failed to inspect file {file_path}: {error}"))?;
    if metadata.len() > max_bytes as u64 {
        return Ok(None);
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(max_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Failed to read file {file_path}: {error}"))?;
    Ok((bytes.len() <= max_bytes).then_some(bytes))
}

/// Read a file through the provider's byte stream using the same parser as
/// local files and converted documents. No seek or remote text command is needed.
pub async fn read_file_from_reader<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    source: &str,
    start_line: usize,
    limit: usize,
    max_line_chars: usize,
    max_total_chars: usize,
) -> Result<ReadFileResult, String> {
    read_async_text(
        reader,
        source,
        start_line,
        limit,
        max_line_chars,
        max_total_chars,
        false,
    )
    .await
}

pub async fn read_file_tail_from_reader<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    source: &str,
    limit: usize,
    max_line_chars: usize,
    max_total_chars: usize,
) -> Result<ReadFileResult, String> {
    read_async_text(
        reader,
        source,
        1,
        limit,
        max_line_chars,
        max_total_chars,
        true,
    )
    .await
}

async fn read_async_text<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    source: &str,
    start_line: usize,
    limit: usize,
    max_line_chars: usize,
    max_total_chars: usize,
    tail: bool,
) -> Result<ReadFileResult, String> {
    use tokio::io::AsyncBufReadExt;
    let mut selection =
        ReadLineSelection::new(start_line, limit, max_line_chars, max_total_chars, tail)?;
    let mut lines = tokio::io::BufReader::new(reader).lines();
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|error| format!("Failed to read {source}: {error}"))?
    {
        selection.push(line);
    }
    selection.finish()
}

/// start_line: starts from 1
pub fn read_file(
    file_path: &str,
    start_line: usize,
    limit: usize,
    max_line_chars: usize,
    max_total_chars: usize,
) -> Result<ReadFileResult, String> {
    let file = File::open(file_path)
        .map_err(|error| format!("Failed to read file {file_path}: {error}"))?;
    read_buffered_text(
        BufReader::new(file),
        &format!("file {file_path}"),
        start_line,
        limit,
        max_line_chars,
        max_total_chars,
        false,
    )
}

/// Page decoded text with the same line numbering and budgets as file streams.
pub fn read_text(
    text: &str,
    start_line: usize,
    limit: usize,
    max_line_chars: usize,
    max_total_chars: usize,
) -> Result<ReadFileResult, String> {
    read_buffered_text(
        BufReader::new(text.as_bytes()),
        "converted document",
        start_line,
        limit,
        max_line_chars,
        max_total_chars,
        false,
    )
}

pub fn read_file_tail(
    file_path: &str,
    limit: usize,
    max_line_chars: usize,
    max_total_chars: usize,
) -> Result<ReadFileResult, String> {
    let file = File::open(file_path)
        .map_err(|error| format!("Failed to read file {file_path}: {error}"))?;
    read_buffered_text(
        BufReader::new(file),
        &format!("file {file_path}"),
        1,
        limit,
        max_line_chars,
        max_total_chars,
        true,
    )
}

pub fn read_text_tail(
    text: &str,
    limit: usize,
    max_line_chars: usize,
    max_total_chars: usize,
) -> Result<ReadFileResult, String> {
    read_buffered_text(
        BufReader::new(text.as_bytes()),
        "converted document",
        1,
        limit,
        max_line_chars,
        max_total_chars,
        true,
    )
}

fn read_buffered_text<R: BufRead>(
    reader: R,
    source: &str,
    start_line: usize,
    limit: usize,
    max_line_chars: usize,
    max_total_chars: usize,
    tail: bool,
) -> Result<ReadFileResult, String> {
    let mut selection =
        ReadLineSelection::new(start_line, limit, max_line_chars, max_total_chars, tail)?;
    for line in reader.lines() {
        selection.push(line.map_err(|error| format!("Failed to read {source}: {error}"))?);
    }
    selection.finish()
}

/// Shared line selection, Unicode truncation, numbering and output budget.
/// Tail buffering retains truncated lines rather than the entire original lines.
struct ReadLineSelection {
    start_line: usize,
    end_line: usize,
    limit: usize,
    max_line_chars: usize,
    max_total_chars: usize,
    total_lines: usize,
    selected_lines: Vec<String>,
    selected_chars: usize,
    hit_total_char_limit: bool,
    content_truncated: bool,
    tail_lines: Option<VecDeque<(usize, String, bool)>>,
}

impl ReadLineSelection {
    fn new(
        start_line: usize,
        limit: usize,
        max_line_chars: usize,
        max_total_chars: usize,
        tail: bool,
    ) -> Result<Self, String> {
        if start_line == 0 {
            return Err("`start_line` should start from 1".to_string());
        }
        if limit == 0 {
            return Err("`limit` can't be 0".to_string());
        }
        if max_total_chars == 0 {
            return Err("`max_total_chars` can't be 0".to_string());
        }
        let end_line = start_line
            .checked_add(limit.saturating_sub(1))
            .ok_or_else(|| "Requested line range is too large".to_string())?;
        Ok(Self {
            start_line,
            end_line,
            limit,
            max_line_chars,
            max_total_chars,
            total_lines: 0,
            selected_lines: Vec::new(),
            selected_chars: 0,
            hit_total_char_limit: false,
            content_truncated: false,
            tail_lines: tail.then(VecDeque::new),
        })
    }

    fn push(&mut self, line: String) {
        self.total_lines += 1;
        if self.tail_lines.is_none()
            && (self.total_lines < self.start_line
                || self.total_lines > self.end_line
                || self.hit_total_char_limit)
        {
            return;
        }
        let truncated = line.chars().count() > self.max_line_chars;
        let line = if truncated {
            format!(
                "{} [truncated]",
                truncate_string_by_chars(&line, self.max_line_chars)
            )
        } else {
            line
        };
        if let Some(tail_lines) = &mut self.tail_lines {
            if tail_lines.len() == self.limit {
                tail_lines.pop_front();
            }
            tail_lines.push_back((self.total_lines, line, truncated));
        } else {
            self.select(self.total_lines, line, truncated);
        }
    }

    fn select(&mut self, line_number: usize, line: String, truncated: bool) {
        let rendered = format!("{line_number:>6}\t{line}");
        let next_chars = self
            .selected_chars
            .saturating_add(usize::from(!self.selected_lines.is_empty()))
            .saturating_add(rendered.chars().count());
        if next_chars > self.max_total_chars {
            self.hit_total_char_limit = true;
            self.content_truncated = true;
        } else {
            self.content_truncated |= truncated;
            self.selected_chars = next_chars;
            self.selected_lines.push(rendered);
        }
    }

    fn finish(mut self) -> Result<ReadFileResult, String> {
        if self.total_lines == 0 {
            return Ok(ReadFileResult {
                start_line: 0,
                end_line: 0,
                total_lines: 0,
                content: String::new(),
                hit_total_char_limit: false,
                content_truncated: false,
            });
        }
        if let Some(tail_lines) = self.tail_lines.take() {
            self.start_line = self
                .total_lines
                .saturating_sub(self.limit)
                .saturating_add(1);
            for (number, line, truncated) in tail_lines {
                self.select(number, line, truncated);
                if self.hit_total_char_limit {
                    break;
                }
            }
        }
        if self.start_line > self.total_lines {
            return Err(format!(
                "`start_line` {} is larger than the number of lines in the file: {}",
                self.start_line, self.total_lines
            ));
        }
        let end_line = if self.selected_lines.is_empty() {
            self.start_line
        } else {
            self.start_line
                .saturating_add(self.selected_lines.len())
                .saturating_sub(1)
        };
        Ok(ReadFileResult {
            start_line: self.start_line,
            end_line,
            total_lines: self.total_lines,
            content: self.selected_lines.join("\n"),
            hit_total_char_limit: self.hit_total_char_limit,
            content_truncated: self.content_truncated,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_read_file_presentation, read_file, read_file_bytes_bounded, read_file_lines_read,
        read_file_tail, read_text, read_text_tail, ReadFileResult,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn truncation_facts_describe_only_the_returned_view() {
        let text = "abcdef\nlast\n";
        let truncated = read_text(text, 1, 2, 4, 100).unwrap();
        assert!(truncated.content_truncated);
        assert!(!truncated.hit_total_char_limit);
        let tail = read_text_tail(text, 1, 4, 100).unwrap();
        assert!(
            !tail.content_truncated,
            "an evicted long line does not truncate the tail view"
        );
        let offset = read_text(text, 2, 1, 4, 100).unwrap();
        assert!(!offset.content_truncated);
        let budget = read_text(text, 1, 2, 50, 14).unwrap();
        assert!(budget.hit_total_char_limit);
        assert!(budget.content_truncated);
        assert!(
            !read_text("literal [truncated]", 1, 1, 100, 100)
                .unwrap()
                .content_truncated
        );
    }

    #[tokio::test]
    async fn async_streams_preserve_unicode_crlf_windows_and_tail() {
        use tokio::io::AsyncWriteExt;
        let source = "first\r\n中😀文\r\nlast";
        let (reader, mut writer) = tokio::io::duplex(1);
        let producer = tokio::spawn(async move {
            for byte in source.as_bytes() {
                writer.write_all(&[*byte]).await.unwrap();
            }
        });
        let result = super::read_file_from_reader(reader, "stream", 2, 1, 2, 100)
            .await
            .unwrap();
        producer.await.unwrap();
        assert_eq!(result.content, "     2\t中😀 [truncated]");
        assert_eq!(result.total_lines, 3);
        assert_eq!(result, read_text(source, 2, 1, 2, 100).unwrap());
        let tail = super::read_file_tail_from_reader(source.as_bytes(), "stream", 2, 50, 100)
            .await
            .unwrap();
        assert_eq!(tail.content, "     2\t中😀文\n     3\tlast");
        assert_eq!(tail, read_text_tail(source, 2, 50, 100).unwrap());
        let empty = super::read_file_from_reader(&b""[..], "stream", 1, 3, 50, 100)
            .await
            .unwrap();
        assert_eq!(empty.total_lines, 0);
    }

    #[tokio::test]
    async fn async_streams_reject_invalid_utf8_outside_the_selected_window() {
        let error =
            super::read_file_from_reader(&b"visible\n\xffhidden"[..], "stream", 1, 1, 50, 100)
                .await
                .unwrap_err();
        assert!(error.contains("Failed to read stream"), "{error}");
    }

    #[tokio::test]
    async fn async_stream_failure_after_a_complete_visible_line_is_not_success() {
        struct FailingReader(bool);
        impl tokio::io::AsyncRead for FailingReader {
            fn poll_read(
                mut self: std::pin::Pin<&mut Self>,
                _cx: &mut std::task::Context<'_>,
                buf: &mut tokio::io::ReadBuf<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                if self.0 {
                    return std::task::Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "connection lost",
                    )));
                }
                self.0 = true;
                buf.put_slice(b"visible\n");
                std::task::Poll::Ready(Ok(()))
            }
        }
        let error = super::read_file_from_reader(FailingReader(false), "stream", 1, 1, 50, 100)
            .await
            .unwrap_err();
        assert!(error.contains("connection lost"), "{error}");
    }

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn write_temp_file(contents: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let process_id = std::process::id();
        let path = std::env::temp_dir().join(format!(
            "bitfun-read-file-test-{process_id}-{timestamp}-{counter}.txt"
        ));
        fs::write(&path, contents).expect("temp file should be written");
        path
    }

    #[test]
    fn truncates_when_total_char_budget_is_hit() {
        let path = write_temp_file("abcdefghijklmnopqrstuvwxyz\nsecond line\nthird line\n");

        let result = read_file(path.to_str().expect("utf-8 path"), 1, 10, 10, 30)
            .expect("read should succeed");

        fs::remove_file(&path).expect("temp file should be deleted");

        assert_eq!(result.start_line, 1);
        assert_eq!(result.end_line, 1);
        assert!(result.hit_total_char_limit);
        assert_eq!(result.content, "     1\tabcdefghij [truncated]");
    }

    #[test]
    fn reads_multiple_lines_when_budget_allows() {
        let path = write_temp_file("one\ntwo\nthree\n");

        let result = read_file(path.to_str().expect("utf-8 path"), 1, 10, 50, 100)
            .expect("read should succeed");

        fs::remove_file(&path).expect("temp file should be deleted");

        assert_eq!(result.end_line, 3);
        assert!(!result.hit_total_char_limit);
        assert_eq!(result.content, "     1\tone\n     2\ttwo\n     3\tthree");
    }

    #[test]
    fn bounded_byte_read_rejects_a_file_before_returning_oversized_content() {
        let path = write_temp_file("12345");

        let rejected = read_file_bytes_bounded(path.to_str().expect("utf-8 path"), 4)
            .expect("bounded read should succeed");
        let accepted = read_file_bytes_bounded(path.to_str().expect("utf-8 path"), 5)
            .expect("bounded read should succeed");

        fs::remove_file(&path).expect("temp file should be deleted");

        assert!(rejected.is_none());
        assert_eq!(accepted, Some(b"12345".to_vec()));
    }

    #[test]
    fn decoded_text_reuses_normal_window_and_tail_semantics() {
        let text = "one\ntwo\nthree\nfour\n";

        let window = read_text(text, 2, 2, 50, 100).expect("window should read");
        let tail = read_text_tail(text, 2, 50, 100).expect("tail should read");

        assert_eq!(window.start_line, 2);
        assert_eq!(window.end_line, 3);
        assert_eq!(window.total_lines, 4);
        assert_eq!(window.content, "     2\ttwo\n     3\tthree");
        assert_eq!(tail.start_line, 3);
        assert_eq!(tail.end_line, 4);
        assert_eq!(tail.content, "     3\tthree\n     4\tfour");
    }

    #[test]
    fn read_file_presentation_reports_continuation_window() {
        let result = ReadFileResult {
            start_line: 1,
            end_line: 2,
            total_lines: 4,
            content: "     1\tone\n     2\ttwo".to_string(),
            hit_total_char_limit: false,
            content_truncated: false,
        };

        let presentation = build_read_file_presentation("src/lib.rs", &result);

        assert_eq!(presentation.lines_read, 2);
        assert!(presentation
            .result_for_assistant
            .contains("Read lines 1-2 from src/lib.rs (4 total lines)"));
        assert!(presentation
            .result_for_assistant
            .contains("Use offset=3 and limit to continue reading."));
    }

    #[test]
    fn reads_tail_window_with_original_line_numbers() {
        let path = write_temp_file("one\ntwo\nthree\nfour\nfive\n");

        let result = read_file_tail(path.to_str().expect("utf-8 path"), 2, 50, 100)
            .expect("tail read should succeed");

        fs::remove_file(&path).expect("temp file should be deleted");

        assert_eq!(result.start_line, 4);
        assert_eq!(result.end_line, 5);
        assert_eq!(result.total_lines, 5);
        assert_eq!(result.content, "     4\tfour\n     5\tfive");
    }

    #[test]
    fn read_file_lines_read_handles_empty_files() {
        let result = ReadFileResult {
            start_line: 0,
            end_line: 0,
            total_lines: 0,
            content: String::new(),
            hit_total_char_limit: false,
            content_truncated: false,
        };

        assert_eq!(read_file_lines_read(&result), 0);
    }
}
