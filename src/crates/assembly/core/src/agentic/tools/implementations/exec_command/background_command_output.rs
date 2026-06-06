use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, Mutex};
use tokio::time::Instant;

pub const BACKGROUND_COMMAND_OUTPUT_CAPTURE_LIMIT_BYTES: usize = 1024 * 1024;
const BACKGROUND_COMMAND_OUTPUT_COMPLETED_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_BACKGROUND_COMMAND_OUTPUT_RECORDS: usize = 128;

static BACKGROUND_COMMAND_OUTPUT_CAPTURE: OnceLock<Arc<BackgroundCommandOutputCapture>> =
    OnceLock::new();

pub fn background_command_output_capture() -> Arc<BackgroundCommandOutputCapture> {
    BACKGROUND_COMMAND_OUTPUT_CAPTURE
        .get_or_init(|| Arc::new(BackgroundCommandOutputCapture::default()))
        .clone()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundCommandOutputStatus {
    Running,
    Exited,
    Interrupted,
    Killed,
    Pruned,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BackgroundCommandSessionKey {
    remote: bool,
    exec_session_id: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundCommandOutputMetadata {
    pub agent_session_id: Option<String>,
    pub exec_session_id: Option<i32>,
    pub command: String,
    pub workdir: Option<String>,
    pub remote: bool,
    pub tty: bool,
    pub status: BackgroundCommandOutputStatus,
    pub exit_code: Option<i32>,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub retained_bytes: usize,
    pub retained_limit_bytes: usize,
    pub truncated_from_start: bool,
}

#[derive(Debug, Clone)]
pub struct StartBackgroundCommandOutputCapture {
    /// Private backend key used before the exec session id is known.
    pub capture_id: String,
    pub agent_session_id: Option<String>,
    pub command: String,
    pub workdir: Option<String>,
    pub remote: bool,
    pub tty: bool,
}

#[derive(Debug, Clone)]
pub struct ReadBackgroundCommandOutputRequest {
    pub exec_session_id: i32,
    pub remote: bool,
    pub cursor: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ListBackgroundCommandOutputRequest {
    pub agent_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadBackgroundCommandOutputResponse {
    pub metadata: BackgroundCommandOutputMetadata,
    pub cursor: u64,
    pub reset: bool,
    pub snapshot: Option<String>,
    pub chunks: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListBackgroundCommandOutputResponse {
    pub activities: Vec<BackgroundCommandOutputMetadata>,
}

#[derive(Default)]
pub struct BackgroundCommandOutputCapture {
    records: Mutex<HashMap<String, BackgroundCommandOutputRecord>>,
    session_index: Mutex<HashMap<BackgroundCommandSessionKey, String>>,
}

struct BackgroundCommandOutputRecord {
    agent_session_id: Option<String>,
    command: String,
    workdir: Option<String>,
    remote: bool,
    tty: bool,
    exec_session_id: Option<i32>,
    status: BackgroundCommandOutputStatus,
    exit_code: Option<i32>,
    started_at: u64,
    ended_at: Option<u64>,
    completed_at: Option<Instant>,
    chunks: VecDeque<BackgroundCommandOutputChunk>,
    next_cursor: u64,
    retained_bytes: usize,
    truncated_from_start: bool,
}

struct BackgroundCommandOutputChunk {
    cursor: u64,
    text: String,
    bytes: usize,
}

impl BackgroundCommandOutputCapture {
    pub async fn start_capture(
        self: &Arc<Self>,
        request: StartBackgroundCommandOutputCapture,
    ) -> mpsc::UnboundedSender<String> {
        self.cleanup_expired().await;
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let record = BackgroundCommandOutputRecord {
            agent_session_id: request.agent_session_id,
            command: request.command,
            workdir: request.workdir,
            remote: request.remote,
            tty: request.tty,
            exec_session_id: None,
            status: BackgroundCommandOutputStatus::Running,
            exit_code: None,
            started_at: now_unix_seconds(),
            ended_at: None,
            completed_at: None,
            chunks: VecDeque::new(),
            next_cursor: 0,
            retained_bytes: 0,
            truncated_from_start: false,
        };
        {
            let mut records = self.records.lock().await;
            records.insert(request.capture_id.clone(), record);
            prune_record_count(&mut records);
        }

        let capture = Arc::clone(self);
        tokio::spawn(async move {
            while let Some(chunk) = rx.recv().await {
                capture.append_chunk(&request.capture_id, chunk).await;
            }
        });

        tx
    }

    pub async fn set_session_id(&self, capture_id: &str, exec_session_id: Option<i32>) {
        let mut records = self.records.lock().await;
        let Some(record) = records.get_mut(capture_id) else {
            return;
        };

        let old_key = record
            .exec_session_id
            .map(|id| BackgroundCommandSessionKey {
                remote: record.remote,
                exec_session_id: id,
            });
        record.exec_session_id = exec_session_id;
        let new_key = record
            .exec_session_id
            .map(|id| BackgroundCommandSessionKey {
                remote: record.remote,
                exec_session_id: id,
            });
        drop(records);

        let mut session_index = self.session_index.lock().await;
        if let Some(old_key) = old_key {
            session_index.remove(&old_key);
        }
        if let Some(new_key) = new_key {
            session_index.insert(new_key, capture_id.to_string());
        }
    }

    pub async fn update_lifecycle(
        &self,
        capture_id: &str,
        exec_session_id: i32,
        status: BackgroundCommandOutputStatus,
        exit_code: Option<i32>,
    ) -> Option<BackgroundCommandOutputMetadata> {
        let mut records = self.records.lock().await;
        let record = records.get_mut(capture_id)?;
        record.exec_session_id = Some(exec_session_id);
        record.status = status;
        record.exit_code = exit_code;
        if status != BackgroundCommandOutputStatus::Running {
            if record.ended_at.is_none() {
                record.ended_at = Some(now_unix_seconds());
            }
            if record.completed_at.is_none() {
                record.completed_at = Some(Instant::now());
            }
        }
        let metadata = record.metadata();
        let key = BackgroundCommandSessionKey {
            remote: record.remote,
            exec_session_id,
        };
        drop(records);

        self.session_index
            .lock()
            .await
            .insert(key, capture_id.to_string());

        Some(metadata)
    }

    pub async fn finish(
        &self,
        capture_id: &str,
        status: BackgroundCommandOutputStatus,
        exit_code: Option<i32>,
    ) -> Option<BackgroundCommandOutputMetadata> {
        let mut records = self.records.lock().await;
        let record = records.get_mut(capture_id)?;
        finish_record(record, status, exit_code);
        Some(record.metadata())
    }

    pub async fn finish_by_session(
        &self,
        remote: bool,
        exec_session_id: i32,
        status: BackgroundCommandOutputStatus,
        exit_code: Option<i32>,
    ) -> Option<BackgroundCommandOutputMetadata> {
        let capture_id = self.capture_id_for_session(remote, exec_session_id).await?;
        self.finish(&capture_id, status, exit_code).await
    }

    pub async fn read(
        &self,
        request: ReadBackgroundCommandOutputRequest,
    ) -> Option<ReadBackgroundCommandOutputResponse> {
        self.cleanup_expired_except_session(request.remote, request.exec_session_id)
            .await;

        let capture_id = self
            .capture_id_for_session(request.remote, request.exec_session_id)
            .await?;
        let records = self.records.lock().await;
        let record = records.get(&capture_id)?;
        let first_cursor = record.chunks.front().map(|chunk| chunk.cursor);
        let should_reset = request
            .cursor
            .zip(first_cursor)
            .is_some_and(|(cursor, first)| cursor < first);
        let snapshot = if request.cursor.is_none() || should_reset {
            Some(record.snapshot())
        } else {
            None
        };
        let chunks = if let Some(cursor) = request.cursor.filter(|_| !should_reset) {
            record
                .chunks
                .iter()
                .filter(|chunk| chunk.cursor >= cursor)
                .map(|chunk| chunk.text.clone())
                .collect()
        } else {
            Vec::new()
        };

        Some(ReadBackgroundCommandOutputResponse {
            metadata: record.metadata(),
            cursor: record.next_cursor,
            reset: should_reset,
            snapshot,
            chunks,
        })
    }

    pub async fn list(
        &self,
        request: ListBackgroundCommandOutputRequest,
    ) -> ListBackgroundCommandOutputResponse {
        self.cleanup_expired().await;

        let records = self.records.lock().await;
        let mut activities = records
            .values()
            .filter(|record| {
                record.exec_session_id.is_some()
                    && request.agent_session_id.as_ref().is_none_or(|session_id| {
                        record.agent_session_id.as_deref() == Some(session_id.as_str())
                    })
            })
            .map(BackgroundCommandOutputRecord::metadata)
            .collect::<Vec<_>>();
        activities.sort_by_key(|metadata| (metadata.started_at, metadata.exec_session_id));

        ListBackgroundCommandOutputResponse { activities }
    }

    async fn capture_id_for_session(&self, remote: bool, exec_session_id: i32) -> Option<String> {
        self.session_index
            .lock()
            .await
            .get(&BackgroundCommandSessionKey {
                remote,
                exec_session_id,
            })
            .cloned()
    }

    async fn append_chunk(&self, capture_id: &str, text: String) {
        if text.is_empty() {
            return;
        }

        let mut records = self.records.lock().await;
        let Some(record) = records.get_mut(capture_id) else {
            return;
        };
        let bytes = text.len();
        record.chunks.push_back(BackgroundCommandOutputChunk {
            cursor: record.next_cursor,
            text,
            bytes,
        });
        record.next_cursor = record.next_cursor.saturating_add(1);
        record.retained_bytes = record.retained_bytes.saturating_add(bytes);
        while record.retained_bytes > BACKGROUND_COMMAND_OUTPUT_CAPTURE_LIMIT_BYTES {
            if let Some(dropped) = record.chunks.pop_front() {
                record.retained_bytes = record.retained_bytes.saturating_sub(dropped.bytes);
                record.truncated_from_start = true;
            } else {
                break;
            }
        }
    }

    async fn cleanup_expired(&self) {
        self.cleanup_expired_except_capture("").await;
    }

    async fn cleanup_expired_except_session(&self, remote: bool, exec_session_id: i32) {
        if let Some(capture_id) = self.capture_id_for_session(remote, exec_session_id).await {
            self.cleanup_expired_except_capture(&capture_id).await;
        } else {
            self.cleanup_expired().await;
        }
    }

    async fn cleanup_expired_except_capture(&self, keep_capture_id: &str) {
        let now = Instant::now();
        let mut removed_keys = Vec::new();
        {
            let mut records = self.records.lock().await;
            records.retain(|capture_id, record| {
                if capture_id == keep_capture_id {
                    return true;
                }
                let keep = record.completed_at.is_none_or(|completed_at| {
                    now.duration_since(completed_at) <= BACKGROUND_COMMAND_OUTPUT_COMPLETED_TTL
                });
                if !keep {
                    if let Some(exec_session_id) = record.exec_session_id {
                        removed_keys.push(BackgroundCommandSessionKey {
                            remote: record.remote,
                            exec_session_id,
                        });
                    }
                }
                keep
            });
        }

        if !removed_keys.is_empty() {
            let mut session_index = self.session_index.lock().await;
            for key in removed_keys {
                session_index.remove(&key);
            }
        }
    }
}

impl BackgroundCommandOutputRecord {
    fn snapshot(&self) -> String {
        self.chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<String>()
    }

    fn metadata(&self) -> BackgroundCommandOutputMetadata {
        BackgroundCommandOutputMetadata {
            agent_session_id: self.agent_session_id.clone(),
            exec_session_id: self.exec_session_id,
            command: self.command.clone(),
            workdir: self.workdir.clone(),
            remote: self.remote,
            tty: self.tty,
            status: self.status,
            exit_code: self.exit_code,
            started_at: self.started_at,
            ended_at: self.ended_at,
            retained_bytes: self.retained_bytes,
            retained_limit_bytes: BACKGROUND_COMMAND_OUTPUT_CAPTURE_LIMIT_BYTES,
            truncated_from_start: self.truncated_from_start,
        }
    }
}

fn finish_record(
    record: &mut BackgroundCommandOutputRecord,
    status: BackgroundCommandOutputStatus,
    exit_code: Option<i32>,
) {
    record.status = status;
    record.exit_code = exit_code;
    if record.ended_at.is_none() {
        record.ended_at = Some(now_unix_seconds());
    }
    if record.completed_at.is_none() {
        record.completed_at = Some(Instant::now());
    }
}

fn prune_record_count(records: &mut HashMap<String, BackgroundCommandOutputRecord>) {
    while records.len() > MAX_BACKGROUND_COMMAND_OUTPUT_RECORDS {
        let Some(oldest_capture_id) = records
            .iter()
            .min_by_key(|(_, record)| (record.completed_at.is_none(), record.started_at))
            .map(|(capture_id, _)| capture_id.clone())
        else {
            break;
        };
        records.remove(&oldest_capture_id);
    }
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{
        background_command_output_capture, BackgroundCommandOutputStatus,
        ReadBackgroundCommandOutputRequest, StartBackgroundCommandOutputCapture,
    };

    #[tokio::test]
    async fn background_command_output_reads_snapshot_then_incremental_chunks() {
        let capture_id = format!("test-capture-{}", uuid::Uuid::new_v4());
        let capture = background_command_output_capture();
        let tx = capture
            .start_capture(StartBackgroundCommandOutputCapture {
                capture_id: capture_id.clone(),
                agent_session_id: Some("agent-session-1".to_string()),
                command: "echo hi".to_string(),
                workdir: None,
                remote: false,
                tty: false,
            })
            .await;

        capture
            .update_lifecycle(
                &capture_id,
                4242,
                BackgroundCommandOutputStatus::Running,
                None,
            )
            .await
            .expect("record exists");
        tx.send("hello".to_string())
            .expect("capture receiver alive");
        tx.send(" world".to_string())
            .expect("capture receiver alive");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let first = capture
            .read(ReadBackgroundCommandOutputRequest {
                exec_session_id: 4242,
                remote: false,
                cursor: None,
            })
            .await
            .expect("record exists");
        assert_eq!(first.snapshot.as_deref(), Some("hello world"));
        assert_eq!(first.cursor, 2);

        tx.send("!".to_string()).expect("capture receiver alive");
        capture
            .finish_by_session(false, 4242, BackgroundCommandOutputStatus::Exited, Some(0))
            .await
            .expect("record exists");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let second = capture
            .read(ReadBackgroundCommandOutputRequest {
                exec_session_id: 4242,
                remote: false,
                cursor: Some(first.cursor),
            })
            .await
            .expect("record exists");
        assert_eq!(second.snapshot, None);
        assert_eq!(second.chunks, vec!["!"]);
        assert_eq!(
            second.metadata.status,
            BackgroundCommandOutputStatus::Exited
        );
    }
}
