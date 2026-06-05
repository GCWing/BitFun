/// Strip a Read-tool line prefix (`spaces + line_number + tab|→`) from one line.
pub fn strip_read_line_number_prefix(line: &str) -> String {
    let mut chars = line.chars().peekable();

    while matches!(chars.peek(), Some(' ')) {
        chars.next();
    }

    let mut saw_digits = false;
    while matches!(chars.peek(), Some(ch) if ch.is_ascii_digit()) {
        saw_digits = true;
        chars.next();
    }

    if !saw_digits {
        return line.to_string();
    }

    match chars.peek().copied() {
        Some('\t') => {
            chars.next();
            chars.collect()
        }
        Some('\u{2192}') => {
            chars.next();
            chars.collect()
        }
        _ => line.to_string(),
    }
}

/// Convert Read-tool cat -n output into raw file content (one line at a time).
pub fn read_tool_output_to_file_content(formatted: &str) -> String {
    formatted
        .lines()
        .map(strip_read_line_number_prefix)
        .collect::<Vec<_>>()
        .join("\n")
}

/// True when every non-empty line still carries a Read-tool prefix.
pub fn all_lines_have_read_prefix(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    text.lines().all(|line| line_has_read_prefix(line))
}

fn line_has_read_prefix(line: &str) -> bool {
    strip_read_line_number_prefix(line) != line
}

#[cfg(test)]
mod tests {
    use super::{
        all_lines_have_read_prefix, read_tool_output_to_file_content, strip_read_line_number_prefix,
    };

    #[test]
    fn strip_tab_prefix() {
        assert_eq!(strip_read_line_number_prefix("     1\talpha"), "alpha");
    }

    #[test]
    fn strip_arrow_prefix() {
        assert_eq!(strip_read_line_number_prefix("     2→beta"), "beta");
    }

    #[test]
    fn leaves_unprefixed_lines_unchanged() {
        assert_eq!(strip_read_line_number_prefix("plain"), "plain");
    }

    #[test]
    fn read_tool_output_to_file_content_strips_each_line() {
        assert_eq!(
            read_tool_output_to_file_content("     1\talpha\n     2\tbeta"),
            "alpha\nbeta"
        );
    }

    #[test]
    fn read_tool_output_to_file_content_strips_crlf_line_endings() {
        assert_eq!(
            read_tool_output_to_file_content("     1\talpha\r\n     2\tbeta\r\n"),
            "alpha\nbeta"
        );
    }

    #[test]
    fn all_lines_have_read_prefix_requires_every_line() {
        assert!(all_lines_have_read_prefix("     1\ta\n     2\tb"));
        assert!(!all_lines_have_read_prefix("     1\ta\nplain"));
    }
}
