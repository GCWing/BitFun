use super::manager::{
    WorkspaceInfo, WorkspaceOpenOptions, WorkspaceStatistics, WorkspaceSummary, WorkspaceType,
};
use super::service::{
    BatchImportResult, WorkspaceCreateOptions, WorkspaceHealthStatus, WorkspaceService,
};
use crate::util::errors::{BitFunError, BitFunResult};
use std::path::PathBuf;
use std::sync::Arc;

/// Workspace provider - simplified workspace access API
pub struct WorkspaceProvider {
    service: Arc<WorkspaceService>,
}

impl WorkspaceProvider {
    /// Creates a new workspace provider.
    pub async fn new() -> BitFunResult<Self> {
        let service = Arc::new(WorkspaceService::new().await?);
        Ok(Self { service })
    }

    /// Creates a workspace provider with a custom service.
    pub fn with_service(service: Arc<WorkspaceService>) -> Self {
        Self { service }
    }

    /// Quick-opens a workspace.
    pub async fn open(&self, path: &str) -> BitFunResult<WorkspaceInfo> {
        self.service.quick_open(path).await
    }

    /// Quickly creates a new project workspace.
    pub async fn create_project(
        &self,
        path: &str,
        project_type: WorkspaceType,
    ) -> BitFunResult<WorkspaceInfo> {
        let path_buf = PathBuf::from(path);

        let options = WorkspaceCreateOptions {
            tags: vec![format!("{:?}", project_type)],
            ..Default::default()
        };

        self.service.create_workspace(path_buf, options).await
    }

    /// Returns the current workspace.
    pub async fn current(&self) -> Option<WorkspaceInfo> {
        self.service.get_current_workspace().await
    }

    /// Switches to a workspace.
    pub async fn switch(&self, workspace_id: &str) -> BitFunResult<()> {
        self.service.switch_to_workspace(workspace_id).await
    }

    /// Lists recent workspaces.
    pub async fn recent(&self, limit: usize) -> Vec<WorkspaceInfo> {
        let mut recent = self.service.get_recent_workspaces().await;
        recent.truncate(limit);
        recent
    }

    /// Searches workspaces.
    pub async fn search(&self, query: &str) -> Vec<WorkspaceSummary> {
        self.service.search_workspaces(query).await
    }

    /// Lists workspaces by type.
    pub async fn by_type(&self, workspace_type: WorkspaceType) -> Vec<WorkspaceSummary> {
        self.service.list_workspaces_by_type(workspace_type).await
    }

    /// Closes the current workspace.
    pub async fn close_current(&self) -> BitFunResult<()> {
        self.service.close_current_workspace().await
    }

    /// Returns the service reference (for advanced operations).
    pub fn get_service(&self) -> Arc<WorkspaceService> {
        self.service.clone()
    }

    /// Returns the workspace summary.
    pub async fn get_summary(&self) -> WorkspaceSystemSummary {
        let quick_summary = self.service.get_quick_summary().await;
        let health = self
            .service
            .health_check()
            .await
            .unwrap_or_else(|_| WorkspaceHealthStatus {
                healthy: false,
                total_workspaces: 0,
                active_workspaces: 0,
                current_workspace_valid: false,
                total_files: 0,
                total_size_mb: 0,
                warnings: vec!["Health check failed".to_string()],
                issues: vec!["Unable to check health".to_string()],
                message: "Health check failed".to_string(),
            });

        WorkspaceSystemSummary {
            total_workspaces: quick_summary.total_workspaces,
            active_workspaces: quick_summary.active_workspaces,
            current_workspace: quick_summary.current_workspace,
            recent_workspaces: quick_summary.recent_workspaces,
            workspace_types: quick_summary.workspace_types,
            healthy: health.healthy,
            warnings: health.warnings,
            total_files: health.total_files,
            total_size_mb: health.total_size_mb,
        }
    }

    /// Quick cleanup.
    pub async fn quick_cleanup(&self) -> BitFunResult<WorkspaceCleanupResult> {
        let invalid_count = self.service.cleanup_invalid_workspaces().await?;

        Ok(WorkspaceCleanupResult {
            invalid_workspaces_removed: invalid_count,
            total_workspaces_after: self.service.get_workspace_count().await,
        })
    }

    /// Batch-imports directories.
    pub async fn import_directories(
        &self,
        directories: Vec<String>,
    ) -> BitFunResult<BatchImportResult> {
        self.service.batch_import_workspaces(directories).await
    }

    /// Detects project type.
    pub async fn detect_project_type(&self, path: &str) -> BitFunResult<WorkspaceType> {
        let path_buf = PathBuf::from(path);

        if !path_buf.exists() {
            return Err(BitFunError::service("Path does not exist".to_string()));
        }

        let temp_workspace = WorkspaceInfo::new(path_buf, WorkspaceOpenOptions::default()).await?;
        Ok(temp_workspace.workspace_type)
    }

    /// Returns workspace file statistics.
    pub async fn get_file_stats(
        &self,
        workspace_id: &str,
    ) -> BitFunResult<Option<WorkspaceStatistics>> {
        if let Some(workspace) = self.service.get_workspace(workspace_id).await {
            Ok(workspace.statistics)
        } else {
            Err(BitFunError::service(format!(
                "Workspace not found: {}",
                workspace_id
            )))
        }
    }

    /// Rescans a workspace.
    pub async fn rescan(&self, workspace_id: &str) -> BitFunResult<WorkspaceInfo> {
        self.service.rescan_workspace(workspace_id).await
    }
}

/// Workspace system summary
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceSystemSummary {
    pub total_workspaces: usize,
    pub active_workspaces: usize,
    pub current_workspace: Option<WorkspaceSummary>,
    pub recent_workspaces: Vec<WorkspaceSummary>,
    pub workspace_types: std::collections::HashMap<WorkspaceType, usize>,
    pub healthy: bool,
    pub warnings: Vec<String>,
    pub total_files: usize,
    pub total_size_mb: u64,
}

/// Workspace cleanup result
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceCleanupResult {
    pub invalid_workspaces_removed: usize,
    pub total_workspaces_after: usize,
}
