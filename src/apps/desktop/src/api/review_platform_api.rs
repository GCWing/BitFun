//! Review platform Tauri commands.

use crate::api::app_state::AppState;
use bitfun_core::service::review_platform::{
    ReviewPlatformCiLog, ReviewPlatformDetailSection, ReviewPlatformError,
    ReviewPlatformIssueEvidence, ReviewPlatformKind, ReviewPlatformPullRequestDetail,
    ReviewPlatformPullRequestDetailPage, ReviewPlatformPullRequestReviewTarget,
    ReviewPlatformService, ReviewPlatformWorkspaceSnapshot,
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
pub async fn review_platform_get_issue(
    _state: State<'_, AppState>,
    request: ReviewPlatformIssueRequest,
) -> Result<ReviewPlatformIssueEvidence, String> {
    ReviewPlatformService::issue(
        request.platform,
        &request.host,
        &request.project_path,
        &request.issue_id,
        request.page,
        request.per_page,
        request.repository_path.as_deref(),
    )
    .await
    .map_err(|error| {
        let safe_error = safe_review_platform_error(&error);
        error!(
            "Failed to get review platform Issue: platform={:?}, host={}, project_path={}, issue_id={}, error={}",
            request.platform,
            request.host,
            request.project_path,
            request.issue_id,
            safe_error
        );
        format!("Failed to get provider Issue evidence: {safe_error}")
    })
}

#[tauri::command]
pub async fn review_platform_get_pull_request_review_target_by_identity(
    _state: State<'_, AppState>,
    request: ReviewPlatformPullRequestIdentityRequest,
) -> Result<ReviewPlatformPullRequestReviewTarget, String> {
    ReviewPlatformService::pull_request_review_target_by_identity(
        request.platform,
        &request.host,
        &request.project_path,
        &request.pull_request_id,
        request.repository_path.as_deref(),
    )
    .await
    .map_err(|error| {
        let safe_error = safe_review_platform_error(&error);
        error!(
            "Failed to prepare review platform pull request target by identity: platform={:?}, host={}, project_path={}, pull_request_id={}, error={}",
            request.platform,
            request.host,
            request.project_path,
            request.pull_request_id,
            safe_error
        );
        format!("Failed to prepare pull request Review target: {safe_error}")
    })
}

fn safe_review_platform_error(error: &ReviewPlatformError) -> String {
    match error {
        ReviewPlatformError::Http { status, .. } => format!("provider returned HTTP {status}"),
        ReviewPlatformError::Network(_) => "provider network request failed".to_string(),
        ReviewPlatformError::Parse(_) => "provider response could not be parsed".to_string(),
        ReviewPlatformError::StaleTarget(_) => {
            "provider target changed during evidence acquisition".to_string()
        }
        ReviewPlatformError::EvidenceTooLarge { .. } => {
            "provider evidence exceeded the allowed size".to_string()
        }
        ReviewPlatformError::TargetIsPullRequest { .. } => {
            "requested Issue is a pull request".to_string()
        }
        ReviewPlatformError::InvalidRepository(_) => "invalid repository".to_string(),
        ReviewPlatformError::RemoteNotFound(_) => "provider remote was not found".to_string(),
        ReviewPlatformError::UnsupportedPlatform(_) => "unsupported provider".to_string(),
        ReviewPlatformError::Api(_) => "provider request was rejected".to_string(),
    }
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformIssueRequest {
    pub platform: ReviewPlatformKind,
    pub host: String,
    pub project_path: String,
    pub issue_id: String,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub repository_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformPullRequestIdentityRequest {
    pub platform: ReviewPlatformKind,
    pub host: String,
    pub project_path: String,
    pub pull_request_id: String,
    pub repository_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn review_platform_request_wire_deserializes_issue_identity_fields() {
        let request: ReviewPlatformIssueRequest = serde_json::from_value(json!({
            "platform": "github",
            "host": "github.com",
            "projectPath": "example/repo",
            "issueId": "42",
            "repositoryPath": "D:/workspace/example",
            "page": 2,
            "perPage": 100
        }))
        .expect("Issue request wire should deserialize");

        assert_eq!(request.platform, ReviewPlatformKind::Github);
        assert_eq!(request.host, "github.com");
        assert_eq!(request.project_path, "example/repo");
        assert_eq!(request.issue_id, "42");
        assert_eq!(
            request.repository_path.as_deref(),
            Some("D:/workspace/example")
        );
        assert_eq!(request.page, Some(2));
        assert_eq!(request.per_page, Some(100));
    }

    #[test]
    fn review_platform_request_wire_deserializes_pull_request_identity_fields() {
        let request: ReviewPlatformPullRequestIdentityRequest = serde_json::from_value(json!({
            "platform": "gitlab",
            "host": "gitlab.com",
            "projectPath": "example/group/repo",
            "pullRequestId": "7",
            "repositoryPath": "D:/workspace/example"
        }))
        .expect("pull request identity wire should deserialize");

        assert_eq!(request.platform, ReviewPlatformKind::Gitlab);
        assert_eq!(request.host, "gitlab.com");
        assert_eq!(request.project_path, "example/group/repo");
        assert_eq!(request.pull_request_id, "7");
        assert_eq!(
            request.repository_path.as_deref(),
            Some("D:/workspace/example")
        );
    }
}
