//! Compatibility facade for review-platform operations.
//!
//! Provider detection, provider DTO mapping, token persistence, and HTTP/Git
//! integration logic live in `bitfun-services-integrations::review_platform`.
//! Core only preserves the legacy static API, injects BitFun storage paths, and
//! connects the product remote-workspace classifier.

use crate::infrastructure::try_get_path_manager_arc;
use std::sync::Arc;

pub use bitfun_services_integrations::review_platform::{
    ReviewAuthSource, ReviewAuthState, ReviewChecks, ReviewDecision, ReviewFileStatus,
    ReviewItemState, ReviewPlatformAccount, ReviewPlatformActionResult,
    ReviewPlatformApprovalRequest, ReviewPlatformAuthChallenge, ReviewPlatformAuthChallengeState,
    ReviewPlatformCapabilities, ReviewPlatformCiItem, ReviewPlatformCiLog, ReviewPlatformCommit,
    ReviewPlatformCreatePullRequestRequest, ReviewPlatformDetailSection, ReviewPlatformError,
    ReviewPlatformFile, ReviewPlatformKind, ReviewPlatformPullRequest,
    ReviewPlatformPullRequestDetail, ReviewPlatformPullRequestDetailPage, ReviewPlatformRemote,
    ReviewPlatformReplyToThreadRequest, ReviewPlatformRepositoryRef,
    ReviewPlatformRequestChangesRequest, ReviewPlatformResolveThreadRequest,
    ReviewPlatformSubmitReviewRequest, ReviewPlatformThread, ReviewPlatformThreadKind,
    ReviewPlatformWorkspaceSnapshot, ReviewSubmitEvent,
};

use bitfun_services_integrations::review_platform::{
    ReviewPlatformService as ReviewPlatformOwnerService, ReviewPlatformWorkspaceClassifier,
    REVIEW_PLATFORM_TOKEN_FILE_NAME,
};

pub struct ReviewPlatformService;

struct CoreReviewPlatformWorkspaceClassifier;

#[async_trait::async_trait]
impl ReviewPlatformWorkspaceClassifier for CoreReviewPlatformWorkspaceClassifier {
    async fn is_remote_workspace_path(&self, path: &str) -> bool {
        crate::service::remote_ssh::workspace_state::is_remote_path(path).await
    }
}

fn owner_service() -> Result<ReviewPlatformOwnerService, ReviewPlatformError> {
    let path_manager =
        try_get_path_manager_arc().map_err(|error| ReviewPlatformError::Api(error.to_string()))?;
    Ok(ReviewPlatformOwnerService::new(
        path_manager
            .user_data_dir()
            .join(REVIEW_PLATFORM_TOKEN_FILE_NAME),
        Arc::new(CoreReviewPlatformWorkspaceClassifier),
    ))
}

impl ReviewPlatformService {
    pub async fn discover_remotes(
        repository_path: &str,
    ) -> Result<Vec<ReviewPlatformRemote>, ReviewPlatformError> {
        owner_service()?.discover_remotes(repository_path).await
    }

    pub async fn workspace_snapshot(
        repository_path: &str,
        remote_id: Option<&str>,
        page: Option<u32>,
        per_page: Option<u32>,
    ) -> Result<ReviewPlatformWorkspaceSnapshot, ReviewPlatformError> {
        owner_service()?
            .workspace_snapshot(repository_path, remote_id, page, per_page)
            .await
    }

    pub async fn pull_request_detail(
        repository_path: &str,
        remote_id: &str,
        pull_request_id: &str,
    ) -> Result<ReviewPlatformPullRequestDetail, ReviewPlatformError> {
        owner_service()?
            .pull_request_detail(repository_path, remote_id, pull_request_id)
            .await
    }

    pub async fn pull_request_detail_page(
        repository_path: &str,
        remote_id: &str,
        pull_request_id: &str,
        section: ReviewPlatformDetailSection,
        page: Option<u32>,
        per_page: Option<u32>,
    ) -> Result<ReviewPlatformPullRequestDetailPage, ReviewPlatformError> {
        owner_service()?
            .pull_request_detail_page(
                repository_path,
                remote_id,
                pull_request_id,
                section,
                page,
                per_page,
            )
            .await
    }

    pub async fn pull_request_ci_log(
        repository_path: &str,
        remote_id: &str,
        pull_request_id: &str,
        ci_item_id: &str,
        ci_item_name: &str,
    ) -> Result<ReviewPlatformCiLog, ReviewPlatformError> {
        owner_service()?
            .pull_request_ci_log(
                repository_path,
                remote_id,
                pull_request_id,
                ci_item_id,
                ci_item_name,
            )
            .await
    }

    pub async fn create_pull_request(
        request: ReviewPlatformCreatePullRequestRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        owner_service()?.create_pull_request(request).await
    }

    pub async fn reply_to_thread(
        request: ReviewPlatformReplyToThreadRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        owner_service()?.reply_to_thread(request).await
    }

    pub async fn submit_review(
        request: ReviewPlatformSubmitReviewRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        owner_service()?.submit_review(request).await
    }

    pub async fn resolve_thread(
        request: ReviewPlatformResolveThreadRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        owner_service()?.resolve_thread(request).await
    }

    pub async fn approve_pull_request(
        request: ReviewPlatformApprovalRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        owner_service()?.approve_pull_request(request).await
    }

    pub async fn revoke_approval(
        request: ReviewPlatformApprovalRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        owner_service()?.revoke_approval(request).await
    }

    pub async fn request_changes(
        request: ReviewPlatformRequestChangesRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        owner_service()?.request_changes(request).await
    }

    pub async fn update_auth_token(
        platform: ReviewPlatformKind,
        host: &str,
        token: &str,
    ) -> Result<(), ReviewPlatformError> {
        owner_service()?
            .update_auth_token(platform, host, token)
            .await
    }

    pub async fn clear_auth_token(
        platform: ReviewPlatformKind,
        host: &str,
    ) -> Result<(), ReviewPlatformError> {
        owner_service()?.clear_auth_token(platform, host).await
    }
}
