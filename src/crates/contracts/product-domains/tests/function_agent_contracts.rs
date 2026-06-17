#![cfg(feature = "function-agents")]

use bitfun_product_domains::function_agents::{
    git_func_agent::{
        assemble_commit_message, build_changes_summary_from_paths, build_commit_prompt,
        detect_change_patterns, extract_module_name, infer_file_type, parse_commit_analysis_json,
        parse_commit_analysis_value, parse_commit_type_label, prepare_commit_prompt,
        truncate_diff_for_commit_prompt, ChangePattern, CommitFormat, CommitMessageOptions,
        CommitType, FileChange, FileChangeType, ProjectContext,
    },
    ports::{
        CommitAiAnalysisRequest, FunctionAgentAiPort, FunctionAgentFuture, FunctionAgentGitPort,
        FunctionAgentRuntimeFacade, GitCommitSnapshot, StartchatGitSnapshot, StartchatTimeSnapshot,
        WorkStateAiAnalysisRequest,
    },
    startchat_func_agent::{
        build_complete_analysis_prompt, combine_git_diffs, limit_quick_actions,
        normalize_predicted_actions, parse_complete_analysis_json, parse_complete_analysis_value,
        parse_git_status_porcelain, parse_predicted_actions_from_values,
        parse_quick_actions_from_values, time_of_day_for_hour, ActionPriority, AheadBehind,
        GitWorkState, QuickActionType, TimeOfDay, WorkStateOptions,
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

    fn startchat_git_snapshot(
        &self,
        _repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, StartchatGitSnapshot> {
        Box::pin(async {
            Ok(StartchatGitSnapshot {
                current_branch: "main".to_string(),
                status_porcelain: " M src/lib.rs\nA  staged.rs\n".to_string(),
                unstaged_diff: "unstaged".to_string(),
                staged_diff: "staged".to_string(),
                unpushed_commits: 2,
                ahead_behind: Some(AheadBehind {
                    ahead: 1,
                    behind: 0,
                }),
                last_commit_timestamp: Some(900),
            })
        })
    }

    fn startchat_time_snapshot(
        &self,
        _repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, StartchatTimeSnapshot> {
        Box::pin(async {
            Ok(StartchatTimeSnapshot {
                last_commit_timestamp: Some(900),
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
        bitfun_product_domains::function_agents::git_func_agent::AICommitAnalysis,
    > {
        Box::pin(async {
            Ok(
                bitfun_product_domains::function_agents::git_func_agent::AICommitAnalysis {
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

    fn analyze_work_state(
        &self,
        _request: WorkStateAiAnalysisRequest,
    ) -> FunctionAgentFuture<
        '_,
        bitfun_product_domains::function_agents::startchat_func_agent::AIGeneratedAnalysis,
    > {
        Box::pin(async {
            Ok(
                bitfun_product_domains::function_agents::startchat_func_agent::AIGeneratedAnalysis {
                    summary: "stub".to_string(),
                    ongoing_work: Vec::new(),
                    predicted_actions: Vec::new(),
                    quick_actions: Vec::new(),
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

    fn startchat_git_snapshot(
        &self,
        _repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, StartchatGitSnapshot> {
        FunctionAgentPortStub.startchat_git_snapshot(_repo_path)
    }

    fn startchat_time_snapshot(
        &self,
        _repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, StartchatTimeSnapshot> {
        FunctionAgentPortStub.startchat_time_snapshot(_repo_path)
    }
}

struct NoGitStateExpectedPortStub;

impl FunctionAgentGitPort for NoGitStateExpectedPortStub {
    fn git_commit_snapshot(
        &self,
        _repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, GitCommitSnapshot> {
        panic!("git_commit_snapshot should not be called")
    }

    fn startchat_git_snapshot(
        &self,
        _repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, StartchatGitSnapshot> {
        panic!("startchat_git_snapshot should not be called")
    }

    fn startchat_time_snapshot(
        &self,
        _repo_path: PathBuf,
    ) -> FunctionAgentFuture<'_, StartchatTimeSnapshot> {
        Box::pin(async {
            Ok(StartchatTimeSnapshot {
                last_commit_timestamp: Some(900),
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
fn git_function_agent_prompt_helpers_preserve_ai_contract() {
    let options = CommitMessageOptions {
        format: CommitFormat::Angular,
        max_title_length: 64,
        language: Language::English,
        ..CommitMessageOptions::default()
    };
    let context = ProjectContext {
        project_type: "rust-workspace".to_string(),
        tech_stack: vec!["Rust".to_string(), "React".to_string()],
        ..ProjectContext::default()
    };

    let prompt = build_commit_prompt(
        "type={project_type}; stack={tech_stack}; format={format_desc}; lang={language_desc}; max={max_title_length}; diff={diff_content}",
        "diff --git a/lib.rs b/lib.rs",
        &context,
        &options,
    );

    assert_eq!(
        prompt,
        "type=rust-workspace; stack=Rust, React; format=Angular Style; lang=English; max=64; diff=diff --git a/lib.rs b/lib.rs"
    );
    assert_eq!(parse_commit_type_label("feature"), CommitType::Feat);
    assert_eq!(parse_commit_type_label("performance"), CommitType::Perf);
    assert_eq!(parse_commit_type_label("unknown"), CommitType::Chore);
}

#[test]
fn git_function_agent_summary_helpers_preserve_commit_shape() {
    let changed_files = vec![
        "src/crates/assembly/core/lib.rs".to_string(),
        "README.md".to_string(),
    ];
    let summary = build_changes_summary_from_paths(&changed_files, 2, 1);

    assert_eq!(summary.total_additions, 30);
    assert_eq!(summary.total_deletions, 15);
    assert_eq!(summary.files_changed, 2);
    assert_eq!(
        summary.file_changes[0].path,
        "src/crates/assembly/core/lib.rs"
    );
    assert_eq!(summary.file_changes[0].file_type, "rs");
    assert!(summary.affected_modules.contains(&"core".to_string()));
    assert!(summary
        .change_patterns
        .contains(&ChangePattern::DocumentationUpdate));

    let message = assemble_commit_message(
        "feat(core): add boundary helper",
        &Some("Move pure helper to owner crate.".to_string()),
        &Some("BREAKING CHANGE: none".to_string()),
    );
    assert_eq!(
        message,
        "feat(core): add boundary helper\n\nMove pure helper to owner crate.\n\nBREAKING CHANGE: none"
    );

    let title_only = assemble_commit_message("chore: tidy", &Some(String::new()), &None);
    assert_eq!(title_only, "chore: tidy");
}

#[test]
fn git_function_agent_analysis_parser_preserves_defaults_and_required_title() {
    let analysis = parse_commit_analysis_value(&serde_json::json!({
        "type": "feature",
        "scope": "core",
        "title": "feat(core): add helper",
        "body": "Move pure parsing policy.",
        "breaking_changes": "none",
        "confidence": 0.95
    }))
    .expect("valid commit analysis");

    assert_eq!(analysis.commit_type, CommitType::Feat);
    assert_eq!(analysis.scope.as_deref(), Some("core"));
    assert_eq!(analysis.title, "feat(core): add helper");
    assert_eq!(analysis.reasoning, "AI analysis");
    assert!((analysis.confidence - 0.95).abs() < f32::EPSILON);

    let fallback = parse_commit_analysis_value(&serde_json::json!({
        "title": "chore: tidy"
    }))
    .expect("fallback commit analysis");
    assert_eq!(fallback.commit_type, CommitType::Chore);
    assert_eq!(fallback.confidence, 0.8);

    let missing_title = parse_commit_analysis_value(&serde_json::json!({
        "type": "fix"
    }));
    assert_eq!(missing_title.unwrap_err(), "Missing title field");
}

#[test]
fn git_function_agent_diff_truncation_preserves_legacy_marker() {
    let short = "diff --git a/lib.rs b/lib.rs";
    assert_eq!(truncate_diff_for_commit_prompt(short, 50), short);

    let long = "a".repeat(140);
    let truncated = truncate_diff_for_commit_prompt(&long, 120);
    assert!(truncated.starts_with(&"a".repeat(20)));
    assert!(truncated.ends_with("\n\n... [content truncated] ..."));
}

#[test]
fn git_function_agent_commit_prompt_preparation_preserves_truncation_boundary() {
    let context = ProjectContext {
        project_type: "library".to_string(),
        tech_stack: vec!["Rust".to_string(), "Cargo".to_string()],
        project_docs: Some("Use conventional commits.".to_string()),
        code_standards: Some("Keep modules small.".to_string()),
    };
    let options = CommitMessageOptions::default();
    let template = "Project: {project_type}\nStack: {tech_stack}\nDiff: {diff_content}\nLanguage: {language_desc}\n";
    let prepared = prepare_commit_prompt(template, &"x".repeat(140), &context, &options, 120);

    assert!(prepared.truncated);
    assert!(prepared
        .diff_content
        .ends_with("\n\n... [content truncated] ..."));
    assert!(prepared.prompt.contains("Diff: "));
    assert!(prepared.prompt.contains("library"));

    let short = prepare_commit_prompt(template, "short diff", &context, &options, 120);
    assert!(!short.truncated);
    assert_eq!(short.diff_content, "short diff");
}

#[test]
fn startchat_options_preserve_existing_defaults() {
    let options = WorkStateOptions::default();

    assert!(options.analyze_git);
    assert!(options.predict_next_actions);
    assert!(options.include_quick_actions);
    assert_eq!(options.language, Language::English);
}

#[test]
fn startchat_prompt_helpers_preserve_ai_contract() {
    let git_state = Some(GitWorkState {
        current_branch: "main".to_string(),
        unstaged_files: 2,
        staged_files: 1,
        unpushed_commits: 3,
        ahead_behind: None,
        modified_files: Vec::new(),
    });

    let prompt = build_complete_analysis_prompt(
        "{lang_instruction}\n{git_state_section}\n{git_diff_section}",
        &git_state,
        "diff --git a/file b/file",
        &Language::Chinese,
    );

    assert!(prompt.contains("Please respond in Chinese."));
    assert!(prompt.contains("Current branch: main"));
    assert!(prompt.contains("Staged files: 1"));
    assert!(prompt.contains("## Code Changes (Git Diff)"));
}

#[test]
fn startchat_action_helpers_preserve_limits_and_defaults() {
    let predicted = parse_predicted_actions_from_values(&[serde_json::json!({
        "description": "Review changes",
        "priority": "High",
        "icon": "search",
        "is_reminder": true
    })]);
    let predicted = normalize_predicted_actions(predicted);

    assert_eq!(predicted.len(), 3);
    assert_eq!(predicted[0].priority, ActionPriority::High);
    assert!(predicted[0].is_reminder);
    assert_eq!(predicted[1].description, "Continue current development");

    let quick = parse_quick_actions_from_values(&[
        serde_json::json!({"title": "Continue", "command": "/continue", "action_type": "Continue"}),
        serde_json::json!({"title": "Status", "command": "/status", "action_type": "ViewStatus"}),
        serde_json::json!({"title": "Commit", "command": "/commit", "action_type": "Commit"}),
        serde_json::json!({"title": "Visualize", "command": "/visualize", "action_type": "Visualize"}),
        serde_json::json!({"title": "Custom 1", "command": "one"}),
        serde_json::json!({"title": "Custom 2", "command": "two"}),
        serde_json::json!({"title": "Custom 3", "command": "three"}),
    ]);
    let quick = limit_quick_actions(quick);

    assert_eq!(quick.len(), 6);
    assert_eq!(quick[0].action_type, QuickActionType::Continue);
    assert_eq!(quick[1].action_type, QuickActionType::ViewStatus);
    assert_eq!(quick[5].title, "Custom 2");
}

#[test]
fn startchat_complete_analysis_parser_preserves_defaults_and_limits() {
    let parsed = parse_complete_analysis_value(&serde_json::json!({
        "summary": "Working on refactor boundaries.",
        "predicted_actions": [
            {"description": "Review changes", "priority": "High", "icon": "search", "is_reminder": true},
            {"description": "Run tests", "priority": "Medium", "icon": "check"},
            {"description": "Open PR", "priority": "Low", "icon": "git-pull-request"},
            {"description": "Extra", "priority": "Low", "icon": "more"}
        ],
        "quick_actions": [
            {"title": "Continue", "command": "/continue", "action_type": "Continue"},
            {"title": "Status", "command": "/status", "action_type": "ViewStatus"},
            {"title": "Commit", "command": "/commit", "action_type": "Commit"},
            {"title": "Visualize", "command": "/visualize", "action_type": "Visualize"},
            {"title": "Custom 1", "command": "one"},
            {"title": "Custom 2", "command": "two"},
            {"title": "Custom 3", "command": "three"}
        ]
    }));

    assert_eq!(parsed.predicted_actions_count, 4);
    assert_eq!(parsed.quick_actions_count, 7);
    assert_eq!(parsed.analysis.summary, "Working on refactor boundaries.");
    assert_eq!(parsed.analysis.predicted_actions.len(), 3);
    assert_eq!(
        parsed.analysis.predicted_actions[0].priority,
        ActionPriority::High
    );
    assert!(parsed.analysis.predicted_actions[0].is_reminder);
    assert_eq!(parsed.analysis.quick_actions.len(), 6);
    assert_eq!(parsed.analysis.quick_actions[5].title, "Custom 2");

    let fallback = parse_complete_analysis_value(&serde_json::json!({}));
    assert_eq!(fallback.predicted_actions_count, 0);
    assert_eq!(fallback.quick_actions_count, 0);
    assert_eq!(
        fallback.analysis.summary,
        "You were working on development, with multiple files modified."
    );
    assert_eq!(fallback.analysis.predicted_actions.len(), 3);
    assert!(fallback.analysis.quick_actions.is_empty());
}

#[test]
fn startchat_git_status_helpers_preserve_porcelain_contract() {
    let (unstaged, staged, files) = parse_git_status_porcelain(
        " M src/lib.rs\nM  Cargo.toml\n?? README.md\nR  old.rs -> new.rs\n",
    );

    assert_eq!(unstaged, 2);
    assert_eq!(staged, 2);
    assert_eq!(files[0].path, "src/lib.rs");
    assert_eq!(files[0].module.as_deref(), Some("src"));
    assert_eq!(files[1].change_type.to_string(), "Modified");
    assert_eq!(files[2].path, "README.md");
    assert_eq!(files[3].change_type.to_string(), "Renamed");

    assert_eq!(time_of_day_for_hour(4), TimeOfDay::Night);
    assert_eq!(time_of_day_for_hour(9), TimeOfDay::Morning);
    assert_eq!(time_of_day_for_hour(14), TimeOfDay::Afternoon);
    assert_eq!(time_of_day_for_hour(20), TimeOfDay::Evening);

    assert_eq!(combine_git_diffs("unstaged", ""), "unstaged");
    assert_eq!(
        combine_git_diffs("unstaged", "staged"),
        "unstaged\n\n=== Staged Changes ===\n\nstaged"
    );
}

#[test]
fn function_agent_ports_keep_ai_and_git_boundaries_explicit() {
    let commit_request = CommitAiAnalysisRequest {
        diff_content: "diff".to_string(),
        project_context: ProjectContext::default(),
        options: CommitMessageOptions::default(),
    };
    let json = serde_json::to_value(&commit_request).unwrap();
    assert_eq!(json["diffContent"], "diff");
    assert_eq!(json["options"]["maxTitleLength"], 72);

    let work_state_request = WorkStateAiAnalysisRequest {
        git_state: None,
        git_diff: "diff".to_string(),
        language: Language::English,
    };
    let json = serde_json::to_value(&work_state_request).unwrap();
    assert_eq!(json["gitDiff"], "diff");
    assert_eq!(json["language"], "English");

    let port: &dyn FunctionAgentGitPort = &FunctionAgentPortStub;
    let _future = port.git_commit_snapshot(PathBuf::from("."));

    let ai_port: &dyn FunctionAgentAiPort = &FunctionAgentPortStub;
    let _future = ai_port.analyze_work_state(work_state_request);
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
    assert_eq!(message.confidence, 1.0);
    assert_eq!(message.changes_summary.files_changed, 1);
    assert_eq!(message.changes_summary.file_changes[0].path, "src/lib.rs");
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

#[test]
fn function_agent_runtime_facade_builds_work_state_from_ports_without_surface_logic() {
    let ports = FunctionAgentPortStub;
    let facade = FunctionAgentRuntimeFacade::new(&ports, &ports);
    let options = WorkStateOptions {
        predict_next_actions: false,
        include_quick_actions: false,
        ..WorkStateOptions::default()
    };

    let analysis = block_on(facade.analyze_work_state(
        PathBuf::from("repo"),
        options,
        960,
        14,
        "2026-05-19T12:00:00+08:00".to_string(),
    ))
    .unwrap();

    let git_state = analysis.current_state.git_state.unwrap();
    assert_eq!(analysis.current_state.summary, "stub");
    assert_eq!(git_state.current_branch, "main");
    assert_eq!(git_state.unstaged_files, 1);
    assert_eq!(git_state.staged_files, 1);
    assert_eq!(git_state.unpushed_commits, 2);
    assert_eq!(git_state.ahead_behind.unwrap().ahead, 1);
    assert_eq!(
        analysis.current_state.time_info.minutes_since_last_commit,
        Some(1)
    );
    assert_eq!(
        analysis.current_state.time_info.time_of_day,
        TimeOfDay::Afternoon
    );
    assert!(analysis.predicted_actions.is_empty());
    assert!(analysis.quick_actions.is_empty());
    assert_eq!(analysis.analyzed_at, "2026-05-19T12:00:00+08:00");
}

#[test]
fn function_agent_runtime_facade_honors_disabled_git_state_boundary_and_preserves_time_info() {
    let git = NoGitStateExpectedPortStub;
    let ai = FunctionAgentPortStub;
    let facade = FunctionAgentRuntimeFacade::new(&git, &ai);
    let options = WorkStateOptions {
        analyze_git: false,
        predict_next_actions: false,
        include_quick_actions: false,
        ..WorkStateOptions::default()
    };

    let analysis = block_on(facade.analyze_work_state(
        PathBuf::from("repo"),
        options,
        960,
        9,
        "2026-05-19T09:00:00+08:00".to_string(),
    ))
    .unwrap();

    assert_eq!(analysis.current_state.summary, "stub");
    assert!(analysis.current_state.git_state.is_none());
    assert_eq!(
        analysis.current_state.time_info.minutes_since_last_commit,
        Some(1)
    );
    assert_eq!(
        analysis.current_state.time_info.time_of_day,
        TimeOfDay::Morning
    );
    assert!(analysis.predicted_actions.is_empty());
    assert!(analysis.quick_actions.is_empty());
}

#[test]
fn git_function_agent_utils_preserve_change_classification() {
    assert_eq!(infer_file_type("src/main.rs"), "rs");
    assert_eq!(
        extract_module_name("src/crates/assembly/core/lib.rs").as_deref(),
        Some("core")
    );

    let patterns = detect_change_patterns(&[
        FileChange {
            path: "src/lib.rs".to_string(),
            change_type: FileChangeType::Modified,
            additions: 20,
            deletions: 2,
            file_type: "rs".to_string(),
        },
        FileChange {
            path: "README.md".to_string(),
            change_type: FileChangeType::Modified,
            additions: 4,
            deletions: 1,
            file_type: "md".to_string(),
        },
    ]);

    assert!(patterns.contains(&ChangePattern::BugFix));
    assert!(patterns.contains(&ChangePattern::DocumentationUpdate));
}

#[test]
fn function_agent_json_helpers_parse_ai_payloads_without_core_runtime() {
    let commit = parse_commit_analysis_json(
        r#"{
            "type": "refactor",
            "title": "refactor(product-domains): move parse helpers",
            "body": "Keep runtime adapters in core.",
            "confidence": 0.92
        }"#,
    )
    .unwrap();
    assert_eq!(commit.commit_type, CommitType::Refactor);
    assert_eq!(
        commit.title,
        "refactor(product-domains): move parse helpers"
    );
    assert_eq!(
        commit.body.as_deref(),
        Some("Keep runtime adapters in core.")
    );
    assert_eq!(commit.confidence, 0.92);

    let missing_title = parse_commit_analysis_json(r#"{"type":"fix"}"#).unwrap_err();
    assert_eq!(missing_title, "Missing title field");

    let invalid_commit = parse_commit_analysis_json("not json").unwrap_err();
    assert!(invalid_commit.starts_with("Failed to parse AI response:"));

    let work_state = parse_complete_analysis_json(
        r#"{
            "summary": "Working on product-domain owner closure.",
            "predicted_actions": [
                {"description": "Run checks", "priority": "High", "icon": "check", "is_reminder": false}
            ],
            "quick_actions": [
                {"title": "Status", "command": "git status", "icon": "git", "action_type": "ViewStatus"}
            ]
        }"#,
    )
    .unwrap();
    assert_eq!(
        work_state.analysis.summary,
        "Working on product-domain owner closure."
    );
    assert_eq!(work_state.predicted_actions_count, 1);
    assert_eq!(work_state.quick_actions_count, 1);
    assert_eq!(work_state.analysis.predicted_actions.len(), 3);
    assert_eq!(work_state.analysis.quick_actions.len(), 1);

    let invalid_work_state = parse_complete_analysis_json("not json").unwrap_err();
    assert!(invalid_work_state.starts_with("Failed to parse complete analysis response:"));
}
