use std::{
    fs,
    io::{BufReader, BufWriter},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    daemon::{
        protocol::{Request, ServerMessage},
        service::DaemonService,
    },
    error::{AppError, Result},
};

#[path = "server/codec.rs"]
mod codec;
#[path = "server/connection.rs"]
mod connection;

use codec::{read_content_length_request, write_content_length_message};
use connection::handle_connection;

#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub bind_addr: String,
    pub state_file: Option<PathBuf>,
    pub stdio: bool,
}

pub fn serve(options: ServerOptions) -> Result<()> {
    if options.stdio {
        return serve_stdio();
    }
    let listener = TcpListener::bind(&options.bind_addr)?;
    listener.set_nonblocking(true)?;
    let local_addr = listener.local_addr()?;
    if let Some(path) = options.state_file.as_deref() {
        write_state_file(path, &local_addr.to_string())?;
    }
    let state_guard = StateFileGuard {
        path: options.state_file.clone(),
    };
    println!("codgrep daemon listening on {local_addr}");

    let running = Arc::new(AtomicBool::new(true));
    let service = DaemonService::new();

    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                let service = service.clone();
                let running = Arc::clone(&running);
                thread::spawn(move || {
                    if let Err(error) = handle_connection(stream, &service, &running) {
                        eprintln!("daemon connection error: {error}");
                    }
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(error.into()),
        }
    }

    service.shutdown_all();
    drop(state_guard);
    Ok(())
}

pub fn serve_stdio() -> Result<()> {
    let service = DaemonService::new();
    let (out_tx, out_rx) = mpsc::channel::<ServerMessage>();
    let connection_id = service.register_connection(out_tx.clone());
    let writer_handle = thread::spawn(move || -> Result<()> {
        let stdout = std::io::stdout();
        let mut writer = BufWriter::new(stdout.lock());
        while let Ok(message) = out_rx.recv() {
            write_content_length_message(&mut writer, &message)?;
        }
        Ok(())
    });

    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    loop {
        let Some(request) = read_content_length_request(&mut reader)? else {
            break;
        };
        let is_shutdown = matches!(request.request, Request::Shutdown | Request::Exit);
        if let Some(response) = service.handle_for_connection(Some(connection_id), request) {
            out_tx
                .send(ServerMessage::Response(response))
                .map_err(|error| AppError::Protocol(format!("failed to send response: {error}")))?;
        }
        if is_shutdown {
            break;
        }
    }

    service.unregister_connection(connection_id);
    service.shutdown_all();
    drop(out_tx);
    writer_handle
        .join()
        .map_err(|_| AppError::Protocol("daemon stdio writer thread panicked".into()))??;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStateFile {
    pub version: u32,
    pub addr: String,
    pub pid: u32,
    pub updated_unix_secs: u64,
}

struct StateFileGuard {
    path: Option<PathBuf>,
}

impl Drop for StateFileGuard {
    fn drop(&mut self) {
        let Some(path) = self.path.as_deref() else {
            return;
        };
        let _ = fs::remove_file(path);
    }
}

pub fn read_state_file(path: &Path) -> Result<DaemonStateFile> {
    let contents = fs::read_to_string(path)?;
    serde_json::from_str(&contents)
        .map_err(|error| AppError::Protocol(format!("invalid daemon state file: {error}")))
}

fn write_state_file(path: &Path, addr: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let record = DaemonStateFile {
        version: 1,
        addr: addr.to_string(),
        pid: std::process::id(),
        updated_unix_secs: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    let json = serde_json::to_vec_pretty(&record).map_err(|error| {
        AppError::Protocol(format!("failed to encode daemon state file: {error}"))
    })?;
    fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{BufRead, BufReader, BufWriter, Write},
        net::TcpStream,
        thread,
        time::Duration,
    };

    use tempfile::tempdir;

    use super::*;
    use crate::daemon::{
        protocol::{
            ClientCapabilities, InitializeParams, Notification, OpenRepoParams, RepoConfig,
            Request, RequestEnvelope, Response, ResponseEnvelope, ServerMessage,
        },
        DaemonClient,
    };

    #[test]
    fn serve_writes_and_removes_state_file() {
        let temp = tempdir().expect("temp dir should succeed");
        let state_file = temp.path().join("daemon-state.json");

        let handle = thread::spawn({
            let state_file = state_file.clone();
            move || {
                serve(ServerOptions {
                    bind_addr: "127.0.0.1:0".into(),
                    state_file: Some(state_file),
                    stdio: false,
                })
                .expect("serve should succeed");
            }
        });

        let state = {
            let mut record = None;
            for _ in 0..100 {
                if let Ok(current) = read_state_file(&state_file) {
                    record = Some(current);
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            record.expect("state file should appear")
        };

        let client = DaemonClient::new(state.addr);
        client
            .send(Request::Shutdown)
            .expect("shutdown should succeed");
        handle.join().expect("server thread should join");
        assert!(!state_file.exists());
    }

    #[test]
    fn initialized_connection_receives_task_notifications() {
        let temp = tempdir().expect("temp dir should succeed");
        let state_file = temp.path().join("daemon-state.json");
        let repo_path = temp.path().join("repo");
        fs::create_dir_all(&repo_path).expect("repo dir should succeed");
        for index in 0..8 {
            fs::write(
                repo_path.join(format!("file-{index}.rs")),
                format!("pub const NAME_{index}: &str = \"HELLO_{index}\";\n"),
            )
            .expect("write should succeed");
        }

        let handle = thread::spawn({
            let state_file = state_file.clone();
            move || {
                serve(ServerOptions {
                    bind_addr: "127.0.0.1:0".into(),
                    state_file: Some(state_file),
                    stdio: false,
                })
                .expect("serve should succeed");
            }
        });

        let state = {
            let mut record = None;
            for _ in 0..100 {
                if let Ok(current) = read_state_file(&state_file) {
                    record = Some(current);
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            record.expect("state file should appear")
        };

        let stream = TcpStream::connect(&state.addr).expect("connect should succeed");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set timeout should succeed");
        let mut reader = BufReader::new(stream.try_clone().expect("clone should succeed"));
        let mut writer = BufWriter::new(stream);

        send_request(
            &mut writer,
            Some(1),
            Request::Initialize {
                params: InitializeParams {
                    client_info: None,
                    capabilities: ClientCapabilities {
                        progress: true,
                        status_notifications: true,
                        task_notifications: true,
                    },
                },
            },
        );
        match read_message(&mut reader) {
            ServerMessage::Response(response) => {
                match response.result.expect("result should exist") {
                    Response::InitializeResult { .. } => {}
                    other => panic!("unexpected initialize response: {other:?}"),
                }
            }
            other => panic!("unexpected message: {other:?}"),
        }

        send_request(&mut writer, None, Request::Initialized);
        send_request(
            &mut writer,
            Some(2),
            Request::OpenRepo {
                params: OpenRepoParams {
                    repo_path: repo_path.clone(),
                    index_path: None,
                    config: RepoConfig::default(),
                    refresh: Default::default(),
                },
            },
        );
        loop {
            match read_message(&mut reader) {
                ServerMessage::Response(response) => {
                    if let Some(error) = response.error {
                        panic!("unexpected error response: {error:?}");
                    }
                    match response.result.expect("result should exist") {
                        Response::RepoOpened { .. } => break,
                        other => panic!("unexpected open response: {other:?}"),
                    }
                }
                ServerMessage::Notification(_) => {}
            }
        }

        let repo_id = fs::canonicalize(&repo_path)
            .expect("repo should canonicalize")
            .to_string_lossy()
            .into_owned();
        let mut saw_status_changed = false;
        let mut saw_progress = false;
        let mut saw_task_finished = false;
        let mut task_id = None;

        send_request(
            &mut writer,
            Some(3),
            Request::IndexBuild {
                params: crate::daemon::protocol::RepoRef {
                    repo_id: repo_id.clone(),
                },
            },
        );

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            match read_message(&mut reader) {
                ServerMessage::Response(response) => {
                    if let Some(error) = response.error {
                        panic!("unexpected error response: {error:?}");
                    }
                    match response.result.expect("result should exist") {
                        Response::TaskStarted { task } => {
                            task_id = Some(task.task_id);
                        }
                        Response::ShutdownAck => break,
                        other => panic!("unexpected response: {other:?}"),
                    }
                }
                ServerMessage::Notification(notification) => match notification.notification {
                    Notification::Progress { params } => {
                        if params.workspace_id == repo_id {
                            saw_progress = true;
                        }
                    }
                    Notification::WorkspaceStatusChanged { params } => {
                        if params.workspace_id == repo_id {
                            saw_status_changed = true;
                        }
                    }
                    Notification::TaskFinished { params } => {
                        if params.task.workspace_id == repo_id {
                            saw_task_finished = true;
                            if let Some(expected) = task_id.as_ref() {
                                assert_eq!(&params.task.task_id, expected);
                            }
                        }
                    }
                },
            }
            if task_id.is_some() && saw_progress && saw_status_changed && saw_task_finished {
                break;
            }
        }

        assert!(task_id.is_some(), "expected task_started response");
        assert!(saw_progress, "expected progress notification");
        assert!(
            saw_status_changed,
            "expected workspace/statusChanged notification"
        );
        assert!(saw_task_finished, "expected task/finished notification");

        let client = DaemonClient::new(state.addr);
        client
            .send(Request::Shutdown)
            .expect("shutdown should succeed");
        handle.join().expect("server thread should join");
    }

    fn send_request(writer: &mut BufWriter<TcpStream>, id: Option<u64>, request: Request) {
        let envelope = RequestEnvelope {
            jsonrpc: "2.0".into(),
            id,
            request,
        };
        serde_json::to_writer(&mut *writer, &envelope).expect("encode should succeed");
        writer.write_all(b"\n").expect("newline should succeed");
        writer.flush().expect("flush should succeed");
    }

    fn read_message(reader: &mut BufReader<TcpStream>) -> ServerMessage {
        loop {
            let mut line = String::new();
            let read = reader.read_line(&mut line).expect("read should succeed");
            assert!(read > 0, "expected server message");
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(&line).unwrap_or_else(|error| {
                panic!("decode json should succeed: {error}; line={line:?}")
            });
            if value.get("method").is_some() {
                let envelope = serde_json::from_value(value).unwrap_or_else(|error| {
                    panic!("decode notification should succeed: {error}; line={line:?}")
                });
                return ServerMessage::Notification(envelope);
            }
            let envelope: ResponseEnvelope =
                serde_json::from_value(value).unwrap_or_else(|error| {
                    panic!("decode response should succeed: {error}; line={line:?}")
                });
            return ServerMessage::Response(envelope);
        }
    }

    #[test]
    fn content_length_stdio_round_trip() {
        let message = ServerMessage::Response(ResponseEnvelope::success(
            Some(7),
            Response::Pong { now_unix_secs: 123 },
        ));
        let mut encoded = Vec::new();
        write_content_length_message(&mut encoded, &message).expect("encode should succeed");
        let mut reader = BufReader::new(std::io::Cursor::new(encoded));
        let request = RequestEnvelope {
            jsonrpc: "2.0".into(),
            id: Some(1),
            request: Request::Ping,
        };
        let mut raw = Vec::new();
        let body = serde_json::to_vec(&request).expect("request should encode");
        write!(&mut raw, "Content-Length: {}\r\n\r\n", body.len()).expect("write should succeed");
        raw.extend_from_slice(&body);
        let mut request_reader = BufReader::new(std::io::Cursor::new(raw));

        let parsed = read_content_length_request(&mut request_reader)
            .expect("request should parse")
            .expect("request should exist");
        assert_eq!(parsed.id, Some(1));
        assert!(matches!(parsed.request, Request::Ping));

        let mut header = String::new();
        reader.read_line(&mut header).expect("header should read");
        assert!(header.starts_with("Content-Length: "));
    }
}
