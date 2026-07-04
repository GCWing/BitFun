use super::types::{
    RuntimeMigrationRecord, WorkspaceRuntimeContext, WorkspaceRuntimeEnsureResult,
    WorkspaceRuntimeTarget, WORKSPACE_RUNTIME_LAYOUT_VERSION,
};
#[cfg(feature = "product-full")]
use crate::agentic::WorkspaceBinding;
use crate::infrastructure::{get_path_manager_arc, PathManager};
use crate::service::remote_ssh::workspace_state::{
    normalize_remote_workspace_path, remote_root_to_mirror_subpath,
    sanitize_ssh_hostname_for_mirror,
};
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_services_core::session::{
    merge_legacy_session_store, move_legacy_path, SessionStoreMigrationError,
    SessionStoreMigrationRecord,
};
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

#[derive(Debug, Clone)]
struct RuntimeMigrationSpec {
    source: PathBuf,
    target: PathBuf,
    strategy: RuntimeMigrationStrategy,
}

#[derive(Debug, Clone, Copy)]
enum RuntimeMigrationStrategy {
    MoveIfTargetMissing,
    MergeSessions,
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

    pub fn context_for_target(&self, target: WorkspaceRuntimeTarget) -> WorkspaceRuntimeContext {
        match target {
            WorkspaceRuntimeTarget::LocalWorkspace { workspace_root } => {
                self.context_for_local_workspace(&workspace_root)
            }
            WorkspaceRuntimeTarget::RemoteWorkspaceMirror {
                ssh_host,
                remote_root,
            } => self.context_for_remote_workspace(&ssh_host, &remote_root),
        }
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
        let normalized_remote_root = normalize_remote_workspace_path(remote_root);
        WorkspaceRuntimeContext::new(
            WorkspaceRuntimeTarget::RemoteWorkspaceMirror {
                ssh_host: ssh_host.to_string(),
                remote_root: normalized_remote_root.clone(),
            },
            self.remote_workspace_runtime_root(ssh_host, &normalized_remote_root),
        )
    }

    pub async fn ensure_workspace_runtime(
        &self,
        target: WorkspaceRuntimeTarget,
    ) -> BitFunResult<WorkspaceRuntimeEnsureResult> {
        let context = self.context_for_target(target);
        let migration_specs = self.migration_specs_for_context(&context);
        self.ensure_runtime_context(context, migration_specs).await
    }

    pub async fn ensure_local_workspace_runtime(
        &self,
        workspace_path: &Path,
    ) -> BitFunResult<WorkspaceRuntimeEnsureResult> {
        self.ensure_workspace_runtime(WorkspaceRuntimeTarget::LocalWorkspace {
            workspace_root: workspace_path.to_path_buf(),
        })
        .await
    }

    pub async fn ensure_remote_workspace_runtime(
        &self,
        ssh_host: &str,
        remote_root: &str,
    ) -> BitFunResult<WorkspaceRuntimeEnsureResult> {
        self.ensure_workspace_runtime(WorkspaceRuntimeTarget::RemoteWorkspaceMirror {
            ssh_host: ssh_host.to_string(),
            remote_root: remote_root.to_string(),
        })
        .await
    }

    #[cfg(feature = "product-full")]
    pub async fn ensure_runtime_for_workspace_binding(
        &self,
        workspace: &WorkspaceBinding,
    ) -> BitFunResult<WorkspaceRuntimeEnsureResult> {
        if workspace.is_remote() {
            self.ensure_remote_workspace_runtime(
                &workspace.session_identity.hostname,
                workspace.session_identity.logical_workspace_path(),
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
        migration_specs: Vec<RuntimeMigrationSpec>,
    ) -> BitFunResult<WorkspaceRuntimeEnsureResult> {
        if self.is_runtime_verified(&context.runtime_root) {
            return Ok(Self::cached_ensure_result(context));
        }

        let runtime_lock = runtime_lock_for(&context.runtime_root);
        let _guard = runtime_lock.lock().await;

        if self.is_runtime_verified(&context.runtime_root) {
            return Ok(Self::cached_ensure_result(context));
        }

        let migrated_entries = self.apply_migration_specs(&migration_specs).await?;
        self.cleanup_legacy_artifacts_for_context(&context).await?;

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

    fn remote_workspace_runtime_root(&self, ssh_host: &str, remote_root_norm: &str) -> PathBuf {
        self.path_manager
            .bitfun_home_dir()
            .join("remote_ssh")
            .join(sanitize_ssh_hostname_for_mirror(ssh_host))
            .join(remote_root_to_mirror_subpath(remote_root_norm))
    }

    fn migration_specs_for_context(
        &self,
        context: &WorkspaceRuntimeContext,
    ) -> Vec<RuntimeMigrationSpec> {
        match &context.target {
            WorkspaceRuntimeTarget::LocalWorkspace { workspace_root } => {
                let legacy_project_root = self.path_manager.project_root(workspace_root);
                vec![
                    RuntimeMigrationSpec {
                        source: legacy_project_root.join("sessions"),
                        target: context.sessions_dir.clone(),
                        strategy: RuntimeMigrationStrategy::MoveIfTargetMissing,
                    },
                    RuntimeMigrationSpec {
                        source: legacy_project_root.join("memory"),
                        target: context.memory_dir.clone(),
                        strategy: RuntimeMigrationStrategy::MoveIfTargetMissing,
                    },
                    RuntimeMigrationSpec {
                        source: legacy_project_root.join("plans"),
                        target: context.plans_dir.clone(),
                        strategy: RuntimeMigrationStrategy::MoveIfTargetMissing,
                    },
                    RuntimeMigrationSpec {
                        source: legacy_project_root.join("snapshots"),
                        target: context.snapshots_dir.clone(),
                        strategy: RuntimeMigrationStrategy::MoveIfTargetMissing,
                    },
                ]
            }
            WorkspaceRuntimeTarget::RemoteWorkspaceMirror {
                ssh_host,
                remote_root,
            } => {
                let runtime_root = self.remote_workspace_runtime_root(ssh_host, remote_root);
                let legacy_sessions_root = runtime_root
                    .join("sessions")
                    .join(".bitfun")
                    .join("sessions");
                vec![RuntimeMigrationSpec {
                    source: legacy_sessions_root,
                    target: context.sessions_dir.clone(),
                    strategy: RuntimeMigrationStrategy::MergeSessions,
                }]
            }
        }
    }

    async fn apply_migration_specs(
        &self,
        specs: &[RuntimeMigrationSpec],
    ) -> BitFunResult<Vec<RuntimeMigrationRecord>> {
        let mut migrated_entries = Vec::new();

        for spec in specs {
            let migrated = match spec.strategy {
                RuntimeMigrationStrategy::MoveIfTargetMissing => {
                    self.migrate_if_target_missing(&spec.source, &spec.target)
                        .await?
                }
                RuntimeMigrationStrategy::MergeSessions => {
                    self.merge_session_store(&spec.source, &spec.target).await?
                }
            };

            if let Some(record) = migrated {
                migrated_entries.push(record);
            }
        }

        Ok(migrated_entries)
    }

    async fn cleanup_legacy_artifacts_for_context(
        &self,
        context: &WorkspaceRuntimeContext,
    ) -> BitFunResult<()> {
        if let WorkspaceRuntimeTarget::RemoteWorkspaceMirror {
            ssh_host,
            remote_root,
        } = &context.target
        {
            let runtime_root = self.remote_workspace_runtime_root(ssh_host, remote_root);
            self.remove_dir_if_empty(&runtime_root.join("sessions").join(".bitfun"))
                .await?;
        }

        Ok(())
    }

    async fn migrate_if_target_missing(
        &self,
        source: &Path,
        target: &Path,
    ) -> BitFunResult<Option<RuntimeMigrationRecord>> {
        if !source.exists() || target.exists() {
            return Ok(None);
        }

        self.move_legacy_path(source, target).await.map(Some)
    }

    async fn move_legacy_path(
        &self,
        source: &Path,
        target: &Path,
    ) -> BitFunResult<RuntimeMigrationRecord> {
        if let Some(parent) = target.parent() {
            self.path_manager.ensure_dir(parent).await?;
        }

        move_legacy_path(source, target)
            .await
            .map(runtime_migration_record)
            .map_err(session_store_migration_error)
    }

    async fn merge_session_store(
        &self,
        source: &Path,
        target: &Path,
    ) -> BitFunResult<Option<RuntimeMigrationRecord>> {
        merge_legacy_session_store(source, target)
            .await
            .map(|record| record.map(runtime_migration_record))
            .map_err(session_store_migration_error)
    }

    async fn remove_dir_if_empty(&self, path: &Path) -> BitFunResult<()> {
        if !path.is_dir() {
            return Ok(());
        }

        let is_empty = match tokio::fs::read_dir(path).await {
            Ok(mut entries) => entries
                .next_entry()
                .await
                .map(|entry| entry.is_none())
                .unwrap_or(false),
            Err(e) => {
                return Err(BitFunError::service(format!(
                    "Failed to inspect directory {}: {}",
                    path.display(),
                    e
                )));
            }
        };

        if is_empty {
            tokio::fs::remove_dir(path).await.map_err(|e| {
                BitFunError::service(format!(
                    "Failed to remove empty legacy directory {}: {}",
                    path.display(),
                    e
                ))
            })?;
        }

        Ok(())
    }
}

fn runtime_migration_record(record: SessionStoreMigrationRecord) -> RuntimeMigrationRecord {
    RuntimeMigrationRecord {
        source: record.source,
        target: record.target,
        strategy: record.strategy,
    }
}

fn session_store_migration_error(error: SessionStoreMigrationError) -> BitFunError {
    if error.is_metadata_deserialization() {
        BitFunError::Deserialization(error.to_string())
    } else if error.is_metadata_serialization() {
        BitFunError::serialization(error.to_string())
    } else {
        BitFunError::service(error.to_string())
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
    #[cfg(test)]
    if let Some(service) = test_workspace_runtime_service_override() {
        return service;
    }

    GLOBAL_WORKSPACE_RUNTIME_SERVICE
        .get_or_init(init_global_workspace_runtime_service)
        .clone()
}

pub fn try_get_workspace_runtime_service_arc() -> BitFunResult<Arc<WorkspaceRuntimeService>> {
    Ok(get_workspace_runtime_service_arc())
}

#[cfg(test)]
thread_local! {
    static TEST_WORKSPACE_RUNTIME_SERVICE: std::cell::RefCell<Option<Arc<WorkspaceRuntimeService>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn test_workspace_runtime_service_override() -> Option<Arc<WorkspaceRuntimeService>> {
    TEST_WORKSPACE_RUNTIME_SERVICE.with(|slot| slot.borrow().clone())
}

#[cfg(test)]
pub struct WorkspaceRuntimeServiceOverrideGuard {
    previous: Option<Arc<WorkspaceRuntimeService>>,
}

#[cfg(test)]
impl Drop for WorkspaceRuntimeServiceOverrideGuard {
    fn drop(&mut self) {
        TEST_WORKSPACE_RUNTIME_SERVICE.with(|slot| {
            *slot.borrow_mut() = self.previous.take();
        });
    }
}

#[cfg(test)]
pub fn set_workspace_runtime_service_for_current_test(
    service: Arc<WorkspaceRuntimeService>,
) -> WorkspaceRuntimeServiceOverrideGuard {
    let previous = TEST_WORKSPACE_RUNTIME_SERVICE.with(|slot| {
        let mut slot = slot.borrow_mut();
        slot.replace(service)
    });
    WorkspaceRuntimeServiceOverrideGuard { previous }
}

#[cfg(test)]
mod tests {
    use super::WorkspaceRuntimeService;
    use crate::infrastructure::PathManager;
    use crate::service::session::{
        SessionMetadata, StoredSessionIndexFile, StoredSessionMetadataFile,
    };
    use std::fs;
    use std::path::Path;
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
        assert!(context.request_traces_dir.exists());
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
    async fn ensure_remote_workspace_runtime_merges_legacy_sessions_only() {
        let test_root =
            std::env::temp_dir().join(format!("bitfun-runtime-test-{}", Uuid::new_v4()));
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            test_root.join("user"),
        ));
        let service = WorkspaceRuntimeService::new(path_manager);

        let context = service.context_for_remote_workspace("example-host", "/root/repo");
        let legacy_sessions_root = context
            .runtime_root
            .join("sessions")
            .join(".bitfun")
            .join("sessions");

        fs::create_dir_all(&legacy_sessions_root).expect("legacy remote sessions should exist");
        fs::create_dir_all(context.sessions_dir.join("existing-session"))
            .expect("new sessions root should exist");

        let mut newer_metadata = SessionMetadata::new(
            "existing-session".to_string(),
            "Existing Session".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        newer_metadata.last_active_at = 200;
        write_session_metadata(
            &context.sessions_dir.join("existing-session"),
            &newer_metadata,
        );

        let mut older_metadata = newer_metadata.clone();
        older_metadata.last_active_at = 100;
        write_session_metadata(
            &legacy_sessions_root.join("existing-session"),
            &older_metadata,
        );
        fs::create_dir_all(legacy_sessions_root.join("legacy-session"))
            .expect("legacy-only session dir should exist");
        let mut legacy_only_metadata = SessionMetadata::new(
            "legacy-session".to_string(),
            "Legacy Session".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        legacy_only_metadata.last_active_at = 150;
        write_session_metadata(
            &legacy_sessions_root.join("legacy-session"),
            &legacy_only_metadata,
        );
        fs::create_dir_all(legacy_sessions_root.join("hidden-session"))
            .expect("hidden legacy session dir should exist");
        let mut hidden_metadata = SessionMetadata::new(
            "hidden-session".to_string(),
            "Hidden Session".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        hidden_metadata.session_kind = bitfun_core_types::SessionKind::Subagent;
        hidden_metadata.last_active_at = 250;
        write_session_metadata(
            &legacy_sessions_root.join("hidden-session"),
            &hidden_metadata,
        );
        write_session_index(
            &legacy_sessions_root.join("index.json"),
            vec![
                hidden_metadata.clone(),
                older_metadata.clone(),
                legacy_only_metadata.clone(),
            ],
        );
        write_session_index(
            &context.sessions_dir.join("index.json"),
            vec![newer_metadata.clone()],
        );

        let ensured = service
            .ensure_remote_workspace_runtime("example-host", "/root/repo")
            .await
            .expect("remote runtime should be ensured");

        assert!(context.sessions_dir.join("legacy-session").exists());
        assert!(context.sessions_dir.join("existing-session").exists());
        assert!(
            !legacy_sessions_root.exists(),
            "legacy sessions root should be removed after merge"
        );

        let merged_metadata: StoredSessionMetadataFile = serde_json::from_slice(
            &fs::read(
                context
                    .sessions_dir
                    .join("existing-session")
                    .join("metadata.json"),
            )
            .expect("merged metadata should exist"),
        )
        .expect("merged metadata should deserialize");
        assert_eq!(merged_metadata.metadata.last_active_at, 200);

        let merged_index: StoredSessionIndexFile = serde_json::from_slice(
            &fs::read(context.sessions_dir.join("index.json"))
                .expect("merged session index should exist"),
        )
        .expect("merged session index should deserialize");
        assert_eq!(merged_index.sessions.len(), 2);
        assert_eq!(merged_index.metadata_file_count, 3);
        assert!(merged_index
            .sessions
            .iter()
            .all(|metadata| metadata.session_id != "hidden-session"));
        assert!(ensured
            .migrated_entries
            .iter()
            .any(|record| record.strategy == "merge_sessions"));
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

    fn write_session_metadata(session_dir: &Path, metadata: &SessionMetadata) {
        fs::create_dir_all(session_dir).expect("session dir should exist");
        let stored = StoredSessionMetadataFile::new(metadata.clone());
        fs::write(
            session_dir.join("metadata.json"),
            serde_json::to_string_pretty(&stored).expect("metadata should serialize"),
        )
        .expect("metadata should write");
    }

    fn write_session_index(path: &Path, sessions: Vec<SessionMetadata>) {
        let index = StoredSessionIndexFile::new(0, sessions);
        fs::write(
            path,
            serde_json::to_string_pretty(&index).expect("index should serialize"),
        )
        .expect("index should write");
    }
}
