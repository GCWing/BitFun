use crate::util::string::normalize_string;
use std::fs;

/// Edit result, contains line number range information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditResult {
    /// Start line number of old_string/new_string (starts from 1)
    pub start_line: usize,
    /// End line number of old_string (starts from 1)
    pub old_end_line: usize,
    /// End line number of new_string after replacement (starts from 1)
    pub new_end_line: usize,
}

/// Result of applying an edit to in-memory content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyEditResult {
    pub new_content: String,
    pub match_count: usize,
    pub edit_result: EditResult,
}

/// Count lines before given byte position (line numbers start from 1)
fn count_lines_before(content: &str, byte_pos: usize) -> usize {
    content[..byte_pos].matches('\n').count() + 1
}

/// Count newlines in string
fn count_newlines(s: &str) -> usize {
    s.matches('\n').count()
}

pub fn apply_edit_to_content(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<ApplyEditResult, String> {
    let uses_crlf = content.contains("\r\n");
    let normalized_old = normalize_string(old_string);
    let normalized_new = normalize_string(new_string);
    let normalized_content = normalize_string(content);
    let matches: Vec<_> = normalized_content.match_indices(&normalized_old).collect();

    if matches.is_empty() {
        return Err("old_string not found in file.".to_string());
    }

    if matches.len() > 1 && !replace_all {
        return Err(format!(
            "`old_string` appears {} times in file, either provide a larger string with more surrounding context to make it unique or use `replace_all` to change every instance of `old_string`.",
            matches.len()
        ));
    }

    let first_match_pos = matches[0].0;
    let start_line = count_lines_before(&normalized_content, first_match_pos);
    let old_end_line = start_line + count_newlines(&normalized_old);
    let new_end_line = start_line + count_newlines(&normalized_new);

    let new_normalized_content = if replace_all {
        normalized_content.replace(&normalized_old, &normalized_new)
    } else {
        normalized_content.replacen(&normalized_old, &normalized_new, 1)
    };

    let new_content = if uses_crlf {
        new_normalized_content.replace("\n", "\r\n")
    } else {
        new_normalized_content
    };

    Ok(ApplyEditResult {
        new_content,
        match_count: matches.len(),
        edit_result: EditResult {
            start_line,
            old_end_line,
            new_end_line,
        },
    })
}

pub fn edit_file(
    file_path: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<EditResult, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;
    let result = apply_edit_to_content(&content, old_string, new_string, replace_all)?;

    fs::write(file_path, &result.new_content)
        .map_err(|e| format!("Failed to write file {}: {}", file_path, e))?;

    Ok(result.edit_result)
}

#[cfg(test)]
mod tests {
    use super::{apply_edit_to_content, edit_file, EditResult};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_temp_file(contents: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("bitfun-edit-file-test-{unique}.txt"));
        fs::write(&path, contents).expect("temp file should be written");
        path
    }

    #[test]
    fn apply_edit_to_content_matches_multiline_lf_input_against_crlf_file() {
        let content = "header\r\nalpha\r\nbeta\r\nfooter\r\n";
        let result = apply_edit_to_content(content, "alpha\nbeta", "alpha\nBETA", false)
            .expect("edit should succeed");

        assert_eq!(result.match_count, 1);
        assert_eq!(
            result.edit_result,
            EditResult {
                start_line: 2,
                old_end_line: 3,
                new_end_line: 3,
            }
        );
        assert_eq!(result.new_content, "header\r\nalpha\r\nBETA\r\nfooter\r\n");
    }

    #[test]
    fn apply_edit_to_content_replace_all_reports_match_count() {
        let result = apply_edit_to_content("one\r\ntwo\r\none\r\n", "one", "ONE", true)
            .expect("replace_all should succeed");

        assert_eq!(result.match_count, 2);
        assert_eq!(result.new_content, "ONE\r\ntwo\r\nONE\r\n");
        assert_eq!(result.edit_result.start_line, 1);
    }

    #[test]
    fn edit_file_preserves_crlf_when_editing_with_lf_old_string() {
        let path = write_temp_file("first\r\nalpha\r\nbeta\r\n");

        let result = edit_file(
            path.to_str().expect("utf-8 path"),
            "alpha\nbeta",
            "alpha\nBETA",
            false,
        )
        .expect("edit should succeed");
        let content = fs::read_to_string(&path).expect("edited file should be readable");

        fs::remove_file(&path).expect("temp file should be deleted");

        assert_eq!(
            result,
            EditResult {
                start_line: 2,
                old_end_line: 3,
                new_end_line: 3,
            }
        );
        assert_eq!(content, "first\r\nalpha\r\nBETA\r\n");
    }
}
