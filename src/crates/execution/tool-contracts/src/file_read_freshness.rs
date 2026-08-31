//! Pure file-read freshness rules for Read/Edit/Write guardrails.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileReadFreshnessFacts<'a> {
    pub content: &'a str,
    pub timestamp_ms: u64,
    pub is_full_file_read: bool,
}

/// Normalize file content for freshness comparison only (not for diffing or
/// content substitution).
///
/// The cached "last Read result" content is reconstructed from formatted
/// `cat -n`-style tool output via a line-split/join, which always drops a
/// trailing newline even when the real file ends with one. A fresh re-read
/// (local `fs::read_to_string` or remote SFTP `read_file_text`) preserves it.
/// Without normalizing this away, every full-file Edit/Write on a file that
/// ends with a newline (the common case) would look "changed" purely from
/// that reconstruction gap. This applies equally to local and remote reads.
pub fn normalize_tool_file_content(content: &str) -> String {
    let normalized = if content.contains("\r\n") {
        content.replace("\r\n", "\n")
    } else {
        content.to_string()
    };
    normalized.trim_end_matches('\n').to_string()
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
    // A known content difference is stronger evidence than timestamps: SFTP
    // timestamps can have second precision, and any filesystem can restore an
    // older mtime while changing the bytes.
    if read_facts.is_full_file_read {
        return file_read_facts_content_matches(read_facts, current_content);
    }

    // Partial reads retain their existing weaker guarantee; the unobserved
    // portion cannot be compared with the cached content.
    current_mtime_ms.is_none_or(|mtime| mtime <= read_facts.timestamp_ms)
}
