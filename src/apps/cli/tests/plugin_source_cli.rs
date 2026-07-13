use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PLUGIN_SOURCE: &[u8] = br#"
import { type Plugin, tool } from "@opencode-ai/plugin"
export const DemoPlugin: Plugin = async () => ({
  tool: {
    ping: tool({
      description: "Ping the workspace",
      args: { topic: tool.schema.string() },
      async execute(args) { return args.topic },
    }),
  },
})
"#;

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn write_package(workspace: &Path, source: &[u8], declared_hash: &str) {
    let package = workspace.join(".bitfun/plugins/acme.demo");
    std::fs::create_dir_all(package.join(".opencode/plugins")).expect("create package directories");
    std::fs::write(package.join(".opencode/plugins/demo.ts"), source).expect("write plugin source");
    let manifest = serde_json::json!({
        "schemaVersion": 1,
        "id": "acme.demo",
        "version": "1.0.0",
        "adapter": "opencode_compatible",
        "files": [{
            "path": ".opencode/plugins/demo.ts",
            "sha256": declared_hash,
        }],
    });
    std::fs::write(
        package.join("bitfun.plugin.json"),
        serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");
}

fn run_cli(workspace: &Path, user_root: &Path, home_root: &Path, args: &[&str]) -> Output {
    let config_root = user_root.join("host-config");
    Command::new(env!("CARGO_BIN_EXE_bitfun-cli"))
        .args(args)
        .current_dir(workspace)
        .env_remove("BITFUN_USER_ROOT")
        .env_remove("BITFUN_HOME")
        .env("BITFUN_E2E_STORAGE_GUARD", "1")
        .env("BITFUN_E2E_USER_ROOT", user_root)
        .env("BITFUN_E2E_HOME", home_root)
        .env("APPDATA", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .env("HOME", home_root)
        .output()
        .expect("run bitfun-cli")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn activation_content_hash(output: &Output) -> String {
    stdout(output)
        .lines()
        .find_map(|line| line.strip_prefix("Content hash: "))
        .expect("activation preview content hash")
        .to_string()
}

fn find_trust_file(root: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == "trust.json") {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_trust_file(&path) {
                return Some(found);
            }
        }
    }
    None
}

#[test]
fn plugin_source_cli_rejects_unavailable_product_paths() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("workspace");
    let config_root = temp.path().join("host-config");
    std::fs::create_dir_all(&workspace).expect("create workspace");

    let output = Command::new(env!("CARGO_BIN_EXE_bitfun-cli"))
        .args(["plugins", "list"])
        .current_dir(&workspace)
        .env_remove("BITFUN_USER_ROOT")
        .env_remove("BITFUN_HOME")
        .env_remove("BITFUN_E2E_USER_ROOT")
        .env_remove("BITFUN_E2E_HOME")
        .env("BITFUN_E2E_STORAGE_GUARD", "1")
        .env("APPDATA", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .output()
        .expect("run bitfun-cli");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("Configuration error"));
    assert!(stderr(&output).contains("BITFUN_E2E_STORAGE_GUARD"));
}

#[test]
fn plugin_source_cli_lifecycle_and_doctor_exit_codes() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("workspace");
    let user_root = temp.path().join("user-root");
    let home_root = temp.path().join("home-root");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let first_source = PLUGIN_SOURCE;
    write_package(&workspace, first_source, &sha256(first_source));

    let list = run_cli(&workspace, &user_root, &home_root, &["plugins", "list"]);
    assert!(list.status.success(), "{}", stderr(&list));
    assert!(stdout(&list).contains("acme.demo 1.0.0 (workspace, unreviewed)"));
    assert!(stdout(&list).contains("Activation: inactive"));

    let approve = run_cli(
        &workspace,
        &user_root,
        &home_root,
        &["plugins", "approve-source", "acme.demo"],
    );
    assert!(approve.status.success(), "{}", stderr(&approve));
    assert!(stdout(&approve).contains("source-approved"));
    assert!(find_trust_file(&home_root).is_some());

    let preview = run_cli(
        &workspace,
        &user_root,
        &home_root,
        &["plugins", "activate", "acme.demo"],
    );
    assert!(preview.status.success(), "{}", stderr(&preview));
    assert!(stdout(&preview).contains("Plugin activation preview"));
    assert!(stdout(&preview).contains("No activation state changed"));
    assert!(stdout(&preview).contains("to confirm this exact package content"));
    assert!(stdout(&preview).contains("Custom tool candidates: supported"));
    assert!(stdout(&preview).contains("Declared candidates requiring permission: 1"));
    let preview_stdout = stdout(&preview);
    assert!(
        preview_stdout.contains("-> ping (risk: high)"),
        "{preview_stdout}"
    );

    let rejected = run_cli(
        &workspace,
        &user_root,
        &home_root,
        &[
            "plugins",
            "activate",
            "acme.demo",
            "--confirm",
            "sha256:stale",
        ],
    );
    assert!(!rejected.status.success());
    assert!(stderr(&rejected).contains("does not match"));
    assert!(stderr(&rejected)
        .contains("Re-run `bitfun-cli plugins activate acme.demo` to preview the current content"));

    let content_hash = activation_content_hash(&preview);

    let activate = run_cli(
        &workspace,
        &user_root,
        &home_root,
        &[
            "plugins",
            "activate",
            "acme.demo",
            "--confirm",
            &content_hash,
        ],
    );
    assert!(activate.status.success(), "{}", stderr(&activate));
    assert!(stdout(&activate).contains("Plugin activation result"));
    assert!(stdout(&activate).contains("Candidates requiring permission: 1"));
    assert!(stdout(&activate).contains("no tool was registered"));

    let active_list = run_cli(&workspace, &user_root, &home_root, &["plugins", "list"]);
    assert!(active_list.status.success(), "{}", stderr(&active_list));
    assert!(stdout(&active_list).contains("Activation: active for candidate projection"));

    let deactivate = run_cli(
        &workspace,
        &user_root,
        &home_root,
        &["plugins", "deactivate", "acme.demo"],
    );
    assert!(deactivate.status.success(), "{}", stderr(&deactivate));
    assert!(stdout(&deactivate).contains("is inactive"));

    let healthy = run_cli(&workspace, &user_root, &home_root, &["doctor"]);
    assert!(healthy.status.success(), "{}", stderr(&healthy));
    assert!(stdout(&healthy).contains("Candidate projection was not probed"));

    let revoke = run_cli(
        &workspace,
        &user_root,
        &home_root,
        &["plugins", "revoke", "acme.demo"],
    );
    assert!(revoke.status.success(), "{}", stderr(&revoke));
    assert!(stdout(&revoke).contains("revoked"));

    let deny = run_cli(
        &workspace,
        &user_root,
        &home_root,
        &["plugins", "deny", "acme.demo"],
    );
    assert!(deny.status.success(), "{}", stderr(&deny));
    assert!(stdout(&deny).contains("denied"));

    let second_source = b"export const DemoPlugin = async () => ({})";
    write_package(&workspace, second_source, &sha256(second_source));
    let changed = run_cli(&workspace, &user_root, &home_root, &["plugins", "list"]);
    assert!(changed.status.success(), "{}", stderr(&changed));
    assert!(stdout(&changed).contains("acme.demo 1.0.0 (workspace, unreviewed)"));

    std::fs::write(
        workspace.join(".bitfun/plugins/acme.demo/.opencode/plugins/demo.ts"),
        b"tampered",
    )
    .expect("tamper package");
    let invalid_approval = run_cli(
        &workspace,
        &user_root,
        &home_root,
        &["plugins", "approve-source", "acme.demo"],
    );
    assert!(!invalid_approval.status.success());
    assert!(stderr(&invalid_approval).contains("hash_mismatch"));
    assert!(
        stderr(&invalid_approval).contains(".opencode\\plugins\\demo.ts")
            || stderr(&invalid_approval).contains(".opencode/plugins/demo.ts")
    );

    let unhealthy = run_cli(&workspace, &user_root, &home_root, &["doctor"]);
    assert_eq!(unhealthy.status.code(), Some(1), "{}", stderr(&unhealthy));
    assert!(stdout(&unhealthy).contains("hash_mismatch"));
}
