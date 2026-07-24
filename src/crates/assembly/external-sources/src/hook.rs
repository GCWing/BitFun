use crate::refresh::{CompletedDeferredDiscovery, DiscoveryRequest};
use crate::{DeferredDiscovery, DiscoveryBatch, DiscoveryLane};
use bitfun_product_domains::external_hook_catalog::{
    ExternalHookCatalogSnapshotV1, ExternalHookProviderIdentity, ExternalHookProviderSnapshot,
    ExternalHookSourceProvider, EXTERNAL_HOOK_CATALOG_SCHEMA_V1,
};
use bitfun_product_domains::external_sources::{
    ExternalSourceAssetKind, ExternalSourceContext, ExternalSourceDiagnostic,
    ExternalSourceDiagnosticSeverity, ExternalSourceProviderError, ProviderId,
};
use std::collections::BTreeSet;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

struct HookProviderGeneration {
    provider: Arc<dyn ExternalHookSourceProvider>,
    identity: ExternalHookProviderIdentity,
    initial_result_received: bool,
    discovery_pending: bool,
    last_success: Option<ExternalHookProviderSnapshot>,
    last_error: Option<ExternalSourceProviderError>,
}

struct HookCatalogState {
    context: ExternalSourceContext,
    providers: Vec<HookProviderGeneration>,
    snapshot: ExternalHookCatalogSnapshotV1,
}

/// One provider-neutral static discovery unit. It contains no runtime Host and
/// can only invoke the read-only source-provider contract.
struct ExternalHookDiscoveryRequest {
    provider_id: ProviderId,
    provider: Arc<dyn ExternalHookSourceProvider>,
    context: ExternalSourceContext,
}

impl ExternalHookDiscoveryRequest {
    fn execute(self) -> ExternalHookDiscoveryResult {
        let candidate = self.provider.discover(&self.context);
        ExternalHookDiscoveryResult {
            provider_id: self.provider_id,
            candidate,
        }
    }
}

#[derive(Clone)]
pub struct ExternalHookDiscoveryResult {
    provider_id: ProviderId,
    candidate: Result<ExternalHookProviderSnapshot, ExternalSourceProviderError>,
}

impl ExternalHookDiscoveryResult {
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    fn failed(provider_id: ProviderId, error: ExternalSourceProviderError) -> Self {
        Self {
            provider_id,
            candidate: Err(error),
        }
    }
}

impl DiscoveryRequest for ExternalHookDiscoveryRequest {
    type Result = ExternalHookDiscoveryResult;

    const DIAGNOSTIC_PREFIX: &'static str = "external_hook";
    const PROVIDER_LABEL: &'static str = "Hook";

    fn provider_id(&self) -> ProviderId {
        self.provider_id.clone()
    }

    fn execute(self) -> Self::Result {
        ExternalHookDiscoveryRequest::execute(self)
    }

    fn failed(provider_id: ProviderId, error: ExternalSourceProviderError) -> Self::Result {
        ExternalHookDiscoveryResult::failed(provider_id, error)
    }
}

/// Owns the static Hook catalog generations and reuses the process-wide typed
/// discovery scheduler. Repeated Desktop/TUI refreshes therefore coalesce per
/// provider instead of accumulating blocking filesystem workers.
pub struct ExternalHookCatalogCoordinator {
    state: Mutex<HookCatalogState>,
    lane: DiscoveryLane<ExternalHookDiscoveryRequest>,
}

impl fmt::Debug for ExternalHookCatalogCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = lock(&self.state);
        formatter
            .debug_struct("ExternalHookCatalogCoordinator")
            .field("context", &state.context)
            .field("providers", &state.providers.len())
            .field("discovery_pending", &state.snapshot.discovery_pending)
            .finish()
    }
}

impl ExternalHookCatalogCoordinator {
    pub fn new(
        context: ExternalSourceContext,
        providers: Vec<Arc<dyn ExternalHookSourceProvider>>,
    ) -> Result<Self, String> {
        let mut provider_ids = BTreeSet::new();
        let mut generations = Vec::with_capacity(providers.len());
        for provider in providers {
            let identity = provider.identity();
            identity
                .validate()
                .map_err(|error| format!("invalid external Hook provider: {error}"))?;
            if !provider_ids.insert(identity.provider_id.clone()) {
                return Err(format!(
                    "duplicate external Hook provider id: {}",
                    identity.provider_id
                ));
            }
            generations.push(HookProviderGeneration {
                provider,
                identity,
                initial_result_received: false,
                discovery_pending: false,
                last_success: None,
                last_error: None,
            });
        }
        let mut snapshot = ExternalHookCatalogSnapshotV1::default();
        snapshot.discovery_pending = !generations.is_empty();
        snapshot.providers = generations
            .iter()
            .map(|provider| provider.identity.clone())
            .collect();
        Ok(Self {
            state: Mutex::new(HookCatalogState {
                context,
                providers: generations,
                snapshot,
            }),
            lane: DiscoveryLane::new(),
        })
    }

    pub fn snapshot(&self) -> ExternalHookCatalogSnapshotV1 {
        lock(&self.state).snapshot.clone()
    }

    pub async fn discover(&self, timeout: Duration) -> DiscoveryBatch<ExternalHookDiscoveryResult> {
        let requests = {
            let mut state = lock(&self.state);
            for provider in &mut state.providers {
                provider.discovery_pending = true;
            }
            rebuild_snapshot(&mut state);
            state
                .providers
                .iter()
                .map(|provider| ExternalHookDiscoveryRequest {
                    provider_id: provider.identity.provider_id.clone(),
                    provider: Arc::clone(&provider.provider),
                    context: state.context.clone(),
                })
                .collect()
        };
        self.lane.discover(requests, timeout).await
    }

    pub fn apply_discovery_results(
        &self,
        results: Vec<ExternalHookDiscoveryResult>,
    ) -> ExternalHookCatalogSnapshotV1 {
        let mut state = lock(&self.state);
        for result in results {
            if let Some(provider) = state
                .providers
                .iter_mut()
                .find(|provider| provider.identity.provider_id == result.provider_id)
            {
                apply_provider_candidate(provider, result.candidate);
            }
        }
        rebuild_snapshot(&mut state)
    }

    pub fn apply_discovery_result(
        &self,
        result: ExternalHookDiscoveryResult,
    ) -> ExternalHookCatalogSnapshotV1 {
        self.apply_discovery_results(vec![result])
    }

    pub async fn complete_deferred(
        &self,
        deferred: DeferredDiscovery<ExternalHookDiscoveryResult>,
    ) -> Option<(
        CompletedDeferredDiscovery<ExternalHookDiscoveryResult>,
        Option<DeferredDiscovery<ExternalHookDiscoveryResult>>,
    )> {
        self.lane.complete_deferred(deferred).await
    }

    pub async fn has_in_flight(&self) -> bool {
        self.lane.has_in_flight().await
    }

    pub async fn finalize_deferred(
        &self,
        completed: CompletedDeferredDiscovery<ExternalHookDiscoveryResult>,
    ) -> Option<ExternalHookDiscoveryResult> {
        self.lane.finalize_deferred(completed).await
    }

    pub async fn resume_abandoned(
        &self,
        deferred: DeferredDiscovery<ExternalHookDiscoveryResult>,
    ) -> Option<DeferredDiscovery<ExternalHookDiscoveryResult>> {
        self.lane.resume_abandoned(deferred).await
    }
}

fn rebuild_snapshot(state: &mut HookCatalogState) -> ExternalHookCatalogSnapshotV1 {
    let mut sources = Vec::new();
    let mut entries = Vec::new();
    let mut stale_provider_ids = Vec::new();
    let mut failed_provider_ids = Vec::new();
    let mut diagnostics = Vec::new();
    let mut entry_keys = BTreeSet::new();
    for provider in &state.providers {
        if let Some(snapshot) = &provider.last_success {
            sources.extend(snapshot.sources.clone());
            diagnostics.extend(snapshot.diagnostics.clone());
            for entry in &snapshot.entries {
                if entry_keys.insert(entry.stable_key.clone()) {
                    entries.push(entry.clone());
                } else {
                    diagnostics.push(hook_diagnostic(
                        ExternalSourceDiagnosticSeverity::Error,
                        "external_hook.duplicate_entry_key",
                        "Duplicate external Hook entry identity was omitted",
                    ));
                }
            }
        }
        if let Some(error) = &provider.last_error {
            if provider.last_success.is_some() {
                stale_provider_ids.push(provider.identity.provider_id.clone());
            } else {
                failed_provider_ids.push(provider.identity.provider_id.clone());
            }
            diagnostics.push(hook_diagnostic(
                if error.transient {
                    ExternalSourceDiagnosticSeverity::Warning
                } else {
                    ExternalSourceDiagnosticSeverity::Error
                },
                &error.code,
                &error.message,
            ));
        }
    }
    diagnostics
        .sort_by(|left, right| (&left.code, &left.message).cmp(&(&right.code, &right.message)));
    state.snapshot = ExternalHookCatalogSnapshotV1 {
        schema_version: EXTERNAL_HOOK_CATALOG_SCHEMA_V1,
        discovery_pending: state
            .providers
            .iter()
            .any(|provider| provider.discovery_pending || !provider.initial_result_received),
        providers: state
            .providers
            .iter()
            .map(|provider| provider.identity.clone())
            .collect(),
        sources,
        entries,
        stale_provider_ids,
        failed_provider_ids,
        diagnostics,
    };
    state.snapshot.clone()
}

fn apply_provider_candidate(
    generation: &mut HookProviderGeneration,
    candidate: Result<ExternalHookProviderSnapshot, ExternalSourceProviderError>,
) {
    if let Err(error) = &candidate {
        if matches!(
            error.code.as_str(),
            "external_hook.discovery_timeout" | "external_hook.discovery_in_progress"
        ) {
            generation.discovery_pending = true;
            return;
        }
    }
    generation.discovery_pending = false;
    generation.initial_result_received = true;
    match candidate {
        Ok(snapshot) => match snapshot.validate() {
            Ok(()) if snapshot.provider == generation.identity => {
                generation.last_success = Some(snapshot);
                generation.last_error = None;
            }
            Ok(()) => {
                generation.last_error = Some(ExternalSourceProviderError::new(
                    "external_hook.provider_identity_mismatch",
                    "Hook provider returned a mismatched identity",
                    false,
                ));
            }
            Err(error) => {
                generation.last_error = Some(ExternalSourceProviderError::new(
                    "external_hook.snapshot_invalid",
                    error.to_string(),
                    false,
                ));
            }
        },
        Err(error) => generation.last_error = Some(error),
    }
}

fn hook_diagnostic(
    severity: ExternalSourceDiagnosticSeverity,
    code: &str,
    message: &str,
) -> ExternalSourceDiagnostic {
    ExternalSourceDiagnostic {
        severity,
        asset_kind: ExternalSourceAssetKind::Hook,
        code: code.to_string(),
        message: message.to_string(),
        source: None,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::error!("External Hook catalog coordinator mutex poisoned");
            poisoned.into_inner()
        }
    }
}
