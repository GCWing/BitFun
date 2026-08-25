//! Local isolated workspace preparation for LoopX tasks.

use super::loopx_cli::{
    LoopxCommandPlan, LoopxProcessError, LoopxProcessObserver, LoopxProcessRunner,
    NoopLoopxProcessObserver, SystemLoopxProcessRunner,
};
use bitfun_product_domains::miniapp::loopx as loopx_contract;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use url::Url;

const WORKSPACE_MARKER_SCHEMA: u32 = 1;
const WORKSPACE_MARKER_NAME: &str = "bitfun-loopx-workspace.json";

#[derive(Debug, Clone)]
pub struct LoopxWorkspaceServiceConfig {
    pub root_dir: PathBuf,
    pub git_executable: PathBuf,
    pub clone_deadline: Duration,
    pub command_deadline: Duration,
    pub terminate_grace: Duration,
}

impl LoopxWorkspaceServiceConfig {
    pub fn new(root_dir: impl Into<PathBuf>, git_executable: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
            git_executable: git_executable.into(),
            clone_deadline: Duration::from_secs(300),
            command_deadline: Duration::from_secs(30),
            terminate_grace: Duration::from_secs(2),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopxWorkspaceLayout {
    pub repository_identity: String,
    pub repository_hash: String,
    pub task_hash: String,
    pub worktree_path: PathBuf,
    pub registry_path: PathBuf,
    pub branch_name: String,
    pub clone_url: String,
}

pub fn plan_workspace_layout(
    root_dir: &Path,
    task_id: &str,
    item: &loopx_contract::LoopxIssueKey,
) -> loopx_contract::LoopxHostResult<LoopxWorkspaceLayout> {
    validate_task_and_item("workspace-plan", task_id, item)?;
    if !root_dir.is_absolute() {
        return Err(host_error(
            loopx_contract::LoopxHostPortErrorKind::InvalidInput,
            "workspace-plan",
            "LoopX workspace root must be absolute",
            false,
        ));
    }
    let repository_identity = item.repository.canonical_id().to_lowercase();
    let repository_hash = sha256_prefix(repository_identity.as_bytes(), 20);
    let task_hash = sha256_prefix(task_id.as_bytes(), 20);
    let worktree_path = root_dir.join(&repository_hash).join(&task_hash);
    Ok(LoopxWorkspaceLayout {
        repository_identity,
        repository_hash,
        task_hash: task_hash.clone(),
        registry_path: worktree_path.join(".loopx").join("registry.json"),
        worktree_path,
        branch_name: format!("bitfun-loopx/{task_hash}"),
        clone_url: format!(
            "https://github.com/{}/{}.git",
            item.repository.owner, item.repository.repository
        ),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopxGitCommandPlan {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub environment: BTreeMap<OsString, OsString>,
    pub current_dir: Option<PathBuf>,
}

pub fn plan_git_clone_command(
    git_executable: &Path,
    layout: &LoopxWorkspaceLayout,
) -> LoopxGitCommandPlan {
    LoopxGitCommandPlan {
        executable: git_executable.to_path_buf(),
        args: vec![
            OsString::from("clone"),
            OsString::from("--no-checkout"),
            OsString::from("--origin"),
            OsString::from("origin"),
            OsString::from("--"),
            OsString::from(&layout.clone_url),
            layout.worktree_path.as_os_str().to_owned(),
        ],
        environment: git_noninteractive_environment(),
        current_dir: None,
    }
}

pub fn canonical_github_remote(remote: &str) -> Option<String> {
    let trimmed = remote.trim().trim_end_matches('/');
    if let Some(path) = trimmed.strip_prefix("git@github.com:") {
        return canonical_github_path(path);
    }
    let parsed = Url::parse(trimmed).ok()?;
    if !parsed.host_str()?.eq_ignore_ascii_case("github.com") {
        return None;
    }
    canonical_github_path(parsed.path().trim_start_matches('/'))
}

pub struct LoopxWorkspaceService {
    config: LoopxWorkspaceServiceConfig,
    runner: Arc<dyn LoopxProcessRunner>,
    observer: Arc<dyn LoopxProcessObserver>,
    mutation_lock: Mutex<()>,
    running: Arc<StdMutex<HashMap<String, CancellationToken>>>,
}

impl std::fmt::Debug for LoopxWorkspaceService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LoopxWorkspaceService")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl LoopxWorkspaceService {
    pub fn new(config: LoopxWorkspaceServiceConfig) -> Self {
        Self::with_runner(
            config,
            Arc::new(SystemLoopxProcessRunner),
            Arc::new(NoopLoopxProcessObserver),
        )
    }

    pub fn with_runner(
        config: LoopxWorkspaceServiceConfig,
        runner: Arc<dyn LoopxProcessRunner>,
        observer: Arc<dyn LoopxProcessObserver>,
    ) -> Self {
        Self {
            config,
            runner,
            observer,
            mutation_lock: Mutex::new(()),
            running: Arc::new(StdMutex::new(HashMap::new())),
        }
    }

    async fn prepare_inner(
        &self,
        request: &loopx_contract::LoopxWorkspacePrepareRequest,
        cancellation: CancellationToken,
    ) -> loopx_contract::LoopxHostResult<loopx_contract::LoopxWorkspacePrepareResult> {
        validate_task_and_item(&request.operation_id, &request.task_id, &request.item)?;
        tokio::fs::create_dir_all(&self.config.root_dir)
            .await
            .map_err(|error| {
                host_error(
                    loopx_contract::LoopxHostPortErrorKind::Io,
                    &request.operation_id,
                    format!("failed to create LoopX workspace root: {error}"),
                    true,
                )
            })?;
        let canonical_root = tokio::fs::canonicalize(&self.config.root_dir)
            .await
            .map_err(|error| {
                host_error(
                    loopx_contract::LoopxHostPortErrorKind::Io,
                    &request.operation_id,
                    format!("failed to resolve LoopX workspace root: {error}"),
                    true,
                )
            })?;
        let layout = plan_workspace_layout(&canonical_root, &request.task_id, &request.item)?;
        let repository_parent = layout
            .worktree_path
            .parent()
            .expect("hashed workspace layout always has a parent");
        tokio::fs::create_dir_all(repository_parent)
            .await
            .map_err(|error| {
                host_error(
                    loopx_contract::LoopxHostPortErrorKind::Io,
                    &request.operation_id,
                    format!("failed to create repository workspace directory: {error}"),
                    true,
                )
            })?;
        ensure_path_boundary(&canonical_root, repository_parent, &request.operation_id).await?;

        if tokio::fs::try_exists(&layout.worktree_path)
            .await
            .map_err(|error| {
                host_error(
                    loopx_contract::LoopxHostPortErrorKind::Io,
                    &request.operation_id,
                    error.to_string(),
                    true,
                )
            })?
        {
            ensure_path_boundary(
                &canonical_root,
                &layout.worktree_path,
                &request.operation_id,
            )
            .await?;
            let marker = read_workspace_marker(&layout, &request.operation_id).await?;
            validate_marker(&marker, &layout, &request.operation_id)?;
            self.verify_remote(&layout, &request.operation_id, cancellation)
                .await?;
            return Ok(workspace_result(&layout, true));
        }

        let clone = plan_git_clone_command(&self.config.git_executable, &layout);
        self.run_git(
            &request.operation_id,
            clone,
            self.config.clone_deadline,
            cancellation.clone(),
        )
        .await?;
        self.verify_remote(&layout, &request.operation_id, cancellation.clone())
            .await?;
        self.run_git(
            &request.operation_id,
            git_checkout_plan(&self.config.git_executable, &layout),
            self.config.command_deadline,
            cancellation,
        )
        .await?;
        write_workspace_marker(&layout, &request.task_id, &request.operation_id).await?;
        Ok(workspace_result(&layout, false))
    }

    async fn verify_inner(
        &self,
        request: &loopx_contract::LoopxWorkspaceVerifyRequest,
        cancellation: CancellationToken,
    ) -> loopx_contract::LoopxHostResult<loopx_contract::LoopxWorkspaceVerifyResult> {
        validate_task_and_item(&request.operation_id, &request.task_id, &request.item)?;
        let canonical_root = match tokio::fs::canonicalize(&self.config.root_dir).await {
            Ok(path) => path,
            Err(error) => {
                return Ok(invalid_workspace(format!(
                    "workspace root is unavailable: {error}"
                )))
            }
        };
        let layout = plan_workspace_layout(&canonical_root, &request.task_id, &request.item)?;
        if Path::new(&request.worktree_path) != layout.worktree_path
            || Path::new(&request.registry_path) != layout.registry_path
        {
            return Ok(invalid_workspace(
                "workspace paths do not match the canonical task layout",
            ));
        }
        if ensure_path_boundary(
            &canonical_root,
            &layout.worktree_path,
            &request.operation_id,
        )
        .await
        .is_err()
        {
            return Ok(invalid_workspace(
                "workspace resolves outside the managed root",
            ));
        }
        let marker = match read_workspace_marker(&layout, &request.operation_id).await {
            Ok(marker) => marker,
            Err(error) => return Ok(invalid_workspace(error.message)),
        };
        if let Err(error) = validate_marker(&marker, &layout, &request.operation_id) {
            return Ok(invalid_workspace(error.message));
        }
        if let Err(error) = self
            .verify_remote(&layout, &request.operation_id, cancellation)
            .await
        {
            if matches!(
                error.kind,
                loopx_contract::LoopxHostPortErrorKind::Cancelled
                    | loopx_contract::LoopxHostPortErrorKind::Timeout
            ) {
                return Err(error);
            }
            return Ok(invalid_workspace(error.message));
        }
        Ok(loopx_contract::LoopxWorkspaceVerifyResult {
            valid: true,
            repository: Some(request.item.repository.clone()),
            message: None,
        })
    }

    async fn verify_remote(
        &self,
        layout: &LoopxWorkspaceLayout,
        operation_id: &str,
        cancellation: CancellationToken,
    ) -> loopx_contract::LoopxHostResult<()> {
        let output = self
            .run_git(
                operation_id,
                git_remote_plan(&self.config.git_executable, layout),
                self.config.command_deadline,
                cancellation,
            )
            .await?;
        let actual = canonical_github_remote(output.stdout.trim()).ok_or_else(|| {
            host_error(
                loopx_contract::LoopxHostPortErrorKind::Conflict,
                operation_id,
                "workspace origin is not a canonical GitHub repository",
                false,
            )
        })?;
        if actual != layout.repository_identity {
            return Err(host_error(
                loopx_contract::LoopxHostPortErrorKind::Conflict,
                operation_id,
                format!(
                    "workspace origin mismatch: expected {}, got {actual}; existing data was preserved",
                    layout.repository_identity
                ),
                false,
            ));
        }
        Ok(())
    }

    async fn run_git(
        &self,
        operation_id: &str,
        plan: LoopxGitCommandPlan,
        deadline: Duration,
        cancellation: CancellationToken,
    ) -> loopx_contract::LoopxHostResult<super::loopx_cli::LoopxProcessOutput> {
        self.runner
            .run(
                LoopxCommandPlan {
                    operation_id: operation_id.to_string(),
                    executable: plan.executable,
                    args: plan.args,
                    current_dir: plan.current_dir,
                    environment: plan.environment,
                    deadline,
                    terminate_grace: self.config.terminate_grace,
                },
                cancellation,
                self.observer.as_ref(),
            )
            .await
            .map_err(|error| map_process_error(error, operation_id))
    }

    fn register_operation(
        &self,
        operation_id: &str,
    ) -> loopx_contract::LoopxHostResult<(CancellationToken, WorkspaceOperationRegistration)> {
        if operation_id.trim().is_empty() {
            return Err(host_error(
                loopx_contract::LoopxHostPortErrorKind::InvalidInput,
                operation_id,
                "operation_id is required",
                false,
            ));
        }
        let mut running = self
            .running
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if running.contains_key(operation_id) {
            return Err(host_error(
                loopx_contract::LoopxHostPortErrorKind::Conflict,
                operation_id,
                "workspace operation is already running",
                true,
            ));
        }
        let cancellation = CancellationToken::new();
        running.insert(operation_id.to_string(), cancellation.clone());
        Ok((
            cancellation,
            WorkspaceOperationRegistration {
                operation_id: operation_id.to_string(),
                running: self.running.clone(),
            },
        ))
    }
}

impl loopx_contract::LoopxWorkspacePort for LoopxWorkspaceService {
    fn prepare(
        &self,
        request: loopx_contract::LoopxWorkspacePrepareRequest,
    ) -> loopx_contract::LoopxHostFuture<'_, loopx_contract::LoopxWorkspacePrepareResult> {
        Box::pin(async move {
            let (cancellation, _registration) = self.register_operation(&request.operation_id)?;
            let _mutation = self.mutation_lock.lock().await;
            self.prepare_inner(&request, cancellation).await
        })
    }

    fn verify(
        &self,
        request: loopx_contract::LoopxWorkspaceVerifyRequest,
    ) -> loopx_contract::LoopxHostFuture<'_, loopx_contract::LoopxWorkspaceVerifyResult> {
        Box::pin(async move {
            let (cancellation, _registration) = self.register_operation(&request.operation_id)?;
            self.verify_inner(&request, cancellation).await
        })
    }

    fn cancel(
        &self,
        request: loopx_contract::LoopxWorkspaceCancelRequest,
    ) -> loopx_contract::LoopxHostFuture<'_, loopx_contract::LoopxWorkspaceCancelResult> {
        Box::pin(async move {
            let running = self
                .running
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let cancelled = if let Some(cancellation) = running.get(&request.target_operation_id) {
                cancellation.cancel();
                true
            } else {
                false
            };
            Ok(loopx_contract::LoopxWorkspaceCancelResult {
                target_operation_id: request.target_operation_id,
                cancelled,
            })
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct LoopxWorkspaceMarker {
    schema_version: u32,
    repository_identity: String,
    repository_hash: String,
    task_id_hash: String,
    branch_name: String,
}

struct WorkspaceOperationRegistration {
    operation_id: String,
    running: Arc<StdMutex<HashMap<String, CancellationToken>>>,
}

impl Drop for WorkspaceOperationRegistration {
    fn drop(&mut self) {
        self.running
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .remove(&self.operation_id);
    }
}

fn git_noninteractive_environment() -> BTreeMap<OsString, OsString> {
    BTreeMap::from([
        (OsString::from("GIT_TERMINAL_PROMPT"), OsString::from("0")),
        (OsString::from("GCM_INTERACTIVE"), OsString::from("Never")),
    ])
}

fn git_remote_plan(git: &Path, layout: &LoopxWorkspaceLayout) -> LoopxGitCommandPlan {
    LoopxGitCommandPlan {
        executable: git.to_path_buf(),
        args: vec![
            OsString::from("-C"),
            layout.worktree_path.as_os_str().to_owned(),
            OsString::from("config"),
            OsString::from("--get"),
            OsString::from("remote.origin.url"),
        ],
        environment: git_noninteractive_environment(),
        current_dir: None,
    }
}

fn git_checkout_plan(git: &Path, layout: &LoopxWorkspaceLayout) -> LoopxGitCommandPlan {
    LoopxGitCommandPlan {
        executable: git.to_path_buf(),
        args: vec![
            OsString::from("-C"),
            layout.worktree_path.as_os_str().to_owned(),
            OsString::from("checkout"),
            OsString::from("-b"),
            OsString::from(&layout.branch_name),
        ],
        environment: git_noninteractive_environment(),
        current_dir: None,
    }
}

async fn read_workspace_marker(
    layout: &LoopxWorkspaceLayout,
    operation_id: &str,
) -> loopx_contract::LoopxHostResult<LoopxWorkspaceMarker> {
    let marker_path = layout
        .worktree_path
        .join(".git")
        .join(WORKSPACE_MARKER_NAME);
    let raw = tokio::fs::read(&marker_path).await.map_err(|error| {
        host_error(
            loopx_contract::LoopxHostPortErrorKind::Conflict,
            operation_id,
            format!(
                "existing workspace has no valid BitFun ownership marker: {error}; existing data was preserved"
            ),
            false,
        )
    })?;
    serde_json::from_slice(&raw).map_err(|error| {
        host_error(
            loopx_contract::LoopxHostPortErrorKind::Conflict,
            operation_id,
            format!(
                "existing workspace ownership marker is invalid: {error}; existing data was preserved"
            ),
            false,
        )
    })
}

async fn write_workspace_marker(
    layout: &LoopxWorkspaceLayout,
    task_id: &str,
    operation_id: &str,
) -> loopx_contract::LoopxHostResult<()> {
    let marker = LoopxWorkspaceMarker {
        schema_version: WORKSPACE_MARKER_SCHEMA,
        repository_identity: layout.repository_identity.clone(),
        repository_hash: layout.repository_hash.clone(),
        task_id_hash: sha256_prefix(task_id.as_bytes(), 20),
        branch_name: layout.branch_name.clone(),
    };
    let git_dir = layout.worktree_path.join(".git");
    let marker_path = git_dir.join(WORKSPACE_MARKER_NAME);
    let temporary_path = git_dir.join(format!("{WORKSPACE_MARKER_NAME}.tmp"));
    let encoded = serde_json::to_vec_pretty(&marker).map_err(|error| {
        host_error(
            loopx_contract::LoopxHostPortErrorKind::Io,
            operation_id,
            error.to_string(),
            false,
        )
    })?;
    tokio::fs::write(&temporary_path, encoded)
        .await
        .map_err(|error| {
            host_error(
                loopx_contract::LoopxHostPortErrorKind::Io,
                operation_id,
                format!("failed to write workspace ownership marker: {error}"),
                true,
            )
        })?;
    tokio::fs::rename(&temporary_path, &marker_path)
        .await
        .map_err(|error| {
            host_error(
                loopx_contract::LoopxHostPortErrorKind::Io,
                operation_id,
                format!("failed to publish workspace ownership marker: {error}"),
                true,
            )
        })?;
    Ok(())
}

fn validate_marker(
    marker: &LoopxWorkspaceMarker,
    layout: &LoopxWorkspaceLayout,
    operation_id: &str,
) -> loopx_contract::LoopxHostResult<()> {
    if marker.schema_version != WORKSPACE_MARKER_SCHEMA
        || marker.repository_identity != layout.repository_identity
        || marker.repository_hash != layout.repository_hash
        || marker.task_id_hash != layout.task_hash
        || marker.branch_name != layout.branch_name
    {
        return Err(host_error(
            loopx_contract::LoopxHostPortErrorKind::Conflict,
            operation_id,
            "existing workspace ownership does not match the requested task; existing data was preserved",
            false,
        ));
    }
    Ok(())
}

async fn ensure_path_boundary(
    canonical_root: &Path,
    path: &Path,
    operation_id: &str,
) -> loopx_contract::LoopxHostResult<()> {
    let canonical = tokio::fs::canonicalize(path).await.map_err(|error| {
        host_error(
            loopx_contract::LoopxHostPortErrorKind::Io,
            operation_id,
            format!("failed to resolve workspace path: {error}"),
            true,
        )
    })?;
    if !canonical.starts_with(canonical_root) {
        return Err(host_error(
            loopx_contract::LoopxHostPortErrorKind::Conflict,
            operation_id,
            "workspace path resolves outside the managed root",
            false,
        ));
    }
    Ok(())
}

fn validate_task_and_item(
    operation_id: &str,
    task_id: &str,
    item: &loopx_contract::LoopxIssueKey,
) -> loopx_contract::LoopxHostResult<()> {
    if operation_id.trim().is_empty() || task_id.trim().is_empty() {
        return Err(host_error(
            loopx_contract::LoopxHostPortErrorKind::InvalidInput,
            operation_id,
            "operation_id and task_id are required",
            false,
        ));
    }
    if !item.repository.host.eq_ignore_ascii_case("github.com")
        || !is_github_slug(&item.repository.owner)
        || !is_github_slug(&item.repository.repository)
        || item.number == 0
    {
        return Err(host_error(
            loopx_contract::LoopxHostPortErrorKind::InvalidInput,
            operation_id,
            "workspace preparation requires a canonical GitHub item",
            false,
        ));
    }
    Ok(())
}

fn is_github_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn canonical_github_path(path: &str) -> Option<String> {
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut segments = path.split('/');
    let owner = segments.next()?;
    let repository = segments.next()?;
    if segments.next().is_some() || !is_github_slug(owner) || !is_github_slug(repository) {
        return None;
    }
    Some(format!("github.com/{owner}/{repository}").to_lowercase())
}

fn sha256_prefix(value: &[u8], length: usize) -> String {
    let digest = hex::encode(Sha256::digest(value));
    digest[..length].to_string()
}

fn workspace_result(
    layout: &LoopxWorkspaceLayout,
    reused: bool,
) -> loopx_contract::LoopxWorkspacePrepareResult {
    loopx_contract::LoopxWorkspacePrepareResult {
        worktree_path: layout.worktree_path.to_string_lossy().into_owned(),
        registry_path: layout.registry_path.to_string_lossy().into_owned(),
        reused,
        repository_verified: true,
    }
}

fn invalid_workspace(message: impl Into<String>) -> loopx_contract::LoopxWorkspaceVerifyResult {
    loopx_contract::LoopxWorkspaceVerifyResult {
        valid: false,
        repository: None,
        message: Some(message.into()),
    }
}

fn map_process_error(
    error: LoopxProcessError,
    operation_id: &str,
) -> loopx_contract::LoopxHostPortError {
    let (kind, retryable) = match &error {
        LoopxProcessError::Cancelled { .. } => {
            (loopx_contract::LoopxHostPortErrorKind::Cancelled, true)
        }
        LoopxProcessError::Timeout { .. } => {
            (loopx_contract::LoopxHostPortErrorKind::Timeout, true)
        }
        LoopxProcessError::Io { .. } => (loopx_contract::LoopxHostPortErrorKind::Io, true),
        _ => (loopx_contract::LoopxHostPortErrorKind::Backend, true),
    };
    host_error(
        kind,
        operation_id,
        format!("workspace git command failed: {error}"),
        retryable,
    )
}

fn host_error(
    kind: loopx_contract::LoopxHostPortErrorKind,
    operation_id: &str,
    message: impl Into<String>,
    retryable: bool,
) -> loopx_contract::LoopxHostPortError {
    loopx_contract::LoopxHostPortError {
        kind,
        message: message.into(),
        operation_id: Some(operation_id.to_string()),
        retryable,
    }
}
