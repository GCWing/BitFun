//! Product composition and lifecycle service for external AI application sources.
//!
//! Concrete ecosystem providers are selected only in this assembly module. The
//! catalog and product surfaces remain provider- and ecosystem-neutral.

pub use bitfun_product_domains::external_sources::{
    prompt_command_conflict_key, ExpandedPromptCommand, ExternalSourceCatalogEntry,
    ExternalSourceCatalogSnapshot, ExternalSourceDiagnostic, ExternalSourceDiagnosticSeverity,
    ExternalSourceLifecycleState, ExternalToolActivationState, ExternalToolApprovalRequest,
    ExternalToolCapability, ExternalToolCatalogEntry, ExternalToolConflict,
    ExternalToolRuntimeKind, PromptCommandAvailability, PromptCommandCatalogEntry,
    PromptCommandDefinition, SourceKey,
};

use crate::external_tools::{
    begin_external_tool_workspace_recovery, external_tool_workspace_requires_recovery,
    merge_tool_state, reconcile_external_tools, release_external_tool_workspace,
    reset_external_tool_workspace_recovery_budget, ExternalToolDecisions, ExternalToolProductState,
    TOOL_CONFLICT_RESELECTION_REQUIRED, UNRESOLVED_TOOL_CONFLICT_CHOICE,
};
use bitfun_external_sources::{
    ExternalSourceCoordinator, ExternalSourceDiscoveryRequest, ExternalSourceDiscoveryResult,
    ExternalToolCoordinator, ExternalToolDiscoveryRequest, ExternalToolDiscoveryResult,
};
use bitfun_opencode_adapter::{OpenCodeCommandProvider, OpenCodeToolProvider};
use bitfun_product_domains::external_sources::{
    ExecutionDomainId, ExternalSourceContext, ExternalToolSourceProvider,
    PromptCommandSourceProvider,
};
use bitfun_services_core::json_store::JsonFileStore;
use bitfun_services_integrations::file_watch::{FileWatchService, FileWatcherConfig};
use dashmap::{mapref::entry::Entry, DashMap};
use futures::future::{join_all, BoxFuture, Shared};
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard, OnceLock, Weak};
use tokio::sync::broadcast;

const PROVIDER_DISCOVERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const EXTERNAL_SOURCE_PREFERENCES_FILE: &str = "external-sources.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ExternalSourcesConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    suppressed_source_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    conflict_choices: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    conflict_lineage_current_keys: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    conflicted_candidate_ids: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    approved_tool_targets: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    declined_tool_decisions: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    tool_conflict_choices: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct ExternalSourcePreferenceStore {
    path: PathBuf,
}

impl ExternalSourcePreferenceStore {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn global() -> Result<Self, String> {
        let path_manager =
            crate::infrastructure::try_get_path_manager_arc().map_err(|error| error.to_string())?;
        Ok(Self::new(
            path_manager
                .user_config_dir()
                .join(EXTERNAL_SOURCE_PREFERENCES_FILE),
        ))
    }

    async fn read(&self) -> Result<ExternalSourcesConfig, String> {
        JsonFileStore
            .read_locked_optional(&self.path)
            .await
            .map(|config| config.unwrap_or_default())
            .map_err(|error| error.to_string())
    }

    async fn update<R>(
        &self,
        update: impl FnOnce(&mut ExternalSourcesConfig) -> R,
    ) -> Result<(R, ExternalSourcesConfig), String> {
        JsonFileStore
            .update_locked(&self.path, ExternalSourcesConfig::default(), update)
            .await
            .map_err(|error| error.to_string())
    }
}

type SharedDiscoveryTask = Shared<BoxFuture<'static, ExternalSourceDiscoveryResult>>;
type SharedToolDiscoveryTask = Shared<BoxFuture<'static, ExternalToolDiscoveryResult>>;

struct InFlightDiscovery {
    task: SharedDiscoveryTask,
    wake_scheduled: bool,
}

struct InFlightToolDiscovery {
    task: SharedToolDiscoveryTask,
    wake_scheduled: bool,
}

#[derive(Clone, Copy)]
enum WorkerRecoveryPolicy {
    Preserve,
    PendingOnce,
    ResetAndAttempt,
}

struct WorkspaceExternalSourceService {
    workspace_root: Option<PathBuf>,
    coordinator: Arc<StdMutex<ExternalSourceCoordinator>>,
    tool_coordinator: Arc<StdMutex<ExternalToolCoordinator>>,
    snapshot: StdMutex<ExternalSourceCatalogSnapshot>,
    updates: broadcast::Sender<ExternalSourceCatalogSnapshot>,
    watch_states: tokio::sync::Mutex<BTreeMap<(PathBuf, bool), bool>>,
    refresh_gate: tokio::sync::Mutex<()>,
    product_rebuild_gate: tokio::sync::Mutex<()>,
    discovery_tasks: tokio::sync::Mutex<
        BTreeMap<bitfun_product_domains::external_sources::ProviderId, InFlightDiscovery>,
    >,
    tool_discovery_tasks: tokio::sync::Mutex<
        BTreeMap<bitfun_product_domains::external_sources::ProviderId, InFlightToolDiscovery>,
    >,
    initial_refresh_completed: AtomicBool,
    background_refresh_scheduled: AtomicBool,
    initial_refresh_gate: tokio::sync::Mutex<()>,
    keepalive_started: AtomicBool,
    last_access_epoch_seconds: AtomicU64,
    watcher: Arc<FileWatchService>,
    #[cfg(test)]
    tool_decision_gate_waiting: tokio::sync::Notify,
    #[cfg(test)]
    tool_decision_gate_acquired: tokio::sync::Notify,
}

impl WorkspaceExternalSourceService {
    async fn create(workspace_root: Option<PathBuf>) -> Result<Arc<Self>, String> {
        let context = ExternalSourceContext {
            workspace_root: workspace_root.clone(),
            execution_domain_id: ExecutionDomainId::new("local-user")
                .map_err(|error| error.to_string())?,
        };
        let providers: Vec<Arc<dyn PromptCommandSourceProvider>> =
            vec![Arc::new(OpenCodeCommandProvider::default())];
        let mut coordinator = ExternalSourceCoordinator::new(context.clone(), providers)?;
        let tool_providers: Vec<Arc<dyn ExternalToolSourceProvider>> =
            vec![Arc::new(OpenCodeToolProvider::default())];
        let mut tool_coordinator = ExternalToolCoordinator::new(context, tool_providers)?;
        let preferences = read_external_sources_config().await?;
        let suppressed_sources = preferences
            .suppressed_source_keys
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        coordinator.replace_suppressed_sources(suppressed_sources.clone());
        tool_coordinator.replace_suppressed_sources(suppressed_sources);
        coordinator.replace_conflict_choices(preferences.conflict_choices.clone());
        coordinator.replace_conflict_lineage_current_keys(
            preferences.conflict_lineage_current_keys.clone(),
        );
        coordinator.replace_conflicted_candidate_ids(preferences.conflicted_candidate_ids.clone());
        let initial_snapshot = merge_tool_state(
            coordinator.snapshot(),
            &tool_coordinator.snapshot(),
            ExternalToolProductState::default(),
        );
        let (updates, _) = broadcast::channel(32);
        let service = Arc::new(Self {
            workspace_root,
            coordinator: Arc::new(StdMutex::new(coordinator)),
            tool_coordinator: Arc::new(StdMutex::new(tool_coordinator)),
            snapshot: StdMutex::new(initial_snapshot),
            updates,
            watch_states: tokio::sync::Mutex::new(BTreeMap::new()),
            refresh_gate: tokio::sync::Mutex::new(()),
            product_rebuild_gate: tokio::sync::Mutex::new(()),
            discovery_tasks: tokio::sync::Mutex::new(BTreeMap::new()),
            tool_discovery_tasks: tokio::sync::Mutex::new(BTreeMap::new()),
            initial_refresh_completed: AtomicBool::new(false),
            background_refresh_scheduled: AtomicBool::new(false),
            initial_refresh_gate: tokio::sync::Mutex::new(()),
            keepalive_started: AtomicBool::new(false),
            last_access_epoch_seconds: AtomicU64::new(epoch_seconds()),
            watcher: Arc::new(FileWatchService::new(FileWatcherConfig::default())),
            #[cfg(test)]
            tool_decision_gate_waiting: tokio::sync::Notify::new(),
            #[cfg(test)]
            tool_decision_gate_acquired: tokio::sync::Notify::new(),
        });
        service.start_watching().await;
        Ok(service)
    }

    async fn refresh(self: &Arc<Self>) -> Result<ExternalSourceCatalogSnapshot, String> {
        self.refresh_with_worker_recovery(WorkerRecoveryPolicy::ResetAndAttempt)
            .await
    }

    async fn refresh_preserving_worker_recovery(
        self: &Arc<Self>,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        self.refresh_with_worker_recovery(WorkerRecoveryPolicy::Preserve)
            .await
    }

    async fn refresh_worker_loss_once(
        self: &Arc<Self>,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        self.refresh_with_worker_recovery(WorkerRecoveryPolicy::PendingOnce)
            .await
    }

    async fn refresh_with_worker_recovery(
        self: &Arc<Self>,
        recovery_policy: WorkerRecoveryPolicy,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        // Preferences are global to the local execution domain and may be
        // changed by another BitFun process. Synchronize before every refresh
        // so a cached CLI/Desktop service cannot keep an externally disabled
        // source active.
        sync_service_preferences(self).await?;
        let _refresh_guard = self.refresh_gate.lock().await;
        if matches!(recovery_policy, WorkerRecoveryPolicy::ResetAndAttempt) {
            reset_external_tool_workspace_recovery_budget(self.workspace_root.as_deref()).await;
        }
        let recovery_targets = if matches!(
            recovery_policy,
            WorkerRecoveryPolicy::PendingOnce | WorkerRecoveryPolicy::ResetAndAttempt
        ) {
            begin_external_tool_workspace_recovery(self.workspace_root.as_deref()).await
        } else {
            BTreeSet::new()
        };
        let requests = lock_coordinator(&self.coordinator).discovery_requests();
        let scheduled = self.prepare_discovery_tasks(requests).await;
        let tool_requests = lock_tool_coordinator(&self.tool_coordinator).discovery_requests();
        let tool_scheduled = self.prepare_tool_discovery_tasks(tool_requests).await;
        let (polled, tool_polled) = tokio::join!(
            poll_discovery_tasks(scheduled, PROVIDER_DISCOVERY_TIMEOUT),
            poll_tool_discovery_tasks(tool_scheduled, PROVIDER_DISCOVERY_TIMEOUT),
        );
        let results = self.finish_discovery_poll(polled).await;
        let tool_results = self.finish_tool_discovery_poll(tool_polled).await;
        let command_snapshot = lock_coordinator(&self.coordinator).apply_discovery_results(results);
        lock_tool_coordinator(&self.tool_coordinator).apply_discovery_results(tool_results);
        self.ensure_watch_roots().await;
        let snapshot = self
            .rebuild_product_snapshot_with_worker_recovery(command_snapshot, &recovery_targets)
            .await;
        let snapshot = snapshot?;
        let _ = self.updates.send(snapshot.clone());
        self.initial_refresh_completed
            .store(true, Ordering::Release);
        Ok(snapshot)
    }

    async fn ensure_initial_refresh_with<F, Fut>(
        &self,
        refresh: F,
    ) -> Result<ExternalSourceCatalogSnapshot, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<ExternalSourceCatalogSnapshot, String>>,
    {
        if self.initial_refresh_completed.load(Ordering::Acquire) {
            return Ok(self.snapshot());
        }
        let _initial_refresh_guard = self.initial_refresh_gate.lock().await;
        if self.initial_refresh_completed.load(Ordering::Acquire) {
            return Ok(self.snapshot());
        }
        let snapshot = refresh().await?;
        self.initial_refresh_completed
            .store(true, Ordering::Release);
        Ok(snapshot)
    }

    async fn ensure_initial_refresh(
        self: &Arc<Self>,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        self.ensure_initial_refresh_with(|| self.refresh()).await
    }

    async fn rebuild_product_snapshot(
        &self,
        command_snapshot: ExternalSourceCatalogSnapshot,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        self.rebuild_product_snapshot_with_worker_recovery(command_snapshot, &BTreeSet::new())
            .await
    }

    async fn rebuild_product_snapshot_with_worker_recovery(
        &self,
        _command_snapshot: ExternalSourceCatalogSnapshot,
        worker_recovery_targets: &BTreeSet<String>,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let _rebuild_guard = self.product_rebuild_gate.lock().await;
        let command_snapshot = lock_coordinator(&self.coordinator).snapshot();
        let preferences = read_external_sources_config().await?;
        let mut state = reconcile_external_tools(
            self.workspace_root.as_deref(),
            "local-user",
            &self.tool_coordinator,
            ExternalToolDecisions {
                approved_targets: &preferences.approved_tool_targets,
                declined_decisions_by_approval: &preferences.declined_tool_decisions,
                conflict_choices: &preferences.tool_conflict_choices,
            },
            worker_recovery_targets,
        )
        .await;
        if let Err(error) = persist_observed_tool_conflicts(&state.conflicts).await {
            state.diagnostics.push(ExternalSourceDiagnostic {
                severity: bitfun_product_domains::external_sources::ExternalSourceDiagnosticSeverity::Warning,
                code: "external_tool.conflict_history_write_failed".to_string(),
                message: format!(
                    "Could not persist external tool conflict history; the current catalog remains fail-closed: {error}"
                ),
                source: None,
            });
        }
        let tool_snapshot = lock_tool_coordinator(&self.tool_coordinator).snapshot();
        let mut snapshot = merge_tool_state(command_snapshot, &tool_snapshot, state);
        let mut current = lock_snapshot(&self.snapshot);
        snapshot.generation = snapshot
            .generation
            .max(current.generation.saturating_add(1));
        *current = snapshot.clone();
        Ok(snapshot)
    }

    async fn prepare_discovery_tasks(
        &self,
        requests: Vec<ExternalSourceDiscoveryRequest>,
    ) -> Vec<(
        bitfun_product_domains::external_sources::ProviderId,
        SharedDiscoveryTask,
        bool,
    )> {
        let mut tasks = self.discovery_tasks.lock().await;
        requests
            .into_iter()
            .map(|request| {
                let provider_id = request.provider_id().clone();
                if let Some(in_flight) = tasks.get(&provider_id) {
                    return (provider_id, in_flight.task.clone(), false);
                }
                let task = spawn_discovery_task(request);
                tasks.insert(
                    provider_id.clone(),
                    InFlightDiscovery {
                        task: task.clone(),
                        wake_scheduled: false,
                    },
                );
                (provider_id, task, true)
            })
            .collect()
    }

    async fn finish_discovery_poll(
        self: &Arc<Self>,
        polled: Vec<DiscoveryPoll>,
    ) -> Vec<ExternalSourceDiscoveryResult> {
        let mut results = Vec::with_capacity(polled.len());
        let mut wake_tasks = Vec::new();
        let mut tasks = self.discovery_tasks.lock().await;
        for poll in polled {
            match poll {
                DiscoveryPoll::Complete(result) => {
                    tasks.remove(&result.provider_id().clone());
                    results.push(result);
                }
                DiscoveryPoll::InFlight(provider_id) => {
                    results.push(discovery_failure(
                        provider_id,
                        "external_source.discovery_in_progress",
                        "provider discovery is still running; using its last valid version",
                    ));
                }
                DiscoveryPoll::TimedOut(provider_id) => {
                    if let Some(in_flight) = tasks.get_mut(&provider_id) {
                        if !in_flight.wake_scheduled {
                            in_flight.wake_scheduled = true;
                            wake_tasks.push((provider_id.clone(), in_flight.task.clone()));
                        }
                    }
                    results.push(discovery_failure(
                        provider_id,
                        "external_source.discovery_timeout",
                        "provider discovery exceeded the 5 second deadline",
                    ));
                }
            }
        }
        drop(tasks);
        for (provider_id, task) in wake_tasks {
            let weak = Arc::downgrade(self);
            tokio::spawn(async move {
                let result = task.await;
                let Some(service) = weak.upgrade() else {
                    return;
                };
                service
                    .complete_deferred_discovery(provider_id, result)
                    .await;
            });
        }
        results
    }

    async fn prepare_tool_discovery_tasks(
        &self,
        requests: Vec<ExternalToolDiscoveryRequest>,
    ) -> Vec<(
        bitfun_product_domains::external_sources::ProviderId,
        SharedToolDiscoveryTask,
        bool,
    )> {
        let mut tasks = self.tool_discovery_tasks.lock().await;
        requests
            .into_iter()
            .map(|request| {
                let provider_id = request.provider_id().clone();
                if let Some(in_flight) = tasks.get(&provider_id) {
                    return (provider_id, in_flight.task.clone(), false);
                }
                let task = spawn_tool_discovery_task(request);
                tasks.insert(
                    provider_id.clone(),
                    InFlightToolDiscovery {
                        task: task.clone(),
                        wake_scheduled: false,
                    },
                );
                (provider_id, task, true)
            })
            .collect()
    }

    async fn finish_tool_discovery_poll(
        self: &Arc<Self>,
        polled: Vec<ToolDiscoveryPoll>,
    ) -> Vec<ExternalToolDiscoveryResult> {
        let mut results = Vec::with_capacity(polled.len());
        let mut wake_tasks = Vec::new();
        let mut tasks = self.tool_discovery_tasks.lock().await;
        for poll in polled {
            match poll {
                ToolDiscoveryPoll::Complete(result) => {
                    tasks.remove(&result.provider_id().clone());
                    results.push(result);
                }
                ToolDiscoveryPoll::InFlight(provider_id) => results.push(tool_discovery_failure(
                    provider_id,
                    "external_tool.discovery_in_progress",
                    "tool provider discovery is still running; using its last valid version",
                )),
                ToolDiscoveryPoll::TimedOut(provider_id) => {
                    if let Some(in_flight) = tasks.get_mut(&provider_id) {
                        if !in_flight.wake_scheduled {
                            in_flight.wake_scheduled = true;
                            wake_tasks.push((provider_id.clone(), in_flight.task.clone()));
                        }
                    }
                    results.push(tool_discovery_failure(
                        provider_id,
                        "external_tool.discovery_timeout",
                        "tool provider discovery exceeded the 5 second deadline",
                    ));
                }
            }
        }
        drop(tasks);
        for (provider_id, task) in wake_tasks {
            let weak = Arc::downgrade(self);
            tokio::spawn(async move {
                let result = task.await;
                let Some(service) = weak.upgrade() else {
                    return;
                };
                service
                    .complete_deferred_tool_discovery(provider_id, result)
                    .await;
            });
        }
        results
    }

    async fn complete_deferred_discovery(
        &self,
        provider_id: bitfun_product_domains::external_sources::ProviderId,
        result: ExternalSourceDiscoveryResult,
    ) {
        let _refresh_guard = self.refresh_gate.lock().await;
        if self
            .discovery_tasks
            .lock()
            .await
            .remove(&provider_id)
            .is_none()
        {
            return;
        }
        let command_snapshot = lock_coordinator(&self.coordinator).apply_discovery_result(result);
        self.ensure_watch_roots().await;
        if let Ok(snapshot) = self.rebuild_product_snapshot(command_snapshot).await {
            let _ = self.updates.send(snapshot);
        }
    }

    async fn complete_deferred_tool_discovery(
        &self,
        provider_id: bitfun_product_domains::external_sources::ProviderId,
        result: ExternalToolDiscoveryResult,
    ) {
        let _refresh_guard = self.refresh_gate.lock().await;
        if self
            .tool_discovery_tasks
            .lock()
            .await
            .remove(&provider_id)
            .is_none()
        {
            return;
        }
        lock_tool_coordinator(&self.tool_coordinator).apply_discovery_result(result);
        self.ensure_watch_roots().await;
        let command_snapshot = lock_coordinator(&self.coordinator).snapshot();
        if let Ok(snapshot) = self.rebuild_product_snapshot(command_snapshot).await {
            let _ = self.updates.send(snapshot);
        }
    }

    fn ensure_background_refresh(self: &Arc<Self>) {
        if self.initial_refresh_completed.load(Ordering::Acquire)
            || self
                .background_refresh_scheduled
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return;
        }
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            let Some(service) = weak.upgrade() else {
                return;
            };
            if let Err(error) = service.ensure_initial_refresh().await {
                log::warn!("Initial external source refresh failed: {}", error);
            }
            service
                .background_refresh_scheduled
                .store(false, Ordering::Release);
        });
    }

    fn touch(&self) {
        self.last_access_epoch_seconds
            .store(epoch_seconds(), Ordering::Release);
    }

    fn ensure_idle_keepalive(self: &Arc<Self>) {
        if self
            .keepalive_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let service = Arc::clone(self);
        tokio::spawn(async move {
            const IDLE_SECONDS: u64 = 300;
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                let idle_for = epoch_seconds()
                    .saturating_sub(service.last_access_epoch_seconds.load(Ordering::Acquire));
                // The keepalive itself and this task account for one strong
                // service reference. A subscription or in-flight operation
                // keeps the service alive independently of idle time.
                if idle_for < IDLE_SECONDS || Arc::strong_count(&service) > 1 {
                    continue;
                }
                let _service_gate = workspace_service_gate().lock().await;
                let idle_for = epoch_seconds()
                    .saturating_sub(service.last_access_epoch_seconds.load(Ordering::Acquire));
                if idle_for < IDLE_SECONDS || Arc::strong_count(&service) > 1 {
                    continue;
                }
                let _rebuild_guard = service.product_rebuild_gate.lock().await;
                if Arc::strong_count(&service) > 1 {
                    continue;
                }
                let key = service.workspace_root.clone();
                if let Some(entry) = workspace_services().get(&key) {
                    let should_remove = entry
                        .value()
                        .upgrade()
                        .is_some_and(|cached| Arc::ptr_eq(&cached, &service));
                    drop(entry);
                    if should_remove {
                        workspace_services().remove(&key);
                        release_external_tool_workspace(key.as_deref()).await;
                    }
                }
                break;
            }
        });
    }

    fn snapshot(&self) -> ExternalSourceCatalogSnapshot {
        lock_snapshot(&self.snapshot).clone()
    }

    async fn set_source_enabled(
        &self,
        stable_key: &str,
        enabled: bool,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let (previous_commands, command_known) = {
            let mut coordinator = lock_coordinator(&self.coordinator);
            let previous = coordinator.suppressed_sources().clone();
            let known = coordinator.set_source_enabled(stable_key, enabled).is_ok();
            (previous, known)
        };
        let (previous_tools, tool_known) = {
            let mut coordinator = lock_tool_coordinator(&self.tool_coordinator);
            let previous = coordinator.suppressed_sources().clone();
            let known = coordinator.set_source_enabled(stable_key, enabled).is_ok();
            (previous, known)
        };
        if !command_known && !tool_known {
            return Err(format!("unknown external source: {stable_key}"));
        }
        let authoritative = match persist_source_enabled_change(stable_key, enabled).await {
            Ok(authoritative) => authoritative,
            Err(error) => {
                lock_coordinator(&self.coordinator).replace_suppressed_sources(previous_commands);
                lock_tool_coordinator(&self.tool_coordinator)
                    .replace_suppressed_sources(previous_tools);
                return Err(error);
            }
        };
        lock_coordinator(&self.coordinator).replace_suppressed_sources(authoritative.clone());
        lock_tool_coordinator(&self.tool_coordinator)
            .replace_suppressed_sources(authoritative.clone());
        propagate_suppressed_sources(&authoritative);
        let command_snapshot = lock_coordinator(&self.coordinator).snapshot();
        self.rebuild_product_snapshot(command_snapshot).await
    }

    async fn set_conflict_choice(
        &self,
        conflict_key: &str,
        candidate_id: &str,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let (previous_choices, previous_lineage_keys, previous_conflicted_ids, participants) = {
            let mut coordinator = lock_coordinator(&self.coordinator);
            let participants = coordinator
                .snapshot()
                .command_conflicts
                .into_iter()
                .find(|conflict| conflict.conflict_key == conflict_key)
                .map(|conflict| {
                    conflict
                        .candidates
                        .into_iter()
                        .map(|candidate| candidate.candidate_id)
                        .collect::<Vec<_>>()
                })
                .ok_or_else(|| format!("unknown external source conflict: {conflict_key}"))?;
            let previous_choices = coordinator.conflict_choices().clone();
            let previous_lineage_keys = coordinator.conflict_lineage_current_keys().clone();
            let previous_conflicted_ids = coordinator.conflicted_candidate_ids().clone();
            coordinator.set_conflict_choice(conflict_key, candidate_id)?;
            (
                previous_choices,
                previous_lineage_keys,
                previous_conflicted_ids,
                participants,
            )
        };
        let (updated_choices, updated_lineage_keys, updated_conflicted_ids) = {
            let coordinator = lock_coordinator(&self.coordinator);
            (
                coordinator.conflict_choices().clone(),
                coordinator.conflict_lineage_current_keys().clone(),
                coordinator.conflicted_candidate_ids().clone(),
            )
        };
        let authoritative =
            match persist_conflict_choice(conflict_key, candidate_id, participants).await {
                Ok(authoritative) => authoritative,
                Err(error) => {
                    let mut coordinator = lock_coordinator(&self.coordinator);
                    coordinator.replace_conflict_choices(previous_choices);
                    coordinator.replace_conflict_lineage_current_keys(previous_lineage_keys);
                    coordinator.replace_conflicted_candidate_ids(previous_conflicted_ids);
                    return Err(error);
                }
            };
        if authoritative.conflict_choices != updated_choices
            || authoritative.conflict_lineage_current_keys != updated_lineage_keys
            || authoritative.conflicted_candidate_ids != updated_conflicted_ids
        {
            log::debug!("External source conflict preferences changed in another workspace");
        }
        propagate_conflict_preferences(&authoritative);
        let command_snapshot = lock_coordinator(&self.coordinator).snapshot();
        self.rebuild_product_snapshot(command_snapshot).await
    }

    async fn set_tool_target_decision(
        &self,
        approval_key: &str,
        decision_key: &str,
        approved: bool,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        // Keep preview validation, preference persistence and the resulting
        // product rebuild in the same ordering domain as watcher refreshes.
        // Otherwise an approval for content v1 could be persisted after a
        // refresh installs v2 with the same capability-based approval key.
        #[cfg(test)]
        self.tool_decision_gate_waiting.notify_one();
        let _refresh_guard = self.refresh_gate.lock().await;
        #[cfg(test)]
        self.tool_decision_gate_acquired.notify_one();
        let snapshot = self.snapshot();
        let known = snapshot.tool_approval_requests.iter().any(|request| {
            request.approval_key == approval_key && request.decision_key == decision_key
        }) || snapshot
            .tools
            .iter()
            .any(|tool| tool.approval_key == approval_key && tool.decision_key == decision_key);
        if !known {
            return Err("external tool decision is stale or unknown".to_string());
        }
        validate_conflict_preference(approval_key, decision_key)?;
        let preferences =
            persist_tool_target_decision(approval_key, decision_key, approved).await?;
        propagate_tool_preferences(&preferences);
        let command_snapshot = lock_coordinator(&self.coordinator).snapshot();
        self.rebuild_product_snapshot(command_snapshot).await
    }

    async fn set_tool_conflict_choice(
        &self,
        conflict_key: &str,
        candidate_id: &str,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let known = self.snapshot().tool_conflicts.iter().any(|conflict| {
            conflict.conflict_key == conflict_key
                && conflict
                    .candidates
                    .iter()
                    .any(|candidate| candidate.candidate_id == candidate_id)
        });
        if !known {
            return Err("external tool conflict choice is stale or unknown".to_string());
        }
        validate_conflict_preference(conflict_key, candidate_id)?;
        let preferences = persist_tool_conflict_choice(conflict_key, candidate_id).await?;
        propagate_tool_preferences(&preferences);
        let command_snapshot = lock_coordinator(&self.coordinator).snapshot();
        self.rebuild_product_snapshot(command_snapshot).await
    }

    async fn expand_command(
        self: &Arc<Self>,
        name: &str,
        arguments: &str,
        expected_candidate_id: Option<&str>,
        expected_content_version: Option<&str>,
    ) -> Result<ExpandedPromptCommand, String> {
        // Explicit invocation refreshes first, so a stable deletion cannot be
        // bypassed by an old menu projection.
        self.refresh_preserving_worker_recovery().await?;
        let coordinator = Arc::clone(&self.coordinator);
        let name = name.to_string();
        let arguments = arguments.to_string();
        let expected_candidate_id = expected_candidate_id.map(str::to_string);
        let expected_content_version = expected_content_version.map(str::to_string);
        tokio::task::spawn_blocking(move || {
            lock_coordinator(&coordinator)
                .expand_command_guarded(
                    &name,
                    &arguments,
                    expected_candidate_id.as_deref(),
                    expected_content_version.as_deref(),
                )
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| format!("external command expansion task failed: {error}"))?
    }

    async fn start_watching(self: &Arc<Self>) {
        let watch_roots = self.watch_roots();
        if watch_roots.is_empty() {
            return;
        }
        self.ensure_watch_roots().await;
        let mut receiver = self.watcher.subscribe();
        let weak: Weak<Self> = Arc::downgrade(self);
        tokio::spawn(async move {
            loop {
                let events = match receiver.recv().await {
                    Ok(events) => events,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        if let Some(service) = weak.upgrade() {
                            let _ = service.refresh().await;
                            continue;
                        }
                        break;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                let Some(service) = weak.upgrade() else {
                    break;
                };
                let watch_roots = service.watch_roots();
                let relevant = events.iter().any(|event| {
                    let path = Path::new(&event.path);
                    watch_roots.iter().any(|root| path.starts_with(&root.path))
                });
                if !relevant {
                    continue;
                }
                if let Err(error) = service.refresh().await {
                    log::warn!(
                        "External source background refresh failed for '{}': {}",
                        service
                            .workspace_root
                            .as_deref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "user-global".to_string()),
                        error
                    );
                }
            }
        });
    }

    async fn ensure_watch_roots(&self) {
        let watch_roots = self.watch_roots();
        let watcher = Arc::clone(&self.watcher);
        let mut states = self.watch_states.lock().await;
        for root in watch_roots {
            let key = (root.path.clone(), root.recursive);
            let exists = root.path.exists();
            let was_available = states.get(&key).copied().unwrap_or(false);
            if !exists {
                states.insert(key, false);
                continue;
            }
            if was_available {
                continue;
            }
            let mut config = FileWatcherConfig::default();
            config.watch_recursively = root.recursive;
            config.ignore_hidden_files = false;
            config.debounce_interval_ms = 350;
            let path = root.path.to_string_lossy().to_string();
            match watcher.watch_path(&path, Some(config)).await {
                Ok(()) => {
                    states.insert(key, true);
                }
                Err(error) => {
                    states.insert(key, false);
                    log::warn!("Failed to watch external source root '{}': {}", path, error);
                }
            }
        }
    }

    fn watch_roots(&self) -> Vec<bitfun_product_domains::external_sources::ExternalWatchRoot> {
        let mut roots = BTreeMap::new();
        for root in lock_coordinator(&self.coordinator)
            .watch_roots()
            .into_iter()
            .chain(lock_tool_coordinator(&self.tool_coordinator).watch_roots())
        {
            roots
                .entry(root.path)
                .and_modify(|recursive| *recursive |= root.recursive)
                .or_insert(root.recursive);
        }
        if let Ok(store) = ExternalSourcePreferenceStore::global() {
            if let Some(parent) = store.path.parent() {
                roots.entry(parent.to_path_buf()).or_insert(false);
            }
        }
        roots
            .into_iter()
            .map(
                |(path, recursive)| bitfun_product_domains::external_sources::ExternalWatchRoot {
                    path,
                    recursive,
                },
            )
            .collect()
    }
}

enum DiscoveryPoll {
    Complete(ExternalSourceDiscoveryResult),
    InFlight(bitfun_product_domains::external_sources::ProviderId),
    TimedOut(bitfun_product_domains::external_sources::ProviderId),
}

enum ToolDiscoveryPoll {
    Complete(ExternalToolDiscoveryResult),
    InFlight(bitfun_product_domains::external_sources::ProviderId),
    TimedOut(bitfun_product_domains::external_sources::ProviderId),
}

async fn poll_discovery_tasks(
    scheduled: Vec<(
        bitfun_product_domains::external_sources::ProviderId,
        SharedDiscoveryTask,
        bool,
    )>,
    timeout: std::time::Duration,
) -> Vec<DiscoveryPoll> {
    join_all(
        scheduled
            .into_iter()
            .map(|(provider_id, task, is_new)| async move {
                if !is_new {
                    return match task.clone().now_or_never() {
                        Some(result) => DiscoveryPoll::Complete(result),
                        None => DiscoveryPoll::InFlight(provider_id),
                    };
                }
                match tokio::time::timeout(timeout, task).await {
                    Ok(result) => DiscoveryPoll::Complete(result),
                    Err(_) => DiscoveryPoll::TimedOut(provider_id),
                }
            }),
    )
    .await
}

async fn poll_tool_discovery_tasks(
    scheduled: Vec<(
        bitfun_product_domains::external_sources::ProviderId,
        SharedToolDiscoveryTask,
        bool,
    )>,
    timeout: std::time::Duration,
) -> Vec<ToolDiscoveryPoll> {
    join_all(
        scheduled
            .into_iter()
            .map(|(provider_id, task, is_new)| async move {
                if !is_new {
                    return match task.clone().now_or_never() {
                        Some(result) => ToolDiscoveryPoll::Complete(result),
                        None => ToolDiscoveryPoll::InFlight(provider_id),
                    };
                }
                match tokio::time::timeout(timeout, task).await {
                    Ok(result) => ToolDiscoveryPoll::Complete(result),
                    Err(_) => ToolDiscoveryPoll::TimedOut(provider_id),
                }
            }),
    )
    .await
}

fn spawn_discovery_task(request: ExternalSourceDiscoveryRequest) -> SharedDiscoveryTask {
    let provider_id = request.provider_id().clone();
    async move {
        match tokio::task::spawn_blocking(move || request.execute()).await {
            Ok(result) => result,
            Err(error) => discovery_failure(
                provider_id,
                "external_source.discovery_task_failed",
                &format!("provider discovery task failed: {error}"),
            ),
        }
    }
    .boxed()
    .shared()
}

fn spawn_tool_discovery_task(request: ExternalToolDiscoveryRequest) -> SharedToolDiscoveryTask {
    let provider_id = request.provider_id().clone();
    async move {
        match tokio::task::spawn_blocking(move || request.execute()).await {
            Ok(result) => result,
            Err(error) => tool_discovery_failure(
                provider_id,
                "external_tool.discovery_task_failed",
                &format!("tool provider discovery task failed: {error}"),
            ),
        }
    }
    .boxed()
    .shared()
}

fn discovery_failure(
    provider_id: bitfun_product_domains::external_sources::ProviderId,
    code: &str,
    message: &str,
) -> ExternalSourceDiscoveryResult {
    ExternalSourceDiscoveryResult::failed(
        provider_id,
        bitfun_product_domains::external_sources::ExternalSourceProviderError::new(
            code, message, true,
        ),
    )
}

fn tool_discovery_failure(
    provider_id: bitfun_product_domains::external_sources::ProviderId,
    code: &str,
    message: &str,
) -> ExternalToolDiscoveryResult {
    ExternalToolDiscoveryResult::failed(
        provider_id,
        bitfun_product_domains::external_sources::ExternalSourceProviderError::new(
            code, message, true,
        ),
    )
}

fn lock_coordinator(
    coordinator: &StdMutex<ExternalSourceCoordinator>,
) -> MutexGuard<'_, ExternalSourceCoordinator> {
    match coordinator.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::error!("External source coordinator mutex was poisoned, recovering lock");
            poisoned.into_inner()
        }
    }
}

fn lock_tool_coordinator(
    coordinator: &StdMutex<ExternalToolCoordinator>,
) -> MutexGuard<'_, ExternalToolCoordinator> {
    match coordinator.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn lock_snapshot(
    snapshot: &StdMutex<ExternalSourceCatalogSnapshot>,
) -> MutexGuard<'_, ExternalSourceCatalogSnapshot> {
    match snapshot.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

static WORKSPACE_SERVICES: OnceLock<
    DashMap<Option<PathBuf>, Weak<WorkspaceExternalSourceService>>,
> = OnceLock::new();
static TOOL_REGISTRY_CHANGE_EPOCH: AtomicU64 = AtomicU64::new(0);
static TOOL_REGISTRY_REBUILD_SCHEDULED: AtomicBool = AtomicBool::new(false);

fn workspace_services() -> &'static DashMap<Option<PathBuf>, Weak<WorkspaceExternalSourceService>> {
    WORKSPACE_SERVICES.get_or_init(DashMap::new)
}

fn workspace_service_gate() -> &'static tokio::sync::Mutex<()> {
    static GATE: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    GATE.get_or_init(|| tokio::sync::Mutex::new(()))
}

fn normalize_workspace_root(workspace_root: Option<&Path>) -> Result<Option<PathBuf>, String> {
    let Some(workspace_root) = workspace_root else {
        return Ok(None);
    };
    if !workspace_root.is_absolute() {
        return Err("external source workspace root must be absolute".to_string());
    }
    Ok(Some(
        dunce::canonicalize(workspace_root).unwrap_or_else(|_| workspace_root.to_path_buf()),
    ))
}

async fn service_for(
    workspace_root: Option<&Path>,
) -> Result<Arc<WorkspaceExternalSourceService>, String> {
    let workspace_root = normalize_workspace_root(workspace_root)?;
    // Serialize cache acquisition with idle retirement. Without this lease
    // gate, a caller could upgrade the weak entry after the retirement count
    // check and have its newly acquired routes removed underneath it.
    let _service_gate = workspace_service_gate().lock().await;
    if let Some(service) = workspace_services()
        .get(&workspace_root)
        .and_then(|service| service.value().upgrade())
    {
        service.touch();
        sync_service_preferences(&service).await?;
        return Ok(service);
    }
    let created = WorkspaceExternalSourceService::create(workspace_root.clone()).await?;
    let service = match workspace_services().entry(workspace_root) {
        Entry::Occupied(mut entry) => match entry.get().upgrade() {
            Some(existing) => existing,
            None => {
                entry.insert(Arc::downgrade(&created));
                created
            }
        },
        Entry::Vacant(entry) => {
            entry.insert(Arc::downgrade(&created));
            created
        }
    };
    service.touch();
    service.ensure_idle_keepalive();
    sync_service_preferences(&service).await?;
    Ok(service)
}

fn epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn read_external_sources_config() -> Result<ExternalSourcesConfig, String> {
    ExternalSourcePreferenceStore::global()?.read().await
}

pub(crate) async fn external_tool_invocation_is_authorized(
    approval_key: &str,
    source_key: &str,
) -> Result<bool, String> {
    let preferences = read_external_sources_config().await?;
    Ok(external_tool_invocation_is_authorized_by(
        &preferences,
        approval_key,
        source_key,
    ))
}

fn external_tool_invocation_is_authorized_by(
    preferences: &ExternalSourcesConfig,
    approval_key: &str,
    source_preference_key: &str,
) -> bool {
    preferences.approved_tool_targets.contains(approval_key)
        && !preferences
            .suppressed_source_keys
            .iter()
            .any(|suppressed| suppressed == source_preference_key)
}

pub(crate) async fn external_tool_conflict_selection_is_current(
    conflict_key: &str,
    candidate_id: Option<&str>,
) -> Result<bool, String> {
    let preferences = read_external_sources_config().await?;
    let persisted = preferences
        .tool_conflict_choices
        .get(conflict_key)
        .map(String::as_str)
        .filter(|choice| {
            *choice != UNRESOLVED_TOOL_CONFLICT_CHOICE
                && *choice != TOOL_CONFLICT_RESELECTION_REQUIRED
        });
    Ok(persisted == candidate_id)
}

async fn persist_observed_tool_conflicts(conflicts: &[ExternalToolConflict]) -> Result<(), String> {
    if conflicts.is_empty() {
        return Ok(());
    }
    let conflicts = conflicts.to_vec();
    ExternalSourcePreferenceStore::global()?
        .update(move |config| {
            for conflict in conflicts {
                reconcile_observed_tool_conflict(
                    &mut config.tool_conflict_choices,
                    &conflict.conflict_key,
                );
            }
        })
        .await
        .map(|_| ())
}

fn reconcile_observed_tool_conflict(choices: &mut BTreeMap<String, String>, conflict_key: &str) {
    if choices.contains_key(conflict_key) {
        return;
    }
    let Some((lineage, _)) = conflict_key.rsplit_once(':') else {
        choices.insert(
            conflict_key.to_string(),
            UNRESOLVED_TOOL_CONFLICT_CHOICE.to_string(),
        );
        return;
    };
    let requires_fail_closed_reselection = choices.iter().any(|(existing_key, choice)| {
        existing_key
            .rsplit_once(':')
            .is_some_and(|(existing_lineage, _)| existing_lineage == lineage)
            && (choice.starts_with("external:") || choice == TOOL_CONFLICT_RESELECTION_REQUIRED)
    });
    choices.retain(|existing_key, _| {
        existing_key
            .rsplit_once(':')
            .is_none_or(|(existing_lineage, _)| existing_lineage != lineage)
    });
    choices.insert(
        conflict_key.to_string(),
        if requires_fail_closed_reselection {
            TOOL_CONFLICT_RESELECTION_REQUIRED.to_string()
        } else {
            UNRESOLVED_TOOL_CONFLICT_CHOICE.to_string()
        },
    );
}

async fn persist_source_enabled_change(
    stable_key: &str,
    enabled: bool,
) -> Result<BTreeSet<String>, String> {
    let stable_key = stable_key.to_string();
    ExternalSourcePreferenceStore::global()?
        .update(move |config| {
            let mut sources = config
                .suppressed_source_keys
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            if enabled {
                sources.remove(&stable_key);
            } else {
                sources.insert(stable_key);
            }
            config.suppressed_source_keys = sources.iter().cloned().collect();
            sources
        })
        .await
        .map(|(sources, _)| sources)
}

async fn persist_conflict_choice(
    conflict_key: &str,
    candidate_id: &str,
    participants: Vec<String>,
) -> Result<ExternalSourcesConfig, String> {
    let conflict_key = conflict_key.to_string();
    let candidate_id = candidate_id.to_string();
    ExternalSourcePreferenceStore::global()?
        .update(move |config| {
            ExternalSourceCoordinator::reconcile_conflict_preferences(
                &mut config.conflict_choices,
                &mut config.conflict_lineage_current_keys,
                &mut config.conflicted_candidate_ids,
                &conflict_key,
                &candidate_id,
                &participants,
            );
        })
        .await
        .map(|(_, config)| config)
}

async fn persist_tool_target_decision(
    approval_key: &str,
    decision_key: &str,
    approved: bool,
) -> Result<ExternalSourcesConfig, String> {
    let approval_key = approval_key.to_string();
    let decision_key = decision_key.to_string();
    ExternalSourcePreferenceStore::global()?
        .update(move |config| {
            reconcile_tool_target_decision(config, approval_key, decision_key, approved);
        })
        .await
        .map(|(_, config)| config)
}

fn reconcile_tool_target_decision(
    config: &mut ExternalSourcesConfig,
    approval_key: String,
    decision_key: String,
    approved: bool,
) {
    if approved {
        config.approved_tool_targets.insert(approval_key.clone());
        config.declined_tool_decisions.remove(&approval_key);
    } else {
        config.approved_tool_targets.remove(&approval_key);
        config
            .declined_tool_decisions
            .insert(approval_key, decision_key);
    }
}

async fn persist_tool_conflict_choice(
    conflict_key: &str,
    candidate_id: &str,
) -> Result<ExternalSourcesConfig, String> {
    let conflict_key = conflict_key.to_string();
    let candidate_id = candidate_id.to_string();
    ExternalSourcePreferenceStore::global()?
        .update(move |config| {
            reconcile_versioned_tool_conflict_choice(
                &mut config.tool_conflict_choices,
                conflict_key,
                candidate_id,
            );
        })
        .await
        .map(|(_, config)| config)
}

fn reconcile_versioned_tool_conflict_choice(
    choices: &mut BTreeMap<String, String>,
    conflict_key: String,
    candidate_id: String,
) {
    if let Some((lineage, _)) = conflict_key.rsplit_once(':') {
        choices.retain(|existing_key, _| {
            existing_key
                .rsplit_once(':')
                .is_none_or(|(existing_lineage, _)| existing_lineage != lineage)
        });
    }
    choices.insert(conflict_key, candidate_id);
}

fn propagate_suppressed_sources(sources: &BTreeSet<String>) {
    for service in workspace_services().iter() {
        let Some(service) = service.value().upgrade() else {
            continue;
        };
        lock_coordinator(&service.coordinator).replace_suppressed_sources(sources.clone());
        lock_tool_coordinator(&service.tool_coordinator)
            .replace_suppressed_sources(sources.clone());
        tokio::spawn(async move {
            let command_snapshot = lock_coordinator(&service.coordinator).snapshot();
            if let Ok(snapshot) = service.rebuild_product_snapshot(command_snapshot).await {
                let _ = service.updates.send(snapshot);
            }
        });
    }
}

fn propagate_conflict_preferences(preferences: &ExternalSourcesConfig) {
    for service in workspace_services().iter() {
        let Some(service) = service.value().upgrade() else {
            continue;
        };
        {
            let mut coordinator = lock_coordinator(&service.coordinator);
            coordinator.replace_conflict_choices(preferences.conflict_choices.clone());
            coordinator.replace_conflict_lineage_current_keys(
                preferences.conflict_lineage_current_keys.clone(),
            );
            coordinator
                .replace_conflicted_candidate_ids(preferences.conflicted_candidate_ids.clone());
        }
        tokio::spawn(async move {
            let command_snapshot = lock_coordinator(&service.coordinator).snapshot();
            if let Ok(snapshot) = service.rebuild_product_snapshot(command_snapshot).await {
                let _ = service.updates.send(snapshot);
            }
        });
    }
}

fn propagate_tool_preferences(_preferences: &ExternalSourcesConfig) {
    for service in workspace_services().iter() {
        let Some(service) = service.value().upgrade() else {
            continue;
        };
        tokio::spawn(async move {
            let command_snapshot = lock_coordinator(&service.coordinator).snapshot();
            if let Ok(snapshot) = service.rebuild_product_snapshot(command_snapshot).await {
                let _ = service.updates.send(snapshot);
            }
        });
    }
}

pub(crate) fn notify_external_tool_registry_changed() {
    TOOL_REGISTRY_CHANGE_EPOCH.fetch_add(1, Ordering::AcqRel);
    if TOOL_REGISTRY_REBUILD_SCHEDULED.swap(true, Ordering::AcqRel) {
        return;
    }
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        TOOL_REGISTRY_REBUILD_SCHEDULED.store(false, Ordering::Release);
        return;
    };
    runtime.spawn(async move {
        loop {
            let observed_epoch = TOOL_REGISTRY_CHANGE_EPOCH.load(Ordering::Acquire);
            let services = workspace_services()
                .iter()
                .filter_map(|entry| entry.value().upgrade())
                .collect::<Vec<_>>();
            for service in services {
                let command_snapshot = lock_coordinator(&service.coordinator).snapshot();
                if let Ok(snapshot) = service.rebuild_product_snapshot(command_snapshot).await {
                    let _ = service.updates.send(snapshot);
                }
            }
            if TOOL_REGISTRY_CHANGE_EPOCH.load(Ordering::Acquire) != observed_epoch {
                continue;
            }
            TOOL_REGISTRY_REBUILD_SCHEDULED.store(false, Ordering::Release);
            if TOOL_REGISTRY_CHANGE_EPOCH.load(Ordering::Acquire) == observed_epoch {
                break;
            }
            if TOOL_REGISTRY_REBUILD_SCHEDULED.swap(true, Ordering::AcqRel) {
                break;
            }
        }
    });
}

async fn sync_service_preferences(service: &WorkspaceExternalSourceService) -> Result<(), String> {
    let preferences = read_external_sources_config().await?;
    let suppressed_sources = preferences
        .suppressed_source_keys
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let command_changed = {
        let mut coordinator = lock_coordinator(&service.coordinator);
        let mut changed = false;
        if coordinator.suppressed_sources() != &suppressed_sources {
            coordinator.replace_suppressed_sources(suppressed_sources.clone());
            changed = true;
        }
        if coordinator.conflict_choices() != &preferences.conflict_choices {
            coordinator.replace_conflict_choices(preferences.conflict_choices.clone());
            changed = true;
        }
        if coordinator.conflict_lineage_current_keys() != &preferences.conflict_lineage_current_keys
        {
            coordinator.replace_conflict_lineage_current_keys(
                preferences.conflict_lineage_current_keys.clone(),
            );
            changed = true;
        }
        if coordinator.conflicted_candidate_ids() != &preferences.conflicted_candidate_ids {
            coordinator
                .replace_conflicted_candidate_ids(preferences.conflicted_candidate_ids.clone());
            changed = true;
        }
        changed
    };
    let tool_changed = {
        let mut coordinator = lock_tool_coordinator(&service.tool_coordinator);
        if coordinator.suppressed_sources() != &suppressed_sources {
            coordinator.replace_suppressed_sources(suppressed_sources);
            true
        } else {
            false
        }
    };
    if command_changed || tool_changed {
        let command_snapshot = lock_coordinator(&service.coordinator).snapshot();
        let snapshot = service.rebuild_product_snapshot(command_snapshot).await?;
        let _ = service.updates.send(snapshot);
    }
    Ok(())
}

fn validate_conflict_preference(conflict_key: &str, candidate_id: &str) -> Result<(), String> {
    if conflict_key.is_empty() || conflict_key.len() > 512 {
        return Err("external source conflict key is invalid".to_string());
    }
    if candidate_id.is_empty() || candidate_id.len() > 512 {
        return Err("external source conflict candidate is invalid".to_string());
    }
    Ok(())
}

pub async fn external_source_conflict_choices() -> Result<
    (
        BTreeMap<String, String>,
        BTreeMap<String, String>,
        BTreeSet<String>,
    ),
    String,
> {
    let preferences = read_external_sources_config().await?;
    Ok((
        preferences.conflict_choices,
        preferences.conflict_lineage_current_keys,
        preferences.conflicted_candidate_ids,
    ))
}

pub async fn remember_external_source_conflict_choice(
    conflict_key: &str,
    candidate_id: &str,
    participants: Vec<String>,
) -> Result<
    (
        BTreeMap<String, String>,
        BTreeMap<String, String>,
        BTreeSet<String>,
    ),
    String,
> {
    validate_conflict_preference(conflict_key, candidate_id)?;
    if participants.is_empty()
        || !participants
            .iter()
            .any(|candidate| candidate == candidate_id)
        || participants
            .iter()
            .any(|candidate| validate_conflict_preference(conflict_key, candidate).is_err())
    {
        return Err("external source conflict participants are invalid".to_string());
    }
    let preferences = persist_conflict_choice(conflict_key, candidate_id, participants).await?;
    propagate_conflict_preferences(&preferences);
    Ok((
        preferences.conflict_choices,
        preferences.conflict_lineage_current_keys,
        preferences.conflicted_candidate_ids,
    ))
}

pub async fn set_external_prompt_command_conflict_choice(
    workspace_root: Option<&Path>,
    conflict_key: &str,
    candidate_id: &str,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    validate_conflict_preference(conflict_key, candidate_id)?;
    service_for(workspace_root)
        .await?
        .set_conflict_choice(conflict_key, candidate_id)
        .await
}

pub async fn set_external_tool_target_decision(
    workspace_root: Option<&Path>,
    approval_key: &str,
    decision_key: &str,
    approved: bool,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    service_for(workspace_root)
        .await?
        .set_tool_target_decision(approval_key, decision_key, approved)
        .await
}

pub async fn set_external_tool_conflict_choice(
    workspace_root: Option<&Path>,
    conflict_key: &str,
    candidate_id: &str,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    service_for(workspace_root)
        .await?
        .set_tool_conflict_choice(conflict_key, candidate_id)
        .await
}

pub async fn external_source_snapshot(
    workspace_root: Option<&Path>,
    force_refresh: bool,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    let service = service_for(workspace_root).await?;
    if force_refresh {
        service.refresh().await
    } else {
        service.ensure_background_refresh();
        Ok(service.snapshot())
    }
}

/// Keep the external-source runtime aligned with an actively assembled product
/// tool catalog. A newly created service performs one synchronous refresh so an
/// idle-retired workspace can restore approved routes before the catalog is
/// exposed to the model. Existing services are only touched; file watchers and
/// explicit refreshes remain responsible for later source changes.
pub(crate) async fn ensure_external_source_workspace_runtime(workspace_root: Option<&Path>) {
    let service = match service_for(workspace_root).await {
        Ok(service) => service,
        Err(error) => {
            log::warn!("Could not retain external source workspace runtime: {error}");
            return;
        }
    };
    if let Err(error) = service.ensure_initial_refresh().await {
        log::warn!("Could not initialize external source workspace runtime: {error}");
        return;
    }
    if external_tool_workspace_requires_recovery(workspace_root).await {
        if let Err(error) = service.refresh_worker_loss_once().await {
            log::warn!("Could not recover external source tool runtime: {error}");
        }
    }
}

pub async fn set_external_source_enabled(
    workspace_root: Option<&Path>,
    source_key: &str,
    enabled: bool,
) -> Result<ExternalSourceCatalogSnapshot, String> {
    service_for(workspace_root)
        .await?
        .set_source_enabled(source_key, enabled)
        .await
}

pub async fn expand_external_prompt_command(
    workspace_root: Option<&Path>,
    name: &str,
    arguments: &str,
    expected_candidate_id: Option<&str>,
    expected_content_version: Option<&str>,
) -> Result<ExpandedPromptCommand, String> {
    service_for(workspace_root)
        .await?
        .expand_command(
            name,
            arguments,
            expected_candidate_id,
            expected_content_version,
        )
        .await
}

pub async fn subscribe_external_source_updates(
    workspace_root: Option<&Path>,
) -> Result<ExternalSourceSubscription, String> {
    let service = service_for(workspace_root).await?;
    let receiver = service.updates.subscribe();
    service.ensure_background_refresh();
    Ok(ExternalSourceSubscription {
        _service: service,
        receiver,
    })
}

pub struct ExternalSourceSubscription {
    _service: Arc<WorkspaceExternalSourceService>,
    receiver: broadcast::Receiver<ExternalSourceCatalogSnapshot>,
}

impl ExternalSourceSubscription {
    pub fn try_recv(
        &mut self,
    ) -> Result<ExternalSourceCatalogSnapshot, broadcast::error::TryRecvError> {
        self.receiver.try_recv()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_product_domains::external_sources::{
        ExternalSourceHealth, ExternalSourceProviderError, ExternalSourceRecord,
        ExternalSourceScope, PromptCommandAvailability, PromptCommandDefinition,
        PromptCommandProviderIdentity, PromptCommandProviderSnapshot, SourceQualifiedCommandId,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct DelayedProvider {
        identity: PromptCommandProviderIdentity,
        source: SourceKey,
        command_name: String,
        delay: std::time::Duration,
        calls: Arc<AtomicUsize>,
    }

    impl PromptCommandSourceProvider for DelayedProvider {
        fn identity(&self) -> PromptCommandProviderIdentity {
            self.identity.clone()
        }

        fn discover(
            &self,
            context: &ExternalSourceContext,
        ) -> Result<PromptCommandProviderSnapshot, ExternalSourceProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(self.delay);
            let record = ExternalSourceRecord {
                key: self.source.clone(),
                ecosystem_id: self.identity.ecosystem_id.clone(),
                display_name: self.identity.display_name.clone(),
                source_kind: "prompt_commands".to_string(),
                scope: ExternalSourceScope::UserGlobal,
                location: format!("/{}", self.command_name),
                execution_domain_id: context.execution_domain_id.clone(),
                health: ExternalSourceHealth::Available,
                content_version: "source-v1".to_string(),
                diagnostics: Vec::new(),
            };
            Ok(PromptCommandProviderSnapshot {
                provider: self.identity.clone(),
                sources: vec![record],
                commands: vec![PromptCommandDefinition {
                    id: SourceQualifiedCommandId::new(
                        self.source.clone(),
                        self.command_name.clone(),
                    )
                    .unwrap(),
                    name: self.command_name.clone(),
                    description: self.command_name.clone(),
                    template: self.command_name.clone(),
                    availability: PromptCommandAvailability::Available,
                    content_version: "command-v1".to_string(),
                }],
                unavailable_command_ids: Vec::new(),
                diagnostics: Vec::new(),
            })
        }

        fn expand(
            &self,
            command: &PromptCommandDefinition,
            _arguments: &str,
        ) -> Result<ExpandedPromptCommand, ExternalSourceProviderError> {
            Ok(ExpandedPromptCommand {
                content: command.template.clone(),
            })
        }

        fn watch_roots(
            &self,
            _context: &ExternalSourceContext,
        ) -> Vec<bitfun_product_domains::external_sources::ExternalWatchRoot> {
            Vec::new()
        }
    }

    fn delayed_provider(
        id: &str,
        delay: std::time::Duration,
        calls: Arc<AtomicUsize>,
    ) -> Arc<dyn PromptCommandSourceProvider> {
        Arc::new(DelayedProvider {
            identity: PromptCommandProviderIdentity::new(id, id, id).unwrap(),
            source: SourceKey::new(id, "global").unwrap(),
            command_name: id.to_string(),
            delay,
            calls,
        })
    }

    fn test_service(
        providers: Vec<Arc<dyn PromptCommandSourceProvider>>,
    ) -> Arc<WorkspaceExternalSourceService> {
        let context = ExternalSourceContext {
            workspace_root: None,
            execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
        };
        let (updates, _) = broadcast::channel(8);
        let coordinator = ExternalSourceCoordinator::new(context.clone(), providers).unwrap();
        let tool_coordinator = ExternalToolCoordinator::new(context, Vec::new()).unwrap();
        let snapshot = merge_tool_state(
            coordinator.snapshot(),
            &tool_coordinator.snapshot(),
            ExternalToolProductState::default(),
        );
        Arc::new(WorkspaceExternalSourceService {
            workspace_root: None,
            coordinator: Arc::new(StdMutex::new(coordinator)),
            tool_coordinator: Arc::new(StdMutex::new(tool_coordinator)),
            snapshot: StdMutex::new(snapshot),
            updates,
            watch_states: tokio::sync::Mutex::new(BTreeMap::new()),
            refresh_gate: tokio::sync::Mutex::new(()),
            product_rebuild_gate: tokio::sync::Mutex::new(()),
            discovery_tasks: tokio::sync::Mutex::new(BTreeMap::new()),
            tool_discovery_tasks: tokio::sync::Mutex::new(BTreeMap::new()),
            initial_refresh_completed: AtomicBool::new(false),
            background_refresh_scheduled: AtomicBool::new(false),
            initial_refresh_gate: tokio::sync::Mutex::new(()),
            keepalive_started: AtomicBool::new(false),
            last_access_epoch_seconds: AtomicU64::new(epoch_seconds()),
            watcher: Arc::new(FileWatchService::new(FileWatcherConfig::default())),
            tool_decision_gate_waiting: tokio::sync::Notify::new(),
            tool_decision_gate_acquired: tokio::sync::Notify::new(),
        })
    }

    #[tokio::test]
    async fn preference_store_merges_updates_from_independent_instances() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("external-sources.json");
        let first = ExternalSourcePreferenceStore::new(path.clone());
        let second = ExternalSourcePreferenceStore::new(path);

        let disable = first.update(|config| {
            config
                .suppressed_source_keys
                .push("opencode:global".to_string());
        });
        let choose = second.update(|config| {
            ExternalSourceCoordinator::reconcile_conflict_preferences(
                &mut config.conflict_choices,
                &mut config.conflict_lineage_current_keys,
                &mut config.conflicted_candidate_ids,
                "prompt_command:local-user:review:v1",
                "candidate-a",
                &["candidate-a".to_string(), "candidate-b".to_string()],
            );
        });
        let (disabled, chosen) = tokio::join!(disable, choose);
        disabled.unwrap();
        chosen.unwrap();

        let persisted = first.read().await.unwrap();
        assert_eq!(persisted.suppressed_source_keys, ["opencode:global"]);
        assert_eq!(
            persisted
                .conflict_choices
                .get("prompt_command:local-user:review:v1")
                .map(String::as_str),
            Some("candidate-a")
        );
        assert_eq!(
            persisted.conflict_lineage_current_keys["prompt_command:local-user:review"],
            "prompt_command:local-user:review:v1"
        );
        assert_eq!(
            persisted.conflicted_candidate_ids,
            BTreeSet::from(["candidate-a".to_string(), "candidate-b".to_string()])
        );
    }

    #[tokio::test]
    async fn invalid_preference_file_is_an_error_instead_of_resetting_choices() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("external-sources.json");
        tokio::fs::write(&path, "{ invalid json").await.unwrap();

        let error = ExternalSourcePreferenceStore::new(path)
            .read()
            .await
            .expect_err("invalid preferences must fail closed");

        assert!(error.contains("deserialize"));
    }

    #[test]
    fn invocation_authorization_uses_the_execution_domain_preference_key() {
        let source = ExternalSourceRecord {
            key: SourceKey::new("opencode", "global-tools").unwrap(),
            ecosystem_id: bitfun_product_domains::external_sources::EcosystemId::new("opencode")
                .unwrap(),
            display_name: "OpenCode tools".to_string(),
            source_kind: "standalone_tools".to_string(),
            scope: ExternalSourceScope::UserGlobal,
            location: "/tools".to_string(),
            execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
            health: ExternalSourceHealth::Available,
            content_version: "v1".to_string(),
            diagnostics: Vec::new(),
        };
        let approval_key = "approval";
        let mut config = ExternalSourcesConfig {
            approved_tool_targets: BTreeSet::from([approval_key.to_string()]),
            ..ExternalSourcesConfig::default()
        };

        config.suppressed_source_keys.push(source.preference_key());
        assert!(!external_tool_invocation_is_authorized_by(
            &config,
            approval_key,
            &source.preference_key()
        ));
        assert!(external_tool_invocation_is_authorized_by(
            &config,
            approval_key,
            &source.key.stable_key()
        ));
    }

    #[test]
    fn observed_tool_conflict_requires_reselection_after_external_lineage_changes() {
        let old = "external_tool:domain:read:old";
        let current = "external_tool:domain:read:new";
        let mut choices = BTreeMap::from([(old.to_string(), "external:source-a".to_string())]);

        reconcile_observed_tool_conflict(&mut choices, current);

        assert!(!choices.contains_key(old));
        assert_eq!(
            choices.get(current).map(String::as_str),
            Some(TOOL_CONFLICT_RESELECTION_REQUIRED)
        );
    }

    #[test]
    fn first_observed_tool_conflict_persists_an_unresolved_lineage() {
        let conflict_key = "external_tool:domain:read:first";
        let mut choices = BTreeMap::new();

        reconcile_observed_tool_conflict(&mut choices, conflict_key);

        assert_eq!(
            choices.get(conflict_key).map(String::as_str),
            Some(UNRESOLVED_TOOL_CONFLICT_CHOICE)
        );
    }

    #[test]
    fn conflict_lineages_are_compact_and_independent() {
        let mut choices = BTreeMap::from([
            (
                "prompt_command:local-user:review:old".to_string(),
                "external-a".to_string(),
            ),
            (
                "native:prompt_command:local-user:help:old".to_string(),
                "bitfun.cli:help".to_string(),
            ),
        ]);
        let mut lineage_keys = BTreeMap::from([
            (
                "prompt_command:local-user:review".to_string(),
                "prompt_command:local-user:review:old".to_string(),
            ),
            (
                "native:prompt_command:local-user:help".to_string(),
                "native:prompt_command:local-user:help:old".to_string(),
            ),
        ]);
        let mut conflicted_ids = BTreeSet::from([
            "external-a".to_string(),
            "external-b".to_string(),
            "bitfun.cli:help".to_string(),
        ]);

        ExternalSourceCoordinator::reconcile_conflict_preferences(
            &mut choices,
            &mut lineage_keys,
            &mut conflicted_ids,
            "native:prompt_command:local-user:help:new",
            "bitfun.cli:help",
            &["bitfun.cli:help".to_string()],
        );

        assert!(choices.contains_key("prompt_command:local-user:review:old"));
        assert!(!choices.contains_key("native:prompt_command:local-user:help:old"));
        assert_eq!(choices.len(), 2);
        assert_eq!(lineage_keys.len(), 2);
    }

    #[test]
    fn tool_decisions_keep_only_the_current_decline_per_approval() {
        let mut config = ExternalSourcesConfig::default();

        reconcile_tool_target_decision(
            &mut config,
            "approval-a".to_string(),
            "decision-v1".to_string(),
            false,
        );
        reconcile_tool_target_decision(
            &mut config,
            "approval-a".to_string(),
            "decision-v2".to_string(),
            false,
        );

        assert_eq!(
            config.declined_tool_decisions,
            BTreeMap::from([("approval-a".to_string(), "decision-v2".to_string())])
        );
        reconcile_tool_target_decision(
            &mut config,
            "approval-a".to_string(),
            "decision-v2".to_string(),
            true,
        );
        assert!(config.declined_tool_decisions.is_empty());
        assert_eq!(
            config.approved_tool_targets,
            BTreeSet::from(["approval-a".to_string()])
        );
    }

    #[tokio::test]
    async fn tool_approval_waits_for_refresh_and_rejects_a_changed_decision() {
        let service = test_service(Vec::new());
        let request = |decision_key: &str, content_version: &str| {
            serde_json::from_value::<ExternalToolApprovalRequest>(serde_json::json!({
                "approvalKey": "approval-a",
                "decisionKey": decision_key,
                "targetId": {
                    "source": { "providerId": "opencode.tools", "sourceId": "project" },
                    "localId": "review.js"
                },
                "sourceDisplayName": "OpenCode project tools",
                "sourceScope": "project",
                "sourceLocation": "/repo/.opencode/tools/review.js",
                "workingDirectory": "/repo",
                "runtimeKind": "java_script",
                "capabilities": ["file_system"],
                "contentVersion": content_version,
                "toolNames": ["review"]
            }))
            .unwrap()
        };
        lock_snapshot(&service.snapshot).tool_approval_requests =
            vec![request("decision-v1", "v1")];

        let refresh_guard = service.refresh_gate.lock().await;
        let decision_service = Arc::clone(&service);
        let decision = tokio::spawn(async move {
            decision_service
                .set_tool_target_decision("approval-a", "decision-v1", true)
                .await
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            service.tool_decision_gate_waiting.notified(),
        )
        .await
        .expect("approval task must reach the refresh gate");
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(50),
                service.tool_decision_gate_acquired.notified(),
            )
            .await
            .is_err(),
            "approval must not enter the decision critical section while refresh owns the gate"
        );

        lock_snapshot(&service.snapshot).tool_approval_requests =
            vec![request("decision-v2", "v2")];
        drop(refresh_guard);
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            service.tool_decision_gate_acquired.notified(),
        )
        .await
        .expect("approval task must enter after the refresh releases the gate");

        let error = decision
            .await
            .unwrap()
            .expect_err("the approval must not apply to the changed content");
        assert_eq!(error, "external tool decision is stale or unknown");
    }

    #[test]
    fn tool_conflict_choices_keep_only_the_current_version_per_lineage() {
        let mut choices = BTreeMap::from([
            (
                "external_tool:local-user:review:old".to_string(),
                "external-a".to_string(),
            ),
            (
                "external_tool:local-user:help:old".to_string(),
                "builtin-help".to_string(),
            ),
        ]);

        reconcile_versioned_tool_conflict_choice(
            &mut choices,
            "external_tool:local-user:review:new".to_string(),
            "external-b".to_string(),
        );

        assert!(!choices.contains_key("external_tool:local-user:review:old"));
        assert_eq!(choices["external_tool:local-user:review:new"], "external-b");
        assert_eq!(choices["external_tool:local-user:help:old"], "builtin-help");
        assert_eq!(choices.len(), 2);
    }

    #[tokio::test]
    async fn slow_provider_is_not_respawned_while_healthy_sibling_updates() {
        let slow_calls = Arc::new(AtomicUsize::new(0));
        let healthy_calls = Arc::new(AtomicUsize::new(0));
        let service = test_service(vec![
            delayed_provider(
                "slow",
                std::time::Duration::from_millis(250),
                Arc::clone(&slow_calls),
            ),
            delayed_provider(
                "healthy",
                std::time::Duration::ZERO,
                Arc::clone(&healthy_calls),
            ),
        ]);

        let requests = lock_coordinator(&service.coordinator).discovery_requests();
        let scheduled = service.prepare_discovery_tasks(requests).await;
        let polled = poll_discovery_tasks(scheduled, std::time::Duration::from_millis(25)).await;
        let results = service.finish_discovery_poll(polled).await;
        let snapshot = lock_coordinator(&service.coordinator).apply_discovery_results(results);
        assert!(snapshot
            .commands
            .iter()
            .any(|command| command.definition.name == "healthy"));

        let requests = lock_coordinator(&service.coordinator).discovery_requests();
        let scheduled = service.prepare_discovery_tasks(requests).await;
        assert!(scheduled
            .iter()
            .any(|(provider_id, _, is_new)| { provider_id.as_str() == "slow" && !is_new }));
        let polled = poll_discovery_tasks(scheduled, std::time::Duration::from_millis(25)).await;
        let results = service.finish_discovery_poll(polled).await;
        let snapshot = lock_coordinator(&service.coordinator).apply_discovery_results(results);

        assert_eq!(slow_calls.load(Ordering::SeqCst), 1);
        assert!(healthy_calls.load(Ordering::SeqCst) >= 2);
        assert!(snapshot
            .commands
            .iter()
            .any(|command| command.definition.name == "healthy"));
    }

    #[tokio::test]
    async fn initial_refresh_waiters_reuse_the_in_flight_result() {
        let service = test_service(Vec::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());

        let background = {
            let service = Arc::clone(&service);
            let snapshot_service = Arc::clone(&service);
            let calls = Arc::clone(&calls);
            let started = Arc::clone(&started);
            let release = Arc::clone(&release);
            tokio::spawn(async move {
                service
                    .ensure_initial_refresh_with(|| async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        started.notify_one();
                        release.notified().await;
                        Ok(snapshot_service.snapshot())
                    })
                    .await
            })
        };

        started.notified().await;
        let catalog_waiter = {
            let service = Arc::clone(&service);
            let snapshot_service = Arc::clone(&service);
            let calls = Arc::clone(&calls);
            tokio::spawn(async move {
                service
                    .ensure_initial_refresh_with(|| async move {
                        calls.fetch_add(100, Ordering::SeqCst);
                        Ok(snapshot_service.snapshot())
                    })
                    .await
            })
        };
        tokio::task::yield_now().await;
        assert!(!catalog_waiter.is_finished());

        release.notify_one();
        background.await.unwrap().unwrap();
        catalog_waiter.await.unwrap().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn failed_initial_refresh_can_be_retried() {
        let service = test_service(Vec::new());
        let first = service
            .ensure_initial_refresh_with(|| async { Err("temporary failure".to_string()) })
            .await;
        assert_eq!(first.unwrap_err(), "temporary failure");

        let calls = Arc::new(AtomicUsize::new(0));
        let snapshot_service = Arc::clone(&service);
        service
            .ensure_initial_refresh_with(|| {
                let calls = Arc::clone(&calls);
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(snapshot_service.snapshot())
                }
            })
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
