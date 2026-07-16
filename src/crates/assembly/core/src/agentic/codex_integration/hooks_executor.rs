//! Codex hooks command executor.
//!
//! Executes Codex plugin hooks commands (shell processes) at agent lifecycle
//! points. Hook configuration is discovered by the codex-adapter and translated
//! to OpenCode event names. This module spawns subprocesses, passes JSON input
//! via stdin, and parses JSON output from stdout.
//!
//! Hook execution follows the Codex specification: external command + JSON
//! stdin/stdout protocol, with timeout enforcement and fail-open semantics.

use std::collections::HashMap;
use std::time::Duration;
use tokio::process::Command as TokioCommand;

/// Default hook timeout (10 minutes).
const DEFAULT_HOOK_TIMEOUT_SECS: u64 = 600;

/// Configuration for a single hook handler to execute.
#[derive(Debug, Clone)]
pub struct HookHandler {
    /// Translated event name (e.g., "tool.execute.before")
    pub event_name: String,
    /// Shell command to execute
    pub command: String,
    /// Timeout in seconds
    pub timeout_secs: u64,
}

/// Result of executing a hook command.
#[derive(Debug)]
pub enum HookResult {
    Success { stdout: String, exit_code: i32 },
    Timeout,
    Error(String),
}

/// Executes a single hook command.
pub async fn execute_hook(handler: &HookHandler, input_json: &str) -> HookResult {
    let shell = if cfg!(windows) {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    };

    let shell_args = if cfg!(windows) {
        vec!["/C".to_string(), handler.command.clone()]
    } else {
        vec!["-lc".to_string(), handler.command.clone()]
    };

    let mut cmd = TokioCommand::new(&shell);
    cmd.args(&shell_args);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return HookResult::Error(format!("spawn failed: {e}")),
    };

    // Write stdin
    use tokio::io::AsyncWriteExt;
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(input_json.as_bytes()).await {
            return HookResult::Error(format!("stdin write failed: {e}"));
        }
    }

    let timeout = Duration::from_secs(handler.timeout_secs);
    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return HookResult::Error(format!("process error: {e}")),
        Err(_) => return HookResult::Timeout,
    };

    HookResult::Success {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    }
}
