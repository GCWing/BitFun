//! Core product-full runtime adapter boundary.
//!
//! Product runtime assembly facts live in `bitfun-product-capabilities`. Core
//! keeps only compatibility exports and adapter wiring that still depends on
//! existing concrete core paths.

mod runtime_services;

use std::path::Path;
use std::sync::Arc;

use bitfun_agent_runtime::sdk::AgentRuntime;
use bitfun_harness::HarnessRegistry;
use bitfun_runtime_services::RuntimeServices;

use crate::agentic::coordination::{ConversationCoordinator, DialogScheduler};
use crate::agentic::core::{Message, Session, SessionConfig, SessionState};
use crate::agentic::persistence::session_branch::{SessionBranchRequest, SessionBranchResult};
use crate::agentic::persistence::PersistenceManager;
use crate::service::session::{DialogTurnData, SessionMetadata};
use crate::service::session_usage::{
    generate_session_usage_report, SessionUsageReport, SessionUsageReportRequest,
};
use crate::service::token_usage::TokenUsageService;
use crate::service_agent_runtime::CoreServiceAgentRuntime;
use crate::util::errors::{BitFunError, BitFunResult};

pub use bitfun_product_capabilities::ProductRuntimeAssembly as CoreProductRuntimeAssembly;
pub use runtime_services::CoreRuntimeServicesProvider;

fn validate_persisted_session_id(session_id: &str) -> BitFunResult<()> {
    bitfun_core_types::validate_session_id(session_id).map_err(BitFunError::Validation)
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
        services: RuntimeServices,
        harness_registry: HarnessRegistry,
    ) -> Result<AgentRuntime, String> {
        CoreServiceAgentRuntime::product_agent_runtime(
            coordinator,
            scheduler,
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
    persistence: Arc<PersistenceManager>,
    token_usage_service: Arc<TokenUsageService>,
}

impl CoreAgentRuntimeCompatibility {
    pub fn build(
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

    pub async fn restore_session(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<Session> {
        self.coordinator
            .restore_session(workspace_path, session_id)
            .await
    }

    pub async fn is_session_loaded(
        &self,
        workspace_path: &Path,
        session_id: &str,
    ) -> BitFunResult<bool> {
        self.coordinator
            .get_session_manager()
            .is_session_loaded_for_workspace_path(workspace_path, session_id)
            .await
    }

    pub async fn get_messages(&self, session_id: &str) -> BitFunResult<Vec<Message>> {
        self.coordinator.get_messages(session_id).await
    }

    pub async fn update_session_model(&self, session_id: &str, model_id: &str) -> BitFunResult<()> {
        self.coordinator
            .update_session_model(session_id, model_id)
            .await
    }

    pub async fn confirm_tool(
        &self,
        tool_id: &str,
        updated_input: Option<serde_json::Value>,
    ) -> BitFunResult<()> {
        self.coordinator.confirm_tool(tool_id, updated_input).await
    }

    pub async fn reject_tool(&self, tool_id: &str, reason: String) -> BitFunResult<()> {
        self.coordinator.reject_tool(tool_id, reason).await
    }

    pub fn submit_user_answers(
        &self,
        tool_id: &str,
        answers: serde_json::Value,
    ) -> BitFunResult<()> {
        crate::agentic::tools::user_input_manager::get_user_input_manager()
            .send_answer(tool_id, answers)
            .map_err(BitFunError::tool)
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

    pub fn is_turn_processing(&self, session_id: &str, turn_id: &str) -> bool {
        self.coordinator
            .get_session_manager()
            .get_session(session_id)
            .is_some_and(|session| {
                matches!(
                    session.state,
                    SessionState::Processing { current_turn_id, .. } if current_turn_id == turn_id
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bitfun_agent_runtime::sdk::AgentRuntime;
    use bitfun_harness::HarnessRegistry;
    use bitfun_runtime_services::RuntimeServices;

    use super::{
        validate_persisted_session_id, CoreAgentRuntimeCompatibility, CoreProductAgentRuntime,
    };
    use crate::agentic::coordination::{ConversationCoordinator, DialogScheduler};
    use crate::service::token_usage::TokenUsageService;

    #[test]
    fn product_agent_runtime_has_one_sdk_safe_builder_boundary() {
        fn build(
            coordinator: Arc<ConversationCoordinator>,
            scheduler: Arc<DialogScheduler>,
            services: RuntimeServices,
            harness_registry: HarnessRegistry,
        ) -> Result<AgentRuntime, String> {
            CoreProductAgentRuntime::build(coordinator, scheduler, services, harness_registry)
        }

        let _ = build;
    }

    #[test]
    fn compatibility_operations_have_one_core_owned_facade() {
        fn build(
            coordinator: Arc<ConversationCoordinator>,
            token_usage_service: Arc<TokenUsageService>,
        ) -> CoreAgentRuntimeCompatibility {
            CoreAgentRuntimeCompatibility::build(coordinator, token_usage_service)
        }

        let _ = build;
        let _ = CoreAgentRuntimeCompatibility::create_session_with_id;
        let _ = CoreAgentRuntimeCompatibility::restore_session;
        let _ = CoreAgentRuntimeCompatibility::get_messages;
        let _ = CoreAgentRuntimeCompatibility::branch_session_at_latest_turn;
        let _ = CoreAgentRuntimeCompatibility::generate_session_usage_report;
        let _ = CoreAgentRuntimeCompatibility::list_persisted_sessions;
        let _ = CoreAgentRuntimeCompatibility::load_persisted_session_turns;
        let _ = CoreAgentRuntimeCompatibility::is_turn_processing;
    }

    #[test]
    fn persisted_session_compatibility_rejects_path_like_ids() {
        let error = validate_persisted_session_id("../../other-project/session")
            .expect_err("compatibility boundary must reject path-like session ids");

        assert!(error.to_string().contains("session_id"), "{error}");
    }
}
