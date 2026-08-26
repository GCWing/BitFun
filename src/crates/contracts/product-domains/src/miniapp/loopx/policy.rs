//! Pure parsing and lifecycle decisions for LoopX tasks.

use super::types::{
    LoopxActionStatus, LoopxCoreEnvironmentFacts, LoopxEnvironmentFactStatus,
    LoopxEnvironmentStatus, LoopxEventsPageStatus, LoopxExistingTask, LoopxIntakeCandidate,
    LoopxIntakeTarget, LoopxIssueKey, LoopxItemKind, LoopxOptionalEnvironmentFacts,
    LoopxPermissionScope, LoopxRemoteItemState, LoopxRepositoryKey, LoopxTaskState,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopxIntakeParseErrorKind {
    Empty,
    UnsupportedHost,
    InvalidRepository,
    UnsupportedPath,
    InvalidItemNumber,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopxIntakeParseError {
    pub kind: LoopxIntakeParseErrorKind,
    pub message: String,
}

impl LoopxIntakeParseError {
    fn new(kind: LoopxIntakeParseErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for LoopxIntakeParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for LoopxIntakeParseError {}

fn valid_owner(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 39
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_repository(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

fn split_host_and_path(input: &str) -> Result<(String, String), LoopxIntakeParseError> {
    let without_suffix = input
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('/');

    if let Some(rest) = without_suffix.strip_prefix("git@") {
        let Some((host, path)) = rest.split_once(':') else {
            return Err(LoopxIntakeParseError::new(
                LoopxIntakeParseErrorKind::InvalidRepository,
                "Invalid GitHub SSH repository URL",
            ));
        };
        return Ok((host.to_ascii_lowercase(), path.to_string()));
    }

    let schemeless = without_suffix
        .strip_prefix("https://")
        .or_else(|| without_suffix.strip_prefix("http://"))
        .unwrap_or(without_suffix);

    if let Some((host, path)) = schemeless.split_once('/') {
        if host.eq_ignore_ascii_case("github.com") || host.eq_ignore_ascii_case("www.github.com") {
            return Ok(("github.com".to_string(), path.to_string()));
        }
        if host.contains('.') || input.contains("://") {
            return Err(LoopxIntakeParseError::new(
                LoopxIntakeParseErrorKind::UnsupportedHost,
                "Only github.com repositories are supported",
            ));
        }
    }

    if !input.contains("://") {
        return Ok(("github.com".to_string(), schemeless.to_string()));
    }

    Err(LoopxIntakeParseError::new(
        LoopxIntakeParseErrorKind::UnsupportedHost,
        "Only github.com repositories are supported",
    ))
}

/// Parse one GitHub repository, issues-list, issue, or pull-request target.
pub fn parse_loopx_intake(input: &str) -> Result<LoopxIntakeTarget, LoopxIntakeParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(LoopxIntakeParseError::new(
            LoopxIntakeParseErrorKind::Empty,
            "A GitHub repository, issue, or pull-request URL is required",
        ));
    }
    if input.chars().any(char::is_whitespace) {
        return Err(LoopxIntakeParseError::new(
            LoopxIntakeParseErrorKind::UnsupportedPath,
            "LoopX intake accepts one GitHub target at a time",
        ));
    }

    let (host, path) = split_host_and_path(input)?;
    if host != "github.com" {
        return Err(LoopxIntakeParseError::new(
            LoopxIntakeParseErrorKind::UnsupportedHost,
            "Only github.com repositories are supported",
        ));
    }

    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.len() < 2 {
        return Err(LoopxIntakeParseError::new(
            LoopxIntakeParseErrorKind::InvalidRepository,
            "GitHub target must include an owner and repository",
        ));
    }

    let owner = segments[0].to_ascii_lowercase();
    let repository = segments[1]
        .strip_suffix(".git")
        .unwrap_or(segments[1])
        .to_ascii_lowercase();
    if !valid_owner(&owner) || !valid_repository(&repository) {
        return Err(LoopxIntakeParseError::new(
            LoopxIntakeParseErrorKind::InvalidRepository,
            "Invalid GitHub owner or repository name",
        ));
    }
    let repository_key = LoopxRepositoryKey {
        host,
        owner,
        repository,
    };

    match segments.as_slice() {
        [_, _] | [_, _, "issues"] | [_, _, "pulls"] => Ok(LoopxIntakeTarget::Repository {
            repository: repository_key,
        }),
        [_, _, collection @ ("issues" | "pull"), number] => {
            let number = number.parse::<u64>().map_err(|_| {
                LoopxIntakeParseError::new(
                    LoopxIntakeParseErrorKind::InvalidItemNumber,
                    "GitHub issue or pull-request number is invalid",
                )
            })?;
            if number == 0 {
                return Err(LoopxIntakeParseError::new(
                    LoopxIntakeParseErrorKind::InvalidItemNumber,
                    "GitHub issue or pull-request number must be positive",
                ));
            }
            let kind = if *collection == "issues" {
                LoopxItemKind::Issue
            } else {
                LoopxItemKind::PullRequest
            };
            Ok(LoopxIntakeTarget::Item {
                item: LoopxIssueKey {
                    repository: repository_key,
                    kind,
                    number,
                },
            })
        }
        _ => Err(LoopxIntakeParseError::new(
            LoopxIntakeParseErrorKind::UnsupportedPath,
            "Paste a repository, issues-list, issue, or pull-request URL",
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum LoopxTransitionDecision {
    NoChange,
    Allowed { next: LoopxTaskState },
    Rejected,
}

pub fn decide_task_transition(
    current: LoopxTaskState,
    next: LoopxTaskState,
) -> LoopxTransitionDecision {
    if current == next {
        return LoopxTransitionDecision::NoChange;
    }

    let allowed = match current {
        LoopxTaskState::Preparing => matches!(
            next,
            LoopxTaskState::Queued
                | LoopxTaskState::Cancelling
                | LoopxTaskState::RecoveryRequired
                | LoopxTaskState::Failed
        ),
        LoopxTaskState::Queued => matches!(
            next,
            LoopxTaskState::Running
                | LoopxTaskState::Cancelling
                | LoopxTaskState::RecoveryRequired
                | LoopxTaskState::Failed
        ),
        LoopxTaskState::Running => matches!(
            next,
            LoopxTaskState::Queued
                | LoopxTaskState::WaitingForUser
                | LoopxTaskState::RetryWait
                | LoopxTaskState::Cancelling
                | LoopxTaskState::RecoveryRequired
                | LoopxTaskState::Completed
                | LoopxTaskState::Failed
        ),
        LoopxTaskState::WaitingForUser => matches!(
            next,
            LoopxTaskState::Queued
                | LoopxTaskState::Cancelling
                | LoopxTaskState::RecoveryRequired
                | LoopxTaskState::Failed
        ),
        LoopxTaskState::RetryWait => matches!(
            next,
            LoopxTaskState::Queued
                | LoopxTaskState::Cancelling
                | LoopxTaskState::RecoveryRequired
                | LoopxTaskState::Failed
        ),
        LoopxTaskState::Cancelling => matches!(
            next,
            LoopxTaskState::Stopped
                | LoopxTaskState::Aborted
                | LoopxTaskState::RecoveryRequired
                | LoopxTaskState::Failed
        ),
        LoopxTaskState::Stopped | LoopxTaskState::Failed => {
            matches!(next, LoopxTaskState::Queued | LoopxTaskState::Archived)
        }
        LoopxTaskState::Aborted => matches!(next, LoopxTaskState::Archived),
        LoopxTaskState::RecoveryRequired => matches!(
            next,
            LoopxTaskState::Queued | LoopxTaskState::Stopped | LoopxTaskState::Failed
        ),
        LoopxTaskState::Completed => matches!(next, LoopxTaskState::Archived),
        LoopxTaskState::Archived => matches!(next, LoopxTaskState::RecoveryRequired),
    };

    if allowed {
        LoopxTransitionDecision::Allowed { next }
    } else {
        LoopxTransitionDecision::Rejected
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum LoopxRestartDecision {
    Preserve { state: LoopxTaskState },
    RequireRecovery,
}

pub fn decide_task_restart(state: LoopxTaskState) -> LoopxRestartDecision {
    match state {
        LoopxTaskState::Preparing | LoopxTaskState::RetryWait => LoopxRestartDecision::Preserve {
            state: LoopxTaskState::Queued,
        },
        state if state.was_executing_at_shutdown() => LoopxRestartDecision::RequireRecovery,
        state => LoopxRestartDecision::Preserve { state },
    }
}

pub fn task_state_after_restart(state: LoopxTaskState) -> LoopxTaskState {
    match decide_task_restart(state) {
        LoopxRestartDecision::RequireRecovery => LoopxTaskState::RecoveryRequired,
        LoopxRestartDecision::Preserve { state } => state,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum LoopxDedupDecision {
    CreateAttempt {
        attempt: u32,
    },
    OpenExisting {
        task_id: String,
    },
    RequireExplicitRetry {
        previous_task_id: String,
        next_attempt: u32,
    },
    ClosedNoop,
    NeedsLiveVerification,
}

pub fn decide_task_dedup(
    key: &LoopxIssueKey,
    remote_state: LoopxRemoteItemState,
    existing: &[LoopxExistingTask],
    retry_terminal: bool,
) -> LoopxDedupDecision {
    let mut matching = existing
        .iter()
        .filter(|task| &task.identity.item == key)
        .collect::<Vec<_>>();
    matching.sort_by_key(|task| task.identity.attempt);

    if let Some(active) = matching.iter().rev().find(|task| !task.state.is_terminal()) {
        return LoopxDedupDecision::OpenExisting {
            task_id: active.task_id.clone(),
        };
    }

    if remote_state.is_resolved() {
        return LoopxDedupDecision::ClosedNoop;
    }
    if remote_state == LoopxRemoteItemState::Unknown {
        return LoopxDedupDecision::NeedsLiveVerification;
    }

    let Some(previous) = matching.last() else {
        return LoopxDedupDecision::CreateAttempt { attempt: 1 };
    };
    let next_attempt = previous.identity.attempt.saturating_add(1).max(1);
    if retry_terminal {
        LoopxDedupDecision::CreateAttempt {
            attempt: next_attempt,
        }
    } else {
        LoopxDedupDecision::RequireExplicitRetry {
            previous_task_id: previous.task_id.clone(),
            next_attempt,
        }
    }
}

pub fn intake_scope_is_pregrantable(scope: LoopxPermissionScope) -> bool {
    matches!(
        scope,
        LoopxPermissionScope::WorkspaceRead
            | LoopxPermissionScope::WorkspaceWrite
            | LoopxPermissionScope::GitLocal
            | LoopxPermissionScope::GithubRead
            | LoopxPermissionScope::AgentExecution
    )
}

pub fn derive_environment_status(
    core: &LoopxCoreEnvironmentFacts,
    optional: &LoopxOptionalEnvironmentFacts,
) -> LoopxEnvironmentStatus {
    let core_statuses = [
        core.sidecar.status,
        core.git_worktree.status,
        core.agent_model.status,
    ];
    if core_statuses.contains(&LoopxEnvironmentFactStatus::Checking) {
        return LoopxEnvironmentStatus::Checking;
    }
    if core_statuses.contains(&LoopxEnvironmentFactStatus::Unavailable) {
        return LoopxEnvironmentStatus::Blocked;
    }
    if core_statuses.contains(&LoopxEnvironmentFactStatus::Unknown) {
        return LoopxEnvironmentStatus::Unknown;
    }
    if core_statuses.contains(&LoopxEnvironmentFactStatus::Degraded) {
        return LoopxEnvironmentStatus::Degraded;
    }

    let optional_statuses = [
        optional.python_fallback.status,
        optional.open_viking.status,
        optional.github_auth.status,
    ];
    if optional_statuses.iter().any(|status| {
        matches!(
            status,
            LoopxEnvironmentFactStatus::Degraded | LoopxEnvironmentFactStatus::Unavailable
        )
    }) {
        LoopxEnvironmentStatus::Degraded
    } else {
        LoopxEnvironmentStatus::Ready
    }
}

pub fn decide_action_status(
    client_request_already_applied: bool,
    expected_revision: u64,
    current_revision: u64,
) -> LoopxActionStatus {
    if client_request_already_applied {
        LoopxActionStatus::Duplicate
    } else if expected_revision != current_revision {
        LoopxActionStatus::RevisionConflict
    } else {
        LoopxActionStatus::Applied
    }
}

pub fn decide_events_page_status(
    current_stream_id: &str,
    requested_stream_id: &str,
    after_cursor: u64,
    oldest_retained_cursor: Option<u64>,
    latest_cursor: u64,
) -> LoopxEventsPageStatus {
    if current_stream_id != requested_stream_id || after_cursor > latest_cursor {
        return LoopxEventsPageStatus::SnapshotRequired;
    }
    if let Some(oldest) = oldest_retained_cursor {
        if after_cursor.saturating_add(1) < oldest {
            return LoopxEventsPageStatus::SnapshotRequired;
        }
    }
    LoopxEventsPageStatus::Current
}

pub fn build_intake_fingerprint(
    target: &LoopxIntakeTarget,
    candidates: &[LoopxIntakeCandidate],
    workspace_path: Option<&str>,
    model_id: &str,
    permission_scopes: &[LoopxPermissionScope],
) -> String {
    let mut item_facts = candidates
        .iter()
        .map(|candidate| {
            format!(
                "{}:{:?}:{}:{}",
                candidate.key.canonical_id(),
                candidate.state,
                candidate.has_images,
                candidate.from_repository
            )
        })
        .collect::<Vec<_>>();
    item_facts.sort();
    let mut scopes = permission_scopes.to_vec();
    scopes.sort();
    scopes.dedup();
    let target_id = match target {
        LoopxIntakeTarget::Repository { repository } => repository.canonical_id(),
        LoopxIntakeTarget::Item { item } => item.canonical_id(),
    };
    let payload = format!(
        "target={target_id}\nitems={}\nworkspace={}\nmodel={model_id}\nscopes={scopes:?}",
        item_facts.join("|"),
        workspace_path.unwrap_or_default(),
    );
    format!("sha256:{}", hex::encode(Sha256::digest(payload.as_bytes())))
}
