use std::process::Command;

const DEPRECATION: &str = "Warning: `bitfun-cli` is deprecated; use `bitfun` instead.";

#[test]
fn legacy_version_matches_primary_and_warns_only_on_stderr() {
    let primary = Command::new(env!("CARGO_BIN_EXE_bitfun"))
        .arg("--version")
        .output()
        .expect("run bitfun --version");
    let legacy = Command::new(env!("CARGO_BIN_EXE_bitfun-cli"))
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
    let primary = Command::new(env!("CARGO_BIN_EXE_bitfun"))
        .arg("--not-a-real-option")
        .output()
        .expect("run invalid primary command");
    let legacy = Command::new(env!("CARGO_BIN_EXE_bitfun-cli"))
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
    let output = Command::new(copied)
        .arg("--version")
        .output()
        .expect("run isolated deprecated launcher");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.starts_with(DEPRECATION));
    assert!(stderr.contains("incomplete installation"));
    assert!(stderr.contains("install both `bitfun` and `bitfun-cli`"));
    assert_eq!(stderr.matches(DEPRECATION).count(), 1);
}
