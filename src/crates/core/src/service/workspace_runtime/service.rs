use super::types::{
    RuntimeMigrationRecord, WorkspaceRuntimeContext, WorkspaceRuntimeEnsureResult,
    WorkspaceRuntimeTarget, WORKSPACE_RUNTIME_LAYOUT_VERSION,
};
use crate::agentic::WorkspaceBinding;
use crate::infrastructure::{get_path_manager_arc, PathManager};
use crate::service::remote_ssh::workspace_state::{
    normalize_remote_workspace_path, remote_root_to_mirror_subpath,
    sanitize_ssh_hostname_for_mirror,
};
use crate::service::session::{StoredSessionIndexFile, StoredSessionMetadataFile};
use crate::util::errors::{BitFunError, BitFunResult};
use log::debug;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
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

    async fn merge_session_store(
        &self,
        source: &Path,
        target: &Path,
    ) -> BitFunResult<Option<RuntimeMigrationRecord>> {
        if !source.exists() {
            return Ok(None);
        }

        std::fs::create_dir_all(target).map_err(|e| {
            BitFunError::service(format!(
                "Failed to create target sessions directory {}: {}",
                target.display(),
                e
            ))
        })?;

        for entry in std::fs::read_dir(source).map_err(|e| {
            BitFunError::service(format!(
                "Failed to read legacy sessions directory {}: {}",
                source.display(),
                e
            ))
        })? {
            let entry = entry.map_err(|e| {
                BitFunError::service(format!(
                    "Failed to inspect legacy sessions entry under {}: {}",
                    source.display(),
                    e
                ))
            })?;
            let source_path = entry.path();
            let file_name = entry.file_name();
            let file_type = entry.file_type().map_err(|e| {
                BitFunError::service(format!(
                    "Failed to read file type for {}: {}",
                    source_path.display(),
                    e
                ))
            })?;

            if file_name
                .to_string_lossy()
                .eq_ignore_ascii_case("index.json")
            {
                remove_path_if_exists(&source_path)?;
                continue;
            }

            if !file_type.is_dir() {
                let target_path = target.join(&file_name);
                if !target_path.exists() {
                    move_path_best_effort(&source_path, &target_path)?;
                } else if files_are_equal(&source_path, &target_path)? {
                    remove_path_if_exists(&source_path)?;
                } else {
                    replace_target_if_source_newer(&source_path, &target_path)?;
                }
                continue;
            }

            let target_path = target.join(&file_name);
            if !target_path.exists() {
                move_path_best_effort(&source_path, &target_path)?;
                continue;
            }

            merge_session_directory(&source_path, &target_path)?;
            remove_path_if_exists(&source_path)?;
        }

        self.rebuild_session_index(target).await?;
        remove_path_if_exists(&source.join("index.json"))?;
        remove_path_if_exists(source)?;

        Ok(Some(RuntimeMigrationRecord {
            source: source.to_path_buf(),
            target: target.to_path_buf(),
            strategy: "merge_sessions".to_string(),
        }))
    }

    async fn rebuild_session_index(&self, sessions_dir: &Path) -> BitFunResult<()> {
        if !sessions_dir.exists() {
            return Ok(());
        }

        let mut sessions = Vec::new();
        for entry in std::fs::read_dir(sessions_dir).map_err(|e| {
            BitFunError::service(format!(
                "Failed to read merged sessions directory {}: {}",
                sessions_dir.display(),
                e
            ))
        })? {
            let entry = entry.map_err(|e| {
                BitFunError::service(format!(
                    "Failed to inspect merged sessions entry under {}: {}",
                    sessions_dir.display(),
                    e
                ))
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|e| {
                BitFunError::service(format!(
                    "Failed to read file type for {}: {}",
                    path.display(),
                    e
                ))
            })?;
            if !file_type.is_dir() {
                continue;
            }

            let metadata_path = path.join("metadata.json");
            let Some(stored) =
                read_json_optional_sync::<StoredSessionMetadataFile>(&metadata_path)?
            else {
                continue;
            };
            if stored.metadata.should_hide_from_user_lists() {
                continue;
            }
            sessions.push(stored.metadata);
        }

        sessions.sort_by(|a, b| b.last_active_at.cmp(&a.last_active_at));
        let index = StoredSessionIndexFile::new(unix_now_ms(), sessions);
        write_json_pretty_async(&sessions_dir.join("index.json"), &index).await
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

fn merge_session_directory(source: &Path, target: &Path) -> BitFunResult<()> {
    std::fs::create_dir_all(target).map_err(|e| {
        BitFunError::service(format!(
            "Failed to create target session directory {}: {}",
            target.display(),
            e
        ))
    })?;

    for entry in std::fs::read_dir(source).map_err(|e| {
        BitFunError::service(format!(
            "Failed to read legacy session directory {}: {}",
            source.display(),
            e
        ))
    })? {
        let entry = entry.map_err(|e| {
            BitFunError::service(format!(
                "Failed to inspect legacy session entry under {}: {}",
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
            if !target_path.exists() {
                move_path_best_effort(&source_path, &target_path)?;
            } else {
                merge_session_directory(&source_path, &target_path)?;
                remove_path_if_exists(&source_path)?;
            }
            continue;
        }

        if file_name_eq(&source_path, "metadata.json") && target_path.exists() {
            merge_session_metadata_file(&source_path, &target_path)?;
            remove_path_if_exists(&source_path)?;
            continue;
        }

        if !target_path.exists() {
            move_path_best_effort(&source_path, &target_path)?;
        } else if files_are_equal(&source_path, &target_path)? {
            remove_path_if_exists(&source_path)?;
        } else {
            replace_target_if_source_newer(&source_path, &target_path)?;
        }
    }

    Ok(())
}

fn merge_session_metadata_file(source: &Path, target: &Path) -> BitFunResult<()> {
    let source_file =
        read_json_optional_sync::<StoredSessionMetadataFile>(source)?.ok_or_else(|| {
            BitFunError::service(format!(
                "Missing readable session metadata in {}",
                source.display()
            ))
        })?;
    let target_file =
        read_json_optional_sync::<StoredSessionMetadataFile>(target)?.ok_or_else(|| {
            BitFunError::service(format!(
                "Missing readable session metadata in {}",
                target.display()
            ))
        })?;

    let chosen = if source_file.metadata.last_active_at > target_file.metadata.last_active_at {
        source_file
    } else {
        target_file
    };

    write_json_pretty_sync(target, &chosen)?;
    Ok(())
}

fn replace_target_if_source_newer(source: &Path, target: &Path) -> BitFunResult<()> {
    if source_is_newer(source, target)? {
        remove_path_if_exists(target)?;
        move_path_best_effort(source, target)
    } else {
        remove_path_if_exists(source)
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

fn read_json_optional_sync<T>(path: &Path) -> BitFunResult<Option<T>>
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return Ok(None);
    }

    let bytes = std::fs::read(path).map_err(|e| {
        BitFunError::service(format!(
            "Failed to read JSON file {}: {}",
            path.display(),
            e
        ))
    })?;
    let value = serde_json::from_slice(&bytes).map_err(|e| {
        BitFunError::service(format!(
            "Failed to deserialize JSON file {}: {}",
            path.display(),
            e
        ))
    })?;
    Ok(Some(value))
}

async fn write_json_pretty_async<T>(path: &Path, value: &T) -> BitFunResult<()>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to create parent directory {}: {}",
                parent.display(),
                e
            ))
        })?;
    }

    let bytes = serde_json::to_vec_pretty(value).map_err(|e| {
        BitFunError::service(format!(
            "Failed to serialize JSON for {}: {}",
            path.display(),
            e
        ))
    })?;
    tokio::fs::write(path, bytes).await.map_err(|e| {
        BitFunError::service(format!(
            "Failed to write JSON file {}: {}",
            path.display(),
            e
        ))
    })
}

fn write_json_pretty_sync<T>(path: &Path, value: &T) -> BitFunResult<()>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            BitFunError::service(format!(
                "Failed to create parent directory {}: {}",
                parent.display(),
                e
            ))
        })?;
    }

    let bytes = serde_json::to_vec_pretty(value).map_err(|e| {
        BitFunError::service(format!(
            "Failed to serialize JSON for {}: {}",
            path.display(),
            e
        ))
    })?;
    std::fs::write(path, bytes).map_err(|e| {
        BitFunError::service(format!(
            "Failed to write JSON file {}: {}",
            path.display(),
            e
        ))
    })
}

fn move_path_best_effort(source: &Path, target: &Path) -> BitFunResult<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            BitFunError::service(format!(
                "Failed to create target parent directory {}: {}",
                parent.display(),
                e
            ))
        })?;
    }

    match std::fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(_) if source.is_dir() => {
            copy_dir_recursive(source, target)?;
            std::fs::remove_dir_all(source).map_err(|e| {
                BitFunError::service(format!(
                    "Failed to remove moved directory {}: {}",
                    source.display(),
                    e
                ))
            })
        }
        Err(_) => {
            std::fs::copy(source, target).map_err(|e| {
                BitFunError::service(format!(
                    "Failed to copy file {} to {}: {}",
                    source.display(),
                    target.display(),
                    e
                ))
            })?;
            std::fs::remove_file(source).map_err(|e| {
                BitFunError::service(format!(
                    "Failed to remove moved file {}: {}",
                    source.display(),
                    e
                ))
            })
        }
    }
}

fn remove_path_if_exists(path: &Path) -> BitFunResult<()> {
    if !path.exists() {
        return Ok(());
    }

    if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|e| {
            BitFunError::service(format!(
                "Failed to remove directory {}: {}",
                path.display(),
                e
            ))
        })
    } else {
        std::fs::remove_file(path).map_err(|e| {
            BitFunError::service(format!("Failed to remove file {}: {}", path.display(), e))
        })
    }
}

fn files_are_equal(left: &Path, right: &Path) -> BitFunResult<bool> {
    let left_bytes = std::fs::read(left).map_err(|e| {
        BitFunError::service(format!("Failed to read file {}: {}", left.display(), e))
    })?;
    let right_bytes = std::fs::read(right).map_err(|e| {
        BitFunError::service(format!("Failed to read file {}: {}", right.display(), e))
    })?;
    Ok(left_bytes == right_bytes)
}

fn source_is_newer(source: &Path, target: &Path) -> BitFunResult<bool> {
    let source_modified = std::fs::metadata(source)
        .map_err(|e| {
            BitFunError::service(format!(
                "Failed to stat source file {}: {}",
                source.display(),
                e
            ))
        })?
        .modified()
        .ok();
    let target_modified = std::fs::metadata(target)
        .map_err(|e| {
            BitFunError::service(format!(
                "Failed to stat target file {}: {}",
                target.display(),
                e
            ))
        })?
        .modified()
        .ok();

    Ok(match (source_modified, target_modified) {
        (Some(source_time), Some(target_time)) => source_time > target_time,
        (Some(_), None) => true,
        _ => false,
    })
}

fn file_name_eq(path: &Path, expected: &str) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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
        write_session_index(
            &legacy_sessions_root.join("index.json"),
            vec![older_metadata.clone(), legacy_only_metadata.clone()],
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
