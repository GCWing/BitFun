//! Issue-fix Tauri commands.
//!
//! Two-step surface: planning is read-only (route projection), execution
//! hands the actual fix to the agent loop as a dialog turn. Nothing in the
//! planning path can create a branch, run a validation command, or open a
//! pull request; execution only submits agent work and returns the outcome,
//! so PR creation stays behind the existing gates.

use bitfun_services_integrations::loopx_issue_fix::orchestrator::{
    ExecutionMode, FixRoute, IssueFixOrchestrator, IssueFixRequest, ReproductionStatus, ScopeClass,
};
use bitfun_services_integrations::loopx_issue_fix::repository_context::{
    RepositoryContext, RepositoryContextBuilder,
};
use bitfun_services_integrations::loopx_issue_fix::LoopxIssueFix;
use bitfun_runtime_ports::{
    AgentDialogTurnRequest, DialogSubmissionPolicy, DialogTriggerSource,
};
use log::error;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::api::app_state::AppState;
use crate::runtime::DesktopRuntimeContext;

/// Whether the feature can run on this host.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixAvailability {
    pub available: bool,
    /// Present only when available, for diagnostics.
    pub program: Option<String>,
}

/// Probe for the `loopx` CLI so the UI can hide its entry point when absent.
#[tauri::command]
pub async fn issue_fix_probe(_state: State<'_, AppState>) -> Result<IssueFixAvailability, String> {
    match LoopxIssueFix::probe() {
        Some(loopx) => Ok(IssueFixAvailability {
            available: true,
            program: Some(loopx.program().display().to_string()),
        }),
        None => Ok(IssueFixAvailability {
            available: false,
            program: None,
        }),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixPlanRequest {
    /// Public-safe `owner/repo`.
    pub repo: String,
    pub issue_ref: String,
    pub issue_url: String,
    /// Local checkout. Only read from; never written in dry-run mode.
    pub repository_path: String,
    pub base_branch: Option<String>,
}

/// One issue's planning result, flattened for the UI.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixPlanResponse {
    pub issue_ref: String,
    /// `fix_pr`, `comment_only`, or `triage_only`.
    pub route: String,
    /// `runnable_successor`, `monitor_continuation`, `user_gate`, or `no_followup`.
    pub next_step: String,
    /// `grounded`, `partial`, `ungrounded`, or `not_provided`.
    pub context_grounding: String,
    /// LoopX's reason codes, passed through verbatim rather than paraphrased.
    pub reason_codes: Vec<String>,
    /// Which of change_scope / reproduction / validation are still unresolved.
    pub unresolved_aspects: Vec<String>,
    /// The branch LoopX would use. Never created in dry-run mode.
    pub issue_branch: Option<String>,
    /// Always false here, since a dry run creates nothing.
    pub branch_ready: bool,
}

/// Ask LoopX which route an issue should take.
///
/// Runs `feasibility` and, on a fix route, a dry-run branch projection. Both are
/// read-only: LoopX reports `external_writes_performed: false` throughout, and
/// `--no-write-domain-state` keeps it out of goal state as well.
///
/// No repository context is supplied yet, because nothing in BitFun generates one.
/// LoopX therefore reports `not_provided` and declines to open a pull request.
/// That is the honest current state rather than a limitation of this command —
/// the reason codes it returns say exactly which evidence is missing.
#[tauri::command]
pub async fn issue_fix_plan_issue(
    _state: State<'_, AppState>,
    request: IssueFixPlanRequest,
) -> Result<IssueFixPlanResponse, String> {
    let Some(loopx) = LoopxIssueFix::probe() else {
        return Err("loopx is not installed on this host".to_string());
    };

    let temp_dir = tempfile::tempdir().map_err(|error| {
        error!("Failed to create a temp dir for the issue-fix context: {error}");
        format!("Failed to prepare the issue-fix workspace: {error}")
    })?;

    let context = empty_repository_context().map_err(|error| {
        error!("Failed to build a placeholder repository context: {error}");
        format!("Failed to prepare issue-fix evidence: {error}")
    })?;

    let base_branch = request.base_branch.as_deref().unwrap_or("main");
    let issue_request = IssueFixRequest {
        repo: &request.repo,
        issue_ref: &request.issue_ref,
        issue_url: &request.issue_url,
        context: &context,
        // Naming a validation surface is what permits `fix_pr` at all. Until
        // BitFun reads the repository and can name a real one, say so plainly
        // instead of asserting a surface that was never checked.
        validation_label: "not yet determined",
        reproduction_label: "not yet investigated",
        reproduction_status: ReproductionStatus::Planned,
        scope_class: ScopeClass::Uncertain,
        base_branch,
    };

    let outcome = IssueFixOrchestrator::new(&loopx)
        .plan_issue(
            &issue_request,
            &request.repository_path,
            temp_dir.path(),
            ExecutionMode::DryRun,
        )
        .await
        .map_err(|error| {
            error!(
                "Failed to plan issue-fix: repo={}, issue={}, error={error}",
                request.repo, request.issue_ref
            );
            format!("Failed to plan this issue: {error}")
        })?;

    Ok(IssueFixPlanResponse {
        issue_ref: outcome.issue_ref,
        route: route_label(outcome.feasibility.route),
        next_step: next_step_label(outcome.feasibility.next_step),
        context_grounding: grounding_label(outcome.feasibility.context_grounding),
        reason_codes: outcome.feasibility.reason_codes,
        unresolved_aspects: outcome.feasibility.unresolved_aspects,
        issue_branch: outcome
            .branch
            .as_ref()
            .map(|branch| branch.issue_branch.clone()),
        branch_ready: outcome
            .branch
            .as_ref()
            .is_some_and(|branch| branch.branch_ready),
    })
}

/// Request to hand one issue's fix to the agent loop.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixExecuteRequest {
    /// The session whose agent loop should do the fixing.
    pub session_id: String,
    /// Public-safe `owner/repo`.
    pub repo: String,
    pub issue_ref: String,
    pub issue_url: String,
    /// Local checkout the agent works in.
    pub repository_path: String,
    pub base_branch: Option<String>,
    /// Issue title, included in the task message so the agent can work
    /// without an extra metadata fetch.
    pub issue_title: Option<String>,
}

/// Outcome of handing an issue to the agent loop.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFixExecuteResponse {
    pub issue_ref: String,
    /// `fix_pr`, `comment_only`, or `triage_only` — the route LoopX selected.
    pub route: String,
    /// Whether the fix task was actually submitted to the agent loop.
    pub submitted: bool,
    /// Why nothing was submitted, when `submitted` is false.
    pub not_submitted_reason: Option<String>,
    /// The dialog turn id, when submitted.
    pub turn_id: Option<String>,
}

/// Ask LoopX for the route, then hand the fix to the agent loop when it is
/// a fixable one.
///
/// This is the step that actually spends model tokens: the returned dialog
/// turn drives the session's agent through reading the repository, patching
/// the issue, and validating the result. Branch creation and PR opening are
/// still separate gates that follow the agent's work; this command never
/// performs those itself.
#[tauri::command]
pub async fn issue_fix_execute(
    runtime: State<'_, DesktopRuntimeContext>,
    request: IssueFixExecuteRequest,
) -> Result<IssueFixExecuteResponse, String> {
    let Some(loopx) = LoopxIssueFix::probe() else {
        return Err("loopx is not installed on this host".to_string());
    };

    let temp_dir = tempfile::tempdir().map_err(|error| {
        error!("Failed to create a temp dir for the issue-fix context: {error}");
        format!("Failed to prepare the issue-fix workspace: {error}")
    })?;

    let context = empty_repository_context().map_err(|error| {
        error!("Failed to build a placeholder repository context: {error}");
        format!("Failed to prepare issue-fix evidence: {error}")
    })?;

    let base_branch = request.base_branch.as_deref().unwrap_or("main");
    let issue_request = IssueFixRequest {
        repo: &request.repo,
        issue_ref: &request.issue_ref,
        issue_url: &request.issue_url,
        context: &context,
        validation_label: "agent-run validation",
        reproduction_label: "agent-investigated reproduction",
        reproduction_status: ReproductionStatus::Planned,
        scope_class: ScopeClass::Uncertain,
        base_branch,
    };

    let outcome = IssueFixOrchestrator::new(&loopx)
        .plan_issue(
            &issue_request,
            &request.repository_path,
            temp_dir.path(),
            ExecutionMode::DryRun,
        )
        .await
        .map_err(|error| {
            error!(
                "Failed to plan issue-fix execution: repo={}, issue={}, error={error}",
                request.repo, request.issue_ref
            );
            format!("Failed to plan this issue: {error}")
        })?;

    let route = route_label(outcome.feasibility.route);
    if outcome.feasibility.route != FixRoute::FixPr {
        return Ok(IssueFixExecuteResponse {
            issue_ref: request.issue_ref.clone(),
            route,
            submitted: false,
            not_submitted_reason: Some(
                "LoopX selected a non-fix route (comment_only or triage_only); nothing was submitted"
                    .to_string(),
            ),
            turn_id: None,
        });
    }

    let message = issue_fix_task_message(&request);
    let session_id = request.session_id.trim().to_string();
    if session_id.is_empty() {
        return Err("session_id is required to execute an issue fix".to_string());
    }

    let dialog_request = AgentDialogTurnRequest {
        session_id: session_id.clone(),
        message: message.clone(),
        original_message: None,
        turn_id: None,
        // Empty: the coordinator resolves the session's own mode instead of
        // overriding it with a hard-coded type.
        agent_type: String::new(),
        workspace_path: Some(request.repository_path.clone()),
        remote_connection_id: None,
        remote_ssh_host: None,
        policy: DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopApi),
        reply_route: None,
        prepended_reminders: Vec::new(),
        attachments: Vec::new(),
        metadata: serde_json::Map::new(),
    };

    let outcome = runtime
        .agent_runtime()
        .submit_dialog_turn(dialog_request)
        .await
        .map_err(|error| {
            error!(
                "Failed to submit the issue-fix dialog turn: repo={}, issue={}, error={error}",
                request.repo, request.issue_ref
            );
            format!("Failed to start the fix task: {error}")
        })?;

    let turn_id = match &outcome {
        bitfun_runtime_ports::DialogSubmitOutcome::Started { turn_id, .. }
        | bitfun_runtime_ports::DialogSubmitOutcome::Queued { turn_id, .. } => {
            Some(turn_id.clone())
        }
    };

    Ok(IssueFixExecuteResponse {
        issue_ref: request.issue_ref,
        route,
        submitted: true,
        not_submitted_reason: None,
        turn_id,
    })
}

/// The task message handed to the agent loop for one issue.
fn issue_fix_task_message(request: &IssueFixExecuteRequest) -> String {
    let title = request
        .issue_title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("(no title captured)");
    format!(
        "Please fix the following repository issue.\n\
         Issue: {repo}#{issue_ref}\n\
         Title: {title}\n\
         URL: {url}\n\n\
         Instructions:\n\
         - Read the relevant repository sources first and locate the code that causes the problem.\n\
         - Make the smallest fix that addresses the reported problem.\n\
         - Validate the change with the repository's focused checks for the touched surface.\n\
         - Report what you changed, how you validated it, and any remaining risks.\n\
         Do not create a branch or open a pull request yourself; report the result here instead.",
        repo = request.repo,
        issue_ref = request.issue_ref,
        title = title,
        url = request.issue_url,
    )
}

/// A context with one advisory placeholder source.
///
/// LoopX rejects a context with no sources, and an advisory memory-retrieval entry
/// grounds nothing — so this reports "we have not read the repository" without
/// overstating what is known.
fn empty_repository_context() -> Result<RepositoryContext, Box<dyn std::error::Error + Send + Sync>>
{
    use bitfun_services_integrations::loopx_issue_fix::repository_context::{
        Freshness, RepositoryContextSource, SourceKind, SupportAspect, Trust,
    };

    let mut builder = RepositoryContextBuilder::new();
    builder.push(RepositoryContextSource {
        source_id: "bitfun-pending-repository-read".to_string(),
        source_kind: SourceKind::MemoryRetrieval,
        reference: "bitfun:issue-fix-pending-read".to_string(),
        trust: Trust::Advisory,
        freshness: Freshness::Unknown,
        supports: vec![SupportAspect::ChangeScope],
        summary: "BitFun has not read repository sources for this issue yet.".to_string(),
        consultation_state: None,
    })?;
    Ok(builder.build()?)
}

fn route_label(
    route: bitfun_services_integrations::loopx_issue_fix::orchestrator::FixRoute,
) -> String {
    use bitfun_services_integrations::loopx_issue_fix::orchestrator::FixRoute;
    match route {
        FixRoute::FixPr => "fix_pr",
        FixRoute::CommentOnly => "comment_only",
        FixRoute::TriageOnly => "triage_only",
    }
    .to_string()
}

fn next_step_label(
    step: bitfun_services_integrations::loopx_issue_fix::orchestrator::NextStep,
) -> String {
    use bitfun_services_integrations::loopx_issue_fix::orchestrator::NextStep;
    match step {
        NextStep::RunnableSuccessor => "runnable_successor",
        NextStep::MonitorContinuation => "monitor_continuation",
        NextStep::UserGate => "user_gate",
        NextStep::NoFollowup => "no_followup",
    }
    .to_string()
}

fn grounding_label(
    grounding: bitfun_services_integrations::loopx_issue_fix::orchestrator::ContextGrounding,
) -> String {
    use bitfun_services_integrations::loopx_issue_fix::orchestrator::ContextGrounding;
    match grounding {
        ContextGrounding::Grounded => "grounded",
        ContextGrounding::Partial => "partial",
        ContextGrounding::Ungrounded => "ungrounded",
        ContextGrounding::NotProvided => "not_provided",
    }
    .to_string()
}
