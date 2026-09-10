//! Stable on-disk envelope for in-flight Session runtime events.

use crate::storage_error::{StorageError as OpenBitFunError, StorageResult as OpenBitFunResult};
use openbitfun_events::AgenticEvent;
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggedEvent {
    pub stream_id: String,
    pub cursor: u64,
    pub event: AgenticEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEventLogSummary {
    pub stream_id: String,
    pub event_count: usize,
    pub turn_ids: BTreeSet<String>,
}

/// Validate an imported JSONL file through the same envelope and durable-prefix
/// semantics used by the runtime store. Migration keeps the original bytes and
/// ordering; a torn tail is retained but is not considered part of the readable
/// event prefix.
pub fn validate_runtime_event_log(
    path: &Path,
    expected_session_id: &str,
) -> OpenBitFunResult<RuntimeEventLogSummary> {
    let file = std::fs::File::open(path).map_err(|error| {
        OpenBitFunError::io(format!(
            "Failed to open runtime event log {}: {error}",
            path.display()
        ))
    })?;
    let mut stream_id: Option<String> = None;
    let mut event_count = 0usize;
    let mut turn_ids = BTreeSet::new();
    for (line_index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|error| {
            OpenBitFunError::io(format!(
                "Failed to read runtime event log {} at line {}: {error}",
                path.display(),
                line_index + 1
            ))
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<LoggedEvent>(&line) else {
            break;
        };
        if let Some(session_id) = record.event.session_id() {
            if session_id != expected_session_id {
                return Err(OpenBitFunError::validation(format!(
                    "Runtime event log {} contains a different Session id",
                    path.display()
                )));
            }
        }
        if let Some(turn_id) = record.event.turn_id() {
            turn_ids.insert(turn_id.to_string());
        }
        match stream_id.as_deref() {
            Some(current) if current != record.stream_id => {
                turn_ids.clear();
                event_count = 0;
                stream_id = Some(record.stream_id);
            }
            None => stream_id = Some(record.stream_id),
            _ => {}
        }
        event_count = event_count.saturating_add(1);
    }
    let stream_id = stream_id.ok_or_else(|| {
        OpenBitFunError::validation(format!(
            "Runtime event log {} contains no events",
            path.display()
        ))
    })?;
    Ok(RuntimeEventLogSummary {
        stream_id,
        event_count,
        turn_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn imported_log_keeps_the_runtime_readers_torn_tail_semantics() {
        let root = std::env::var_os("OPENBITFUN_TEST_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&root).unwrap();
        let temp = tempfile::Builder::new()
            .prefix("openbitfun-runtime-event-")
            .tempdir_in(root)
            .unwrap();
        let path = temp.path().join("session-1.jsonl");
        fs::write(
            &path,
            concat!(
                "{\"streamId\":\"stream-1\",\"cursor\":1,\"event\":{\"type\":\"TextChunk\",\"session_id\":\"session-1\",\"turn_id\":\"turn-1\",\"round_id\":\"round-1\",\"text\":\"durable\"}}\n",
                "{\"streamId\":\"stream-1\",\"cursor\":"
            ),
        )
        .unwrap();

        let summary = validate_runtime_event_log(&path, "session-1").unwrap();
        assert_eq!(summary.stream_id, "stream-1");
        assert_eq!(summary.event_count, 1);
        assert_eq!(summary.turn_ids, BTreeSet::from(["turn-1".to_string()]));
    }
}
