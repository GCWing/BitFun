//! Framework-neutral Desktop session use cases.
//!
//! Tauri commands map their transport DTOs into this application boundary.
//! Rich Desktop persistence views remain on Core's compatibility facade while
//! stable lifecycle operations use the Agent Runtime SDK.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use bitfun_agent_runtime::sdk::{
    AgentRuntime, AgentSessionArchiveStateRequest, AgentSessionDeleteRequest,
    AgentSessionForkAtTurnRequest, AgentSessionRenameRequest, AgentSessionUsageRequest,
};
use bitfun_core::agentic::coordination::{ConversationCoordinator, DialogScheduler};
use bitfun_core::agentic::core::Session;
use bitfun_core::agentic::persistence::{SessionBranchResult, SessionMetadataPage};
use bitfun_core::agentic::session::SessionViewRestoreTiming;
use bitfun_core::product_runtime::{CoreAgentRuntimeCompatibility, CoreProductAgentRuntime};
use bitfun_core::service::remote_ssh::workspace_state::get_effective_session_path;
use bitfun_core::service::remote_ssh::SSHConnectionManager;
use bitfun_core::service::session::{DialogTurnData, SessionMetadata, SessionStatus};
use bitfun_core::service::session_usage::SessionUsageReport;
use bitfun_core::service::token_usage::TokenUsageService;
use bitfun_core::service::workspace::WorkspaceService;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

const UI_CUSTOM_METADATA_KEYS: [&str; 3] = ["titleSource", "titleKey", "titleParams"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UiSessionMetadataField {
    SessionName,
    Tags,
    Todos,
    ReviewActionState,
    UnreadCompletion,
    NeedsUserAttention,
    TitleMetadata,
}

#[derive(Debug, Clone)]
pub(crate) struct DesktopSessionScopeRequest {
    pub workspace_path: String,
    pub remote_connection_id: Option<String>,
    pub remote_ssh_host: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DesktopSessionApplicationError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Core(String),
    #[error("{0}")]
    Runtime(String),
    #[error("{0}")]
    RestoreBeforeRename(String),
}

pub(crate) type DesktopSessionApplicationResult<T> = Result<T, DesktopSessionApplicationError>;

#[derive(Debug)]
pub(crate) struct DesktopSessionViewRestore {
    pub session: Session,
    pub turns: Vec<DialogTurnData>,
    pub total_turn_count: usize,
    pub timings: SessionViewRestoreTiming,
}

#[derive(Debug)]
pub(crate) struct DesktopSessionWithTurnsRestore {
    pub session: Session,
    pub turns: Vec<DialogTurnData>,
}

#[derive(Clone)]
struct ResolvedDesktopSessionScope {
    workspace_path: String,
    effective_storage_path: PathBuf,
    remote_connection_id: Option<String>,
    requested_remote_ssh_host: Option<String>,
    resolved_remote_ssh_host: Option<String>,
}

#[derive(Clone)]
struct DesktopSessionScopeResolver {
    workspace_service: Arc<WorkspaceService>,
    ssh_manager: Arc<RwLock<Option<SSHConnectionManager>>>,
}

impl DesktopSessionScopeResolver {
    async fn resolve(&self, request: DesktopSessionScopeRequest) -> ResolvedDesktopSessionScope {
        let remote_connection_id = normalized_optional(request.remote_connection_id.as_deref());
        let requested_remote_ssh_host = normalized_optional(request.remote_ssh_host.as_deref());
        let mut registered_remote_ssh_host = None;
        if requested_remote_ssh_host.is_none() {
            if let Some(connection_id) = remote_connection_id.as_deref() {
                registered_remote_ssh_host = self
                    .workspace_service
                    .remote_ssh_host_for_remote_workspace(connection_id, &request.workspace_path)
                    .await;
            }
        }
        let mut saved_remote_ssh_host = None;
        if requested_remote_ssh_host.is_none() && registered_remote_ssh_host.is_none() {
            if let Some(connection_id) = remote_connection_id.as_deref() {
                let manager = self.ssh_manager.read().await.clone();
                if let Some(manager) = manager {
                    saved_remote_ssh_host = manager
                        .get_saved_host_for_connection_id(connection_id)
                        .await;
                }
            }
        }
        let resolved_remote_ssh_host = choose_remote_ssh_host(
            requested_remote_ssh_host.as_deref(),
            registered_remote_ssh_host.as_deref(),
            saved_remote_ssh_host.as_deref(),
        );
        let effective_storage_path = get_effective_session_path(
            &request.workspace_path,
            remote_connection_id.as_deref(),
            resolved_remote_ssh_host.as_deref(),
        )
        .await;

        ResolvedDesktopSessionScope {
            workspace_path: request.workspace_path,
            effective_storage_path,
            remote_connection_id,
            requested_remote_ssh_host,
            resolved_remote_ssh_host,
        }
    }
}

fn normalized_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn choose_remote_ssh_host(
    requested: Option<&str>,
    registered: Option<&str>,
    saved: Option<&str>,
) -> Option<String> {
    normalized_optional(requested)
        .or_else(|| normalized_optional(registered))
        .or_else(|| normalized_optional(saved))
}

#[async_trait]
pub(crate) trait DesktopSessionHostEffects: Send + Sync {
    async fn release_session(&self, session_id: &str);
    fn notify_session_changed(&self, session_id: &str, workspace_path: &str);
    fn notify_session_deleted(&self, session_id: &str);
}

#[derive(Clone)]
pub(crate) struct DesktopSessionApplication {
    agent_runtime: AgentRuntime,
    compatibility: CoreAgentRuntimeCompatibility,
    scope_resolver: DesktopSessionScopeResolver,
    host_effects: Arc<dyn DesktopSessionHostEffects>,
}

impl DesktopSessionApplication {
    pub(crate) fn build(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
        token_usage_service: Arc<TokenUsageService>,
        workspace_service: Arc<WorkspaceService>,
        ssh_manager: Arc<RwLock<Option<SSHConnectionManager>>>,
        host_effects: Arc<dyn DesktopSessionHostEffects>,
    ) -> Result<Self, String> {
        let agent_runtime = CoreProductAgentRuntime::build_session_surface(
            coordinator.clone(),
            scheduler.clone(),
            token_usage_service,
        )?;
        let compatibility = CoreAgentRuntimeCompatibility::build(coordinator, scheduler);

        Ok(Self {
            agent_runtime,
            compatibility,
            scope_resolver: DesktopSessionScopeResolver {
                workspace_service,
                ssh_manager,
            },
            host_effects,
        })
    }

    pub(crate) fn agent_runtime(&self) -> &AgentRuntime {
        &self.agent_runtime
    }

    async fn resolved_scope(
        &self,
        request: DesktopSessionScopeRequest,
    ) -> ResolvedDesktopSessionScope {
        self.scope_resolver.resolve(request).await
    }

    fn storage_path(&self, scope: &ResolvedDesktopSessionScope) -> PathBuf {
        scope.effective_storage_path.clone()
    }

    pub(crate) async fn list_persisted_sessions(
        &self,
        request: DesktopSessionScopeRequest,
    ) -> DesktopSessionApplicationResult<Vec<SessionMetadata>> {
        let scope = self.resolved_scope(request).await;
        let storage_path = self.storage_path(&scope);
        self.compatibility
            .list_persisted_sessions(&storage_path)
            .await
            .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))
    }

    pub(crate) async fn list_persisted_sessions_page(
        &self,
        request: DesktopSessionScopeRequest,
        cursor: Option<&str>,
        limit: usize,
    ) -> DesktopSessionApplicationResult<SessionMetadataPage> {
        let scope = self.resolved_scope(request).await;
        let storage_path = self.storage_path(&scope);
        self.compatibility
            .list_persisted_sessions_page(&storage_path, cursor, limit)
            .await
            .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))
    }

    pub(crate) async fn list_archived_sessions(
        &self,
        request: DesktopSessionScopeRequest,
    ) -> DesktopSessionApplicationResult<Vec<SessionMetadata>> {
        let sessions = self.list_persisted_sessions(request).await?;
        Ok(sessions
            .into_iter()
            .filter(|session| session.status == SessionStatus::Archived)
            .collect())
    }

    pub(crate) async fn load_session_turns(
        &self,
        request: DesktopSessionScopeRequest,
        session_id: &str,
        limit: Option<usize>,
    ) -> DesktopSessionApplicationResult<Vec<DialogTurnData>> {
        let scope = self.resolved_scope(request).await;
        let storage_path = self.storage_path(&scope);
        self.compatibility
            .load_persisted_session_turns(&storage_path, session_id, limit)
            .await
            .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))
    }

    pub(crate) async fn load_session_metadata(
        &self,
        request: DesktopSessionScopeRequest,
        session_id: &str,
    ) -> DesktopSessionApplicationResult<Option<SessionMetadata>> {
        let scope = self.resolved_scope(request).await;
        let storage_path = self.storage_path(&scope);
        self.compatibility
            .load_persisted_session_metadata(&storage_path, session_id)
            .await
            .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))
    }

    pub(crate) async fn touch_session(
        &self,
        request: DesktopSessionScopeRequest,
        session_id: &str,
    ) -> DesktopSessionApplicationResult<()> {
        let scope = self.resolved_scope(request).await;
        let storage_path = self.storage_path(&scope);
        self.compatibility
            .touch_persisted_session(&storage_path, session_id)
            .await
            .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))
    }

    pub(crate) async fn save_ui_metadata(
        &self,
        request: DesktopSessionScopeRequest,
        incoming: SessionMetadata,
        fields: Vec<UiSessionMetadataField>,
    ) -> DesktopSessionApplicationResult<()> {
        if fields.is_empty() {
            return Err(DesktopSessionApplicationError::Validation(
                "At least one session metadata field is required".to_string(),
            ));
        }
        let workspace_path = request.workspace_path.clone();
        let scope = self.resolved_scope(request).await;
        let storage_path = self.storage_path(&scope);
        let session_id = incoming.session_id.clone();
        self.compatibility
            .update_persisted_session_metadata(&storage_path, &session_id, |current| {
                merge_ui_owned_session_metadata(current, &incoming, &fields);
            })
            .await
            .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))?;
        self.host_effects
            .notify_session_changed(&session_id, &workspace_path);
        Ok(())
    }

    pub(crate) async fn generate_usage_report(
        &self,
        request: DesktopSessionScopeRequest,
        session_id: String,
        include_hidden_subagents: bool,
    ) -> DesktopSessionApplicationResult<SessionUsageReport> {
        let scope = self.resolved_scope(request).await;
        let storage_path = self.storage_path(&scope);
        let mut report = self
            .agent_runtime
            .generate_session_usage(AgentSessionUsageRequest {
                session_id,
                workspace_path: Some(storage_path.to_string_lossy().to_string()),
                remote_connection_id: scope.remote_connection_id.clone(),
                remote_ssh_host: scope.requested_remote_ssh_host.clone(),
                include_hidden_subagents,
            })
            .await
            .map_err(|error| DesktopSessionApplicationError::Runtime(error.into_message()))?;
        report.workspace.path_label = Some(scope.workspace_path);
        report.workspace.remote_connection_id = scope.remote_connection_id;
        report.workspace.remote_ssh_host = scope.requested_remote_ssh_host;
        Ok(report)
    }

    pub(crate) async fn fork_session(
        &self,
        request: DesktopSessionScopeRequest,
        source_session_id: String,
        source_turn_id: String,
    ) -> DesktopSessionApplicationResult<SessionBranchResult> {
        let scope = self.resolved_scope(request).await;
        let result = self
            .agent_runtime
            .fork_session_at_turn(AgentSessionForkAtTurnRequest {
                workspace_path: scope.effective_storage_path.to_string_lossy().into_owned(),
                source_session_id,
                source_turn_id,
                remote_connection_id: scope.remote_connection_id,
                remote_ssh_host: scope.resolved_remote_ssh_host,
            })
            .await
            .map_err(|error| DesktopSessionApplicationError::Runtime(error.into_message()))?;
        Ok(SessionBranchResult {
            session_id: result.session_id,
            session_name: result.session_name,
            agent_type: result.agent_type,
        })
    }

    pub(crate) async fn set_session_archived(
        &self,
        request: DesktopSessionScopeRequest,
        session_id: String,
        archived: bool,
    ) -> DesktopSessionApplicationResult<()> {
        let scope = self.resolved_scope(request).await;
        self.agent_runtime
            .set_session_archived(AgentSessionArchiveStateRequest {
                workspace_path: scope.effective_storage_path.to_string_lossy().into_owned(),
                session_id,
                archived,
                remote_connection_id: scope.remote_connection_id,
                remote_ssh_host: scope.resolved_remote_ssh_host,
            })
            .await
            .map_err(|error| DesktopSessionApplicationError::Runtime(error.into_message()))
    }

    pub(crate) async fn delete_session(
        &self,
        request: DesktopSessionScopeRequest,
        session_id: String,
    ) -> DesktopSessionApplicationResult<()> {
        let scope = self.resolved_scope(request).await;
        delete_session_with_host_effects(
            &self.agent_runtime,
            self.host_effects.as_ref(),
            scope,
            session_id,
        )
        .await
    }

    pub(crate) async fn rename_session(
        &self,
        request: Option<DesktopSessionScopeRequest>,
        session_id: String,
        title: String,
    ) -> DesktopSessionApplicationResult<String> {
        let normalized_title = title.trim().to_string();
        if let Some(request) = request {
            let scope = self.resolved_scope(request).await;
            if !self
                .compatibility
                .is_session_loaded_in_memory(&session_id)
                .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))?
            {
                let storage_path = self.storage_path(&scope);
                self.compatibility
                    .restore_session_from_storage_path(&storage_path, &session_id, false)
                    .await
                    .map_err(|error| {
                        DesktopSessionApplicationError::RestoreBeforeRename(error.to_string())
                    })?;
            }
            self.agent_runtime
                .rename_session(AgentSessionRenameRequest {
                    workspace_path: scope.effective_storage_path.to_string_lossy().into_owned(),
                    session_id: session_id.clone(),
                    session_name: title,
                    remote_connection_id: scope.remote_connection_id,
                    remote_ssh_host: scope.resolved_remote_ssh_host,
                })
                .await
                .map_err(|error| DesktopSessionApplicationError::Runtime(error.into_message()))?;
            self.host_effects
                .notify_session_changed(&session_id, &scope.workspace_path);
            return Ok(normalized_title);
        }

        if !self
            .compatibility
            .is_session_loaded_in_memory(&session_id)
            .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))?
        {
            return Err(DesktopSessionApplicationError::Validation(
                "workspace_path is required when the session is not loaded".to_string(),
            ));
        }
        let updated_title = self
            .compatibility
            .update_loaded_session_title(&session_id, &title)
            .await
            .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))?;
        self.host_effects.notify_session_changed(&session_id, "");
        Ok(updated_title)
    }

    pub(crate) async fn ensure_session_loaded(
        &self,
        request: DesktopSessionScopeRequest,
        session_id: &str,
        include_internal: bool,
    ) -> DesktopSessionApplicationResult<()> {
        if self
            .compatibility
            .is_session_loaded_in_memory(session_id)
            .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))?
        {
            return Ok(());
        }
        if request.workspace_path.trim().is_empty() {
            return Err(DesktopSessionApplicationError::Validation(
                "workspace_path is required when the session is not loaded".to_string(),
            ));
        }
        let scope = self.resolved_scope(request).await;
        let storage_path = self.storage_path(&scope);
        self.compatibility
            .ensure_session_loaded_from_storage_path(&storage_path, session_id, include_internal)
            .await
            .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))
    }

    pub(crate) async fn restore_session(
        &self,
        request: DesktopSessionScopeRequest,
        session_id: &str,
        include_internal: bool,
    ) -> DesktopSessionApplicationResult<Session> {
        let scope = self.resolved_scope(request).await;
        let storage_path = self.storage_path(&scope);
        self.compatibility
            .restore_session_from_storage_path(&storage_path, session_id, include_internal)
            .await
            .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))
    }

    pub(crate) async fn restore_session_view<F>(
        &self,
        request: DesktopSessionScopeRequest,
        session_id: &str,
        include_internal: bool,
        tail_turn_count: Option<usize>,
        on_storage_path_resolved: F,
    ) -> DesktopSessionApplicationResult<DesktopSessionViewRestore>
    where
        F: FnOnce(u64) + Send,
    {
        let path_started_at = Instant::now();
        let scope = self.resolved_scope(request).await;
        let storage_path = self.storage_path(&scope);
        let resolve_storage_path_duration_ms =
            path_started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;
        on_storage_path_resolved(resolve_storage_path_duration_ms);
        let (session, turns, total_turn_count, mut timings) = self
            .compatibility
            .restore_session_view_from_storage_path(
                &storage_path,
                session_id,
                include_internal,
                tail_turn_count,
            )
            .await
            .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))?;
        timings.resolve_storage_path_duration_ms = resolve_storage_path_duration_ms;
        Ok(DesktopSessionViewRestore {
            session,
            turns,
            total_turn_count,
            timings,
        })
    }

    pub(crate) async fn restore_session_with_turns<F>(
        &self,
        request: DesktopSessionScopeRequest,
        session_id: &str,
        include_internal: bool,
        on_storage_path_resolved: F,
    ) -> DesktopSessionApplicationResult<DesktopSessionWithTurnsRestore>
    where
        F: FnOnce(u64) + Send,
    {
        let path_started_at = Instant::now();
        let scope = self.resolved_scope(request).await;
        let storage_path = self.storage_path(&scope);
        let resolve_storage_path_duration_ms =
            path_started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;
        on_storage_path_resolved(resolve_storage_path_duration_ms);
        let (session, turns) = self
            .compatibility
            .restore_session_with_turns_from_storage_path(
                &storage_path,
                session_id,
                include_internal,
            )
            .await
            .map_err(|error| DesktopSessionApplicationError::Core(error.to_string()))?;
        Ok(DesktopSessionWithTurnsRestore { session, turns })
    }
}

async fn delete_session_with_host_effects(
    agent_runtime: &AgentRuntime,
    host_effects: &dyn DesktopSessionHostEffects,
    scope: ResolvedDesktopSessionScope,
    session_id: String,
) -> DesktopSessionApplicationResult<()> {
    host_effects.release_session(&session_id).await;
    agent_runtime
        .delete_session(AgentSessionDeleteRequest {
            workspace_path: scope.effective_storage_path.to_string_lossy().into_owned(),
            session_id: session_id.clone(),
            remote_connection_id: scope.remote_connection_id,
            remote_ssh_host: scope.resolved_remote_ssh_host,
        })
        .await
        .map_err(|error| DesktopSessionApplicationError::Runtime(error.into_message()))?;
    host_effects.notify_session_deleted(&session_id);
    Ok(())
}

fn merge_ui_owned_session_metadata(
    current: &mut SessionMetadata,
    incoming: &SessionMetadata,
    fields: &[UiSessionMetadataField],
) {
    if fields.contains(&UiSessionMetadataField::SessionName) {
        current.session_name = incoming.session_name.clone();
    }
    if fields.contains(&UiSessionMetadataField::Tags) {
        current.tags = incoming.tags.clone();
    }
    if fields.contains(&UiSessionMetadataField::Todos) {
        current.todos = incoming.todos.clone();
    }
    if fields.contains(&UiSessionMetadataField::ReviewActionState) {
        current.review_action_state = incoming.review_action_state.clone();
    }
    if fields.contains(&UiSessionMetadataField::UnreadCompletion) {
        current.unread_completion = incoming.unread_completion.clone();
    }
    if fields.contains(&UiSessionMetadataField::NeedsUserAttention) {
        current.needs_user_attention = incoming.needs_user_attention.clone();
    }
    if fields.contains(&UiSessionMetadataField::TitleMetadata) {
        let mut custom = current
            .custom_metadata
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .cloned()
            .unwrap_or_default();
        let incoming_custom = incoming
            .custom_metadata
            .as_ref()
            .and_then(serde_json::Value::as_object);
        for key in UI_CUSTOM_METADATA_KEYS {
            custom.remove(key);
            if let Some(value) = incoming_custom.and_then(|metadata| metadata.get(key)) {
                custom.insert(key.to_string(), value.clone());
            }
        }
        current.custom_metadata = (!custom.is_empty()).then(|| serde_json::Value::Object(custom));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_agent_runtime::sdk::{
        AgentRuntimeBuilder, AgentSessionCreateRequest, AgentSessionCreateResult,
        AgentSessionListRequest, AgentSessionManagementPort, AgentSessionSummary,
        AgentSessionWorkspaceBinding, AgentSessionWorkspaceRequest, AgentSubmissionPort,
        AgentSubmissionRequest, AgentSubmissionResult, PortError, PortErrorKind, PortResult,
    };
    use bitfun_core::service::session::{SessionKind, SessionMemoryMode};
    use serde_json::json;
    use std::sync::Mutex;

    struct RecordingDeletePort {
        events: Arc<Mutex<Vec<&'static str>>>,
        fail_delete: bool,
    }

    struct NoopSubmissionPort;

    #[async_trait]
    impl AgentSubmissionPort for NoopSubmissionPort {
        async fn create_session(
            &self,
            request: AgentSessionCreateRequest,
        ) -> PortResult<AgentSessionCreateResult> {
            Ok(AgentSessionCreateResult {
                session_id: "unused".to_string(),
                session_name: request.session_name,
                agent_type: request.agent_type,
            })
        }

        async fn submit_message(
            &self,
            _request: AgentSubmissionRequest,
        ) -> PortResult<AgentSubmissionResult> {
            Ok(AgentSubmissionResult {
                turn_id: "unused".to_string(),
                accepted: true,
            })
        }

        async fn resolve_session_agent_type(
            &self,
            _session_id: &str,
        ) -> PortResult<Option<String>> {
            Ok(None)
        }
    }

    #[async_trait]
    impl AgentSessionManagementPort for RecordingDeletePort {
        async fn list_sessions(
            &self,
            _request: AgentSessionListRequest,
        ) -> PortResult<Vec<AgentSessionSummary>> {
            Ok(Vec::new())
        }

        async fn delete_session(&self, _request: AgentSessionDeleteRequest) -> PortResult<()> {
            self.events.lock().unwrap().push("durable_delete");
            if self.fail_delete {
                return Err(PortError::new(PortErrorKind::Backend, "delete failed"));
            }
            Ok(())
        }

        async fn resolve_session_workspace_binding(
            &self,
            _request: AgentSessionWorkspaceRequest,
        ) -> PortResult<Option<AgentSessionWorkspaceBinding>> {
            Ok(None)
        }
    }

    struct RecordingHostEffects {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    #[async_trait]
    impl DesktopSessionHostEffects for RecordingHostEffects {
        async fn release_session(&self, _session_id: &str) {
            self.events.lock().unwrap().push("release");
        }

        fn notify_session_changed(&self, _session_id: &str, _workspace_path: &str) {}

        fn notify_session_deleted(&self, _session_id: &str) {
            self.events.lock().unwrap().push("relay_delete");
        }
    }

    fn delete_test_scope() -> ResolvedDesktopSessionScope {
        ResolvedDesktopSessionScope {
            workspace_path: "D:/workspace/project".to_string(),
            effective_storage_path: PathBuf::from("D:/managed/project/sessions"),
            remote_connection_id: None,
            requested_remote_ssh_host: None,
            resolved_remote_ssh_host: None,
        }
    }

    fn delete_test_runtime(
        events: Arc<Mutex<Vec<&'static str>>>,
        fail_delete: bool,
    ) -> AgentRuntime {
        AgentRuntimeBuilder::new()
            .with_submission_port(Arc::new(NoopSubmissionPort))
            .with_session_management_port(Arc::new(RecordingDeletePort {
                events,
                fail_delete,
            }))
            .build()
            .expect("delete test runtime")
    }

    #[test]
    fn optional_scope_values_are_trimmed_without_inventing_identity() {
        assert_eq!(
            normalized_optional(Some(" host ")),
            Some("host".to_string())
        );
        assert_eq!(normalized_optional(Some("  ")), None);
        assert_eq!(normalized_optional(None), None);
    }

    #[test]
    fn remote_host_resolution_preserves_request_registry_and_offline_saved_precedence() {
        assert_eq!(
            choose_remote_ssh_host(Some("request-host"), Some("live-host"), Some("saved-host")),
            Some("request-host".to_string())
        );
        assert_eq!(
            choose_remote_ssh_host(None, Some("live-host"), Some("saved-host")),
            Some("live-host".to_string())
        );
        assert_eq!(
            choose_remote_ssh_host(None, None, Some(" saved-host ")),
            Some("saved-host".to_string())
        );
    }

    #[tokio::test]
    async fn local_session_storage_identity_survives_workspace_removal() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-desktop-session-path-test-{}",
            uuid::Uuid::new_v4()
        ));
        let workspace_path = root.join("project");
        std::fs::create_dir_all(&workspace_path).expect("workspace directory");
        let workspace_path = workspace_path
            .canonicalize()
            .expect("canonical workspace path")
            .to_string_lossy()
            .into_owned();

        let before_removal = get_effective_session_path(&workspace_path, None, None).await;
        std::fs::remove_dir_all(&workspace_path).expect("remove workspace directory");
        let after_removal = get_effective_session_path(&workspace_path, None, None).await;

        assert_eq!(after_removal, before_removal);
        assert_eq!(
            after_removal.file_name().and_then(|value| value.to_str()),
            Some("sessions")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn offline_saved_ssh_host_resolves_to_the_same_remote_session_tree() {
        let connection_id = "offline-saved-host-test";
        let workspace_path = "/srv/offline-project";
        let saved_host =
            choose_remote_ssh_host(None, None, Some("saved.example")).expect("saved SSH host");

        let resolved = get_effective_session_path(
            workspace_path,
            Some(connection_id),
            Some(saved_host.as_str()),
        )
        .await;
        let unresolved =
            get_effective_session_path(workspace_path, Some(connection_id), None).await;

        assert_ne!(resolved, unresolved);
        assert_eq!(
            resolved.file_name().and_then(|value| value.to_str()),
            Some("sessions")
        );
        assert!(!resolved
            .components()
            .any(|component| component.as_os_str() == std::ffi::OsStr::new("_unresolved")));
    }

    #[test]
    fn application_boundary_stays_framework_neutral() {
        let source = include_str!("session_application.rs");
        let tauri_namespace = ["tauri", "::"].concat();
        let tauri_state = ["tauri", "::", "State"].concat();
        assert!(!source.contains(&tauri_namespace));
        assert!(!source.contains(&tauri_state));
        for forbidden in [["crate", "::", "api"].concat(), ["bitfun", "_acp"].concat()] {
            assert!(!source.contains(&forbidden), "unexpected {forbidden}");
        }
        for forbidden in [
            ["Product", "Assembler"].concat(),
            ["Runtime", "Services"].concat(),
            ["Harness", "Registry"].concat(),
        ] {
            assert!(!source.contains(&forbidden), "unexpected {forbidden}");
        }
    }

    #[tokio::test]
    async fn delete_orders_host_release_durable_delete_and_relay_tombstone() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let runtime = delete_test_runtime(events.clone(), false);
        let host_effects = RecordingHostEffects {
            events: events.clone(),
        };

        delete_session_with_host_effects(
            &runtime,
            &host_effects,
            delete_test_scope(),
            "session-1".to_string(),
        )
        .await
        .expect("delete should succeed");

        assert_eq!(
            events.lock().unwrap().as_slice(),
            ["release", "durable_delete", "relay_delete"]
        );
    }

    #[tokio::test]
    async fn delete_failure_does_not_publish_relay_tombstone() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let runtime = delete_test_runtime(events.clone(), true);
        let host_effects = RecordingHostEffects {
            events: events.clone(),
        };

        let error = delete_session_with_host_effects(
            &runtime,
            &host_effects,
            delete_test_scope(),
            "session-1".to_string(),
        )
        .await
        .expect_err("durable delete should fail");

        assert!(matches!(error, DesktopSessionApplicationError::Runtime(_)));
        assert_eq!(
            events.lock().unwrap().as_slice(),
            ["release", "durable_delete"]
        );
    }

    #[test]
    fn ui_metadata_merge_preserves_core_authoritative_fields_and_custom_keys() {
        let mut current = SessionMetadata::new(
            "session".to_string(),
            "Current".to_string(),
            "plan".to_string(),
            "model-a".to_string(),
        );
        current.last_submitted_agent_type = Some("plan".to_string());
        current.memory_mode = SessionMemoryMode::Polluted;
        current.session_kind = SessionKind::Standard;
        current.status = SessionStatus::Archived;
        current.turn_count = 7;
        current.custom_metadata = Some(json!({
            "threadGoal": { "objective": "preserve" },
            "titleSource": "i18n",
            "titleKey": "old"
        }));

        let mut incoming = current.clone();
        incoming.session_name = "Renamed".to_string();
        incoming.agent_type = "agentic".to_string();
        incoming.model_name = "stale-model".to_string();
        incoming.memory_mode = SessionMemoryMode::Enabled;
        incoming.status = SessionStatus::Active;
        incoming.turn_count = 1;
        incoming.review_action_state = Some(json!({ "phase": "fixing" }));
        incoming.custom_metadata = Some(json!({
            "titleSource": "i18n",
            "titleKey": "new",
            "untrustedCoreKey": "drop"
        }));

        merge_ui_owned_session_metadata(
            &mut current,
            &incoming,
            &[
                UiSessionMetadataField::SessionName,
                UiSessionMetadataField::Tags,
                UiSessionMetadataField::Todos,
                UiSessionMetadataField::ReviewActionState,
                UiSessionMetadataField::UnreadCompletion,
                UiSessionMetadataField::NeedsUserAttention,
                UiSessionMetadataField::TitleMetadata,
            ],
        );

        assert_eq!(current.session_name, "Renamed");
        assert_eq!(current.agent_type, "plan");
        assert_eq!(current.model_name, "model-a");
        assert_eq!(current.memory_mode, SessionMemoryMode::Polluted);
        assert_eq!(current.status, SessionStatus::Archived);
        assert_eq!(current.turn_count, 7);
        assert_eq!(current.review_action_state, incoming.review_action_state);
        let custom = current.custom_metadata.unwrap();
        assert_eq!(custom["threadGoal"]["objective"], "preserve");
        assert_eq!(custom["titleKey"], "new");
        assert!(custom.get("untrustedCoreKey").is_none());
    }

    #[test]
    fn ui_metadata_field_mask_keeps_independent_writers_isolated() {
        let mut current = SessionMetadata::new(
            "session".to_string(),
            "Current".to_string(),
            "agentic".to_string(),
            "auto".to_string(),
        );
        current.review_action_state = Some(json!({ "phase": "review_completed" }));
        current.unread_completion = Some("completed".to_string());
        current.needs_user_attention = Some("ask_user".to_string());

        let mut stale_general_update = current.clone();
        stale_general_update.session_name = "Renamed".to_string();
        stale_general_update.review_action_state = None;
        merge_ui_owned_session_metadata(
            &mut current,
            &stale_general_update,
            &[UiSessionMetadataField::SessionName],
        );
        assert_eq!(current.session_name, "Renamed");
        assert_eq!(
            current.review_action_state,
            Some(json!({ "phase": "review_completed" }))
        );
        assert_eq!(current.unread_completion.as_deref(), Some("completed"));
        assert_eq!(current.needs_user_attention.as_deref(), Some("ask_user"));

        let mut review_update = current.clone();
        review_update.review_action_state = Some(json!({ "phase": "fixing" }));
        review_update.unread_completion = None;
        review_update.needs_user_attention = None;
        merge_ui_owned_session_metadata(
            &mut current,
            &review_update,
            &[UiSessionMetadataField::ReviewActionState],
        );
        assert_eq!(
            current.review_action_state,
            Some(json!({ "phase": "fixing" }))
        );
        assert_eq!(current.unread_completion.as_deref(), Some("completed"));
        assert_eq!(current.needs_user_attention.as_deref(), Some("ask_user"));
    }
}
