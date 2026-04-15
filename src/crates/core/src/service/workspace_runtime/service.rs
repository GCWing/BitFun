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
        let legacy_project_root = self.path_manager.project_root(workspace_path);
        self.ensure_runtime_context(context, Some(legacy_project_root))
            .await
    }

    pub async fn ensure_remote_workspace_runtime(
        &self,
        ssh_host: &str,
        remote_root: &str,
    ) -> BitFunResult<WorkspaceRuntimeEnsureResult> {
        let context = self.context_for_remote_workspace(ssh_host, remote_root);
        self.ensure_runtime_context(context, None).await
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
        legacy_project_root: Option<PathBuf>,
    ) -> BitFunResult<WorkspaceRuntimeEnsureResult> {
        if self.is_runtime_verified(&context.runtime_root) {
            return Ok(Self::cached_ensure_result(context));
        }

        let runtime_lock = runtime_lock_for(&context.runtime_root);
        let _guard = runtime_lock.lock().await;

        if self.is_runtime_verified(&context.runtime_root) {
            return Ok(Self::cached_ensure_result(context));
        }

        let mut migrated_entries = Vec::new();
        if let Some(legacy_project_root) = legacy_project_root.as_deref() {
            migrated_entries = self
                .migrate_legacy_project_runtime_data(legacy_project_root, &context)
                .await?;
        }

        let mut created_directories = Vec::new();
        for dir in context.required_directories() {
            if !dir.exists() {
                self.path_manager.ensure_dir(dir).await?;
                created_directories.push(dir.to_path_buf());
            }
        }

        if !context.layout_state_file.exists()
            || !created_directories.is_empty()
            || !migrated_entries.is_empty()
        {
            self.persist_layout_state(&context, &migrated_entries)
                .await?;
        }

        self.mark_runtime_verified(&context.runtime_root);

        if !created_directories.is_empty() || !migrated_entries.is_empty() {
            debug!(
                "Workspace runtime ensured: root={} created_dirs={} migrated_entries={}",
                context.runtime_root.display(),
                created_directories.len(),
                migrated_entries.len()
            );
        }

        Ok(WorkspaceRuntimeEnsureResult {
            context,
            created_directories,
            migrated_entries,
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

    async fn migrate_legacy_project_runtime_data(
        &self,
        legacy_project_root: &Path,
        context: &WorkspaceRuntimeContext,
    ) -> BitFunResult<Vec<RuntimeMigrationRecord>> {
        if !legacy_project_root.exists() {
            return Ok(Vec::new());
        }

        let mut migrated_entries = Vec::new();
        let mappings = vec![
            (
                vec![legacy_project_root.join("sessions")],
                context.sessions_dir.clone(),
            ),
            (
                vec![legacy_project_root.join("memory")],
                context.memory_dir.clone(),
            ),
            (
                vec![legacy_project_root.join("plans")],
                context.plans_dir.clone(),
            ),
            (
                vec![legacy_project_root.join("snapshots")],
                context.snapshots_dir.clone(),
            ),
            (
                vec![legacy_project_root.join("ai_memories.json")],
                context.runtime_root.join("ai_memories.json"),
            ),
        ];

        for (candidates, target) in mappings {
            if let Some(record) = self
                .migrate_first_existing_path(&candidates, &target)
                .await?
            {
                migrated_entries.push(record);
            }
        }

        Ok(migrated_entries)
    }

    async fn migrate_first_existing_path(
        &self,
        candidates: &[PathBuf],
        target: &Path,
    ) -> BitFunResult<Option<RuntimeMigrationRecord>> {
        if target.exists() {
            return Ok(None);
        }

        for candidate in candidates {
            if !candidate.exists() {
                continue;
            }

            return self.move_legacy_path(candidate, target).await.map(Some);
        }

        Ok(None)
    }

    async fn move_legacy_path(
        &self,
        source: &Path,
        target: &Path,
    ) -> BitFunResult<RuntimeMigrationRecord> {
        if let Some(parent) = target.parent() {
            self.path_manager.ensure_dir(parent).await?;
        }

        match tokio::fs::rename(source, target).await {
            Ok(()) => Ok(RuntimeMigrationRecord {
                source: source.to_path_buf(),
                target: target.to_path_buf(),
                strategy: "rename".to_string(),
            }),
            Err(_) if source.is_dir() => {
                copy_dir_recursive(source, target)?;
                std::fs::remove_dir_all(source).map_err(|e| {
                    BitFunError::service(format!(
                        "Failed to remove legacy directory {}: {}",
                        source.display(),
                        e
                    ))
                })?;
                Ok(RuntimeMigrationRecord {
                    source: source.to_path_buf(),
                    target: target.to_path_buf(),
                    strategy: "copy_dir".to_string(),
                })
            }
            Err(_) => {
                std::fs::copy(source, target).map_err(|e| {
                    BitFunError::service(format!(
                        "Failed to copy legacy file {} to {}: {}",
                        source.display(),
                        target.display(),
                        e
                    ))
                })?;
                std::fs::remove_file(source).map_err(|e| {
                    BitFunError::service(format!(
                        "Failed to remove legacy file {}: {}",
                        source.display(),
                        e
                    ))
                })?;
                Ok(RuntimeMigrationRecord {
                    source: source.to_path_buf(),
                    target: target.to_path_buf(),
                    strategy: "copy_file".to_string(),
                })
            }
        }
    }
}

fn copy_dir_recursive(source: &Path, target: &Path) -> BitFunResult<()> {
    std::fs::create_dir_all(target).map_err(|e| {
        BitFunError::service(format!(
            "Failed to create target directory {}: {}",
            target.display(),
            e
        ))
    })?;

    for entry in std::fs::read_dir(source).map_err(|e| {
        BitFunError::service(format!(
            "Failed to read legacy directory {}: {}",
            source.display(),
            e
        ))
    })? {
        let entry = entry.map_err(|e| {
            BitFunError::service(format!(
                "Failed to inspect legacy directory entry under {}: {}",
                source.display(),
                e
            ))
        })?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type().map_err(|e| {
            BitFunError::service(format!(
                "Failed to read file type for {}: {}",
                source_path.display(),
                e
            ))
        })?;

        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else if file_type.is_file() {
            std::fs::copy(&source_path, &target_path).map_err(|e| {
                BitFunError::service(format!(
                    "Failed to copy legacy file {} to {}: {}",
                    source_path.display(),
                    target_path.display(),
                    e
                ))
            })?;
        }
    }

    Ok(())
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
    async fn ensure_local_workspace_runtime_migrates_legacy_runtime_entries() {
        let test_root =
            std::env::temp_dir().join(format!("bitfun-runtime-test-{}", Uuid::new_v4()));
        let workspace_root = test_root.join("workspace");
        let legacy_root = workspace_root.join(".bitfun");
        fs::create_dir_all(legacy_root.join("sessions")).expect("legacy sessions should exist");
        fs::write(legacy_root.join("sessions").join("s1.json"), "{}")
            .expect("legacy session file should be written");

        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            test_root.join("user"),
        ));
        let service = WorkspaceRuntimeService::new(path_manager.clone());

        let ensured = service
            .ensure_local_workspace_runtime(&workspace_root)
            .await
            .expect("runtime should be ensured");

        assert!(ensured.context.sessions_dir.join("s1.json").exists());
        assert!(!legacy_root.join("sessions").exists());
        assert_eq!(ensured.migrated_entries.len(), 1);

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
