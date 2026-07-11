//! Platform-neutral pull request review data service.
//!
//! This module owns provider detection, token handling, provider DTO mapping,
//! and provider-neutral review-platform response semantics. Concrete HTTP
//! transport lives in `review_platform_http`.

use crate::review_platform_http::{
    send_json as send_review_json, send_json_response as send_review_json_response,
    send_json_response_bounded as send_review_json_response_bounded, send_text as send_review_text,
    ReviewHttpClient, ReviewHttpError, ReviewHttpHeaders, ReviewHttpRequest, ReviewJsonResponse,
};
use bitfun_services_core::process_manager;
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

pub const REVIEW_PLATFORM_TOKEN_FILE_NAME: &str = "review-platform-tokens.json";

const USER_AGENT_VALUE: &str = "ReviewPlatform";
const ACCEPT_HEADER: &str = "accept";
const AUTHORIZATION_HEADER: &str = "authorization";
const USER_AGENT_HEADER: &str = "user-agent";
const DEFAULT_PR_PAGE: u32 = 1;
const DEFAULT_PR_PAGE_SIZE: u32 = 10;
const MAX_PR_PAGE_SIZE: u32 = 50;
const PROVIDER_ENRICH_CONCURRENCY: usize = 4;
const MAX_CI_LOG_CHARS: usize = 80_000;
const MAX_REVIEW_TARGET_FILES: usize = 500;
const MAX_REVIEW_TARGET_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_REVIEW_FILE_DIFF_CHARS: usize = 80_000;

#[derive(Debug, thiserror::Error)]
pub enum ReviewPlatformError {
    #[error("Invalid repository path: {0}")]
    InvalidRepository(String),
    #[error("Remote not found: {0}")]
    RemoteNotFound(String),
    #[error("Unsupported review platform: {0}")]
    UnsupportedPlatform(String),
    #[error("Provider API failed: {0}")]
    Api(String),
    #[error("Provider API failed: HTTP {status}{message}")]
    Http { status: u16, message: String },
    #[error("Network error: {0}")]
    Network(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Pull request target changed: {0}")]
    StaleTarget(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewPlatformKind {
    Github,
    Gitlab,
    Gitcode,
    Unknown,
}

impl ReviewPlatformKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Gitlab => "gitlab",
            Self::Gitcode => "gitcode",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewAuthState {
    NotConnected,
    NotRequired,
    Connected,
    Expired,
    Error,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewAuthSource {
    Env,
    Stored,
    None,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewItemState {
    Open,
    Merged,
    Closed,
    Draft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Approved,
    ChangesRequested,
    Commented,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformAccount {
    pub id: String,
    pub platform: ReviewPlatformKind,
    pub label: String,
    pub username: Option<String>,
    pub host: String,
    pub auth_state: ReviewAuthState,
    pub auth_source: ReviewAuthSource,
    pub scopes: Vec<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformRepositoryRef {
    pub provider_id: String,
    pub platform: ReviewPlatformKind,
    pub host: String,
    pub owner: String,
    pub name: String,
    pub project_path: String,
    pub default_branch: String,
    pub workspace_path: Option<String>,
    pub web_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformRemote {
    pub id: String,
    pub name: String,
    pub url: String,
    pub platform: ReviewPlatformKind,
    pub host: String,
    pub owner: String,
    pub repository_name: String,
    pub project_path: String,
    pub web_url: String,
    pub supported: bool,
    pub auth_state: ReviewAuthState,
    pub auth_source: ReviewAuthSource,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewChecks {
    pub total: i32,
    pub passed: i32,
    pub failed: i32,
    pub pending: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformCiItem {
    pub id: String,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub detail: Option<String>,
    pub stage: Option<String>,
    pub web_url: Option<String>,
    pub log: Option<String>,
    pub log_truncated: bool,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformPullRequest {
    pub id: String,
    pub number: i64,
    pub title: String,
    pub state: ReviewItemState,
    pub author: String,
    pub source_branch: String,
    pub target_branch: String,
    pub base_revision: Option<String>,
    pub head_revision: Option<String>,
    pub updated_at: String,
    pub web_url: String,
    pub additions: i32,
    pub deletions: i32,
    pub changed_files: i32,
    pub comments: i32,
    pub review_decision: ReviewDecision,
    pub checks: ReviewChecks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: ReviewFileStatus,
    pub additions: i32,
    pub deletions: i32,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformReviewTargetFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: ReviewFileStatus,
    pub additions: i32,
    pub deletions: i32,
    pub diff_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformPullRequestReviewTarget {
    pub pull_request: ReviewPlatformPullRequest,
    pub files: Vec<ReviewPlatformReviewTargetFile>,
    pub omitted_file_count: usize,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformPullRequestFileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub status: ReviewFileStatus,
    pub base_revision: String,
    pub head_revision: String,
    pub diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformCommit {
    pub hash: String,
    pub short_hash: String,
    pub title: String,
    pub author: String,
    pub committed_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewPlatformThreadKind {
    Review,
    Comment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformThread {
    pub id: String,
    pub provider_thread_id: Option<String>,
    pub provider_comment_id: Option<String>,
    pub kind: ReviewPlatformThreadKind,
    pub reply_to_provider_comment_id: Option<String>,
    pub file_path: Option<String>,
    pub line: Option<i64>,
    pub resolved: bool,
    pub author: String,
    pub body: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformPullRequestDetail {
    #[serde(flatten)]
    pub pull_request: ReviewPlatformPullRequest,
    pub body: String,
    pub ci: Vec<ReviewPlatformCiItem>,
    pub files: Vec<ReviewPlatformFile>,
    pub commits: Vec<ReviewPlatformCommit>,
    pub threads: Vec<ReviewPlatformThread>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewPlatformDetailSection {
    Overview,
    Ci,
    Files,
    Commits,
    Reviews,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformPullRequestDetailPage {
    #[serde(flatten)]
    pub pull_request: ReviewPlatformPullRequest,
    pub body: String,
    pub ci: Vec<ReviewPlatformCiItem>,
    pub files: Vec<ReviewPlatformFile>,
    pub commits: Vec<ReviewPlatformCommit>,
    pub threads: Vec<ReviewPlatformThread>,
    pub section: ReviewPlatformDetailSection,
    pub pagination: ReviewPlatformPagination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformCiLog {
    pub ci_item_id: String,
    pub log: Option<String>,
    pub truncated: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformCapabilities {
    pub can_create_review: bool,
    pub can_create_pull_request: bool,
    pub can_reply_to_thread: bool,
    pub can_resolve_thread: bool,
    pub can_approve: bool,
    pub can_revoke_approval: bool,
    pub can_request_changes: bool,
    pub can_merge: bool,
    pub supports_draft_review: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSubmitEvent {
    Comment,
    Approve,
    RequestChanges,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformCreatePullRequestRequest {
    pub repository_path: String,
    pub remote_id: Option<String>,
    pub title: String,
    pub source_branch: String,
    pub target_branch: String,
    pub body: Option<String>,
    pub draft: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformReplyToThreadRequest {
    pub repository_path: String,
    pub remote_id: String,
    pub pull_request_id: String,
    pub thread_id: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformSubmitReviewRequest {
    pub repository_path: String,
    pub remote_id: String,
    pub pull_request_id: String,
    pub event: ReviewSubmitEvent,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformResolveThreadRequest {
    pub repository_path: String,
    pub remote_id: String,
    pub pull_request_id: String,
    pub thread_id: String,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformApprovalRequest {
    pub repository_path: String,
    pub remote_id: String,
    pub pull_request_id: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformRequestChangesRequest {
    pub repository_path: String,
    pub remote_id: String,
    pub pull_request_id: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformActionResult {
    pub success: bool,
    pub message: String,
    pub web_url: Option<String>,
    pub pull_request: Option<ReviewPlatformPullRequest>,
    pub thread: Option<ReviewPlatformThread>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewPlatformAuthChallengeState {
    Missing,
    Invalid,
    InsufficientScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformAuthChallenge {
    pub platform: ReviewPlatformKind,
    pub host: String,
    pub remote_id: String,
    pub project_path: String,
    pub state: ReviewPlatformAuthChallengeState,
    pub message: String,
    pub required_scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformWorkspaceSnapshot {
    pub remotes: Vec<ReviewPlatformRemote>,
    pub selected_remote_id: Option<String>,
    pub accounts: Vec<ReviewPlatformAccount>,
    pub repository: Option<ReviewPlatformRepositoryRef>,
    pub pull_requests: Vec<ReviewPlatformPullRequest>,
    pub pagination: ReviewPlatformPagination,
    pub capabilities: ReviewPlatformCapabilities,
    pub message: Option<String>,
    pub auth_challenge: Option<ReviewPlatformAuthChallenge>,
}

/// Classifies workspace paths that must be handled by a product-level remote
/// runtime before review-platform code probes the local Git repository.
#[async_trait::async_trait]
pub trait ReviewPlatformWorkspaceClassifier: Send + Sync {
    async fn is_remote_workspace_path(&self, path: &str) -> bool;
}

#[derive(Clone)]
pub struct ReviewPlatformService {
    token_store_path: PathBuf,
    workspace_classifier: Arc<dyn ReviewPlatformWorkspaceClassifier>,
}

struct LocalOnlyReviewPlatformWorkspaceClassifier;

#[async_trait::async_trait]
impl ReviewPlatformWorkspaceClassifier for LocalOnlyReviewPlatformWorkspaceClassifier {
    async fn is_remote_workspace_path(&self, _path: &str) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy)]
struct PullRequestPagination {
    page: u32,
    per_page: u32,
}

impl PullRequestPagination {
    fn new(page: Option<u32>, per_page: Option<u32>) -> Self {
        Self {
            page: page.unwrap_or(DEFAULT_PR_PAGE).max(1),
            per_page: per_page
                .unwrap_or(DEFAULT_PR_PAGE_SIZE)
                .clamp(1, MAX_PR_PAGE_SIZE),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlatformPagination {
    pub page: u32,
    pub per_page: u32,
    pub total: Option<u64>,
    pub has_next: bool,
}

#[derive(Debug, Clone)]
struct ReviewPlatformPullRequestPage {
    items: Vec<ReviewPlatformPullRequest>,
    pagination: ReviewPlatformPagination,
}

#[derive(Debug, Clone)]
struct ProviderContext {
    remote: ReviewPlatformRemote,
    api_base_url: String,
    token: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ReviewPlatformAuthTokens {
    tokens: HashMap<String, String>,
}

impl ReviewPlatformAuthTokens {
    fn get(&self, platform: ReviewPlatformKind, host: &str) -> Option<&str> {
        token_key(platform, host).and_then(|key| self.tokens.get(&key).map(String::as_str))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredReviewPlatformTokens {
    #[serde(default)]
    tokens: HashMap<String, StoredReviewPlatformToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredReviewPlatformToken {
    token: String,
    updated_at: String,
}

impl ReviewPlatformService {
    /// Creates a review-platform owner with an explicit workspace classifier.
    /// Product assembly should inject remote-aware classification here.
    pub fn new(
        token_store_path: PathBuf,
        workspace_classifier: Arc<dyn ReviewPlatformWorkspaceClassifier>,
    ) -> Self {
        Self {
            token_store_path,
            workspace_classifier,
        }
    }

    /// Creates a local-only owner for services tests and local Git-only hosts.
    pub fn new_local_only(token_store_path: PathBuf) -> Self {
        Self::new(
            token_store_path,
            Arc::new(LocalOnlyReviewPlatformWorkspaceClassifier),
        )
    }

    pub fn token_store_path(&self) -> &Path {
        &self.token_store_path
    }

    async fn is_remote_workspace_path(&self, repository_path: &str) -> bool {
        self.workspace_classifier
            .is_remote_workspace_path(repository_path)
            .await
    }

    pub async fn discover_remotes(
        &self,
        repository_path: &str,
    ) -> Result<Vec<ReviewPlatformRemote>, ReviewPlatformError> {
        let auth_tokens = self.load_stored_tokens().await?;
        Self::discover_remotes_with_tokens(repository_path, &auth_tokens).await
    }

    async fn discover_remotes_with_tokens(
        repository_path: &str,
        auth_tokens: &ReviewPlatformAuthTokens,
    ) -> Result<Vec<ReviewPlatformRemote>, ReviewPlatformError> {
        let root = get_repository_root(repository_path).await?;
        let output = execute_git_command(&root, &["remote", "-v"]).await?;

        let mut seen = HashSet::new();
        let mut remotes = Vec::new();

        for line in output.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            if parts.get(2).is_some_and(|kind| *kind != "(fetch)") {
                continue;
            }
            let remote_name = parts[0];
            let remote_url = parts[1];
            let key = format!("{}|{}", remote_name, remote_url);
            if !seen.insert(key) {
                continue;
            }
            if let Some(remote) = parse_remote(remote_name, remote_url, auth_tokens) {
                remotes.push(remote);
            }
        }

        Ok(remotes)
    }

    pub async fn workspace_snapshot(
        &self,
        repository_path: &str,
        remote_id: Option<&str>,
        page: Option<u32>,
        per_page: Option<u32>,
    ) -> Result<ReviewPlatformWorkspaceSnapshot, ReviewPlatformError> {
        if self.is_remote_workspace_path(repository_path).await {
            return Ok(empty_snapshot(
                Vec::new(),
                None,
                None,
                "Pull request browsing is not available for remote SSH workspaces yet.",
            ));
        }

        let pagination_request = PullRequestPagination::new(page, per_page);
        let auth_tokens = self.load_stored_tokens().await?;
        let root = get_repository_root(repository_path).await?;
        let remotes = Self::discover_remotes_with_tokens(&root, &auth_tokens).await?;
        let selected_remote = select_remote(&remotes, remote_id).cloned();

        let Some(remote) = selected_remote else {
            return Ok(empty_snapshot(
                remotes,
                None,
                None,
                "No Git remotes were found",
            ));
        };

        if !remote.supported {
            return Ok(empty_snapshot(
                remotes,
                Some(remote.id.clone()),
                Some(account_for_remote(&remote)),
                remote
                    .message
                    .as_deref()
                    .unwrap_or("Unsupported remote provider"),
            ));
        }

        if remote.platform == ReviewPlatformKind::Gitcode
            && token_for_remote(&remote, &auth_tokens).is_none()
        {
            return Ok(empty_snapshot(
                remotes,
                Some(remote.id.clone()),
                Some(account_for_remote(&remote)),
                "GitCode pull request APIs require a Personal Access Token. Add a token for this remote and refresh.",
            ));
        }

        let ctx = provider_context(remote.clone(), &auth_tokens)?;
        let provider = provider_for(ctx.remote.platform);
        let repository = Some(repository_ref(&ctx.remote, Some(root)));
        let account = account_for_remote(&ctx.remote);
        let capabilities = capabilities_for_remote(&remote);
        match provider.list_pull_requests(&ctx, pagination_request).await {
            Ok(page) => Ok(ReviewPlatformWorkspaceSnapshot {
                remotes,
                selected_remote_id: Some(remote.id.clone()),
                accounts: vec![account],
                repository,
                pull_requests: page.items,
                pagination: page.pagination,
                capabilities,
                message: None,
                auth_challenge: None,
            }),
            Err(error) if is_auth_http_error(&error) => {
                let challenge = auth_challenge_for_remote(
                    &remote,
                    &error,
                    token_for_remote(&remote, &auth_tokens).is_some(),
                );
                let mut account = account;
                account.auth_state = auth_state_for_challenge(challenge.state);
                account.auth_source =
                    if matches!(challenge.state, ReviewPlatformAuthChallengeState::Missing) {
                        ReviewAuthSource::None
                    } else {
                        account.auth_source
                    };
                account.message = Some(challenge.message.clone());
                Ok(auth_required_snapshot(
                    remotes,
                    remote,
                    repository,
                    account,
                    capabilities,
                    challenge,
                ))
            }
            Err(error) => Err(error),
        }
    }

    pub async fn pull_request_detail(
        &self,
        repository_path: &str,
        remote_id: &str,
        pull_request_id: &str,
    ) -> Result<ReviewPlatformPullRequestDetail, ReviewPlatformError> {
        if self.is_remote_workspace_path(repository_path).await {
            return Err(ReviewPlatformError::UnsupportedPlatform(
                "remote SSH workspace".to_string(),
            ));
        }

        let auth_tokens = self.load_stored_tokens().await?;
        let root = get_repository_root(repository_path).await?;
        let remotes = Self::discover_remotes_with_tokens(&root, &auth_tokens).await?;
        let remote = remotes
            .into_iter()
            .find(|remote| remote.id == remote_id)
            .ok_or_else(|| ReviewPlatformError::RemoteNotFound(remote_id.to_string()))?;
        if !remote.supported {
            return Err(ReviewPlatformError::UnsupportedPlatform(remote.host));
        }
        let ctx = provider_context(remote, &auth_tokens)?;
        provider_for(ctx.remote.platform)
            .pull_request_detail(&ctx, pull_request_id)
            .await
    }

    pub async fn pull_request_review_target(
        &self,
        repository_path: &str,
        remote_id: &str,
        pull_request_id: &str,
    ) -> Result<ReviewPlatformPullRequestReviewTarget, ReviewPlatformError> {
        let ctx = self
            .provider_context_for_repository(repository_path, Some(remote_id))
            .await?;
        provider_for(ctx.remote.platform)
            .pull_request_review_target(&ctx, pull_request_id)
            .await
    }

    pub async fn pull_request_file_diff(
        &self,
        repository_path: &str,
        remote_id: &str,
        pull_request_id: &str,
        expected_base_revision: &str,
        expected_head_revision: &str,
        file_path: &str,
        file_page_hint: Option<u32>,
    ) -> Result<ReviewPlatformPullRequestFileDiff, ReviewPlatformError> {
        let ctx = self
            .provider_context_for_repository(repository_path, Some(remote_id))
            .await?;
        provider_for(ctx.remote.platform)
            .pull_request_file_diff(
                &ctx,
                pull_request_id,
                expected_base_revision,
                expected_head_revision,
                file_path,
                file_page_hint,
            )
            .await
    }

    pub async fn pull_request_detail_page(
        &self,
        repository_path: &str,
        remote_id: &str,
        pull_request_id: &str,
        section: ReviewPlatformDetailSection,
        page: Option<u32>,
        per_page: Option<u32>,
    ) -> Result<ReviewPlatformPullRequestDetailPage, ReviewPlatformError> {
        let ctx = self
            .provider_context_for_repository(repository_path, Some(remote_id))
            .await?;
        provider_for(ctx.remote.platform)
            .pull_request_detail_page(
                &ctx,
                pull_request_id,
                section,
                PullRequestPagination::new(page, per_page),
            )
            .await
    }

    pub async fn pull_request_ci_log(
        &self,
        repository_path: &str,
        remote_id: &str,
        pull_request_id: &str,
        ci_item_id: &str,
        ci_item_name: &str,
    ) -> Result<ReviewPlatformCiLog, ReviewPlatformError> {
        let ctx = self
            .provider_context_for_repository(repository_path, Some(remote_id))
            .await?;
        provider_for(ctx.remote.platform)
            .pull_request_ci_log(&ctx, pull_request_id, ci_item_id, ci_item_name)
            .await
    }

    pub async fn create_pull_request(
        &self,
        request: ReviewPlatformCreatePullRequestRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        let ctx = self
            .provider_context_for_repository(&request.repository_path, request.remote_id.as_deref())
            .await?;
        provider_for(ctx.remote.platform)
            .create_pull_request(&ctx, &request)
            .await
    }

    pub async fn reply_to_thread(
        &self,
        request: ReviewPlatformReplyToThreadRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        let ctx = self
            .provider_context_for_repository(
                &request.repository_path,
                Some(request.remote_id.as_str()),
            )
            .await?;
        provider_for(ctx.remote.platform)
            .reply_to_thread(&ctx, &request)
            .await
    }

    pub async fn submit_review(
        &self,
        request: ReviewPlatformSubmitReviewRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        let ctx = self
            .provider_context_for_repository(
                &request.repository_path,
                Some(request.remote_id.as_str()),
            )
            .await?;
        provider_for(ctx.remote.platform)
            .submit_review(&ctx, &request)
            .await
    }

    pub async fn resolve_thread(
        &self,
        request: ReviewPlatformResolveThreadRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        let ctx = self
            .provider_context_for_repository(
                &request.repository_path,
                Some(request.remote_id.as_str()),
            )
            .await?;
        provider_for(ctx.remote.platform)
            .resolve_thread(&ctx, &request)
            .await
    }

    pub async fn approve_pull_request(
        &self,
        request: ReviewPlatformApprovalRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        let ctx = self
            .provider_context_for_repository(
                &request.repository_path,
                Some(request.remote_id.as_str()),
            )
            .await?;
        provider_for(ctx.remote.platform)
            .approve_pull_request(&ctx, &request)
            .await
    }

    pub async fn revoke_approval(
        &self,
        request: ReviewPlatformApprovalRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        let ctx = self
            .provider_context_for_repository(
                &request.repository_path,
                Some(request.remote_id.as_str()),
            )
            .await?;
        provider_for(ctx.remote.platform)
            .revoke_approval(&ctx, &request)
            .await
    }

    pub async fn request_changes(
        &self,
        request: ReviewPlatformRequestChangesRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        let ctx = self
            .provider_context_for_repository(
                &request.repository_path,
                Some(request.remote_id.as_str()),
            )
            .await?;
        provider_for(ctx.remote.platform)
            .request_changes(&ctx, &request)
            .await
    }

    async fn provider_context_for_repository(
        &self,
        repository_path: &str,
        remote_id: Option<&str>,
    ) -> Result<ProviderContext, ReviewPlatformError> {
        if self.is_remote_workspace_path(repository_path).await {
            return Err(ReviewPlatformError::UnsupportedPlatform(
                "remote SSH workspace".to_string(),
            ));
        }

        let auth_tokens = self.load_stored_tokens().await?;
        let root = get_repository_root(repository_path).await?;
        let remotes = Self::discover_remotes_with_tokens(&root, &auth_tokens).await?;
        let remote = select_remote_for_action(&remotes, remote_id)?.clone();
        if !remote.supported {
            return Err(ReviewPlatformError::UnsupportedPlatform(remote.host));
        }
        provider_context(remote, &auth_tokens)
    }

    pub async fn update_auth_token(
        &self,
        platform: ReviewPlatformKind,
        host: &str,
        token: &str,
    ) -> Result<(), ReviewPlatformError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(ReviewPlatformError::Api(
                "Token cannot be empty".to_string(),
            ));
        }
        let key = token_key(platform, host)
            .ok_or_else(|| ReviewPlatformError::UnsupportedPlatform(host.to_string()))?;
        let mut stored = self.load_stored_token_file().await?;
        stored.tokens.insert(
            key,
            StoredReviewPlatformToken {
                token: token.to_string(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
        );
        self.save_stored_token_file(&stored).await
    }

    pub async fn clear_auth_token(
        &self,
        platform: ReviewPlatformKind,
        host: &str,
    ) -> Result<(), ReviewPlatformError> {
        let key = token_key(platform, host)
            .ok_or_else(|| ReviewPlatformError::UnsupportedPlatform(host.to_string()))?;
        let mut stored = self.load_stored_token_file().await?;
        stored.tokens.remove(&key);
        self.save_stored_token_file(&stored).await
    }
}

#[async_trait::async_trait]
trait ReviewProvider: Sync {
    async fn list_pull_requests(
        &self,
        ctx: &ProviderContext,
        pagination: PullRequestPagination,
    ) -> Result<ReviewPlatformPullRequestPage, ReviewPlatformError>;

    async fn pull_request_detail(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
    ) -> Result<ReviewPlatformPullRequestDetail, ReviewPlatformError>;

    async fn pull_request_review_target(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
    ) -> Result<ReviewPlatformPullRequestReviewTarget, ReviewPlatformError> {
        let detail = self.pull_request_detail(ctx, pull_request_id).await?;
        Ok(review_target_from_parts(detail.pull_request, detail.files))
    }

    async fn pull_request_file_diff(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
        expected_base_revision: &str,
        expected_head_revision: &str,
        file_path: &str,
        _file_page_hint: Option<u32>,
    ) -> Result<ReviewPlatformPullRequestFileDiff, ReviewPlatformError> {
        let detail = self.pull_request_detail(ctx, pull_request_id).await?;
        file_diff_from_parts(
            detail.pull_request,
            detail.files,
            expected_base_revision,
            expected_head_revision,
            file_path,
        )
    }

    async fn pull_request_detail_page(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
        section: ReviewPlatformDetailSection,
        pagination: PullRequestPagination,
    ) -> Result<ReviewPlatformPullRequestDetailPage, ReviewPlatformError> {
        let detail = self.pull_request_detail(ctx, pull_request_id).await?;
        let ci_total = detail.ci.len();
        let file_total = detail.files.len();
        let commit_total = detail.commits.len();
        let thread_total = detail.threads.len();
        let (ci, files, commits, threads) = match section {
            ReviewPlatformDetailSection::Overview => {
                (Vec::new(), Vec::new(), Vec::new(), Vec::new())
            }
            ReviewPlatformDetailSection::Ci => (
                slice_page(detail.ci, pagination),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            ReviewPlatformDetailSection::Files => (
                Vec::new(),
                slice_page(detail.files, pagination),
                Vec::new(),
                Vec::new(),
            ),
            ReviewPlatformDetailSection::Commits => (
                Vec::new(),
                Vec::new(),
                slice_page(detail.commits, pagination),
                Vec::new(),
            ),
            ReviewPlatformDetailSection::Reviews => (
                Vec::new(),
                Vec::new(),
                Vec::new(),
                slice_page(detail.threads, pagination),
            ),
        };
        let total = match section {
            ReviewPlatformDetailSection::Overview => 0,
            ReviewPlatformDetailSection::Ci => ci_total,
            ReviewPlatformDetailSection::Files => file_total,
            ReviewPlatformDetailSection::Commits => commit_total,
            ReviewPlatformDetailSection::Reviews => thread_total,
        };
        Ok(ReviewPlatformPullRequestDetailPage {
            pull_request: detail.pull_request,
            body: detail.body,
            ci,
            files,
            commits,
            threads,
            section,
            pagination: pagination_from_total(pagination, total),
        })
    }

    async fn pull_request_ci_log(
        &self,
        ctx: &ProviderContext,
        _pull_request_id: &str,
        _ci_item_id: &str,
        _ci_item_name: &str,
    ) -> Result<ReviewPlatformCiLog, ReviewPlatformError> {
        Err(ReviewPlatformError::UnsupportedPlatform(format!(
            "{} CI logs",
            platform_label(ctx.remote.platform)
        )))
    }

    async fn create_pull_request(
        &self,
        ctx: &ProviderContext,
        _request: &ReviewPlatformCreatePullRequestRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        Err(ReviewPlatformError::UnsupportedPlatform(format!(
            "{} pull request creation",
            platform_label(ctx.remote.platform)
        )))
    }

    async fn reply_to_thread(
        &self,
        ctx: &ProviderContext,
        _request: &ReviewPlatformReplyToThreadRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        Err(ReviewPlatformError::UnsupportedPlatform(format!(
            "{} thread replies",
            platform_label(ctx.remote.platform)
        )))
    }

    async fn submit_review(
        &self,
        ctx: &ProviderContext,
        _request: &ReviewPlatformSubmitReviewRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        Err(ReviewPlatformError::UnsupportedPlatform(format!(
            "{} review submission",
            platform_label(ctx.remote.platform)
        )))
    }

    async fn resolve_thread(
        &self,
        ctx: &ProviderContext,
        _request: &ReviewPlatformResolveThreadRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        Err(ReviewPlatformError::UnsupportedPlatform(format!(
            "{} thread resolution",
            platform_label(ctx.remote.platform)
        )))
    }

    async fn approve_pull_request(
        &self,
        ctx: &ProviderContext,
        _request: &ReviewPlatformApprovalRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        Err(ReviewPlatformError::UnsupportedPlatform(format!(
            "{} pull request approval",
            platform_label(ctx.remote.platform)
        )))
    }

    async fn revoke_approval(
        &self,
        ctx: &ProviderContext,
        _request: &ReviewPlatformApprovalRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        Err(ReviewPlatformError::UnsupportedPlatform(format!(
            "{} approval revocation",
            platform_label(ctx.remote.platform)
        )))
    }

    async fn request_changes(
        &self,
        ctx: &ProviderContext,
        _request: &ReviewPlatformRequestChangesRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        Err(ReviewPlatformError::UnsupportedPlatform(format!(
            "{} native change requests",
            platform_label(ctx.remote.platform)
        )))
    }
}

struct GithubProvider;
struct GitlabProvider;
struct GitcodeProvider;
struct UnsupportedProvider;

fn provider_for(platform: ReviewPlatformKind) -> &'static dyn ReviewProvider {
    match platform {
        ReviewPlatformKind::Github => &GithubProvider,
        ReviewPlatformKind::Gitlab => &GitlabProvider,
        ReviewPlatformKind::Gitcode => &GitcodeProvider,
        ReviewPlatformKind::Unknown => &UnsupportedProvider,
    }
}

fn ensure_pull_request_revisions_stable(
    initial: &ReviewPlatformPullRequest,
    confirmed: &ReviewPlatformPullRequest,
) -> Result<(), ReviewPlatformError> {
    if initial.id != confirmed.id
        || initial.base_revision != confirmed.base_revision
        || initial.head_revision != confirmed.head_revision
    {
        return Err(ReviewPlatformError::StaleTarget(
            "pull request revisions changed while preparing Review evidence".to_string(),
        ));
    }
    Ok(())
}

async fn github_review_target_parts(
    ctx: &ProviderContext,
    pull_request_id: &str,
) -> Result<(ReviewPlatformPullRequest, Vec<ReviewPlatformFile>), ReviewPlatformError> {
    let client = http_client()?;
    let base = format!(
        "{}/repos/{}/{}/pulls/{}",
        ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request_id
    );
    let initial_detail =
        send_bounded_json(github_request(client.clone(), &base, ctx.token.as_deref())).await?;
    let token = ctx.token.clone();
    let files_url = format!("{}/files", base);
    let files = fetch_bounded_paginated_array(
        |page| {
            let page = page.to_string();
            github_request(client.clone(), &files_url, token.as_deref())
                .query(&[("per_page", "100"), ("page", &page)])
        },
        github_next_page,
        MAX_REVIEW_TARGET_FILES.saturating_add(1),
    )
    .await?;
    let confirmed_detail =
        send_bounded_json(github_request(client, &base, ctx.token.as_deref())).await?;
    let initial_pull_request = github_pull_request_from_value(&initial_detail);
    let confirmed_pull_request = github_pull_request_from_value(&confirmed_detail);
    ensure_pull_request_revisions_stable(&initial_pull_request, &confirmed_pull_request)?;
    Ok((
        confirmed_pull_request,
        array_items(&files)
            .iter()
            .map(github_file_from_value)
            .collect(),
    ))
}

async fn github_review_file_parts(
    ctx: &ProviderContext,
    pull_request_id: &str,
    file_path: &str,
    file_page_hint: Option<u32>,
) -> Result<(ReviewPlatformPullRequest, Vec<ReviewPlatformFile>), ReviewPlatformError> {
    let client = http_client()?;
    let base = format!(
        "{}/repos/{}/{}/pulls/{}",
        ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request_id
    );
    let token = ctx.token.clone();
    let files_url = format!("{}/files", base);
    let file = fetch_bounded_paginated_file(
        |page| {
            let page = page.to_string();
            github_request(client.clone(), &files_url, token.as_deref())
                .query(&[("per_page", "100"), ("page", &page)])
        },
        github_next_page,
        file_page_hint.unwrap_or(1),
        if file_page_hint.is_some() {
            100
        } else {
            MAX_REVIEW_TARGET_FILES
        },
        file_path,
        github_file_from_value,
    )
    .await?;
    let detail = send_bounded_json(github_request(client, &base, ctx.token.as_deref())).await?;
    Ok((
        github_pull_request_from_value(&detail),
        file.into_iter().collect(),
    ))
}

async fn gitlab_review_target_parts(
    ctx: &ProviderContext,
    pull_request_id: &str,
) -> Result<(ReviewPlatformPullRequest, Vec<ReviewPlatformFile>), ReviewPlatformError> {
    let client = http_client()?;
    let project = urlencoding::encode(&ctx.remote.project_path);
    let base = format!(
        "{}/projects/{}/merge_requests/{}",
        ctx.api_base_url, project, pull_request_id
    );
    let initial_detail =
        send_bounded_json(gitlab_request(client.clone(), &base, ctx.token.as_deref())).await?;
    let token = ctx.token.clone();
    let diffs_url = format!("{}/diffs", base);
    let diffs = fetch_bounded_paginated_array(
        |page| {
            let page = page.to_string();
            gitlab_request(client.clone(), &diffs_url, token.as_deref())
                .query(&[("per_page", "100"), ("page", &page)])
        },
        gitlab_next_page,
        MAX_REVIEW_TARGET_FILES.saturating_add(1),
    )
    .await?;
    let files = array_items(&diffs)
        .iter()
        .map(gitlab_file_from_value)
        .collect::<Vec<_>>();
    let confirmed_detail =
        send_bounded_json(gitlab_request(client, &base, ctx.token.as_deref())).await?;
    let initial_pull_request = gitlab_pull_request_from_value(&initial_detail);
    let confirmed_pull_request = gitlab_pull_request_from_value(&confirmed_detail);
    ensure_pull_request_revisions_stable(&initial_pull_request, &confirmed_pull_request)?;
    Ok((confirmed_pull_request, files))
}

async fn gitlab_review_file_parts(
    ctx: &ProviderContext,
    pull_request_id: &str,
    file_path: &str,
    file_page_hint: Option<u32>,
) -> Result<(ReviewPlatformPullRequest, Vec<ReviewPlatformFile>), ReviewPlatformError> {
    let client = http_client()?;
    let project = urlencoding::encode(&ctx.remote.project_path);
    let base = format!(
        "{}/projects/{}/merge_requests/{}",
        ctx.api_base_url, project, pull_request_id
    );
    let token = ctx.token.clone();
    let diffs_url = format!("{}/diffs", base);
    let file = fetch_bounded_paginated_file(
        |page| {
            let page = page.to_string();
            gitlab_request(client.clone(), &diffs_url, token.as_deref())
                .query(&[("per_page", "100"), ("page", &page)])
        },
        gitlab_next_page,
        file_page_hint.unwrap_or(1),
        if file_page_hint.is_some() {
            100
        } else {
            MAX_REVIEW_TARGET_FILES
        },
        file_path,
        gitlab_file_from_value,
    )
    .await?;
    let detail = send_bounded_json(gitlab_request(client, &base, ctx.token.as_deref())).await?;
    Ok((
        gitlab_pull_request_from_value(&detail),
        file.into_iter().collect(),
    ))
}

async fn gitcode_review_target_parts(
    ctx: &ProviderContext,
    pull_request_id: &str,
) -> Result<(ReviewPlatformPullRequest, Vec<ReviewPlatformFile>), ReviewPlatformError> {
    let client = http_client()?;
    let base = format!(
        "{}/repos/{}/{}/pulls/{}",
        ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request_id
    );
    let initial_detail =
        send_bounded_json(gitcode_request(client.clone(), &base, ctx.token.as_deref())).await?;
    let token = ctx.token.clone();
    let files_url = format!("{}/files", base);
    let files = fetch_bounded_paginated_array(
        |page| {
            let page = page.to_string();
            gitcode_request(client.clone(), &files_url, token.as_deref())
                .query(&[("per_page", "100"), ("page", &page)])
        },
        github_next_page,
        MAX_REVIEW_TARGET_FILES.saturating_add(1),
    )
    .await?;
    let confirmed_detail =
        send_bounded_json(gitcode_request(client, &base, ctx.token.as_deref())).await?;
    let initial_pull_request = gitcode_pull_request_from_value(&initial_detail);
    let confirmed_pull_request = gitcode_pull_request_from_value(&confirmed_detail);
    ensure_pull_request_revisions_stable(&initial_pull_request, &confirmed_pull_request)?;
    Ok((
        confirmed_pull_request,
        array_items(&files)
            .iter()
            .map(gitcode_file_from_value)
            .collect(),
    ))
}

async fn gitcode_review_file_parts(
    ctx: &ProviderContext,
    pull_request_id: &str,
    file_path: &str,
    file_page_hint: Option<u32>,
) -> Result<(ReviewPlatformPullRequest, Vec<ReviewPlatformFile>), ReviewPlatformError> {
    let client = http_client()?;
    let base = format!(
        "{}/repos/{}/{}/pulls/{}",
        ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request_id
    );
    let token = ctx.token.clone();
    let files_url = format!("{}/files", base);
    let file = fetch_bounded_paginated_file(
        |page| {
            let page = page.to_string();
            gitcode_request(client.clone(), &files_url, token.as_deref())
                .query(&[("per_page", "100"), ("page", &page)])
        },
        github_next_page,
        file_page_hint.unwrap_or(1),
        if file_page_hint.is_some() {
            100
        } else {
            MAX_REVIEW_TARGET_FILES
        },
        file_path,
        gitcode_file_from_value,
    )
    .await?;
    let detail = send_bounded_json(gitcode_request(client, &base, ctx.token.as_deref())).await?;
    Ok((
        gitcode_pull_request_from_value(&detail),
        file.into_iter().collect(),
    ))
}

#[async_trait::async_trait]
impl ReviewProvider for GithubProvider {
    async fn list_pull_requests(
        &self,
        ctx: &ProviderContext,
        pagination: PullRequestPagination,
    ) -> Result<ReviewPlatformPullRequestPage, ReviewPlatformError> {
        let url = format!(
            "{}/repos/{}/{}/pulls",
            ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name
        );
        let per_page = pagination.per_page.to_string();
        let page = pagination.page.to_string();
        let response = send_json_response(
            github_request(http_client()?, &url, ctx.token.as_deref()).query(&[
                ("state", "all"),
                ("per_page", &per_page),
                ("page", &page),
            ]),
        )
        .await?;
        let items = response.value.as_array().ok_or_else(|| {
            ReviewPlatformError::Parse("GitHub pull response was not an array".to_string())
        })?;
        let total = pagination_total_from_links(&response.headers, pagination, items.len());
        let has_next = link_header_has_rel(&response.headers, "next");

        let pull_requests = items
            .iter()
            .map(github_pull_request_from_value)
            .collect::<Vec<_>>();
        let pull_requests = enrich_github_pull_request_counts(ctx, pull_requests).await;

        Ok(ReviewPlatformPullRequestPage {
            items: pull_requests,
            pagination: ReviewPlatformPagination {
                page: pagination.page,
                per_page: pagination.per_page,
                total,
                has_next,
            },
        })
    }

    async fn pull_request_detail(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
    ) -> Result<ReviewPlatformPullRequestDetail, ReviewPlatformError> {
        let base = format!(
            "{}/repos/{}/{}/pulls/{}",
            ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request_id
        );
        let client = http_client()?;
        let detail = send_json(github_request(client.clone(), &base, ctx.token.as_deref())).await?;
        let token = ctx.token.clone();
        let files_url = format!("{}/files", base);
        let files = fetch_paginated_array(
            |page| {
                let page = page.to_string();
                github_request(client.clone(), &files_url, token.as_deref())
                    .query(&[("per_page", "100"), ("page", &page)])
            },
            github_next_page,
        )
        .await?;
        let token = ctx.token.clone();
        let commits_url = format!("{}/commits", base);
        let commits = fetch_paginated_array(
            |page| {
                let page = page.to_string();
                github_request(client.clone(), &commits_url, token.as_deref())
                    .query(&[("per_page", "100"), ("page", &page)])
            },
            github_next_page,
        )
        .await?;
        let token = ctx.token.clone();
        let reviews_url = format!("{}/reviews", base);
        let reviews = fetch_paginated_array(
            |page| {
                let page = page.to_string();
                github_request(client.clone(), &reviews_url, token.as_deref())
                    .query(&[("per_page", "100"), ("page", &page)])
            },
            github_next_page,
        )
        .await?;
        let token = ctx.token.clone();
        let review_comments_url = format!("{}/comments", base);
        let review_comments = fetch_paginated_array(
            |page| {
                let page = page.to_string();
                github_request(client.clone(), &review_comments_url, token.as_deref())
                    .query(&[("per_page", "100"), ("page", &page)])
            },
            github_next_page,
        )
        .await?;
        let token = ctx.token.clone();
        let issue_comments_url = format!(
            "{}/repos/{}/{}/issues/{}/comments",
            ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request_id
        );
        let issue_comments = fetch_paginated_array(
            |page| {
                let page = page.to_string();
                github_request(client.clone(), &issue_comments_url, token.as_deref())
                    .query(&[("per_page", "100"), ("page", &page)])
            },
            github_next_page,
        )
        .await?;

        let mut pull_request = github_pull_request_from_value(&detail);
        pull_request.review_decision = github_review_decision(&reviews);
        let (checks, ci) = github_checks_and_ci(ctx, &client, &detail).await;
        pull_request.checks = checks;

        Ok(ReviewPlatformPullRequestDetail {
            body: value_string(&detail, "body"),
            pull_request,
            ci,
            files: array_items(&files)
                .iter()
                .map(github_file_from_value)
                .collect(),
            commits: array_items(&commits)
                .iter()
                .map(github_commit_from_value)
                .collect(),
            threads: github_threads(&reviews, &review_comments, &issue_comments),
        })
    }

    async fn pull_request_review_target(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
    ) -> Result<ReviewPlatformPullRequestReviewTarget, ReviewPlatformError> {
        let (pull_request, files) = github_review_target_parts(ctx, pull_request_id).await?;
        Ok(review_target_from_parts(pull_request, files))
    }

    async fn pull_request_file_diff(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
        expected_base_revision: &str,
        expected_head_revision: &str,
        file_path: &str,
        file_page_hint: Option<u32>,
    ) -> Result<ReviewPlatformPullRequestFileDiff, ReviewPlatformError> {
        let (pull_request, files) =
            github_review_file_parts(ctx, pull_request_id, file_path, file_page_hint).await?;
        file_diff_from_parts(
            pull_request,
            files,
            expected_base_revision,
            expected_head_revision,
            file_path,
        )
    }

    async fn pull_request_detail_page(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
        section: ReviewPlatformDetailSection,
        pagination: PullRequestPagination,
    ) -> Result<ReviewPlatformPullRequestDetailPage, ReviewPlatformError> {
        github_pull_request_detail_page(ctx, pull_request_id, section, pagination).await
    }

    async fn pull_request_ci_log(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
        ci_item_id: &str,
        ci_item_name: &str,
    ) -> Result<ReviewPlatformCiLog, ReviewPlatformError> {
        if ci_item_id.starts_with("status-") {
            return Ok(ReviewPlatformCiLog {
                ci_item_id: ci_item_id.to_string(),
                log: None,
                truncated: false,
                message: Some(
                    "GitHub commit statuses do not expose logs; use the linked target URL instead."
                        .to_string(),
                ),
            });
        }

        let client = http_client()?;
        let base = format!(
            "{}/repos/{}/{}/pulls/{}",
            ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request_id
        );
        let detail = send_json(github_request(client.clone(), &base, ctx.token.as_deref())).await?;
        let sha = nested_string(&detail, &["head", "sha"]);
        if sha.trim().is_empty() {
            return Ok(ReviewPlatformCiLog {
                ci_item_id: ci_item_id.to_string(),
                log: None,
                truncated: false,
                message: Some("GitHub pull request head SHA was not available.".to_string()),
            });
        }

        let check_run_id = ci_item_id.strip_prefix("check-run-").unwrap_or(ci_item_id);
        github_actions_log_for_check_run_item(ctx, &client, check_run_id, ci_item_name, &sha).await
    }

    async fn create_pull_request(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformCreatePullRequestRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        let token = require_write_token(ctx, "Creating a pull request")?;
        let url = format!(
            "{}/repos/{}/{}/pulls",
            ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name
        );
        let payload = json!({
            "title": request.title,
            "head": request.source_branch,
            "base": request.target_branch,
            "body": request.body.clone().unwrap_or_default(),
            "draft": request.draft.unwrap_or(false),
        });
        let value =
            send_json(github_post_request(http_client()?, &url, Some(token)).json(&payload))
                .await?;
        let pull_request = github_pull_request_from_value(&value);
        let web_url = Some(pull_request.web_url.clone());
        Ok(ReviewPlatformActionResult {
            success: true,
            message: format!("Created pull request #{}", pull_request.number),
            web_url,
            pull_request: Some(pull_request),
            thread: None,
        })
    }

    async fn reply_to_thread(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformReplyToThreadRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        let token = require_write_token(ctx, "Replying to a pull request thread")?;
        let comment_id = parse_provider_comment_id(&request.thread_id).ok_or_else(|| {
            ReviewPlatformError::Api(
                "GitHub replies require a review comment thread id such as comment-123".to_string(),
            )
        })?;
        let url = format!(
            "{}/repos/{}/{}/pulls/{}/comments/{}/replies",
            ctx.api_base_url,
            ctx.remote.owner,
            ctx.remote.repository_name,
            request.pull_request_id,
            comment_id
        );
        let value = send_json(
            github_post_request(http_client()?, &url, Some(token))
                .json(&json!({ "body": request.body })),
        )
        .await?;
        let thread = github_thread_from_review_comment(&value);
        Ok(ReviewPlatformActionResult {
            success: true,
            message: "Replied to pull request thread".to_string(),
            web_url: value
                .get("html_url")
                .and_then(Value::as_str)
                .map(str::to_string),
            pull_request: None,
            thread: Some(thread),
        })
    }

    async fn submit_review(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformSubmitReviewRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        let event = match request.event {
            ReviewSubmitEvent::Comment => "COMMENT",
            ReviewSubmitEvent::Approve => "APPROVE",
            ReviewSubmitEvent::RequestChanges => "REQUEST_CHANGES",
        };
        github_submit_review(ctx, &request.pull_request_id, event, &request.body).await
    }

    async fn approve_pull_request(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformApprovalRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        github_submit_review(
            ctx,
            &request.pull_request_id,
            "APPROVE",
            request.body.as_deref().unwrap_or(""),
        )
        .await
    }

    async fn request_changes(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformRequestChangesRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        github_submit_review(
            ctx,
            &request.pull_request_id,
            "REQUEST_CHANGES",
            &request.body,
        )
        .await
    }
}

async fn github_submit_review(
    ctx: &ProviderContext,
    pull_request_id: &str,
    event: &str,
    body: &str,
) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
    let token = require_write_token(ctx, "Submitting a pull request review")?;
    let url = format!(
        "{}/repos/{}/{}/pulls/{}/reviews",
        ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request_id
    );
    let value = send_json(
        github_post_request(http_client()?, &url, Some(token)).json(&json!({
            "body": body,
            "event": event,
        })),
    )
    .await?;
    Ok(ReviewPlatformActionResult {
        success: true,
        message: format!("Submitted GitHub review with event {}", event),
        web_url: value
            .get("html_url")
            .and_then(Value::as_str)
            .map(str::to_string),
        pull_request: None,
        thread: None,
    })
}

async fn github_pull_request_detail_page(
    ctx: &ProviderContext,
    pull_request_id: &str,
    section: ReviewPlatformDetailSection,
    pagination: PullRequestPagination,
) -> Result<ReviewPlatformPullRequestDetailPage, ReviewPlatformError> {
    let client = http_client()?;
    let base = format!(
        "{}/repos/{}/{}/pulls/{}",
        ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request_id
    );
    let detail = send_json(github_request(client.clone(), &base, ctx.token.as_deref())).await?;
    let mut pull_request = github_pull_request_from_value(&detail);
    let (checks, ci_all) = github_checks_and_ci(ctx, &client, &detail).await;
    pull_request.checks = checks;

    let mut files = Vec::new();
    let mut commits = Vec::new();
    let mut threads = Vec::new();
    let mut ci = Vec::new();
    let mut section_pagination = empty_detail_pagination(section, pagination);

    match section {
        ReviewPlatformDetailSection::Overview => {}
        ReviewPlatformDetailSection::Ci => {
            section_pagination = pagination_from_total(pagination, ci_all.len());
            ci = slice_page(ci_all, pagination);
        }
        ReviewPlatformDetailSection::Files => {
            let response = fetch_array_page(
                github_request(
                    client.clone(),
                    &format!("{}/files", base),
                    ctx.token.as_deref(),
                ),
                pagination,
            )
            .await?;
            section_pagination = pagination_from_response(&response, pagination);
            files = array_items(&response.value)
                .iter()
                .map(github_file_from_value)
                .collect();
        }
        ReviewPlatformDetailSection::Commits => {
            let response = fetch_array_page(
                github_request(
                    client.clone(),
                    &format!("{}/commits", base),
                    ctx.token.as_deref(),
                ),
                pagination,
            )
            .await?;
            section_pagination = pagination_from_response(&response, pagination);
            commits = array_items(&response.value)
                .iter()
                .map(github_commit_from_value)
                .collect();
        }
        ReviewPlatformDetailSection::Reviews => {
            let reviews_url = format!("{}/reviews", base);
            let reviews = fetch_array_page(
                github_request(client.clone(), &reviews_url, ctx.token.as_deref()),
                pagination,
            )
            .await?;
            let review_comments = fetch_array_page(
                github_request(
                    client.clone(),
                    &format!("{}/comments", base),
                    ctx.token.as_deref(),
                ),
                pagination,
            )
            .await?;
            let issue_comments = fetch_array_page(
                github_request(
                    client.clone(),
                    &format!(
                        "{}/repos/{}/{}/issues/{}/comments",
                        ctx.api_base_url,
                        ctx.remote.owner,
                        ctx.remote.repository_name,
                        pull_request_id
                    ),
                    ctx.token.as_deref(),
                ),
                pagination,
            )
            .await?;
            pull_request.review_decision = github_review_decision(&reviews.value);
            section_pagination = combine_page_pagination(
                pagination,
                &[
                    pagination_from_response(&reviews, pagination),
                    pagination_from_response(&review_comments, pagination),
                    pagination_from_response(&issue_comments, pagination),
                ],
            );
            threads = github_threads(
                &reviews.value,
                &review_comments.value,
                &issue_comments.value,
            );
        }
    }

    Ok(ReviewPlatformPullRequestDetailPage {
        pull_request,
        body: value_string(&detail, "body"),
        ci,
        files,
        commits,
        threads,
        section,
        pagination: section_pagination,
    })
}

#[async_trait::async_trait]
impl ReviewProvider for GitlabProvider {
    async fn list_pull_requests(
        &self,
        ctx: &ProviderContext,
        pagination: PullRequestPagination,
    ) -> Result<ReviewPlatformPullRequestPage, ReviewPlatformError> {
        gitlab_list_pull_requests(ctx, pagination).await
    }

    async fn pull_request_detail(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
    ) -> Result<ReviewPlatformPullRequestDetail, ReviewPlatformError> {
        gitlab_pull_request_detail(ctx, pull_request_id).await
    }

    async fn pull_request_review_target(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
    ) -> Result<ReviewPlatformPullRequestReviewTarget, ReviewPlatformError> {
        let (pull_request, files) = gitlab_review_target_parts(ctx, pull_request_id).await?;
        Ok(review_target_from_parts(pull_request, files))
    }

    async fn pull_request_file_diff(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
        expected_base_revision: &str,
        expected_head_revision: &str,
        file_path: &str,
        file_page_hint: Option<u32>,
    ) -> Result<ReviewPlatformPullRequestFileDiff, ReviewPlatformError> {
        let (pull_request, files) =
            gitlab_review_file_parts(ctx, pull_request_id, file_path, file_page_hint).await?;
        file_diff_from_parts(
            pull_request,
            files,
            expected_base_revision,
            expected_head_revision,
            file_path,
        )
    }

    async fn pull_request_detail_page(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
        section: ReviewPlatformDetailSection,
        pagination: PullRequestPagination,
    ) -> Result<ReviewPlatformPullRequestDetailPage, ReviewPlatformError> {
        gitlab_pull_request_detail_page(ctx, pull_request_id, section, pagination).await
    }

    async fn pull_request_ci_log(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
        ci_item_id: &str,
        ci_item_name: &str,
    ) -> Result<ReviewPlatformCiLog, ReviewPlatformError> {
        gitlab_pull_request_ci_log(ctx, pull_request_id, ci_item_id, ci_item_name).await
    }

    async fn create_pull_request(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformCreatePullRequestRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        gitlab_create_pull_request(ctx, request, "merge request").await
    }

    async fn reply_to_thread(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformReplyToThreadRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        gitlab_reply_to_thread(ctx, request, "merge request").await
    }

    async fn submit_review(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformSubmitReviewRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        if request.event != ReviewSubmitEvent::Comment {
            return Err(ReviewPlatformError::UnsupportedPlatform(
                "GitLab submit_review supports comments only; use approve_pull_request for approvals"
                    .to_string(),
            ));
        }
        gitlab_add_merge_request_note(
            ctx,
            &request.pull_request_id,
            &request.body,
            "Added merge request comment",
        )
        .await
    }

    async fn resolve_thread(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformResolveThreadRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        gitlab_resolve_thread(ctx, request, "merge request").await
    }

    async fn approve_pull_request(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformApprovalRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        gitlab_approve_pull_request(ctx, request, "merge request").await
    }

    async fn revoke_approval(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformApprovalRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        gitlab_revoke_approval(ctx, request, "merge request").await
    }
}

async fn gitlab_list_pull_requests(
    ctx: &ProviderContext,
    pagination: PullRequestPagination,
) -> Result<ReviewPlatformPullRequestPage, ReviewPlatformError> {
    let project = urlencoding::encode(&ctx.remote.project_path);
    let url = format!("{}/projects/{}/merge_requests", ctx.api_base_url, project);
    let per_page = pagination.per_page.to_string();
    let page = pagination.page.to_string();
    let response = send_json_response(
        gitlab_request(http_client()?, &url, ctx.token.as_deref()).query(&[
            ("state", "all"),
            ("per_page", &per_page),
            ("page", &page),
        ]),
    )
    .await?;
    let items = response.value.as_array().ok_or_else(|| {
        ReviewPlatformError::Parse("GitLab merge request response was not an array".to_string())
    })?;
    let total = header_u64(&response.headers, "x-total");
    let has_next = header_string(&response.headers, "x-next-page")
        .is_some_and(|value| !value.trim().is_empty())
        || total
            .map(|total| u64::from(pagination.page) * u64::from(pagination.per_page) < total)
            .unwrap_or(false);

    let pull_requests = items
        .iter()
        .map(gitlab_pull_request_from_value)
        .collect::<Vec<_>>();
    let pull_requests = enrich_gitlab_pull_request_counts(ctx, pull_requests).await;

    Ok(ReviewPlatformPullRequestPage {
        items: pull_requests,
        pagination: ReviewPlatformPagination {
            page: pagination.page,
            per_page: pagination.per_page,
            total,
            has_next,
        },
    })
}

async fn gitlab_pull_request_detail(
    ctx: &ProviderContext,
    pull_request_id: &str,
) -> Result<ReviewPlatformPullRequestDetail, ReviewPlatformError> {
    let client = http_client()?;
    let project = urlencoding::encode(&ctx.remote.project_path);
    let base = format!(
        "{}/projects/{}/merge_requests/{}",
        ctx.api_base_url, project, pull_request_id
    );
    let detail = send_json(gitlab_request(client.clone(), &base, ctx.token.as_deref())).await?;
    let changes = send_json(gitlab_request(
        client.clone(),
        &format!("{}/changes", base),
        ctx.token.as_deref(),
    ))
    .await?;
    let token = ctx.token.clone();
    let commits_url = format!("{}/commits", base);
    let commits = fetch_paginated_array(
        |page| {
            let page = page.to_string();
            gitlab_request(client.clone(), &commits_url, token.as_deref())
                .query(&[("per_page", "100"), ("page", &page)])
        },
        gitlab_next_page,
    )
    .await?;
    let token = ctx.token.clone();
    let discussions_url = format!("{}/discussions", base);
    let discussions = fetch_paginated_array(
        |page| {
            let page = page.to_string();
            gitlab_request(client.clone(), &discussions_url, token.as_deref())
                .query(&[("per_page", "100"), ("page", &page)])
        },
        gitlab_next_page,
    )
    .await?;
    let token = ctx.token.clone();
    let notes_url = format!("{}/notes", base);
    let notes = fetch_paginated_array(
        |page| {
            let page = page.to_string();
            gitlab_request(client.clone(), &notes_url, token.as_deref())
                .query(&[("per_page", "100"), ("page", &page)])
        },
        gitlab_next_page,
    )
    .await?;

    let mut pull_request = gitlab_pull_request_from_value(&detail);
    let files = gitlab_files(&changes);
    apply_files_stats(&mut pull_request, &files);
    let ci = gitlab_pipeline_summary_item(&detail)
        .into_iter()
        .collect::<Vec<_>>();
    pull_request.checks = summarize_ci_items(&ci);

    Ok(ReviewPlatformPullRequestDetail {
        body: value_string(&detail, "description"),
        pull_request,
        ci,
        files,
        commits: array_items(&commits)
            .iter()
            .map(gitlab_commit_from_value)
            .collect(),
        threads: gitlab_threads(&discussions, &notes),
    })
}

async fn gitlab_pull_request_detail_page(
    ctx: &ProviderContext,
    pull_request_id: &str,
    section: ReviewPlatformDetailSection,
    pagination: PullRequestPagination,
) -> Result<ReviewPlatformPullRequestDetailPage, ReviewPlatformError> {
    let client = http_client()?;
    let project = urlencoding::encode(&ctx.remote.project_path);
    let base = format!(
        "{}/projects/{}/merge_requests/{}",
        ctx.api_base_url, project, pull_request_id
    );
    let detail = send_json(gitlab_request(client.clone(), &base, ctx.token.as_deref())).await?;
    let mut pull_request = gitlab_pull_request_from_value(&detail);
    let changes = send_json(gitlab_request(
        client.clone(),
        &format!("{}/changes", base),
        ctx.token.as_deref(),
    ))
    .await?;
    let all_files = gitlab_files(&changes);
    apply_files_stats(&mut pull_request, &all_files);
    let mut ci = gitlab_pipeline_summary_item(&detail)
        .into_iter()
        .collect::<Vec<_>>();
    pull_request.checks = summarize_ci_items(&ci);
    let mut files = Vec::new();
    let mut commits = Vec::new();
    let mut threads = Vec::new();
    let mut section_pagination = empty_detail_pagination(section, pagination);

    match section {
        ReviewPlatformDetailSection::Overview => {}
        ReviewPlatformDetailSection::Ci => {
            if let Some(pipeline_id) = detail
                .get("head_pipeline")
                .and_then(|value| value.get("id"))
                .and_then(Value::as_i64)
                .map(|id| id.to_string())
                .or_else(|| {
                    detail
                        .get("head_pipeline")
                        .and_then(|value| value.get("id"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
            {
                let jobs = gitlab_pipeline_jobs(
                    ctx,
                    client.clone(),
                    &urlencoding::encode(&ctx.remote.project_path),
                    &pipeline_id,
                )
                .await;
                if !jobs.is_empty() {
                    ci = jobs;
                    pull_request.checks = summarize_ci_items(&ci);
                }
            }
            section_pagination = pagination_from_total(pagination, ci.len());
            ci = slice_page(ci, pagination);
        }
        ReviewPlatformDetailSection::Files => {
            section_pagination = pagination_from_total(pagination, all_files.len());
            files = slice_page(all_files, pagination);
        }
        ReviewPlatformDetailSection::Commits => {
            let response = fetch_array_page(
                gitlab_request(
                    client.clone(),
                    &format!("{}/commits", base),
                    ctx.token.as_deref(),
                ),
                pagination,
            )
            .await?;
            section_pagination = pagination_from_response(&response, pagination);
            commits = array_items(&response.value)
                .iter()
                .map(gitlab_commit_from_value)
                .collect();
        }
        ReviewPlatformDetailSection::Reviews => {
            let discussions = fetch_array_page(
                gitlab_request(
                    client.clone(),
                    &format!("{}/discussions", base),
                    ctx.token.as_deref(),
                ),
                pagination,
            )
            .await?;
            let notes = fetch_array_page(
                gitlab_request(
                    client.clone(),
                    &format!("{}/notes", base),
                    ctx.token.as_deref(),
                ),
                pagination,
            )
            .await?;
            section_pagination = combine_page_pagination(
                pagination,
                &[
                    pagination_from_response(&discussions, pagination),
                    pagination_from_response(&notes, pagination),
                ],
            );
            threads = gitlab_threads(&discussions.value, &notes.value);
        }
    }

    Ok(ReviewPlatformPullRequestDetailPage {
        pull_request,
        body: value_string(&detail, "description"),
        ci,
        files,
        commits,
        threads,
        section,
        pagination: section_pagination,
    })
}

async fn gitlab_create_pull_request(
    ctx: &ProviderContext,
    request: &ReviewPlatformCreatePullRequestRequest,
    label: &str,
) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
    let token = require_write_token(ctx, &format!("Creating a {}", label))?;
    let project = urlencoding::encode(&ctx.remote.project_path);
    let url = format!("{}/projects/{}/merge_requests", ctx.api_base_url, project);
    let value = send_json(
        gitlab_post_request(http_client()?, &url, Some(token)).json(&json!({
            "title": request.title,
            "source_branch": request.source_branch,
            "target_branch": request.target_branch,
            "description": request.body.clone().unwrap_or_default(),
        })),
    )
    .await?;
    let pull_request = gitlab_pull_request_from_value(&value);
    let web_url = Some(pull_request.web_url.clone());
    Ok(ReviewPlatformActionResult {
        success: true,
        message: format!("Created {} !{}", label, pull_request.number),
        web_url,
        pull_request: Some(pull_request),
        thread: None,
    })
}

async fn gitlab_reply_to_thread(
    ctx: &ProviderContext,
    request: &ReviewPlatformReplyToThreadRequest,
    label: &str,
) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
    let token = require_write_token(ctx, &format!("Replying to a {} thread", label))?;
    let discussion_id = parse_provider_thread_id(&request.thread_id).ok_or_else(|| {
        ReviewPlatformError::Api(
            "Replies require a discussion thread id from pull request detail".to_string(),
        )
    })?;
    let project = urlencoding::encode(&ctx.remote.project_path);
    let url = format!(
        "{}/projects/{}/merge_requests/{}/discussions/{}/notes",
        ctx.api_base_url, project, request.pull_request_id, discussion_id
    );
    let value = send_json(
        gitlab_post_request(http_client()?, &url, Some(token))
            .json(&json!({ "body": request.body })),
    )
    .await?;
    let thread = gitlab_thread_from_note(
        &value,
        Some(discussion_id.to_string()),
        false,
        ReviewPlatformThreadKind::Comment,
        None,
    );
    Ok(ReviewPlatformActionResult {
        success: true,
        message: format!("Replied to {} discussion", label),
        web_url: None,
        pull_request: None,
        thread: Some(thread),
    })
}

async fn gitlab_add_merge_request_note(
    ctx: &ProviderContext,
    pull_request_id: &str,
    body: &str,
    message: &str,
) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
    let token = require_write_token(ctx, "Adding a merge request comment")?;
    let project = urlencoding::encode(&ctx.remote.project_path);
    let url = format!(
        "{}/projects/{}/merge_requests/{}/notes",
        ctx.api_base_url, project, pull_request_id
    );
    let value = send_json(
        gitlab_post_request(http_client()?, &url, Some(token)).json(&json!({ "body": body })),
    )
    .await?;
    let thread =
        gitlab_thread_from_note(&value, None, false, ReviewPlatformThreadKind::Comment, None);
    Ok(ReviewPlatformActionResult {
        success: true,
        message: message.to_string(),
        web_url: None,
        pull_request: None,
        thread: Some(thread),
    })
}

async fn gitlab_resolve_thread(
    ctx: &ProviderContext,
    request: &ReviewPlatformResolveThreadRequest,
    label: &str,
) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
    let token = require_write_token(ctx, &format!("Resolving a {} thread", label))?;
    let discussion_id = parse_provider_thread_id(&request.thread_id).ok_or_else(|| {
        ReviewPlatformError::Api(
            "Thread resolution requires a discussion thread id from pull request detail"
                .to_string(),
        )
    })?;
    let project = urlencoding::encode(&ctx.remote.project_path);
    let url = format!(
        "{}/projects/{}/merge_requests/{}/discussions/{}",
        ctx.api_base_url, project, request.pull_request_id, discussion_id
    );
    send_json(
        gitlab_put_request(http_client()?, &url, Some(token))
            .json(&json!({ "resolved": request.resolved })),
    )
    .await?;
    Ok(ReviewPlatformActionResult {
        success: true,
        message: if request.resolved {
            format!("Resolved {} discussion", label)
        } else {
            format!("Reopened {} discussion", label)
        },
        web_url: None,
        pull_request: None,
        thread: None,
    })
}

async fn gitlab_approve_pull_request(
    ctx: &ProviderContext,
    request: &ReviewPlatformApprovalRequest,
    label: &str,
) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
    let token = require_write_token(ctx, &format!("Approving a {}", label))?;
    let project = urlencoding::encode(&ctx.remote.project_path);
    let url = format!(
        "{}/projects/{}/merge_requests/{}/approve",
        ctx.api_base_url, project, request.pull_request_id
    );
    send_json(gitlab_post_request(http_client()?, &url, Some(token))).await?;
    if let Some(body) = request
        .body
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let _ = gitlab_add_merge_request_note(
            ctx,
            &request.pull_request_id,
            body,
            "Added approval note",
        )
        .await;
    }
    Ok(ReviewPlatformActionResult {
        success: true,
        message: format!("Approved {}", label),
        web_url: None,
        pull_request: None,
        thread: None,
    })
}

async fn gitlab_revoke_approval(
    ctx: &ProviderContext,
    request: &ReviewPlatformApprovalRequest,
    label: &str,
) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
    let token = require_write_token(ctx, &format!("Revoking approval for a {}", label))?;
    let project = urlencoding::encode(&ctx.remote.project_path);
    let url = format!(
        "{}/projects/{}/merge_requests/{}/unapprove",
        ctx.api_base_url, project, request.pull_request_id
    );
    send_json(gitlab_post_request(http_client()?, &url, Some(token))).await?;
    Ok(ReviewPlatformActionResult {
        success: true,
        message: format!("Revoked approval for {}", label),
        web_url: None,
        pull_request: None,
        thread: None,
    })
}

async fn gitcode_add_pull_request_comment(
    ctx: &ProviderContext,
    pull_request_id: &str,
    body: &str,
) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
    let token = require_write_token(ctx, "Adding a GitCode pull request comment")?;
    let url = format!(
        "{}/repos/{}/{}/pulls/{}/comments",
        ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request_id
    );
    let value = send_json(
        gitcode_post_request(http_client()?, &url, Some(token)).json(&json!({ "body": body })),
    )
    .await?;
    let thread = gitcode_threads(&Value::Array(vec![value]))
        .into_iter()
        .next();
    Ok(ReviewPlatformActionResult {
        success: true,
        message: "Added GitCode pull request comment".to_string(),
        web_url: None,
        pull_request: None,
        thread,
    })
}

async fn gitcode_pull_request_detail_page(
    ctx: &ProviderContext,
    pull_request_id: &str,
    section: ReviewPlatformDetailSection,
    pagination: PullRequestPagination,
) -> Result<ReviewPlatformPullRequestDetailPage, ReviewPlatformError> {
    let client = http_client()?;
    let base = format!(
        "{}/repos/{}/{}/pulls/{}",
        ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request_id
    );
    let detail = send_json(gitcode_request(client.clone(), &base, ctx.token.as_deref())).await?;
    let mut ci = gitcode_ci_items(&detail);
    let mut pull_request = gitcode_pull_request_from_value(&detail);
    pull_request.checks = summarize_ci_items(&ci);
    let mut files = Vec::new();
    let mut commits = Vec::new();
    let mut threads = Vec::new();
    let mut section_pagination = empty_detail_pagination(section, pagination);

    match section {
        ReviewPlatformDetailSection::Overview => {}
        ReviewPlatformDetailSection::Ci => {
            section_pagination = pagination_from_total(pagination, ci.len());
            ci = slice_page(ci, pagination);
        }
        ReviewPlatformDetailSection::Files => {
            if let Ok(response) = fetch_array_page(
                gitcode_request(
                    client.clone(),
                    &format!("{}/files", base),
                    ctx.token.as_deref(),
                ),
                pagination,
            )
            .await
            {
                section_pagination = pagination_from_response(&response, pagination);
                files = array_items(&response.value)
                    .iter()
                    .map(gitcode_file_from_value)
                    .collect();
            }
        }
        ReviewPlatformDetailSection::Commits => {
            if let Ok(response) = fetch_array_page(
                gitcode_request(
                    client.clone(),
                    &format!("{}/commits", base),
                    ctx.token.as_deref(),
                ),
                pagination,
            )
            .await
            {
                section_pagination = pagination_from_response(&response, pagination);
                commits = array_items(&response.value)
                    .iter()
                    .map(gitcode_commit_from_value)
                    .collect();
            }
        }
        ReviewPlatformDetailSection::Reviews => {
            if let Ok(response) = fetch_array_page(
                gitcode_request(
                    client.clone(),
                    &format!("{}/comments", base),
                    ctx.token.as_deref(),
                ),
                pagination,
            )
            .await
            {
                section_pagination = pagination_from_response(&response, pagination);
                threads = gitcode_threads(&response.value);
            }
        }
    }

    Ok(ReviewPlatformPullRequestDetailPage {
        body: first_non_empty(&[
            value_string(&detail, "body"),
            value_string(&detail, "description"),
        ]),
        pull_request,
        ci,
        files,
        commits,
        threads,
        section,
        pagination: section_pagination,
    })
}

#[async_trait::async_trait]
impl ReviewProvider for GitcodeProvider {
    async fn list_pull_requests(
        &self,
        ctx: &ProviderContext,
        pagination: PullRequestPagination,
    ) -> Result<ReviewPlatformPullRequestPage, ReviewPlatformError> {
        let url = format!(
            "{}/repos/{}/{}/pulls",
            ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name
        );
        let per_page = pagination.per_page.to_string();
        let page = pagination.page.to_string();
        let response = send_json_response(
            gitcode_request(http_client()?, &url, ctx.token.as_deref()).query(&[
                ("state", "all"),
                ("per_page", &per_page),
                ("page", &page),
            ]),
        )
        .await?;
        let items = response.value.as_array().ok_or_else(|| {
            ReviewPlatformError::Parse("GitCode pull response was not an array".to_string())
        })?;
        let total = header_u64(&response.headers, "x-total").or_else(|| {
            link_header_last_page(&response.headers).map(|last_page| {
                if last_page == pagination.page {
                    (u64::from(last_page.saturating_sub(1)) * u64::from(pagination.per_page))
                        + items.len() as u64
                } else {
                    u64::from(last_page) * u64::from(pagination.per_page)
                }
            })
        });
        let has_next = link_header_has_rel(&response.headers, "next")
            || total
                .map(|total| u64::from(pagination.page) * u64::from(pagination.per_page) < total)
                .unwrap_or(items.len() == pagination.per_page as usize);

        let pull_requests = items
            .iter()
            .map(gitcode_pull_request_from_value)
            .collect::<Vec<_>>();
        let pull_requests = enrich_gitcode_pull_request_counts(ctx, pull_requests).await;

        Ok(ReviewPlatformPullRequestPage {
            items: pull_requests,
            pagination: ReviewPlatformPagination {
                page: pagination.page,
                per_page: pagination.per_page,
                total,
                has_next,
            },
        })
    }

    async fn pull_request_detail(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
    ) -> Result<ReviewPlatformPullRequestDetail, ReviewPlatformError> {
        let client = http_client()?;
        let base = format!(
            "{}/repos/{}/{}/pulls/{}",
            ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request_id
        );
        let detail =
            send_json(gitcode_request(client.clone(), &base, ctx.token.as_deref())).await?;
        let token = ctx.token.clone();
        let files_url = format!("{}/files", base);
        let files = fetch_paginated_array(
            |page| {
                let page = page.to_string();
                gitcode_request(client.clone(), &files_url, token.as_deref())
                    .query(&[("per_page", "100"), ("page", &page)])
            },
            github_next_page,
        )
        .await
        .unwrap_or(Value::Array(Vec::new()));
        let token = ctx.token.clone();
        let commits_url = format!("{}/commits", base);
        let commits = fetch_paginated_array(
            |page| {
                let page = page.to_string();
                gitcode_request(client.clone(), &commits_url, token.as_deref())
                    .query(&[("per_page", "100"), ("page", &page)])
            },
            github_next_page,
        )
        .await
        .unwrap_or(Value::Array(Vec::new()));
        let token = ctx.token.clone();
        let comments_url = format!("{}/comments", base);
        let comments = fetch_paginated_array(
            |page| {
                let page = page.to_string();
                gitcode_request(client.clone(), &comments_url, token.as_deref())
                    .query(&[("per_page", "100"), ("page", &page)])
            },
            github_next_page,
        )
        .await
        .unwrap_or(Value::Array(Vec::new()));
        let ci = gitcode_ci_items(&detail);
        let mut pull_request = gitcode_pull_request_from_value(&detail);
        pull_request.checks = summarize_ci_items(&ci);

        Ok(ReviewPlatformPullRequestDetail {
            body: first_non_empty(&[
                value_string(&detail, "body"),
                value_string(&detail, "description"),
            ]),
            pull_request,
            ci,
            files: array_items(&files)
                .iter()
                .map(gitcode_file_from_value)
                .collect(),
            commits: array_items(&commits)
                .iter()
                .map(gitcode_commit_from_value)
                .collect(),
            threads: gitcode_threads(&comments),
        })
    }

    async fn pull_request_review_target(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
    ) -> Result<ReviewPlatformPullRequestReviewTarget, ReviewPlatformError> {
        let (pull_request, files) = gitcode_review_target_parts(ctx, pull_request_id).await?;
        Ok(review_target_from_parts(pull_request, files))
    }

    async fn pull_request_file_diff(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
        expected_base_revision: &str,
        expected_head_revision: &str,
        file_path: &str,
        file_page_hint: Option<u32>,
    ) -> Result<ReviewPlatformPullRequestFileDiff, ReviewPlatformError> {
        let (pull_request, files) =
            gitcode_review_file_parts(ctx, pull_request_id, file_path, file_page_hint).await?;
        file_diff_from_parts(
            pull_request,
            files,
            expected_base_revision,
            expected_head_revision,
            file_path,
        )
    }

    async fn pull_request_detail_page(
        &self,
        ctx: &ProviderContext,
        pull_request_id: &str,
        section: ReviewPlatformDetailSection,
        pagination: PullRequestPagination,
    ) -> Result<ReviewPlatformPullRequestDetailPage, ReviewPlatformError> {
        gitcode_pull_request_detail_page(ctx, pull_request_id, section, pagination).await
    }

    async fn pull_request_ci_log(
        &self,
        _ctx: &ProviderContext,
        _pull_request_id: &str,
        ci_item_id: &str,
        _ci_item_name: &str,
    ) -> Result<ReviewPlatformCiLog, ReviewPlatformError> {
        Ok(ReviewPlatformCiLog {
            ci_item_id: ci_item_id.to_string(),
            log: None,
            truncated: false,
            message: Some(
                "GitCode CI log retrieval is not available through a documented API.".to_string(),
            ),
        })
    }

    async fn create_pull_request(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformCreatePullRequestRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        let token = require_write_token(ctx, "Creating a GitCode pull request")?;
        let url = format!(
            "{}/repos/{}/{}/pulls",
            ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name
        );
        let value = send_json(
            gitcode_post_request(http_client()?, &url, Some(token)).json(&json!({
                "title": request.title,
                "head": request.source_branch,
                "base": request.target_branch,
                "body": request.body.clone().unwrap_or_default(),
                "draft": request.draft.unwrap_or(false),
            })),
        )
        .await?;
        let pull_request = gitcode_pull_request_from_value(&value);
        let web_url = Some(pull_request.web_url.clone());
        Ok(ReviewPlatformActionResult {
            success: true,
            message: format!("Created GitCode pull request #{}", pull_request.number),
            web_url,
            pull_request: Some(pull_request),
            thread: None,
        })
    }

    async fn submit_review(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformSubmitReviewRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        if request.event != ReviewSubmitEvent::Comment {
            return Err(ReviewPlatformError::UnsupportedPlatform(
                "GitCode submit_review supports comments only; use approve_pull_request for review processing"
                    .to_string(),
            ));
        }
        gitcode_add_pull_request_comment(ctx, &request.pull_request_id, &request.body).await
    }

    async fn approve_pull_request(
        &self,
        ctx: &ProviderContext,
        request: &ReviewPlatformApprovalRequest,
    ) -> Result<ReviewPlatformActionResult, ReviewPlatformError> {
        let token = require_write_token(ctx, "Approving a GitCode pull request")?;
        let url = format!(
            "{}/repos/{}/{}/pulls/{}/review",
            ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, request.pull_request_id
        );
        send_json(
            gitcode_post_request(http_client()?, &url, Some(token))
                .json(&json!({ "force": false })),
        )
        .await?;
        if let Some(body) = request
            .body
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            let _ = gitcode_add_pull_request_comment(ctx, &request.pull_request_id, body).await;
        }
        Ok(ReviewPlatformActionResult {
            success: true,
            message: "Approved GitCode pull request".to_string(),
            web_url: None,
            pull_request: None,
            thread: None,
        })
    }
}

#[async_trait::async_trait]
impl ReviewProvider for UnsupportedProvider {
    async fn list_pull_requests(
        &self,
        ctx: &ProviderContext,
        _pagination: PullRequestPagination,
    ) -> Result<ReviewPlatformPullRequestPage, ReviewPlatformError> {
        Err(ReviewPlatformError::UnsupportedPlatform(
            ctx.remote.host.clone(),
        ))
    }

    async fn pull_request_detail(
        &self,
        ctx: &ProviderContext,
        _pull_request_id: &str,
    ) -> Result<ReviewPlatformPullRequestDetail, ReviewPlatformError> {
        Err(ReviewPlatformError::UnsupportedPlatform(
            ctx.remote.host.clone(),
        ))
    }
}

fn http_client() -> Result<ReviewHttpClient, ReviewPlatformError> {
    ReviewHttpClient::new_review_platform().map_err(review_http_error)
}

type JsonResponse = ReviewJsonResponse;

fn review_http_error(error: ReviewHttpError) -> ReviewPlatformError {
    match error {
        ReviewHttpError::BuildClient(message) | ReviewHttpError::Network(message) => {
            ReviewPlatformError::Network(message)
        }
        ReviewHttpError::Http { status, message } => ReviewPlatformError::Http { status, message },
        ReviewHttpError::Parse(message) => ReviewPlatformError::Parse(message),
        ReviewHttpError::ResponseTooLarge { limit_bytes } => ReviewPlatformError::Parse(format!(
            "Provider response exceeded the {limit_bytes} byte Review limit"
        )),
    }
}

async fn send_json(request: ReviewHttpRequest) -> Result<Value, ReviewPlatformError> {
    send_review_json(request).await.map_err(review_http_error)
}

async fn send_json_response(
    request: ReviewHttpRequest,
) -> Result<JsonResponse, ReviewPlatformError> {
    send_review_json_response(request)
        .await
        .map_err(review_http_error)
}

async fn send_bounded_json(request: ReviewHttpRequest) -> Result<Value, ReviewPlatformError> {
    send_review_json_response_bounded(request, MAX_REVIEW_TARGET_RESPONSE_BYTES)
        .await
        .map(|response| response.value)
        .map_err(review_http_error)
}

async fn send_bounded_json_response(
    request: ReviewHttpRequest,
) -> Result<JsonResponse, ReviewPlatformError> {
    send_review_json_response_bounded(request, MAX_REVIEW_TARGET_RESPONSE_BYTES)
        .await
        .map_err(review_http_error)
}

async fn send_text(request: ReviewHttpRequest) -> Result<String, ReviewPlatformError> {
    send_review_text(request).await.map_err(review_http_error)
}

async fn fetch_paginated_array<F>(
    mut build_request: F,
    next_page: fn(&ReviewHttpHeaders, u32) -> Option<u32>,
) -> Result<Value, ReviewPlatformError>
where
    F: FnMut(u32) -> ReviewHttpRequest,
{
    let mut page = 1;
    let mut values = Vec::new();

    loop {
        let response = send_json_response(build_request(page)).await?;
        let items = response.value.as_array().ok_or_else(|| {
            ReviewPlatformError::Parse("Provider paginated response was not an array".to_string())
        })?;
        values.extend(items.iter().cloned());

        let Some(next) = next_page(&response.headers, page).filter(|next| *next > page) else {
            break;
        };
        page = next;
    }

    Ok(Value::Array(values))
}

async fn fetch_array_page(
    request: ReviewHttpRequest,
    pagination: PullRequestPagination,
) -> Result<JsonResponse, ReviewPlatformError> {
    let page = pagination.page.to_string();
    let per_page = pagination.per_page.to_string();
    let response =
        send_json_response(request.query(&[("per_page", &per_page), ("page", &page)])).await?;
    response.value.as_array().ok_or_else(|| {
        ReviewPlatformError::Parse("Provider paginated response was not an array".to_string())
    })?;
    Ok(response)
}

fn pagination_from_response(
    response: &JsonResponse,
    pagination: PullRequestPagination,
) -> ReviewPlatformPagination {
    let item_count = response.value.as_array().map(Vec::len).unwrap_or(0);
    let total = header_u64(&response.headers, "x-total")
        .or_else(|| pagination_total_from_links(&response.headers, pagination, item_count));
    ReviewPlatformPagination {
        page: pagination.page,
        per_page: pagination.per_page,
        total,
        has_next: link_header_has_rel(&response.headers, "next")
            || header_string(&response.headers, "x-next-page")
                .is_some_and(|value| !value.trim().is_empty())
            || total
                .map(|total| u64::from(pagination.page) * u64::from(pagination.per_page) < total)
                .unwrap_or(false),
    }
}

fn combine_page_pagination(
    pagination: PullRequestPagination,
    pages: &[ReviewPlatformPagination],
) -> ReviewPlatformPagination {
    let totals = if pages.iter().any(|page| page.has_next) {
        None
    } else {
        pages
            .iter()
            .map(|page| page.total)
            .collect::<Option<Vec<_>>>()
            .map(|values| values.into_iter().sum())
    };
    ReviewPlatformPagination {
        page: pagination.page,
        per_page: pagination.per_page,
        total: totals,
        has_next: pages.iter().any(|page| page.has_next),
    }
}

fn header_string(headers: &ReviewHttpHeaders, name: &str) -> Option<String> {
    headers.get(name).map(str::to_string)
}

fn header_u64(headers: &ReviewHttpHeaders, name: &str) -> Option<u64> {
    header_string(headers, name).and_then(|value| value.parse::<u64>().ok())
}

fn link_header_has_rel(headers: &ReviewHttpHeaders, rel: &str) -> bool {
    header_string(headers, "link")
        .as_deref()
        .is_some_and(|value| {
            value
                .split(',')
                .any(|part| part.contains(&format!("rel=\"{}\"", rel)))
        })
}

fn link_header_last_page(headers: &ReviewHttpHeaders) -> Option<u32> {
    let link = header_string(headers, "link")?;
    for part in link.split(',') {
        if !part.contains("rel=\"last\"") {
            continue;
        }
        let url = part
            .split(';')
            .next()?
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>');
        return query_param_u32(url, "page");
    }
    None
}

fn pagination_total_from_links(
    headers: &ReviewHttpHeaders,
    pagination: PullRequestPagination,
    item_count: usize,
) -> Option<u64> {
    if let Some(last_page) = link_header_last_page(headers) {
        if pagination.per_page == 1 {
            return Some(u64::from(last_page));
        }
        if last_page == pagination.page {
            return Some(
                u64::from(pagination.page.saturating_sub(1)) * u64::from(pagination.per_page)
                    + item_count as u64,
            );
        }
        return None;
    }

    Some(
        u64::from(pagination.page.saturating_sub(1)) * u64::from(pagination.per_page)
            + item_count as u64,
    )
}

fn pagination_from_total(
    pagination: PullRequestPagination,
    total: usize,
) -> ReviewPlatformPagination {
    ReviewPlatformPagination {
        page: pagination.page,
        per_page: pagination.per_page,
        total: Some(total as u64),
        has_next: usize::try_from(pagination.page)
            .ok()
            .is_some_and(|page| page * (pagination.per_page as usize) < total),
    }
}

fn slice_page<T>(items: Vec<T>, pagination: PullRequestPagination) -> Vec<T> {
    let start = pagination
        .page
        .saturating_sub(1)
        .saturating_mul(pagination.per_page) as usize;
    items
        .into_iter()
        .skip(start)
        .take(pagination.per_page as usize)
        .collect()
}

fn empty_detail_pagination(
    section: ReviewPlatformDetailSection,
    pagination: PullRequestPagination,
) -> ReviewPlatformPagination {
    ReviewPlatformPagination {
        page: pagination.page,
        per_page: pagination.per_page,
        total: if section == ReviewPlatformDetailSection::Overview {
            Some(0)
        } else {
            None
        },
        has_next: false,
    }
}

fn github_next_page(headers: &ReviewHttpHeaders, current_page: u32) -> Option<u32> {
    if link_header_has_rel(headers, "next") {
        Some(current_page.saturating_add(1))
    } else {
        None
    }
}

fn gitlab_next_page(headers: &ReviewHttpHeaders, _current_page: u32) -> Option<u32> {
    header_string(headers, "x-next-page").and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            trimmed.parse::<u32>().ok()
        }
    })
}

fn query_param_u32(url: &str, name: &str) -> Option<u32> {
    let query = url.split_once('?')?.1;
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            if key == name {
                return value.parse::<u32>().ok();
            }
        }
    }
    None
}

async fn enrich_github_pull_request_counts(
    ctx: &ProviderContext,
    pull_requests: Vec<ReviewPlatformPullRequest>,
) -> Vec<ReviewPlatformPullRequest> {
    let Ok(client) = http_client() else {
        return pull_requests;
    };
    let futures = pull_requests.into_iter().map(|mut pull_request| {
        let client = client.clone();
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request.id
        );
        let token = ctx.token.clone();
        async move {
            if let Ok(value) = send_json(github_request(client, &url, token.as_deref())).await {
                pull_request.additions = value_i64(&value, "additions") as i32;
                pull_request.deletions = value_i64(&value, "deletions") as i32;
                pull_request.changed_files = value_i64(&value, "changed_files") as i32;
                pull_request.comments =
                    (value_i64(&value, "comments") + value_i64(&value, "review_comments")) as i32;
            }
            pull_request
        }
    });
    stream::iter(futures)
        .buffered(PROVIDER_ENRICH_CONCURRENCY)
        .collect()
        .await
}

async fn enrich_gitlab_pull_request_counts(
    ctx: &ProviderContext,
    pull_requests: Vec<ReviewPlatformPullRequest>,
) -> Vec<ReviewPlatformPullRequest> {
    let Ok(client) = http_client() else {
        return pull_requests;
    };
    let project = urlencoding::encode(&ctx.remote.project_path).to_string();
    let futures = pull_requests.into_iter().map(|mut pull_request| {
        let client = client.clone();
        let url = format!(
            "{}/projects/{}/merge_requests/{}/changes",
            ctx.api_base_url, project, pull_request.id
        );
        let token = ctx.token.clone();
        async move {
            if let Ok(value) = send_json(gitlab_request(client, &url, token.as_deref())).await {
                let files = gitlab_files(&value);
                apply_files_stats(&mut pull_request, &files);
            }
            pull_request
        }
    });
    stream::iter(futures)
        .buffered(PROVIDER_ENRICH_CONCURRENCY)
        .collect()
        .await
}

async fn enrich_gitcode_pull_request_counts(
    ctx: &ProviderContext,
    pull_requests: Vec<ReviewPlatformPullRequest>,
) -> Vec<ReviewPlatformPullRequest> {
    let Ok(client) = http_client() else {
        return pull_requests;
    };
    let futures = pull_requests.into_iter().map(|mut pull_request| {
        let client = client.clone();
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, pull_request.id
        );
        let token = ctx.token.clone();
        async move {
            if let Ok(value) = send_json(gitcode_request(client, &url, token.as_deref())).await {
                let detail = gitcode_pull_request_from_value(&value);
                pull_request.additions = detail.additions;
                pull_request.deletions = detail.deletions;
                pull_request.changed_files = detail.changed_files;
                pull_request.comments = detail.comments;
            }
            pull_request
        }
    });
    stream::iter(futures)
        .buffered(PROVIDER_ENRICH_CONCURRENCY)
        .collect()
        .await
}

fn github_request(client: ReviewHttpClient, url: &str, token: Option<&str>) -> ReviewHttpRequest {
    let mut request = client
        .get(url)
        .header(USER_AGENT_HEADER, USER_AGENT_VALUE)
        .header(ACCEPT_HEADER, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = token {
        request = request.header(AUTHORIZATION_HEADER, format!("Bearer {}", token));
    }
    request
}

fn github_post_request(
    client: ReviewHttpClient,
    url: &str,
    token: Option<&str>,
) -> ReviewHttpRequest {
    let mut request = client
        .post(url)
        .header(USER_AGENT_HEADER, USER_AGENT_VALUE)
        .header(ACCEPT_HEADER, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = token {
        request = request.header(AUTHORIZATION_HEADER, format!("Bearer {}", token));
    }
    request
}

fn gitlab_request(client: ReviewHttpClient, url: &str, token: Option<&str>) -> ReviewHttpRequest {
    let mut request = client
        .get(url)
        .header(USER_AGENT_HEADER, USER_AGENT_VALUE)
        .header(ACCEPT_HEADER, "application/json");
    if let Some(token) = token {
        request = request.header("PRIVATE-TOKEN", token);
    }
    request
}

fn gitlab_post_request(
    client: ReviewHttpClient,
    url: &str,
    token: Option<&str>,
) -> ReviewHttpRequest {
    let mut request = client
        .post(url)
        .header(USER_AGENT_HEADER, USER_AGENT_VALUE)
        .header(ACCEPT_HEADER, "application/json");
    if let Some(token) = token {
        request = request.header("PRIVATE-TOKEN", token);
    }
    request
}

fn gitlab_put_request(
    client: ReviewHttpClient,
    url: &str,
    token: Option<&str>,
) -> ReviewHttpRequest {
    let mut request = client
        .put(url)
        .header(USER_AGENT_HEADER, USER_AGENT_VALUE)
        .header(ACCEPT_HEADER, "application/json");
    if let Some(token) = token {
        request = request.header("PRIVATE-TOKEN", token);
    }
    request
}

fn gitcode_request(client: ReviewHttpClient, url: &str, token: Option<&str>) -> ReviewHttpRequest {
    let mut request = client
        .get(url)
        .header(USER_AGENT_HEADER, USER_AGENT_VALUE)
        .header(ACCEPT_HEADER, "application/json");
    if let Some(token) = token {
        request = request
            .header("PRIVATE-TOKEN", token)
            .header(AUTHORIZATION_HEADER, format!("Bearer {}", token))
            .query(&[("access_token", token)]);
    }
    request
}

fn gitcode_post_request(
    client: ReviewHttpClient,
    url: &str,
    token: Option<&str>,
) -> ReviewHttpRequest {
    let mut request = client
        .post(url)
        .header(USER_AGENT_HEADER, USER_AGENT_VALUE)
        .header(ACCEPT_HEADER, "application/json");
    if let Some(token) = token {
        request = request
            .header("PRIVATE-TOKEN", token)
            .header(AUTHORIZATION_HEADER, format!("Bearer {}", token))
            .query(&[("access_token", token)]);
    }
    request
}

fn require_write_token<'a>(
    ctx: &'a ProviderContext,
    action: &str,
) -> Result<&'a str, ReviewPlatformError> {
    ctx.token.as_deref().ok_or_else(|| {
        ReviewPlatformError::Api(format!(
            "{} requires a {} token for {}",
            action,
            platform_label(ctx.remote.platform),
            ctx.remote.host
        ))
    })
}

fn provider_context(
    remote: ReviewPlatformRemote,
    auth_tokens: &ReviewPlatformAuthTokens,
) -> Result<ProviderContext, ReviewPlatformError> {
    let api_base_url = match remote.platform {
        ReviewPlatformKind::Github => "https://api.github.com".to_string(),
        ReviewPlatformKind::Gitlab => format!("https://{}/api/v4", remote.host),
        ReviewPlatformKind::Gitcode => "https://api.gitcode.com/api/v5".to_string(),
        ReviewPlatformKind::Unknown => {
            return Err(ReviewPlatformError::UnsupportedPlatform(remote.host));
        }
    };
    let token = token_for_remote(&remote, auth_tokens);
    Ok(ProviderContext {
        remote,
        api_base_url,
        token,
    })
}

fn token_for_remote(
    remote: &ReviewPlatformRemote,
    auth_tokens: &ReviewPlatformAuthTokens,
) -> Option<String> {
    auth_tokens
        .get(remote.platform, &remote.host)
        .map(str::to_string)
        .or_else(|| env_token_for_platform(remote.platform))
}

fn env_token_for_platform(platform: ReviewPlatformKind) -> Option<String> {
    let names: &[&str] = match platform {
        ReviewPlatformKind::Github => &["GITHUB_TOKEN", "GH_TOKEN"],
        ReviewPlatformKind::Gitlab => &["GITLAB_TOKEN", "GITLAB_PRIVATE_TOKEN"],
        ReviewPlatformKind::Gitcode => &["GITCODE_TOKEN"],
        ReviewPlatformKind::Unknown => &[],
    };
    names.iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn auth_for_platform_host(
    platform: ReviewPlatformKind,
    host: &str,
    auth_tokens: &ReviewPlatformAuthTokens,
) -> (ReviewAuthState, ReviewAuthSource) {
    if platform == ReviewPlatformKind::Unknown {
        return (ReviewAuthState::Unsupported, ReviewAuthSource::Unsupported);
    }
    if auth_tokens.get(platform, host).is_some() {
        return (ReviewAuthState::Connected, ReviewAuthSource::Stored);
    }
    if env_token_for_platform(platform).is_some() {
        return (ReviewAuthState::Connected, ReviewAuthSource::Env);
    }
    if platform == ReviewPlatformKind::Gitcode {
        (ReviewAuthState::NotConnected, ReviewAuthSource::None)
    } else {
        (ReviewAuthState::NotRequired, ReviewAuthSource::None)
    }
}

fn token_key(platform: ReviewPlatformKind, host: &str) -> Option<String> {
    if platform == ReviewPlatformKind::Unknown {
        return None;
    }
    let host = host.trim().to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    Some(format!("{}:{}", platform.as_str(), host))
}

async fn get_repository_root(repository_path: &str) -> Result<String, ReviewPlatformError> {
    let output = execute_git_command(repository_path, &["rev-parse", "--show-toplevel"]).await?;
    let root = output
        .lines()
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ReviewPlatformError::InvalidRepository(
                "Git repository root was not returned".to_string(),
            )
        })?;

    Ok(normalize_repository_root(root))
}

async fn execute_git_command(
    current_dir: &str,
    args: &[&str],
) -> Result<String, ReviewPlatformError> {
    let current_dir_path = Path::new(current_dir);
    let current_dir_path = if current_dir_path.is_file() {
        current_dir_path.parent().unwrap_or(current_dir_path)
    } else {
        current_dir_path
    };

    let output = process_manager::create_tokio_command("git")
        .current_dir(current_dir_path)
        .args(args)
        .output()
        .await
        .map_err(|error| {
            ReviewPlatformError::InvalidRepository(format!(
                "Failed to execute git command: {}",
                error
            ))
        })?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }

    let message = if output.stderr.is_empty() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        String::from_utf8_lossy(&output.stderr).to_string()
    };
    Err(ReviewPlatformError::InvalidRepository(message))
}

fn normalize_repository_root(root: &str) -> String {
    let root = root.trim();
    #[cfg(windows)]
    {
        root.replace('/', "\\")
    }
    #[cfg(not(windows))]
    {
        root.to_string()
    }
}

impl ReviewPlatformService {
    async fn load_stored_tokens(&self) -> Result<ReviewPlatformAuthTokens, ReviewPlatformError> {
        let stored = self.load_stored_token_file().await?;
        Ok(ReviewPlatformAuthTokens {
            tokens: stored
                .tokens
                .into_iter()
                .filter_map(|(key, entry)| {
                    let token = entry.token.trim().to_string();
                    if token.is_empty() {
                        None
                    } else {
                        Some((key, token))
                    }
                })
                .collect(),
        })
    }

    async fn load_stored_token_file(
        &self,
    ) -> Result<StoredReviewPlatformTokens, ReviewPlatformError> {
        let path = self.token_store_path();
        match fs::read_to_string(path).await {
            Ok(content) => serde_json::from_str::<StoredReviewPlatformTokens>(&content)
                .map_err(|error| ReviewPlatformError::Parse(error.to_string())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(StoredReviewPlatformTokens::default())
            }
            Err(error) => Err(ReviewPlatformError::Api(format!(
                "Failed to read review platform token store: {}",
                error
            ))),
        }
    }

    async fn save_stored_token_file(
        &self,
        stored: &StoredReviewPlatformTokens,
    ) -> Result<(), ReviewPlatformError> {
        let path = self.token_store_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|error| {
                ReviewPlatformError::Api(format!(
                    "Failed to create review platform token store directory: {}",
                    error
                ))
            })?;
        }
        let content = serde_json::to_string_pretty(stored)
            .map_err(|error| ReviewPlatformError::Parse(error.to_string()))?;
        fs::write(path, content).await.map_err(|error| {
            ReviewPlatformError::Api(format!(
                "Failed to write review platform token store: {}",
                error
            ))
        })
    }
}

fn select_remote<'a>(
    remotes: &'a [ReviewPlatformRemote],
    remote_id: Option<&str>,
) -> Option<&'a ReviewPlatformRemote> {
    if let Some(remote_id) = remote_id {
        if let Some(remote) = remotes.iter().find(|remote| remote.id == remote_id) {
            return Some(remote);
        }
    }
    remotes
        .iter()
        .find(|remote| remote.supported)
        .or_else(|| remotes.first())
}

fn select_remote_for_action<'a>(
    remotes: &'a [ReviewPlatformRemote],
    remote_id: Option<&str>,
) -> Result<&'a ReviewPlatformRemote, ReviewPlatformError> {
    if let Some(remote_id) = remote_id {
        return remotes
            .iter()
            .find(|remote| remote.id == remote_id)
            .ok_or_else(|| ReviewPlatformError::RemoteNotFound(remote_id.to_string()));
    }

    let supported = remotes
        .iter()
        .filter(|remote| remote.supported)
        .collect::<Vec<_>>();
    match supported.as_slice() {
        [] => remotes
            .first()
            .ok_or_else(|| ReviewPlatformError::RemoteNotFound("default".to_string())),
        [remote] => Ok(remote),
        _ => Err(ReviewPlatformError::Api(format!(
            "Multiple supported review platform remotes were found. Provide remote_id explicitly. Candidate remotes:\n{}",
            supported
                .iter()
                .map(|remote| format!(
                    "- remote_id: {} | name: {} | platform: {:?} | project: {} | url: {}",
                    remote.id, remote.name, remote.platform, remote.project_path, remote.web_url
                ))
                .collect::<Vec<_>>()
                .join("\n")
        ))),
    }
}

fn empty_snapshot(
    remotes: Vec<ReviewPlatformRemote>,
    selected_remote_id: Option<String>,
    account: Option<ReviewPlatformAccount>,
    message: &str,
) -> ReviewPlatformWorkspaceSnapshot {
    let mut accounts = account.into_iter().collect::<Vec<_>>();
    if let Some(account) = accounts.first_mut() {
        if account.message.is_none() && !message.trim().is_empty() {
            account.message = Some(message.to_string());
        }
    }

    ReviewPlatformWorkspaceSnapshot {
        remotes,
        selected_remote_id,
        accounts,
        repository: None,
        pull_requests: Vec::new(),
        pagination: ReviewPlatformPagination {
            page: DEFAULT_PR_PAGE,
            per_page: DEFAULT_PR_PAGE_SIZE,
            total: Some(0),
            has_next: false,
        },
        capabilities: ReviewPlatformCapabilities {
            can_create_review: false,
            can_create_pull_request: false,
            can_reply_to_thread: false,
            can_resolve_thread: false,
            can_approve: false,
            can_revoke_approval: false,
            can_request_changes: false,
            can_merge: false,
            supports_draft_review: false,
        },
        message: if message.trim().is_empty() {
            None
        } else {
            Some(message.to_string())
        },
        auth_challenge: None,
    }
}

fn auth_required_snapshot(
    remotes: Vec<ReviewPlatformRemote>,
    remote: ReviewPlatformRemote,
    repository: Option<ReviewPlatformRepositoryRef>,
    account: ReviewPlatformAccount,
    capabilities: ReviewPlatformCapabilities,
    challenge: ReviewPlatformAuthChallenge,
) -> ReviewPlatformWorkspaceSnapshot {
    ReviewPlatformWorkspaceSnapshot {
        remotes,
        selected_remote_id: Some(remote.id),
        accounts: vec![account],
        repository,
        pull_requests: Vec::new(),
        pagination: ReviewPlatformPagination {
            page: DEFAULT_PR_PAGE,
            per_page: DEFAULT_PR_PAGE_SIZE,
            total: Some(0),
            has_next: false,
        },
        capabilities,
        message: Some(challenge.message.clone()),
        auth_challenge: Some(challenge),
    }
}

fn repository_ref(
    remote: &ReviewPlatformRemote,
    workspace_path: Option<String>,
) -> ReviewPlatformRepositoryRef {
    ReviewPlatformRepositoryRef {
        provider_id: remote.id.clone(),
        platform: remote.platform,
        host: remote.host.clone(),
        owner: remote.owner.clone(),
        name: remote.repository_name.clone(),
        project_path: remote.project_path.clone(),
        default_branch: "main".to_string(),
        workspace_path,
        web_url: remote.web_url.clone(),
    }
}

fn account_for_remote(remote: &ReviewPlatformRemote) -> ReviewPlatformAccount {
    ReviewPlatformAccount {
        id: remote.id.clone(),
        platform: remote.platform,
        label: format!("{} ({})", platform_label(remote.platform), remote.host),
        username: None,
        host: remote.host.clone(),
        auth_state: remote.auth_state,
        auth_source: remote.auth_source,
        scopes: if matches!(
            remote.auth_source,
            ReviewAuthSource::Env | ReviewAuthSource::Stored
        ) {
            vec!["pull_request:read".to_string()]
        } else {
            Vec::new()
        },
        message: remote.message.clone(),
    }
}

fn capabilities_for_remote(_remote: &ReviewPlatformRemote) -> ReviewPlatformCapabilities {
    let platform = _remote.platform;
    ReviewPlatformCapabilities {
        can_create_review: matches!(
            platform,
            ReviewPlatformKind::Github | ReviewPlatformKind::Gitlab | ReviewPlatformKind::Gitcode
        ),
        can_create_pull_request: matches!(
            platform,
            ReviewPlatformKind::Github | ReviewPlatformKind::Gitlab | ReviewPlatformKind::Gitcode
        ),
        can_reply_to_thread: matches!(
            platform,
            ReviewPlatformKind::Github | ReviewPlatformKind::Gitlab
        ),
        can_resolve_thread: matches!(platform, ReviewPlatformKind::Gitlab),
        can_approve: matches!(
            platform,
            ReviewPlatformKind::Github | ReviewPlatformKind::Gitlab | ReviewPlatformKind::Gitcode
        ),
        can_revoke_approval: matches!(platform, ReviewPlatformKind::Gitlab),
        can_request_changes: matches!(platform, ReviewPlatformKind::Github),
        can_merge: false,
        supports_draft_review: matches!(platform, ReviewPlatformKind::Github),
    }
}

fn platform_label(platform: ReviewPlatformKind) -> &'static str {
    match platform {
        ReviewPlatformKind::Github => "GitHub",
        ReviewPlatformKind::Gitlab => "GitLab",
        ReviewPlatformKind::Gitcode => "GitCode",
        ReviewPlatformKind::Unknown => "Git",
    }
}

fn required_scopes_for_platform(platform: ReviewPlatformKind) -> Vec<String> {
    match platform {
        ReviewPlatformKind::Github => vec!["repo".to_string(), "pull_requests:read".to_string()],
        ReviewPlatformKind::Gitlab => {
            vec!["read_api".to_string(), "api for write actions".to_string()]
        }
        ReviewPlatformKind::Gitcode => vec!["pull_request".to_string()],
        ReviewPlatformKind::Unknown => Vec::new(),
    }
}

fn auth_state_for_challenge(state: ReviewPlatformAuthChallengeState) -> ReviewAuthState {
    match state {
        ReviewPlatformAuthChallengeState::Missing => ReviewAuthState::NotConnected,
        ReviewPlatformAuthChallengeState::Invalid => ReviewAuthState::Expired,
        ReviewPlatformAuthChallengeState::InsufficientScope => ReviewAuthState::Error,
    }
}

fn is_auth_http_error(error: &ReviewPlatformError) -> bool {
    matches!(
        error,
        ReviewPlatformError::Http {
            status: 401 | 403,
            ..
        }
    )
}

fn auth_challenge_for_remote(
    remote: &ReviewPlatformRemote,
    error: &ReviewPlatformError,
    has_token: bool,
) -> ReviewPlatformAuthChallenge {
    let status = match error {
        ReviewPlatformError::Http { status, .. } => *status,
        _ => 0,
    };
    let state = if !has_token {
        ReviewPlatformAuthChallengeState::Missing
    } else if status == 403 {
        ReviewPlatformAuthChallengeState::InsufficientScope
    } else {
        ReviewPlatformAuthChallengeState::Invalid
    };
    let action = match state {
        ReviewPlatformAuthChallengeState::Missing => "Add",
        ReviewPlatformAuthChallengeState::Invalid => "Update",
        ReviewPlatformAuthChallengeState::InsufficientScope => "Update",
    };
    let reason = match state {
        ReviewPlatformAuthChallengeState::Missing => {
            "a token is required to access this repository"
        }
        ReviewPlatformAuthChallengeState::Invalid => "the saved or environment token was rejected",
        ReviewPlatformAuthChallengeState::InsufficientScope => {
            "the token does not have enough permissions"
        }
    };
    ReviewPlatformAuthChallenge {
        platform: remote.platform,
        host: remote.host.clone(),
        remote_id: remote.id.clone(),
        project_path: remote.project_path.clone(),
        state,
        message: format!(
            "{} {} token for {}: {}.",
            action,
            platform_label(remote.platform),
            remote.host,
            reason
        ),
        required_scopes: required_scopes_for_platform(remote.platform),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CiOutcome {
    Passed,
    Failed,
    Pending,
}

fn summarize_ci_items(items: &[ReviewPlatformCiItem]) -> ReviewChecks {
    let mut checks = empty_checks();
    for item in items {
        match ci_item_outcome(item) {
            CiOutcome::Passed => checks.passed += 1,
            CiOutcome::Failed => checks.failed += 1,
            CiOutcome::Pending => checks.pending += 1,
        }
    }
    checks.total = checks.passed + checks.failed + checks.pending;
    checks
}

fn ci_item_outcome(item: &ReviewPlatformCiItem) -> CiOutcome {
    let status = item.status.trim().to_ascii_lowercase();
    let conclusion = item
        .conclusion
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();

    if conclusion.is_empty() {
        return ci_status_outcome(&status);
    }

    match conclusion.as_str() {
        "success" | "neutral" | "skipped" | "passed" => CiOutcome::Passed,
        "failure" | "timed_out" | "timed-out" | "cancelled" | "canceled" | "action_required"
        | "error" => CiOutcome::Failed,
        "queued"
        | "pending"
        | "running"
        | "in_progress"
        | "in progress"
        | "created"
        | "manual"
        | "scheduled"
        | "waiting_for_resource"
        | "preparing"
        | "requested" => CiOutcome::Pending,
        _ => ci_status_outcome(&status),
    }
}

fn ci_status_outcome(status: &str) -> CiOutcome {
    match status.trim().to_ascii_lowercase().as_str() {
        "success" | "passed" | "pass" | "skipped" | "ok" | "available" | "can_be_merged"
        | "mergeable" | "true" | "enabled" | "active" => CiOutcome::Passed,
        "failure" | "failed" | "fail" | "error" | "cancelled" | "canceled" | "cannot_be_merged"
        | "conflict" | "blocked" | "false" | "disabled" | "inactive" => CiOutcome::Failed,
        "pending"
        | "queued"
        | "running"
        | "in_progress"
        | "in progress"
        | "created"
        | "manual"
        | "scheduled"
        | "waiting_for_resource"
        | "preparing"
        | "requested"
        | "checking"
        | "unchecked"
        | "completed" => CiOutcome::Pending,
        _ => CiOutcome::Pending,
    }
}

fn ci_log_value(text: String) -> (Option<String>, bool) {
    let extracted = ci_error_excerpt(&text);
    let Some(excerpt) = extracted else {
        return (None, false);
    };
    let char_count = excerpt.chars().count();
    if char_count <= MAX_CI_LOG_CHARS {
        return (Some(excerpt), false);
    }

    (
        Some(format!(
            "[Error excerpt truncated: showing first {} of {} chars]\n{}",
            MAX_CI_LOG_CHARS,
            char_count,
            excerpt.chars().take(MAX_CI_LOG_CHARS).collect::<String>()
        )),
        true,
    )
}

fn empty_ci_log() -> (Option<String>, bool) {
    (None, false)
}

fn ci_error_excerpt(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !is_ci_error_line(line) {
            continue;
        }

        let start = index.saturating_sub(2);
        let mut end = (index + 6).min(lines.len());
        while end < lines.len() && lines[end].trim().is_empty() {
            end += 1;
        }
        ranges.push((start, end));
    }

    if ranges.is_empty() {
        return None;
    }

    ranges.sort_unstable_by_key(|range| range.0);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in ranges {
        if let Some(last) = merged.last_mut() {
            if start <= last.1.saturating_add(1) {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    let mut output = String::new();
    for (index, (start, end)) in merged.iter().enumerate() {
        if index > 0 {
            output.push_str("\n...\n");
        }
        for line in &lines[*start..*end] {
            output.push_str(line);
            output.push('\n');
        }
    }

    let output = output.trim_end_matches('\n').trim().to_string();
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

fn is_ci_error_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("##[error]")
        || lower.contains("error:")
        || lower.contains(" failed")
        || lower.contains("failure")
        || lower.contains("fatal")
        || lower.contains("exception")
        || lower.contains("traceback")
        || lower.contains("panic")
        || lower.contains("assertion failed")
        || lower.contains("command failed")
        || lower.contains("exited with code")
        || lower.contains("return code")
        || lower.contains("build failed")
        || lower.contains("test failed")
}

async fn github_checks_and_ci(
    ctx: &ProviderContext,
    client: &ReviewHttpClient,
    pull_detail: &Value,
) -> (ReviewChecks, Vec<ReviewPlatformCiItem>) {
    let sha = nested_string(pull_detail, &["head", "sha"]);
    if sha.trim().is_empty() {
        return (empty_checks(), Vec::new());
    }

    let mut ci_items = Vec::new();
    let status_url = format!(
        "{}/repos/{}/{}/commits/{}/status",
        ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, sha
    );
    if let Ok(status) = send_json(github_request(
        client.clone(),
        &status_url,
        ctx.token.as_deref(),
    ))
    .await
    {
        let statuses = status
            .get("statuses")
            .and_then(Value::as_array)
            .map(|items| items.as_slice())
            .unwrap_or(&[]);
        for (index, item) in statuses.iter().enumerate() {
            ci_items.push(ReviewPlatformCiItem {
                id: format!(
                    "status-{}",
                    first_non_empty(&[value_string(item, "id"), index.to_string()])
                ),
                name: first_non_empty(&[
                    value_string(item, "context"),
                    value_string(item, "description"),
                    "Status".to_string(),
                ]),
                status: value_string(item, "state"),
                conclusion: None,
                detail: optional_string(item, "description"),
                stage: None,
                web_url: optional_string(item, "target_url"),
                log: None,
                log_truncated: false,
                started_at: None,
                finished_at: None,
            });
        }
    }

    let check_runs_url = format!(
        "{}/repos/{}/{}/commits/{}/check-runs",
        ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, sha
    );
    if let Ok(check_runs) = send_json(
        github_request(client.clone(), &check_runs_url, ctx.token.as_deref())
            .query(&[("per_page", "100")]),
    )
    .await
    {
        for (index, item) in check_runs
            .get("check_runs")
            .and_then(Value::as_array)
            .map(|items| items.as_slice())
            .unwrap_or(&[])
            .iter()
            .enumerate()
        {
            ci_items.push(ReviewPlatformCiItem {
                id: format!(
                    "check-run-{}",
                    first_non_empty(&[value_string(item, "id"), index.to_string()])
                ),
                name: first_non_empty(&[value_string(item, "name"), "Check run".to_string()]),
                status: value_string(item, "status"),
                conclusion: optional_string(item, "conclusion"),
                detail: nested_optional_string(item, &["output", "summary"])
                    .or_else(|| nested_optional_string(item, &["output", "text"]))
                    .or_else(|| optional_string(item, "details_url")),
                stage: None,
                web_url: optional_string(item, "html_url")
                    .or_else(|| optional_string(item, "details_url")),
                log: None,
                log_truncated: false,
                started_at: optional_string(item, "started_at"),
                finished_at: optional_string(item, "completed_at"),
            });
        }
    }

    let checks = summarize_ci_items(&ci_items);
    (checks, ci_items)
}

async fn github_actions_jobs_for_head_sha(
    ctx: &ProviderContext,
    client: &ReviewHttpClient,
    sha: &str,
) -> Vec<Value> {
    let runs_url = format!(
        "{}/repos/{}/{}/actions/runs",
        ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name
    );
    let runs = match send_json(
        github_request(client.clone(), &runs_url, ctx.token.as_deref())
            .query(&[("head_sha", sha), ("per_page", "100")]),
    )
    .await
    {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    let mut jobs = Vec::new();
    for run in runs
        .get("workflow_runs")
        .and_then(Value::as_array)
        .map(|items| items.as_slice())
        .unwrap_or(&[])
    {
        let run_id = value_string(run, "id");
        if run_id.trim().is_empty() {
            continue;
        }
        let jobs_url = format!(
            "{}/repos/{}/{}/actions/runs/{}/jobs",
            ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, run_id
        );
        if let Ok(value) = send_json(
            github_request(client.clone(), &jobs_url, ctx.token.as_deref())
                .query(&[("per_page", "100")]),
        )
        .await
        {
            jobs.extend(
                value
                    .get("jobs")
                    .and_then(Value::as_array)
                    .map(|items| items.as_slice())
                    .unwrap_or(&[])
                    .iter()
                    .cloned(),
            );
        }
    }

    jobs
}

async fn github_actions_log_for_check_run_item(
    ctx: &ProviderContext,
    client: &ReviewHttpClient,
    check_run_id: &str,
    check_run_name: &str,
    head_sha: &str,
) -> Result<ReviewPlatformCiLog, ReviewPlatformError> {
    let action_jobs = github_actions_jobs_for_head_sha(ctx, client, head_sha).await;
    let check_run = action_jobs
        .iter()
        .find(|job| {
            let check_run_url = value_string(job, "check_run_url");
            check_run_url.ends_with(&format!("/check-runs/{}", check_run_id))
                || value_string(job, "name") == check_run_name
        })
        .cloned();

    let Some(job) = check_run else {
        return Ok(ReviewPlatformCiLog {
            ci_item_id: format!("check-run-{}", check_run_id),
            log: None,
            truncated: false,
            message: Some(
                "No matching GitHub Actions job was found for this check run.".to_string(),
            ),
        });
    };

    let job_id = value_string(&job, "id");
    if job_id.trim().is_empty() {
        return Ok(ReviewPlatformCiLog {
            ci_item_id: format!("check-run-{}", check_run_id),
            log: None,
            truncated: false,
            message: Some("The matching GitHub Actions job does not expose a job id.".to_string()),
        });
    }

    let logs_url = format!(
        "{}/repos/{}/{}/actions/jobs/{}/logs",
        ctx.api_base_url, ctx.remote.owner, ctx.remote.repository_name, job_id
    );
    let text = send_text(github_request(
        client.clone(),
        &logs_url,
        ctx.token.as_deref(),
    ))
    .await?;
    let (log, truncated) = ci_log_value(text);
    let message = log
        .as_ref()
        .is_none()
        .then_some("No error lines were detected in the GitHub Actions job log.".to_string());
    Ok(ReviewPlatformCiLog {
        ci_item_id: format!("check-run-{}", check_run_id),
        log,
        truncated,
        message,
    })
}

fn gitlab_pipeline_summary_item(detail: &Value) -> Option<ReviewPlatformCiItem> {
    let pipeline = detail.get("head_pipeline")?;
    let status = value_string(pipeline, "status");
    if status.trim().is_empty() {
        return None;
    }
    Some(ReviewPlatformCiItem {
        id: first_non_empty(&[
            value_string(pipeline, "id"),
            value_string(pipeline, "iid"),
            "head-pipeline".to_string(),
        ]),
        name: "Pipeline".to_string(),
        status,
        conclusion: None,
        detail: nested_optional_string(pipeline, &["detailed_status", "text"])
            .or_else(|| nested_optional_string(pipeline, &["detailed_status", "label"])),
        stage: None,
        web_url: optional_string(pipeline, "web_url"),
        log: None,
        log_truncated: false,
        started_at: optional_string(pipeline, "started_at"),
        finished_at: optional_string(pipeline, "finished_at"),
    })
}

async fn gitlab_pipeline_jobs(
    ctx: &ProviderContext,
    client: ReviewHttpClient,
    project: &str,
    pipeline_id: &str,
) -> Vec<ReviewPlatformCiItem> {
    let jobs_url = format!(
        "{}/projects/{}/pipelines/{}/jobs",
        ctx.api_base_url, project, pipeline_id
    );
    if let Ok(response) = fetch_paginated_array(
        |page| {
            let page = page.to_string();
            gitlab_request(client.clone(), &jobs_url, ctx.token.as_deref())
                .query(&[("per_page", "100"), ("page", &page)])
        },
        gitlab_next_page,
    )
    .await
    {
        let mut jobs = Vec::new();
        for (index, job) in array_items(&response).iter().enumerate() {
            let provider_id = value_string(job, "id");
            let id = first_non_empty(&[provider_id.clone(), index.to_string()]);
            jobs.push(ReviewPlatformCiItem {
                id,
                name: first_non_empty(&[value_string(job, "name"), "Job".to_string()]),
                status: value_string(job, "status"),
                conclusion: None,
                detail: optional_string(job, "failure_reason"),
                stage: optional_string(job, "stage"),
                web_url: optional_string(job, "web_url"),
                log: None,
                log_truncated: false,
                started_at: optional_string(job, "started_at"),
                finished_at: optional_string(job, "finished_at"),
            });
        }
        return jobs;
    }
    Vec::new()
}

async fn gitlab_job_trace(
    ctx: &ProviderContext,
    client: ReviewHttpClient,
    project: &str,
    job_id: &str,
) -> (Option<String>, bool) {
    if job_id.trim().is_empty() {
        return empty_ci_log();
    }
    let trace_url = format!(
        "{}/projects/{}/jobs/{}/trace",
        ctx.api_base_url, project, job_id
    );
    match send_text(gitlab_request(client, &trace_url, ctx.token.as_deref())).await {
        Ok(text) => ci_log_value(text),
        Err(_) => empty_ci_log(),
    }
}

async fn gitlab_pull_request_ci_log(
    ctx: &ProviderContext,
    _pull_request_id: &str,
    ci_item_id: &str,
    _ci_item_name: &str,
) -> Result<ReviewPlatformCiLog, ReviewPlatformError> {
    if ci_item_id == "head-pipeline" || ci_item_id == "pipeline" {
        return Ok(ReviewPlatformCiLog {
            ci_item_id: ci_item_id.to_string(),
            log: None,
            truncated: false,
            message: Some("Pipeline summaries do not expose a separate job trace.".to_string()),
        });
    }

    let client = http_client()?;
    let project = urlencoding::encode(&ctx.remote.project_path).to_string();
    let (log, truncated) = gitlab_job_trace(ctx, client, &project, ci_item_id).await;
    let message = log
        .as_ref()
        .is_none()
        .then_some("No error lines were detected in the job trace.".to_string());
    Ok(ReviewPlatformCiLog {
        ci_item_id: ci_item_id.to_string(),
        log,
        truncated,
        message,
    })
}

fn gitcode_ci_items(detail: &Value) -> Vec<ReviewPlatformCiItem> {
    let mut items = Vec::new();
    let pipeline_status = first_non_empty(&[
        value_string(detail, "pipeline_status"),
        value_string(detail, "pipeline_status_with_code_quality"),
    ]);
    if !pipeline_status.trim().is_empty() {
        items.push(ReviewPlatformCiItem {
            id: first_non_empty(&[
                value_string(detail, "head_pipeline_id"),
                "pipeline".to_string(),
            ]),
            name: "Pipeline".to_string(),
            status: pipeline_status,
            conclusion: None,
            detail: optional_string(detail, "pipeline_status_with_code_quality"),
            stage: None,
            web_url: optional_string(detail, "web_url")
                .or_else(|| optional_string(detail, "html_url")),
            log: None,
            log_truncated: false,
            started_at: None,
            finished_at: None,
        });
    }

    let codequality_status = value_string(detail, "codequality_status");
    if !codequality_status.trim().is_empty() {
        items.push(ReviewPlatformCiItem {
            id: first_non_empty(&[
                format!("{}-codequality", value_string(detail, "head_pipeline_id")),
                "codequality".to_string(),
            ]),
            name: "Code quality".to_string(),
            status: codequality_status,
            conclusion: None,
            detail: None,
            stage: None,
            web_url: optional_string(detail, "web_url")
                .or_else(|| optional_string(detail, "html_url")),
            log: None,
            log_truncated: false,
            started_at: None,
            finished_at: None,
        });
    }

    items
}

fn parse_remote(
    remote_name: &str,
    remote_url: &str,
    auth_tokens: &ReviewPlatformAuthTokens,
) -> Option<ReviewPlatformRemote> {
    let parsed = parse_remote_url(remote_url)?;
    let host_lower = parsed.host.to_ascii_lowercase();
    let platform = if host_lower.contains("github.com") {
        ReviewPlatformKind::Github
    } else if host_lower.contains("gitlab") {
        ReviewPlatformKind::Gitlab
    } else if host_lower.contains("gitcode") {
        ReviewPlatformKind::Gitcode
    } else {
        ReviewPlatformKind::Unknown
    };

    let segments: Vec<&str> = parsed
        .path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() < 2 {
        return None;
    }
    let owner = segments.first()?.to_string();
    let repository_name = segments.last()?.trim_end_matches(".git").to_string();
    let project_path = segments
        .iter()
        .map(|segment| segment.trim_end_matches(".git"))
        .collect::<Vec<_>>()
        .join("/");

    let supported = platform != ReviewPlatformKind::Unknown;
    let (auth_state, auth_source) = auth_for_platform_host(platform, &parsed.host, auth_tokens);
    let web_url = format!("{}://{}/{}", parsed.scheme, parsed.host, project_path);

    Some(ReviewPlatformRemote {
        id: format!(
            "{}:{}:{}",
            remote_name,
            platform.as_str(),
            project_path.replace('/', "__")
        ),
        name: remote_name.to_string(),
        url: sanitize_remote_url(remote_url),
        platform,
        host: parsed.host,
        owner,
        repository_name,
        project_path,
        web_url,
        supported,
        auth_state,
        auth_source,
        message: if !supported {
            Some("This remote is detected, but no provider adapter is available yet.".to_string())
        } else if platform == ReviewPlatformKind::Gitcode
            && auth_state == ReviewAuthState::NotConnected
        {
            Some("Add a GitCode token to load pull requests.".to_string())
        } else {
            None
        },
    })
}

#[derive(Debug)]
struct ParsedRemoteUrl {
    scheme: String,
    host: String,
    path: String,
}

fn parse_remote_url(remote_url: &str) -> Option<ParsedRemoteUrl> {
    if let Some(scheme_end) = remote_url.find("://") {
        let scheme = &remote_url[..scheme_end];
        let rest = &remote_url[scheme_end + 3..];
        let slash = rest.find('/')?;
        let authority = &rest[..slash];
        let host_part = authority.rsplit('@').next().unwrap_or(authority);
        let host = host_part.split(':').next().unwrap_or(host_part);
        let path = rest[slash + 1..].trim_end_matches(".git").to_string();
        return Some(ParsedRemoteUrl {
            scheme: if scheme == "ssh" { "https" } else { scheme }.to_string(),
            host: host.to_string(),
            path,
        });
    }

    if let Some((user_host, path)) = remote_url.split_once(':') {
        if user_host.contains('@') && !path.contains('\\') {
            let host = user_host.rsplit('@').next()?.to_string();
            return Some(ParsedRemoteUrl {
                scheme: "https".to_string(),
                host,
                path: path.trim_end_matches(".git").to_string(),
            });
        }
    }

    None
}

fn sanitize_remote_url(remote_url: &str) -> String {
    if let Some(scheme_end) = remote_url.find("://") {
        let scheme = &remote_url[..scheme_end];
        let rest = &remote_url[scheme_end + 3..];
        if let Some(slash) = rest.find('/') {
            let authority = &rest[..slash];
            if authority.contains('@') {
                let host = authority.rsplit('@').next().unwrap_or(authority);
                return format!("{}://{}/{}", scheme, host, &rest[slash + 1..]);
            }
        }
    }
    remote_url.to_string()
}

fn github_pull_request_from_value(value: &Value) -> ReviewPlatformPullRequest {
    let number = value_i64(value, "number");
    let state = if value_bool(value, "draft") {
        ReviewItemState::Draft
    } else if !value_string(value, "merged_at").is_empty() {
        ReviewItemState::Merged
    } else {
        match value_string(value, "state").as_str() {
            "closed" => ReviewItemState::Closed,
            _ => ReviewItemState::Open,
        }
    };

    ReviewPlatformPullRequest {
        id: number.to_string(),
        number,
        title: value_string(value, "title"),
        state,
        author: nested_string(value, &["user", "login"]),
        source_branch: nested_string(value, &["head", "ref"]),
        target_branch: nested_string(value, &["base", "ref"]),
        base_revision: non_empty_option(nested_string(value, &["base", "sha"])),
        head_revision: non_empty_option(nested_string(value, &["head", "sha"])),
        updated_at: value_string(value, "updated_at"),
        web_url: value_string(value, "html_url"),
        additions: value_i64(value, "additions") as i32,
        deletions: value_i64(value, "deletions") as i32,
        changed_files: value_i64(value, "changed_files") as i32,
        comments: (value_i64(value, "comments") + value_i64(value, "review_comments")) as i32,
        review_decision: ReviewDecision::Pending,
        checks: empty_checks(),
    }
}

fn gitlab_pull_request_from_value(value: &Value) -> ReviewPlatformPullRequest {
    let number = value_i64(value, "iid");
    let state = if value_bool(value, "draft") || value_bool(value, "work_in_progress") {
        ReviewItemState::Draft
    } else {
        match value_string(value, "state").as_str() {
            "merged" => ReviewItemState::Merged,
            "closed" => ReviewItemState::Closed,
            _ => ReviewItemState::Open,
        }
    };
    let changed_files = value_string(value, "changes_count")
        .trim_end_matches('+')
        .parse::<i32>()
        .unwrap_or(0);

    ReviewPlatformPullRequest {
        id: number.to_string(),
        number,
        title: value_string(value, "title"),
        state,
        author: first_non_empty(&[
            nested_string(value, &["author", "username"]),
            nested_string(value, &["author", "name"]),
        ]),
        source_branch: value_string(value, "source_branch"),
        target_branch: value_string(value, "target_branch"),
        base_revision: non_empty_option(first_non_empty(&[
            nested_string(value, &["diff_refs", "base_sha"]),
            value_string(value, "base_sha"),
        ])),
        head_revision: non_empty_option(first_non_empty(&[
            nested_string(value, &["diff_refs", "head_sha"]),
            value_string(value, "sha"),
            value_string(value, "head_sha"),
        ])),
        updated_at: value_string(value, "updated_at"),
        web_url: value_string(value, "web_url"),
        additions: 0,
        deletions: 0,
        changed_files,
        comments: value_i64(value, "user_notes_count") as i32,
        review_decision: ReviewDecision::Pending,
        checks: empty_checks(),
    }
}

fn gitcode_pull_request_from_value(value: &Value) -> ReviewPlatformPullRequest {
    let number = first_non_zero(&[value_i64(value, "number"), value_i64(value, "id")]);
    let state = match value_string(value, "state").as_str() {
        "merged" => ReviewItemState::Merged,
        "closed" => ReviewItemState::Closed,
        _ => ReviewItemState::Open,
    };
    ReviewPlatformPullRequest {
        id: number.to_string(),
        number,
        title: value_string(value, "title"),
        state,
        author: first_non_empty(&[
            nested_string(value, &["user", "login"]),
            nested_string(value, &["user", "name"]),
            nested_string(value, &["author", "login"]),
        ]),
        source_branch: first_non_empty(&[
            nested_string(value, &["head", "ref"]),
            value_string(value, "head_branch"),
        ]),
        target_branch: first_non_empty(&[
            nested_string(value, &["base", "ref"]),
            value_string(value, "base_branch"),
        ]),
        base_revision: non_empty_option(first_non_empty(&[
            nested_string(value, &["base", "sha"]),
            value_string(value, "base_sha"),
        ])),
        head_revision: non_empty_option(first_non_empty(&[
            nested_string(value, &["head", "sha"]),
            value_string(value, "head_sha"),
            value_string(value, "sha"),
        ])),
        updated_at: value_string(value, "updated_at"),
        web_url: first_non_empty(&[
            value_string(value, "html_url"),
            value_string(value, "web_url"),
        ]),
        additions: value_i64(value, "additions") as i32,
        deletions: value_i64(value, "deletions") as i32,
        changed_files: value_i64(value, "changed_files") as i32,
        comments: value_i64(value, "comments") as i32,
        review_decision: ReviewDecision::Pending,
        checks: empty_checks(),
    }
}

fn github_file_from_value(value: &Value) -> ReviewPlatformFile {
    ReviewPlatformFile {
        path: value_string(value, "filename"),
        old_path: value
            .get("previous_filename")
            .and_then(Value::as_str)
            .map(str::to_string),
        status: file_status(&value_string(value, "status")),
        additions: value_i64(value, "additions") as i32,
        deletions: value_i64(value, "deletions") as i32,
        patch: optional_string(value, "patch"),
    }
}

fn gitcode_file_from_value(value: &Value) -> ReviewPlatformFile {
    ReviewPlatformFile {
        path: first_non_empty(&[
            value_string(value, "filename"),
            value_string(value, "new_path"),
        ]),
        old_path: value
            .get("previous_filename")
            .and_then(Value::as_str)
            .map(str::to_string),
        status: file_status(&value_string(value, "status")),
        additions: value_i64(value, "additions") as i32,
        deletions: value_i64(value, "deletions") as i32,
        patch: optional_string(value, "patch").or_else(|| optional_string(value, "diff")),
    }
}

fn gitlab_files(value: &Value) -> Vec<ReviewPlatformFile> {
    value
        .get("changes")
        .and_then(Value::as_array)
        .unwrap_or(&Vec::new())
        .iter()
        .map(gitlab_file_from_value)
        .collect()
}

fn gitlab_file_from_value(change: &Value) -> ReviewPlatformFile {
    let diff = value_string(change, "diff");
    let (additions, deletions) = count_diff_lines(&diff);
    let status = if value_bool(change, "new_file") {
        ReviewFileStatus::Added
    } else if value_bool(change, "deleted_file") {
        ReviewFileStatus::Deleted
    } else if value_bool(change, "renamed_file") {
        ReviewFileStatus::Renamed
    } else {
        ReviewFileStatus::Modified
    };
    let diff_available = !diff.trim().is_empty()
        && !value_bool(change, "collapsed")
        && !value_bool(change, "too_large");
    ReviewPlatformFile {
        path: value_string(change, "new_path"),
        old_path: change
            .get("old_path")
            .and_then(Value::as_str)
            .map(str::to_string),
        status,
        additions,
        deletions,
        patch: diff_available.then_some(diff),
    }
}

fn github_commit_from_value(value: &Value) -> ReviewPlatformCommit {
    let hash = value_string(value, "sha");
    ReviewPlatformCommit {
        short_hash: short_hash(&hash),
        hash,
        title: first_line(&nested_string(value, &["commit", "message"])),
        author: first_non_empty(&[
            nested_string(value, &["author", "login"]),
            nested_string(value, &["commit", "author", "name"]),
        ]),
        committed_at: nested_string(value, &["commit", "author", "date"]),
    }
}

fn gitlab_commit_from_value(value: &Value) -> ReviewPlatformCommit {
    let hash = value_string(value, "id");
    ReviewPlatformCommit {
        short_hash: first_non_empty(&[value_string(value, "short_id"), short_hash(&hash)]),
        hash,
        title: first_non_empty(&[
            value_string(value, "title"),
            first_line(&value_string(value, "message")),
        ]),
        author: value_string(value, "author_name"),
        committed_at: first_non_empty(&[
            value_string(value, "committed_date"),
            value_string(value, "created_at"),
        ]),
    }
}

fn gitcode_commit_from_value(value: &Value) -> ReviewPlatformCommit {
    let hash = first_non_empty(&[value_string(value, "sha"), value_string(value, "id")]);
    ReviewPlatformCommit {
        short_hash: short_hash(&hash),
        hash,
        title: first_non_empty(&[
            nested_string(value, &["commit", "message"])
                .lines()
                .next()
                .unwrap_or_default()
                .to_string(),
            value_string(value, "message"),
        ]),
        author: first_non_empty(&[
            nested_string(value, &["author", "login"]),
            nested_string(value, &["commit", "author", "name"]),
        ]),
        committed_at: first_non_empty(&[
            nested_string(value, &["commit", "author", "date"]),
            value_string(value, "created_at"),
        ]),
    }
}

fn github_review_decision(reviews: &Value) -> ReviewDecision {
    let mut latest_by_author: HashMap<String, String> = HashMap::new();
    let mut anonymous_states = Vec::new();
    for review in array_items(reviews) {
        let state = value_string(review, "state");
        if state == "DISMISSED" || state.trim().is_empty() {
            continue;
        }
        let author = nested_string(review, &["user", "login"]);
        if author.trim().is_empty() {
            anonymous_states.push(state);
        } else {
            latest_by_author.insert(author, state);
        }
    }

    let states = latest_by_author
        .values()
        .chain(anonymous_states.iter())
        .map(String::as_str)
        .collect::<Vec<_>>();

    if states.iter().any(|state| *state == "CHANGES_REQUESTED") {
        return ReviewDecision::ChangesRequested;
    }
    if states.iter().any(|state| *state == "APPROVED") {
        return ReviewDecision::Approved;
    }
    if states.iter().any(|state| *state == "COMMENTED") {
        return ReviewDecision::Commented;
    }
    ReviewDecision::Pending
}

fn github_threads(
    reviews: &Value,
    review_comments: &Value,
    issue_comments: &Value,
) -> Vec<ReviewPlatformThread> {
    let mut threads = Vec::new();
    for review in array_items(reviews) {
        let body = github_review_body(review);
        threads.push(ReviewPlatformThread {
            id: format!("review-{}", value_i64(review, "id")),
            provider_thread_id: None,
            provider_comment_id: value_i64(review, "id")
                .checked_abs()
                .map(|id| id.to_string()),
            kind: ReviewPlatformThreadKind::Review,
            reply_to_provider_comment_id: None,
            file_path: None,
            line: None,
            resolved: false,
            author: nested_string(review, &["user", "login"]),
            body,
            updated_at: first_non_empty(&[
                value_string(review, "submitted_at"),
                value_string(review, "updated_at"),
            ]),
        });
    }
    for comment in array_items(review_comments) {
        threads.push(github_thread_from_review_comment(comment));
    }
    for comment in array_items(issue_comments) {
        threads.push(github_thread_from_issue_comment(comment));
    }
    threads
}

fn github_review_body(review: &Value) -> String {
    let body = value_string(review, "body");
    if !body.trim().is_empty() {
        return body;
    }
    match value_string(review, "state").as_str() {
        "APPROVED" => "Approved this pull request.".to_string(),
        "CHANGES_REQUESTED" => "Requested changes.".to_string(),
        "COMMENTED" => "Submitted a pull request review.".to_string(),
        state if !state.trim().is_empty() => format!("Submitted a {} review.", state),
        _ => "Submitted a pull request review.".to_string(),
    }
}

fn github_thread_from_review_comment(comment: &Value) -> ReviewPlatformThread {
    let comment_id = first_non_empty(&[
        value_string(comment, "id"),
        value_i64(comment, "id").to_string(),
    ]);
    ReviewPlatformThread {
        id: format!("comment-{}", comment_id),
        provider_thread_id: None,
        provider_comment_id: Some(comment_id),
        kind: ReviewPlatformThreadKind::Comment,
        reply_to_provider_comment_id: value_i64(comment, "in_reply_to_id")
            .checked_abs()
            .map(|id| id.to_string())
            .or_else(|| {
                comment
                    .get("in_reply_to_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
        file_path: comment
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string),
        line: comment
            .get("line")
            .and_then(Value::as_i64)
            .or_else(|| comment.get("original_line").and_then(Value::as_i64)),
        resolved: false,
        author: nested_string(comment, &["user", "login"]),
        body: value_string(comment, "body"),
        updated_at: value_string(comment, "updated_at"),
    }
}

fn github_thread_from_issue_comment(comment: &Value) -> ReviewPlatformThread {
    let comment_id = first_non_empty(&[
        value_string(comment, "id"),
        value_i64(comment, "id").to_string(),
    ]);
    ReviewPlatformThread {
        id: format!("issue-comment-{}", comment_id),
        provider_thread_id: None,
        provider_comment_id: Some(comment_id),
        kind: ReviewPlatformThreadKind::Comment,
        reply_to_provider_comment_id: None,
        file_path: None,
        line: None,
        resolved: false,
        author: nested_string(comment, &["user", "login"]),
        body: value_string(comment, "body"),
        updated_at: value_string(comment, "updated_at"),
    }
}

fn gitlab_threads(discussions: &Value, notes: &Value) -> Vec<ReviewPlatformThread> {
    let mut threads = Vec::new();
    let mut seen_comment_ids = HashSet::new();
    for discussion in array_items(discussions) {
        let discussion_id = value_string(discussion, "id");
        let resolved = value_bool(discussion, "resolved");
        let discussion_notes = discussion
            .get("notes")
            .and_then(Value::as_array)
            .map(|notes| notes.as_slice())
            .unwrap_or(&[]);
        let mut root_comment_id: Option<String> = None;
        for (index, note) in discussion_notes.iter().enumerate() {
            let kind = if index == 0 {
                ReviewPlatformThreadKind::Review
            } else {
                ReviewPlatformThreadKind::Comment
            };
            let reply_to = if index == 0 {
                None
            } else {
                root_comment_id.clone()
            };
            let thread = gitlab_thread_from_note(
                note,
                Some(discussion_id.clone()),
                resolved,
                kind,
                reply_to,
            );
            if root_comment_id.is_none() {
                root_comment_id = thread.provider_comment_id.clone();
            }
            if let Some(comment_id) = thread.provider_comment_id.clone() {
                seen_comment_ids.insert(comment_id);
            }
            threads.push(thread);
        }
    }
    for note in array_items(notes) {
        let thread =
            gitlab_thread_from_note(note, None, false, ReviewPlatformThreadKind::Comment, None);
        if let Some(comment_id) = thread.provider_comment_id.as_ref() {
            if seen_comment_ids.contains(comment_id) {
                continue;
            }
            seen_comment_ids.insert(comment_id.clone());
        }
        threads.push(thread);
    }
    threads
}

fn gitlab_thread_from_note(
    note: &Value,
    discussion_id: Option<String>,
    discussion_resolved: bool,
    kind: ReviewPlatformThreadKind,
    reply_to_provider_comment_id: Option<String>,
) -> ReviewPlatformThread {
    let note_id = value_string(note, "id");
    let id = match discussion_id.as_deref() {
        Some(discussion_id) if !discussion_id.trim().is_empty() => {
            format!("discussion-{}:note-{}", discussion_id, note_id)
        }
        _ => format!("note-{}", note_id),
    };

    ReviewPlatformThread {
        id,
        provider_thread_id: discussion_id,
        provider_comment_id: Some(note_id),
        kind,
        reply_to_provider_comment_id,
        file_path: nested_optional_string(note, &["position", "new_path"])
            .or_else(|| nested_optional_string(note, &["position", "old_path"])),
        line: note
            .pointer("/position/new_line")
            .and_then(Value::as_i64)
            .or_else(|| note.pointer("/position/old_line").and_then(Value::as_i64)),
        resolved: discussion_resolved || value_bool(note, "resolved"),
        author: first_non_empty(&[
            nested_string(note, &["author", "username"]),
            nested_string(note, &["author", "name"]),
        ]),
        body: value_string(note, "body"),
        updated_at: first_non_empty(&[
            value_string(note, "updated_at"),
            value_string(note, "created_at"),
        ]),
    }
}

fn parse_provider_comment_id(thread_id: &str) -> Option<&str> {
    let trimmed = thread_id.trim();
    trimmed
        .strip_prefix("comment-")
        .or_else(|| trimmed.strip_prefix("note-"))
        .or_else(|| trimmed.split_once(":note-").map(|(_, note_id)| note_id))
        .or_else(|| {
            if trimmed.chars().all(|ch| ch.is_ascii_digit()) {
                Some(trimmed)
            } else {
                None
            }
        })
        .filter(|value| !value.trim().is_empty())
}

fn parse_provider_thread_id(thread_id: &str) -> Option<&str> {
    let trimmed = thread_id.trim();
    trimmed
        .strip_prefix("discussion-")
        .map(|value| {
            value
                .split_once(":note-")
                .map(|(id, _)| id)
                .unwrap_or(value)
        })
        .or_else(|| {
            if trimmed
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            {
                Some(trimmed)
            } else {
                None
            }
        })
        .filter(|value| !value.trim().is_empty())
}

fn gitcode_threads(value: &Value) -> Vec<ReviewPlatformThread> {
    array_items(value)
        .iter()
        .map(|comment| ReviewPlatformThread {
            id: value_string(comment, "id"),
            provider_thread_id: None,
            provider_comment_id: Some(value_string(comment, "id")),
            kind: ReviewPlatformThreadKind::Comment,
            reply_to_provider_comment_id: comment
                .get("in_reply_to_id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    comment
                        .get("in_reply_to_id")
                        .and_then(Value::as_i64)
                        .map(|id| id.to_string())
                }),
            file_path: comment
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_string),
            line: comment.get("line").and_then(Value::as_i64),
            resolved: false,
            author: first_non_empty(&[
                nested_string(comment, &["user", "login"]),
                nested_string(comment, &["user", "name"]),
            ]),
            body: value_string(comment, "body"),
            updated_at: first_non_empty(&[
                value_string(comment, "updated_at"),
                value_string(comment, "created_at"),
            ]),
        })
        .collect()
}

fn empty_checks() -> ReviewChecks {
    ReviewChecks {
        total: 0,
        passed: 0,
        failed: 0,
        pending: 0,
    }
}

fn file_status(status: &str) -> ReviewFileStatus {
    match status {
        "added" | "new" => ReviewFileStatus::Added,
        "removed" | "deleted" => ReviewFileStatus::Deleted,
        "renamed" => ReviewFileStatus::Renamed,
        _ => ReviewFileStatus::Modified,
    }
}

fn count_diff_lines(diff: &str) -> (i32, i32) {
    let mut additions = 0;
    let mut deletions = 0;
    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if line.starts_with('+') {
            additions += 1;
        } else if line.starts_with('-') {
            deletions += 1;
        }
    }
    (additions, deletions)
}

fn file_has_complete_patch(file: &ReviewPlatformFile) -> bool {
    let Some(patch) = file
        .patch
        .as_deref()
        .filter(|patch| !patch.trim().is_empty())
    else {
        return false;
    };
    if patch.chars().count() > MAX_REVIEW_FILE_DIFF_CHARS {
        return false;
    }
    let (patch_additions, patch_deletions) = count_diff_lines(patch);
    patch_additions == file.additions
        && patch_deletions == file.deletions
        && (patch_additions > 0 || patch_deletions > 0)
}

fn apply_files_stats(pull_request: &mut ReviewPlatformPullRequest, files: &[ReviewPlatformFile]) {
    if pull_request.changed_files <= 0 {
        pull_request.changed_files = files.len() as i32;
    }
    let (additions, deletions) = files.iter().fold((0, 0), |acc, file| {
        (acc.0 + file.additions, acc.1 + file.deletions)
    });
    pull_request.additions = additions;
    pull_request.deletions = deletions;
}

async fn fetch_bounded_paginated_array<F>(
    mut build_request: F,
    next_page: fn(&ReviewHttpHeaders, u32) -> Option<u32>,
    max_items: usize,
) -> Result<Value, ReviewPlatformError>
where
    F: FnMut(u32) -> ReviewHttpRequest,
{
    let mut page = 1;
    let mut values = Vec::new();
    while values.len() < max_items {
        let response = send_bounded_json_response(build_request(page)).await?;
        let items = response.value.as_array().ok_or_else(|| {
            ReviewPlatformError::Parse("Provider paginated response was not an array".to_string())
        })?;
        values.extend(items.iter().take(max_items - values.len()).cloned());
        if values.len() >= max_items {
            break;
        }
        let Some(next) = next_page(&response.headers, page).filter(|next| *next > page) else {
            break;
        };
        page = next;
    }
    Ok(Value::Array(values))
}

async fn fetch_bounded_paginated_file<F, M>(
    mut build_request: F,
    next_page: fn(&ReviewHttpHeaders, u32) -> Option<u32>,
    start_page: u32,
    max_items: usize,
    file_path: &str,
    map_file: M,
) -> Result<Option<ReviewPlatformFile>, ReviewPlatformError>
where
    F: FnMut(u32) -> ReviewHttpRequest,
    M: Fn(&Value) -> ReviewPlatformFile,
{
    let mut page = start_page.max(1);
    let mut visited = 0usize;
    while visited < max_items {
        let response = send_bounded_json_response(build_request(page)).await?;
        let items = response.value.as_array().ok_or_else(|| {
            ReviewPlatformError::Parse("Provider paginated response was not an array".to_string())
        })?;
        for item in items.iter().take(max_items - visited) {
            let file = map_file(item);
            if file.path == file_path || file.old_path.as_deref() == Some(file_path) {
                return Ok(Some(file));
            }
            visited += 1;
        }
        if visited >= max_items {
            break;
        }
        let Some(next) = next_page(&response.headers, page).filter(|next| *next > page) else {
            break;
        };
        page = next;
    }
    Ok(None)
}

fn review_target_from_parts(
    pull_request: ReviewPlatformPullRequest,
    files: Vec<ReviewPlatformFile>,
) -> ReviewPlatformPullRequestReviewTarget {
    let provider_file_count =
        usize::try_from(pull_request.changed_files.max(0)).unwrap_or_default();
    let known_file_count = provider_file_count.max(files.len());
    let kept_file_count = files.len().min(MAX_REVIEW_TARGET_FILES);
    let omitted_file_count = known_file_count.saturating_sub(kept_file_count);
    let mut limitations = Vec::new();
    if provider_file_count > files.len() {
        limitations.push("provider_file_list_incomplete".to_string());
    }
    if omitted_file_count > 0 {
        limitations.push("review_target_file_limit_exceeded".to_string());
    }

    let files = files
        .into_iter()
        .take(MAX_REVIEW_TARGET_FILES)
        .map(|file| {
            let diff_available = file_has_complete_patch(&file);
            ReviewPlatformReviewTargetFile {
                path: file.path,
                old_path: file.old_path,
                status: file.status,
                additions: file.additions,
                deletions: file.deletions,
                diff_available,
            }
        })
        .collect::<Vec<_>>();
    if files.iter().any(|file| !file.diff_available) {
        limitations.push("provider_file_diff_unavailable".to_string());
    }

    ReviewPlatformPullRequestReviewTarget {
        pull_request,
        files,
        omitted_file_count,
        limitations,
    }
}

fn file_diff_from_parts(
    pull_request: ReviewPlatformPullRequest,
    files: Vec<ReviewPlatformFile>,
    expected_base_revision: &str,
    expected_head_revision: &str,
    file_path: &str,
) -> Result<ReviewPlatformPullRequestFileDiff, ReviewPlatformError> {
    let base_revision = pull_request.base_revision.as_deref().unwrap_or_default();
    let head_revision = pull_request.head_revision.as_deref().unwrap_or_default();
    if base_revision != expected_base_revision || head_revision != expected_head_revision {
        return Err(ReviewPlatformError::StaleTarget(format!(
            "expected {expected_base_revision}..{expected_head_revision}, provider returned {base_revision}..{head_revision}"
        )));
    }
    let file = files
        .into_iter()
        .find(|file| file.path == file_path || file.old_path.as_deref() == Some(file_path))
        .ok_or_else(|| {
            ReviewPlatformError::Api(format!(
                "Pull request file is not available from the provider: {file_path}"
            ))
        })?;
    if file.path.contains(['\n', '\r'])
        || file
            .old_path
            .as_deref()
            .is_some_and(|path| path.contains(['\n', '\r']))
    {
        return Err(ReviewPlatformError::Parse(
            "Provider returned an invalid pull request file path".to_string(),
        ));
    }
    if !file_has_complete_patch(&file) {
        return Err(ReviewPlatformError::Api(format!(
            "Exact provider diff is unavailable for pull request file: {}",
            file.path
        )));
    }
    let patch = file.patch.as_deref().ok_or_else(|| {
        ReviewPlatformError::Api(format!(
            "Exact provider diff is unavailable for pull request file: {}",
            file.path
        ))
    })?;
    let old_path = file.old_path.as_deref().unwrap_or(&file.path);
    let diff = if patch.starts_with("diff --git ") {
        patch.to_string()
    } else {
        let old_marker = if file.status == ReviewFileStatus::Added {
            "/dev/null".to_string()
        } else {
            format!("a/{old_path}")
        };
        let new_marker = if file.status == ReviewFileStatus::Deleted {
            "/dev/null".to_string()
        } else {
            format!("b/{}", file.path)
        };
        format!(
            "diff --git a/{old_path} b/{}\n--- {old_marker}\n+++ {new_marker}\n{patch}",
            file.path
        )
    };

    Ok(ReviewPlatformPullRequestFileDiff {
        path: file.path,
        old_path: file.old_path,
        status: file.status,
        base_revision: base_revision.to_string(),
        head_revision: head_revision.to_string(),
        diff,
    })
}

fn array_items<'a>(value: &'a Value) -> &'a [Value] {
    value
        .as_array()
        .map(|items| items.as_slice())
        .unwrap_or(&[])
}

fn value_string(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        _ => String::new(),
    }
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
}

fn non_empty_option(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn nested_string(value: &Value, path: &[&str]) -> String {
    nested_optional_string(value, path).unwrap_or_default()
}

fn nested_optional_string(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    match current {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn value_i64(value: &Value, key: &str) -> i64 {
    value
        .get(key)
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str()?.parse::<i64>().ok())
        })
        .unwrap_or(0)
}

fn value_bool(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(|value| {
            value
                .as_bool()
                .or_else(|| value.as_str().map(|text| text.eq_ignore_ascii_case("true")))
        })
        .unwrap_or(false)
}

fn first_non_empty(values: &[String]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}

fn first_non_zero(values: &[i64]) -> i64 {
    values
        .iter()
        .copied()
        .find(|value| *value != 0)
        .unwrap_or(0)
}

fn first_line(value: &str) -> String {
    value.lines().next().unwrap_or_default().to_string()
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(7).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        path::PathBuf,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tokio::fs;

    struct AlwaysRemoteWorkspace;

    #[async_trait::async_trait]
    impl ReviewPlatformWorkspaceClassifier for AlwaysRemoteWorkspace {
        async fn is_remote_workspace_path(&self, _path: &str) -> bool {
            true
        }
    }

    fn temp_token_store_path(name: &str) -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "bitfun-review-platform-{name}-{}-{id}.json",
            std::process::id()
        ))
    }

    #[tokio::test]
    async fn token_store_uses_injected_path() {
        let path = temp_token_store_path("token-store");
        let service = ReviewPlatformService::new_local_only(path.clone());

        service
            .update_auth_token(ReviewPlatformKind::Github, "GitHub.com", " secret-token ")
            .await
            .expect("token update should write injected store");

        let tokens = service
            .load_stored_tokens()
            .await
            .expect("stored token file should be readable");
        assert_eq!(
            tokens.get(ReviewPlatformKind::Github, "github.com"),
            Some("secret-token")
        );
        assert!(path.exists());

        let _ = fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn workspace_snapshot_uses_injected_remote_classifier_before_git_probe() {
        let path = temp_token_store_path("remote-classifier");
        let service = ReviewPlatformService::new(path, Arc::new(AlwaysRemoteWorkspace));

        let snapshot = service
            .workspace_snapshot("not-a-git-repository", None, None, None)
            .await
            .expect("remote workspace should return unsupported snapshot");

        assert_eq!(
            snapshot.message.as_deref(),
            Some("Pull request browsing is not available for remote SSH workspaces yet.")
        );
        assert!(snapshot.remotes.is_empty());
        assert!(snapshot.pull_requests.is_empty());
    }

    #[tokio::test]
    async fn repository_root_accepts_nested_and_file_paths() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-review-platform-git-root-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ));
        let nested = root.join("nested").join("dir");
        let file = nested.join("tracked.txt");
        fs::create_dir_all(&nested)
            .await
            .expect("temporary nested directory should be created");
        fs::write(&file, "content")
            .await
            .expect("temporary file should be created");

        execute_git_command(
            root.to_str()
                .expect("temporary repository path should be valid UTF-8"),
            &["init"],
        )
        .await
        .expect("git init should succeed for repository root test");

        let expected = normalize_repository_root(
            root.to_str()
                .expect("temporary repository path should be valid UTF-8"),
        );
        let nested_root = get_repository_root(
            nested
                .to_str()
                .expect("nested repository path should be valid UTF-8"),
        )
        .await
        .expect("nested directory should resolve repository root");
        let file_root = get_repository_root(
            file.to_str()
                .expect("file repository path should be valid UTF-8"),
        )
        .await
        .expect("file path should resolve repository root");

        assert_eq!(nested_root, expected);
        assert_eq!(file_root, expected);

        let _ = fs::remove_dir_all(root).await;
    }

    #[test]
    fn github_review_decision_uses_latest_review_per_author() {
        let reviews = json!([
            {
                "id": 1,
                "state": "CHANGES_REQUESTED",
                "user": { "login": "alice" }
            },
            {
                "id": 2,
                "state": "APPROVED",
                "user": { "login": "alice" }
            }
        ]);

        assert_eq!(github_review_decision(&reviews), ReviewDecision::Approved);
    }

    #[test]
    fn github_pull_request_keeps_immutable_revisions() {
        let pull_request = github_pull_request_from_value(&json!({
            "number": 42,
            "title": "Review target",
            "state": "open",
            "user": { "login": "alice" },
            "head": { "ref": "feature", "sha": "2222222222222222222222222222222222222222" },
            "base": { "ref": "main", "sha": "1111111111111111111111111111111111111111" },
            "updated_at": "2026-07-11T00:00:00Z",
            "html_url": "https://github.com/example/repo/pull/42",
            "changed_files": 1
        }));

        assert_eq!(
            pull_request.base_revision.as_deref(),
            Some("1111111111111111111111111111111111111111")
        );
        assert_eq!(
            pull_request.head_revision.as_deref(),
            Some("2222222222222222222222222222222222222222")
        );
    }

    #[test]
    fn review_target_reports_unavailable_provider_diffs_without_embedding_them() {
        let mut pull_request = github_pull_request_from_value(&json!({
            "number": 42,
            "title": "Review target",
            "state": "open",
            "head": { "ref": "feature", "sha": "2222222222222222222222222222222222222222" },
            "base": { "ref": "main", "sha": "1111111111111111111111111111111111111111" },
            "changed_files": 2
        }));
        pull_request.changed_files = 2;
        let target = review_target_from_parts(
            pull_request,
            vec![
                ReviewPlatformFile {
                    path: "src/lib.rs".to_string(),
                    old_path: None,
                    status: ReviewFileStatus::Modified,
                    additions: 1,
                    deletions: 1,
                    patch: Some("@@ -1 +1 @@\n-old\n+new".to_string()),
                },
                ReviewPlatformFile {
                    path: "assets/image.png".to_string(),
                    old_path: None,
                    status: ReviewFileStatus::Modified,
                    additions: 0,
                    deletions: 0,
                    patch: None,
                },
            ],
        );

        assert_eq!(target.files.len(), 2);
        assert!(target.files[0].diff_available);
        assert!(!target.files[1].diff_available);
        assert!(target
            .limitations
            .contains(&"provider_file_diff_unavailable".to_string()));
    }

    #[test]
    fn review_target_rejects_a_non_empty_but_truncated_provider_patch() {
        let mut pull_request = github_pull_request_from_value(&json!({
            "number": 42,
            "title": "Review target",
            "state": "open",
            "head": { "ref": "feature", "sha": "2222222222222222222222222222222222222222" },
            "base": { "ref": "main", "sha": "1111111111111111111111111111111111111111" },
            "changed_files": 1
        }));
        pull_request.changed_files = 1;

        let target = review_target_from_parts(
            pull_request,
            vec![ReviewPlatformFile {
                path: "src/lib.rs".to_string(),
                old_path: None,
                status: ReviewFileStatus::Modified,
                additions: 10,
                deletions: 10,
                patch: Some("@@ -1 +1 @@\n-old\n+new".to_string()),
            }],
        );

        assert!(!target.files[0].diff_available);
        assert!(target
            .limitations
            .contains(&"provider_file_diff_unavailable".to_string()));
    }

    #[test]
    fn pull_request_file_diff_rejects_changed_head() {
        let pull_request = github_pull_request_from_value(&json!({
            "number": 42,
            "title": "Review target",
            "state": "open",
            "head": { "ref": "feature", "sha": "2222222222222222222222222222222222222222" },
            "base": { "ref": "main", "sha": "1111111111111111111111111111111111111111" }
        }));
        let result = file_diff_from_parts(
            pull_request,
            Vec::new(),
            "1111111111111111111111111111111111111111",
            "3333333333333333333333333333333333333333",
            "src/lib.rs",
        );

        assert!(matches!(result, Err(ReviewPlatformError::StaleTarget(_))));
    }

    #[test]
    fn review_target_rejects_revisions_that_change_during_preparation() {
        let initial = github_pull_request_from_value(&json!({
            "number": 42,
            "title": "Review target",
            "state": "open",
            "head": { "sha": "2222222222222222222222222222222222222222" },
            "base": { "sha": "1111111111111111111111111111111111111111" }
        }));
        let confirmed = github_pull_request_from_value(&json!({
            "number": 42,
            "title": "Review target",
            "state": "open",
            "head": { "sha": "3333333333333333333333333333333333333333" },
            "base": { "sha": "1111111111111111111111111111111111111111" }
        }));

        assert!(matches!(
            ensure_pull_request_revisions_stable(&initial, &confirmed),
            Err(ReviewPlatformError::StaleTarget(_))
        ));
    }

    #[test]
    fn github_review_decision_keeps_active_change_request_from_any_reviewer() {
        let reviews = json!([
            {
                "id": 1,
                "state": "APPROVED",
                "user": { "login": "alice" }
            },
            {
                "id": 2,
                "state": "CHANGES_REQUESTED",
                "user": { "login": "bob" }
            }
        ]);

        assert_eq!(
            github_review_decision(&reviews),
            ReviewDecision::ChangesRequested
        );
    }

    #[test]
    fn github_threads_include_issue_comments_and_review_comments() {
        let reviews = json!([]);
        let review_comments = json!([
            {
                "id": 10,
                "path": "src/lib.rs",
                "line": 8,
                "user": { "login": "alice" },
                "body": "Inline comment",
                "updated_at": "2026-05-18T01:00:00Z"
            }
        ]);
        let issue_comments = json!([
            {
                "id": 20,
                "user": { "login": "bob" },
                "body": "Conversation comment",
                "updated_at": "2026-05-18T02:00:00Z"
            }
        ]);

        let threads = github_threads(&reviews, &review_comments, &issue_comments);

        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].id, "comment-10");
        assert_eq!(threads[0].file_path.as_deref(), Some("src/lib.rs"));
        assert_eq!(threads[1].id, "issue-comment-20");
        assert_eq!(threads[1].file_path, None);
        assert_eq!(threads[1].body, "Conversation comment");
    }

    #[test]
    fn github_threads_keep_empty_body_reviews_visible() {
        let reviews = json!([
            {
                "id": 30,
                "state": "APPROVED",
                "user": { "login": "alice" },
                "body": "",
                "submitted_at": "2026-05-18T03:00:00Z"
            }
        ]);

        let threads = github_threads(&reviews, &json!([]), &json!([]));

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, "review-30");
        assert_eq!(threads[0].body, "Approved this pull request.");
    }

    #[test]
    fn github_review_comment_replies_track_parent_comment() {
        let threads = github_threads(
            &json!([]),
            &json!([
                {
                    "id": 40,
                    "in_reply_to_id": 10,
                    "user": { "login": "alice" },
                    "body": "Reply",
                    "updated_at": "2026-05-18T04:30:00Z"
                }
            ]),
            &json!([]),
        );

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].kind, ReviewPlatformThreadKind::Comment);
        assert_eq!(
            threads[0].reply_to_provider_comment_id.as_deref(),
            Some("10")
        );
    }

    #[test]
    fn gitlab_threads_include_top_level_notes_without_duplication() {
        let discussions = json!([
            {
                "id": "discussion-1",
                "resolved": false,
                "notes": [
                    {
                        "id": "100",
                        "author": { "username": "alice" },
                        "body": "Inline note",
                        "updated_at": "2026-05-18T04:00:00Z",
                        "position": { "new_path": "src/lib.rs", "new_line": 12 }
                    }
                ]
            }
        ]);
        let notes = json!([
            {
                "id": "100",
                "author": { "username": "alice" },
                "body": "Inline note",
                "updated_at": "2026-05-18T04:00:00Z",
                "position": { "new_path": "src/lib.rs", "new_line": 12 }
            },
            {
                "id": "200",
                "author": { "username": "bob" },
                "body": "Top-level note",
                "updated_at": "2026-05-18T05:00:00Z"
            }
        ]);

        let threads = gitlab_threads(&discussions, &notes);

        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].id, "discussion-discussion-1:note-100");
        assert_eq!(threads[1].id, "note-200");
        assert_eq!(threads[1].file_path, None);
        assert_eq!(threads[1].body, "Top-level note");
    }

    #[test]
    fn gitlab_discussion_threads_mark_root_as_review_and_replies_as_comments() {
        let discussions = json!([
            {
                "id": "discussion-2",
                "resolved": false,
                "notes": [
                    {
                        "id": "300",
                        "author": { "username": "alice" },
                        "body": "Root note",
                        "updated_at": "2026-05-18T06:00:00Z"
                    },
                    {
                        "id": "301",
                        "author": { "username": "bob" },
                        "body": "Reply note",
                        "updated_at": "2026-05-18T06:05:00Z"
                    }
                ]
            }
        ]);

        let threads = gitlab_threads(&discussions, &json!([]));

        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].kind, ReviewPlatformThreadKind::Review);
        assert_eq!(threads[0].reply_to_provider_comment_id, None);
        assert_eq!(threads[1].kind, ReviewPlatformThreadKind::Comment);
        assert_eq!(
            threads[1].reply_to_provider_comment_id.as_deref(),
            Some("300")
        );
    }

    #[test]
    fn summarize_ci_items_counts_provider_outcomes() {
        let items = vec![
            ReviewPlatformCiItem {
                id: "build".to_string(),
                name: "Build".to_string(),
                status: "completed".to_string(),
                conclusion: Some("success".to_string()),
                detail: None,
                stage: Some("build".to_string()),
                web_url: None,
                log: None,
                log_truncated: false,
                started_at: None,
                finished_at: None,
            },
            ReviewPlatformCiItem {
                id: "test".to_string(),
                name: "Test".to_string(),
                status: "failed".to_string(),
                conclusion: None,
                detail: None,
                stage: Some("test".to_string()),
                web_url: None,
                log: None,
                log_truncated: false,
                started_at: None,
                finished_at: None,
            },
            ReviewPlatformCiItem {
                id: "deploy".to_string(),
                name: "Deploy".to_string(),
                status: "running".to_string(),
                conclusion: None,
                detail: None,
                stage: Some("deploy".to_string()),
                web_url: None,
                log: None,
                log_truncated: false,
                started_at: None,
                finished_at: None,
            },
        ];

        let checks = summarize_ci_items(&items);

        assert_eq!(checks.total, 3);
        assert_eq!(checks.passed, 1);
        assert_eq!(checks.failed, 1);
        assert_eq!(checks.pending, 1);
    }

    #[test]
    fn ci_log_value_extracts_error_excerpt_only() {
        let text = [
            "running setup",
            "downloading dependencies",
            "cargo test failed with exit code 101",
            "thread 'main' panicked at src/lib.rs:4",
            "uploading artifacts",
        ]
        .join("\n");

        let (log, truncated) = ci_log_value(text);

        let log = log.expect("log should be present");
        assert!(!truncated);
        assert!(log.contains("cargo test failed"));
        assert!(log.contains("panicked at src/lib.rs"));
    }

    #[test]
    fn ci_log_value_reports_when_no_error_lines_match() {
        let (log, truncated) = ci_log_value("all checks passed".to_string());

        assert!(!truncated);
        assert!(log.is_none());
    }
}
