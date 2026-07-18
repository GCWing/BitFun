//! Core product-full runtime adapter boundary.
//!
//! Product runtime assembly facts live in `bitfun-product-capabilities`. Core
//! keeps only compatibility exports and adapter wiring that still depends on
//! existing concrete core paths.

mod runtime_services;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bitfun_agent_runtime::sdk::{
    AgentEventSource, AgentRuntime, AgentSessionForkPort, AgentSessionForkRequest,
    AgentSessionForkResult, AgentSessionUsagePort, AgentSessionUsageRequest,
    AgentTurnSettlementPort, AgentTurnSettlementRequest,
};
use bitfun_harness::HarnessRegistry;
use bitfun_runtime_ports::{
    PortError, PortErrorKind, PortResult, SessionStoragePathRequest, SessionStorePort,
    SessionViewRestoreTiming,
};
use bitfun_runtime_services::RuntimeServices;

use crate::agentic::coordination::{
    ConversationCoordinator, DialogScheduler, SessionMaintenancePermit,
};
use crate::agentic::core::{Session, SessionConfig};
use crate::agentic::keyed_lock::KeyedAsyncLockGuard;
use crate::agentic::persistence::session_branch::{SessionBranchRequest, SessionBranchResult};
use crate::agentic::persistence::{PersistenceManager, SessionMetadataPage};
use crate::agentic::session::CoreSessionStorePort;
use crate::service::session::{DialogTurnData, SessionMetadata};
use crate::service::session_usage::{
    generate_session_usage_report, SessionUsageReport, SessionUsageReportRequest,
};
use crate::service::snapshot::{
    get_snapshot_manager_for_workspace, initialize_snapshot_manager_for_workspace, SnapshotManager,
};
use crate::service::token_usage::TokenUsageService;
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};

pub use bitfun_product_capabilities::ProductRuntimeAssembly as CoreProductRuntimeAssembly;
pub use runtime_services::CoreRuntimeServicesProvider;

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

async fn ensure_snapshot_manager(workspace_path: &Path) -> BitFunResult<Arc<SnapshotManager>> {
    if let Some(manager) = get_snapshot_manager_for_workspace(workspace_path) {
        return Ok(manager);
    }
    initialize_snapshot_manager_for_workspace(workspace_path.to_path_buf(), None)
        .await
        .map_err(|error| BitFunError::service(error.to_string()))?;
    get_snapshot_manager_for_workspace(workspace_path).ok_or_else(|| {
        BitFunError::service(format!(
            "Snapshot manager is unavailable for workspace {}",
            workspace_path.display()
        ))
    })
}

/// Product-assembly entry for the public Agent Runtime SDK.
///
/// Concrete coordinator and scheduler ownership remains in Core. Product
/// surfaces receive only the SDK runtime assembled from validated services and
/// harnesses; plugin-host bindings are deliberately not part of this API.
pub struct CoreProductAgentRuntime;

impl CoreProductAgentRuntime {
    pub fn build(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
        token_usage_service: Arc<TokenUsageService>,
        services: RuntimeServices,
        harness_registry: HarnessRegistry,
    ) -> Result<AgentRuntime, String> {
        let session_operations = Arc::new(CoreSessionOperationsPort::new(
            coordinator.clone(),
            scheduler.clone(),
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
    token_usage_service: Arc<TokenUsageService>,
}

impl CoreAgentRuntimeCompatibility {
    pub fn build(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
        token_usage_service: Arc<TokenUsageService>,
    ) -> Self {
        let persistence = coordinator.get_session_manager().persistence_manager();

        Self {
            coordinator,
            scheduler,
            persistence,
            token_usage_service,
        }
    }

    /// Compatibility shim for callers migrating to the Agent Runtime SDK.
    #[deprecated(note = "use AgentRuntime::create_session_with_id")]
    pub async fn create_session_with_id(
        &self,
        session_id: String,
        session_name: String,
        agent_type: String,
        workspace_path: String,
    ) -> BitFunResult<Session> {
        self.coordinator
            .create_session_with_id(
                Some(session_id),
                session_name,
                agent_type,
                SessionConfig {
                    workspace_path: Some(workspace_path),
                    ..Default::default()
                },
            )
            .await
    }

    pub async fn update_session_agent_type(
        &self,
        session_id: &str,
        agent_type: &str,
    ) -> BitFunResult<()> {
        self.coordinator
            .update_session_agent_type(session_id, agent_type)
            .await
    }

    pub async fn create_session_with_workspace(
        &self,
        session_id: Option<String>,
        session_name: String,
        agent_type: String,
        config: SessionConfig,
        workspace_path: String,
    ) -> BitFunResult<Session> {
        self.coordinator
            .create_session_with_workspace(
                session_id,
                session_name,
                agent_type,
                config,
                workspace_path,
            )
            .await
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

    pub async fn restore_session_for_workspace(
        &self,
        request: SessionStoragePathRequest,
        session_id: &str,
        include_internal: bool,
    ) -> BitFunResult<Session> {
        validate_persisted_session_id(session_id)?;
        if include_internal {
            self.coordinator
                .restore_internal_session_for_workspace(request, session_id)
                .await
        } else {
            self.coordinator
                .restore_session_for_workspace(request, session_id)
                .await
        }
    }

    pub async fn branch_session_at_latest_turn(
        &self,
        workspace_path: &Path,
        source_session_id: &str,
    ) -> BitFunResult<SessionBranchResult> {
        let (_, turns) = self
            .coordinator
            .restore_session_view(workspace_path, source_session_id)
            .await?;
        let source_turn_id = turns
            .last()
            .map(|turn| turn.turn_id.clone())
            .ok_or_else(|| {
                BitFunError::Validation("Session has no persisted turns to fork".to_string())
            })?;

        self.persistence
            .branch_session(
                workspace_path,
                &SessionBranchRequest {
                    source_session_id: source_session_id.to_string(),
                    source_turn_id,
                },
            )
            .await
    }

    pub async fn generate_session_usage_report(
        &self,
        request: SessionUsageReportRequest,
    ) -> BitFunResult<SessionUsageReport> {
        validate_persisted_session_id(&request.session_id)?;
        generate_session_usage_report(
            self.persistence.as_ref(),
            Some(self.token_usage_service.as_ref()),
            request,
        )
        .await
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

    pub fn is_session_loaded_in_memory(&self, session_id: &str) -> BitFunResult<bool> {
        validate_persisted_session_id(session_id)?;
        Ok(self
            .coordinator
            .get_session_manager()
            .get_session(session_id)
            .is_some())
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

    pub async fn update_session_title_for_storage_path(
        &self,
        storage_path: &Path,
        session_id: &str,
        title: &str,
    ) -> BitFunResult<()> {
        validate_persisted_session_id(session_id)?;
        self.ensure_session_loaded_from_storage_path(storage_path, session_id, false)
            .await?;
        self.coordinator
            .update_session_title(session_id, title)
            .await?;
        Ok(())
    }

    pub async fn get_thread_goal(
        &self,
        session_id: &str,
        storage_path: &Path,
    ) -> BitFunResult<Option<bitfun_runtime_ports::ThreadGoal>> {
        validate_persisted_session_id(session_id)?;
        self.coordinator
            .get_thread_goal(session_id, storage_path)
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

    pub async fn save_persisted_session_metadata(
        &self,
        workspace_path: &Path,
        metadata: &SessionMetadata,
    ) -> BitFunResult<()> {
        validate_persisted_session_id(&metadata.session_id)?;
        self.persistence
            .save_session_metadata(workspace_path, metadata)
            .await
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

    pub async fn get_session_snapshot_files(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Vec<PathBuf>> {
        validate_persisted_session_id(session_id)?;
        ensure_snapshot_manager(workspace_path)
            .await?
            .get_session_files(session_id)
            .await
            .map_err(|error| BitFunError::service(error.to_string()))
    }

    pub async fn get_session_snapshot_stats(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Option<serde_json::Value>> {
        validate_persisted_session_id(session_id)?;
        let Some(manager) = get_snapshot_manager_for_workspace(workspace_path) else {
            return Ok(None);
        };
        manager
            .get_session_stats(session_id)
            .await
            .map(Some)
            .map_err(|error| BitFunError::service(error.to_string()))
    }

    pub async fn rollback_workspace_files_to_turn(
        &self,
        workspace_path: &Path,
        session_id: &str,
        turn_index: usize,
    ) -> BitFunResult<Vec<PathBuf>> {
        validate_persisted_session_id(session_id)?;
        ensure_snapshot_manager(workspace_path)
            .await?
            .rollback_to_turn(session_id, turn_index)
            .await
            .map_err(|error| BitFunError::service(error.to_string()))
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

    pub async fn append_completed_local_command_turn(
        &self,
        session_id: &str,
        content: String,
        turn_id: Option<String>,
        timestamp_ms: Option<u64>,
        user_message_metadata: Option<serde_json::Value>,
    ) -> BitFunResult<DialogTurnData> {
        self.coordinator
            .get_session_manager()
            .append_completed_local_command_turn(
                session_id,
                content,
                turn_id,
                timestamp_ms,
                user_message_metadata,
            )
            .await
    }
}

#[derive(Clone)]
struct CoreSessionOperationsPort {
    compatibility: CoreAgentRuntimeCompatibility,
    coordinator: Arc<ConversationCoordinator>,
}

impl CoreSessionOperationsPort {
    fn new(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
        token_usage_service: Arc<TokenUsageService>,
    ) -> Self {
        Self {
            compatibility: CoreAgentRuntimeCompatibility::build(
                coordinator.clone(),
                scheduler,
                token_usage_service,
            ),
            coordinator,
        }
    }
}

fn runtime_port_error(error: BitFunError) -> PortError {
    let kind = match &error {
        BitFunError::Validation(_) => PortErrorKind::InvalidRequest,
        BitFunError::NotFound(_) => PortErrorKind::NotFound,
        BitFunError::Timeout(_) => PortErrorKind::Timeout,
        BitFunError::Cancelled(_) => PortErrorKind::Cancelled,
        _ => PortErrorKind::Backend,
    };
    PortError::new(kind, error.to_string())
}

fn validate_local_session_fork_request(request: &AgentSessionForkRequest) -> PortResult<()> {
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
        validate_local_session_fork_request(&request)?;
        let result = self
            .compatibility
            .branch_session_at_latest_turn(
                Path::new(&request.workspace_path),
                &request.source_session_id,
            )
            .await
            .map_err(runtime_port_error)?;
        Ok(AgentSessionForkResult {
            session_id: result.session_id,
            session_name: result.session_name,
            agent_type: result.agent_type,
        })
    }
}

#[async_trait::async_trait]
impl AgentSessionUsagePort for CoreSessionOperationsPort {
    async fn generate_session_usage(
        &self,
        request: AgentSessionUsageRequest,
    ) -> PortResult<SessionUsageReport> {
        self.compatibility
            .generate_session_usage_report(request)
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
    use std::sync::Arc;

    use bitfun_agent_runtime::sdk::AgentRuntime;
    use bitfun_harness::HarnessRegistry;
    use bitfun_runtime_services::RuntimeServices;

    use super::{
        validate_local_session_fork_request, validate_persisted_session_id,
        CoreAgentRuntimeCompatibility, CoreProductAgentRuntime,
    };
    use crate::agentic::coordination::{ConversationCoordinator, DialogScheduler};
    use crate::service::token_usage::TokenUsageService;
    use bitfun_agent_runtime::sdk::{AgentSessionForkRequest, PortErrorKind};

    #[test]
    fn product_agent_runtime_has_one_sdk_safe_builder_boundary() {
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
        let _ = CoreProductAgentRuntime::build_acp;
    }

    #[test]
    #[allow(deprecated)]
    fn compatibility_operations_have_one_core_owned_facade() {
        fn build(
            coordinator: Arc<ConversationCoordinator>,
            scheduler: Arc<DialogScheduler>,
            token_usage_service: Arc<TokenUsageService>,
        ) -> CoreAgentRuntimeCompatibility {
            CoreAgentRuntimeCompatibility::build(coordinator, scheduler, token_usage_service)
        }

        let _ = build;
        let _ = CoreAgentRuntimeCompatibility::create_session_with_id;
        let _ = CoreAgentRuntimeCompatibility::branch_session_at_latest_turn;
        let _ = CoreAgentRuntimeCompatibility::generate_session_usage_report;
        let _ = CoreAgentRuntimeCompatibility::list_persisted_sessions;
        let _ = CoreAgentRuntimeCompatibility::load_persisted_session_turns;
        let _ = CoreAgentRuntimeCompatibility::update_session_agent_type;
    }

    #[test]
    fn persisted_session_compatibility_rejects_path_like_ids() {
        let error = validate_persisted_session_id("../../other-project/session")
            .expect_err("compatibility boundary must reject path-like session ids");

        assert!(error.to_string().contains("session_id"), "{error}");
    }

    #[test]
    fn local_session_fork_rejects_remote_identity() {
        let error = validate_local_session_fork_request(&AgentSessionForkRequest {
            workspace_path: "D:/workspace/project".to_string(),
            source_session_id: "session-1".to_string(),
            remote_connection_id: Some("remote-1".to_string()),
            remote_ssh_host: None,
        })
        .expect_err("the local fork provider must reject remote identity");

        assert_eq!(error.kind, PortErrorKind::NotAvailable);
    }
}
