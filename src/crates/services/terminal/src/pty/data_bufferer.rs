//! Data Bufferer - Reduces message frequency to frontend
//!
//! This module implements data buffering to reduce the number of messages
//! sent to the frontend, improving performance.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, RwLock};

use crate::config::BufferingConfig;

/// Buffered data ready to be sent
#[derive(Debug, Clone)]
pub struct BufferedData {
    /// Process ID
    pub process_id: u32,
    /// Accumulated data
    pub data: Vec<u8>,
    /// Timestamp when first data was added to buffer
    pub start_time: Instant,
}

/// Data bufferer that accumulates PTY output before sending
pub struct DataBufferer {
    /// Buffering configuration
    config: BufferingConfig,

    /// Active buffers per process
    buffers: Arc<RwLock<HashMap<u32, ProcessBuffer>>>,

    /// Channel for flushed data
    output_tx: mpsc::Sender<BufferedData>,

    /// Channel receiver for flushed data
    output_rx: Arc<RwLock<mpsc::Receiver<BufferedData>>>,
}

/// Per-process buffer
struct ProcessBuffer {
    /// Accumulated data
    data: Vec<u8>,
    /// When the first byte was added
    start_time: Instant,
    /// Flush timer handle
    flush_scheduled: bool,
}

impl ProcessBuffer {
    fn new() -> Self {
        Self {
            data: Vec::with_capacity(4096),
            start_time: Instant::now(),
            flush_scheduled: false,
        }
    }
}

impl DataBufferer {
    /// Create a new data bufferer
    pub fn new(config: BufferingConfig) -> Self {
        let (output_tx, output_rx) = mpsc::channel(1024);

        Self {
            config,
            buffers: Arc::new(RwLock::new(HashMap::new())),
            output_tx,
            output_rx: Arc::new(RwLock::new(output_rx)),
        }
    }

    /// Start buffering for a process
    pub async fn start_buffering(&self, process_id: u32) {
        let mut buffers = self.buffers.write().await;
        buffers.insert(process_id, ProcessBuffer::new());
    }

    /// Stop buffering for a process
    pub async fn stop_buffering(&self, process_id: u32) {
        // Flush any remaining data
        self.flush_buffer(process_id).await;

        let mut buffers = self.buffers.write().await;
        buffers.remove(&process_id);
    }

    /// Add data to the buffer
    pub async fn buffer_data(&self, process_id: u32, data: &[u8]) {
        if !self.config.enabled {
            // Buffering disabled, send immediately
            let _ = self
                .output_tx
                .send(BufferedData {
                    process_id,
                    data: data.to_vec(),
                    start_time: Instant::now(),
                })
                .await;
            return;
        }

        let should_schedule_flush;
        let should_flush_now;

        {
            let mut buffers = self.buffers.write().await;
            let buffer = buffers.entry(process_id).or_insert_with(ProcessBuffer::new);

            // Reset start time if buffer was empty
            if buffer.data.is_empty() {
                buffer.start_time = Instant::now();
            }

            buffer.data.extend_from_slice(data);

            // Check if we should flush
            should_flush_now = buffer.data.len() >= self.config.max_buffer_size;
            should_schedule_flush = !buffer.flush_scheduled && !should_flush_now;

            if should_schedule_flush {
                buffer.flush_scheduled = true;
            }
        }

        if should_flush_now {
            self.flush_buffer(process_id).await;
        } else if should_schedule_flush {
            // Schedule a flush after the interval
            let buffers = self.buffers.clone();
            let output_tx = self.output_tx.clone();
            let flush_interval = Duration::from_millis(self.config.flush_interval_ms);

            tokio::spawn(async move {
                tokio::time::sleep(flush_interval).await;
                Self::do_flush(process_id, buffers, output_tx).await;
            });
        }
    }

    /// Flush the buffer for a process
    pub async fn flush_buffer(&self, process_id: u32) {
        Self::do_flush(process_id, self.buffers.clone(), self.output_tx.clone()).await;
    }

    /// Internal flush implementation
    async fn do_flush(
        process_id: u32,
        buffers: Arc<RwLock<HashMap<u32, ProcessBuffer>>>,
        output_tx: mpsc::Sender<BufferedData>,
    ) {
        let data = {
            let mut buffers = buffers.write().await;
            if let Some(buffer) = buffers.get_mut(&process_id) {
                if buffer.data.is_empty() {
                    buffer.flush_scheduled = false;
                    return;
                }

                let data = std::mem::take(&mut buffer.data);
                let start_time = buffer.start_time;
                buffer.flush_scheduled = false;

                Some(BufferedData {
                    process_id,
                    data,
                    start_time,
                })
            } else {
                None
            }
        };

        if let Some(buffered_data) = data {
            let _ = output_tx.send(buffered_data).await;
        }
    }

    /// Receive the next batch of buffered data
    pub async fn recv(&self) -> Option<BufferedData> {
        let mut rx = self.output_rx.write().await;
        rx.recv().await
    }

    /// Try to receive buffered data without blocking
    pub async fn try_recv(&self) -> Option<BufferedData> {
        let mut rx = self.output_rx.write().await;
        rx.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_buffering_disabled() {
        let config = BufferingConfig {
            enabled: false,
            flush_interval_ms: 5,
            max_buffer_size: 1024,
        };

        let bufferer = DataBufferer::new(config);
        bufferer.start_buffering(1).await;

        bufferer.buffer_data(1, b"hello").await;

        let data = bufferer
            .recv()
            .await
            .expect("expected buffered data when buffering is disabled");
        assert_eq!(data.process_id, 1);
        assert_eq!(data.data, b"hello");
    }

    #[tokio::test]
    async fn test_buffering_enabled() {
        let config = BufferingConfig {
            enabled: true,
            flush_interval_ms: 10,
            max_buffer_size: 1024,
        };

        let bufferer = DataBufferer::new(config);
        bufferer.start_buffering(1).await;

        bufferer.buffer_data(1, b"hello").await;
        bufferer.buffer_data(1, b" world").await;

        // Wait for flush
        tokio::time::sleep(Duration::from_millis(20)).await;

        let data = bufferer
            .recv()
            .await
            .expect("expected buffered data after flush interval");
        assert_eq!(data.process_id, 1);
        assert_eq!(data.data, b"hello world");
    }

    #[tokio::test]
    async fn test_max_buffer_size_flush() {
        let config = BufferingConfig {
            enabled: true,
            flush_interval_ms: 1000, // Long interval
            max_buffer_size: 10,     // Small buffer
        };

        let bufferer = DataBufferer::new(config);
        bufferer.start_buffering(1).await;

        // This should trigger immediate flush
        bufferer.buffer_data(1, b"0123456789AB").await;

        let data = bufferer
            .recv()
            .await
            .expect("expected immediate flush when max buffer size is exceeded");
        assert_eq!(data.data.len(), 12);
    }
}
