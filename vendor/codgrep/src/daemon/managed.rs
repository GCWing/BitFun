use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use crate::{
    config::BuildConfig,
    error::{AppError, Result},
};

use super::{
    client::DaemonClient,
    convert::build_config_from_open,
    protocol::{EnsureRepoParams, GlobParams, OpenRepoParams, RepoStatus, Response, SearchParams},
    server::read_state_file,
};

const DEFAULT_DAEMON_STATE_FILE: &str = "daemon-state.json";
const DEFAULT_DAEMON_START_LOCK_FILE: &str = "daemon-state.lock";
const MIN_STALE_STARTUP_LOCK_AGE: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct ManagedDaemonClient {
    daemon_program: Option<OsString>,
    start_timeout: Duration,
    retry_interval: Duration,
}

#[derive(Debug, Clone)]
pub struct EnsuredRepo {
    pub addr: String,
    pub repo_id: String,
    pub status: RepoStatus,
    pub indexed_docs: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct OpenedRepo {
    pub addr: String,
    pub repo_id: String,
    pub status: RepoStatus,
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_daemon_program(mut self, program: impl Into<OsString>) -> Self {
        self.daemon_program = Some(program.into());
        self
    }

    pub fn with_start_timeout(mut self, timeout: Duration) -> Self {
        self.start_timeout = timeout;
        self
    }

    pub fn with_retry_interval(mut self, interval: Duration) -> Self {
        self.retry_interval = interval;
        self
    }

    pub fn ensure_repo(&self, params: EnsureRepoParams) -> Result<EnsuredRepo> {
        let state_file = daemon_state_file_path(&params)?;
        let lock_file = daemon_start_lock_file_path(&state_file);
        if let Ok(repo) = self.try_ensure_repo(&state_file, &params) {
            return Ok(repo);
        }

        let started = Instant::now();
        loop {
            if let Ok(repo) = self.try_ensure_repo(&state_file, &params) {
                return Ok(repo);
            }

            if let Some(_guard) =
                self.try_acquire_startup_lock(&lock_file, MIN_STALE_STARTUP_LOCK_AGE)?
            {
                if let Ok(repo) = self.try_ensure_repo(&state_file, &params) {
                    return Ok(repo);
                }
                self.spawn_daemon(&state_file)?;
                loop {
                    match self.try_ensure_repo(&state_file, &params) {
                        Ok(repo) => return Ok(repo),
                        Err(error) if started.elapsed() < self.start_timeout => {
                            let _ = error;
                            std::thread::sleep(self.retry_interval);
                        }
                        Err(error) => return Err(error),
                    }
                }
            }

            match self.try_ensure_repo(&state_file, &params) {
                Ok(repo) => return Ok(repo),
                Err(error) if started.elapsed() < self.start_timeout => {
                    let _ = error;
                    std::thread::sleep(self.retry_interval);
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub fn open_repo(&self, params: OpenRepoParams) -> Result<OpenedRepo> {
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

    pub fn search(&self, repo: EnsureRepoParams, mut params: SearchParams) -> Result<Response> {
        let ensured = self.ensure_repo(repo)?;
        params.repo_id = ensured.repo_id;
        let client = DaemonClient::new(ensured.addr);
        client.search(params)
    }

    pub fn glob(&self, repo: EnsureRepoParams, mut params: GlobParams) -> Result<Response> {
        let ensured = self.ensure_repo(repo)?;
        params.repo_id = ensured.repo_id;
        let client = DaemonClient::new(ensured.addr);
        client.glob(params)
    }

    fn try_ensure_repo(&self, state_file: &Path, params: &EnsureRepoParams) -> Result<EnsuredRepo> {
        let state = read_state_file(state_file)?;
        let client = DaemonClient::new(state.addr.clone());
        match client.ensure_repo(params.clone())? {
            Response::RepoEnsured {
                repo_id,
                status,
                indexed_docs,
            } => Ok(EnsuredRepo {
                addr: state.addr,
                repo_id,
                status,
                indexed_docs,
            }),
            other => Err(AppError::Protocol(format!(
                "unexpected ensure_repo response: {other:?}"
            ))),
        }
    }

    fn try_open_repo(&self, state_file: &Path, params: &OpenRepoParams) -> Result<OpenedRepo> {
        let state = read_state_file(state_file)?;
        let client = DaemonClient::new(state.addr.clone());
        match client.open_repo(params.clone())? {
            Response::RepoOpened { repo_id, status } => Ok(OpenedRepo {
                addr: state.addr,
                repo_id,
                status,
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
            .or_else(|| std::env::var_os("CODGREP_DAEMON_BIN"))
            .unwrap_or_else(|| OsString::from("cg"));

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

pub fn daemon_state_file_path(params: &EnsureRepoParams) -> Result<PathBuf> {
    daemon_state_file_path_from_build_config(build_config_from_ensure(params))
}

pub fn daemon_state_file_path_from_open(params: &OpenRepoParams) -> Result<PathBuf> {
    daemon_state_file_path_from_build_config(build_config_from_open_params(params))
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

fn build_config_from_ensure(params: &EnsureRepoParams) -> BuildConfig {
    build_config_from_open(
        params.repo_path.clone(),
        params.index_path.clone(),
        params.config.tokenizer,
        params.config.corpus_mode,
        params.config.include_hidden,
        params.config.max_file_size,
        params.config.min_sparse_len,
        params.config.max_sparse_len,
    )
}

fn build_config_from_open_params(params: &OpenRepoParams) -> BuildConfig {
    build_config_from_open(
        params.repo_path.clone(),
        params.index_path.clone(),
        params.config.tokenizer,
        params.config.corpus_mode,
        params.config.include_hidden,
        params.config.max_file_size,
        params.config.min_sparse_len,
        params.config.max_sparse_len,
    )
}

fn daemon_state_file_path_from_build_config(build_config: BuildConfig) -> Result<PathBuf> {
    let build_config = build_config.normalized()?;
    Ok(build_config.index_path.join(DEFAULT_DAEMON_STATE_FILE))
}
