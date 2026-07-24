use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bitfun_agent_runtime::sdk::AgentRuntime;
use bitfun_core::agentic::system::{self, AgenticSystem};
use bitfun_core::product_assembly::{DeliveryProfile, ProductAssembler, ProductAssemblyInput};
use bitfun_core::product_runtime::{
    build_local_runtime_services, ensure_product_dialog_scheduler, CoreProductAgentEventSource,
    CoreProductAgentRuntime, CoreRuntimeServicesProvider,
};

const RUNTIME_EVENT_BUFFER: usize = 256;
const DELIVERY_PROFILE: DeliveryProfile = DeliveryProfile::Sdk;

pub(crate) struct SdkHostRuntime {
    workspace_root: PathBuf,
    agent_runtime: AgentRuntime,
    _agent_events: CoreProductAgentEventSource,
}

impl SdkHostRuntime {
    pub(crate) fn select_process_profile() -> Result<()> {
        system::select_agentic_system_profile(DELIVERY_PROFILE)
            .context("Failed to select SDK Host delivery profile")
    }

    pub(crate) async fn build(workspace_root: impl AsRef<Path>) -> Result<Self> {
        let (workspace_root, services) =
            build_local_runtime_services(workspace_root, RUNTIME_EVENT_BUFFER)?;

        // The SDK Host keeps its own product identity. The SDK and CLI profiles
        // currently select the same assembly-plan ceiling from shared facts.
        // The Host's effective wire capability set remains a strict subset.
        let parts = ProductAssembler::new()
            .assemble(ProductAssemblyInput::new(DELIVERY_PROFILE, services))
            .context("Failed to assemble SDK Host product runtime")?;
        let agentic_system = system::init_agentic_system_for_profile(parts.plan().profile())
            .await
            .context("Failed to initialize agentic system")?;
        bind_core_execution_ports(&agentic_system);
        let scheduler = ensure_product_dialog_scheduler(&agentic_system);
        let (services, harness_registry, _disabled_plugin_runtime) = parts.into_runtime_parts();
        let agent_events = CoreProductAgentEventSource::new(agentic_system.event_queue.clone());
        let agent_runtime = CoreProductAgentRuntime::build_sdk_host(
            agentic_system.coordinator,
            scheduler,
            agentic_system.token_usage_service,
            agent_events.runtime_source(),
            services,
            harness_registry,
        )
        .map_err(anyhow::Error::msg)
        .context("Failed to build Agent SDK runtime")?;

        Ok(Self {
            workspace_root,
            agent_runtime,
            _agent_events: agent_events,
        })
    }

    pub(crate) fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub(crate) fn agent_runtime(&self) -> &AgentRuntime {
        &self.agent_runtime
    }
}

fn bind_core_execution_ports(agentic_system: &AgenticSystem) {
    agentic_system
        .coordinator
        .set_terminal_port(CoreRuntimeServicesProvider::terminal_port());
    agentic_system
        .coordinator
        .set_remote_exec_port(CoreRuntimeServicesProvider::remote_exec_port());
}

pub(crate) async fn initialize_terminal_service() {
    use bitfun_core::infrastructure::try_get_path_manager_arc;
    use bitfun_core::service::runtime::RuntimeManager;
    use bitfun_core::service::terminal::{TerminalApi, TerminalConfig};

    let mut config = TerminalConfig::default();
    match try_get_path_manager_arc() {
        Ok(path_manager) => {
            config.shell_integration.scripts_dir =
                Some(path_manager.user_data_dir().join("sdk-host/temp/scripts"));
            config.transcript.root_dir = Some(path_manager.user_data_dir().join("terminals"));
        }
        Err(error) => {
            tracing::warn!(
                "Failed to configure SDK Host terminal storage; recording is disabled: {}",
                error
            );
        }
    }

    if let Ok(runtime_manager) = RuntimeManager::new() {
        let current_path = std::env::var("PATH").ok();
        if let Some(merged_path) = runtime_manager.merged_path_env(current_path.as_deref()) {
            config.env.insert("PATH".to_string(), merged_path.clone());
            #[cfg(windows)]
            config.env.insert("Path".to_string(), merged_path);
        }
    } else {
        tracing::warn!("Failed to initialize SDK Host terminal runtime PATH");
    }

    let _terminal_api = TerminalApi::new(config).await;
}
