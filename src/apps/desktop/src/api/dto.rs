//! DTO Module

use bitfun_core::service::remote_ssh::normalize_remote_workspace_path;
use bitfun_core::service::workspace::manager::WorkspaceKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceTypeDto {
    SingleProject,
    MultiProject,
    Documentation,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceKindDto {
    Normal,
    Assistant,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatisticsDto {
    pub total_files: usize,
    pub total_lines: usize,
    pub total_size: usize,
    pub files_by_language: HashMap<String, usize>,
    pub files_by_extension: HashMap<String, usize>,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIdentityDto {
    pub name: Option<String>,
    pub creature: Option<String>,
    pub vibe: Option<String>,
    pub emoji: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWorktreeInfoDto {
    pub path: String,
    pub branch: Option<String>,
    pub main_repo_path: String,
    pub is_main: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfoDto {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub workspace_type: WorkspaceTypeDto,
    pub workspace_kind: WorkspaceKindDto,
    pub assistant_id: Option<String>,
    pub languages: Vec<String>,
    pub opened_at: String,
    pub last_accessed: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub statistics: Option<ProjectStatisticsDto>,
    pub identity: Option<WorkspaceIdentityDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorkspaceWorktreeInfoDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_name: Option<String>,
}

impl WorkspaceInfoDto {
    pub fn from_workspace_info(
        info: &bitfun_core::service::workspace::manager::WorkspaceInfo,
    ) -> Self {
        let connection_id = info
            .metadata
            .get("connectionId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let connection_name = info
            .metadata
            .get("connectionName")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let root_path = if matches!(info.workspace_kind, WorkspaceKind::Remote) {
            normalize_remote_workspace_path(&info.root_path.to_string_lossy())
        } else {
            info.root_path.to_string_lossy().to_string()
        };

        Self {
            id: info.id.clone(),
            name: info.name.clone(),
            root_path,
            workspace_type: WorkspaceTypeDto::from_workspace_type(&info.workspace_type),
            workspace_kind: WorkspaceKindDto::from_workspace_kind(&info.workspace_kind),
            assistant_id: info.assistant_id.clone(),
            languages: info.languages.clone(),
            opened_at: info.opened_at.to_rfc3339(),
            last_accessed: info.last_accessed.to_rfc3339(),
            description: info.description.clone(),
            tags: info.tags.clone(),
            statistics: info
                .statistics
                .as_ref()
                .map(ProjectStatisticsDto::from_workspace_statistics),
            identity: info
                .identity
                .as_ref()
                .map(WorkspaceIdentityDto::from_workspace_identity),
            worktree: info
                .worktree
                .as_ref()
                .map(WorkspaceWorktreeInfoDto::from_workspace_worktree_info),
            connection_id,
            connection_name,
        }
    }
}

impl WorkspaceIdentityDto {
    pub fn from_workspace_identity(
        identity: &bitfun_core::service::workspace::manager::WorkspaceIdentity,
    ) -> Self {
        Self {
            name: identity.name.clone(),
            creature: identity.creature.clone(),
            vibe: identity.vibe.clone(),
            emoji: identity.emoji.clone(),
        }
    }
}

impl WorkspaceWorktreeInfoDto {
    pub fn from_workspace_worktree_info(
        info: &bitfun_core::service::workspace::manager::WorkspaceWorktreeInfo,
    ) -> Self {
        Self {
            path: info.path.clone(),
            branch: info.branch.clone(),
            main_repo_path: info.main_repo_path.clone(),
            is_main: info.is_main,
        }
    }
}

impl WorkspaceTypeDto {
    pub fn from_workspace_type(
        workspace_type: &bitfun_core::service::workspace::manager::WorkspaceType,
    ) -> Self {
        use bitfun_core::service::workspace::manager::WorkspaceType;
        match workspace_type {
            WorkspaceType::RustProject
            | WorkspaceType::NodeProject
            | WorkspaceType::PythonProject
            | WorkspaceType::JavaProject
            | WorkspaceType::CppProject
            | WorkspaceType::WebProject
            | WorkspaceType::MobileProject => WorkspaceTypeDto::SingleProject,
            WorkspaceType::Other => WorkspaceTypeDto::Other,
        }
    }
}

impl WorkspaceKindDto {
    pub fn from_workspace_kind(
        workspace_kind: &bitfun_core::service::workspace::manager::WorkspaceKind,
    ) -> Self {
        use bitfun_core::service::workspace::manager::WorkspaceKind;
        match workspace_kind {
            WorkspaceKind::Normal => WorkspaceKindDto::Normal,
            WorkspaceKind::Assistant => WorkspaceKindDto::Assistant,
            WorkspaceKind::Remote => WorkspaceKindDto::Remote,
        }
    }
}

impl ProjectStatisticsDto {
    pub fn from_workspace_statistics(
        stats: &bitfun_core::service::workspace::manager::WorkspaceStatistics,
    ) -> Self {
        Self {
            total_files: stats.total_files,
            total_lines: 0, // Temporarily set to 0 as the internal structure lacks this field
            total_size: stats.total_size_bytes as usize,
            files_by_language: HashMap::new(), // Temporarily empty, requires future implementation
            files_by_extension: stats.file_extensions.clone(),
            last_updated: stats
                .last_modified
                .map_or_else(|| chrono::Utc::now().to_rfc3339(), |dt| dt.to_rfc3339()),
        }
    }
}
