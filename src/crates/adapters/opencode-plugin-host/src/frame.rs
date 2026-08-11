use crate::PluginHostError;
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub(super) async fn read_frame<R>(stream: &mut R, limit: usize) -> Result<Value, PluginHostError>
where
    R: AsyncRead + Unpin,
{
    let length = stream.read_u32().await.map_err(PluginHostError::Io)?;
    let length = usize::try_from(length).map_err(|_| {
        PluginHostError::InvalidHandshake("frame length does not fit usize".to_string())
    })?;
    if length == 0 || length > limit {
        return Err(PluginHostError::InvalidHandshake(format!(
            "frame length {length} exceeds limit {limit}"
        )));
    }
    let mut payload = vec![0; length];
    stream
        .read_exact(&mut payload)
        .await
        .map_err(PluginHostError::Io)?;
    serde_json::from_slice(&payload)
        .map_err(|error| PluginHostError::InvalidHandshake(error.to_string()))
}

pub(super) async fn write_frame<W>(
    stream: &mut W,
    value: &Value,
    limit: usize,
) -> Result<(), PluginHostError>
where
    W: AsyncWrite + Unpin,
{
    let payload = serde_json::to_vec(value)
        .map_err(|error| PluginHostError::InvalidHandshake(error.to_string()))?;
    if payload.is_empty() || payload.len() > limit {
        return Err(PluginHostError::InvalidHandshake(format!(
            "response length {} exceeds limit {limit}",
            payload.len()
        )));
    }
    let length = u32::try_from(payload.len()).map_err(|_| {
        PluginHostError::InvalidHandshake("response length exceeds u32".to_string())
    })?;
    stream
        .write_u32(length)
        .await
        .map_err(PluginHostError::Io)?;
    stream
        .write_all(&payload)
        .await
        .map_err(PluginHostError::Io)
}
