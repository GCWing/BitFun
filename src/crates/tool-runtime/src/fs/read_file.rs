use crate::util::string::truncate_string_by_chars;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;

#[derive(Debug)]
pub struct ReadFileResult {
    pub start_line: usize,
    pub end_line: usize,
    pub total_lines: usize,
    pub content: String,
    pub hit_total_char_limit: bool,
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

#[cfg(test)]
mod tests {
    use super::read_file;
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
}
