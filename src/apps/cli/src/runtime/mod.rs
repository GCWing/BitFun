use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use bitfun_agent_runtime::sdk::AgentRuntime;
use bitfun_core::agentic::coordination::{self, DialogScheduler};
use bitfun_core::agentic::system::AgenticSystem;
use bitfun_core::product_assembly::{ProductAssemblyPlan, ProductServiceCapabilityAvailability};
use bitfun_core::product_runtime::{CoreAgentRuntimeCompatibility, CoreProductAgentRuntime};
use bitfun_core::runtime_ports::PluginRuntimeAvailability;
use bitfun_runtime_services::RuntimeServices;

use crate::product_assembly::assemble_cli_runtime_parts;

pub(crate) mod approval;
pub(crate) mod events;
pub(crate) mod services;

use approval::{CliApprovalController, CliApprovalPolicy, CliPermissionService};
use events::CliAgentEventSource;
use services::{CliClock, CliRuntimeEventSink, CliRuntimeServicesProvider};

const RUNTIME_EVENT_BUFFER: usize = 256;

#[derive(Debug, Clone)]
pub(crate) struct CliProductRuntimeState {
    plan: ProductAssemblyPlan,
    service_availability: Vec<ProductServiceCapabilityAvailability>,
    plugin_runtime: PluginRuntimeAvailability,
    harness_provider_ids: Vec<String>,
}

impl CliProductRuntimeState {
    pub(crate) fn plan(&self) -> &ProductAssemblyPlan {
        &self.plan
    }

    pub(crate) fn service_availability(&self) -> &[ProductServiceCapabilityAvailability] {
        &self.service_availability
    }

    pub(crate) const fn plugin_runtime(&self) -> PluginRuntimeAvailability {
        self.plugin_runtime
    }

    pub(crate) fn harness_provider_ids(&self) -> &[String] {
        &self.harness_provider_ids
    }
}

#[derive(Clone)]
pub(crate) struct CliRuntimeContext {
    workspace_root: PathBuf,
    agentic_system: AgenticSystem,
    agent_runtime: AgentRuntime,
    compatibility: CoreAgentRuntimeCompatibility,
    agent_events: CliAgentEventSource,
    services: RuntimeServices,
    product: CliProductRuntimeState,
    approval_policy: CliApprovalPolicy,
    approval_controller: Arc<CliApprovalController>,
}

impl CliRuntimeContext {
    pub(crate) fn build(
        agentic_system: AgenticSystem,
        workspace_root: impl AsRef<Path>,
        approval_policy: CliApprovalPolicy,
    ) -> Result<Self> {
        let scheduler = ensure_dialog_scheduler(&agentic_system);
        let runtime_events = Arc::new(CliRuntimeEventSink::new(RUNTIME_EVENT_BUFFER));
        let provider = CliRuntimeServicesProvider::new(
            workspace_root,
            Arc::new(CliPermissionService::new(approval_policy)),
            runtime_events.clone(),
            Arc::new(CliClock),
        )?;
        let workspace_root = provider.workspace_root().to_path_buf();
        let parts = assemble_cli_runtime_parts(provider.build()?)
            .context("Failed to assemble CLI product runtime")?;

        let product = CliProductRuntimeState {
            plan: parts.plan().clone(),
            service_availability: parts.service_availability().to_vec(),
            plugin_runtime: parts.plugin_runtime().availability(),
            harness_provider_ids: parts
                .harness_registry()
                .provider_ids()
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
        };
        let (services, harness_registry, _disabled_plugin_runtime) = parts.into_runtime_parts();
        let agent_runtime = CoreProductAgentRuntime::build(
            agentic_system.coordinator.clone(),
            scheduler.clone(),
            services.clone(),
            harness_registry,
        )
        .map_err(anyhow::Error::msg)
        .context("Failed to build CLI Agent Runtime SDK")?;
        let compatibility = CoreAgentRuntimeCompatibility::build(
            agentic_system.coordinator.clone(),
            agentic_system.token_usage_service.clone(),
        );

        debug_assert_eq!(
            agent_runtime.harness_provider_ids(),
            product
                .harness_provider_ids
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );

        Ok(Self {
            workspace_root,
            agent_events: CliAgentEventSource::new(agentic_system.event_queue.clone()),
            agentic_system,
            agent_runtime,
            compatibility,
            services,
            product,
            approval_policy,
            approval_controller: Arc::new(CliApprovalController::new()),
        })
    }

    pub(crate) fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub(crate) fn agentic_system(&self) -> &AgenticSystem {
        &self.agentic_system
    }

    pub(crate) fn agent_runtime(&self) -> &AgentRuntime {
        &self.agent_runtime
    }

    pub(crate) fn compatibility(&self) -> &CoreAgentRuntimeCompatibility {
        &self.compatibility
    }

    pub(crate) fn agent_events(&self) -> &CliAgentEventSource {
        &self.agent_events
    }

    pub(crate) fn services(&self) -> &RuntimeServices {
        &self.services
    }

    pub(crate) fn product(&self) -> &CliProductRuntimeState {
        &self.product
    }

    pub(crate) const fn approval_policy(&self) -> CliApprovalPolicy {
        self.approval_policy
    }

    pub(crate) fn approval_controller(&self) -> &Arc<CliApprovalController> {
        &self.approval_controller
    }
}

fn ensure_dialog_scheduler(agentic_system: &AgenticSystem) -> Arc<DialogScheduler> {
    if let Some(scheduler) = coordination::get_global_scheduler() {
        return scheduler;
    }

    let session_manager = agentic_system.coordinator.get_session_manager().clone();
    let scheduler = DialogScheduler::new(agentic_system.coordinator.clone(), session_manager);
    agentic_system
        .coordinator
        .set_scheduler_notifier(scheduler.outcome_sender());
    agentic_system
        .coordinator
        .set_round_injection_source(scheduler.round_injection_monitor());
    coordination::set_global_scheduler(scheduler.clone());
    scheduler
}
