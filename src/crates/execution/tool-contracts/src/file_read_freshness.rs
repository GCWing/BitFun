//! Pure file-read freshness rules for Read/Edit/Write guardrails.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileReadFreshnessFacts<'a> {
    pub content: &'a str,
    pub timestamp_ms: u64,
    pub is_full_file_read: bool,
}

pub fn normalize_tool_file_content(content: &str) -> String {
    if content.contains("\r\n") {
        content.replace("\r\n", "\n")
    } else {
        content.to_string()
    }
}

pub fn file_read_facts_content_matches(
    read_facts: FileReadFreshnessFacts<'_>,
    current_content: &str,
) -> bool {
    read_facts.is_full_file_read
        && normalize_tool_file_content(current_content)
            == normalize_tool_file_content(read_facts.content)
}

pub fn file_read_facts_are_fresh(
    read_facts: FileReadFreshnessFacts<'_>,
    current_content: &str,
    current_mtime_ms: Option<u64>,
) -> bool {
    if let Some(current_mtime_ms) = current_mtime_ms {
        if current_mtime_ms <= read_facts.timestamp_ms {
            return true;
        }
        return file_read_facts_content_matches(read_facts, current_content);
    }

    if read_facts.is_full_file_read {
        return file_read_facts_content_matches(read_facts, current_content);
    }

    true
}
