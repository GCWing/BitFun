use crate::http::{StreamDescriptor, MAX_STREAM_CHUNK_BYTES};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{Mutex, Notify};

const DEFAULT_MAX_ACTIVE_STREAMS: usize = 128;
const DEFAULT_MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StreamReadParams {
    #[serde(rename = "instanceID")]
    pub instance_id: String,
    #[serde(rename = "streamID")]
    pub stream_id: String,
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StreamReadResult {
    pub data: String,
    pub eof: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StreamCancelParams {
    #[serde(rename = "instanceID")]
    pub instance_id: String,
    #[serde(rename = "streamID")]
    pub stream_id: String,
    #[serde(rename = "reason")]
    pub _reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct StreamCancelResult {
    pub cancelled: bool,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum StreamRegistryError {
    #[error("response stream registry capacity was reached")]
    Capacity,
    #[error("response stream body exceeds the registry byte limit")]
    BodyTooLarge,
    #[error("maxBytes must be between 1 and {MAX_STREAM_CHUNK_BYTES}")]
    InvalidMaxBytes,
    #[error("response stream belongs to a different plugin instance")]
    InstanceMismatch,
}

#[derive(Clone)]
pub(crate) struct PluginHostStreamRegistry {
    state: Arc<Mutex<StreamRegistryState>>,
    sequence: Arc<AtomicU64>,
    changed: Arc<Notify>,
    max_active_streams: usize,
    max_total_bytes: usize,
}

struct StreamRegistryState {
    streams: HashMap<String, ResponseStream>,
    total_bytes: usize,
}

struct ResponseStream {
    instance_id: String,
    bytes: Vec<u8>,
    offset: usize,
}

impl Default for PluginHostStreamRegistry {
    fn default() -> Self {
        Self::with_limits(DEFAULT_MAX_ACTIVE_STREAMS, DEFAULT_MAX_TOTAL_BYTES)
    }
}

impl PluginHostStreamRegistry {
    pub(crate) fn with_limits(max_active_streams: usize, max_total_bytes: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(StreamRegistryState {
                streams: HashMap::new(),
                total_bytes: 0,
            })),
            sequence: Arc::new(AtomicU64::new(0)),
            changed: Arc::new(Notify::new()),
            max_active_streams,
            max_total_bytes,
        }
    }

    pub(crate) async fn add(
        &self,
        instance_id: &str,
        bytes: Vec<u8>,
    ) -> Result<StreamDescriptor, StreamRegistryError> {
        let mut state = self.state.lock().await;
        if state.streams.len() >= self.max_active_streams {
            return Err(StreamRegistryError::Capacity);
        }
        if bytes.len() > self.max_total_bytes
            || state.total_bytes.saturating_add(bytes.len()) > self.max_total_bytes
        {
            return Err(StreamRegistryError::BodyTooLarge);
        }
        let length = bytes.len();
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let stream_id = format!("backend-response-stream:{sequence}");
        state.total_bytes += length;
        state.streams.insert(
            stream_id.clone(),
            ResponseStream {
                instance_id: instance_id.to_string(),
                bytes,
                offset: 0,
            },
        );
        Ok(StreamDescriptor {
            stream_id,
            length: Some(length),
        })
    }

    pub(crate) async fn read(
        &self,
        params: StreamReadParams,
    ) -> Result<StreamReadResult, StreamRegistryError> {
        let max_bytes = params.max_bytes.unwrap_or(MAX_STREAM_CHUNK_BYTES);
        if !(1..=MAX_STREAM_CHUNK_BYTES).contains(&max_bytes) {
            return Err(StreamRegistryError::InvalidMaxBytes);
        }
        let mut state = self.state.lock().await;
        let Some(stream) = state.streams.get_mut(&params.stream_id) else {
            return Ok(StreamReadResult {
                data: String::new(),
                eof: true,
            });
        };
        if stream.instance_id != params.instance_id {
            return Err(StreamRegistryError::InstanceMismatch);
        }
        let end = stream
            .offset
            .saturating_add(max_bytes)
            .min(stream.bytes.len());
        let data = BASE64_STANDARD.encode(&stream.bytes[stream.offset..end]);
        stream.offset = end;
        let eof = stream.offset == stream.bytes.len();
        if eof {
            let removed = state
                .streams
                .remove(&params.stream_id)
                .expect("stream exists");
            state.total_bytes = state.total_bytes.saturating_sub(removed.bytes.len());
            self.changed.notify_waiters();
        }
        Ok(StreamReadResult { data, eof })
    }

    pub(crate) async fn cancel(
        &self,
        params: StreamCancelParams,
    ) -> Result<StreamCancelResult, StreamRegistryError> {
        let mut state = self.state.lock().await;
        let Some(stream) = state.streams.get(&params.stream_id) else {
            return Ok(StreamCancelResult { cancelled: false });
        };
        if stream.instance_id != params.instance_id {
            return Err(StreamRegistryError::InstanceMismatch);
        }
        let removed = state
            .streams
            .remove(&params.stream_id)
            .expect("stream exists");
        state.total_bytes = state.total_bytes.saturating_sub(removed.bytes.len());
        self.changed.notify_waiters();
        Ok(StreamCancelResult { cancelled: true })
    }

    pub(crate) async fn cancel_instance(&self, instance_id: &str) -> usize {
        let mut state = self.state.lock().await;
        let stream_ids = state
            .streams
            .iter()
            .filter(|(_, stream)| stream.instance_id == instance_id)
            .map(|(stream_id, _)| stream_id.clone())
            .collect::<Vec<_>>();
        for stream_id in &stream_ids {
            if let Some(stream) = state.streams.remove(stream_id) {
                state.total_bytes = state.total_bytes.saturating_sub(stream.bytes.len());
            }
        }
        if !stream_ids.is_empty() {
            self.changed.notify_waiters();
        }
        stream_ids.len()
    }

    pub(crate) async fn cancel_all(&self) -> usize {
        let mut state = self.state.lock().await;
        let count = state.streams.len();
        state.streams.clear();
        state.total_bytes = 0;
        if count > 0 {
            self.changed.notify_waiters();
        }
        count
    }

    pub(crate) async fn active_count(&self) -> usize {
        self.state.lock().await.streams.len()
    }

    pub(crate) async fn wait_until_empty(&self, timeout: Duration) -> bool {
        let wait = async {
            loop {
                let changed = self.changed.notified();
                if self.active_count().await == 0 {
                    return;
                }
                changed.await;
            }
        };
        tokio::time::timeout(timeout, wait).await.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PluginHostStreamRegistry, StreamCancelParams, StreamReadParams, StreamRegistryError,
    };
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use base64::Engine;

    #[tokio::test]
    async fn reads_chunks_and_removes_stream_at_eof() {
        let registry = PluginHostStreamRegistry::with_limits(2, 32);
        let descriptor = registry
            .add("instance:1", b"abcdef".to_vec())
            .await
            .expect("add stream");
        let first = registry
            .read(StreamReadParams {
                instance_id: "instance:1".to_string(),
                stream_id: descriptor.stream_id.clone(),
                max_bytes: Some(2),
            })
            .await
            .expect("first chunk");
        assert_eq!(BASE64_STANDARD.decode(first.data).unwrap(), b"ab");
        assert!(!first.eof);
        let second = registry
            .read(StreamReadParams {
                instance_id: "instance:1".to_string(),
                stream_id: descriptor.stream_id.clone(),
                max_bytes: Some(8),
            })
            .await
            .expect("second chunk");
        assert_eq!(BASE64_STANDARD.decode(second.data).unwrap(), b"cdef");
        assert!(second.eof);
        assert_eq!(registry.active_count().await, 0);
    }

    #[tokio::test]
    async fn enforces_instance_ownership_and_cancel() {
        let registry = PluginHostStreamRegistry::default();
        let descriptor = registry
            .add("instance:1", b"body".to_vec())
            .await
            .expect("add stream");
        assert_eq!(
            registry
                .read(StreamReadParams {
                    instance_id: "instance:2".to_string(),
                    stream_id: descriptor.stream_id.clone(),
                    max_bytes: None,
                })
                .await,
            Err(StreamRegistryError::InstanceMismatch)
        );
        assert!(
            registry
                .cancel(StreamCancelParams {
                    instance_id: "instance:1".to_string(),
                    stream_id: descriptor.stream_id,
                    _reason: Some("test".to_string()),
                })
                .await
                .expect("cancel")
                .cancelled
        );
    }

    #[tokio::test]
    async fn waits_for_response_streams_to_drain() {
        let registry = PluginHostStreamRegistry::default();
        let descriptor = registry
            .add("instance:1", b"body".to_vec())
            .await
            .expect("add stream");
        let waiter = {
            let registry = registry.clone();
            tokio::spawn(async move {
                registry
                    .wait_until_empty(std::time::Duration::from_secs(1))
                    .await
            })
        };
        registry
            .cancel(StreamCancelParams {
                instance_id: "instance:1".to_string(),
                stream_id: descriptor.stream_id,
                _reason: Some("test".to_string()),
            })
            .await
            .expect("cancel");
        assert!(waiter.await.expect("wait task"));
    }
}
