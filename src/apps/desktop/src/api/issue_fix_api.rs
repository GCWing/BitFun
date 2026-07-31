//! Issue-fix Tauri commands.
//!
//! Deliberately dry-run only. There is no execute flag anywhere in this surface,
//! so nothing reachable from the UI can create a branch, run a validation
//! command, or open a pull request. Granting that authority is a separate,
//! explicit step — see `docs/development/loopx-issue-fix-integration.md`.

use bitfun_services_integrations::loopx_issue_fix::orchestrator::{
    ExecutionMode, IssueFixOrchestrator, IssueFixRequest, ReproductionStatus, ScopeClass,
};
use bitfun_services_integrations::loopx_issue_fix::repository_context::{
    RepositoryContext, RepositoryContextBuilder,
};
use bitfun_services_integrations::loopx_issue_fix::LoopxIssueFix;
use log::error;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::api::app_state::AppState;

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
