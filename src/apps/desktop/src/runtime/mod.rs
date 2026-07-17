use std::sync::Arc;

use bitfun_agent_runtime::sdk::{
    AgentDialogTurnPort, AgentInteractionResponsePort, AgentRuntime, AgentRuntimeBuilder,
    AgentSubmissionPort, AgentTurnCancellationPort, RuntimeBuildError,
};
use bitfun_core::agentic::coordination::{ConversationCoordinator, DialogScheduler};

/// Desktop-owned access to the Agent Runtime SDK interaction facade.
///
/// Core remains the sole owner of the coordinator, scheduler, sessions, tool
/// pipeline, and Agentic event queue. This context exposes only the interaction
/// ports used by current Tauri commands; it does not claim that the complete
/// Desktop delivery profile or its product services have been assembled.
pub struct DesktopRuntimeContext {
    agent_runtime: AgentRuntime,
}

impl DesktopRuntimeContext {
    pub(crate) fn build(
        coordinator: Arc<ConversationCoordinator>,
        scheduler: Arc<DialogScheduler>,
    ) -> Result<Self, RuntimeBuildError> {
        let submission: Arc<dyn AgentSubmissionPort> = coordinator.clone();
        let interaction_response: Arc<dyn AgentInteractionResponsePort> = coordinator;
        let dialog_turn: Arc<dyn AgentDialogTurnPort> = scheduler.clone();
        let cancellation: Arc<dyn AgentTurnCancellationPort> = scheduler;
        let agent_runtime = AgentRuntimeBuilder::new()
            .with_submission_port(submission)
            .with_dialog_turn_port(dialog_turn)
            .with_cancellation_port(cancellation)
            .with_interaction_response_port(interaction_response)
            .build()?;

        Ok(Self { agent_runtime })
    }

    pub(crate) fn agent_runtime(&self) -> &AgentRuntime {
        &self.agent_runtime
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn desktop_runtime_wiring_reuses_existing_core_owners() {
        let runtime_source = include_str!("mod.rs");
        let coordinator_constructor = ["ConversationCoordinator", "::new"].concat();
        let scheduler_constructor = ["DialogScheduler", "::new"].concat();
        assert!(!runtime_source.contains(&coordinator_constructor));
        assert!(!runtime_source.contains(&scheduler_constructor));

        let app_source = include_str!("../lib.rs");
        assert!(app_source.contains("DesktopRuntimeContext::build("));
        assert!(app_source.contains(".manage(desktop_runtime)"));

        assert!(runtime_source.contains("with_dialog_turn_port"));
        assert!(runtime_source.contains("with_cancellation_port"));
        assert!(runtime_source.contains("with_interaction_response_port"));
    }

    #[test]
    fn desktop_interaction_runtime_does_not_claim_unimplemented_product_services() {
        let runtime_source = include_str!("mod.rs");
        let product_assembler = ["Product", "Assembler"].concat();
        let runtime_services = ["Runtime", "Services"].concat();
        let desktop_services_provider = ["DesktopRuntime", "ServicesProvider"].concat();

        assert!(!runtime_source.contains(&product_assembler));
        assert!(!runtime_source.contains(&runtime_services));
        assert!(!runtime_source.contains(&desktop_services_provider));
    }
}
