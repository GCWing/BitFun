#![recursion_limit = "512"]

//! Phase 2 integration tests: the generic app-server role exposes real
//! `bitfun_agent_runtime` SDK operations over the in-memory channel transport.
//!
//! The mock provider only implements `AgentSubmissionPort` (the port behind
//! `run`, `create_session`, and `submit_turn`), matching `sdk_minimal.rs`. The
//! other operations (`list_sessions`, `delete_session`, `cancel_turn`) need
//! separate ports; without them injected the runtime returns a missing-port
//! error, which these tests assert maps to an error at the JSON-RPC boundary.
//!
//! Each `on_receive_request` on `BitfunAppServer::serve` chains a
//! `ChainedHandler` layer; the full agent-kernel + permission + git + config
//! surface now monomorphizes into a handler tower deeper than the default
//! recursion limit when this test instantiates the connection. The lifted
//! limit keeps the chain compiling as more host-service groups land.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_client_protocol::{ConnectionTo, SentRequest};
use async_trait::async_trait;
use bitfun_agent_runtime::event_queue::{EventQueue, EventQueueConfig};
use bitfun_agent_runtime::sdk::{
    AgentEventSource, AgentEventStream, AgentRuntimeBuilder, AgentSessionCreateRequest,
    AgentSessionCreateResult, AgentSessionDeleteRequest, AgentSessionListRequest,
    AgentSubmissionPort, AgentSubmissionRequest, AgentSubmissionResult, AgentSubmissionSource,
    AgentTurnCancellationRequest, AgenticEvent, PortResult,
};
use bitfun_app_server::schema::{
    CancelTurnMessage, CreateSessionMessage, CreateSessionResponse, DeleteSessionMessage,
    FrontendEventNotification, ListSessionsMessage, RespondPermissionMessage, RunMessage,
    RunResponse, RunSessionSpec, SubmitDialogTurnBody, SubmitDialogTurnMessage,
    SubmitDialogTurnResponse, SubmitTurnMessage, SubmitTurnResponse,
};
use bitfun_app_server::{transport, AppClient, AppServer, BitfunAppRuntime, BitfunAppServer};
use tokio::task::LocalSet;

/// Minimal `AgentSubmissionPort` mock modeled on `sdk_minimal.rs`.
#[derive(Debug, Default)]
struct ExampleAgentProvider {
    created_sessions: Mutex<Vec<AgentSessionCreateRequest>>,
    submitted_turns: Mutex<Vec<AgentSubmissionRequest>>,
    submitted_dialog_turns: Mutex<Vec<bitfun_agent_runtime::sdk::AgentDialogTurnRequest>>,
}

#[async_trait]
impl AgentSubmissionPort for ExampleAgentProvider {
    async fn create_session(
        &self,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult> {
        self.created_sessions.lock().unwrap().push(request.clone());
        Ok(AgentSessionCreateResult::new(
            "example-session",
            request.session_name,
            request.agent_type,
        ))
    }

    async fn submit_message(
        &self,
        request: AgentSubmissionRequest,
    ) -> PortResult<AgentSubmissionResult> {
        self.submitted_turns.lock().unwrap().push(request.clone());
        Ok(AgentSubmissionResult {
            turn_id: request
                .turn_id
                .unwrap_or_else(|| "example-turn".to_string()),
            accepted: true,
        })
    }

    async fn resolve_session_agent_type(&self, _session_id: &str) -> PortResult<Option<String>> {
        Ok(Some("agentic".to_string()))
    }
}

#[async_trait::async_trait]
impl bitfun_agent_runtime::sdk::AgentDialogTurnPort for ExampleAgentProvider {
    async fn submit_dialog_turn(
        &self,
        request: bitfun_agent_runtime::sdk::AgentDialogTurnRequest,
    ) -> PortResult<bitfun_agent_runtime::sdk::DialogSubmitOutcome> {
        self.submitted_dialog_turns
            .lock()
            .unwrap()
            .push(request.clone());
        Ok(bitfun_agent_runtime::sdk::DialogSubmitOutcome::Started {
            session_id: request.session_id,
            turn_id: request
                .turn_id
                .unwrap_or_else(|| "example-dialog-turn".to_string()),
        })
    }
}

fn build_runtime() -> bitfun_agent_runtime::sdk::AgentRuntime {
    let provider = Arc::new(ExampleAgentProvider::default());
    let events = AgentEventStream::new();
    AgentRuntimeBuilder::new()
        .with_submission_port(provider.clone())
        .with_dialog_turn_port(provider)
        .with_event_stream(events)
        .build()
        .expect("runtime should build with submission + dialog turn ports")
}

/// Wrap the test runtime with a fresh `AgentEventSource` backed by an isolated
/// `EventQueue`, so the app-server's event forwarder has something to drain.
fn build_app_runtime() -> BitfunAppRuntime {
    let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let event_source = AgentEventSource::new(event_queue);
    BitfunAppRuntime::new(build_runtime(), event_source)
}

/// Like [`build_app_runtime`] but also hands back the backing `EventQueue` so a
/// test can publish into it and assert the server forwards the event to the
/// client as an `agent/event` notification.
fn build_app_runtime_with_queue() -> (BitfunAppRuntime, Arc<EventQueue>) {
    let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
    let event_source = AgentEventSource::new(event_queue.clone());
    (
        BitfunAppRuntime::new(build_runtime(), event_source),
        event_queue,
    )
}

async fn recv<T>(response: SentRequest<T>) -> Result<T, agent_client_protocol::Error>
where
    T: agent_client_protocol::JsonRpcResponse + Send,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    response.on_receiving_result(async move |result| {
        tx.send(result)
            .map_err(|_| agent_client_protocol::Error::internal_error())
    })?;
    rx.await
        .map_err(|_| agent_client_protocol::Error::internal_error())?
}

fn spawn_server(
    runtime: BitfunAppRuntime,
    transport: impl agent_client_protocol::ConnectTo<AppServer> + 'static,
) {
    tokio::task::spawn_local(async move {
        let _ = BitfunAppServer::new(runtime).serve(transport).await;
    });
}

#[tokio::test(flavor = "current_thread")]
async fn run_round_trips_through_create_and_submit() {
    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();
            let runtime = build_app_runtime();
            spawn_server(runtime, server_transport);

            let result = AppClient
                .builder()
                .connect_with(client_transport, async |cx: ConnectionTo<AppServer>| {
                    let response = recv(cx.send_request(RunMessage {
                        session: RunSessionSpec::Create {
                            session_name: "Example SDK Session".to_string(),
                            agent_type: "agentic".to_string(),
                            workspace_path: None,
                        },
                        message: "hello from an app-server client".to_string(),
                        turn_id: None,
                        source: Some(AgentSubmissionSource::Cli),
                    }))
                    .await?;
                    assert_eq!(response.session_id, "example-session");
                    assert_eq!(response.turn_id, "example-turn");
                    assert_eq!(response.agent_type.as_deref(), Some("agentic"));
                    assert!(response.accepted);
                    Ok(())
                })
                .await;
            assert!(result.is_ok(), "{result:?}");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn submit_dialog_turn_carries_agent_type_and_starts() {
    // `start_dialog_turn`-style calls must route to `agent/submitDialogTurn`
    // (not `agent/submitTurn`): the dialog-turn body carries `agentType` and a
    // `policy`, which the bare submission request does not. This test pins the
    // field path: the mock records the dialog request, the server defaults the
    // omitted `policy` to the desktop UI source, and the response is `Started`.
    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();
            let runtime = build_app_runtime();
            spawn_server(runtime, server_transport);

            let result = AppClient
                .builder()
                .connect_with(client_transport, async |cx: ConnectionTo<AppServer>| {
                    let response = recv(cx.send_request(SubmitDialogTurnMessage(
                        SubmitDialogTurnBody {
                            session_id: "example-session".to_string(),
                            message: "hello dialog".to_string(),
                            original_message: None,
                            turn_id: None,
                            execution: Default::default(),
                            agent_type: "agentic".to_string(),
                            workspace_path: None,
                            remote_connection_id: None,
                            remote_ssh_host: None,
                            policy: None,
                            attachments: Vec::new(),
                            metadata: serde_json::Map::new(),
                        },
                    )))
                    .await?;
                    let SubmitDialogTurnResponse::Started {
                        session_id,
                        turn_id,
                    } = response
                    else {
                        panic!("expected Started, got {response:?}");
                    };
                    assert_eq!(session_id, "example-session");
                    assert_eq!(turn_id, "example-dialog-turn");
                    Ok(())
                })
                .await;
            assert!(result.is_ok(), "{result:?}");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn respond_permission_routes_to_the_permission_surface() {
    // The permission commands must route through the app-server `agent/*`
    // surface. The mock runtime here ships without a permission request
    // manager (matching `cancel_turn`'s missing-port test), so the SDK
    // returns `MissingPermissionRequestManager`; what this pins is that the
    // request reaches the handler and the runtime error surfaces cleanly as a
    // JSON-RPC error -- not an "unknown method" fallthrough.
    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();
            let runtime = build_app_runtime();
            spawn_server(runtime, server_transport);

            let result = AppClient
                .builder()
                .connect_with(client_transport, async |cx: ConnectionTo<AppServer>| {
                    let result = recv(cx.send_request(RespondPermissionMessage {
                        request_id: "perm-1".to_string(),
                        reply: bitfun_agent_runtime::sdk::PermissionReply::Once,
                    }))
                    .await;
                    assert!(
                        result.is_err(),
                        "respondPermission without a permission manager should error, got {result:?}"
                    );
                    Ok(())
                })
                .await;
            assert!(result.is_ok(), "{result:?}");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn create_session_returns_provider_session_id() {
    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();
            let runtime = build_app_runtime();
            spawn_server(runtime, server_transport);

            let result = AppClient
                .builder()
                .connect_with(client_transport, async |cx: ConnectionTo<AppServer>| {
                    let response = recv(cx.send_request(CreateSessionMessage(
                        AgentSessionCreateRequest {
                            session_name: "direct create".to_string(),
                            agent_type: "agentic".to_string(),
                            workspace_path: None,
                            project_workspace_path: None,
                            execution_target: None,
                            workspace_id: None,
                            remote_connection_id: None,
                            remote_ssh_host: None,
                            model_id: None,
                            metadata: Default::default(),
                        },
                    )))
                    .await?;
                    let CreateSessionResponse(inner) = response;
                    assert_eq!(inner.session_id, "example-session");
                    assert_eq!(inner.agent_type, "agentic");
                    Ok(())
                })
                .await;
            assert!(result.is_ok(), "{result:?}");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn submit_turn_surfaces_provider_result() {
    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();
            let runtime = build_app_runtime();
            spawn_server(runtime, server_transport);

            let result = AppClient
                .builder()
                .connect_with(client_transport, async |cx: ConnectionTo<AppServer>| {
                    let response =
                        recv(cx.send_request(SubmitTurnMessage(AgentSubmissionRequest {
                            session_id: "example-session".to_string(),
                            message: "follow-up message".to_string(),
                            turn_id: None,
                            source: Some(AgentSubmissionSource::Cli),
                            attachments: Vec::new(),
                            metadata: Default::default(),
                        })))
                        .await?;
                    let SubmitTurnResponse(inner) = response;
                    assert_eq!(inner.turn_id, "example-turn");
                    assert!(inner.accepted);
                    Ok(())
                })
                .await;
            assert!(result.is_ok(), "{result:?}");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn list_sessions_maps_missing_port_to_internal_error() {
    // `AgentSessionManagementPort` is not injected, so the runtime returns
    // `MissingSessionManagementPort`. The server must surface that as a
    // JSON-RPC error, not crash.
    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();
            let runtime = build_app_runtime();
            spawn_server(runtime, server_transport);

            let result = AppClient
                .builder()
                .connect_with(client_transport, async |cx: ConnectionTo<AppServer>| {
                    let result = recv(cx.send_request(ListSessionsMessage(
                        AgentSessionListRequest {
                            workspace_path: ".".to_string(),
                            remote_connection_id: None,
                            remote_ssh_host: None,
                        },
                    )))
                    .await;
                    assert!(
                        result.is_err(),
                        "listSessions without session-management port should error, got {result:?}"
                    );
                    Ok(())
                })
                .await;
            assert!(result.is_ok(), "{result:?}");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn delete_session_maps_missing_port_to_internal_error() {
    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();
            let runtime = build_app_runtime();
            spawn_server(runtime, server_transport);

            let result = AppClient
                .builder()
                .connect_with(client_transport, async |cx: ConnectionTo<AppServer>| {
                    let result = recv(cx.send_request(DeleteSessionMessage(
                        AgentSessionDeleteRequest {
                            workspace_path: ".".to_string(),
                            session_id: "example-session".to_string(),
                            remote_connection_id: None,
                            remote_ssh_host: None,
                        },
                    )))
                    .await;
                    assert!(
                        result.is_err(),
                        "deleteSession without session-management port should error, got {result:?}"
                    );
                    Ok(())
                })
                .await;
            assert!(result.is_ok(), "{result:?}");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn cancel_turn_maps_missing_port_to_internal_error() {
    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();
            let runtime = build_app_runtime();
            spawn_server(runtime, server_transport);

            let result = AppClient
                .builder()
                .connect_with(client_transport, async |cx: ConnectionTo<AppServer>| {
                    let result = recv(cx.send_request(CancelTurnMessage(
                        AgentTurnCancellationRequest {
                            session_id: "example-session".to_string(),
                            turn_id: None,
                            source: Some(AgentSubmissionSource::Cli),
                            requester_session_id: None,
                            reason: None,
                            wait_timeout_ms: None,
                            cancel_descendants: true,
                        },
                    )))
                    .await;
                    assert!(
                        result.is_err(),
                        "cancelTurn without cancellation port should error, got {result:?}"
                    );
                    Ok(())
                })
                .await;
            assert!(result.is_ok(), "{result:?}");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_agent_method_returns_method_not_found() {
    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();
            let runtime = build_app_runtime();
            spawn_server(runtime, server_transport);

            let result = AppClient
                .builder()
                .connect_with(client_transport, async |cx: ConnectionTo<AppServer>| {
                    let response = recv(cx.send_request(UnknownAgentRequest)).await;
                    assert!(
                        response.is_err(),
                        "unknown method should yield method_not_found, got {response:?}"
                    );
                    Ok(())
                })
                .await;
            assert!(result.is_ok(), "{result:?}");
        })
        .await;
}

/// The app-server must forward runtime events from its injected
/// `AgentEventSource` to the client as `agent/event` notifications, not leave
/// the client to subscribe to the runtime queue directly.
#[tokio::test(flavor = "current_thread")]
async fn runtime_events_are_forwarded_as_agent_event_notifications() {
    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();
            let (runtime, event_queue) = build_app_runtime_with_queue();
            spawn_server(runtime, server_transport);

            let received: Arc<Mutex<Vec<FrontendEventNotification>>> =
                Arc::new(Mutex::new(Vec::new()));
            let received_for_client = received.clone();
            let queue_for_client = event_queue.clone();

            let result = AppClient
                .builder()
                .on_receive_notification(
                    {
                        let received = received_for_client.clone();
                        async move |notification: FrontendEventNotification,
                                    _cx: ConnectionTo<AppServer>| {
                            received.lock().unwrap().push(notification);
                            Ok(())
                        }
                    },
                    agent_client_protocol::on_receive_notification!(),
                )
                .connect_with(client_transport, async |_cx: ConnectionTo<AppServer>| {
                    // Let the server's forwarder subscribe before publishing.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    queue_for_client
                        .enqueue(
                            AgenticEvent::SessionStateChanged {
                                session_id: "s1".to_string(),
                                new_state: "ready".to_string(),
                            },
                            None,
                        )
                        .await
                        .expect("event should enqueue");
                    // Allow time for the server forwarder + client dispatch.
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    Ok(())
                })
                .await;
            assert!(result.is_ok(), "{result:?}");

            let received = received.lock().unwrap().clone();
            assert_eq!(
                received.len(),
                1,
                "should receive exactly one projected frontend event, got {received:?}"
            );
            // Step 2: the server now projects the runtime event to the frontend
            // shape (`agentic://<type>`) before pushing, so the client sees a
            // `FrontendEventNotification` rather than the raw envelope.
            assert_eq!(received[0].event, "agentic://session-state-changed");
            assert_eq!(
                received[0].payload["sessionId"].as_str(),
                Some("s1"),
                "projected payload should carry the session id: {:?}",
                received[0].payload
            );
        })
        .await;
}

#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, agent_client_protocol::JsonRpcRequest,
)]
#[request(method = "agent/__unknown_for_test", response = UnknownAgentResponse)]
struct UnknownAgentRequest;

#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, agent_client_protocol::JsonRpcResponse,
)]
struct UnknownAgentResponse;

#[allow(dead_code)]
fn _document_run_response_shape_in_tests(_r: RunResponse) {}

/// The Server Host drives the app-server through the real `client::connect`
/// handle (not an inline `AppClient::builder().connect_with` main_fn like the
/// round-trip tests above). `connect` parks its main loop on a shutdown
/// receiver and returns an `AppServerClient` the host holds for the process
/// lifetime. This regression test pins that contract: after `connect`
/// returns, an RPC sent through the returned handle must still reach the
/// server and get a response. A previous version dropped `shutdown_tx` right
/// before returning (`let _ = shutdown_tx;`), which let the parked main loop
/// resume immediately, cancelling the connection's background actors -- every
/// subsequent RPC then surfaced as `send failed because receiver is gone`
/// (from `Task::spawn`'s `unbounded_send` failure). This test fails loud if
/// that regression returns, because `create_session` would error instead of
/// returning the mock session id.
#[tokio::test(flavor = "current_thread")]
async fn client_connect_keeps_connection_alive_after_return() {
    let local = LocalSet::new();
    local
        .run_until(async {
            let (server_transport, client_transport) = transport::in_memory_channel_pair();
            let runtime = build_app_runtime();
            spawn_server(runtime, server_transport);

            let client = bitfun_app_server::connect(client_transport)
                .await
                .expect("app-server client should connect");

            // The connect task must still be parked on the shutdown receiver
            // here -- otherwise the connection's background actors are gone and
            // this RPC surfaces `send failed because receiver is gone`.
            let response = client
                .create_session(AgentSessionCreateRequest {
                    session_name: "post-connect session".to_string(),
                    agent_type: "agentic".to_string(),
                    workspace_path: None,
                    project_workspace_path: None,
                    execution_target: None,
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                    model_id: None,
                    metadata: Default::default(),
                })
                .await
                .expect("RPC after connect() must succeed -- connection should still be alive");
            assert_eq!(response.session_id, "example-session");
            assert_eq!(response.agent_type, "agentic");

            client.shutdown().await;
        })
        .await;
}
