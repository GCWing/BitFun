use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use bitfun_agent_runtime::sdk::{
    AgentRuntimeBuilder, AgentSessionClosePort, AgentSessionCreateRequest,
    AgentSessionCreateResult, AgentSessionDeleteRequest, AgentSessionListRequest,
    AgentSessionManagementPort, AgentSessionSummary, AgentSessionWorkspaceBinding,
    AgentSessionWorkspaceRequest, AgentSubmissionPort, AgentSubmissionRequest,
    AgentSubmissionResult, AgentTransientSessionDiscardRequest, PortResult,
};
use bitfun_sdk_host_app::transport::{serve_streams, SdkHostTransportConfig};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Notify;
use tokio::time::{timeout, Duration};

struct MinimalOwner;

struct BlockingCreateOwner {
    calls: AtomicUsize,
    deleted: AtomicUsize,
    release: Notify,
}

impl BlockingCreateOwner {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            deleted: AtomicUsize::new(0),
            release: Notify::new(),
        }
    }
}

#[async_trait]
impl AgentSubmissionPort for MinimalOwner {
    async fn create_session(
        &self,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult> {
        Ok(AgentSessionCreateResult {
            session_id: "unused".to_string(),
            session_name: request.session_name,
            agent_type: request.agent_type,
        })
    }

    async fn create_session_with_id(
        &self,
        session_id: String,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult> {
        Ok(AgentSessionCreateResult {
            session_id,
            session_name: request.session_name,
            agent_type: request.agent_type,
        })
    }

    async fn create_transient_session_with_id(
        &self,
        session_id: String,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult> {
        self.create_session_with_id(session_id, request).await
    }

    async fn submit_message(
        &self,
        request: AgentSubmissionRequest,
    ) -> PortResult<AgentSubmissionResult> {
        Ok(AgentSubmissionResult {
            turn_id: request.turn_id.unwrap_or_else(|| "unused".to_string()),
            accepted: true,
        })
    }

    async fn resolve_session_agent_type(&self, _session_id: &str) -> PortResult<Option<String>> {
        Ok(Some("agentic".to_string()))
    }
}

#[async_trait]
impl AgentSessionClosePort for MinimalOwner {
    async fn discard_transient_session(
        &self,
        _request: AgentTransientSessionDiscardRequest,
    ) -> PortResult<bool> {
        Ok(false)
    }
}

#[async_trait]
impl AgentSubmissionPort for BlockingCreateOwner {
    async fn create_session(
        &self,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult> {
        if self.calls.fetch_add(1, Ordering::AcqRel) == 0 {
            self.release.notified().await;
        }
        Ok(AgentSessionCreateResult {
            session_id: "session-blocking".to_string(),
            session_name: request.session_name,
            agent_type: request.agent_type,
        })
    }

    async fn create_session_with_id(
        &self,
        session_id: String,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult> {
        if self.calls.fetch_add(1, Ordering::AcqRel) == 0 {
            self.release.notified().await;
        }
        Ok(AgentSessionCreateResult {
            session_id,
            session_name: request.session_name,
            agent_type: request.agent_type,
        })
    }

    async fn create_transient_session_with_id(
        &self,
        session_id: String,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult> {
        self.create_session_with_id(session_id, request).await
    }

    async fn submit_message(
        &self,
        request: AgentSubmissionRequest,
    ) -> PortResult<AgentSubmissionResult> {
        Ok(AgentSubmissionResult {
            turn_id: request.turn_id.unwrap_or_else(|| "unused".to_string()),
            accepted: true,
        })
    }

    async fn resolve_session_agent_type(&self, _session_id: &str) -> PortResult<Option<String>> {
        Ok(Some("agentic".to_string()))
    }
}

#[async_trait]
impl AgentSessionClosePort for BlockingCreateOwner {
    async fn discard_transient_session(
        &self,
        _request: AgentTransientSessionDiscardRequest,
    ) -> PortResult<bool> {
        self.deleted.fetch_add(1, Ordering::AcqRel);
        Ok(true)
    }
}

#[async_trait]
impl AgentSessionManagementPort for BlockingCreateOwner {
    async fn list_sessions(
        &self,
        _request: AgentSessionListRequest,
    ) -> PortResult<Vec<AgentSessionSummary>> {
        Ok(Vec::new())
    }

    async fn delete_session(&self, _request: AgentSessionDeleteRequest) -> PortResult<()> {
        self.deleted.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    async fn resolve_session_workspace_binding(
        &self,
        _request: AgentSessionWorkspaceRequest,
    ) -> PortResult<Option<AgentSessionWorkspaceBinding>> {
        Ok(None)
    }
}

#[tokio::test]
async fn stdio_transport_serves_initialize_and_shutdown_without_non_protocol_stdout() {
    let owner = Arc::new(MinimalOwner);
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_session_close_port(owner)
        .build()
        .unwrap();
    let (client, server) = tokio::io::duplex(16 * 1024);
    let (client_read, mut client_write) = tokio::io::split(client);
    let (server_read, server_write) = tokio::io::split(server);
    let task = tokio::spawn(serve_streams(
        runtime,
        "D:/workspace/project",
        server_read,
        server_write,
        SdkHostTransportConfig::default(),
    ));
    client_write
        .write_all(
            concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1,\"clientInfo\":{\"name\":\"fixture\",\"version\":\"0.1\"},\"capabilities\":{\"serverNotifications\":true}}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"shutdown\",\"params\":{}}\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    client_write.shutdown().await.unwrap();

    let mut lines = BufReader::new(client_read).lines();
    let initialized: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    let shutdown: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();

    assert_eq!(initialized["id"], 1);
    assert_eq!(initialized["result"]["protocolVersion"], 1);
    assert_eq!(shutdown["id"], 2);
    assert_eq!(shutdown["result"]["accepted"], true);
    assert!(lines.next_line().await.unwrap().is_none());
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn stdio_transport_executes_json_rpc_notifications_without_replying() {
    let owner = Arc::new(MinimalOwner);
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_session_close_port(owner)
        .build()
        .unwrap();
    let (client, server) = tokio::io::duplex(16 * 1024);
    let (client_read, mut client_write) = tokio::io::split(client);
    let (server_read, server_write) = tokio::io::split(server);
    let task = tokio::spawn(serve_streams(
        runtime,
        "D:/workspace/project",
        server_read,
        server_write,
        SdkHostTransportConfig::default(),
    ));
    client_write
        .write_all(
            concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1,\"clientInfo\":{\"name\":\"fixture\",\"version\":\"0.1\"},\"capabilities\":{\"serverNotifications\":true}}}\n",
                "{\"jsonrpc\":\"2.0\",\"method\":\"shutdown\",\"params\":{}}\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    client_write.shutdown().await.unwrap();

    let mut lines = BufReader::new(client_read).lines();
    let initialized: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(initialized["id"], 1);
    timeout(Duration::from_secs(1), task)
        .await
        .expect("shutdown notification should execute")
        .unwrap()
        .unwrap();
    assert!(lines.next_line().await.unwrap().is_none());
}

#[tokio::test]
async fn malformed_and_oversized_lines_fail_closed_with_standard_parse_errors() {
    let owner = Arc::new(MinimalOwner);
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_session_close_port(owner)
        .build()
        .unwrap();
    let (client, server) = tokio::io::duplex(16 * 1024);
    let (client_read, mut client_write) = tokio::io::split(client);
    let (server_read, server_write) = tokio::io::split(server);
    let task = tokio::spawn(serve_streams(
        runtime,
        "D:/workspace/project",
        server_read,
        server_write,
        SdkHostTransportConfig {
            max_line_bytes: 128,
            ..SdkHostTransportConfig::default()
        },
    ));
    client_write.write_all(b"not-json\n").await.unwrap();
    client_write
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":7}\n")
        .await
        .unwrap();
    client_write
        .write_all(format!("{}\n", "x".repeat(256)).as_bytes())
        .await
        .unwrap();
    client_write.shutdown().await.unwrap();

    let mut lines = BufReader::new(client_read).lines();
    let malformed: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    let invalid_request: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    let oversized: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(malformed["id"], serde_json::Value::Null);
    assert_eq!(malformed["error"]["code"], -32700);
    assert_eq!(malformed["error"]["data"]["code"], "invalid_request");
    assert_eq!(invalid_request["id"], 7);
    assert_eq!(invalid_request["error"]["code"], -32600);
    assert_eq!(invalid_request["error"]["data"]["code"], "invalid_request");
    assert_eq!(oversized["error"]["code"], -32700);
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn transport_accepts_input_while_an_owner_call_is_pending_and_bounds_requests() {
    let owner = Arc::new(BlockingCreateOwner::new());
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .build()
        .unwrap();
    let (client, server) = tokio::io::duplex(16 * 1024);
    let (client_read, mut client_write) = tokio::io::split(client);
    let (server_read, server_write) = tokio::io::split(server);
    let task = tokio::spawn(serve_streams(
        runtime,
        "D:/workspace/project",
        server_read,
        server_write,
        SdkHostTransportConfig {
            host: bitfun_sdk_host::host::SdkHostConfig {
                max_in_flight_requests: 1,
                ..Default::default()
            },
            ..Default::default()
        },
    ));
    client_write
        .write_all(
            concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1,\"clientInfo\":{\"name\":\"fixture\",\"version\":\"0.1\"},\"capabilities\":{\"serverNotifications\":true}}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"session/create\",\"params\":{}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"session/create\",\"params\":{}}\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();

    let mut lines = BufReader::new(client_read).lines();
    let initialized: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(initialized["id"], 1);
    let overloaded: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(overloaded["id"], 3);
    assert_eq!(overloaded["error"]["data"]["code"], "overloaded");

    owner.release.notify_one();
    let created: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(created["id"], 2);
    client_write
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"session/create\",\"params\":{}}\n")
        .await
        .unwrap();
    let recovered: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(recovered["id"], 4);
    assert!(recovered["result"]["sessionId"].is_string());
    client_write
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"shutdown\",\"params\":{}}\n")
        .await
        .unwrap();
    client_write.shutdown().await.unwrap();
    let shutdown: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(shutdown["id"], 5);
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn shutdown_remains_available_when_the_data_request_budget_is_exhausted() {
    let owner = Arc::new(BlockingCreateOwner::new());
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .build()
        .unwrap();
    let (client, server) = tokio::io::duplex(16 * 1024);
    let (client_read, mut client_write) = tokio::io::split(client);
    let (server_read, server_write) = tokio::io::split(server);
    let task = tokio::spawn(serve_streams(
        runtime,
        "D:/workspace/project",
        server_read,
        server_write,
        SdkHostTransportConfig {
            shutdown_total_timeout_ms: 500,
            host: bitfun_sdk_host::host::SdkHostConfig {
                max_in_flight_requests: 1,
                ..Default::default()
            },
            ..Default::default()
        },
    ));
    client_write
        .write_all(
            b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1,\"clientInfo\":{\"name\":\"fixture\",\"version\":\"0.1\"},\"capabilities\":{\"serverNotifications\":true}}}\n",
        )
        .await
        .unwrap();
    let mut lines = BufReader::new(client_read).lines();
    let initialized: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(initialized["id"], 1);

    client_write
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"session/create\",\"params\":{}}\n")
        .await
        .unwrap();
    timeout(Duration::from_secs(1), async {
        while owner.calls.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("blocking data request must start");

    client_write
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"shutdown\",\"params\":{}}\n")
        .await
        .unwrap();
    client_write.shutdown().await.unwrap();

    let shutdown: serde_json::Value = serde_json::from_str(
        &timeout(Duration::from_secs(1), lines.next_line())
            .await
            .expect("shutdown must bypass exhausted data request capacity")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(shutdown["id"], 3);
    assert_eq!(shutdown["result"]["accepted"], true);
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn duplicate_initialize_does_not_abort_an_in_flight_request() {
    let owner = Arc::new(BlockingCreateOwner::new());
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .build()
        .unwrap();
    let (client, server) = tokio::io::duplex(16 * 1024);
    let (client_read, mut client_write) = tokio::io::split(client);
    let (server_read, server_write) = tokio::io::split(server);
    let task = tokio::spawn(serve_streams(
        runtime,
        "D:/workspace/project",
        server_read,
        server_write,
        SdkHostTransportConfig::default(),
    ));
    client_write
        .write_all(
            concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1,\"clientInfo\":{\"name\":\"fixture\",\"version\":\"0.1\"},\"capabilities\":{\"serverNotifications\":true}}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"session/create\",\"params\":{}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1,\"clientInfo\":{\"name\":\"fixture\",\"version\":\"0.1\"},\"capabilities\":{\"serverNotifications\":true}}}\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();

    let mut lines = BufReader::new(client_read).lines();
    let initialized: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(initialized["id"], 1);
    let duplicate: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(duplicate["id"], 3);
    assert_eq!(duplicate["error"]["data"]["code"], "already_initialized");

    owner.release.notify_one();
    let created: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(created["id"], 2);
    client_write
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"shutdown\",\"params\":{}}\n")
        .await
        .unwrap();
    client_write.shutdown().await.unwrap();
    let shutdown: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(shutdown["id"], 4);
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn connection_eof_cleans_a_session_created_after_its_request_is_aborted() {
    let owner = Arc::new(BlockingCreateOwner::new());
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .build()
        .unwrap();
    let (client, server) = tokio::io::duplex(16 * 1024);
    let (client_read, mut client_write) = tokio::io::split(client);
    let (server_read, server_write) = tokio::io::split(server);
    let mut task = tokio::spawn(serve_streams(
        runtime,
        "D:/workspace/project",
        server_read,
        server_write,
        SdkHostTransportConfig {
            shutdown_total_timeout_ms: 100,
            ..SdkHostTransportConfig::default()
        },
    ));
    client_write
        .write_all(
            concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1,\"clientInfo\":{\"name\":\"fixture\",\"version\":\"0.1\"},\"capabilities\":{\"serverNotifications\":true}}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"session/create\",\"params\":{}}\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut lines = BufReader::new(client_read).lines();
    let initialized: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(initialized["id"], 1);
    timeout(Duration::from_secs(1), async {
        while owner.calls.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("session creation should start");

    client_write.shutdown().await.unwrap();
    timeout(Duration::from_secs(1), &mut task)
        .await
        .expect("transient Session cleanup must stay within the Host deadline")
        .unwrap()
        .unwrap();
    assert_eq!(owner.deleted.load(Ordering::Acquire), 1);
    assert!(lines.next_line().await.unwrap().is_none());
}

#[tokio::test]
async fn explicit_shutdown_bounds_request_drain_and_transient_cleanup_together() {
    let owner = Arc::new(BlockingCreateOwner::new());
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_session_management_port(owner.clone())
        .with_session_close_port(owner.clone())
        .build()
        .unwrap();
    let (client, server) = tokio::io::duplex(16 * 1024);
    let (client_read, mut client_write) = tokio::io::split(client);
    let (server_read, server_write) = tokio::io::split(server);
    let mut task = tokio::spawn(serve_streams(
        runtime,
        "D:/workspace/project",
        server_read,
        server_write,
        SdkHostTransportConfig {
            shutdown_total_timeout_ms: 100,
            ..SdkHostTransportConfig::default()
        },
    ));
    client_write
        .write_all(
            concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1,\"clientInfo\":{\"name\":\"fixture\",\"version\":\"0.1\"},\"capabilities\":{\"serverNotifications\":true}}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"session/create\",\"params\":{}}\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut lines = BufReader::new(client_read).lines();
    let initialized: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(initialized["id"], 1);
    timeout(Duration::from_secs(1), async {
        while owner.calls.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("session creation should start");

    client_write
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"shutdown\",\"params\":{}}\n")
        .await
        .unwrap();
    let shutdown: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(shutdown["id"], 3);
    timeout(Duration::from_secs(1), &mut task)
        .await
        .expect("request drain and transient cleanup must share one total deadline")
        .unwrap()
        .unwrap();
    assert_eq!(owner.deleted.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn requests_before_a_successful_initialize_cannot_cross_the_handshake() {
    let owner = Arc::new(MinimalOwner);
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_session_close_port(owner)
        .build()
        .unwrap();
    let (client, server) = tokio::io::duplex(16 * 1024);
    let (client_read, mut client_write) = tokio::io::split(client);
    let (server_read, server_write) = tokio::io::split(server);
    let task = tokio::spawn(serve_streams(
        runtime,
        "D:/workspace/project",
        server_read,
        server_write,
        SdkHostTransportConfig::default(),
    ));
    client_write
        .write_all(
            concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":999,\"clientInfo\":{\"name\":\"fixture\",\"version\":\"0.1\"},\"capabilities\":{\"serverNotifications\":true}}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"session/create\",\"params\":{}}\n",
                "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1,\"clientInfo\":{\"name\":\"fixture\",\"version\":\"0.1\"},\"capabilities\":{\"serverNotifications\":true}}}\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();

    let mut lines = BufReader::new(client_read).lines();
    let mismatch: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    let pre_initialize: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    let initialized: serde_json::Value =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(mismatch["id"], 1);
    assert_eq!(mismatch["error"]["data"]["code"], "version_mismatch");
    assert_eq!(pre_initialize["id"], 2);
    assert_eq!(pre_initialize["error"]["data"]["code"], "not_initialized");
    assert_eq!(initialized["id"], 3);
    assert_eq!(initialized["result"]["protocolVersion"], 1);

    client_write
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"shutdown\",\"params\":{}}\n")
        .await
        .unwrap();
    client_write.shutdown().await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&lines.next_line().await.unwrap().unwrap())
            .unwrap()["id"],
        4
    );
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn blocked_output_times_out_and_ends_the_connection() {
    let owner = Arc::new(MinimalOwner);
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(owner.clone())
        .with_session_close_port(owner)
        .build()
        .unwrap();
    let (client, server) = tokio::io::duplex(64);
    let (_client_read, mut client_write) = tokio::io::split(client);
    let (server_read, server_write) = tokio::io::split(server);
    let task = tokio::spawn(serve_streams(
        runtime,
        "D:/workspace/project",
        server_read,
        server_write,
        SdkHostTransportConfig {
            write_timeout_ms: 20,
            ..Default::default()
        },
    ));
    client_write
        .write_all(
            b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1,\"clientInfo\":{\"name\":\"fixture\",\"version\":\"0.1\"},\"capabilities\":{\"serverNotifications\":true}}}\n",
        )
        .await
        .unwrap();

    let result = timeout(Duration::from_secs(1), task)
        .await
        .expect("blocked SDK Host output must have a deadline")
        .unwrap();
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::BrokenPipe);
}
