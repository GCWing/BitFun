mod support;

use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use support::{CliTestEnvironment, MockOpenAiServer};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

struct AppServerProcess {
    child: tokio::process::Child,
    stdin: Option<tokio::process::ChildStdin>,
    stdout: Option<BufReader<tokio::process::ChildStdout>>,
    stderr_reader: tokio::task::JoinHandle<String>,
}

impl AppServerProcess {
    async fn spawn(environment: &CliTestEnvironment) -> Self {
        let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_openbitfun"));
        command
            .arg("server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        environment.apply_tokio_environment(&mut command);

        let mut child = command.spawn().expect("start production app-server Host");
        let stdin = child.stdin.take().expect("app-server stdin");
        let stdout = BufReader::new(child.stdout.take().expect("app-server stdout"));
        let mut stderr = child.stderr.take().expect("app-server stderr");
        let stderr_reader = tokio::spawn(async move {
            let mut bytes = Vec::new();
            stderr
                .read_to_end(&mut bytes)
                .await
                .expect("read app-server stderr");
            String::from_utf8_lossy(&bytes).into_owned()
        });

        Self {
            child,
            stdin: Some(stdin),
            stdout: Some(stdout),
            stderr_reader,
        }
    }

    async fn initialize(&mut self) -> Value {
        self.request(
            1,
            "app/initialize",
            json!({
                "protocolVersion": 3,
                "client": { "name": "stdio-host-test", "version": "0.0.1" }
            }),
        )
        .await
    }

    async fn request(&mut self, id: i64, method: &str, params: Value) -> Value {
        self.send_request(id, method, params).await;
        self.read_response(id, method).await
    }

    async fn send_request(&mut self, id: i64, method: &str, params: Value) {
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let mut request = serde_json::to_vec(&request).expect("serialize app-server request");
        request.push(b'\n');
        let stdin = self
            .stdin
            .as_mut()
            .expect("app-server stdin remains available");
        stdin
            .write_all(&request)
            .await
            .expect("write app-server request");
        stdin.flush().await.expect("flush app-server request");
    }

    async fn read_response(&mut self, expected_id: i64, operation: &str) -> Value {
        self.read_until(operation, |message| {
            message.get("id").and_then(Value::as_i64) == Some(expected_id)
        })
        .await
    }

    async fn read_until(&mut self, operation: &str, predicate: impl Fn(&Value) -> bool) -> Value {
        loop {
            let mut line = String::new();
            let bytes_read = tokio::time::timeout(
                Duration::from_secs(60),
                self.stdout
                    .as_mut()
                    .expect("app-server stdout remains available")
                    .read_line(&mut line),
            )
            .await
            .unwrap_or_else(|_| panic!("app-server {operation} timed out"))
            .expect("read app-server stdout");
            assert_ne!(
                bytes_read, 0,
                "app-server stdout closed while waiting for {operation}"
            );

            let message: Value = serde_json::from_str(&line).unwrap_or_else(|error| {
                panic!("app-server stdout contained non-JSON data: {error}: {line}")
            });
            if predicate(&message) {
                return message;
            }
            assert!(
                message.get("method").is_some() || message.get("error").is_some(),
                "unexpected app-server response while waiting for {operation}: {message}"
            );
        }
    }

    async fn wait_for_exit(&mut self, operation: &str) -> std::process::ExitStatus {
        tokio::time::timeout(Duration::from_secs(10), self.child.wait())
            .await
            .unwrap_or_else(|_| panic!("app-server Host did not exit after {operation}"))
            .expect("wait for app-server Host")
    }

    async fn shutdown(mut self) -> String {
        drop(self.stdin.take());
        drop(self.stdout.take());
        let status = self.wait_for_exit("stdin close").await;
        assert!(status.success(), "app-server Host exited with {status}");
        self.stderr_reader
            .await
            .expect("join app-server stderr reader")
    }
}

fn advertised_methods(initialize: &Value) -> Vec<String> {
    initialize
        .pointer("/result/capabilities")
        .and_then(Value::as_array)
        .expect("capabilities array")
        .iter()
        .filter_map(|capability| capability.get("methods"))
        .flat_map(|methods| methods.as_array().expect("methods array").iter())
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

#[tokio::test]
async fn app_server_stdio_initializes_and_advertises_only_served_capabilities() {
    let model = MockOpenAiServer::immediate();
    let environment = CliTestEnvironment::new();
    environment.initialize_git_repository();
    environment.configure_mock_model(model.base_url());

    let mut server = AppServerProcess::spawn(&environment).await;
    let initialize = server.initialize().await;
    assert!(initialize.get("error").is_none(), "{initialize}");
    assert_eq!(
        initialize.pointer("/result/server/name"),
        Some(&json!("openbitfun-app-server"))
    );
    assert_eq!(
        initialize.pointer("/result/limits/maxFrameBytes"),
        Some(&json!(16777216))
    );

    let methods = advertised_methods(&initialize);
    assert!(
        methods.contains(&"session/reloadContext".to_string()),
        "context reload must be advertised when the CLI provides the reviewed compatibility port"
    );
    for served in [
        "agent/createSession",
        "session/sync",
        "search/sessionContent",
        "git/isRepository",
        "config/getConfig",
        "i18n/getCurrentLanguage",
        "externalHook/snapshot",
        "nativeHook/overview",
        "skill/list",
        "subagent/list",
        "externalSource/snapshot",
    ] {
        assert!(
            methods.contains(&served.to_string()),
            "{served} must be advertised by the stdio Host"
        );
    }
    for denied in [
        "config/setConfig",
        "config/saveCloudSpeechConfig",
        "config/setAgentProfileConfig",
        "config/resetAgentProfileConfig",
        "i18n/setLanguage",
        "i18n/setConfig",
        "model/list",
        "model/get",
        "model/add",
        "model/update",
        "model/delete",
        "model/setDefault",
        "skill/setEnabled",
        "subagent/setEnabled",
        "mcp/list",
        "account/snapshot",
        "settingsSync/snapshot",
        "worktree/bindSession",
        "externalSource/control",
        "externalSource/review",
        "externalSource/setNativeCommandChoice",
        "externalSource/expandCommand",
        "externalSource/apply",
        "externalHook/plan",
        "externalHook/apply",
        "externalHook/mutate",
    ] {
        assert!(
            !methods.contains(&denied.to_string()),
            "{denied} must not be advertised by the stdio Host"
        );
    }

    let health = server.request(2, "app/health", json!({})).await;
    assert_eq!(health.pointer("/result/status"), Some(&json!("ready")));

    let stderr = server.shutdown().await;
    assert!(
        !stderr.contains("panicked"),
        "app-server Host panicked: {stderr}"
    );
}

#[tokio::test]
async fn app_server_stdio_fails_closed_on_scope_and_allowlist() {
    let model = MockOpenAiServer::immediate();
    let environment = CliTestEnvironment::new();
    environment.initialize_git_repository();
    environment.configure_mock_model(model.base_url());

    let mut server = AppServerProcess::spawn(&environment).await;
    let initialize = server.initialize().await;
    assert!(initialize.get("error").is_none(), "{initialize}");

    let outside = environment
        .workspace()
        .parent()
        .expect("workspace parent")
        .join("outside")
        .to_string_lossy()
        .to_string();
    let denied_path = server
        .request(2, "git/isRepository", json!({ "repositoryPath": outside }))
        .await;
    assert_eq!(denied_path.pointer("/error/code"), Some(&json!(-32602)));
    assert_eq!(
        denied_path.pointer("/error/data/reason"),
        Some(&json!("path_outside_workspace_scope"))
    );

    let denied_method = server
        .request(
            3,
            "config/setConfig",
            json!({ "configId": "mode", "value": "Cowork" }),
        )
        .await;
    assert_eq!(denied_method.pointer("/error/code"), Some(&json!(-32601)));
    assert_eq!(
        denied_method.pointer("/error/data/reason"),
        Some(&json!("method_not_allowed_by_host_policy"))
    );

    let stderr = server.shutdown().await;
    assert!(
        !stderr.contains("panicked"),
        "app-server Host panicked: {stderr}"
    );
}

#[tokio::test]
async fn app_server_stdio_exits_deterministically_on_stdin_eof() {
    let model = MockOpenAiServer::immediate();
    let environment = CliTestEnvironment::new();
    environment.initialize_git_repository();
    environment.configure_mock_model(model.base_url());

    let mut server = AppServerProcess::spawn(&environment).await;
    let initialize = server.initialize().await;
    assert!(initialize.get("error").is_none(), "{initialize}");

    drop(server.stdin.take());
    drop(server.stdout.take());
    let status = server.wait_for_exit("stdin EOF").await;
    assert!(status.success(), "app-server Host exited with {status}");

    let stderr = server
        .stderr_reader
        .await
        .expect("join app-server stderr reader");
    assert!(
        !stderr.contains("panicked"),
        "app-server Host panicked: {stderr}"
    );
}

#[tokio::test]
async fn app_server_stdio_fails_closed_on_oversized_frames() {
    let model = MockOpenAiServer::immediate();
    let environment = CliTestEnvironment::new();
    environment.initialize_git_repository();
    environment.configure_mock_model(model.base_url());

    let mut server = AppServerProcess::spawn(&environment).await;
    let initialize = server.initialize().await;
    assert!(initialize.get("error").is_none(), "{initialize}");

    // 16 MiB + 1 bytes with no newline exceeds the advertised frame limit.
    let oversized: Vec<u8> = vec![b'x'; 16 * 1024 * 1024 + 1];
    let mut stdin = server.stdin.take().expect("app-server stdin");
    stdin
        .write_all(&oversized)
        .await
        .expect("write oversized frame");
    stdin.flush().await.expect("flush oversized frame");
    drop(stdin);

    let error = server
        .read_until("frame limit error", |message| {
            message.pointer("/error/data/reason") == Some(&json!("frame_exceeds_host_limit"))
        })
        .await;
    assert_eq!(error.pointer("/error/code"), Some(&json!(-32602)));

    drop(server.stdout.take());
    let status = server.wait_for_exit("oversized frame").await;
    assert!(
        !status.success(),
        "oversized frames must terminate the Host with failure"
    );

    let stderr = server
        .stderr_reader
        .await
        .expect("join app-server stderr reader");
    assert!(
        !stderr.contains("panicked"),
        "app-server Host panicked: {stderr}"
    );
}
