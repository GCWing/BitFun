use super::{
    accept_authenticated_connection, complete_handshake, read_frame, validate_config, write_frame,
    PluginHost, PluginHostConfig, PluginHostError, PluginHostShutdownDisposition,
    PluginHostShutdownPolicy, DEFAULT_MAX_FRAME_BYTES, MAX_FRAME_BYTES, MIN_NEGOTIATED_FRAME_BYTES,
};
use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;

mod peer_tests;

#[test]
fn relative_entry_is_rejected_before_process_start() {
    let config = PluginHostConfig {
        runtime_command: PathBuf::from("bun"),
        entry: PathBuf::from("dist/extension-host.js"),
        working_directory: PathBuf::from("."),
        cache_directory: std::env::temp_dir(),
        log_file: std::env::temp_dir().join("plugin-host.log"),
        log_level: "debug".to_string(),
    };

    assert!(matches!(
        validate_config(&config),
        Err(PluginHostError::RelativeEntry(_))
    ));
}

#[tokio::test]
async fn handshake_accepts_matching_token_and_returns_cache_directory() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener should have address");
    let cache_directory = std::env::temp_dir().join("openbitfun-plugin-host-test-cache");
    let expected_cache_directory = cache_directory.to_string_lossy().into_owned();
    let host = tokio::spawn(async move {
        let mut stream = TcpStream::connect(address)
            .await
            .expect("fake host should connect");
        write_frame(
            &mut stream,
            &json!({
                "jsonrpc": "2.0",
                "id": "host:1",
                "method": "backend.handshake",
                "params": {
                    "token": "test-token",
                    "protocolVersion": 1,
                    "opencodeVersion": "1.17.18",
                    "maxFrameBytes": DEFAULT_MAX_FRAME_BYTES,
                    "capabilities": ["config-contributors-v1", "config-contributions-v2", "generation-fencing-v1", "unknown-v1"]
                }
            }),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("fake host should write handshake");
        read_frame(&mut stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("fake host should read handshake response")
    });
    let (mut backend_stream, _) = listener
        .accept()
        .await
        .expect("backend should accept fake host");

    let negotiated = complete_handshake(&mut backend_stream, "test-token", &cache_directory)
        .await
        .expect("matching handshake should succeed");
    let response = host.await.expect("fake host task should finish");

    assert_eq!(negotiated.max_frame_bytes, DEFAULT_MAX_FRAME_BYTES);
    assert!(negotiated.capabilities.supports("config-contributors-v1"));
    assert!(negotiated.capabilities.supports("config-contributions-v2"));
    assert!(negotiated.capabilities.supports("generation-fencing-v1"));
    assert!(!negotiated.capabilities.supports("unknown-v1"));
    assert_eq!(
        response["result"]["cacheDirectory"],
        expected_cache_directory
    );
}

#[tokio::test]
async fn startup_timeout_covers_a_connected_client_that_never_handshakes() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener should have address");
    let _client = TcpStream::connect(address)
        .await
        .expect("fake host should connect");

    let result = accept_authenticated_connection(
        &listener,
        "test-token",
        &std::env::temp_dir(),
        Duration::from_millis(25),
    )
    .await;

    assert!(matches!(result, Err(PluginHostError::StartupTimeout)));
}

#[tokio::test]
async fn startup_failure_waits_until_the_spawned_process_is_reaped() {
    let runtime_available = Command::new("node")
        .arg("--version")
        .output()
        .await
        .is_ok_and(|output| output.status.success());
    if !runtime_available {
        return;
    }
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let script = directory.path().join("never-connects.mjs");
    tokio::fs::write(
        &script,
        r#"import fs from "node:fs";
fs.writeFileSync("child.pid", String(process.pid));
setInterval(() => {}, 1000);
"#,
    )
    .await
    .expect("startup failure fixture should be written");
    let result = PluginHost::start_with_timeout(
        PluginHostConfig {
            runtime_command: PathBuf::from("node"),
            entry: script,
            working_directory: directory.path().to_path_buf(),
            cache_directory: directory.path().join("cache"),
            log_file: directory.path().join("plugin-host.log"),
            log_level: "debug".to_string(),
        },
        Duration::from_secs(1),
    )
    .await;

    assert!(matches!(result, Err(PluginHostError::StartupTimeout)));
    let process_id = tokio::fs::read_to_string(directory.path().join("child.pid"))
        .await
        .expect("fixture should publish its process id")
        .parse::<u32>()
        .expect("fixture process id should be numeric");
    assert!(
        !process_is_running(process_id).await,
        "PluginHost::start must not return while its failed child is still alive"
    );
}

#[cfg(windows)]
async fn process_is_running(process_id: u32) -> bool {
    let filter = format!("PID eq {process_id}");
    Command::new("tasklist")
        .args(["/FI", &filter, "/FO", "CSV", "/NH"])
        .output()
        .await
        .is_ok_and(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains(&format!("\"{process_id}\""))
        })
}

#[cfg(unix)]
async fn process_is_running(process_id: u32) -> bool {
    Command::new("kill")
        .args(["-0", &process_id.to_string()])
        .status()
        .await
        .is_ok_and(|status| status.success())
}

#[tokio::test]
async fn handshake_clamps_requested_frame_limit_to_the_safe_range() {
    async fn negotiate(requested: usize) -> usize {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("test listener should bind");
        let address = listener.local_addr().expect("listener should have address");
        let host = tokio::spawn(async move {
            let mut stream = TcpStream::connect(address)
                .await
                .expect("fake host should connect");
            write_frame(
                &mut stream,
                &json!({
                    "jsonrpc": "2.0",
                    "id": "host:1",
                    "method": "backend.handshake",
                    "params": {
                        "token": "test-token",
                        "protocolVersion": 1,
                        "opencodeVersion": "1.17.18",
                        "maxFrameBytes": requested
                    }
                }),
                DEFAULT_MAX_FRAME_BYTES,
            )
            .await
            .expect("fake host should write handshake");
            read_frame(&mut stream, DEFAULT_MAX_FRAME_BYTES)
                .await
                .expect("fake host should read handshake response");
        });
        let (mut stream, _) = listener.accept().await.expect("backend should accept host");
        let negotiated = complete_handshake(&mut stream, "test-token", &std::env::temp_dir())
            .await
            .expect("handshake should succeed");
        host.await.expect("fake host should finish");
        negotiated.max_frame_bytes
    }

    assert_eq!(negotiate(1).await, MIN_NEGOTIATED_FRAME_BYTES);
    assert_eq!(negotiate(usize::MAX).await, MAX_FRAME_BYTES);
}

#[tokio::test]
async fn node_child_connects_and_completes_authenticated_handshake() {
    assert_runtime_child_connects("node").await;
}

#[tokio::test]
async fn bun_child_connects_and_completes_authenticated_handshake() {
    assert_runtime_child_connects("bun").await;
}

#[tokio::test]
async fn configured_bun_host_connects_and_completes_authenticated_handshake() {
    let Some(entry) = std::env::var_os("OPENBITFUN_TEST_BUN_HOST_ENTRY").map(PathBuf::from) else {
        return;
    };
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let working_directory = entry
        .parent()
        .expect("configured Bun host entry should have a parent")
        .to_path_buf();

    let mut host = PluginHost::start(PluginHostConfig {
        runtime_command: PathBuf::from("bun"),
        entry,
        working_directory,
        cache_directory: directory.path().join("cache"),
        log_file: directory.path().join("plugin-host.log"),
        log_level: "debug".to_string(),
    })
    .await
    .expect("configured Bun host should complete handshake");

    assert!(host.is_connected().expect("host status should be readable"));
}

#[tokio::test]
async fn child_stdout_and_stderr_are_written_to_plugin_host_log() {
    let runtime_available = Command::new("node")
        .arg("--version")
        .output()
        .await
        .is_ok_and(|output| output.status.success());
    if !runtime_available {
        return;
    }
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let script = directory.path().join("logging-host.mjs");
    let log_file = directory.path().join("logs").join("plugin-host.log");
    tokio::fs::write(
        &script,
        r#"import net from "node:net";
console.log("fixture stdout");
console.error("fixture stderr");
console.error(`fixture level=${process.env.OPENCODE_EXTENSION_HOST_LOG_LEVEL}`);
const [host, port] = process.env.OPENCODE_EXTENSION_HOST_RPC_ADDRESS.split(":");
const socket = net.createConnection({ host, port: Number(port) });
const request = Buffer.from(JSON.stringify({
  jsonrpc: "2.0",
  id: "host:1",
  method: "backend.handshake",
  params: {
    token: process.env.OPENCODE_EXTENSION_HOST_RPC_TOKEN,
    protocolVersion: 1,
    opencodeVersion: "1.17.18",
    maxFrameBytes: 16777216
  }
}));
const header = Buffer.alloc(4);
header.writeUInt32BE(request.length);
socket.write(Buffer.concat([header, request]));
"#,
    )
    .await
    .expect("fake plugin host should be written");

    let host = PluginHost::start(PluginHostConfig {
        runtime_command: PathBuf::from("node"),
        entry: script,
        working_directory: directory.path().to_path_buf(),
        cache_directory: directory.path().join("cache"),
        log_file: log_file.clone(),
        log_level: "info".to_string(),
    })
    .await
    .expect("runtime child should complete handshake");
    for _ in 0..20 {
        let content = tokio::fs::read_to_string(&log_file)
            .await
            .unwrap_or_default();
        if content.contains("[stdout] fixture stdout")
            && content.contains("[stderr] fixture stderr")
            && content.contains("[stderr] fixture level=info")
        {
            drop(host);
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let content = tokio::fs::read_to_string(&log_file)
        .await
        .expect("plugin host log should be readable");
    assert!(content.contains("[stdout] fixture stdout"));
    assert!(content.contains("[stderr] fixture stderr"));
    assert!(content.contains("[stderr] fixture level=info"));
}

#[tokio::test]
async fn plugin_host_shutdown_waits_for_rpc_response_and_process_exit() {
    let runtime_available = Command::new("node")
        .arg("--version")
        .output()
        .await
        .is_ok_and(|output| output.status.success());
    if !runtime_available {
        return;
    }
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let script = directory.path().join("shutdown-host.mjs");
    tokio::fs::write(&script, graceful_shutdown_fixture())
        .await
        .expect("graceful shutdown fixture should be written");
    let log_file = directory.path().join("plugin-host.log");
    let host = PluginHost::start(PluginHostConfig {
        runtime_command: PathBuf::from("node"),
        entry: script,
        working_directory: directory.path().to_path_buf(),
        cache_directory: directory.path().join("cache"),
        log_file: log_file.clone(),
        log_level: "debug".to_string(),
    })
    .await
    .expect("runtime child should complete handshake");
    let descendant_id = tokio::fs::read_to_string(directory.path().join("descendant.pid"))
        .await
        .expect("graceful fixture should publish its descendant id")
        .parse::<u32>()
        .expect("descendant id should be numeric");

    let report = host.shutdown(PluginHostShutdownPolicy::default()).await;

    assert_eq!(report.disposition, PluginHostShutdownDisposition::Graceful);
    assert!(report.rpc_completed);
    assert_eq!(report.exit_code, Some(0));
    assert!(
        !process_is_running(descendant_id).await,
        "graceful shutdown must reap the whole process tree before returning"
    );
    let log = tokio::fs::read_to_string(log_file)
        .await
        .expect("plugin host shutdown log should be readable");
    assert!(log.contains("[stdout] fixture shutdown complete"));
}

#[tokio::test]
async fn plugin_host_shutdown_reports_a_nonzero_exit_as_not_graceful() {
    let runtime_available = Command::new("node")
        .arg("--version")
        .output()
        .await
        .is_ok_and(|output| output.status.success());
    if !runtime_available {
        return;
    }
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let script = directory.path().join("failed-shutdown-host.mjs");
    tokio::fs::write(&script, failed_shutdown_fixture())
        .await
        .expect("failed shutdown fixture should be written");
    let host = PluginHost::start(PluginHostConfig {
        runtime_command: PathBuf::from("node"),
        entry: script,
        working_directory: directory.path().to_path_buf(),
        cache_directory: directory.path().join("cache"),
        log_file: directory.path().join("plugin-host.log"),
        log_level: "debug".to_string(),
    })
    .await
    .expect("runtime child should complete handshake");

    let report = host.shutdown(PluginHostShutdownPolicy::default()).await;

    assert_eq!(
        report.disposition,
        PluginHostShutdownDisposition::ExitedAfterShutdown
    );
    assert!(report.rpc_completed);
    assert_eq!(report.exit_code, Some(7));
}

#[tokio::test]
async fn plugin_host_shutdown_forces_a_host_that_ignores_shutdown_and_eof() {
    let runtime_available = Command::new("node")
        .arg("--version")
        .output()
        .await
        .is_ok_and(|output| output.status.success());
    if !runtime_available {
        return;
    }
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let script = directory.path().join("hanging-shutdown-host.mjs");
    tokio::fs::write(&script, hanging_shutdown_fixture())
        .await
        .expect("hanging shutdown fixture should be written");
    let host = PluginHost::start(PluginHostConfig {
        runtime_command: PathBuf::from("node"),
        entry: script,
        working_directory: directory.path().to_path_buf(),
        cache_directory: directory.path().join("cache"),
        log_file: directory.path().join("plugin-host.log"),
        log_level: "debug".to_string(),
    })
    .await
    .expect("runtime child should complete handshake");
    let policy = PluginHostShutdownPolicy {
        drain_timeout: Duration::from_millis(50),
        rpc_timeout: Duration::from_millis(50),
        exit_timeout: Duration::from_millis(50),
        eof_timeout: Duration::from_millis(50),
        terminate_grace: Duration::from_millis(50),
    };

    let report = host.shutdown(policy).await;

    assert_eq!(report.disposition, PluginHostShutdownDisposition::Forced);
    assert!(!report.rpc_completed);
    assert!(report.duration_ms < 2_000);
}

async fn assert_runtime_child_connects(runtime_command: &str) {
    let runtime_available = Command::new(runtime_command)
        .arg("--version")
        .output()
        .await
        .is_ok_and(|output| output.status.success());
    if !runtime_available {
        return;
    }
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let script = directory.path().join("fake-host.mjs");
    tokio::fs::write(
        &script,
        r#"import net from "node:net";
const [host, port] = process.env.OPENCODE_EXTENSION_HOST_RPC_ADDRESS.split(":");
const socket = net.createConnection({ host, port: Number(port) });
const request = Buffer.from(JSON.stringify({
  jsonrpc: "2.0",
  id: "host:1",
  method: "backend.handshake",
  params: {
    token: process.env.OPENCODE_EXTENSION_HOST_RPC_TOKEN,
    protocolVersion: 1,
    opencodeVersion: "1.17.18",
    maxFrameBytes: 16777216
  }
}));
const header = Buffer.alloc(4);
header.writeUInt32BE(request.length);
socket.write(Buffer.concat([header, request]));
"#,
    )
    .await
    .expect("fake plugin host should be written");

    let mut host = PluginHost::start(PluginHostConfig {
        runtime_command: PathBuf::from(runtime_command),
        entry: script,
        working_directory: directory.path().to_path_buf(),
        cache_directory: directory.path().join("cache"),
        log_file: directory.path().join("plugin-host.log"),
        log_level: "debug".to_string(),
    })
    .await
    .expect("runtime child should complete handshake");

    assert!(host.is_connected().expect("host status should be readable"));
}

fn graceful_shutdown_fixture() -> &'static str {
    r#"import fs from "node:fs";
import net from "node:net";
import { spawn } from "node:child_process";
const descendant = spawn(process.execPath, ["-e", "setInterval(() => {}, 1000)"], { stdio: "ignore" });
descendant.unref();
fs.writeFileSync("descendant.pid", String(descendant.pid));
const [host, port] = process.env.OPENCODE_EXTENSION_HOST_RPC_ADDRESS.split(":");
const socket = net.createConnection({ host, port: Number(port) });
let buffer = Buffer.alloc(0);
function send(message) {
  const payload = Buffer.from(JSON.stringify(message));
  const header = Buffer.alloc(4);
  header.writeUInt32BE(payload.length);
  socket.write(Buffer.concat([header, payload]));
}
socket.on("connect", () => send({
  jsonrpc: "2.0",
  id: "host:1",
  method: "backend.handshake",
  params: {
    token: process.env.OPENCODE_EXTENSION_HOST_RPC_TOKEN,
    protocolVersion: 1,
    opencodeVersion: "1.17.18",
    maxFrameBytes: 16777216
  }
}));
socket.on("data", (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);
  while (buffer.length >= 4) {
    const length = buffer.readUInt32BE(0);
    if (buffer.length < length + 4) return;
    const message = JSON.parse(buffer.subarray(4, length + 4).toString());
    buffer = buffer.subarray(length + 4);
    if (message.method === "host.shutdown") {
      console.log("fixture shutdown complete");
      send({ jsonrpc: "2.0", id: message.id, result: { closed: true } });
      socket.end();
    }
  }
});
"#
}

fn hanging_shutdown_fixture() -> &'static str {
    r#"import net from "node:net";
const [host, port] = process.env.OPENCODE_EXTENSION_HOST_RPC_ADDRESS.split(":");
const socket = net.createConnection({ host, port: Number(port) });
let buffer = Buffer.alloc(0);
function send(message) {
  const payload = Buffer.from(JSON.stringify(message));
  const header = Buffer.alloc(4);
  header.writeUInt32BE(payload.length);
  socket.write(Buffer.concat([header, payload]));
}
socket.on("connect", () => send({
  jsonrpc: "2.0",
  id: "host:1",
  method: "backend.handshake",
  params: {
    token: process.env.OPENCODE_EXTENSION_HOST_RPC_TOKEN,
    protocolVersion: 1,
    opencodeVersion: "1.17.18",
    maxFrameBytes: 16777216
  }
}));
socket.on("data", (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);
});
setInterval(() => {}, 1000);
"#
}

fn failed_shutdown_fixture() -> &'static str {
    r#"import net from "node:net";
const [host, port] = process.env.OPENCODE_EXTENSION_HOST_RPC_ADDRESS.split(":");
const socket = net.createConnection({ host, port: Number(port) });
let buffer = Buffer.alloc(0);
function send(message) {
  const payload = Buffer.from(JSON.stringify(message));
  const header = Buffer.alloc(4);
  header.writeUInt32BE(payload.length);
  socket.write(Buffer.concat([header, payload]));
}
socket.on("connect", () => send({
  jsonrpc: "2.0",
  id: "host:1",
  method: "backend.handshake",
  params: {
    token: process.env.OPENCODE_EXTENSION_HOST_RPC_TOKEN,
    protocolVersion: 1,
    opencodeVersion: "1.17.18",
    maxFrameBytes: 16777216
  }
}));
socket.on("data", (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);
  while (buffer.length >= 4) {
    const length = buffer.readUInt32BE(0);
    if (buffer.length < length + 4) return;
    const message = JSON.parse(buffer.subarray(4, length + 4).toString());
    buffer = buffer.subarray(length + 4);
    if (message.method === "host.shutdown") {
      send({ jsonrpc: "2.0", id: message.id, result: { closed: true } });
      process.exitCode = 7;
      socket.end();
    }
  }
});
"#
}
