//! Session-scoped file read state used to gate Edit/Write reliability.

use bitfun_agent_tools::{
    file_read_facts_are_fresh, file_read_facts_content_matches, FileReadFreshnessFacts,
};
use dashmap::DashMap;
use std::sync::Arc;

pub const FILE_UNEXPECTEDLY_MODIFIED_ERROR: &str =
    "File has been unexpectedly modified. Read it again before attempting to write it.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMutationKind {
    Edit,
    Write,
}

impl FileMutationKind {
    fn tool_name(self) -> &'static str {
        match self {
            Self::Edit => "Edit",
            Self::Write => "Write",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReadState {
    /// Raw file content without Read-tool line-number prefixes (LF-normalized view).
    pub content: String,
    /// File mtime in milliseconds since UNIX epoch when recorded, if known.
    pub timestamp_ms: u64,
    pub start_line: usize,
    pub end_line: usize,
    pub total_lines: usize,
    /// True when this entry was populated by auto-injection and the model has
    /// not explicitly read the file. Range reads from the Read tool do not set this.
    pub is_partial_view: bool,
}

impl FileReadState {
    pub fn from_read_tool_content(
        content: String,
        timestamp_ms: u64,
        start_line: usize,
        end_line: usize,
        total_lines: usize,
    ) -> Self {
        Self {
            content,
            timestamp_ms,
            start_line,
            end_line,
            total_lines,
            is_partial_view: false,
        }
    }

    pub fn from_full_content(content: &str, timestamp_ms: u64) -> Self {
        let line_count = content.lines().count();
        let (start_line, end_line) = if line_count == 0 {
            (0, 0)
        } else {
            (1, line_count)
        };

        Self {
            content: content.to_string(),
            timestamp_ms,
            start_line,
            end_line,
            total_lines: line_count,
            is_partial_view: false,
        }
    }

    pub fn is_full_file_read(&self) -> bool {
        if self.is_partial_view {
            return false;
        }

        if self.total_lines == 0 {
            return self.start_line == 0 && self.end_line == 0;
        }

        self.start_line == 1 && self.end_line >= self.total_lines
    }
}

pub fn validate_prior_read_state(
    logical_path: &str,
    read_state: Option<&FileReadState>,
    mutation: FileMutationKind,
) -> Option<String> {
    let Some(read_state) = read_state else {
        return Some(format!(
            "Use Read to load the current contents of {} before calling {} on it.",
            logical_path,
            mutation.tool_name()
        ));
    };

    if read_state.is_partial_view {
        return Some(format!(
            "Use Read to load the full contents of {} before calling {} on it.",
            logical_path,
            mutation.tool_name()
        ));
    }

    None
}

pub fn content_unchanged_since_full_read(
    read_state: &FileReadState,
    current_content: &str,
) -> bool {
    file_read_facts_content_matches(file_read_freshness_facts(read_state), current_content)
}

pub fn assert_file_not_unexpectedly_modified(
    read_state: Option<&FileReadState>,
    current_content: &str,
    current_mtime_ms: Option<u64>,
) -> Result<(), String> {
    let Some(read_state) = read_state else {
        return Err(FILE_UNEXPECTEDLY_MODIFIED_ERROR.to_string());
    };

    if !file_read_facts_are_fresh(
        file_read_freshness_facts(read_state),
        current_content,
        current_mtime_ms,
    ) {
        return Err(FILE_UNEXPECTEDLY_MODIFIED_ERROR.to_string());
    }

    Ok(())
}

pub fn validate_edit_content_freshness_against_read_state(
    logical_path: &str,
    read_state: &FileReadState,
    current_content: &str,
    current_mtime_ms: Option<u64>,
) -> Option<String> {
    if file_read_facts_are_fresh(
        file_read_freshness_facts(read_state),
        current_content,
        current_mtime_ms,
    ) {
        return None;
    }

    if current_mtime_ms.is_some() {
        return Some(format!(
            "The file {} changed after it was last read. Use Read again, then retry Edit.",
            logical_path
        ));
    }

    Some(format!(
        "The file {} no longer matches the last Read result. Use Read again, then retry Edit.",
        logical_path
    ))
}

pub fn validate_write_mtime_freshness_against_read_state(
    logical_path: &str,
    read_state: &FileReadState,
    current_mtime_ms: u64,
) -> Option<String> {
    if current_mtime_ms > read_state.timestamp_ms {
        return Some(format!(
            "The file {} changed after it was last read. Use Read again, then retry Write.",
            logical_path
        ));
    }

    None
}

pub fn validate_write_content_freshness_against_read_state(
    logical_path: &str,
    read_state: &FileReadState,
    current_content: &str,
) -> Option<String> {
    if !file_read_facts_are_fresh(file_read_freshness_facts(read_state), current_content, None) {
        return Some(format!(
            "The file {} no longer matches the last Read result. Use Read again, then retry Write.",
            logical_path
        ));
    }

    None
}

fn file_read_freshness_facts(read_state: &FileReadState) -> FileReadFreshnessFacts<'_> {
    FileReadFreshnessFacts {
        content: &read_state.content,
        timestamp_ms: read_state.timestamp_ms,
        is_full_file_read: read_state.is_full_file_read(),
    }
}

#[derive(Default)]
pub struct FileReadStateStore {
    session_states: Arc<DashMap<String, DashMap<String, FileReadState>>>,
}

impl FileReadStateStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(&self, session_id: &str) {
        self.session_states
            .entry(session_id.to_string())
            .or_default();
    }

    pub fn delete_session(&self, session_id: &str) {
        self.session_states.remove(session_id);
    }

    pub fn clear_session(&self, session_id: &str) {
        if let Some(states) = self.session_states.get(session_id) {
            states.clear();
        }
    }

    pub fn set(&self, session_id: &str, logical_path: &str, state: FileReadState) {
        let session_states = self
            .session_states
            .entry(session_id.to_string())
            .or_default();
        session_states.insert(logical_path.to_string(), state);
    }

    pub fn get(&self, session_id: &str, logical_path: &str) -> Option<FileReadState> {
        self.session_states
            .get(session_id)
            .and_then(|states| states.get(logical_path).map(|entry| entry.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        assert_file_not_unexpectedly_modified, validate_edit_content_freshness_against_read_state,
        validate_prior_read_state, validate_write_content_freshness_against_read_state,
        validate_write_mtime_freshness_against_read_state, FileMutationKind, FileReadState,
        FileReadStateStore,
    };

    fn sample_state(
        start_line: usize,
        end_line: usize,
        total_lines: usize,
        is_partial_view: bool,
    ) -> FileReadState {
        FileReadState {
            content: String::new(),
            timestamp_ms: 0,
            start_line,
            end_line,
            total_lines,
            is_partial_view,
        }
    }

    #[test]
    fn file_read_state_accepts_nonempty_whole_file() {
        let state = sample_state(1, 10, 10, false);
        assert!(state.is_full_file_read());
    }

    #[test]
    fn file_read_state_rejects_partial_view() {
        let state = sample_state(1, 10, 10, true);
        assert!(!state.is_full_file_read());
    }

    #[test]
    fn file_read_state_accepts_empty_file_from_read_tool() {
        let state = sample_state(0, 0, 0, false);
        assert!(state.is_full_file_read());
    }

    #[test]
    fn file_read_state_rejects_empty_file_with_one_based_range() {
        let state = sample_state(1, 0, 0, false);
        assert!(!state.is_full_file_read());
    }

    #[test]
    fn file_read_state_store_scopes_entries_by_session() {
        let store = FileReadStateStore::new();
        store.create_session("session-a");
        store.create_session("session-b");

        store.set("session-a", "src/lib.rs", sample_state(1, 1, 1, false));

        assert!(store.get("session-a", "src/lib.rs").is_some());
        assert!(store.get("session-b", "src/lib.rs").is_none());
        assert!(store.get("session-a", "src/main.rs").is_none());
    }

    #[test]
    fn file_read_state_store_clear_and_delete_drop_session_entries() {
        let store = FileReadStateStore::new();
        store.set("session-a", "src/lib.rs", sample_state(1, 1, 1, false));
        store.set("session-b", "src/lib.rs", sample_state(1, 1, 1, false));

        store.clear_session("session-a");
        assert!(store.get("session-a", "src/lib.rs").is_none());
        assert!(store.get("session-b", "src/lib.rs").is_some());

        store.delete_session("session-b");
        assert!(store.get("session-b", "src/lib.rs").is_none());
    }

    #[test]
    fn validate_prior_read_state_requires_initial_read() {
        assert_eq!(
            validate_prior_read_state("src/main.rs", None, FileMutationKind::Edit).as_deref(),
            Some("Use Read to load the current contents of src/main.rs before calling Edit on it.")
        );
    }

    #[test]
    fn validate_prior_read_state_rejects_auto_injected_partial_view() {
        let state = sample_state(1, 10, 10, true);

        assert_eq!(
            validate_prior_read_state("src/main.rs", Some(&state), FileMutationKind::Write)
                .as_deref(),
            Some("Use Read to load the full contents of src/main.rs before calling Write on it.")
        );
    }

    #[test]
    fn validate_prior_read_state_accepts_read_tool_range() {
        let state = sample_state(50, 100, 556, false);

        assert!(
            validate_prior_read_state("src/main.rs", Some(&state), FileMutationKind::Edit)
                .is_none()
        );
    }

    #[test]
    fn file_read_state_from_full_content_sets_empty_file_range() {
        let state = FileReadState::from_full_content("", 100);

        assert_eq!(state.start_line, 0);
        assert_eq!(state.end_line, 0);
        assert_eq!(state.total_lines, 0);
        assert!(state.is_full_file_read());
    }

    #[test]
    fn content_unchanged_since_full_read_rejects_partial_ranges() {
        let state = sample_state(50, 100, 556, false);

        assert!(!super::content_unchanged_since_full_read(
            &state, "middle\n"
        ));
    }

    #[test]
    fn validate_edit_content_freshness_rejects_partial_read_range_without_full_file() {
        let state = FileReadState {
            content: "middle\n".to_string(),
            timestamp_ms: 100,
            start_line: 50,
            end_line: 100,
            total_lines: 556,
            is_partial_view: false,
        };

        assert!(validate_edit_content_freshness_against_read_state(
            "src/state.js",
            &state,
            "different full file\n",
            Some(200),
        )
        .is_some());
    }

    #[test]
    fn assert_file_not_unexpectedly_modified_allows_matching_full_read_after_newer_mtime() {
        let state = read_state("alpha\n", 100);

        assert!(assert_file_not_unexpectedly_modified(Some(&state), "alpha\n", Some(200)).is_ok());
    }

    #[test]
    fn assert_file_not_unexpectedly_modified_rejects_changed_full_read_after_newer_mtime() {
        let state = read_state("alpha\n", 100);

        assert!(assert_file_not_unexpectedly_modified(Some(&state), "beta\n", Some(200)).is_err());
    }

    #[test]
    fn assert_file_not_unexpectedly_modified_rejects_partial_read_after_newer_mtime() {
        let state = FileReadState {
            content: "middle\n".to_string(),
            timestamp_ms: 100,
            start_line: 50,
            end_line: 100,
            total_lines: 556,
            is_partial_view: false,
        };

        assert!(
            assert_file_not_unexpectedly_modified(Some(&state), "full file\n", Some(200)).is_err()
        );
    }

    fn read_state(content: &str, timestamp_ms: u64) -> FileReadState {
        FileReadState {
            content: content.to_string(),
            timestamp_ms,
            start_line: 1,
            end_line: 1,
            total_lines: 1,
            is_partial_view: false,
        }
    }

    #[test]
    fn validate_edit_content_freshness_allows_matching_remote_content_without_mtime() {
        let state = read_state("alpha\n", 100);

        assert!(validate_edit_content_freshness_against_read_state(
            "src/main.rs",
            &state,
            "alpha\n",
            None,
        )
        .is_none());
    }

    #[test]
    fn validate_edit_content_freshness_allows_remote_content_missing_cached_trailing_newline() {
        // Regression test: the Read tool's cached content is reconstructed via
        // a line-split/join that drops a trailing newline, while a remote
        // SFTP re-read of the same unchanged file preserves it. Remote
        // workspaces have no mtime to short-circuit the comparison, so this
        // must not be reported as "no longer matches the last Read result".
        let state = read_state("alpha\nbeta", 100);

        assert!(validate_edit_content_freshness_against_read_state(
            "Caddyfile",
            &state,
            "alpha\nbeta\n",
            None,
        )
        .is_none());
    }

    #[test]
    fn validate_edit_content_freshness_rejects_changed_remote_content_without_mtime() {
        let state = read_state("alpha\n", 100);

        assert_eq!(
            validate_edit_content_freshness_against_read_state(
                "src/main.rs",
                &state,
                "beta\n",
                None,
            )
            .as_deref(),
            Some(
                "The file src/main.rs no longer matches the last Read result. Use Read again, then retry Edit."
            )
        );
    }

    #[test]
    fn validate_edit_content_freshness_allows_newer_mtime_when_full_read_content_matches() {
        let state = read_state("alpha\n", 100);

        assert!(validate_edit_content_freshness_against_read_state(
            "src/main.rs",
            &state,
            "alpha\n",
            Some(200),
        )
        .is_none());
    }

    #[test]
    fn validate_edit_content_freshness_rejects_newer_mtime_when_content_differs() {
        let state = read_state("alpha\n", 100);

        assert!(validate_edit_content_freshness_against_read_state(
            "src/main.rs",
            &state,
            "beta\n",
            Some(200),
        )
        .is_some());
    }

    #[test]
    fn validate_edit_content_freshness_ignores_older_mtime_even_when_content_differs() {
        let state = read_state("alpha\n", 200);

        assert!(validate_edit_content_freshness_against_read_state(
            "src/main.rs",
            &state,
            "beta\n",
            Some(100),
        )
        .is_none());
    }

    #[test]
    fn validate_write_mtime_freshness_rejects_newer_mtime_before_content_compare() {
        let state = read_state("alpha\n", 100);

        assert_eq!(
            validate_write_mtime_freshness_against_read_state("src/main.rs", &state, 200)
                .as_deref(),
            Some("The file src/main.rs changed after it was last read. Use Read again, then retry Write.")
        );
    }

    #[test]
    fn validate_write_mtime_freshness_allows_older_mtime_without_content_compare() {
        let state = read_state("alpha\n", 200);

        assert!(
            validate_write_mtime_freshness_against_read_state("src/main.rs", &state, 100).is_none()
        );
    }

    #[test]
    fn validate_write_content_freshness_rejects_changed_remote_content_without_mtime() {
        let state = read_state("alpha\n", 100);

        assert_eq!(
            validate_write_content_freshness_against_read_state(
                "src/main.rs",
                &state,
                "beta\n",
            )
            .as_deref(),
            Some(
                "The file src/main.rs no longer matches the last Read result. Use Read again, then retry Write."
            )
        );
    }
}
