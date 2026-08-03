//! devecocli subprocess runner — shared by all HarmonyOS tools.
//!
//! Direct port of deveco-code `devecocli-run.ts`. Spawns the `devecocli` binary
//! via `tokio::process::Command`, captures stdout/stderr (64 KB cap per stream),
//! enforces a timeout, and optionally writes combined output to a log file.

use crate::agentic::tools::framework::ToolUseContext;
use crate::util::errors::{BitFunError, BitFunResult};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

pub(crate) const OUTPUT_LIMIT: usize = 64 * 1024;
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);

pub(crate) const DEVECOCLI_MISSING: &str =
    "devecocli is not installed or not in PATH. Install with: npm install -g devecocli (or @deveco/deveco-cli)";

pub(crate) const HDC_MISSING: &str = "hdc is not installed or not in PATH. hdc ships with the HarmonyOS SDK / DevEco Studio; ensure the SDK platform-tools directory is on PATH.";

pub(crate) struct DevecocliOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub cwd: String,
}

pub(crate) struct DevecocliOptions {
    pub log_path: Option<String>,
    pub timeout: Duration,
}

impl Default for DevecocliOptions {
    fn default() -> Self {
        Self {
            log_path: None,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

pub(crate) fn resolve_harmony_cwd(context: &ToolUseContext) -> String {
    // 1. Check session CWD (set by switch_cwd tool)
    if let Some(cwd) = super::session_cwd::get_session_cwd(context.session_id.as_deref()) {
        return cwd;
    }
    // 2. Fall back to workspace root
    if let Some(root) = context.workspace_root() {
        return root.to_string_lossy().to_string();
    }
    // 3. Fall back to process cwd
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string())
}

pub(crate) fn truncate_output(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    format!(
        "{}\n\n[output truncated at {} bytes]",
        &text[..limit],
        limit
    )
}

/// Spawn a shell-wrapped HarmonyOS CLI binary (`devecocli` or `hdc`) and capture
/// stdout/stderr (64 KB cap per stream), enforcing a timeout. `missing_msg` is
/// returned when the binary cannot be resolved in the user's shell PATH.
///
/// Wrapping in the user's configured terminal shell (same as ExecCommand) is
/// required so that PATH, PATHEXT, and shell profiles are honored — without it,
/// npm-installed `.cmd` shims and SDK paths only present in shell profiles
/// cannot be found.
async fn run_shell_command(
    binary: &str,
    args: &[&str],
    context: &ToolUseContext,
    options: DevecocliOptions,
    missing_msg: &str,
) -> BitFunResult<DevecocliOutput> {
    let cwd = resolve_harmony_cwd(context);
    let full_command = format!("{} {}", binary, args.join(" "));
    log::info!("{} {} (cwd: {})", binary, args.join(" "), cwd);

    let shell_argv = super::exec_command::resolve_shell_argv_for_command(&full_command).await;
    if shell_argv.is_empty() {
        return Err(BitFunError::tool(missing_msg.to_string()));
    }

    let mut command = Command::new(&shell_argv[0]);
    command.args(&shell_argv[1..]);
    command
        .current_dir(&cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    let child = match command.spawn() {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(BitFunError::tool(missing_msg.to_string()));
        }
        Err(e) => {
            return Err(BitFunError::tool(format!(
                "Failed to spawn shell for {}: {}",
                binary, e
            )));
        }
    };

    let wait_future = child.wait_with_output();
    let output = match tokio::time::timeout(options.timeout, wait_future).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            return Err(BitFunError::tool(format!(
                "{} {} failed to collect output: {}",
                binary,
                args.join(" "),
                e
            )));
        }
        Err(_) => {
            return Err(BitFunError::tool(format!(
                "{} {} timed out after {:?}",
                binary,
                args.join(" "),
                options.timeout
            )));
        }
    };

    let stdout = truncate_output(&String::from_utf8_lossy(&output.stdout), OUTPUT_LIMIT);
    let stderr = truncate_output(&String::from_utf8_lossy(&output.stderr), OUTPUT_LIMIT);
    let exit_code = output.status.code().unwrap_or(-1);

    if exit_code == 127 {
        let combined = format!("{}\n{}", stderr, stdout);
        let lower = combined.to_lowercase();
        if lower.contains("enoent")
            || lower.contains("not recognized")
            || lower.contains("command not found")
        {
            return Err(BitFunError::tool(missing_msg.to_string()));
        }
    }

    if let Some(log_path) = &options.log_path {
        let resolved = if Path::new(log_path).is_absolute() {
            log_path.clone()
        } else {
            Path::new(&cwd).join(log_path).to_string_lossy().to_string()
        };
        let content = [stdout.as_str(), stderr.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        if let Err(e) = tokio::fs::write(&resolved, &content).await {
            log::warn!("Failed to write log file {}: {}", resolved, e);
        }
    }

    let cwd_resolved = std::fs::canonicalize(&cwd)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or(cwd);

    Ok(DevecocliOutput {
        stdout,
        stderr,
        exit_code,
        cwd: cwd_resolved,
    })
}

pub(crate) async fn run_devecocli(
    args: &[&str],
    context: &ToolUseContext,
    options: DevecocliOptions,
) -> BitFunResult<DevecocliOutput> {
    run_shell_command("devecocli", args, context, options, DEVECOCLI_MISSING).await
}

/// Spawn the `hdc` binary directly (bypassing devecocli). Used by the automatic
/// `start_app` fallback path when devecocli cannot enumerate or drive devices.
pub(crate) async fn run_hdc(
    args: &[&str],
    context: &ToolUseContext,
    options: DevecocliOptions,
) -> BitFunResult<DevecocliOutput> {
    run_shell_command("hdc", args, context, options, HDC_MISSING).await
}

#[cfg(test)]
mod tests {
    use super::truncate_output;

    #[test]
    fn truncate_output_preserves_short_strings() {
        assert_eq!(truncate_output("hello", 100), "hello");
        let long = "a".repeat(100);
        let truncated = truncate_output(&long, 50);
        assert!(truncated.contains("[output truncated at 50 bytes]"));
    }
}
