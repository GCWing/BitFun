use anyhow::{Context, Result};

use bitfun_core::infrastructure::ai::AIClientFactory;
use bitfun_core::product_runtime::CoreRuntimeServicesProvider;
use bitfun_core::service::config::initialize_global_config;

pub use bitfun_core::agentic::system::AgenticSystem;

pub async fn init_agentic_system() -> Result<AgenticSystem> {
    let system = bitfun_core::agentic::system::init_agentic_system()
        .await
        .context("Failed to initialize agentic system")?;
    system
        .coordinator
        .set_terminal_port(CoreRuntimeServicesProvider::terminal_port());
    system
        .coordinator
        .set_remote_exec_port(CoreRuntimeServicesProvider::remote_exec_port());
    Ok(system)
}

pub async fn init_agentic_system_for_cli() -> Result<AgenticSystem> {
    initialize_global_config()
        .await
        .context("Failed to initialize global config service")?;
    AIClientFactory::initialize_global()
        .await
        .context("Failed to initialize global AIClientFactory")?;
    init_agentic_system().await
}
