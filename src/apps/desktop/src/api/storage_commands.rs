//! Storage Management API

use crate::api::AppState;
use bitfun_core::infrastructure::storage::{CleanupPolicy, CleanupResult, CleanupService};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoragePathsInfo {
    pub user_config_dir: PathBuf,
    pub user_data_dir: PathBuf,
    pub cache_root: PathBuf,
    pub logs_dir: PathBuf,
    pub backups_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub workspaces_dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub total_size_mb: f64,
    pub config_size_mb: f64,
    pub cache_size_mb: f64,
    pub logs_size_mb: f64,
    pub temp_size_mb: f64,
}

#[tauri::command]
pub async fn get_storage_paths(state: State<'_, AppState>) -> Result<StoragePathsInfo, String> {
    let workspace_service = &state.workspace_service;
    let path_manager = workspace_service.path_manager();

    Ok(StoragePathsInfo {
        user_config_dir: path_manager.user_config_dir(),
        user_data_dir: path_manager.user_data_dir(),
        cache_root: path_manager.cache_root(),
        logs_dir: path_manager.logs_dir(),
        backups_dir: path_manager.backups_dir(),
        temp_dir: path_manager.temp_dir(),
        workspaces_dir: path_manager.workspaces_dir(),
    })
}

#[tauri::command]
pub async fn get_project_storage_paths(
    state: State<'_, AppState>,
    workspace_path: String,
) -> Result<ProjectStoragePathsInfo, String> {
    let workspace_service = &state.workspace_service;
    let path_manager = workspace_service.path_manager();

    let workspace_path = PathBuf::from(workspace_path);

    Ok(ProjectStoragePathsInfo {
        project_root: path_manager.project_root(&workspace_path),
        config_file: path_manager.project_config_file(&workspace_path),
        agents_dir: path_manager.project_agents_dir(&workspace_path),
        sessions_dir: path_manager.project_sessions_dir(&workspace_path),
        cache_dir: path_manager.project_cache_dir(&workspace_path),
        logs_dir: path_manager.project_logs_dir(&workspace_path),
    })
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStoragePathsInfo {
    pub project_root: PathBuf,
    pub config_file: PathBuf,
    pub agents_dir: PathBuf,
    pub sessions_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub logs_dir: PathBuf,
}

#[tauri::command]
pub async fn cleanup_storage(state: State<'_, AppState>) -> Result<CleanupResult, String> {
    let workspace_service = &state.workspace_service;
    let path_manager = workspace_service.path_manager();

    let policy = CleanupPolicy::default();
    let cleanup_service = CleanupService::new((&**path_manager).clone(), policy);

    cleanup_service
        .cleanup_all()
        .await
        .map_err(|e| format!("Cleanup failed: {}", e))
}

#[tauri::command]
pub async fn cleanup_storage_with_policy(
    state: State<'_, AppState>,
    policy: CleanupPolicy,
) -> Result<CleanupResult, String> {
    let workspace_service = &state.workspace_service;
    let path_manager = workspace_service.path_manager();

    let cleanup_service = CleanupService::new((&**path_manager).clone(), policy);

    cleanup_service
        .cleanup_all()
        .await
        .map_err(|e| format!("Cleanup failed: {}", e))
}

#[tauri::command]
pub async fn get_storage_statistics(state: State<'_, AppState>) -> Result<StorageStats, String> {
    let workspace_service = &state.workspace_service;
    let path_manager = workspace_service.path_manager();

    let config_size = calculate_dir_size(&path_manager.user_config_dir()).await?;
    let cache_size = calculate_dir_size(&path_manager.cache_root()).await?;
    let logs_size = calculate_dir_size(&path_manager.logs_dir()).await?;
    let temp_size = calculate_dir_size(&path_manager.temp_dir()).await?;

    let total_size = config_size + cache_size + logs_size + temp_size;

    Ok(StorageStats {
        total_size_mb: bytes_to_mb(total_size),
        config_size_mb: bytes_to_mb(config_size),
        cache_size_mb: bytes_to_mb(cache_size),
        logs_size_mb: bytes_to_mb(logs_size),
        temp_size_mb: bytes_to_mb(temp_size),
    })
}

#[tauri::command]
pub async fn initialize_project_storage(
    state: State<'_, AppState>,
    workspace_path: String,
) -> Result<(), String> {
    let workspace_service = &state.workspace_service;
    let path_manager = workspace_service.path_manager();

    let workspace_path = PathBuf::from(workspace_path);

    path_manager
        .initialize_project_directories(&workspace_path)
        .await
        .map_err(|e| format!("Failed to initialize project directories: {}", e))
}

fn calculate_dir_size(
    dir: &std::path::Path,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64, String>> + Send + '_>> {
    Box::pin(async move {
        let mut total = 0u64;

        if !dir.exists() {
            return Ok(0);
        }

        let mut read_dir = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| format!("Failed to read directory: {}", e))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| format!("Failed to read directory entry: {}", e))?
        {
            let metadata = entry
                .metadata()
                .await
                .map_err(|e| format!("Failed to get metadata: {}", e))?;

            if metadata.is_dir() {
                total += calculate_dir_size(&entry.path()).await?;
            } else {
                total += metadata.len();
            }
        }

        Ok(total)
    })
}

fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / 1_048_576.0
}
