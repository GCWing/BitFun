//! Workspace service - advanced workspace management API
//!
//! Provides comprehensive workspace management functionality.

use super::manager::{
    ScanOptions, WorkspaceInfo, WorkspaceManager, WorkspaceManagerConfig,
    WorkspaceManagerStatistics, WorkspaceStatus, WorkspaceSummary, WorkspaceType,
};
use crate::infrastructure::set_workspace_path;
use crate::infrastructure::storage::{PersistenceService, StorageOptions};
use crate::infrastructure::{try_get_path_manager_arc, PathManager};
use crate::util::errors::*;
use log::{info, warn};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
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
    pub description: Option<String>,
    pub tags: Vec<String>,
}

impl Default for WorkspaceCreateOptions {
    fn default() -> Self {
        Self {
            scan_options: ScanOptions::default(),
            auto_set_current: true,
            add_to_recent: true,
            description: None,
            tags: Vec::new(),
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
        let result = {
            let mut manager = self.manager.write().await;
            manager.open_workspace(path).await
        };

        if result.is_ok() {
            if let Err(e) = self.save_workspace_data().await {
                warn!("Failed to save workspace data after opening: {}", e);
            }
            self.sync_global_workspace_path().await;
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

        let mut manager = self.manager.write().await;
        let mut workspace = manager.open_workspace(path).await?;

        if let Some(description) = options.description {
            workspace.description = Some(description);
        }

        workspace.tags = options.tags;

        manager
            .get_workspaces_mut()
            .insert(workspace.id.clone(), workspace.clone());

        drop(manager);
        self.sync_global_workspace_path().await;

        Ok(workspace)
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
            self.sync_global_workspace_path().await;
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
            self.sync_global_workspace_path().await;
        }

        result
    }

    /// Switches to the specified workspace.
    pub async fn switch_to_workspace(&self, workspace_id: &str) -> BitFunResult<()> {
        let result = {
            let mut manager = self.manager.write().await;
            manager.set_current_workspace(workspace_id.to_string())
        };

        if result.is_ok() {
            self.sync_global_workspace_path().await;
        }

        result
    }

    /// Returns the current workspace.
    pub async fn get_current_workspace(&self) -> Option<WorkspaceInfo> {
        let manager = self.manager.read().await;
        manager.get_current_workspace().cloned()
    }

    /// Returns workspace details.
    pub async fn get_workspace(&self, workspace_id: &str) -> Option<WorkspaceInfo> {
        let manager = self.manager.read().await;
        manager.get_workspace(workspace_id).cloned()
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
            self.sync_global_workspace_path().await;
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

        let new_workspace = WorkspaceInfo::new(workspace_path, ScanOptions::default()).await?;

        {
            let mut manager = self.manager.write().await;
            manager
                .get_workspaces_mut()
                .insert(workspace_id.to_string(), new_workspace.clone());
        }

        Ok(new_workspace)
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
            self.sync_global_workspace_path().await;
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
        self.sync_global_workspace_path().await;

        Ok(result)
    }

    /// Returns a quick summary.
    pub async fn get_quick_summary(&self) -> WorkspaceQuickSummary {
        let stats = self.get_statistics().await;
        let current_workspace = self.get_current_workspace().await;
        let recent_workspaces = self.get_recent_workspaces().await;

        WorkspaceQuickSummary {
            total_workspaces: stats.total_workspaces,
            active_workspaces: stats.active_workspaces,
            current_workspace: current_workspace.map(|w| w.get_summary()),
            recent_workspaces: recent_workspaces
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
            current_workspace_id: manager.get_current_workspace().map(|w| w.id.clone()),
            recent_workspaces: manager.get_recent_workspaces().clone(),
            saved_at: chrono::Utc::now(),
        };

        self.persistence
            .save_json("workspace_data", &workspace_data, StorageOptions::default())
            .await
            .map_err(|e| BitFunError::service(format!("Failed to save workspace data: {}", e)))?;

        Ok(())
    }

    async fn sync_global_workspace_path(&self) {
        let path = self
            .get_current_workspace()
            .await
            .map(|workspace| workspace.root_path);
        set_workspace_path(path);
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
            manager.set_recent_workspaces(data.recent_workspaces);

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
            manager.set_recent_workspaces(data.recent_workspaces);
        }

        Ok(())
    }

    /// Saves workspace data manually (public API).
    pub async fn manual_save(&self) -> BitFunResult<()> {
        self.save_workspace_data().await
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
    pub workspace_types: std::collections::HashMap<WorkspaceType, usize>,
}

/// Workspace persistence data.
#[derive(Debug, Serialize, Deserialize)]
struct WorkspacePersistenceData {
    pub workspaces: std::collections::HashMap<String, WorkspaceInfo>,
    pub current_workspace_id: Option<String>,
    pub recent_workspaces: Vec<String>,
    pub saved_at: chrono::DateTime<chrono::Utc>,
}
