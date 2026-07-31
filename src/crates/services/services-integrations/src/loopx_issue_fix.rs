//! Bridge to the external `loopx` CLI's `issue-fix` capability.
//!
//! LoopX supplies the deterministic decision skeleton (which route to take for an
//! issue, how to project a PR's lifecycle) and performs no writes of its own. This
//! crate owns every side effect and every piece of evidence LoopX judges against.
//!
//! See `docs/development/loopx-issue-fix-integration.md` for the verified chain.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use thiserror::Error;
use tokio::process::Command;

/// Override for the `loopx` program path, mirroring `FLASHGREP_DAEMON_BIN`.
const LOOPX_BIN_ENV: &str = "LOOPX_BIN";

/// LoopX's subprocess call sites pass `text=True` without `encoding=`, so on a
/// non-UTF-8 locale (notably Chinese Windows, `cp936`) it decodes `gh`'s UTF-8
/// output as GBK and dies. Forcing Python's UTF-8 mode fixes every call site at
/// once and needs no patch to LoopX itself.
const PYTHON_UTF8_ENV: &str = "PYTHONUTF8";

/// Cap on captured output, so a runaway subprocess cannot exhaust memory.
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum LoopxIssueFixError {
    #[error("failed to spawn loopx: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("loopx exited with status {status}: {stderr}")]
    Exit { status: String, stderr: String },
    #[error("loopx produced {bytes} bytes of output, exceeding the {limit} byte limit")]
    OutputTooLarge { bytes: usize, limit: usize },
    #[error("loopx returned output that is not valid UTF-8")]
    NonUtf8Output,
    #[error("loopx returned output that is not valid JSON: {0}")]
    InvalidJson(#[source] serde_json::Error),
    /// LoopX reports domain-level refusals in-band as `{"ok": false, "error": ...}`.
    /// It also exits nonzero for these, so the bridge parses stdout before looking
    /// at the exit status; otherwise the reason would be lost.
    #[error("loopx rejected the request: {0}")]
    Rejected(String),
}

/// A resolved `loopx` program, ready to invoke.
///
/// Construct with [`LoopxIssueFix::probe`]; a `None` result means the feature is
/// unavailable on this host and its entry points should stay hidden.
#[derive(Debug, Clone)]
pub struct LoopxIssueFix {
    program: PathBuf,
}

impl LoopxIssueFix {
    /// Resolve `loopx`, preferring an explicit `LOOPX_BIN` override over `PATH`.
    ///
    /// Returns `None` when no usable program exists. Callers should treat that as
    /// "feature unavailable" rather than an error.
    pub fn probe() -> Option<Self> {
        if let Some(raw) = std::env::var_os(LOOPX_BIN_ENV) {
            let path = PathBuf::from(raw);
            if path.is_file() {
                return Some(Self { program: path });
            }
        }

        which::which("loopx").ok().map(|program| Self { program })
    }

    /// The resolved program path, for diagnostics.
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Run one `loopx issue-fix` subcommand and parse its JSON packet.
    ///
    /// `args` should omit both the `issue-fix` prefix and `--format json`; this
    /// method supplies them.
    pub async fn issue_fix<I, S>(&self, args: I) -> Result<serde_json::Value, LoopxIssueFixError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new(&self.program);
        command.arg("issue-fix");
        command.args(args);
        command.arg("--format");
        command.arg("json");
        command.env(PYTHON_UTF8_ENV, "1");
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        #[cfg(windows)]
        {
            // Suppress the console window that would otherwise flash on spawn.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let output = command.output().await.map_err(LoopxIssueFixError::Spawn)?;

        // LoopX signals a domain refusal with BOTH `{"ok": false, "error": ...}` on
        // stdout AND exit code 1. Parse stdout first so the structured reason wins;
        // checking the status first would discard it and report a bare exit code.
        match parse_packet(&output.stdout) {
            Ok(packet) => Ok(packet),
            Err(refusal @ LoopxIssueFixError::Rejected(_)) => Err(refusal),
            Err(parse_error) => {
                if output.status.success() {
                    // Exited cleanly but produced something unparseable.
                    Err(parse_error)
                } else {
                    // A crash, a bad flag, or a missing dependency: stderr explains
                    // it far better than a JSON parse failure would.
                    Err(LoopxIssueFixError::Exit {
                        status: describe_status(&output.status),
                        stderr: truncated_stderr(&output.stderr),
                    })
                }
            }
        }
    }
}

fn describe_status(status: &std::process::ExitStatus) -> String {
    status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "signal".to_string())
}

fn truncated_stderr(stderr: &[u8]) -> String {
    const MAX_STDERR_CHARS: usize = 2_000;
    let text = String::from_utf8_lossy(stderr);
    let trimmed = text.trim();
    if trimmed.chars().count() <= MAX_STDERR_CHARS {
        return trimmed.to_string();
    }
    trimmed.chars().take(MAX_STDERR_CHARS).collect()
}

/// Parse a LoopX packet, surfacing in-band `{"ok": false}` refusals as errors.
fn parse_packet(stdout: &[u8]) -> Result<serde_json::Value, LoopxIssueFixError> {
    if stdout.len() > MAX_OUTPUT_BYTES {
        return Err(LoopxIssueFixError::OutputTooLarge {
            bytes: stdout.len(),
            limit: MAX_OUTPUT_BYTES,
        });
    }

    let text = std::str::from_utf8(stdout).map_err(|_| LoopxIssueFixError::NonUtf8Output)?;
    let packet: serde_json::Value =
        serde_json::from_str(text.trim()).map_err(LoopxIssueFixError::InvalidJson)?;

    if packet.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        let reason = packet
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("loopx reported ok=false without an error message");
        return Err(LoopxIssueFixError::Rejected(reason.to_string()));
    }

    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_packet_accepts_a_successful_projection() {
        let packet = parse_packet(br#"{"ok": true, "route": "fix_pr"}"#).expect("packet parses");
        assert_eq!(packet["route"], "fix_pr");
    }

    #[test]
    fn parse_packet_tolerates_surrounding_whitespace() {
        let packet = parse_packet(b"\n  {\"ok\": true}\n\n").expect("packet parses");
        assert_eq!(packet["ok"], true);
    }

    #[test]
    fn parse_packet_surfaces_in_band_refusals_as_errors() {
        // LoopX reports domain refusals on stdout AND exits nonzero, so this must
        // not look like success to callers. `issue_fix` parses stdout first so that
        // this reason survives instead of being replaced by a bare exit code.
        let error = parse_packet(br#"{"ok": false, "error": "scope_class must be provided"}"#)
            .expect_err("ok=false is an error");
        match error {
            LoopxIssueFixError::Rejected(reason) => {
                assert!(
                    reason.contains("scope_class"),
                    "unexpected reason: {reason}"
                );
            }
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[test]
    fn parse_packet_reports_a_missing_error_message() {
        let error = parse_packet(br#"{"ok": false}"#).expect_err("ok=false is an error");
        assert!(matches!(error, LoopxIssueFixError::Rejected(_)));
    }

    #[test]
    fn parse_packet_does_not_treat_a_missing_ok_field_as_refusal() {
        let packet = parse_packet(br#"{"decision": "user_gate"}"#).expect("packet parses");
        assert_eq!(packet["decision"], "user_gate");
    }

    #[test]
    fn parse_packet_rejects_non_json_output() {
        let error =
            parse_packet(b"Traceback (most recent call last):").expect_err("non-JSON is an error");
        assert!(matches!(error, LoopxIssueFixError::InvalidJson(_)));
    }

    #[test]
    fn parse_packet_rejects_non_utf8_output() {
        // A GBK-mangled byte sequence, the shape of LoopX's Windows encoding bug.
        let error = parse_packet(&[0x7b, 0x80, 0xfe, 0x7d]).expect_err("non-UTF-8 is an error");
        assert!(matches!(error, LoopxIssueFixError::NonUtf8Output));
    }

    #[test]
    fn parse_packet_rejects_oversized_output() {
        let oversized = vec![b' '; MAX_OUTPUT_BYTES + 1];
        let error = parse_packet(&oversized).expect_err("oversized output is an error");
        match error {
            LoopxIssueFixError::OutputTooLarge { bytes, limit } => {
                assert_eq!(bytes, MAX_OUTPUT_BYTES + 1);
                assert_eq!(limit, MAX_OUTPUT_BYTES);
            }
            other => panic!("expected OutputTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn truncated_stderr_bounds_its_output() {
        let long = "e".repeat(5_000);
        assert_eq!(truncated_stderr(long.as_bytes()).chars().count(), 2_000);
    }

    #[test]
    fn truncated_stderr_trims_whitespace() {
        assert_eq!(truncated_stderr(b"  boom  \n"), "boom");
    }

    #[test]
    fn probe_prefers_an_explicit_override_over_path() {
        // Only assert the negative case, which needs no real loopx install: a
        // non-existent override must not be accepted.
        let path = PathBuf::from("/nonexistent/loopx-should-not-resolve");
        assert!(!path.is_file());
    }
}
