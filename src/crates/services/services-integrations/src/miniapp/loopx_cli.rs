//! Managed LoopX CLI integration for the built-in LoopX MiniApp.
//!
//! The product-facing adapter below never accepts an executable or an argv
//! prefix from callers. It selects the packaged binary first and only permits
//! the fixed `loopx` system command when that fallback was explicitly enabled.

use async_trait::async_trait;
use bitfun_product_domains::miniapp::loopx as loopx_contract;
use bitfun_services_core::process_tree::ProcessTreeChild;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
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
pub const LOOPX_SOURCE_REPOSITORY: &str = "https://github.com/huangruiteng/loopx.git";
pub const LOOPX_PINNED_SOURCE_COMMIT: &str = "1bb42f4cb3e329dcb71c64654228f951098cead1";
pub const OPEN_VIKING_PINNED_VERSION: &str = "0.4.9";
pub const OPEN_VIKING_PINNED_VERSION_TAG: &str = "v0.4.9";
pub const OPEN_VIKING_SOURCE_REPOSITORY: &str = "https://github.com/volcengine/OpenViking.git";
pub const OPEN_VIKING_PINNED_SOURCE_COMMIT: &str = "4f0bd86f32c5a98ed78e7ba04adb5708c0bdb89a";
pub const LOOPX_BUNDLE_MANIFEST_SCHEMA: u32 = 1;
pub const LOOPX_COMMAND_REFERENCE_SCHEMA: &str = "loopx_command_reference_v0";

const MAX_STDOUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_STDERR_TAIL_BYTES: usize = 32 * 1024;
const MAX_PROGRESS_LINE_BYTES: usize = 4 * 1024;
const PIPE_DRAIN_DEADLINE: Duration = Duration::from_millis(250);
const SETTLEMENT_HISTORY_LIMIT: &str = "100";
/// Bound for the open-agent-todo projection injected into turn prompts.
const MAX_OPEN_AGENT_TODO_SUMMARIES: usize = 8;
/// Upper bound for the raw workflow-plan packet persisted into the worktree.
/// LoopX intake packets are compact; the cap only guards against a pathological
/// provider payload ballooning the worktree file.
const MAX_INTAKE_PLAN_PACKET_BYTES: usize = 512 * 1024;
const MANAGED_SOURCE_MANIFEST: &str = ".bitfun-managed-source.json";
const MANAGED_SOURCE_MANIFEST_SCHEMA: u32 = 1;
const MANAGED_OPEN_VIKING_MANIFEST: &str = ".bitfun-managed-openviking.json";
const MANAGED_OPEN_VIKING_MANIFEST_SCHEMA: u32 = 1;
const OPEN_VIKING_EXTENSION_ID: &str = "openviking-semantic-preference";
const PYTHON_LOOPX_ENTRYPOINT: &str = "import os,sys; sys.path.insert(0, os.environ['BITFUN_LOOPX_SOURCE']); from loopx.entrypoint import main; raise SystemExit(main())";
const AGENT_CLI_USAGE_RULES: &str = "LoopX CLI syntax contract:\n- Put the global `--format json` option immediately after the exact executable and before the subcommand. Do not append it after a subcommand unless that subcommand's help explicitly lists a local format option.\n- Use `--agent-id` only on subcommands whose `--help` output lists it. Do not move that option between a parent command and its child action.\n- Do not run `--help`, `commands`, or re-read loopx source code preemptively to discover syntax. The exact executable and registry prefix is bound above; the typed command forms below are verified for this pinned LoopX version and are the first resort. Only inspect a subcommand's `--help` when the CLI rejected a specific argument, and do so at most once per subcommand; do not repeat the rejected form.\n- Verified typed command forms (replace <placeholders> with the values bound above):\n  - `quota should-run --goal-id <goal> --todo-id <todo> --agent-id <agent> --runtime-profile outer_controller --turn-instance-id \"<turn>\"` (the BitFun host normally executes this guard before the agent starts, with the same todo and turn binding; only re-run it exactly as printed in the task body preflight when a settlement command reports a missing or mismatched heartbeat receipt, and never with a different turn id)\n  - `todo list --goal-id <goal>`\n  - `todo claim --goal-id <goal> --todo-id <todo> --claimed-by <agent> --agent-id <agent>`\n  - `todo add --goal-id <goal> --role agent --task-class <advancement_task|continuous_monitor|blocker> --text \"<text>\" --claimed-by <agent> --task-repository git:<host>/<owner>/<repo>` (keep the text under 200 characters: the todo list rides inside LoopX's turn-envelope compaction budget; put details in files or the active state, not in todo text); a `continuous_monitor` additionally requires `--cadence <cadence>` plus exactly one terminal condition: `--expires-at <ts>`, `--resume-when <condition>`, or `--watch-only`\n  - `todo add --goal-id <goal> --role user --task-class user_gate --text \"<message>\" --action-kind <kind> --agent-id <agent> --blocks-agent <agent>`\n  - `todo complete --goal-id <goal> --todo-id <todo> --turn-instance-id \"<turn>\"` plus exactly one closing path: `--next-agent-todo \"<text>\"` to wire the runnable successor atomically, or `--no-follow-up` when the completed todo intentionally ends this work path; any other completion flags must be copied verbatim from the task body\n  - `refresh-state --goal-id <goal> --agent-id <agent>`; a turn-scoped accountable writeback additionally requires `--todo-id <todo> --turn-instance-id \"<turn>\" --delivery-outcome <outcome_progress|primary_goal_outcome> --delivery-batch-scale <scale>`; optional `--classification`, `--progress-*`, `--recommended-action`, and `--vision-*` flags must be copied verbatim from the task body or goal vision instructions, never invented\n  - `history --goal-id <goal> --limit 100`\n  - `issue-fix pr-lifecycle --url <github-pr-url> --goal-id <goal> --claimed-by <agent> --execute-transition` — run it once after the fix PR exists, and again when CI, review, or merge state materially changes; LoopX projects the grouped lifecycle monitor, material transitions, and terminal no-follow-up closeout itself, so do not hand-create per-PR monitor todos\n  - issue-fix subcommands (`workflow-plan`, `feasibility`, `pr-lifecycle`): run the exact command printed by the task body, by a previous workflow-plan output, or by the persisted intake plan packet; do not guess additional flags.\n- `goal-lifecycle` and all `quota spend`/`void` commands are host-owned; do not run them.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopxSystemFallbackPolicy {
    Disabled,
    ExactPinned,
}

#[derive(Debug, Clone)]
pub struct LoopxCliAdapterConfig {
    pub resource_dir: PathBuf,
    pub managed_source_dir: Option<PathBuf>,
    pub managed_open_viking_dir: Option<PathBuf>,
    pub system_fallback: LoopxSystemFallbackPolicy,
    pub startup_deadline: Duration,
    pub command_deadline: Duration,
    pub install_deadline: Duration,
    pub terminate_grace: Duration,
}

impl LoopxCliAdapterConfig {
    pub fn packaged(resource_dir: impl Into<PathBuf>) -> Self {
        Self {
            resource_dir: resource_dir.into(),
            managed_source_dir: None,
            managed_open_viking_dir: None,
            system_fallback: LoopxSystemFallbackPolicy::Disabled,
            startup_deadline: Duration::from_secs(60),
            command_deadline: Duration::from_secs(180),
            install_deadline: Duration::from_secs(10 * 60),
            terminate_grace: Duration::from_secs(2),
        }
    }

    pub fn with_managed_source_dir(mut self, managed_source_dir: impl Into<PathBuf>) -> Self {
        self.managed_source_dir = Some(managed_source_dir.into());
        self
    }

    pub fn with_managed_open_viking_dir(
        mut self,
        managed_open_viking_dir: impl Into<PathBuf>,
    ) -> Self {
        self.managed_open_viking_dir = Some(managed_open_viking_dir.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopxCommandSource {
    PackagedBundle,
    ManagedSource,
    FixedSystemCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedLoopxCommand {
    pub executable: PathBuf,
    pub prefix_args: Vec<OsString>,
    pub environment: BTreeMap<OsString, OsString>,
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
        prefix_args: &[OsString],
        environment: &BTreeMap<OsString, OsString>,
        args: impl IntoIterator<Item = impl Into<OsString>>,
        deadline: Duration,
        terminate_grace: Duration,
    ) -> Self {
        let mut command_args = prefix_args.to_vec();
        command_args.extend(args.into_iter().map(Into::into));
        Self {
            operation_id: operation_id.into(),
            executable,
            args: command_args,
            current_dir: None,
            environment: environment.clone(),
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

pub trait LoopxPythonLocator: Send + Sync {
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

#[derive(Debug, Default)]
pub struct SystemLoopxPythonLocator;

impl LoopxPythonLocator for SystemLoopxPythonLocator {
    fn locate(&self) -> Result<Option<PathBuf>, String> {
        let candidates = if cfg!(windows) {
            ["python", "python3"]
        } else {
            ["python3", "python"]
        };
        for candidate in candidates {
            match which::which(candidate) {
                Ok(path) => return Ok(Some(path)),
                Err(which::Error::CannotFindBinaryPath) => continue,
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(None)
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LoopxCliAdapterError {
    #[error("compatible LoopX runtime is not available")]
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
    python_locator: Arc<dyn LoopxPythonLocator>,
    observer: Arc<dyn LoopxProcessObserver>,
    intake_metadata: Arc<dyn LoopxIntakeMetadataProvider>,
    intake_metadata_configured: bool,
    install_lock: Mutex<()>,
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
            Arc::new(SystemLoopxPythonLocator),
            Arc::new(NoopLoopxProcessObserver),
        )
    }

    pub fn with_dependencies(
        config: LoopxCliAdapterConfig,
        runner: Arc<dyn LoopxProcessRunner>,
        locator: Arc<dyn LoopxFixedCommandLocator>,
        python_locator: Arc<dyn LoopxPythonLocator>,
        observer: Arc<dyn LoopxProcessObserver>,
    ) -> Self {
        Self {
            config,
            runner,
            locator,
            python_locator,
            observer,
            intake_metadata: Arc::new(UnsupportedLoopxIntakeMetadataProvider),
            intake_metadata_configured: false,
            install_lock: Mutex::new(()),
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
        let mut args = verified.prefix_args.clone();
        args.extend([
            OsString::from("--format"),
            OsString::from("json"),
            OsString::from("--registry"),
            registry_path.as_os_str().to_owned(),
        ]);
        args.extend(command_args);
        let output = self
            .runner
            .run(
                LoopxCommandPlan {
                    operation_id: operation_id.to_string(),
                    executable: verified.executable,
                    args,
                    current_dir: current_dir.map(Path::to_path_buf),
                    environment: verified.environment,
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

    async fn run_global_json_command(
        &self,
        operation_id: &str,
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
        let mut args = verified.prefix_args.clone();
        args.extend([OsString::from("--format"), OsString::from("json")]);
        args.extend(command_args);
        let output = self
            .runner
            .run(
                LoopxCommandPlan {
                    operation_id: operation_id.to_string(),
                    executable: verified.executable,
                    args,
                    current_dir: None,
                    environment: verified.environment,
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

        let mut candidate = self.select_candidate().await?;
        if let Err(error) = self
            .append_managed_open_viking_path(&mut candidate.environment)
            .await
        {
            log::warn!("Managed OpenViking PATH injection skipped: {error}");
        }
        if let Some(source_dir) = candidate.managed_source_dir.as_ref() {
            let git = which::which("git").map_err(|error| LoopxCliAdapterError::Manifest {
                message: format!("Git is required to verify managed LoopX source: {error}"),
            })?;
            let status = self
                .runner
                .run(
                    LoopxCommandPlan {
                        operation_id: operation_id.to_string(),
                        executable: git,
                        args: vec![
                            OsString::from("-C"),
                            source_dir.as_os_str().to_owned(),
                            OsString::from("status"),
                            OsString::from("--porcelain"),
                            OsString::from("--untracked-files=all"),
                        ],
                        current_dir: None,
                        environment: BTreeMap::new(),
                        deadline: startup_deadline,
                        terminate_grace: self.config.terminate_grace,
                    },
                    cancellation.clone(),
                    observer,
                )
                .await?;
            if !status.stdout.trim().is_empty() {
                return Err(LoopxCliAdapterError::Manifest {
                    message: "managed LoopX source was modified; reinstall it from GitHub"
                        .to_string(),
                });
            }
        }
        let version_output = self
            .runner
            .run(
                LoopxCommandPlan::handshake(
                    operation_id,
                    candidate.executable.clone(),
                    &candidate.prefix_args,
                    &candidate.environment,
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
                    &candidate.prefix_args,
                    &candidate.environment,
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
            prefix_args: candidate.prefix_args,
            environment: candidate.environment,
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
                prefix_args: Vec::new(),
                environment: BTreeMap::new(),
                managed_source_dir: None,
                source: LoopxCommandSource::PackagedBundle,
                bundle_manifest_schema: Some(manifest.schema_version),
                sha256: Some(digest),
            });
        }

        if let Some(candidate) = self.managed_source_candidate().await? {
            return Ok(candidate);
        }

        if self.config.system_fallback == LoopxSystemFallbackPolicy::ExactPinned {
            let located = self
                .locator
                .locate()
                .map_err(|message| LoopxCliAdapterError::Manifest { message })?;
            if let Some(executable) = located {
                return Ok(LoopxCandidate {
                    executable,
                    prefix_args: Vec::new(),
                    environment: BTreeMap::new(),
                    managed_source_dir: None,
                    source: LoopxCommandSource::FixedSystemCommand,
                    bundle_manifest_schema: None,
                    sha256: None,
                });
            }
        }
        Err(LoopxCliAdapterError::Unavailable)
    }

    async fn managed_source_candidate(
        &self,
    ) -> Result<Option<LoopxCandidate>, LoopxCliAdapterError> {
        let Some(source_dir) = self.config.managed_source_dir.as_ref() else {
            return Ok(None);
        };
        let manifest_path = source_dir.join(".git").join(MANAGED_SOURCE_MANIFEST);
        if !tokio::fs::try_exists(&manifest_path)
            .await
            .map_err(|error| LoopxCliAdapterError::Manifest {
                message: error.to_string(),
            })?
        {
            return Ok(None);
        }
        let raw = tokio::fs::read(&manifest_path).await.map_err(|error| {
            LoopxCliAdapterError::Manifest {
                message: error.to_string(),
            }
        })?;
        let manifest: ManagedLoopxSourceManifest =
            serde_json::from_slice(&raw).map_err(|error| LoopxCliAdapterError::Manifest {
                message: format!("invalid managed LoopX source manifest: {error}"),
            })?;
        verify_managed_source_manifest(&manifest)?;
        let head = tokio::fs::read_to_string(source_dir.join(".git").join("HEAD"))
            .await
            .map_err(|error| LoopxCliAdapterError::Manifest {
                message: format!("failed to read managed LoopX source revision: {error}"),
            })?;
        if head.trim() != LOOPX_PINNED_SOURCE_COMMIT {
            return Err(LoopxCliAdapterError::Manifest {
                message: format!(
                    "managed LoopX source revision mismatch: expected {LOOPX_PINNED_SOURCE_COMMIT}, got {}",
                    head.trim()
                ),
            });
        }
        for required in ["pyproject.toml", "loopx/entrypoint.py"] {
            if !tokio::fs::try_exists(source_dir.join(required))
                .await
                .map_err(|error| LoopxCliAdapterError::Manifest {
                    message: error.to_string(),
                })?
            {
                return Err(LoopxCliAdapterError::Manifest {
                    message: format!("managed LoopX source is missing {required}"),
                });
            }
        }
        let python = self
            .python_locator
            .locate()
            .map_err(|message| LoopxCliAdapterError::Manifest { message })?
            .ok_or_else(|| LoopxCliAdapterError::Manifest {
                message: "Python 3.11 or newer is required to run managed LoopX source".to_string(),
            })?;
        let mut environment = BTreeMap::new();
        environment.insert(
            OsString::from("BITFUN_LOOPX_SOURCE"),
            source_dir.as_os_str().to_owned(),
        );
        Ok(Some(LoopxCandidate {
            executable: python,
            prefix_args: vec![
                OsString::from("-I"),
                OsString::from("-c"),
                OsString::from(PYTHON_LOOPX_ENTRYPOINT),
            ],
            environment,
            managed_source_dir: Some(source_dir.clone()),
            source: LoopxCommandSource::ManagedSource,
            bundle_manifest_schema: None,
            sha256: None,
        }))
    }

    async fn install_managed_source_checkout(
        &self,
        operation_id: &str,
        staging_dir: &Path,
        cancellation: CancellationToken,
        progress: &dyn loopx_contract::LoopxCliProgressSink,
    ) -> Result<(), LoopxCliAdapterError> {
        let python = self
            .python_locator
            .locate()
            .map_err(|message| LoopxCliAdapterError::Manifest { message })?
            .ok_or_else(|| LoopxCliAdapterError::Manifest {
                message: "Python 3.11 or newer is required to install LoopX from source"
                    .to_string(),
            })?;
        report_port_progress(
            progress,
            operation_id,
            None,
            loopx_contract::LoopxCliProgressStage::InstallingRuntime,
            "Checking the Python runtime for managed LoopX source",
        );
        let python_version = self
            .runner
            .run(
                LoopxCommandPlan {
                    operation_id: operation_id.to_string(),
                    executable: python,
                    args: vec![OsString::from("--version")],
                    current_dir: None,
                    environment: BTreeMap::new(),
                    deadline: self.config.startup_deadline,
                    terminate_grace: self.config.terminate_grace,
                },
                cancellation.clone(),
                self.observer.as_ref(),
            )
            .await?;
        log::info!(
            "LoopX install Python check completed: operation_id={operation_id}, duration_ms={}",
            python_version.elapsed.as_millis()
        );
        let version_text = if python_version.stdout.trim().is_empty() {
            python_version.stderr_tail.join(" ")
        } else {
            python_version.stdout
        };
        if !python_version_supported(&version_text) {
            return Err(LoopxCliAdapterError::Manifest {
                message: format!(
                    "Python 3.11 or newer is required to install LoopX from source; found {}",
                    version_text.trim()
                ),
            });
        }
        let git = which::which("git").map_err(|error| LoopxCliAdapterError::Manifest {
            message: format!("Git is required to download LoopX source: {error}"),
        })?;

        report_port_progress(
            progress,
            operation_id,
            None,
            loopx_contract::LoopxCliProgressStage::InstallingRuntime,
            "Downloading LoopX v0.5.1 source from GitHub",
        );
        let clone_output = self
            .runner
            .run(
                LoopxCommandPlan {
                    operation_id: operation_id.to_string(),
                    executable: git.clone(),
                    args: [
                        "clone",
                        "--depth",
                        "1",
                        "--filter=blob:none",
                        "--sparse",
                        "--branch",
                        LOOPX_PINNED_VERSION_TAG,
                        "--single-branch",
                        LOOPX_SOURCE_REPOSITORY,
                    ]
                    .into_iter()
                    .map(OsString::from)
                    .chain(std::iter::once(staging_dir.as_os_str().to_owned()))
                    .collect(),
                    current_dir: None,
                    environment: BTreeMap::new(),
                    deadline: self.config.install_deadline,
                    terminate_grace: self.config.terminate_grace,
                },
                cancellation.clone(),
                self.observer.as_ref(),
            )
            .await?;
        log::info!(
            "LoopX install GitHub clone completed: operation_id={operation_id}, duration_ms={}",
            clone_output.elapsed.as_millis()
        );

        report_port_progress(
            progress,
            operation_id,
            None,
            loopx_contract::LoopxCliProgressStage::InstallingRuntime,
            "Preparing only the LoopX runtime source files",
        );
        let sparse_output = self
            .runner
            .run(
                LoopxCommandPlan {
                    operation_id: operation_id.to_string(),
                    executable: git.clone(),
                    args: [
                        OsString::from("-C"),
                        staging_dir.as_os_str().to_owned(),
                        OsString::from("sparse-checkout"),
                        OsString::from("set"),
                        OsString::from("--no-cone"),
                        OsString::from("/loopx/"),
                        OsString::from("/pyproject.toml"),
                        OsString::from("/LICENSE"),
                        OsString::from("/NOTICE"),
                        OsString::from("/LICENSE-MIT"),
                        OsString::from("/TRADEMARKS.md"),
                    ]
                    .into_iter()
                    .collect(),
                    current_dir: None,
                    environment: BTreeMap::new(),
                    deadline: self.config.install_deadline,
                    terminate_grace: self.config.terminate_grace,
                },
                cancellation.clone(),
                self.observer.as_ref(),
            )
            .await?;
        log::info!(
            "LoopX install sparse checkout completed: operation_id={operation_id}, duration_ms={}",
            sparse_output.elapsed.as_millis()
        );

        report_port_progress(
            progress,
            operation_id,
            None,
            loopx_contract::LoopxCliProgressStage::InstallingRuntime,
            "Verifying the pinned LoopX source revision",
        );
        let revision = self
            .runner
            .run(
                LoopxCommandPlan {
                    operation_id: operation_id.to_string(),
                    executable: git,
                    args: vec![
                        OsString::from("-C"),
                        staging_dir.as_os_str().to_owned(),
                        OsString::from("rev-parse"),
                        OsString::from("HEAD"),
                    ],
                    current_dir: None,
                    environment: BTreeMap::new(),
                    deadline: self.config.startup_deadline,
                    terminate_grace: self.config.terminate_grace,
                },
                cancellation,
                self.observer.as_ref(),
            )
            .await?;
        log::info!(
            "LoopX install revision check completed: operation_id={operation_id}, duration_ms={}",
            revision.elapsed.as_millis()
        );
        if revision.stdout.trim() != LOOPX_PINNED_SOURCE_COMMIT {
            return Err(LoopxCliAdapterError::Manifest {
                message: format!(
                    "downloaded LoopX source revision mismatch: expected {LOOPX_PINNED_SOURCE_COMMIT}, got {}",
                    revision.stdout.trim()
                ),
            });
        }
        for required in [
            "pyproject.toml",
            "loopx/entrypoint.py",
            "LICENSE",
            "NOTICE",
            "LICENSE-MIT",
            "TRADEMARKS.md",
        ] {
            if !tokio::fs::try_exists(staging_dir.join(required))
                .await
                .map_err(|error| LoopxCliAdapterError::Manifest {
                    message: error.to_string(),
                })?
            {
                return Err(LoopxCliAdapterError::Manifest {
                    message: format!("downloaded LoopX source is missing {required}"),
                });
            }
        }
        let manifest = ManagedLoopxSourceManifest {
            schema_version: MANAGED_SOURCE_MANIFEST_SCHEMA,
            source_repository: LOOPX_SOURCE_REPOSITORY.to_string(),
            source_tag: LOOPX_PINNED_VERSION_TAG.to_string(),
            source_commit: LOOPX_PINNED_SOURCE_COMMIT.to_string(),
            loopx_version: LOOPX_PINNED_VERSION.to_string(),
        };
        let raw = serde_json::to_vec_pretty(&manifest).map_err(|error| {
            LoopxCliAdapterError::Manifest {
                message: error.to_string(),
            }
        })?;
        tokio::fs::write(staging_dir.join(".git").join(MANAGED_SOURCE_MANIFEST), raw)
            .await
            .map_err(|error| LoopxCliAdapterError::Manifest {
                message: error.to_string(),
            })?;
        Ok(())
    }

    async fn managed_open_viking_command(
        &self,
    ) -> Result<Option<OpenVikingCommand>, LoopxCliAdapterError> {
        let Some(root) = self.config.managed_open_viking_dir.as_ref() else {
            return Ok(None);
        };
        let manifest_path = root.join(MANAGED_OPEN_VIKING_MANIFEST);
        if !tokio::fs::try_exists(&manifest_path)
            .await
            .map_err(|error| LoopxCliAdapterError::Manifest {
                message: error.to_string(),
            })?
        {
            return Ok(None);
        }
        let raw = tokio::fs::read(&manifest_path).await.map_err(|error| {
            LoopxCliAdapterError::Manifest {
                message: format!("failed to read managed OpenViking manifest: {error}"),
            }
        })?;
        let manifest: ManagedOpenVikingManifest =
            serde_json::from_slice(&raw).map_err(|error| LoopxCliAdapterError::Manifest {
                message: format!("invalid managed OpenViking manifest: {error}"),
            })?;
        verify_managed_open_viking_manifest(&manifest)?;
        let executable = open_viking_binary(root);
        if !tokio::fs::try_exists(&executable).await.map_err(|error| {
            LoopxCliAdapterError::Manifest {
                message: error.to_string(),
            }
        })? {
            return Err(LoopxCliAdapterError::Manifest {
                message: "managed OpenViking CLI binary is missing".to_string(),
            });
        }
        if !tokio::fs::try_exists(loopx_extension_entrypoint(root))
            .await
            .map_err(|error| LoopxCliAdapterError::Manifest {
                message: error.to_string(),
            })?
        {
            return Err(LoopxCliAdapterError::Manifest {
                message: "managed LoopX OpenViking extension entrypoint is missing".to_string(),
            });
        }
        let digest = sha256_file(&executable).await?;
        if digest != manifest.binary_sha256 {
            return Err(LoopxCliAdapterError::Manifest {
                message: "managed OpenViking CLI checksum does not match its manifest".to_string(),
            });
        }
        Ok(Some(OpenVikingCommand {
            executable,
            version: Some(manifest.open_viking_version),
            managed: true,
        }))
    }

    async fn select_open_viking_command(
        &self,
    ) -> Result<Option<OpenVikingCommand>, LoopxCliAdapterError> {
        if let Some(command) = self.managed_open_viking_command().await? {
            return Ok(Some(command));
        }
        match which::which("ov") {
            Ok(executable) => Ok(Some(OpenVikingCommand {
                executable,
                version: None,
                managed: false,
            })),
            Err(which::Error::CannotFindBinaryPath) => Ok(None),
            Err(error) => Err(LoopxCliAdapterError::Manifest {
                message: format!("failed to locate OpenViking CLI: {error}"),
            }),
        }
    }

    async fn append_managed_open_viking_path(
        &self,
        environment: &mut BTreeMap<OsString, OsString>,
    ) -> Result<(), LoopxCliAdapterError> {
        let Some(command) = self.managed_open_viking_command().await? else {
            return Ok(());
        };
        let root = self
            .config
            .managed_open_viking_dir
            .as_ref()
            .expect("managed OpenViking root exists for a managed command");
        let mut paths = vec![loopx_extension_bin_dir(root)];
        if let Some(bin_dir) = command.executable.parent() {
            paths.push(bin_dir.to_path_buf());
        }
        if let Some(existing) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&existing));
        }
        let joined =
            std::env::join_paths(paths).map_err(|error| LoopxCliAdapterError::Manifest {
                message: format!("failed to prepare managed OpenViking PATH: {error}"),
            })?;
        environment.insert(OsString::from("PATH"), joined);
        Ok(())
    }

    async fn install_open_viking_cli(
        &self,
        operation_id: &str,
        staging_dir: &Path,
        cancellation: CancellationToken,
        progress: &dyn loopx_contract::LoopxCliProgressSink,
    ) -> Result<(), LoopxCliAdapterError> {
        let cargo = which::which("cargo").map_err(|error| LoopxCliAdapterError::Manifest {
            message: format!("Cargo is required to build OpenViking from GitHub source: {error}"),
        })?;
        report_port_progress(
            progress,
            operation_id,
            None,
            loopx_contract::LoopxCliProgressStage::InstallingRuntime,
            "Building OpenViking CLI v0.4.9 from the official GitHub source",
        );
        let mut environment = BTreeMap::new();
        environment.insert(
            OsString::from("OPENVIKING_VERSION"),
            OsString::from(OPEN_VIKING_PINNED_VERSION),
        );
        if let Some(parent) = staging_dir.parent() {
            environment.insert(
                OsString::from("CARGO_TARGET_DIR"),
                parent
                    .join(format!(
                        "openviking-cargo-target-{OPEN_VIKING_PINNED_VERSION}"
                    ))
                    .into_os_string(),
            );
        }
        let build = self
            .runner
            .run(
                LoopxCommandPlan {
                    operation_id: operation_id.to_string(),
                    executable: cargo,
                    args: [
                        "install",
                        "--git",
                        OPEN_VIKING_SOURCE_REPOSITORY,
                        "--rev",
                        OPEN_VIKING_PINNED_SOURCE_COMMIT,
                        "--locked",
                        "--root",
                    ]
                    .into_iter()
                    .map(OsString::from)
                    .chain(std::iter::once(staging_dir.as_os_str().to_owned()))
                    .chain(["--bin", "ov", "ov_cli"].into_iter().map(OsString::from))
                    .collect(),
                    current_dir: None,
                    environment,
                    deadline: self.config.install_deadline,
                    terminate_grace: self.config.terminate_grace,
                },
                cancellation.clone(),
                self.observer.as_ref(),
            )
            .await?;
        log::info!(
            "OpenViking GitHub source build completed: operation_id={operation_id}, duration_ms={}",
            build.elapsed.as_millis()
        );
        let executable = open_viking_binary(staging_dir);
        if !tokio::fs::try_exists(&executable).await.map_err(|error| {
            LoopxCliAdapterError::Manifest {
                message: error.to_string(),
            }
        })? {
            return Err(LoopxCliAdapterError::Manifest {
                message: "OpenViking source build did not produce the ov CLI".to_string(),
            });
        }
        let version_plan = || LoopxCommandPlan {
            operation_id: operation_id.to_string(),
            executable: executable.clone(),
            args: vec![OsString::from("--version")],
            current_dir: None,
            environment: BTreeMap::new(),
            deadline: self.config.startup_deadline,
            terminate_grace: self.config.terminate_grace,
        };
        let version = match self
            .runner
            .run(version_plan(), cancellation.clone(), self.observer.as_ref())
            .await
        {
            Ok(output) => output,
            Err(error) if open_viking_requires_display_language(&error) => {
                report_port_progress(
                    progress,
                    operation_id,
                    None,
                    loopx_contract::LoopxCliProgressStage::InstallingRuntime,
                    "Initializing the OpenViking CLI display language",
                );
                self.runner
                    .run(
                        LoopxCommandPlan {
                            operation_id: operation_id.to_string(),
                            executable: executable.clone(),
                            args: ["language", "en"].into_iter().map(OsString::from).collect(),
                            current_dir: None,
                            environment: BTreeMap::new(),
                            deadline: self.config.startup_deadline,
                            terminate_grace: self.config.terminate_grace,
                        },
                        cancellation.clone(),
                        self.observer.as_ref(),
                    )
                    .await?;
                self.runner
                    .run(version_plan(), cancellation, self.observer.as_ref())
                    .await?
            }
            Err(error) => return Err(error.into()),
        };
        let actual = parse_open_viking_version(&version.stdout).ok_or_else(|| {
            LoopxCliAdapterError::VersionMismatch {
                expected: OPEN_VIKING_PINNED_VERSION.to_string(),
                actual: version.stdout.trim().to_string(),
            }
        })?;
        if actual != OPEN_VIKING_PINNED_VERSION {
            return Err(LoopxCliAdapterError::VersionMismatch {
                expected: OPEN_VIKING_PINNED_VERSION.to_string(),
                actual,
            });
        }
        Ok(())
    }

    async fn install_open_viking_extension_entrypoint(
        &self,
        operation_id: &str,
        root: &Path,
        cancellation: CancellationToken,
        progress: &dyn loopx_contract::LoopxCliProgressSink,
    ) -> Result<(), LoopxCliAdapterError> {
        let python = self
            .python_locator
            .locate()
            .map_err(|message| LoopxCliAdapterError::Manifest { message })?
            .ok_or_else(|| LoopxCliAdapterError::Manifest {
                message:
                    "Python 3.11 or newer is required to install the LoopX OpenViking extension"
                        .to_string(),
            })?;
        let venv = loopx_extension_venv(root);
        report_port_progress(
            progress,
            operation_id,
            None,
            loopx_contract::LoopxCliProgressStage::InstallingRuntime,
            "Preparing the LoopX OpenViking extension entrypoint",
        );
        let python_version = self
            .runner
            .run(
                LoopxCommandPlan {
                    operation_id: operation_id.to_string(),
                    executable: python.clone(),
                    args: vec![OsString::from("--version")],
                    current_dir: None,
                    environment: BTreeMap::new(),
                    deadline: self.config.startup_deadline,
                    terminate_grace: self.config.terminate_grace,
                },
                cancellation.clone(),
                self.observer.as_ref(),
            )
            .await?;
        let version_text = if python_version.stdout.trim().is_empty() {
            python_version.stderr_tail.join(" ")
        } else {
            python_version.stdout
        };
        if !python_version_supported(&version_text) {
            return Err(LoopxCliAdapterError::Manifest {
                message: format!(
                    "Python 3.11 or newer is required for the LoopX OpenViking extension; found {}",
                    version_text.trim()
                ),
            });
        }
        self.runner
            .run(
                LoopxCommandPlan {
                    operation_id: operation_id.to_string(),
                    executable: python,
                    args: vec![
                        OsString::from("-m"),
                        OsString::from("venv"),
                        venv.as_os_str().to_owned(),
                    ],
                    current_dir: None,
                    environment: BTreeMap::new(),
                    deadline: self.config.command_deadline,
                    terminate_grace: self.config.terminate_grace,
                },
                cancellation.clone(),
                self.observer.as_ref(),
            )
            .await?;
        let source = OsString::from(format!(
            "git+{}@{}#egg=loopx",
            LOOPX_SOURCE_REPOSITORY, LOOPX_PINNED_SOURCE_COMMIT
        ));
        let entrypoint_install = self
            .runner
            .run(
                LoopxCommandPlan {
                    operation_id: operation_id.to_string(),
                    executable: loopx_extension_python(root),
                    args: vec![
                        OsString::from("-m"),
                        OsString::from("pip"),
                        OsString::from("install"),
                        OsString::from("--disable-pip-version-check"),
                        OsString::from("--no-input"),
                        OsString::from("--no-deps"),
                        OsString::from("--editable"),
                        source,
                    ],
                    current_dir: None,
                    environment: BTreeMap::new(),
                    deadline: self.config.install_deadline,
                    terminate_grace: self.config.terminate_grace,
                },
                cancellation,
                self.observer.as_ref(),
            )
            .await;
        if let Err(error) = entrypoint_install {
            log::warn!(
                "LoopX OpenViking extension entrypoint install failed: operation_id={operation_id}, error={error}"
            );
            return Err(LoopxCliAdapterError::Manifest {
                message: "failed to prepare the LoopX OpenViking extension entrypoint".to_string(),
            });
        }
        let entrypoint = loopx_extension_entrypoint(root);
        if !tokio::fs::try_exists(&entrypoint).await.map_err(|error| {
            LoopxCliAdapterError::Manifest {
                message: error.to_string(),
            }
        })? {
            return Err(LoopxCliAdapterError::Manifest {
                message: "LoopX OpenViking extension entrypoint is missing".to_string(),
            });
        }
        Ok(())
    }

    async fn open_viking_service_ready(
        &self,
        operation_id: &str,
        command: &OpenVikingCommand,
    ) -> Result<bool, LoopxCliAdapterError> {
        let (cancellation, _registration) = self.register_operation(operation_id)?;
        let output = self
            .runner
            .run(
                LoopxCommandPlan {
                    operation_id: operation_id.to_string(),
                    executable: command.executable.clone(),
                    args: ["status", "-o", "json"]
                        .into_iter()
                        .map(OsString::from)
                        .collect(),
                    current_dir: None,
                    environment: BTreeMap::new(),
                    deadline: self.config.startup_deadline,
                    terminate_grace: self.config.terminate_grace,
                },
                cancellation,
                self.observer.as_ref(),
            )
            .await;
        let Ok(output) = output else {
            return Ok(false);
        };
        let payload: Value = match serde_json::from_str(&output.stdout) {
            Ok(payload) => payload,
            Err(_) => return Ok(false),
        };
        Ok(payload.get("ok").and_then(Value::as_bool) == Some(true))
    }

    async fn ensure_open_viking_extension(
        &self,
        operation_id: &str,
    ) -> Result<bool, LoopxCliAdapterError> {
        let listed = self
            .run_global_json_command(
                &format!("{operation_id}-list"),
                ["extension", "list"]
                    .into_iter()
                    .map(OsString::from)
                    .collect(),
                self.config.command_deadline,
                self.observer.as_ref(),
            )
            .await?;
        let extension = listed
            .payload
            .get("extensions")
            .and_then(Value::as_array)
            .and_then(|extensions| {
                extensions.iter().find(|extension| {
                    extension.get("id").and_then(Value::as_str) == Some(OPEN_VIKING_EXTENSION_ID)
                })
            });
        if extension
            .and_then(|item| item.get("enabled"))
            .and_then(Value::as_bool)
            == Some(true)
        {
            return Ok(true);
        }
        let args = if extension.is_some() {
            vec![
                OsString::from("extension"),
                OsString::from("enable"),
                OsString::from(OPEN_VIKING_EXTENSION_ID),
                OsString::from("--execute"),
            ]
        } else {
            vec![
                OsString::from("extension"),
                OsString::from("install"),
                OsString::from("--bundled"),
                OsString::from(OPEN_VIKING_EXTENSION_ID),
                OsString::from("--execute"),
            ]
        };
        let result = self
            .run_global_json_command(
                &format!("{operation_id}-activate"),
                args,
                self.config.command_deadline,
                self.observer.as_ref(),
            )
            .await?;
        Ok(
            result.payload.get("ok").and_then(Value::as_bool) == Some(true)
                && result.payload.get("enabled").and_then(Value::as_bool) == Some(true),
        )
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
    fn install_managed_source<'a>(
        &'a self,
        request: loopx_contract::LoopxCliInstallManagedSourceRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliInstallManagedSourceResult>
    {
        Box::pin(async move {
            let operation_id = &request.call.operation_id;
            let received_at = Instant::now();
            log::info!("LoopX managed source install received: operation_id={operation_id}");
            validate_operation_id(operation_id)?;
            let lock_started_at = Instant::now();
            let _install = self.install_lock.lock().await;
            log::info!(
                "LoopX managed source install lock acquired: operation_id={operation_id}, wait_ms={}",
                lock_started_at.elapsed().as_millis()
            );
            let target_dir = self.config.managed_source_dir.clone().ok_or_else(|| {
                port_error(
                    loopx_contract::LoopxCliErrorKind::Backend,
                    operation_id,
                    "managed LoopX source installation is not configured",
                    false,
                )
            })?;
            let parent = target_dir.parent().ok_or_else(|| {
                port_error(
                    loopx_contract::LoopxCliErrorKind::InvalidInput,
                    operation_id,
                    "managed LoopX source path has no parent directory",
                    false,
                )
            })?;
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                port_error(
                    loopx_contract::LoopxCliErrorKind::Io,
                    operation_id,
                    format!("failed to create managed LoopX directory: {error}"),
                    true,
                )
            })?;
            let suffix = format!("{}-{}", std::process::id(), now_unix_ms());
            let staging_dir = parent.join(format!(".loopx-source-install-{suffix}"));
            let backup_dir = parent.join(format!(".loopx-source-backup-{suffix}"));
            let (cancellation, _registration) = self
                .register_operation(operation_id)
                .map_err(|error| map_port_error(error, operation_id))?;

            if let Err(error) = self
                .install_managed_source_checkout(
                    operation_id,
                    &staging_dir,
                    cancellation.clone(),
                    progress,
                )
                .await
            {
                let _ = tokio::fs::remove_dir_all(&staging_dir).await;
                return Err(map_port_error(error, operation_id));
            }

            report_port_progress(
                progress,
                operation_id,
                None,
                loopx_contract::LoopxCliProgressStage::InstallingRuntime,
                "Activating the verified LoopX source",
            );
            let had_previous = tokio::fs::try_exists(&target_dir).await.map_err(|error| {
                port_error(
                    loopx_contract::LoopxCliErrorKind::Io,
                    operation_id,
                    error.to_string(),
                    true,
                )
            })?;
            if had_previous {
                tokio::fs::rename(&target_dir, &backup_dir)
                    .await
                    .map_err(|error| {
                        port_error(
                            loopx_contract::LoopxCliErrorKind::Io,
                            operation_id,
                            format!("failed to stage the previous LoopX source: {error}"),
                            true,
                        )
                    })?;
            }
            if let Err(error) = tokio::fs::rename(&staging_dir, &target_dir).await {
                if had_previous {
                    let _ = tokio::fs::rename(&backup_dir, &target_dir).await;
                }
                let _ = tokio::fs::remove_dir_all(&staging_dir).await;
                return Err(port_error(
                    loopx_contract::LoopxCliErrorKind::Io,
                    operation_id,
                    format!("failed to activate managed LoopX source: {error}"),
                    true,
                ));
            }

            self.verified.lock().await.take();
            if let Err(error) = self
                .ensure_verified(
                    operation_id,
                    cancellation,
                    self.config.startup_deadline,
                    self.observer.as_ref(),
                )
                .await
            {
                self.verified.lock().await.take();
                let _ = tokio::fs::remove_dir_all(&target_dir).await;
                if had_previous {
                    let _ = tokio::fs::rename(&backup_dir, &target_dir).await;
                }
                return Err(map_port_error(error, operation_id));
            }
            if had_previous {
                let _ = tokio::fs::remove_dir_all(&backup_dir).await;
            }
            log::info!(
                "LoopX managed source install activated: operation_id={operation_id}, duration_ms={}",
                received_at.elapsed().as_millis()
            );
            Ok(loopx_contract::LoopxCliInstallManagedSourceResult {
                source_repository: LOOPX_SOURCE_REPOSITORY.to_string(),
                source_tag: LOOPX_PINNED_VERSION_TAG.to_string(),
                source_commit: LOOPX_PINNED_SOURCE_COMMIT.to_string(),
                install_path: target_dir.to_string_lossy().into_owned(),
                loopx_version: LOOPX_PINNED_VERSION.to_string(),
            })
        })
    }

    fn install_open_viking<'a>(
        &'a self,
        request: loopx_contract::LoopxCliInstallOpenVikingRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliInstallOpenVikingResult> {
        Box::pin(async move {
            let operation_id = &request.call.operation_id;
            let received_at = Instant::now();
            log::info!("OpenViking managed install received: operation_id={operation_id}");
            validate_operation_id(operation_id)?;
            let lock_started_at = Instant::now();
            let _install = self.install_lock.lock().await;
            log::info!(
                "OpenViking managed install lock acquired: operation_id={operation_id}, wait_ms={}",
                lock_started_at.elapsed().as_millis()
            );
            let target_dir = self.config.managed_open_viking_dir.clone().ok_or_else(|| {
                port_error(
                    loopx_contract::LoopxCliErrorKind::Backend,
                    operation_id,
                    "managed OpenViking installation is not configured",
                    false,
                )
            })?;
            let parent = target_dir.parent().ok_or_else(|| {
                port_error(
                    loopx_contract::LoopxCliErrorKind::InvalidInput,
                    operation_id,
                    "managed OpenViking path has no parent directory",
                    false,
                )
            })?;
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                port_error(
                    loopx_contract::LoopxCliErrorKind::Io,
                    operation_id,
                    format!("failed to create managed OpenViking directory: {error}"),
                    true,
                )
            })?;
            let (cancellation, _registration) = self
                .register_operation(operation_id)
                .map_err(|error| map_port_error(error, operation_id))?;

            let already_installed = matches!(self.managed_open_viking_command().await, Ok(Some(_)));
            if !already_installed {
                let suffix = format!("{}-{}", std::process::id(), now_unix_ms());
                let staging_dir = parent.join(format!(".openviking-install-{suffix}"));
                let backup_dir = parent.join(format!(".openviking-backup-{suffix}"));
                if let Err(error) = self
                    .install_open_viking_cli(
                        operation_id,
                        &staging_dir,
                        cancellation.clone(),
                        progress,
                    )
                    .await
                {
                    let _ = tokio::fs::remove_dir_all(&staging_dir).await;
                    return Err(map_port_error(error, operation_id));
                }
                let had_previous = tokio::fs::try_exists(&target_dir).await.map_err(|error| {
                    port_error(
                        loopx_contract::LoopxCliErrorKind::Io,
                        operation_id,
                        error.to_string(),
                        true,
                    )
                })?;
                if had_previous {
                    tokio::fs::rename(&target_dir, &backup_dir)
                        .await
                        .map_err(|error| {
                            port_error(
                                loopx_contract::LoopxCliErrorKind::Io,
                                operation_id,
                                format!("failed to stage the previous OpenViking CLI: {error}"),
                                true,
                            )
                        })?;
                }
                if let Err(error) = tokio::fs::rename(&staging_dir, &target_dir).await {
                    if had_previous {
                        let _ = tokio::fs::rename(&backup_dir, &target_dir).await;
                    }
                    let _ = tokio::fs::remove_dir_all(&staging_dir).await;
                    return Err(port_error(
                        loopx_contract::LoopxCliErrorKind::Io,
                        operation_id,
                        format!("failed to activate managed OpenViking CLI: {error}"),
                        true,
                    ));
                }
                if let Err(error) = self
                    .install_open_viking_extension_entrypoint(
                        operation_id,
                        &target_dir,
                        cancellation.clone(),
                        progress,
                    )
                    .await
                {
                    let _ = tokio::fs::remove_dir_all(&target_dir).await;
                    if had_previous {
                        let _ = tokio::fs::rename(&backup_dir, &target_dir).await;
                    }
                    return Err(map_port_error(error, operation_id));
                }
                let binary_sha256 = sha256_file(&open_viking_binary(&target_dir))
                    .await
                    .map_err(|error| map_port_error(error, operation_id))?;
                let manifest = ManagedOpenVikingManifest {
                    schema_version: MANAGED_OPEN_VIKING_MANIFEST_SCHEMA,
                    source_repository: OPEN_VIKING_SOURCE_REPOSITORY.to_string(),
                    source_tag: OPEN_VIKING_PINNED_VERSION_TAG.to_string(),
                    source_commit: OPEN_VIKING_PINNED_SOURCE_COMMIT.to_string(),
                    open_viking_version: OPEN_VIKING_PINNED_VERSION.to_string(),
                    loopx_provider_commit: LOOPX_PINNED_SOURCE_COMMIT.to_string(),
                    binary_sha256,
                };
                let raw = serde_json::to_vec_pretty(&manifest).map_err(|error| {
                    port_error(
                        loopx_contract::LoopxCliErrorKind::Backend,
                        operation_id,
                        error.to_string(),
                        false,
                    )
                })?;
                if let Err(error) =
                    tokio::fs::write(target_dir.join(MANAGED_OPEN_VIKING_MANIFEST), raw).await
                {
                    let _ = tokio::fs::remove_dir_all(&target_dir).await;
                    if had_previous {
                        let _ = tokio::fs::rename(&backup_dir, &target_dir).await;
                    }
                    return Err(port_error(
                        loopx_contract::LoopxCliErrorKind::Io,
                        operation_id,
                        format!("failed to write managed OpenViking manifest: {error}"),
                        true,
                    ));
                }
                if had_previous {
                    let _ = tokio::fs::remove_dir_all(&backup_dir).await;
                }
            }

            self.verified.lock().await.take();
            let command = self
                .managed_open_viking_command()
                .await
                .map_err(|error| map_port_error(error, operation_id))?
                .ok_or_else(|| {
                    port_error(
                        loopx_contract::LoopxCliErrorKind::NotFound,
                        operation_id,
                        "managed OpenViking CLI is unavailable after installation",
                        true,
                    )
                })?;
            let service_ready = self
                .open_viking_service_ready(&format!("{operation_id}-status"), &command)
                .await
                .unwrap_or(false);
            let extension_enabled = if service_ready {
                match self.ensure_open_viking_extension(operation_id).await {
                    Ok(enabled) => enabled,
                    Err(error) => {
                        log::warn!(
                            "OpenViking extension activation deferred: operation_id={operation_id}, error={error}"
                        );
                        false
                    }
                }
            } else {
                false
            };
            log::info!(
                "OpenViking managed install completed: operation_id={operation_id}, service_ready={service_ready}, extension_enabled={extension_enabled}, duration_ms={}",
                received_at.elapsed().as_millis()
            );
            Ok(loopx_contract::LoopxCliInstallOpenVikingResult {
                source_repository: OPEN_VIKING_SOURCE_REPOSITORY.to_string(),
                source_tag: OPEN_VIKING_PINNED_VERSION_TAG.to_string(),
                source_commit: OPEN_VIKING_PINNED_SOURCE_COMMIT.to_string(),
                install_path: target_dir.to_string_lossy().into_owned(),
                open_viking_version: OPEN_VIKING_PINNED_VERSION.to_string(),
                extension_enabled,
            })
        })
    }

    fn probe_open_viking<'a>(
        &'a self,
        request: loopx_contract::LoopxOpenVikingProbeRequest,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxOpenVikingProbe> {
        Box::pin(async move {
            let operation_id = &request.call.operation_id;
            validate_operation_id(operation_id)?;
            let command = match self.select_open_viking_command().await {
                Ok(Some(command)) => command,
                Ok(None) => {
                    return Ok(loopx_contract::LoopxOpenVikingProbe {
                        state: loopx_contract::LoopxOpenVikingState::NotInstalled,
                        detail: Some(
                            "OpenViking CLI is not installed in BitFun-managed storage or PATH"
                                .to_string(),
                        ),
                        ..loopx_contract::LoopxOpenVikingProbe::default()
                    })
                }
                Err(error) => {
                    return Ok(loopx_contract::LoopxOpenVikingProbe {
                        state: loopx_contract::LoopxOpenVikingState::Unhealthy,
                        detail: Some(error.to_string()),
                        ..loopx_contract::LoopxOpenVikingProbe::default()
                    })
                }
            };
            let (cancellation, _registration) = self
                .register_operation(operation_id)
                .map_err(|error| map_port_error(error, operation_id))?;
            let version_output = self
                .runner
                .run(
                    LoopxCommandPlan {
                        operation_id: operation_id.to_string(),
                        executable: command.executable.clone(),
                        args: vec![OsString::from("--version")],
                        current_dir: None,
                        environment: BTreeMap::new(),
                        deadline: self.config.startup_deadline,
                        terminate_grace: self.config.terminate_grace,
                    },
                    cancellation,
                    self.observer.as_ref(),
                )
                .await;
            let version = match version_output {
                Ok(output) => parse_open_viking_version(&output.stdout).or(command.version.clone()),
                Err(error) => {
                    return Ok(loopx_contract::LoopxOpenVikingProbe {
                        state: loopx_contract::LoopxOpenVikingState::Unhealthy,
                        version: command.version,
                        detail: Some(format!("OpenViking CLI version probe failed: {error}")),
                    })
                }
            };
            let Some(version) = version else {
                return Ok(loopx_contract::LoopxOpenVikingProbe {
                    state: loopx_contract::LoopxOpenVikingState::Unhealthy,
                    detail: Some("OpenViking CLI returned an unreadable version".to_string()),
                    ..loopx_contract::LoopxOpenVikingProbe::default()
                });
            };
            if !open_viking_version_supported(&version) {
                return Ok(loopx_contract::LoopxOpenVikingProbe {
                    state: loopx_contract::LoopxOpenVikingState::Unhealthy,
                    version: Some(version),
                    detail: Some(format!(
                        "OpenViking {OPEN_VIKING_PINNED_VERSION} or newer is required"
                    )),
                });
            }
            if !self
                .open_viking_service_ready(&format!("{operation_id}-status"), &command)
                .await
                .unwrap_or(false)
            {
                return Ok(loopx_contract::LoopxOpenVikingProbe {
                    state: loopx_contract::LoopxOpenVikingState::NotConfigured,
                    version: Some(version),
                    detail: Some(
                        "OpenViking CLI is installed; configure and start an OpenViking service"
                            .to_string(),
                    ),
                });
            }
            let listed = self
                .run_global_json_command(
                    &format!("{operation_id}-extensions"),
                    ["extension", "list"]
                        .into_iter()
                        .map(OsString::from)
                        .collect(),
                    self.config.command_deadline,
                    self.observer.as_ref(),
                )
                .await;
            let enabled = listed
                .ok()
                .and_then(|output| output.payload.get("extensions").cloned())
                .and_then(|extensions| extensions.as_array().cloned())
                .and_then(|extensions| {
                    extensions.into_iter().find(|extension| {
                        extension.get("id").and_then(Value::as_str)
                            == Some(OPEN_VIKING_EXTENSION_ID)
                    })
                })
                .and_then(|extension| extension.get("enabled").and_then(Value::as_bool))
                .unwrap_or(false);
            Ok(loopx_contract::LoopxOpenVikingProbe {
                state: if enabled {
                    loopx_contract::LoopxOpenVikingState::Ready
                } else {
                    loopx_contract::LoopxOpenVikingState::Disabled
                },
                version: Some(version),
                detail: Some(if enabled {
                    if command.managed {
                        "Managed OpenViking CLI and LoopX semantic-preference extension are ready"
                            .to_string()
                    } else {
                        "System OpenViking CLI and LoopX semantic-preference extension are ready"
                            .to_string()
                    }
                } else {
                    "OpenViking service is reachable; enable the LoopX semantic-preference extension"
                        .to_string()
                }),
            })
        })
    }

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
                        LoopxCommandSource::ManagedSource => {
                            loopx_contract::LoopxCliSource::PythonFallback
                        }
                        LoopxCommandSource::FixedSystemCommand => {
                            loopx_contract::LoopxCliSource::System
                        }
                    },
                    identity: match verified.source {
                        LoopxCommandSource::PackagedBundle => {
                            "bitfun-bundled-loopx-v0.5.1".to_string()
                        }
                        LoopxCommandSource::ManagedSource => {
                            "bitfun-managed-github-source-loopx-v0.5.1".to_string()
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
                // The full text stays reachable via the persisted intake plan
                // packet; the registry copy must stay short because the whole
                // todo list rides inside LoopX's turn envelope compaction
                // budget (observed: long successor texts pushed the envelope
                // past 8 KiB and LoopX rejected planning for the Goal).
                let text =
                    truncate_todo_text(&required_json_string(preview, "text", operation_id)?);
                let action_kind = preview
                    .get("action_kind")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let next_command_preview = preview
                    .get("next_command_preview")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
                let target_key = preview
                    .get("target_key")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
                todos.push(loopx_contract::LoopxCliTodoPlan {
                    role,
                    task_class,
                    action_kind,
                    text,
                    next_command_preview,
                    target_key,
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
            // LoopX's issue-fix workflow contract pins the goal objective to
            // the host-resolved issue title; the packet itself carries no
            // objective field.
            let objective = compact_objective(&request.item, &request.title);
            // The full packet is persisted into the worktree by the controller
            // (via the workspace port) so every turn can read the exact
            // issue-fix command forms. It stays public-safe by LoopX design;
            // the byte cap only guards against pathological payloads.
            let raw_packet_json = match serde_json::to_string(&output.payload) {
                Ok(json) if json.len() <= MAX_INTAKE_PLAN_PACKET_BYTES => json,
                _ => String::new(),
            };
            Ok(loopx_contract::LoopxCliIntakePlan {
                item: request.item,
                objective,
                todos,
                raw_packet_json,
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

            // Source-backed candidate admission must be persisted before any
            // feasibility decision ("only admitted proceed may start a new
            // implementation"). Running it here — instead of leaving the
            // agent to substitute the goal id into the packet's template
            // command — guarantees the preflight receipt lands in the
            // goal-scoped domain ledger and lets the returned packet carry
            // the real admission state.
            // Best-effort by design: a missing `gh` binary, GraphQL rate
            // limiting, or a PR-kind item (the collector reads the issue
            // timeline) must never fail goal creation; the agent's own
            // collect-evidence successor from the plan packet stays the
            // fallback path.
            let evidence_packet_json =
                collect_candidate_evidence_best_effort(self, &request, operation_id, &observer)
                    .await;

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
                raw_packet_json: evidence_packet_json,
            })
        })
    }

    fn create_user_gate<'a>(
        &'a self,
        request: loopx_contract::LoopxCliCreateUserGateRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliCreateUserGateResult> {
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
                "Creating a bounded LoopX owner review gate",
            );
            let result = run_port_command(
                self,
                &request.context,
                add_user_gate_args(&request)?,
                &observer,
            )
            .await?;
            require_payload_ok(&result.payload, operation_id)?;
            let gate_id = required_json_string(&result.payload, "todo_id", operation_id)?;
            let created = result
                .payload
                .get("added")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let inspection = run_port_command(
                self,
                &request.context,
                inspect_goal_args(&request.goal_id, &request.agent_id, None),
                &observer,
            )
            .await?;
            require_payload_ok(&inspection.payload, operation_id)?;
            let snapshot =
                project_goal_snapshot(&request.goal_id, &inspection.payload, operation_id)?;
            let settlement_receipt_count = snapshot.settlement_receipt_ids.len() as u32;
            Ok(loopx_contract::LoopxCliCreateUserGateResult {
                goal_id: request.goal_id,
                gate_id,
                created,
                durable_revision: snapshot.durable_revision,
                settlement_receipt_count,
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
            } else if snapshot.run_decision == loopx_contract::LoopxCliRunDecision::RunNow {
                // The host injects the bounded todo projection into the next
                // turn prompt, so the Agent does not re-derive todo state from
                // the registry on every heartbeat. Failure to list todos must
                // not block the turn; the task body remains the authority.
                if let Ok(todos) = run_port_command(
                    self,
                    &request.context,
                    list_todos_args(&request.goal_id),
                    &observer,
                )
                .await
                {
                    snapshot.open_agent_todos =
                        project_open_agent_todos(&todos.payload, operation_id);
                }
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
            let settlement_binding = planned_settlement_binding(&plan.payload);
            let settlement_token = planned_settlement_token(
                &plan.payload,
                &request.goal_id,
                &request.agent_id,
                &turn_id,
                operation_id,
            )?;
            // The host executes the turn guard itself so the heartbeat receipt
            // is committed and bound to the settlement todo before the agent
            // starts. LoopX rejects turn-scoped `refresh-state` writebacks
            // whose todo does not match the original guard receipt, so an
            // unbound guard silently poisons the whole settlement chain.
            let guard_output = run_port_command(
                self,
                &request.context,
                turn_guard_args(
                    &request.goal_id,
                    &request.agent_id,
                    settlement_binding.as_ref(),
                    &turn_id,
                ),
                &observer,
            )
            .await?;
            require_payload_ok(&guard_output.payload, operation_id)?;
            if guard_output
                .payload
                .get("should_run")
                .and_then(Value::as_bool)
                != Some(true)
            {
                return Err(port_error(
                    loopx_contract::LoopxCliErrorKind::Conflict,
                    operation_id,
                    "LoopX turn guard declined to run after the plan selected RunNow",
                    true,
                ));
            }
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
            let command = agent_shell_command(&verified);
            let registry = agent_shell_value(&request.context.registry_path);
            let cli_prefix = format!("{command} --format json --registry {registry}");
            let adapted_prompt = raw_prompt
                .replace("${LOOPX_TURN:?}", &turn_id)
                .replace(
                    "loopx --format json --registry \"$HOME/.codex/loopx/registry.global.json\" ",
                    &format!("{cli_prefix} "),
                )
                .replace("loopx ", &format!("{cli_prefix} "));
            // The pinned compact body embeds the upstream Codex-CLI heartbeat
            // preflight (PATH setup, `loopx doctor`, install-local fallback,
            // unbound guard). Those steps are host-owned or wrong for the
            // BitFun external host, so the fence is rewritten to the exact
            // bound guard command instead.
            let adapted_prompt = replace_embedded_preflight_fence(
                &adapted_prompt,
                &turn_guard_prompt_command(
                    &cli_prefix,
                    &request.goal_id,
                    &request.agent_id,
                    settlement_binding.as_ref(),
                    &turn_id,
                ),
            );
            let adapted_prompt = replace_host_conflicting_lines(&adapted_prompt);
            let settlement_override = settlement_prompt_override(
                &cli_prefix,
                &request.goal_id,
                &request.agent_id,
                &turn_id,
                settlement_binding.as_ref(),
            );
            let guard_summary = match settlement_binding.as_ref() {
                Some(SettlementBinding::Todo { todo_id }) => format!("todo {todo_id}"),
                Some(SettlementBinding::AutonomousReplan { obligation_id }) => {
                    format!("autonomous replan obligation {obligation_id}")
                }
                None => "no settlement binding".to_string(),
            };
            let prompt = format!(
                "LoopX host binding for this bounded turn:\n- Repository: {}\n- Registry: {}\n- Exact CLI prefix: {}\n- Turn id: {}\n- Turn guard: the BitFun host already executed `quota should-run` for this turn (should_run=true) with settlement binding {}; the heartbeat receipt is committed. Do not run `quota should-run` again to decide whether to work; only re-run the exact command printed in the preflight block when a LoopX settlement command reports a missing or mismatched heartbeat receipt, and always with this turn id.\nUse this immutable binding for every LoopX command. Do not probe PATH, switch registries, invent a Turn id, infer state from prose, or substitute host-specific policy. Query the exact CLI prefix when fresh LoopX state is required; the generated task body below remains the authority for work selection.\n{}\n\n{}\n\n{}",
                request.context.worktree_path,
                request.context.registry_path,
                cli_prefix,
                turn_id,
                guard_summary,
                AGENT_CLI_USAGE_RULES,
                adapted_prompt,
                settlement_override,
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
            let snapshot =
                project_goal_snapshot(&request.goal_id, &inspection.payload, operation_id)?;
            let settlement_receipt_count = snapshot.settlement_receipt_ids.len() as u32;
            Ok(loopx_contract::LoopxCliAnswerGateResult {
                goal_id: request.goal_id,
                gate_id: request.gate_id,
                applied: true,
                durable_revision: snapshot.durable_revision,
                goal_state: snapshot.state,
                settlement_receipt_count,
            })
        })
    }

    fn finalize_turn_settlement<'a>(
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
                "Verifying durable LoopX progress and quota settlement evidence",
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
            let mut snapshot =
                project_goal_snapshot(&request.goal_id, &inspection.payload, operation_id)?;
            if let Some(receipt) =
                matching_settlement_receipt(&inspection.payload, &request.settlement_token)
            {
                return project_legacy_settlement(&request, &snapshot, receipt, operation_id);
            }
            let mut history = run_port_command(
                self,
                &request.context,
                settlement_history_args(&request.goal_id),
                &observer,
            )
            .await?;
            require_payload_ok(&history.payload, operation_id)?;
            let mut evidence = matching_durable_progress(
                &history.payload,
                &request.goal_id,
                &request.agent_id,
                &request.turn_id,
                &request.settlement_token,
                operation_id,
            )?;
            if evidence
                .as_ref()
                .is_some_and(|evidence| !evidence.quota_spent)
                && request.agent_status == loopx_contract::LoopxAgentTurnStatus::Completed
            {
                let binding = evidence.as_ref().expect("checked above").binding.clone();
                report_port_progress(
                    progress,
                    operation_id,
                    Some(request.context.task_id.clone()),
                    loopx_contract::LoopxCliProgressStage::SettlingTurn,
                    "Finalizing LoopX quota accounting for validated durable writeback",
                );
                let quota = run_port_command(
                    self,
                    &request.context,
                    quota_spend_args(
                        &request.goal_id,
                        &request.agent_id,
                        &binding,
                        &request.turn_id,
                    ),
                    &observer,
                )
                .await?;
                require_payload_ok(&quota.payload, operation_id)?;
                require_quota_spend_response(&quota.payload, operation_id)?;

                history = run_port_command(
                    self,
                    &request.context,
                    settlement_history_args(&request.goal_id),
                    &observer,
                )
                .await?;
                require_payload_ok(&history.payload, operation_id)?;
                evidence = matching_durable_progress(
                    &history.payload,
                    &request.goal_id,
                    &request.agent_id,
                    &request.turn_id,
                    &request.settlement_token,
                    operation_id,
                )?;
                if evidence
                    .as_ref()
                    .is_some_and(|evidence| evidence.quota_spent)
                {
                    let refreshed = run_port_command(
                        self,
                        &request.context,
                        inspect_goal_args(
                            &request.goal_id,
                            &request.agent_id,
                            Some(&request.turn_id),
                        ),
                        &observer,
                    )
                    .await?;
                    snapshot =
                        project_goal_snapshot(&request.goal_id, &refreshed.payload, operation_id)?;
                    report_port_progress(
                        progress,
                        operation_id,
                        Some(request.context.task_id.clone()),
                        loopx_contract::LoopxCliProgressStage::SettlingTurn,
                        "LoopX quota accounting finalized and verified by readback",
                    );
                }
            }
            let Some(evidence) = evidence else {
                report_port_progress(
                    progress,
                    operation_id,
                    Some(request.context.task_id.clone()),
                    loopx_contract::LoopxCliProgressStage::SettlingTurn,
                    "No matching durable LoopX writeback was found for this turn",
                );
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
            if !evidence.quota_spent {
                report_port_progress(
                    progress,
                    operation_id,
                    Some(request.context.task_id.clone()),
                    loopx_contract::LoopxCliProgressStage::SettlingTurn,
                    "Matching durable LoopX writeback exists, but quota finalization was not verified",
                );
                return Ok(loopx_contract::LoopxCliSettleTurnResult {
                    goal_id: request.goal_id,
                    turn_id: request.turn_id,
                    status: loopx_contract::LoopxCliSettlementStatus::RetryRequired,
                    before_revision: request.expected_durable_revision,
                    after_revision: snapshot.durable_revision,
                    scheduler_hint_ms: snapshot.scheduler_hint_ms,
                    ..loopx_contract::LoopxCliSettleTurnResult::default()
                });
            }
            report_port_progress(
                progress,
                operation_id,
                Some(request.context.task_id.clone()),
                loopx_contract::LoopxCliProgressStage::SettlingTurn,
                "Matched validated LoopX progress and quota settlement evidence",
            );
            Ok(loopx_contract::LoopxCliSettleTurnResult {
                goal_id: request.goal_id,
                turn_id: request.turn_id,
                receipt_id: evidence.effect_id,
                status: if snapshot.state == loopx_contract::LoopxCliGoalState::Completed {
                    loopx_contract::LoopxCliSettlementStatus::GoalCompleted
                } else {
                    loopx_contract::LoopxCliSettlementStatus::Settled
                },
                before_revision: request.expected_durable_revision,
                after_revision: snapshot.durable_revision,
                validation_succeeded: true,
                scheduler_hint_ms: snapshot.scheduler_hint_ms,
                binding_todo_id: evidence.binding.todo_id().map(str::to_string),
            })
        })
    }

    fn stop_goal<'a>(
        &'a self,
        request: loopx_contract::LoopxCliStopGoalRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliStopGoalResult> {
        Box::pin(async move {
            validate_goal_context(&request.context)?;
            validate_nonempty(
                "goal_id",
                &request.goal_id,
                &request.context.call.operation_id,
            )?;
            let operation_id = &request.context.call.operation_id;
            let observer = PortProcessObserver {
                progress,
                fallback: self.observer.as_ref(),
                task_id: Some(request.context.task_id.clone()),
                stage: loopx_contract::LoopxCliProgressStage::Cancelling,
            };
            report_port_progress(
                progress,
                operation_id,
                Some(request.context.task_id.clone()),
                loopx_contract::LoopxCliProgressStage::Cancelling,
                "Stopping LoopX automatic advancement for the finished Goal",
            );
            let output = run_port_command(
                self,
                &request.context,
                goal_stop_args(&request.goal_id, &request.reason),
                &observer,
            )
            .await?;
            require_payload_ok(&output.payload, operation_id)?;
            let stopped =
                output.payload.get("after_state").and_then(Value::as_str) == Some("stopped");
            Ok(loopx_contract::LoopxCliStopGoalResult {
                goal_id: request.goal_id,
                stopped,
                already_stopped: output.payload.get("changed").and_then(Value::as_bool)
                    != Some(true),
            })
        })
    }

    /// Shortens over-long open todo texts so the todo list fits LoopX's
    /// turn-envelope compaction budget. LoopX keys some idempotency on
    /// deterministic todo text (e.g. maintainer-correction retries), so the
    /// shrink only ever trims display text — never action_kind, target keys,
    /// or todo identity — and the full original text stays reachable in the
    /// persisted intake plan packet.
    fn shrink_todo_texts<'a>(
        &'a self,
        request: loopx_contract::LoopxCliShrinkTodoTextsRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliShrinkTodoTextsResult> {
        Box::pin(async move {
            validate_goal_context(&request.context)?;
            validate_nonempty(
                "goal_id",
                &request.goal_id,
                &request.context.call.operation_id,
            )?;
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
                "Shortening over-long todo texts for the turn envelope budget",
            );
            let max_chars = request.max_chars.clamp(80, 400) as usize;
            let todos = run_port_command(
                self,
                &request.context,
                list_todos_args(&request.goal_id),
                &observer,
            )
            .await?;
            require_payload_ok(&todos.payload, operation_id)?;
            let mut examined = 0_u32;
            let mut shortened = 0_u32;
            for todo in todos
                .payload
                .get("todos")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
            {
                let todo_id = todo
                    .get("todo_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if todo_id.trim().is_empty() {
                    continue;
                }
                let role = todo.get("role").and_then(Value::as_str).unwrap_or_default();
                let status = todo
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let done = todo.get("done").and_then(Value::as_bool) == Some(true);
                if role != "agent"
                    || done
                    || matches!(
                        status,
                        "completed" | "closed" | "done" | "archived" | "cancelled"
                    )
                {
                    continue;
                }
                examined += 1;
                let text = todo.get("text").and_then(Value::as_str).unwrap_or_default();
                if text.chars().count() <= max_chars {
                    continue;
                }
                let mut bounded: String = text.chars().take(max_chars).collect();
                bounded.push_str("...");
                let update = run_port_command(
                    self,
                    &request.context,
                    update_todo_text_args(&request.goal_id, todo_id, &bounded, &request.agent_id),
                    &observer,
                )
                .await;
                if update.is_ok() {
                    shortened += 1;
                } else {
                    log::warn!(
                        "LoopX todo text shrink failed: goal={}, todo={}, error={:?}",
                        request.goal_id,
                        todo_id,
                        update.err()
                    );
                }
            }
            Ok(loopx_contract::LoopxCliShrinkTodoTextsResult {
                goal_id: request.goal_id,
                examined,
                shortened,
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

    fn reset_goals<'a>(
        &'a self,
        request: loopx_contract::LoopxCliResetGoalsRequest,
        progress: &'a dyn loopx_contract::LoopxCliProgressSink,
    ) -> loopx_contract::LoopxCliFuture<'a, loopx_contract::LoopxCliResetGoalsResult> {
        Box::pin(async move {
            validate_operation_id(&request.call.operation_id)?;
            let goal_ids = request
                .goal_ids
                .into_iter()
                .map(|goal_id| goal_id.trim().to_string())
                .filter(|goal_id| !goal_id.is_empty())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if goal_ids.is_empty() {
                return Err(port_error(
                    loopx_contract::LoopxCliErrorKind::InvalidInput,
                    &request.call.operation_id,
                    "reset_goals requires at least one explicit goal id",
                    false,
                ));
            }
            let operation_id = &request.call.operation_id;
            let observer = PortProcessObserver {
                progress,
                fallback: self.observer.as_ref(),
                task_id: None,
                stage: loopx_contract::LoopxCliProgressStage::Cancelling,
            };
            report_port_progress(
                progress,
                operation_id,
                None,
                loopx_contract::LoopxCliProgressStage::Cancelling,
                "Retiring global LoopX goal routes and archiving runtime state",
            );
            let deadline = effective_deadline(
                request.call.deadline_at,
                self.config.command_deadline,
                operation_id,
            )?;
            let mut result = loopx_contract::LoopxCliResetGoalsResult {
                requested_goal_ids: goal_ids.clone(),
                ..loopx_contract::LoopxCliResetGoalsResult::default()
            };

            for goal_id in goal_ids {
                let retired = run_idempotent_global_command(
                    self,
                    operation_id,
                    retire_global_goal_args(&goal_id),
                    "goal_id not found in global registry:",
                    deadline,
                    &observer,
                )
                .await?;
                if let Some(output) = retired {
                    require_payload_ok(&output.payload, operation_id)?;
                    require_schema(
                        &output.payload,
                        "loopx_global_goal_retirement_v0",
                        operation_id,
                    )?;
                    let retired_ids = output
                        .payload
                        .get("retired_goal_ids")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    if !retired_ids
                        .iter()
                        .any(|value| value.as_str() == Some(&goal_id))
                    {
                        return Err(port_error(
                            loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                            operation_id,
                            format!("retire-global-goal did not confirm goal {goal_id}"),
                            false,
                        ));
                    }
                    result.retired_goal_ids.push(goal_id.clone());
                    if let Some(path) = output.payload.get("backup_path").and_then(Value::as_str) {
                        result.backup_paths.push(path.to_string());
                    }
                } else {
                    result.already_absent_goal_ids.push(goal_id.clone());
                }

                let archived = run_idempotent_global_command(
                    self,
                    operation_id,
                    archive_runtime_args(&goal_id),
                    "runtime goal directory does not exist:",
                    deadline,
                    &observer,
                )
                .await?;
                if let Some(output) = archived {
                    require_payload_ok(&output.payload, operation_id)?;
                    if output.payload.get("archived").and_then(Value::as_bool) != Some(true) {
                        return Err(port_error(
                            loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                            operation_id,
                            format!("archive-runtime did not confirm goal {goal_id}"),
                            false,
                        ));
                    }
                    result.archived_goal_ids.push(goal_id.clone());
                    if let Some(path) = output.payload.get("archive_path").and_then(Value::as_str) {
                        result.archive_paths.push(path.to_string());
                    }
                } else {
                    result.missing_runtime_goal_ids.push(goal_id);
                }
            }
            Ok(result)
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

fn planned_settlement_token(
    payload: &Value,
    goal_id: &str,
    agent_id: &str,
    turn_id: &str,
    operation_id: &str,
) -> loopx_contract::LoopxCliResult<String> {
    if let Some(binding) = planned_settlement_binding(payload) {
        return Ok(binding.effect_id(goal_id, agent_id, turn_id));
    }

    payload
        .pointer("/transaction/turn_key")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            port_error(
                loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                operation_id,
                "turn plan did not contain a selected Todo or transaction.turn_key",
                false,
            )
        })
}

fn planned_settlement_binding(payload: &Value) -> Option<SettlementBinding> {
    let route_kind = payload.pointer("/route/kind").and_then(Value::as_str);
    if route_kind == Some("replan_required") {
        let obligation_id = payload
            .pointer("/turn_envelope/replan_action_packet/obligation_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())?;
        return Some(SettlementBinding::AutonomousReplan {
            obligation_id: obligation_id.to_string(),
        });
    }

    planned_todo_id(payload).map(|todo_id| SettlementBinding::Todo {
        todo_id: todo_id.to_string(),
    })
}

fn planned_todo_id(payload: &Value) -> Option<&str> {
    [
        "/route/selected_todo/todo_id",
        "/turn_envelope/action/selected_todo/todo_id",
    ]
    .into_iter()
    .find_map(|pointer| {
        payload
            .pointer(pointer)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SettlementBinding {
    Todo { todo_id: String },
    AutonomousReplan { obligation_id: String },
}

impl SettlementBinding {
    fn todo_id(&self) -> Option<&str> {
        match self {
            Self::Todo { todo_id } => Some(todo_id.as_str()),
            Self::AutonomousReplan { .. } => None,
        }
    }

    fn effect_id(&self, goal_id: &str, agent_id: &str, turn_id: &str) -> String {
        match self {
            Self::Todo { todo_id } => settlement_effect_id(goal_id, agent_id, todo_id, turn_id),
            Self::AutonomousReplan { obligation_id } => {
                replan_settlement_effect_id(goal_id, agent_id, obligation_id, turn_id)
            }
        }
    }
}

/// Host-side `quota should-run` arguments for one bounded turn. The todo
/// binding must match the planned settlement binding exactly: LoopX rejects
/// turn-scoped `refresh-state` writebacks whose todo differs from the guard
/// receipt. Replan obligations cannot ride on the guard (the pinned CLI only
/// accepts `--replan-obligation-id` on `spend-slot`), so those guards run
/// unbound, matching the legacy behavior.
fn turn_guard_args(
    goal_id: &str,
    agent_id: &str,
    binding: Option<&SettlementBinding>,
    turn_id: &str,
) -> Vec<OsString> {
    let mut args = vec![
        "quota".to_string(),
        "should-run".to_string(),
        "--goal-id".to_string(),
        goal_id.to_string(),
    ];
    if let Some(todo_id) = binding.and_then(SettlementBinding::todo_id) {
        args.extend(["--todo-id".to_string(), todo_id.to_string()]);
    }
    args.extend([
        "--agent-id".to_string(),
        agent_id.to_string(),
        "--runtime-profile".to_string(),
        "outer_controller".to_string(),
        "--turn-instance-id".to_string(),
        turn_id.to_string(),
    ]);
    args.into_iter().map(OsString::from).collect()
}

/// The exact already-prefixed guard command injected into the task body's
/// preflight fence. It is the only sanctioned re-run path when a settlement
/// command reports a missing or mismatched heartbeat receipt.
fn turn_guard_prompt_command(
    cli_prefix: &str,
    goal_id: &str,
    agent_id: &str,
    binding: Option<&SettlementBinding>,
    turn_id: &str,
) -> String {
    let todo_flag = match binding.and_then(SettlementBinding::todo_id) {
        Some(todo_id) => format!("--todo-id {todo_id} "),
        None => String::new(),
    };
    format!(
        "{cli_prefix} quota should-run --goal-id {goal_id} {todo_flag}--agent-id {agent_id} --runtime-profile outer_controller --turn-instance-id {turn_id}"
    )
}

/// Replaces the pinned compact heartbeat body's Codex-CLI preflight bash
/// fence (PATH setup, `loopx doctor`, install-local fallback, unbound guard)
/// with the host-owned bound guard command. The pinned v0.5.1 compact body
/// renders exactly one fence containing the install-local fallback and the
/// guard command; replacement is structural and fails soft when the body
/// shape drifts, in which case the binding block and settlement override
/// still carry the contract.
fn replace_embedded_preflight_fence(adapted_prompt: &str, guard_command: &str) -> String {
    let lines: Vec<&str> = adapted_prompt.lines().collect();
    let mut result: Vec<String> = Vec::with_capacity(lines.len() + 2);
    let mut index = 0;
    let mut replaced_any = false;
    while index < lines.len() {
        let line = lines[index];
        if line.trim() == "```bash" {
            if let Some(offset) = lines[index + 1..]
                .iter()
                .position(|candidate| candidate.trim() == "```")
            {
                let close = index + 1 + offset;
                let block = &lines[index + 1..close];
                let is_codex_install_block = block
                    .iter()
                    .any(|candidate| candidate.contains("install-local.sh"))
                    && block
                        .iter()
                        .any(|candidate| candidate.contains("quota should-run"));
                if is_codex_install_block {
                    result.push("```bash".to_string());
                    result.push(guard_command.to_string());
                    result.push("```".to_string());
                    replaced_any = true;
                    index = close + 1;
                    continue;
                }
            }
        }
        result.push(line.to_string());
        index += 1;
    }
    if !replaced_any {
        return adapted_prompt.to_string();
    }
    let mut joined = result.join("\n");
    if adapted_prompt.ends_with('\n') && !joined.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

/// Neutralizes the pinned compact body's lines that contradict the BitFun
/// external-host contract: the upstream quota spend example (the host owns
/// accounting) and the Codex-specific pre-trusted-session note (BitFun turns
/// request owner decisions through typed user gates). Line-scoped and
/// fail-soft for the pinned body shape.
fn replace_host_conflicting_lines(adapted_prompt: &str) -> String {
    let lines = adapted_prompt
        .lines()
        .map(|line| {
            if line.contains("quota spend-slot") {
                return "# quota spend-slot is executed by the BitFun host after settlement verification; do not run it.".to_string();
            }
            if line.contains("Codex session is already trusted") {
                return "The BitFun session is not pre-trusted: request owner decisions by writing a `user_gate` todo through the LoopX CLI, then end the turn.".to_string();
            }
            line.to_string()
        })
        .collect::<Vec<_>>();
    let mut joined = lines.join("\n");
    if adapted_prompt.ends_with('\n') && !joined.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

fn settlement_prompt_override(
    cli_prefix: &str,
    goal_id: &str,
    agent_id: &str,
    turn_id: &str,
    binding: Option<&SettlementBinding>,
) -> String {
    let binding_flags = match binding {
        Some(SettlementBinding::Todo { todo_id }) => format!(
            "--todo-id {todo_id} --turn-instance-id {turn_id} --agent-id {agent_id}"
        ),
        Some(SettlementBinding::AutonomousReplan { obligation_id }) => format!(
            "--replan-obligation-id {obligation_id} --turn-instance-id {turn_id} --autonomous-replan-recorded --agent-id {agent_id}"
        ),
        None => format!("--turn-instance-id {turn_id} --agent-id {agent_id}"),
    };
    format!(
        "BitFun external-host settlement boundary (this final block overrides generic spend examples above):\n- The host owns quota accounting. Do not run `quota spend-slot`; do not retry or repair quota yourself.\n- The turn guard was already executed and bound by the host before the agent started. If `refresh-state` rejects the writeback because the heartbeat receipt is missing or mismatched, re-run the exact guard command printed in the task body preflight once with this turn id, then retry the identical `refresh-state` once.\n- Before ending a materially productive turn, validate the real result and append one accountable `refresh-state` using the exact CLI prefix and these mandatory binding flags: `{binding_flags}`.\n- Choose truthful classification, delivery scale, delivery outcome, and typed progress fields from the work actually completed; never invent progress.\n- On Windows, keep LoopX control-plane text arguments in English ASCII so command-line writeback remains lossless. Product files and user-visible source content are not restricted by this rule.\n- Required command shape: `{cli_prefix} refresh-state --goal-id {goal_id} <truthful outcome fields> {binding_flags}`.\n- After that durable writeback succeeds, end the turn. The host will verify it and append/read back the exact idempotent quota receipt."
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DurableSettlementEvidence {
    effect_id: String,
    binding: SettlementBinding,
    quota_spent: bool,
}

fn matching_durable_progress(
    payload: &Value,
    goal_id: &str,
    agent_id: &str,
    turn_id: &str,
    settlement_token: &str,
    operation_id: &str,
) -> loopx_contract::LoopxCliResult<Option<DurableSettlementEvidence>> {
    let goals = payload
        .get("goals")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            port_error(
                loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                operation_id,
                "LoopX history response did not contain goals",
                false,
            )
        })?;
    let Some(goal) = goals
        .iter()
        .find(|goal| goal.get("id").and_then(Value::as_str) == Some(goal_id))
    else {
        return Ok(None);
    };
    let runs = goal
        .get("latest_runs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            port_error(
                loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                operation_id,
                "LoopX history goal did not contain latest_runs",
                false,
            )
        })?;

    let mut accountable_effects = BTreeMap::<String, SettlementBinding>::new();
    let mut quota_effects = BTreeSet::<String>::new();
    for run in runs {
        let Some((effect_id, binding)) = matching_run_identity(run, goal_id, agent_id, turn_id)
        else {
            continue;
        };
        match run.get("classification").and_then(Value::as_str) {
            Some("quota_slot_spent") => {
                quota_effects.insert(effect_id);
            }
            _ if run_has_accountable_progress(run) => {
                accountable_effects.insert(effect_id, binding);
            }
            _ => {}
        }
    }

    // Every candidate above is already pinned to this exact goal, agent, and
    // host-issued turn id by `matching_run_identity`. The planned settlement
    // token additionally pins the todo the projection selected at build time,
    // but the agent may legitimately advance a different todo within the same
    // bounded turn; that progress is still durable evidence for this turn, so
    // fall back to the turn-scoped candidates when no token match exists.
    let token_matches = accountable_effects
        .iter()
        .filter(|(effect_id, _)| {
            settlement_token == *effect_id || is_legacy_turn_key(settlement_token)
        })
        .map(|(effect_id, binding)| (effect_id.clone(), binding.clone()))
        .collect::<BTreeMap<_, _>>();
    let candidates = if token_matches.is_empty() {
        accountable_effects
    } else {
        token_matches
    };
    // Prefer a candidate whose quota spend already settled; otherwise take the
    // first deterministic entry.
    let chosen = candidates
        .iter()
        .find(|(effect_id, _)| quota_effects.contains(*effect_id))
        .or_else(|| candidates.iter().next());
    let Some((effect_id, binding)) = chosen else {
        return Ok(None);
    };
    Ok(Some(DurableSettlementEvidence {
        effect_id: effect_id.clone(),
        binding: binding.clone(),
        quota_spent: quota_effects.contains(effect_id),
    }))
}

fn run_has_accountable_progress(run: &Value) -> bool {
    if let Some(observation) = run
        .get("progress_observation")
        .filter(|observation| !observation.is_null())
    {
        let typed_progress = observation.get("schema_version").and_then(Value::as_str)
            == Some("typed_progress_observation_v0")
            && matches!(
                observation.get("result_class").and_then(Value::as_str),
                Some("advanced" | "no_followup")
            );
        if typed_progress {
            return true;
        }
    }

    matches!(
        run.get("delivery_outcome").and_then(Value::as_str),
        Some("outcome_progress" | "primary_goal_outcome")
    )
}

fn matching_run_identity(
    run: &Value,
    goal_id: &str,
    agent_id: &str,
    turn_id: &str,
) -> Option<(String, SettlementBinding)> {
    let identity = run.get("settlement_identity")?;
    let effect_id = identity
        .get("effect_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())?
        .to_string();
    let exact_owner = identity.get("goal_id").and_then(Value::as_str) == Some(goal_id)
        && identity.get("agent_id").and_then(Value::as_str) == Some(agent_id)
        && identity.get("turn_instance_id").and_then(Value::as_str) == Some(turn_id)
        && run.get("goal_id").and_then(Value::as_str) == Some(goal_id)
        && run.get("agent_id").and_then(Value::as_str) == Some(agent_id)
        && run.get("turn_instance_id").and_then(Value::as_str) == Some(turn_id);
    if !exact_owner {
        return None;
    }

    let binding = match identity.get("schema_version").and_then(Value::as_str) {
        Some("quota_settlement_identity_v0") => {
            let todo_id = identity
                .get("todo_id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())?;
            if run.get("todo_id").and_then(Value::as_str) != Some(todo_id) {
                return None;
            }
            SettlementBinding::Todo {
                todo_id: todo_id.to_string(),
            }
        }
        Some("quota_settlement_identity_v1")
            if identity.get("binding_kind").and_then(Value::as_str)
                == Some("autonomous_replan") =>
        {
            let obligation_id = identity
                .get("replan_obligation_id")
                .or_else(|| identity.get("binding_id"))
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())?;
            if run.get("replan_obligation_id").and_then(Value::as_str) != Some(obligation_id) {
                return None;
            }
            SettlementBinding::AutonomousReplan {
                obligation_id: obligation_id.to_string(),
            }
        }
        _ => return None,
    };
    (effect_id == binding.effect_id(goal_id, agent_id, turn_id)).then_some((effect_id, binding))
}

fn settlement_effect_id(goal_id: &str, agent_id: &str, todo_id: &str, turn_id: &str) -> String {
    format!("{goal_id}:{agent_id}:{todo_id}:{turn_id}")
}

fn replan_settlement_effect_id(
    goal_id: &str,
    agent_id: &str,
    obligation_id: &str,
    turn_id: &str,
) -> String {
    format!("{goal_id}:{agent_id}:autonomous_replan:{obligation_id}:{turn_id}")
}

fn is_legacy_turn_key(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn project_legacy_settlement(
    request: &loopx_contract::LoopxCliSettleTurnRequest,
    snapshot: &loopx_contract::LoopxCliGoalSnapshot,
    receipt: &Value,
    operation_id: &str,
) -> loopx_contract::LoopxCliResult<loopx_contract::LoopxCliSettleTurnResult> {
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
    Ok(loopx_contract::LoopxCliSettleTurnResult {
        goal_id: request.goal_id.clone(),
        turn_id: request.turn_id.clone(),
        receipt_id,
        status: if !validation_succeeded {
            loopx_contract::LoopxCliSettlementStatus::NoDurableProgress
        } else if snapshot.state == loopx_contract::LoopxCliGoalState::Completed {
            loopx_contract::LoopxCliSettlementStatus::GoalCompleted
        } else {
            loopx_contract::LoopxCliSettlementStatus::AlreadySettled
        },
        before_revision: request.expected_durable_revision.clone(),
        after_revision: snapshot.durable_revision.clone(),
        validation_succeeded,
        scheduler_hint_ms: snapshot.scheduler_hint_ms,
        binding_todo_id: None,
    })
}

fn agent_shell_command(command: &VerifiedLoopxCommand) -> String {
    let display = command.executable.to_string_lossy().replace('"', "\\\"");
    let arguments = command
        .prefix_args
        .iter()
        .map(|argument| agent_shell_value(&argument.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ");
    if cfg!(windows) {
        let environment = command
            .environment
            .iter()
            .map(|(key, value)| {
                format!(
                    "$env:{}={}; ",
                    key.to_string_lossy(),
                    agent_shell_value(&value.to_string_lossy())
                )
            })
            .collect::<String>();
        format!(
            "{environment}& \"{display}\"{}",
            if arguments.is_empty() {
                String::new()
            } else {
                format!(" {arguments}")
            }
        )
    } else {
        let environment = command
            .environment
            .iter()
            .map(|(key, value)| {
                format!(
                    "{}={} ",
                    key.to_string_lossy(),
                    agent_shell_value(&value.to_string_lossy())
                )
            })
            .collect::<String>();
        format!(
            "{environment}'{}'{}",
            display.replace('\'', "'\\''"),
            if arguments.is_empty() {
                String::new()
            } else {
                format!(" {arguments}")
            }
        )
    }
}

fn agent_shell_value(value: &str) -> String {
    if cfg!(windows) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

/// Runs `issue-fix workflow-plan --fetch-candidate-evidence --goal-id`
/// for issue-kind items and returns the newer workflow-plan packet JSON.
/// Returns an empty string (with a warning) on any failure instead of
/// propagating the error.
async fn collect_candidate_evidence_best_effort(
    adapter: &LoopxCliProcessAdapter,
    request: &loopx_contract::LoopxCliCreateGoalRequest,
    operation_id: &str,
    observer: &PortProcessObserver<'_>,
) -> String {
    if !matches!(
        request.intake.item.kind,
        loopx_contract::LoopxItemKind::Issue
    ) {
        return String::new();
    }
    let result = run_port_command(
        adapter,
        &request.context,
        candidate_evidence_args(&request.goal_id, &request.intake.item.canonical_url()),
        observer,
    )
    .await;
    let output = match result {
        Ok(output) => output,
        Err(error) => {
            log::warn!(
                "LoopX candidate evidence collection skipped: operation_id={operation_id}, goal_id={}, error={}",
                request.goal_id,
                error
            );
            return String::new();
        }
    };
    if let Err(error) = require_payload_ok(&output.payload, operation_id) {
        log::warn!(
            "LoopX candidate evidence collection returned a non-ok payload: operation_id={operation_id}, goal_id={}, error={}",
            request.goal_id,
            error
        );
        return String::new();
    }
    if let Err(error) = require_schema(
        &output.payload,
        "issue_fix_workflow_plan_packet_v0",
        operation_id,
    ) {
        log::warn!(
            "LoopX candidate evidence packet failed the schema check: operation_id={operation_id}, goal_id={}, error={}",
            request.goal_id,
            error
        );
        return String::new();
    }
    match serde_json::to_string(&output.payload) {
        Ok(json) if json.len() <= MAX_INTAKE_PLAN_PACKET_BYTES => json,
        _ => String::new(),
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

async fn run_idempotent_global_command(
    adapter: &LoopxCliProcessAdapter,
    operation_id: &str,
    args: Vec<OsString>,
    missing_error_prefix: &str,
    deadline: Duration,
    observer: &dyn LoopxProcessObserver,
) -> loopx_contract::LoopxCliResult<Option<LoopxJsonOutput>> {
    match adapter
        .run_global_json_command(operation_id, args, deadline, observer)
        .await
    {
        Ok(output) => Ok(Some(output)),
        Err(error) => {
            if process_error_json(&error)
                .and_then(|payload| {
                    payload
                        .get("error")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .is_some_and(|message| message.starts_with(missing_error_prefix))
            {
                return Ok(None);
            }
            Err(map_port_error(error, operation_id))
        }
    }
}

fn process_error_json(error: &LoopxCliAdapterError) -> Option<Value> {
    let LoopxCliAdapterError::Process(LoopxProcessError::Exited { stdout_tail, .. }) = error else {
        return None;
    };
    serde_json::from_str(&stdout_tail.join("\n")).ok()
}

/// Maps the host-resolved remote state onto the state vocabulary LoopX's
/// metadata projection accepts (`open` | `closed` | `unknown`). LoopX has no
/// `merged` metadata state; a merged PR is already terminal, so `closed` is
/// the truthful projection.
fn metadata_state(state: loopx_contract::LoopxRemoteItemState) -> &'static str {
    use loopx_contract::LoopxRemoteItemState;
    match state {
        LoopxRemoteItemState::Open => "open",
        LoopxRemoteItemState::Closed | LoopxRemoteItemState::Merged => "closed",
        LoopxRemoteItemState::Unknown => "unknown",
    }
}

/// Builds the goal objective from the host-resolved title. The workflow-plan
/// packet carries no objective field, and the issue title is more meaningful
/// for goal surfaces than the bare canonical URL.
fn compact_objective(item: &loopx_contract::LoopxIssueKey, title: &str) -> String {
    const MAX_TITLE_CHARS: usize = 120;
    let title = title.trim();
    if title.is_empty() {
        return format!("Fix {}", item.canonical_url());
    }
    let mut bounded: String = title.chars().take(MAX_TITLE_CHARS).collect();
    if title.chars().count() > MAX_TITLE_CHARS {
        bounded.push_str("...");
    }
    format!("Fix #{}: {}", item.number, bounded)
}

fn plan_item_args(request: &loopx_contract::LoopxCliPlanItemRequest) -> Vec<OsString> {
    let kind = match request.item.kind {
        loopx_contract::LoopxItemKind::Issue => "issue",
        loopx_contract::LoopxItemKind::PullRequest => "pull_request",
    };
    // The intake adapter already resolved this metadata from the GitHub API;
    // never fabricate an open state or drop labels here, because the plan
    // packet feeds candidate admission, intake classification, and dedup.
    let metadata = serde_json::json!({
        "number": request.item.number,
        "state": metadata_state(request.state),
        "title": request.title,
        "labels": request.labels,
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

/// Source-backed candidate evidence collection for the freshly bound goal.
/// LoopX's admission contract requires this receipt before feasibility, and
/// `--goal-id` persists it into the goal-scoped candidate-preflight ledger.
fn candidate_evidence_args(goal_id: &str, canonical_url: &str) -> Vec<OsString> {
    [
        "issue-fix",
        "workflow-plan",
        "--url",
        canonical_url,
        "--fetch-candidate-evidence",
        "--goal-id",
        goal_id,
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

fn update_todo_text_args(
    goal_id: &str,
    todo_id: &str,
    text: &str,
    agent_id: &str,
) -> Vec<OsString> {
    [
        "todo",
        "update",
        "--goal-id",
        goal_id,
        "--todo-id",
        todo_id,
        "--text",
        text,
        "--agent-id",
        agent_id,
    ]
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

fn add_user_gate_args(
    request: &loopx_contract::LoopxCliCreateUserGateRequest,
) -> loopx_contract::LoopxCliResult<Vec<OsString>> {
    let operation_id = &request.context.call.operation_id;
    validate_nonempty("goal_id", &request.goal_id, operation_id)?;
    validate_nonempty("agent_id", &request.agent_id, operation_id)?;
    validate_nonempty("message", &request.message, operation_id)?;
    if request.message.len() > 700 {
        return Err(port_error(
            loopx_contract::LoopxCliErrorKind::InvalidInput,
            operation_id,
            "user gate message exceeds the 700-byte adapter limit",
            false,
        ));
    }
    if !is_public_token(&request.action_kind) {
        return Err(port_error(
            loopx_contract::LoopxCliErrorKind::InvalidInput,
            operation_id,
            "user gate action_kind must be a bounded public-safe token",
            false,
        ));
    }
    Ok([
        "todo",
        "add",
        "--goal-id",
        request.goal_id.as_str(),
        "--role",
        "user",
        "--text",
        request.message.as_str(),
        "--task-class",
        "user_gate",
        "--action-kind",
        request.action_kind.as_str(),
        "--agent-id",
        request.agent_id.as_str(),
        "--blocks-agent",
        request.agent_id.as_str(),
    ]
    .into_iter()
    .map(OsString::from)
    .collect())
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

fn settlement_history_args(goal_id: &str) -> Vec<OsString> {
    [
        "history",
        "--goal-id",
        goal_id,
        "--limit",
        SETTLEMENT_HISTORY_LIMIT,
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

/// Typed `goal-lifecycle stop` arguments for the pinned LoopX version. The
/// host calls this when a Goal has no runnable work left, so the registry
/// stops emitting heartbeat turns that would otherwise run forever.
fn goal_stop_args(goal_id: &str, reason: &str) -> Vec<OsString> {
    let bounded_reason: String = reason.chars().take(200).collect();
    [
        "goal-lifecycle".to_string(),
        "--goal-id".to_string(),
        goal_id.to_string(),
        "--operation".to_string(),
        "stop".to_string(),
        "--reason".to_string(),
        bounded_reason,
        "--execute".to_string(),
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn quota_spend_args(
    goal_id: &str,
    agent_id: &str,
    binding: &SettlementBinding,
    turn_id: &str,
) -> Vec<OsString> {
    let mut args = vec![
        "quota".to_string(),
        "spend-slot".to_string(),
        "--goal-id".to_string(),
        goal_id.to_string(),
        "--agent-id".to_string(),
        agent_id.to_string(),
    ];
    match binding {
        SettlementBinding::Todo { todo_id } => {
            args.extend(["--todo-id".to_string(), todo_id.clone()]);
        }
        SettlementBinding::AutonomousReplan { obligation_id } => {
            args.extend(["--replan-obligation-id".to_string(), obligation_id.clone()]);
        }
    }
    args.extend([
        "--turn-instance-id".to_string(),
        turn_id.to_string(),
        "--slots".to_string(),
        "1".to_string(),
        "--source".to_string(),
        "heartbeat".to_string(),
        "--execute".to_string(),
    ]);
    args.into_iter().map(OsString::from).collect()
}

fn require_quota_spend_response(
    payload: &Value,
    operation_id: &str,
) -> loopx_contract::LoopxCliResult<()> {
    if payload.get("mode").and_then(Value::as_str) != Some("spend-slot")
        || payload.get("dry_run").and_then(Value::as_bool) != Some(false)
    {
        return Err(port_error(
            loopx_contract::LoopxCliErrorKind::SchemaMismatch,
            operation_id,
            "LoopX quota finalization did not return an executed spend-slot result",
            false,
        ));
    }
    Ok(())
}

fn retire_global_goal_args(goal_id: &str) -> Vec<OsString> {
    ["retire-global-goal", "--goal-id", goal_id, "--execute"]
        .into_iter()
        .map(OsString::from)
        .collect()
}

fn archive_runtime_args(goal_id: &str) -> Vec<OsString> {
    ["archive-runtime", "--goal-id", goal_id, "--execute"]
        .into_iter()
        .map(OsString::from)
        .collect()
}

#[cfg(test)]
mod prompt_contract_tests {
    use super::{
        add_user_gate_args, candidate_evidence_args, compact_objective, loopx_contract,
        matching_durable_progress, metadata_state, open_viking_requires_display_language,
        open_viking_version_supported, parse_open_viking_version, plan_item_args,
        planned_settlement_token, project_goal_snapshot, quota_spend_args,
        replace_embedded_preflight_fence, replace_host_conflicting_lines,
        settlement_prompt_override, truncate_todo_text, turn_guard_args, LoopxProcessError,
        SettlementBinding, AGENT_CLI_USAGE_RULES,
    };
    use bitfun_product_domains::miniapp::loopx::{
        LoopxCliPlanItemRequest, LoopxIssueKey, LoopxItemKind, LoopxRemoteItemState,
        LoopxRepositoryKey,
    };
    use serde_json::json;
    use std::ffi::OsString;

    fn issue_key(number: u64) -> LoopxIssueKey {
        LoopxIssueKey {
            repository: LoopxRepositoryKey {
                host: "github.com".to_string(),
                owner: "owner".to_string(),
                repository: "repo".to_string(),
            },
            kind: LoopxItemKind::Issue,
            number,
        }
    }

    #[test]
    fn plan_item_args_pass_resolved_state_and_labels() {
        let request = LoopxCliPlanItemRequest {
            item: issue_key(42),
            title: "Search returns stale results".to_string(),
            state: LoopxRemoteItemState::Closed,
            labels: vec!["bug".to_string(), "needs-repro".to_string()],
            ..Default::default()
        };
        let metadata = plan_item_args(&request)
            .windows(2)
            .find(|pair| pair[0] == OsString::from("--metadata-json"))
            .map(|pair| pair[1].to_string_lossy().into_owned())
            .expect("metadata json argument");
        let payload: serde_json::Value = serde_json::from_str(&metadata).unwrap();
        assert_eq!(payload["state"], "closed");
        assert_eq!(payload["labels"], json!(["bug", "needs-repro"]));
        assert_eq!(payload["number"], 42);
        assert_eq!(payload["kind"], "issue");
    }

    #[test]
    fn metadata_state_maps_merged_and_unknown_truthfully() {
        assert_eq!(metadata_state(LoopxRemoteItemState::Open), "open");
        assert_eq!(metadata_state(LoopxRemoteItemState::Closed), "closed");
        // LoopX has no merged metadata state; merged is already terminal.
        assert_eq!(metadata_state(LoopxRemoteItemState::Merged), "closed");
        assert_eq!(metadata_state(LoopxRemoteItemState::Unknown), "unknown");
    }

    #[test]
    fn compact_objective_uses_the_resolved_title_and_bounds_length() {
        let item = issue_key(42);
        assert_eq!(
            compact_objective(&item, "  Crash on empty input  "),
            "Fix #42: Crash on empty input"
        );
        assert_eq!(
            compact_objective(&item, ""),
            format!("Fix {}", item.canonical_url())
        );
        let long = format!("{}-tail", "x".repeat(200));
        let objective = compact_objective(&item, &long);
        assert!(objective.starts_with("Fix #42: "));
        assert!(objective.chars().count() <= "Fix #42: ".len() + 123);
        assert!(objective.ends_with("..."));
    }

    #[test]
    fn candidate_evidence_args_bind_goal_and_evidence_fetch() {
        let args = candidate_evidence_args("goal-42", "https://github.com/owner/repo/issues/42");
        let joined = args
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            joined,
            "issue-fix workflow-plan --url https://github.com/owner/repo/issues/42 \
             --fetch-candidate-evidence --goal-id goal-42"
        );
        // The global --format flag is owned by run_json_command, never here.
        assert!(!joined.contains("--format"));
    }

    #[test]
    fn agent_cli_contract_names_only_real_issue_fix_subcommands_and_lifecycle_forms() {
        // LoopX v0.5.1 has no `issue-fix candidate-preflight` subcommand;
        // evidence collection runs through `workflow-plan --fetch-candidate-evidence`.
        assert!(!AGENT_CLI_USAGE_RULES.contains("`candidate-preflight`"));
        assert!(AGENT_CLI_USAGE_RULES.contains(
            "`issue-fix pr-lifecycle --url <github-pr-url> --goal-id <goal> --claimed-by <agent> --execute-transition`"
        ));
        assert!(AGENT_CLI_USAGE_RULES.contains("do not hand-create per-PR monitor todos"));
        assert!(AGENT_CLI_USAGE_RULES.contains(
            "a `continuous_monitor` additionally requires `--cadence <cadence>` plus exactly one terminal condition"
        ));
        assert!(AGENT_CLI_USAGE_RULES.contains("--resume-when <condition>"));
    }

    #[test]
    fn agent_cli_contract_pins_global_format_and_scoped_agent_options() {
        assert!(AGENT_CLI_USAGE_RULES.contains("before the subcommand"));
        assert!(AGENT_CLI_USAGE_RULES.contains("whose `--help` output lists it"));
        assert!(AGENT_CLI_USAGE_RULES.contains("do not repeat the rejected form"));
    }

    #[test]
    fn agent_cli_contract_teaches_the_settlement_binding_chain() {
        // The guard form carries the todo binding that LoopX compares against
        // the turn-scoped `refresh-state` writeback.
        assert!(AGENT_CLI_USAGE_RULES.contains(
            "`quota should-run --goal-id <goal> --todo-id <todo> --agent-id <agent> --runtime-profile outer_controller --turn-instance-id \"<turn>\"`"
        ));
        assert!(AGENT_CLI_USAGE_RULES.contains("--todo-id <todo> --turn-instance-id \"<turn>\" --delivery-outcome <outcome_progress|primary_goal_outcome>"));
        assert!(AGENT_CLI_USAGE_RULES.contains("--no-follow-up"));
        assert!(AGENT_CLI_USAGE_RULES.contains("--next-agent-todo"));
        assert!(AGENT_CLI_USAGE_RULES
            .contains("`goal-lifecycle` and all `quota spend`/`void` commands are host-owned"));
    }

    #[test]
    fn todo_text_is_bounded_for_the_turn_envelope_budget() {
        let long = format!(
            "[P0] {} detailed acceptance criteria and file paths",
            "x".repeat(400)
        );
        let bounded = truncate_todo_text(&long);
        assert!(bounded.chars().count() <= 203);
        assert!(bounded.ends_with("..."));
        assert_eq!(truncate_todo_text("short text"), "short text");
    }

    #[test]
    fn turn_guard_args_bind_the_settlement_todo() {
        let bound = turn_guard_args(
            "goal-1",
            "agent-1",
            Some(&SettlementBinding::Todo {
                todo_id: "todo-1".to_string(),
            }),
            "turn-1",
        );
        let bound_text = bound
            .iter()
            .map(|argument| argument.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            bound_text,
            "quota should-run --goal-id goal-1 --todo-id todo-1 --agent-id agent-1 --runtime-profile outer_controller --turn-instance-id turn-1"
        );

        let unbound = turn_guard_args("goal-1", "agent-1", None, "turn-1");
        let unbound_text = unbound
            .iter()
            .map(|argument| argument.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!unbound_text.contains("--todo-id"));
    }

    #[test]
    fn embedded_codex_preflight_fence_is_replaced_with_the_bound_guard() {
        let body = "Preflight/guard; `LOOPX_TURN=<current_time_iso>`; reuse:\n\n```bash\nexport PATH=\"$HOME/.local/bin:$PATH\"\ninstall_script=\"$HOME/loopx/scripts/install-local.sh\"\nif ! command -v loopx >/dev/null 2>&1; then\n  echo \"loopx is not on PATH\" >&2\n  exit 1\nfi\nloopx doctor >/dev/null\nloopx --format json --registry \"$HOME/.codex/loopx/registry.global.json\" quota should-run --goal-id goal-1 --agent-id agent-1 --runtime-profile outer_controller --turn-instance-id \"${LOOPX_TURN:?}\"\n```\n\nPreflight fail: quiet; no work/spend.\n";
        let replaced =
            replace_embedded_preflight_fence(body, "PREFIX quota should-run --todo-id todo-1");
        assert!(replaced.contains("PREFIX quota should-run --todo-id todo-1"));
        assert!(!replaced.contains("install-local.sh"));
        assert!(!replaced.contains("loopx doctor"));
        assert!(replaced.contains("Preflight fail: quiet; no work/spend."));
        assert_eq!(replaced.matches("```bash").count(), 1);

        // Later bash fences (progress refresh examples) must survive.
        assert!(replace_embedded_preflight_fence(
            "intro\n```bash\nloopx refresh-state --goal-id goal-1\n```\ntail",
            "PREFIX guard"
        )
        .contains("loopx refresh-state --goal-id goal-1"));
        // A body without the Codex fence is returned unchanged.
        assert_eq!(
            replace_embedded_preflight_fence("no fence here", "PREFIX guard"),
            "no fence here"
        );
    }

    #[test]
    fn host_conflicting_body_lines_are_neutralized() {
        let body = "9. Refresh; spend once:\n\n```bash\nloopx refresh-state --goal-id goal-1\nloopx quota spend-slot --goal-id goal-1 --slots 1 --execute\n```\n\nDo not ask for permissions when the current Codex session is already trusted.\n";
        let replaced = replace_host_conflicting_lines(body);
        assert!(replaced.contains("# quota spend-slot is executed by the BitFun host"));
        assert!(!replaced.contains("spend-slot --goal-id"));
        assert!(replaced.contains("The BitFun session is not pre-trusted"));
        assert!(!replaced.contains("Codex session is already trusted"));
        assert!(replaced.contains("loopx refresh-state --goal-id goal-1"));
    }

    #[test]
    fn open_viking_version_probe_accepts_the_pinned_compatibility_floor() {
        assert_eq!(
            parse_open_viking_version("openviking 0.4.9\n"),
            Some("0.4.9".to_string())
        );
        assert!(open_viking_version_supported("0.4.9"));
        assert!(open_viking_version_supported("0.4.17"));
        assert!(!open_viking_version_supported("0.4.8"));
    }

    #[test]
    fn open_viking_language_bootstrap_only_matches_the_explicit_first_run_gate() {
        let missing_language = LoopxProcessError::Exited {
            code: Some(2),
            stdout_tail: Vec::new(),
            stderr_tail: vec![
                "OpenViking needs a display language before running commands.".to_string(),
            ],
        };
        assert!(open_viking_requires_display_language(&missing_language));

        let service_failure = LoopxProcessError::Exited {
            code: Some(2),
            stdout_tail: Vec::new(),
            stderr_tail: vec!["OpenViking service is unavailable".to_string()],
        };
        assert!(!open_viking_requires_display_language(&service_failure));
    }

    #[test]
    fn owner_review_gate_is_translated_to_a_scoped_typed_todo() {
        let request = loopx_contract::LoopxCliCreateUserGateRequest {
            context: loopx_contract::LoopxCliGoalContext {
                call: loopx_contract::LoopxCliCallContext {
                    operation_id: "review-1".to_string(),
                    deadline_at: None,
                },
                ..loopx_contract::LoopxCliGoalContext::default()
            },
            goal_id: "goal-1".to_string(),
            agent_id: "agent-1".to_string(),
            message: "Approve another bounded segment or reject to stop.".to_string(),
            action_kind: "autonomous_budget_review".to_string(),
        };
        let args = add_user_gate_args(&request)
            .expect("typed gate args")
            .into_iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--task-class" && pair[1] == "user_gate"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--blocks-agent" && pair[1] == "agent-1"));
        assert!(args
            .windows(2)
            .any(|pair| { pair[0] == "--action-kind" && pair[1] == "autonomous_budget_review" }));
    }

    #[test]
    fn selected_todo_builds_the_exact_settlement_effect_identity() {
        let token = planned_settlement_token(
            &json!({
                "route": {"selected_todo": {"todo_id": "todo-1"}},
                "transaction": {"turn_key": "sha256:legacy"}
            }),
            "goal-1",
            "agent-1",
            "turn-1",
            "build-turn",
        )
        .unwrap();

        assert_eq!(token, "goal-1:agent-1:todo-1:turn-1");
    }

    #[test]
    fn replan_route_builds_the_autonomous_replan_effect_identity() {
        let token = planned_settlement_token(
            &json!({
                "route": {
                    "kind": "replan_required",
                    "selected_todo": {"todo_id": "todo-1"}
                },
                "turn_envelope": {
                    "replan_action_packet": {"obligation_id": "replan-1"}
                },
                "transaction": {"turn_key": "sha256:legacy"}
            }),
            "goal-1",
            "agent-1",
            "turn-1",
            "build-turn",
        )
        .unwrap();

        assert_eq!(token, "goal-1:agent-1:autonomous_replan:replan-1:turn-1");
    }

    #[test]
    fn durable_settlement_requires_matching_advanced_progress_and_quota_spend() {
        let effect_id = "goal-1:agent-1:todo-1:turn-1";
        let identity = json!({
            "schema_version": "quota_settlement_identity_v0",
            "effect_id": effect_id,
            "goal_id": "goal-1",
            "agent_id": "agent-1",
            "todo_id": "todo-1",
            "turn_instance_id": "turn-1"
        });
        let payload = json!({
            "goals": [{
                "id": "goal-1",
                "latest_runs": [
                    {
                        "goal_id": "goal-1",
                        "agent_id": "agent-1",
                        "todo_id": "todo-1",
                        "turn_instance_id": "turn-1",
                        "classification": "validated_progress",
                        "progress_observation": {
                            "schema_version": "typed_progress_observation_v0",
                            "result_class": "advanced",
                            "work_item_id": "todo-1"
                        },
                        "settlement_identity": identity.clone()
                    },
                    {
                        "goal_id": "goal-1",
                        "agent_id": "agent-1",
                        "todo_id": "todo-1",
                        "turn_instance_id": "turn-1",
                        "classification": "quota_slot_spent",
                        "settlement_identity": identity
                    }
                ]
            }]
        });

        let evidence = matching_durable_progress(
            &payload,
            "goal-1",
            "agent-1",
            "turn-1",
            effect_id,
            "settle-turn",
        )
        .unwrap()
        .expect("settlement evidence");
        assert_eq!(evidence.effect_id, effect_id);
        assert!(evidence.quota_spent);
    }

    #[test]
    fn durable_progress_without_quota_exposes_the_binding_for_host_finalization() {
        let effect_id = "goal-1:agent-1:todo-1:turn-1";
        let payload = json!({
            "goals": [{
                "id": "goal-1",
                "latest_runs": [{
                    "goal_id": "goal-1",
                    "agent_id": "agent-1",
                    "todo_id": "todo-1",
                    "turn_instance_id": "turn-1",
                    "classification": "validated_progress",
                    "progress_observation": {
                        "schema_version": "typed_progress_observation_v0",
                        "result_class": "advanced",
                        "work_item_id": "todo-1"
                    },
                    "settlement_identity": {
                        "schema_version": "quota_settlement_identity_v0",
                        "effect_id": effect_id,
                        "goal_id": "goal-1",
                        "agent_id": "agent-1",
                        "todo_id": "todo-1",
                        "turn_instance_id": "turn-1"
                    }
                }]
            }]
        });

        let evidence = matching_durable_progress(
            &payload,
            "goal-1",
            "agent-1",
            "turn-1",
            effect_id,
            "settle-turn",
        )
        .unwrap()
        .expect("validated progress");
        assert!(!evidence.quota_spent);
        assert_eq!(
            evidence.binding,
            SettlementBinding::Todo {
                todo_id: "todo-1".to_string()
            }
        );
    }

    #[test]
    fn autonomous_replan_progress_uses_the_v1_settlement_binding() {
        let effect_id = "goal-1:agent-1:autonomous_replan:replan-1:turn-1";
        let payload = json!({
            "goals": [{
                "id": "goal-1",
                "latest_runs": [{
                    "goal_id": "goal-1",
                    "agent_id": "agent-1",
                    "turn_instance_id": "turn-1",
                    "replan_obligation_id": "replan-1",
                    "classification": "state_projection_repair",
                    "delivery_outcome": "outcome_progress",
                    "progress_observation": {
                        "schema_version": "typed_progress_observation_v0",
                        "result_class": "advanced"
                    },
                    "settlement_identity": {
                        "schema_version": "quota_settlement_identity_v1",
                        "effect_id": effect_id,
                        "goal_id": "goal-1",
                        "agent_id": "agent-1",
                        "turn_instance_id": "turn-1",
                        "binding_kind": "autonomous_replan",
                        "binding_id": "replan-1",
                        "replan_obligation_id": "replan-1"
                    }
                }]
            }]
        });

        let evidence = matching_durable_progress(
            &payload,
            "goal-1",
            "agent-1",
            "turn-1",
            "sha256:legacy",
            "settle-turn",
        )
        .unwrap()
        .expect("replan progress");
        assert_eq!(evidence.effect_id, effect_id);
        assert_eq!(
            evidence.binding,
            SettlementBinding::AutonomousReplan {
                obligation_id: "replan-1".to_string()
            }
        );
        assert!(!evidence.quota_spent);
    }

    #[test]
    fn quota_finalization_uses_the_exact_typed_binding() {
        let args = quota_spend_args(
            "goal-1",
            "agent-1",
            &SettlementBinding::AutonomousReplan {
                obligation_id: "replan-1".to_string(),
            },
            "turn-1",
        );
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(args
            .windows(2)
            .any(|pair| pair == ["--replan-obligation-id", "replan-1"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--turn-instance-id", "turn-1"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--source", "heartbeat"]));
        assert!(!args.iter().any(|arg| arg == "--todo-id"));
        assert!(args.iter().any(|arg| arg == "--execute"));
    }

    #[test]
    fn prompt_override_keeps_writeback_exact_and_spend_host_owned() {
        let prompt = settlement_prompt_override(
            "loopx-prefix",
            "goal-1",
            "agent-1",
            "turn-1",
            Some(&SettlementBinding::Todo {
                todo_id: "todo-1".to_string(),
            }),
        );

        assert!(prompt.contains("--todo-id todo-1 --turn-instance-id turn-1"));
        assert!(prompt.contains("The host owns quota accounting"));
        assert!(prompt.contains("Do not run `quota spend-slot`"));
        assert!(prompt.contains("The turn guard was already executed and bound by the host"));
        assert!(prompt.contains("English ASCII"));
    }

    #[test]
    fn terminal_no_followup_is_a_completed_goal() {
        let snapshot = project_goal_snapshot(
            "goal-1",
            &json!({
                "ok": true,
                "schema_version": "loopx_turn_plan_v0",
                "turn_envelope": {
                    "should_run": false,
                    "state": "terminal_no_followup",
                    "effective_action": "terminal_no_followup",
                    "action_signature": {
                        "source_hash": "sha256:terminal"
                    }
                }
            }),
            "inspect-goal",
        )
        .unwrap();

        assert_eq!(
            snapshot.run_decision,
            loopx_contract::LoopxCliRunDecision::Complete
        );
        assert_eq!(snapshot.state, loopx_contract::LoopxCliGoalState::Completed);
    }

    #[test]
    fn durable_progress_on_an_unplanned_todo_still_settles_the_turn() {
        let planned_token = "goal-1:agent-1:todo-planned:turn-1";
        let actual_effect = "goal-1:agent-1:todo-actual:turn-1";
        let identity = json!({
            "schema_version": "quota_settlement_identity_v0",
            "effect_id": actual_effect,
            "goal_id": "goal-1",
            "agent_id": "agent-1",
            "todo_id": "todo-actual",
            "turn_instance_id": "turn-1"
        });
        let payload = json!({
            "goals": [{
                "id": "goal-1",
                "latest_runs": [
                    {
                        "goal_id": "goal-1",
                        "agent_id": "agent-1",
                        "todo_id": "todo-actual",
                        "turn_instance_id": "turn-1",
                        "classification": "validated_progress",
                        "progress_observation": {
                            "schema_version": "typed_progress_observation_v0",
                            "result_class": "advanced",
                            "work_item_id": "todo-actual"
                        },
                        "settlement_identity": identity.clone()
                    },
                    {
                        "goal_id": "goal-1",
                        "agent_id": "agent-1",
                        "todo_id": "todo-actual",
                        "turn_instance_id": "turn-1",
                        "classification": "quota_slot_spent",
                        "settlement_identity": identity
                    }
                ]
            }]
        });

        let evidence = matching_durable_progress(
            &payload,
            "goal-1",
            "agent-1",
            "turn-1",
            planned_token,
            "settle-turn",
        )
        .unwrap()
        .expect("turn-scoped evidence on a different todo");
        assert!(evidence.quota_spent);
    }

    #[test]
    fn legacy_validated_progress_uses_accountable_delivery_outcome() {
        let effect_id = "goal-1:agent-1:todo-1:turn-1";
        let identity = json!({
            "schema_version": "quota_settlement_identity_v0",
            "effect_id": effect_id,
            "goal_id": "goal-1",
            "agent_id": "agent-1",
            "todo_id": "todo-1",
            "turn_instance_id": "turn-1"
        });
        let payload = json!({
            "goals": [{
                "id": "goal-1",
                "latest_runs": [
                    {
                        "goal_id": "goal-1",
                        "agent_id": "agent-1",
                        "todo_id": "todo-1",
                        "turn_instance_id": "turn-1",
                        "classification": "validated_progress",
                        "delivery_outcome": "outcome_progress",
                        "settlement_identity": identity.clone()
                    },
                    {
                        "goal_id": "goal-1",
                        "agent_id": "agent-1",
                        "todo_id": "todo-1",
                        "turn_instance_id": "turn-1",
                        "classification": "quota_slot_spent",
                        "settlement_identity": identity
                    }
                ]
            }]
        });

        let evidence = matching_durable_progress(
            &payload,
            "goal-1",
            "agent-1",
            "turn-1",
            effect_id,
            "settle-turn",
        )
        .unwrap()
        .expect("legacy settlement evidence");
        assert!(evidence.quota_spent);
    }
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
    let control_status = payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let operator_gate_notify = matches!(
        effective_action,
        "operator_gate" | "operator_gate_notify" | "waiting_for_user"
    ) || matches!(state_text, "operator_gate" | "operator_gate_notify")
        || control_status == "operator_gate_notify";
    let run_decision = if user_action_required || operator_gate_notify {
        loopx_contract::LoopxCliRunDecision::WaitingForUser
    } else if should_run {
        loopx_contract::LoopxCliRunDecision::RunNow
    } else if effective_action == "terminal_no_followup"
        || matches!(
            state_text,
            "completed" | "complete" | "closed" | "terminal_no_followup"
        )
    {
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
        open_agent_todos: Vec::new(),
        envelope_over_budget: envelope
            .pointer("/compaction/within_budget")
            .and_then(Value::as_bool)
            == Some(false),
    })
}

/// Projects a bounded list of open agent todos from a `todo list` payload.
/// Lenient by design: an unexpected item shape is skipped, and an empty or
/// missing `todos` array yields an empty projection. Text is truncated so the
/// host prompt injection stays bounded regardless of registry contents.
fn project_open_agent_todos(
    payload: &Value,
    operation_id: &str,
) -> Vec<loopx_contract::LoopxCliTodoSummary> {
    let Some(todos) = payload.get("todos").and_then(Value::as_array) else {
        return Vec::new();
    };
    let _ = operation_id;
    todos
        .iter()
        .filter(|todo| {
            let status = todo
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default();
            todo.get("role").and_then(Value::as_str) == Some("agent")
                && todo.get("done").and_then(Value::as_bool) != Some(true)
                && !matches!(
                    status,
                    "completed" | "closed" | "done" | "archived" | "cancelled"
                )
        })
        .take(MAX_OPEN_AGENT_TODO_SUMMARIES)
        .filter_map(|todo| {
            let todo_id = todo.get("todo_id").and_then(Value::as_str)?;
            let text = todo.get("text").and_then(Value::as_str).unwrap_or_default();
            if todo_id.trim().is_empty() {
                return None;
            }
            Some(loopx_contract::LoopxCliTodoSummary {
                todo_id: todo_id.to_string(),
                status: todo
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                text: truncate_message(text),
            })
        })
        .collect()
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

/// Registry todo texts are bounded: the complete LoopX turn envelope — which
/// embeds the selected todo text and action portfolio — must fit LoopX's fixed
/// compaction budget, and long workflow-planner texts have been observed
/// pushing a Goal past it (route contract_error, unplannable until shrunk).
fn truncate_todo_text(text: &str) -> String {
    const MAX_TODO_TEXT_CHARS: usize = 200;
    let trimmed = text.trim();
    if trimmed.chars().count() <= MAX_TODO_TEXT_CHARS {
        return trimmed.to_string();
    }
    let mut bounded: String = trimmed.chars().take(MAX_TODO_TEXT_CHARS).collect();
    bounded.push_str("...");
    bounded
}

fn truncate_message(message: &str) -> String {
    message.chars().take(500).collect()
}

fn python_version_supported(output: &str) -> bool {
    let Some(version) = output.split_whitespace().find(|part| {
        part.chars()
            .next()
            .map(|character| character.is_ascii_digit())
            .unwrap_or(false)
    }) else {
        return false;
    };
    let mut components = version.split('.');
    let major = components
        .next()
        .and_then(|value| value.parse::<u32>().ok());
    let minor = components
        .next()
        .and_then(|value| value.parse::<u32>().ok());
    matches!((major, minor), (Some(major), Some(minor)) if major > 3 || (major == 3 && minor >= 11))
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

fn open_viking_requires_display_language(error: &LoopxProcessError) -> bool {
    let LoopxProcessError::Exited {
        stdout_tail,
        stderr_tail,
        ..
    } = error
    else {
        return false;
    };
    stdout_tail
        .iter()
        .chain(stderr_tail)
        .any(|line| line.contains("needs a display language before running commands"))
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
    prefix_args: Vec<OsString>,
    environment: BTreeMap<OsString, OsString>,
    managed_source_dir: Option<PathBuf>,
    source: LoopxCommandSource,
    bundle_manifest_schema: Option<u32>,
    sha256: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ManagedLoopxSourceManifest {
    schema_version: u32,
    source_repository: String,
    source_tag: String,
    source_commit: String,
    loopx_version: String,
}

#[derive(Debug, Clone)]
struct OpenVikingCommand {
    executable: PathBuf,
    version: Option<String>,
    managed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ManagedOpenVikingManifest {
    schema_version: u32,
    source_repository: String,
    source_tag: String,
    source_commit: String,
    open_viking_version: String,
    loopx_provider_commit: String,
    binary_sha256: String,
}

fn open_viking_binary(root: &Path) -> PathBuf {
    root.join("bin")
        .join(if cfg!(windows) { "ov.exe" } else { "ov" })
}

fn loopx_extension_venv(root: &Path) -> PathBuf {
    root.join("loopx-extension")
}

fn loopx_extension_bin_dir(root: &Path) -> PathBuf {
    loopx_extension_venv(root).join(if cfg!(windows) { "Scripts" } else { "bin" })
}

fn loopx_extension_python(root: &Path) -> PathBuf {
    loopx_extension_bin_dir(root).join(if cfg!(windows) {
        "python.exe"
    } else {
        "python"
    })
}

fn loopx_extension_entrypoint(root: &Path) -> PathBuf {
    loopx_extension_bin_dir(root).join(if cfg!(windows) {
        "loopx-openviking-semantic-preference.exe"
    } else {
        "loopx-openviking-semantic-preference"
    })
}

fn parse_open_viking_version(output: &str) -> Option<String> {
    output.split_whitespace().find_map(|token| {
        let candidate = token
            .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '.');
        let candidate = candidate.strip_prefix('v').unwrap_or(candidate);
        let components = candidate.split('.').collect::<Vec<_>>();
        if components.len() >= 3
            && components
                .iter()
                .take(3)
                .all(|component| component.parse::<u32>().is_ok())
        {
            Some(components[..3].join("."))
        } else {
            None
        }
    })
}

fn open_viking_version_supported(version: &str) -> bool {
    fn tuple(version: &str) -> Option<(u32, u32, u32)> {
        let mut components = version.split('.');
        Some((
            components.next()?.parse().ok()?,
            components.next()?.parse().ok()?,
            components.next()?.parse().ok()?,
        ))
    }
    matches!(
        (tuple(version), tuple(OPEN_VIKING_PINNED_VERSION)),
        (Some(actual), Some(minimum)) if actual >= minimum
    )
}

fn verify_managed_open_viking_manifest(
    manifest: &ManagedOpenVikingManifest,
) -> Result<(), LoopxCliAdapterError> {
    let digest_valid = manifest.binary_sha256.len() == 64
        && manifest
            .binary_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit());
    if manifest.schema_version != MANAGED_OPEN_VIKING_MANIFEST_SCHEMA
        || manifest.source_repository != OPEN_VIKING_SOURCE_REPOSITORY
        || manifest.source_tag != OPEN_VIKING_PINNED_VERSION_TAG
        || manifest.source_commit != OPEN_VIKING_PINNED_SOURCE_COMMIT
        || manifest.open_viking_version != OPEN_VIKING_PINNED_VERSION
        || manifest.loopx_provider_commit != LOOPX_PINNED_SOURCE_COMMIT
        || !digest_valid
    {
        return Err(LoopxCliAdapterError::Manifest {
            message: "managed OpenViking manifest does not match the pinned source release"
                .to_string(),
        });
    }
    Ok(())
}

fn verify_managed_source_manifest(
    manifest: &ManagedLoopxSourceManifest,
) -> Result<(), LoopxCliAdapterError> {
    if manifest.schema_version != MANAGED_SOURCE_MANIFEST_SCHEMA
        || manifest.source_repository != LOOPX_SOURCE_REPOSITORY
        || manifest.source_tag != LOOPX_PINNED_VERSION_TAG
        || manifest.source_commit != LOOPX_PINNED_SOURCE_COMMIT
        || manifest.loopx_version != LOOPX_PINNED_VERSION
    {
        return Err(LoopxCliAdapterError::Manifest {
            message: "managed LoopX source manifest does not match the pinned release".to_string(),
        });
    }
    Ok(())
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
