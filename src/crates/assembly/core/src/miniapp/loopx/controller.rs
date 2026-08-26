use super::{LoopxPersistedState, LoopxStateStore, LoopxTaskRuntimeRecord};
use bitfun_product_domains::miniapp::loopx::*;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};

const DEFAULT_AGENT_ID: &str = "bitfun-agent";
const EVENT_CHANNEL_CAPACITY: usize = 256;
const INTAKE_PREVIEW_TTL_MS: i64 = 5 * 60 * 1000;
const MAX_INTAKE_PREVIEWS: usize = 64;

struct ScheduledTask {
    task_id: String,
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
    previews: RwLock<HashMap<String, LoopxIntakePreview>>,
    active_tasks: Mutex<HashMap<String, bool>>,
    active_repositories: Mutex<HashMap<String, String>>,
    event_sender: broadcast::Sender<LoopxEvent>,
    task_sender: mpsc::UnboundedSender<ScheduledTask>,
    load_error: RwLock<Option<String>>,
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
            previews: RwLock::new(HashMap::new()),
            active_tasks: Mutex::new(HashMap::new()),
            active_repositories: Mutex::new(HashMap::new()),
            event_sender,
            task_sender,
            load_error: RwLock::new(load_error),
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
            let runtime = state.runtime.get(&task.task_id).cloned().unwrap_or_default();
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
        let github_auth = self.probe_github_auth();
        let (handshake, workspace, agent, github_auth) =
            tokio::join!(handshake, workspace, agent, github_auth);
        self.record_progress(progress.take()).await?;
        self.commit_environment(handshake, workspace, agent, github_auth)
            .await
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
            LoopxActionKind::ResumeRepository | LoopxActionKind::ResetAll => unreachable!(),
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
        self.update_task_phase(
            &task.task_id,
            task.generation,
            LoopxPhase::ValidatingProgress,
            "Agent turn ended; validating durable LoopX progress",
        )
        .await?;
        let progress = BufferedProgress::default();
        let result = self
            .cli
            .settle_turn(
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

    pub async fn handle_agent_activity(
        &self,
        turn_id: &str,
        tool_name: Option<String>,
    ) -> Result<(), String> {
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
        let updated = self
            .mutate_task(&task_id, None, |task, _| {
                if task.state != LoopxTaskState::Running {
                    return;
                }
                task.last_output_at = Some(now_ms());
                task.current_tool = tool_name.clone();
                task.revision = task.revision.saturating_add(1);
            })
            .await?;
        if let Some(tool_name) = tool_name {
            self.append_task_event(
                &updated,
                LoopxEventKind::Log,
                &format!("Agent tool activity: {tool_name}"),
                false,
            )
            .await?;
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
                },
                &progress,
            )
            .await
            .map_err(|error| error.to_string())?;
        self.record_progress(progress.take()).await?;
        let goal_id = goal_id_for(&task.identity);
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
        self.bind_goal(&task_id, task.generation, created).await?;
        self.drive_turn(task_id).await
    }

    async fn drive_turn(self: &Arc<Self>, task_id: String) -> Result<(), String> {
        let task = self.task(&task_id).await?;
        let runtime = self.runtime(&task_id).await;
        let progress = BufferedProgress::default();
        let inspected = self
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
        match inspected.run_decision {
            LoopxCliRunDecision::Wait => {
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
                if let Some(delay) = inspected.scheduler_hint_ms {
                    self.enqueue_task(task_id, Duration::from_millis(delay))?;
                }
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
                self.release_repository(&task).await;
                let updated = self
                    .transition_task(
                        &task_id,
                        task.generation,
                        LoopxTaskState::WaitingForUser,
                        LoopxPhase::WaitingForApproval,
                        "LoopX requires an explicit user decision",
                    )
                    .await?;
                let mut details = BTreeMap::new();
                details.insert("gateId".to_string(), gate_id);
                if let Some(action_kind) = action_kind {
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
                self.release_repository(&task).await;
                self.transition_task(
                    &task_id,
                    task.generation,
                    LoopxTaskState::Completed,
                    LoopxPhase::Finished,
                    "LoopX goal completed",
                )
                .await?;
                self.schedule_next_for_repository(
                    &task.identity.item.repository.canonical_id(),
                    Some(&task_id),
                )
                .await;
                Ok(())
            }
            LoopxCliRunDecision::Failed => {
                self.release_repository(&task).await;
                self.fail_task(&task_id, "LoopX reported a failed goal".to_string())
                    .await
            }
            LoopxCliRunDecision::RunNow => {
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
                        prompt: turn.prompt,
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
    /// abort pause or reset: the controller's persisted state is the task
    /// lifecycle source of truth, so teardown failures are logged and the
    /// operation continues.
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
                task.error = None;
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
        let (tasks, runtimes, previous_stream_id) = {
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
            (tasks, runtimes, previous_stream_id)
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
        self.agent
            .reset(LoopxAgentResetRequest {
                operation_id: format!("reset-history-{}", uuid::Uuid::new_v4()),
            })
            .await
            .map_err(|error| error.to_string())?;

        let fresh = LoopxPersistedState::new(now_ms());
        self.store.clear().await?;
        {
            let _mutation = self.mutation_lock.lock().await;
            *self.state.write().await = fresh.clone();
            self.active_tasks.lock().await.clear();
            self.active_repositories.lock().await.clear();
            self.previews.write().await.clear();
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
        // The fresh state starts with an unknown environment; validate it again
        // so the UI can re-enable intake after "clear and start over".
        self.refresh_environment().await?;
        Ok(LoopxActionResponse {
            current_revision: fresh.revision,
            message: Some(format!(
                "Cleared {} LoopX tasks, managed workspaces, and persisted controller state",
                tasks.len()
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
        let response = self
            .transition_action(
                &task.task_id,
                LoopxTaskState::Queued,
                LoopxPhase::Queued,
                &request.client_request_id,
            )
            .await?;
        if request.action == LoopxActionKind::Approve {
            let task_id = task.task_id.clone();
            self.enqueue_task(task_id, Duration::ZERO)?;
        } else {
            self.release_repository(task).await;
            self.schedule_next_for_repository(
                &task.identity.item.repository.canonical_id(),
                Some(&task.task_id),
            )
            .await;
        }
        Ok(response)
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
        let updated = self
            .mutate_task(&task.task_id, None, |task, runtime| {
                task.state = final_state;
                task.phase = phase;
                task.revision = task.revision.saturating_add(1);
                task.current_turn_id = None;
                task.deadline_at = None;
                task.error = (agent_status == LoopxAgentTurnStatus::Failed)
                    .then(|| failure_summary.unwrap_or("Agent turn failed").to_string());
                task.settlement = LoopxSettlementSummary {
                    turn_id: Some(settlement.turn_id.clone()),
                    receipt_id: Some(settlement.receipt_id.clone()),
                    durable_revision: Some(settlement.after_revision.clone()),
                    settled_at: Some(now_ms()),
                };
                runtime.session_id = None;
                runtime.agent_turn_id = None;
                runtime.expected_durable_revision = Some(settlement.after_revision.clone());
            })
            .await?;
        self.release_repository(&updated).await;
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
            self.schedule_next_for_repository(
                &updated.identity.item.repository.canonical_id(),
                Some(&updated.task_id),
            )
            .await;
        }
        if final_state == LoopxTaskState::Queued {
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
        github_auth: LoopxGithubAuthProbe,
    ) -> Result<(), String> {
        let _mutation = self.mutation_lock.lock().await;
        let mut state = self.state.write().await;
        let checked_at = Some(now_ms());
        let sidecar = match handshake {
            Ok(manifest) => LoopxEnvironmentFact {
                status: LoopxEnvironmentFactStatus::Available,
                version: Some(manifest.loopx_version),
                detail: Some(manifest.executable.identity),
                checked_at,
                ..LoopxEnvironmentFact::default()
            },
            Err(error) => unavailable_environment_fact(error.to_string(), checked_at),
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

        state.environment.revision = state.environment.revision.saturating_add(1);
        state.environment.checked_at = checked_at;
        state.environment.core.sidecar = sidecar;
        state.environment.core.git_worktree = git_worktree;
        state.environment.core.agent_model = agent_model;
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
            task.state = LoopxTaskState::Queued;
            task.phase = LoopxPhase::InspectingGoal;
            task.revision = task.revision.saturating_add(1);
            runtime.expected_durable_revision = Some(goal.durable_revision.clone());
        })
        .await
        .map(|_| ())
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
    ) {
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
            let _ = self.enqueue_task(task_id, Duration::ZERO);
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
            LoopxTaskState::Failed | LoopxTaskState::RecoveryRequired
        )
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

fn github_auth_fact_status(probe: &LoopxGithubAuthProbe) -> LoopxEnvironmentFactStatus {
    if probe.authenticated {
        LoopxEnvironmentFactStatus::Available
    } else if probe.rate_limit_remaining.is_some() {
        LoopxEnvironmentFactStatus::Degraded
    } else {
        LoopxEnvironmentFactStatus::Unavailable
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
    fn repository_recovery_excludes_intentionally_stopped_tasks() {
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
        assert!(!is_repository_recovery_candidate(
            &task(LoopxTaskState::Stopped),
            &repository_id,
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
}
