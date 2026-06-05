/// Parses a Git log line formatted as `hash|author|email|date|message`.
pub fn parse_git_log_line(line: &str) -> Option<(String, String, String, String, String)> {
    let parts: Vec<&str> = line.splitn(5, '|').collect();
    if parts.len() == 5 {
        Some((
            parts[0].to_string(),
            parts[1].to_string(),
            parts[2].to_string(),
            parts[3].to_string(),
            parts[4].to_string(),
        ))
    } else {
        None
    }
}

/// Parses a Git branch list line, preserving the current-branch marker.
pub fn parse_branch_line(line: &str) -> Option<(String, bool)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(stripped) = trimmed.strip_prefix("* ") {
        Some((stripped.to_string(), true))
    } else if let Some(stripped) = trimmed.strip_prefix("  ") {
        Some((stripped.to_string(), false))
    } else {
        Some((trimmed.to_string(), false))
    }
}
