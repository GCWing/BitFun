//! LSP protocol handling
//!
//! Implements encoding and decoding of JSON-RPC messages.

use anyhow::{anyhow, Result};
use log::{error, warn};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};

use super::types::{JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};

/// Reads an LSP message.
///
/// LSP uses HTTP-style headers:
/// Content-Length: xxx\r\n
/// \r\n
/// {json content}
pub async fn read_message(reader: &mut BufReader<ChildStdout>) -> Result<JsonRpcMessage> {
    let mut content_length: Option<usize> = None;
    let mut line_count = 0;
    let mut empty_line_count = 0;
    let mut found_lsp_header = false;

    const MAX_LINES: usize = 100;
    const MAX_EMPTY_LINES: usize = 50;

    loop {
        let mut raw_line = Vec::new();
        let _bytes_read =
            tokio::io::AsyncBufReadExt::read_until(reader, b'\n', &mut raw_line).await?;
        line_count += 1;

        let header = match String::from_utf8(raw_line.clone()) {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    "[LSP Protocol] Line {} contains non-UTF8 data: {:?}",
                    line_count, e
                );

                String::from_utf8_lossy(&raw_line).to_string()
            }
        };

        let header = header.trim();

        if line_count > MAX_LINES {
            return Err(anyhow!(
                "Protocol error: Read {} lines without finding valid LSP header. \
                 The LSP server may be outputting non-protocol data to stdout. \
                 Check server stderr logs for details.",
                line_count
            ));
        }

        if !found_lsp_header && header.is_empty() {
            empty_line_count += 1;

            if empty_line_count > MAX_EMPTY_LINES {
                return Err(anyhow!(
                    "Protocol error: Skipped {} empty lines without finding LSP header. \
                     The LSP server stdout may be misconfigured. \
                     Ensure the server only outputs LSP protocol messages to stdout.",
                    empty_line_count
                ));
            }

            if empty_line_count <= 10 || empty_line_count % 10 == 0 {
                warn!(
                    "[LSP Protocol] Skipped {} empty lines, still waiting for LSP header (will fail after {} empty lines)",
                    empty_line_count,
                    MAX_EMPTY_LINES
                );
            }
            continue;
        }

        if found_lsp_header && header.is_empty() {
            break;
        }

        if header.starts_with("Content-Length:") {
            found_lsp_header = true;
            let length_str = header
                .strip_prefix("Content-Length:")
                .ok_or_else(|| anyhow!("Invalid Content-Length header"))?
                .trim();
            content_length = Some(length_str.parse()?);
        } else if header.starts_with("Content-Type:") {
            found_lsp_header = true;
        } else if !header.is_empty() {
            if found_lsp_header {
                warn!("[LSP Protocol] Unexpected header line: {:?}", header);
            } else {
                if line_count <= 10 {
                    warn!("[LSP Protocol] Non-LSP output (skipping): {:?}", header);
                }
            }
        }
    }

    let content_length = content_length.ok_or_else(|| {
        error!(
            "[LSP Protocol] Missing Content-Length header after {} lines",
            line_count
        );
        anyhow!("Missing Content-Length header")
    })?;

    let mut buffer = vec![0u8; content_length];
    tokio::io::AsyncReadExt::read_exact(reader, &mut buffer).await?;

    let message: JsonRpcMessage = serde_json::from_slice(&buffer).map_err(|e| {
        let content_preview = String::from_utf8_lossy(&buffer);
        let preview = if content_preview.len() > 500 {
            let pos = content_preview
                .char_indices()
                .take_while(|(i, _)| *i < 500)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(0);
            format!("{}...", &content_preview[..pos])
        } else {
            content_preview.to_string()
        };
        error!("[LSP Protocol] Failed to parse JSON: {}", e);
        error!("[LSP Protocol] Content preview: {}", preview);
        anyhow!("Failed to parse JSON: {}", e)
    })?;

    Ok(message)
}

/// Writes an LSP message.
pub async fn write_message(writer: &mut ChildStdin, message: &JsonRpcMessage) -> Result<()> {
    let content = serde_json::to_string(message)?;
    let content_bytes = content.as_bytes();

    let header = format!("Content-Length: {}\r\n\r\n", content_bytes.len());

    writer.write_all(header.as_bytes()).await?;
    writer.write_all(content_bytes).await?;
    writer.flush().await?;

    Ok(())
}

/// Creates a request message.
pub fn create_request(
    id: u64,
    method: impl Into<String>,
    params: Option<serde_json::Value>,
) -> JsonRpcMessage {
    JsonRpcMessage::Request(JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: method.into(),
        params,
    })
}

/// Creates a notification message.
pub fn create_notification(
    method: impl Into<String>,
    params: Option<serde_json::Value>,
) -> JsonRpcMessage {
    JsonRpcMessage::Notification(JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: method.into(),
        params,
    })
}

/// Extracts the result from a response.
pub fn extract_result(response: JsonRpcResponse) -> Result<serde_json::Value> {
    if let Some(error) = response.error {
        return Err(anyhow!("LSP Error {}: {}", error.code, error.message));
    }

    response
        .result
        .ok_or_else(|| anyhow!("Missing result in response"))
}
