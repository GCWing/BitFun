use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Json;
use axum::Router;
use bitfun_core::service::mcp::server::MCPConnection;
use futures::Stream;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt;

#[derive(Clone, Default)]
struct TestState {
    sse_clients_by_session: Arc<Mutex<HashMap<String, Vec<mpsc::UnboundedSender<String>>>>>,
    sse_connected: Arc<AtomicBool>,
    sse_connected_notify: Arc<Notify>,
    saw_session_header: Arc<AtomicBool>,
    saw_roots_capability: Arc<AtomicBool>,
    saw_sampling_capability: Arc<AtomicBool>,
    saw_elicitation_capability: Arc<AtomicBool>,
}

async fn sse_handler(
    State(state): State<TestState>,
    headers: HeaderMap,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let session_id = headers
        .get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let (tx, rx) = mpsc::unbounded_channel::<String>();
    {
        let mut guard = state.sse_clients_by_session.lock().await;
        guard.entry(session_id).or_default().push(tx);
    }

    if !state.sse_connected.swap(true, Ordering::SeqCst) {
        state.sse_connected_notify.notify_waiters();
    }

    let stream = UnboundedReceiverStream::new(rx).map(|data| Ok(Event::default().data(data)));
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ka"),
    )
}

async fn post_handler(
    State(state): State<TestState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let method = body.get("method").and_then(Value::as_str).unwrap_or("");
    let id = body.get("id").cloned().unwrap_or(Value::Null);

    match method {
        "initialize" => {
            let capabilities = body
                .get("params")
                .and_then(|params| params.get("capabilities"))
                .cloned()
                .unwrap_or(Value::Null);
            if capabilities.get("roots").is_some() {
                state.saw_roots_capability.store(true, Ordering::SeqCst);
            }
            if capabilities.get("sampling").is_some() {
                state.saw_sampling_capability.store(true, Ordering::SeqCst);
            }
            if capabilities.get("elicitation").is_some() {
                state
                    .saw_elicitation_capability
                    .store(true, Ordering::SeqCst);
            }

            let response = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {
                        "tools": { "listChanged": false }
                    },
                    "serverInfo": { "name": "test-mcp", "version": "1.0.0" }
                }
            });

            let mut response_headers = HeaderMap::new();
            response_headers.insert(
                "Mcp-Session-Id",
                "test-session".parse().expect("valid header value"),
            );
            (StatusCode::OK, response_headers, Json(response)).into_response()
        }
        // BigModel-style quirk: return 200 with an empty body (and no Content-Type),
        // which should be treated as Accepted by the client.
        "notifications/initialized" => StatusCode::OK.into_response(),
        "tools/list" => {
            let sid = headers
                .get("Mcp-Session-Id")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if sid == "test-session" {
                state.saw_session_header.store(true, Ordering::SeqCst);
            }

            let payload = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [
                        {
                            "name": "hello",
                            "title": "Hello Tool",
                            "description": "test tool",
                            "inputSchema": { "type": "object", "properties": {} },
                            "outputSchema": { "type": "object", "properties": { "message": { "type": "string" } } },
                            "annotations": {
                                "title": "Hello",
                                "readOnlyHint": true,
                                "destructiveHint": false,
                                "openWorldHint": true
                            },
                            "icons": [
                                {
                                    "src": "https://example.com/tool.png",
                                    "mimeType": "image/png",
                                    "sizes": ["32x32"]
                                }
                            ],
                            "_meta": {
                                "ui": {
                                    "resourceUri": "ui://hello/widget"
                                }
                            }
                        }
                    ],
                    "nextCursor": null
                }
            })
            .to_string();

            let clients = state.sse_clients_by_session.clone();
            tokio::spawn(async move {
                let mut guard = clients.lock().await;
                let Some(list) = guard.get_mut("test-session") else {
                    return;
                };
                list.retain(|tx| tx.send(payload.clone()).is_ok());
            });

            StatusCode::ACCEPTED.into_response()
        }
        _ => {
            let response = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            });
            (StatusCode::OK, Json(response)).into_response()
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_mcp_streamable_http_accepts_202_and_delivers_response_via_sse() {
    let state = TestState::default();
    let app = Router::new()
        .route("/mcp", get(sse_handler).post(post_handler))
        .with_state(state.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let url = format!("http://{addr}/mcp");
    let connection = MCPConnection::new_remote("test-server", url, Default::default(), false)
        .await
        .expect("remote connection should be created");

    connection
        .initialize("BitFunTest", "0.0.0")
        .await
        .expect("initialize should succeed");

    // `Notify::notify_waiters` only wakes tasks already waiting. The rmcp client may open the
    // SSE GET during `initialize` and fire notify before we await `notified()`, which would
    // drop the wakeup and time out. The atomic records that the handler ran at least once.
    if !state.sse_connected.load(Ordering::SeqCst) {
        tokio::time::timeout(
            Duration::from_secs(2),
            state.sse_connected_notify.notified(),
        )
        .await
        .expect("SSE stream should connect");
    }

    let tools = connection
        .list_tools(None)
        .await
        .expect("tools/list should resolve via SSE");
    assert_eq!(tools.tools.len(), 1);
    assert_eq!(tools.tools[0].name, "hello");
    assert_eq!(tools.tools[0].title.as_deref(), Some("Hello Tool"));
    assert_eq!(
        tools.tools[0]
            .annotations
            .as_ref()
            .and_then(|annotations| annotations.read_only_hint),
        Some(true)
    );
    assert_eq!(
        tools.tools[0]
            .meta
            .as_ref()
            .and_then(|meta| meta.ui.as_ref())
            .and_then(|ui| ui.resource_uri.as_deref()),
        Some("ui://hello/widget")
    );

    assert!(
        state.saw_session_header.load(Ordering::SeqCst),
        "client should forward session id header on subsequent requests"
    );
    assert!(
        state.saw_roots_capability.load(Ordering::SeqCst),
        "client should advertise roots capability"
    );
    assert!(
        state.saw_sampling_capability.load(Ordering::SeqCst),
        "client should advertise sampling capability"
    );
    assert!(
        state.saw_elicitation_capability.load(Ordering::SeqCst),
        "client should advertise elicitation capability"
    );
}
