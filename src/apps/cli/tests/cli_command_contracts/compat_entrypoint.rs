use std::process::{Command, Output};

const DEPRECATION: &str = "Warning: `bitfun-cli` is deprecated; use `bitfun` instead.";

/// Run a just-written executable, retrying while Linux reports `ETXTBSY`.
///
/// The tests in this file run as threads of one process. When one copies a
/// binary, the destination is briefly open for writing; if a sibling thread
/// forks for its own `Command` in that window, the child inherits the write
/// descriptor, and an `execve` of that file fails with "Text file busy" until
/// the child clears it. The descriptor is `CLOEXEC`, so the window is short and
/// the race is invisible until a loaded CI runner widens it.
///
/// Retrying is the fix available to a caller: the writer is already closed by
/// the time `fs::copy` returns, and the inherited copy is out of our hands.
fn run_freshly_written(command: &mut Command) -> std::io::Result<Output> {
    for _ in 0..50 {
        match command.output() {
            Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            result => return result,
        }
    }
    command.output()
}

#[test]
fn legacy_version_matches_primary_and_warns_only_on_stderr() {
    let primary =
        bitfun_services_core::process_manager::create_command(env!("CARGO_BIN_EXE_bitfun"))
            .arg("--version")
            .output()
            .expect("run bitfun --version");
    let legacy =
        bitfun_services_core::process_manager::create_command(env!("CARGO_BIN_EXE_bitfun-cli"))
            .arg("--version")
            .output()
            .expect("run deprecated bitfun-cli --version");

    assert!(primary.status.success());
    assert!(legacy.status.success());
    assert_eq!(legacy.stdout, primary.stdout);
    assert_eq!(String::from_utf8_lossy(&legacy.stderr).trim(), DEPRECATION);
    assert!(!String::from_utf8_lossy(&primary.stderr).contains("deprecated"));
}

#[test]
fn legacy_forwards_clap_failure_exit_code() {
    let primary =
        bitfun_services_core::process_manager::create_command(env!("CARGO_BIN_EXE_bitfun"))
            .arg("--not-a-real-option")
            .output()
            .expect("run invalid primary command");
    let legacy =
        bitfun_services_core::process_manager::create_command(env!("CARGO_BIN_EXE_bitfun-cli"))
            .arg("--not-a-real-option")
            .output()
            .expect("run invalid legacy command");

    assert_eq!(legacy.status.code(), primary.status.code());
    assert!(String::from_utf8_lossy(&legacy.stderr).starts_with(DEPRECATION));
}

#[test]
fn legacy_reports_a_missing_primary_without_recursing() {
    let temp = tempfile::tempdir().expect("create temporary install directory");
    let file_name = if cfg!(windows) {
        "bitfun-cli.exe"
    } else {
        "bitfun-cli"
    };
    let copied = temp.path().join(file_name);
    std::fs::copy(env!("CARGO_BIN_EXE_bitfun-cli"), &copied)
        .expect("copy deprecated launcher without primary sibling");
    let output = run_freshly_written(
        bitfun_services_core::process_manager::create_command(copied).arg("--version"),
    )
    .expect("run isolated deprecated launcher");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.starts_with(DEPRECATION));
    assert!(stderr.contains("incomplete installation"));
    assert!(stderr.contains("install both `bitfun` and `bitfun-cli`"));
    assert_eq!(stderr.matches(DEPRECATION).count(), 1);
}
