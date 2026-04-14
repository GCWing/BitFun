use std::{
    io::{BufRead, BufReader, BufWriter, Write},
    net::TcpStream,
    sync::atomic::{AtomicU64, Ordering},
    sync::Mutex,
};

use crate::error::{AppError, Result};

use super::protocol::{
    EnsureRepoParams, GlobParams, OpenRepoParams, RepoRef, Request, RequestEnvelope, Response,
    ResponseEnvelope, SearchParams, TaskRef,
};

#[derive(Debug)]
pub struct DaemonClient {
    addr: String,
    next_id: AtomicU64,
    connection: Mutex<Option<DaemonConnection>>,
}

#[derive(Debug)]
struct DaemonConnection {
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
}

impl DaemonClient {
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            addr: addr.into(),
            next_id: AtomicU64::new(1),
            connection: Mutex::new(None),
        }
    }

    pub fn send(&self, request: Request) -> Result<Response> {
        let request_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let envelope = RequestEnvelope {
            jsonrpc: "2.0".into(),
            id: Some(request_id),
            request,
        };

        let mut connection = self
            .connection
            .lock()
            .map_err(|_| AppError::Protocol("daemon connection mutex poisoned".into()))?;
        let response = match self.send_with_connection(&mut connection, &envelope) {
            Ok(response) => response,
            Err(_) => {
                *connection = None;
                self.send_with_connection(&mut connection, &envelope)?
            }
        };

        if response.id != Some(request_id) {
            return Err(AppError::Protocol(format!(
                "daemon response id mismatch: expected {request_id:?}, got {:?}",
                response.id
            )));
        }

        let ResponseEnvelope {
            jsonrpc,
            result,
            error,
            ..
        } = response;

        if jsonrpc != "2.0" {
            return Err(AppError::Protocol(format!(
                "unsupported daemon jsonrpc version: {}",
                jsonrpc
            )));
        }

        if let Some(error) = error {
            return Err(AppError::Protocol(error.message));
        }

        result.ok_or_else(|| AppError::Protocol("daemon response missing result".into()))
    }

    fn send_with_connection(
        &self,
        connection: &mut Option<DaemonConnection>,
        envelope: &RequestEnvelope,
    ) -> Result<ResponseEnvelope> {
        let connection = match connection {
            Some(connection) => connection,
            None => {
                *connection = Some(self.connect()?);
                connection
                    .as_mut()
                    .expect("connection must exist after successful connect")
            }
        };

        serde_json::to_writer(&mut connection.writer, envelope)
            .map_err(|error| AppError::Protocol(format!("failed to encode request: {error}")))?;
        connection.writer.write_all(b"\n")?;
        connection.writer.flush()?;

        let mut line = String::new();
        let read = connection.reader.read_line(&mut line)?;
        if read == 0 {
            return Err(AppError::Protocol(
                "daemon closed connection without a response".into(),
            ));
        }
        serde_json::from_str(&line)
            .map_err(|error| AppError::Protocol(format!("failed to decode response: {error}")))
    }

    fn connect(&self) -> Result<DaemonConnection> {
        let stream = TcpStream::connect(&self.addr)?;
        let reader = BufReader::new(stream.try_clone()?);
        let writer = BufWriter::new(stream);
        Ok(DaemonConnection { reader, writer })
    }

    pub fn open_repo(&self, params: OpenRepoParams) -> Result<Response> {
        self.send(Request::OpenRepo { params })
    }

    pub fn ensure_repo(&self, params: EnsureRepoParams) -> Result<Response> {
        self.send(Request::EnsureRepo { params })
    }

    pub fn get_repo_status(&self, repo_id: impl Into<String>) -> Result<Response> {
        self.send(Request::GetRepoStatus {
            params: RepoRef {
                repo_id: repo_id.into(),
            },
        })
    }

    pub fn search(&self, params: SearchParams) -> Result<Response> {
        self.send(Request::Search { params })
    }

    pub fn glob(&self, params: GlobParams) -> Result<Response> {
        self.send(Request::Glob { params })
    }

    pub fn index_build(&self, repo_id: impl Into<String>) -> Result<Response> {
        self.send(Request::IndexBuild {
            params: RepoRef {
                repo_id: repo_id.into(),
            },
        })
    }

    pub fn index_rebuild(&self, repo_id: impl Into<String>) -> Result<Response> {
        self.send(Request::IndexRebuild {
            params: RepoRef {
                repo_id: repo_id.into(),
            },
        })
    }

    pub fn task_status(&self, task_id: impl Into<String>) -> Result<Response> {
        self.send(Request::TaskStatus {
            params: TaskRef {
                task_id: task_id.into(),
            },
        })
    }

    pub fn task_cancel(&self, task_id: impl Into<String>) -> Result<Response> {
        self.send(Request::TaskCancel {
            params: TaskRef {
                task_id: task_id.into(),
            },
        })
    }
}
