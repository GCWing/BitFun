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
use futures_util::Stream;
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
                            "description": "test tool",
                            "inputSchema": { "type": "object", "properties": {} }
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
    let connection = MCPConnection::new_remote(url, Default::default());

    connection
        .initialize("BitFunTest", "0.0.0")
        .await
        .expect("initialize should succeed");

    tokio::time::timeout(
        Duration::from_secs(2),
        state.sse_connected_notify.notified(),
    )
    .await
    .expect("SSE stream should connect");

    let tools = connection
        .list_tools(None)
        .await
        .expect("tools/list should resolve via SSE");
    assert_eq!(tools.tools.len(), 1);
    assert_eq!(tools.tools[0].name, "hello");

    assert!(
        state.saw_session_header.load(Ordering::SeqCst),
        "client should forward session id header on subsequent requests"
    );
}
