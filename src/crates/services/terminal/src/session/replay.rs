//! Terminal replay history.
//!
//! This stores enough PTY output context for frontend recovery without trying
//! to serialize an xterm.js buffer.  Each data chunk is tagged with the PTY
//! dimensions that were active when the backend received it.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// One replay step for rebuilding a frontend terminal instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalReplayEvent {
    /// Terminal columns active for this replay step.
    pub cols: u16,
    /// Terminal rows active for this replay step.
    pub rows: u16,
    /// Raw terminal data to write after applying the dimensions.
    #[serde(default)]
    pub data: String,
}

impl TerminalReplayEvent {
    pub fn resize_marker(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            data: String::new(),
        }
    }

    pub fn data(cols: u16, rows: u16, data: String) -> Self {
        Self { cols, rows, data }
    }

    fn is_resize_marker(&self) -> bool {
        self.data.is_empty()
    }
}

/// Bounded replay history for one terminal session.
#[derive(Debug, Clone)]
pub struct TerminalReplayHistory {
    events: VecDeque<TerminalReplayEvent>,
    max_bytes: usize,
    max_events: usize,
}

impl Default for TerminalReplayHistory {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
            max_bytes: Self::DEFAULT_MAX_BYTES,
            max_events: Self::DEFAULT_MAX_EVENTS,
        }
    }
}

impl TerminalReplayHistory {
    /// Default maximum output payload retained for replay: 100KB.
    pub const DEFAULT_MAX_BYTES: usize = 100 * 1024;
    /// Maximum event count retained for replay.
    pub const DEFAULT_MAX_EVENTS: usize = 2_000;

    pub fn record_output(&mut self, cols: u16, rows: u16, data: &str) {
        if data.is_empty() {
            return;
        }

        // Keep contiguous output with identical PTY dimensions in one event so
        // replay applies geometry only at real resize boundaries.
        match self.events.back_mut() {
            Some(last) if last.cols == cols && last.rows == rows => {
                last.data.push_str(data);
            }
            _ => self
                .events
                .push_back(TerminalReplayEvent::data(cols, rows, data.to_string())),
        }

        self.trim();
    }

    pub fn record_resize(&mut self, cols: u16, rows: u16) {
        // A resize marker carries no data; it exists only so the frontend can
        // apply the new geometry before the next output chunk.
        match self.events.back_mut() {
            Some(last) if last.cols == cols && last.rows == rows => {}
            Some(last) if last.is_resize_marker() => {
                last.cols = cols;
                last.rows = rows;
            }
            _ => self
                .events
                .push_back(TerminalReplayEvent::resize_marker(cols, rows)),
        }

        self.trim();
    }

    pub fn replace_events(&mut self, events: Vec<TerminalReplayEvent>) {
        self.events = events.into();
        self.trim();
    }

    pub fn events(&self) -> Vec<TerminalReplayEvent> {
        self.events.iter().cloned().collect()
    }

    pub fn data(&self) -> String {
        self.events
            .iter()
            .map(|event| event.data.as_str())
            .collect()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn size_bytes(&self) -> usize {
        self.events.iter().map(|event| event.data.len()).sum()
    }

    fn trim(&mut self) {
        while self.events.len() > self.max_events {
            self.events.pop_front();
        }

        let mut total_size = self.size_bytes();
        while total_size > self.max_bytes && !self.events.is_empty() {
            if let Some(oldest) = self.events.pop_front() {
                total_size = total_size.saturating_sub(oldest.data.len());
            }
        }

        while self.events.len() > 1
            && self
                .events
                .front()
                .map(TerminalReplayEvent::is_resize_marker)
                .unwrap_or(false)
        {
            // Dropping old output can leave a leading resize-only event. Remove
            // it so a restored terminal does not treat pure geometry as content.
            self.events.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesces_output_with_matching_dimensions() {
        let mut history = TerminalReplayHistory::default();

        history.record_output(80, 24, "hello");
        history.record_output(80, 24, " world");

        assert_eq!(
            history.events(),
            vec![TerminalReplayEvent::data(80, 24, "hello world".to_string())]
        );
    }

    #[test]
    fn keeps_dimension_changes_in_order() {
        let mut history = TerminalReplayHistory::default();

        history.record_output(80, 24, "a");
        history.record_resize(100, 30);
        history.record_output(100, 30, "b");

        assert_eq!(
            history.events(),
            vec![
                TerminalReplayEvent::data(80, 24, "a".to_string()),
                TerminalReplayEvent::data(100, 30, "b".to_string()),
            ]
        );
    }

    #[test]
    fn coalesces_consecutive_resize_markers() {
        let mut history = TerminalReplayHistory::default();

        history.record_output(80, 24, "a");
        history.record_resize(90, 25);
        history.record_resize(100, 30);

        assert_eq!(
            history.events(),
            vec![
                TerminalReplayEvent::data(80, 24, "a".to_string()),
                TerminalReplayEvent::resize_marker(100, 30),
            ]
        );
    }
}
