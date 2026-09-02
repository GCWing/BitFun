use super::tool_activity::ToolActivityProjection;
use super::{LoopxPersistedState, LoopxStateStore, LoopxTaskRuntimeRecord};
use crate::util::elapsed_ms_u64;
use bitfun_product_domains::miniapp::loopx::*;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};

const DEFAULT_AGENT_ID: &str = "bitfun-agent";
const EVENT_CHANNEL_CAPACITY: usize = 256;
const INTAKE_PREVIEW_TTL_MS: i64 = 5 * 60 * 1000;
const MAX_INTAKE_PREVIEWS: usize = 64;
const MAX_AGENT_SUMMARY_CHARS: usize = 16_000;
const GOAL_RECONCILE_TTL_MS: i64 = 30_000;
const GOAL_RECONCILE_DEADLINE_MS: i64 = 30_000;
/// Budget is counted at verified durable settlements, never at tool-call or
/// prompt-pattern level. A natural LoopX gate or Goal terminal state wins
/// before this policy is considered.
const AUTONOMY_REVIEW_ACTION_KIND: &str = "autonomous_budget_review";
/// Bounds for the host goal-facts block injected into each turn prompt.
const MAX_HOST_FACT_TODOS: usize = 8;
const MAX_HOST_FACT_TODO_CHARS: usize = 160;
const MAX_HOST_FACT_SUMMARY_CHARS: usize = 2_000;
/// loopx 0.5.1's turn plan exposes only cadence labels, never a numeric
/// scheduler hint, so the host owns the heartbeat cadence for waiting goals.
const WAIT_RESCHEDULE_FALLBACK_MS: u64 = 60_000;

struct ScheduledTask {
    task_id: String,
}

struct InProgressGuard<'a>(&'a AtomicBool);

impl Drop for InProgressGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[derive(Default)]
struct BufferedProgress(StdMutex<Vec<LoopxCliProgress>>);

impl BufferedProgress {
    fn take(&self) -> Vec<LoopxCliProgress> {
        std::mem::take(
            &mut *self
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }
}

impl LoopxCliProgressSink for BufferedProgress {
    fn report(&self, progress: LoopxCliProgress) {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(progress);
    }
}

pub struct LoopxController {
    cli: Arc<dyn LoopxCliPort>,
    workspace: Arc<dyn LoopxWorkspacePort>,
    agent: Arc<dyn LoopxAgentPort>,
    store: LoopxStateStore,
    state: RwLock<LoopxPersistedState>,
    mutation_lock: Mutex<()>,
    reconcile_lock: Mutex<()>,
    previews: RwLock<HashMap<String, LoopxIntakePreview>>,
    active_tasks: Mutex<HashMap<String, bool>>,
    active_repositories: Mutex<HashMap<String, String>>,
    event_sender: broadcast::Sender<LoopxEvent>,
    task_sender: mpsc::UnboundedSender<ScheduledTask>,
    load_error: RwLock<Option<String>>,
    install_in_progress: AtomicBool,
    reset_in_progress: AtomicBool,
}

impl LoopxController {
    pub async fn load(
        cli: Arc<dyn LoopxCliPort>,
        workspace: Arc<dyn LoopxWorkspacePort>,
        agent: Arc<dyn LoopxAgentPort>,
        store: LoopxStateStore,
    ) -> Arc<Self> {
        let now = now_ms();
        let (mut persisted, load_error) = match store.load().await {
            Ok(Some(state)) => (state, None),
            Ok(None) => (LoopxPersistedState::new(now), None),
            Err(error) => (LoopxPersistedState::new(now), Some(error)),
        };
        let restart_changed = load_error.is_none() && persisted.apply_restart_policy(now);
        let (event_sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let (task_sender, mut task_receiver) = mpsc::unbounded_channel::<ScheduledTask>();
        let controller = Arc::new(Self {
            cli,
            workspace,
            agent,
            store,
            state: RwLock::new(persisted),
            mutation_lock: Mutex::new(()),
            reconcile_lock: Mutex::new(()),
            previews: RwLock::new(HashMap::new()),
            active_tasks: Mutex::new(HashMap::new()),
            active_repositories: Mutex::new(HashMap::new()),
            event_sender,
            task_sender,
            load_error: RwLock::new(load_error),
            install_in_progress: AtomicBool::new(false),
            reset_in_progress: AtomicBool::new(false),
        });
        if restart_changed {
            if let Err(error) = controller.persist_current().await {
                *controller.load_error.write().await = Some(error);
            }
        }
        let task_runner = Arc::clone(&controller);
        tokio::spawn(async move {
            while let Some(scheduled) = task_receiver.recv().await {
                let task_runner = Arc::clone(&task_runner);
                tokio::spawn(async move {
                    if !task_runner.reserve_scheduled_task(&scheduled.task_id).await {
                        return;
                    }
                    loop {
                        let result = task_runner.drive_task(scheduled.task_id.clone()).await;
                        if let Err(error) = result {
                            let _ = task_runner.fail_task(&scheduled.task_id, error).await;
                        }
                        if !task_runner.release_scheduled_task(&scheduled.task_id).await {
                            break;
                        }
                        if !task_runner.reserve_scheduled_task(&scheduled.task_id).await {
                            break;
                        }
                    }
                });
            }
        });
        controller.enqueue_ready_tasks_after_load().await;
        controller
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LoopxEvent> {
        self.event_sender.subscribe()
    }

    pub async fn attach(
        &self,
        execution_domain: LoopxExecutionDomain,
        execution_support: LoopxExecutionSupport,
        unsupported_reason: Option<String>,
    ) -> LoopxAttachResponse {
        let environment_ready =
            self.state.read().await.environment.status == LoopxEnvironmentStatus::Ready;
        if execution_support == LoopxExecutionSupport::Supported
            && environment_ready
            && self.load_error.read().await.is_none()
        {
            self.reconcile_goal_projections(false).await;
        }
        let state = self.state.read().await;
        let mut snapshot = state.snapshot(
            execution_domain,
            execution_support,
            unsupported_reason,
            now_ms(),
        );
        if let Some(error) = self.load_error.read().await.clone() {
            snapshot.execution_support = LoopxExecutionSupport::UnsupportedExecutionDomain;
            snapshot.unsupported_reason = Some(error);
            snapshot.environment.status = LoopxEnvironmentStatus::Blocked;
        }
        LoopxAttachResponse { snapshot }
    }

    /// Re-hydrates LoopX-owned projections after the trusted Desktop surface
    /// observes a suspend/resume clock discontinuity. Active Agent turns are
    /// preserved: Windows can resume their subprocess tree successfully, so
    /// this path invalidates stale clients and refreshes only read-only host
    /// facts instead of manufacturing a failure or duplicate turn.
    pub async fn handle_host_resume(self: &Arc<Self>) -> Result<(), String> {
        if self.reset_in_progress.load(Ordering::Acquire) {
            return Ok(());
        }
        {
            let _mutation = self.mutation_lock.lock().await;
            let mut state = self.state.write().await;
            let start_cursor = state.cursor;
            state.revision = state.revision.saturating_add(1);
            state.append_event(LoopxEvent {
                kind: LoopxEventKind::SnapshotInvalidated,
                level: LoopxEventLevel::Info,
                source: LoopxEventSource::Controller,
                message: "Host resumed; refreshing LoopX projections".to_string(),
                occurred_at: now_ms(),
                ..LoopxEvent::default()
            });
            let persisted = state.clone();
            drop(state);
            self.store.save(&persisted).await?;
            self.broadcast_new_events(&persisted, start_cursor);
        }

        if let Err(error) = self.refresh_environment().await {
            log::warn!("LoopX environment refresh after host resume failed: {error}");
        }
        self.reconcile_goal_projections(true).await;
        Ok(())
    }

    /// Refresh the read-only LoopX Goal projection before presenting persisted
    /// host jobs. Failures preserve the last local projection and are surfaced
    /// in logs; they never manufacture a Goal transition or local fallback.
    async fn reconcile_goal_projections(&self, force: bool) {
        let Ok(_reconcile) = self.reconcile_lock.try_lock() else {
            return;
        };
        let now = now_ms();
        let candidates = {
            let state = self.state.read().await;
            state
                .tasks
                .iter()
                .filter(|task| {
                    task.goal_id.as_deref().is_some_and(|id| !id.is_empty())
                        && task
                            .workspace_path
                            .as_deref()
                            .is_some_and(|path| !path.is_empty())
                        && !matches!(
                            task.state,
                            LoopxTaskState::Preparing
                                | LoopxTaskState::Running
                                | LoopxTaskState::Cancelling
                                | LoopxTaskState::Aborted
                                | LoopxTaskState::Archived
                        )
                        // Passive states change through explicit host actions. Re-
                        // inspecting them on every UI attach only spawns sidecar
                        // processes that can time out. Suspend/resume keeps the force
                        // path so an externally changed Goal is still repaired.
                        && (force
                            || (task.state == LoopxTaskState::WaitingForUser
                                && task.pending_gate_id.is_none())
                            || !matches!(
                                task.state,
                                LoopxTaskState::WaitingForUser
                                    | LoopxTaskState::Completed
                                    | LoopxTaskState::Stopped
                                    | LoopxTaskState::Failed
                                    | LoopxTaskState::RecoveryRequired
                            ))
                        && (force
                            || task.goal_state.is_none()
                            || now.saturating_sub(task.updated_at) >= GOAL_RECONCILE_TTL_MS)
                })
                .filter_map(|task| {
                    let runtime = state.runtime.get(&task.task_id)?.clone();
                    (!runtime.registry_path.is_empty()).then(|| (task.clone(), runtime))
                })
                .collect::<Vec<_>>()
        };

        for (task, runtime) in candidates {
            let mut context = goal_context(&task, &runtime);
            context.call.operation_id =
                format!("reconcile-goal-{}-{}", task.task_id, uuid::Uuid::new_v4());
            context.call.deadline_at = Some(now_ms().saturating_add(GOAL_RECONCILE_DEADLINE_MS));
            let progress = BufferedProgress::default();
            let result = self
                .cli
                .inspect_goal(
                    LoopxCliInspectGoalRequest {
                        context,
                        goal_id: task.goal_id.clone().unwrap_or_default(),
                        agent_id: task
                            .agent_id
                            .clone()
                            .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
                    },
                    &progress,
                )
                .await;
            if let Err(error) = self.record_progress(progress.take()).await {
                log::warn!(
                    "Failed to persist LoopX reconciliation progress: task_id={}, error={}",
                    task.task_id,
                    error
                );
            }
            let snapshot = match result {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    log::warn!(
                        "LoopX Goal reconciliation failed: task_id={}, goal_id={}, error={}",
                        task.task_id,
                        task.goal_id.as_deref().unwrap_or("unknown"),
                        error
                    );
                    continue;
                }
            };
            if let Err(error) = self.apply_goal_projection(&task, &snapshot).await {
                log::warn!(
                    "Failed to apply LoopX Goal projection: task_id={}, goal_id={}, error={}",
                    task.task_id,
                    snapshot.goal_id,
                    error
                );
            }
        }
    }

    pub async fn events_since(&self, request: LoopxEventsSinceRequest) -> LoopxEventsSinceResponse {
        self.state.read().await.events_since(
            &request.stream_id,
            request.after_cursor,
            request.limit,
        )
    }

    pub async fn turn_output_since(
        &self,
        request: LoopxTurnOutputSinceRequest,
    ) -> LoopxTurnOutputSinceResponse {
        let (task, runtime) = {
            let state = self.state.read().await;
            let Some(task) = state
                .tasks
                .iter()
                .find(|task| task.task_id == request.task_id)
            else {
                return LoopxTurnOutputSinceResponse {
                    status: LoopxTurnOutputStatus::TaskNotFound,
                    task_id: request.task_id,
                    message: Some("LoopX task was not found".to_string()),
                    ..LoopxTurnOutputSinceResponse::default()
                };
            };
            let runtime = state
                .runtime
                .get(&task.task_id)
                .cloned()
                .unwrap_or_default();
            (task.clone(), runtime)
        };

        if task.state != LoopxTaskState::Running || task.phase != LoopxPhase::AgentRunning {
            return LoopxTurnOutputSinceResponse {
                status: LoopxTurnOutputStatus::NotRunning,
                task_id: task.task_id,
                turn_id: task.current_turn_id,
                message: Some("LoopX task does not have an active Agent turn".to_string()),
                ..LoopxTurnOutputSinceResponse::default()
            };
        }
        let Some(session_id) = runtime.session_id else {
            return LoopxTurnOutputSinceResponse {
                status: LoopxTurnOutputStatus::OutputUnavailable,
                task_id: task.task_id,
                turn_id: task.current_turn_id,
                message: Some("LoopX Agent session output is unavailable".to_string()),
                ..LoopxTurnOutputSinceResponse::default()
            };
        };
        let Some(turn_id) = runtime
            .agent_turn_id
            .clone()
            .or_else(|| task.current_turn_id.clone())
        else {
            return LoopxTurnOutputSinceResponse {
                status: LoopxTurnOutputStatus::OutputUnavailable,
                task_id: task.task_id,
                message: Some("LoopX Agent turn output is unavailable".to_string()),
                ..LoopxTurnOutputSinceResponse::default()
            };
        };
        if request
            .turn_id
            .as_deref()
            .is_some_and(|requested| requested != turn_id)
        {
            return LoopxTurnOutputSinceResponse {
                status: LoopxTurnOutputStatus::StaleTurn,
                task_id: task.task_id,
                turn_id: Some(turn_id),
                message: Some("LoopX task moved to a different Agent turn".to_string()),
                ..LoopxTurnOutputSinceResponse::default()
            };
        }

        match self
            .agent
            .output_since(LoopxAgentOutputSinceRequest {
                operation_id: format!("output-agent-{}", uuid::Uuid::new_v4()),
                session_id,
                turn_id: turn_id.clone(),
                stream_id: request.stream_id,
                after_cursor: request.after_cursor,
                limit: request.limit,
            })
            .await
        {
            Ok(page) => LoopxTurnOutputSinceResponse {
                status: LoopxTurnOutputStatus::Current,
                task_id: task.task_id,
                turn_id: Some(turn_id),
                stream_id: page.stream_id,
                events: page.events,
                next_cursor: page.next_cursor,
                has_more: page.has_more,
                message: None,
            },
            Err(error) => LoopxTurnOutputSinceResponse {
                status: LoopxTurnOutputStatus::OutputUnavailable,
                task_id: task.task_id,
                turn_id: Some(turn_id),
                message: Some(error.to_string()),
                ..LoopxTurnOutputSinceResponse::default()
            },
        }
    }

    pub async fn refresh_environment(self: &Arc<Self>) -> Result<(), String> {
        self.ensure_writable().await?;
        let probe_id = uuid::Uuid::new_v4();
        self.mark_environment_checking().await?;
        let progress = BufferedProgress::default();
        let handshake = self.cli.handshake(
            LoopxCliHandshakeRequest {
                call: LoopxCliCallContext {
                    operation_id: format!("environment-sidecar-{probe_id}"),
                    deadline_at: None,
                },
                ..LoopxCliHandshakeRequest::default()
            },
            &progress,
        );
        let workspace = self.workspace.probe(LoopxWorkspaceProbeRequest {
            operation_id: format!("environment-workspace-{probe_id}"),
            repository: None,
        });
        let agent = self.agent.probe(LoopxAgentProbeRequest {
            operation_id: format!("environment-agent-{probe_id}"),
            model_id: Some("auto".to_string()),
        });
        let open_viking = self.cli.probe_open_viking(LoopxOpenVikingProbeRequest {
            call: LoopxCliCallContext {
                operation_id: format!("environment-openviking-{probe_id}"),
                deadline_at: None,
            },
        });
        let github_auth = self.probe_github_auth();
        let (handshake, workspace, agent, open_viking, github_auth) =
            tokio::join!(handshake, workspace, agent, open_viking, github_auth);
        self.record_progress(progress.take()).await?;
        self.commit_environment(handshake, workspace, agent, open_viking, github_auth)
            .await?;
        self.reconcile_goal_projections(true).await;
        Ok(())
    }

    async fn probe_github_auth(&self) -> LoopxGithubAuthProbe {
        let operation_id = format!("github-auth-{}", uuid::Uuid::new_v4());
        match self
            .cli
            .probe_github_auth(LoopxGithubAuthProbeRequest {
                call: LoopxCliCallContext {
                    operation_id,
                    deadline_at: None,
                },
            })
            .await
        {
            Ok(probe) => probe,
            Err(error) => LoopxGithubAuthProbe {
                authenticated: false,
                detail: Some(format!("GitHub auth probe failed: {error}")),
                ..LoopxGithubAuthProbe::default()
            },
        }
    }

    pub async fn resolve_intake(
        &self,
        request: LoopxResolveIntakeRequest,
    ) -> Result<LoopxResolveIntakeResponse, String> {
        self.ensure_writable().await?;
        let target = parse_loopx_intake(&request.input).map_err(|error| error.to_string())?;
        let probe_id = uuid::Uuid::new_v4();
        let repository = target.repository().clone();
        let model_id = request.model_id;
        let progress = BufferedProgress::default();
        let resolved = self.cli.resolve_intake(
            LoopxCliResolveIntakeRequest {
                call: LoopxCliCallContext {
                    operation_id: format!("resolve-metadata-{probe_id}"),
                    deadline_at: None,
                },
                input: request.input,
                target: target.clone(),
            },
            &progress,
        );
        let workspace_probe = self.workspace.probe(LoopxWorkspaceProbeRequest {
            operation_id: format!("resolve-workspace-{probe_id}"),
            repository: Some(repository),
        });
        let agent_probe = self.agent.probe(LoopxAgentProbeRequest {
            operation_id: format!("resolve-agent-{probe_id}"),
            model_id: Some(model_id.clone()),
        });
        let (resolved, workspace_probe, agent_probe) =
            tokio::join!(resolved, workspace_probe, agent_probe);
        self.record_progress(progress.take()).await?;
        let resolved = resolved.map_err(|error| error.to_string())?;
        let scopes = vec![
            LoopxPermissionScope::WorkspaceRead,
            LoopxPermissionScope::WorkspaceWrite,
            LoopxPermissionScope::GitLocal,
            LoopxPermissionScope::GithubRead,
            LoopxPermissionScope::AgentExecution,
        ];
        let model = match agent_probe {
            Ok(probe) => LoopxModelCapability {
                model_id,
                available: true,
                supports_images: probe.supports_images,
                detail: Some(format!("Resolved Agent model: {}", probe.model_id)),
            },
            Err(error) => LoopxModelCapability {
                model_id,
                available: false,
                supports_images: false,
                detail: Some(error.to_string()),
            },
        };
        let workspace = match workspace_probe {
            Ok(probe) => LoopxWorkspacePreview {
                disposition: LoopxWorkspaceDisposition::CloneRequired,
                path: None,
                repository_verified: probe.repository_verified,
                detail: Some(format!(
                    "{}; repository access verified",
                    probe
                        .git_version
                        .unwrap_or_else(|| "Git available".to_string())
                )),
            },
            Err(error) => LoopxWorkspacePreview {
                disposition: LoopxWorkspaceDisposition::Unavailable,
                path: None,
                repository_verified: false,
                detail: Some(error.to_string()),
            },
        };
        let fingerprint = build_intake_fingerprint(
            &resolved.target,
            &resolved.candidates,
            None,
            &model.model_id,
            &scopes,
        );
        let preview_resolved_at = if resolved.resolved_at > 0 {
            resolved.resolved_at
        } else {
            now_ms()
        };
        let expires_at = preview_resolved_at.saturating_add(INTAKE_PREVIEW_TTL_MS);
        let preview = LoopxIntakePreview {
            fingerprint: fingerprint.clone(),
            target: resolved.target,
            repository: resolved.repository,
            workspace,
            candidates: resolved.candidates,
            truncated: resolved.truncated,
            model,
            permission_scopes: scopes,
            resolved_at: preview_resolved_at,
            expires_at: Some(expires_at),
        };
        let mut previews = self.previews.write().await;
        prune_intake_previews(&mut previews, now_ms());
        previews.insert(fingerprint, preview.clone());
        prune_intake_previews(&mut previews, now_ms());
        Ok(LoopxResolveIntakeResponse { preview })
    }

    pub async fn create_tasks(
        self: &Arc<Self>,
        request: LoopxCreateTaskRequest,
    ) -> Result<LoopxCreateTaskResponse, String> {
        self.ensure_writable().await?;
        if request.client_request_id.trim().is_empty() {
            return Err("clientRequestId is required".to_string());
        }
        let selected = request
            .selected_items
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        if selected.is_empty() {
            return Err("Select at least one issue or pull request".to_string());
        }
        {
            let state = self.state.read().await;
            if state.has_processed_request(&request.client_request_id) {
                return Ok(LoopxCreateTaskResponse {
                    outcomes: existing_outcomes(&state, &selected),
                    snapshot_revision: state.revision,
                });
            }
        }
        let preview = match {
            let mut previews = self.previews.write().await;
            prune_intake_previews(&mut previews, now_ms());
            previews.get(&request.preview_fingerprint).cloned()
        } {
            Some(preview) => preview,
            None => {
                let state = self.state.read().await;
                if state.has_processed_request(&request.client_request_id) {
                    return Ok(LoopxCreateTaskResponse {
                        outcomes: existing_outcomes(&state, &selected),
                        snapshot_revision: state.revision,
                    });
                }
                return Err("Intake preview is missing or stale; resolve it again".to_string());
            }
        };
        if selected.iter().any(|key| {
            !preview
                .candidates
                .iter()
                .any(|candidate| &candidate.key == key)
        }) {
            return Err("Selected item was not present in the intake preview".to_string());
        }
        if preview.workspace.disposition == LoopxWorkspaceDisposition::Unavailable
            || !preview.workspace.repository_verified
        {
            return Err(preview.workspace.detail.clone().unwrap_or_else(|| {
                "The repository workspace did not pass live Git verification".to_string()
            }));
        }
        if !preview.model.available {
            return Err(preview
                .model
                .detail
                .clone()
                .unwrap_or_else(|| "The selected Agent model is unavailable".to_string()));
        }
        if request.granted_scopes.iter().any(|scope| {
            !preview.permission_scopes.contains(scope) || !intake_scope_is_pregrantable(*scope)
        }) {
            return Err("Intake includes a permission scope that was not previewed".to_string());
        }

        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        let start_cursor = state.cursor;
        if state.has_processed_request(&request.client_request_id) {
            return Ok(LoopxCreateTaskResponse {
                outcomes: existing_outcomes(&state, &selected),
                snapshot_revision: state.revision,
            });
        }
        let existing = state
            .tasks
            .iter()
            .map(|task| LoopxExistingTask {
                task_id: task.task_id.clone(),
                identity: task.identity.clone(),
                state: task.state,
            })
            .collect::<Vec<_>>();
        let batch_id = (selected.len() > 1).then(|| uuid::Uuid::new_v4().to_string());
        let now = now_ms();
        let mut outcomes = Vec::new();
        let mut created_task_ids = Vec::new();
        for key in selected {
            let candidate = preview
                .candidates
                .iter()
                .find(|candidate| candidate.key == key)
                .expect("selected candidates were validated before mutation");
            match decide_task_dedup(&key, candidate.state, &existing, request.retry_terminal) {
                LoopxDedupDecision::OpenExisting { task_id } => {
                    outcomes.push(LoopxCreateTaskOutcome {
                        item: key,
                        kind: LoopxCreateTaskOutcomeKind::OpenedExisting,
                        task_id: Some(task_id),
                        ..LoopxCreateTaskOutcome::default()
                    })
                }
                LoopxDedupDecision::RequireExplicitRetry {
                    previous_task_id,
                    next_attempt,
                } => outcomes.push(LoopxCreateTaskOutcome {
                    item: key,
                    kind: LoopxCreateTaskOutcomeKind::RetryConfirmationRequired,
                    task_id: Some(previous_task_id),
                    attempt: Some(next_attempt),
                    ..LoopxCreateTaskOutcome::default()
                }),
                LoopxDedupDecision::ClosedNoop => outcomes.push(LoopxCreateTaskOutcome {
                    item: key,
                    kind: LoopxCreateTaskOutcomeKind::ClosedNoop,
                    ..LoopxCreateTaskOutcome::default()
                }),
                LoopxDedupDecision::NeedsLiveVerification => {
                    outcomes.push(LoopxCreateTaskOutcome {
                        item: key,
                        kind: LoopxCreateTaskOutcomeKind::NeedsLiveVerification,
                        ..LoopxCreateTaskOutcome::default()
                    })
                }
                LoopxDedupDecision::CreateAttempt { attempt } => {
                    let task_id = uuid::Uuid::new_v4().to_string();
                    let operation_id = format!("prepare-{task_id}-1");
                    let task = LoopxTaskSnapshot {
                        task_id: task_id.clone(),
                        batch_id: batch_id.clone(),
                        identity: LoopxTaskIdentity {
                            item: key.clone(),
                            attempt,
                            title: candidate.title.clone(),
                            description: candidate.description.clone(),
                            state: candidate.state,
                            labels: candidate.labels.clone(),
                        },
                        generation: 1,
                        revision: 1,
                        agent_id: Some(DEFAULT_AGENT_ID.to_string()),
                        state: LoopxTaskState::Preparing,
                        phase: LoopxPhase::PreparingWorkspace,
                        model_id: Some(request.model_id.clone()),
                        granted_scopes: request.granted_scopes.clone(),
                        created_at: now,
                        updated_at: now,
                        ..LoopxTaskSnapshot::default()
                    };
                    state.runtime.insert(
                        task_id.clone(),
                        LoopxTaskRuntimeRecord {
                            operation_id,
                            ..LoopxTaskRuntimeRecord::default()
                        },
                    );
                    state.tasks.push(task);
                    state.revision = state.revision.saturating_add(1);
                    state.append_event(LoopxEvent {
                        task_id: Some(task_id.clone()),
                        generation: Some(1),
                        revision: Some(1),
                        kind: LoopxEventKind::TaskCreated,
                        source: LoopxEventSource::Controller,
                        phase: Some(LoopxPhase::PreparingWorkspace),
                        message: "LoopX task reserved before workspace preparation".to_string(),
                        important: true,
                        occurred_at: now,
                        ..LoopxEvent::default()
                    });
                    outcomes.push(LoopxCreateTaskOutcome {
                        item: key,
                        kind: LoopxCreateTaskOutcomeKind::Created,
                        task_id: Some(task_id.clone()),
                        attempt: Some(attempt),
                        ..LoopxCreateTaskOutcome::default()
                    });
                    created_task_ids.push(task_id);
                }
            }
        }
        state.record_processed_request(request.client_request_id);
        let snapshot_revision = state.revision;
        let persisted = state.clone();
        drop(state);
        self.store.save(&persisted).await?;
        drop(_mutation);
        self.broadcast_new_events(&persisted, start_cursor);

        for task_id in created_task_ids {
            self.enqueue_task(task_id, Duration::ZERO)?;
        }
        Ok(LoopxCreateTaskResponse {
            outcomes,
            snapshot_revision,
        })
    }

    pub async fn action(
        self: &Arc<Self>,
        request: LoopxActionRequest,
    ) -> Result<LoopxActionResponse, String> {
        self.ensure_writable().await?;
        if request.action == LoopxActionKind::RetryEnvironment {
            self.refresh_environment().await?;
            return Ok(LoopxActionResponse {
                current_revision: self.state.read().await.revision,
                ..LoopxActionResponse::default()
            });
        }
        if request.action == LoopxActionKind::InstallLoopx {
            return self.start_loopx_install(&request).await;
        }
        if request.action == LoopxActionKind::InstallOpenViking {
            return self.start_open_viking_install(&request).await;
        }
        if request.action == LoopxActionKind::ResumeRepository {
            return self.resume_repository(&request).await;
        }
        if request.action == LoopxActionKind::ResetAll {
            return self.reset_all(&request).await;
        }
        let task_id = request
            .task_id
            .clone()
            .ok_or_else(|| "taskId is required".to_string())?;
        let (task, runtime) = {
            let state = self.state.read().await;
            if state.has_processed_request(&request.client_request_id) {
                return Ok(LoopxActionResponse {
                    status: LoopxActionStatus::Duplicate,
                    current_revision: state.revision,
                    task: state
                        .tasks
                        .iter()
                        .find(|task| task.task_id == task_id)
                        .cloned(),
                    ..LoopxActionResponse::default()
                });
            }
            let task = state
                .tasks
                .iter()
                .find(|task| task.task_id == task_id)
                .cloned()
                .ok_or_else(|| "LoopX task not found".to_string())?;
            if request.action == LoopxActionKind::Resume
                && matches!(
                    task.state,
                    LoopxTaskState::Preparing
                        | LoopxTaskState::Queued
                        | LoopxTaskState::Running
                        | LoopxTaskState::RetryWait
                )
            {
                return Ok(LoopxActionResponse {
                    status: LoopxActionStatus::Duplicate,
                    current_revision: task.revision,
                    task: Some(task),
                    message: Some("Task is already queued or running".to_string()),
                });
            }
            if task.revision != request.expected_revision {
                return Ok(LoopxActionResponse {
                    status: LoopxActionStatus::RevisionConflict,
                    current_revision: task.revision,
                    task: Some(task),
                    message: Some("Task changed; refresh before applying the action".to_string()),
                });
            }
            (
                task,
                state.runtime.get(&task_id).cloned().unwrap_or_default(),
            )
        };

        match request.action {
            LoopxActionKind::Pause => {
                self.pause_task(&task, &runtime, &request.client_request_id)
                    .await
            }
            LoopxActionKind::Abort => {
                self.abort_task(&task, &runtime, &request.client_request_id)
                    .await
            }
            LoopxActionKind::Resume => self.resume_task(&task, &request.client_request_id).await,
            LoopxActionKind::ResumeRepository
            | LoopxActionKind::ResetAll
            | LoopxActionKind::InstallLoopx
            | LoopxActionKind::InstallOpenViking => unreachable!(),
            LoopxActionKind::Approve | LoopxActionKind::Reject => {
                self.answer_gate(&task, &runtime, &request).await
            }
            LoopxActionKind::Archive => {
                let response = self
                    .transition_action(
                        &task_id,
                        LoopxTaskState::Archived,
                        LoopxPhase::Finished,
                        &request.client_request_id,
                    )
                    .await?;
                // Explicit user action: archive is the only workflow that
                // destroys the task worktree (and its bare repository when
                // the last worktree is gone). Terminal states keep their
                // worktrees so the user can inspect agent output first.
                self.dispose_task_workspace(&task).await;
                Ok(response)
            }
            LoopxActionKind::Restore => {
                self.transition_action(
                    &task_id,
                    LoopxTaskState::RecoveryRequired,
                    LoopxPhase::Recovering,
                    &request.client_request_id,
                )
                .await
            }
            LoopxActionKind::RetryEnvironment => unreachable!(),
        }
    }

    async fn start_loopx_install(
        self: &Arc<Self>,
        request: &LoopxActionRequest,
    ) -> Result<LoopxActionResponse, String> {
        let started_at = Instant::now();
        if request.client_request_id.trim().is_empty() {
            return Err("clientRequestId is required".to_string());
        }
        {
            let state = self.state.read().await;
            if state.has_processed_request(&request.client_request_id) {
                return Ok(LoopxActionResponse {
                    status: LoopxActionStatus::Duplicate,
                    current_revision: state.revision,
                    message: Some("LoopX installation request was already applied".to_string()),
                    ..LoopxActionResponse::default()
                });
            }
            if state.environment.core.sidecar.status == LoopxEnvironmentFactStatus::Available {
                return Ok(LoopxActionResponse {
                    status: LoopxActionStatus::Duplicate,
                    current_revision: state.revision,
                    message: Some("A compatible LoopX runtime is already available".to_string()),
                    ..LoopxActionResponse::default()
                });
            }
        }
        if self
            .install_in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(LoopxActionResponse {
                status: LoopxActionStatus::Duplicate,
                current_revision: self.state.read().await.revision,
                message: Some("LoopX installation is already running".to_string()),
                ..LoopxActionResponse::default()
            });
        }
        let current_revision = match self.mark_loopx_installing(&request.client_request_id).await {
            Ok(revision) => revision,
            Err(error) => {
                self.install_in_progress.store(false, Ordering::Release);
                return Err(error);
            }
        };
        log::info!(
            "LoopX installation state persisted: request_id={}, revision={}, duration_ms={}",
            request.client_request_id,
            current_revision,
            elapsed_ms_u64(started_at)
        );
        let request_id = request.client_request_id.clone();
        let controller = Arc::clone(self);
        tokio::spawn(async move {
            let _install_guard = InProgressGuard(&controller.install_in_progress);
            log::info!("LoopX installation background task started: request_id={request_id}");
            if let Err(error) = controller.run_loopx_install(&request_id).await {
                log::error!(
                    "LoopX managed source installation failed: request_id={request_id}, error={error}"
                );
                let _ = controller.mark_loopx_install_failed(&error).await;
            }
        });
        Ok(LoopxActionResponse {
            status: LoopxActionStatus::Applied,
            current_revision,
            message: Some("LoopX installation started".to_string()),
            ..LoopxActionResponse::default()
        })
    }

    async fn run_loopx_install(self: &Arc<Self>, request_id: &str) -> Result<(), String> {
        let progress = BufferedProgress::default();
        let operation_id = format!("install-loopx-{}", uuid::Uuid::new_v4());
        let started_at = Instant::now();
        log::info!(
            "LoopX installation service call started: request_id={request_id}, operation_id={operation_id}"
        );
        let result = self
            .cli
            .install_managed_source(
                LoopxCliInstallManagedSourceRequest {
                    call: LoopxCliCallContext {
                        operation_id: operation_id.clone(),
                        deadline_at: None,
                    },
                },
                &progress,
            )
            .await;
        self.record_progress(progress.take()).await?;
        let installed = result.map_err(|error| error.to_string())?;
        log::info!(
            "LoopX installation service call completed: request_id={request_id}, operation_id={operation_id}, version={}, source={}, commit={}, duration_ms={}",
            installed.loopx_version,
            installed.source_repository,
            installed.source_commit,
            elapsed_ms_u64(started_at)
        );
        self.refresh_environment().await?;
        Ok(())
    }

    async fn mark_loopx_installing(&self, request_id: &str) -> Result<u64, String> {
        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        let checked_at = Some(now_ms());
        state.environment.revision = state.environment.revision.saturating_add(1);
        state.environment.status = LoopxEnvironmentStatus::Checking;
        state.environment.checked_at = checked_at;
        state.environment.core.sidecar = LoopxEnvironmentFact {
            status: LoopxEnvironmentFactStatus::Checking,
            version: Some(LOOPX_PINNED_VERSION.to_string()),
            detail: Some("Downloading runtime files from the official GitHub source".to_string()),
            checked_at,
            ..LoopxEnvironmentFact::default()
        };
        state.record_processed_request(request_id.to_string());
        state.revision = state.revision.saturating_add(1);
        let current_revision = state.revision;
        let start_cursor = state.cursor;
        state.append_event(LoopxEvent {
            kind: LoopxEventKind::EnvironmentChanged,
            source: LoopxEventSource::System,
            message: "LoopX managed source installation started".to_string(),
            occurred_at: now_ms(),
            ..LoopxEvent::default()
        });
        let persisted = state.clone();
        drop(state);
        self.store.save(&persisted).await?;
        self.broadcast_new_events(&persisted, start_cursor);
        Ok(current_revision)
    }

    async fn mark_loopx_install_failed(&self, error: &str) -> Result<(), String> {
        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        let checked_at = Some(now_ms());
        state.environment.revision = state.environment.revision.saturating_add(1);
        state.environment.status = LoopxEnvironmentStatus::Blocked;
        state.environment.checked_at = checked_at;
        state.environment.core.sidecar = unavailable_loopx_environment_fact(error, checked_at);
        state.revision = state.revision.saturating_add(1);
        let start_cursor = state.cursor;
        state.append_event(LoopxEvent {
            kind: LoopxEventKind::EnvironmentChanged,
            level: LoopxEventLevel::Error,
            source: LoopxEventSource::System,
            message: format!("LoopX managed source installation failed: {error}"),
            important: true,
            occurred_at: now_ms(),
            ..LoopxEvent::default()
        });
        let persisted = state.clone();
        drop(state);
        self.store.save(&persisted).await?;
        self.broadcast_new_events(&persisted, start_cursor);
        Ok(())
    }

    async fn start_open_viking_install(
        self: &Arc<Self>,
        request: &LoopxActionRequest,
    ) -> Result<LoopxActionResponse, String> {
        let started_at = Instant::now();
        if request.client_request_id.trim().is_empty() {
            return Err("clientRequestId is required".to_string());
        }
        {
            let state = self.state.read().await;
            if state.has_processed_request(&request.client_request_id) {
                return Ok(LoopxActionResponse {
                    status: LoopxActionStatus::Duplicate,
                    current_revision: state.revision,
                    message: Some(
                        "OpenViking installation request was already applied".to_string(),
                    ),
                    ..LoopxActionResponse::default()
                });
            }
            if state.environment.optional.open_viking.status
                == LoopxEnvironmentFactStatus::Available
            {
                return Ok(LoopxActionResponse {
                    status: LoopxActionStatus::Duplicate,
                    current_revision: state.revision,
                    message: Some("OpenViking is already available".to_string()),
                    ..LoopxActionResponse::default()
                });
            }
        }
        if self
            .install_in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(LoopxActionResponse {
                status: LoopxActionStatus::Duplicate,
                current_revision: self.state.read().await.revision,
                message: Some("An environment installation is already running".to_string()),
                ..LoopxActionResponse::default()
            });
        }
        let current_revision = match self
            .mark_open_viking_installing(&request.client_request_id)
            .await
        {
            Ok(revision) => revision,
            Err(error) => {
                self.install_in_progress.store(false, Ordering::Release);
                return Err(error);
            }
        };
        log::info!(
            "OpenViking installation state persisted: request_id={}, revision={}, duration_ms={}",
            request.client_request_id,
            current_revision,
            elapsed_ms_u64(started_at)
        );
        let request_id = request.client_request_id.clone();
        let controller = Arc::clone(self);
        tokio::spawn(async move {
            let _install_guard = InProgressGuard(&controller.install_in_progress);
            log::info!("OpenViking installation background task started: request_id={request_id}");
            if let Err(error) = controller.run_open_viking_install(&request_id).await {
                log::error!(
                    "OpenViking managed installation failed: request_id={request_id}, error={error}"
                );
                let _ = controller.mark_open_viking_install_failed(&error).await;
            }
        });
        Ok(LoopxActionResponse {
            status: LoopxActionStatus::Applied,
            current_revision,
            message: Some("OpenViking installation started".to_string()),
            ..LoopxActionResponse::default()
        })
    }

    async fn run_open_viking_install(self: &Arc<Self>, request_id: &str) -> Result<(), String> {
        let progress = BufferedProgress::default();
        let operation_id = format!("install-openviking-{}", uuid::Uuid::new_v4());
        let started_at = Instant::now();
        log::info!(
            "OpenViking installation service call started: request_id={request_id}, operation_id={operation_id}"
        );
        let result = self
            .cli
            .install_open_viking(
                LoopxCliInstallOpenVikingRequest {
                    call: LoopxCliCallContext {
                        operation_id: operation_id.clone(),
                        deadline_at: None,
                    },
                },
                &progress,
            )
            .await;
        self.record_progress(progress.take()).await?;
        let installed = result.map_err(|error| error.to_string())?;
        log::info!(
            "OpenViking installation service call completed: request_id={request_id}, operation_id={operation_id}, version={}, source={}, commit={}, extension_enabled={}, duration_ms={}",
            installed.open_viking_version,
            installed.source_repository,
            installed.source_commit,
            installed.extension_enabled,
            elapsed_ms_u64(started_at)
        );
        self.refresh_environment().await?;
        Ok(())
    }

    async fn mark_open_viking_installing(&self, request_id: &str) -> Result<u64, String> {
        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        let checked_at = Some(now_ms());
        state.environment.revision = state.environment.revision.saturating_add(1);
        state.environment.checked_at = checked_at;
        state.environment.optional.open_viking = LoopxEnvironmentFact {
            status: LoopxEnvironmentFactStatus::Checking,
            version: Some("0.4.9".to_string()),
            detail: Some(
                "Building the pinned OpenViking CLI from the official GitHub source".to_string(),
            ),
            checked_at,
            ..LoopxEnvironmentFact::default()
        };
        state.environment.optional.feedback_memory = LoopxEnvironmentFact {
            status: LoopxEnvironmentFactStatus::Checking,
            detail: Some("Waiting for the OpenViking runtime check".to_string()),
            checked_at,
            ..LoopxEnvironmentFact::default()
        };
        state.environment.status =
            derive_environment_status(&state.environment.core, &state.environment.optional);
        state.record_processed_request(request_id.to_string());
        state.revision = state.revision.saturating_add(1);
        let current_revision = state.revision;
        let start_cursor = state.cursor;
        state.append_event(LoopxEvent {
            kind: LoopxEventKind::EnvironmentChanged,
            source: LoopxEventSource::System,
            message: "OpenViking managed installation started".to_string(),
            occurred_at: now_ms(),
            ..LoopxEvent::default()
        });
        let persisted = state.clone();
        drop(state);
        self.store.save(&persisted).await?;
        self.broadcast_new_events(&persisted, start_cursor);
        Ok(current_revision)
    }

    async fn mark_open_viking_install_failed(&self, error: &str) -> Result<(), String> {
        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        let checked_at = Some(now_ms());
        state.environment.revision = state.environment.revision.saturating_add(1);
        state.environment.checked_at = checked_at;
        state.environment.optional.open_viking =
            unavailable_open_viking_environment_fact(error, checked_at);
        state.environment.optional.feedback_memory = LoopxEnvironmentFact {
            status: LoopxEnvironmentFactStatus::Unavailable,
            detail: Some("Feedback memory requires a working OpenViking runtime".to_string()),
            checked_at,
            ..LoopxEnvironmentFact::default()
        };
        state.environment.status =
            derive_environment_status(&state.environment.core, &state.environment.optional);
        state.revision = state.revision.saturating_add(1);
        let start_cursor = state.cursor;
        state.append_event(LoopxEvent {
            kind: LoopxEventKind::EnvironmentChanged,
            level: LoopxEventLevel::Error,
            source: LoopxEventSource::System,
            message: format!("OpenViking managed installation failed: {error}"),
            occurred_at: now_ms(),
            ..LoopxEvent::default()
        });
        let persisted = state.clone();
        drop(state);
        self.store.save(&persisted).await?;
        self.broadcast_new_events(&persisted, start_cursor);
        Ok(())
    }

    pub async fn handle_agent_terminal(
        self: &Arc<Self>,
        turn_id: &str,
        status: LoopxAgentTurnStatus,
        summary: Option<String>,
        blocks_repository: bool,
    ) -> Result<(), String> {
        let (task, runtime) = {
            let state = self.state.read().await;
            let Some((task_id, runtime)) = state
                .runtime
                .iter()
                .find(|(_, runtime)| runtime.agent_turn_id.as_deref() == Some(turn_id))
            else {
                return Ok(());
            };
            let Some(task) = state.tasks.iter().find(|task| &task.task_id == task_id) else {
                return Ok(());
            };
            (task.clone(), runtime.clone())
        };
        if task.state != LoopxTaskState::Running {
            return Ok(());
        }
        log::info!(
            "LoopX Agent terminal handling started: task_id={}, goal_id={}, agent_turn_id={}, status={:?}",
            task.task_id,
            task.goal_id.as_deref().unwrap_or("unknown"),
            turn_id,
            status
        );
        if let Some(summary) = summary
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let bounded = bounded_agent_summary(summary);
            self.mutate_task(&task.task_id, None, |current, _| {
                if current.generation != task.generation {
                    return;
                }
                current.last_agent_summary = Some(bounded);
                current.last_agent_summary_at = Some(now_ms());
                current.revision = current.revision.saturating_add(1);
            })
            .await?;
        }
        self.update_task_phase(
            &task.task_id,
            task.generation,
            LoopxPhase::ValidatingProgress,
            "Agent turn ended; finalizing LoopX-owned durable settlement",
        )
        .await?;
        let progress = BufferedProgress::default();
        let settlement_started = Instant::now();
        let result = self
            .cli
            .finalize_turn_settlement(
                LoopxCliSettleTurnRequest {
                    context: goal_context(&task, &runtime),
                    goal_id: task.goal_id.clone().unwrap_or_default(),
                    agent_id: task
                        .agent_id
                        .clone()
                        .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
                    turn_id: runtime.loopx_turn_id.clone().unwrap_or_default(),
                    settlement_token: runtime.settlement_token.clone().unwrap_or_default(),
                    expected_durable_revision: runtime
                        .expected_durable_revision
                        .clone()
                        .unwrap_or_default(),
                    agent_status: status,
                    agent_summary: summary.clone(),
                },
                &progress,
            )
            .await;
        match &result {
            Ok(settlement) => log::info!(
                "LoopX turn settlement completed: task_id={}, goal_id={}, loopx_turn_id={}, status={:?}, duration_ms={}, scheduler_hint_ms={:?}",
                task.task_id,
                task.goal_id.as_deref().unwrap_or("unknown"),
                settlement.turn_id,
                settlement.status,
                settlement_started.elapsed().as_millis(),
                settlement.scheduler_hint_ms
            ),
            Err(error) => log::warn!(
                "LoopX turn settlement failed: task_id={}, goal_id={}, duration_ms={}, error={}",
                task.task_id,
                task.goal_id.as_deref().unwrap_or("unknown"),
                settlement_started.elapsed().as_millis(),
                error
            ),
        }
        self.record_progress(progress.take()).await?;
        let finish_result = if let (Some(session_id), Some(agent_turn_id)) =
            (runtime.session_id.clone(), runtime.agent_turn_id.clone())
        {
            self.agent
                .finish(LoopxAgentFinishRequest {
                    operation_id: format!("finish-agent-{}", uuid::Uuid::new_v4()),
                    task_id: task.task_id.clone(),
                    generation: task.generation,
                    worktree_path: task.workspace_path.clone().unwrap_or_default(),
                    session_id,
                    turn_id: agent_turn_id,
                })
                .await
                .map_err(|error| error.to_string())
        } else {
            Ok(LoopxAgentFinishResult::default())
        };
        match &finish_result {
            Ok(finish) => log::info!(
                "LoopX transient Agent session finished: task_id={}, session_id={}, discarded={}",
                task.task_id,
                finish.session_id,
                finish.discarded
            ),
            Err(error) => log::warn!(
                "LoopX transient Agent session cleanup failed: task_id={}, error={}",
                task.task_id,
                error
            ),
        }
        match (result, finish_result) {
            (Ok(settlement), Ok(_)) => {
                self.apply_settlement(
                    &task,
                    settlement,
                    status,
                    summary.as_deref(),
                    blocks_repository,
                )
                .await
            }
            (Err(error), _) => self.fail_task(&task.task_id, error.to_string()).await,
            (Ok(_), Err(error)) => self.fail_task(&task.task_id, error).await,
        }
    }

    pub(super) async fn handle_agent_activity(&self, turn_id: &str) -> Result<(), String> {
        let task_id = {
            let state = self.state.read().await;
            state
                .runtime
                .iter()
                .find(|(_, runtime)| runtime.agent_turn_id.as_deref() == Some(turn_id))
                .map(|(task_id, _)| task_id.clone())
        };
        let Some(task_id) = task_id else {
            return Ok(());
        };
        self.mutate_task(&task_id, None, |task, _| {
            if task.state != LoopxTaskState::Running {
                return;
            }
            task.last_output_at = Some(now_ms());
            task.revision = task.revision.saturating_add(1);
        })
        .await?;
        Ok(())
    }

    pub(super) async fn handle_agent_tool_activity(
        &self,
        turn_id: &str,
        activity: ToolActivityProjection,
    ) -> Result<(), String> {
        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        let Some(task_id) = state
            .runtime
            .iter()
            .find(|(_, runtime)| runtime.agent_turn_id.as_deref() == Some(turn_id))
            .map(|(task_id, _)| task_id.clone())
        else {
            return Ok(());
        };
        let Some(task_index) = state.tasks.iter().position(|task| task.task_id == task_id) else {
            return Ok(());
        };
        if state.tasks[task_index].state != LoopxTaskState::Running {
            return Ok(());
        }

        let now = now_ms();
        {
            let task = &mut state.tasks[task_index];
            task.last_output_at = Some(now);
            task.updated_at = now;
            task.current_tool = activity.current_tool.clone();
            task.revision = task.revision.saturating_add(1);
        }
        let updated = state.tasks[task_index].clone();
        state.revision = state.revision.saturating_add(1);
        state.append_event(LoopxEvent {
            task_id: Some(updated.task_id.clone()),
            generation: Some(updated.generation),
            revision: Some(updated.revision),
            kind: LoopxEventKind::Log,
            level: if activity.important {
                LoopxEventLevel::Error
            } else {
                LoopxEventLevel::Info
            },
            source: LoopxEventSource::Agent,
            phase: Some(updated.phase),
            message: activity.message,
            important: activity.important,
            tool_name: Some(activity.tool_name),
            details: activity.details,
            occurred_at: now,
            ..LoopxEvent::default()
        });
        let persisted = state.clone();
        let event = persisted.events.last().cloned();
        drop(state);
        self.store.save(&persisted).await?;
        if let Some(event) = event {
            let _ = self.event_sender.send(event);
        }
        Ok(())
    }

    async fn drive_task(self: &Arc<Self>, task_id: String) -> Result<(), String> {
        let task = self.task(&task_id).await?;
        if !matches!(
            task.state,
            LoopxTaskState::Preparing | LoopxTaskState::Queued
        ) {
            return Ok(());
        }
        if !self.reserve_repository(&task).await {
            self.transition_task(
                &task_id,
                task.generation,
                LoopxTaskState::Queued,
                LoopxPhase::Queued,
                "Another task for this repository is running",
            )
            .await?;
            return Ok(());
        }
        let workspace = self
            .workspace
            .prepare(LoopxWorkspacePrepareRequest {
                operation_id: format!("workspace-{}-{}", task.task_id, task.generation),
                task_id: task.task_id.clone(),
                item: task.identity.item.clone(),
            })
            .await
            .map_err(|error| error.to_string())?;
        if !workspace.repository_verified {
            return Err("Prepared worktree does not match the requested repository".to_string());
        }
        self.bind_workspace(&task_id, task.generation, &workspace)
            .await?;
        let task = self.task(&task_id).await?;
        if task_has_bound_goal(&task) {
            return self.drive_turn(task_id).await;
        }
        let runtime = self.runtime(&task_id).await;
        let progress = BufferedProgress::default();
        let intake = self
            .cli
            .plan_item(
                LoopxCliPlanItemRequest {
                    context: goal_context(&task, &runtime),
                    item: task.identity.item.clone(),
                    title: task.identity.title.clone(),
                    state: task.identity.state,
                    labels: task.identity.labels.clone(),
                },
                &progress,
            )
            .await
            .map_err(|error| error.to_string())?;
        self.record_progress(progress.take()).await?;
        let goal_id = goal_id_for(&task.identity);
        let raw_packet_json = intake.raw_packet_json.clone();
        let progress = BufferedProgress::default();
        let created = self
            .cli
            .create_goal(
                LoopxCliCreateGoalRequest {
                    context: goal_context(&task, &runtime),
                    goal_id: goal_id.clone(),
                    agent_id: task
                        .agent_id
                        .clone()
                        .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
                    intake,
                    granted_scopes: task.granted_scopes.clone(),
                },
                &progress,
            )
            .await
            .map_err(|error| error.to_string())?;
        self.record_progress(progress.take()).await?;
        // The evidence-collection step inside goal creation persists the
        // source-qualified candidate admission receipt and returns a newer
        // workflow-plan packet; it supersedes the unconfigured plan packet.
        let raw_packet_json = if created.raw_packet_json.trim().is_empty() {
            raw_packet_json
        } else {
            created.raw_packet_json.clone()
        };
        self.bind_goal(&task_id, task.generation, created).await?;
        // Persist the raw workflow-plan packet into the worktree so every
        // turn can reach the exact issue-fix command forms; the LoopX `todo
        // add` surface cannot carry next-command previews, and without a
        // reachable copy the Agent never enters the issue-fix pipeline.
        if let Some(path) = self.persist_intake_plan(&task, &raw_packet_json).await {
            self.mutate_task(&task_id, None, |current, _| {
                if current.generation != task.generation {
                    return;
                }
                current.intake_plan_path = Some(path.clone());
                current.revision = current.revision.saturating_add(1);
            })
            .await?;
        }
        self.drive_turn(task_id).await
    }

    async fn persist_intake_plan(
        &self,
        task: &LoopxTaskSnapshot,
        raw_packet_json: &str,
    ) -> Option<String> {
        if raw_packet_json.trim().is_empty() {
            return None;
        }
        let worktree = task.workspace_path.as_deref()?;
        let request = LoopxWorkspaceIntakePlanRequest {
            operation_id: format!("intake-plan-{}-{}", task.task_id, uuid::Uuid::new_v4()),
            worktree_path: worktree.to_string(),
            packet_json: raw_packet_json.to_string(),
        };
        match self.workspace.persist_intake_plan(request).await {
            Ok(result) if result.wrote && !result.path.is_empty() => Some(result.path),
            Ok(_) => None,
            Err(error) => {
                log::warn!(
                    "LoopX intake plan persistence skipped for task {}: {}",
                    task.task_id,
                    error
                );
                None
            }
        }
    }

    /// Completes the host task for a Goal that has no runnable work left and
    /// stops the Goal's automatic advancement through the LoopX-owned
    /// lifecycle transition. LoopX never reaches `terminal_no_followup` on
    /// this integration path by itself, so without this the scheduler would
    /// keep requeueing the task after the last todo closed. The stop is
    /// best-effort: a failure is logged and the host task still completes,
    /// because the LoopX registry of a finished task is inert once the host
    /// job is terminal and the worktree is disposed.
    async fn complete_finished_goal(
        self: &Arc<Self>,
        task: &LoopxTaskSnapshot,
        runtime: &LoopxTaskRuntimeRecord,
        message: &str,
    ) -> Result<(), String> {
        let goal_id = task.goal_id.clone().unwrap_or_default();
        if !goal_id.is_empty() {
            let progress = BufferedProgress::default();
            let stop_request = LoopxCliStopGoalRequest {
                context: goal_context(task, runtime),
                goal_id,
                reason: message.to_string(),
            };
            match self.cli.stop_goal(stop_request, &progress).await {
                Ok(result) => {
                    self.record_progress(progress.take()).await?;
                    log::info!(
                        "LoopX Goal stopped after finish: task_id={}, goal_id={}, stopped={}, already_stopped={}",
                        task.task_id,
                        result.goal_id,
                        result.stopped,
                        result.already_stopped
                    );
                }
                Err(error) => {
                    let _ = progress.take();
                    log::warn!(
                        "LoopX Goal stop failed (host task still completes): task_id={}, error={}",
                        task.task_id,
                        error
                    );
                }
            }
        }
        self.release_repository(task).await;
        let updated = self
            .transition_task(
                &task.task_id,
                task.generation,
                LoopxTaskState::Completed,
                LoopxPhase::Finished,
                message,
            )
            .await?;
        self.schedule_next_for_repository(
            &updated.identity.item.repository.canonical_id(),
            Some(&updated.task_id),
        )
        .await;
        Ok(())
    }

    async fn drive_turn(self: &Arc<Self>, task_id: String) -> Result<(), String> {
        let task = self.task(&task_id).await?;
        let runtime = self.runtime(&task_id).await;
        let progress = BufferedProgress::default();
        let mut inspected = self
            .cli
            .inspect_goal(
                LoopxCliInspectGoalRequest {
                    context: goal_context(&task, &runtime),
                    goal_id: task.goal_id.clone().unwrap_or_default(),
                    agent_id: task
                        .agent_id
                        .clone()
                        .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
                },
                &progress,
            )
            .await
            .map_err(|error| error.to_string())?;
        self.record_progress(progress.take()).await?;
        self.record_goal_state(&task, inspected.state).await?;
        match inspected.run_decision {
            LoopxCliRunDecision::Wait => {
                if inspected.state == LoopxCliGoalState::Archived {
                    return self
                        .complete_finished_goal(&task, &runtime, "LoopX Goal was archived")
                        .await;
                }
                self.release_repository(&task).await;
                self.transition_task(
                    &task_id,
                    task.generation,
                    LoopxTaskState::Queued,
                    LoopxPhase::Queued,
                    "LoopX is waiting before the next bounded turn",
                )
                .await?;
                self.schedule_next_for_repository(
                    &task.identity.item.repository.canonical_id(),
                    Some(&task_id),
                )
                .await;
                // loopx 0.5.1 never emits a numeric scheduler hint, and a
                // waiting goal with no requeue would sleep forever. The host
                // owns the heartbeat cadence: honor an explicit hint when one
                // exists, otherwise fall back to a bounded polling interval.
                let delay = inspected
                    .scheduler_hint_ms
                    .unwrap_or(WAIT_RESCHEDULE_FALLBACK_MS);
                self.enqueue_task(task_id, Duration::from_millis(delay))?;
                Ok(())
            }
            LoopxCliRunDecision::WaitingForUser => {
                let gate = inspected.pending_user_gate.ok_or_else(|| {
                    "LoopX requested a user decision without an answerable gate".to_string()
                })?;
                let LoopxCliUserGate {
                    gate_id,
                    message,
                    action_kind,
                } = gate;
                let durable_revision = inspected.durable_revision.clone();
                self.release_repository(&task).await;
                let updated = self
                    .mutate_task(&task_id, None, |current, current_runtime| {
                        if current.generation != task.generation {
                            return;
                        }
                        current.state = LoopxTaskState::WaitingForUser;
                        current.phase = LoopxPhase::WaitingForApproval;
                        current.pending_gate_id = Some(gate_id.clone());
                        current.pending_gate_message = Some(message.clone());
                        current.pending_gate_action_kind = action_kind.clone();
                        current.revision = current.revision.saturating_add(1);
                        current_runtime.expected_durable_revision = Some(durable_revision.clone());
                    })
                    .await?;
                let mut details = BTreeMap::new();
                details.insert("gateId".to_string(), gate_id.clone());
                if let Some(action_kind) = action_kind.clone() {
                    details.insert("actionKind".to_string(), action_kind);
                }
                self.append_task_event_with_details(
                    &updated,
                    LoopxEventKind::ApprovalRequired,
                    &message,
                    true,
                    details,
                )
                .await?;
                self.schedule_next_for_repository(
                    &task.identity.item.repository.canonical_id(),
                    Some(&task_id),
                )
                .await;
                Ok(())
            }
            LoopxCliRunDecision::Complete => {
                return self
                    .complete_finished_goal(&task, &runtime, "LoopX goal completed")
                    .await;
            }
            LoopxCliRunDecision::Failed => {
                self.release_repository(&task).await;
                self.fail_task(&task_id, "LoopX reported a failed goal".to_string())
                    .await
            }
            LoopxCliRunDecision::RunNow => {
                if inspected.open_todo_count == 0
                    && inspected.waiting_user_todo_count == 0
                    && inspected.open_agent_todos.is_empty()
                {
                    // LoopX keeps a Goal eligible forever even when nothing is
                    // runnable: it re-selects unclaimed candidate todos and
                    // never reaches `terminal_no_followup` on this integration
                    // path. With no open todos there is nothing to run, so
                    // finish the host task and stop the Goal's heartbeat
                    // instead of requeueing it forever.
                    return self
                        .complete_finished_goal(
                            &task,
                            &runtime,
                            "LoopX Goal has no open todos left; BitFun completed the task",
                        )
                        .await;
                }
                if inspected.envelope_over_budget {
                    // LoopX rejected its own turn envelope (compaction budget):
                    // the Goal cannot be planned until its durable state
                    // shrinks. Degrade to a loud queued backoff instead of
                    // failing the task into recovery, which would loop forever
                    // (every rebuild hits the same rejection) and would forge
                    // an agent failure that never happened. Bounded self-heal:
                    // shorten over-long open todo texts back to the documented
                    // bound first; that has been observed bringing an
                    // over-budget envelope back inside it.
                    let shrink_progress = BufferedProgress::default();
                    let shrink = self
                        .cli
                        .shrink_todo_texts(
                            LoopxCliShrinkTodoTextsRequest {
                                context: goal_context(&task, &runtime),
                                goal_id: task.goal_id.clone().unwrap_or_default(),
                                agent_id: task
                                    .agent_id
                                    .clone()
                                    .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
                                max_chars: MAX_HOST_FACT_TODO_CHARS as u32,
                            },
                            &shrink_progress,
                        )
                        .await;
                    match &shrink {
                        Ok(result) => {
                            self.record_progress(shrink_progress.take()).await?;
                            log::info!(
                                "LoopX todo text shrink applied: task_id={}, examined={}, shortened={}",
                                task.task_id,
                                result.examined,
                                result.shortened
                            );
                        }
                        Err(error) => {
                            let _ = shrink_progress.take();
                            log::warn!(
                                "LoopX todo text shrink skipped for task {}: {}",
                                task.task_id,
                                error
                            );
                        }
                    }
                    let re_inspected = self
                        .cli
                        .inspect_goal(
                            LoopxCliInspectGoalRequest {
                                context: goal_context(&task, &runtime),
                                goal_id: task.goal_id.clone().unwrap_or_default(),
                                agent_id: task
                                    .agent_id
                                    .clone()
                                    .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
                            },
                            &BufferedProgress::default(),
                        )
                        .await;
                    if let Ok(retry) = re_inspected {
                        if !retry.envelope_over_budget {
                            log::info!(
                                "LoopX turn envelope back within budget after todo shrink: task_id={}",
                                task.task_id
                            );
                            inspected = retry;
                            return Ok(());
                        }
                        log::warn!(
                            "LoopX turn envelope still over budget after todo shrink: task_id={}",
                            task.task_id
                        );
                    }
                    self.release_repository(&task).await;
                    let message = "LoopX turn envelope exceeded its compaction budget (route contract_error); the Goal durable state for this Issue must shrink before work can resume. BitFun keeps the task queued and retries with backoff.";
                    let updated = self
                        .transition_task(
                            &task_id,
                            task.generation,
                            LoopxTaskState::Queued,
                            LoopxPhase::Queued,
                            message,
                        )
                        .await?;
                    self.append_task_event(&updated, LoopxEventKind::StateChanged, message, true)
                        .await?;
                    self.schedule_next_for_repository(
                        &updated.identity.item.repository.canonical_id(),
                        Some(&updated.task_id),
                    )
                    .await;
                    self.enqueue_task(task_id, Duration::from_millis(WAIT_RESCHEDULE_FALLBACK_MS))?;
                    return Ok(());
                }
                if task.state == LoopxTaskState::RecoveryRequired {
                    // Restart-interrupted runs land here; the owner decides
                    // via the explicit recovery action in the UI (nothing
                    // silent, nothing forged, worktree and evidence kept).
                    self.release_repository(&task).await;
                    return Ok(());
                }
                let playbook_path = self.ensure_playbook_path(&task).await;
                let goal_facts =
                    host_turn_context_block(&task, &inspected, playbook_path.as_deref());
                let progress = BufferedProgress::default();
                let turn = self
                    .cli
                    .build_turn(
                        LoopxCliBuildTurnRequest {
                            context: goal_context(&task, &runtime),
                            goal_id: task.goal_id.clone().unwrap_or_default(),
                            agent_id: task
                                .agent_id
                                .clone()
                                .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
                            expected_durable_revision: inspected.durable_revision,
                        },
                        &progress,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                self.record_progress(progress.take()).await?;
                self.bind_turn(&task, &turn).await?;
                let started = self
                    .agent
                    .start(LoopxAgentStartRequest {
                        operation_id: format!("agent-{}-{}", task.task_id, task.generation),
                        task_id: task.task_id.clone(),
                        generation: task.generation,
                        worktree_path: task.workspace_path.clone().unwrap_or_default(),
                        prompt: goal_facts + turn.prompt.as_str(),
                        model_id: task.model_id.clone().unwrap_or_else(|| "auto".to_string()),
                        metadata: LoopxAgentTurnMetadata {
                            goal_id: task.goal_id.clone().unwrap_or_default(),
                            loopx_turn_id: turn.turn_id,
                            item: task.identity.item.clone(),
                            attempt: task.identity.attempt,
                        },
                    })
                    .await
                    .map_err(|error| error.to_string())?;
                self.bind_agent_run(&task, started).await
            }
        }
    }

    /// Best-effort teardown of a task's agent run (cancel then finish). A stale
    /// session — for example one persisted before a host restart — must not
    /// abort pause or reset. The local record owns only host-job cleanup; LoopX
    /// remains authoritative for Goal lifecycle, so teardown failures are
    /// logged and the operator action continues.
    async fn teardown_agent_run(
        self: &Arc<Self>,
        task: &LoopxTaskSnapshot,
        runtime: &LoopxTaskRuntimeRecord,
    ) {
        let (Some(session_id), Some(turn_id)) =
            (runtime.session_id.as_ref(), runtime.agent_turn_id.as_ref())
        else {
            return;
        };
        if let Err(error) = self
            .agent
            .cancel(LoopxAgentCancelRequest {
                operation_id: format!("teardown-agent-{}", uuid::Uuid::new_v4()),
                target_operation_id: runtime.operation_id.clone(),
                task_id: task.task_id.clone(),
                generation: task.generation,
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
            })
            .await
        {
            log::warn!(
                "LoopX agent cancel skipped for task {}: {}",
                task.task_id,
                error
            );
        }
        if let Err(error) = self
            .agent
            .finish(LoopxAgentFinishRequest {
                operation_id: format!("teardown-agent-finish-{}", uuid::Uuid::new_v4()),
                task_id: task.task_id.clone(),
                generation: task.generation,
                worktree_path: task.workspace_path.clone().unwrap_or_default(),
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
            })
            .await
        {
            log::warn!(
                "LoopX agent finish skipped for task {}: {}",
                task.task_id,
                error
            );
        }
    }

    async fn pause_task(
        self: &Arc<Self>,
        task: &LoopxTaskSnapshot,
        runtime: &LoopxTaskRuntimeRecord,
        request_id: &str,
    ) -> Result<LoopxActionResponse, String> {
        self.transition_task(
            &task.task_id,
            task.generation,
            LoopxTaskState::Cancelling,
            LoopxPhase::Cancelling,
            "Cancelling the active LoopX task",
        )
        .await?;
        self.teardown_agent_run(task, runtime).await;
        let progress = BufferedProgress::default();
        let _ = self
            .cli
            .cancel(
                LoopxCliCancelRequest {
                    call: LoopxCliCallContext {
                        operation_id: format!("cancel-cli-{}", uuid::Uuid::new_v4()),
                        deadline_at: None,
                    },
                    target_operation_id: runtime.operation_id.clone(),
                },
                &progress,
            )
            .await;
        self.record_progress(progress.take()).await?;
        self.release_repository(task).await;
        let response = self
            .transition_action(
                &task.task_id,
                LoopxTaskState::Stopped,
                LoopxPhase::Finished,
                request_id,
            )
            .await?;
        self.schedule_next_for_repository(
            &task.identity.item.repository.canonical_id(),
            Some(&task.task_id),
        )
        .await;
        Ok(response)
    }

    async fn abort_task(
        self: &Arc<Self>,
        task: &LoopxTaskSnapshot,
        runtime: &LoopxTaskRuntimeRecord,
        request_id: &str,
    ) -> Result<LoopxActionResponse, String> {
        self.transition_task(
            &task.task_id,
            task.generation,
            LoopxTaskState::Cancelling,
            LoopxPhase::Cancelling,
            "Aborting the active LoopX task",
        )
        .await?;
        self.teardown_agent_run(task, runtime).await;
        let progress = BufferedProgress::default();
        let _ = self
            .cli
            .cancel(
                LoopxCliCancelRequest {
                    call: LoopxCliCallContext {
                        operation_id: format!("abort-cli-{}", uuid::Uuid::new_v4()),
                        deadline_at: None,
                    },
                    target_operation_id: runtime.operation_id.clone(),
                },
                &progress,
            )
            .await;
        self.record_progress(progress.take()).await?;
        self.release_repository(task).await;
        let response = self
            .transition_action(
                &task.task_id,
                LoopxTaskState::Aborted,
                LoopxPhase::Finished,
                request_id,
            )
            .await?;
        self.schedule_next_for_repository(
            &task.identity.item.repository.canonical_id(),
            Some(&task.task_id),
        )
        .await;
        Ok(response)
    }

    async fn resume_task(
        self: &Arc<Self>,
        task: &LoopxTaskSnapshot,
        request_id: &str,
    ) -> Result<LoopxActionResponse, String> {
        if !matches!(
            task.state,
            LoopxTaskState::Stopped | LoopxTaskState::Failed | LoopxTaskState::RecoveryRequired
        ) {
            return Ok(LoopxActionResponse {
                status: LoopxActionStatus::Rejected,
                current_revision: task.revision,
                task: Some(task.clone()),
                message: Some(
                    "Only stopped, failed, or recovery-required tasks can resume".to_string(),
                ),
            });
        }
        let updated = self
            .mutate_task(&task.task_id, Some(request_id), |task, runtime| {
                task.generation = task.generation.saturating_add(1);
                task.revision = task.revision.saturating_add(1);
                task.state = LoopxTaskState::Queued;
                task.phase = LoopxPhase::Recovering;
                task.current_turn_id = None;
                task.pending_gate_id = None;
                task.pending_gate_message = None;
                task.pending_gate_action_kind = None;
                task.error = None;
                task.autonomous_turns_since_review = 0;
                runtime.operation_id = format!("resume-{}-{}", task.task_id, task.generation);
                runtime.session_id = None;
                runtime.agent_turn_id = None;
                runtime.loopx_turn_id = None;
                runtime.settlement_token = None;
                runtime.expected_durable_revision = None;
            })
            .await?;
        let task_id = task.task_id.clone();
        self.enqueue_task(task_id, Duration::ZERO)?;
        Ok(LoopxActionResponse {
            current_revision: updated.revision,
            task: Some(updated),
            ..LoopxActionResponse::default()
        })
    }

    async fn resume_repository(
        self: &Arc<Self>,
        request: &LoopxActionRequest,
    ) -> Result<LoopxActionResponse, String> {
        let repository = request
            .repository
            .as_ref()
            .ok_or_else(|| "repository is required for resume_repository".to_string())?;
        let repository_id = repository.canonical_id();
        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        if state.has_processed_request(&request.client_request_id) {
            return Ok(LoopxActionResponse {
                status: LoopxActionStatus::Duplicate,
                current_revision: state.revision,
                message: Some("Repository resume was already applied".to_string()),
                ..LoopxActionResponse::default()
            });
        }
        if state.revision != request.expected_revision {
            return Ok(LoopxActionResponse {
                status: LoopxActionStatus::RevisionConflict,
                current_revision: state.revision,
                message: Some(
                    "Task list changed; refresh before resuming the repository".to_string(),
                ),
                ..LoopxActionResponse::default()
            });
        }

        let task_indexes = state
            .tasks
            .iter()
            .enumerate()
            .filter_map(|(index, task)| {
                is_repository_recovery_candidate(task, &repository_id).then_some(index)
            })
            .collect::<Vec<_>>();
        let start_cursor = state.cursor;
        let now = now_ms();
        let mut task_ids = Vec::with_capacity(task_indexes.len());
        for task_index in task_indexes {
            let task_id = state.tasks[task_index].task_id.clone();
            let mut runtime = state.runtime.remove(&task_id).unwrap_or_default();
            {
                let task = &mut state.tasks[task_index];
                task.generation = task.generation.saturating_add(1);
                task.revision = task.revision.saturating_add(1);
                task.state = LoopxTaskState::Queued;
                task.phase = LoopxPhase::Recovering;
                task.current_turn_id = None;
                task.error = None;
                task.updated_at = now;
                runtime.operation_id = format!("resume-{}-{}", task.task_id, task.generation);
                runtime.session_id = None;
                runtime.agent_turn_id = None;
                runtime.loopx_turn_id = None;
                runtime.settlement_token = None;
                runtime.expected_durable_revision = None;
            }
            let updated = state.tasks[task_index].clone();
            state.runtime.insert(task_id.clone(), runtime);
            state.revision = state.revision.saturating_add(1);
            state.append_event(LoopxEvent {
                task_id: Some(task_id.clone()),
                generation: Some(updated.generation),
                revision: Some(updated.revision),
                kind: LoopxEventKind::StateChanged,
                source: LoopxEventSource::Controller,
                phase: Some(LoopxPhase::Recovering),
                message: "Task queued by repository resume".to_string(),
                occurred_at: now,
                ..LoopxEvent::default()
            });
            task_ids.push(task_id);
        }
        state.record_processed_request(request.client_request_id.clone());
        let resumed_count = task_ids.len();
        let current_revision = state.revision;
        let persisted = state.clone();
        drop(state);
        self.store.save(&persisted).await?;
        drop(_mutation);
        self.broadcast_new_events(&persisted, start_cursor);
        for task_id in task_ids {
            self.enqueue_task(task_id, Duration::ZERO)?;
        }
        Ok(LoopxActionResponse {
            current_revision,
            message: Some(format!("Queued {resumed_count} repository tasks")),
            ..LoopxActionResponse::default()
        })
    }

    async fn reset_all(
        self: &Arc<Self>,
        request: &LoopxActionRequest,
    ) -> Result<LoopxActionResponse, String> {
        if self
            .reset_in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(LoopxActionResponse {
                status: LoopxActionStatus::Duplicate,
                current_revision: self.state.read().await.revision,
                message: Some("LoopX reset is already in progress".to_string()),
                ..LoopxActionResponse::default()
            });
        }
        let _reset_guard = InProgressGuard(&self.reset_in_progress);
        let (tasks, runtimes, previous_stream_id, environment) = {
            let _mutation = self.mutation_lock.lock().await;
            let mut state = self.state.write().await;
            if state.has_processed_request(&request.client_request_id) {
                return Ok(LoopxActionResponse {
                    status: LoopxActionStatus::Duplicate,
                    current_revision: state.revision,
                    message: Some("LoopX reset was already applied".to_string()),
                    ..LoopxActionResponse::default()
                });
            }
            if state.revision != request.expected_revision {
                return Ok(LoopxActionResponse {
                    status: LoopxActionStatus::RevisionConflict,
                    current_revision: state.revision,
                    message: Some("LoopX state changed; refresh before resetting".to_string()),
                    ..LoopxActionResponse::default()
                });
            }
            let tasks = state.tasks.clone();
            let runtimes = state.runtime.clone();
            let environment = state.environment.clone();
            for task in &mut state.tasks {
                if matches!(
                    task.state,
                    LoopxTaskState::Preparing
                        | LoopxTaskState::Queued
                        | LoopxTaskState::Running
                        | LoopxTaskState::RetryWait
                        | LoopxTaskState::Cancelling
                ) {
                    task.state = LoopxTaskState::Cancelling;
                    task.phase = LoopxPhase::Cancelling;
                    task.revision = task.revision.saturating_add(1);
                    task.updated_at = now_ms();
                }
            }
            state.record_processed_request(request.client_request_id.clone());
            state.revision = state.revision.saturating_add(1);
            let persisted = state.clone();
            let previous_stream_id = state.stream_id.clone();
            drop(state);
            self.store.save(&persisted).await?;
            (tasks, runtimes, previous_stream_id, environment)
        };

        for task in &tasks {
            let runtime = runtimes.get(&task.task_id).cloned().unwrap_or_default();
            self.teardown_agent_run(task, &runtime).await;
            if !runtime.operation_id.trim().is_empty() {
                let progress = BufferedProgress::default();
                let _ = self
                    .cli
                    .cancel(
                        LoopxCliCancelRequest {
                            call: LoopxCliCallContext {
                                operation_id: format!("reset-cli-{}", uuid::Uuid::new_v4()),
                                deadline_at: None,
                            },
                            target_operation_id: runtime.operation_id.clone(),
                        },
                        &progress,
                    )
                    .await;
            }
            let _ = self
                .workspace
                .cancel(LoopxWorkspaceCancelRequest {
                    operation_id: format!("reset-workspace-{}", uuid::Uuid::new_v4()),
                    target_operation_id: format!("workspace-{}-{}", task.task_id, task.generation),
                    task_id: task.task_id.clone(),
                })
                .await;
        }

        self.workspace
            .reset(LoopxWorkspaceResetRequest {
                operation_id: format!("reset-workspaces-{}", uuid::Uuid::new_v4()),
            })
            .await
            .map_err(|error| error.to_string())?;
        let goal_ids = tasks
            .iter()
            .map(|task| goal_id_for(&task.identity))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let reset_goals = if goal_ids.is_empty() {
            LoopxCliResetGoalsResult::default()
        } else {
            let progress = BufferedProgress::default();
            let result = self
                .cli
                .reset_goals(
                    LoopxCliResetGoalsRequest {
                        call: LoopxCliCallContext {
                            operation_id: format!("reset-goals-{}", uuid::Uuid::new_v4()),
                            deadline_at: None,
                        },
                        goal_ids,
                    },
                    &progress,
                )
                .await
                .map_err(|error| error.to_string())?;
            self.record_progress(progress.take()).await?;
            result
        };
        log::info!(
            "LoopX reset goal cleanup completed: requested={}, retired={}, already_absent={}, archived={}, missing_runtime={}",
            reset_goals.requested_goal_ids.len(),
            reset_goals.retired_goal_ids.len(),
            reset_goals.already_absent_goal_ids.len(),
            reset_goals.archived_goal_ids.len(),
            reset_goals.missing_runtime_goal_ids.len()
        );
        self.agent
            .reset(LoopxAgentResetRequest {
                operation_id: format!("reset-history-{}", uuid::Uuid::new_v4()),
            })
            .await
            .map_err(|error| error.to_string())?;

        let mut fresh = LoopxPersistedState::new(now_ms());
        fresh.environment = environment;
        self.store.clear().await?;
        {
            let _mutation = self.mutation_lock.lock().await;
            *self.state.write().await = fresh.clone();
            self.active_tasks.lock().await.clear();
            self.active_repositories.lock().await.clear();
            self.previews.write().await.clear();
            *self.load_error.write().await = None;
        }
        let _ = self.event_sender.send(LoopxEvent {
            stream_id: fresh.stream_id,
            cursor: 0,
            kind: LoopxEventKind::SnapshotInvalidated,
            level: LoopxEventLevel::Info,
            source: LoopxEventSource::Controller,
            message: format!("LoopX reset replaced stream {previous_stream_id}"),
            occurred_at: now_ms(),
            ..LoopxEvent::default()
        });
        Ok(LoopxActionResponse {
            current_revision: fresh.revision,
            message: Some(format!(
                "Cleared {} LoopX tasks, {} global goal routes, managed workspaces, runtime state, and persisted controller state",
                tasks.len(),
                reset_goals.retired_goal_ids.len()
                    + reset_goals.already_absent_goal_ids.len()
            )),
            ..LoopxActionResponse::default()
        })
    }

    async fn answer_gate(
        self: &Arc<Self>,
        task: &LoopxTaskSnapshot,
        runtime: &LoopxTaskRuntimeRecord,
        request: &LoopxActionRequest,
    ) -> Result<LoopxActionResponse, String> {
        let gate_id = request
            .gate_id
            .clone()
            .ok_or_else(|| "gateId is required".to_string())?;
        let is_autonomy_review = self.is_autonomy_review_gate(&task.task_id, &gate_id).await;
        let progress = BufferedProgress::default();
        let result = self
            .cli
            .answer_gate(
                LoopxCliAnswerGateRequest {
                    context: goal_context(task, runtime),
                    goal_id: task.goal_id.clone().unwrap_or_default(),
                    agent_id: task
                        .agent_id
                        .clone()
                        .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
                    gate_id,
                    decision: if request.action == LoopxActionKind::Approve {
                        LoopxCliGateDecision::Approve
                    } else {
                        LoopxCliGateDecision::Reject
                    },
                    note: request.note.clone(),
                    granted_scope: None,
                },
                &progress,
            )
            .await
            .map_err(|error| error.to_string())?;
        self.record_progress(progress.take()).await?;
        if !result.applied {
            return Ok(LoopxActionResponse {
                status: LoopxActionStatus::Rejected,
                current_revision: task.revision,
                task: Some(task.clone()),
                message: Some("LoopX did not apply the gate decision".to_string()),
            });
        }
        self.record_goal_state(task, result.goal_state).await?;
        self.mutate_task(&task.task_id, None, |current, current_runtime| {
            if current.generation != task.generation {
                return;
            }
            current.autonomous_turns_since_review = 0;
            current.stagnant_settlements_since_review = 0;
            current.autonomy_review_baseline_receipts = result.settlement_receipt_count;
            current.revision = current.revision.saturating_add(1);
            current_runtime.expected_durable_revision = Some(result.durable_revision.clone());
        })
        .await?;
        let stop_after_reject = is_autonomy_review && request.action == LoopxActionKind::Reject;
        let response = self
            .transition_action(
                &task.task_id,
                if stop_after_reject {
                    LoopxTaskState::Stopped
                } else {
                    LoopxTaskState::Queued
                },
                if stop_after_reject {
                    LoopxPhase::Finished
                } else {
                    LoopxPhase::Queued
                },
                &request.client_request_id,
            )
            .await?;
        self.release_repository(task).await;
        if stop_after_reject {
            if let Some(updated) = response.task.as_ref() {
                self.append_task_event(
                    updated,
                    LoopxEventKind::StateChanged,
                    "Owner stopped autonomous investigation at the review gate",
                    true,
                )
                .await?;
            }
            self.schedule_next_for_repository(
                &task.identity.item.repository.canonical_id(),
                Some(&task.task_id),
            )
            .await;
        } else {
            self.enqueue_task(task.task_id.clone(), Duration::ZERO)?;
        }
        Ok(response)
    }

    /// Best-effort repository playbook path for the task's turn prompt. A
    /// failure must never block the turn; the heartbeat task body remains the
    /// authority for work selection.
    async fn ensure_playbook_path(&self, task: &LoopxTaskSnapshot) -> Option<String> {
        let Some(worktree) = task.workspace_path.as_deref() else {
            return None;
        };
        let request = LoopxWorkspacePlaybookRequest {
            operation_id: format!("playbook-{}-{}", task.task_id, uuid::Uuid::new_v4()),
            worktree_path: worktree.to_string(),
        };
        match self.workspace.ensure_playbook(request).await {
            Ok(result) if !result.playbook_path.is_empty() => Some(result.playbook_path),
            Ok(_) => None,
            Err(error) => {
                log::warn!(
                    "LoopX repository playbook unavailable for task {}: {}",
                    task.task_id,
                    error
                );
                None
            }
        }
    }

    async fn is_autonomy_review_gate(&self, task_id: &str, gate_id: &str) -> bool {
        self.state.read().await.events.iter().rev().any(|event| {
            event.task_id.as_deref() == Some(task_id)
                && event.kind == LoopxEventKind::ApprovalRequired
                && event.details.get("gateId").map(String::as_str) == Some(gate_id)
                && event.details.get("actionKind").map(String::as_str)
                    == Some(AUTONOMY_REVIEW_ACTION_KIND)
        })
    }

    async fn apply_settlement(
        self: &Arc<Self>,
        task: &LoopxTaskSnapshot,
        settlement: LoopxCliSettleTurnResult,
        agent_status: LoopxAgentTurnStatus,
        failure_summary: Option<&str>,
        blocks_repository: bool,
    ) -> Result<(), String> {
        let final_state = if agent_status == LoopxAgentTurnStatus::Failed {
            LoopxTaskState::RecoveryRequired
        } else {
            match settlement.status {
                LoopxCliSettlementStatus::GoalCompleted => LoopxTaskState::Completed,
                LoopxCliSettlementStatus::Settled | LoopxCliSettlementStatus::AlreadySettled => {
                    LoopxTaskState::Queued
                }
                LoopxCliSettlementStatus::NoDurableProgress
                | LoopxCliSettlementStatus::RetryRequired => LoopxTaskState::RecoveryRequired,
            }
        };
        let phase = if final_state == LoopxTaskState::Completed {
            LoopxPhase::Finished
        } else if final_state == LoopxTaskState::RecoveryRequired {
            LoopxPhase::Recovering
        } else {
            LoopxPhase::Queued
        };
        // Output-dimension evidence for the convergence gate. The probe is
        // read-only and best-effort: a probe failure must not fail the
        // settlement, and treats the turn as non-stagnant (unchanged counter
        // would unfairly punish the next turn, so it resets to keep the gate
        // conservative).
        let mut non_bookkeeping_changes: Option<bool> = None;
        if final_state == LoopxTaskState::Queued
            && agent_status != LoopxAgentTurnStatus::Failed
            && settlement.status == LoopxCliSettlementStatus::Settled
        {
            let request = LoopxWorkspaceMutationsRequest {
                operation_id: format!("mutations-{}-{}", task.task_id, uuid::Uuid::new_v4()),
                worktree_path: task.workspace_path.clone().unwrap_or_default(),
            };
            if !request.worktree_path.trim().is_empty() {
                match self.workspace.probe_mutations(request).await {
                    Ok(result) => non_bookkeeping_changes = Some(result.has_changes),
                    Err(error) => {
                        log::warn!(
                            "LoopX worktree mutation probe skipped for task {}: {}",
                            task.task_id,
                            error
                        );
                    }
                }
            }
        }
        let updated = self
            .mutate_task(&task.task_id, None, |task, runtime| {
                task.state = final_state;
                task.phase = phase;
                task.goal_state = Some(match settlement.status {
                    LoopxCliSettlementStatus::GoalCompleted => LoopxCliGoalState::Completed,
                    _ => LoopxCliGoalState::Active,
                });
                task.revision = task.revision.saturating_add(1);
                task.current_turn_id = None;
                task.pending_gate_id = None;
                task.pending_gate_message = None;
                task.pending_gate_action_kind = None;
                task.deadline_at = None;
                task.error = (agent_status == LoopxAgentTurnStatus::Failed)
                    .then(|| failure_summary.unwrap_or("Agent turn failed").to_string());
                task.settlement = LoopxSettlementSummary {
                    turn_id: Some(settlement.turn_id.clone()),
                    receipt_id: Some(settlement.receipt_id.clone()),
                    durable_revision: Some(settlement.after_revision.clone()),
                    settled_at: Some(now_ms()),
                };
                if final_state == LoopxTaskState::Queued {
                    task.autonomous_turns_since_review =
                        task.autonomous_turns_since_review.saturating_add(1);
                    match non_bookkeeping_changes {
                        Some(true) | None => task.stagnant_settlements_since_review = 0,
                        Some(false) => {
                            task.stagnant_settlements_since_review =
                                task.stagnant_settlements_since_review.saturating_add(1);
                        }
                    }
                    match settlement.binding_todo_id.as_deref() {
                        Some(todo_id)
                            if Some(todo_id) == task.current_settlement_todo_id.as_deref() =>
                        {
                            task.settlements_on_current_todo =
                                task.settlements_on_current_todo.saturating_add(1);
                        }
                        Some(todo_id) => {
                            task.settlements_on_current_todo = 1;
                            task.current_settlement_todo_id = Some(todo_id.to_string());
                        }
                        None => {
                            task.settlements_on_current_todo = 0;
                            task.current_settlement_todo_id = None;
                        }
                    }
                } else if final_state == LoopxTaskState::Completed {
                    task.autonomous_turns_since_review = 0;
                    task.stagnant_settlements_since_review = 0;
                    task.settlements_on_current_todo = 0;
                    task.current_settlement_todo_id = None;
                }
                runtime.session_id = None;
                runtime.agent_turn_id = None;
                runtime.expected_durable_revision = Some(settlement.after_revision.clone());
            })
            .await?;
        log::info!(
            "LoopX task settlement applied: task_id={}, goal_id={}, final_state={:?}, phase={:?}, settlement_status={:?}",
            updated.task_id,
            updated.goal_id.as_deref().unwrap_or("unknown"),
            updated.state,
            updated.phase,
            settlement.status
        );
        self.release_repository(&updated).await;
        let mut yielded_repository = false;
        if agent_status == LoopxAgentTurnStatus::Failed {
            let reason = failure_summary.unwrap_or("Agent turn failed");
            self.append_task_event(&updated, LoopxEventKind::StateChanged, reason, true)
                .await?;
            if blocks_repository {
                self.pause_repository_after_failure(&updated, reason)
                    .await?;
            }
        } else {
            let (kind, message, important) = match final_state {
                LoopxTaskState::Completed => (
                    LoopxEventKind::SettlementRecorded,
                    "LoopX goal completed",
                    false,
                ),
                LoopxTaskState::RecoveryRequired => (
                    LoopxEventKind::StateChanged,
                    "LoopX turn requires recovery after settlement",
                    true,
                ),
                _ => (
                    LoopxEventKind::SettlementRecorded,
                    "LoopX turn settlement recorded",
                    false,
                ),
            };
            self.append_task_event(&updated, kind, message, important)
                .await?;
            yielded_repository = self
                .schedule_next_for_repository(
                    &updated.identity.item.repository.canonical_id(),
                    Some(&updated.task_id),
                )
                .await;
            if yielded_repository {
                self.suppress_pending_task_rerun(&updated.task_id).await;
            }
        }
        if should_requeue_after_settlement(final_state, yielded_repository) {
            let task_id = task.task_id.clone();
            let delay = settlement.scheduler_hint_ms.unwrap_or(0);
            self.enqueue_task(task_id, Duration::from_millis(delay))?;
        }
        Ok(())
    }

    async fn pause_repository_after_failure(
        &self,
        failed_task: &LoopxTaskSnapshot,
        reason: &str,
    ) -> Result<(), String> {
        let repository_id = failed_task.identity.item.repository.canonical_id();
        let message = format!(
            "Repository queue paused after Issue #{} failed: {}",
            failed_task.identity.item.number,
            reason.chars().take(700).collect::<String>()
        );
        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        let start_cursor = state.cursor;
        let now = now_ms();
        let mut paused = Vec::new();
        for task in &mut state.tasks {
            if task.task_id == failed_task.task_id
                || task.identity.item.repository.canonical_id() != repository_id
                || task.state != LoopxTaskState::Queued
            {
                continue;
            }
            task.state = LoopxTaskState::RecoveryRequired;
            task.phase = LoopxPhase::Recovering;
            task.error = Some(message.clone());
            task.current_turn_id = None;
            task.deadline_at = None;
            task.revision = task.revision.saturating_add(1);
            task.updated_at = now;
            paused.push(task.clone());
        }
        for task in &paused {
            state.revision = state.revision.saturating_add(1);
            state.append_event(LoopxEvent {
                task_id: Some(task.task_id.clone()),
                generation: Some(task.generation),
                revision: Some(task.revision),
                kind: LoopxEventKind::StateChanged,
                level: LoopxEventLevel::Error,
                source: LoopxEventSource::Controller,
                phase: Some(LoopxPhase::Recovering),
                message: message.clone(),
                important: true,
                occurred_at: now,
                ..LoopxEvent::default()
            });
        }
        state.environment.core.agent_model.status = LoopxEnvironmentFactStatus::Degraded;
        state.environment.core.agent_model.detail = Some(reason.to_string());
        state.environment.core.agent_model.checked_at = Some(now);
        state.environment.status =
            derive_environment_status(&state.environment.core, &state.environment.optional);
        state.environment.revision = state.environment.revision.saturating_add(1);
        state.revision = state.revision.saturating_add(1);
        state.append_event(LoopxEvent {
            kind: LoopxEventKind::EnvironmentChanged,
            level: LoopxEventLevel::Error,
            source: LoopxEventSource::System,
            message: "Agent model runtime failed; repository queue paused".to_string(),
            important: true,
            occurred_at: now,
            ..LoopxEvent::default()
        });
        let persisted = state.clone();
        drop(state);
        self.store.save(&persisted).await?;
        drop(_mutation);
        self.broadcast_new_events(&persisted, start_cursor);
        Ok(())
    }

    async fn reserve_repository(&self, task: &LoopxTaskSnapshot) -> bool {
        let repo = task.identity.item.repository.canonical_id();
        let mut active = self.active_repositories.lock().await;
        match active.get(&repo) {
            Some(owner) => owner == &task.task_id,
            None => {
                active.insert(repo, task.task_id.clone());
                true
            }
        }
    }

    async fn release_repository(&self, task: &LoopxTaskSnapshot) {
        let repo = task.identity.item.repository.canonical_id();
        let mut active = self.active_repositories.lock().await;
        if active.get(&repo) == Some(&task.task_id) {
            active.remove(&repo);
        }
    }

    async fn mark_environment_checking(&self) -> Result<(), String> {
        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        let checked_at = Some(now_ms());
        state.environment.revision = state.environment.revision.saturating_add(1);
        state.environment.status = LoopxEnvironmentStatus::Checking;
        state.environment.checked_at = checked_at;
        state.environment.core.sidecar = checking_environment_fact(checked_at);
        state.environment.core.git_worktree = checking_environment_fact(checked_at);
        state.environment.core.agent_model = checking_environment_fact(checked_at);
        state.environment.optional.open_viking = checking_environment_fact(checked_at);
        state.environment.optional.feedback_memory = checking_environment_fact(checked_at);
        state.environment.optional.github_auth = checking_environment_fact(checked_at);
        state.revision = state.revision.saturating_add(1);
        state.append_event(LoopxEvent {
            kind: LoopxEventKind::EnvironmentChanged,
            source: LoopxEventSource::System,
            message: "LoopX environment validation started".to_string(),
            occurred_at: now_ms(),
            ..LoopxEvent::default()
        });
        let persisted = state.clone();
        let event = persisted.events.last().cloned();
        drop(state);
        self.store.save(&persisted).await?;
        if let Some(event) = event {
            let _ = self.event_sender.send(event);
        }
        Ok(())
    }

    async fn commit_environment(
        &self,
        handshake: LoopxCliResult<LoopxCliManifest>,
        workspace: LoopxHostResult<LoopxWorkspaceProbeResult>,
        agent: LoopxHostResult<LoopxAgentProbeResult>,
        open_viking: LoopxCliResult<LoopxOpenVikingProbe>,
        github_auth: LoopxGithubAuthProbe,
    ) -> Result<(), String> {
        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        let checked_at = Some(now_ms());
        let (sidecar, python_fallback) = match handshake {
            Ok(manifest) => {
                let python_fallback =
                    if manifest.executable.source == LoopxCliSource::PythonFallback {
                        LoopxEnvironmentFact {
                            status: LoopxEnvironmentFactStatus::Available,
                            version: Some("Python 3.11+".to_string()),
                            detail: Some(
                                "Managed LoopX source runs in isolated Python mode".to_string(),
                            ),
                            checked_at,
                            ..LoopxEnvironmentFact::default()
                        }
                    } else {
                        LoopxEnvironmentFact {
                            status: LoopxEnvironmentFactStatus::Unknown,
                            detail: Some("Not required by the selected LoopX runtime".to_string()),
                            checked_at,
                            ..LoopxEnvironmentFact::default()
                        }
                    };
                (
                    LoopxEnvironmentFact {
                        status: LoopxEnvironmentFactStatus::Available,
                        version: Some(manifest.loopx_version),
                        detail: Some(manifest.executable.identity),
                        checked_at,
                        ..LoopxEnvironmentFact::default()
                    },
                    python_fallback,
                )
            }
            Err(error)
                if matches!(
                    error.kind,
                    LoopxCliErrorKind::NotFound | LoopxCliErrorKind::VersionMismatch
                ) =>
            {
                (
                    unavailable_loopx_environment_fact(error.to_string(), checked_at),
                    LoopxEnvironmentFact::default(),
                )
            }
            Err(error) => (
                unavailable_environment_fact(error.to_string(), checked_at),
                LoopxEnvironmentFact::default(),
            ),
        };
        let git_worktree = match workspace {
            Ok(probe) => LoopxEnvironmentFact {
                status: LoopxEnvironmentFactStatus::Available,
                version: probe.git_version,
                detail: Some(format!("Writable workspace root: {}", probe.workspace_root)),
                checked_at,
                ..LoopxEnvironmentFact::default()
            },
            Err(error) => unavailable_environment_fact(error.to_string(), checked_at),
        };
        let agent_model = match agent {
            Ok(probe) => LoopxEnvironmentFact {
                status: LoopxEnvironmentFactStatus::Available,
                version: Some(probe.model_id),
                detail: Some("Configured Agent model is enabled for text chat".to_string()),
                checked_at,
                ..LoopxEnvironmentFact::default()
            },
            Err(error) => unavailable_environment_fact(error.to_string(), checked_at),
        };
        let github_auth = LoopxEnvironmentFact {
            status: github_auth_fact_status(&github_auth),
            detail: github_auth.detail,
            checked_at,
            ..LoopxEnvironmentFact::default()
        };
        let open_viking = open_viking_environment_fact(open_viking, checked_at);
        let feedback_memory = feedback_memory_environment_fact(&open_viking, checked_at);

        state.environment.revision = state.environment.revision.saturating_add(1);
        state.environment.checked_at = checked_at;
        state.environment.core.sidecar = sidecar;
        state.environment.core.git_worktree = git_worktree;
        state.environment.core.agent_model = agent_model;
        state.environment.optional.python_fallback = python_fallback;
        state.environment.optional.open_viking = open_viking;
        state.environment.optional.feedback_memory = feedback_memory;
        state.environment.optional.github_auth = github_auth;
        state.environment.status =
            derive_environment_status(&state.environment.core, &state.environment.optional);
        let status = state.environment.status;
        state.revision = state.revision.saturating_add(1);
        state.append_event(LoopxEvent {
            kind: LoopxEventKind::EnvironmentChanged,
            level: if status == LoopxEnvironmentStatus::Blocked {
                LoopxEventLevel::Error
            } else {
                LoopxEventLevel::Info
            },
            source: LoopxEventSource::System,
            message: format!("LoopX environment validation finished with status {status:?}"),
            important: status == LoopxEnvironmentStatus::Blocked,
            occurred_at: now_ms(),
            ..LoopxEvent::default()
        });
        let persisted = state.clone();
        let event = persisted.events.last().cloned();
        drop(state);
        self.store.save(&persisted).await?;
        if let Some(event) = event {
            let _ = self.event_sender.send(event);
        }
        Ok(())
    }

    async fn record_progress(&self, progress: Vec<LoopxCliProgress>) -> Result<(), String> {
        if progress.is_empty() {
            return Ok(());
        }
        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        let start_cursor = state.cursor;
        for item in progress {
            if is_normal_process_lifecycle_message(&item.message) {
                continue;
            }
            state.append_event(LoopxEvent {
                task_id: item.task_id,
                kind: LoopxEventKind::Progress,
                source: LoopxEventSource::Sidecar,
                message: item.message,
                occurred_at: item.occurred_at,
                ..LoopxEvent::default()
            });
        }
        let persisted = state.clone();
        drop(state);
        self.store.save(&persisted).await?;
        self.broadcast_new_events(&persisted, start_cursor);
        Ok(())
    }

    async fn bind_workspace(
        &self,
        task_id: &str,
        generation: u64,
        workspace: &LoopxWorkspacePrepareResult,
    ) -> Result<(), String> {
        self.mutate_task(task_id, None, |task, runtime| {
            if task.generation != generation {
                return;
            }
            task.workspace_path = Some(workspace.worktree_path.clone());
            task.phase = LoopxPhase::CreatingGoal;
            task.revision = task.revision.saturating_add(1);
            runtime.registry_path = workspace.registry_path.clone();
        })
        .await
        .map(|_| ())
    }

    async fn bind_goal(
        &self,
        task_id: &str,
        generation: u64,
        goal: LoopxCliCreateGoalResult,
    ) -> Result<(), String> {
        self.mutate_task(task_id, None, |task, runtime| {
            if task.generation != generation {
                return;
            }
            task.goal_id = Some(goal.goal_id.clone());
            task.goal_state = Some(LoopxCliGoalState::Active);
            task.state = LoopxTaskState::Queued;
            task.phase = LoopxPhase::InspectingGoal;
            task.revision = task.revision.saturating_add(1);
            runtime.expected_durable_revision = Some(goal.durable_revision.clone());
        })
        .await
        .map(|_| ())
    }

    async fn record_goal_state(
        &self,
        task: &LoopxTaskSnapshot,
        goal_state: LoopxCliGoalState,
    ) -> Result<(), String> {
        if task.goal_state == Some(goal_state) {
            return Ok(());
        }
        self.mutate_task(&task.task_id, None, |current, _| {
            if current.generation != task.generation {
                return;
            }
            current.goal_state = Some(goal_state);
            current.revision = current.revision.saturating_add(1);
        })
        .await
        .map(|_| ())
    }

    async fn apply_goal_projection(
        &self,
        expected: &LoopxTaskSnapshot,
        goal: &LoopxCliGoalSnapshot,
    ) -> Result<(), String> {
        let projection = project_host_task_from_goal(expected.state, expected.phase, goal.state);
        let preserve_pending_gate = preserve_unanswered_local_gate(expected, goal);
        let pending_gate_id = if preserve_pending_gate {
            expected.pending_gate_id.as_deref()
        } else {
            goal.pending_user_gate
                .as_ref()
                .map(|gate| gate.gate_id.as_str())
        };
        let pending_gate_message = if preserve_pending_gate {
            expected.pending_gate_message.as_deref()
        } else {
            goal.pending_user_gate
                .as_ref()
                .map(|gate| gate.message.as_str())
        };
        let pending_gate_action_kind = if preserve_pending_gate {
            expected.pending_gate_action_kind.as_deref()
        } else {
            goal.pending_user_gate
                .as_ref()
                .and_then(|gate| gate.action_kind.as_deref())
        };
        if expected.goal_state == Some(goal.state)
            && expected.state == projection.state
            && expected.phase == projection.phase
            && expected.pending_gate_id.as_deref() == pending_gate_id
            && expected.pending_gate_message.as_deref() == pending_gate_message
            && expected.pending_gate_action_kind.as_deref() == pending_gate_action_kind
        {
            return Ok(());
        }

        let host_state_changed = expected.state != projection.state;
        let updated = self
            .mutate_task(&expected.task_id, None, |task, runtime| {
                if task.generation != expected.generation {
                    return;
                }
                let preserve_pending_gate = preserve_unanswered_local_gate(task, goal);
                let current = project_host_task_from_goal(task.state, task.phase, goal.state);
                task.goal_state = Some(goal.state);
                task.state = current.state;
                task.phase = current.phase;
                // The authoritative Goal projection is healthy again: a stale
                // environment-level error (for example a coordination store
                // schema rejection from a cross-build data home) must not keep
                // resurfacing on a task that is demonstrably running.
                if !matches!(
                    current.state,
                    LoopxTaskState::RecoveryRequired | LoopxTaskState::Failed
                ) {
                    task.error = None;
                }
                if !preserve_pending_gate {
                    task.pending_gate_id = goal
                        .pending_user_gate
                        .as_ref()
                        .map(|gate| gate.gate_id.clone());
                    task.pending_gate_message = goal
                        .pending_user_gate
                        .as_ref()
                        .map(|gate| gate.message.clone());
                    task.pending_gate_action_kind = goal
                        .pending_user_gate
                        .as_ref()
                        .and_then(|gate| gate.action_kind.clone());
                }
                task.revision = task.revision.saturating_add(1);
                runtime.expected_durable_revision = Some(goal.durable_revision.clone());
                if task.state.is_terminal() {
                    task.current_turn_id = None;
                    task.current_tool = None;
                    task.deadline_at = None;
                    task.retry_at = None;
                }
            })
            .await?;

        if host_state_changed {
            self.append_task_event(
                &updated,
                LoopxEventKind::SnapshotInvalidated,
                "BitFun host task reconciled with authoritative LoopX Goal state",
                false,
            )
            .await?;
            if updated.state == LoopxTaskState::Queued {
                self.enqueue_task(updated.task_id.clone(), Duration::ZERO)?;
            }
        }
        Ok(())
    }

    async fn bind_turn(
        &self,
        task: &LoopxTaskSnapshot,
        turn: &LoopxCliBuildTurnResult,
    ) -> Result<(), String> {
        let generation = task.generation;
        self.mutate_task(&task.task_id, None, |task, runtime| {
            if task.generation != generation {
                return;
            }
            task.phase = LoopxPhase::StartingAgent;
            task.deadline_at = turn.deadline_at;
            task.current_turn_id = Some(turn.turn_id.clone());
            task.revision = task.revision.saturating_add(1);
            runtime.loopx_turn_id = Some(turn.turn_id.clone());
            runtime.settlement_token = Some(turn.settlement_token.clone());
            runtime.expected_durable_revision = Some(turn.durable_revision.clone());
        })
        .await
        .map(|_| ())
    }

    async fn bind_agent_run(
        &self,
        task: &LoopxTaskSnapshot,
        run: LoopxAgentStartResult,
    ) -> Result<(), String> {
        let updated = self
            .mutate_task(&task.task_id, None, |task, runtime| {
                task.state = LoopxTaskState::Running;
                task.phase = LoopxPhase::AgentRunning;
                task.current_turn_id = Some(run.turn_id.clone());
                task.last_output_at = Some(now_ms());
                task.revision = task.revision.saturating_add(1);
                runtime.session_id = Some(run.session_id.clone());
                runtime.agent_turn_id = Some(run.turn_id.clone());
            })
            .await?;
        self.append_task_event(
            &updated,
            LoopxEventKind::StateChanged,
            "Agent turn started",
            false,
        )
        .await
    }

    async fn transition_task(
        &self,
        task_id: &str,
        generation: u64,
        state: LoopxTaskState,
        phase: LoopxPhase,
        message: &str,
    ) -> Result<LoopxTaskSnapshot, String> {
        let updated = self
            .mutate_task(task_id, None, |task, _| {
                if task.generation != generation {
                    return;
                }
                task.state = state;
                task.phase = phase;
                if state != LoopxTaskState::WaitingForUser {
                    task.pending_gate_id = None;
                    task.pending_gate_message = None;
                    task.pending_gate_action_kind = None;
                }
                task.revision = task.revision.saturating_add(1);
            })
            .await?;
        self.append_task_event(
            &updated,
            LoopxEventKind::StateChanged,
            message,
            state == LoopxTaskState::RecoveryRequired,
        )
        .await?;
        Ok(updated)
    }

    async fn transition_action(
        &self,
        task_id: &str,
        state: LoopxTaskState,
        phase: LoopxPhase,
        request_id: &str,
    ) -> Result<LoopxActionResponse, String> {
        let updated = self
            .mutate_task(task_id, Some(request_id), |task, _| {
                task.state = state;
                task.phase = phase;
                if state != LoopxTaskState::WaitingForUser {
                    task.pending_gate_id = None;
                    task.pending_gate_message = None;
                    task.pending_gate_action_kind = None;
                }
                if state == LoopxTaskState::Queued {
                    task.autonomous_turns_since_review = 0;
                    task.stagnant_settlements_since_review = 0;
                    task.settlements_on_current_todo = 0;
                    task.current_settlement_todo_id = None;
                }
                task.revision = task.revision.saturating_add(1);
            })
            .await?;
        Ok(LoopxActionResponse {
            current_revision: updated.revision,
            task: Some(updated),
            ..LoopxActionResponse::default()
        })
    }

    async fn update_task_phase(
        &self,
        task_id: &str,
        generation: u64,
        phase: LoopxPhase,
        message: &str,
    ) -> Result<(), String> {
        let updated = self
            .mutate_task(task_id, None, |task, _| {
                if task.generation != generation {
                    return;
                }
                task.phase = phase;
                task.revision = task.revision.saturating_add(1);
            })
            .await?;
        self.append_task_event(&updated, LoopxEventKind::PhaseChanged, message, false)
            .await
    }

    async fn fail_task(self: &Arc<Self>, task_id: &str, error: String) -> Result<(), String> {
        let updated = self
            .mutate_task(task_id, None, |task, _| {
                let workspace_was_never_prepared = task.workspace_path.is_none()
                    && task.goal_id.is_none()
                    && task.current_turn_id.is_none();
                task.state = if workspace_was_never_prepared {
                    LoopxTaskState::Failed
                } else {
                    LoopxTaskState::RecoveryRequired
                };
                task.phase = if workspace_was_never_prepared {
                    LoopxPhase::Finished
                } else {
                    LoopxPhase::Recovering
                };
                task.pending_gate_id = None;
                task.pending_gate_message = None;
                task.pending_gate_action_kind = None;
                task.error = Some(error.clone());
                task.deadline_at = None;
                task.revision = task.revision.saturating_add(1);
            })
            .await?;
        self.release_repository(&updated).await;
        self.append_task_event(&updated, LoopxEventKind::StateChanged, &error, true)
            .await?;
        self.schedule_next_for_repository(
            &updated.identity.item.repository.canonical_id(),
            Some(&updated.task_id),
        )
        .await;
        Ok(())
    }

    /// Best-effort cleanup of the task's on-disk worktree. Called only from
    /// the explicit Archive action; failure is recorded as an event, never
    /// fatal to the transition.
    async fn dispose_task_workspace(self: &Arc<Self>, task: &LoopxTaskSnapshot) {
        if task.workspace_path.is_none() {
            return;
        }
        let progress = BufferedProgress::default();
        let result = self
            .workspace
            .dispose(LoopxWorkspaceDisposeRequest {
                operation_id: format!("dispose-{}", uuid::Uuid::new_v4()),
                task_id: task.task_id.clone(),
                item: task.identity.item.clone(),
            })
            .await
            .map_err(|error| error.message.clone());
        self.record_progress(progress.take()).await.ok();
        let important = result.is_err();
        let message = match result {
            Ok(disposed) if disposed.removed => {
                "Archived task worktree cleaned up (disk space released)".to_string()
            }
            Ok(_) => "Archived task had no managed worktree to clean up".to_string(),
            Err(error) => {
                // Keep the archive transition; surface cleanup failure.
                format!("Failed to clean up archived task worktree: {error}")
            }
        };
        let _ = self
            .append_task_event(task, LoopxEventKind::StateChanged, &message, important)
            .await;
    }

    async fn schedule_next_for_repository(
        self: &Arc<Self>,
        repository_id: &str,
        exclude_task_id: Option<&str>,
    ) -> bool {
        let next = {
            let state = self.state.read().await;
            state
                .tasks
                .iter()
                .find(|task| {
                    task.state == LoopxTaskState::Queued
                        && task.identity.item.repository.canonical_id() == repository_id
                        && exclude_task_id != Some(task.task_id.as_str())
                })
                .map(|task| task.task_id.clone())
        };
        if let Some(task_id) = next {
            self.enqueue_task(task_id, Duration::ZERO).is_ok()
        } else {
            false
        }
    }

    async fn enqueue_ready_tasks_after_load(&self) {
        if self.load_error.read().await.is_some() {
            return;
        }
        let task_ids = {
            let state = self.state.read().await;
            state
                .tasks
                .iter()
                .filter(|task| task.state == LoopxTaskState::Queued)
                .map(|task| task.task_id.clone())
                .collect::<Vec<_>>()
        };
        for task_id in task_ids {
            let _ = self.enqueue_task(task_id, Duration::ZERO);
        }
    }

    fn enqueue_task(&self, task_id: String, delay: Duration) -> Result<(), String> {
        if !delay.is_zero() {
            let sender = self.task_sender.clone();
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                let _ = sender.send(ScheduledTask { task_id });
            });
            return Ok(());
        }
        self.task_sender
            .send(ScheduledTask { task_id })
            .map_err(|_| "LoopX controller task runner is unavailable".to_string())
    }

    async fn reserve_scheduled_task(&self, task_id: &str) -> bool {
        let mut active = self.active_tasks.lock().await;
        match active.get_mut(task_id) {
            Some(pending) => {
                *pending = true;
                false
            }
            None => {
                active.insert(task_id.to_string(), false);
                true
            }
        }
    }

    async fn release_scheduled_task(&self, task_id: &str) -> bool {
        self.active_tasks
            .lock()
            .await
            .remove(task_id)
            .unwrap_or(false)
    }

    async fn suppress_pending_task_rerun(&self, task_id: &str) {
        if let Some(pending) = self.active_tasks.lock().await.get_mut(task_id) {
            *pending = false;
        }
    }

    async fn mutate_task(
        &self,
        task_id: &str,
        request_id: Option<&str>,
        update: impl FnOnce(&mut LoopxTaskSnapshot, &mut LoopxTaskRuntimeRecord),
    ) -> Result<LoopxTaskSnapshot, String> {
        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        let task_index = state
            .tasks
            .iter()
            .position(|task| task.task_id == task_id)
            .ok_or_else(|| "LoopX task not found".to_string())?;
        let mut runtime = state.runtime.remove(task_id).unwrap_or_default();
        update(&mut state.tasks[task_index], &mut runtime);
        state.tasks[task_index].updated_at = now_ms();
        let updated = state.tasks[task_index].clone();
        state.runtime.insert(task_id.to_string(), runtime);
        state.revision = state.revision.saturating_add(1);
        if let Some(request_id) = request_id {
            state.record_processed_request(request_id.to_string());
        }
        let persisted = state.clone();
        drop(state);
        self.store.save(&persisted).await?;
        Ok(updated)
    }

    async fn append_task_event(
        &self,
        task: &LoopxTaskSnapshot,
        kind: LoopxEventKind,
        message: &str,
        important: bool,
    ) -> Result<(), String> {
        self.append_task_event_with_details(task, kind, message, important, BTreeMap::new())
            .await
    }

    async fn append_task_event_with_details(
        &self,
        task: &LoopxTaskSnapshot,
        kind: LoopxEventKind,
        message: &str,
        important: bool,
        details: BTreeMap<String, String>,
    ) -> Result<(), String> {
        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        state.append_event(LoopxEvent {
            task_id: Some(task.task_id.clone()),
            generation: Some(task.generation),
            revision: Some(task.revision),
            kind,
            level: if kind == LoopxEventKind::ApprovalRequired {
                LoopxEventLevel::Warning
            } else if important {
                LoopxEventLevel::Error
            } else {
                LoopxEventLevel::Info
            },
            source: LoopxEventSource::Controller,
            phase: Some(task.phase),
            message: message.to_string(),
            important,
            details,
            occurred_at: now_ms(),
            ..LoopxEvent::default()
        });
        let persisted = state.clone();
        let event = persisted.events.last().cloned();
        drop(state);
        self.store.save(&persisted).await?;
        if let Some(event) = event {
            let _ = self.event_sender.send(event);
        }
        Ok(())
    }

    async fn task(&self, task_id: &str) -> Result<LoopxTaskSnapshot, String> {
        self.state
            .read()
            .await
            .tasks
            .iter()
            .find(|task| task.task_id == task_id)
            .cloned()
            .ok_or_else(|| "LoopX task not found".to_string())
    }

    async fn runtime(&self, task_id: &str) -> LoopxTaskRuntimeRecord {
        self.state
            .read()
            .await
            .runtime
            .get(task_id)
            .cloned()
            .unwrap_or_default()
    }

    async fn ensure_writable(&self) -> Result<(), String> {
        match self.load_error.read().await.clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    async fn persist_current(&self) -> Result<(), String> {
        let state = self.state.read().await.clone();
        self.store.save(&state).await
    }

    fn broadcast_new_events(&self, state: &LoopxPersistedState, after_cursor: u64) {
        for event in state
            .events
            .iter()
            .filter(|event| event.cursor > after_cursor)
        {
            let _ = self.event_sender.send(event.clone());
        }
    }
}

fn is_normal_process_lifecycle_message(message: &str) -> bool {
    matches!(
        message,
        "Starting LoopX process" | "LoopX process exited successfully"
    )
}

fn task_has_bound_goal(task: &LoopxTaskSnapshot) -> bool {
    task.goal_id
        .as_deref()
        .is_some_and(|goal_id| !goal_id.trim().is_empty())
}

fn is_repository_recovery_candidate(task: &LoopxTaskSnapshot, repository_id: &str) -> bool {
    task.identity.item.repository.canonical_id() == repository_id
        && matches!(
            task.state,
            LoopxTaskState::Stopped | LoopxTaskState::Failed | LoopxTaskState::RecoveryRequired
        )
}

fn should_requeue_after_settlement(final_state: LoopxTaskState, yielded_repository: bool) -> bool {
    final_state == LoopxTaskState::Queued && !yielded_repository
}

fn goal_context(task: &LoopxTaskSnapshot, runtime: &LoopxTaskRuntimeRecord) -> LoopxCliGoalContext {
    LoopxCliGoalContext {
        call: LoopxCliCallContext {
            operation_id: runtime.operation_id.clone(),
            deadline_at: task.deadline_at,
        },
        task_id: task.task_id.clone(),
        generation: task.generation,
        worktree_path: task.workspace_path.clone().unwrap_or_default(),
        registry_path: runtime.registry_path.clone(),
    }
}

fn goal_id_for(identity: &LoopxTaskIdentity) -> String {
    let item = &identity.item;
    let kind = match item.kind {
        LoopxItemKind::Issue => "issue",
        LoopxItemKind::PullRequest => "pr",
    };
    let suffix = if identity.attempt > 1 {
        format!("-{}", identity.attempt)
    } else {
        String::new()
    };
    format!(
        "bfx-{}-{}-{kind}-{}{}",
        item.repository.owner, item.repository.repository, item.number, suffix
    )
}

fn existing_outcomes(
    state: &LoopxPersistedState,
    selected: &std::collections::BTreeSet<LoopxIssueKey>,
) -> Vec<LoopxCreateTaskOutcome> {
    selected
        .iter()
        .map(|item| {
            let task = state
                .tasks
                .iter()
                .filter(|task| &task.identity.item == item)
                .max_by_key(|task| task.identity.attempt);
            LoopxCreateTaskOutcome {
                item: item.clone(),
                kind: LoopxCreateTaskOutcomeKind::OpenedExisting,
                task_id: task.map(|task| task.task_id.clone()),
                attempt: task.map(|task| task.identity.attempt),
                ..LoopxCreateTaskOutcome::default()
            }
        })
        .collect()
}

fn prune_intake_previews(previews: &mut HashMap<String, LoopxIntakePreview>, now: i64) {
    previews.retain(|_, preview| intake_preview_is_fresh(preview, now));
    if previews.len() <= MAX_INTAKE_PREVIEWS {
        return;
    }

    let mut by_age = previews
        .iter()
        .map(|(fingerprint, preview)| (fingerprint.clone(), preview.resolved_at))
        .collect::<Vec<_>>();
    by_age.sort_by_key(|(_, resolved_at)| *resolved_at);
    let excess = previews.len().saturating_sub(MAX_INTAKE_PREVIEWS);
    for (fingerprint, _) in by_age.into_iter().take(excess) {
        previews.remove(&fingerprint);
    }
}

fn intake_preview_is_fresh(preview: &LoopxIntakePreview, now: i64) -> bool {
    match preview.expires_at {
        Some(expires_at) => expires_at > now,
        None => false,
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn bounded_agent_summary(summary: &str) -> String {
    let mut chars = summary.chars();
    let bounded = chars
        .by_ref()
        .take(MAX_AGENT_SUMMARY_CHARS)
        .collect::<String>();
    if chars.next().is_some() {
        format!("{bounded}\n\n[Summary truncated by LoopX host]")
    } else {
        bounded
    }
}

fn github_auth_fact_status(probe: &LoopxGithubAuthProbe) -> LoopxEnvironmentFactStatus {
    if probe.authenticated {
        LoopxEnvironmentFactStatus::Available
    } else if probe.rate_limit_remaining.is_some() {
        LoopxEnvironmentFactStatus::Degraded
    } else {
        LoopxEnvironmentFactStatus::Unavailable
    }
}

fn open_viking_environment_fact(
    probe: LoopxCliResult<LoopxOpenVikingProbe>,
    checked_at: Option<i64>,
) -> LoopxEnvironmentFact {
    match probe {
        Ok(probe) => {
            let base = LoopxEnvironmentFact {
                version: probe.version,
                detail: probe.detail,
                checked_at,
                ..LoopxEnvironmentFact::default()
            };
            match probe.state {
                LoopxOpenVikingState::NotInstalled | LoopxOpenVikingState::Unhealthy => {
                    unavailable_open_viking_environment_fact(
                        base.detail
                            .unwrap_or_else(|| "OpenViking runtime is unavailable".to_string()),
                        checked_at,
                    )
                }
                LoopxOpenVikingState::NotConfigured => LoopxEnvironmentFact {
                    status: LoopxEnvironmentFactStatus::Degraded,
                    remediation: Some(
                        "Configure and start an OpenViking service, then retry the environment check"
                            .to_string(),
                    ),
                    ..base
                },
                LoopxOpenVikingState::Disabled => LoopxEnvironmentFact {
                    status: LoopxEnvironmentFactStatus::Disabled,
                    remediation: Some(
                        "Enable the LoopX semantic-preference extension".to_string(),
                    ),
                    remediation_action: LoopxEnvironmentRemediationAction::InstallOpenViking,
                    ..base
                },
                LoopxOpenVikingState::Ready => LoopxEnvironmentFact {
                    status: LoopxEnvironmentFactStatus::Available,
                    ..base
                },
                LoopxOpenVikingState::Unknown => base,
            }
        }
        Err(error) => unavailable_open_viking_environment_fact(error.to_string(), checked_at),
    }
}

fn feedback_memory_environment_fact(
    open_viking: &LoopxEnvironmentFact,
    checked_at: Option<i64>,
) -> LoopxEnvironmentFact {
    match open_viking.status {
        LoopxEnvironmentFactStatus::Checking => checking_environment_fact(checked_at),
        LoopxEnvironmentFactStatus::Unavailable => LoopxEnvironmentFact {
            status: LoopxEnvironmentFactStatus::Unavailable,
            detail: Some("Feedback memory requires a working OpenViking runtime".to_string()),
            checked_at,
            ..LoopxEnvironmentFact::default()
        },
        LoopxEnvironmentFactStatus::Unknown => LoopxEnvironmentFact::default(),
        _ => LoopxEnvironmentFact {
            status: LoopxEnvironmentFactStatus::Disabled,
            detail: Some(
                "Human feedback ingestion and recall are explicit per-goal opt-ins and are not enabled automatically"
                    .to_string(),
            ),
            checked_at,
            ..LoopxEnvironmentFact::default()
        },
    }
}

fn checking_environment_fact(checked_at: Option<i64>) -> LoopxEnvironmentFact {
    LoopxEnvironmentFact {
        status: LoopxEnvironmentFactStatus::Checking,
        checked_at,
        ..LoopxEnvironmentFact::default()
    }
}

fn unavailable_environment_fact(
    detail: impl Into<String>,
    checked_at: Option<i64>,
) -> LoopxEnvironmentFact {
    LoopxEnvironmentFact {
        status: LoopxEnvironmentFactStatus::Unavailable,
        detail: Some(detail.into()),
        checked_at,
        ..LoopxEnvironmentFact::default()
    }
}

fn unavailable_loopx_environment_fact(
    detail: impl Into<String>,
    checked_at: Option<i64>,
) -> LoopxEnvironmentFact {
    LoopxEnvironmentFact {
        status: LoopxEnvironmentFactStatus::Unavailable,
        detail: Some(detail.into()),
        remediation: Some(
            "Download the pinned LoopX source from GitHub into BitFun-managed storage".to_string(),
        ),
        remediation_action: LoopxEnvironmentRemediationAction::InstallLoopx,
        checked_at,
        ..LoopxEnvironmentFact::default()
    }
}

fn unavailable_open_viking_environment_fact(
    detail: impl Into<String>,
    checked_at: Option<i64>,
) -> LoopxEnvironmentFact {
    LoopxEnvironmentFact {
        status: LoopxEnvironmentFactStatus::Unavailable,
        detail: Some(detail.into()),
        remediation: Some(
            "Build the pinned OpenViking CLI from its official GitHub source".to_string(),
        ),
        remediation_action: LoopxEnvironmentRemediationAction::InstallOpenViking,
        checked_at,
        ..LoopxEnvironmentFact::default()
    }
}

/// Builds the bounded host goal-facts block prepended to every LoopX turn
/// prompt. It restates authoritative durable facts the host already observed
/// (goal state, receipts, todo projection), points at the repository playbook
/// and the persisted intake plan packet, and carries the previous turn's
/// bounded agent summary as an explicitly non-authoritative recap. This is not
/// progress inference: LoopX durable writeback remains the only settlement
/// evidence.
fn host_turn_context_block(
    task: &LoopxTaskSnapshot,
    inspected: &LoopxCliGoalSnapshot,
    playbook_path: Option<&str>,
) -> String {
    let mut block = String::from("<bitfun_host_goal_facts>\n");
    block.push_str(
        "Host-verified facts from LoopX durable state at turn build time (authoritative):\n",
    );
    block.push_str(&format!(
        "- Goal: {} | state: {:?} | durable revision: {}\n",
        task.goal_id.as_deref().unwrap_or("unknown"),
        inspected.state,
        inspected.durable_revision,
    ));
    block.push_str(&format!(
        "- Durable settlement receipts so far: {}\n",
        inspected.settlement_receipt_ids.len(),
    ));
    if let Some(latest_receipt) = inspected.settlement_receipt_ids.last() {
        block.push_str(&format!("- Latest receipt: {latest_receipt}\n"));
    }
    block.push_str(&format!(
        "- Open todos: {} total (waiting for user: {})\n",
        inspected.open_todo_count, inspected.waiting_user_todo_count,
    ));
    if !inspected.open_agent_todos.is_empty() {
        block.push_str("- Current open agent todos:\n");
        for todo in inspected.open_agent_todos.iter().take(MAX_HOST_FACT_TODOS) {
            block.push_str(&format!(
                "  - [{}] {} {}\n",
                todo.todo_id,
                todo.status,
                bounded_text(&todo.text, MAX_HOST_FACT_TODO_CHARS),
            ));
        }
    }
    block.push_str(
        "- The BitFun host already executed this turn's quota guard before the agent started, with the settlement todo bound, and the exact guard command is printed in the task body preflight. Do not re-run preflight to decide whether to work; only re-run the exact printed guard command when a LoopX settlement command reports a missing or mismatched heartbeat receipt.\n",
    );
    if let Some(path) = task
        .intake_plan_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        block.push_str(&format!(
            "- Initial issue-fix workflow plan packet (exact next commands, successor targets, and dependencies): {path}. Read it before running issue-fix subcommands; newer packets printed by later workflow-plan runs supersede it.\n"
        ));
    }
    if let Some(playbook_path) = playbook_path {
        block.push_str(&format!(
            "- Repository playbook (workflow map, anti-patterns, lessons from earlier issues): {playbook_path}. Read it before running LoopX commands; do not run `--help`, `commands`, or read loopx source code to rediscover syntax.\n"
        ));
    }
    if let Some(summary) = task
        .last_agent_summary
        .as_deref()
        .filter(|summary| !summary.trim().is_empty())
    {
        block.push_str(
            "Non-authoritative recap of the previous settled turn (verify against durable state before acting; may be partial):\n",
        );
        block.push_str(&bounded_text(summary, MAX_HOST_FACT_SUMMARY_CHARS));
        block.push('\n');
    }
    block.push_str("</bitfun_host_goal_facts>\n\n");
    block
}

fn bounded_text(text: &str, max_chars: usize) -> String {
    let mut bounded: String = text.chars().take(max_chars).collect();
    if text.chars().count() > max_chars {
        bounded.push_str("...[truncated]");
    }
    bounded
}

/// Reconciliation may replace a local gate with a durable gate projection, but
/// it must never infer approval from an active Goal. Only an explicit gate
/// answer transitions the host task away from WaitingForUser.
fn preserve_unanswered_local_gate(task: &LoopxTaskSnapshot, goal: &LoopxCliGoalSnapshot) -> bool {
    task.state == LoopxTaskState::WaitingForUser
        && task.pending_gate_id.is_some()
        && goal.pending_user_gate.is_none()
        && !matches!(
            goal.state,
            LoopxCliGoalState::Completed | LoopxCliGoalState::Failed | LoopxCliGoalState::Archived
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_ids_are_per_item_and_attempt() {
        let identity = LoopxTaskIdentity {
            item: LoopxIssueKey {
                repository: LoopxRepositoryKey {
                    host: "github.com".to_string(),
                    owner: "owner".to_string(),
                    repository: "repo".to_string(),
                },
                kind: LoopxItemKind::Issue,
                number: 42,
            },
            attempt: 2,
            ..Default::default()
        };
        assert_eq!(goal_id_for(&identity), "bfx-owner-repo-issue-42-2");
    }

    #[test]
    fn repository_recovery_includes_all_resumable_tasks() {
        let repository = LoopxRepositoryKey {
            host: "github.com".to_string(),
            owner: "owner".to_string(),
            repository: "repo".to_string(),
        };
        let task = |state| LoopxTaskSnapshot {
            identity: LoopxTaskIdentity {
                item: LoopxIssueKey {
                    repository: repository.clone(),
                    kind: LoopxItemKind::Issue,
                    number: 42,
                },
                ..LoopxTaskIdentity::default()
            },
            state,
            ..LoopxTaskSnapshot::default()
        };
        let repository_id = repository.canonical_id();

        assert!(is_repository_recovery_candidate(
            &task(LoopxTaskState::RecoveryRequired),
            &repository_id,
        ));
        assert!(is_repository_recovery_candidate(
            &task(LoopxTaskState::Failed),
            &repository_id,
        ));
        assert!(is_repository_recovery_candidate(
            &task(LoopxTaskState::Stopped),
            &repository_id,
        ));
    }

    #[test]
    fn agent_summary_projection_is_bounded_on_character_boundaries() {
        let summary = "界".repeat(MAX_AGENT_SUMMARY_CHARS + 1);
        let bounded = bounded_agent_summary(&summary);

        assert!(bounded.ends_with("[Summary truncated by LoopX host]"));
        assert_eq!(bounded.matches('界').count(), MAX_AGENT_SUMMARY_CHARS);
    }

    #[test]
    fn settled_task_does_not_self_requeue_after_yielding_repository() {
        assert!(!should_requeue_after_settlement(
            LoopxTaskState::Queued,
            true,
        ));
        assert!(should_requeue_after_settlement(
            LoopxTaskState::Queued,
            false,
        ));
        assert!(!should_requeue_after_settlement(
            LoopxTaskState::Completed,
            false,
        ));
    }

    #[test]
    fn successful_process_lifecycle_messages_are_not_persisted_as_task_events() {
        assert!(is_normal_process_lifecycle_message(
            "Starting LoopX process"
        ));
        assert!(is_normal_process_lifecycle_message(
            "LoopX process exited successfully"
        ));
        assert!(!is_normal_process_lifecycle_message(
            "LoopX process exited with an error"
        ));
        assert!(!is_normal_process_lifecycle_message(
            "Building an idempotent external-host LoopX turn"
        ));
    }

    #[test]
    fn resumed_tasks_with_a_goal_skip_intake_and_goal_creation() {
        assert!(task_has_bound_goal(&LoopxTaskSnapshot {
            goal_id: Some("goal-42".to_string()),
            ..LoopxTaskSnapshot::default()
        }));
        assert!(!task_has_bound_goal(&LoopxTaskSnapshot::default()));
        assert!(!task_has_bound_goal(&LoopxTaskSnapshot {
            goal_id: Some("  ".to_string()),
            ..LoopxTaskSnapshot::default()
        }));
    }

    #[test]
    fn reconciliation_cannot_clear_an_unanswered_local_gate() {
        let task = LoopxTaskSnapshot {
            state: LoopxTaskState::WaitingForUser,
            phase: LoopxPhase::WaitingForApproval,
            pending_gate_id: Some("todo_owner_review".to_string()),
            ..LoopxTaskSnapshot::default()
        };
        let active_goal = LoopxCliGoalSnapshot {
            state: LoopxCliGoalState::Active,
            ..LoopxCliGoalSnapshot::default()
        };
        assert!(preserve_unanswered_local_gate(&task, &active_goal));

        let answered_task = LoopxTaskSnapshot {
            state: LoopxTaskState::Queued,
            ..task.clone()
        };
        assert!(!preserve_unanswered_local_gate(
            &answered_task,
            &active_goal,
        ));

        let completed_goal = LoopxCliGoalSnapshot {
            state: LoopxCliGoalState::Completed,
            ..active_goal
        };
        assert!(!preserve_unanswered_local_gate(&task, &completed_goal));
    }

    #[test]
    fn host_goal_facts_block_restates_durable_facts_and_playbook() {
        let task = LoopxTaskSnapshot {
            goal_id: Some("bfx-owner-repo-issue-42".to_string()),
            last_agent_summary: Some("Previous turn summary text.".to_string()),
            ..LoopxTaskSnapshot::default()
        };
        let inspected = LoopxCliGoalSnapshot {
            goal_id: "bfx-owner-repo-issue-42".to_string(),
            state: LoopxCliGoalState::Active,
            durable_revision: "rev-9".to_string(),
            run_decision: LoopxCliRunDecision::RunNow,
            open_todo_count: 3,
            waiting_user_todo_count: 1,
            settlement_receipt_ids: vec!["receipt-1".to_string(), "receipt-2".to_string()],
            open_agent_todos: vec![LoopxCliTodoSummary {
                todo_id: "todo-1".to_string(),
                status: "claimed".to_string(),
                text: "Collect candidate evidence".to_string(),
            }],
            ..LoopxCliGoalSnapshot::default()
        };

        let block =
            host_turn_context_block(&task, &inspected, Some("repo/LOOPX_AGENT_PLAYBOOK.md"));

        assert!(block.starts_with("<bitfun_host_goal_facts>"));
        assert!(block
            .contains("- Goal: bfx-owner-repo-issue-42 | state: Active | durable revision: rev-9"));
        assert!(block.contains("Durable settlement receipts so far: 2"));
        assert!(block.contains("Latest receipt: receipt-2"));
        assert!(block.contains("Open todos: 3 total (waiting for user: 1)"));
        assert!(block.contains("[todo-1] claimed Collect candidate evidence"));
        assert!(block.contains("Repository playbook (workflow map, anti-patterns, lessons from earlier issues): repo/LOOPX_AGENT_PLAYBOOK.md"));
        assert!(block.contains("Non-authoritative recap of the previous settled turn"));
        assert!(block.contains("Previous turn summary text."));
        assert!(block.ends_with("</bitfun_host_goal_facts>\n\n"));
    }

    #[test]
    fn host_goal_facts_block_omits_empty_recap_and_playbook() {
        let task = LoopxTaskSnapshot::default();
        let inspected = LoopxCliGoalSnapshot::default();

        let block = host_turn_context_block(&task, &inspected, None);

        assert!(!block.contains("Repository playbook"));
        assert!(!block.contains("Non-authoritative recap"));
        assert!(block.contains("Durable settlement receipts so far: 0"));
    }

    #[test]
    fn bounded_text_truncates_on_char_boundaries() {
        let text = "界".repeat(MAX_HOST_FACT_SUMMARY_CHARS + 1);
        let bounded = bounded_text(&text, MAX_HOST_FACT_SUMMARY_CHARS);
        assert!(bounded.chars().count() <= MAX_HOST_FACT_SUMMARY_CHARS + "...[truncated]".len());
        assert!(bounded.ends_with("...[truncated]"));
    }
}
