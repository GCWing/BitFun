use super::types::{
    RuntimeMigrationRecord, WorkspaceRuntimeContext, WorkspaceRuntimeEnsureResult,
    WorkspaceRuntimeTarget, WORKSPACE_RUNTIME_LAYOUT_VERSION,
};
use crate::agentic::WorkspaceBinding;
use crate::infrastructure::{get_path_manager_arc, PathManager};
use crate::service::remote_ssh::workspace_state::remote_workspace_runtime_root;
use crate::util::errors::{BitFunError, BitFunResult};
use log::debug;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Mutex as AsyncMutex;

#[derive(Debug)]
pub struct WorkspaceRuntimeService {
    path_manager: Arc<PathManager>,
    verified_runtime_roots: Mutex<HashSet<PathBuf>>,
}

#[derive(Debug, Serialize)]
struct RuntimeLayoutState {
    layout_version: u32,
    runtime_root: String,
    target_kind: String,
    target_descriptor: String,
    migrated_entries: Vec<RuntimeMigrationRecordState>,
}

#[derive(Debug, Serialize)]
struct RuntimeMigrationRecordState {
    source: String,
    target: String,
    strategy: String,
}

impl WorkspaceRuntimeService {
    pub fn new(path_manager: Arc<PathManager>) -> Self {
        Self {
            path_manager,
            verified_runtime_roots: Mutex::new(HashSet::new()),
        }
    }

    pub fn path_manager(&self) -> &Arc<PathManager> {
        &self.path_manager
    }

    pub fn context_for_local_workspace(&self, workspace_path: &Path) -> WorkspaceRuntimeContext {
        WorkspaceRuntimeContext::new(
            WorkspaceRuntimeTarget::LocalWorkspace {
                workspace_root: workspace_path.to_path_buf(),
            },
            self.path_manager.project_runtime_root(workspace_path),
        )
    }

    pub fn context_for_remote_workspace(
        &self,
        ssh_host: &str,
        remote_root: &str,
    ) -> WorkspaceRuntimeContext {
        WorkspaceRuntimeContext::new(
            WorkspaceRuntimeTarget::RemoteWorkspaceMirror {
                ssh_host: ssh_host.to_string(),
                remote_root: remote_root.to_string(),
            },
            remote_workspace_runtime_root(ssh_host, remote_root),
        )
    }

    pub async fn ensure_local_workspace_runtime(
        &self,
        workspace_path: &Path,
    ) -> BitFunResult<WorkspaceRuntimeEnsureResult> {
        let context = self.context_for_local_workspace(workspace_path);
        self.ensure_runtime_context(context).await
    }

    pub async fn ensure_remote_workspace_runtime(
        &self,
        ssh_host: &str,
        remote_root: &str,
    ) -> BitFunResult<WorkspaceRuntimeEnsureResult> {
        let context = self.context_for_remote_workspace(ssh_host, remote_root);
        self.ensure_runtime_context(context).await
    }

    pub async fn ensure_runtime_for_workspace_binding(
        &self,
        workspace: &WorkspaceBinding,
    ) -> BitFunResult<WorkspaceRuntimeEnsureResult> {
        if workspace.is_remote() {
            self.ensure_remote_workspace_runtime(
                &workspace.session_identity.hostname,
                &workspace.session_identity.workspace_path,
            )
            .await
        } else {
            self.ensure_local_workspace_runtime(workspace.root_path())
                .await
        }
    }

    async fn ensure_runtime_context(
        &self,
        context: WorkspaceRuntimeContext,
    ) -> BitFunResult<WorkspaceRuntimeEnsureResult> {
        if self.is_runtime_verified(&context.runtime_root) {
            return Ok(Self::cached_ensure_result(context));
        }

        let runtime_lock = runtime_lock_for(&context.runtime_root);
        let _guard = runtime_lock.lock().await;

        if self.is_runtime_verified(&context.runtime_root) {
            return Ok(Self::cached_ensure_result(context));
        }

        let mut created_directories = Vec::new();
        for dir in context.required_directories() {
            if !dir.exists() {
                self.path_manager.ensure_dir(dir).await?;
                created_directories.push(dir.to_path_buf());
            }
        }

        if !context.layout_state_file.exists() || !created_directories.is_empty() {
            self.persist_layout_state(&context, &[]).await?;
        }

        self.mark_runtime_verified(&context.runtime_root);

        if !created_directories.is_empty() {
            debug!(
                "Workspace runtime ensured: root={} created_dirs={} migrated_entries={}",
                context.runtime_root.display(),
                created_directories.len(),
                0
            );
        }

        Ok(WorkspaceRuntimeEnsureResult {
            context,
            created_directories,
            migrated_entries: Vec::new(),
        })
    }

    fn cached_ensure_result(context: WorkspaceRuntimeContext) -> WorkspaceRuntimeEnsureResult {
        WorkspaceRuntimeEnsureResult {
            context,
            created_directories: Vec::new(),
            migrated_entries: Vec::new(),
        }
    }

    fn is_runtime_verified(&self, runtime_root: &Path) -> bool {
        self.verified_runtime_roots
            .lock()
            .expect("workspace runtime verified cache poisoned")
            .contains(runtime_root)
    }

    fn mark_runtime_verified(&self, runtime_root: &Path) {
        self.verified_runtime_roots
            .lock()
            .expect("workspace runtime verified cache poisoned")
            .insert(runtime_root.to_path_buf());
    }

    async fn persist_layout_state(
        &self,
        context: &WorkspaceRuntimeContext,
        migrated_entries: &[RuntimeMigrationRecord],
    ) -> BitFunResult<()> {
        let target_descriptor = match &context.target {
            WorkspaceRuntimeTarget::LocalWorkspace { workspace_root } => {
                workspace_root.display().to_string()
            }
            WorkspaceRuntimeTarget::RemoteWorkspaceMirror {
                ssh_host,
                remote_root,
            } => {
                format!("{}:{}", ssh_host, remote_root)
            }
        };

        let state = RuntimeLayoutState {
            layout_version: WORKSPACE_RUNTIME_LAYOUT_VERSION,
            runtime_root: context.runtime_root.display().to_string(),
            target_kind: context.target.kind().to_string(),
            target_descriptor,
            migrated_entries: migrated_entries
                .iter()
                .map(|record| RuntimeMigrationRecordState {
                    source: record.source.display().to_string(),
                    target: record.target.display().to_string(),
                    strategy: record.strategy.clone(),
                })
                .collect(),
        };

        let bytes = serde_json::to_vec_pretty(&state).map_err(|e| {
            BitFunError::service(format!("Failed to serialize runtime state: {}", e))
        })?;
        tokio::fs::write(&context.layout_state_file, bytes)
            .await
            .map_err(|e| {
                BitFunError::service(format!(
                    "Failed to write runtime layout state '{}': {}",
                    context.layout_state_file.display(),
                    e
                ))
            })?;
        Ok(())
    }
}

fn runtime_lock_for(runtime_root: &Path) -> Arc<AsyncMutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>> = OnceLock::new();

    let locks = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = locks.lock().expect("workspace runtime lock store poisoned");
    guard
        .entry(runtime_root.to_path_buf())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

static GLOBAL_WORKSPACE_RUNTIME_SERVICE: OnceLock<Arc<WorkspaceRuntimeService>> = OnceLock::new();

fn init_global_workspace_runtime_service() -> Arc<WorkspaceRuntimeService> {
    Arc::new(WorkspaceRuntimeService::new(get_path_manager_arc()))
}

pub fn get_workspace_runtime_service_arc() -> Arc<WorkspaceRuntimeService> {
    GLOBAL_WORKSPACE_RUNTIME_SERVICE
        .get_or_init(init_global_workspace_runtime_service)
        .clone()
}

pub fn try_get_workspace_runtime_service_arc() -> BitFunResult<Arc<WorkspaceRuntimeService>> {
    Ok(get_workspace_runtime_service_arc())
}

#[cfg(test)]
mod tests {
    use super::WorkspaceRuntimeService;
    use crate::infrastructure::PathManager;
    use std::fs;
    use std::sync::Arc;
    use std::time::Duration;
    use uuid::Uuid;

    #[tokio::test]
    async fn ensure_local_workspace_runtime_creates_complete_layout_without_project_dot_dir() {
        let test_root =
            std::env::temp_dir().join(format!("bitfun-runtime-test-{}", Uuid::new_v4()));
        let workspace_root = test_root.join("workspace");
        fs::create_dir_all(&workspace_root).expect("workspace should exist");

        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            test_root.join("user"),
        ));
        let service = WorkspaceRuntimeService::new(path_manager.clone());

        let ensured = service
            .ensure_local_workspace_runtime(&workspace_root)
            .await
            .expect("runtime should be ensured");

        let context = ensured.context;
        assert!(context.runtime_root.exists());
        assert!(context.sessions_dir.exists());
        assert!(context.snapshot_by_hash_dir.exists());
        assert!(context.snapshot_metadata_dir.exists());
        assert!(context.snapshot_baselines_dir.exists());
        assert!(context.snapshot_operations_dir.exists());
        assert!(context.locks_dir.exists());
        assert!(context.layout_state_file.exists());
        assert!(!path_manager
            .project_root(&workspace_root)
            .join("context")
            .exists());

        let _ = fs::remove_dir_all(&test_root);
    }

    #[tokio::test]
    async fn ensure_local_workspace_runtime_uses_verified_cache_on_repeat_calls() {
        let test_root =
            std::env::temp_dir().join(format!("bitfun-runtime-test-{}", Uuid::new_v4()));
        let workspace_root = test_root.join("workspace");
        fs::create_dir_all(&workspace_root).expect("workspace should exist");

        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            test_root.join("user"),
        ));
        let service = WorkspaceRuntimeService::new(path_manager);

        let first = service
            .ensure_local_workspace_runtime(&workspace_root)
            .await
            .expect("first ensure should succeed");
        let first_modified = fs::metadata(&first.context.layout_state_file)
            .expect("layout state should exist")
            .modified()
            .expect("layout state should have modified time");

        tokio::time::sleep(Duration::from_millis(20)).await;

        let second = service
            .ensure_local_workspace_runtime(&workspace_root)
            .await
            .expect("second ensure should succeed");
        let second_modified = fs::metadata(&second.context.layout_state_file)
            .expect("layout state should still exist")
            .modified()
            .expect("layout state should have modified time");

        assert!(second.created_directories.is_empty());
        assert!(second.migrated_entries.is_empty());
        assert_eq!(first_modified, second_modified);

        let _ = fs::remove_dir_all(&test_root);
    }
}
