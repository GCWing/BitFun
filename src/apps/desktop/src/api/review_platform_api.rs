//! Review platform Tauri commands.

use crate::api::app_state::AppState;
use bitfun_core::service::review_platform::{
    ReviewPlatformCiLog, ReviewPlatformDetailSection, ReviewPlatformKind,
    ReviewPlatformPullRequestDetail, ReviewPlatformPullRequestDetailPage,
    ReviewPlatformPullRequestReviewTarget, ReviewPlatformService, ReviewPlatformWorkspaceSnapshot,
};
use log::error;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformWorkspaceSnapshotRequest {
    pub repository_path: String,
    pub remote_id: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformPullRequestDetailRequest {
    pub repository_path: String,
    pub remote_id: String,
    pub pull_request_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformPullRequestDetailPageRequest {
    pub repository_path: String,
    pub remote_id: String,
    pub pull_request_id: String,
    pub section: ReviewPlatformDetailSection,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformPullRequestCiLogRequest {
    pub repository_path: String,
    pub remote_id: String,
    pub pull_request_id: String,
    pub ci_item_id: String,
    pub ci_item_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformUpdateAuthTokenRequest {
    pub platform: ReviewPlatformKind,
    pub host: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformClearAuthTokenRequest {
    pub platform: ReviewPlatformKind,
    pub host: String,
}

#[tauri::command]
pub async fn review_platform_get_workspace_snapshot(
    _state: State<'_, AppState>,
    request: ReviewPlatformWorkspaceSnapshotRequest,
) -> Result<ReviewPlatformWorkspaceSnapshot, String> {
    ReviewPlatformService::workspace_snapshot(
        &request.repository_path,
        request.remote_id.as_deref(),
        request.page,
        request.per_page,
    )
    .await
    .map_err(|error| {
        error!(
            "Failed to get review platform workspace snapshot: path={}, remote_id={:?}, error={}",
            request.repository_path, request.remote_id, error
        );
        format!(
            "Failed to get review platform workspace snapshot: {}",
            error
        )
    })
}

#[tauri::command]
pub async fn review_platform_get_pull_request_detail(
    _state: State<'_, AppState>,
    request: ReviewPlatformPullRequestDetailRequest,
) -> Result<ReviewPlatformPullRequestDetail, String> {
    ReviewPlatformService::pull_request_detail(
        &request.repository_path,
        &request.remote_id,
        &request.pull_request_id,
    )
    .await
    .map_err(|error| {
        error!(
            "Failed to get review platform pull request detail: path={}, remote_id={}, pull_request_id={}, error={}",
            request.repository_path,
            request.remote_id,
            request.pull_request_id,
            error
        );
        format!("Failed to get review platform pull request detail: {}", error)
    })
}

#[tauri::command]
pub async fn review_platform_get_pull_request_review_target(
    _state: State<'_, AppState>,
    request: ReviewPlatformPullRequestDetailRequest,
) -> Result<ReviewPlatformPullRequestReviewTarget, String> {
    ReviewPlatformService::pull_request_review_target(
        &request.repository_path,
        &request.remote_id,
        &request.pull_request_id,
    )
    .await
    .map_err(|error| {
        error!(
            "Failed to prepare review platform pull request target: path={}, remote_id={}, pull_request_id={}, error={}",
            request.repository_path,
            request.remote_id,
            request.pull_request_id,
            error
        );
        format!("Failed to prepare pull request Review target: {}", error)
    })
}

#[tauri::command]
pub async fn review_platform_get_pull_request_detail_page(
    _state: State<'_, AppState>,
    request: ReviewPlatformPullRequestDetailPageRequest,
) -> Result<ReviewPlatformPullRequestDetailPage, String> {
    ReviewPlatformService::pull_request_detail_page(
        &request.repository_path,
        &request.remote_id,
        &request.pull_request_id,
        request.section,
        request.page,
        request.per_page,
    )
    .await
    .map_err(|error| {
        error!(
            "Failed to get review platform pull request detail page: path={}, remote_id={}, pull_request_id={}, section={:?}, page={:?}, per_page={:?}, error={}",
            request.repository_path,
            request.remote_id,
            request.pull_request_id,
            request.section,
            request.page,
            request.per_page,
            error
        );
        format!(
            "Failed to get review platform pull request detail page: {}",
            error
        )
    })
}

#[tauri::command]
pub async fn review_platform_get_pull_request_ci_log(
    _state: State<'_, AppState>,
    request: ReviewPlatformPullRequestCiLogRequest,
) -> Result<ReviewPlatformCiLog, String> {
    ReviewPlatformService::pull_request_ci_log(
        &request.repository_path,
        &request.remote_id,
        &request.pull_request_id,
        &request.ci_item_id,
        &request.ci_item_name,
    )
    .await
    .map_err(|error| {
        error!(
            "Failed to get review platform CI log: path={}, remote_id={}, pull_request_id={}, ci_item_id={}, error={}",
            request.repository_path,
            request.remote_id,
            request.pull_request_id,
            request.ci_item_id,
            error
        );
        format!("Failed to get review platform CI log: {}", error)
    })
}

#[tauri::command]
pub async fn review_platform_update_auth_token(
    _state: State<'_, AppState>,
    request: ReviewPlatformUpdateAuthTokenRequest,
) -> Result<(), String> {
    ReviewPlatformService::update_auth_token(request.platform, &request.host, &request.token)
        .await
        .map_err(|error| {
            error!(
                "Failed to update review platform auth token: platform={:?}, host={}, error={}",
                request.platform, request.host, error
            );
            format!("Failed to update review platform auth token: {}", error)
        })
}

#[tauri::command]
pub async fn review_platform_clear_auth_token(
    _state: State<'_, AppState>,
    request: ReviewPlatformClearAuthTokenRequest,
) -> Result<(), String> {
    ReviewPlatformService::clear_auth_token(request.platform, &request.host)
        .await
        .map_err(|error| {
            error!(
                "Failed to clear review platform auth token: platform={:?}, host={}, error={}",
                request.platform, request.host, error
            );
            format!("Failed to clear review platform auth token: {}", error)
        })
}
