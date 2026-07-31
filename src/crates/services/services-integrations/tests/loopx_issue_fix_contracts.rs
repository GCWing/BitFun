//! Contract tests for the `loopx` issue-fix bridge.
//!
//! These exercise the real `loopx` CLI when it is installed. When it is not, each
//! test reports a skip rather than failing, so CI hosts without LoopX stay green —
//! the feature is probe-gated at runtime for exactly the same reason.

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

/// A grounded repository context, written to a temp file per call.
///
/// LoopX will not select `fix_pr` without one — an ungrounded request yields
/// `repository_context_not_provided` in its reason codes and falls back to
/// `triage_only`. `reference` values must be repo-relative; LoopX rejects local
/// absolute paths as unsafe to publish.
fn write_repository_context(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("repository-context.json");
    let context = serde_json::json!({
        "schema_version": "issue_fix_repository_context_input_v0",
        "repository_revision": "9ed5c5fec0000000000000000000000000000000",
        "sources": [
            {
                "source_id": "workspace-item-icon",
                "source_kind": "source_code",
                "reference": "src/web-ui/src/app/components/NavPanel/sections/workspaces/WorkspaceItem.tsx",
                "trust": "verified",
                "freshness": "current",
                "supports": ["architecture", "change_scope", "reproduction"],
                "summary": "Icon ternary renders an arrow for the active workspace row and a folder for its siblings."
            },
            {
                "source_id": "workspace-layout-guard",
                "source_kind": "source_code",
                "reference": "src/web-ui/src/app/components/NavPanel/sections/workspaces/WorkspaceListSectionLayout.test.ts",
                "trust": "verified",
                "freshness": "current",
                "supports": ["validation"],
                "summary": "Raw-text layout guard over the workspace component; focused coverage would need adding."
            }
        ]
    });
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

/// Naming a validation surface is not optional: LoopX refuses `fix_pr` when it
/// cannot see how a fix would be checked, even with everything else grounded.
#[tokio::test]
async fn omitting_the_validation_surface_downgrades_to_triage() {
    let Some(loopx) = loopx_or_skip("omitting_the_validation_surface_downgrades_to_triage") else {
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
        "context should still be grounded: {reasons:?}"
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
