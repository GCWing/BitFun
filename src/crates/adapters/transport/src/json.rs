use std::io::Write;

use serde::Serialize;

/// Serialize a JSON payload without buffering more than the configured budget.
pub fn encode_json_with_limit<T>(value: &T, max_bytes: usize) -> Result<Vec<u8>, JsonCodecError>
where
    T: Serialize + ?Sized,
{
    let mut writer = CappedWriter::new(max_bytes);
    let result = serde_json::to_writer(&mut writer, value);
    if writer.overflowed {
        return Err(JsonCodecError::PayloadTooLarge {
            minimum_size: max_bytes.saturating_add(1),
            max_bytes,
        });
    }
    result.map_err(JsonCodecError::Encode)?;
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
            return Err(std::io::Error::other("JSON payload is too large"));
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum JsonCodecError {
    #[error("JSON payload exceeds {max_bytes} bytes (at least {minimum_size} bytes)")]
    PayloadTooLarge {
        minimum_size: usize,
        max_bytes: usize,
    },
    #[error("failed to serialize JSON payload")]
    Encode(#[source] serde_json::Error),
}
