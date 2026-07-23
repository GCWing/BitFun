//! Core product-full runtime adapter boundary.
//!
//! Product runtime assembly facts live in `bitfun-product-capabilities`. Core
//! keeps only compatibility exports and adapter wiring that still depends on
//! existing concrete core paths.

mod runtime_services;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bitfun_agent_runtime::permission::PermissionRequestManager;
use bitfun_agent_runtime::sdk::{
    AgentEventSource, AgentRuntime, AgentSessionForkAtTurnRequest, AgentSessionForkPort,
    AgentSessionForkRequest, AgentSessionForkResult, AgentSessionUsagePort,
    AgentSessionUsageRequest, AgentTurnSettlementPort, AgentTurnSettlementRequest,
};
use bitfun_harness::HarnessRegistry;
use bitfun_runtime_ports::{
    ClockPort, LocalWorkspaceSnapshotPort, LocalWorkspaceSnapshotSessionRequest,
    LocalWorkspaceSnapshotStats, LocalWorkspaceSnapshotTurnRequest, PortError, PortErrorKind,
    PortResult, RuntimeServiceCapability, RuntimeServicePort, SessionStoragePathRequest,
    SessionStorePort, SessionViewRestoreTiming,
};
use bitfun_runtime_services::RuntimeServices;
use bitfun_services_core::permission_store::ProjectPermissionSqliteStore;

use crate::agentic::coordination::{
    ConversationCoordinator, DialogScheduler, SessionMaintenancePermit,
};
use crate::agentic::core::Session;
use crate::agentic::keyed_lock::KeyedAsyncLockGuard;
use crate::agentic::persistence::session_branch::SessionBranchRequest;
use crate::agentic::persistence::{PersistenceManager, SessionMetadataPage};
use crate::agentic::session::CoreSessionStorePort;
use crate::service::session::{DialogTurnData, SessionMetadata};
use crate::service::session_usage::{generate_session_usage_report, SessionUsageReport};
use crate::service::snapshot::{
    get_snapshot_manager_for_workspace, initialize_snapshot_manager_for_workspace, SnapshotError,
    SnapshotManager,
};
use crate::service::token_usage::TokenUsageService;
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};

pub use bitfun_product_capabilities::ProductRuntimeAssembly as CoreProductRuntimeAssembly;
pub use runtime_services::CoreRuntimeServicesProvider;

#[derive(Debug, Clone, Copy, Default)]
struct SystemPermissionClock;

impl RuntimeServicePort for SystemPermissionClock {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Clock
    }
}

impl ClockPort for SystemPermissionClock {
    fn now_unix_millis(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or_default()
    }
}

static PERMISSION_REQUEST_MANAGER: OnceLock<Arc<PermissionRequestManager>> = OnceLock::new();

/// Returns the process-shared permission request owner used by product
/// surfaces. Pending requests remain process-local; only remembered grants and
/// audit facts are written to the user data directory.
pub fn core_permission_request_manager() -> Result<Arc<PermissionRequestManager>, String> {
    if let Some(manager) = PERMISSION_REQUEST_MANAGER.get() {
        return Ok(manager.clone());
    }

    let path_manager = crate::infrastructure::PathManager::new()
        .map_err(|error| format!("Failed to initialize permission path manager: {error}"))?;
    let store = Arc::new(ProjectPermissionSqliteStore::new(
        path_manager.user_data_dir().join("permissions"),
    ));
    let audit_store: Arc<dyn bitfun_runtime_ports::PermissionAuditStorePort> = store.clone();
    let reply_store: Arc<dyn bitfun_runtime_ports::PermissionReplyStorePort> = store.clone();
    let grant_store: Arc<dyn bitfun_runtime_ports::PermissionGrantStorePort> = store;
    let manager = Arc::new(
        PermissionRequestManager::new(audit_store, reply_store, Arc::new(SystemPermissionClock))
            .with_grant_store(grant_store),
    );
    let _ = PERMISSION_REQUEST_MANAGER.set(manager);
    PERMISSION_REQUEST_MANAGER
        .get()
        .cloned()
        .ok_or_else(|| "Failed to initialize shared permission request manager".to_string())
}

/// Serializes one compatibility mutation with Core's session lifecycle.
pub struct CoreSessionMutationPermit {
    _guard: KeyedAsyncLockGuard,
    session_id: String,
    storage_path: PathBuf,
}

/// Holds Core's scheduler boundary while a product compatibility operation
/// mutates session state that must not overlap turn dispatch.
pub struct CoreSessionMaintenancePermit {
    _permit: SessionMaintenancePermit,
}

fn validate_persisted_session_id(session_id: &str) -> BitFunResult<()> {
    bitfun_core_types::validate_session_id(session_id).map_err(BitFunError::Validation)
}

fn latest_persisted_turn_id(turns: &[DialogTurnData]) -> BitFunResult<String> {
    turns
        .last()
        .map(|turn| turn.turn_id.clone())
        .ok_or_else(|| {
            BitFunError::Validation("Session has no persisted turns to fork".to_string())
        })
}

async fn generate_core_session_usage_report(
    persistence: &PersistenceManager,
    token_usage_service: &TokenUsageService,
    request: AgentSessionUsageRequest,
) -> BitFunResult<SessionUsageReport> {
    validate_persisted_session_id(&request.session_id)?;
    generate_session_usage_report(persistence, Some(token_usage_service), request).await
}

fn snapshot_port_error(error: SnapshotError) -> PortError {
    let kind = match &error {
        SnapshotError::SnapshotNotFound(_)
        | SnapshotError::SessionNotFound(_)
        | SnapshotError::OperationNotFound(_)
        | SnapshotError::FileNotFound(_) => PortErrorKind::NotFound,
        SnapshotError::Io(_)
        | SnapshotError::Serialization(_)
        | SnapshotError::GitIsolationFailure(_)
        | SnapshotError::ConfigError(_)
        | SnapshotError::ToolExecution(_) => PortErrorKind::Backend,
    };
    PortError::new(kind, error.to_string())
}

fn snapshot_initialization_port_error(error: SnapshotError) -> PortError {
    PortError::new(PortErrorKind::NotAvailable, error.to_string())
}

fn validate_local_snapshot_workspace(workspace_path: &Path) -> PortResult<()> {
    if workspace_path.as_os_str().is_empty() {
        return Err(PortError::new(
            PortErrorKind::InvalidRequest,
            "workspace path is required",
        ));
    }
    if !workspace_path.is_dir() {
        return Err(PortError::new(
            PortErrorKind::InvalidRequest,
            format!(
                "Workspace directory does not exist: {}",
                workspace_path.display()
            ),
        ));
    }
    Ok(())
}

async fn ensure_local_snapshot_manager(workspace_path: &Path) -> PortResult<Arc<SnapshotManager>> {
    validate_local_snapshot_workspace(workspace_path)?;
    if let Some(manager) = get_snapshot_manager_for_workspace(workspace_path) {
        return Ok(manager);
    }
    initialize_snapshot_manager_for_workspace(workspace_path.to_path_buf(), None)
        .await
        .map_err(snapshot_initialization_port_error)?;
    get_snapshot_manager_for_workspace(workspace_path).ok_or_else(|| {
        PortError::new(
            PortErrorKind::Backend,
            format!(
                "Snapshot manager is unavailable for workspace {}",
                workspace_path.display()
            ),
        )
    })
}

/// Core-backed access to the existing local workspace snapshot owner.
///
/// The returned port is intentionally separate from the Agent Runtime SDK and
/// does not accept remote workspace identity.
pub struct CoreLocalWorkspaceSnapshot;

impl CoreLocalWorkspaceSnapshot {
    pub fn build() -> Arc<dyn LocalWorkspaceSnapshotPort> {
        Arc::new(Self)
    }
}

#[async_trait::async_trait]
impl LocalWorkspaceSnapshotPort for CoreLocalWorkspaceSnapshot {
    async fn prepare_local_workspace(&self, workspace_path: PathBuf) -> PortResult<()> {
        ensure_local_snapshot_manager(&workspace_path).await?;
        Ok(())
    }

    async fn get_session_files(
        &self,
        request: LocalWorkspaceSnapshotSessionRequest,
    ) -> PortResult<Vec<PathBuf>> {
        validate_persisted_session_id(&request.session_id).map_err(runtime_port_error)?;
        ensure_local_snapshot_manager(&request.workspace_path)
            .await?
            .get_session_files(&request.session_id)
            .await
            .map_err(snapshot_port_error)
    }

    async fn get_session_stats(
        &self,
        request: LocalWorkspaceSnapshotSessionRequest,
    ) -> PortResult<LocalWorkspaceSnapshotStats> {
        validate_persisted_session_id(&request.session_id).map_err(runtime_port_error)?;
        let stats = ensure_local_snapshot_manager(&request.workspace_path)
            .await?
            .get_session_stats_fact(&request.session_id)
            .await
            .map_err(snapshot_port_error)?;
        Ok(LocalWorkspaceSnapshotStats {
            session_id: stats.session_id,
            total_files: stats.total_files,
            total_turns: stats.total_turns,
            total_changes: stats.total_changes,
        })
    }

    async fn rollback_workspace_files_to_turn(
        &self,
        request: LocalWorkspaceSnapshotTurnRequest,
    ) -> PortResult<Vec<PathBuf>> {
        validate_persisted_session_id(&request.session_id).map_err(runtime_port_error)?;
        ensure_local_snapshot_manager(&request.workspace_path)
            .await?
            .rollback_to_turn(&request.session_id, request.turn_index)
            .await
            .map_err(snapshot_port_error)
    }
}

/// Product-assembly entry for the public Agent Runtime SDK.
///
/// Concrete coordinator and scheduler ownership remains in Core. Product
/// surfaces receive only the SDK runtime assembled from validated services and
/// harnesses; plugin-host bindings are deliberately not part of this API.
pub struct CoreProductAgentRuntime;

impl CoreProductAgentRuntime {
    /// Build a narrow session and interaction facade for an existing product
    /// owner. This does not assemble runtime services, harnesses, events, or a
    /// complete delivery profile.
    pub fn build_session_surface(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
        token_usage_service: Arc<TokenUsageService>,
    ) -> Result<AgentRuntime, String> {
        let session_operations = Arc::new(CoreSessionOperationsPort::new(
            coordinator.clone(),
            token_usage_service,
        ));
        CoreServiceAgentRuntime::session_surface_agent_runtime(
            coordinator,
            scheduler,
            session_operations.clone(),
            session_operations,
        )
    }

    pub fn build(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
        token_usage_service: Arc<TokenUsageService>,
        services: RuntimeServices,
        harness_registry: HarnessRegistry,
    ) -> Result<AgentRuntime, String> {
        let session_operations = Arc::new(CoreSessionOperationsPort::new(
            coordinator.clone(),
            token_usage_service,
        ));
        CoreServiceAgentRuntime::product_agent_runtime(
            coordinator,
            scheduler,
            session_operations.clone(),
            session_operations.clone(),
            session_operations,
            services,
            harness_registry,
        )
    }

    /// Build the ACP surface with its protocol requirement that a session
    /// rejects a second prompt while another turn is active.
    pub fn build_acp(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
        event_source: AgentEventSource,
        services: RuntimeServices,
        harness_registry: HarnessRegistry,
    ) -> Result<AgentRuntime, String> {
        CoreServiceAgentRuntime::acp_product_agent_runtime(
            coordinator,
            scheduler,
            event_source,
            services,
            harness_registry,
        )
    }
}

/// Core-owned compatibility boundary for product operations not yet exposed by
/// the public Agent Runtime SDK.
///
/// This facade does not own execution. It delegates to the same coordinator,
/// session manager, persistence manager, and user-input channels used by Core.
#[derive(Clone)]
pub struct CoreAgentRuntimeCompatibility {
    coordinator: Arc<ConversationCoordinator>,
    scheduler: Arc<DialogScheduler>,
    persistence: Arc<PersistenceManager>,
}

impl CoreAgentRuntimeCompatibility {
    pub fn build(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
    ) -> Self {
        let persistence = coordinator.get_session_manager().persistence_manager();

        Self {
            coordinator,
            scheduler,
            persistence,
        }
    }

    pub async fn restore_session_from_storage_path(
        &self,
        storage_path: &Path,
        session_id: &str,
        include_internal: bool,
    ) -> BitFunResult<Session> {
        validate_persisted_session_id(session_id)?;
        if include_internal {
            self.coordinator
                .restore_internal_session_from_storage_path(storage_path, session_id)
                .await
        } else {
            self.coordinator
                .restore_session_from_storage_path(storage_path, session_id)
                .await
        }
    }

    pub async fn restore_session_view_from_storage_path(
        &self,
        storage_path: &Path,
        session_id: &str,
        include_internal: bool,
        tail_turn_count: Option<usize>,
    ) -> BitFunResult<(
        Session,
        Vec<DialogTurnData>,
        usize,
        SessionViewRestoreTiming,
    )> {
        validate_persisted_session_id(session_id)?;
        if let Some(tail_turn_count) = tail_turn_count {
            if include_internal {
                self.coordinator
                    .restore_internal_session_view_from_storage_path_tail_timed(
                        storage_path,
                        session_id,
                        tail_turn_count,
                    )
                    .await
            } else {
                self.coordinator
                    .restore_session_view_from_storage_path_tail_timed(
                        storage_path,
                        session_id,
                        tail_turn_count,
                    )
                    .await
            }
        } else {
            let (session, turns, timing) = if include_internal {
                self.coordinator
                    .restore_internal_session_view_from_storage_path_timed(storage_path, session_id)
                    .await?
            } else {
                self.coordinator
                    .restore_session_view_from_storage_path_timed(storage_path, session_id)
                    .await?
            };
            let total_turn_count = turns.len();
            Ok((session, turns, total_turn_count, timing))
        }
    }

    pub async fn restore_session_with_turns_from_storage_path(
        &self,
        storage_path: &Path,
        session_id: &str,
        include_internal: bool,
    ) -> BitFunResult<(Session, Vec<DialogTurnData>)> {
        validate_persisted_session_id(session_id)?;
        if include_internal {
            self.coordinator
                .restore_internal_session_with_turns_from_storage_path(storage_path, session_id)
                .await
        } else {
            self.coordinator
                .restore_session_with_turns_from_storage_path(storage_path, session_id)
                .await
        }
    }

    pub async fn restore_session_view_for_workspace(
        &self,
        request: SessionStoragePathRequest,
        session_id: &str,
        include_internal: bool,
        tail_turn_count: Option<usize>,
    ) -> BitFunResult<(
        Session,
        Vec<DialogTurnData>,
        usize,
        SessionViewRestoreTiming,
    )> {
        validate_persisted_session_id(session_id)?;
        if let Some(tail_turn_count) = tail_turn_count {
            let storage_path = self.resolve_persisted_session_storage_path(request).await?;
            if include_internal {
                self.coordinator
                    .restore_internal_session_view_from_storage_path_tail_timed(
                        &storage_path,
                        session_id,
                        tail_turn_count,
                    )
                    .await
            } else {
                self.coordinator
                    .restore_session_view_from_storage_path_tail_timed(
                        &storage_path,
                        session_id,
                        tail_turn_count,
                    )
                    .await
            }
        } else {
            let (session, turns, timing) = if include_internal {
                self.coordinator
                    .restore_internal_session_view_for_workspace_timed(request, session_id)
                    .await?
            } else {
                self.coordinator
                    .restore_session_view_for_workspace_timed(request, session_id)
                    .await?
            };
            let total_turn_count = turns.len();
            Ok((session, turns, total_turn_count, timing))
        }
    }

    pub async fn restore_session_with_turns_for_workspace(
        &self,
        request: SessionStoragePathRequest,
        session_id: &str,
        include_internal: bool,
    ) -> BitFunResult<(Session, Vec<DialogTurnData>)> {
        validate_persisted_session_id(session_id)?;
        if include_internal {
            self.coordinator
                .restore_internal_session_with_turns_for_workspace(request, session_id)
                .await
        } else {
            self.coordinator
                .restore_session_with_turns_for_workspace(request, session_id)
                .await
        }
    }

    pub async fn list_persisted_sessions(
        &self,
        workspace_path: &Path,
    ) -> BitFunResult<Vec<SessionMetadata>> {
        self.persistence.list_session_metadata(workspace_path).await
    }

    pub async fn list_persisted_sessions_page(
        &self,
        workspace_path: &Path,
        cursor: Option<&str>,
        limit: usize,
    ) -> BitFunResult<SessionMetadataPage> {
        self.persistence
            .list_session_metadata_page(workspace_path, cursor, limit)
            .await
    }

    pub async fn load_persisted_session_metadata(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Option<SessionMetadata>> {
        validate_persisted_session_id(session_id)?;
        self.persistence
            .load_session_metadata(workspace_path, session_id)
            .await
    }

    pub async fn update_persisted_session_metadata(
        &self,
        workspace_path: &Path,
        session_id: &str,
        update: impl FnOnce(&mut SessionMetadata),
    ) -> BitFunResult<()> {
        validate_persisted_session_id(session_id)?;
        self.persistence
            .update_session_metadata(workspace_path, session_id, update)
            .await
    }

    pub fn is_session_loaded_in_memory(&self, session_id: &str) -> BitFunResult<bool> {
        validate_persisted_session_id(session_id)?;
        Ok(self
            .coordinator
            .get_session_manager()
            .get_session(session_id)
            .is_some())
    }

    pub async fn update_loaded_session_title(
        &self,
        session_id: &str,
        title: &str,
    ) -> BitFunResult<String> {
        validate_persisted_session_id(session_id)?;
        self.coordinator
            .update_session_title(session_id, title)
            .await
    }

    pub async fn resolve_persisted_session_storage_path(
        &self,
        request: SessionStoragePathRequest,
    ) -> BitFunResult<PathBuf> {
        CoreSessionStorePort::with_path_manager(self.persistence.path_manager().clone())
            .resolve_session_storage_path(request)
            .await
            .map(|resolution| resolution.effective_storage_path)
            .map_err(|error| BitFunError::Session(error.to_string()))
    }

    pub fn is_session_loaded_from_storage_path(
        &self,
        storage_path: &Path,
        session_id: &str,
    ) -> BitFunResult<bool> {
        validate_persisted_session_id(session_id)?;
        self.coordinator
            .get_session_manager()
            .is_session_loaded_from_storage_path(storage_path, session_id)
    }

    pub async fn ensure_session_loaded_from_storage_path(
        &self,
        storage_path: &Path,
        session_id: &str,
        include_internal: bool,
    ) -> BitFunResult<()> {
        if self.is_session_loaded_from_storage_path(storage_path, session_id)? {
            return Ok(());
        }
        if include_internal {
            self.coordinator
                .restore_internal_session_from_storage_path(storage_path, session_id)
                .await?;
        } else {
            self.coordinator
                .restore_session_from_storage_path(storage_path, session_id)
                .await?;
        }
        Ok(())
    }

    pub async fn begin_persisted_session_mutation(
        &self,
        storage_path: &Path,
        session_id: &str,
    ) -> BitFunResult<CoreSessionMutationPermit> {
        validate_persisted_session_id(session_id)?;
        let session_manager = self.coordinator.get_session_manager();
        let guard = session_manager.acquire_session_mutation(session_id).await?;
        session_manager.validate_session_storage_path_binding(session_id, storage_path)?;
        Ok(CoreSessionMutationPermit {
            _guard: guard,
            session_id: session_id.to_string(),
            storage_path: storage_path.to_path_buf(),
        })
    }

    pub async fn begin_session_maintenance(
        &self,
        storage_path: &Path,
        session_id: &str,
        wait_timeout_ms: u64,
    ) -> BitFunResult<CoreSessionMaintenancePermit> {
        let permit = self
            .scheduler
            .begin_session_maintenance(
                session_id,
                storage_path,
                std::time::Duration::from_millis(wait_timeout_ms),
            )
            .await?;
        Ok(CoreSessionMaintenancePermit { _permit: permit })
    }

    /// Compatibility-only lifecycle operation for ACP setup compensation and
    /// session/close. It releases loaded Core state but preserves persistence
    /// and the storage binding so the same session can be restored later.
    pub async fn unload_persisted_session(&self, session_id: &str) -> BitFunResult<bool> {
        validate_persisted_session_id(session_id)?;
        self.coordinator
            .get_session_manager()
            .unload_session_from_memory(session_id)
            .await
    }

    pub async fn cancel_background_subagents_for_parent(
        &self,
        parent_session_id: &str,
        subagent_session_id: &str,
    ) -> BitFunResult<usize> {
        self.coordinator
            .cancel_background_subagents_for_parent(parent_session_id, subagent_session_id)
            .await
    }

    pub async fn rollback_persisted_session_context_to_turn_start(
        &self,
        permit: &CoreSessionMutationPermit,
        target_turn: usize,
    ) -> BitFunResult<()> {
        self.coordinator
            .get_session_manager()
            .rollback_context_to_turn_start_locked(
                &permit.storage_path,
                &permit.session_id,
                target_turn,
            )
            .await
    }

    pub async fn validate_persisted_session_context_rollback(
        &self,
        permit: &CoreSessionMutationPermit,
        target_turn: usize,
    ) -> BitFunResult<()> {
        self.coordinator
            .get_session_manager()
            .validate_rollback_context_to_turn_start_locked(
                &permit.storage_path,
                &permit.session_id,
                target_turn,
            )
            .await
    }

    pub async fn load_persisted_session_turns(
        &self,
        workspace_path: &Path,
        session_id: &str,
        limit: Option<usize>,
    ) -> BitFunResult<Vec<DialogTurnData>> {
        validate_persisted_session_id(session_id)?;
        if let Some(limit) = limit {
            self.persistence
                .load_recent_turns(workspace_path, session_id, limit)
                .await
        } else {
            self.persistence
                .load_session_turns(workspace_path, session_id)
                .await
        }
    }

    pub async fn touch_persisted_session(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<()> {
        validate_persisted_session_id(session_id)?;
        self.persistence
            .touch_session(workspace_path, session_id)
            .await
    }

    pub async fn save_persisted_dialog_turn(
        &self,
        workspace_path: &Path,
        turn: &DialogTurnData,
    ) -> BitFunResult<()> {
        validate_persisted_session_id(&turn.session_id)?;
        self.persistence
            .save_dialog_turn(workspace_path, turn)
            .await
    }

    pub async fn delete_hidden_subagent_sessions_for_parent_turns(
        &self,
        workspace_path: &Path,
        parent_session_id: &str,
        parent_dialog_turn_ids: &std::collections::HashSet<String>,
    ) -> BitFunResult<Vec<String>> {
        validate_persisted_session_id(parent_session_id)?;
        self.coordinator
            .delete_hidden_subagent_sessions_for_parent_turns(
                workspace_path,
                parent_session_id,
                parent_dialog_turn_ids,
            )
            .await
    }
}

#[derive(Clone)]
struct CoreSessionOperationsPort {
    coordinator: Arc<ConversationCoordinator>,
    persistence: Arc<PersistenceManager>,
    token_usage_service: Arc<TokenUsageService>,
}

impl CoreSessionOperationsPort {
    fn new(
        coordinator: Arc<ConversationCoordinator>,
        token_usage_service: Arc<TokenUsageService>,
    ) -> Self {
        let persistence = coordinator.get_session_manager().persistence_manager();
        Self {
            coordinator,
            persistence,
            token_usage_service,
        }
    }

    async fn resolve_fork_storage_path(
        &self,
        workspace_path: String,
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
    ) -> PortResult<PathBuf> {
        CoreSessionStorePort::with_path_manager(self.persistence.path_manager().clone())
            .resolve_session_storage_path(SessionStoragePathRequest {
                workspace_path: PathBuf::from(workspace_path),
                remote_connection_id,
                remote_ssh_host,
            })
            .await
            .map(|resolution| resolution.effective_storage_path)
    }

    async fn fork_at_persisted_turn(
        &self,
        storage_path: &Path,
        source_session_id: String,
        source_turn_id: String,
    ) -> PortResult<AgentSessionForkResult> {
        if source_turn_id.trim().is_empty() {
            return Err(PortError::new(
                PortErrorKind::InvalidRequest,
                "source turn id is required",
            ));
        }
        let source_session_id_for_coordination = source_session_id.clone();
        let result = self
            .persistence
            .branch_session(
                storage_path,
                &SessionBranchRequest {
                    source_session_id,
                    source_turn_id,
                },
            )
            .await
            .map_err(runtime_port_error)?;
        if let Err(error) = self
            .coordinator
            .initialize_fork_coordination(&source_session_id_for_coordination, &result.session_id)
            .await
        {
            if let Err(cleanup_error) = self
                .persistence
                .delete_session(storage_path, &result.session_id)
                .await
            {
                return Err(PortError::new(
                    PortErrorKind::CleanupRequired,
                    format!(
                        "Session fork coordination initialization failed and rollback did not complete: session_id={}, error={}, cleanup_error={}",
                        result.session_id, error, cleanup_error
                    ),
                ));
            }
            return Err(runtime_port_error(error));
        }
        Ok(AgentSessionForkResult {
            session_id: result.session_id,
            session_name: result.session_name,
            agent_type: result.agent_type,
        })
    }
}

fn runtime_port_error(error: BitFunError) -> PortError {
    let kind = match &error {
        BitFunError::Validation(_) => PortErrorKind::InvalidRequest,
        BitFunError::NotFound(_) => PortErrorKind::NotFound,
        BitFunError::Timeout(_) => PortErrorKind::Timeout,
        BitFunError::Cancelled(_) => PortErrorKind::Cancelled,
        BitFunError::SessionCreateCleanupRequired { .. } => PortErrorKind::CleanupRequired,
        _ => PortErrorKind::Backend,
    };
    PortError::new(kind, error.to_string())
}

fn validate_latest_turn_fork_scope(request: &AgentSessionForkRequest) -> PortResult<()> {
    if request.remote_connection_id.is_some() || request.remote_ssh_host.is_some() {
        return Err(PortError::new(
            PortErrorKind::NotAvailable,
            "Remote session fork is not supported by the local CLI runtime",
        ));
    }
    Ok(())
}

#[async_trait::async_trait]
impl AgentSessionForkPort for CoreSessionOperationsPort {
    async fn fork_session(
        &self,
        request: AgentSessionForkRequest,
    ) -> PortResult<AgentSessionForkResult> {
        validate_latest_turn_fork_scope(&request)?;
        let AgentSessionForkRequest {
            workspace_path,
            source_session_id,
            remote_connection_id,
            remote_ssh_host,
        } = request;
        let storage_path = self
            .resolve_fork_storage_path(workspace_path, remote_connection_id, remote_ssh_host)
            .await?;
        let (_, turns, _) = self
            .coordinator
            .restore_session_view_from_storage_path_timed(&storage_path, &source_session_id)
            .await
            .map_err(runtime_port_error)?;
        let source_turn_id = latest_persisted_turn_id(&turns).map_err(runtime_port_error)?;
        self.fork_at_persisted_turn(&storage_path, source_session_id, source_turn_id)
            .await
    }

    async fn fork_session_at_turn(
        &self,
        request: AgentSessionForkAtTurnRequest,
    ) -> PortResult<AgentSessionForkResult> {
        let storage_path = self
            .resolve_fork_storage_path(
                request.workspace_path,
                request.remote_connection_id,
                request.remote_ssh_host,
            )
            .await?;
        self.fork_at_persisted_turn(
            &storage_path,
            request.source_session_id,
            request.source_turn_id,
        )
        .await
    }
}

#[async_trait::async_trait]
impl AgentSessionUsagePort for CoreSessionOperationsPort {
    async fn generate_session_usage(
        &self,
        request: AgentSessionUsageRequest,
    ) -> PortResult<SessionUsageReport> {
        generate_core_session_usage_report(
            self.persistence.as_ref(),
            self.token_usage_service.as_ref(),
            request,
        )
        .await
        .map_err(runtime_port_error)
    }
}

#[async_trait::async_trait]
impl AgentTurnSettlementPort for CoreSessionOperationsPort {
    async fn wait_for_turn_settlement(
        &self,
        request: AgentTurnSettlementRequest,
    ) -> PortResult<()> {
        if request.wait_timeout_ms == 0 {
            return Err(PortError::new(
                PortErrorKind::InvalidRequest,
                "turn settlement timeout must be greater than zero",
            ));
        }
        self.coordinator
            .wait_for_turn_settlement(
                &request.session_id,
                &request.turn_id,
                Duration::from_millis(request.wait_timeout_ms),
            )
            .await
            .map_err(runtime_port_error)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use bitfun_agent_runtime::sdk::AgentRuntime;
    use bitfun_harness::HarnessRegistry;
    use bitfun_runtime_ports::{
        LocalWorkspaceSnapshotSessionRequest, LocalWorkspaceSnapshotTurnRequest,
    };
    use bitfun_runtime_services::RuntimeServices;
    use uuid::Uuid;

    use super::{
        generate_core_session_usage_report, latest_persisted_turn_id, runtime_port_error,
        validate_latest_turn_fork_scope, validate_persisted_session_id,
        CoreAgentRuntimeCompatibility, CoreLocalWorkspaceSnapshot, CoreProductAgentRuntime,
        CoreSessionOperationsPort,
    };
    use crate::agentic::coordination::{ConversationCoordinator, DialogScheduler};
    use crate::agentic::events::{EventQueue, EventQueueConfig, EventRouter};
    use crate::agentic::execution::{
        ExecutionEngine, ExecutionEngineConfig, RoundExecutor, StreamProcessor,
    };
    use crate::agentic::persistence::PersistenceManager;
    use crate::agentic::session::{
        compression::{CompressionConfig, ContextCompressor},
        PromptCachePolicy, SessionContextStore, SessionManager, SessionManagerConfig,
    };
    use crate::agentic::tools::registry::ToolRegistry;
    use crate::agentic::tools::{ToolPipeline, ToolStateManager};
    use crate::infrastructure::PathManager;
    use crate::service::session::{DialogTurnData, SessionMetadata, UserMessageData};
    use crate::service::session_usage::UsageTokenSource;
    use crate::service::snapshot::manager::clear_snapshot_manager_for_test;
    use crate::service::token_usage::TokenUsageService;
    use crate::service::workspace_runtime::{
        set_workspace_runtime_service_for_current_test, WorkspaceRuntimeService,
    };
    use crate::util::errors::BitFunError;
    use bitfun_agent_runtime::sdk::{
        AgentSessionForkPort, AgentSessionForkRequest, AgentSessionUsageRequest, PortErrorKind,
    };
    use tokio::sync::RwLock as TokioRwLock;

    struct TestWorkspace {
        path: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "bitfun-product-runtime-compatibility-test-{}",
                Uuid::new_v4()
            ));
            std::fs::create_dir_all(&path).expect("test workspace should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn path_manager(&self) -> Arc<PathManager> {
            Arc::new(PathManager::with_user_root_for_tests(
                self.path.join("user-root"),
            ))
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            clear_snapshot_manager_for_test(&self.path);
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn product_agent_runtime_exposes_reviewed_full_and_narrow_builders() {
        fn build(
            coordinator: Arc<ConversationCoordinator>,
            scheduler: Arc<DialogScheduler>,
            token_usage_service: Arc<TokenUsageService>,
            services: RuntimeServices,
            harness_registry: HarnessRegistry,
        ) -> Result<AgentRuntime, String> {
            CoreProductAgentRuntime::build(
                coordinator,
                scheduler,
                token_usage_service,
                services,
                harness_registry,
            )
        }

        let _ = build;
        let _ = CoreProductAgentRuntime::build_session_surface;
        let _ = CoreProductAgentRuntime::build_acp;
    }

    #[test]
    fn sdk_session_operations_depend_directly_on_core_owners() {
        fn build(
            coordinator: Arc<ConversationCoordinator>,
            token_usage_service: Arc<TokenUsageService>,
        ) -> CoreSessionOperationsPort {
            CoreSessionOperationsPort::new(coordinator, token_usage_service)
        }

        let _ = build;
    }

    #[test]
    fn remaining_compatibility_operations_have_one_core_owned_facade() {
        fn build(
            coordinator: Arc<ConversationCoordinator>,
            scheduler: Arc<DialogScheduler>,
        ) -> CoreAgentRuntimeCompatibility {
            CoreAgentRuntimeCompatibility::build(coordinator, scheduler)
        }

        let _ = build;
        let _ = CoreAgentRuntimeCompatibility::list_persisted_sessions;
        let _ = CoreAgentRuntimeCompatibility::load_persisted_session_turns;
        let _ = CoreAgentRuntimeCompatibility::unload_persisted_session;
    }

    #[test]
    fn persisted_session_compatibility_rejects_path_like_ids() {
        let error = validate_persisted_session_id("../../other-project/session")
            .expect_err("compatibility boundary must reject path-like session ids");

        assert!(error.to_string().contains("session_id"), "{error}");
    }

    #[tokio::test]
    async fn local_workspace_snapshot_port_concurrently_prepares_and_returns_typed_empty_facts() {
        let workspace = TestWorkspace::new();
        let _runtime_guard = set_workspace_runtime_service_for_current_test(Arc::new(
            WorkspaceRuntimeService::new(workspace.path_manager()),
        ));
        let port = CoreLocalWorkspaceSnapshot::build();

        let first = port.prepare_local_workspace(workspace.path().to_path_buf());
        let second = port.prepare_local_workspace(workspace.path().to_path_buf());
        let (first, second) = tokio::join!(first, second);
        first.expect("first local snapshot preparation should succeed");
        second.expect("concurrent local snapshot preparation should reuse the owner");

        let request = LocalWorkspaceSnapshotSessionRequest {
            workspace_path: workspace.path().to_path_buf(),
            session_id: "session-empty".to_string(),
        };
        assert!(port
            .get_session_files(request.clone())
            .await
            .expect("session files should be available")
            .is_empty());
        let stats = port
            .get_session_stats(request)
            .await
            .expect("typed stats should be available");
        assert_eq!(stats.session_id, "session-empty");
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_turns, 0);
        assert_eq!(stats.total_changes, 0);
        assert!(port
            .rollback_workspace_files_to_turn(LocalWorkspaceSnapshotTurnRequest {
                workspace_path: workspace.path().to_path_buf(),
                session_id: "session-empty".to_string(),
                turn_index: 0,
            })
            .await
            .expect("an empty session rollback remains a no-op")
            .is_empty());
    }

    #[tokio::test]
    async fn local_workspace_snapshot_port_rejects_non_local_inputs_before_backend_access() {
        let workspace = TestWorkspace::new();
        let port = CoreLocalWorkspaceSnapshot::build();

        let invalid_session = port
            .get_session_files(LocalWorkspaceSnapshotSessionRequest {
                workspace_path: workspace.path().to_path_buf(),
                session_id: "../other-session".to_string(),
            })
            .await
            .expect_err("path-like session ids must be rejected");
        assert_eq!(invalid_session.kind, PortErrorKind::InvalidRequest);

        let missing_workspace = workspace.path().join("missing");
        let invalid_workspace = port
            .prepare_local_workspace(missing_workspace)
            .await
            .expect_err("missing local workspaces must be rejected");
        assert_eq!(invalid_workspace.kind, PortErrorKind::InvalidRequest);
    }

    #[test]
    fn session_create_rollback_residual_remains_typed_across_the_runtime_port() {
        let error = runtime_port_error(BitFunError::SessionCreateCleanupRequired {
            session_id: "session-1".to_string(),
            error: "metadata write failed".to_string(),
            cleanup_error: "session directory is locked".to_string(),
        });

        assert_eq!(error.kind, PortErrorKind::CleanupRequired);
        assert!(error.message.contains("session-1"), "{error}");
    }

    #[test]
    fn local_session_fork_uses_latest_persisted_turn_and_preserves_empty_error() {
        let turns = [
            DialogTurnData::new(
                "turn-1".to_string(),
                0,
                "session-1".to_string(),
                UserMessageData {
                    id: "user-1".to_string(),
                    content: "first".to_string(),
                    timestamp: 1,
                    metadata: None,
                },
            ),
            DialogTurnData::new(
                "turn-2".to_string(),
                1,
                "session-1".to_string(),
                UserMessageData {
                    id: "user-2".to_string(),
                    content: "second".to_string(),
                    timestamp: 2,
                    metadata: None,
                },
            ),
        ];

        assert_eq!(
            latest_persisted_turn_id(&turns).expect("latest turn should be selected"),
            "turn-2"
        );

        let error = runtime_port_error(
            latest_persisted_turn_id(&[]).expect_err("empty sessions cannot be forked"),
        );
        assert_eq!(error.kind, PortErrorKind::InvalidRequest);
        assert_eq!(
            error.message,
            "Validation error: Session has no persisted turns to fork"
        );
    }

    #[test]
    fn latest_turn_session_fork_keeps_remote_identity_unsupported() {
        for (remote_connection_id, remote_ssh_host) in [
            (Some("remote-1".to_string()), None),
            (None, Some("host-1".to_string())),
        ] {
            let request = AgentSessionForkRequest {
                workspace_path: "D:/workspace/project".to_string(),
                source_session_id: "session-1".to_string(),
                remote_connection_id,
                remote_ssh_host,
            };
            let error = validate_latest_turn_fork_scope(&request)
                .expect_err("latest-turn remote fork must remain unsupported");

            assert_eq!(error.kind, PortErrorKind::NotAvailable);
        }
    }

    #[tokio::test]
    async fn latest_turn_fork_restores_from_the_resolved_storage_path() {
        let workspace = TestWorkspace::new();
        let workspace_root = workspace.path().join("project");
        std::fs::create_dir_all(&workspace_root).expect("workspace root");
        let path_manager = workspace.path_manager();
        let storage_path = WorkspaceRuntimeService::new(path_manager.clone())
            .context_for_local_workspace(&workspace_root)
            .sessions_dir;
        std::fs::create_dir_all(&storage_path).expect("resolved session storage");
        let persistence =
            Arc::new(PersistenceManager::new(path_manager).expect("persistence manager"));
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            persistence.clone(),
            SessionManagerConfig {
                max_active_sessions: 100,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(TokioRwLock::new(ToolRegistry::new())),
            Arc::new(ToolStateManager::new(event_queue.clone())),
            None,
        ));
        let execution_engine = Arc::new(ExecutionEngine::new(
            Arc::new(RoundExecutor::new(
                Arc::new(StreamProcessor::new(event_queue.clone())),
                event_queue.clone(),
                tool_pipeline.clone(),
            )),
            event_queue.clone(),
            session_manager.clone(),
            Arc::new(ContextCompressor::new(CompressionConfig::default())),
            ExecutionEngineConfig::default(),
        ));
        let coordinator = Arc::new(ConversationCoordinator::new(
            session_manager,
            execution_engine,
            tool_pipeline,
            event_queue,
            Arc::new(EventRouter::new()),
        ));
        let token_usage_service = Arc::new(
            TokenUsageService::new_in_base_dir(workspace.path().join("tokens"))
                .await
                .expect("token usage service"),
        );
        let port = CoreSessionOperationsPort::new(coordinator, token_usage_service);

        let session_id = "session-latest-fork";
        persistence
            .save_session_metadata(
                &storage_path,
                &SessionMetadata::new(
                    session_id.to_string(),
                    "Latest fork".to_string(),
                    "agentic".to_string(),
                    "model-a".to_string(),
                ),
            )
            .await
            .expect("session metadata");
        let mut turn = DialogTurnData::new(
            "turn-latest".to_string(),
            0,
            session_id.to_string(),
            UserMessageData {
                id: "user-latest".to_string(),
                content: "fork here".to_string(),
                timestamp: 1,
                metadata: None,
            },
        );
        turn.mark_completed();
        persistence
            .save_dialog_turn(&storage_path, &turn)
            .await
            .expect("persisted turn");

        let result = port
            .fork_session(AgentSessionForkRequest {
                workspace_path: workspace_root.to_string_lossy().into_owned(),
                source_session_id: session_id.to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
            .expect("latest-turn fork should restore through the resolved storage path");

        assert_ne!(result.session_id, session_id);
        assert_eq!(result.agent_type, "agentic");
    }

    #[tokio::test]
    async fn sdk_usage_provider_validates_ids_and_keeps_live_token_enrichment() {
        let workspace = TestWorkspace::new();
        let persistence =
            PersistenceManager::new(workspace.path_manager()).expect("persistence manager");
        let token_usage_service = TokenUsageService::new_in_base_dir(workspace.path.join("tokens"))
            .await
            .expect("token usage service");

        let invalid = generate_core_session_usage_report(
            &persistence,
            &token_usage_service,
            AgentSessionUsageRequest {
                session_id: "../other-session".to_string(),
                workspace_path: Some(workspace.path().to_string_lossy().into_owned()),
                remote_connection_id: None,
                remote_ssh_host: None,
                include_hidden_subagents: false,
            },
        )
        .await
        .expect_err("path-like session ids must be rejected before persistence access");
        assert_eq!(
            runtime_port_error(invalid).kind,
            PortErrorKind::InvalidRequest
        );

        let session_id = "session-usage";
        persistence
            .save_session_metadata(
                workspace.path(),
                &SessionMetadata::new(
                    session_id.to_string(),
                    "Usage session".to_string(),
                    "agentic".to_string(),
                    "model-a".to_string(),
                ),
            )
            .await
            .expect("session metadata should persist");
        let mut turn = DialogTurnData::new(
            "turn-usage".to_string(),
            0,
            session_id.to_string(),
            UserMessageData {
                id: "user-usage".to_string(),
                content: "measure this session".to_string(),
                timestamp: 1,
                metadata: None,
            },
        );
        turn.mark_completed();
        persistence
            .save_dialog_turn(workspace.path(), &turn)
            .await
            .expect("dialog turn should persist");
        token_usage_service
            .record_usage(
                "model-config-a".to_string(),
                "model-a".to_string(),
                session_id.to_string(),
                turn.turn_id.clone(),
                10,
                5,
                Some(2),
                None,
                false,
            )
            .await
            .expect("live token usage should persist");

        let report = generate_core_session_usage_report(
            &persistence,
            &token_usage_service,
            AgentSessionUsageRequest {
                session_id: session_id.to_string(),
                workspace_path: Some(workspace.path().to_string_lossy().into_owned()),
                remote_connection_id: None,
                remote_ssh_host: None,
                include_hidden_subagents: false,
            },
        )
        .await
        .expect("usage provider should generate a report");

        assert_eq!(report.tokens.source, UsageTokenSource::TokenUsageRecords);
        assert_eq!(report.tokens.input_tokens, Some(10));
        assert_eq!(report.tokens.output_tokens, Some(5));
        assert_eq!(report.tokens.total_tokens, Some(15));
    }
}
