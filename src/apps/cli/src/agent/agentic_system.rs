use anyhow::{Context, Result};

use bitfun_core::infrastructure::ai::AIClientFactory;
use bitfun_core::service::config::initialize_global_config;

pub use bitfun_core::agentic::system::{
    init_agentic_system, init_agentic_system_with_config, init_agentic_system_with_options,
    AgenticSystem,
};

pub async fn init_agentic_system_for_cli() -> Result<AgenticSystem> {
    init_agentic_system_for_cli_with_options(None).await
}

/// Initialize the agentic system for the CLI with an optional per-dialog-turn
/// round-limit override (`0` = unlimited). The override wins over the
/// configured `ai.max_rounds` / `ai.max_turns` value.
pub async fn init_agentic_system_for_cli_with_options(
    max_rounds_override: Option<usize>,
) -> Result<AgenticSystem> {
    initialize_global_config()
        .await
        .context("Failed to initialize global config service")?;
    AIClientFactory::initialize_global()
        .await
        .context("Failed to initialize global AIClientFactory")?;
    init_agentic_system_with_options(
        bitfun_core::agentic::session::SessionManagerConfig::default(),
        max_rounds_override,
    )
    .await
    .context("Failed to initialize agentic system")
}
