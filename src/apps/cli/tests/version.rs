use std::path::Path;
use std::process::Command;

#[test]
fn version_flag_prints_evaluation_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_bitfun-cli"))
        .arg("--version")
        .output()
        .expect("bitfun-cli --version should run");

    assert!(
        output.status.success(),
        "bitfun-cli --version failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        format!(
            "bitfun {} (commit {})",
            env!("CARGO_PKG_VERSION"),
            env!("BITFUN_CLI_BUILD_COMMIT")
        )
    );
}

#[test]
fn evaluation_version_is_not_desktop_workspace_version() {
    let root_manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../Cargo.toml");
    let manifest_text =
        std::fs::read_to_string(&root_manifest).expect("workspace Cargo.toml should be readable");
    let manifest: toml::Value =
        toml::from_str(&manifest_text).expect("workspace Cargo.toml should parse");
    let workspace_version = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .expect("workspace package version should exist");

    assert_ne!(env!("CARGO_PKG_VERSION"), workspace_version);
}
