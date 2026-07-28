//! Product-owned managed Git worktree lifecycle.
//!
//! Concrete Git and filesystem operations remain in `services-integrations`.
//! This module owns registry reconciliation, idempotency, lifecycle policy,
//! session association, and safe-removal decisions.

use crate::infrastructure::events::{emit_global_event, BackendEvent};
use crate::infrastructure::{get_path_manager_arc, PathManager};
use crate::service::config::GlobalConfigManager;
use crate::service::git::{GitError, GitService, GitWorktreeInfo};
use crate::service::workspace::{
    get_global_workspace_service, WorkspaceActivityMode, WorkspaceCreateOptions,
};
use crate::service::workspace_runtime::get_workspace_runtime_service_arc;
use bitfun_core_types::{
    SessionExecutionTarget, SessionExecutionTargetKind, WorktreeError, WorktreeErrorCode,
    WorktreeLifecycle, WorktreeSessionSummary, WorktreeSettings, WorktreeSummary,
};
use bitfun_services_core::json_store::JsonFileStore;
use bitfun_services_core::session::{SessionMetadata, SessionMetadataStore, SessionStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

const WORKTREE_REGISTRY_VERSION: u32 = 1;
const REGISTRY_FILE_NAME: &str = "worktrees.json";

mod session_binding;

pub use session_binding::{WorktreeSessionBindingRequest, WorktreeSessionBindingResult};

static REPOSITORY_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeListRequest {
    pub project_workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeCreateRequest {
    pub request_id: String,
    pub project_workspace_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_ref: Option<String>,
    #[serde(default)]
    pub copy_local_changes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeCreateResult {
    pub worktree: WorktreeSummary,
    pub execution_target: SessionExecutionTarget,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeCreateBranchRequest {
    pub request_id: String,
    pub project_workspace_path: String,
    pub worktree_id: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreePromoteRequest {
    pub request_id: String,
    pub project_workspace_path: String,
    pub worktree_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRemoveRequest {
    pub request_id: String,
    pub project_workspace_path: String,
    pub worktree_id: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRecreateRequest {
    pub request_id: String,
    pub project_workspace_path: String,
    pub worktree_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeMutationResult {
    pub worktree: WorktreeSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRemoveResult {
    pub worktree_id: String,
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorktreeRegistry {
    version: u32,
    project_workspace_path: String,
    #[serde(default)]
    worktrees: Vec<RegisteredWorktree>,
    #[serde(default)]
    receipts: HashMap<String, WorktreeOperationReceipt>,
}

impl WorktreeRegistry {
    fn new(project_workspace_path: &Path) -> Self {
        Self {
            version: WORKTREE_REGISTRY_VERSION,
            project_workspace_path: path_string(project_workspace_path),
            worktrees: Vec::new(),
            receipts: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisteredWorktree {
    worktree_id: String,
    path: String,
    base_ref: Option<String>,
    base_commit: String,
    branch: Option<String>,
    lifecycle: WorktreeLifecycle,
    created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum WorktreeOperationReceipt {
    Create {
        worktree_id: String,
        source_workspace_path: String,
        base_ref: String,
        copy_local_changes: bool,
    },
    CreateBranch {
        worktree_id: String,
        branch: String,
    },
    Promote {
        worktree_id: String,
    },
    Remove {
        worktree_id: String,
        #[serde(default)]
        force: bool,
    },
    Recreate {
        worktree_id: String,
    },
}

impl WorktreeOperationReceipt {
    fn worktree_id(&self) -> &str {
        match self {
            Self::Create { worktree_id, .. }
            | Self::CreateBranch { worktree_id, .. }
            | Self::Promote { worktree_id }
            | Self::Remove { worktree_id, .. }
            | Self::Recreate { worktree_id } => worktree_id,
        }
    }
}

struct RepositoryContext {
    project_workspace_path: PathBuf,
    common_git_dir: PathBuf,
    registry_path: PathBuf,
    settings: WorktreeSettings,
}

pub struct WorktreeService;

impl WorktreeService {
    /// Stable session identity for an idempotent worktree-session request.
    pub fn session_id_for_request(request_id: &str) -> Result<String, WorktreeError> {
        validate_request_id(request_id)?;
        Ok(format!("worktree-session-{}", short_hash(request_id)))
    }

    /// Compensates a just-created worktree when atomic session creation fails.
    /// This is intentionally not exposed through Tauri or Agent tools.
    pub async fn rollback_created(
        project_workspace_path: &str,
        worktree_id: &str,
    ) -> Result<(), WorktreeError> {
        let context = Self::repository_context(Path::new(project_workspace_path)).await?;
        let lock = repository_lock(&context.common_git_dir);
        let _guard = lock.lock().await;
        let _process_guard = Self::acquire_repository_process_lock(&context).await?;
        let mut registry = Self::load_registry(&context).await?;
        let record = registry
            .worktrees
            .iter()
            .find(|record| record.worktree_id == worktree_id)
            .cloned()
            .ok_or_else(|| {
                error(
                    WorktreeErrorCode::WorktreeNotFound,
                    "Managed worktree was not found during rollback",
                )
            })?;
        if Path::new(&record.path).exists() {
            GitService::remove_worktree(&context.project_workspace_path, &record.path, true)
                .await
                .map_err(map_git_error)?;
        } else {
            GitService::prune_worktrees(&context.project_workspace_path)
                .await
                .map_err(map_git_error)?;
        }
        let mut cleanup_issues = Vec::new();
        if let Some(workspace_service) = get_global_workspace_service() {
            if let Some(workspace) = workspace_service
                .get_workspace_by_path(Path::new(&record.path))
                .await
            {
                if let Err(remove_error) = workspace_service.remove_workspace(&workspace.id).await {
                    cleanup_issues.push(format!(
                        "workspace registration could not be removed: {remove_error}"
                    ));
                }
            }
        }
        registry
            .worktrees
            .retain(|registered| registered.worktree_id != worktree_id);
        registry
            .receipts
            .retain(|_, receipt| receipt.worktree_id() != worktree_id);
        if let Err(registry_error) = Self::save_registry(&context, &registry).await {
            cleanup_issues.push(format!("registry could not be updated: {registry_error}"));
        }
        notify_changed(&context.project_workspace_path).await;
        if cleanup_issues.is_empty() {
            Ok(())
        } else {
            Err(WorktreeError {
                code: WorktreeErrorCode::RollbackIncomplete,
                message: cleanup_issues.join("; "),
                recovery_path: Some(record.path),
            })
        }
    }

    /// User-level worktree defaults (root directory, branch prefix, copy policy).
    pub async fn settings() -> WorktreeSettings {
        load_settings().await
    }

    pub async fn list(request: WorktreeListRequest) -> Result<Vec<WorktreeSummary>, WorktreeError> {
        let context = Self::repository_context(Path::new(&request.project_workspace_path)).await?;
        let lock = repository_lock(&context.common_git_dir);
        let _guard = lock.lock().await;
        let _process_guard = Self::acquire_repository_process_lock(&context).await?;
        let mut registry = Self::load_registry(&context).await?;
        let (summaries, changed) = Self::reconcile(&context, &mut registry).await?;
        if changed {
            Self::save_registry(&context, &registry).await?;
        }
        Ok(summaries)
    }

    pub async fn create(
        request: WorktreeCreateRequest,
    ) -> Result<WorktreeCreateResult, WorktreeError> {
        validate_request_id(&request.request_id)?;
        let context = Self::repository_context(Path::new(&request.project_workspace_path)).await?;
        let lock = repository_lock(&context.common_git_dir);
        let _guard = lock.lock().await;
        let _process_guard = Self::acquire_repository_process_lock(&context).await?;
        let mut registry = Self::load_registry(&context).await?;
        let source_path = request
            .source_workspace_path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| context.project_workspace_path.clone());
        let source_workspace_path = normalized_lookup_path(&source_path);
        let base_ref = request.base_ref.as_deref().unwrap_or("HEAD").trim();

        if let Some(receipt) = registry.receipts.get(&request.request_id).cloned() {
            return match receipt {
                WorktreeOperationReceipt::Create {
                    worktree_id,
                    source_workspace_path: receipt_source,
                    base_ref: receipt_base_ref,
                    copy_local_changes,
                } if receipt_source == source_workspace_path
                    && receipt_base_ref == base_ref
                    && copy_local_changes == request.copy_local_changes =>
                {
                    Self::create_result_for_id(&context, &mut registry, &worktree_id, false).await
                }
                _ => Err(error(
                    WorktreeErrorCode::RequestConflict,
                    "The requestId was already used with different worktree creation parameters",
                )),
            };
        }

        let source_repository = GitService::resolve_worktree_repository(&source_path)
            .await
            .map_err(map_git_error)?;
        if source_repository.common_git_dir != context.common_git_dir {
            return Err(error(
                WorktreeErrorCode::InvalidPath,
                "The source workspace does not belong to the selected project repository",
            ));
        }

        let base_commit = GitService::resolve_revision(&source_path, base_ref)
            .await
            .map_err(|git_error| map_base_ref_error(git_error, base_ref))?;
        if request.copy_local_changes {
            let source_head = GitService::resolve_revision(&source_path, "HEAD")
                .await
                .map_err(map_git_error)?;
            if source_head != base_commit {
                return Err(error(
                    WorktreeErrorCode::CopyConflict,
                    "Local changes can only be copied when the selected base resolves to source HEAD",
                ));
            }
        }

        let worktree_id = Uuid::new_v4().simple().to_string();
        let repository_id = repository_id(&context.common_git_dir);
        let target_path =
            managed_target_path(&context.settings, &repository_id, &worktree_id).await?;

        GitService::add_detached_worktree(
            &context.project_workspace_path,
            &target_path,
            &base_commit,
        )
        .await
        .map_err(map_git_error)?;

        if request.copy_local_changes {
            if let Err(copy_error) =
                GitService::copy_local_changes(&source_path, &target_path).await
            {
                return Err(Self::rollback_new_worktree(
                    &context,
                    &target_path,
                    map_copy_error(copy_error),
                )
                .await);
            }
        }

        let tracked_workspace_id = if let Some(workspace_service) = get_global_workspace_service() {
            match workspace_service
                .track_workspace_activity(
                    target_path.clone(),
                    WorkspaceCreateOptions::default(),
                    WorkspaceActivityMode::RefreshMetadata,
                )
                .await
            {
                Ok(workspace) => Some(workspace.id),
                Err(track_error) => {
                    return Err(Self::rollback_new_worktree(
                        &context,
                        &target_path,
                        error(
                            WorktreeErrorCode::IoFailed,
                            format!("Failed to register the worktree workspace: {track_error}"),
                        ),
                    )
                    .await);
                }
            }
        } else {
            None
        };

        registry.worktrees.push(RegisteredWorktree {
            worktree_id: worktree_id.clone(),
            path: path_string(&target_path),
            base_ref: Some(base_ref.to_string()),
            base_commit: base_commit.clone(),
            branch: None,
            lifecycle: WorktreeLifecycle::Managed,
            created_at_ms: current_unix_ms(),
        });
        registry.receipts.insert(
            request.request_id,
            WorktreeOperationReceipt::Create {
                worktree_id: worktree_id.clone(),
                source_workspace_path,
                base_ref: base_ref.to_string(),
                copy_local_changes: request.copy_local_changes,
            },
        );
        if let Err(registry_error) = Self::save_registry(&context, &registry).await {
            return Err(Self::rollback_new_worktree_with_workspace(
                &context,
                &target_path,
                tracked_workspace_id.as_deref(),
                registry_error,
            )
            .await);
        }

        let result =
            Self::create_result_for_id(&context, &mut registry, &worktree_id, true).await?;
        notify_changed(&context.project_workspace_path).await;
        Ok(result)
    }

    pub async fn create_branch(
        request: WorktreeCreateBranchRequest,
    ) -> Result<WorktreeMutationResult, WorktreeError> {
        validate_request_id(&request.request_id)?;
        let branch = request.branch.trim().to_string();
        if branch.is_empty() {
            return Err(error(
                WorktreeErrorCode::InvalidPath,
                "Branch name cannot be empty",
            ));
        }
        let context = Self::repository_context(Path::new(&request.project_workspace_path)).await?;
        let lock = repository_lock(&context.common_git_dir);
        let _guard = lock.lock().await;
        let _process_guard = Self::acquire_repository_process_lock(&context).await?;
        let mut registry = Self::load_registry(&context).await?;

        if let Some(receipt) = registry.receipts.get(&request.request_id).cloned() {
            return match receipt {
                WorktreeOperationReceipt::CreateBranch {
                    worktree_id,
                    branch: receipt_branch,
                } if worktree_id == request.worktree_id && receipt_branch == branch => {
                    Self::mutation_result_for_id(&context, &mut registry, &worktree_id).await
                }
                _ => Err(error(
                    WorktreeErrorCode::RequestConflict,
                    "The requestId was already used with different branch parameters",
                )),
            };
        }

        let record = registry
            .worktrees
            .iter_mut()
            .find(|record| record.worktree_id == request.worktree_id)
            .ok_or_else(|| {
                error(
                    WorktreeErrorCode::WorktreeNotFound,
                    "Managed worktree was not found",
                )
            })?;
        if !Path::new(&record.path).is_dir() {
            return Err(error(
                WorktreeErrorCode::WorktreeNotFound,
                "Worktree directory is missing; recreate it before creating a branch",
            ));
        }
        let info = GitService::create_worktree_branch(&record.path, &branch)
            .await
            .map_err(map_branch_error)?;
        record.branch = info.branch;
        registry.receipts.insert(
            request.request_id,
            WorktreeOperationReceipt::CreateBranch {
                worktree_id: request.worktree_id.clone(),
                branch,
            },
        );
        Self::save_registry(&context, &registry).await?;
        let result =
            Self::mutation_result_for_id(&context, &mut registry, &request.worktree_id).await?;
        notify_changed(&context.project_workspace_path).await;
        Ok(result)
    }

    pub async fn promote(
        request: WorktreePromoteRequest,
    ) -> Result<WorktreeMutationResult, WorktreeError> {
        validate_request_id(&request.request_id)?;
        let context = Self::repository_context(Path::new(&request.project_workspace_path)).await?;
        let lock = repository_lock(&context.common_git_dir);
        let _guard = lock.lock().await;
        let _process_guard = Self::acquire_repository_process_lock(&context).await?;
        let mut registry = Self::load_registry(&context).await?;
        if let Some(receipt) = registry.receipts.get(&request.request_id).cloned() {
            return match receipt {
                WorktreeOperationReceipt::Promote { worktree_id }
                    if worktree_id == request.worktree_id =>
                {
                    Self::mutation_result_for_id(&context, &mut registry, &worktree_id).await
                }
                _ => Err(error(
                    WorktreeErrorCode::RequestConflict,
                    "The requestId was already used with different promote parameters",
                )),
            };
        }
        let record = registry
            .worktrees
            .iter_mut()
            .find(|record| record.worktree_id == request.worktree_id)
            .ok_or_else(|| {
                error(
                    WorktreeErrorCode::WorktreeNotFound,
                    "Managed worktree was not found",
                )
            })?;
        if record.lifecycle != WorktreeLifecycle::Managed {
            return Err(error(
                WorktreeErrorCode::InvalidPath,
                "Only managed worktrees can be kept as permanent worktrees",
            ));
        }
        record.lifecycle = WorktreeLifecycle::Permanent;
        registry.receipts.insert(
            request.request_id,
            WorktreeOperationReceipt::Promote {
                worktree_id: request.worktree_id.clone(),
            },
        );
        Self::save_registry(&context, &registry).await?;
        let result =
            Self::mutation_result_for_id(&context, &mut registry, &request.worktree_id).await?;
        notify_changed(&context.project_workspace_path).await;
        Ok(result)
    }

    pub async fn remove(
        request: WorktreeRemoveRequest,
    ) -> Result<WorktreeRemoveResult, WorktreeError> {
        validate_request_id(&request.request_id)?;
        let context = Self::repository_context(Path::new(&request.project_workspace_path)).await?;
        let lock = repository_lock(&context.common_git_dir);
        let _guard = lock.lock().await;
        let _process_guard = Self::acquire_repository_process_lock(&context).await?;
        let mut registry = Self::load_registry(&context).await?;
        if let Some(receipt) = registry.receipts.get(&request.request_id) {
            return match receipt {
                WorktreeOperationReceipt::Remove { worktree_id, force }
                    if worktree_id == &request.worktree_id && *force == request.force =>
                {
                    Ok(WorktreeRemoveResult {
                        worktree_id: worktree_id.clone(),
                        removed: true,
                    })
                }
                _ => Err(error(
                    WorktreeErrorCode::RequestConflict,
                    "The requestId was already used with different remove parameters",
                )),
            };
        }

        let (summaries, _) = Self::reconcile(&context, &mut registry).await?;
        let summary = summaries
            .iter()
            .find(|summary| summary.worktree_id == request.worktree_id)
            .ok_or_else(|| {
                error(
                    WorktreeErrorCode::WorktreeNotFound,
                    "Managed worktree was not found",
                )
            })?;
        validate_removal(summary, request.force)?;

        GitService::remove_worktree(
            &context.project_workspace_path,
            &summary.path,
            request.force,
        )
        .await
        .map_err(map_git_error)?;
        let mut cleanup_issues = Vec::new();
        if let Some(workspace_service) = get_global_workspace_service() {
            if let Some(workspace) = workspace_service
                .get_workspace_by_path(Path::new(&summary.path))
                .await
            {
                if let Err(remove_error) = workspace_service.remove_workspace(&workspace.id).await {
                    cleanup_issues.push(format!(
                        "workspace registration could not be removed: {remove_error}"
                    ));
                }
            }
        }
        registry
            .worktrees
            .retain(|record| record.worktree_id != request.worktree_id);
        registry.receipts.insert(
            request.request_id,
            WorktreeOperationReceipt::Remove {
                worktree_id: request.worktree_id.clone(),
                force: request.force,
            },
        );
        if let Err(registry_error) = Self::save_registry(&context, &registry).await {
            cleanup_issues.push(format!("registry could not be updated: {registry_error}"));
        }
        notify_changed(&context.project_workspace_path).await;
        if !cleanup_issues.is_empty() {
            return Err(WorktreeError {
                code: WorktreeErrorCode::RollbackIncomplete,
                message: format!(
                    "Worktree was removed, but cleanup did not complete: {}",
                    cleanup_issues.join("; ")
                ),
                recovery_path: Some(summary.path.clone()),
            });
        }
        Ok(WorktreeRemoveResult {
            worktree_id: request.worktree_id,
            removed: true,
        })
    }

    pub async fn recreate(
        request: WorktreeRecreateRequest,
    ) -> Result<WorktreeMutationResult, WorktreeError> {
        validate_request_id(&request.request_id)?;
        let context = Self::repository_context(Path::new(&request.project_workspace_path)).await?;
        let lock = repository_lock(&context.common_git_dir);
        let _guard = lock.lock().await;
        let _process_guard = Self::acquire_repository_process_lock(&context).await?;
        let mut registry = Self::load_registry(&context).await?;
        if let Some(receipt) = registry.receipts.get(&request.request_id).cloned() {
            return match receipt {
                WorktreeOperationReceipt::Recreate { worktree_id }
                    if worktree_id == request.worktree_id =>
                {
                    Self::mutation_result_for_id(&context, &mut registry, &worktree_id).await
                }
                _ => Err(error(
                    WorktreeErrorCode::RequestConflict,
                    "The requestId was already used with different recreate parameters",
                )),
            };
        }

        let record = registry
            .worktrees
            .iter()
            .find(|record| record.worktree_id == request.worktree_id)
            .cloned()
            .ok_or_else(|| {
                error(
                    WorktreeErrorCode::WorktreeNotFound,
                    "Managed worktree was not found",
                )
            })?;
        if Path::new(&record.path).exists() {
            return Err(error(
                WorktreeErrorCode::InvalidPath,
                "Worktree directory already exists",
            ));
        }
        GitService::prune_worktrees(&context.project_workspace_path)
            .await
            .map_err(map_git_error)?;
        GitService::add_detached_worktree(
            &context.project_workspace_path,
            &record.path,
            &record.base_commit,
        )
        .await
        .map_err(map_git_error)?;
        if let Some(branch) = record.branch.as_deref() {
            if let Err(branch_error) =
                GitService::attach_worktree_branch(&record.path, branch).await
            {
                return Err(Self::rollback_new_worktree(
                    &context,
                    Path::new(&record.path),
                    map_branch_error(branch_error),
                )
                .await);
            }
        }
        registry.receipts.insert(
            request.request_id,
            WorktreeOperationReceipt::Recreate {
                worktree_id: request.worktree_id.clone(),
            },
        );
        Self::save_registry(&context, &registry).await?;
        let result =
            Self::mutation_result_for_id(&context, &mut registry, &request.worktree_id).await?;
        notify_changed(&context.project_workspace_path).await;
        Ok(result)
    }

    async fn repository_context(project_path: &Path) -> Result<RepositoryContext, WorktreeError> {
        if !project_path.is_dir() {
            return Err(error(
                WorktreeErrorCode::InvalidPath,
                "Project workspace path does not exist",
            ));
        }
        let repository_info = GitService::resolve_worktree_repository(project_path)
            .await
            .map_err(map_git_error)?;
        let worktrees = GitService::list_worktrees(project_path)
            .await
            .map_err(map_git_error)?;
        let project_workspace_path = worktrees
            .iter()
            .find(|worktree| worktree.is_main)
            .map(|worktree| PathBuf::from(&worktree.path))
            .unwrap_or_else(|| repository_info.query_path.clone());
        let project_workspace_path =
            std::fs::canonicalize(&project_workspace_path).unwrap_or(project_workspace_path);
        let runtime = get_workspace_runtime_service_arc()
            .ensure_local_workspace_runtime(&project_workspace_path)
            .await
            .map_err(|runtime_error| {
                error(
                    WorktreeErrorCode::IoFailed,
                    format!("Failed to initialize project runtime: {runtime_error}"),
                )
            })?;
        Ok(RepositoryContext {
            project_workspace_path,
            common_git_dir: repository_info.common_git_dir,
            registry_path: runtime.context.config_dir.join(REGISTRY_FILE_NAME),
            settings: load_settings().await,
        })
    }

    async fn load_registry(context: &RepositoryContext) -> Result<WorktreeRegistry, WorktreeError> {
        let registry = JsonFileStore
            .read_optional(&context.registry_path)
            .await
            .map_err(|store_error| {
                error(
                    WorktreeErrorCode::IoFailed,
                    format!("Failed to read worktree registry: {store_error}"),
                )
            })?
            .unwrap_or_else(|| WorktreeRegistry::new(&context.project_workspace_path));
        if registry.version != WORKTREE_REGISTRY_VERSION {
            return Err(error(
                WorktreeErrorCode::IoFailed,
                format!(
                    "Unsupported worktree registry version: {}",
                    registry.version
                ),
            ));
        }
        Ok(registry)
    }

    async fn acquire_repository_process_lock(
        context: &RepositoryContext,
    ) -> Result<bitfun_services_core::json_store::JsonFileCrossProcessLock, WorktreeError> {
        JsonFileStore
            .acquire_cross_process_lock(&context.registry_path)
            .await
            .map_err(|lock_error| {
                error(
                    WorktreeErrorCode::IoFailed,
                    format!("Failed to lock the worktree registry: {lock_error}"),
                )
            })
    }

    async fn save_registry(
        context: &RepositoryContext,
        registry: &WorktreeRegistry,
    ) -> Result<(), WorktreeError> {
        JsonFileStore
            .write_atomic_strict(&context.registry_path, registry)
            .await
            .map_err(|store_error| {
                error(
                    WorktreeErrorCode::IoFailed,
                    format!("Failed to persist worktree registry: {store_error}"),
                )
            })
    }

    async fn reconcile(
        context: &RepositoryContext,
        registry: &mut WorktreeRegistry,
    ) -> Result<(Vec<WorktreeSummary>, bool), WorktreeError> {
        let git_worktrees = GitService::list_worktrees(&context.project_workspace_path)
            .await
            .map_err(map_git_error)?;
        let sessions = load_project_sessions(&context.project_workspace_path).await?;
        let registered_by_path = registry
            .worktrees
            .iter()
            .map(|record| {
                (
                    normalized_lookup_path(Path::new(&record.path)),
                    record.clone(),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut seen_registered_ids = HashSet::new();
        let mut summaries = Vec::new();
        let mut changed = false;

        for git_worktree in git_worktrees {
            let lookup_path = normalized_lookup_path(Path::new(&git_worktree.path));
            let missing = git_worktree.is_prunable || !Path::new(&git_worktree.path).is_dir();
            let registered = registered_by_path.get(&lookup_path);
            if let Some(record) = registered {
                seen_registered_ids.insert(record.worktree_id.clone());
            }
            let worktree_id = if git_worktree.is_main {
                "main".to_string()
            } else if let Some(record) = registered {
                record.worktree_id.clone()
            } else {
                let worktree_id = format!(
                    "external-{}",
                    short_hash(&format!(
                        "{}:{lookup_path}",
                        path_string(&context.common_git_dir)
                    ))
                );
                registry.worktrees.push(RegisteredWorktree {
                    worktree_id: worktree_id.clone(),
                    path: git_worktree.path.clone(),
                    base_ref: git_worktree.branch.clone(),
                    base_commit: git_worktree.head.clone(),
                    branch: git_worktree.branch.clone(),
                    lifecycle: WorktreeLifecycle::External,
                    created_at_ms: current_unix_ms(),
                });
                seen_registered_ids.insert(worktree_id.clone());
                changed = true;
                worktree_id
            };
            let lifecycle = registered
                .map(|record| record.lifecycle)
                .unwrap_or(WorktreeLifecycle::External);
            summaries.push(
                build_summary(
                    context,
                    &worktree_id,
                    lifecycle,
                    git_worktree,
                    missing,
                    &sessions,
                )
                .await?,
            );
        }

        for record in registry.worktrees.iter() {
            if seen_registered_ids.contains(&record.worktree_id) {
                continue;
            }
            let missing_info = GitWorktreeInfo {
                path: record.path.clone(),
                branch: record.branch.clone(),
                head: record.base_commit.clone(),
                is_main: false,
                is_locked: false,
                is_prunable: true,
            };
            summaries.push(
                build_summary(
                    context,
                    &record.worktree_id,
                    record.lifecycle,
                    missing_info,
                    true,
                    &sessions,
                )
                .await?,
            );
        }

        summaries.sort_by(|left, right| {
            right
                .is_main
                .cmp(&left.is_main)
                .then_with(|| left.path.cmp(&right.path))
        });
        if let Some(workspace_service) = get_global_workspace_service() {
            for summary in &summaries {
                if summary.is_main
                    || summary.missing
                    || summary.lifecycle == WorktreeLifecycle::External
                    || workspace_service
                        .get_workspace_by_path(Path::new(&summary.path))
                        .await
                        .is_some()
                {
                    continue;
                }
                workspace_service
                    .track_workspace_activity(
                        PathBuf::from(&summary.path),
                        WorkspaceCreateOptions::default(),
                        WorkspaceActivityMode::RefreshMetadata,
                    )
                    .await
                    .map_err(|workspace_error| {
                        error(
                            WorktreeErrorCode::IoFailed,
                            format!(
                                "Failed to restore a managed worktree workspace registration: {workspace_error}"
                            ),
                        )
                    })?;
            }
        }
        Ok((summaries, changed))
    }

    async fn create_result_for_id(
        context: &RepositoryContext,
        registry: &mut WorktreeRegistry,
        worktree_id: &str,
        created: bool,
    ) -> Result<WorktreeCreateResult, WorktreeError> {
        let record = registry
            .worktrees
            .iter()
            .find(|record| record.worktree_id == worktree_id)
            .cloned()
            .ok_or_else(|| {
                error(
                    WorktreeErrorCode::WorktreeNotFound,
                    "Idempotent worktree result no longer exists",
                )
            })?;
        let (summaries, changed) = Self::reconcile(context, registry).await?;
        if changed {
            Self::save_registry(context, registry).await?;
        }
        let worktree = summaries
            .into_iter()
            .find(|summary| summary.worktree_id == worktree_id)
            .ok_or_else(|| {
                error(
                    WorktreeErrorCode::WorktreeNotFound,
                    "Created worktree could not be reconciled",
                )
            })?;
        Ok(WorktreeCreateResult {
            execution_target: SessionExecutionTarget {
                kind: SessionExecutionTargetKind::ManagedWorktree,
                worktree_id: Some(record.worktree_id),
                root_path: record.path,
                base_ref: record.base_ref,
                base_commit: Some(record.base_commit),
                branch: record.branch,
                lifecycle: Some(record.lifecycle),
            },
            worktree,
            created,
        })
    }

    async fn mutation_result_for_id(
        context: &RepositoryContext,
        registry: &mut WorktreeRegistry,
        worktree_id: &str,
    ) -> Result<WorktreeMutationResult, WorktreeError> {
        let (summaries, changed) = Self::reconcile(context, registry).await?;
        if changed {
            Self::save_registry(context, registry).await?;
        }
        let worktree = summaries
            .into_iter()
            .find(|summary| summary.worktree_id == worktree_id)
            .ok_or_else(|| {
                error(
                    WorktreeErrorCode::WorktreeNotFound,
                    "Worktree could not be reconciled",
                )
            })?;
        Ok(WorktreeMutationResult { worktree })
    }

    async fn rollback_new_worktree(
        context: &RepositoryContext,
        target_path: &Path,
        original_error: WorktreeError,
    ) -> WorktreeError {
        match GitService::remove_worktree(
            &context.project_workspace_path,
            &path_string(target_path),
            true,
        )
        .await
        {
            Ok(_) => original_error,
            Err(rollback_error) => WorktreeError {
                code: WorktreeErrorCode::RollbackIncomplete,
                message: format!(
                    "{}; automatic rollback also failed: {}",
                    original_error.message, rollback_error
                ),
                recovery_path: Some(path_string(target_path)),
            },
        }
    }

    async fn rollback_new_worktree_with_workspace(
        context: &RepositoryContext,
        target_path: &Path,
        workspace_id: Option<&str>,
        original_error: WorktreeError,
    ) -> WorktreeError {
        let mut rollback_issues = Vec::new();
        if let (Some(workspace_service), Some(workspace_id)) =
            (get_global_workspace_service(), workspace_id)
        {
            if let Err(remove_error) = workspace_service.remove_workspace(workspace_id).await {
                rollback_issues.push(format!(
                    "workspace registration could not be removed: {remove_error}"
                ));
            }
        }
        let git_rollback =
            Self::rollback_new_worktree(context, target_path, original_error.clone()).await;
        if git_rollback.code == WorktreeErrorCode::RollbackIncomplete {
            rollback_issues.push(git_rollback.message);
        }
        if rollback_issues.is_empty() {
            original_error
        } else {
            WorktreeError {
                code: WorktreeErrorCode::RollbackIncomplete,
                message: format!(
                    "{}; automatic rollback did not complete: {}",
                    original_error.message,
                    rollback_issues.join("; ")
                ),
                recovery_path: Some(path_string(target_path)),
            }
        }
    }
}

async fn notify_changed(project_workspace_path: &Path) {
    if let Some(workspace_service) = get_global_workspace_service() {
        workspace_service
            .invalidate_worktree_topology(project_workspace_path)
            .await;
    }
    if let Err(event_error) = emit_global_event(BackendEvent::Custom {
        event_name: "worktree://changed".to_string(),
        payload: serde_json::json!({
            "projectWorkspacePath": path_string(project_workspace_path),
        }),
    })
    .await
    {
        log::warn!("Failed to emit worktree change event: {event_error}");
    }
}

async fn build_summary(
    context: &RepositoryContext,
    worktree_id: &str,
    lifecycle: WorktreeLifecycle,
    git_worktree: GitWorktreeInfo,
    missing: bool,
    sessions: &[SessionMetadata],
) -> Result<WorktreeSummary, WorktreeError> {
    let associated = sessions
        .iter()
        .filter(|metadata| {
            metadata
                .execution_target
                .as_ref()
                .and_then(|target| target.worktree_id.as_deref())
                == Some(worktree_id)
                || metadata.workspace_path.as_deref() == Some(git_worktree.path.as_str())
        })
        .collect::<Vec<_>>();
    let session_summaries = associated
        .iter()
        .map(|metadata| WorktreeSessionSummary {
            session_id: metadata.session_id.clone(),
            session_name: metadata.session_name.clone(),
            status: session_status_name(&metadata.status).to_string(),
            archived: matches!(metadata.status, SessionStatus::Archived),
        })
        .collect::<Vec<_>>();
    let running_session_count = associated
        .iter()
        .filter(|metadata| !matches!(metadata.status, SessionStatus::Archived))
        .count();
    let (dirty, unpublished) = if missing {
        (false, false)
    } else {
        (
            GitService::worktree_is_dirty(&git_worktree.path)
                .await
                .map_err(map_git_error)?,
            if git_worktree.branch.is_none() {
                GitService::worktree_has_unpublished_commits(&git_worktree.path)
                    .await
                    .map_err(map_git_error)?
            } else {
                false
            },
        )
    };
    Ok(WorktreeSummary {
        worktree_id: worktree_id.to_string(),
        project_workspace_path: path_string(&context.project_workspace_path),
        path: git_worktree.path,
        head: git_worktree.head,
        branch: git_worktree.branch,
        lifecycle,
        is_main: git_worktree.is_main,
        dirty,
        locked: git_worktree.is_locked,
        missing,
        has_unpublished_commits: unpublished,
        associated_session_count: session_summaries.len(),
        running_session_count,
        sessions: session_summaries,
    })
}

async fn load_project_sessions(
    project_workspace_path: &Path,
) -> Result<Vec<SessionMetadata>, WorktreeError> {
    let context =
        get_workspace_runtime_service_arc().context_for_local_workspace(project_workspace_path);
    SessionMetadataStore::new(context.sessions_dir)
        .list_metadata_including_internal()
        .await
        .map_err(|session_error| {
            error(
                WorktreeErrorCode::IoFailed,
                format!("Failed to read project sessions: {session_error}"),
            )
        })
}

async fn load_settings() -> WorktreeSettings {
    match GlobalConfigManager::get_service().await {
        Ok(config_service) => config_service
            .get_config::<WorktreeSettings>(Some("app.worktrees"))
            .await
            .unwrap_or_default(),
        Err(_) => WorktreeSettings::default(),
    }
}

fn resolve_managed_root(
    settings: &WorktreeSettings,
    path_manager: &PathManager,
) -> Result<PathBuf, WorktreeError> {
    let configured = settings.root_path.trim();
    let portable_configured = configured.replace('\\', "/");
    if portable_configured.is_empty() || portable_configured == "~/.bitfun/worktrees" {
        return Ok(path_manager.worktrees_root());
    }
    if portable_configured == "~" {
        return dirs::home_dir().ok_or_else(|| {
            error(
                WorktreeErrorCode::InvalidPath,
                "Unable to resolve the configured home directory",
            )
        });
    }
    if let Some(suffix) = portable_configured.strip_prefix("~/") {
        return dirs::home_dir()
            .map(|home| home.join(suffix))
            .ok_or_else(|| {
                error(
                    WorktreeErrorCode::InvalidPath,
                    "Unable to resolve the configured home directory",
                )
            });
    }
    let path = PathBuf::from(configured);
    if !path.is_absolute() {
        return Err(error(
            WorktreeErrorCode::InvalidPath,
            "Worktree root must be an absolute path or start with ~/ (or ~\\ on Windows)",
        ));
    }
    Ok(path)
}

async fn managed_target_path(
    settings: &WorktreeSettings,
    repository_id: &str,
    worktree_id: &str,
) -> Result<PathBuf, WorktreeError> {
    let configured_root = resolve_managed_root(settings, get_path_manager_arc().as_ref())?;
    tokio::fs::create_dir_all(&configured_root)
        .await
        .map_err(|io_error| {
            error(
                WorktreeErrorCode::IoFailed,
                format!("Failed to create the managed worktree root: {io_error}"),
            )
        })?;
    let canonical_root = tokio::fs::canonicalize(&configured_root)
        .await
        .map_err(|io_error| {
            error(
                WorktreeErrorCode::IoFailed,
                format!("Failed to resolve the managed worktree root: {io_error}"),
            )
        })?;
    let repository_root = canonical_root.join(repository_id);
    match tokio::fs::symlink_metadata(&repository_root).await {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(error(
                WorktreeErrorCode::InvalidPath,
                "Managed repository worktree root must be a regular directory",
            ));
        }
        Ok(_) => {}
        Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => {
            tokio::fs::create_dir(&repository_root)
                .await
                .map_err(|create_error| {
                    error(
                        WorktreeErrorCode::IoFailed,
                        format!("Failed to create the repository worktree root: {create_error}"),
                    )
                })?;
        }
        Err(io_error) => {
            return Err(error(
                WorktreeErrorCode::IoFailed,
                format!("Failed to inspect the repository worktree root: {io_error}"),
            ));
        }
    }
    let canonical_repository_root =
        tokio::fs::canonicalize(&repository_root)
            .await
            .map_err(|io_error| {
                error(
                    WorktreeErrorCode::IoFailed,
                    format!("Failed to resolve the repository worktree root: {io_error}"),
                )
            })?;
    if !canonical_repository_root.starts_with(&canonical_root) {
        return Err(error(
            WorktreeErrorCode::InvalidPath,
            "Managed repository worktree root escapes the configured root",
        ));
    }
    let target_path = canonical_repository_root.join(worktree_id);
    match tokio::fs::symlink_metadata(&target_path).await {
        Ok(_) => Err(error(
            WorktreeErrorCode::InvalidPath,
            "Managed worktree target already exists",
        )),
        Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => Ok(target_path),
        Err(io_error) => Err(error(
            WorktreeErrorCode::IoFailed,
            format!("Failed to inspect the managed worktree target: {io_error}"),
        )),
    }
}

fn repository_lock(common_git_dir: &Path) -> Arc<AsyncMutex<()>> {
    let locks = REPOSITORY_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = locks.lock().expect("worktree repository lock map poisoned");
    locks
        .entry(common_git_dir.to_path_buf())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

fn repository_id(common_git_dir: &Path) -> String {
    short_hash(&path_string(common_git_dir))
}

fn short_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(digest)[..16].to_string()
}

fn normalized_lookup_path(path: &Path) -> String {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    path_string(&path)
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn current_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn session_status_name(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Active => "active",
        SessionStatus::Archived => "archived",
        SessionStatus::Completed => "completed",
    }
}

fn validate_request_id(request_id: &str) -> Result<(), WorktreeError> {
    if request_id.trim().is_empty() || request_id.len() > 200 {
        return Err(error(
            WorktreeErrorCode::RequestConflict,
            "requestId must be between 1 and 200 bytes",
        ));
    }
    Ok(())
}

fn error(code: WorktreeErrorCode, message: impl Into<String>) -> WorktreeError {
    WorktreeError {
        code,
        message: message.into(),
        recovery_path: None,
    }
}

fn validate_removal(summary: &WorktreeSummary, force: bool) -> Result<(), WorktreeError> {
    if summary.is_main {
        return Err(error(
            WorktreeErrorCode::InvalidPath,
            "The main worktree cannot be removed",
        ));
    }
    if summary.locked {
        return Err(error(
            WorktreeErrorCode::WorktreeLocked,
            "The worktree is locked by Git",
        ));
    }
    if summary.running_session_count > 0 {
        return Err(error(
            WorktreeErrorCode::WorktreeBusy,
            "The worktree has active or unarchived sessions",
        ));
    }
    if !force && summary.dirty {
        return Err(error(
            WorktreeErrorCode::DirtyWorktree,
            "The worktree contains local changes",
        ));
    }
    if !force && summary.has_unpublished_commits {
        return Err(error(
            WorktreeErrorCode::UnpublishedCommits,
            "Detached HEAD contains commits that are not reachable from any ref",
        ));
    }
    if summary.missing {
        return Err(error(
            WorktreeErrorCode::WorktreeNotFound,
            "The worktree directory is missing; recreate it or remove the stale Git record manually",
        ));
    }
    Ok(())
}

fn map_base_ref_error(git_error: GitError, base_ref: &str) -> WorktreeError {
    let text = git_error.to_string();
    if text.to_ascii_lowercase().contains("unborn")
        || text.contains("reference 'HEAD' not found")
        || text.contains("needed a single revision")
    {
        error(
            WorktreeErrorCode::UnbornRepo,
            "The repository has no initial commit",
        )
    } else {
        error(
            WorktreeErrorCode::InvalidBaseRef,
            format!("Failed to resolve base ref '{base_ref}': {text}"),
        )
    }
}

fn map_branch_error(git_error: GitError) -> WorktreeError {
    let text = git_error.to_string();
    if text.contains("already exists") {
        error(WorktreeErrorCode::BranchExists, text)
    } else {
        map_git_error(git_error)
    }
}

fn map_copy_error(git_error: GitError) -> WorktreeError {
    error(WorktreeErrorCode::CopyConflict, git_error.to_string())
}

fn map_git_error(git_error: GitError) -> WorktreeError {
    match git_error {
        GitError::RepositoryNotFound(message) => {
            error(WorktreeErrorCode::NotGitRepository, message)
        }
        GitError::InvalidPath(message) => error(WorktreeErrorCode::InvalidPath, message),
        GitError::IoError(io_error) => error(WorktreeErrorCode::IoFailed, io_error.to_string()),
        other => {
            let message = other.to_string();
            if message.to_ascii_lowercase().contains("unborn") || message.contains("initial commit")
            {
                error(WorktreeErrorCode::UnbornRepo, message)
            } else {
                error(WorktreeErrorCode::GitFailed, message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        repository_id, resolve_managed_root, validate_removal, RegisteredWorktree,
        RepositoryContext, WorktreeOperationReceipt, WorktreeRegistry, WorktreeService,
    };
    use crate::infrastructure::PathManager;
    use bitfun_core_types::{
        WorktreeErrorCode, WorktreeLifecycle, WorktreeSettings, WorktreeSummary,
    };
    use std::path::Path;

    fn removable_summary() -> WorktreeSummary {
        WorktreeSummary {
            worktree_id: "wt-1".to_string(),
            project_workspace_path: "/repo".to_string(),
            path: "/worktrees/wt-1".to_string(),
            head: "0123456789abcdef".to_string(),
            branch: None,
            lifecycle: WorktreeLifecycle::Managed,
            is_main: false,
            dirty: false,
            locked: false,
            missing: false,
            has_unpublished_commits: false,
            associated_session_count: 0,
            running_session_count: 0,
            sessions: Vec::new(),
        }
    }

    #[test]
    fn repository_ids_are_stable_and_path_sensitive() {
        assert_eq!(repository_id(Path::new("/repo/.git")).len(), 16);
        assert_eq!(
            repository_id(Path::new("/repo/.git")),
            repository_id(Path::new("/repo/.git"))
        );
        assert_ne!(
            repository_id(Path::new("/repo/.git")),
            repository_id(Path::new("/other/.git"))
        );
    }

    #[test]
    fn relative_custom_roots_are_rejected() {
        let path_manager = PathManager::new().expect("path manager");
        let settings = WorktreeSettings {
            root_path: "relative/worktrees".to_string(),
            ..WorktreeSettings::default()
        };
        assert!(resolve_managed_root(&settings, &path_manager).is_err());
    }

    #[test]
    fn windows_style_default_root_uses_the_managed_path_contract() {
        let user_root = std::env::temp_dir().join("bitfun-worktree-root-test");
        let path_manager = PathManager::with_user_root_for_tests(user_root);
        let settings = WorktreeSettings {
            root_path: r"~\.bitfun\worktrees".to_string(),
            ..WorktreeSettings::default()
        };

        assert_eq!(
            resolve_managed_root(&settings, &path_manager).unwrap(),
            path_manager.worktrees_root()
        );
    }

    #[test]
    fn request_ids_map_to_stable_session_ids() {
        let first = WorktreeService::session_id_for_request("request-123").unwrap();
        let replay = WorktreeService::session_id_for_request("request-123").unwrap();
        let other = WorktreeService::session_id_for_request("request-456").unwrap();
        assert_eq!(first, replay);
        assert_ne!(first, other);
        assert!(first.starts_with("worktree-session-"));
    }

    #[test]
    fn safe_removal_rejects_every_protected_state() {
        let mut summary = removable_summary();
        summary.is_main = true;
        assert_eq!(
            validate_removal(&summary, false).unwrap_err().code,
            WorktreeErrorCode::InvalidPath
        );

        let mut summary = removable_summary();
        summary.locked = true;
        assert_eq!(
            validate_removal(&summary, true).unwrap_err().code,
            WorktreeErrorCode::WorktreeLocked
        );

        let mut summary = removable_summary();
        summary.running_session_count = 1;
        assert_eq!(
            validate_removal(&summary, true).unwrap_err().code,
            WorktreeErrorCode::WorktreeBusy
        );

        let mut summary = removable_summary();
        summary.dirty = true;
        assert_eq!(
            validate_removal(&summary, false).unwrap_err().code,
            WorktreeErrorCode::DirtyWorktree
        );

        let mut summary = removable_summary();
        summary.has_unpublished_commits = true;
        assert_eq!(
            validate_removal(&summary, false).unwrap_err().code,
            WorktreeErrorCode::UnpublishedCommits
        );

        let mut summary = removable_summary();
        summary.missing = true;
        assert_eq!(
            validate_removal(&summary, true).unwrap_err().code,
            WorktreeErrorCode::WorktreeNotFound
        );
    }

    #[test]
    fn force_only_bypasses_discardable_local_work() {
        let mut summary = removable_summary();
        summary.dirty = true;
        summary.has_unpublished_commits = true;
        assert!(validate_removal(&summary, true).is_ok());
    }

    #[tokio::test]
    async fn registry_round_trip_restores_binding_and_idempotency_receipt() {
        let root = tempfile::tempdir().expect("temp root");
        let project = root.path().join("repo");
        let common_git_dir = project.join(".git");
        std::fs::create_dir_all(&common_git_dir).expect("repository dirs");
        let context = RepositoryContext {
            project_workspace_path: project.clone(),
            common_git_dir,
            registry_path: root.path().join("runtime/worktrees.json"),
            settings: WorktreeSettings::default(),
        };
        let mut registry = WorktreeRegistry::new(&project);
        registry.worktrees.push(RegisteredWorktree {
            worktree_id: "wt-restored".to_string(),
            path: "/managed/wt-restored".to_string(),
            base_ref: Some("main".to_string()),
            base_commit: "0123456789abcdef".to_string(),
            branch: None,
            lifecycle: WorktreeLifecycle::Managed,
            created_at_ms: 123,
        });
        registry.receipts.insert(
            "request-restored".to_string(),
            WorktreeOperationReceipt::Create {
                worktree_id: "wt-restored".to_string(),
                source_workspace_path: project.to_string_lossy().to_string(),
                base_ref: "main".to_string(),
                copy_local_changes: false,
            },
        );

        WorktreeService::save_registry(&context, &registry)
            .await
            .expect("save registry");
        let restored = WorktreeService::load_registry(&context)
            .await
            .expect("load registry");

        assert_eq!(restored.worktrees.len(), 1);
        assert_eq!(restored.worktrees[0].worktree_id, "wt-restored");
        assert_eq!(
            restored
                .receipts
                .get("request-restored")
                .expect("receipt")
                .worktree_id(),
            "wt-restored"
        );
    }
}
