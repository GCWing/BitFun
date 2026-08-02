use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bitfun_services_integrations::mcp::server::MCPConnection;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex, Notify};

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

struct TestRequest {
    method: String,
    target: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

async fn read_request(stream: &mut TcpStream) -> std::io::Result<Option<TestRequest>> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_offset = loop {
        if let Some(offset) = header_end(&buffer) {
            break offset;
        }

        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(None);
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > 1024 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "test request headers exceeded one MiB",
            ));
        }
    };

    let headers_text = String::from_utf8_lossy(&buffer[..header_offset]);
    let mut lines = headers_text.split("\r\n");
    let mut request_line = lines.next().unwrap_or_default().split_whitespace();
    let method = request_line.next().unwrap_or_default().to_string();
    let target = request_line.next().unwrap_or_default().to_string();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect::<HashMap<_, _>>();
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let body_offset = header_offset + 4;
    while buffer.len() < body_offset + content_length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "test request body ended early",
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let body = buffer[body_offset..body_offset + content_length].to_vec();

    Ok(Some(TestRequest {
        method,
        target,
        headers,
        body,
    }))
}

async fn write_response(
    stream: &mut TcpStream,
    status: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> std::io::Result<()> {
    let mut response = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        response.push_str(name);
        response.push_str(": ");
        response.push_str(value);
        response.push_str("\r\n");
    }
    response.push_str("\r\n");
    response.push_str(body);
    stream.write_all(response.as_bytes()).await
}

async fn serve_sse(
    mut stream: TcpStream,
    state: TestState,
    session_id: String,
) -> std::io::Result<()> {
    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nTransfer-Encoding: chunked\r\nConnection: keep-alive\r\n\r\n",
        )
        .await?;

    let (tx, rx) = mpsc::unbounded_channel::<String>();
    {
        let mut guard = state.sse_clients_by_session.lock().await;
        guard.entry(session_id).or_default().push(tx);
    }

    if !state.sse_connected.swap(true, Ordering::SeqCst) {
        state.sse_connected_notify.notify_waiters();
    }

    let mut rx = rx;
    while let Some(payload) = rx.recv().await {
        let event = format!("data: {payload}\n\n");
        stream
            .write_all(format!("{:X}\r\n", event.len()).as_bytes())
            .await?;
        stream.write_all(event.as_bytes()).await?;
        stream.write_all(b"\r\n").await?;
        stream.flush().await?;
    }
    stream.write_all(b"0\r\n\r\n").await?;
    Ok(())
}

async fn handle_post(
    stream: &mut TcpStream,
    state: &TestState,
    headers: &HashMap<String, String>,
    body: &Value,
) -> std::io::Result<()> {
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

            write_response(
                stream,
                "200 OK",
                &[
                    ("Content-Type", "application/json"),
                    ("Mcp-Session-Id", "test-session"),
                ],
                &response.to_string(),
            )
            .await
        }
        // BigModel-style quirk: return 200 with an empty body (and no Content-Type),
        // which should be treated as Accepted by the client.
        "notifications/initialized" => write_response(stream, "200 OK", &[], "").await,
        "tools/list" => {
            let sid = headers
                .get("mcp-session-id")
                .map(String::as_str)
                .unwrap_or_default();
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

            write_response(stream, "202 Accepted", &[], "").await?;
            let mut guard = state.sse_clients_by_session.lock().await;
            if let Some(list) = guard.get_mut("test-session") {
                list.retain(|tx| tx.send(payload.clone()).is_ok());
            }
            Ok(())
        }
        _ => {
            let response = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            });
            write_response(
                stream,
                "200 OK",
                &[("Content-Type", "application/json")],
                &response.to_string(),
            )
            .await
        }
    }
}

async fn handle_connection(mut stream: TcpStream, state: TestState) -> std::io::Result<()> {
    let Some(request) = read_request(&mut stream).await? else {
        return Ok(());
    };

    if request.target != "/mcp" {
        return write_response(&mut stream, "404 Not Found", &[], "").await;
    }

    match request.method.as_str() {
        "GET" => {
            let session_id = request
                .headers
                .get("mcp-session-id")
                .cloned()
                .unwrap_or_default();
            serve_sse(stream, state, session_id).await
        }
        "POST" => {
            let is_json = request
                .headers
                .get("content-type")
                .and_then(|value| value.split(';').next())
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"));
            if !is_json {
                return write_response(&mut stream, "415 Unsupported Media Type", &[], "").await;
            }
            let body = serde_json::from_slice(&request.body)?;
            handle_post(&mut stream, &state, &request.headers, &body).await
        }
        _ => write_response(&mut stream, "405 Method Not Allowed", &[], "").await,
    }
}

async fn raw_status_line(addr: std::net::SocketAddr, request: &str) -> String {
    let mut stream = TcpStream::connect(addr)
        .await
        .expect("test fixture should accept a raw request");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("raw request should be written");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .expect("test fixture should close its rejection response");
    String::from_utf8_lossy(&response)
        .lines()
        .next()
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn remote_mcp_streamable_http_accepts_202_and_delivers_response_via_sse() {
    let state = TestState::default();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_state = state.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let connection_state = server_state.clone();
            tokio::spawn(async move {
                handle_connection(stream, connection_state)
                    .await
                    .expect("test MCP connection should complete");
            });
        }
    });

    assert_eq!(
        raw_status_line(
            addr,
            "POST /wrong HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
        )
        .await,
        "HTTP/1.1 404 Not Found",
    );
    assert_eq!(
        raw_status_line(
            addr,
            "POST /mcp HTTP/1.1\r\nHost: localhost\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\n{}",
        )
        .await,
        "HTTP/1.1 415 Unsupported Media Type",
    );
    assert_eq!(
        raw_status_line(addr, "PUT /mcp HTTP/1.1\r\nHost: localhost\r\n\r\n").await,
        "HTTP/1.1 405 Method Not Allowed",
    );

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
