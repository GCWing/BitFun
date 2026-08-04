//! Contract tests for the `loopx` issue-fix bridge.
//!
//! These exercise the real `loopx` CLI when it is installed. When it is not, each
//! test reports a skip rather than failing, so CI hosts without LoopX stay green —
//! the feature is probe-gated at runtime for exactly the same reason.

use bitfun_services_integrations::loopx_issue_fix::orchestrator::{
    ContextGrounding, ExecutionMode, FixRoute, IssueFixOrchestrator, IssueFixRequest, NextStep,
    ReproductionStatus, ScopeClass,
};
use bitfun_services_integrations::loopx_issue_fix::repository_context::{
    ContextStatus, Freshness, RepositoryContextBuilder, RepositoryContextSource, SourceKind,
    SupportAspect, Trust,
};
use bitfun_services_integrations::loopx_issue_fix::{LoopxIssueFix, LoopxIssueFixError};

/// Resolve LoopX or explain the skip. Keeps the skip reason in one place.
fn loopx_or_skip(test_name: &str) -> Option<LoopxIssueFix> {
    match LoopxIssueFix::probe() {
        Some(loopx) => Some(loopx),
        None => {
            eprintln!("skipping {test_name}: loopx is not installed on this host");
            None
        }
    }
}

const ISSUE_URL: &str = "https://github.com/GCWing/BitFun/issues/1849";

/// A grounded repository context, built through the real generator.
///
/// LoopX will not select `fix_pr` without one — an ungrounded request yields
/// `repository_context_not_provided` in its reason codes and falls back to
/// `triage_only`. Building this with `RepositoryContextBuilder` rather than a
/// hand-written literal is the point: it proves the generator's own prediction of
/// "grounded" matches what LoopX actually decides.
fn grounded_repository_context(
) -> bitfun_services_integrations::loopx_issue_fix::repository_context::RepositoryContext {
    let mut builder = RepositoryContextBuilder::new()
        .repository_revision("9ed5c5fec0000000000000000000000000000000");
    builder
        .push(RepositoryContextSource {
            source_id: "workspace-item-icon".to_string(),
            source_kind: SourceKind::SourceCode,
            reference:
                "src/web-ui/src/app/components/NavPanel/sections/workspaces/WorkspaceItem.tsx"
                    .to_string(),
            trust: Trust::Verified,
            freshness: Freshness::Current,
            supports: vec![
                SupportAspect::Architecture,
                SupportAspect::ChangeScope,
                SupportAspect::Reproduction,
            ],
            summary: "Icon ternary renders an arrow for the active workspace row and a folder for its siblings."
                .to_string(),
            consultation_state: None,
        })
        .expect("the change-scope source is valid");
    builder
        .push(RepositoryContextSource {
            source_id: "workspace-layout-guard".to_string(),
            source_kind: SourceKind::TestSurface,
            reference:
                "src/web-ui/src/app/components/NavPanel/sections/workspaces/WorkspaceListSectionLayout.test.ts"
                    .to_string(),
            trust: Trust::Verified,
            freshness: Freshness::Current,
            supports: vec![SupportAspect::Validation],
            summary: "Raw-text layout guard over the workspace component; focused coverage would need adding."
                .to_string(),
            consultation_state: None,
        })
        .expect("the validation source is valid");

    // If the generator and LoopX ever disagree about what grounds an aspect, this
    // assertion fails before the subprocess call and localizes the bug here.
    assert_eq!(
        builder.context_status(),
        ContextStatus::Grounded,
        "the generator should predict a grounded context"
    );

    builder.build().expect("context builds")
}

/// The same context, written to a temp file for the raw-CLI tests below.
fn write_repository_context(dir: &std::path::Path) -> std::path::PathBuf {
    let context = grounded_repository_context();
    let path = dir.join("repository-context.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&context).expect("context serializes"),
    )
    .expect("context file is written");
    path
}

/// Read-only feasibility flags. `--no-write-domain-state` keeps LoopX from
/// touching goal state, and every issue-fix projection is write-free by design.
fn feasibility_args<'a>(scope_class: &'a str, context_path: &'a str) -> Vec<&'a str> {
    vec![
        "feasibility",
        "--repo",
        "GCWing/BitFun",
        "--issue-ref",
        "1849",
        "--url",
        ISSUE_URL,
        "--reproduction-status",
        "confirmed",
        "--reproduction-label",
        "workspace-row-icon-branch",
        // Naming a validation surface is mandatory for `fix_pr`; without it LoopX
        // reports `validation_surface_named` as unmet and downgrades to triage.
        "--validation-label",
        "web-ui focused vitest",
        "--repository-context-json",
        context_path,
        "--no-write-domain-state",
        "--scope-class",
        scope_class,
    ]
}

#[test]
fn probe_reports_a_usable_program_path_when_loopx_is_installed() {
    let Some(loopx) = loopx_or_skip("probe_reports_a_usable_program_path_when_loopx_is_installed")
    else {
        return;
    };
    assert!(
        loopx.program().is_file(),
        "probe returned a path that is not a file: {}",
        loopx.program().display()
    );
}

#[tokio::test]
async fn bounded_scope_with_reproduction_selects_the_fix_pr_route() {
    let Some(loopx) = loopx_or_skip("bounded_scope_with_reproduction_selects_the_fix_pr_route")
    else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir is created");
    let context = write_repository_context(dir.path());
    let context = context.to_str().expect("context path is valid UTF-8");

    let packet = loopx
        .issue_fix(feasibility_args("bounded", context))
        .await
        .expect("feasibility projection succeeds");

    assert_eq!(packet["decision"]["route"], "fix_pr");
    assert_eq!(packet["transition"]["decision"], "runnable_successor");
    // The whole integration rests on LoopX never writing; assert it explicitly.
    assert_eq!(packet["external_writes_performed"], false);
    assert_eq!(packet["todo_write_performed"], false);
}

#[tokio::test]
async fn oversized_scope_refuses_to_open_a_pull_request() {
    let Some(loopx) = loopx_or_skip("oversized_scope_refuses_to_open_a_pull_request") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir is created");
    let context = write_repository_context(dir.path());
    let context = context.to_str().expect("context path is valid UTF-8");

    let packet = loopx
        .issue_fix(feasibility_args("oversized", context))
        .await
        .expect("feasibility projection succeeds");

    // Evidence is present and the issue reproduces, yet an oversized change scope
    // must still not produce a PR. This gate is the reason for the integration.
    assert_eq!(packet["decision"]["route"], "triage_only");
    assert_eq!(packet["transition"]["decision"], "no_followup");
}

/// Naming a validation surface via `--validation-label` is mandatory for
/// `fix_pr`. A fully grounded context does not substitute for it: LoopX refuses to
/// open a PR when it cannot see how the fix would be checked.
///
/// Read together with `the_generator_prediction_matches_what_loopx_decides`, which
/// shows the converse — the label without full grounding *is* enough. So the label
/// is the real gate, and context grounding is not.
#[tokio::test]
async fn omitting_the_validation_label_downgrades_to_triage() {
    let Some(loopx) = loopx_or_skip("omitting_the_validation_label_downgrades_to_triage") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir is created");
    let context = write_repository_context(dir.path());
    let context = context.to_str().expect("context path is valid UTF-8");

    let packet = loopx
        .issue_fix([
            "feasibility",
            "--repo",
            "GCWing/BitFun",
            "--issue-ref",
            "1849",
            "--url",
            ISSUE_URL,
            "--reproduction-status",
            "confirmed",
            "--reproduction-label",
            "workspace-row-icon-branch",
            "--repository-context-json",
            context,
            "--no-write-domain-state",
            "--scope-class",
            "bounded",
        ])
        .await
        .expect("feasibility projection succeeds");

    assert_eq!(packet["decision"]["route"], "triage_only");
    let reasons = packet["decision"]["reason_codes"]
        .as_array()
        .expect("reason codes are an array");
    assert!(
        reasons
            .iter()
            .any(|code| code == "repository_context_grounded"),
        "grounding is intact; only the label is missing: {reasons:?}"
    );
}

/// The generator predicts grounding locally so callers can decide what else to
/// read before paying for a subprocess call. That prediction is only useful if it
/// agrees with LoopX, so assert the agreement against the real CLI.
///
/// Note what this does *not* claim: a partial context still permits `fix_pr` as
/// long as `--validation-label` names a validation surface. LoopX distinguishes
/// "which test files did you read" (a context source) from "how will you check
/// this fix" (the label), and only the latter gates the route.
#[tokio::test]
async fn the_generator_prediction_matches_what_loopx_decides() {
    let Some(loopx) = loopx_or_skip("the_generator_prediction_matches_what_loopx_decides") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir is created");

    // A context with no validation source: the generator must call this partial
    // and name validation as the gap.
    let mut builder = RepositoryContextBuilder::new()
        .repository_revision("9ed5c5fec0000000000000000000000000000000");
    builder
        .push(RepositoryContextSource {
            source_id: "scope-only".to_string(),
            source_kind: SourceKind::SourceCode,
            reference:
                "src/web-ui/src/app/components/NavPanel/sections/workspaces/WorkspaceItem.tsx"
                    .to_string(),
            trust: Trust::Verified,
            freshness: Freshness::Current,
            supports: vec![SupportAspect::ChangeScope, SupportAspect::Reproduction],
            summary: "Only change scope and reproduction; nothing covers validation.".to_string(),
            consultation_state: None,
        })
        .expect("the scope source is valid");

    assert_eq!(
        builder.context_status(),
        ContextStatus::Partial,
        "no validation source means partial"
    );
    assert_eq!(
        builder.ungrounded_required_aspects(),
        vec![SupportAspect::Validation]
    );

    let path = dir.path().join("partial-context.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&builder.build().expect("context builds"))
            .expect("context serializes"),
    )
    .expect("context file is written");
    let path = path.to_str().expect("path is valid UTF-8");

    let packet = loopx
        .issue_fix(feasibility_args("bounded", path))
        .await
        .expect("feasibility projection succeeds");

    // LoopX must reach the same verdict the generator predicted, aspect for
    // aspect. This is the assertion that catches drift between the two.
    let context = &packet["observation"]["repository_context"];
    assert_eq!(context["context_status"], "partial");
    assert_eq!(
        context["unresolved_required_aspects"]
            .as_array()
            .expect("unresolved aspects are an array"),
        &vec![serde_json::Value::from("validation")]
    );
    assert_eq!(context["coverage"]["change_scope"]["status"], "grounded");
    assert_eq!(context["coverage"]["reproduction"]["status"], "grounded");
    assert_eq!(context["coverage"]["validation"]["status"], "missing");

    let reasons = packet["decision"]["reason_codes"]
        .as_array()
        .expect("reason codes are an array")
        .iter()
        .filter_map(|code| code.as_str())
        .collect::<Vec<_>>();
    assert!(
        reasons.contains(&"repository_context_partial"),
        "a partial context must be recorded as such: {reasons:?}"
    );
    assert!(
        !reasons.contains(&"repository_context_grounded"),
        "a partial context must not read as grounded: {reasons:?}"
    );
}

#[tokio::test]
async fn in_band_refusal_becomes_a_rejected_error() {
    let Some(loopx) = loopx_or_skip("in_band_refusal_becomes_a_rejected_error") else {
        return;
    };

    // Omitting --reproduction-label makes LoopX refuse. It reports refusals as
    // `{"ok": false, "error": ...}` on stdout *and* exits nonzero, so the bridge
    // must parse stdout first — otherwise the reason is lost behind a bare exit
    // code, which is exactly the bug this test caught.
    let error = loopx
        .issue_fix([
            "feasibility",
            "--repo",
            "GCWing/BitFun",
            "--issue-ref",
            "1849",
            "--url",
            ISSUE_URL,
            "--reproduction-status",
            "confirmed",
            "--scope-class",
            "bounded",
            "--no-write-domain-state",
        ])
        .await
        .expect_err("a missing required label must surface as an error");

    match error {
        LoopxIssueFixError::Rejected(reason) => {
            assert!(
                reason.contains("reproduction_label"),
                "unexpected refusal reason: {reason}"
            );
        }
        other => panic!("expected an in-band refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn an_unknown_subcommand_surfaces_a_nonzero_exit() {
    let Some(loopx) = loopx_or_skip("an_unknown_subcommand_surfaces_a_nonzero_exit") else {
        return;
    };

    let error = loopx
        .issue_fix(["definitely-not-a-subcommand"])
        .await
        .expect_err("an unknown subcommand must fail");

    assert!(
        matches!(error, LoopxIssueFixError::Exit { .. }),
        "expected a nonzero exit, got {error:?}"
    );
}

/// LoopX's own subprocess calls omit `encoding=`, so on a non-UTF-8 locale it
/// decodes `gh` output as the local codepage and dies. The bridge sets
/// `PYTHONUTF8=1` to fix all of its call sites at once; this test proves the
/// fetch path works, which is exactly what fails without it.
#[tokio::test]
async fn fetching_public_metadata_survives_a_non_utf8_host_locale() {
    let Some(loopx) = loopx_or_skip("fetching_public_metadata_survives_a_non_utf8_host_locale")
    else {
        return;
    };

    let result = loopx
        .issue_fix([
            "workflow-plan",
            "--repo",
            "GCWing/BitFun",
            "--issue-ref",
            "1849",
            "--url",
            ISSUE_URL,
            "--fetch-metadata",
        ])
        .await;

    match result {
        Ok(packet) => {
            assert_eq!(packet["external_reads_performed"], true);
            // The issue title is Chinese; reaching this point means no mojibake.
            assert_eq!(packet["issue_signal"]["repo"], "GCWing/BitFun");
        }
        // `gh` may be absent or unauthenticated on a CI host. That is an
        // environment gap, not an encoding regression, so tolerate it — but let
        // any other failure fail the test.
        Err(LoopxIssueFixError::Rejected(reason)) => {
            assert!(
                reason.contains("gh") || reason.contains("metadata fetch"),
                "unexpected refusal while fetching metadata: {reason}"
            );
            eprintln!("tolerating environment gap: {reason}");
        }
        Err(other) => panic!("metadata fetch failed unexpectedly: {other:?}"),
    }
}

/// One issue's request, pointing at the real BitFun repository.
fn issue_request<'a>(
    context: &'a bitfun_services_integrations::loopx_issue_fix::repository_context::RepositoryContext,
    scope_class: ScopeClass,
) -> IssueFixRequest<'a> {
    IssueFixRequest {
        repo: "GCWing/BitFun",
        issue_ref: "1849",
        issue_url: ISSUE_URL,
        context,
        validation_label: "web-ui focused vitest",
        reproduction_label: "workspace-row-icon-branch",
        reproduction_status: ReproductionStatus::Confirmed,
        scope_class,
        base_branch: "main",
    }
}

/// The orchestrator's whole reason to exist: reading LoopX's nested JSON without
/// mistaking a refusal for approval. This drives the real CLI end to end.
#[tokio::test]
async fn the_orchestrator_plans_a_bounded_issue_as_a_fix() {
    let Some(loopx) = loopx_or_skip("the_orchestrator_plans_a_bounded_issue_as_a_fix") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir is created");
    let context = grounded_repository_context();
    let request = issue_request(&context, ScopeClass::Bounded);

    let outcome = IssueFixOrchestrator::new(&loopx)
        .plan_issue(
            &request,
            env!("CARGO_MANIFEST_DIR"),
            dir.path(),
            // Dry run: nothing may touch the working tree in a test.
            ExecutionMode::DryRun,
        )
        .await
        .expect("planning succeeds");

    assert_eq!(outcome.issue_ref, "1849");
    assert_eq!(outcome.feasibility.route, FixRoute::FixPr);
    assert_eq!(outcome.feasibility.next_step, NextStep::RunnableSuccessor);
    assert_eq!(
        outcome.feasibility.context_grounding,
        ContextGrounding::Grounded
    );
    assert!(!outcome.feasibility.reason_codes.is_empty());

    let branch = outcome.branch.expect("a fix route prepares a branch");
    assert_eq!(branch.issue_branch, "codex/issue-1849-fix");
    assert_eq!(branch.base_branch, "main");
    assert_eq!(branch.branch_action, "dry_run");
    assert!(!branch.branch_ready, "a dry run creates nothing");
    assert!(!branch.validation_executed);
    // The PR gate must stay shut: a dry run has neither validation nor evidence.
    assert!(
        !branch.may_open_pull_request(outcome.feasibility.route),
        "a dry run must never permit a pull request"
    );
}

/// An oversized scope must not even reach branch preparation. Under
/// `ExecutionMode::Execute` that would create a branch LoopX just declined to
/// justify, so the skip is a safety property, not an optimization.
#[tokio::test]
async fn the_orchestrator_skips_the_branch_on_a_triage_route() {
    let Some(loopx) = loopx_or_skip("the_orchestrator_skips_the_branch_on_a_triage_route") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir is created");
    let context = grounded_repository_context();
    let request = issue_request(&context, ScopeClass::Oversized);

    let outcome = IssueFixOrchestrator::new(&loopx)
        .plan_issue(
            &request,
            env!("CARGO_MANIFEST_DIR"),
            dir.path(),
            ExecutionMode::DryRun,
        )
        .await
        .expect("planning succeeds");

    assert_eq!(outcome.feasibility.route, FixRoute::TriageOnly);
    assert_eq!(outcome.feasibility.next_step, NextStep::NoFollowup);
    assert!(
        outcome.branch.is_none(),
        "a declined route must not prepare a branch"
    );
}

/// LoopX raises `user_gate` for semantic ambiguity and missing write authority.
/// The orchestrator must surface it as a distinct step a caller cannot cross.
#[tokio::test]
async fn the_orchestrator_surfaces_a_user_gate_from_a_pull_request() {
    let Some(loopx) = loopx_or_skip("the_orchestrator_surfaces_a_user_gate_from_a_pull_request")
    else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir is created");

    let metadata = dir.path().join("pr.json");
    std::fs::write(
        &metadata,
        serde_json::json!({
            "number": 9999,
            "state": "OPEN",
            "isDraft": false,
            "mergeable": "MERGEABLE",
            "reviewDecision": "",
            "statusCheckRollup": [{"state": "SUCCESS"}],
        })
        .to_string(),
    )
    .expect("metadata is written");

    let correction = dir.path().join("correction.json");
    std::fs::write(
        &correction,
        serde_json::json!({
            "schema_version": "issue_fix_maintainer_correction_input_v0",
            "correction_kind": "semantic_ambiguity",
            "source_kind": "maintainer_comment",
            "source_ref": "GCWing/BitFun:issues/1849#comment",
            "summary": "maintainer suggests highlighting the session instead of the workspace",
            "user_question": "Should the arrow be removed or replaced with a check glyph?",
        })
        .to_string(),
    )
    .expect("correction is written");

    // Raw call: the orchestrator's lifecycle method does not take a correction,
    // so drive the CLI directly and assert the decision the orchestrator would
    // then have to classify.
    let packet = loopx
        .issue_fix([
            "pr-lifecycle",
            "--repo",
            "GCWing/BitFun",
            "--pr-ref",
            "9999",
            "--issue-ref",
            "1849",
            "--metadata-json",
            metadata.to_str().expect("path is UTF-8"),
            "--maintainer-correction-json",
            correction.to_str().expect("path is UTF-8"),
            "--no-write-domain-state",
        ])
        .await
        .expect("lifecycle projection succeeds");

    assert_eq!(packet["transition"]["decision"], "user_gate");
    assert_eq!(packet["transition"]["role"], "user");
    assert!(
        NextStep::UserGate.requires_human(),
        "the orchestrator must treat this as a human gate"
    );
}

/// The lifecycle method against a mocked PR state, through the typed path.
#[tokio::test]
async fn the_orchestrator_projects_a_merged_pull_request_as_terminal() {
    let Some(loopx) = loopx_or_skip("the_orchestrator_projects_a_merged_pull_request_as_terminal")
    else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir is created");
    let metadata = dir.path().join("merged.json");
    std::fs::write(
        &metadata,
        serde_json::json!({
            "number": 9999,
            "state": "MERGED",
            "isDraft": false,
            "reviewDecision": "APPROVED",
            "statusCheckRollup": [{"state": "SUCCESS"}],
        })
        .to_string(),
    )
    .expect("metadata is written");

    let outcome = IssueFixOrchestrator::new(&loopx)
        .pull_request_lifecycle(
            "GCWing/BitFun",
            "9999",
            "1849",
            Some(metadata.to_str().expect("path is UTF-8")),
        )
        .await
        .expect("lifecycle projection succeeds");

    assert_eq!(outcome.next_step, NextStep::NoFollowup);
    assert_eq!(outcome.state, "MERGED");
    assert_eq!(outcome.state_bucket, "terminal");
    assert!(!outcome.next_step.requires_human());
}
