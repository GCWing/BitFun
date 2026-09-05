use std::path::Path;
use std::process::Output;

fn run_cli(workspace: &Path, user_root: &Path, home_root: &Path, args: &[&str]) -> Output {
    let config_root = user_root.join("host-config");
    openbitfun_services_core::process_manager::create_command(env!("CARGO_BIN_EXE_openbitfun"))
        .args(args)
        .current_dir(workspace)
        .env_remove("OPENBITFUN_USER_ROOT")
        .env_remove("OPENBITFUN_HOME")
        .env("OPENBITFUN_E2E_STORAGE_GUARD", "1")
        .env("OPENBITFUN_E2E_USER_ROOT", user_root)
        .env("OPENBITFUN_E2E_HOME", home_root)
        .env("APPDATA", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .env("HOME", home_root)
        .output()
        .expect("run openbitfun")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn environment() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("workspace");
    let user_root = temp.path().join("user-root");
    let home_root = temp.path().join("home-root");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    (temp, workspace, user_root, home_root)
}

#[test]
fn non_interactive_add_requires_an_explicit_server_type() {
    let (_temp, workspace, user_root, home_root) = environment();
    let output = run_cli(
        &workspace,
        &user_root,
        &home_root,
        &[
            "mcp",
            "add",
            "--non-interactive",
            "--name",
            "missing-type",
            "--command",
            "printf hello",
        ],
    );

    assert!(!output.status.success(), "{}", stdout(&output));
    assert!(stderr(&output).contains("--type"), "{}", stderr(&output));
}

#[test]
fn add_rejects_an_existing_server_without_changing_its_config() {
    let (_temp, workspace, user_root, home_root) = environment();
    let first = run_cli(
        &workspace,
        &user_root,
        &home_root,
        &[
            "mcp",
            "add",
            "--non-interactive",
            "--name",
            "duplicate",
            "--type",
            "local",
            "--command",
            "printf hello",
        ],
    );
    assert!(first.status.success(), "{}", stderr(&first));

    let duplicate = run_cli(
        &workspace,
        &user_root,
        &home_root,
        &[
            "mcp",
            "add",
            "--non-interactive",
            "--name",
            "duplicate",
            "--type",
            "remote",
            "--url",
            "https://example.com/mcp",
        ],
    );
    assert!(!duplicate.status.success(), "{}", stdout(&duplicate));
    assert!(
        stderr(&duplicate).contains("MCP server already exists: duplicate"),
        "{}",
        stderr(&duplicate)
    );

    let config = run_cli(&workspace, &user_root, &home_root, &["mcp", "config"]);
    assert!(config.status.success(), "{}", stderr(&config));
    let config: serde_json::Value =
        serde_json::from_slice(&config.stdout).expect("parse stored MCP config");
    let server = &config["mcpServers"]["duplicate"];
    assert_eq!(server["type"], "stdio");
    assert_eq!(server["command"], "printf");
    assert_eq!(server["args"], serde_json::json!(["hello"]));
    assert!(server.get("url").is_none());
}

#[test]
fn add_preserves_quoted_command_arguments() {
    let (_temp, workspace, user_root, home_root) = environment();
    let add = run_cli(
        &workspace,
        &user_root,
        &home_root,
        &[
            "mcp",
            "add",
            "--non-interactive",
            "--name",
            "quoted-command",
            "--type",
            "local",
            "--command",
            r#"node "path with spaces/server.js" --flag "hello world""#,
        ],
    );
    assert!(add.status.success(), "{}", stderr(&add));

    let config = run_cli(&workspace, &user_root, &home_root, &["mcp", "config"]);
    assert!(config.status.success(), "{}", stderr(&config));
    let config: serde_json::Value =
        serde_json::from_slice(&config.stdout).expect("parse stored MCP config");
    let server = &config["mcpServers"]["quoted-command"];
    assert_eq!(server["command"], "node");
    assert_eq!(
        server["args"],
        serde_json::json!(["path with spaces/server.js", "--flag", "hello world"])
    );
}
