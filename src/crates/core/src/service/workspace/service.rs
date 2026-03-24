//! Workspace service - advanced workspace management API
//!
//! Provides comprehensive workspace management functionality.

use super::manager::{
    ScanOptions, WorkspaceIdentity, WorkspaceInfo, WorkspaceKind, WorkspaceManager,
    WorkspaceManagerConfig, WorkspaceManagerStatistics, WorkspaceOpenOptions, WorkspaceStatus,
    WorkspaceSummary, WorkspaceType,
};
use crate::infrastructure::storage::{PersistenceService, StorageOptions};
use crate::infrastructure::{try_get_path_manager_arc, PathManager};
use crate::service::bootstrap::initialize_workspace_persona_files;
use crate::util::errors::*;
use log::{info, warn};

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

/// Workspace service.
pub struct WorkspaceService {
    manager: Arc<RwLock<WorkspaceManager>>,
    #[allow(dead_code)]
    config: WorkspaceManagerConfig,
    persistence: Arc<PersistenceService>,
    path_manager: Arc<PathManager>,
}

/// Workspace creation options.
#[derive(Debug, Clone)]
pub struct WorkspaceCreateOptions {
    pub scan_options: ScanOptions,
    pub auto_set_current: bool,
    pub add_to_recent: bool,
    pub workspace_kind: WorkspaceKind,
    pub assistant_id: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    /// See [`crate::service::workspace::manager::WorkspaceOpenOptions::remote_connection_id`].
    pub remote_connection_id: Option<String>,
}

impl Default for WorkspaceCreateOptions {
    fn default() -> Self {
        Self {
            scan_options: ScanOptions::default(),
            auto_set_current: true,
            add_to_recent: true,
            workspace_kind: WorkspaceKind::Normal,
            assistant_id: None,
            display_name: None,
            description: None,
            tags: Vec::new(),
            remote_connection_id: None,
        }
    }
}

/// Batch import result.
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchImportResult {
    pub successful: Vec<String>,
    pub failed: Vec<(String, String)>, // (path, error_message)
    pub total_processed: usize,
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIdentityChangedEvent {
    pub workspace_id: String,
    pub workspace_path: String,
    pub name: String,
    pub identity: Option<WorkspaceIdentity>,
    pub changed_fields: Vec<String>,
}

#[derive(Debug, Clone)]
struct AssistantWorkspaceDescriptor {
    path: PathBuf,
    assistant_id: Option<String>,
    display_name: String,
}

impl WorkspaceService {
    /// Creates a new workspace service.
    pub async fn new() -> BitFunResult<Self> {
        let config = WorkspaceManagerConfig::default();
        Self::with_config(config).await
    }

    /// Creates a workspace service with a custom configuration.
    pub async fn with_config(config: WorkspaceManagerConfig) -> BitFunResult<Self> {
        let path_manager = try_get_path_manager_arc()?;

        path_manager.initialize_user_directories().await?;

        let persistence = Arc::new(
            PersistenceService::new_user_level(path_manager.clone())
                .await
                .map_err(|e| {
                    BitFunError::service(format!("Failed to create persistence service: {}", e))
                })?,
        );

        let manager = WorkspaceManager::new(config.clone());

        let service = Self {
            manager: Arc::new(RwLock::new(manager)),
            config,
            persistence,
            path_manager,
        };

        if let Err(e) = service.load_workspace_history_only().await {
            warn!("Failed to load workspace history on startup: {}", e);
        }

        if let Err(e) = service.remap_legacy_assistant_workspace_records().await {
            warn!(
                "Failed to remap legacy assistant workspace records on startup: {}",
                e
            );
        }

        if let Err(e) = service.ensure_assistant_workspaces().await {
            warn!("Failed to ensure assistant workspaces on startup: {}", e);
        }

        Ok(service)
    }

    /// Returns the path manager.
    pub fn path_manager(&self) -> &Arc<PathManager> {
        &self.path_manager
    }

    /// Returns the persistence service.
    pub fn persistence(&self) -> &Arc<PersistenceService> {
        &self.persistence
    }

    /// Opens a workspace.
    pub async fn open_workspace(&self, path: PathBuf) -> BitFunResult<WorkspaceInfo> {
        self.open_workspace_with_options(path, WorkspaceCreateOptions::default())
            .await
    }

    /// Opens a workspace with explicit workspace metadata.
    pub async fn open_workspace_with_options(
        &self,
        path: PathBuf,
        options: WorkspaceCreateOptions,
    ) -> BitFunResult<WorkspaceInfo> {
        let options = self.normalize_workspace_options_for_path(&path, options);
        let result = {
            let mut manager = self.manager.write().await;
            manager
                .open_workspace_with_options(path, Self::to_manager_open_options(&options))
                .await
        };

        if result.is_ok() {
            if let Err(e) = self.save_workspace_data().await {
                warn!("Failed to save workspace data after opening: {}", e);
            }
        }

        result
    }

    /// Quickly opens a workspace (using default options).
    pub async fn quick_open(&self, path: &str) -> BitFunResult<WorkspaceInfo> {
        let path_buf = PathBuf::from(path);
        self.open_workspace(path_buf).await
    }

    /// Creates a workspace (for a new project).
    pub async fn create_workspace(
        &self,
        path: PathBuf,
        options: WorkspaceCreateOptions,
    ) -> BitFunResult<WorkspaceInfo> {
        if !path.exists() {
            tokio::fs::create_dir_all(&path).await.map_err(|e| {
                BitFunError::service(format!("Failed to create workspace directory: {}", e))
            })?;
        }

        let mut workspace = self
            .open_workspace_with_options(path, options.clone())
            .await?;

        if let Some(description) = options.description {
            workspace.description = Some(description);
        }

        workspace.tags = options.tags;

        {
            let mut manager = self.manager.write().await;
            manager
                .get_workspaces_mut()
                .insert(workspace.id.clone(), workspace.clone());
        }

        self.save_workspace_data().await?;

        Ok(workspace)
    }

    /// Creates and opens a new assistant workspace, then sets it as current.
    pub async fn create_assistant_workspace(
        &self,
        assistant_id: Option<String>,
    ) -> BitFunResult<WorkspaceInfo> {
        let assistant_id = match assistant_id {
            Some(id) if !id.trim().is_empty() => id.trim().to_string(),
            _ => self.generate_assistant_workspace_id().await?,
        };
        let display_name = Self::assistant_display_name(Some(&assistant_id));
        let path = self
            .path_manager
            .assistant_workspace_dir(&assistant_id, None);
        let options = WorkspaceCreateOptions {
            auto_set_current: true,
            add_to_recent: false,
            workspace_kind: WorkspaceKind::Assistant,
            assistant_id: Some(assistant_id),
            display_name: Some(display_name),
            ..Default::default()
        };

        if !path.exists() {
            fs::create_dir_all(&path).await.map_err(|e| {
                BitFunError::service(format!(
                    "Failed to create assistant workspace directory '{}': {}",
                    path.display(),
                    e
                ))
            })?;
        }

        // New assistant dirs get persona files at creation; coordinator also fills missing files when opening.
        initialize_workspace_persona_files(&path).await?;

        self.create_workspace(path, options).await
    }

    /// Closes the current workspace.
    pub async fn close_current_workspace(&self) -> BitFunResult<()> {
        let result = {
            let mut manager = self.manager.write().await;
            manager.close_current_workspace()
        };

        if result.is_ok() {
            if let Err(e) = self.save_workspace_data().await {
                warn!("Failed to save workspace data after closing: {}", e);
            }
        }

        result
    }

    /// Closes the specified workspace.
    pub async fn close_workspace(&self, workspace_id: &str) -> BitFunResult<()> {
        let result = {
            let mut manager = self.manager.write().await;
            manager.close_workspace(workspace_id)
        };

        if result.is_ok() {
            if let Err(e) = self.save_workspace_data().await {
                warn!("Failed to save workspace data after closing: {}", e);
            }
        }

        result
    }

    /// Sets the active workspace from the opened workspace list.
    pub async fn set_active_workspace(&self, workspace_id: &str) -> BitFunResult<()> {
        let result = {
            let mut manager = self.manager.write().await;
            manager.set_active_workspace(workspace_id)
        };

        if result.is_ok() {
            if let Err(e) = self.save_workspace_data().await {
                warn!(
                    "Failed to save workspace data after switching active workspace: {}",
                    e
                );
            }
        }

        result
    }

    /// Reorders the opened workspaces without changing active or recent state.
    pub async fn reorder_opened_workspaces(&self, workspace_ids: Vec<String>) -> BitFunResult<()> {
        let current_ids = {
            let manager = self.manager.read().await;
            manager.get_opened_workspace_ids().clone()
        };

        if workspace_ids.len() != current_ids.len() {
            return Err(BitFunError::service(format!(
                "Opened workspace count mismatch: expected {}, got {}",
                current_ids.len(),
                workspace_ids.len()
            )));
        }

        let requested_ids = workspace_ids.iter().cloned().collect::<HashSet<_>>();
        if requested_ids.len() != workspace_ids.len() {
            return Err(BitFunError::service(
                "Opened workspace order contains duplicate ids".to_string(),
            ));
        }

        let current_id_set = current_ids.iter().cloned().collect::<HashSet<_>>();
        if requested_ids != current_id_set {
            return Err(BitFunError::service(
                "Opened workspace order must contain exactly the currently opened workspace ids"
                    .to_string(),
            ));
        }

        {
            let mut manager = self.manager.write().await;
            manager.set_opened_workspace_ids(workspace_ids.clone());
        }

        if let Err(error) = self.save_workspace_data().await {
            let mut manager = self.manager.write().await;
            manager.set_opened_workspace_ids(current_ids);
            return Err(error);
        }

        Ok(())
    }

    /// Switches to the specified workspace.
    pub async fn switch_to_workspace(&self, workspace_id: &str) -> BitFunResult<()> {
        self.set_active_workspace(workspace_id).await
    }

    /// Returns the current workspace.
    pub async fn get_current_workspace(&self) -> Option<WorkspaceInfo> {
        let manager = self.manager.read().await;
        manager.get_current_workspace().cloned()
    }

    /// Best-effort synchronous read for contexts that cannot `await`.
    pub fn try_get_current_workspace_path(&self) -> Option<PathBuf> {
        self.manager.try_read().ok().and_then(|manager| {
            manager
                .get_current_workspace()
                .map(|workspace| workspace.root_path.clone())
        })
    }

    /// Returns workspace details.
    pub async fn get_workspace(&self, workspace_id: &str) -> Option<WorkspaceInfo> {
        let manager = self.manager.read().await;
        manager.get_workspace(workspace_id).cloned()
    }

    /// Returns workspace details by root path.
    pub async fn get_workspace_by_path(&self, path: &Path) -> Option<WorkspaceInfo> {
        let manager = self.manager.read().await;
        manager
            .get_workspaces()
            .values()
            .find(|workspace| workspace.root_path == path)
            .cloned()
    }

    /// Returns all currently opened workspaces.
    pub async fn get_opened_workspaces(&self) -> Vec<WorkspaceInfo> {
        let manager = self.manager.read().await;
        manager
            .get_opened_workspace_infos()
            .into_iter()
            .cloned()
            .collect()
    }

    /// Returns all tracked assistant workspaces, including inactive ones.
    pub async fn get_assistant_workspaces(&self) -> Vec<WorkspaceInfo> {
        let manager = self.manager.read().await;
        manager
            .get_workspaces()
            .values()
            .filter(|workspace| workspace.workspace_kind == WorkspaceKind::Assistant)
            .cloned()
            .collect()
    }

    /// Lists all workspaces.
    pub async fn list_workspaces(&self) -> Vec<WorkspaceSummary> {
        let manager = self.manager.read().await;
        manager.list_workspaces()
    }

    /// Lists workspaces by type.
    pub async fn list_workspaces_by_type(
        &self,
        workspace_type: WorkspaceType,
    ) -> Vec<WorkspaceSummary> {
        let manager = self.manager.read().await;
        manager
            .list_workspaces()
            .into_iter()
            .filter(|ws| ws.workspace_type == workspace_type)
            .collect()
    }

    /// Lists workspaces by status.
    pub async fn list_workspaces_by_status(
        &self,
        status: WorkspaceStatus,
    ) -> Vec<WorkspaceSummary> {
        let manager = self.manager.read().await;
        manager
            .list_workspaces()
            .into_iter()
            .filter(|ws| ws.status == status)
            .collect()
    }

    /// Returns recently accessed workspaces.
    pub async fn get_recent_workspaces(&self) -> Vec<WorkspaceInfo> {
        let manager = self.manager.read().await;
        let recent_ids = manager.get_recent_workspaces();
        let mut recent_workspaces = Vec::new();

        for workspace_id in recent_ids {
            if let Some(workspace) = manager.get_workspaces().get(workspace_id) {
                recent_workspaces.push(workspace.clone());
            }
        }

        recent_workspaces
    }

    /// Returns recently accessed assistant workspaces.
    pub async fn get_recent_assistant_workspaces(&self) -> Vec<WorkspaceInfo> {
        let manager = self.manager.read().await;
        let recent_ids = manager.get_recent_assistant_workspaces();
        let mut recent_workspaces = Vec::new();

        for workspace_id in recent_ids {
            if let Some(workspace) = manager.get_workspaces().get(workspace_id) {
                recent_workspaces.push(workspace.clone());
            }
        }

        recent_workspaces
    }

    /// Searches workspaces.
    pub async fn search_workspaces(&self, query: &str) -> Vec<WorkspaceSummary> {
        let manager = self.manager.read().await;
        manager.search_workspaces(query)
    }

    /// Removes a workspace.
    pub async fn remove_workspace(&self, workspace_id: &str) -> BitFunResult<()> {
        let result = {
            let mut manager = self.manager.write().await;
            manager.remove_workspace(workspace_id)
        };

        if result.is_ok() {
            if let Err(e) = self.save_workspace_data().await {
                warn!("Failed to save workspace data after removal: {}", e);
            }
        }

        result
    }

    /// Removes workspaces in batch.
    pub async fn batch_remove_workspaces(
        &self,
        workspace_ids: Vec<String>,
    ) -> BitFunResult<BatchRemoveResult> {
        let mut result = BatchRemoveResult {
            successful: Vec::new(),
            failed: Vec::new(),
            total_processed: workspace_ids.len(),
        };

        for workspace_id in workspace_ids {
            match self.remove_workspace(&workspace_id).await {
                Ok(_) => result.successful.push(workspace_id),
                Err(e) => result.failed.push((workspace_id, e.to_string())),
            }
        }

        Ok(result)
    }

    /// Rescans a workspace.
    pub async fn rescan_workspace(&self, workspace_id: &str) -> BitFunResult<WorkspaceInfo> {
        let workspace_path = {
            let manager = self.manager.read().await;
            if let Some(workspace) = manager.get_workspace(workspace_id) {
                workspace.root_path.clone()
            } else {
                return Err(BitFunError::service(format!(
                    "Workspace not found: {}",
                    workspace_id
                )));
            }
        };

        let existing_workspace = {
            let manager = self.manager.read().await;
            manager.get_workspace(workspace_id).cloned()
        };
        let Some(existing_workspace) = existing_workspace else {
            return Err(BitFunError::service(format!(
                "Workspace not found: {}",
                workspace_id
            )));
        };
        let new_workspace = WorkspaceInfo::new(
            workspace_path,
            WorkspaceOpenOptions {
                scan_options: ScanOptions::default(),
                auto_set_current: existing_workspace.status == WorkspaceStatus::Active,
                add_to_recent: false,
                workspace_kind: existing_workspace.workspace_kind.clone(),
                assistant_id: existing_workspace.assistant_id.clone(),
                display_name: Some(existing_workspace.name.clone()),
                remote_connection_id: existing_workspace
                    .remote_ssh_connection_id()
                    .map(str::to_string),
            },
        )
        .await?;
        let mut new_workspace = new_workspace;
        new_workspace.id = existing_workspace.id.clone();
        new_workspace.opened_at = existing_workspace.opened_at;
        new_workspace.description = existing_workspace.description.clone();
        new_workspace.tags = existing_workspace.tags.clone();
        new_workspace.metadata = existing_workspace.metadata.clone();

        {
            let mut manager = self.manager.write().await;
            manager
                .get_workspaces_mut()
                .insert(workspace_id.to_string(), new_workspace.clone());
        }

        if let Err(e) = self.save_workspace_data().await {
            warn!("Failed to save workspace data after rescan: {}", e);
        }

        Ok(new_workspace)
    }

    /// Refreshes the parsed `IDENTITY.md` content for an assistant workspace.
    pub async fn refresh_workspace_identity(
        &self,
        workspace_id: &str,
    ) -> BitFunResult<Option<WorkspaceIdentityChangedEvent>> {
        let workspace = {
            let manager = self.manager.read().await;
            manager.get_workspace(workspace_id).cloned()
        }
        .ok_or_else(|| BitFunError::service(format!("Workspace not found: {}", workspace_id)))?;

        if workspace.workspace_kind != WorkspaceKind::Assistant {
            return Ok(None);
        }

        let updated_identity =
            match WorkspaceIdentity::load_from_workspace_root(&workspace.root_path).await {
                Ok(identity) => identity,
                Err(error) => {
                    warn!(
                        "Failed to refresh workspace identity: workspace_id={} path={} error={}",
                        workspace_id,
                        workspace.root_path.display(),
                        error
                    );
                    return Ok(None);
                }
            };

        let changed_fields = WorkspaceIdentity::collect_changed_fields(
            workspace.identity.as_ref(),
            updated_identity.as_ref(),
        );
        let fallback_name = Self::assistant_display_name(workspace.assistant_id.as_deref());
        let updated_name = updated_identity
            .as_ref()
            .and_then(|identity| identity.name.clone())
            .unwrap_or(fallback_name);

        if changed_fields.is_empty() && workspace.name == updated_name {
            return Ok(None);
        }

        {
            let mut manager = self.manager.write().await;
            let workspace = manager
                .get_workspaces_mut()
                .get_mut(workspace_id)
                .ok_or_else(|| {
                    BitFunError::service(format!("Workspace not found: {}", workspace_id))
                })?;

            workspace.identity = updated_identity.clone();
            workspace.name = updated_name.clone();
        }

        if let Err(e) = self.save_workspace_data().await {
            warn!(
                "Failed to save workspace data after identity refresh: workspace_id={} error={}",
                workspace_id, e
            );
        }

        Ok(Some(WorkspaceIdentityChangedEvent {
            workspace_id: workspace.id,
            workspace_path: workspace.root_path.to_string_lossy().to_string(),
            name: updated_name,
            identity: updated_identity,
            changed_fields,
        }))
    }

    /// Updates workspace information.
    pub async fn update_workspace_info(
        &self,
        workspace_id: &str,
        updates: WorkspaceInfoUpdates,
    ) -> BitFunResult<()> {
        let mut manager = self.manager.write().await;

        if let Some(workspace) = manager.get_workspaces_mut().get_mut(workspace_id) {
            if let Some(name) = updates.name {
                workspace.name = name;
            }

            if let Some(description) = updates.description {
                workspace.description = Some(description);
            }

            if let Some(tags) = updates.tags {
                workspace.tags = tags;
            }

            workspace.last_accessed = chrono::Utc::now();

            Ok(())
        } else {
            Err(BitFunError::service(format!(
                "Workspace not found: {}",
                workspace_id
            )))
        }
    }

    /// Imports workspaces in batch.
    pub async fn batch_import_workspaces(
        &self,
        paths: Vec<String>,
    ) -> BitFunResult<BatchImportResult> {
        let mut result = BatchImportResult {
            successful: Vec::new(),
            failed: Vec::new(),
            total_processed: paths.len(),
            skipped: Vec::new(),
        };

        for path_str in paths {
            let path = PathBuf::from(&path_str);

            if !path.exists() {
                result
                    .failed
                    .push((path_str, "Path does not exist".to_string()));
                continue;
            }

            if !path.is_dir() {
                result
                    .failed
                    .push((path_str, "Path is not a directory".to_string()));
                continue;
            }

            {
                let manager = self.manager.read().await;
                if manager
                    .get_workspaces()
                    .values()
                    .any(|w| w.root_path == path)
                {
                    result.skipped.push(path_str);
                    continue;
                }
            }

            match self.open_workspace(path).await {
                Ok(workspace) => {
                    result.successful.push(workspace.id);
                }
                Err(e) => {
                    result.failed.push((path_str, e.to_string()));
                }
            }
        }

        Ok(result)
    }

    /// Cleans up invalid workspaces.
    pub async fn cleanup_invalid_workspaces(&self) -> BitFunResult<usize> {
        let result = {
            let mut manager = self.manager.write().await;
            manager.cleanup_invalid_workspaces().await
        };

        if result.is_ok() {
            if let Err(e) = self.save_workspace_data().await {
                warn!("Failed to save workspace data after cleanup: {}", e);
            }
        }

        result
    }

    /// Returns statistics.
    pub async fn get_statistics(&self) -> WorkspaceManagerStatistics {
        let manager = self.manager.read().await;
        manager.get_statistics()
    }

    /// Returns the workspace count.
    pub async fn get_workspace_count(&self) -> usize {
        let manager = self.manager.read().await;
        manager.get_workspace_count()
    }

    /// Runs a health check.
    pub async fn health_check(&self) -> BitFunResult<WorkspaceHealthStatus> {
        let stats = self.get_statistics().await;

        let mut warnings = Vec::new();
        let mut issues = Vec::new();

        if stats.total_workspaces == 0 {
            warnings.push("No workspaces found".to_string());
        }

        if stats.active_workspaces == 0 {
            warnings.push("No active workspaces".to_string());
        }

        if stats.inactive_workspaces > stats.active_workspaces * 3 {
            issues.push("Too many inactive workspaces, consider cleanup".to_string());
        }

        let current_workspace_valid = match self.get_current_workspace().await {
            Some(current) => current.is_valid().await,
            None => true,
        };

        if !current_workspace_valid {
            issues.push("Current workspace path is invalid".to_string());
        }

        let healthy = issues.is_empty() && current_workspace_valid;

        Ok(WorkspaceHealthStatus {
            healthy,
            total_workspaces: stats.total_workspaces,
            active_workspaces: stats.active_workspaces,
            current_workspace_valid,
            total_files: stats.total_files,
            total_size_mb: stats.total_size_bytes / (1024 * 1024),
            warnings,
            issues: issues.clone(),
            message: if healthy {
                "Workspace system is healthy".to_string()
            } else {
                format!("{} issues detected", issues.len())
            },
        })
    }

    /// Exports workspace configuration.
    pub async fn export_workspaces(&self) -> BitFunResult<WorkspaceExport> {
        let manager = self.manager.read().await;
        let workspaces: Vec<WorkspaceInfo> = manager.get_workspaces().values().cloned().collect();
        let current_workspace_id = manager.get_current_workspace().map(|w| w.id.clone());
        let _recent_workspaces = manager.get_recent_workspaces().clone();

        Ok(WorkspaceExport {
            workspaces,
            current_workspace_id,
            recent_workspaces: manager
                .get_recent_workspace_infos()
                .iter()
                .map(|w| w.id.clone())
                .collect(),
            recent_assistant_workspaces: manager
                .get_recent_assistant_workspace_infos()
                .iter()
                .map(|w| w.id.clone())
                .collect(),
            export_timestamp: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    /// Imports workspace configuration.
    pub async fn import_workspaces(
        &self,
        export: WorkspaceExport,
        overwrite: bool,
    ) -> BitFunResult<WorkspaceImportResult> {
        let mut result = WorkspaceImportResult {
            imported_workspaces: 0,
            skipped_workspaces: 0,
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        let mut manager = self.manager.write().await;

        for workspace in export.workspaces {
            if !workspace.is_valid().await {
                result.warnings.push(format!(
                    "Workspace path no longer valid: {:?}",
                    workspace.root_path
                ));
                continue;
            }

            if !overwrite && manager.get_workspaces().contains_key(&workspace.id) {
                result.skipped_workspaces += 1;
                continue;
            }

            manager
                .get_workspaces_mut()
                .insert(workspace.id.clone(), workspace);
            result.imported_workspaces += 1;
        }

        manager.set_recent_workspaces(export.recent_workspaces.clone());
        manager.set_recent_assistant_workspaces(export.recent_assistant_workspaces.clone());

        if let Some(current_id) = export.current_workspace_id {
            if manager.get_workspaces().contains_key(&current_id) {
                if let Err(e) = manager.set_current_workspace(current_id) {
                    result
                        .warnings
                        .push(format!("Failed to restore current workspace: {}", e));
                }
            } else {
                result
                    .warnings
                    .push("Current workspace not found in import".to_string());
            }
        }

        drop(manager);

        Ok(result)
    }

    /// Returns a quick summary.
    pub async fn get_quick_summary(&self) -> WorkspaceQuickSummary {
        let stats = self.get_statistics().await;
        let current_workspace = self.get_current_workspace().await;
        let recent_workspaces = self.get_recent_workspaces().await;
        let recent_assistant_workspaces = self.get_recent_assistant_workspaces().await;

        WorkspaceQuickSummary {
            total_workspaces: stats.total_workspaces,
            active_workspaces: stats.active_workspaces,
            current_workspace: current_workspace.map(|w| w.get_summary()),
            recent_workspaces: recent_workspaces
                .into_iter()
                .take(5)
                .map(|w| w.get_summary())
                .collect(),
            recent_assistant_workspaces: recent_assistant_workspaces
                .into_iter()
                .take(5)
                .map(|w| w.get_summary())
                .collect(),
            workspace_types: stats.workspaces_by_type,
        }
    }

    /// Saves workspace data locally.
    async fn save_workspace_data(&self) -> BitFunResult<()> {
        let manager = self.manager.read().await;

        let workspace_data = WorkspacePersistenceData {
            workspaces: manager.get_workspaces().clone(),
            opened_workspace_ids: manager.get_opened_workspace_ids().clone(),
            current_workspace_id: manager.get_current_workspace().map(|w| w.id.clone()),
            recent_workspaces: manager.get_recent_workspaces().clone(),
            recent_assistant_workspaces: manager.get_recent_assistant_workspaces().clone(),
            saved_at: chrono::Utc::now(),
        };

        self.persistence
            .save_json("workspace_data", &workspace_data, StorageOptions::default())
            .await
            .map_err(|e| BitFunError::service(format!("Failed to save workspace data: {}", e)))?;

        Ok(())
    }

    /// Loads workspace data from local storage.
    #[allow(dead_code)]
    async fn load_workspace_data(&self) -> BitFunResult<()> {
        let workspace_data: Option<WorkspacePersistenceData> = self
            .persistence
            .load_json("workspace_data")
            .await
            .map_err(|e| BitFunError::service(format!("Failed to load workspace data: {}", e)))?;

        if let Some(data) = workspace_data {
            let mut manager = self.manager.write().await;

            *manager.get_workspaces_mut() = data.workspaces;
            manager.set_opened_workspace_ids(data.opened_workspace_ids);
            manager.set_recent_workspaces(data.recent_workspaces);
            manager.set_recent_assistant_workspaces(data.recent_assistant_workspaces);

            if let Some(current_id) = data.current_workspace_id {
                if let Some(workspace) = manager.get_workspaces().get(&current_id) {
                    if workspace.is_valid().await {
                        if let Err(e) = manager.set_current_workspace(current_id) {
                            warn!("Failed to restore current workspace: {}", e);
                        }
                    } else {
                        warn!("Current workspace path no longer valid, skipping restore");
                    }
                }
            }

            info!(
                "Loaded {} workspaces from local storage",
                manager.get_workspaces().len()
            );
        } else {
            info!("No saved workspace data found, starting fresh");
        }

        Ok(())
    }

    /// Loads workspace history only without restoring the current workspace (used on startup).
    async fn load_workspace_history_only(&self) -> BitFunResult<()> {
        let workspace_data: Option<WorkspacePersistenceData> = self
            .persistence
            .load_json("workspace_data")
            .await
            .map_err(|e| BitFunError::service(format!("Failed to load workspace data: {}", e)))?;

        if let Some(data) = workspace_data {
            let mut manager = self.manager.write().await;

            *manager.get_workspaces_mut() = data.workspaces;
            manager.set_opened_workspace_ids(data.opened_workspace_ids.clone());
            manager.set_recent_workspaces(data.recent_workspaces);
            manager.set_recent_assistant_workspaces(data.recent_assistant_workspaces);

            let current_id = data
                .current_workspace_id
                .or_else(|| data.opened_workspace_ids.first().cloned());

            if let Some(current_id) = current_id {
                if manager.get_workspaces().contains_key(&current_id) {
                    if let Err(e) = manager.set_current_workspace(current_id) {
                        warn!("Failed to restore current workspace on startup: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    fn to_manager_open_options(options: &WorkspaceCreateOptions) -> WorkspaceOpenOptions {
        WorkspaceOpenOptions {
            scan_options: options.scan_options.clone(),
            auto_set_current: options.auto_set_current,
            add_to_recent: options.add_to_recent,
            workspace_kind: options.workspace_kind.clone(),
            assistant_id: options.assistant_id.clone(),
            display_name: options.display_name.clone(),
            remote_connection_id: options.remote_connection_id.clone(),
        }
    }

    fn assistant_display_name(assistant_id: Option<&str>) -> String {
        match assistant_id {
            Some(id) if !id.trim().is_empty() => format!("Claw {}", id.trim()),
            _ => "Claw".to_string(),
        }
    }

    async fn generate_assistant_workspace_id(&self) -> BitFunResult<String> {
        for _ in 0..32 {
            let assistant_id = uuid::Uuid::new_v4()
                .simple()
                .to_string()
                .chars()
                .take(8)
                .collect::<String>();
            let path = self
                .path_manager
                .assistant_workspace_dir(&assistant_id, None);

            if fs::try_exists(&path).await.map_err(|e| {
                BitFunError::service(format!(
                    "Failed to check assistant workspace path '{}': {}",
                    path.display(),
                    e
                ))
            })? {
                continue;
            }

            if self.get_workspace_by_path(&path).await.is_none() {
                return Ok(assistant_id);
            }
        }

        Err(BitFunError::service(
            "Failed to allocate a unique assistant workspace id".to_string(),
        ))
    }

    fn assistant_descriptor_from_path(&self, path: &Path) -> Option<AssistantWorkspaceDescriptor> {
        let default_workspace = self.path_manager.default_assistant_workspace_dir(None);
        if path == default_workspace {
            return Some(AssistantWorkspaceDescriptor {
                path: path.to_path_buf(),
                assistant_id: None,
                display_name: Self::assistant_display_name(None),
            });
        }

        let assistant_root = self.path_manager.assistant_workspace_base_dir(None);
        if path.parent()? != assistant_root {
            return None;
        }

        let file_name = path.file_name()?.to_string_lossy();
        let assistant_id = file_name.strip_prefix("workspace-")?;
        if assistant_id.trim().is_empty() {
            return None;
        }

        Some(AssistantWorkspaceDescriptor {
            path: path.to_path_buf(),
            assistant_id: Some(assistant_id.to_string()),
            display_name: Self::assistant_display_name(Some(assistant_id)),
        })
    }

    fn legacy_assistant_descriptor_from_path(
        &self,
        path: &Path,
    ) -> Option<AssistantWorkspaceDescriptor> {
        let default_workspace = self
            .path_manager
            .legacy_default_assistant_workspace_dir(None);
        if path == default_workspace {
            return Some(AssistantWorkspaceDescriptor {
                path: path.to_path_buf(),
                assistant_id: None,
                display_name: Self::assistant_display_name(None),
            });
        }

        let assistant_root = self.path_manager.legacy_assistant_workspace_base_dir(None);
        if path.parent()? != assistant_root {
            return None;
        }

        let file_name = path.file_name()?.to_string_lossy();
        let assistant_id = file_name.strip_prefix("workspace-")?;
        if assistant_id.trim().is_empty() {
            return None;
        }

        Some(AssistantWorkspaceDescriptor {
            path: path.to_path_buf(),
            assistant_id: Some(assistant_id.to_string()),
            display_name: Self::assistant_display_name(Some(assistant_id)),
        })
    }

    async fn remap_legacy_assistant_workspace_records(&self) -> BitFunResult<()> {
        let mut changed = false;
        let mut manager = self.manager.write().await;

        for workspace in manager.get_workspaces_mut().values_mut() {
            let Some(descriptor) = self.legacy_assistant_descriptor_from_path(&workspace.root_path)
            else {
                continue;
            };
            let new_path = self
                .path_manager
                .resolve_assistant_workspace_dir(descriptor.assistant_id.as_deref(), None);

            if workspace.root_path != new_path {
                info!(
                    "Remap legacy assistant workspace record: workspace_id={}, from={}, to={}",
                    workspace.id,
                    workspace.root_path.display(),
                    new_path.display()
                );
                workspace.root_path = new_path;
                changed = true;
            }

            if workspace.workspace_kind != WorkspaceKind::Assistant {
                workspace.workspace_kind = WorkspaceKind::Assistant;
                changed = true;
            }

            if workspace.assistant_id != descriptor.assistant_id {
                workspace.assistant_id = descriptor.assistant_id.clone();
                changed = true;
            }
        }

        drop(manager);

        if changed {
            self.save_workspace_data().await?;
        }

        Ok(())
    }

    async fn migrate_legacy_assistant_workspaces(&self) -> BitFunResult<()> {
        let assistant_root = self.path_manager.assistant_workspace_base_dir(None);
        fs::create_dir_all(&assistant_root).await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to create assistant workspace root '{}': {}",
                assistant_root.display(),
                e
            ))
        })?;

        let legacy_root = self.path_manager.legacy_assistant_workspace_base_dir(None);
        let default_legacy_workspace = self
            .path_manager
            .legacy_default_assistant_workspace_dir(None);
        let default_workspace = self.path_manager.default_assistant_workspace_dir(None);

        if fs::try_exists(&default_legacy_workspace)
            .await
            .map_err(|e| {
                BitFunError::service(format!(
                    "Failed to inspect legacy assistant workspace '{}': {}",
                    default_legacy_workspace.display(),
                    e
                ))
            })?
            && !fs::try_exists(&default_workspace).await.map_err(|e| {
                BitFunError::service(format!(
                    "Failed to inspect assistant workspace '{}': {}",
                    default_workspace.display(),
                    e
                ))
            })?
        {
            fs::rename(&default_legacy_workspace, &default_workspace)
                .await
                .map_err(|e| {
                    BitFunError::service(format!(
                        "Failed to migrate assistant workspace '{}' to '{}': {}",
                        default_legacy_workspace.display(),
                        default_workspace.display(),
                        e
                    ))
                })?;
            info!(
                "Migrated default assistant workspace: from={}, to={}",
                default_legacy_workspace.display(),
                default_workspace.display()
            );
        }

        let mut entries = fs::read_dir(&legacy_root).await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to read legacy assistant workspace root '{}': {}",
                legacy_root.display(),
                e
            ))
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to iterate legacy assistant workspace root '{}': {}",
                legacy_root.display(),
                e
            ))
        })? {
            let file_type = entry.file_type().await.map_err(|e| {
                BitFunError::service(format!(
                    "Failed to inspect legacy assistant workspace entry '{}': {}",
                    entry.path().display(),
                    e
                ))
            })?;
            if !file_type.is_dir() {
                continue;
            }

            let file_name = entry.file_name().to_string_lossy().to_string();
            let Some(assistant_id) = file_name.strip_prefix("workspace-") else {
                continue;
            };
            if assistant_id.trim().is_empty() {
                continue;
            }

            let target_path = self
                .path_manager
                .assistant_workspace_dir(assistant_id, None);
            if fs::try_exists(&target_path).await.map_err(|e| {
                BitFunError::service(format!(
                    "Failed to inspect assistant workspace '{}': {}",
                    target_path.display(),
                    e
                ))
            })? {
                continue;
            }

            fs::rename(entry.path(), &target_path).await.map_err(|e| {
                BitFunError::service(format!(
                    "Failed to migrate assistant workspace '{}' to '{}': {}",
                    file_name,
                    target_path.display(),
                    e
                ))
            })?;
            info!(
                "Migrated named assistant workspace: assistant_id={}, to={}",
                assistant_id,
                target_path.display()
            );
        }

        Ok(())
    }

    fn normalize_workspace_options_for_path(
        &self,
        path: &Path,
        mut options: WorkspaceCreateOptions,
    ) -> WorkspaceCreateOptions {
        if options.workspace_kind == WorkspaceKind::Remote {
            return options;
        }

        if options.workspace_kind == WorkspaceKind::Assistant {
            if options.display_name.is_none() {
                options.display_name = Some(Self::assistant_display_name(
                    options.assistant_id.as_deref(),
                ));
            }
            return options;
        }

        if let Some(descriptor) = self.assistant_descriptor_from_path(path) {
            options.workspace_kind = WorkspaceKind::Assistant;
            if options.assistant_id.is_none() {
                options.assistant_id = descriptor.assistant_id;
            }
            if options.display_name.is_none() {
                options.display_name = Some(descriptor.display_name);
            }
        }

        options
    }

    async fn discover_assistant_workspaces(
        &self,
    ) -> BitFunResult<Vec<AssistantWorkspaceDescriptor>> {
        self.migrate_legacy_assistant_workspaces().await?;

        let assistant_root = self.path_manager.assistant_workspace_base_dir(None);
        fs::create_dir_all(&assistant_root).await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to create assistant workspace root '{}': {}",
                assistant_root.display(),
                e
            ))
        })?;

        let default_workspace = self.path_manager.default_assistant_workspace_dir(None);
        fs::create_dir_all(&default_workspace).await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to create default assistant workspace '{}': {}",
                default_workspace.display(),
                e
            ))
        })?;

        let mut descriptors = vec![AssistantWorkspaceDescriptor {
            path: default_workspace,
            assistant_id: None,
            display_name: Self::assistant_display_name(None),
        }];

        let mut entries = fs::read_dir(&assistant_root).await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to read assistant workspace root '{}': {}",
                assistant_root.display(),
                e
            ))
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to iterate assistant workspace root '{}': {}",
                assistant_root.display(),
                e
            ))
        })? {
            let file_type = entry.file_type().await.map_err(|e| {
                BitFunError::service(format!(
                    "Failed to inspect assistant workspace entry '{}': {}",
                    entry.path().display(),
                    e
                ))
            })?;
            if !file_type.is_dir() {
                continue;
            }

            let file_name = entry.file_name().to_string_lossy().to_string();
            let Some(assistant_id) = file_name.strip_prefix("workspace-") else {
                continue;
            };
            if assistant_id.trim().is_empty() {
                continue;
            }

            descriptors.push(AssistantWorkspaceDescriptor {
                path: entry.path(),
                assistant_id: Some(assistant_id.to_string()),
                display_name: Self::assistant_display_name(Some(assistant_id)),
            });
        }

        descriptors.sort_by(|left, right| {
            match (left.assistant_id.is_some(), right.assistant_id.is_some()) {
                (false, true) => std::cmp::Ordering::Less,
                (true, false) => std::cmp::Ordering::Greater,
                _ => left.path.cmp(&right.path),
            }
        });

        Ok(descriptors)
    }

    async fn ensure_assistant_workspaces(&self) -> BitFunResult<()> {
        let descriptors = self.discover_assistant_workspaces().await?;
        let mut has_current_workspace = self.get_current_workspace().await.is_some();
        let has_opened_remote = {
            let manager = self.manager.read().await;
            manager
                .get_opened_workspace_infos()
                .iter()
                .any(|w| w.workspace_kind == WorkspaceKind::Remote)
        };

        for descriptor in descriptors {
            // If a remote workspace tab exists but nothing is current yet (e.g. pending SSH
            // reconnect), do not auto-activate the default assistant workspace — that would look
            // like a spurious new local workspace.
            let should_activate = !has_current_workspace
                && !has_opened_remote
                && descriptor.assistant_id.is_none();
            let options = WorkspaceCreateOptions {
                auto_set_current: should_activate,
                add_to_recent: false,
                workspace_kind: WorkspaceKind::Assistant,
                assistant_id: descriptor.assistant_id.clone(),
                display_name: Some(descriptor.display_name.clone()),
                ..Default::default()
            };

            self.open_workspace_with_options(descriptor.path, options)
                .await?;
            has_current_workspace = true;
        }

        Ok(())
    }

    /// Saves workspace data manually (public API).
    pub async fn manual_save(&self) -> BitFunResult<()> {
        self.save_workspace_data().await
    }

    /// Returns whether a path is a managed assistant workspace.
    pub fn is_assistant_workspace_path(&self, path: &Path) -> bool {
        self.assistant_descriptor_from_path(path).is_some()
    }

    /// Clears all persisted data.
    pub async fn clear_persistent_data(&self) -> BitFunResult<()> {
        self.persistence
            .delete("workspace_data")
            .await
            .map_err(|e| BitFunError::service(format!("Failed to clear workspace data: {}", e)))?;

        Ok(())
    }

    /// Returns the underlying `WorkspaceManager` handle.
    /// Used to share workspace state with other services (e.g. Agent).
    pub fn get_manager(&self) -> Arc<RwLock<WorkspaceManager>> {
        self.manager.clone()
    }
}

/// Workspace info updates.
#[derive(Debug, Clone)]
pub struct WorkspaceInfoUpdates {
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Batch remove result.
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchRemoveResult {
    pub successful: Vec<String>,
    pub failed: Vec<(String, String)>,
    pub total_processed: usize,
}

/// Workspace health status.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceHealthStatus {
    pub healthy: bool,
    pub total_workspaces: usize,
    pub active_workspaces: usize,
    pub current_workspace_valid: bool,
    pub total_files: usize,
    pub total_size_mb: u64,
    pub warnings: Vec<String>,
    pub issues: Vec<String>,
    pub message: String,
}

/// Workspace export format.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceExport {
    pub workspaces: Vec<WorkspaceInfo>,
    pub current_workspace_id: Option<String>,
    pub recent_workspaces: Vec<String>,
    #[serde(default)]
    pub recent_assistant_workspaces: Vec<String>,
    pub export_timestamp: String,
    pub version: String,
}

/// Workspace import result.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceImportResult {
    pub imported_workspaces: usize,
    pub skipped_workspaces: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Workspace quick summary.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceQuickSummary {
    pub total_workspaces: usize,
    pub active_workspaces: usize,
    pub current_workspace: Option<WorkspaceSummary>,
    pub recent_workspaces: Vec<WorkspaceSummary>,
    #[serde(default)]
    pub recent_assistant_workspaces: Vec<WorkspaceSummary>,
    pub workspace_types: std::collections::HashMap<WorkspaceType, usize>,
}

/// Workspace persistence data.
#[derive(Debug, Serialize, Deserialize)]
struct WorkspacePersistenceData {
    pub workspaces: std::collections::HashMap<String, WorkspaceInfo>,
    #[serde(default)]
    pub opened_workspace_ids: Vec<String>,
    pub current_workspace_id: Option<String>,
    #[serde(default)]
    pub recent_workspaces: Vec<String>,
    #[serde(default)]
    pub recent_assistant_workspaces: Vec<String>,
    pub saved_at: chrono::DateTime<chrono::Utc>,
}

// ── Global workspace service singleton ──────────────────────────────

static GLOBAL_WORKSPACE_SERVICE: std::sync::OnceLock<Arc<WorkspaceService>> =
    std::sync::OnceLock::new();

pub fn set_global_workspace_service(service: Arc<WorkspaceService>) {
    match GLOBAL_WORKSPACE_SERVICE.set(service) {
        Ok(_) => info!("Global workspace service set"),
        Err(_) => info!("Global workspace service already exists, skipping set"),
    }
}

pub fn get_global_workspace_service() -> Option<Arc<WorkspaceService>> {
    GLOBAL_WORKSPACE_SERVICE.get().cloned()
}
