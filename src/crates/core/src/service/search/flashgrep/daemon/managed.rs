use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use serde::Deserialize;

use crate::service::search::flashgrep::error::{AppError, Result};

use super::protocol::{OpenRepoParams, Request, RequestEnvelope, Response, ResponseEnvelope};

const DEFAULT_DAEMON_STATE_FILE: &str = "daemon-state.json";
const DEFAULT_DAEMON_START_LOCK_FILE: &str = "daemon-state.lock";
const MIN_STALE_STARTUP_LOCK_AGE: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub(crate) struct ManagedDaemonClient {
    daemon_program: Option<OsString>,
    start_timeout: Duration,
    retry_interval: Duration,
}

#[derive(Debug, Clone)]
pub(crate) struct OpenedRepo {
    pub addr: String,
    pub repo_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DaemonStateFile {
    addr: String,
}

impl Default for ManagedDaemonClient {
    fn default() -> Self {
        Self {
            daemon_program: None,
            start_timeout: Duration::from_secs(10),
            retry_interval: Duration::from_millis(100),
        }
    }
}

impl ManagedDaemonClient {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_daemon_program(mut self, program: impl Into<OsString>) -> Self {
        self.daemon_program = Some(program.into());
        self
    }

    pub(crate) fn with_start_timeout(mut self, timeout: Duration) -> Self {
        self.start_timeout = timeout;
        self
    }

    pub(crate) fn with_retry_interval(mut self, interval: Duration) -> Self {
        self.retry_interval = interval;
        self
    }

    pub(crate) fn open_repo(&self, params: OpenRepoParams) -> Result<OpenedRepo> {
        let state_file = daemon_state_file_path_from_open(&params)?;
        let lock_file = daemon_start_lock_file_path(&state_file);
        if let Ok(repo) = self.try_open_repo(&state_file, &params) {
            return Ok(repo);
        }

        let started = Instant::now();
        loop {
            if let Ok(repo) = self.try_open_repo(&state_file, &params) {
                return Ok(repo);
            }

            if let Some(_guard) =
                self.try_acquire_startup_lock(&lock_file, MIN_STALE_STARTUP_LOCK_AGE)?
            {
                if let Ok(repo) = self.try_open_repo(&state_file, &params) {
                    return Ok(repo);
                }
                self.spawn_daemon(&state_file)?;
                loop {
                    match self.try_open_repo(&state_file, &params) {
                        Ok(repo) => return Ok(repo),
                        Err(error) if started.elapsed() < self.start_timeout => {
                            let _ = error;
                            std::thread::sleep(self.retry_interval);
                        }
                        Err(error) => return Err(error),
                    }
                }
            }

            match self.try_open_repo(&state_file, &params) {
                Ok(repo) => return Ok(repo),
                Err(error) if started.elapsed() < self.start_timeout => {
                    let _ = error;
                    std::thread::sleep(self.retry_interval);
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn try_open_repo(&self, state_file: &Path, params: &OpenRepoParams) -> Result<OpenedRepo> {
        let state = read_state_file(state_file)?;
        match send_request(
            &state.addr,
            Request::OpenRepo {
                params: params.clone(),
            },
        )? {
            Response::RepoOpened { repo_id, status: _ } => Ok(OpenedRepo {
                addr: state.addr,
                repo_id,
            }),
            other => Err(AppError::Protocol(format!(
                "unexpected open_repo response: {other:?}"
            ))),
        }
    }

    fn try_acquire_startup_lock(
        &self,
        lock_file: &Path,
        stale_after: Duration,
    ) -> Result<Option<StartupLockGuard>> {
        if let Some(parent) = lock_file.parent() {
            fs::create_dir_all(parent)?;
        }

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(lock_file)
        {
            Ok(mut file) => {
                let _ = writeln!(file, "pid={}", std::process::id());
                Ok(Some(StartupLockGuard {
                    path: lock_file.to_path_buf(),
                }))
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if startup_lock_is_stale(lock_file, stale_after) {
                    match fs::remove_file(lock_file) {
                        Ok(()) => self.try_acquire_startup_lock(lock_file, stale_after),
                        Err(remove_error)
                            if remove_error.kind() == std::io::ErrorKind::NotFound =>
                        {
                            Ok(None)
                        }
                        Err(remove_error) => Err(remove_error.into()),
                    }
                } else {
                    Ok(None)
                }
            }
            Err(error) => Err(error.into()),
        }
    }

    fn spawn_daemon(&self, state_file: &Path) -> Result<()> {
        if state_file.exists() {
            fs::remove_file(state_file)?;
        }

        let program = self
            .daemon_program
            .clone()
            .or_else(|| std::env::var_os("FLASHGREP_DAEMON_BIN"))
            .unwrap_or_else(|| OsString::from("flashgrep"));

        let mut command = Command::new(program);
        command
            .arg("serve")
            .arg("--bind")
            .arg("127.0.0.1:0")
            .arg("--state-file")
            .arg(state_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.spawn()?;
        Ok(())
    }
}

struct StartupLockGuard {
    path: PathBuf,
}

impl Drop for StartupLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn read_state_file(path: &Path) -> Result<DaemonStateFile> {
    let contents = fs::read_to_string(path)?;
    serde_json::from_str(&contents)
        .map_err(|error| AppError::Protocol(format!("invalid daemon state file: {error}")))
}

fn send_request(addr: &str, request: Request) -> Result<Response> {
    let envelope = RequestEnvelope {
        jsonrpc: "2.0".into(),
        id: Some(1),
        request,
    };

    let stream = std::net::TcpStream::connect(addr)?;
    let reader_stream = stream.try_clone()?;
    let mut reader = std::io::BufReader::new(reader_stream);
    let mut writer = std::io::BufWriter::new(stream);

    serde_json::to_writer(&mut writer, &envelope)
        .map_err(|error| AppError::Protocol(format!("failed to encode request: {error}")))?;
    writer.write_all(b"\n")?;
    writer.flush()?;

    let mut line = String::new();
    let read = std::io::BufRead::read_line(&mut reader, &mut line)?;
    if read == 0 {
        return Err(AppError::Protocol(
            "daemon closed connection without a response".into(),
        ));
    }

    let response: ResponseEnvelope = serde_json::from_str(&line)
        .map_err(|error| AppError::Protocol(format!("failed to decode response: {error}")))?;

    if response.jsonrpc != "2.0" {
        return Err(AppError::Protocol(format!(
            "unsupported daemon jsonrpc version: {}",
            response.jsonrpc
        )));
    }

    if let Some(error) = response.error {
        return Err(AppError::Protocol(error.message));
    }

    response
        .result
        .ok_or_else(|| AppError::Protocol("daemon response missing result".into()))
}

fn daemon_state_file_path_from_open(params: &OpenRepoParams) -> Result<PathBuf> {
    let storage_root = params
        .storage_root
        .clone()
        .unwrap_or_else(|| params.repo_path.join(".flashgrep-index-engine"));
    Ok(storage_root.join(DEFAULT_DAEMON_STATE_FILE))
}

fn daemon_start_lock_file_path(state_file: &Path) -> PathBuf {
    state_file
        .parent()
        .map(|parent| parent.join(DEFAULT_DAEMON_START_LOCK_FILE))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DAEMON_START_LOCK_FILE))
}

fn startup_lock_is_stale(path: &Path, stale_after: Duration) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age >= stale_after)
}
