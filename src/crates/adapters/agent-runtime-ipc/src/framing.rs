use crate::RuntimeIpcFrame;
use std::io::Write;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub(crate) const MAX_REQUEST_FRAME_BYTES: usize = 128 * 1024;
pub(crate) const MAX_RESPONSE_FRAME_BYTES: usize = 8 * 1024 * 1024;

pub(crate) async fn write_frame<W>(
    writer: &mut W,
    frame: &RuntimeIpcFrame,
) -> Result<(), RuntimeIpcIoError>
where
    W: AsyncWrite + Unpin,
{
    write_frame_with_limit(writer, frame, MAX_REQUEST_FRAME_BYTES).await
}

pub(crate) async fn write_frame_with_limit<W>(
    writer: &mut W,
    frame: &RuntimeIpcFrame,
    max_bytes: usize,
) -> Result<(), RuntimeIpcIoError>
where
    W: AsyncWrite + Unpin,
{
    let bytes = serialize_frame_with_limit(frame, max_bytes)?;
    writer
        .write_u32(bytes.len() as u32)
        .await
        .map_err(RuntimeIpcIoError::Io)?;
    writer
        .write_all(&bytes)
        .await
        .map_err(RuntimeIpcIoError::Io)?;
    writer.flush().await.map_err(RuntimeIpcIoError::Io)
}

pub(crate) async fn read_frame<R>(reader: &mut R) -> Result<RuntimeIpcFrame, RuntimeIpcIoError>
where
    R: AsyncRead + Unpin,
{
    read_frame_strict_with_limit(reader, MAX_REQUEST_FRAME_BYTES).await
}

pub(crate) async fn read_frame_strict_with_limit<R>(
    reader: &mut R,
    max_bytes: usize,
) -> Result<RuntimeIpcFrame, RuntimeIpcIoError>
where
    R: AsyncRead + Unpin,
{
    RuntimeIpcFrameReader::new(max_bytes)
        .read_strict(reader)
        .await
}

pub(crate) struct RuntimeIpcFrameReader {
    max_bytes: usize,
    buffer: Vec<u8>,
}

impl RuntimeIpcFrameReader {
    pub(crate) fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            buffer: Vec::new(),
        }
    }

    pub(crate) async fn read_strict<R>(
        &mut self,
        reader: &mut R,
    ) -> Result<RuntimeIpcFrame, RuntimeIpcIoError>
    where
        R: AsyncRead + Unpin,
    {
        self.fill(reader, 4).await?;
        let size =
            u32::from_be_bytes(self.buffer[..4].try_into().expect("four-byte header")) as usize;
        if size > self.max_bytes {
            return Err(RuntimeIpcIoError::FrameTooLarge {
                size,
                max_bytes: self.max_bytes,
            });
        }
        self.fill(reader, size + 4).await?;
        let payload = self.buffer.split_off(4);
        self.buffer.clear();
        parse_strict_frame(&payload)
    }

    pub(crate) fn frame_started(&self) -> bool {
        !self.buffer.is_empty()
    }

    pub(crate) async fn wait_for_frame_start<R>(
        &mut self,
        reader: &mut R,
    ) -> Result<(), RuntimeIpcIoError>
    where
        R: AsyncRead + Unpin,
    {
        self.fill(reader, 1).await
    }

    async fn fill<R>(&mut self, reader: &mut R, target: usize) -> Result<(), RuntimeIpcIoError>
    where
        R: AsyncRead + Unpin,
    {
        let mut chunk = [0; 8 * 1024];
        while self.buffer.len() < target {
            let remaining = (target - self.buffer.len()).min(chunk.len());
            match reader
                .read(&mut chunk[..remaining])
                .await
                .map_err(RuntimeIpcIoError::Io)?
            {
                0 => {
                    return Err(RuntimeIpcIoError::Io(
                        std::io::ErrorKind::UnexpectedEof.into(),
                    ))
                }
                read => self.buffer.extend_from_slice(&chunk[..read]),
            }
        }
        Ok(())
    }
}

fn parse_strict_frame(bytes: &[u8]) -> Result<RuntimeIpcFrame, RuntimeIpcIoError> {
    let original = serde_json::from_slice::<serde_json::Value>(bytes)
        .map_err(RuntimeIpcIoError::Deserialize)?;
    let frame = serde_json::from_value(original.clone()).map_err(RuntimeIpcIoError::Deserialize)?;
    let canonical = serde_json::to_value(&frame).map_err(RuntimeIpcIoError::Serialize)?;
    if let Some(path) = first_unknown_field(&original, &canonical, "$".to_string()) {
        return Err(RuntimeIpcIoError::UnknownField { path });
    }
    Ok(frame)
}

pub(crate) fn serialize_frame_with_limit(
    frame: &RuntimeIpcFrame,
    max_bytes: usize,
) -> Result<Vec<u8>, RuntimeIpcIoError> {
    let mut writer = CappedWriter::new(max_bytes);
    let result = serde_json::to_writer(&mut writer, frame);
    if writer.overflowed {
        return Err(RuntimeIpcIoError::FrameTooLarge {
            size: max_bytes.saturating_add(1),
            max_bytes,
        });
    }
    result.map_err(RuntimeIpcIoError::Serialize)?;
    Ok(writer.bytes)
}

struct CappedWriter {
    bytes: Vec<u8>,
    max_bytes: usize,
    overflowed: bool,
}

impl CappedWriter {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(max_bytes.min(16 * 1024)),
            max_bytes,
            overflowed: false,
        }
    }
}

impl Write for CappedWriter {
    fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
        let remaining = self.max_bytes.saturating_sub(self.bytes.len());
        if input.len() > remaining {
            self.bytes.extend_from_slice(&input[..remaining]);
            self.overflowed = true;
            return Err(std::io::Error::other("runtime IPC frame is too large"));
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn first_unknown_field(
    original: &serde_json::Value,
    canonical: &serde_json::Value,
    path: String,
) -> Option<String> {
    match (original, canonical) {
        (serde_json::Value::Object(original), serde_json::Value::Object(canonical)) => {
            for (key, value) in original {
                let next_path = format!("{path}.{key}");
                let Some(canonical_value) = canonical.get(key) else {
                    return Some(next_path);
                };
                if let Some(unknown) = first_unknown_field(value, canonical_value, next_path) {
                    return Some(unknown);
                }
            }
            None
        }
        (serde_json::Value::Array(original), serde_json::Value::Array(canonical)) => original
            .iter()
            .zip(canonical)
            .enumerate()
            .find_map(|(index, (original, canonical))| {
                first_unknown_field(original, canonical, format!("{path}[{index}]"))
            }),
        _ => None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeIpcIoError {
    #[error("runtime IPC frame exceeds {max_bytes} bytes: {size}")]
    FrameTooLarge { size: usize, max_bytes: usize },
    #[error("runtime IPC transport failed")]
    Io(#[source] std::io::Error),
    #[error("failed to serialize runtime IPC frame")]
    Serialize(#[source] serde_json::Error),
    #[error("runtime IPC frame is invalid")]
    Deserialize(#[source] serde_json::Error),
    #[error("runtime IPC frame contains an unknown field at {path}")]
    UnknownField { path: String },
}
