use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use openbitfun_core::util::elapsed_ms;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStartupTraceEvent {
    pub trace_id: String,
    pub phase: String,
    pub at_ms: u64,
    pub since_process_start_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStartupTraceSnapshot {
    pub trace_id: String,
    pub events: Vec<DesktopStartupTraceEvent>,
}

#[derive(Clone)]
pub struct DesktopStartupTrace {
    trace_id: String,
    started_at: Instant,
    events: Arc<Mutex<Vec<DesktopStartupTraceEvent>>>,
    persistence: Arc<Mutex<Option<StartupTracePersistence>>>,
}

struct StartupTracePersistence {
    path: PathBuf,
    file: File,
}

impl DesktopStartupTrace {
    pub fn new(trace_id: String, started_at: Instant) -> Self {
        Self {
            trace_id,
            started_at,
            events: Arc::new(Mutex::new(Vec::new())),
            persistence: Arc::new(Mutex::new(None)),
        }
    }

    pub fn new_persisted(
        trace_id: String,
        started_at: Instant,
        path: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to create native startup trace directory {}: {}",
                    parent.display(),
                    error
                )
            })?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| {
                format!(
                    "Failed to open native startup trace {}: {}",
                    path.display(),
                    error
                )
            })?;
        Ok(Self {
            trace_id,
            started_at,
            events: Arc::new(Mutex::new(Vec::new())),
            persistence: Arc::new(Mutex::new(Some(StartupTracePersistence { path, file }))),
        })
    }

    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    pub fn record_phase(&self, phase: impl Into<String>, category: impl Into<String>) {
        self.record_event(phase.into(), Some(category.into()), None, None, None, None);
    }

    pub fn record_step(
        &self,
        phase: impl Into<String>,
        category: impl Into<String>,
        step: impl Into<String>,
        duration_ms: u128,
    ) {
        self.record_event(
            phase.into(),
            Some(category.into()),
            Some(step.into()),
            None,
            None,
            Some(duration_ms),
        );
    }

    pub fn record_elapsed_step(
        &self,
        category: impl Into<String>,
        step: impl Into<String>,
        started_at: Instant,
    ) -> u128 {
        let duration_ms = elapsed_ms(started_at);
        self.record_step("native_step_end", category, step, duration_ms);
        duration_ms
    }

    pub fn record_tauri_command_elapsed(
        &self,
        command: impl Into<String>,
        target: Option<&str>,
        started_at: Instant,
    ) -> u128 {
        let duration_ms = elapsed_ms(started_at);
        let command = command.into();
        self.record_event(
            "native_step_end".to_string(),
            Some("tauri_command".to_string()),
            Some(command.clone()),
            Some(command),
            target.map(ToOwned::to_owned),
            Some(duration_ms),
        );
        duration_ms
    }

    pub fn snapshot(&self) -> DesktopStartupTraceSnapshot {
        let events = self
            .events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default();
        DesktopStartupTraceSnapshot {
            trace_id: self.trace_id.clone(),
            events,
        }
    }

    pub fn record_logging_ready_and_stop_persistence(&self) {
        self.record_phase("logging_ready", "native_logging");
        if let Ok(mut persistence) = self.persistence.lock() {
            *persistence = None;
        }
    }

    fn record_event(
        &self,
        phase: String,
        category: Option<String>,
        step: Option<String>,
        command: Option<String>,
        target: Option<String>,
        duration_ms: Option<u128>,
    ) {
        let since_process_start_ms = to_u64(elapsed_ms(self.started_at));
        let duration_ms = duration_ms.map(to_u64);
        let event = DesktopStartupTraceEvent {
            trace_id: self.trace_id.clone(),
            phase,
            at_ms: since_process_start_ms,
            since_process_start_ms,
            category,
            step,
            command,
            target,
            duration_ms,
        };

        if let Ok(mut events) = self.events.lock() {
            events.push(event.clone());
        }
        self.persist_event(&event);
    }

    fn persist_event(&self, event: &DesktopStartupTraceEvent) {
        let serialized = match serde_json::to_vec(event) {
            Ok(serialized) => serialized,
            Err(error) => {
                log::warn!("Failed to serialize native startup trace event: {}", error);
                return;
            }
        };
        let failure = {
            let Ok(mut persistence_guard) = self.persistence.lock() else {
                log::warn!("Native startup trace writer lock is poisoned");
                return;
            };
            let Some(persistence) = persistence_guard.as_mut() else {
                return;
            };
            let result = persistence
                .file
                .write_all(&serialized)
                .and_then(|_| persistence.file.write_all(b"\n"))
                .and_then(|_| persistence.file.flush());
            match result {
                Ok(()) => None,
                Err(error) => {
                    let path = persistence.path.clone();
                    *persistence_guard = None;
                    Some((path, error))
                }
            }
        };
        if let Some((path, error)) = failure {
            log::warn!(
                "Failed to persist native startup trace, disabling writer: path={}, error={}",
                path.display(),
                error
            );
        }
    }
}

fn to_u64(value: u128) -> u64 {
    value.min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn records_tauri_command_with_safe_target() {
        let trace = DesktopStartupTrace::new(
            "trace-test".to_string(),
            Instant::now() - Duration::from_millis(20),
        );

        trace.record_tauri_command_elapsed(
            "get_config",
            Some("app.auto_update"),
            Instant::now() - Duration::from_millis(7),
        );

        let snapshot = trace.snapshot();
        let event = snapshot.events.last().expect("event should be recorded");

        assert_eq!(event.category.as_deref(), Some("tauri_command"));
        assert_eq!(event.command.as_deref(), Some("get_config"));
        assert_eq!(event.target.as_deref(), Some("app.auto_update"));
        assert!(event.duration_ms.unwrap_or_default() >= 7);
    }

    #[test]
    fn persists_each_startup_event_as_flushed_json_line() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let path = temp_dir.path().join("native-startup-trace.jsonl");
        let trace = DesktopStartupTrace::new_persisted(
            "trace-persisted".to_string(),
            Instant::now(),
            &path,
        )
        .expect("create persisted trace");

        trace.record_step("native_step_end", "native_pre_tauri", "load_config", 12);

        let content = std::fs::read_to_string(path).expect("read persisted trace");
        let event: serde_json::Value =
            serde_json::from_str(content.trim()).expect("parse JSON line");
        assert_eq!(event["traceId"], "trace-persisted");
        assert_eq!(event["step"], "load_config");
        assert_eq!(event["durationMs"], 12);
    }
}
