//! Pull request / review platform tool.
//!
//! This tool exposes hosted review-platform operations to the agent while
//! keeping provider-specific product semantics inside `ReviewPlatformService`.

use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::service::review_platform::{
    ReviewPlatformApprovalRequest, ReviewPlatformCreatePullRequestRequest,
    ReviewPlatformDetailSection, ReviewPlatformError, ReviewPlatformKind, ReviewPlatformRemote,
    ReviewPlatformReplyToThreadRequest, ReviewPlatformRequestChangesRequest,
    ReviewPlatformResolveThreadRequest, ReviewPlatformService, ReviewPlatformSubmitReviewRequest,
    ReviewSubmitEvent,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

const ACTION_WORKSPACE_SNAPSHOT: &str = "get_workspace_snapshot";
const ACTION_LIST_REMOTES: &str = "list_remotes";
const ACTION_LIST: &str = "list_pull_requests";
const ACTION_COUNT: &str = "count_pull_requests";
const ACTION_GET: &str = "get_pull_request";
const ACTION_GET_DETAIL_PAGE: &str = "get_pull_request_detail_page";
const ACTION_GET_CI_LOG: &str = "get_pull_request_ci_log";
const ACTION_CREATE: &str = "create_pull_request";
const ACTION_REPLY: &str = "reply_to_thread";
const ACTION_SUBMIT_REVIEW: &str = "submit_review";
const ACTION_APPROVE: &str = "approve_pull_request";
const ACTION_REVOKE_APPROVAL: &str = "revoke_approval";
const ACTION_REQUEST_CHANGES: &str = "request_changes";
const ACTION_RESOLVE: &str = "resolve_thread";
const ACTION_UPDATE_AUTH_TOKEN: &str = "update_auth_token";
const ACTION_CLEAR_AUTH_TOKEN: &str = "clear_auth_token";

const WRITE_ACTIONS: &[&str] = &[
    ACTION_CREATE,
    ACTION_REPLY,
    ACTION_SUBMIT_REVIEW,
    ACTION_APPROVE,
    ACTION_REVOKE_APPROVAL,
    ACTION_REQUEST_CHANGES,
    ACTION_RESOLVE,
    ACTION_UPDATE_AUTH_TOKEN,
    ACTION_CLEAR_AUTH_TOKEN,
];

pub struct ReviewPlatformTool;

impl ReviewPlatformTool {
    pub fn new() -> Self {
        Self
    }

    fn repository_path(input: &Value, context: &ToolUseContext) -> BitFunResult<String> {
        let requested = input
            .get("repository_path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());

        if let Some(path) = requested {
            return context.resolve_workspace_tool_path(path);
        }

        context
            .workspace
            .as_ref()
            .map(|workspace| workspace.root_path_string())
            .ok_or_else(|| BitFunError::tool("repository_path is required".to_string()))
    }

    fn string_field(input: &Value, key: &str) -> BitFunResult<String> {
        input
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| BitFunError::tool(format!("{} is required", key)))
    }

    fn optional_string_field(input: &Value, key: &str) -> Option<String> {
        input
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }

    fn submit_event(input: &Value) -> BitFunResult<ReviewSubmitEvent> {
        match input
            .get("event")
            .and_then(Value::as_str)
            .unwrap_or("comment")
        {
            "comment" => Ok(ReviewSubmitEvent::Comment),
            "approve" => Ok(ReviewSubmitEvent::Approve),
            "request_changes" => Ok(ReviewSubmitEvent::RequestChanges),
            other => Err(BitFunError::tool(format!(
                "Unsupported review event: {}",
                other
            ))),
        }
    }

    fn detail_section(input: &Value) -> BitFunResult<ReviewPlatformDetailSection> {
        match input
            .get("section")
            .and_then(Value::as_str)
            .unwrap_or("overview")
        {
            "overview" => Ok(ReviewPlatformDetailSection::Overview),
            "ci" => Ok(ReviewPlatformDetailSection::Ci),
            "files" => Ok(ReviewPlatformDetailSection::Files),
            "commits" => Ok(ReviewPlatformDetailSection::Commits),
            "reviews" => Ok(ReviewPlatformDetailSection::Reviews),
            other => Err(BitFunError::tool(format!(
                "Unsupported pull request detail section: {}",
                other
            ))),
        }
    }

    fn platform_kind(input: &Value) -> BitFunResult<ReviewPlatformKind> {
        match Self::string_field(input, "platform")?.as_str() {
            "github" => Ok(ReviewPlatformKind::Github),
            "gitlab" => Ok(ReviewPlatformKind::Gitlab),
            "gitcode" => Ok(ReviewPlatformKind::Gitcode),
            "unknown" => Ok(ReviewPlatformKind::Unknown),
            other => Err(BitFunError::tool(format!(
                "Unsupported review platform kind: {}",
                other
            ))),
        }
    }

    async fn resolve_remote_id(repository_path: &str, input: &Value) -> BitFunResult<String> {
        if let Some(remote_id) = Self::optional_string_field(input, "remote_id") {
            return Ok(remote_id);
        }

        let remotes = ReviewPlatformService::discover_remotes(repository_path)
            .await
            .map_err(|error| BitFunError::tool(error.to_string()))?;
        let supported = supported_remotes(&remotes);
        match supported.as_slice() {
            [] => Err(BitFunError::tool(
                "No supported review platform remote found".to_string(),
            )),
            [remote] => Ok(remote.id.clone()),
            _ => Err(BitFunError::tool(remote_ambiguity_message(&supported))),
        }
    }

    async fn resolve_remote_id_for_list(
        repository_path: &str,
        input: &Value,
    ) -> BitFunResult<Result<String, Value>> {
        if let Some(remote_id) = Self::optional_string_field(input, "remote_id") {
            return Ok(Ok(remote_id));
        }

        let remotes = ReviewPlatformService::discover_remotes(repository_path)
            .await
            .map_err(|error| BitFunError::tool(error.to_string()))?;
        let supported = supported_remotes(&remotes);
        match supported.as_slice() {
            [] => Err(BitFunError::tool(
                "No supported review platform remote found".to_string(),
            )),
            [remote] => Ok(Ok(remote.id.clone())),
            _ => Ok(Err(json!({
                "action": ACTION_LIST,
                "repositoryPath": repository_path,
                "status": "needs_remote_selection",
                "message": "Multiple supported review platform remotes were found. Provide remote_id explicitly.",
                "candidateRemotes": supported,
            }))),
        }
    }

    fn action(input: &Value) -> Option<&str> {
        input.get("action").and_then(Value::as_str)
    }

    fn auth_required_result(
        action: &str,
        repository_path: &str,
        remote_id: &str,
        error: &ReviewPlatformError,
    ) -> Option<Value> {
        let status = match error {
            ReviewPlatformError::Http { status, .. } if *status == 401 || *status == 403 => *status,
            _ => return None,
        };
        let state = if status == 403 {
            "insufficient_scope"
        } else {
            "invalid"
        };
        Some(json!({
            "action": action,
            "repositoryPath": repository_path,
            "remoteId": remote_id,
            "status": "needs_auth",
            "authChallenge": {
                "state": state,
                "message": if status == 403 {
                    "Review platform authentication is missing required permissions. Update authentication in the pull request panel, then retry."
                } else {
                    "Review platform authentication is required or was rejected. Configure authentication in the pull request panel, then retry."
                },
            },
            "openPanel": {
                "type": "review-platform-auth",
                "workspacePath": repository_path,
                "remoteId": remote_id,
            },
        }))
    }

    fn render_action_result(output: &Value) -> Option<String> {
        let result = output.get("result")?;
        let message = result
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Review platform action completed");
        let web_url = result.get("webUrl").and_then(Value::as_str);
        let pr = result.get("pullRequest");

        let mut lines = vec![message.to_string()];
        if let Some(pr) = pr {
            let title = pr
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Pull request");
            let number = pr.get("number").and_then(Value::as_i64).unwrap_or_default();
            let url = pr.get("webUrl").and_then(Value::as_str).or(web_url);
            if let Some(url) = url {
                lines.push(format!("[#{} {}]({})", number, title, url));
            }
        } else if let Some(url) = web_url {
            lines.push(url.to_string());
        }
        Some(lines.join("\n"))
    }
}

#[async_trait]
impl Tool for ReviewPlatformTool {
    fn name(&self) -> &str {
        "ReviewPlatform"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Read and operate on hosted pull requests / merge requests.

Use this for remote review-platform operations such as discovering remotes, loading the workspace PR snapshot, counting pull requests, listing pull requests, opening full or paginated pull request detail, loading CI logs, creating a pull request, replying to review threads, submitting a comment review, approving, revoking approval, requesting changes, or resolving a review thread. Use the Git tool for local repository state and branch/commit/push operations.

GitHub authentication is owned by the local `gh` CLI and must never use token actions. Authentication-token actions are only for GitLab and GitCode when the user explicitly provides a token or asks to clear a stored token. Never guess or expose token values.

When returning pull request results to the user, include the provider web URL so the chat UI can open the pull request detail panel naturally."#.to_string())
    }

    fn short_description(&self) -> String {
        "Inspect and operate on hosted pull requests / merge requests.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        ACTION_WORKSPACE_SNAPSHOT,
                        ACTION_LIST_REMOTES,
                        ACTION_LIST,
                        ACTION_COUNT,
                        ACTION_GET,
                        ACTION_GET_DETAIL_PAGE,
                        ACTION_GET_CI_LOG,
                        ACTION_CREATE,
                        ACTION_REPLY,
                        ACTION_SUBMIT_REVIEW,
                        ACTION_APPROVE,
                        ACTION_REVOKE_APPROVAL,
                        ACTION_REQUEST_CHANGES,
                        ACTION_RESOLVE,
                        ACTION_UPDATE_AUTH_TOKEN,
                        ACTION_CLEAR_AUTH_TOKEN
                    ],
                    "description": "Review platform action to perform."
                },
                "repository_path": {
                    "type": "string",
                    "description": "Repository path. Omit to use the current workspace."
                },
                "remote_id": {
                    "type": "string",
                    "description": "Review platform remote id. Omit to use the only supported remote; provide it explicitly when the repository has multiple supported review-platform remotes."
                },
                "pull_request_id": {
                    "type": "string",
                    "description": "Pull request or merge request number/id."
                },
                "page": {
                    "type": "integer",
                    "description": "Page number for list_pull_requests, get_workspace_snapshot, or get_pull_request_detail_page."
                },
                "per_page": {
                    "type": "integer",
                    "description": "Page size for list_pull_requests, get_workspace_snapshot, or get_pull_request_detail_page."
                },
                "section": {
                    "type": "string",
                    "enum": ["overview", "ci", "files", "commits", "reviews"],
                    "description": "Detail section for get_pull_request_detail_page."
                },
                "ci_item_id": {
                    "type": "string",
                    "description": "CI item id for get_pull_request_ci_log."
                },
                "ci_item_name": {
                    "type": "string",
                    "description": "CI item display name for get_pull_request_ci_log; used by providers that need a job name fallback."
                },
                "platform": {
                    "type": "string",
                    "enum": ["github", "gitlab", "gitcode", "unknown"],
                    "description": "GitLab or GitCode platform kind for update_auth_token or clear_auth_token. GitHub uses local gh authentication."
                },
                "host": {
                    "type": "string",
                    "description": "Review platform host for update_auth_token or clear_auth_token."
                },
                "token": {
                    "type": "string",
                    "description": "GitLab or GitCode personal access token for update_auth_token. Only provide this when the user explicitly asks to store that token. Never provide a GitHub token."
                },
                "title": {
                    "type": "string",
                    "description": "Pull request title for create_pull_request."
                },
                "source_branch": {
                    "type": "string",
                    "description": "Source/head branch for create_pull_request."
                },
                "target_branch": {
                    "type": "string",
                    "description": "Target/base branch for create_pull_request."
                },
                "body": {
                    "type": "string",
                    "description": "Pull request body, review body, or comment body depending on action."
                },
                "draft": {
                    "type": "boolean",
                    "description": "Create a draft pull request when the provider supports it."
                },
                "thread_id": {
                    "type": "string",
                    "description": "Thread id returned by get_pull_request for reply_to_thread or resolve_thread."
                },
                "event": {
                    "type": "string",
                    "enum": ["comment", "approve", "request_changes"],
                    "description": "Review event for submit_review."
                },
                "resolved": {
                    "type": "boolean",
                    "description": "Whether resolve_thread should mark the thread resolved or reopened."
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        input
            .and_then(Self::action)
            .is_some_and(|action| !WRITE_ACTIONS.contains(&action))
    }

    fn needs_permissions(&self, input: Option<&Value>) -> bool {
        input
            .and_then(Self::action)
            .map(|action| WRITE_ACTIONS.contains(&action))
            .unwrap_or(true)
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let Some(action) = Self::action(input) else {
            return ValidationResult {
                result: false,
                message: Some("action is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        };
        let valid = [
            ACTION_WORKSPACE_SNAPSHOT,
            ACTION_LIST_REMOTES,
            ACTION_LIST,
            ACTION_COUNT,
            ACTION_GET,
            ACTION_GET_DETAIL_PAGE,
            ACTION_GET_CI_LOG,
            ACTION_CREATE,
            ACTION_REPLY,
            ACTION_SUBMIT_REVIEW,
            ACTION_APPROVE,
            ACTION_REVOKE_APPROVAL,
            ACTION_REQUEST_CHANGES,
            ACTION_RESOLVE,
            ACTION_UPDATE_AUTH_TOKEN,
            ACTION_CLEAR_AUTH_TOKEN,
        ];
        if !valid.contains(&action) {
            return ValidationResult {
                result: false,
                message: Some(format!("Unsupported ReviewPlatform action: {}", action)),
                error_code: Some(400),
                meta: None,
            };
        }
        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let action = Self::action(input).unwrap_or("unknown");
        match action {
            ACTION_WORKSPACE_SNAPSHOT => "Load review platform workspace snapshot".to_string(),
            ACTION_LIST_REMOTES => "List review platform remotes".to_string(),
            ACTION_LIST => "List pull requests".to_string(),
            ACTION_COUNT => "Count pull requests".to_string(),
            ACTION_GET => format!(
                "Open pull request {}",
                input
                    .get("pull_request_id")
                    .and_then(Value::as_str)
                    .unwrap_or("detail")
            ),
            ACTION_GET_DETAIL_PAGE => "Load pull request detail page".to_string(),
            ACTION_GET_CI_LOG => "Load pull request CI log".to_string(),
            ACTION_CREATE => "Create pull request".to_string(),
            ACTION_REPLY => "Reply to pull request thread".to_string(),
            ACTION_SUBMIT_REVIEW => "Submit pull request review".to_string(),
            ACTION_APPROVE => "Approve pull request".to_string(),
            ACTION_REVOKE_APPROVAL => "Revoke pull request approval".to_string(),
            ACTION_REQUEST_CHANGES => "Request pull request changes".to_string(),
            ACTION_RESOLVE => "Resolve pull request thread".to_string(),
            ACTION_UPDATE_AUTH_TOKEN => "Update review platform auth token".to_string(),
            ACTION_CLEAR_AUTH_TOKEN => "Clear review platform auth token".to_string(),
            _ => format!("Review platform action: {}", action),
        }
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        let action = output.get("action").and_then(Value::as_str).unwrap_or("");
        if output
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| status == "needs_auth")
        {
            let message = output
                .pointer("/authChallenge/message")
                .and_then(Value::as_str)
                .unwrap_or("Review platform authentication is required.");
            return format!("{} Ask the user to configure provider authentication in the pull request panel, then retry this action.", message);
        }
        if let Some(action_result) = Self::render_action_result(output) {
            return action_result;
        }

        match action {
            ACTION_LIST_REMOTES => {
                let remotes = output
                    .get("remotes")
                    .and_then(Value::as_array)
                    .map(|items| items.as_slice())
                    .unwrap_or(&[]);
                let mut lines = vec![format!("Found {} review platform remotes.", remotes.len())];
                lines.extend(remotes.iter().map(|remote| {
                    let id = remote.get("id").and_then(Value::as_str).unwrap_or("");
                    let name = remote.get("name").and_then(Value::as_str).unwrap_or("");
                    let platform = remote.get("platform").and_then(Value::as_str).unwrap_or("");
                    let project = remote
                        .get("projectPath")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let url = remote.get("webUrl").and_then(Value::as_str).unwrap_or("");
                    format!(
                        "- remote_id: {} | name: {} | platform: {} | project: {} | url: {}",
                        id, name, platform, project, url
                    )
                }));
                lines.join("\n")
            }
            ACTION_WORKSPACE_SNAPSHOT => {
                let snapshot = output.get("snapshot");
                let Some(snapshot) = snapshot else {
                    return "Review platform workspace snapshot loaded.".to_string();
                };
                let remotes = snapshot
                    .get("remotes")
                    .and_then(Value::as_array)
                    .map(|items| items.len())
                    .unwrap_or(0);
                let prs = snapshot
                    .get("pullRequests")
                    .and_then(Value::as_array)
                    .map(|items| items.len())
                    .unwrap_or(0);
                let selected = snapshot
                    .get("selectedRemoteId")
                    .and_then(Value::as_str)
                    .unwrap_or("none");
                let message = snapshot.get("message").and_then(Value::as_str);
                match message {
                    Some(message) if !message.is_empty() => format!(
                        "Loaded review platform snapshot: selected remote {}, {} remotes, {} pull requests. {}",
                        selected, remotes, prs, message
                    ),
                    _ => format!(
                        "Loaded review platform snapshot: selected remote {}, {} remotes, {} pull requests.",
                        selected, remotes, prs
                    ),
                }
            }
            ACTION_COUNT => {
                if output
                    .get("status")
                    .and_then(Value::as_str)
                    .is_some_and(|status| status == "needs_remote_selection")
                {
                    let remotes = output
                        .get("candidateRemotes")
                        .and_then(Value::as_array)
                        .map(|items| items.as_slice())
                        .unwrap_or(&[]);
                    let mut lines = vec![
                        "Multiple review platform remotes were found. Ask the user which remote to use, then retry with remote_id.".to_string(),
                        "Candidate remotes:".to_string(),
                    ];
                    lines.extend(remotes.iter().map(|remote| {
                        let id = remote.get("id").and_then(Value::as_str).unwrap_or("");
                        let name = remote.get("name").and_then(Value::as_str).unwrap_or("");
                        let platform = remote.get("platform").and_then(Value::as_str).unwrap_or("");
                        let project = remote
                            .get("projectPath")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let url = remote.get("webUrl").and_then(Value::as_str).unwrap_or("");
                        format!(
                            "- remote_id: {} | name: {} | platform: {} | project: {} | url: {}",
                            id, name, platform, project, url
                        )
                    }));
                    return lines.join("\n");
                }

                let remote_id = output.get("remoteId").and_then(Value::as_str).unwrap_or("");
                let total = output.get("total").and_then(Value::as_u64);
                match total {
                    Some(total) => format!("Remote {} has {} pull requests.", remote_id, total),
                    None => format!(
                        "Remote {} did not return an exact pull request count.",
                        remote_id
                    ),
                }
            }
            ACTION_LIST => {
                if output
                    .get("status")
                    .and_then(Value::as_str)
                    .is_some_and(|status| status == "needs_remote_selection")
                {
                    let remotes = output
                        .get("candidateRemotes")
                        .and_then(Value::as_array)
                        .map(|items| items.as_slice())
                        .unwrap_or(&[]);
                    let mut lines = vec![
                        "Multiple review platform remotes were found. Ask the user which remote to use, then retry with remote_id.".to_string(),
                        "Candidate remotes:".to_string(),
                    ];
                    lines.extend(remotes.iter().map(|remote| {
                        let id = remote.get("id").and_then(Value::as_str).unwrap_or("");
                        let name = remote.get("name").and_then(Value::as_str).unwrap_or("");
                        let platform = remote.get("platform").and_then(Value::as_str).unwrap_or("");
                        let project = remote
                            .get("projectPath")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let url = remote.get("webUrl").and_then(Value::as_str).unwrap_or("");
                        format!(
                            "- remote_id: {} | name: {} | platform: {} | project: {} | url: {}",
                            id, name, platform, project, url
                        )
                    }));
                    return lines.join("\n");
                }

                let prs = output
                    .pointer("/snapshot/pullRequests")
                    .and_then(Value::as_array)
                    .map(|items| items.as_slice())
                    .unwrap_or(&[]);
                let pagination = output
                    .get("snapshot")
                    .and_then(|snapshot| snapshot.get("pagination"));
                let page = pagination
                    .and_then(|value| value.get("page"))
                    .and_then(Value::as_u64)
                    .unwrap_or(1);
                let per_page = pagination
                    .and_then(|value| value.get("perPage"))
                    .and_then(Value::as_u64)
                    .unwrap_or(prs.len() as u64);
                let total = pagination
                    .and_then(|value| value.get("total"))
                    .and_then(Value::as_u64);
                let has_next = pagination
                    .and_then(|value| value.get("hasNext"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let remote_id = output.get("remoteId").and_then(Value::as_str).unwrap_or("");

                let mut lines = vec![match total {
                    Some(total) => format!(
                        "Remote {} has {} pull requests. Showing {} from page {} (page size {}).",
                        remote_id,
                        total,
                        prs.len(),
                        page,
                        per_page
                    ),
                    None => format!(
                        "Remote {} returned {} pull requests on page {} (page size {}).{}",
                        remote_id,
                        prs.len(),
                        page,
                        per_page,
                        if has_next {
                            " More pages are available; this is not the total count."
                        } else {
                            ""
                        }
                    ),
                }];
                if prs.is_empty() {
                    return lines.join("\n");
                }
                lines.extend(prs.iter().take(10).map(|pr| {
                    let number = pr.get("number").and_then(Value::as_i64).unwrap_or_default();
                    let title = pr
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("Untitled");
                    let state = pr.get("state").and_then(Value::as_str).unwrap_or("unknown");
                    let url = pr.get("webUrl").and_then(Value::as_str).unwrap_or("");
                    if url.is_empty() {
                        format!("#{} {} ({})", number, title, state)
                    } else {
                        format!("[#{} {}]({}) ({})", number, title, url, state)
                    }
                }));
                lines.join("\n")
            }
            ACTION_GET => {
                let pr = output.get("pullRequest");
                let Some(pr) = pr else {
                    return "Pull request detail loaded.".to_string();
                };
                let number = pr.get("number").and_then(Value::as_i64).unwrap_or_default();
                let title = pr
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("Untitled");
                let url = pr.get("webUrl").and_then(Value::as_str).unwrap_or("");
                let files = output
                    .get("files")
                    .and_then(Value::as_array)
                    .map(|items| items.len())
                    .unwrap_or(0);
                let threads = output
                    .get("threads")
                    .and_then(Value::as_array)
                    .map(|items| items.len())
                    .unwrap_or(0);
                if url.is_empty() {
                    format!(
                        "Loaded PR #{} {} ({} files, {} threads)",
                        number, title, files, threads
                    )
                } else {
                    format!(
                        "Loaded [#{} {}]({}) ({} files, {} threads)",
                        number, title, url, files, threads
                    )
                }
            }
            ACTION_GET_DETAIL_PAGE => {
                let section = output
                    .get("section")
                    .and_then(Value::as_str)
                    .unwrap_or("detail");
                let items = output
                    .get("items")
                    .and_then(Value::as_array)
                    .map(|items| items.len())
                    .unwrap_or(0);
                let pagination = output.get("pagination");
                let page = pagination
                    .and_then(|value| value.get("page"))
                    .and_then(Value::as_u64)
                    .unwrap_or(1);
                let has_next = pagination
                    .and_then(|value| value.get("hasNext"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                format!(
                    "Loaded pull request {} page {} with {} items.{}",
                    section,
                    page,
                    items,
                    if has_next {
                        " More pages are available."
                    } else {
                        ""
                    }
                )
            }
            ACTION_GET_CI_LOG => {
                let ci_item_id = output.get("ciItemId").and_then(Value::as_str).unwrap_or("");
                let truncated = output
                    .get("truncated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let log_chars = output
                    .get("log")
                    .and_then(Value::as_str)
                    .map(str::len)
                    .unwrap_or(0);
                format!(
                    "Loaded CI log for {} ({} characters).{}",
                    ci_item_id,
                    log_chars,
                    if truncated {
                        " The log was truncated."
                    } else {
                        ""
                    }
                )
            }
            ACTION_UPDATE_AUTH_TOKEN => "Review platform auth token updated.".to_string(),
            ACTION_CLEAR_AUTH_TOKEN => "Review platform auth token cleared.".to_string(),
            _ => "Review platform action completed.".to_string(),
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let action = Self::string_field(input, "action")?;
        let repository_path = match action.as_str() {
            ACTION_UPDATE_AUTH_TOKEN | ACTION_CLEAR_AUTH_TOKEN => {
                Self::optional_string_field(input, "repository_path")
                    .map(|path| context.resolve_workspace_tool_path(&path))
                    .transpose()?
                    .or_else(|| {
                        context
                            .workspace
                            .as_ref()
                            .map(|workspace| workspace.root_path_string())
                    })
                    .unwrap_or_default()
            }
            _ => Self::repository_path(input, context)?,
        };

        let data = match action.as_str() {
            ACTION_LIST_REMOTES => {
                let remotes = ReviewPlatformService::discover_remotes(&repository_path)
                    .await
                    .map_err(|error| BitFunError::tool(error.to_string()))?;
                json!({
                    "action": action,
                    "repositoryPath": repository_path,
                    "remotes": remotes,
                })
            }
            ACTION_WORKSPACE_SNAPSHOT => {
                let page = input
                    .get("page")
                    .and_then(Value::as_u64)
                    .map(|value| value as u32);
                let per_page = input
                    .get("per_page")
                    .and_then(Value::as_u64)
                    .map(|value| value as u32);
                let remote_id = Self::optional_string_field(input, "remote_id");
                let snapshot = ReviewPlatformService::workspace_snapshot(
                    &repository_path,
                    remote_id.as_deref(),
                    page,
                    per_page,
                )
                .await
                .map_err(|error| BitFunError::tool(error.to_string()))?;
                let status = if snapshot.auth_challenge.is_some() {
                    "needs_auth"
                } else {
                    "ok"
                };
                let auth_challenge = snapshot.auth_challenge.clone();
                let selected_remote_id = snapshot.selected_remote_id.clone();
                let panel_remote_id = selected_remote_id.clone();
                json!({
                    "action": action,
                    "repositoryPath": repository_path,
                    "remoteId": selected_remote_id,
                    "status": status,
                    "authChallenge": auth_challenge,
                    "snapshot": snapshot,
                    "openPanel": if status == "needs_auth" {
                        json!({
                            "type": "review-platform-auth",
                            "workspacePath": repository_path,
                            "remoteId": panel_remote_id,
                        })
                    } else {
                        Value::Null
                    },
                })
            }
            ACTION_COUNT => {
                let remote_id =
                    match Self::resolve_remote_id_for_list(&repository_path, input).await? {
                        Ok(remote_id) => remote_id,
                        Err(mut selection_result) => {
                            if let Some(obj) = selection_result.as_object_mut() {
                                obj.insert("action".to_string(), json!(ACTION_COUNT));
                            }
                            let result_for_assistant =
                                self.render_result_for_assistant(&selection_result);
                            return Ok(vec![ToolResult::Result {
                                data: selection_result,
                                result_for_assistant: Some(result_for_assistant),
                                image_attachments: None,
                            }]);
                        }
                    };
                let snapshot = ReviewPlatformService::workspace_snapshot(
                    &repository_path,
                    Some(remote_id.as_str()),
                    Some(1),
                    Some(1),
                )
                .await
                .map_err(|error| BitFunError::tool(error.to_string()))?;
                if snapshot.auth_challenge.is_some() {
                    json!({
                        "action": action,
                        "repositoryPath": repository_path,
                        "remoteId": remote_id,
                        "status": "needs_auth",
                        "authChallenge": snapshot.auth_challenge,
                        "snapshot": snapshot,
                        "openPanel": {
                            "type": "review-platform-auth",
                            "workspacePath": repository_path,
                            "remoteId": remote_id,
                        },
                    })
                } else {
                    json!({
                        "action": action,
                        "repositoryPath": repository_path,
                        "remoteId": remote_id,
                        "total": snapshot.pagination.total,
                        "hasNext": snapshot.pagination.has_next,
                    })
                }
            }
            ACTION_LIST => {
                let page = input
                    .get("page")
                    .and_then(Value::as_u64)
                    .map(|value| value as u32);
                let per_page = input
                    .get("per_page")
                    .and_then(Value::as_u64)
                    .map(|value| value as u32);
                let remote_id =
                    match Self::resolve_remote_id_for_list(&repository_path, input).await? {
                        Ok(remote_id) => remote_id,
                        Err(selection_result) => {
                            let result_for_assistant =
                                self.render_result_for_assistant(&selection_result);
                            return Ok(vec![ToolResult::Result {
                                data: selection_result,
                                result_for_assistant: Some(result_for_assistant),
                                image_attachments: None,
                            }]);
                        }
                    };
                let snapshot = ReviewPlatformService::workspace_snapshot(
                    &repository_path,
                    Some(remote_id.as_str()),
                    page,
                    per_page,
                )
                .await
                .map_err(|error| BitFunError::tool(error.to_string()))?;
                if snapshot.auth_challenge.is_some() {
                    json!({
                        "action": action,
                        "repositoryPath": repository_path,
                        "remoteId": remote_id,
                        "status": "needs_auth",
                        "authChallenge": snapshot.auth_challenge,
                        "snapshot": snapshot,
                        "openPanel": {
                            "type": "review-platform-auth",
                            "workspacePath": repository_path,
                            "remoteId": remote_id,
                        },
                    })
                } else {
                    json!({
                        "action": action,
                        "repositoryPath": repository_path,
                        "remoteId": remote_id,
                        "snapshot": snapshot,
                    })
                }
            }
            ACTION_GET => {
                let pull_request_id = Self::string_field(input, "pull_request_id")?;
                let remote_id = Self::resolve_remote_id(&repository_path, input).await?;
                match ReviewPlatformService::pull_request_detail(
                    &repository_path,
                    &remote_id,
                    &pull_request_id,
                )
                .await
                {
                    Ok(detail) => json!({
                        "action": action,
                        "repositoryPath": repository_path,
                        "remoteId": remote_id,
                        "pullRequest": detail.pull_request,
                        "body": detail.body,
                        "ci": detail.ci,
                        "files": detail.files,
                        "commits": detail.commits,
                        "threads": detail.threads,
                    }),
                    Err(error) => {
                        if let Some(result) = Self::auth_required_result(
                            &action,
                            &repository_path,
                            &remote_id,
                            &error,
                        ) {
                            result
                        } else {
                            return Err(BitFunError::tool(error.to_string()));
                        }
                    }
                }
            }
            ACTION_GET_DETAIL_PAGE => {
                let pull_request_id = Self::string_field(input, "pull_request_id")?;
                let remote_id = Self::resolve_remote_id(&repository_path, input).await?;
                let section = Self::detail_section(input)?;
                let page = input
                    .get("page")
                    .and_then(Value::as_u64)
                    .map(|value| value as u32);
                let per_page = input
                    .get("per_page")
                    .and_then(Value::as_u64)
                    .map(|value| value as u32);
                match ReviewPlatformService::pull_request_detail_page(
                    &repository_path,
                    &remote_id,
                    &pull_request_id,
                    section,
                    page,
                    per_page,
                )
                .await
                {
                    Ok(detail) => {
                        let items = match section {
                            ReviewPlatformDetailSection::Overview => json!([]),
                            ReviewPlatformDetailSection::Ci => json!(detail.ci),
                            ReviewPlatformDetailSection::Files => json!(detail.files),
                            ReviewPlatformDetailSection::Commits => json!(detail.commits),
                            ReviewPlatformDetailSection::Reviews => json!(detail.threads),
                        };
                        json!({
                            "action": action,
                            "repositoryPath": repository_path,
                            "remoteId": remote_id,
                            "pullRequest": detail.pull_request,
                            "body": detail.body,
                            "section": detail.section,
                            "pagination": detail.pagination,
                            "items": items,
                            "detailPage": detail,
                        })
                    }
                    Err(error) => {
                        if let Some(result) = Self::auth_required_result(
                            &action,
                            &repository_path,
                            &remote_id,
                            &error,
                        ) {
                            result
                        } else {
                            return Err(BitFunError::tool(error.to_string()));
                        }
                    }
                }
            }
            ACTION_GET_CI_LOG => {
                let pull_request_id = Self::string_field(input, "pull_request_id")?;
                let remote_id = Self::resolve_remote_id(&repository_path, input).await?;
                let ci_item_id = Self::string_field(input, "ci_item_id")?;
                let ci_item_name = Self::string_field(input, "ci_item_name")?;
                match ReviewPlatformService::pull_request_ci_log(
                    &repository_path,
                    &remote_id,
                    &pull_request_id,
                    &ci_item_id,
                    &ci_item_name,
                )
                .await
                {
                    Ok(ci_log) => json!({
                        "action": action,
                        "repositoryPath": repository_path,
                        "remoteId": remote_id,
                        "pullRequestId": pull_request_id,
                        "ciItemId": ci_log.ci_item_id,
                        "log": ci_log.log,
                        "truncated": ci_log.truncated,
                        "message": ci_log.message,
                    }),
                    Err(error) => {
                        if let Some(result) = Self::auth_required_result(
                            &action,
                            &repository_path,
                            &remote_id,
                            &error,
                        ) {
                            result
                        } else {
                            return Err(BitFunError::tool(error.to_string()));
                        }
                    }
                }
            }
            ACTION_CREATE => {
                let remote_id = Self::resolve_remote_id(&repository_path, input).await?;
                let request = ReviewPlatformCreatePullRequestRequest {
                    repository_path: repository_path.clone(),
                    remote_id: Some(remote_id),
                    title: Self::string_field(input, "title")?,
                    source_branch: Self::string_field(input, "source_branch")?,
                    target_branch: Self::string_field(input, "target_branch")?,
                    body: Self::optional_string_field(input, "body"),
                    draft: input.get("draft").and_then(Value::as_bool),
                };
                let result = ReviewPlatformService::create_pull_request(request)
                    .await
                    .map_err(|error| BitFunError::tool(error.to_string()))?;
                json!({ "action": action, "result": result })
            }
            ACTION_REPLY => {
                let remote_id = Self::resolve_remote_id(&repository_path, input).await?;
                let request = ReviewPlatformReplyToThreadRequest {
                    repository_path: repository_path.clone(),
                    remote_id,
                    pull_request_id: Self::string_field(input, "pull_request_id")?,
                    thread_id: Self::string_field(input, "thread_id")?,
                    body: Self::string_field(input, "body")?,
                };
                let result = ReviewPlatformService::reply_to_thread(request)
                    .await
                    .map_err(|error| BitFunError::tool(error.to_string()))?;
                json!({ "action": action, "result": result })
            }
            ACTION_SUBMIT_REVIEW => {
                let remote_id = Self::resolve_remote_id(&repository_path, input).await?;
                let request = ReviewPlatformSubmitReviewRequest {
                    repository_path: repository_path.clone(),
                    remote_id,
                    pull_request_id: Self::string_field(input, "pull_request_id")?,
                    event: Self::submit_event(input)?,
                    body: Self::string_field(input, "body")?,
                };
                let result = ReviewPlatformService::submit_review(request)
                    .await
                    .map_err(|error| BitFunError::tool(error.to_string()))?;
                json!({ "action": action, "result": result })
            }
            ACTION_APPROVE => {
                let remote_id = Self::resolve_remote_id(&repository_path, input).await?;
                let request = ReviewPlatformApprovalRequest {
                    repository_path: repository_path.clone(),
                    remote_id,
                    pull_request_id: Self::string_field(input, "pull_request_id")?,
                    body: Self::optional_string_field(input, "body"),
                };
                let result = ReviewPlatformService::approve_pull_request(request)
                    .await
                    .map_err(|error| BitFunError::tool(error.to_string()))?;
                json!({ "action": action, "result": result })
            }
            ACTION_REVOKE_APPROVAL => {
                let remote_id = Self::resolve_remote_id(&repository_path, input).await?;
                let request = ReviewPlatformApprovalRequest {
                    repository_path: repository_path.clone(),
                    remote_id,
                    pull_request_id: Self::string_field(input, "pull_request_id")?,
                    body: None,
                };
                let result = ReviewPlatformService::revoke_approval(request)
                    .await
                    .map_err(|error| BitFunError::tool(error.to_string()))?;
                json!({ "action": action, "result": result })
            }
            ACTION_REQUEST_CHANGES => {
                let remote_id = Self::resolve_remote_id(&repository_path, input).await?;
                let request = ReviewPlatformRequestChangesRequest {
                    repository_path: repository_path.clone(),
                    remote_id,
                    pull_request_id: Self::string_field(input, "pull_request_id")?,
                    body: Self::string_field(input, "body")?,
                };
                let result = ReviewPlatformService::request_changes(request)
                    .await
                    .map_err(|error| BitFunError::tool(error.to_string()))?;
                json!({ "action": action, "result": result })
            }
            ACTION_RESOLVE => {
                let remote_id = Self::resolve_remote_id(&repository_path, input).await?;
                let request = ReviewPlatformResolveThreadRequest {
                    repository_path: repository_path.clone(),
                    remote_id,
                    pull_request_id: Self::string_field(input, "pull_request_id")?,
                    thread_id: Self::string_field(input, "thread_id")?,
                    resolved: input
                        .get("resolved")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                };
                let result = ReviewPlatformService::resolve_thread(request)
                    .await
                    .map_err(|error| BitFunError::tool(error.to_string()))?;
                json!({ "action": action, "result": result })
            }
            ACTION_UPDATE_AUTH_TOKEN => {
                let platform = Self::platform_kind(input)?;
                let host = Self::string_field(input, "host")?;
                let token = Self::string_field(input, "token")?;
                ReviewPlatformService::update_auth_token(platform, &host, &token)
                    .await
                    .map_err(|error| BitFunError::tool(error.to_string()))?;
                json!({
                    "action": action,
                    "repositoryPath": repository_path,
                    "platform": platform,
                    "host": host,
                    "status": "ok",
                })
            }
            ACTION_CLEAR_AUTH_TOKEN => {
                let platform = Self::platform_kind(input)?;
                let host = Self::string_field(input, "host")?;
                ReviewPlatformService::clear_auth_token(platform, &host)
                    .await
                    .map_err(|error| BitFunError::tool(error.to_string()))?;
                json!({
                    "action": action,
                    "repositoryPath": repository_path,
                    "platform": platform,
                    "host": host,
                    "status": "ok",
                })
            }
            _ => return Err(BitFunError::tool(format!("Unsupported action: {}", action))),
        };

        let result_for_assistant = self.render_result_for_assistant(&data);
        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}

impl Default for ReviewPlatformTool {
    fn default() -> Self {
        Self::new()
    }
}

fn supported_remotes(remotes: &[ReviewPlatformRemote]) -> Vec<&ReviewPlatformRemote> {
    remotes.iter().filter(|remote| remote.supported).collect()
}

fn remote_ambiguity_message(remotes: &[&ReviewPlatformRemote]) -> String {
    let mut lines = vec![
        "Multiple supported review platform remotes were found. Provide remote_id explicitly."
            .to_string(),
        "Candidate remotes:".to_string(),
    ];
    lines.extend(remotes.iter().map(|remote| {
        format!(
            "- remote_id: {} | name: {} | platform: {:?} | project: {} | url: {}",
            remote.id, remote.name, remote.platform, remote.project_path, remote.web_url
        )
    }));
    lines.join("\n")
}
