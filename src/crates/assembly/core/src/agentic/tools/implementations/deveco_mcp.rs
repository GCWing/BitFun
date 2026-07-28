//! Minimal MCP stdio client for calling devecocli's "check" tool.
//!
//! Spawns `devecocli serve mcp` as a subprocess, performs the JSON-RPC
//! handshake (initialize → initialized notification → tools/call), extracts
//! the text from the response, then kills the process.

use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
const INIT_TIMEOUT_SECS: u64 = 60;
const CALL_TIMEOUT_SECS: u64 = 300;

/// Spawn `devecocli serve mcp` in `project_path`, call the "check" tool with
/// `files`, and return the text content from the MCP response.
pub async fn run_deveco_check(files: &[String], project_path: &str) -> Result<String, String> {
    let mut child = Command::new("devecocli")
        .arg("serve")
        .arg("mcp")
        .current_dir(project_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn `devecocli serve mcp` (is devecocli installed?): {}", e))?;

    let stdin = child
        .stdin
        .take()
        .ok_or("Failed to capture devecocli stdin")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Failed to capture devecocli stdout")?;

    let mut reader = BufReader::new(stdout).lines();

    // 1. initialize
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "bitfun", "version": "1.0" }
        }
    });
    write_json_line(stdin, &init_req).await?;

    let _init_resp = timeout(Duration::from_secs(INIT_TIMEOUT_SECS), read_response(&mut reader, 1))
        .await
        .map_err(|_| "Timeout waiting for MCP initialize response".to_string())??;

    // 2. initialized notification
    let init_notif = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    write_json_line(stdin, &init_notif).await?;

    // 3. tools/call — check
    let call_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "check",
            "arguments": { "files": files }
        }
    });
    write_json_line(stdin, &call_req).await?;

    // 4. read response
    let call_resp = timeout(Duration::from_secs(CALL_TIMEOUT_SECS), read_response(&mut reader, 2))
        .await
        .map_err(|_| "Timeout waiting for MCP check response".to_string())??;

    // kill the process — we don't need it anymore
    let _ = child.kill().await;

    extract_text_from_response(&call_resp)
}

async fn write_json_line(
    mut stdin: tokio::process::ChildStdin,
    value: &Value,
) -> Result<(), String> {
    let line = serde_json::to_string(value).map_err(|e| e.to_string())?;
    stdin
        .write_all(line.as_bytes())
        .await
        .map_err(|e| format!("Failed to write to MCP stdin: {}", e))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|e| format!("Failed to write newline to MCP stdin: {}", e))?;
    stdin
        .flush()
        .await
        .map_err(|e| format!("Failed to flush MCP stdin: {}", e))?;
    Ok(())
}

/// Read lines from the MCP stdout, skipping notifications, until we find a
/// response matching `expected_id`.
async fn read_response(
    reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    expected_id: i64,
) -> Result<Value, String> {
    loop {
        let line = reader
            .next_line()
            .await
            .map_err(|e| format!("Error reading MCP response: {}", e))?
            .ok_or("EOF: MCP server closed stdout before responding")?;

        if line.trim().is_empty() {
            continue;
        }

        let msg: Value = serde_json::from_str(&line)
            .map_err(|e| format!("Invalid JSON from MCP server: {} (line: {})", e, line))?;

        if msg.get("id").and_then(|v| v.as_i64()) == Some(expected_id) {
            return Ok(msg);
        }
        // Skip notifications and other messages
    }
}

/// Extract the text content from an MCP `tools/call` response.
fn extract_text_from_response(resp: &Value) -> Result<String, String> {
    // Check for JSON-RPC error
    if let Some(error) = resp.get("error") {
        let msg = error
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown MCP error");
        return Err(msg.to_string());
    }

    let result = resp
        .get("result")
        .ok_or("Missing 'result' in MCP response".to_string())?;

    // MCP tool call result can have isError: true
    if result
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Err(extract_content_text(result));
    }

    Ok(extract_content_text(result))
}

fn extract_content_text(value: &Value) -> String {
    if let Some(content) = value.get("content").and_then(|v| v.as_array()) {
        let texts: Vec<String> = content
            .iter()
            .filter_map(|c| {
                c.get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        if !texts.is_empty() {
            return texts.join("\n");
        }
    }
    serde_json::to_string_pretty(value).unwrap_or_default()
}
