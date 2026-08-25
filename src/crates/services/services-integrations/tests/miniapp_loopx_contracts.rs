use async_trait::async_trait;
use bitfun_product_domains::miniapp::loopx::{
    LoopxCliCallContext, LoopxCliCreateGoalRequest, LoopxCliErrorKind, LoopxCliGoalContext,
    LoopxCliHandshakeRequest, LoopxCliIntakePlan, LoopxCliPlanItemRequest, LoopxCliPort,
    LoopxCliProgress, LoopxCliProgressSink, LoopxCliSource, LoopxCliTodoPlan, LoopxIssueKey,
    LoopxItemKind, LoopxPermissionScope, LoopxRepositoryKey, LoopxWorkspacePort,
    LoopxWorkspacePrepareRequest,
};
use bitfun_services_integrations::miniapp::loopx_cli::{
    LoopxCliAdapterConfig, LoopxCliProcessAdapter, LoopxCommandPlan, LoopxCommandSource,
    LoopxFixedCommandLocator, LoopxProcessError, LoopxProcessObserver, LoopxProcessOutput,
    LoopxProcessProgress, LoopxProcessRunner, LoopxProgressStage, LoopxSystemFallbackPolicy,
    NoopLoopxProcessObserver, SystemLoopxProcessRunner, LOOPX_COMMAND_REFERENCE_SCHEMA,
};
use bitfun_services_integrations::miniapp::loopx_workspace::{
    canonical_github_remote, plan_git_clone_command, plan_workspace_layout, LoopxWorkspaceService,
    LoopxWorkspaceServiceConfig,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct RecordingProgressSink(Mutex<Vec<LoopxCliProgress>>);

impl LoopxCliProgressSink for RecordingProgressSink {
    fn report(&self, progress: LoopxCliProgress) {
        self.0.lock().unwrap().push(progress);
    }
}

#[derive(Default)]
struct RecordingProcessObserver(Mutex<Vec<LoopxProcessProgress>>);

impl LoopxProcessObserver for RecordingProcessObserver {
    fn on_progress(&self, progress: LoopxProcessProgress) {
        self.0.lock().unwrap().push(progress);
    }
}

#[derive(Default)]
struct FakeRunner {
    results: Mutex<VecDeque<Result<LoopxProcessOutput, LoopxProcessError>>>,
    plans: Mutex<Vec<LoopxCommandPlan>>,
}

impl FakeRunner {
    fn with_results(
        results: impl IntoIterator<Item = Result<LoopxProcessOutput, LoopxProcessError>>,
    ) -> Self {
        Self {
            results: Mutex::new(results.into_iter().collect()),
            plans: Mutex::new(Vec::new()),
        }
    }

    fn plans(&self) -> Vec<LoopxCommandPlan> {
        self.plans.lock().unwrap().clone()
    }
}

#[async_trait]
impl LoopxProcessRunner for FakeRunner {
    async fn run(
        &self,
        plan: LoopxCommandPlan,
        _cancellation: CancellationToken,
        _observer: &dyn LoopxProcessObserver,
    ) -> Result<LoopxProcessOutput, LoopxProcessError> {
        self.plans.lock().unwrap().push(plan);
        self.results
            .lock()
            .unwrap()
            .pop_front()
            .expect("fake process result")
    }
}

#[derive(Default)]
struct WorkspaceFakeRunner {
    plans: Mutex<Vec<LoopxCommandPlan>>,
    remote: Mutex<String>,
}

impl WorkspaceFakeRunner {
    fn new(remote: &str) -> Self {
        Self {
            plans: Mutex::new(Vec::new()),
            remote: Mutex::new(remote.to_string()),
        }
    }
}

#[async_trait]
impl LoopxProcessRunner for WorkspaceFakeRunner {
    async fn run(
        &self,
        plan: LoopxCommandPlan,
        _cancellation: CancellationToken,
        _observer: &dyn LoopxProcessObserver,
    ) -> Result<LoopxProcessOutput, LoopxProcessError> {
        if plan.args.first() == Some(&OsString::from("clone")) {
            let target = PathBuf::from(plan.args.last().expect("clone target"));
            std::fs::create_dir_all(target.join(".git")).unwrap();
        }
        let stdout = if plan
            .args
            .windows(3)
            .any(|args| args == ["config", "--get", "remote.origin.url"])
        {
            format!("{}\n", self.remote.lock().unwrap())
        } else {
            String::new()
        };
        self.plans.lock().unwrap().push(plan);
        Ok(LoopxProcessOutput {
            stdout,
            stderr_tail: Vec::new(),
            elapsed: Duration::from_millis(1),
        })
    }
}

struct FakeLocator {
    path: Option<PathBuf>,
    calls: AtomicUsize,
}

impl FakeLocator {
    fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            calls: AtomicUsize::new(0),
        }
    }
}

impl LoopxFixedCommandLocator for FakeLocator {
    fn locate(&self) -> Result<Option<PathBuf>, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.path.clone())
    }
}

fn output(stdout: impl Into<String>) -> Result<LoopxProcessOutput, LoopxProcessError> {
    Ok(LoopxProcessOutput {
        stdout: stdout.into(),
        stderr_tail: Vec::new(),
        elapsed: Duration::from_millis(1),
    })
}

fn stage_bundle(root: &Path, version: &str, schema: u32) -> PathBuf {
    let bundle = root.join("loopx");
    std::fs::create_dir_all(&bundle).unwrap();
    let executable = bundle.join(if cfg!(windows) { "loopx.exe" } else { "loopx" });
    let bytes = b"test-only-loopx-binary";
    std::fs::write(&executable, bytes).unwrap();
    let digest = hex::encode(Sha256::digest(bytes));
    std::fs::write(
        bundle.join("manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": schema,
            "name": "loopx",
            "version": version,
            "sha256": format!("sha256:{digest}"),
        }))
        .unwrap(),
    )
    .unwrap();
    executable
}

fn handshake_results(
    version: &str,
    schema: &str,
) -> Vec<Result<LoopxProcessOutput, LoopxProcessError>> {
    vec![
        output(format!("{version}\n")),
        output(json!({"ok": true, "schema_version": schema}).to_string()),
    ]
}

fn adapter_with_runner(
    resource_dir: &Path,
    runner: Arc<FakeRunner>,
    locator: Arc<FakeLocator>,
) -> LoopxCliProcessAdapter {
    let mut config = LoopxCliAdapterConfig::packaged(resource_dir);
    config.system_fallback = LoopxSystemFallbackPolicy::ExactPinned;
    config.startup_deadline = Duration::from_secs(3);
    config.command_deadline = Duration::from_secs(9);
    LoopxCliProcessAdapter::with_dependencies(
        config,
        runner,
        locator,
        Arc::new(NoopLoopxProcessObserver),
    )
}

fn handshake_request(operation_id: &str) -> LoopxCliHandshakeRequest {
    LoopxCliHandshakeRequest {
        call: LoopxCliCallContext {
            operation_id: operation_id.to_string(),
            deadline_at: None,
        },
        ..LoopxCliHandshakeRequest::default()
    }
}

#[test]
fn packaged_startup_budget_covers_measured_windows_onefile_cold_start() {
    let config = LoopxCliAdapterConfig::packaged(PathBuf::from("resources"));
    assert!(config.startup_deadline >= Duration::from_secs(60));
    assert_eq!(config.command_deadline, Duration::from_secs(180));
}

#[tokio::test]
async fn packaged_bundle_is_preferred_and_exactly_handshaken() {
    let temporary = tempfile::tempdir().unwrap();
    let bundled = stage_bundle(temporary.path(), "v0.2.13", 1);
    let runner = Arc::new(FakeRunner::with_results(handshake_results(
        "loopx 0.2.13",
        LOOPX_COMMAND_REFERENCE_SCHEMA,
    )));
    let locator = Arc::new(FakeLocator::new(Some(PathBuf::from("system-loopx"))));
    let adapter = adapter_with_runner(temporary.path(), runner.clone(), locator.clone());

    let manifest = adapter
        .handshake(
            handshake_request("handshake-bundle"),
            &RecordingProgressSink::default(),
        )
        .await
        .unwrap();

    assert_eq!(manifest.executable.source, LoopxCliSource::Bundled);
    assert_eq!(manifest.loopx_version, "0.2.13");
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(
        manifest.executable.path.as_deref(),
        Some(bundled.to_string_lossy().as_ref())
    );
    assert!(manifest
        .executable
        .sha256
        .as_deref()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(locator.calls.load(Ordering::SeqCst), 0);
    let plans = runner.plans();
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0].executable, bundled);
    assert_eq!(plans[0].args, vec![OsString::from("--version")]);
    assert_eq!(
        plans[1].args,
        ["--format", "json", "commands"]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn runtime_version_mismatch_is_a_non_retryable_typed_error() {
    let temporary = tempfile::tempdir().unwrap();
    stage_bundle(temporary.path(), "v0.2.13", 1);
    let runner = Arc::new(FakeRunner::with_results([output("loopx 0.2.12\n")]));
    let adapter = adapter_with_runner(temporary.path(), runner, Arc::new(FakeLocator::new(None)));

    let error = adapter
        .handshake(
            handshake_request("handshake-version"),
            &RecordingProgressSink::default(),
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, LoopxCliErrorKind::VersionMismatch);
    assert!(!error.retryable);
}

#[tokio::test]
async fn command_reference_schema_mismatch_is_rejected() {
    let temporary = tempfile::tempdir().unwrap();
    stage_bundle(temporary.path(), "v0.2.13", 1);
    let runner = Arc::new(FakeRunner::with_results(handshake_results(
        "loopx 0.2.13",
        "future_schema_v99",
    )));
    let adapter = adapter_with_runner(temporary.path(), runner, Arc::new(FakeLocator::new(None)));

    let error = adapter
        .handshake(
            handshake_request("handshake-schema"),
            &RecordingProgressSink::default(),
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, LoopxCliErrorKind::SchemaMismatch);
}

#[tokio::test]
async fn item_plan_uses_structured_registry_and_worktree_arguments() {
    let temporary = tempfile::tempdir().unwrap();
    stage_bundle(temporary.path(), "v0.2.13", 1);
    let worktree = temporary.path().join("worktree");
    std::fs::create_dir_all(&worktree).unwrap();
    let registry = worktree.join(".loopx").join("registry.json");
    let workflow = json!({
        "ok": true,
        "schema_version": "issue_fix_workflow_plan_packet_v0",
        "objective": "Fix issue 42",
        "ordered_loopx_todo_writeback_preview": [{
            "role": "agent",
            "task_class": "advancement_task",
            "action_kind": "fix_issue",
            "text": "[P1] Fix issue #42"
        }]
    });
    let runner = Arc::new(FakeRunner::with_results(
        handshake_results("loopx 0.2.13", LOOPX_COMMAND_REFERENCE_SCHEMA)
            .into_iter()
            .chain([output(workflow.to_string())]),
    ));
    let adapter = adapter_with_runner(
        temporary.path(),
        runner.clone(),
        Arc::new(FakeLocator::new(None)),
    );
    let item = LoopxIssueKey {
        repository: LoopxRepositoryKey {
            host: "github.com".to_string(),
            owner: "owner".to_string(),
            repository: "repo".to_string(),
        },
        kind: LoopxItemKind::Issue,
        number: 42,
    };
    let request = LoopxCliPlanItemRequest {
        context: LoopxCliGoalContext {
            call: LoopxCliCallContext {
                operation_id: "plan-item".to_string(),
                deadline_at: None,
            },
            task_id: "task-42".to_string(),
            generation: 1,
            worktree_path: worktree.to_string_lossy().into_owned(),
            registry_path: registry.to_string_lossy().into_owned(),
        },
        item,
    };

    let plan = adapter
        .plan_item(request, &RecordingProgressSink::default())
        .await
        .unwrap();

    assert_eq!(plan.objective, "Fix issue 42");
    assert_eq!(plan.todos.len(), 1);
    let command = runner.plans().pop().unwrap();
    assert_eq!(command.current_dir.as_deref(), Some(worktree.as_path()));
    assert!(command.environment.is_empty());
    assert_eq!(command.deadline, Duration::from_secs(9));
    assert_eq!(
        command.args,
        vec![
            OsString::from("--format"),
            OsString::from("json"),
            OsString::from("--registry"),
            registry.as_os_str().to_owned(),
            OsString::from("issue-fix"),
            OsString::from("workflow-plan"),
            OsString::from("--url"),
            OsString::from("https://github.com/owner/repo/issues/42"),
            OsString::from("--repo-path"),
            worktree.as_os_str().to_owned(),
            OsString::from("--fetch-metadata"),
            OsString::from("--fetch-timeout-seconds"),
            OsString::from("20"),
        ]
    );
}

#[tokio::test]
async fn create_goal_recovery_does_not_duplicate_an_existing_planned_todo() {
    let temporary = tempfile::tempdir().unwrap();
    stage_bundle(temporary.path(), "v0.2.13", 1);
    let worktree = temporary.path().join("worktree");
    std::fs::create_dir_all(&worktree).unwrap();
    let registry = worktree.join(".loopx").join("registry.json");
    let planned_todo = LoopxCliTodoPlan {
        role: "agent".to_string(),
        task_class: "advancement_task".to_string(),
        action_kind: Some("fix_issue".to_string()),
        text: "[P1] Fix issue #42".to_string(),
    };
    let results = handshake_results("loopx 0.2.13", LOOPX_COMMAND_REFERENCE_SCHEMA)
        .into_iter()
        .chain([
            output(json!({"ok": true, "state_action": "kept"}).to_string()),
            output(json!({"ok": true}).to_string()),
            output(
                json!({
                    "ok": true,
                    "todos": [{
                        "role": planned_todo.role.clone(),
                        "task_class": planned_todo.task_class.clone(),
                        "action_kind": planned_todo.action_kind.clone(),
                        "text": planned_todo.text.clone(),
                    }]
                })
                .to_string(),
            ),
            output(
                json!({
                    "ok": true,
                    "schema_version": "loopx_turn_plan_v0",
                    "turn_envelope": {
                        "action_signature": {"source_decision_hash": "sha256:durable-revision"}
                    },
                    "transaction": {"turn_key": "sha256:turn"}
                })
                .to_string(),
            ),
        ]);
    let runner = Arc::new(FakeRunner::with_results(results));
    let adapter = adapter_with_runner(
        temporary.path(),
        runner.clone(),
        Arc::new(FakeLocator::new(None)),
    );
    let item = LoopxIssueKey {
        repository: LoopxRepositoryKey {
            host: "github.com".to_string(),
            owner: "owner".to_string(),
            repository: "repo".to_string(),
        },
        kind: LoopxItemKind::Issue,
        number: 42,
    };
    let request = LoopxCliCreateGoalRequest {
        context: LoopxCliGoalContext {
            call: LoopxCliCallContext {
                operation_id: "create-recovery".to_string(),
                deadline_at: None,
            },
            task_id: "task-42".to_string(),
            generation: 2,
            worktree_path: worktree.to_string_lossy().into_owned(),
            registry_path: registry.to_string_lossy().into_owned(),
        },
        goal_id: "goal-42".to_string(),
        agent_id: "bitfun-agent".to_string(),
        intake: LoopxCliIntakePlan {
            item,
            objective: "Fix issue 42".to_string(),
            todos: vec![planned_todo],
        },
        granted_scopes: vec![LoopxPermissionScope::WorkspaceWrite],
    };

    let result = adapter
        .create_goal(request, &RecordingProgressSink::default())
        .await
        .unwrap();

    assert!(!result.created);
    assert_eq!(result.durable_revision, "sha256:durable-revision");
    let commands = runner
        .plans()
        .into_iter()
        .skip(2)
        .map(|plan| plan.args)
        .collect::<Vec<_>>();
    assert_eq!(commands.len(), 4);
    assert!(!commands.iter().any(|args| {
        args.windows(2)
            .any(|pair| pair[0] == OsString::from("todo") && pair[1] == OsString::from("add"))
    }));
}

#[test]
fn workspace_plan_uses_hashed_paths_and_noninteractive_structured_clone() {
    let root = if cfg!(windows) {
        PathBuf::from(r"C:\managed-loopx")
    } else {
        PathBuf::from("/managed-loopx")
    };
    let item = LoopxIssueKey {
        repository: LoopxRepositoryKey {
            host: "github.com".to_string(),
            owner: "Owner".to_string(),
            repository: "Repo".to_string(),
        },
        kind: LoopxItemKind::Issue,
        number: 42,
    };

    let layout = plan_workspace_layout(&root, "task-sensitive-name", &item).unwrap();
    let command = plan_git_clone_command(Path::new("git"), &layout);

    assert!(layout.worktree_path.starts_with(&root));
    assert!(!layout.worktree_path.to_string_lossy().contains("Owner"));
    assert!(layout
        .registry_path
        .ends_with(Path::new(".loopx/registry.json")));
    assert_eq!(command.args[0], OsString::from("clone"));
    assert_eq!(command.args[1], OsString::from("--no-checkout"));
    assert!(command.args.contains(&OsString::from("--")));
    assert_eq!(
        command
            .environment
            .get(&OsString::from("GIT_TERMINAL_PROMPT")),
        Some(&OsString::from("0"))
    );
    assert_eq!(
        canonical_github_remote("git@github.com:OWNER/Repo.git").as_deref(),
        Some("github.com/owner/repo")
    );
}

#[tokio::test]
async fn workspace_prepare_clones_once_then_reuses_verified_marker_and_origin() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("loopx-workspaces");
    let runner = Arc::new(WorkspaceFakeRunner::new(
        "https://github.com/owner/repo.git",
    ));
    let mut config = LoopxWorkspaceServiceConfig::new(&root, "git");
    config.clone_deadline = Duration::from_secs(7);
    config.command_deadline = Duration::from_secs(3);
    let service = LoopxWorkspaceService::with_runner(
        config,
        runner.clone(),
        Arc::new(NoopLoopxProcessObserver),
    );
    let item = LoopxIssueKey {
        repository: LoopxRepositoryKey {
            host: "github.com".to_string(),
            owner: "owner".to_string(),
            repository: "repo".to_string(),
        },
        kind: LoopxItemKind::Issue,
        number: 42,
    };
    let request = LoopxWorkspacePrepareRequest {
        operation_id: "workspace-first".to_string(),
        task_id: "task-42".to_string(),
        item: item.clone(),
    };

    let created = service.prepare(request).await.unwrap();
    let reused = service
        .prepare(LoopxWorkspacePrepareRequest {
            operation_id: "workspace-second".to_string(),
            task_id: "task-42".to_string(),
            item,
        })
        .await
        .unwrap();

    assert!(!created.reused);
    assert!(created.repository_verified);
    assert!(reused.reused);
    assert_eq!(created.worktree_path, reused.worktree_path);
    assert!(Path::new(&created.registry_path).ends_with(Path::new(".loopx/registry.json")));
    let plans = runner.plans.lock().unwrap();
    assert_eq!(plans.len(), 4);
    assert_eq!(plans[0].deadline, Duration::from_secs(7));
    assert_eq!(plans[1].deadline, Duration::from_secs(3));
    assert_eq!(plans[2].deadline, Duration::from_secs(3));
    assert_eq!(plans[3].deadline, Duration::from_secs(3));
}

#[test]
fn managed_process_fixture() {
    match std::env::var("BITFUN_LOOPX_PROCESS_FIXTURE").as_deref() {
        Ok("exit") => std::process::exit(23),
        Ok("sleep") => std::thread::sleep(Duration::from_secs(60)),
        Ok("stderr") => eprintln!("fixture progress line"),
        _ => {}
    }
}

fn fixture_plan(kind: &str, deadline: Duration) -> LoopxCommandPlan {
    let mut environment = BTreeMap::new();
    environment.insert(
        OsString::from("BITFUN_LOOPX_PROCESS_FIXTURE"),
        OsString::from(kind),
    );
    LoopxCommandPlan {
        operation_id: format!("fixture-{kind}"),
        executable: std::env::current_exe().unwrap(),
        args: ["--exact", "managed_process_fixture", "--nocapture"]
            .into_iter()
            .map(OsString::from)
            .collect(),
        current_dir: None,
        environment,
        deadline,
        terminate_grace: Duration::from_millis(50),
    }
}

#[tokio::test]
async fn child_exit_is_reported_immediately() {
    let started = Instant::now();
    let error = SystemLoopxProcessRunner
        .run(
            fixture_plan("exit", Duration::from_secs(5)),
            CancellationToken::new(),
            &NoopLoopxProcessObserver,
        )
        .await
        .unwrap_err();

    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(matches!(
        error,
        LoopxProcessError::Exited { code: Some(23), .. }
    ));
}

#[tokio::test]
async fn deadline_terminates_the_managed_process_tree() {
    let started = Instant::now();
    let error = SystemLoopxProcessRunner
        .run(
            fixture_plan("sleep", Duration::from_millis(50)),
            CancellationToken::new(),
            &NoopLoopxProcessObserver,
        )
        .await
        .unwrap_err();

    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(matches!(error, LoopxProcessError::Timeout { .. }));
}

#[tokio::test]
async fn stderr_is_streamed_as_progress_before_success() {
    let observer = RecordingProcessObserver::default();
    SystemLoopxProcessRunner
        .run(
            fixture_plan("stderr", Duration::from_secs(5)),
            CancellationToken::new(),
            &observer,
        )
        .await
        .unwrap();

    let progress = observer.0.lock().unwrap();
    assert!(progress.iter().any(|event| {
        event.stage == LoopxProgressStage::Stderr && event.message.contains("fixture progress line")
    }));
    assert_eq!(
        progress.last().map(|event| event.stage),
        Some(LoopxProgressStage::Exited)
    );
}

#[test]
fn packaged_source_enum_stays_distinct_from_system_fallback() {
    assert_ne!(
        LoopxCommandSource::PackagedBundle,
        LoopxCommandSource::FixedSystemCommand
    );
}
