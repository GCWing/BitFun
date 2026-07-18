use std::process::Stdio;
use std::time::Duration;

use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn acp_stdio_initializes_the_production_assembled_runtime() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("workspace");
    let user_root = temp.path().join("user-root");
    let home_root = temp.path().join("home-root");
    let config_root = temp.path().join("host-config");
    std::fs::create_dir_all(&workspace).expect("create workspace");

    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_bitfun-cli"))
        .arg("acp")
        .current_dir(&workspace)
        .env_remove("BITFUN_USER_ROOT")
        .env_remove("BITFUN_HOME")
        .env("BITFUN_E2E_STORAGE_GUARD", "1")
        .env("BITFUN_E2E_USER_ROOT", &user_root)
        .env("BITFUN_E2E_HOME", &home_root)
        .env("APPDATA", &config_root)
        .env("XDG_CONFIG_HOME", &config_root)
        .env("HOME", &home_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("start production ACP server");

    let mut stdin = child.stdin.take().expect("ACP stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("ACP stdout"));
    let mut stderr = child.stderr.take().expect("ACP stderr");
    let stderr_reader = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr
            .read_to_end(&mut bytes)
            .await
            .expect("read ACP stderr");
        String::from_utf8_lossy(&bytes).into_owned()
    });

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": 1,
            "clientCapabilities": {}
        }
    });
    let mut request = serde_json::to_vec(&request).expect("serialize initialize request");
    request.push(b'\n');
    stdin
        .write_all(&request)
        .await
        .expect("write ACP initialize request");
    stdin.flush().await.expect("flush ACP initialize request");

    let mut response_line = String::new();
    tokio::time::timeout(
        Duration::from_secs(60),
        stdout.read_line(&mut response_line),
    )
    .await
    .expect("production ACP initialize should not hang")
    .expect("read ACP initialize response");
    let response: serde_json::Value =
        serde_json::from_str(&response_line).expect("valid ACP JSON-RPC response");
    assert_eq!(response.get("id"), Some(&json!(1)));
    assert_eq!(response.pointer("/result/protocolVersion"), Some(&json!(1)));
    assert_eq!(
        response.pointer("/result/agentInfo/name"),
        Some(&json!("bitfun-acp"))
    );

    // ACP is a long-lived subprocess whose host owns termination. Close its
    // request stream, then always reap it; the kill fallback covers protocol
    // transport tasks that remain alive after the client finishes its check.
    drop(stdin);
    drop(stdout);
    let status = match tokio::time::timeout(Duration::from_secs(1), child.wait()).await {
        Ok(result) => Some(result.expect("wait for ACP server")),
        Err(_) => {
            child.kill().await.expect("stop hung ACP server");
            let _ = child.wait().await.expect("reap ACP server");
            None
        }
    };
    let stderr = stderr_reader.await.expect("join ACP stderr reader");

    if let Some(status) = status {
        assert!(
            status.success(),
            "ACP server exited unsuccessfully: {stderr}"
        );
    }
}
