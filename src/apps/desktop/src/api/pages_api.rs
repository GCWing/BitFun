//! BitFun Page Tauri commands (Save Version → Deploy).

use bitfun_services_integrations::remote_connect::{
    create_page_open_link_on_relay, delete_page_from_relay, delete_page_version_on_relay,
    deploy_page_version_on_relay, list_page_versions_from_relay, list_pages_from_relay,
    publish_page_to_relay, save_page_version_to_relay, unpublish_page_from_relay,
    update_page_on_relay, PageInfo, PageOpenLink, PagePublishResult, PageSaveVersionResult,
    PageVersionInfo,
};
use serde::Deserialize;

use super::remote_connect_api::{
    account_context_generation, account_context_is_current, read_account_context_for_generation,
};

fn ensure_page_account_is_current(generation: u64) -> Result<(), String> {
    if account_context_is_current(generation) {
        Ok(())
    } else {
        Err("Account changed while the Page operation was in progress; retry for the current account"
            .to_string())
    }
}

#[derive(Debug, Deserialize)]
pub struct PagePublishRequest {
    pub directory: String,
    pub slug: String,
    pub visibility: String,
    pub title: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PageSlugRequest {
    pub slug: String,
    pub generation: String,
}

#[derive(Debug, Deserialize)]
pub struct PageUpdateRequest {
    pub slug: String,
    pub generation: String,
    pub visibility: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PageDeployRequest {
    pub slug: String,
    pub generation: String,
    pub version_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PageDeleteVersionRequest {
    pub slug: String,
    pub generation: String,
    pub version_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PageOpenRequest {
    pub slug: String,
    pub generation: String,
    pub version_id: Option<String>,
}

/// Save a new immutable version (does not change production).
#[tauri::command]
pub async fn page_save_version(
    request: PagePublishRequest,
) -> Result<PageSaveVersionResult, String> {
    // `directory` is explicitly a desktop-host path. Remote PagePublish tool
    // calls are rejected from their typed workspace context before reaching
    // this adapter; inferring ownership from the global remote-root registry
    // would misclassify every local absolute path when an SSH root is `/`.
    let generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(generation).await?;
    let result = save_page_version_to_relay(
        &relay_url,
        &session.token,
        &request.directory,
        &request.slug,
        &request.visibility,
        request.title.as_deref(),
        request.note.as_deref(),
    )
    .await;
    ensure_page_account_is_current(generation)?;
    result.map_err(|e| e.to_string())
}

/// Legacy alias for [`page_save_version`] (save only, no deploy).
#[tauri::command]
pub async fn page_publish(request: PagePublishRequest) -> Result<PagePublishResult, String> {
    let generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(generation).await?;
    let result = publish_page_to_relay(
        &relay_url,
        &session.token,
        &request.directory,
        &request.slug,
        &request.visibility,
        request.title.as_deref(),
    )
    .await;
    ensure_page_account_is_current(generation)?;
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_list() -> Result<Vec<PageInfo>, String> {
    let generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(generation).await?;
    let result = list_pages_from_relay(&relay_url, &session.token).await;
    ensure_page_account_is_current(generation)?;
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_list_versions(request: PageSlugRequest) -> Result<Vec<PageVersionInfo>, String> {
    let generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(generation).await?;
    let result = list_page_versions_from_relay(
        &relay_url,
        &session.token,
        &request.slug,
        &request.generation,
    )
    .await;
    ensure_page_account_is_current(generation)?;
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_create_open_link(request: PageOpenRequest) -> Result<PageOpenLink, String> {
    let generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(generation).await?;
    let result = create_page_open_link_on_relay(
        &relay_url,
        &session.token,
        &request.slug,
        request.version_id.as_deref(),
        &request.generation,
    )
    .await;
    ensure_page_account_is_current(generation)?;
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_deploy(request: PageDeployRequest) -> Result<PageInfo, String> {
    let generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(generation).await?;
    let result = deploy_page_version_on_relay(
        &relay_url,
        &session.token,
        &request.slug,
        &request.version_id,
        &request.generation,
    )
    .await;
    ensure_page_account_is_current(generation)?;
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_delete_version(request: PageDeleteVersionRequest) -> Result<(), String> {
    let generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(generation).await?;
    let result = delete_page_version_on_relay(
        &relay_url,
        &session.token,
        &request.slug,
        &request.version_id,
        &request.generation,
    )
    .await;
    ensure_page_account_is_current(generation)?;
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_update(request: PageUpdateRequest) -> Result<PageInfo, String> {
    let generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(generation).await?;
    let result = update_page_on_relay(
        &relay_url,
        &session.token,
        &request.slug,
        &request.generation,
        request.visibility.as_deref(),
        request.title.as_deref(),
    )
    .await;
    ensure_page_account_is_current(generation)?;
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_unpublish(request: PageSlugRequest) -> Result<(), String> {
    let generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(generation).await?;
    let result = unpublish_page_from_relay(
        &relay_url,
        &session.token,
        &request.slug,
        &request.generation,
    )
    .await;
    ensure_page_account_is_current(generation)?;
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_delete(request: PageSlugRequest) -> Result<(), String> {
    let generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(generation).await?;
    let result = delete_page_from_relay(
        &relay_url,
        &session.token,
        &request.slug,
        &request.generation,
    )
    .await;
    ensure_page_account_is_current(generation)?;
    result.map_err(|e| e.to_string())
}
