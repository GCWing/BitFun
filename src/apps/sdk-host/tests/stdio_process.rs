use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn standalone_sdk_host_negotiates_and_shuts_down_without_cli() {
    let temp = tempfile::tempdir().expect("isolated SDK Host environment");
    let workspace = temp.path().join("workspace");
    let user_root = temp.path().join("user-root");
    let home_root = temp.path().join("home");
    let config_root = temp.path().join("config-root");
    for path in [&workspace, &user_root, &home_root, &config_root] {
        std::fs::create_dir_all(path).expect("SDK Host fixture directory");
    }

    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_bitfun-sdk-host"))
        .current_dir(&workspace)
        .env_remove("BITFUN_USER_ROOT")
        .env_remove("BITFUN_HOME")
        .env("BITFUN_E2E_STORAGE_GUARD", "1")
        .env("BITFUN_E2E_USER_ROOT", &user_root)
        .env("BITFUN_E2E_HOME", &home_root)
        .env("APPDATA", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .env("HOME", &home_root)
        .env("USERPROFILE", &home_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("start standalone Agent SDK Host");

    let mut stdin = child.stdin.take().expect("SDK Host stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("SDK Host stdout"));
    let mut stderr = child.stderr.take().expect("SDK Host stderr");

    send_request(
        &mut stdin,
        1,
        "initialize",
        json!({
            "protocolVersion": 1,
            "clientInfo": { "name": "standalone-process-fixture", "version": "0.1.0" },
            "capabilities": { "serverNotifications": true }
        }),
    )
    .await;
    let initialized = read_response(&mut stdout, "initialize").await;
    assert_eq!(initialized["id"], 1);
    assert_eq!(initialized["result"]["protocolVersion"], 1);

    send_request(&mut stdin, 2, "shutdown", json!({})).await;
    let shutdown = read_response(&mut stdout, "shutdown").await;
    assert_eq!(shutdown["id"], 2);
    assert_eq!(shutdown["result"]["accepted"], true);
    drop(stdin);

    let status = tokio::time::timeout(Duration::from_secs(15), child.wait())
        .await
        .expect("SDK Host must stop after shutdown")
        .expect("wait for SDK Host");
    let mut stderr_output = Vec::new();
    stderr
        .read_to_end(&mut stderr_output)
        .await
        .expect("read SDK Host stderr");
    assert!(
        status.success(),
        "SDK Host failed: {}",
        String::from_utf8_lossy(&stderr_output)
    );
}

async fn send_request(
    stdin: &mut tokio::process::ChildStdin,
    id: i64,
    method: &str,
    params: Value,
) {
    let mut line = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    }))
    .expect("serialize SDK Host request");
    line.push(b'\n');
    stdin
        .write_all(&line)
        .await
        .expect("write SDK Host request");
    stdin.flush().await.expect("flush SDK Host request");
}

async fn read_response(
    stdout: &mut BufReader<tokio::process::ChildStdout>,
    operation: &str,
) -> Value {
    let mut line = String::new();
    let bytes = tokio::time::timeout(Duration::from_secs(60), stdout.read_line(&mut line))
        .await
        .unwrap_or_else(|_| panic!("SDK Host {operation} timed out"))
        .expect("read SDK Host stdout");
    assert_ne!(bytes, 0, "SDK Host stdout closed during {operation}");
    serde_json::from_str(&line)
        .unwrap_or_else(|error| panic!("SDK Host stdout was not JSON: {error}: {line}"))
}
