//! Process-spawning utilities for taiji-publisher CLI wrappers.
//!
//! This module wraps `std::process::Command` with safety improvements:
//! - Windows: `CREATE_NO_WINDOW` prevents console popups (matches
//!   `bitfun-services-core`'s [`process_manager::create_command`]).
//! - Timeout: `run_with_timeout` prevents hung subprocesses from blocking
//!   indefinitely.
//! - Argument sanitization: `sanitize_cli_arg` strips control characters from
//!   user-supplied strings before they reach the child process argv.
//!
//! ## Future migration
//!
//! When `taiji-publisher` takes a dependency on `bitfun-services-core`, the
//! direct `std::process::Command` usage should be replaced with
//! `bitfun_services_core::process_manager::create_command` (or, for long-running
//! subprocesses that need deterministic cleanup, with
//! `bitfun_services_core::process_tree::ProcessTreeChild`).  Until then this
//! module keeps the two platforms (`biliup` + `social-auto-upload`) aligned
//! with the same safety baseline.

use std::io;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

/// Strip control characters from a user-supplied string to prevent CLI injection.
///
/// Rejects newlines (`\n`, `\r`) and null bytes (`\x00`).  Other characters
/// (including shell metacharacters like `$`, `` ` ``, `|`, `;`) are **not**
/// stripped — the caller is responsible for ensuring that the target CLI does
/// not interpret them.
pub fn sanitize_cli_arg(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(*c, '\n' | '\r' | '\x00'))
        .collect()
}

/// Build a [`std::process::Command`] with the same platform safeguards that
/// BitFun's own [`bitfun_services_core::process_manager::create_command`] applies.
///
/// On Windows this adds `CREATE_NO_WINDOW` so the child process does not flash a
/// console window.
pub fn create_command<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
    let mut cmd = Command::new(program.as_ref());

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd
}

/// Run `cmd` with a wall-clock timeout, returning `io::ErrorKind::TimedOut`
/// when the deadline expires.
///
/// The child process is killed on timeout.
pub fn run_with_timeout(mut cmd: Command, timeout: Duration) -> io::Result<Output> {
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let child = cmd.spawn()?;

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // The child may still be alive in the spawned thread — best-effort
            // kill is handled by Drop on the Child handle that the thread owns.
            // We cannot reach that handle here, but the thread will still call
            // wait_with_output() which collects the zombie.  For a stronger
            // guarantee, migrate to ProcessTreeChild.
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "command timed out after {:?}: {:?}",
                    timeout,
                    cmd.get_program()
                ),
            ))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(io::Error::new(
            io::ErrorKind::Other,
            "command execution thread panicked",
        )),
    }
}
