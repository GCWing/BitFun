use bitfun_services_integrations::debug_log::{append_log_async, DebugLogConfig, DebugLogEntry};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn debug_log_owner_appends_legacy_partially_redacted_ndjson_and_skips_http_when_disabled() {
    let temp = tempfile::tempdir().expect("tempdir");
    let log_path = temp.path().join("debug.log");

    append_log_async(
        DebugLogEntry {
            location: "test/location".to_string(),
            message: "hello".to_string(),
            data: json!({
                "token": "0123456789abcdef",
                "nested": { "api_key": "secret-key" },
                "safe": "value"
            }),
            session_id: String::new(),
            run_id: Some("run-1".to_string()),
            hypothesis_id: None,
            timestamp: Some(123),
            id: Some("log-fixed".to_string()),
        },
        Some(DebugLogConfig {
            log_path: log_path.clone(),
            ingest_url: Some("http://127.0.0.1:1/unused".to_string()),
            session_id: "session-default".to_string(),
        }),
        false,
    )
    .await
    .expect("append");

    let line = std::fs::read_to_string(log_path).expect("log file");
    let value: serde_json::Value = serde_json::from_str(line.trim()).expect("json line");

    assert_eq!(value["id"], "log-fixed");
    assert_eq!(value["timestamp"], 123);
    assert_eq!(value["sessionId"], "session-default");
    assert_eq!(value["data"]["safe"], "value");
    assert_eq!(value["data"]["token"], "0123456789***");
    assert_eq!(value["data"]["nested"]["api_key"], "secret-key***");
}

#[tokio::test]
async fn debug_log_owner_dispatches_the_same_redacted_payload_when_http_is_enabled() {
    let temp = tempfile::tempdir().expect("tempdir");
    let log_path = temp.path().join("debug.log");
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let url = format!("http://{}", listener.local_addr().expect("addr"));

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let mut request = Vec::new();
        let mut buf = [0u8; 1024];
        let header_end;

        loop {
            let read = socket.read(&mut buf).await.expect("read request");
            assert!(read > 0, "client closed before headers");
            request.extend_from_slice(&buf[..read]);
            if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                header_end = index + 4;
                break;
            }
        }

        let headers = std::str::from_utf8(&request[..header_end]).expect("headers utf8");
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("content length"))
            })
            .expect("content-length");

        while request.len() < header_end + content_length {
            let read = socket.read(&mut buf).await.expect("read body");
            assert!(read > 0, "client closed before body");
            request.extend_from_slice(&buf[..read]);
        }

        socket
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
            .await
            .expect("response");

        serde_json::from_slice::<serde_json::Value>(
            &request[header_end..header_end + content_length],
        )
        .expect("body json")
    });

    append_log_async(
        DebugLogEntry {
            location: "test/http".to_string(),
            message: "send".to_string(),
            data: json!({
                "token": "0123456789abcdef",
                "safe": "value"
            }),
            session_id: String::new(),
            run_id: None,
            hypothesis_id: None,
            timestamp: Some(456),
            id: Some("log-http".to_string()),
        },
        Some(DebugLogConfig {
            log_path: log_path.clone(),
            ingest_url: Some(url),
            session_id: "session-http".to_string(),
        }),
        true,
    )
    .await
    .expect("append");

    let file_line = std::fs::read_to_string(log_path).expect("log file");
    let file_value: serde_json::Value = serde_json::from_str(file_line.trim()).expect("json line");
    let http_value = timeout(Duration::from_secs(5), server)
        .await
        .expect("server timeout")
        .expect("server");

    assert_eq!(http_value, file_value);
    assert_eq!(http_value["id"], "log-http");
    assert_eq!(http_value["sessionId"], "session-http");
    assert_eq!(http_value["data"]["safe"], "value");
    assert_eq!(http_value["data"]["token"], "0123456789***");
}
