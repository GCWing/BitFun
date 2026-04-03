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

/// Count lines before given byte position (line numbers start from 1)
fn count_lines_before(content: &str, byte_pos: usize) -> usize {
    content[..byte_pos].matches('\n').count() + 1
}

/// Count newlines in string
fn count_newlines(s: &str) -> usize {
    s.matches('\n').count()
}

pub fn edit_file(
    file_path: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<EditResult, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;

    // Detect file line ending format
    let uses_crlf = content.contains("\r\n");

    // Normalize old_string and new_string (unified conversion to \n)
    let normalized_old = normalize_string(old_string);
    let normalized_new = normalize_string(new_string);

    // Normalize content for matching
    let normalized_content = normalize_string(&content);

    // Find matches in normalized content
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

    // Get first match position (replace_all also only returns first match line number)
    let first_match_pos = matches[0].0;

    // Calculate old_string line number range
    let start_line = count_lines_before(&normalized_content, first_match_pos);
    let old_newlines = count_newlines(&normalized_old);
    let old_end_line = start_line + old_newlines;

    // Calculate new_string line number range (start line number is the same)
    let new_newlines = count_newlines(&normalized_new);
    let new_end_line = start_line + new_newlines;

    // Replace in normalized content
    let mut new_content = normalized_content.replace(&normalized_old, &normalized_new);

    // If original file uses CRLF, restore CRLF format
    if uses_crlf {
        new_content = new_content.replace("\n", "\r\n");
    }

    fs::write(file_path, &new_content)
        .map_err(|e| format!("Failed to write file {}: {}", file_path, e))?;

    Ok(EditResult {
        start_line,
        old_end_line,
        new_end_line,
    })
}
