pub fn normalize_string(s: &str) -> String {
    if s.contains("\r\n") {
        s.replace("\r\n", "\n")
    } else {
        s.to_string()
    }
}

pub fn truncate_string_by_chars(s: &str, kept_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    chars[..kept_chars].iter().collect()
}

pub fn escape_posix_single_quotes(value: &str) -> String {
    value.replace('\'', "'\\''")
}

pub fn shell_single_quote(value: &str) -> String {
    format!("'{}'", escape_posix_single_quotes(value))
}
