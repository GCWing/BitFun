use std::io::{BufRead, Write};

use crate::{
    daemon::protocol::{RequestEnvelope, ServerMessage},
    error::{AppError, Result},
};

pub(super) fn read_content_length_request(
    reader: &mut impl BufRead,
) -> Result<Option<RequestEnvelope>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            let length = value.trim().parse::<usize>().map_err(|error| {
                AppError::Protocol(format!("invalid Content-Length header: {error}"))
            })?;
            content_length = Some(length);
        }
    }

    let content_length =
        content_length.ok_or_else(|| AppError::Protocol("missing Content-Length header".into()))?;
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map_err(|error| AppError::Protocol(format!("invalid daemon request: {error}")))
}

pub(super) fn write_content_length_message(
    writer: &mut impl Write,
    message: &ServerMessage,
) -> Result<()> {
    let body = serde_json::to_vec(message)
        .map_err(|error| AppError::Protocol(format!("failed to encode response: {error}")))?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}
