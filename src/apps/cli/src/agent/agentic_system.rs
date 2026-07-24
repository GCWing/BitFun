use anyhow::{Context, Result};

use bitfun_core::product_assembly::DeliveryProfile;
use bitfun_core::product_runtime::CoreRuntimeServicesProvider;

pub(crate) use bitfun_core::agentic::system::AgenticSystem;

pub(crate) fn select_agentic_system_profile(profile: DeliveryProfile) -> Result<()> {
    bitfun_core::agentic::system::select_agentic_system_profile(profile)
        .context("Failed to select agentic system delivery profile")
}

pub(crate) async fn init_agentic_system(profile: DeliveryProfile) -> Result<AgenticSystem> {
    let system = bitfun_core::agentic::system::init_agentic_system_for_profile(profile)
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
