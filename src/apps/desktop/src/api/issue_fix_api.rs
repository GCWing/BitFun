//! Continuous Issue-Fix commands.
//!
//! LoopX Kernel owns the durable issue todos and lifecycle decisions. BitFun's
//! persistent Cron service is only the host wake mechanism for one ordinary
//! Agent session; no BitFun Thread Goal participates in this path.

use std::path::Path;

use bitfun_core::agentic::coordination::ConversationCoordinator;
use bitfun_core::agentic::core::SessionConfig;
use bitfun_core::service::cron::{
    get_global_cron_service, CreateCronJobRequest, CronJob, CronJobPayload, CronJobRunStatus,
    CronJobTarget, CronSchedule, CronWorkspaceRef, UpdateCronJobRequest,
};
use bitfun_services_integrations::loopx_issue_fix::autonomous::{
    AutonomousControlState, AutonomousIssueFix, AutonomousLightState, IssueSelection, UserDecision,
};
use bitfun_services_integrations::loopx_issue_fix::LoopxIssueFix;
use log::{error, warn};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

use crate::api::app_state::AppState;

const WAKE_INTERVAL_MS: u64 = 10 * 60 * 1_000;
const JOB_NAME_PREFIX: &str = "LoopX Issue Fix: ";

/// Serializes host-loop mutations so two concurrent starts cannot both pass
/// the duplicate-job check and create twin cron jobs.
static HOST_LOOP_LOCK: Mutex<()> = Mutex::const_new(());

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixAvailability {
    pub available: bool,
    pub program: Option<String>,
    /// Where the loopx binary came from: "override" (LOOPX_BIN), "path", or
    /// null when unavailable. The desktop host points LOOPX_BIN at the bundled
    /// sidecar on startup, so "override" also covers the shipped binary.
    pub source: Option<&'static str>,
    /// GitHub CLI presence — the agent's evidence/PR channel.
    pub gh_installed: bool,
    /// `gh auth status` reports an active github.com login.
    pub gh_authenticated: bool,
}

/// One readiness probe for everything continuous Issue-Fix needs at runtime:
/// the LoopX kernel CLI, the GitHub CLI, and a GitHub login. The panel renders
/// targeted guidance for whichever tier is missing.
#[tauri::command]
pub async fn issue_fix_probe(_state: State<'_, AppState>) -> Result<IssueFixAvailability, String> {
    let loopx = LoopxIssueFix::probe();
    let (gh_installed, gh_authenticated) = probe_github_cli().await;
    Ok(match loopx {
        Some(loopx) => IssueFixAvailability {
            available: true,
            program: Some(loopx.program().display().to_string()),
            source: Some(if std::env::var_os("LOOPX_BIN").is_some() {
                "override"
            } else {
                "path"
            }),
            gh_installed,
            gh_authenticated,
        },
        None => IssueFixAvailability {
            available: false,
            program: None,
            source: None,
            gh_installed,
            gh_authenticated,
        },
    })
}

/// Detect the GitHub CLI and an active github.com login without touching any
/// stored BitFun tokens: the heartbeat agent shells out to `gh` itself, so
/// what matters is exactly what `gh` sees.
async fn probe_github_cli() -> (bool, bool) {
    let mut command = tokio::process::Command::new("gh");
    command
        .args(["auth", "status", "--hostname", "github.com"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        // tokio's Command exposes creation_flags directly; suppress the
        // console window that would otherwise flash on spawn.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    match command.status().await {
        Ok(status) => (true, status.success()),
        Err(_) => (false, false),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixAutonomousStatusRequest {
    pub repository_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixHostLoopState {
    pub enabled: bool,
    pub job_id: Option<String>,
    pub session_id: Option<String>,
    pub active_turn_id: Option<String>,
    pub next_run_at_ms: Option<i64>,
    pub last_run_status: Option<String>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
}

impl Default for IssueFixHostLoopState {
    fn default() -> Self {
        Self {
            enabled: false,
            job_id: None,
            session_id: None,
            active_turn_id: None,
            next_run_at_ms: None,
            last_run_status: None,
            last_error: None,
            consecutive_failures: 0,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixAutonomousStatusResponse {
    #[serde(flatten)]
    pub control: AutonomousControlState,
    pub host_loop: IssueFixHostLoopState,
}

#[tauri::command]
pub async fn issue_fix_autonomous_status(
    _state: State<'_, AppState>,
    request: IssueFixAutonomousStatusRequest,
) -> Result<Option<IssueFixAutonomousStatusResponse>, String> {
    let repository_path = required_repository_path(&request.repository_path)?;
    // A repository that has never started continuous fixing has no LoopX
    // control plane yet; that is a normal pre-bootstrap state, not an error.
    if !AutonomousIssueFix::is_bootstrapped(repository_path) {
        return Ok(None);
    }
    let loopx =
        LoopxIssueFix::probe().ok_or_else(|| "loopx is not installed on this host".to_string())?;
    let control = AutonomousIssueFix::new(loopx)
        .inspect(repository_path)
        .await
        .map_err(|error| {
            error!("Failed to inspect LoopX Issue-Fix state: {error}");
            format!("Failed to read LoopX Issue-Fix state: {error}")
        })?;
    let host_loop = host_loop_state(&control.goal_id, repository_path).await;
    Ok(Some(IssueFixAutonomousStatusResponse { control, host_loop }))
}

/// Background-poll response: LoopX todo projection without `quota should-run`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixAutonomousPollResponse {
    #[serde(flatten)]
    pub light: AutonomousLightState,
    pub host_loop: IssueFixHostLoopState,
}

/// Cheap status for the panel's poll loop. Unlike `issue_fix_autonomous_status`
/// this never runs `quota should-run` (which appends a LoopX rollout event per
/// call), so polling it on an interval does not grow LoopX's event log.
#[tauri::command]
pub async fn issue_fix_autonomous_poll(
    _state: State<'_, AppState>,
    request: IssueFixAutonomousStatusRequest,
) -> Result<Option<IssueFixAutonomousPollResponse>, String> {
    let repository_path = required_repository_path(&request.repository_path)?;
    if !AutonomousIssueFix::is_bootstrapped(repository_path) {
        return Ok(None);
    }
    let loopx =
        LoopxIssueFix::probe().ok_or_else(|| "loopx is not installed on this host".to_string())?;
    let light = AutonomousIssueFix::new(loopx)
        .poll(repository_path)
        .await
        .map_err(|error| {
            error!("Failed to poll LoopX Issue-Fix todos: {error}");
            format!("Failed to poll LoopX Issue-Fix state: {error}")
        })?;
    let host_loop = host_loop_state(&light.goal_id, repository_path).await;
    Ok(Some(IssueFixAutonomousPollResponse { light, host_loop }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixAnswerUserQuestionRequest {
    pub repository_path: String,
    pub todo_id: String,
    pub decision: UserDecision,
    pub reason: Option<String>,
}

#[tauri::command]
pub async fn issue_fix_answer_user_question(
    _state: State<'_, AppState>,
    request: IssueFixAnswerUserQuestionRequest,
) -> Result<IssueFixAutonomousStatusResponse, String> {
    let repository_path = required_repository_path(&request.repository_path)?;
    let loopx =
        LoopxIssueFix::probe().ok_or_else(|| "loopx is not installed on this host".to_string())?;
    let autonomous = AutonomousIssueFix::new(loopx);
    let control = autonomous
        .answer_user_question(
            repository_path,
            &request.todo_id,
            request.decision,
            request.reason.as_deref(),
        )
        .await
        .map_err(|error| {
            error!("Failed to answer LoopX Issue-Fix user question: {error}");
            format!("Failed to answer LoopX Issue-Fix user question: {error}")
        })?;
    // The wake must not race a concurrent Stop: run_job_now's manual trigger
    // bypasses enabled=false, so re-read the job state under the same lock
    // Stop holds while disabling.
    let _guard = HOST_LOOP_LOCK.lock().await;
    let mut host_loop = host_loop_state(&control.goal_id, repository_path).await;
    if host_loop.enabled {
        if let (Some(cron), Some(job_id)) = (get_global_cron_service(), host_loop.job_id.as_deref())
        {
            // The stored heartbeat prompt is a snapshot; a gate answer is a
            // natural point to re-sync it with the installed LoopX version.
            match autonomous.heartbeat_prompt(repository_path).await {
                Ok(prompt) => {
                    if let Err(error) = cron
                        .update_job(
                            job_id,
                            UpdateCronJobRequest {
                                payload: Some(CronJobPayload { text: prompt }),
                                ..UpdateCronJobRequest::default()
                            },
                        )
                        .await
                    {
                        warn!("Failed to refresh the Issue-Fix heartbeat prompt: {error}");
                    }
                }
                Err(error) => {
                    warn!("Failed to regenerate the Issue-Fix heartbeat prompt: {error}")
                }
            }
            match cron.run_job_now(job_id).await {
                Ok(job) => host_loop = project_host_loop(&job),
                Err(error) => warn!(
                    "LoopX Issue-Fix user decision was recorded but the host loop could not be woken immediately: {error}"
                ),
            }
        }
    }
    Ok(IssueFixAutonomousStatusResponse { control, host_loop })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixAutonomousIssueRequest {
    pub issue_ref: String,
    pub issue_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixStartAutonomousRequest {
    /// Existing session to host the heartbeat. Empty when `hidden_host` asks
    /// the backend to create (or reuse) a hidden session instead.
    #[serde(default)]
    pub session_id: String,
    /// MiniApp mode: host the heartbeat in a hidden session owned by the
    /// backend, invisible in the sidebar. The repair loop keeps running when
    /// the MiniApp is closed because scheduling stays host-owned.
    #[serde(default)]
    pub hidden_host: bool,
    pub repo: String,
    pub repository_path: String,
    pub issues: Vec<IssueFixAutonomousIssueRequest>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixStartAutonomousResponse {
    #[serde(flatten)]
    pub control: AutonomousControlState,
    pub host_loop: IssueFixHostLoopState,
    pub added_issue_refs: Vec<String>,
    pub immediate_turn_id: Option<String>,
}

#[tauri::command]
pub async fn issue_fix_start_autonomous(
    _state: State<'_, AppState>,
    coordinator: State<'_, std::sync::Arc<ConversationCoordinator>>,
    request: IssueFixStartAutonomousRequest,
) -> Result<IssueFixStartAutonomousResponse, String> {
    let repository_path = required_repository_path(&request.repository_path)?;
    let cron =
        get_global_cron_service().ok_or_else(|| "Cron service is not initialized".to_string())?;
    let loopx =
        LoopxIssueFix::probe().ok_or_else(|| "loopx is not installed on this host".to_string())?;
    let selections = request
        .issues
        .into_iter()
        .map(|issue| IssueSelection {
            issue_ref: issue.issue_ref,
            issue_url: issue.issue_url,
        })
        .collect::<Vec<_>>();

    let autonomous = AutonomousIssueFix::new(loopx);
    // First use on a fresh repository: create the LoopX goal and the host
    // agent lane before writing intake todos. No-op when already connected.
    autonomous
        .ensure_bootstrapped(repository_path)
        .await
        .map_err(|error| {
            error!("Failed to bootstrap LoopX for continuous Issue-Fix: {error}");
            format!("Failed to connect this repository to LoopX: {error}")
        })?;

    let plan = autonomous
        .start(repository_path, request.repo.trim(), &selections)
        .await
        .map_err(|error| {
            error!("Failed to start continuous LoopX Issue-Fix: {error}");
            format!("Failed to start continuous Issue-Fix: {error}")
        })?;

    // Resolve the heartbeat host session. MiniApp mode asks the backend for a
    // hidden session (invisible in the sidebar, reused across starts via the
    // existing job's binding); panel mode passes an explicit visible session.
    let session_id = if request.hidden_host {
        resolve_hidden_heartbeat_session(
            &coordinator,
            &plan.control.goal_id,
            repository_path,
            cron.list_jobs().await,
        )
        .await?
    } else {
        let session_id = request.session_id.trim();
        if session_id.is_empty() {
            return Err("session_id is required for continuous issue fixing".to_string());
        }
        session_id.to_string()
    };

    let job_name = job_name(&plan.control.goal_id);
    let workspace = CronWorkspaceRef {
        workspace_id: None,
        workspace_path: repository_path.display().to_string(),
        project_workspace_path: Some(repository_path.display().to_string()),
        execution_target: None,
        remote_connection_id: None,
        remote_ssh_host: None,
    };
    let target = CronJobTarget::Session {
        session_id: session_id.clone(),
        workspace,
    };
    let schedule = CronSchedule::Every {
        every_ms: WAKE_INTERVAL_MS,
        anchor_ms: None,
    };
    let payload = CronJobPayload {
        text: plan.heartbeat_prompt,
    };

    let _guard = HOST_LOOP_LOCK.lock().await;
    let matching = resolve_host_loop_job(&job_name, repository_path).await?;
    let job = if let Some(existing) = matching {
        cron.update_job(
            &existing.id,
            UpdateCronJobRequest {
                name: Some(job_name),
                schedule: Some(schedule),
                payload: Some(payload),
                enabled: Some(true),
                target: Some(target),
            },
        )
        .await
    } else {
        cron.create_job(CreateCronJobRequest {
            name: job_name,
            schedule,
            payload,
            enabled: true,
            target,
        })
        .await
    }
    .map_err(|error| {
        error!("Failed to persist the continuous Issue-Fix host loop: {error}");
        format!("Failed to persist continuous Issue-Fix host loop: {error}")
    })?;

    let triggered = cron.run_job_now(&job.id).await.map_err(|error| {
        error!("Failed to trigger the continuous Issue-Fix host loop: {error}");
        format!("Failed to trigger continuous Issue-Fix host loop: {error}")
    })?;
    let immediate_turn_id = triggered.state.active_turn_id.clone();
    let host_loop = project_host_loop(&triggered);

    Ok(IssueFixStartAutonomousResponse {
        control: plan.control,
        host_loop,
        added_issue_refs: plan.added_issue_refs,
        immediate_turn_id,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixStopAutonomousRequest {
    pub repository_path: String,
}

/// Disable the host wake loop. LoopX Kernel state (goal, todos, gates) is left
/// untouched: stopping the heartbeat is a host scheduling concern, and a later
/// start resumes exactly where the Kernel says the work stands.
#[tauri::command]
pub async fn issue_fix_stop_autonomous(
    _state: State<'_, AppState>,
    request: IssueFixStopAutonomousRequest,
) -> Result<IssueFixHostLoopState, String> {
    let repository_path = required_repository_path(&request.repository_path)?;
    // Stop is the kill switch: it must work even when the LoopX registry is
    // broken or the goal identity changed, so a failed lookup only demotes
    // which job gets projected, never aborts the disable sweep.
    let current_name = match AutonomousIssueFix::identity(repository_path) {
        Ok((goal_id, _)) => job_name(&goal_id),
        Err(error) => {
            warn!("Stopping Issue-Fix host loops without a resolvable LoopX goal: {error}");
            String::new()
        }
    };
    let cron =
        get_global_cron_service().ok_or_else(|| "Cron service is not initialized".to_string())?;

    let _guard = HOST_LOOP_LOCK.lock().await;
    // Disable every Issue-Fix loop bound to this repository, not just the
    // current goal's: this must also catch jobs orphaned by an older goal
    // identity.
    let mut stopped: Option<CronJob> = None;
    for job in cron.list_jobs().await {
        if !job.name.starts_with(JOB_NAME_PREFIX) || !job_targets_repository(&job, repository_path)
        {
            continue;
        }
        let disabled = cron
            .update_job(
                &job.id,
                UpdateCronJobRequest {
                    enabled: Some(false),
                    ..UpdateCronJobRequest::default()
                },
            )
            .await
            .map_err(|error| {
                error!("Failed to stop the continuous Issue-Fix host loop: {error}");
                format!("Failed to stop continuous Issue-Fix host loop: {error}")
            })?;
        if disabled.name == current_name || stopped.is_none() {
            stopped = Some(disabled);
        }
    }
    Ok(stopped
        .as_ref()
        .map(project_host_loop)
        .unwrap_or_default())
}

fn required_repository_path(raw: &str) -> Result<&Path, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("repository_path is required".to_string());
    }
    let path = Path::new(trimmed);
    if !path.is_dir() {
        return Err(format!(
            "Repository path does not exist: {}",
            path.display()
        ));
    }
    Ok(path)
}

fn job_name(goal_id: &str) -> String {
    format!("{JOB_NAME_PREFIX}{goal_id}")
}

/// Session name for the hidden heartbeat host (MiniApp mode). Not user-facing
/// in the sidebar; shows up only in diagnostics.
const HIDDEN_HOST_SESSION_NAME: &str = "LoopX Issue-Fix heartbeat";
const HIDDEN_HOST_AGENT_KIND: &str = "agentic";

/// Find or create the hidden session hosting this goal's heartbeat.
///
/// Reuse order: the existing job's bound session (when it still exists) wins,
/// so restarts keep the conversation context; otherwise a fresh hidden
/// session is created. Hidden sessions never appear in the sidebar, which is
/// what lets the MiniApp own the whole Issue-Fix experience while scheduling
/// stays host-side.
async fn resolve_hidden_heartbeat_session(
    coordinator: &ConversationCoordinator,
    goal_id: &str,
    repository_path: &Path,
    jobs: Vec<CronJob>,
) -> Result<String, String> {
    let existing = matching_jobs(&job_name(goal_id), repository_path, jobs)
        .into_iter()
        .filter_map(|job| job.session_id().map(str::to_string))
        .find(|session_id| {
            coordinator
                .get_session_manager()
                .get_session(session_id)
                .is_some()
        });
    if let Some(session_id) = existing {
        return Ok(session_id);
    }
    let config = SessionConfig {
        enable_tools: true,
        safe_mode: true,
        auto_compact: true,
        enable_context_compression: true,
        ..Default::default()
    };
    let session = coordinator
        .create_hidden_subagent_session_with_workspace(
            None,
            HIDDEN_HOST_SESSION_NAME.to_string(),
            HIDDEN_HOST_AGENT_KIND.to_string(),
            config,
            repository_path.display().to_string(),
            Some("issue-fix".to_string()),
        )
        .await
        .map_err(|error| format!("Failed to create the hidden heartbeat session: {error}"))?;
    Ok(session.session_id)
}

/// Project the current goal's host loop, tolerating duplicates.
///
/// Duplicate jobs (from a concurrent start racing the create) must not brick
/// every status call: project the most recently updated one and leave the
/// cleanup to the next start, which holds `HOST_LOOP_LOCK`.
async fn host_loop_state(goal_id: &str, repository_path: &Path) -> IssueFixHostLoopState {
    let Some(cron) = get_global_cron_service() else {
        return IssueFixHostLoopState::default();
    };
    let mut matching = matching_jobs(&job_name(goal_id), repository_path, cron.list_jobs().await);
    if matching.len() > 1 {
        warn!(
            "Found {} continuous Issue-Fix host loops for goal {goal_id}; projecting the newest",
            matching.len()
        );
        matching.sort_by_key(|job| std::cmp::Reverse(job.updated_at_ms));
    }
    matching
        .first()
        .map(project_host_loop)
        .unwrap_or_default()
}

/// Pick the canonical host-loop job for `start`, deleting duplicates and
/// disabling stale jobs left behind by an older goal identity. Callers must
/// hold `HOST_LOOP_LOCK`.
async fn resolve_host_loop_job(
    name: &str,
    repository_path: &Path,
) -> Result<Option<CronJob>, String> {
    let Some(cron) = get_global_cron_service() else {
        return Err("Cron service is not initialized".to_string());
    };
    let mut canonical: Option<CronJob> = None;
    for job in cron.list_jobs().await {
        if !job.name.starts_with(JOB_NAME_PREFIX) || !job_targets_repository(&job, repository_path)
        {
            continue;
        }
        if job.name != name {
            // A job from a previous goal identity (e.g. after re-bootstrap)
            // would keep firing its stale prompt invisibly; park it.
            if job.enabled {
                warn!("Disabling stale continuous Issue-Fix host loop {}", job.name);
                let _ = cron
                    .update_job(
                        &job.id,
                        UpdateCronJobRequest {
                            enabled: Some(false),
                            ..UpdateCronJobRequest::default()
                        },
                    )
                    .await;
            }
            continue;
        }
        match &canonical {
            Some(kept) if kept.updated_at_ms >= job.updated_at_ms => {
                warn!("Deleting duplicate continuous Issue-Fix host loop {}", job.id);
                let _ = cron.delete_job(&job.id).await;
            }
            Some(kept) => {
                warn!("Deleting duplicate continuous Issue-Fix host loop {}", kept.id);
                let _ = cron.delete_job(&kept.id).await;
                canonical = Some(job);
            }
            None => canonical = Some(job),
        }
    }
    Ok(canonical)
}

fn matching_jobs(name: &str, repository_path: &Path, jobs: Vec<CronJob>) -> Vec<CronJob> {
    jobs.into_iter()
        .filter(|job| job.name == name && job_targets_repository(job, repository_path))
        .collect()
}

fn job_targets_repository(job: &CronJob, repository_path: &Path) -> bool {
    same_path(&job.workspace().workspace_path, repository_path)
        || job
            .workspace()
            .project_workspace_path
            .as_deref()
            .is_some_and(|path| same_path(path, repository_path))
}

fn same_path(candidate: &str, expected: &Path) -> bool {
    let candidate = candidate.replace('/', "\\");
    let expected = expected.display().to_string().replace('/', "\\");
    let candidate = candidate.trim_end_matches('\\');
    let expected = expected.trim_end_matches('\\');
    // Case-insensitive comparison is a Windows filesystem property; on other
    // platforms /repos/Foo and /repos/foo are distinct repositories.
    #[cfg(windows)]
    {
        candidate.eq_ignore_ascii_case(expected)
    }
    #[cfg(not(windows))]
    {
        candidate == expected
    }
}

fn run_status_label(status: CronJobRunStatus) -> &'static str {
    match status {
        CronJobRunStatus::Queued => "queued",
        CronJobRunStatus::Running => "running",
        CronJobRunStatus::Ok => "ok",
        CronJobRunStatus::Error => "error",
        CronJobRunStatus::Cancelled => "cancelled",
    }
}

fn project_host_loop(job: &CronJob) -> IssueFixHostLoopState {
    IssueFixHostLoopState {
        enabled: job.enabled,
        job_id: Some(job.id.clone()),
        session_id: job.session_id().map(str::to_string),
        active_turn_id: job.state.active_turn_id.clone(),
        next_run_at_ms: job.state.next_run_at_ms,
        last_run_status: job
            .state
            .last_run_status
            .map(|status| run_status_label(status).to_string()),
        last_error: job.state.last_error.clone(),
        consecutive_failures: job.state.consecutive_failures,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_job_name_is_goal_scoped() {
        assert_eq!(job_name("bitfun-goal"), "LoopX Issue Fix: bitfun-goal");
    }

    #[test]
    fn windows_path_matching_is_case_and_separator_insensitive() {
        assert!(same_path(
            "C:/codeagent/BitFun/",
            Path::new("c:\\codeagent\\bitfun")
        ));
    }
}
