use crate::util::string::{shell_single_quote, truncate_string_by_chars};
use std::collections::VecDeque;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;

const REMOTE_TOTAL_LINES_MARKER: &str = "__BITFUN_TOTAL_LINES__=";
const REMOTE_HIT_TOTAL_CHAR_LIMIT_MARKER: &str = "__BITFUN_HIT_TOTAL_CHAR_LIMIT__=";

#[derive(Debug)]
pub struct ReadFileResult {
    pub start_line: usize,
    pub end_line: usize,
    pub total_lines: usize,
    pub content: String,
    pub hit_total_char_limit: bool,
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

pub fn build_remote_read_command(
    resolved_path: &str,
    start_line: usize,
    limit: usize,
    max_line_chars: usize,
    max_total_chars: usize,
) -> Result<String, String> {
    let end_line = start_line
        .checked_add(limit.saturating_sub(1))
        .ok_or_else(|| "Requested line range is too large".to_string())?;
    let escaped_path = shell_single_quote(resolved_path);

    Ok(format!(
        "if [ ! -f {path} ]; then exit 3; fi; awk -v start={start} -v end={end} -v max={max} -v budget={budget} 'BEGIN {{ total = 0; used = 0; hit = 0; }} {{ total = NR; if (!hit && NR >= start && NR <= end) {{ line = $0; if (length(line) > max) {{ line = substr(line, 1, max) \" [truncated]\"; }} rendered = sprintf(\"%6d\\t%s\", NR, line); extra = (used > 0 ? 1 : 0); next_used = used + extra + length(rendered); if (next_used > budget) {{ hit = 1; next; }} print rendered; used = next_used; }} }} END {{ printf(\"{marker}%d\\n\", total) > \"/dev/stderr\"; printf(\"{hit_marker}%d\\n\", hit) > \"/dev/stderr\"; }}' {path}",
        path = escaped_path,
        start = start_line,
        end = end_line,
        max = max_line_chars,
        budget = max_total_chars,
        marker = REMOTE_TOTAL_LINES_MARKER,
        hit_marker = REMOTE_HIT_TOTAL_CHAR_LIMIT_MARKER,
    ))
}

pub fn build_remote_tail_read_command(
    resolved_path: &str,
    limit: usize,
    max_line_chars: usize,
    max_total_chars: usize,
) -> Result<String, String> {
    if limit == 0 {
        return Err("`limit` can't be 0".to_string());
    }

    let escaped_path = shell_single_quote(resolved_path);

    Ok(format!(
        "if [ ! -f {path} ]; then exit 3; fi; awk -v limit={limit} -v max={max} -v budget={budget} 'BEGIN {{ total = 0; used = 0; hit = 0; }} {{ total = NR; slot = ((NR - 1) % limit) + 1; lines[slot] = $0; nums[slot] = NR; }} END {{ start = total - limit + 1; if (start < 1) start = 1; for (nr = start; nr <= total; nr++) {{ slot = ((nr - 1) % limit) + 1; line = lines[slot]; if (length(line) > max) {{ line = substr(line, 1, max) \" [truncated]\"; }} rendered = sprintf(\"%6d\\t%s\", nums[slot], line); extra = (used > 0 ? 1 : 0); next_used = used + extra + length(rendered); if (next_used > budget) {{ hit = 1; break; }} print rendered; used = next_used; }} printf(\"{marker}%d\\n\", total) > \"/dev/stderr\"; printf(\"{hit_marker}%d\\n\", hit) > \"/dev/stderr\"; }}' {path}",
        path = escaped_path,
        limit = limit,
        max = max_line_chars,
        budget = max_total_chars,
        marker = REMOTE_TOTAL_LINES_MARKER,
        hit_marker = REMOTE_HIT_TOTAL_CHAR_LIMIT_MARKER,
    ))
}

pub fn parse_remote_read_output(
    stdout: &str,
    stderr: &str,
    status: i32,
    resolved_path: &str,
    start_line: usize,
) -> Result<ReadFileResult, String> {
    let mut total_lines = None;
    let mut hit_total_char_limit = false;
    let mut stderr_messages = Vec::new();
    for line in stderr.lines() {
        if let Some(rest) = line.strip_prefix(REMOTE_TOTAL_LINES_MARKER) {
            total_lines = rest.trim().parse::<usize>().ok();
        } else if let Some(rest) = line.strip_prefix(REMOTE_HIT_TOTAL_CHAR_LIMIT_MARKER) {
            hit_total_char_limit = rest.trim() == "1";
        } else if !line.trim().is_empty() {
            stderr_messages.push(line.to_string());
        }
    }

    if status != 0 {
        let message = if status == 3 {
            format!("File not found or not a regular file: {}", resolved_path)
        } else if !stderr_messages.is_empty() {
            stderr_messages.join("\n")
        } else {
            format!(
                "Failed to read file: remote command exited with status {}",
                status
            )
        };
        return Err(message);
    }

    let total_lines = total_lines.ok_or_else(|| {
        "Failed to read file: remote command did not return line-count markers".to_string()
    })?;

    if total_lines == 0 {
        return Ok(ReadFileResult {
            start_line: 0,
            end_line: 0,
            total_lines: 0,
            content: String::new(),
            hit_total_char_limit,
        });
    }

    if start_line > total_lines {
        return Err(format!(
            "`start_line` {} is larger than the number of lines in the file: {}",
            start_line, total_lines
        ));
    }

    let content = stdout.trim_end_matches('\n').to_string();
    let lines_read = if content.is_empty() {
        0
    } else {
        content.lines().count()
    };
    let end_line = if lines_read == 0 {
        start_line
    } else {
        (start_line + lines_read).saturating_sub(1)
    };

    Ok(ReadFileResult {
        start_line,
        end_line,
        total_lines,
        content,
        hit_total_char_limit,
    })
}

pub fn parse_remote_tail_read_output(
    stdout: &str,
    stderr: &str,
    status: i32,
    resolved_path: &str,
    limit: usize,
) -> Result<ReadFileResult, String> {
    let mut result = parse_remote_read_output(stdout, stderr, status, resolved_path, 1)?;

    if result.total_lines == 0 {
        return Ok(result);
    }

    let start_line = result.total_lines.saturating_sub(limit).saturating_add(1);
    let lines_read = if result.content.is_empty() {
        0
    } else {
        result.content.lines().count()
    };
    result.start_line = start_line;
    result.end_line = if lines_read == 0 {
        start_line
    } else {
        start_line.saturating_add(lines_read).saturating_sub(1)
    };

    Ok(result)
}

/// start_line: starts from 1
pub fn read_file(
    file_path: &str,
    start_line: usize,
    limit: usize,
    max_line_chars: usize,
    max_total_chars: usize,
) -> Result<ReadFileResult, String> {
    if start_line == 0 {
        return Err("`start_line` should start from 1".to_string());
    }
    if limit == 0 {
        return Err("`limit` can't be 0".to_string());
    }
    if max_total_chars == 0 {
        return Err("`max_total_chars` can't be 0".to_string());
    }
    let end_line_inclusive = start_line
        .checked_add(limit.saturating_sub(1))
        .ok_or_else(|| "Requested line range is too large".to_string())?;

    let file =
        File::open(file_path).map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;
    let reader = BufReader::new(file);

    let mut total_lines = 0usize;
    let mut selected_lines = Vec::new();
    let mut selected_chars = 0usize;
    let mut hit_total_char_limit = false;

    for line_result in reader.lines() {
        let line = line_result.map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;
        total_lines += 1;

        if total_lines < start_line || total_lines > end_line_inclusive || hit_total_char_limit {
            continue;
        }

        let line_content = if line.chars().count() > max_line_chars {
            format!(
                "{} [truncated]",
                truncate_string_by_chars(&line, max_line_chars)
            )
        } else {
            line
        };

        let rendered_line = format!("{:>6}\t{}", total_lines, line_content);
        let separator_chars = usize::from(!selected_lines.is_empty());
        let next_total_chars = selected_chars
            .saturating_add(separator_chars)
            .saturating_add(rendered_line.chars().count());

        if next_total_chars > max_total_chars {
            hit_total_char_limit = true;
            continue;
        }

        selected_chars = next_total_chars;
        selected_lines.push(rendered_line);
    }

    if total_lines == 0 {
        return Ok(ReadFileResult {
            start_line: 0,
            end_line: 0,
            total_lines: 0,
            content: String::new(),
            hit_total_char_limit,
        });
    }

    if start_line > total_lines {
        return Err(format!(
            "`start_line` {} is larger than the number of lines in the file: {}",
            start_line, total_lines
        ));
    }

    let end_line = if selected_lines.is_empty() {
        start_line
    } else {
        (start_line + selected_lines.len()).saturating_sub(1)
    };

    Ok(ReadFileResult {
        start_line,
        end_line,
        total_lines,
        content: selected_lines.join("\n"),
        hit_total_char_limit,
    })
}

pub fn read_file_tail(
    file_path: &str,
    limit: usize,
    max_line_chars: usize,
    max_total_chars: usize,
) -> Result<ReadFileResult, String> {
    if limit == 0 {
        return Err("`limit` can't be 0".to_string());
    }
    if max_total_chars == 0 {
        return Err("`max_total_chars` can't be 0".to_string());
    }

    let file =
        File::open(file_path).map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;
    let reader = BufReader::new(file);

    let mut total_lines = 0usize;
    let mut tail_lines = VecDeque::with_capacity(limit);

    for line_result in reader.lines() {
        let line = line_result.map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;
        total_lines += 1;

        if tail_lines.len() == limit {
            tail_lines.pop_front();
        }
        tail_lines.push_back((total_lines, line));
    }

    if total_lines == 0 {
        return Ok(ReadFileResult {
            start_line: 0,
            end_line: 0,
            total_lines: 0,
            content: String::new(),
            hit_total_char_limit: false,
        });
    }

    let start_line = total_lines.saturating_sub(limit).saturating_add(1);
    let mut selected_lines = Vec::new();
    let mut selected_chars = 0usize;
    let mut hit_total_char_limit = false;

    for (line_number, line) in tail_lines {
        let line_content = if line.chars().count() > max_line_chars {
            format!(
                "{} [truncated]",
                truncate_string_by_chars(&line, max_line_chars)
            )
        } else {
            line
        };

        let rendered_line = format!("{:>6}\t{}", line_number, line_content);
        let separator_chars = usize::from(!selected_lines.is_empty());
        let next_total_chars = selected_chars
            .saturating_add(separator_chars)
            .saturating_add(rendered_line.chars().count());

        if next_total_chars > max_total_chars {
            hit_total_char_limit = true;
            break;
        }

        selected_chars = next_total_chars;
        selected_lines.push(rendered_line);
    }

    let end_line = if selected_lines.is_empty() {
        start_line
    } else {
        start_line
            .saturating_add(selected_lines.len())
            .saturating_sub(1)
    };

    Ok(ReadFileResult {
        start_line,
        end_line,
        total_lines,
        content: selected_lines.join("\n"),
        hit_total_char_limit,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_read_file_presentation, read_file, read_file_lines_read, read_file_tail,
        ReadFileResult,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

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
    fn read_file_presentation_reports_continuation_window() {
        let result = ReadFileResult {
            start_line: 1,
            end_line: 2,
            total_lines: 4,
            content: "     1\tone\n     2\ttwo".to_string(),
            hit_total_char_limit: false,
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
        };

        assert_eq!(read_file_lines_read(&result), 0);
    }
}
