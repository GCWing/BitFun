use crate::RuntimeIpcFrame;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const MAX_FRAME_BYTES: usize = 64 * 1024;

pub async fn write_frame<W>(
    writer: &mut W,
    frame: &RuntimeIpcFrame,
) -> Result<(), RuntimeIpcIoError>
where
    W: AsyncWrite + Unpin,
{
    let bytes = serde_json::to_vec(frame).map_err(RuntimeIpcIoError::Serialize)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(RuntimeIpcIoError::FrameTooLarge { size: bytes.len() });
    }
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

pub async fn read_frame<R>(reader: &mut R) -> Result<RuntimeIpcFrame, RuntimeIpcIoError>
where
    R: AsyncRead + Unpin,
{
    let size = reader.read_u32().await.map_err(RuntimeIpcIoError::Io)? as usize;
    if size > MAX_FRAME_BYTES {
        return Err(RuntimeIpcIoError::FrameTooLarge { size });
    }
    let mut bytes = vec![0; size];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(RuntimeIpcIoError::Io)?;
    serde_json::from_slice(&bytes).map_err(RuntimeIpcIoError::Deserialize)
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeIpcIoError {
    #[error("runtime IPC frame exceeds {MAX_FRAME_BYTES} bytes: {size}")]
    FrameTooLarge { size: usize },
    #[error("runtime IPC transport failed")]
    Io(#[source] std::io::Error),
    #[error("failed to serialize runtime IPC frame")]
    Serialize(#[source] serde_json::Error),
    #[error("runtime IPC frame is invalid")]
    Deserialize(#[source] serde_json::Error),
}
