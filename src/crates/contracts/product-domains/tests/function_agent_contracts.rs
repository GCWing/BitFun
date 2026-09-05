#![cfg(feature = "function-agents")]

use openbitfun_product_domains::function_agents::{
    git_func_agent::{
        assemble_commit_message, build_changes_summary_from_paths, parse_commit_analysis_json,
        CommitFormat, CommitMessageOptions, CommitType, ProjectContext,
    },
    ports::{
        CommitAiAnalysisRequest, FunctionAgentAiPort, FunctionAgentFuture, FunctionAgentGitPort,
        FunctionAgentRuntimeFacade, GitCommitSnapshot,
    },
    AgentErrorType, Language,
};
use std::future::Future;
use std::path::PathBuf;
use std::pin::pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

struct FunctionAgentPortStub;

impl FunctionAgentGitPort for FunctionAgentPortStub {
    fn git_commit_snapshot(
        &self,
        _repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, GitCommitSnapshot> {
        Box::pin(async {
            Ok(GitCommitSnapshot {
                staged_paths: vec!["src/lib.rs".to_string()],
                staged_count: 1,
                unstaged_count: 0,
                diff_content: "diff".to_string(),
                project_context: ProjectContext::default(),
            })
        })
    }
}

impl FunctionAgentAiPort for FunctionAgentPortStub {
    fn analyze_commit(
        &self,
        _request: CommitAiAnalysisRequest,
    ) -> FunctionAgentFuture<
        '_,
        openbitfun_product_domains::function_agents::git_func_agent::AICommitAnalysis,
    > {
        Box::pin(async {
            Ok(
                openbitfun_product_domains::function_agents::git_func_agent::AICommitAnalysis {
                    commit_type: CommitType::Chore,
                    scope: None,
                    title: "chore: test".to_string(),
                    body: None,
                    breaking_changes: None,
                    reasoning: "stub".to_string(),
                    confidence: 1.0,
                },
            )
        })
    }
}

struct EmptyCommitPortStub;

impl FunctionAgentGitPort for EmptyCommitPortStub {
    fn git_commit_snapshot(
        &self,
        _repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, GitCommitSnapshot> {
        Box::pin(async {
            Ok(GitCommitSnapshot {
                staged_paths: Vec::new(),
                staged_count: 0,
                unstaged_count: 1,
                diff_content: String::new(),
                project_context: ProjectContext::default(),
            })
        })
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = noop_waker();
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match Future::poll(future.as_mut(), &mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn noop_waker() -> Waker {
    unsafe fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    unsafe fn wake(_: *const ()) {}
    unsafe fn wake_by_ref(_: *const ()) {}
    unsafe fn drop(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

#[test]
fn git_commit_options_preserve_existing_defaults() {
    let options = CommitMessageOptions::default();
    assert_eq!(options.format, CommitFormat::Conventional);
    assert!(options.include_files);
    assert!(options.include_body);
    assert_eq!(options.max_title_length, 72);
    assert_eq!(options.language, Language::Chinese);
}

#[test]
fn git_commit_helpers_preserve_contract() {
    let analysis = parse_commit_analysis_json(
        r#"{"type":"feat","scope":"core","title":"feat(core): add task models","confidence":0.9}"#,
    )
    .unwrap();
    assert_eq!(analysis.commit_type, CommitType::Feat);
    assert_eq!(analysis.scope.as_deref(), Some("core"));
    assert_eq!(analysis.title, "feat(core): add task models");
    assert_eq!(
        assemble_commit_message("title", &Some("body".into()), &None),
        "title\n\nbody"
    );
    let summary = build_changes_summary_from_paths(&["src/lib.rs".into()], 1, 0);
    assert_eq!(summary.files_changed, 1);
}

#[test]
fn function_agent_runtime_facade_generates_commit_message_from_ports() {
    let ports = FunctionAgentPortStub;
    let facade = FunctionAgentRuntimeFacade::new(&ports, &ports);
    let message = block_on(
        facade.generate_commit_message(PathBuf::from("repo"), CommitMessageOptions::default()),
    )
    .unwrap();
    assert_eq!(message.title, "chore: test");
    assert_eq!(message.full_message, "chore: test");
    assert_eq!(message.commit_type, CommitType::Chore);
    assert_eq!(message.changes_summary.files_changed, 1);
}

#[test]
fn function_agent_runtime_facade_preserves_empty_staging_error() {
    let git = EmptyCommitPortStub;
    let ai = FunctionAgentPortStub;
    let facade = FunctionAgentRuntimeFacade::new(&git, &ai);
    let error = block_on(
        facade.generate_commit_message(PathBuf::from("repo"), CommitMessageOptions::default()),
    )
    .unwrap_err();
    assert_eq!(error.error_type, AgentErrorType::InvalidInput);
    assert_eq!(
        error.message,
        "Staging area is empty, please stage files first"
    );
}
