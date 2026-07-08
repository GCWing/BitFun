//! Minimal Plugin Runtime Host boundary.

mod adapter;

use async_trait::async_trait;
use bitfun_runtime_ports::{
    validate_plugin_dispatch_response, validate_plugin_runtime_read_response, PluginAuditRef,
    PluginDiagnostic, PluginDiagnosticDetail, PluginDiagnosticSeverity, PluginDispatchEnvelope,
    PluginHostLifecyclePhase, PluginQuarantineClearCondition, PluginQuarantineReason,
    PluginQuarantineScope, PluginQuarantineState, PluginResponseEnvelope,
    PluginRuntimeAvailability, PluginRuntimeClient, PluginRuntimeReadRequest,
    PluginRuntimeReadResponse, PluginRuntimeUnavailableReason, PluginStatusKind,
    PluginStatusSnapshot, PortError, PortErrorKind, PortResult,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use adapter::PluginHostAdapter;

type HostClock = Arc<dyn Fn() -> u64 + Send + Sync>;
const MAX_CACHED_DISPATCHES: usize = 256;

#[derive(Clone)]
pub struct PluginRuntimeHost {
    adapter: Arc<dyn PluginHostAdapter>,
    clock: HostClock,
    dispatch_locks: Arc<Mutex<HashMap<PluginDispatchLockKey, Arc<tokio::sync::Mutex<()>>>>>,
    state: Arc<Mutex<PluginRuntimeHostState>>,
}

#[derive(Default)]
struct PluginRuntimeHostState {
    cached_dispatches: HashMap<DispatchCacheKey, PluginResponseEnvelope>,
    cached_dispatch_order: VecDeque<DispatchCacheKey>,
    disposed_domains: HashSet<ExecutionDomainKey>,
    diagnostics: HashMap<String, PluginDiagnostic>,
    quarantines: HashMap<QuarantineCacheKey, StoredQuarantine>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ExecutionDomainKey {
    project_domain_id: String,
    workspace_id: String,
}

impl ExecutionDomainKey {
    fn new(project_domain_id: impl Into<String>, workspace_id: impl Into<String>) -> Self {
        Self {
            project_domain_id: project_domain_id.into(),
            workspace_id: workspace_id.into(),
        }
    }

    fn from_read_request(request: &PluginRuntimeReadRequest) -> Self {
        Self::new(
            request.project_domain_id.clone(),
            request.workspace_id.clone(),
        )
    }

    fn from_envelope(envelope: &PluginDispatchEnvelope) -> Self {
        Self::new(
            envelope.project_domain_id.clone(),
            envelope.workspace_id.clone(),
        )
    }

    fn matches_parts(&self, project_domain_id: &str, workspace_id: &str) -> bool {
        self.project_domain_id == project_domain_id && self.workspace_id == workspace_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DispatchCacheKey {
    event_id: String,
    event_type: String,
    event_version: String,
    project_domain_id: String,
    workspace_id: String,
    plugin_id: String,
    source: String,
    content_hash: String,
    extension_point_id: String,
    capability_id: String,
    capability_owner: String,
    project_epoch: u64,
    trust_epoch: u64,
    policy_epoch: u64,
    tool_registry_epoch: Option<u64>,
    payload_ref: Option<String>,
    correlation_id: String,
    causation_id: Option<String>,
    idempotency_key: String,
}

impl DispatchCacheKey {
    fn from_envelope(envelope: &PluginDispatchEnvelope) -> Self {
        Self {
            event_id: envelope.event_id.clone(),
            event_type: envelope.event_type.clone(),
            event_version: envelope.event_version.clone(),
            project_domain_id: envelope.project_domain_id.clone(),
            workspace_id: envelope.workspace_id.clone(),
            plugin_id: envelope.source.plugin_id.clone(),
            source: envelope.source.source.clone(),
            content_hash: envelope.source.content_hash.clone(),
            extension_point_id: envelope.extension_point_id.clone(),
            capability_id: envelope.declared_capability.capability_id.clone(),
            capability_owner: format!(
                "{:?}:{}",
                envelope.declared_capability.owner.kind, envelope.declared_capability.owner.id
            ),
            project_epoch: envelope.epochs.project_epoch,
            trust_epoch: envelope.epochs.trust_epoch,
            policy_epoch: envelope.epochs.policy_epoch,
            tool_registry_epoch: envelope.epochs.tool_registry_epoch,
            payload_ref: envelope.payload_ref.as_ref().map(|payload| {
                format!(
                    "{}:{}:{:?}:{:?}:{}",
                    payload.payload_id,
                    payload.schema_version,
                    payload.data_classification,
                    payload.redaction,
                    payload.uri.as_deref().unwrap_or("")
                )
            }),
            correlation_id: envelope.correlation_id.clone(),
            causation_id: envelope.causation_id.clone(),
            idempotency_key: envelope.idempotency_key.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PluginDispatchLockKey {
    project_domain_id: String,
    workspace_id: String,
    plugin_id: String,
}

impl PluginDispatchLockKey {
    fn from_envelope(envelope: &PluginDispatchEnvelope) -> Self {
        Self {
            project_domain_id: envelope.project_domain_id.clone(),
            workspace_id: envelope.workspace_id.clone(),
            plugin_id: envelope.source.plugin_id.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct QuarantineCacheKey {
    event_id: String,
    event_type: String,
    event_version: String,
    project_domain_id: String,
    workspace_id: String,
    plugin_id: String,
    source: String,
    content_hash: String,
    extension_point_id: String,
    project_epoch: u64,
    trust_epoch: u64,
    policy_epoch: u64,
    tool_registry_epoch: Option<u64>,
    correlation_id: String,
    causation_id: Option<String>,
    idempotency_key: String,
}

impl QuarantineCacheKey {
    fn from_envelope(envelope: &PluginDispatchEnvelope) -> Self {
        Self {
            event_id: envelope.event_id.clone(),
            event_type: envelope.event_type.clone(),
            event_version: envelope.event_version.clone(),
            project_domain_id: envelope.project_domain_id.clone(),
            workspace_id: envelope.workspace_id.clone(),
            plugin_id: envelope.source.plugin_id.clone(),
            source: envelope.source.source.clone(),
            content_hash: envelope.source.content_hash.clone(),
            extension_point_id: envelope.extension_point_id.clone(),
            project_epoch: envelope.epochs.project_epoch,
            trust_epoch: envelope.epochs.trust_epoch,
            policy_epoch: envelope.epochs.policy_epoch,
            tool_registry_epoch: envelope.epochs.tool_registry_epoch,
            correlation_id: envelope.correlation_id.clone(),
            causation_id: envelope.causation_id.clone(),
            idempotency_key: envelope.idempotency_key.clone(),
        }
    }
}

#[derive(Clone)]
struct StoredQuarantine {
    state: PluginQuarantineState,
    key: QuarantineCacheKey,
}

impl PluginRuntimeHost {
    pub fn new(adapter: Arc<dyn PluginHostAdapter>) -> Self {
        Self::with_clock(adapter, current_unix_ms)
    }

    fn with_clock<F>(adapter: Arc<dyn PluginHostAdapter>, clock: F) -> Self
    where
        F: Fn() -> u64 + Send + Sync + 'static,
    {
        Self {
            adapter,
            clock: Arc::new(clock),
            dispatch_locks: Arc::new(Mutex::new(HashMap::new())),
            state: Arc::new(Mutex::new(PluginRuntimeHostState::default())),
        }
    }

    pub fn dispose_project(
        &self,
        project_domain_id: impl Into<String>,
        workspace_id: impl Into<String>,
    ) {
        self.state
            .lock()
            .expect("plugin host state poisoned")
            .disposed_domains
            .insert(ExecutionDomainKey::new(project_domain_id, workspace_id));
    }

    pub fn restart(&self, project_domain_id: impl Into<String>, workspace_id: impl Into<String>) {
        let domain = ExecutionDomainKey::new(project_domain_id, workspace_id);
        {
            let mut state = self.state.lock().expect("plugin host state poisoned");
            state
                .cached_dispatches
                .retain(|key, _| !domain.matches_parts(&key.project_domain_id, &key.workspace_id));
            state
                .cached_dispatch_order
                .retain(|key| !domain.matches_parts(&key.project_domain_id, &key.workspace_id));

            let removed_diagnostic_ids = state
                .quarantines
                .values()
                .filter(|stored| {
                    domain.matches_parts(&stored.key.project_domain_id, &stored.key.workspace_id)
                })
                .flat_map(|stored| stored.state.diagnostic_ids.iter().cloned())
                .collect::<HashSet<_>>();
            state.quarantines.retain(|_, stored| {
                !domain.matches_parts(&stored.key.project_domain_id, &stored.key.workspace_id)
            });
            let retained_diagnostic_ids = state
                .quarantines
                .values()
                .flat_map(|stored| stored.state.diagnostic_ids.iter().cloned())
                .collect::<HashSet<_>>();
            for diagnostic_id in removed_diagnostic_ids {
                if !retained_diagnostic_ids.contains(&diagnostic_id) {
                    state.diagnostics.remove(&diagnostic_id);
                }
            }
        }
        self.dispatch_locks
            .lock()
            .expect("plugin host dispatch locks poisoned")
            .retain(|key, lock| {
                !domain.matches_parts(&key.project_domain_id, &key.workspace_id)
                    || Arc::strong_count(lock) > 1
            });
    }

    fn overlay_host_quarantines(
        &self,
        request: &PluginRuntimeReadRequest,
        response: &mut PluginRuntimeReadResponse,
    ) {
        let mut quarantines = {
            let state = self.state.lock().expect("plugin host state poisoned");
            state
                .quarantines
                .values()
                .filter(|stored| {
                    stored.key.project_domain_id == request.project_domain_id
                        && stored.key.workspace_id == request.workspace_id
                        && (request.plugin_ids.is_empty()
                            || request.plugin_ids.contains(&stored.state.source.plugin_id))
                })
                .map(|stored| {
                    let diagnostics = stored
                        .state
                        .diagnostic_ids
                        .iter()
                        .filter_map(|diagnostic_id| state.diagnostics.get(diagnostic_id).cloned())
                        .collect::<Vec<_>>();
                    (stored.state.clone(), diagnostics)
                })
                .collect::<Vec<_>>()
        };
        quarantines.sort_by(|(left, _), (right, _)| left.quarantine_id.cmp(&right.quarantine_id));

        for (quarantine, diagnostics) in quarantines {
            if !response
                .sources
                .iter()
                .any(|source| source == &quarantine.source)
            {
                response.sources.push(quarantine.source.clone());
            }

            if let Some(status) = response
                .plugin_statuses
                .iter_mut()
                .find(|status| status.source == quarantine.source)
            {
                status.status = PluginStatusKind::Quarantined;
                status.availability = PluginRuntimeAvailability::projection_only(
                    PluginRuntimeUnavailableReason::HostUnavailable,
                );
                status.quarantine = Some(quarantine.clone());
                status.updated_at_ms = quarantine.created_at_ms;
                for diagnostic_id in &quarantine.diagnostic_ids {
                    if !status.diagnostic_ids.contains(diagnostic_id) {
                        status.diagnostic_ids.push(diagnostic_id.clone());
                    }
                }
            } else {
                response.plugin_statuses.push(PluginStatusSnapshot {
                    source: quarantine.source.clone(),
                    status: PluginStatusKind::Quarantined,
                    availability: PluginRuntimeAvailability::projection_only(
                        PluginRuntimeUnavailableReason::HostUnavailable,
                    ),
                    config_validation: None,
                    quarantine: Some(quarantine.clone()),
                    diagnostic_ids: quarantine.diagnostic_ids.clone(),
                    updated_at_ms: quarantine.created_at_ms,
                });
            }

            for diagnostic in diagnostics {
                if response
                    .diagnostics
                    .iter()
                    .any(|existing| existing.diagnostic_id == diagnostic.diagnostic_id)
                {
                    continue;
                }
                response.diagnostics.push(diagnostic);
            }
        }
    }

    fn active_quarantine(
        &self,
        envelope: &PluginDispatchEnvelope,
    ) -> Option<PluginQuarantineState> {
        self.state
            .lock()
            .expect("plugin host state poisoned")
            .quarantines
            .values()
            .filter(|stored| {
                stored.key.project_domain_id == envelope.project_domain_id
                    && stored.key.workspace_id == envelope.workspace_id
                    && stored.key.plugin_id == envelope.source.plugin_id
            })
            .map(|stored| stored.state.clone())
            .min_by(|left, right| left.created_at_ms.cmp(&right.created_at_ms))
    }

    fn now(&self) -> u64 {
        (self.clock)()
    }

    fn is_domain_disposed(&self, domain: &ExecutionDomainKey) -> bool {
        self.state
            .lock()
            .expect("plugin host state poisoned")
            .disposed_domains
            .contains(domain)
    }

    fn cached_response(&self, key: &DispatchCacheKey) -> Option<PluginResponseEnvelope> {
        let mut state = self.state.lock().expect("plugin host state poisoned");
        let cached = state.cached_dispatches.get(key).cloned();
        if cached.is_some() {
            state
                .cached_dispatch_order
                .retain(|cached_key| cached_key != key);
            state.cached_dispatch_order.push_back(key.clone());
        }
        cached
    }

    fn dispatch_lock(&self, key: &PluginDispatchLockKey) -> Arc<tokio::sync::Mutex<()>> {
        self.dispatch_locks
            .lock()
            .expect("plugin host dispatch locks poisoned")
            .entry(key.clone())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    fn release_dispatch_lock(
        &self,
        key: &PluginDispatchLockKey,
        lock: &Arc<tokio::sync::Mutex<()>>,
    ) {
        let mut locks = self
            .dispatch_locks
            .lock()
            .expect("plugin host dispatch locks poisoned");
        if Arc::strong_count(lock) == 2 {
            if let Some(stored) = locks.get(key) {
                if Arc::ptr_eq(stored, lock) {
                    locks.remove(key);
                }
            }
        }
    }

    fn cache_response(&self, key: DispatchCacheKey, response: PluginResponseEnvelope) {
        let mut state = self.state.lock().expect("plugin host state poisoned");
        if state.cached_dispatches.contains_key(&key) {
            state
                .cached_dispatch_order
                .retain(|cached_key| cached_key != &key);
        }
        state.cached_dispatch_order.push_back(key.clone());
        state.cached_dispatches.insert(key, response);
        while state.cached_dispatch_order.len() > MAX_CACHED_DISPATCHES {
            if let Some(evicted) = state.cached_dispatch_order.pop_front() {
                state.cached_dispatches.remove(&evicted);
            }
        }
    }

    fn quarantine_response(
        &self,
        envelope: PluginDispatchEnvelope,
        reason: PluginQuarantineReason,
        diagnostic_code: &'static str,
        diagnostic_message: String,
        detail: PluginDiagnosticDetail,
    ) -> PluginResponseEnvelope {
        let audit = audit_ref(&envelope);
        let quarantine_key = QuarantineCacheKey::from_envelope(&envelope);
        let quarantine_id = quarantine_id(&quarantine_key);
        let diagnostic_id = format!("{diagnostic_code}:{quarantine_id}");
        let quarantine = PluginQuarantineState {
            schema_version: 1,
            quarantine_id,
            scope: PluginQuarantineScope::ProjectPlugin {
                project_domain_id: envelope.project_domain_id.clone(),
                workspace_id: envelope.workspace_id.clone(),
                plugin_id: envelope.source.plugin_id.clone(),
            },
            reason,
            source: envelope.source.clone(),
            audit: audit.clone(),
            created_at_ms: self.now(),
            log_ref: None,
            clears_when: vec![PluginQuarantineClearCondition::HostRestarted],
            diagnostic_ids: vec![diagnostic_id.clone()],
        };
        let diagnostic = PluginDiagnostic {
            diagnostic_id,
            severity: PluginDiagnosticSeverity::Error,
            source: envelope.source.clone(),
            code: diagnostic_code.to_string(),
            message: diagnostic_message,
            detail,
            audit,
            retryable: false,
        };
        {
            let mut state = self.state.lock().expect("plugin host state poisoned");
            state
                .diagnostics
                .insert(diagnostic.diagnostic_id.clone(), diagnostic.clone());
            state.quarantines.insert(
                quarantine_key.clone(),
                StoredQuarantine {
                    state: quarantine.clone(),
                    key: quarantine_key,
                },
            );
        }

        PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id,
            project_domain_id: envelope.project_domain_id,
            workspace_id: envelope.workspace_id,
            adapter_id: self.adapter.adapter_id().to_string(),
            plugin_id: Some(envelope.source.plugin_id),
            completed_at_ms: self.now(),
            effects: Vec::new(),
            diagnostics: vec![diagnostic],
            quarantine: Some(quarantine),
            plugin_statuses: Vec::new(),
            observed_epochs: envelope.epochs,
        }
    }

    async fn dispatch_locked(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        let active_quarantine = self.active_quarantine(&envelope);
        if let Err(message) = validate_dispatch_envelope(&envelope) {
            if let Some(quarantine) = active_quarantine {
                return Err(active_quarantine_error(&envelope, &quarantine));
            }
            return Ok(self.quarantine_response(
                envelope,
                PluginQuarantineReason::HostFailure,
                "plugin_host.invalid_dispatch_envelope",
                format!("Plugin host dispatch envelope is invalid: {message}"),
                PluginDiagnosticDetail::HostLifecycle {
                    phase: PluginHostLifecyclePhase::Dispatch,
                },
            ));
        }

        let domain = ExecutionDomainKey::from_envelope(&envelope);
        if self.is_domain_disposed(&domain) {
            return Err(PortError::new(
                PortErrorKind::NotAvailable,
                format!(
                    "plugin runtime host project workspace is disposed: {}/{}",
                    envelope.project_domain_id, envelope.workspace_id
                ),
            ));
        }

        let cache_key = DispatchCacheKey::from_envelope(&envelope);

        if let Some(cached) = self.cached_response(&cache_key) {
            if cached.quarantine.is_some() && cached.effects.is_empty() {
                return Ok(cached);
            }
        }

        if let Some(quarantine) = active_quarantine {
            return Err(active_quarantine_error(&envelope, &quarantine));
        }

        if let Some(cached) = self.cached_response(&cache_key) {
            return Ok(cached);
        }

        if envelope.deadline_ms == 0 {
            let response = self.quarantine_response(
                envelope,
                PluginQuarantineReason::DeadlineExceeded,
                "plugin_host.deadline_exceeded",
                "Plugin host dispatch deadline was already expired".to_string(),
                PluginDiagnosticDetail::Deadline {
                    deadline_ms: 0,
                    elapsed_ms: 0,
                },
            );
            self.cache_response(cache_key, response.clone());
            return Ok(response);
        }

        let deadline_ms = envelope.deadline_ms;
        let adapter_result = tokio::time::timeout(
            Duration::from_millis(deadline_ms),
            self.adapter.dispatch(envelope.clone()),
        )
        .await;
        let response = match adapter_result {
            Ok(Ok(response)) => {
                if let Err(error) = validate_plugin_dispatch_response(
                    &envelope,
                    &response,
                    Some(self.adapter.adapter_id()),
                ) {
                    self.quarantine_response(
                        envelope,
                        PluginQuarantineReason::AdapterFailure,
                        "plugin_host.invalid_response",
                        format!(
                            "Plugin host adapter returned an invalid response: {}",
                            error.message
                        ),
                        PluginDiagnosticDetail::Adapter {
                            adapter_id: self.adapter.adapter_id().to_string(),
                        },
                    )
                } else {
                    response
                }
            }
            Ok(Err(error)) => self.quarantine_response(
                envelope,
                PluginQuarantineReason::AdapterFailure,
                "plugin_host.adapter_failure",
                format!("Plugin host adapter failed: {}", error.message),
                PluginDiagnosticDetail::Adapter {
                    adapter_id: self.adapter.adapter_id().to_string(),
                },
            ),
            Err(_) => self.quarantine_response(
                envelope,
                PluginQuarantineReason::DeadlineExceeded,
                "plugin_host.deadline_exceeded",
                "Plugin host dispatch exceeded its deadline".to_string(),
                PluginDiagnosticDetail::Deadline {
                    deadline_ms,
                    elapsed_ms: deadline_ms,
                },
            ),
        };

        self.cache_response(cache_key, response.clone());
        Ok(response)
    }
}

#[async_trait]
impl PluginRuntimeClient for PluginRuntimeHost {
    fn availability(&self) -> PluginRuntimeAvailability {
        PluginRuntimeAvailability::Available
    }

    async fn read_plugins(
        &self,
        request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        let domain = ExecutionDomainKey::from_read_request(&request);
        if self.is_domain_disposed(&domain) {
            return Err(PortError::new(
                PortErrorKind::NotAvailable,
                format!(
                    "plugin runtime host project workspace is disposed: {}/{}",
                    request.project_domain_id, request.workspace_id
                ),
            ));
        }

        let mut response = self.adapter.read_plugins(request.clone()).await?;
        validate_plugin_runtime_read_response(&request, &response)?;
        if !request.plugin_ids.is_empty() {
            response
                .sources
                .retain(|source| request.plugin_ids.contains(&source.plugin_id));
            response
                .plugin_statuses
                .retain(|status| request.plugin_ids.contains(&status.source.plugin_id));
        }
        response.diagnostics.retain(|diagnostic| {
            response
                .sources
                .iter()
                .any(|source| source == &diagnostic.source)
                || response
                    .plugin_statuses
                    .iter()
                    .any(|status| status.source == diagnostic.source)
        });
        self.overlay_host_quarantines(&request, &mut response);
        validate_plugin_runtime_read_response(&request, &response)?;
        Ok(response)
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        let lock_key = PluginDispatchLockKey::from_envelope(&envelope);
        let dispatch_lock = self.dispatch_lock(&lock_key);
        let result = {
            let _dispatch_guard = dispatch_lock.lock().await;
            self.dispatch_locked(envelope).await
        };
        self.release_dispatch_lock(&lock_key, &dispatch_lock);
        result
    }
}

fn quarantine_id(key: &QuarantineCacheKey) -> String {
    let tool_epoch = key
        .tool_registry_epoch
        .map(|epoch| epoch.to_string())
        .unwrap_or_else(|| "none".to_string());
    let parts = vec![
        key.event_id.clone(),
        key.event_type.clone(),
        key.event_version.clone(),
        key.project_domain_id.clone(),
        key.workspace_id.clone(),
        key.plugin_id.clone(),
        key.source.clone(),
        key.content_hash.clone(),
        key.extension_point_id.clone(),
        key.project_epoch.to_string(),
        key.trust_epoch.to_string(),
        key.policy_epoch.to_string(),
        tool_epoch,
        key.correlation_id.clone(),
        key.causation_id.clone().unwrap_or_default(),
        key.idempotency_key.clone(),
    ];
    let material = parts
        .iter()
        .map(|value| format!("{}:{value}", value.len()))
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "quarantine:{:016x}{:016x}",
        fnv1a64(material.as_bytes(), 0xcbf29ce484222325),
        fnv1a64(material.as_bytes(), 0x84222325cbf29ce4)
    )
}

fn active_quarantine_error(
    envelope: &PluginDispatchEnvelope,
    quarantine: &PluginQuarantineState,
) -> PortError {
    PortError::new(
        PortErrorKind::NotAvailable,
        format!(
            "plugin runtime host has active quarantine {} for plugin {}",
            quarantine.quarantine_id, envelope.source.plugin_id
        ),
    )
}

fn fnv1a64(bytes: &[u8], offset_basis: u64) -> u64 {
    bytes.iter().fold(offset_basis, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn validate_dispatch_envelope(envelope: &PluginDispatchEnvelope) -> Result<(), &'static str> {
    if envelope.event_id.trim().is_empty() {
        return Err("event_id is empty");
    }
    if envelope.event_type.trim().is_empty() {
        return Err("event_type is empty");
    }
    if envelope.event_version.trim().is_empty() {
        return Err("event_version is empty");
    }
    if envelope.project_domain_id.trim().is_empty() {
        return Err("project_domain_id is empty");
    }
    if envelope.workspace_id.trim().is_empty() {
        return Err("workspace_id is empty");
    }
    if envelope.extension_point_id.trim().is_empty() {
        return Err("extension_point_id is empty");
    }
    if envelope.correlation_id.trim().is_empty() {
        return Err("correlation_id is empty");
    }
    if envelope.idempotency_key.trim().is_empty() {
        return Err("idempotency_key is empty");
    }
    if envelope.source.plugin_id.trim().is_empty() {
        return Err("source.plugin_id is empty");
    }
    if envelope.source.source.trim().is_empty() {
        return Err("source.source is empty");
    }
    if envelope.source.content_hash.trim().is_empty() {
        return Err("source.content_hash is empty");
    }
    if envelope.declared_capability.capability_id.trim().is_empty() {
        return Err("declared_capability.capability_id is empty");
    }
    if envelope.declared_capability.owner.id.trim().is_empty() {
        return Err("declared_capability.owner.id is empty");
    }

    Ok(())
}

fn current_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn audit_ref(envelope: &PluginDispatchEnvelope) -> PluginAuditRef {
    PluginAuditRef {
        correlation_id: envelope.correlation_id.clone(),
        event_id: Some(envelope.event_id.clone()),
    }
}
