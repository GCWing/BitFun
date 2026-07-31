//! Run one issue through LoopX's deterministic decision chain.
//!
//! The chain is `workflow-plan` → `feasibility` → `caller-repo-branch` →
//! `pr-lifecycle`. LoopX decides *what* to do at each step and writes nothing;
//! BitFun supplies the evidence and performs every side effect.
//!
//! This module exists mostly to make LoopX's JSON safe to consume. The fields
//! that matter sit at non-obvious paths — the route is `decision.route`, not
//! `route`, and the lifecycle decision is `transition.decision` — which is easy
//! to read wrong from the markdown rendering, where both appear flattened. Typed
//! outcomes here mean a caller cannot silently misread a refusal as approval.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::repository_context::RepositoryContext;
use super::{LoopxIssueFix, LoopxIssueFixError};

/// Which resolution LoopX selected for an issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixRoute {
    /// Prepare a branch, validate it, and open a pull request.
    FixPr,
    /// Draft a maintainer comment; posting still needs an explicit gate.
    CommentOnly,
    /// Record a blocker instead of opening an ungrounded patch loop.
    TriageOnly,
}

impl FixRoute {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "fix_pr" => Some(Self::FixPr),
            "comment_only" => Some(Self::CommentOnly),
            "triage_only" => Some(Self::TriageOnly),
            _ => None,
        }
    }

    /// Whether this route may lead to a pull request at all.
    pub fn permits_pull_request(self) -> bool {
        self == Self::FixPr
    }
}

/// What LoopX says should happen next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextStep {
    /// There is agent work to do now.
    RunnableSuccessor,
    /// Keep watching; create no successor.
    MonitorContinuation,
    /// A human must decide before anything else happens.
    UserGate,
    /// Terminal; nothing follows.
    NoFollowup,
}

impl NextStep {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "runnable_successor" => Some(Self::RunnableSuccessor),
            "monitor_continuation" => Some(Self::MonitorContinuation),
            "user_gate" => Some(Self::UserGate),
            "no_followup" => Some(Self::NoFollowup),
            _ => None,
        }
    }

    /// Whether a caller must stop and ask a human.
    ///
    /// LoopX raises this for semantic ambiguity and for missing write authority.
    /// Crossing it automatically would defeat the gate.
    pub fn requires_human(self) -> bool {
        self == Self::UserGate
    }
}

/// How much of the issue's context BitFun managed to ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextGrounding {
    Grounded,
    Partial,
    Ungrounded,
    NotProvided,
}

impl ContextGrounding {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "grounded" => Some(Self::Grounded),
            "partial" => Some(Self::Partial),
            "ungrounded" => Some(Self::Ungrounded),
            "not_provided" => Some(Self::NotProvided),
            _ => None,
        }
    }
}

/// LoopX's route decision for one issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeasibilityOutcome {
    pub route: FixRoute,
    pub next_step: NextStep,
    pub context_grounding: ContextGrounding,
    /// Why LoopX decided this, verbatim. Useful to show a user why a fix was
    /// declined without reinterpreting it.
    pub reason_codes: Vec<String>,
    /// Which of change_scope / reproduction / validation are still unresolved.
    pub unresolved_aspects: Vec<String>,
}

/// The state of a prepared issue branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchOutcome {
    pub issue_branch: String,
    pub base_branch: String,
    /// `dry_run` until a caller opts into execution.
    pub branch_action: String,
    pub branch_ready: bool,
    pub validation_executed: bool,
    pub validation_passed: bool,
    pub changed_files: Vec<String>,
    /// Both this and a `FixRoute::FixPr` route must hold before opening a PR.
    pub review_packet_ready: bool,
    pub review_packet_summary: String,
    /// Why the packet is not ready yet, when it is not.
    pub readiness_blockers: Vec<String>,
}

impl BranchOutcome {
    /// Whether a pull request may be opened for this branch.
    ///
    /// Deliberately requires the route *and* packet readiness together: the
    /// feature ships without a runtime kill switch, so this gate lives on the
    /// action rather than relying on a disabled toggle.
    pub fn may_open_pull_request(&self, route: FixRoute) -> bool {
        route.permits_pull_request() && self.review_packet_ready && self.validation_passed
    }
}

/// How an open pull request should be followed up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestOutcome {
    pub next_step: NextStep,
    pub state: String,
    pub state_bucket: String,
    pub reason: String,
    /// Write scopes the successor would need, when it needs any.
    pub required_write_scopes: Vec<String>,
}

/// What a caller should do next after planning an issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanOutcome {
    pub issue_ref: String,
    pub feasibility: FeasibilityOutcome,
    /// Absent when the route does not lead to a branch, or when planning only.
    pub branch: Option<BranchOutcome>,
}

/// Inputs for one issue's run.
#[derive(Debug, Clone)]
pub struct IssueFixRequest<'a> {
    /// Public-safe `owner/repo` label.
    pub repo: &'a str,
    pub issue_ref: &'a str,
    pub issue_url: &'a str,
    /// Evidence BitFun gathered by reading the repository.
    pub context: &'a RepositoryContext,
    /// How the fix would be checked. LoopX will not select `fix_pr` without
    /// this, whatever the context says.
    pub validation_label: &'a str,
    /// Compact label for the reproduction, not a raw command.
    pub reproduction_label: &'a str,
    pub reproduction_status: ReproductionStatus,
    pub scope_class: ScopeClass,
    pub base_branch: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReproductionStatus {
    Confirmed,
    Planned,
    Missing,
    Blocked,
}

impl ReproductionStatus {
    fn as_arg(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Planned => "planned",
            Self::Missing => "missing",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeClass {
    Bounded,
    Uncertain,
    Oversized,
}

impl ScopeClass {
    fn as_arg(self) -> &'static str {
        match self {
            Self::Bounded => "bounded",
            Self::Uncertain => "uncertain",
            Self::Oversized => "oversized",
        }
    }
}

/// Whether a step may touch the working tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Plan only. Nothing is created, nothing runs.
    DryRun,
    /// Create or claim the issue branch and run the validation command.
    Execute {
        /// Runs in the repository. A caller must have explicit approval for it.
        validation_command: &'static str,
    },
}

#[derive(Debug)]
pub enum OrchestratorError {
    Loopx(LoopxIssueFixError),
    /// A field LoopX is contracted to return was missing or unrecognized.
    UnexpectedPacket {
        field: &'static str,
        value: String,
    },
    ContextWrite(std::io::Error),
    ContextSerialize(serde_json::Error),
    /// The context file path could not be passed to LoopX as text.
    NonUtf8Path,
}

impl std::fmt::Display for OrchestratorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Loopx(error) => write!(f, "{error}"),
            Self::UnexpectedPacket { field, value } => write!(
                f,
                "loopx returned an unrecognized {field}: {value:?}; the CLI contract may have changed"
            ),
            Self::ContextWrite(error) => {
                write!(f, "failed to write the repository context: {error}")
            }
            Self::ContextSerialize(error) => {
                write!(f, "failed to serialize the repository context: {error}")
            }
            Self::NonUtf8Path => write!(f, "the repository context path is not valid UTF-8"),
        }
    }
}

impl std::error::Error for OrchestratorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Loopx(error) => Some(error),
            Self::ContextWrite(error) => Some(error),
            Self::ContextSerialize(error) => Some(error),
            _ => None,
        }
    }
}

impl From<LoopxIssueFixError> for OrchestratorError {
    fn from(error: LoopxIssueFixError) -> Self {
        Self::Loopx(error)
    }
}

/// Drives one issue through the chain.
pub struct IssueFixOrchestrator<'a> {
    loopx: &'a LoopxIssueFix,
}

impl<'a> IssueFixOrchestrator<'a> {
    pub fn new(loopx: &'a LoopxIssueFix) -> Self {
        Self { loopx }
    }

    /// Ask LoopX which route this issue should take.
    ///
    /// Read-only: LoopX writes nothing, and `--no-write-domain-state` keeps it
    /// from touching goal state either.
    pub async fn feasibility(
        &self,
        request: &IssueFixRequest<'_>,
        context_dir: &Path,
    ) -> Result<FeasibilityOutcome, OrchestratorError> {
        let context_path = write_context(request.context, context_dir)?;
        let context_path = context_path
            .to_str()
            .ok_or(OrchestratorError::NonUtf8Path)?;

        let packet = self
            .loopx
            .issue_fix([
                "feasibility",
                "--repo",
                request.repo,
                "--issue-ref",
                request.issue_ref,
                "--url",
                request.issue_url,
                "--reproduction-status",
                request.reproduction_status.as_arg(),
                "--scope-class",
                request.scope_class.as_arg(),
                "--reproduction-label",
                request.reproduction_label,
                "--validation-label",
                request.validation_label,
                "--repository-context-json",
                context_path,
                "--no-write-domain-state",
            ])
            .await?;

        parse_feasibility(&packet)
    }

    /// Prepare the issue branch and, when executing, run the validation command.
    ///
    /// In [`ExecutionMode::DryRun`] this creates nothing; the returned
    /// `branch_action` reports `dry_run`.
    pub async fn prepare_branch(
        &self,
        request: &IssueFixRequest<'_>,
        repo_path: &str,
        mode: ExecutionMode,
    ) -> Result<BranchOutcome, OrchestratorError> {
        let mut args = vec![
            "caller-repo-branch",
            "--repo-path",
            repo_path,
            "--repo",
            request.repo,
            "--issue-ref",
            request.issue_ref,
            "--url",
            request.issue_url,
            "--base-branch",
            request.base_branch,
            "--validation-label",
            request.validation_label,
        ];
        if let ExecutionMode::Execute { validation_command } = mode {
            args.push("--validation-command");
            args.push(validation_command);
            args.push("--execute");
        }

        let packet = self.loopx.issue_fix(args).await?;
        parse_branch(&packet)
    }

    /// Project an open pull request's lifecycle onto a next step.
    pub async fn pull_request_lifecycle(
        &self,
        repo: &str,
        pull_request_ref: &str,
        issue_ref: &str,
        metadata_path: Option<&str>,
    ) -> Result<PullRequestOutcome, OrchestratorError> {
        let mut args = vec![
            "pr-lifecycle",
            "--repo",
            repo,
            "--pr-ref",
            pull_request_ref,
            "--issue-ref",
            issue_ref,
            "--no-write-domain-state",
        ];
        match metadata_path {
            Some(path) => {
                args.push("--metadata-json");
                args.push(path);
            }
            None => args.push("--fetch-metadata"),
        }

        let packet = self.loopx.issue_fix(args).await?;
        parse_pull_request(&packet)
    }

    /// Plan one issue: decide the route, then prepare a branch only when the
    /// route actually permits a pull request.
    pub async fn plan_issue(
        &self,
        request: &IssueFixRequest<'_>,
        repo_path: &str,
        context_dir: &Path,
        mode: ExecutionMode,
    ) -> Result<PlanOutcome, OrchestratorError> {
        let feasibility = self.feasibility(request, context_dir).await?;

        // Skip the branch entirely on a non-fix route. Preparing one would be
        // wasted work at best, and on `--execute` it would create a branch LoopX
        // just declined to justify.
        let branch = if feasibility.route.permits_pull_request() {
            Some(self.prepare_branch(request, repo_path, mode).await?)
        } else {
            None
        };

        Ok(PlanOutcome {
            issue_ref: request.issue_ref.to_string(),
            feasibility,
            branch,
        })
    }
}

fn write_context(
    context: &RepositoryContext,
    dir: &Path,
) -> Result<std::path::PathBuf, OrchestratorError> {
    let path = dir.join("loopx-repository-context.json");
    let bytes = serde_json::to_vec(context).map_err(OrchestratorError::ContextSerialize)?;
    std::fs::write(&path, bytes).map_err(OrchestratorError::ContextWrite)?;
    Ok(path)
}

fn required_str<'p>(
    packet: &'p serde_json::Value,
    path: &[&str],
    field: &'static str,
) -> Result<&'p str, OrchestratorError> {
    let mut cursor = packet;
    for key in path {
        cursor = &cursor[key];
    }
    cursor
        .as_str()
        .ok_or_else(|| OrchestratorError::UnexpectedPacket {
            field,
            value: cursor.to_string(),
        })
}

fn string_list(packet: &serde_json::Value, path: &[&str]) -> Vec<String> {
    let mut cursor = packet;
    for key in path {
        cursor = &cursor[key];
    }
    cursor
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_feasibility(packet: &serde_json::Value) -> Result<FeasibilityOutcome, OrchestratorError> {
    // `decision.route`, not `route`: the markdown rendering flattens these, so
    // reading the top level here would silently yield null.
    let route_text = required_str(packet, &["decision", "route"], "decision.route")?;
    let route = FixRoute::parse(route_text).ok_or_else(|| OrchestratorError::UnexpectedPacket {
        field: "decision.route",
        value: route_text.to_string(),
    })?;

    let step_text = required_str(packet, &["transition", "decision"], "transition.decision")?;
    let next_step =
        NextStep::parse(step_text).ok_or_else(|| OrchestratorError::UnexpectedPacket {
            field: "transition.decision",
            value: step_text.to_string(),
        })?;

    let grounding_text = required_str(
        packet,
        &["observation", "repository_context", "context_status"],
        "observation.repository_context.context_status",
    )?;
    let context_grounding = ContextGrounding::parse(grounding_text).ok_or_else(|| {
        OrchestratorError::UnexpectedPacket {
            field: "observation.repository_context.context_status",
            value: grounding_text.to_string(),
        }
    })?;

    Ok(FeasibilityOutcome {
        route,
        next_step,
        context_grounding,
        reason_codes: string_list(packet, &["decision", "reason_codes"]),
        unresolved_aspects: string_list(
            packet,
            &[
                "observation",
                "repository_context",
                "unresolved_required_aspects",
            ],
        ),
    })
}

fn parse_branch(packet: &serde_json::Value) -> Result<BranchOutcome, OrchestratorError> {
    let artifact = &packet["caller_repo_branch"];
    let review_packet = &packet["review_packet"];

    Ok(BranchOutcome {
        issue_branch: required_str(
            artifact,
            &["issue_branch"],
            "caller_repo_branch.issue_branch",
        )?
        .to_string(),
        base_branch: required_str(artifact, &["base_branch"], "caller_repo_branch.base_branch")?
            .to_string(),
        branch_action: required_str(
            artifact,
            &["branch_action"],
            "caller_repo_branch.branch_action",
        )?
        .to_string(),
        branch_ready: artifact["branch_ready"].as_bool().unwrap_or(false),
        validation_executed: artifact["validation"]["executed"]
            .as_bool()
            .unwrap_or(false),
        validation_passed: artifact["validation"]["passed"].as_bool().unwrap_or(false),
        changed_files: string_list(artifact, &["changed_files"]),
        review_packet_ready: review_packet["ready"].as_bool().unwrap_or(false),
        review_packet_summary: review_packet["summary"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        readiness_blockers: string_list(review_packet, &["readiness_blockers"]),
    })
}

fn parse_pull_request(packet: &serde_json::Value) -> Result<PullRequestOutcome, OrchestratorError> {
    let step_text = required_str(packet, &["transition", "decision"], "transition.decision")?;
    let next_step =
        NextStep::parse(step_text).ok_or_else(|| OrchestratorError::UnexpectedPacket {
            field: "transition.decision",
            value: step_text.to_string(),
        })?;

    Ok(PullRequestOutcome {
        next_step,
        // These live under `observation` and `grouped_monitor_projection`, not at
        // the top level — one more reason this parsing belongs in one place.
        state: packet["observation"]["state"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        state_bucket: packet["grouped_monitor_projection"]["state_bucket"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        reason: packet["transition"]["reason"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        required_write_scopes: string_list(packet, &["transition", "required_write_scopes"]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feasibility_reads_the_nested_route_and_decision() {
        // The exact shape LoopX returns. Both fields are nested; a top-level read
        // would yield null and, before typing, would have looked like success.
        let packet = serde_json::json!({
            "ok": true,
            "decision": {
                "route": "fix_pr",
                "reason_codes": ["reproduction_confirmed", "validation_surface_named"],
            },
            "transition": {"decision": "runnable_successor"},
            "observation": {
                "repository_context": {
                    "context_status": "grounded",
                    "unresolved_required_aspects": [],
                },
            },
        });

        let outcome = parse_feasibility(&packet).expect("packet parses");
        assert_eq!(outcome.route, FixRoute::FixPr);
        assert!(outcome.route.permits_pull_request());
        assert_eq!(outcome.next_step, NextStep::RunnableSuccessor);
        assert_eq!(outcome.context_grounding, ContextGrounding::Grounded);
        assert_eq!(outcome.reason_codes.len(), 2);
        assert!(outcome.unresolved_aspects.is_empty());
    }

    #[test]
    fn a_triage_route_does_not_permit_a_pull_request() {
        let packet = serde_json::json!({
            "decision": {"route": "triage_only", "reason_codes": ["scope_oversized"]},
            "transition": {"decision": "no_followup"},
            "observation": {
                "repository_context": {
                    "context_status": "partial",
                    "unresolved_required_aspects": ["validation"],
                },
            },
        });

        let outcome = parse_feasibility(&packet).expect("packet parses");
        assert_eq!(outcome.route, FixRoute::TriageOnly);
        assert!(!outcome.route.permits_pull_request());
        assert_eq!(outcome.unresolved_aspects, vec!["validation"]);
    }

    #[test]
    fn a_user_gate_requires_a_human() {
        assert!(NextStep::UserGate.requires_human());
        for step in [
            NextStep::RunnableSuccessor,
            NextStep::MonitorContinuation,
            NextStep::NoFollowup,
        ] {
            assert!(!step.requires_human(), "{step:?} should not gate");
        }
    }

    #[test]
    fn an_unrecognized_route_is_an_error_not_a_default() {
        // Silently defaulting an unknown route could turn a refusal into a PR.
        let packet = serde_json::json!({
            "decision": {"route": "ship_it_immediately"},
            "transition": {"decision": "runnable_successor"},
            "observation": {"repository_context": {"context_status": "grounded"}},
        });

        let error = parse_feasibility(&packet).expect_err("unknown routes are rejected");
        match error {
            OrchestratorError::UnexpectedPacket { field, value } => {
                assert_eq!(field, "decision.route");
                assert_eq!(value, "ship_it_immediately");
            }
            other => panic!("expected UnexpectedPacket, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_route_is_an_error() {
        let packet = serde_json::json!({"ok": true});
        let error = parse_feasibility(&packet).expect_err("a missing route is rejected");
        assert!(matches!(
            error,
            OrchestratorError::UnexpectedPacket {
                field: "decision.route",
                ..
            }
        ));
    }

    #[test]
    fn branch_parsing_reports_dry_run_state() {
        let packet = serde_json::json!({
            "caller_repo_branch": {
                "issue_branch": "codex/issue-1849-fix",
                "base_branch": "main",
                "branch_action": "dry_run",
                "branch_ready": false,
                "validation": {"executed": false, "passed": false},
                "changed_files": [],
            },
            "review_packet": {
                "ready": false,
                "summary": "validation is not PR-ready yet",
                "readiness_blockers": ["validation_not_run"],
            },
        });

        let outcome = parse_branch(&packet).expect("packet parses");
        assert_eq!(outcome.issue_branch, "codex/issue-1849-fix");
        assert_eq!(outcome.branch_action, "dry_run");
        assert!(!outcome.branch_ready);
        assert_eq!(outcome.readiness_blockers, vec!["validation_not_run"]);
    }

    #[test]
    fn opening_a_pull_request_needs_route_packet_and_validation_together() {
        let ready = BranchOutcome {
            issue_branch: "codex/issue-1-fix".to_string(),
            base_branch: "main".to_string(),
            branch_action: "created".to_string(),
            branch_ready: true,
            validation_executed: true,
            validation_passed: true,
            changed_files: vec!["src/a.rs".to_string()],
            review_packet_ready: true,
            review_packet_summary: "ready".to_string(),
            readiness_blockers: Vec::new(),
        };
        assert!(ready.may_open_pull_request(FixRoute::FixPr));

        // Each condition alone must be able to veto.
        assert!(
            !ready.may_open_pull_request(FixRoute::CommentOnly),
            "a non-fix route must veto"
        );
        let mut unvalidated = ready.clone();
        unvalidated.validation_passed = false;
        assert!(
            !unvalidated.may_open_pull_request(FixRoute::FixPr),
            "failing validation must veto"
        );
        let mut unready = ready.clone();
        unready.review_packet_ready = false;
        assert!(
            !unready.may_open_pull_request(FixRoute::FixPr),
            "an unready packet must veto"
        );
    }

    #[test]
    fn pull_request_lifecycle_parses_each_decision() {
        for (decision, expected) in [
            ("runnable_successor", NextStep::RunnableSuccessor),
            ("monitor_continuation", NextStep::MonitorContinuation),
            ("user_gate", NextStep::UserGate),
            ("no_followup", NextStep::NoFollowup),
        ] {
            // The real packet shape: state sits under `observation` and the bucket
            // under `grouped_monitor_projection`, verified against the CLI.
            let packet = serde_json::json!({
                "observation": {"state": "OPEN"},
                "grouped_monitor_projection": {"state_bucket": "review_required"},
                "transition": {
                    "decision": decision,
                    "reason": "a compact reason",
                    "required_write_scopes": ["write"],
                },
            });
            let outcome = parse_pull_request(&packet).expect("packet parses");
            assert_eq!(outcome.next_step, expected, "for {decision}");
            assert_eq!(outcome.state, "OPEN");
            assert_eq!(outcome.state_bucket, "review_required");
            assert_eq!(outcome.required_write_scopes, vec!["write"]);
        }
    }

    #[test]
    fn an_unrecognized_lifecycle_decision_is_an_error() {
        let packet = serde_json::json!({"transition": {"decision": "merge_it_now"}});
        let error = parse_pull_request(&packet).expect_err("unknown decisions are rejected");
        assert!(matches!(
            error,
            OrchestratorError::UnexpectedPacket {
                field: "transition.decision",
                ..
            }
        ));
    }

    #[test]
    fn missing_optional_fields_fall_back_rather_than_failing() {
        // Optional evidence should degrade to empty, unlike the decision fields
        // above, where guessing would be unsafe.
        let packet = serde_json::json!({
            "caller_repo_branch": {
                "issue_branch": "b",
                "base_branch": "main",
                "branch_action": "dry_run",
            },
            "review_packet": {},
        });
        let outcome = parse_branch(&packet).expect("packet parses");
        assert!(!outcome.branch_ready);
        assert!(outcome.changed_files.is_empty());
        assert!(outcome.review_packet_summary.is_empty());
    }
}
