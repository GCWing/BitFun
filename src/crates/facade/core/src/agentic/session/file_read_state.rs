//! Session-scoped cache of files the agent has read, used to gate Edit/Write reliability.

use dashmap::DashMap;
use log::debug;
use std::sync::Arc;

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

#[cfg(test)]
mod tests {
    use super::FileReadState;

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
    fn is_full_file_read_accepts_nonempty_whole_file() {
        let state = sample_state(1, 10, 10, false);
        assert!(state.is_full_file_read());
    }

    #[test]
    fn is_full_file_read_rejects_partial_view() {
        let state = sample_state(1, 10, 10, true);
        assert!(!state.is_full_file_read());
    }

    #[test]
    fn is_full_file_read_accepts_empty_file_from_read_tool() {
        let state = sample_state(0, 0, 0, false);
        assert!(state.is_full_file_read());
    }

    #[test]
    fn is_full_file_read_rejects_empty_file_with_one_based_range() {
        let state = sample_state(1, 0, 0, false);
        assert!(!state.is_full_file_read());
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
            .or_insert_with(DashMap::new);
        debug!("Created file read state cache: session_id={}", session_id);
    }

    pub fn delete_session(&self, session_id: &str) {
        self.session_states.remove(session_id);
        debug!("Deleted file read state cache: session_id={}", session_id);
    }

    pub fn clear_session(&self, session_id: &str) {
        if let Some(states) = self.session_states.get(session_id) {
            states.clear();
            debug!("Cleared file read state cache: session_id={}", session_id);
        }
    }

    pub fn set(&self, session_id: &str, logical_path: &str, state: FileReadState) {
        let session_states = self
            .session_states
            .entry(session_id.to_string())
            .or_insert_with(DashMap::new);
        session_states.insert(logical_path.to_string(), state);
    }

    pub fn get(&self, session_id: &str, logical_path: &str) -> Option<FileReadState> {
        self.session_states
            .get(session_id)
            .and_then(|states| states.get(logical_path).map(|entry| entry.clone()))
    }
}
