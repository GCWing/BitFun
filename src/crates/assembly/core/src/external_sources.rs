//! Product composition and lifecycle service for external AI application sources.
//!
//! Concrete ecosystem providers are selected only in this assembly module. The
//! catalog and product surfaces remain provider- and ecosystem-neutral.

pub use bitfun_product_domains::external_sources::{
    prompt_command_conflict_key, ExpandedPromptCommand, ExternalSourceCatalogEntry,
    ExternalSourceCatalogSnapshot, ExternalSourceDiagnostic, ExternalSourceLifecycleState,
    PromptCommandAvailability, PromptCommandCatalogEntry, PromptCommandDefinition, SourceKey,
};

use bitfun_external_sources::{
    ExternalSourceCoordinator, ExternalSourceDiscoveryRequest, ExternalSourceDiscoveryResult,
};
use bitfun_opencode_adapter::OpenCodeCommandProvider;
use bitfun_product_domains::external_sources::{
    ExecutionDomainId, ExternalSourceContext, PromptCommandSourceProvider,
};
use bitfun_services_core::json_store::JsonFileStore;
use bitfun_services_integrations::file_watch::{FileWatchService, FileWatcherConfig};
use dashmap::{mapref::entry::Entry, DashMap};
use futures::future::{join_all, BoxFuture, Shared};
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
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

struct InFlightDiscovery {
    task: SharedDiscoveryTask,
    wake_scheduled: bool,
}

struct WorkspaceExternalSourceService {
    workspace_root: Option<PathBuf>,
    coordinator: Arc<StdMutex<ExternalSourceCoordinator>>,
    updates: broadcast::Sender<ExternalSourceCatalogSnapshot>,
    watch_states: tokio::sync::Mutex<BTreeMap<(PathBuf, bool), bool>>,
    refresh_gate: tokio::sync::Mutex<()>,
    discovery_tasks: tokio::sync::Mutex<
        BTreeMap<bitfun_product_domains::external_sources::ProviderId, InFlightDiscovery>,
    >,
    initial_refresh_started: AtomicBool,
    keepalive_started: AtomicBool,
    last_access_epoch_seconds: AtomicU64,
    watcher: Arc<FileWatchService>,
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
        let mut coordinator = ExternalSourceCoordinator::new(context, providers)?;
        let preferences = read_external_sources_config().await?;
        coordinator.replace_suppressed_sources(
            preferences.suppressed_source_keys.iter().cloned().collect(),
        );
        coordinator.replace_conflict_choices(preferences.conflict_choices);
        coordinator
            .replace_conflict_lineage_current_keys(preferences.conflict_lineage_current_keys);
        coordinator.replace_conflicted_candidate_ids(preferences.conflicted_candidate_ids);
        let (updates, _) = broadcast::channel(32);
        let service = Arc::new(Self {
            workspace_root,
            coordinator: Arc::new(StdMutex::new(coordinator)),
            updates,
            watch_states: tokio::sync::Mutex::new(BTreeMap::new()),
            refresh_gate: tokio::sync::Mutex::new(()),
            discovery_tasks: tokio::sync::Mutex::new(BTreeMap::new()),
            initial_refresh_started: AtomicBool::new(false),
            keepalive_started: AtomicBool::new(false),
            last_access_epoch_seconds: AtomicU64::new(epoch_seconds()),
            watcher: Arc::new(FileWatchService::new(FileWatcherConfig::default())),
        });
        service.start_watching().await;
        Ok(service)
    }

    async fn refresh(self: &Arc<Self>) -> Result<ExternalSourceCatalogSnapshot, String> {
        self.initial_refresh_started.store(true, Ordering::Release);
        // Preferences are global to the local execution domain and may be
        // changed by another BitFun process. Synchronize before every refresh
        // so a cached CLI/Desktop service cannot keep an externally disabled
        // source active.
        sync_service_preferences(self).await?;
        let _refresh_guard = self.refresh_gate.lock().await;
        let requests = lock_coordinator(&self.coordinator).discovery_requests();
        let scheduled = self.prepare_discovery_tasks(requests).await;
        let polled = poll_discovery_tasks(scheduled, PROVIDER_DISCOVERY_TIMEOUT).await;
        let results = self.finish_discovery_poll(polled).await;
        let snapshot = lock_coordinator(&self.coordinator).apply_discovery_results(results);
        self.ensure_watch_roots().await;
        let _ = self.updates.send(snapshot.clone());
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
        let snapshot = lock_coordinator(&self.coordinator).apply_discovery_result(result);
        self.ensure_watch_roots().await;
        let _ = self.updates.send(snapshot);
    }

    fn ensure_background_refresh(self: &Arc<Self>) {
        if self
            .initial_refresh_started
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
            if let Err(error) = service.refresh().await {
                log::warn!("Initial external source refresh failed: {}", error);
            }
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
                let key = service.workspace_root.clone();
                if let Some(entry) = workspace_services().get(&key) {
                    let should_remove = entry
                        .value()
                        .upgrade()
                        .is_some_and(|cached| Arc::ptr_eq(&cached, &service));
                    drop(entry);
                    if should_remove {
                        workspace_services().remove(&key);
                    }
                }
                break;
            }
        });
    }

    fn snapshot(&self) -> ExternalSourceCatalogSnapshot {
        lock_coordinator(&self.coordinator).snapshot()
    }

    async fn set_source_enabled(
        &self,
        stable_key: &str,
        enabled: bool,
    ) -> Result<ExternalSourceCatalogSnapshot, String> {
        let previous = {
            let mut coordinator = lock_coordinator(&self.coordinator);
            let previous = coordinator.suppressed_sources().clone();
            coordinator.set_source_enabled(stable_key, enabled)?;
            previous
        };
        let updated = lock_coordinator(&self.coordinator)
            .suppressed_sources()
            .clone();
        let authoritative = match persist_source_enabled_change(stable_key, enabled).await {
            Ok(authoritative) => authoritative,
            Err(error) => {
                lock_coordinator(&self.coordinator).replace_suppressed_sources(previous);
                return Err(error);
            }
        };
        if authoritative != updated {
            log::debug!("External source suppression preferences changed in another workspace");
        }
        propagate_suppressed_sources(&authoritative);
        Ok(self.snapshot())
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
        Ok(self.snapshot())
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
        self.refresh().await?;
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
        let watch_roots = lock_coordinator(&self.coordinator).watch_roots();
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
                let watch_roots = lock_coordinator(&service.coordinator).watch_roots();
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
        let watch_roots = lock_coordinator(&self.coordinator).watch_roots();
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
}

enum DiscoveryPoll {
    Complete(ExternalSourceDiscoveryResult),
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

static WORKSPACE_SERVICES: OnceLock<
    DashMap<Option<PathBuf>, Weak<WorkspaceExternalSourceService>>,
> = OnceLock::new();

fn workspace_services() -> &'static DashMap<Option<PathBuf>, Weak<WorkspaceExternalSourceService>> {
    WORKSPACE_SERVICES.get_or_init(DashMap::new)
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

fn propagate_suppressed_sources(sources: &BTreeSet<String>) {
    for service in workspace_services().iter() {
        let Some(service) = service.value().upgrade() else {
            continue;
        };
        let snapshot = {
            let mut coordinator = lock_coordinator(&service.coordinator);
            coordinator.replace_suppressed_sources(sources.clone());
            coordinator.snapshot()
        };
        let _ = service.updates.send(snapshot);
    }
}

fn propagate_conflict_preferences(preferences: &ExternalSourcesConfig) {
    for service in workspace_services().iter() {
        let Some(service) = service.value().upgrade() else {
            continue;
        };
        let snapshot = {
            let mut coordinator = lock_coordinator(&service.coordinator);
            coordinator.replace_conflict_choices(preferences.conflict_choices.clone());
            coordinator.replace_conflict_lineage_current_keys(
                preferences.conflict_lineage_current_keys.clone(),
            );
            coordinator
                .replace_conflicted_candidate_ids(preferences.conflicted_candidate_ids.clone());
            coordinator.snapshot()
        };
        let _ = service.updates.send(snapshot);
    }
}

async fn sync_service_preferences(service: &WorkspaceExternalSourceService) -> Result<(), String> {
    let preferences = read_external_sources_config().await?;
    let suppressed_sources = preferences
        .suppressed_source_keys
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let (changed, snapshot) = {
        let mut coordinator = lock_coordinator(&service.coordinator);
        let mut changed = false;
        if coordinator.suppressed_sources() != &suppressed_sources {
            coordinator.replace_suppressed_sources(suppressed_sources);
            changed = true;
        }
        if coordinator.conflict_choices() != &preferences.conflict_choices {
            coordinator.replace_conflict_choices(preferences.conflict_choices.clone());
            changed = true;
        }
        if coordinator.conflict_lineage_current_keys() != &preferences.conflict_lineage_current_keys
        {
            coordinator
                .replace_conflict_lineage_current_keys(preferences.conflict_lineage_current_keys);
            changed = true;
        }
        if coordinator.conflicted_candidate_ids() != &preferences.conflicted_candidate_ids {
            coordinator.replace_conflicted_candidate_ids(preferences.conflicted_candidate_ids);
            changed = true;
        }
        (changed, coordinator.snapshot())
    };
    if changed {
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
        Arc::new(WorkspaceExternalSourceService {
            workspace_root: None,
            coordinator: Arc::new(StdMutex::new(
                ExternalSourceCoordinator::new(context, providers).unwrap(),
            )),
            updates,
            watch_states: tokio::sync::Mutex::new(BTreeMap::new()),
            refresh_gate: tokio::sync::Mutex::new(()),
            discovery_tasks: tokio::sync::Mutex::new(BTreeMap::new()),
            initial_refresh_started: AtomicBool::new(false),
            keepalive_started: AtomicBool::new(false),
            last_access_epoch_seconds: AtomicU64::new(epoch_seconds()),
            watcher: Arc::new(FileWatchService::new(FileWatcherConfig::default())),
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
}
