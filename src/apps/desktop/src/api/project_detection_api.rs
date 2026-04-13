//! Workspace project detection (shared with language tooling; no LSP).

use bitfun_core::service::project_detection::ProjectDetector;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectProjectRequest {
    pub workspace_path: String,
}

#[tauri::command]
pub async fn detect_project(request: DetectProjectRequest) -> Result<serde_json::Value, String> {
    let workspace_path = PathBuf::from(&request.workspace_path);
    let project_info = ProjectDetector::detect(&workspace_path)
        .await
        .map_err(|e| format!("Failed to detect project: {}", e))?;

    serde_json::to_value(&project_info)
        .map_err(|e| format!("Failed to serialize project info: {}", e))
}
