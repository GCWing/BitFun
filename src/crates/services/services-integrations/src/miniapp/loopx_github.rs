use super::loopx_cli::LoopxIntakeMetadataProvider;
use async_trait::async_trait;
use bitfun_product_domains::miniapp::loopx as loopx_contract;
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const GITHUB_API_ROOT: &str = "https://api.github.com";
const INTAKE_PAGE_SIZE: usize = 50;

#[derive(Clone)]
pub struct GithubLoopxIntakeMetadataProvider {
    client: Client,
}

impl GithubLoopxIntakeMetadataProvider {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .user_agent("BitFun LoopX MiniApp")
            .build()
            .map_err(|error| format!("Failed to build GitHub client: {error}"))?;
        Ok(Self { client })
    }

    async fn get_json(
        &self,
        path: &str,
        deadline: Duration,
        operation_id: &str,
    ) -> loopx_contract::LoopxCliResult<Value> {
        let response = self
            .client
            .get(format!("{GITHUB_API_ROOT}{path}"))
            .timeout(deadline)
            .send()
            .await
            .map_err(|error| {
                loopx_contract::LoopxCliError::new(
                    if error.is_timeout() {
                        loopx_contract::LoopxCliErrorKind::Timeout
                    } else {
                        loopx_contract::LoopxCliErrorKind::Backend
                    },
                    format!("GitHub intake request failed: {error}"),
                )
                .for_operation(operation_id)
                .retryable(true)
            })?;
        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            return Err(loopx_contract::LoopxCliError::new(
                loopx_contract::LoopxCliErrorKind::NotFound,
                "GitHub repository, issue, or pull request was not found",
            )
            .for_operation(operation_id));
        }
        if !status.is_success() {
            let message = if status == StatusCode::FORBIDDEN {
                if response
                    .headers()
                    .get("x-ratelimit-remaining")
                    .and_then(|value| value.to_str().ok())
                    == Some("0")
                {
                    "GitHub API rate limit exceeded; authenticate with GitHub to raise the limit"
                        .to_string()
                } else {
                    "GitHub intake request was forbidden (403); the repository may be private or access is blocked"
                        .to_string()
                }
            } else {
                format!("GitHub intake request returned HTTP {status}")
            };
            return Err(loopx_contract::LoopxCliError::new(
                loopx_contract::LoopxCliErrorKind::Backend,
                message,
            )
            .for_operation(operation_id)
            .retryable(status == StatusCode::FORBIDDEN || status.is_server_error()));
        }
        response.json::<Value>().await.map_err(|error| {
            loopx_contract::LoopxCliError::new(
                loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                format!("GitHub intake response was invalid: {error}"),
            )
            .for_operation(operation_id)
        })
    }
}

#[async_trait]
impl LoopxIntakeMetadataProvider for GithubLoopxIntakeMetadataProvider {
    async fn resolve(
        &self,
        request: &loopx_contract::LoopxCliResolveIntakeRequest,
        deadline: Duration,
    ) -> loopx_contract::LoopxCliResult<loopx_contract::LoopxCliResolveIntakeResult> {
        let repository = request.target.repository().clone();
        let operation_id = &request.call.operation_id;
        let mut candidates = Vec::new();
        let mut truncated = false;
        match &request.target {
            loopx_contract::LoopxIntakeTarget::Item { item } => {
                let collection = match item.kind {
                    loopx_contract::LoopxItemKind::Issue => "issues",
                    loopx_contract::LoopxItemKind::PullRequest => "pulls",
                };
                let value = self
                    .get_json(
                        &format!(
                            "/repos/{}/{}/{collection}/{}",
                            repository.owner, repository.repository, item.number
                        ),
                        deadline,
                        operation_id,
                    )
                    .await?;
                candidates.push(candidate_from_value(item.clone(), &value, false));
            }
            loopx_contract::LoopxIntakeTarget::Repository { .. } => {
                let value = self
                    .get_json(
                        &format!(
                            "/repos/{}/{}/issues?state=open&sort=updated&direction=desc&per_page={INTAKE_PAGE_SIZE}",
                            repository.owner, repository.repository
                        ),
                        deadline,
                        operation_id,
                    )
                    .await?;
                let rows = value.as_array().ok_or_else(|| {
                    loopx_contract::LoopxCliError::new(
                        loopx_contract::LoopxCliErrorKind::SchemaMismatch,
                        "GitHub issues response was not an array",
                    )
                    .for_operation(operation_id)
                })?;
                truncated = rows.len() == INTAKE_PAGE_SIZE;
                for value in rows {
                    if value.get("pull_request").is_some() {
                        continue;
                    }
                    let Some(number) = value.get("number").and_then(Value::as_u64) else {
                        continue;
                    };
                    candidates.push(candidate_from_value(
                        loopx_contract::LoopxIssueKey {
                            repository: repository.clone(),
                            kind: loopx_contract::LoopxItemKind::Issue,
                            number,
                        },
                        value,
                        true,
                    ));
                }
            }
        }
        Ok(loopx_contract::LoopxCliResolveIntakeResult {
            target: request.target.clone(),
            repository,
            candidates,
            truncated,
            resolved_at: now_ms(),
        })
    }

    async fn probe_auth(
        &self,
        deadline: Duration,
    ) -> loopx_contract::LoopxCliResult<loopx_contract::LoopxGithubAuthProbe> {
        const OPERATION_ID: &str = "github-auth-probe";
        let response = self
            .client
            .get(format!("{GITHUB_API_ROOT}/rate_limit"))
            .timeout(deadline)
            .send()
            .await
            .map_err(|error| {
                loopx_contract::LoopxCliError::new(
                    if error.is_timeout() {
                        loopx_contract::LoopxCliErrorKind::Timeout
                    } else {
                        loopx_contract::LoopxCliErrorKind::Backend
                    },
                    format!("GitHub auth probe failed: {error}"),
                )
                .for_operation(OPERATION_ID)
                .retryable(true)
            })?;
        let status = response.status();
        if !status.is_success() {
            let detail = match status {
                StatusCode::FORBIDDEN => {
                    "GitHub API returned 403; the request may be rate-limited or blocked"
                }
                StatusCode::UNAUTHORIZED => "GitHub API requires authentication for this request",
                _ => "GitHub API is unreachable for LoopX intake",
            };
            return Ok(loopx_contract::LoopxGithubAuthProbe {
                authenticated: false,
                detail: Some(detail.to_string()),
                ..loopx_contract::LoopxGithubAuthProbe::default()
            });
        }

        let parsed = response.json::<Value>().await.ok();
        let limit = parsed
            .as_ref()
            .and_then(|value| value.pointer("/rate/limit"))
            .and_then(Value::as_u64);
        let remaining = parsed
            .as_ref()
            .and_then(|value| value.pointer("/rate/remaining"))
            .and_then(Value::as_u64);
        let authenticated = limit.is_some_and(|value| value >= 1000);
        let detail = if remaining == Some(0) {
            Some(
                "GitHub API rate limit is exhausted; authenticate with GitHub to raise it"
                    .to_string(),
            )
        } else if authenticated {
            Some("Authenticated GitHub access".to_string())
        } else {
            Some("Unauthenticated GitHub access (60 requests/hour)".to_string())
        };
        Ok(loopx_contract::LoopxGithubAuthProbe {
            authenticated,
            rate_limit_remaining: remaining,
            detail,
        })
    }
}

fn candidate_from_value(
    key: loopx_contract::LoopxIssueKey,
    value: &Value,
    from_repository: bool,
) -> loopx_contract::LoopxIntakeCandidate {
    let merged = value.get("merged_at").is_some_and(|value| !value.is_null());
    let state = if merged {
        loopx_contract::LoopxRemoteItemState::Merged
    } else {
        match value.get("state").and_then(Value::as_str) {
            Some("open") => loopx_contract::LoopxRemoteItemState::Open,
            Some("closed") => loopx_contract::LoopxRemoteItemState::Closed,
            _ => loopx_contract::LoopxRemoteItemState::Unknown,
        }
    };
    let body = value
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or_default();
    loopx_contract::LoopxIntakeCandidate {
        url: key.canonical_url(),
        title: value
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        state,
        state_reason: value
            .get("state_reason")
            .and_then(Value::as_str)
            .map(str::to_string),
        from_repository,
        has_images: body.contains("![") || body.to_ascii_lowercase().contains("<img"),
        default_selected: !from_repository,
        key,
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_projection_does_not_retain_issue_body() {
        let key = loopx_contract::LoopxIssueKey {
            repository: loopx_contract::LoopxRepositoryKey {
                host: "github.com".to_string(),
                owner: "owner".to_string(),
                repository: "repo".to_string(),
            },
            kind: loopx_contract::LoopxItemKind::Issue,
            number: 7,
        };
        let value = serde_json::json!({
            "number": 7,
            "title": "Visible title",
            "state": "open",
            "body": "private-looking body ![image](https://example.test/image.png)"
        });
        let candidate = candidate_from_value(key, &value, false);
        assert_eq!(candidate.title, "Visible title");
        assert!(candidate.has_images);
        assert!(!serde_json::to_string(&candidate)
            .expect("serialize")
            .contains("private-looking"));
    }
}
