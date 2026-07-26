//! Hook dispatch: run matching command handlers for one lifecycle event.
//!
//! Process interface (Codex-compatible):
//! - The JSON payload is written to the handler's stdin.
//! - Exit code 0: stdout is interpreted as a JSON decision document when it
//!   parses; otherwise, for events where plain stdout is context
//!   (SessionStart, UserPromptSubmit, SubagentStart), the text becomes
//!   model-visible context.
//! - Exit code 2: the event is blocked; stderr provides the blocking reason.
//! - Any other exit code, spawn failure, or timeout: a non-blocking warning.

use super::output::{non_empty, AgentHookOutcome, RawHookOutput};
use super::payload::AgentHookPayload;
use super::settings::{AgentHookEvent, AgentHookHandler, AgentHookSettings};
use log::{debug, warn};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Cap for a single model-visible hook text (reason or context). Larger
/// output is truncated with a marker, mirroring the Codex output budget.
pub const MAX_HOOK_MODEL_OUTPUT_BYTES: usize = 10_000;

/// Cap for captured process output retained in memory.
const MAX_CAPTURED_OUTPUT_BYTES: usize = 1024 * 1024;

/// Executes configured hooks for agent lifecycle events.
#[derive(Debug, Default)]
pub struct AgentHookEngine {
    settings: AgentHookSettings,
}

impl AgentHookEngine {
    pub fn new(settings: AgentHookSettings) -> Self {
        Self { settings }
    }

    pub fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }

    pub fn has_rules(&self, event: AgentHookEvent) -> bool {
        self.settings.has_rules(event)
    }

    pub fn settings(&self) -> &AgentHookSettings {
        &self.settings
    }

    /// Run every matching handler for the payload's event, sequentially in
    /// configuration order (user layers before project layers), and fold
    /// their results into one [`AgentHookOutcome`].
    pub async fn dispatch(&self, payload: &AgentHookPayload, cwd: &Path) -> AgentHookOutcome {
        let event = payload.event();
        let mut outcome = AgentHookOutcome::default();
        let rules = self.settings.rules_for(event);
        if rules.is_empty() {
            return outcome;
        }
        let matcher_value = payload.event.matcher_value();
        let payload_json = payload.to_json().to_string();
        'rules: for rule in rules {
            if !rule.matcher.matches(matcher_value) {
                continue;
            }
            for handler in &rule.handlers {
                outcome.executed_handlers += 1;
                let finalized = self
                    .run_and_apply(event, handler, &payload_json, cwd, &mut outcome)
                    .await;
                if finalized {
                    break 'rules;
                }
            }
        }
        outcome
    }

    /// Run one handler and fold its result into `outcome`. Returns `true`
    /// when the dispatch is finalized (blocked or denied) and remaining
    /// handlers must not run.
    async fn run_and_apply(
        &self,
        event: AgentHookEvent,
        handler: &AgentHookHandler,
        payload_json: &str,
        cwd: &Path,
        outcome: &mut AgentHookOutcome,
    ) -> bool {
        let command = handler.effective_command();
        let timeout = handler.effective_timeout(event);
        debug!(
            "Running agent hook: event={}, command={}, timeout_ms={}",
            event,
            command,
            timeout.as_millis()
        );
        let run = run_hook_command(command, payload_json, cwd, timeout).await;
        match run {
            HookCommandRun::SpawnFailed(error) => {
                outcome.warnings.push(format!(
                    "Hook '{command}' for {event} could not be started: {error}"
                ));
                false
            }
            HookCommandRun::TimedOut => {
                outcome.warnings.push(format!(
                    "Hook '{command}' for {event} timed out after {}s and was killed",
                    timeout.as_secs()
                ));
                false
            }
            HookCommandRun::Completed {
                exit_code,
                stdout,
                stderr,
            } => match exit_code {
                Some(0) => match serde_json::from_str::<RawHookOutput>(stdout.trim()) {
                    Ok(output) => outcome.apply_output(output),
                    Err(_) => {
                        let text = stdout.trim();
                        if !text.is_empty() && event.plain_stdout_is_context() {
                            outcome.additional_context.push(truncate_model_output(text));
                        }
                        false
                    }
                },
                Some(2) => {
                    let reason = non_empty(Some(stderr)).unwrap_or_else(|| {
                        format!("Hook '{command}' blocked this {event} event (exit code 2).")
                    });
                    if outcome.block_reason.is_none() {
                        outcome.block_reason = Some(truncate_model_output(&reason));
                    }
                    true
                }
                Some(code) => {
                    outcome.warnings.push(format!(
                        "Hook '{command}' for {event} exited with non-blocking code {code}"
                    ));
                    false
                }
                None => {
                    outcome.warnings.push(format!(
                        "Hook '{command}' for {event} was terminated by a signal"
                    ));
                    false
                }
            },
        }
    }
}

enum HookCommandRun {
    Completed {
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    TimedOut,
    SpawnFailed(String),
}

async fn run_hook_command(
    command: &str,
    payload_json: &str,
    cwd: &Path,
    timeout: Duration,
) -> HookCommandRun {
    let mut process = if cfg!(windows) {
        let mut process = Command::new("cmd");
        process.arg("/C").arg(command);
        process
    } else {
        let mut process = Command::new("sh");
        process.arg("-c").arg(command);
        process
    };
    if let Some(cwd) = existing_dir(cwd) {
        process.current_dir(cwd);
    }
    process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(error) => return HookCommandRun::SpawnFailed(error.to_string()),
    };
    let mut stdin = child.stdin.take();

    // The stdin write must be inside the timeout and must not be awaited to
    // completion before the child is reaped: a handler that never reads stdin
    // blocks the write once the payload exceeds the pipe buffer, and a
    // handler that exits early makes the write fail with EPIPE. Driving the
    // write concurrently with `wait_with_output` covers both, and the whole
    // interaction is bounded by one timeout.
    let interaction = async {
        let write = async {
            if let Some(mut stdin) = stdin.take() {
                let _ = stdin.write_all(payload_json.as_bytes()).await;
                let _ = stdin.shutdown().await;
            }
        };
        let (_, output) = tokio::join!(write, child.wait_with_output());
        output
    };

    match tokio::time::timeout(timeout, interaction).await {
        Ok(Ok(output)) => HookCommandRun::Completed {
            exit_code: output.status.code(),
            stdout: bounded_lossy_string(output.stdout),
            stderr: bounded_lossy_string(output.stderr),
        },
        Ok(Err(error)) => HookCommandRun::SpawnFailed(error.to_string()),
        // `kill_on_drop` reaps the child when the timeout drops the future.
        Err(_) => HookCommandRun::TimedOut,
    }
}

fn existing_dir(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        return None;
    }
    if path.is_dir() {
        Some(path.to_path_buf())
    } else {
        warn!(
            "Hook working directory does not exist; running without it: {}",
            path.display()
        );
        None
    }
}

fn bounded_lossy_string(mut bytes: Vec<u8>) -> String {
    if bytes.len() > MAX_CAPTURED_OUTPUT_BYTES {
        bytes.truncate(MAX_CAPTURED_OUTPUT_BYTES);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Truncate model-visible hook text to the output budget, preserving UTF-8.
pub(crate) fn truncate_model_output(text: &str) -> String {
    if text.len() <= MAX_HOOK_MODEL_OUTPUT_BYTES {
        return text.to_string();
    }
    let mut end = MAX_HOOK_MODEL_OUTPUT_BYTES;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[hook output truncated]", &text[..end])
}
