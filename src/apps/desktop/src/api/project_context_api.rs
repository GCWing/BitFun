//! Project Context API

use bitfun_core::service::project_context::{
    CategoryInfo, ContextDocumentStatus, FileConflictAction, ImportedDocument,
    ProjectContextConfig, ProjectContextService,
};
use std::path::Path;

#[tauri::command]
pub async fn get_document_statuses(
    workspace_path: String,
) -> Result<Vec<ContextDocumentStatus>, String> {
    let service = ProjectContextService::new();
    let workspace = Path::new(&workspace_path);

    service
        .get_document_statuses(workspace)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_document_enabled(
    workspace_path: String,
    doc_id: String,
    enabled: bool,
) -> Result<(), String> {
    let service = ProjectContextService::new();
    let workspace = Path::new(&workspace_path);

    service
        .toggle_document(workspace, &doc_id, enabled)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_context_document(
    workspace_path: String,
    doc_id: String,
) -> Result<String, String> {
    let service = ProjectContextService::new();
    let workspace = Path::new(&workspace_path);

    service
        .create_document(workspace, &doc_id)
        .await
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn generate_context_document(
    workspace_path: String,
    doc_id: String,
) -> Result<String, String> {
    let service = ProjectContextService::new();
    let workspace = Path::new(&workspace_path);

    service
        .generate_document(workspace, &doc_id)
        .await
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_context_document_generation(
    workspace_path: String,
    doc_id: String,
) -> Result<(), String> {
    let service = ProjectContextService::new();
    let _workspace = Path::new(&workspace_path);

    service
        .cancel_generate_document(&doc_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_project_context_config(
    workspace_path: String,
) -> Result<ProjectContextConfig, String> {
    let service = ProjectContextService::new();
    let workspace = Path::new(&workspace_path);

    service
        .load_config(workspace)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_project_context_config(
    workspace_path: String,
    config: ProjectContextConfig,
) -> Result<(), String> {
    let service = ProjectContextService::new();
    let workspace = Path::new(&workspace_path);

    service
        .save_config(workspace, &config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_project_category(
    workspace_path: String,
    name: String,
    description: Option<String>,
    icon: String,
) -> Result<String, String> {
    let service = ProjectContextService::new();
    let workspace = Path::new(&workspace_path);

    service
        .create_category(workspace, name, description, icon)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_project_category(
    workspace_path: String,
    category_id: String,
) -> Result<(), String> {
    let service = ProjectContextService::new();
    let workspace = Path::new(&workspace_path);

    service
        .delete_category(workspace, &category_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_all_categories(workspace_path: String) -> Result<Vec<CategoryInfo>, String> {
    let service = ProjectContextService::new();
    let workspace = Path::new(&workspace_path);

    service
        .get_all_categories(workspace)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_project_document(
    workspace_path: String,
    source_path: String,
    name: String,
    category_id: String,
    priority: String,
    on_conflict: String,
) -> Result<ImportedDocument, String> {
    let service = ProjectContextService::new();
    let workspace = Path::new(&workspace_path);
    let source = Path::new(&source_path);

    let conflict_action = match on_conflict.as_str() {
        "skip" => FileConflictAction::Skip,
        "overwrite" => FileConflictAction::Overwrite,
        "rename" => FileConflictAction::Rename,
        _ => FileConflictAction::Rename,
    };

    service
        .import_document(
            workspace,
            source,
            name,
            category_id,
            priority,
            conflict_action,
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_imported_document(
    workspace_path: String,
    doc_id: String,
) -> Result<(), String> {
    let service = ProjectContextService::new();
    let workspace = Path::new(&workspace_path);

    service
        .delete_imported_document(workspace, &doc_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_imported_document_enabled(
    workspace_path: String,
    doc_id: String,
    enabled: bool,
) -> Result<(), String> {
    let service = ProjectContextService::new();
    let workspace = Path::new(&workspace_path);

    service
        .toggle_imported_document(workspace, &doc_id, enabled)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_context_document(workspace_path: String, doc_id: String) -> Result<(), String> {
    let service = ProjectContextService::new();
    let workspace = Path::new(&workspace_path);

    service
        .delete_context_document(workspace, &doc_id)
        .await
        .map_err(|e| e.to_string())
}
