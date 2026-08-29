use crate::{
    hook_function_runtime, read_frame, read_host_stream, register_backend_handlers, write_frame,
    BackendDiagnosticError, BackendDiagnosticEvent, BackendRouteFailure, BackendRouteRequest,
    HostStreamReadError, JsonRpcPeer, OpenCodeBackendHandler, PluginDeclaration, PluginHostError,
    PluginInstanceOpenRequest, PluginPrepareRequest, StreamDescriptor, DEFAULT_MAX_FRAME_BYTES,
};
use async_trait::async_trait;
use bitfun_runtime_ports::{
    HookFunctionGeneration, HookFunctionRegistrationBatch, HookFunctionRegistrationSink,
    HookFunctionReverseAsk, HookFunctionReverseMetadata, HookFunctionReverseReply,
    HookFunctionReverseSink, HookFunctionStartRequest, HookFunctionToolContext,
    HookFunctionToolRequest, PortErrorKind, PortResult,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

#[derive(Default)]
struct CapturingRegistrationSink {
    batches: Mutex<Vec<HookFunctionRegistrationBatch>>,
}

#[async_trait]
impl HookFunctionRegistrationSink for CapturingRegistrationSink {
    async fn publish_generation(&self, batch: HookFunctionRegistrationBatch) -> PortResult<()> {
        self.batches.lock().expect("batch lock").push(batch);
        Ok(())
    }
}

struct UnusedReverseSink;

struct BlockingBackend {
    started: Arc<tokio::sync::Notify>,
    dropped: Arc<AtomicBool>,
}

struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

#[async_trait]
impl OpenCodeBackendHandler for BlockingBackend {
    async fn handle_route(
        &self,
        _request: BackendRouteRequest,
    ) -> Result<serde_json::Value, BackendRouteFailure> {
        let _drop_flag = DropFlag(self.dropped.clone());
        self.started.notify_one();
        std::future::pending().await
    }

    async fn publish_diagnostic(
        &self,
        _event: BackendDiagnosticEvent,
    ) -> Result<(), BackendDiagnosticError> {
        Ok(())
    }
}

#[async_trait]
impl HookFunctionReverseSink for UnusedReverseSink {
    async fn metadata(&self, _update: HookFunctionReverseMetadata) -> PortResult<()> {
        Ok(())
    }

    async fn ask(&self, _request: HookFunctionReverseAsk) -> PortResult<HookFunctionReverseReply> {
        Ok(HookFunctionReverseReply::Once)
    }
}

#[tokio::test]
async fn hook_runtime_start_publishes_a_typed_complete_generation() {
    let (backend_stream, mut host_stream) = connected_streams().await;
    let peer = JsonRpcPeer::start_with_capabilities(
        backend_stream,
        21,
        DEFAULT_MAX_FRAME_BYTES,
        crate::PluginHostCapabilities::all_supported(),
    );
    let runtime = hook_function_runtime(peer.client());
    let host = tokio::spawn(async move {
        let request = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("instance open request should be readable");
        assert_eq!(request["method"], "host.instance.open");
        write_frame(
            &mut host_stream,
            &json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "result": {
                    "instanceID": "instance-a",
                    "generationKey": "generation-a",
                    "revision": "revision-a",
                    "config": {"model": "fixture"},
                    "configContributors": [],
                    "configContributions": [],
                    "diagnostics": [],
                    "hooks": ["tool.execute.before", "tool.execute.after"],
                    "tools": [{
                        "registrationID": "registration-a",
                        "id": "echo",
                        "description": "Echo input",
                        "parameters": {"type": "object"}
                    }],
                    "auth": [],
                    "providers": [],
                    "workspaces": [],
                    "gatewayURL": "http://127.0.0.1:1234/"
                }
            }),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("instance open response should be written");
    });
    let generation = HookFunctionGeneration {
        instance_id: "instance-a".to_string(),
        generation_key: "generation-a".to_string(),
        revision: "revision-a".to_string(),
    };
    let registrations = Arc::new(CapturingRegistrationSink::default());

    let started = runtime
        .start(
            HookFunctionStartRequest {
                generation: generation.clone(),
                project_id: "project-a".to_string(),
                project_worktree: "C:/workspace".to_string(),
                project_created_at_ms: 42,
                config: serde_json::Map::new(),
                directory: "C:/workspace".to_string(),
                worktree: "C:/workspace".to_string(),
                plugins: Vec::new(),
                configuration_fingerprint: None,
                expected_content_digests: BTreeMap::new(),
                expected_review_digest: None,
            },
            registrations.clone(),
            Arc::new(UnusedReverseSink),
            Duration::from_secs(1),
        )
        .await
        .expect("typed generation should start");

    assert_eq!(started, generation);
    let batches = registrations.batches.lock().expect("batch lock");
    assert_eq!(batches[0].generation, generation);
    assert_eq!(batches[0].tools[0].id, "echo");
    assert_eq!(batches[0].hooks.len(), 2);
    host.await.expect("fake host should finish");
}

#[tokio::test]
async fn hook_runtime_tool_timeout_is_outcome_unknown_even_when_cancel_is_confirmed() {
    let (backend_stream, mut host_stream) = connected_streams().await;
    let peer = JsonRpcPeer::start_with_capabilities(
        backend_stream,
        22,
        DEFAULT_MAX_FRAME_BYTES,
        crate::PluginHostCapabilities::all_supported(),
    );
    let runtime = hook_function_runtime(peer.client());
    let host = tokio::spawn(async move {
        let execute = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("tool execute request should be readable");
        assert_eq!(execute["method"], "host.tool.execute");

        let cancel = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("tool cancel request should be readable");
        assert_eq!(cancel["method"], "host.tool.cancel");
        assert_eq!(cancel["params"]["executionID"], "execution-a");
        write_frame(
            &mut host_stream,
            &json!({
                "jsonrpc": "2.0",
                "id": cancel["id"],
                "result": {"cancelled": true}
            }),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("tool cancel response should be written");
    });

    let error = runtime
        .execute_tool(
            HookFunctionToolRequest {
                generation: HookFunctionGeneration {
                    instance_id: "instance-a".to_string(),
                    generation_key: "generation-a".to_string(),
                    revision: "revision-a".to_string(),
                },
                execution_id: "execution-a".to_string(),
                registration_id: "registration-a".to_string(),
                args: json!({"value": 1}),
                context: HookFunctionToolContext {
                    session_id: "session-a".to_string(),
                    message_id: "message-a".to_string(),
                    agent: "agentic".to_string(),
                    call_id: Some("call-a".to_string()),
                },
            },
            Duration::from_millis(20),
        )
        .await
        .expect_err("a dispatched tool timeout must not claim a definite cancellation");

    assert_eq!(error.kind, PortErrorKind::OutcomeUnknown);
    host.await.expect("fake host should finish");
}

#[tokio::test]
async fn backend_drain_cancels_and_joins_an_admitted_route() {
    let (backend_stream, mut host_stream) = connected_streams().await;
    let peer = JsonRpcPeer::start(backend_stream, 23, DEFAULT_MAX_FRAME_BYTES);
    let started = Arc::new(tokio::sync::Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let bridge = register_backend_handlers(
        peer.client(),
        Arc::new(BlockingBackend {
            started: started.clone(),
            dropped: dropped.clone(),
        }),
    )
    .await
    .expect("backend handlers should register");
    let host = tokio::spawn(async move {
        write_frame(
            &mut host_stream,
            &json!({
                "jsonrpc": "2.0",
                "id": "host:route:1",
                "method": "backend.http.request",
                "params": {
                    "instanceID": "instance-a",
                    "requestID": "request-a",
                    "method": "GET",
                    "path": "/project",
                    "headers": [],
                    "body": null
                }
            }),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("backend route request should be written");
        read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("drained route should receive an RPC response")
    });

    started.notified().await;
    assert!(bridge.begin_draining().await);
    assert!(dropped.load(Ordering::Acquire));
    let response = host.await.expect("fake host should finish");
    assert_eq!(response["error"]["code"], -32000);
}

#[tokio::test]
async fn backend_handlers_do_not_retain_a_dropped_bridge() {
    let (backend_stream, _host_stream) = connected_streams().await;
    let peer = JsonRpcPeer::start(backend_stream, 24, DEFAULT_MAX_FRAME_BYTES);
    let bridge = register_backend_handlers(
        peer.client(),
        Arc::new(BlockingBackend {
            started: Arc::new(tokio::sync::Notify::new()),
            dropped: Arc::new(AtomicBool::new(false)),
        }),
    )
    .await
    .expect("backend handlers should register");
    let weak = Arc::downgrade(&bridge);

    drop(bridge);

    assert!(weak.upgrade().is_none());
}

#[tokio::test]
async fn peer_correlates_out_of_order_responses_by_request_id() {
    let (backend_stream, mut host_stream) = connected_streams().await;
    let peer = JsonRpcPeer::start(backend_stream, 7, DEFAULT_MAX_FRAME_BYTES);
    let client = peer.client();
    let host = tokio::spawn(async move {
        let first = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("first request should be readable");
        let second = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("second request should be readable");
        assert_eq!(first["id"], "backend:7:1");
        assert_eq!(second["id"], "backend:7:2");
        write_frame(
            &mut host_stream,
            &json!({"jsonrpc": "2.0", "id": second["id"], "result": second["params"]}),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("second response should be written first");
        write_frame(
            &mut host_stream,
            &json!({"jsonrpc": "2.0", "id": first["id"], "result": first["params"]}),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("first response should be written second");
    });

    let first = client.request("host.first", json!({"value": 1}), Duration::from_secs(1));
    let second = client.request("host.second", json!({"value": 2}), Duration::from_secs(1));
    let (first_result, second_result) = tokio::join!(first, second);

    assert_eq!(
        first_result.expect("first request should resolve"),
        json!({"value": 1})
    );
    assert_eq!(
        second_result.expect("second request should resolve"),
        json!({"value": 2})
    );
    host.await.expect("fake host should finish");
}

#[tokio::test]
async fn peer_handles_reentrant_host_request_while_backend_request_is_pending() {
    let (backend_stream, mut host_stream) = connected_streams().await;
    let peer = JsonRpcPeer::start(backend_stream, 8, DEFAULT_MAX_FRAME_BYTES);
    let client = peer.client();
    client
        .register_handler("backend.echo", |params| async move { Ok(params) })
        .await
        .expect("handler should register");
    let host = tokio::spawn(async move {
        let backend_request = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("backend request should be readable");
        write_frame(
            &mut host_stream,
            &json!({
                "jsonrpc": "2.0",
                "id": "host:2",
                "method": "backend.echo",
                "params": {"reentrant": true}
            }),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("reentrant request should be written");
        let reentrant_response = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("reentrant response should be readable");
        write_frame(
            &mut host_stream,
            &json!({
                "jsonrpc": "2.0",
                "id": backend_request["id"],
                "result": reentrant_response["result"]
            }),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("backend response should be written");
    });

    let result = client
        .request("host.instance.open", json!({}), Duration::from_secs(1))
        .await
        .expect("backend request should resolve after reentrant request");

    assert_eq!(result, json!({"reentrant": true}));
    host.await.expect("fake host should finish");
}

#[tokio::test]
async fn client_opens_a_typed_plugin_instance() {
    let (backend_stream, mut host_stream) = connected_streams().await;
    let peer = JsonRpcPeer::start(backend_stream, 13, DEFAULT_MAX_FRAME_BYTES);
    let client = peer.client();
    let host = tokio::spawn(async move {
        let request = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("instance open request should be readable");
        assert_eq!(request["method"], "host.instance.open");
        assert_eq!(request["params"]["instanceID"], "bitfun:test-instance");
        assert_eq!(request["params"]["plugins"][0]["spec"], "bitfun-demo-echo");
        write_frame(
            &mut host_stream,
            &json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "result": {
                    "instanceID": "bitfun:test-instance",
                    "generationKey": "generation-fixture",
                    "revision": "revision-fixture"
                }
            }),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("instance open response should be written");
    });

    let result = client
        .open_instance(
            PluginInstanceOpenRequest {
                instance_id: "bitfun:test-instance".to_string(),
                generation_key: "generation-fixture".to_string(),
                revision: "revision-fixture".to_string(),
                project: json!({"id": "project", "worktree": "C:/workspace"}),
                config: serde_json::Map::new(),
                directory: "C:/workspace".to_string(),
                worktree: "C:/workspace".to_string(),
                plugins: vec![PluginDeclaration {
                    spec: "bitfun-demo-echo".to_string(),
                    options: None,
                    base_directory: None,
                }],
                configuration_fingerprint: Some("fixture-open".to_string()),
                expected_content_digests: None,
                expected_review_digest: None,
            },
            Duration::from_secs(1),
        )
        .await
        .expect("instance open should resolve");

    assert_eq!(result["instanceID"], "bitfun:test-instance");
    host.await.expect("fake host should finish");
}

#[tokio::test]
async fn client_prepares_typed_plugins() {
    let (backend_stream, mut host_stream) = connected_streams().await;
    let peer = JsonRpcPeer::start(backend_stream, 14, DEFAULT_MAX_FRAME_BYTES);
    let client = peer.client();
    let host = tokio::spawn(async move {
        let request = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("plugin prepare request should be readable");
        assert_eq!(request["method"], "host.plugins.prepare");
        assert_eq!(
            request["params"]["configurationFingerprint"],
            "fixture-prewarm"
        );
        assert_eq!(request["params"]["plugins"][0]["spec"], "bitfun-demo-echo");
        write_frame(
            &mut host_stream,
            &json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "result": {
                    "reviewDigest": "0".repeat(64),
                    "reviewed": [{
                        "spec": "bitfun-demo-echo",
                        "source": "npm",
                        "identity": "npm:bitfun-demo-echo",
                        "canonicalSource": "bitfun-demo-echo",
                        "baseDirectory": "C:/workspace",
                        "optionsDigest": "1".repeat(64)
                    }],
                    "prepared": [{
                        "spec": "bitfun-demo-echo",
                        "identity": "npm:bitfun-demo-echo",
                        "source": "npm",
                        "target": "bitfun-demo-echo",
                        "entry": "C:/cache/bitfun-demo-echo/index.js",
                        "cache": "hit",
                        "contentHash": "2".repeat(64)
                    }],
                    "failed": [],
                    "diagnostics": []
                }
            }),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("plugin prepare response should be written");
    });

    let result = client
        .prepare_plugins(
            PluginPrepareRequest {
                plugins: vec![PluginDeclaration {
                    spec: "bitfun-demo-echo".to_string(),
                    options: None,
                    base_directory: None,
                }],
                configuration_fingerprint: Some("fixture-prewarm".to_string()),
                default_base_directory: None,
                allow_install: Some(false),
            },
            Duration::from_secs(1),
        )
        .await
        .expect("plugin prepare should resolve");

    assert_eq!(result.review_digest, "0".repeat(64));
    assert_eq!(result.reviewed_count, 1);
    assert_eq!(result.failed_count, 0);
    assert_eq!(
        result.content_digests.get("npm:bitfun-demo-echo"),
        Some(&"2".repeat(64))
    );
    host.await.expect("fake host should finish");
}

#[tokio::test]
async fn peer_fails_pending_requests_when_host_disconnects() {
    let (backend_stream, mut host_stream) = connected_streams().await;
    let peer = JsonRpcPeer::start(backend_stream, 9, DEFAULT_MAX_FRAME_BYTES);
    let client = peer.client();
    let host = tokio::spawn(async move {
        read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("request should be readable");
    });

    let error = client
        .request("host.never", json!({}), Duration::from_secs(1))
        .await
        .expect_err("disconnect should fail the pending request");

    assert!(matches!(error, PluginHostError::ConnectionClosed(_)));
    host.await.expect("fake host should finish");
}

#[tokio::test]
async fn read_host_stream_cancels_after_an_invalid_response() {
    let (backend_stream, mut host_stream) = connected_streams().await;
    let peer = JsonRpcPeer::start(backend_stream, 15, DEFAULT_MAX_FRAME_BYTES);
    let client = peer.client();
    let host = tokio::spawn(async move {
        let read_request = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("stream read request should be readable");
        assert_eq!(read_request["method"], "host.stream.read");
        write_frame(
            &mut host_stream,
            &json!({
                "jsonrpc": "2.0",
                "id": read_request["id"],
                "result": {"data": "not-base64!", "eof": true}
            }),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("invalid stream response should be written");
        let cancel_request = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("stream cancel request should be readable");
        assert_eq!(cancel_request["method"], "host.stream.cancel");
        assert_eq!(cancel_request["params"]["instanceID"], "instance");
        assert_eq!(cancel_request["params"]["streamID"], "host-stream:1");
        write_frame(
            &mut host_stream,
            &json!({
                "jsonrpc": "2.0",
                "id": cancel_request["id"],
                "result": {"cancelled": true}
            }),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("stream cancel response should be written");
    });

    let error = read_host_stream(
        &client,
        "instance",
        &StreamDescriptor {
            stream_id: "host-stream:1".to_string(),
            length: None,
        },
        1024,
        Duration::from_secs(1),
    )
    .await
    .expect_err("invalid base64 should fail the stream read");
    assert!(matches!(error, HostStreamReadError::InvalidBase64(_)));
    host.await.expect("fake host should finish");
}

#[tokio::test]
async fn peer_rejects_response_with_result_and_error() {
    let (backend_stream, mut host_stream) = connected_streams().await;
    let peer = JsonRpcPeer::start(backend_stream, 10, DEFAULT_MAX_FRAME_BYTES);
    let client = peer.client();
    let host = tokio::spawn(async move {
        let request = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("request should be readable");
        write_frame(
            &mut host_stream,
            &json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "result": {"invalid": true},
                "error": {"code": -32603, "message": "invalid envelope"}
            }),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("malformed response should be written");
    });

    let error = client
        .request("host.invalid", json!({}), Duration::from_secs(1))
        .await
        .expect_err("malformed response should fail the request");

    assert!(matches!(error, PluginHostError::Protocol(_)));
    host.await.expect("fake host should finish");
}

#[tokio::test]
async fn draining_waits_for_admitted_request_and_rejects_new_requests() {
    let (backend_stream, mut host_stream) = connected_streams().await;
    let peer = JsonRpcPeer::start(backend_stream, 11, DEFAULT_MAX_FRAME_BYTES);
    let client = peer.client();
    let host = tokio::spawn(async move {
        let active = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("active request should be readable");
        assert_eq!(active["method"], "host.active");
        tokio::time::sleep(Duration::from_millis(50)).await;
        write_frame(
            &mut host_stream,
            &json!({"jsonrpc": "2.0", "id": active["id"], "result": {"done": true}}),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("active response should be written");

        let shutdown = read_frame(&mut host_stream, DEFAULT_MAX_FRAME_BYTES)
            .await
            .expect("shutdown request should be readable");
        assert_eq!(shutdown["method"], "host.shutdown");
        write_frame(
            &mut host_stream,
            &json!({"jsonrpc": "2.0", "id": shutdown["id"], "result": {"closed": true}}),
            DEFAULT_MAX_FRAME_BYTES,
        )
        .await
        .expect("shutdown response should be written");
    });

    let active_client = client.clone();
    let active = tokio::spawn(async move {
        active_client
            .request("host.active", json!({}), Duration::from_secs(1))
            .await
    });
    tokio::task::yield_now().await;

    let admitted = client.begin_draining().await;
    assert_eq!(admitted, 1);
    let error = client
        .request("host.rejected", json!({}), Duration::from_secs(1))
        .await
        .expect_err("new request should be rejected while draining");
    assert!(matches!(error, PluginHostError::ShuttingDown));
    assert!(client.wait_for_pending(Duration::from_secs(1)).await);
    assert_eq!(
        active
            .await
            .expect("active request task should finish")
            .expect("active request should complete"),
        json!({"done": true})
    );
    assert_eq!(
        client
            .request_during_shutdown("host.shutdown", json!({}), Duration::from_secs(1))
            .await
            .expect("shutdown request should complete"),
        json!({"closed": true})
    );
    host.await.expect("fake host should finish");
}

#[tokio::test]
async fn draining_rejects_new_notifications() {
    let (backend_stream, _host_stream) = connected_streams().await;
    let peer = JsonRpcPeer::start(backend_stream, 12, DEFAULT_MAX_FRAME_BYTES);
    let client = peer.client();
    client.begin_draining().await;

    let error = client
        .notify("host.rejected", json!({}))
        .await
        .expect_err("new notification should be rejected while draining");

    assert!(matches!(error, PluginHostError::ShuttingDown));
}

async fn connected_streams() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener should have address");
    let host = tokio::spawn(async move {
        TcpStream::connect(address)
            .await
            .expect("fake host should connect")
    });
    let (backend_stream, _) = listener
        .accept()
        .await
        .expect("backend should accept fake host");
    let host_stream = host.await.expect("fake host connection should finish");
    (backend_stream, host_stream)
}
