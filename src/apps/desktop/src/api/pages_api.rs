//! BitFun Page Tauri commands (Save Version → Deploy).

use bitfun_services_integrations::remote_connect::{
    delete_page_version_on_relay, deploy_page_version_on_relay, list_page_versions_from_relay,
    list_pages_from_relay, publish_page_to_relay, save_page_version_to_relay,
    unpublish_page_from_relay, update_page_on_relay, PageInfo, PagePublishResult,
    PageSaveVersionResult, PageVersionInfo,
};
use serde::Deserialize;

use super::remote_connect_api::read_account_context;

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
}

#[derive(Debug, Deserialize)]
pub struct PageUpdateRequest {
    pub slug: String,
    pub visibility: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PageDeployRequest {
    pub slug: String,
    pub version_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PageDeleteVersionRequest {
    pub slug: String,
    pub version_id: String,
}

/// Save a new immutable version (does not change production).
#[tauri::command]
pub async fn page_save_version(
    request: PagePublishRequest,
) -> Result<PageSaveVersionResult, String> {
    let (session, relay_url) = read_account_context().await?;
    save_page_version_to_relay(
        &relay_url,
        &session.token,
        &request.directory,
        &request.slug,
        &request.visibility,
        request.title.as_deref(),
        request.note.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

/// Legacy alias for [`page_save_version`] (save only, no deploy).
#[tauri::command]
pub async fn page_publish(request: PagePublishRequest) -> Result<PagePublishResult, String> {
    let (session, relay_url) = read_account_context().await?;
    publish_page_to_relay(
        &relay_url,
        &session.token,
        &request.directory,
        &request.slug,
        &request.visibility,
        request.title.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_list() -> Result<Vec<PageInfo>, String> {
    let (session, relay_url) = read_account_context().await?;
    list_pages_from_relay(&relay_url, &session.token)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_list_versions(request: PageSlugRequest) -> Result<Vec<PageVersionInfo>, String> {
    let (session, relay_url) = read_account_context().await?;
    list_page_versions_from_relay(&relay_url, &session.token, &request.slug)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_deploy(request: PageDeployRequest) -> Result<PageInfo, String> {
    let (session, relay_url) = read_account_context().await?;
    deploy_page_version_on_relay(
        &relay_url,
        &session.token,
        &request.slug,
        &request.version_id,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_delete_version(request: PageDeleteVersionRequest) -> Result<(), String> {
    let (session, relay_url) = read_account_context().await?;
    delete_page_version_on_relay(
        &relay_url,
        &session.token,
        &request.slug,
        &request.version_id,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_update(request: PageUpdateRequest) -> Result<PageInfo, String> {
    let (session, relay_url) = read_account_context().await?;
    update_page_on_relay(
        &relay_url,
        &session.token,
        &request.slug,
        request.visibility.as_deref(),
        request.title.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn page_unpublish(request: PageSlugRequest) -> Result<(), String> {
    let (session, relay_url) = read_account_context().await?;
    unpublish_page_from_relay(&relay_url, &session.token, &request.slug)
        .await
        .map_err(|e| e.to_string())
}
