//! Managed LoopX CLI integration for the built-in LoopX MiniApp.
//!
//! The product-facing adapter below never accepts an executable or an argv
//! prefix from callers. It selects the packaged binary first and only permits
//! the fixed `loopx` system command when that fallback was explicitly enabled.

use async_trait::async_trait;
use bitfun_product_domains::miniapp::loopx as loopx_contract;
use bitfun_services_core::process_tree::ProcessTreeChild;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

pub const LOOPX_PINNED_VERSION: &str = "0.5.1";
pub const LOOPX_PINNED_VERSION_TAG: &str = "v0.5.1";
pub const LOOPX_PINNED_VERSION_OUTPUT: &str = "loopx 0.5.1";
pub const LOOPX_BUNDLE_MANIFEST_SCHEMA: u32 = 1;
pub const LOOPX_COMMAND_REFERENCE_SCHEMA: &str = "loopx_command_reference_v0";

const MAX_STDOUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_STDERR_TAIL_BYTES: usize = 32 * 1024;
const MAX_PROGRESS_LINE_BYTES: usize = 4 * 1024;
const PIPE_DRAIN_DEADLINE: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopxSystemFallbackPolicy {
    Disabled,
    ExactPinned,
}

#[derive(Debug, Clone)]
pub struct LoopxCliAdapterConfig {
    pub resource_dir: PathBuf,
    pub system_fallback: LoopxSystemFallbackPolicy,
    pub startup_deadline: Duration,
    pub command_deadline: Duration,
    pub terminate_grace: Duration,
}

impl LoopxCliAdapterConfig {
    pub fn packaged(resource_dir: impl Into<PathBuf>) -> Self {
        Self {
            resource_dir: resource_dir.into(),
            system_fallback: LoopxSystemFallbackPolicy::Disabled,
            startup_deadline: Duration::from_secs(60),
            command_deadline: Duration::from_secs(180),
            terminate_grace: Duration::from_secs(2),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopxCommandSource {
    PackagedBundle,
    FixedSystemCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedLoopxCommand {
    pub executable: PathBuf,
    pub source: LoopxCommandSource,
    pub version: String,
    pub bundle_manifest_schema: Option<u32>,
    pub command_reference_schema: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopxCommandPlan {
    pub operation_id: String,
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: Option<PathBuf>,
    pub environment: BTreeMap<OsString, OsString>,
    pub deadline: Duration,
    pub terminate_grace: Duration,
}

impl LoopxCommandPlan {
    fn handshake(
        operation_id: impl Into<String>,
        executable: PathBuf,
        args: impl IntoIterator<Item = impl Into<OsString>>,
        deadline: Duration,
        terminate_grace: Duration,
    ) -> Self {
        Self {
            operation_id: operation_id.into(),
            executable,
            args: args.into_iter().map(Into::into).collect(),
            current_dir: None,
            environment: BTreeMap::new(),
            deadline,
            terminate_grace,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopxProgressStage {
    Starting,
    Stderr,
    Exited,
    Cancelling,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopxProcessProgress {
    pub operation_id: String,
    pub stage: LoopxProgressStage,
    pub message: String,
    pub occurred_at_unix_ms: u64,
}

pub trait LoopxProcessObserver: Send + Sync {
    fn on_progress(&self, progress: LoopxProcessProgress);
}

#[derive(Debug, Default)]
pub struct NoopLoopxProcessObserver;

impl LoopxProcessObserver for NoopLoopxProcessObserver {
    fn on_progress(&self, _progress: LoopxProcessProgress) {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopxProcessOutput {
    pub stdout: String,
    pub stderr_tail: Vec<String>,
    pub elapsed: Duration,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LoopxProcessError {
    #[error("failed to start LoopX process: {message}")]
    Start { message: String },
    #[error("LoopX process IO failed: {message}")]
    Io { message: String },
    #[error("LoopX process exited with status {code:?}")]
    Exited {
        code: Option<i32>,
        stdout_tail: Vec<String>,
        stderr_tail: Vec<String>,
    },
    #[error("LoopX process timed out after {deadline_ms} ms")]
    Timeout {
        deadline_ms: u64,
        stderr_tail: Vec<String>,
    },
    #[error("LoopX process was cancelled")]
    Cancelled { stderr_tail: Vec<String> },
    #[error("LoopX stdout exceeded the {limit_bytes}-byte limit")]
    OutputLimit { limit_bytes: usize },
}

#[async_trait]
pub trait LoopxProcessRunner: Send + Sync {
    async fn run(
        &self,
        plan: LoopxCommandPlan,
        cancellation: CancellationToken,
        observer: &dyn LoopxProcessObserver,
    ) -> Result<LoopxProcessOutput, LoopxProcessError>;
}

#[derive(Debug, Default)]
pub struct SystemLoopxProcessRunner;

#[async_trait]
impl LoopxProcessRunner for SystemLoopxProcessRunner {
    async fn run(
        &self,
        plan: LoopxCommandPlan,
        cancellation: CancellationToken,
        observer: &dyn LoopxProcessObserver,
    ) -> Result<LoopxProcessOutput, LoopxProcessError> {
        emit_progress(
            observer,
            &plan.operation_id,
            LoopxProgressStage::Starting,
            "Starting LoopX process",
        );

        let started = Instant::now();
        let mut command = Command::new(&plan.executable);
        command
            .args(&plan.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(current_dir) = &plan.current_dir {
            command.current_dir(current_dir);
        }
        if !plan.environment.is_empty() {
            command.envs(&plan.environment);
        }

        let mut child = ProcessTreeChild::spawn(&mut command)
            .await
            .map_err(|error| LoopxProcessError::Start {
                message: error.to_string(),
            })?;
        let stdout = child.take_stdout().ok_or_else(|| LoopxProcessError::Io {
            message: "LoopX stdout pipe was not available".to_string(),
        })?;
        let stderr = child.take_stderr().ok_or_else(|| LoopxProcessError::Io {
            message: "LoopX stderr pipe was not available".to_string(),
        })?;

        let mut stdout_task = tokio::spawn(capture_stdout(stdout));
        let (stderr_line_tx, mut stderr_line_rx) = mpsc::channel(128);
        let mut stderr_task =
            tokio::spawn(async move { capture_stderr(stderr, stderr_line_tx).await });

        enum Completion {
            Exited(std::io::Result<std::process::ExitStatus>),
            Cancelled,
            TimedOut,
        }

        let deadline = tokio::time::sleep(plan.deadline);
        tokio::pin!(deadline);
        let completion = loop {
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => break Completion::Cancelled,
                _ = &mut deadline => break Completion::TimedOut,
                status = child.wait() => break Completion::Exited(status),
                line = stderr_line_rx.recv() => {
                    if let Some(line) = line {
                        emit_progress(
                            observer,
                            &plan.operation_id,
                            LoopxProgressStage::Stderr,
                            &line,
                        );
                    }
                }
            }
        };
        while let Ok(line) = stderr_line_rx.try_recv() {
            emit_progress(
                observer,
                &plan.operation_id,
                LoopxProgressStage::Stderr,
                &line,
            );
        }

        match completion {
            Completion::Cancelled => {
                emit_progress(
                    observer,
                    &plan.operation_id,
                    LoopxProgressStage::Cancelling,
                    "Cancelling LoopX process tree",
                );
                let _ = child.terminate(plan.terminate_grace).await;
                let stderr_tail = drain_stderr_task(&mut stderr_task).await;
                stdout_task.abort();
                Err(LoopxProcessError::Cancelled { stderr_tail })
            }
            Completion::TimedOut => {
                emit_progress(
                    observer,
                    &plan.operation_id,
                    LoopxProgressStage::TimedOut,
                    "LoopX process deadline expired",
                );
                let _ = child.terminate(plan.terminate_grace).await;
                let stderr_tail = drain_stderr_task(&mut stderr_task).await;
                stdout_task.abort();
                Err(LoopxProcessError::Timeout {
                    deadline_ms: duration_millis(plan.deadline),
                    stderr_tail,
                })
            }
            Completion::Exited(status) => {
                let status = status.map_err(|error| LoopxProcessError::Io {
                    message: error.to_string(),
                })?;
                let stdout_capture = drain_stdout_task(&mut stdout_task).await?;
                let stderr_tail = drain_stderr_task(&mut stderr_task).await;
                emit_progress(
                    observer,
                    &plan.operation_id,
                    LoopxProgressStage::Exited,
                    if status.success() {
                        "LoopX process exited successfully"
                    } else {
                        "LoopX process exited with an error"
                    },
                );
                if !status.success() {
                    return Err(LoopxProcessError::Exited {
                        code: status.code(),
                        stdout_tail: output_tail(&stdout_capture.bytes),
                        stderr_tail,
                    });
                }
                if stdout_capture.exceeded_limit {
                    return Err(LoopxProcessError::OutputLimit {
                        limit_bytes: MAX_STDOUT_BYTES,
                    });
                }
                Ok(LoopxProcessOutput {
                    stdout: String::from_utf8_lossy(&stdout_capture.bytes).into_owned(),
                    stderr_tail,
                    elapsed: started.elapsed(),
                })
            }
        }
    }
}

pub trait LoopxFixedCommandLocator: Send + Sync {
    fn locate(&self) -> Result<Option<PathBuf>, String>;
}

#[async_trait]
pub trait LoopxIntakeMetadataProvider: Send + Sync {
    async fn resolve(
        &self,
        request: &loopx_contract::LoopxCliResolveIntakeRequest,
        deadline: Duration,
    ) -> loopx_contract::LoopxCliResult<loopx_contract::LoopxCliResolveIntakeResult>;

    /// Pre-flight GitHub access probe used to populate the `github_auth`
    /// environment fact before any intake is submitted.
    async fn probe_auth(
        &self,
        deadline: Duration,
    ) -> loopx_contract::LoopxCliResult<loopx_contract::LoopxGithubAuthProbe>;
}

#[derive(Debug, Default)]
pub struct UnsupportedLoopxIntakeMetadataProvider;

#[async_trait]
impl LoopxIntakeMetadataProvider for UnsupportedLoopxIntakeMetadataProvider {
    async fn resolve(
        &self,
        request: &loopx_contract::LoopxCliResolveIntakeRequest,
        _deadline: Duration,
    ) -> loopx_contract::LoopxCliResult<loopx_contract::LoopxCliResolveIntakeResult> {
        Err(loopx_contract::LoopxCliError::new(
            loopx_contract::LoopxCliErrorKind::NotFound,
            "LoopX intake metadata provider is not configured",
        )
        .for_operation(&request.call.operation_id)
        .retryable(true))
    }

    async fn probe_auth(
        &self,
        _deadline: Duration,
    ) -> loopx_contract::LoopxCliResult<loopx_contract::LoopxGithubAuthProbe> {
        Ok(loopx_contract::LoopxGithubAuthProbe {
            authenticated: false,
            detail: Some("GitHub intake metadata provider is not configured".to_string()),
            ..loopx_contract::LoopxGithubAuthProbe::default()
        })
    }
}

#[derive(Debug, Default)]
pub struct SystemLoopxFixedCommandLocator;

impl LoopxFixedCommandLocator for SystemLoopxFixedCommandLocator {
    fn locate(&self) -> Result<Option<PathBuf>, String> {
        match which::which("loopx") {
            Ok(path) => Ok(Some(path)),
            Err(which::Error::CannotFindBinaryPath) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LoopxCliAdapterError {
    #[error("packaged LoopX bundle is not available")]
    Unavailable,
    #[error("invalid packaged LoopX manifest: {message}")]
    Manifest { message: String },
    #[error("LoopX version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },
    #[error("LoopX schema mismatch: expected {expected}, got {actual}")]
    SchemaMismatch { expected: String, actual: String },
    #[error("LoopX operation id is already running: {operation_id}")]
    Conflict { operation_id: String },
    #[error("LoopX returned invalid JSON: {message}")]
    InvalidJson { message: String },
    #[error(transparent)]
    Process(#[from] LoopxProcessError),
}

#[derive(Debug, Clone)]
pub struct LoopxJsonOutput {
    pub payload: Value,
    pub stderr_tail: Vec<String>,
    pub elapsed: Duration,
}

pub struct LoopxCliProcessAdapter {
    config: LoopxCliAdapterConfig,
    runner: Arc<dyn LoopxProcessRunner>,
    locator: Arc<dyn LoopxFixedCommandLocator>,
    observer: Arc<dyn LoopxProcessObserver>,
    intake_metadata: Arc<dyn LoopxIntakeMetadataProvider>,
    intake_metadata_configured: bool,
    verified: Mutex<Option<VerifiedLoopxCommand>>,
    running: Arc<StdMutex<HashMap<String, CancellationToken>>>,
}

impl std::fmt::Debug for LoopxCliProcessAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LoopxCliProcessAdapter")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl LoopxCliProcessAdapter {
    pub fn new(config: LoopxCliAdapterConfig) -> Self {
        Self::with_dependencies(
            config,
            Arc::new(SystemLoopxProcessRunner),
            Arc::new(SystemLoopxFixedCommandLocator),
            Arc::new(NoopLoopxProcessObserver),
        )
    }

    pub fn with_dependencies(
        config: LoopxCliAdapterConfig,
        runner: Arc<dyn LoopxProcessRunner>,
        locator: Arc<dyn LoopxFixedCommandLocator>,
        observer: Arc<dyn LoopxProcessObserver>,
    ) -> Self {
        Self {
            config,
            runner,
            locator,
            observer,
            intake_metadata: Arc::new(UnsupportedLoopxIntakeMetadataProvider),
            intake_metadata_configured: false,
            verified: Mutex::new(None),
            running: Arc::new(StdMutex::new(HashMap::new())),
        }
    }

    pub fn with_intake_metadata_provider(
        mut self,
        provider: Arc<dyn LoopxIntakeMetadataProvider>,
    ) -> Self {
        self.intake_metadata = provider;
        self.intake_metadata_configured = true;
        self
    }

    pub async fn verify_handshake(
        &self,
        operation_id: &str,
    ) -> Result<VerifiedLoopxCommand, LoopxCliAdapterError> {
        let (cancellation, _registration) = self.register_operation(operation_id)?;
        self.ensure_verified(
            operation_id,
            cancellation,
            self.config.startup_deadline,
            self.observer.as_ref(),
        )
        .await
    }

    pub fn cancel_operation(&self, operation_id: &str) -> bool {
        let running = self
            .running
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(cancellation) = running.get(operation_id) {
            cancellation.cancel();
            true
        } else {
            false
        }
    }

    async fn run_json_command(
        &self,
        operation_id: &str,
        registry_path: &Path,
        current_dir: Option<&Path>,
        command_args: Vec<OsString>,
        deadline: Duration,
        observer: &dyn LoopxProcessObserver,
    ) -> Result<LoopxJsonOutput, LoopxCliAdapterError> {
        let (cancellation, _registration) = self.register_operation(operation_id)?;
        let verified = self
            .ensure_verified(
                operation_id,
                cancellation.clone(),
                deadline.min(self.config.startup_deadline),
                observer,
            )
            .await?;
        let mut args = vec![
            OsString::from("--format"),
            OsString::from("json"),
            OsString::from("--registry"),
            registry_path.as_os_str().to_owned(),
        ];
        args.extend(command_args);
        let output = self
            .runner
            .run(
                LoopxCommandPlan {
                    operation_id: operation_id.to_string(),
                    executable: verified.executable,
                    args,
                    current_dir: current_dir.map(Path::to_path_buf),
                    environment: BTreeMap::new(),
                    deadline,
                    terminate_grace: self.config.terminate_grace,
                },
                cancellation,
                observer,
            )
            .await?;
        let payload = serde_json::from_str(&output.stdout).map_err(|error| {
            LoopxCliAdapterError::InvalidJson {
                message: error.to_string(),
            }
        })?;
        Ok(LoopxJsonOutput {
            payload,
            stderr_tail: output.stderr_tail,
            elapsed: output.elapsed,
        })
    }

    async fn ensure_verified(
        &self,
        operation_id: &str,
        cancellation: CancellationToken,
        startup_deadline: Duration,
        observer: &dyn LoopxProcessObserver,
    ) -> Result<VerifiedLoopxCommand, LoopxCliAdapterError> {
        let mut verified_guard = self.verified.lock().await;
        if let Some(verified) = verified_guard.as_ref() {
            return Ok(verified.clone());
        }

        let candidate = self.select_candidate().await?;
        let version_output = self
            .runner
            .run(
                LoopxCommandPlan::handshake(
                    operation_id,
                    candidate.executable.clone(),
                    ["--version"],
                    startup_deadline,
                    self.config.terminate_grace,
                ),
                cancellation.clone(),
                observer,
            )
            .await?;
        let actual_version = version_output.stdout.trim();
        if actual_version != LOOPX_PINNED_VERSION_OUTPUT {
            return Err(LoopxCliAdapterError::VersionMismatch {
                expected: LOOPX_PINNED_VERSION_OUTPUT.to_string(),
                actual: actual_version.to_string(),
            });
        }

        let schema_output = self
            .runner
            .run(
                LoopxCommandPlan::handshake(
                    operation_id,
                    candidate.executable.clone(),
                    ["--format", "json", "commands"],
                    startup_deadline,
                    self.config.terminate_grace,
                ),
                cancellation,
                observer,
            )
            .await?;
        let schema_payload: Value =
            serde_json::from_str(&schema_output.stdout).map_err(|error| {
                LoopxCliAdapterError::InvalidJson {
                    message: error.to_string(),
                }
            })?;
        let actual_schema = schema_payload
            .get("schema_version")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if schema_payload.get("ok").and_then(Value::as_bool) != Some(true)
            || actual_schema != LOOPX_COMMAND_REFERENCE_SCHEMA
        {
            return Err(LoopxCliAdapterError::SchemaMismatch {
                expected: LOOPX_COMMAND_REFERENCE_SCHEMA.to_string(),
                actual: actual_schema.to_string(),
            });
        }

        let verified = VerifiedLoopxCommand {
            executable: candidate.executable,
            source: candidate.source,
            version: LOOPX_PINNED_VERSION.to_string(),
            bundle_manifest_schema: candidate.bundle_manifest_schema,
            command_reference_schema: LOOPX_COMMAND_REFERENCE_SCHEMA.to_string(),
            sha256: candidate.sha256,
        };
        *verified_guard = Some(verified.clone());
        Ok(verified)
    }

    async fn select_candidate(&self) -> Result<LoopxCandidate, LoopxCliAdapterError> {
        let bundle_dir = self.config.resource_dir.join("loopx");
        let executable = bundle_dir.join(if cfg!(windows) { "loopx.exe" } else { "loopx" });
        let manifest_path = bundle_dir.join("manifest.json");
        let executable_exists = tokio::fs::try_exists(&executable).await.map_err(|error| {
            LoopxCliAdapterError::Manifest {
                message: error.to_string(),
            }
        })?;
        let manifest_exists = tokio::fs::try_exists(&manifest_path)
            .await
            .map_err(|error| LoopxCliAdapterError::Manifest {
                message: error.to_string(),
            })?;

        if executable_exists || manifest_exists {
            if !executable_exists || !manifest_exists {
                return Err(LoopxCliAdapterError::Manifest {
                    message: "bundle must contain both the executable and manifest.json"
                        .to_string(),
                });
            }
            let raw = tokio::fs::read(&manifest_path).await.map_err(|error| {
                LoopxCliAdapterError::Manifest {
                    message: error.to_string(),
                }
            })?;
            let manifest: BundledLoopxManifest =
                serde_json::from_slice(&raw).map_err(|error| LoopxCliAdapterError::Manifest {
                    message: error.to_string(),
                })?;
            verify_manifest(&manifest)?;
            let digest = sha256_file(&executable).await?;
            let expected_digest = manifest
                .sha256
                .strip_prefix("sha256:")
                .unwrap_or(&manifest.sha256);
            if digest != expected_digest {
                return Err(LoopxCliAdapterError::Manifest {
                    message: "bundle executable checksum does not match manifest.json".to_string(),
                });
            }
            return Ok(LoopxCandidate {
                executable,
                source: LoopxCommandSource::PackagedBundle,
                bundle_manifest_schema: Some(manifest.schema_version),
                sha256: Some(digest),
            });
        }

        if self.config.system_fallback == LoopxSystemFallbackPolicy::ExactPinned {
            let located = self
                .locator
                .locate()
                .map_err(|message| LoopxCliAdapterError::Manifest { message })?;
            if let Some(executable) = located {
                return Ok(LoopxCandidate {
                    executable,
                    source: LoopxCommandSource::FixedSystemCommand,
                    bundle_manifest_schema: None,
                    sha256: None,
                });
            }
        }
        Err(LoopxCliAdapterError::Unavailable)
    }

    fn register_operation(
        &self,
        operation_id: &str,
    ) -> Result<(CancellationToken, OperationRegistration), LoopxCliAdapterError> {
        let mut running = self
            .running
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if running.contains_key(operation_id) {
            return Err(LoopxCliAdapterError::Conflict {
                operation_id: operation_id.to_string(),
            });
        }
        let cancellation = CancellationToken::new();
        running.insert(operation_id.to_string(), cancellation.clone());
        Ok((
            cancellation,
            OperationRegistration {
                operation_id: operation_id.to_string(),
                running: self.running.clone(),
            },
        ))
    }
}

struct PortProcessObserver<'a> {
    progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    fallback: &'a dyn LoopxProcessObserver,
    task_id: Option<String>,
    stage: loopx_contract::LoopxCliProgressStage,
}

impl LoopxProcessObserver for PortProcessObserver<'_> {
    fn on_progress(&self, progress: LoopxProcessProgress) {
        self.fallback.on_progress(progress.clone());
        self.progress.report(loopx_contract::LoopxCliProgress {
            operation_id: progress.operation_id,
            task_id: self.task_id.clone(),
            stage: self.stage,
            message: progress.message,
            occurred_at: progress.occurred_at_unix_ms.try_into().unwrap_or(i64::MAX),
        });
    }
}

impl loopx_contract::LoopxCliPort for LoopxCliProcessAdapter {
    fn handshake<'a>(
        &'a self,
        request: loopx_contract::LoopxCliHandshakeRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliManifest> {
        Box::pin(async move {
            validate_operation_id(&request.call.operation_id)?;
            report_port_progress(
                progress,
                &request.call.operation_id,
                None,
                loopx_contract::LoopxCliProgressStage::StartingSidecar,
                "Selecting the managed LoopX executable",
            );
            if request.required_loopx_version != LOOPX_PINNED_VERSION {
                return Err(port_error(
                    loopx_contract::LoopxCliErrorKind::VersionMismatch,
                    &request.call.operation_id,
                    format!(
                        "adapter is pinned to LoopX {LOOPX_PINNED_VERSION}; requested {}",
                        request.required_loopx_version
                    ),
                    false,
                ));
            }
            if request.required_schema_version != loopx_contract::LOOPX_CLI_SCHEMA_VERSION {
                return Err(port_error(
                    loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                    &request.call.operation_id,
                    format!(
                        "adapter schema is {}; requested {}",
                        loopx_contract::LOOPX_CLI_SCHEMA_VERSION,
                        request.required_schema_version
                    ),
                    false,
                ));
            }
            let deadline = effective_deadline(
                request.call.deadline_at,
                self.config.startup_deadline,
                &request.call.operation_id,
            )?;
            let observer = PortProcessObserver {
                progress,
                fallback: self.observer.as_ref(),
                task_id: None,
                stage: loopx_contract::LoopxCliProgressStage::Handshake,
            };
            let (cancellation, _registration) = self
                .register_operation(&request.call.operation_id)
                .map_err(|error| map_port_error(error, &request.call.operation_id))?;
            let verified = self
                .ensure_verified(
                    &request.call.operation_id,
                    cancellation,
                    deadline,
                    &observer,
                )
                .await
                .map_err(|error| map_port_error(error, &request.call.operation_id))?;
            let mut capabilities = vec![
                "issue_fix_workflow_plan_v0".to_string(),
                "goal_bootstrap_v0".to_string(),
                "loopx_turn_plan_v0".to_string(),
                "heartbeat_prompt_v0".to_string(),
                "typed_gate_decision_v0".to_string(),
                "managed_process_tree_v1".to_string(),
            ];
            if self.intake_metadata_configured {
                capabilities.push("intake_metadata_provider_v1".to_string());
            }
            Ok(loopx_contract::LoopxCliManifest {
                adapter_version: env!("CARGO_PKG_VERSION").to_string(),
                loopx_version: verified.version,
                schema_version: loopx_contract::LOOPX_CLI_SCHEMA_VERSION,
                executable: loopx_contract::LoopxCliExecutableIdentity {
                    source: match verified.source {
                        LoopxCommandSource::PackagedBundle => {
                            loopx_contract::LoopxCliSource::Bundled
                        }
                        LoopxCommandSource::FixedSystemCommand => {
                            loopx_contract::LoopxCliSource::System
                        }
                    },
                    identity: match verified.source {
                        LoopxCommandSource::PackagedBundle => {
                            "bitfun-bundled-loopx-v0.5.1".to_string()
                        }
                        LoopxCommandSource::FixedSystemCommand => {
                            "fixed-system-loopx-v0.5.1".to_string()
                        }
                    },
                    path: Some(verified.executable.to_string_lossy().into_owned()),
                    sha256: verified.sha256.map(|digest| format!("sha256:{digest}")),
                },
                capabilities,
            })
        })
    }

    fn resolve_intake<'a>(
        &'a self,
        request: loopx_contract::LoopxCliResolveIntakeRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliResolveIntakeResult> {
        Box::pin(async move {
            validate_operation_id(&request.call.operation_id)?;
            let deadline = effective_deadline(
                request.call.deadline_at,
                self.config.command_deadline,
                &request.call.operation_id,
            )?;
            report_port_progress(
                progress,
                &request.call.operation_id,
                None,
                loopx_contract::LoopxCliProgressStage::ResolvingIntake,
                "Resolving live repository metadata",
            );
            self.intake_metadata.resolve(&request, deadline).await
        })
    }

    fn probe_github_auth<'a>(
        &'a self,
        request: loopx_contract::LoopxGithubAuthProbeRequest,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxGithubAuthProbe> {
        Box::pin(async move {
            validate_operation_id(&request.call.operation_id)?;
            let deadline = effective_deadline(
                request.call.deadline_at,
                self.config.command_deadline,
                &request.call.operation_id,
            )?;
            self.intake_metadata.probe_auth(deadline).await
        })
    }

    fn plan_item<'a>(
        &'a self,
        request: loopx_contract::LoopxCliPlanItemRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliIntakePlan> {
        Box::pin(async move {
            validate_goal_context(&request.context)?;
            validate_github_item(&request.item, &request.context.call.operation_id)?;
            let observer = PortProcessObserver {
                progress,
                fallback: self.observer.as_ref(),
                task_id: Some(request.context.task_id.clone()),
                stage: loopx_contract::LoopxCliProgressStage::PlanningItem,
            };
            let operation_id = &request.context.call.operation_id;
            report_port_progress(
                progress,
                operation_id,
                Some(request.context.task_id.clone()),
                loopx_contract::LoopxCliProgressStage::PlanningItem,
                "Building the pinned LoopX issue-fix plan",
            );
            let deadline = effective_deadline(
                request.context.call.deadline_at,
                self.config.command_deadline,
                operation_id,
            )?;
            let output = self
                .run_json_command(
                    operation_id,
                    Path::new(&request.context.registry_path),
                    Some(Path::new(&request.context.worktree_path)),
                    plan_item_args(&request),
                    deadline,
                    &observer,
                )
                .await
                .map_err(|error| map_port_error(error, operation_id))?;
            require_payload_ok(&output.payload, operation_id)?;
            require_schema(
                &output.payload,
                "issue_fix_workflow_plan_packet_v0",
                operation_id,
            )?;
            let previews = output
                .payload
                .get("ordered_loopx_todo_writeback_preview")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    port_error(
                        loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                        operation_id,
                        "workflow plan did not contain ordered_loopx_todo_writeback_preview",
                        false,
                    )
                })?;
            let mut todos = Vec::with_capacity(previews.len());
            for preview in previews {
                let role = required_json_string(preview, "role", operation_id)?;
                let task_class = required_json_string(preview, "task_class", operation_id)?;
                let text = required_json_string(preview, "text", operation_id)?;
                let action_kind = preview
                    .get("action_kind")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                todos.push(loopx_contract::LoopxCliTodoPlan {
                    role,
                    task_class,
                    action_kind,
                    text,
                });
            }
            if todos.is_empty() {
                return Err(port_error(
                    loopx_contract::LoopxCliErrorKind::Backend,
                    operation_id,
                    "workflow plan produced no writable todos",
                    true,
                ));
            }
            let objective = output
                .payload
                .get("objective")
                .and_then(Value::as_str)
                .or_else(|| {
                    output
                        .payload
                        .pointer("/issue_signal/title")
                        .and_then(Value::as_str)
                })
                .map(str::to_string)
                .unwrap_or_else(|| format!("Fix {}", request.item.canonical_url()));
            Ok(loopx_contract::LoopxCliIntakePlan {
                item: request.item,
                objective,
                todos,
            })
        })
    }

    fn create_goal<'a>(
        &'a self,
        request: loopx_contract::LoopxCliCreateGoalRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliCreateGoalResult> {
        Box::pin(async move {
            validate_goal_context(&request.context)?;
            validate_nonempty(
                "goal_id",
                &request.goal_id,
                &request.context.call.operation_id,
            )?;
            validate_nonempty(
                "agent_id",
                &request.agent_id,
                &request.context.call.operation_id,
            )?;
            if request.intake.todos.is_empty() {
                return Err(port_error(
                    loopx_contract::LoopxCliErrorKind::InvalidInput,
                    &request.context.call.operation_id,
                    "create_goal requires at least one planned todo",
                    false,
                ));
            }
            let operation_id = &request.context.call.operation_id;
            let observer = PortProcessObserver {
                progress,
                fallback: self.observer.as_ref(),
                task_id: Some(request.context.task_id.clone()),
                stage: loopx_contract::LoopxCliProgressStage::CreatingGoal,
            };
            report_port_progress(
                progress,
                operation_id,
                Some(request.context.task_id.clone()),
                loopx_contract::LoopxCliProgressStage::CreatingGoal,
                "Creating one LoopX goal for the selected item",
            );

            let bootstrap =
                run_port_command(self, &request.context, bootstrap_args(&request), &observer)
                    .await?;
            require_payload_ok(&bootstrap.payload, operation_id)?;
            let state_action = bootstrap
                .payload
                .get("state_action")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    port_error(
                        loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                        operation_id,
                        "bootstrap response did not contain state_action",
                        false,
                    )
                })?;
            let created = matches!(state_action, "created" | "replaced");

            let registration = run_port_command(
                self,
                &request.context,
                register_agent_args(&request),
                &observer,
            )
            .await?;
            require_payload_ok(&registration.payload, operation_id)?;

            let existing_todos = run_port_command(
                self,
                &request.context,
                list_todos_args(&request.goal_id),
                &observer,
            )
            .await?;
            require_payload_ok(&existing_todos.payload, operation_id)?;
            let existing_todos = existing_todos
                .payload
                .get("todos")
                .and_then(Value::as_array)
                .cloned()
                .ok_or_else(|| {
                    port_error(
                        loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                        operation_id,
                        "todo list response did not contain todos",
                        false,
                    )
                })?;
            for todo in &request.intake.todos {
                if existing_todos
                    .iter()
                    .any(|existing| todo_matches(existing, todo))
                {
                    continue;
                }
                let result = run_port_command(
                    self,
                    &request.context,
                    add_todo_args(&request, todo)?,
                    &observer,
                )
                .await?;
                require_payload_ok(&result.payload, operation_id)?;
            }

            let inspection = run_port_command(
                self,
                &request.context,
                inspect_goal_args(&request.goal_id, &request.agent_id, None),
                &observer,
            )
            .await?;
            require_payload_ok(&inspection.payload, operation_id)?;
            let durable_revision = extract_durable_revision(&inspection.payload, operation_id)?;
            Ok(loopx_contract::LoopxCliCreateGoalResult {
                goal_id: request.goal_id,
                created,
                durable_revision,
            })
        })
    }

    fn inspect_goal<'a>(
        &'a self,
        request: loopx_contract::LoopxCliInspectGoalRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliGoalSnapshot> {
        Box::pin(async move {
            validate_goal_context(&request.context)?;
            let operation_id = &request.context.call.operation_id;
            let observer = PortProcessObserver {
                progress,
                fallback: self.observer.as_ref(),
                task_id: Some(request.context.task_id.clone()),
                stage: loopx_contract::LoopxCliProgressStage::InspectingGoal,
            };
            report_port_progress(
                progress,
                operation_id,
                Some(request.context.task_id.clone()),
                loopx_contract::LoopxCliProgressStage::InspectingGoal,
                "Inspecting durable LoopX goal state",
            );
            let output = run_port_command(
                self,
                &request.context,
                inspect_goal_args(&request.goal_id, &request.agent_id, None),
                &observer,
            )
            .await?;
            let mut snapshot =
                project_goal_snapshot(&request.goal_id, &output.payload, operation_id)?;
            if snapshot.run_decision == loopx_contract::LoopxCliRunDecision::WaitingForUser {
                let todos = run_port_command(
                    self,
                    &request.context,
                    list_todos_args(&request.goal_id),
                    &observer,
                )
                .await?;
                snapshot.pending_user_gate =
                    Some(project_pending_user_gate(&todos.payload, operation_id)?);
            }
            Ok(snapshot)
        })
    }

    fn build_turn<'a>(
        &'a self,
        request: loopx_contract::LoopxCliBuildTurnRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliBuildTurnResult> {
        Box::pin(async move {
            validate_goal_context(&request.context)?;
            let operation_id = &request.context.call.operation_id;
            let observer = PortProcessObserver {
                progress,
                fallback: self.observer.as_ref(),
                task_id: Some(request.context.task_id.clone()),
                stage: loopx_contract::LoopxCliProgressStage::BuildingTurn,
            };
            report_port_progress(
                progress,
                operation_id,
                Some(request.context.task_id.clone()),
                loopx_contract::LoopxCliProgressStage::BuildingTurn,
                "Building an idempotent external-host LoopX turn",
            );
            let turn_id = stable_turn_id(&request);
            let plan = run_port_command(
                self,
                &request.context,
                inspect_goal_args(&request.goal_id, &request.agent_id, Some(&turn_id)),
                &observer,
            )
            .await?;
            require_payload_ok(&plan.payload, operation_id)?;
            require_schema(&plan.payload, "loopx_turn_plan_v0", operation_id)?;
            let durable_revision = extract_durable_revision(&plan.payload, operation_id)?;
            if durable_revision != request.expected_durable_revision {
                return Err(port_error(
                    loopx_contract::LoopxCliErrorKind::Conflict,
                    operation_id,
                    "durable LoopX state changed before the turn was built",
                    true,
                ));
            }
            let settlement_token = plan
                .payload
                .pointer("/transaction/turn_key")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    port_error(
                        loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                        operation_id,
                        "turn plan did not contain transaction.turn_key",
                        false,
                    )
                })?
                .to_string();
            let prompt_output = run_port_command(
                self,
                &request.context,
                heartbeat_prompt_args(&request.goal_id, &request.agent_id),
                &observer,
            )
            .await?;
            require_payload_ok(&prompt_output.payload, operation_id)?;
            let raw_prompt = prompt_output
                .payload
                .get("task_body")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    port_error(
                        loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                        operation_id,
                        "heartbeat-prompt did not produce task_body",
                        false,
                    )
                })?;
            let verified = self.verified.lock().await.clone().ok_or_else(|| {
                port_error(
                    loopx_contract::LoopxCliErrorKind::Backend,
                    operation_id,
                    "LoopX executable identity disappeared after handshake",
                    true,
                )
            })?;
            let command = agent_shell_command(&verified.executable);
            let prompt = format!(
                "LoopX control-plane binding for this turn:\n- Repository: {}\n- Registry: {}\n- Exact command: {}\n- Turn id: {}\nUse this exact command and registry for every LoopX progress, todo, refresh, and settlement operation. Do not probe PATH and do not use a global registry. The host will accept success only when LoopX exposes a matching durable validation/settlement receipt.\n\n{}",
                request.context.worktree_path,
                request.context.registry_path,
                command,
                turn_id,
                raw_prompt
                    .replace("${LOOPX_TURN:?}", &turn_id)
                    .replace("loopx ", &format!("{command} ")),
            );
            Ok(loopx_contract::LoopxCliBuildTurnResult {
                goal_id: request.goal_id,
                turn_id,
                prompt,
                settlement_token,
                durable_revision,
                scheduler_hint_ms: scheduler_hint_ms(&plan.payload),
                deadline_at: request.context.call.deadline_at,
            })
        })
    }

    fn answer_gate<'a>(
        &'a self,
        request: loopx_contract::LoopxCliAnswerGateRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliAnswerGateResult> {
        Box::pin(async move {
            validate_goal_context(&request.context)?;
            let operation_id = &request.context.call.operation_id;
            let observer = PortProcessObserver {
                progress,
                fallback: self.observer.as_ref(),
                task_id: Some(request.context.task_id.clone()),
                stage: loopx_contract::LoopxCliProgressStage::AnsweringGate,
            };
            report_port_progress(
                progress,
                operation_id,
                Some(request.context.task_id.clone()),
                loopx_contract::LoopxCliProgressStage::AnsweringGate,
                "Applying the typed LoopX gate decision",
            );
            let decision = run_port_command(
                self,
                &request.context,
                answer_gate_args(&request)?,
                &observer,
            )
            .await?;
            require_payload_ok(&decision.payload, operation_id)?;
            let inspection = run_port_command(
                self,
                &request.context,
                inspect_goal_args(&request.goal_id, &request.agent_id, None),
                &observer,
            )
            .await?;
            require_payload_ok(&inspection.payload, operation_id)?;
            Ok(loopx_contract::LoopxCliAnswerGateResult {
                goal_id: request.goal_id,
                gate_id: request.gate_id,
                applied: true,
                durable_revision: extract_durable_revision(&inspection.payload, operation_id)?,
            })
        })
    }

    fn settle_turn<'a>(
        &'a self,
        request: loopx_contract::LoopxCliSettleTurnRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliSettleTurnResult> {
        Box::pin(async move {
            validate_goal_context(&request.context)?;
            validate_nonempty(
                "agent_id",
                &request.agent_id,
                &request.context.call.operation_id,
            )?;
            report_port_progress(
                progress,
                &request.context.call.operation_id,
                Some(request.context.task_id.clone()),
                loopx_contract::LoopxCliProgressStage::SettlingTurn,
                "Verifying the durable LoopX settlement receipt",
            );
            let operation_id = &request.context.call.operation_id;
            let observer = PortProcessObserver {
                progress,
                fallback: self.observer.as_ref(),
                task_id: Some(request.context.task_id.clone()),
                stage: loopx_contract::LoopxCliProgressStage::SettlingTurn,
            };
            let inspection = run_port_command(
                self,
                &request.context,
                inspect_goal_args(&request.goal_id, &request.agent_id, Some(&request.turn_id)),
                &observer,
            )
            .await?;
            require_payload_ok(&inspection.payload, operation_id)?;
            require_schema(&inspection.payload, "loopx_turn_plan_v0", operation_id)?;
            let snapshot =
                project_goal_snapshot(&request.goal_id, &inspection.payload, operation_id)?;
            let receipt =
                matching_settlement_receipt(&inspection.payload, &request.settlement_token);
            let Some(receipt) = receipt else {
                let status = if matches!(
                    request.agent_status,
                    loopx_contract::LoopxAgentTurnStatus::Failed
                        | loopx_contract::LoopxAgentTurnStatus::Cancelled
                        | loopx_contract::LoopxAgentTurnStatus::Interrupted
                ) {
                    loopx_contract::LoopxCliSettlementStatus::RetryRequired
                } else {
                    loopx_contract::LoopxCliSettlementStatus::NoDurableProgress
                };
                return Ok(loopx_contract::LoopxCliSettleTurnResult {
                    goal_id: request.goal_id,
                    turn_id: request.turn_id,
                    status,
                    before_revision: request.expected_durable_revision,
                    after_revision: snapshot.durable_revision,
                    scheduler_hint_ms: snapshot.scheduler_hint_ms,
                    ..loopx_contract::LoopxCliSettleTurnResult::default()
                });
            };
            let receipt_id = receipt
                .get("receipt_id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    port_error(
                        loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                        operation_id,
                        "matching LoopX settlement receipt has no receipt_id",
                        false,
                    )
                })?
                .to_string();
            let validation_succeeded = receipt
                .get("validation_succeeded")
                .and_then(Value::as_bool)
                .or_else(|| {
                    receipt
                        .pointer("/validation/succeeded")
                        .and_then(Value::as_bool)
                })
                .unwrap_or(false);
            if !validation_succeeded {
                return Ok(loopx_contract::LoopxCliSettleTurnResult {
                    goal_id: request.goal_id,
                    turn_id: request.turn_id,
                    receipt_id,
                    status: loopx_contract::LoopxCliSettlementStatus::NoDurableProgress,
                    before_revision: request.expected_durable_revision,
                    after_revision: snapshot.durable_revision,
                    validation_succeeded: false,
                    scheduler_hint_ms: snapshot.scheduler_hint_ms,
                });
            }
            Ok(loopx_contract::LoopxCliSettleTurnResult {
                goal_id: request.goal_id,
                turn_id: request.turn_id,
                receipt_id,
                status: if snapshot.state == loopx_contract::LoopxCliGoalState::Completed {
                    loopx_contract::LoopxCliSettlementStatus::GoalCompleted
                } else {
                    loopx_contract::LoopxCliSettlementStatus::AlreadySettled
                },
                before_revision: request.expected_durable_revision,
                after_revision: snapshot.durable_revision,
                validation_succeeded: true,
                scheduler_hint_ms: snapshot.scheduler_hint_ms,
            })
        })
    }

    fn cancel<'a>(
        &'a self,
        request: loopx_contract::LoopxCliCancelRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliCancelResult> {
        Box::pin(async move {
            validate_operation_id(&request.call.operation_id)?;
            validate_nonempty(
                "target_operation_id",
                &request.target_operation_id,
                &request.call.operation_id,
            )?;
            report_port_progress(
                progress,
                &request.call.operation_id,
                None,
                loopx_contract::LoopxCliProgressStage::Cancelling,
                "Cancelling the managed LoopX process tree",
            );
            let cancelled = self.cancel_operation(&request.target_operation_id);
            Ok(loopx_contract::LoopxCliCancelResult {
                operation_id: request.call.operation_id,
                target_operation_id: request.target_operation_id,
                cancelled,
            })
        })
    }
}

fn matching_settlement_receipt<'a>(payload: &'a Value, turn_key: &str) -> Option<&'a Value> {
    payload
        .pointer("/transaction/receipts")
        .and_then(Value::as_array)?
        .iter()
        .find(|receipt| {
            receipt.get("turn_key").and_then(Value::as_str) == Some(turn_key)
                || receipt.get("settlement_token").and_then(Value::as_str) == Some(turn_key)
        })
}

fn agent_shell_command(path: &Path) -> String {
    let display = path.to_string_lossy().replace('"', "\\\"");
    if cfg!(windows) {
        format!("& \"{display}\"")
    } else {
        format!("'{}'", display.replace('\'', "'\\''"))
    }
}

async fn run_port_command(
    adapter: &LoopxCliProcessAdapter,
    context: &loopx_contract::LoopxCliGoalContext,
    args: Vec<OsString>,
    observer: &dyn LoopxProcessObserver,
) -> loopx_contract::LoopxCliResult<LoopxJsonOutput> {
    let operation_id = &context.call.operation_id;
    let deadline = effective_deadline(
        context.call.deadline_at,
        adapter.config.command_deadline,
        operation_id,
    )?;
    adapter
        .run_json_command(
            operation_id,
            Path::new(&context.registry_path),
            Some(Path::new(&context.worktree_path)),
            args,
            deadline,
            observer,
        )
        .await
        .map_err(|error| map_port_error(error, operation_id))
}

fn plan_item_args(request: &loopx_contract::LoopxCliPlanItemRequest) -> Vec<OsString> {
    let kind = match request.item.kind {
        loopx_contract::LoopxItemKind::Issue => "issue",
        loopx_contract::LoopxItemKind::PullRequest => "pull_request",
    };
    let metadata = serde_json::json!({
        "number": request.item.number,
        "state": "open",
        "title": request.title,
        "labels": [],
        "kind": kind,
    })
    .to_string();
    [
        "issue-fix".to_string(),
        "workflow-plan".to_string(),
        "--url".to_string(),
        request.item.canonical_url(),
        "--repo-path".to_string(),
        request.context.worktree_path.clone(),
        "--metadata-json".to_string(),
        metadata,
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn bootstrap_args(request: &loopx_contract::LoopxCliCreateGoalRequest) -> Vec<OsString> {
    let mut args = vec![
        "bootstrap".to_string(),
        "--project".to_string(),
        request.context.worktree_path.clone(),
        "--goal-id".to_string(),
        request.goal_id.clone(),
        "--objective".to_string(),
        request.intake.objective.clone(),
        "--adapter-kind".to_string(),
        "read_only_project_map_v0".to_string(),
        "--adapter-status".to_string(),
        "connected-read-only".to_string(),
        "--no-onboarding-scan".to_string(),
        "--codex-app-heartbeat".to_string(),
        "no".to_string(),
    ];
    if request
        .granted_scopes
        .contains(&loopx_contract::LoopxPermissionScope::WorkspaceWrite)
    {
        args.extend(["--write-scope".to_string(), "write".to_string()]);
    }
    args.into_iter().map(OsString::from).collect()
}

fn register_agent_args(request: &loopx_contract::LoopxCliCreateGoalRequest) -> Vec<OsString> {
    [
        "register-agent",
        "--goal-id",
        request.goal_id.as_str(),
        "--agent-id",
        request.agent_id.as_str(),
        "--execute",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn list_todos_args(goal_id: &str) -> Vec<OsString> {
    ["todo", "list", "--goal-id", goal_id]
        .into_iter()
        .map(OsString::from)
        .collect()
}

fn todo_matches(existing: &Value, planned: &loopx_contract::LoopxCliTodoPlan) -> bool {
    existing.get("role").and_then(Value::as_str) == Some(planned.role.as_str())
        && existing.get("task_class").and_then(Value::as_str) == Some(planned.task_class.as_str())
        && existing.get("text").and_then(Value::as_str) == Some(planned.text.as_str())
        && existing.get("action_kind").and_then(Value::as_str) == planned.action_kind.as_deref()
}

fn add_todo_args(
    request: &loopx_contract::LoopxCliCreateGoalRequest,
    todo: &loopx_contract::LoopxCliTodoPlan,
) -> loopx_contract::LoopxCliResult<Vec<OsString>> {
    let operation_id = &request.context.call.operation_id;
    if !matches!(todo.role.as_str(), "agent" | "user") {
        return Err(port_error(
            loopx_contract::LoopxCliErrorKind::InvalidInput,
            operation_id,
            "todo role must be agent or user",
            false,
        ));
    }
    if !matches!(
        todo.task_class.as_str(),
        "advancement_task" | "continuous_monitor" | "user_gate" | "user_action" | "blocker"
    ) {
        return Err(port_error(
            loopx_contract::LoopxCliErrorKind::InvalidInput,
            operation_id,
            "todo task_class is not supported by LoopX v0.5.1",
            false,
        ));
    }
    validate_nonempty("todo.text", &todo.text, operation_id)?;
    let repository = &request.intake.item.repository;
    let mut args = vec![
        "todo".to_string(),
        "add".to_string(),
        "--goal-id".to_string(),
        request.goal_id.clone(),
        "--role".to_string(),
        todo.role.clone(),
        "--task-class".to_string(),
        todo.task_class.clone(),
        "--text".to_string(),
        todo.text.clone(),
    ];
    if let Some(action_kind) = &todo.action_kind {
        if !is_public_token(action_kind) {
            return Err(port_error(
                loopx_contract::LoopxCliErrorKind::InvalidInput,
                operation_id,
                "todo action_kind must be a bounded public-safe token",
                false,
            ));
        }
        args.extend(["--action-kind".to_string(), action_kind.clone()]);
    }
    if todo.role == "agent" {
        args.extend(["--claimed-by".to_string(), request.agent_id.clone()]);
        args.extend([
            "--task-repository".to_string(),
            format!(
                "git:{}/{}/{}",
                repository.host, repository.owner, repository.repository
            )
            .to_lowercase(),
        ]);
    } else {
        args.extend(["--agent-id".to_string(), request.agent_id.clone()]);
    }
    Ok(args.into_iter().map(OsString::from).collect())
}

fn inspect_goal_args(goal_id: &str, agent_id: &str, turn_id: Option<&str>) -> Vec<OsString> {
    let mut args = vec![
        "turn".to_string(),
        "plan".to_string(),
        "--goal-id".to_string(),
        goal_id.to_string(),
        "--agent-id".to_string(),
        agent_id.to_string(),
        "--host".to_string(),
        "generic-cli".to_string(),
        "--execution-mode".to_string(),
        "isolated-headless".to_string(),
        "--scheduler-owner".to_string(),
        "outer_controller".to_string(),
        "--include-transaction-detail".to_string(),
    ];
    if let Some(turn_id) = turn_id {
        args.extend(["--turn-instance-id".to_string(), turn_id.to_string()]);
    }
    args.into_iter().map(OsString::from).collect()
}

fn heartbeat_prompt_args(goal_id: &str, agent_id: &str) -> Vec<OsString> {
    [
        "heartbeat-prompt",
        "--goal-id",
        goal_id,
        "--agent-id",
        agent_id,
        "--host-surface",
        "generic_cli",
        "--scheduler-owner",
        "outer_controller",
        "--execution-mode",
        "isolated_headless",
        "--compact",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn answer_gate_args(
    request: &loopx_contract::LoopxCliAnswerGateRequest,
) -> loopx_contract::LoopxCliResult<Vec<OsString>> {
    let operation_id = &request.context.call.operation_id;
    validate_nonempty("gate_id", &request.gate_id, operation_id)?;
    let decision = match request.decision {
        loopx_contract::LoopxCliGateDecision::Approve => "approve",
        loopx_contract::LoopxCliGateDecision::Reject => "reject",
    };
    let mut args = vec![
        "todo".to_string(),
        "complete".to_string(),
        "--goal-id".to_string(),
        request.goal_id.clone(),
        "--todo-id".to_string(),
        request.gate_id.clone(),
        "--decision-outcome".to_string(),
        decision.to_string(),
        "--agent-id".to_string(),
        request.agent_id.clone(),
    ];
    if let Some(note) = &request.note {
        if note.len() > 500 {
            return Err(port_error(
                loopx_contract::LoopxCliErrorKind::InvalidInput,
                operation_id,
                "gate note exceeds the 500-byte adapter limit",
                false,
            ));
        }
        args.extend(["--note".to_string(), note.clone()]);
    }
    Ok(args.into_iter().map(OsString::from).collect())
}

fn project_goal_snapshot(
    goal_id: &str,
    payload: &Value,
    operation_id: &str,
) -> loopx_contract::LoopxCliResult<loopx_contract::LoopxCliGoalSnapshot> {
    require_payload_ok(payload, operation_id)?;
    require_schema(payload, "loopx_turn_plan_v0", operation_id)?;
    let envelope = payload.get("turn_envelope").ok_or_else(|| {
        port_error(
            loopx_contract::LoopxCliErrorKind::SchemaMismatch,
            operation_id,
            "turn plan did not contain turn_envelope",
            false,
        )
    })?;
    let should_run = envelope.get("should_run").and_then(Value::as_bool) == Some(true);
    let user_action_required = envelope
        .pointer("/user/action_required")
        .and_then(Value::as_bool)
        .or_else(|| envelope.get("action_required").and_then(Value::as_bool))
        == Some(true);
    let state_text = envelope
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let effective_action = envelope
        .get("effective_action")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let run_decision = if should_run {
        loopx_contract::LoopxCliRunDecision::RunNow
    } else if user_action_required {
        loopx_contract::LoopxCliRunDecision::WaitingForUser
    } else if matches!(state_text, "completed" | "complete" | "closed") {
        loopx_contract::LoopxCliRunDecision::Complete
    } else if matches!(state_text, "failed" | "error") {
        loopx_contract::LoopxCliRunDecision::Failed
    } else {
        loopx_contract::LoopxCliRunDecision::Wait
    };
    let state = match run_decision {
        loopx_contract::LoopxCliRunDecision::RunNow => loopx_contract::LoopxCliGoalState::Active,
        loopx_contract::LoopxCliRunDecision::WaitingForUser => {
            loopx_contract::LoopxCliGoalState::WaitingForUser
        }
        loopx_contract::LoopxCliRunDecision::Complete => {
            loopx_contract::LoopxCliGoalState::Completed
        }
        loopx_contract::LoopxCliRunDecision::Failed => loopx_contract::LoopxCliGoalState::Failed,
        loopx_contract::LoopxCliRunDecision::Wait => {
            if effective_action == "archived" {
                loopx_contract::LoopxCliGoalState::Archived
            } else {
                loopx_contract::LoopxCliGoalState::Active
            }
        }
    };
    Ok(loopx_contract::LoopxCliGoalSnapshot {
        goal_id: goal_id.to_string(),
        state,
        durable_revision: extract_durable_revision(payload, operation_id)?,
        run_decision,
        scheduler_hint_ms: scheduler_hint_ms(payload),
        open_todo_count: envelope
            .get("open_count")
            .and_then(Value::as_u64)
            .unwrap_or_default()
            .try_into()
            .unwrap_or(u32::MAX),
        waiting_user_todo_count: envelope
            .pointer("/user/open_count")
            .and_then(Value::as_u64)
            .unwrap_or_default()
            .try_into()
            .unwrap_or(u32::MAX),
        pending_user_gate: None,
        last_turn_id: payload
            .get("turn_instance_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        settlement_receipt_ids: payload
            .pointer("/transaction/receipts")
            .and_then(Value::as_array)
            .map(|receipts| {
                receipts
                    .iter()
                    .filter_map(|receipt| {
                        receipt
                            .get("receipt_id")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn project_pending_user_gate(
    payload: &Value,
    operation_id: &str,
) -> loopx_contract::LoopxCliResult<loopx_contract::LoopxCliUserGate> {
    require_payload_ok(payload, operation_id)?;
    let todos = payload
        .get("todos")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            port_error(
                loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                operation_id,
                "todo list response did not contain todos",
                false,
            )
        })?;
    let gate = todos
        .iter()
        .find(|todo| {
            let status = todo
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default();
            todo.get("role").and_then(Value::as_str) == Some("user")
                && todo.get("task_class").and_then(Value::as_str) == Some("user_gate")
                && todo.get("done").and_then(Value::as_bool) != Some(true)
                && !matches!(
                    status,
                    "completed" | "closed" | "done" | "archived" | "cancelled"
                )
        })
        .ok_or_else(|| {
            port_error(
                loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                operation_id,
                "LoopX requested a user decision without an open typed user gate",
                false,
            )
        })?;
    Ok(loopx_contract::LoopxCliUserGate {
        gate_id: required_json_string(gate, "todo_id", operation_id)?,
        message: truncate_message(&required_json_string(gate, "text", operation_id)?),
        action_kind: gate
            .get("action_kind")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string),
    })
}

fn extract_durable_revision(
    payload: &Value,
    operation_id: &str,
) -> loopx_contract::LoopxCliResult<String> {
    [
        "/turn_envelope/action_signature/source_decision_hash",
        "/turn_envelope/action_signature/source_hash",
        "/transaction/turn_key",
    ]
    .into_iter()
    .find_map(|pointer| {
        payload
            .pointer(pointer)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
    })
    .map(str::to_string)
    .ok_or_else(|| {
        port_error(
            loopx_contract::LoopxCliErrorKind::SchemaMismatch,
            operation_id,
            "LoopX turn plan did not expose a durable revision identity",
            false,
        )
    })
}

fn stable_turn_id(request: &loopx_contract::LoopxCliBuildTurnRequest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(request.context.task_id.as_bytes());
    hasher.update([0]);
    hasher.update(request.context.generation.to_le_bytes());
    hasher.update([0]);
    hasher.update(request.goal_id.as_bytes());
    hasher.update([0]);
    hasher.update(request.expected_durable_revision.as_bytes());
    let digest = hex::encode(hasher.finalize());
    format!("bitfun-{}", &digest[..32])
}

fn scheduler_hint_ms(payload: &Value) -> Option<u64> {
    [
        "/turn_envelope/scheduler/hint_ms",
        "/turn_envelope/scheduler/next_poll_ms",
        "/scheduler_hint_ms",
    ]
    .into_iter()
    .find_map(|pointer| payload.pointer(pointer).and_then(Value::as_u64))
}

fn require_payload_ok(payload: &Value, operation_id: &str) -> loopx_contract::LoopxCliResult<()> {
    if payload.get("ok").and_then(Value::as_bool) == Some(false) {
        let message = payload
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("LoopX rejected the operation");
        return Err(port_error(
            loopx_contract::LoopxCliErrorKind::Backend,
            operation_id,
            truncate_message(message),
            false,
        ));
    }
    Ok(())
}

fn require_schema(
    payload: &Value,
    expected: &str,
    operation_id: &str,
) -> loopx_contract::LoopxCliResult<()> {
    let actual = payload
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if actual != expected {
        return Err(port_error(
            loopx_contract::LoopxCliErrorKind::SchemaMismatch,
            operation_id,
            format!("expected response schema {expected}, got {actual}"),
            false,
        ));
    }
    Ok(())
}

fn required_json_string(
    payload: &Value,
    field: &str,
    operation_id: &str,
) -> loopx_contract::LoopxCliResult<String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            port_error(
                loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                operation_id,
                format!("LoopX response field {field} is missing or invalid"),
                false,
            )
        })
}

fn validate_goal_context(
    context: &loopx_contract::LoopxCliGoalContext,
) -> loopx_contract::LoopxCliResult<()> {
    let operation_id = &context.call.operation_id;
    validate_operation_id(operation_id)?;
    validate_nonempty("task_id", &context.task_id, operation_id)?;
    validate_nonempty("worktree_path", &context.worktree_path, operation_id)?;
    validate_nonempty("registry_path", &context.registry_path, operation_id)?;
    if !Path::new(&context.worktree_path).is_absolute()
        || !Path::new(&context.registry_path).is_absolute()
    {
        return Err(port_error(
            loopx_contract::LoopxCliErrorKind::InvalidInput,
            operation_id,
            "LoopX local worktree and registry paths must be absolute",
            false,
        ));
    }
    Ok(())
}

fn validate_github_item(
    item: &loopx_contract::LoopxIssueKey,
    operation_id: &str,
) -> loopx_contract::LoopxCliResult<()> {
    if !item.repository.host.eq_ignore_ascii_case("github.com")
        || item.repository.owner.is_empty()
        || item.repository.repository.is_empty()
        || item.number == 0
    {
        return Err(port_error(
            loopx_contract::LoopxCliErrorKind::InvalidInput,
            operation_id,
            "LoopX v0.5.1 issue-fix planning requires a canonical GitHub item",
            false,
        ));
    }
    Ok(())
}

fn validate_operation_id(operation_id: &str) -> loopx_contract::LoopxCliResult<()> {
    validate_nonempty("operation_id", operation_id, operation_id)
}

fn validate_nonempty(
    field: &str,
    value: &str,
    operation_id: &str,
) -> loopx_contract::LoopxCliResult<()> {
    if value.trim().is_empty() {
        return Err(port_error(
            loopx_contract::LoopxCliErrorKind::InvalidInput,
            operation_id,
            format!("{field} is required"),
            false,
        ));
    }
    Ok(())
}

fn effective_deadline(
    deadline_at: Option<i64>,
    configured: Duration,
    operation_id: &str,
) -> loopx_contract::LoopxCliResult<Duration> {
    let Some(deadline_at) = deadline_at else {
        return Ok(configured);
    };
    let remaining_ms = deadline_at.saturating_sub(now_unix_ms());
    if remaining_ms <= 0 {
        return Err(port_error(
            loopx_contract::LoopxCliErrorKind::Timeout,
            operation_id,
            "LoopX operation deadline has already expired",
            true,
        ));
    }
    Ok(configured.min(Duration::from_millis(
        remaining_ms.try_into().unwrap_or(u64::MAX),
    )))
}

fn map_port_error(
    error: LoopxCliAdapterError,
    operation_id: &str,
) -> loopx_contract::LoopxCliError {
    let (kind, retryable) = match &error {
        LoopxCliAdapterError::Unavailable => (loopx_contract::LoopxCliErrorKind::NotFound, true),
        LoopxCliAdapterError::Manifest { .. } => (loopx_contract::LoopxCliErrorKind::Io, false),
        LoopxCliAdapterError::VersionMismatch { .. } => {
            (loopx_contract::LoopxCliErrorKind::VersionMismatch, false)
        }
        LoopxCliAdapterError::SchemaMismatch { .. } => {
            (loopx_contract::LoopxCliErrorKind::SchemaMismatch, false)
        }
        LoopxCliAdapterError::Conflict { .. } => {
            (loopx_contract::LoopxCliErrorKind::Conflict, true)
        }
        LoopxCliAdapterError::InvalidJson { .. } => {
            (loopx_contract::LoopxCliErrorKind::Backend, false)
        }
        LoopxCliAdapterError::Process(LoopxProcessError::Timeout { .. }) => {
            (loopx_contract::LoopxCliErrorKind::Timeout, true)
        }
        LoopxCliAdapterError::Process(LoopxProcessError::Cancelled { .. }) => {
            (loopx_contract::LoopxCliErrorKind::Cancelled, true)
        }
        LoopxCliAdapterError::Process(LoopxProcessError::Io { .. }) => {
            (loopx_contract::LoopxCliErrorKind::Io, true)
        }
        LoopxCliAdapterError::Process(_) => (loopx_contract::LoopxCliErrorKind::Process, true),
    };
    let message = match &error {
        LoopxCliAdapterError::Process(LoopxProcessError::Exited {
            code,
            stdout_tail,
            stderr_tail,
        }) if !stderr_tail.is_empty() || !stdout_tail.is_empty() => {
            let details = if stderr_tail.is_empty() {
                stdout_tail
            } else {
                stderr_tail
            };
            format!(
                "LoopX process exited with status {code:?}: {}",
                process_error_detail(details)
            )
        }
        _ => error.to_string(),
    };
    port_error(kind, operation_id, message, retryable)
}

fn port_error(
    kind: loopx_contract::LoopxCliErrorKind,
    operation_id: &str,
    message: impl Into<String>,
    retryable: bool,
) -> loopx_contract::LoopxCliError {
    loopx_contract::LoopxCliError::new(kind, message)
        .for_operation(operation_id)
        .retryable(retryable)
}

fn report_port_progress(
    progress: &dyn loopx_contract::LoopxCliProgressSink,
    operation_id: &str,
    task_id: Option<String>,
    stage: loopx_contract::LoopxCliProgressStage,
    message: &str,
) {
    progress.report(loopx_contract::LoopxCliProgress {
        operation_id: operation_id.to_string(),
        task_id,
        stage,
        message: message.to_string(),
        occurred_at: now_unix_ms(),
    });
}

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn truncate_message(message: &str) -> String {
    message.chars().take(500).collect()
}

fn output_tail(bytes: &[u8]) -> Vec<String> {
    let lines = String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .take(20)
        .map(str::to_string)
        .collect::<Vec<_>>();
    lines.into_iter().rev().collect()
}

fn process_error_detail(lines: &[String]) -> String {
    let detail = lines
        .iter()
        .rev()
        .find(|line| line.contains("\"error\""))
        .cloned()
        .unwrap_or_else(|| lines.join(" | "));
    truncate_message(&detail)
}

fn is_public_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

#[derive(Debug)]
struct LoopxCandidate {
    executable: PathBuf,
    source: LoopxCommandSource,
    bundle_manifest_schema: Option<u32>,
    sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BundledLoopxManifest {
    schema_version: u32,
    name: String,
    version: String,
    sha256: String,
}

fn verify_manifest(manifest: &BundledLoopxManifest) -> Result<(), LoopxCliAdapterError> {
    if manifest.schema_version != LOOPX_BUNDLE_MANIFEST_SCHEMA {
        return Err(LoopxCliAdapterError::SchemaMismatch {
            expected: LOOPX_BUNDLE_MANIFEST_SCHEMA.to_string(),
            actual: manifest.schema_version.to_string(),
        });
    }
    if manifest.name != "loopx" {
        return Err(LoopxCliAdapterError::Manifest {
            message: format!("expected bundle name loopx, got {}", manifest.name),
        });
    }
    if manifest.version != LOOPX_PINNED_VERSION_TAG {
        return Err(LoopxCliAdapterError::VersionMismatch {
            expected: LOOPX_PINNED_VERSION_TAG.to_string(),
            actual: manifest.version.clone(),
        });
    }
    let digest = manifest.sha256.strip_prefix("sha256:").unwrap_or_default();
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(LoopxCliAdapterError::Manifest {
            message: "manifest sha256 must contain a 64-digit sha256 digest".to_string(),
        });
    }
    Ok(())
}

async fn sha256_file(path: &Path) -> Result<String, LoopxCliAdapterError> {
    let mut file =
        tokio::fs::File::open(path)
            .await
            .map_err(|error| LoopxCliAdapterError::Manifest {
                message: error.to_string(),
            })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read =
            file.read(&mut buffer)
                .await
                .map_err(|error| LoopxCliAdapterError::Manifest {
                    message: error.to_string(),
                })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

struct OperationRegistration {
    operation_id: String,
    running: Arc<StdMutex<HashMap<String, CancellationToken>>>,
}

impl Drop for OperationRegistration {
    fn drop(&mut self) {
        self.running
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .remove(&self.operation_id);
    }
}

struct StdoutCapture {
    bytes: Vec<u8>,
    exceeded_limit: bool,
}

async fn capture_stdout(mut reader: impl AsyncRead + Unpin) -> StdoutCapture {
    let mut bytes = Vec::new();
    let mut exceeded_limit = false;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = match reader.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        let remaining = MAX_STDOUT_BYTES.saturating_sub(bytes.len());
        if read > remaining {
            bytes.extend_from_slice(&buffer[..remaining]);
            exceeded_limit = true;
        } else {
            bytes.extend_from_slice(&buffer[..read]);
        }
    }
    StdoutCapture {
        bytes,
        exceeded_limit,
    }
}

async fn capture_stderr(
    mut reader: impl AsyncRead + Unpin,
    line_sender: mpsc::Sender<String>,
) -> Vec<String> {
    let mut pending = Vec::new();
    let mut tail = VecDeque::new();
    let mut tail_bytes = 0_usize;
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = match reader.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        pending.extend_from_slice(&buffer[..read]);
        while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
            let line = pending.drain(..=newline).collect::<Vec<_>>();
            record_stderr_line(&line, &line_sender, &mut tail, &mut tail_bytes);
        }
        if pending.len() > MAX_PROGRESS_LINE_BYTES * 2 {
            let line = pending.drain(..MAX_PROGRESS_LINE_BYTES).collect::<Vec<_>>();
            record_stderr_line(&line, &line_sender, &mut tail, &mut tail_bytes);
        }
    }
    if !pending.is_empty() {
        record_stderr_line(&pending, &line_sender, &mut tail, &mut tail_bytes);
    }
    tail.into_iter().collect()
}

fn record_stderr_line(
    raw: &[u8],
    line_sender: &mpsc::Sender<String>,
    tail: &mut VecDeque<String>,
    tail_bytes: &mut usize,
) {
    let raw = raw
        .strip_suffix(b"\n")
        .unwrap_or(raw)
        .strip_suffix(b"\r")
        .unwrap_or(raw);
    if raw.is_empty() {
        return;
    }
    let line = String::from_utf8_lossy(&raw[..raw.len().min(MAX_PROGRESS_LINE_BYTES)]).into_owned();
    let _ = line_sender.try_send(line.clone());
    *tail_bytes += line.len();
    tail.push_back(line);
    while *tail_bytes > MAX_STDERR_TAIL_BYTES {
        if let Some(removed) = tail.pop_front() {
            *tail_bytes = tail_bytes.saturating_sub(removed.len());
        } else {
            break;
        }
    }
}

async fn drain_stdout_task(
    task: &mut tokio::task::JoinHandle<StdoutCapture>,
) -> Result<StdoutCapture, LoopxProcessError> {
    match tokio::time::timeout(PIPE_DRAIN_DEADLINE, &mut *task).await {
        Ok(Ok(capture)) => Ok(capture),
        Ok(Err(error)) => Err(LoopxProcessError::Io {
            message: error.to_string(),
        }),
        Err(_) => {
            task.abort();
            Err(LoopxProcessError::Io {
                message: "LoopX stdout pipe stayed open after process exit".to_string(),
            })
        }
    }
}

async fn drain_stderr_task(task: &mut tokio::task::JoinHandle<Vec<String>>) -> Vec<String> {
    match tokio::time::timeout(PIPE_DRAIN_DEADLINE, &mut *task).await {
        Ok(Ok(tail)) => tail,
        Ok(Err(_)) => Vec::new(),
        Err(_) => {
            task.abort();
            Vec::new()
        }
    }
}

fn emit_progress(
    observer: &dyn LoopxProcessObserver,
    operation_id: &str,
    stage: LoopxProgressStage,
    message: &str,
) {
    observer.on_progress(LoopxProcessProgress {
        operation_id: operation_id.to_string(),
        stage,
        message: message.to_string(),
        occurred_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
    });
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}
