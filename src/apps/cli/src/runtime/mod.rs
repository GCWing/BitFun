use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use bitfun_agent_runtime::sdk::AgentRuntime;
use bitfun_core::agentic::system::AgenticSystem;
use bitfun_core::product_assembly::{ProductAssemblyPlan, ProductServiceCapabilityAvailability};
use bitfun_core::product_runtime::{
    build_local_runtime_services, ensure_product_dialog_scheduler, CoreAgentRuntimeCompatibility,
    CoreLocalWorkspaceSnapshot, CoreProductAgentRuntime,
};
use bitfun_core::runtime_ports::PluginRuntimeAvailability;
use bitfun_runtime_ports::LocalWorkspaceSnapshotPort;
use bitfun_runtime_services::RuntimeServices;

use crate::product_assembly::{assemble_acp_runtime_parts, assemble_cli_runtime_parts};

pub(crate) mod approval;
pub(crate) mod events;

use approval::CliApprovalPolicy;
use events::CliAgentEventSource;

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
    agent_runtime: AgentRuntime,
    local_workspace_snapshot: Arc<dyn LocalWorkspaceSnapshotPort>,
    compatibility: CoreAgentRuntimeCompatibility,
    agent_events: CliAgentEventSource,
    services: RuntimeServices,
    product: CliProductRuntimeState,
    approval_policy: CliApprovalPolicy,
}

impl CliRuntimeContext {
    pub(crate) fn build(
        agentic_system: AgenticSystem,
        workspace_root: impl AsRef<Path>,
        approval_policy: CliApprovalPolicy,
    ) -> Result<Self> {
        let scheduler = ensure_product_dialog_scheduler(&agentic_system);
        let (workspace_root, services) =
            build_local_runtime_services(workspace_root, RUNTIME_EVENT_BUFFER)?;
        let parts = assemble_cli_runtime_parts(services)
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
        let agent_events = CliAgentEventSource::new(agentic_system.event_queue.clone());
        let agent_runtime = CoreProductAgentRuntime::build(
            agentic_system.coordinator.clone(),
            scheduler.clone(),
            agentic_system.token_usage_service.clone(),
            services.clone(),
            harness_registry,
        )
        .map_err(anyhow::Error::msg)
        .context("Failed to build CLI Agent Runtime SDK")?;
        let compatibility =
            CoreAgentRuntimeCompatibility::build(agentic_system.coordinator.clone(), scheduler);
        let local_workspace_snapshot = CoreLocalWorkspaceSnapshot::build();

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
            agent_events,
            agent_runtime,
            local_workspace_snapshot,
            compatibility,
            services,
            product,
            approval_policy,
        })
    }

    pub(crate) fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub(crate) fn agent_runtime(&self) -> &AgentRuntime {
        &self.agent_runtime
    }

    pub(crate) fn compatibility(&self) -> &CoreAgentRuntimeCompatibility {
        &self.compatibility
    }

    pub(crate) fn local_workspace_snapshot(&self) -> &Arc<dyn LocalWorkspaceSnapshotPort> {
        &self.local_workspace_snapshot
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
}

#[derive(Clone)]
pub(crate) struct AcpRuntimeContext {
    _agent_events: CliAgentEventSource,
    agent_runtime: AgentRuntime,
    compatibility: CoreAgentRuntimeCompatibility,
}

impl AcpRuntimeContext {
    pub(crate) fn build(
        agentic_system: AgenticSystem,
        workspace_root: impl AsRef<Path>,
    ) -> Result<Self> {
        let scheduler = ensure_product_dialog_scheduler(&agentic_system);
        let (_, services) = build_local_runtime_services(workspace_root, RUNTIME_EVENT_BUFFER)?;
        let parts = assemble_acp_runtime_parts(services)
            .context("Failed to assemble ACP product runtime")?;
        let (services, harness_registry, _disabled_plugin_runtime) = parts.into_runtime_parts();
        let agent_events = CliAgentEventSource::new(agentic_system.event_queue.clone());
        let agent_runtime = CoreProductAgentRuntime::build_acp(
            agentic_system.coordinator.clone(),
            scheduler.clone(),
            agent_events.runtime_source(),
            services,
            harness_registry,
        )
        .map_err(anyhow::Error::msg)
        .context("Failed to build ACP Agent Runtime SDK")?;
        let compatibility =
            CoreAgentRuntimeCompatibility::build(agentic_system.coordinator, scheduler);

        Ok(Self {
            _agent_events: agent_events,
            agent_runtime,
            compatibility,
        })
    }

    pub(crate) fn parts(&self) -> (AgentRuntime, CoreAgentRuntimeCompatibility) {
        (self.agent_runtime.clone(), self.compatibility.clone())
    }
}
