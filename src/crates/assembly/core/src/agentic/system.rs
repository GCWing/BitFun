//! Agentic system assembly shared by CLI, ACP, and other hosts.

use std::sync::Arc;

use anyhow::Result;
use log::info;

use crate::agentic::coordination;
use crate::agentic::events;
use crate::agentic::execution;
use crate::agentic::goal_mode::ThreadGoalTokenSubscriber;
use crate::agentic::persistence;
use crate::agentic::session;
use crate::agentic::tools;
use crate::infrastructure::ai::AIClientFactory;
use crate::infrastructure::try_get_path_manager_arc;
use crate::service::config::get_global_config_service;
use crate::service::config::types::AIConfig;
use crate::service::token_usage::{TokenUsageService, TokenUsageSubscriber};

/// Resolve the effective per-dialog-turn round limit.
///
/// An explicit override wins over the configured value: `Some(0)` means
/// unlimited (`usize::MAX`), `Some(n)` pins the limit to `n`. Without an
/// override the configured value is used (at least 1).
fn resolve_max_rounds(configured: usize, max_rounds_override: Option<usize>) -> usize {
    match max_rounds_override {
        Some(0) => usize::MAX, // unlimited
        Some(n) => n,
        None => configured.max(1),
    }
}

/// Resolve the per-dialog-turn round limit (`max_rounds`, configurable as
/// `max_turns` in the AI config) into an execution engine configuration.
///
/// An explicit override wins over the configured value: `Some(0)` means
/// unlimited (`usize::MAX`), `Some(n)` pins the limit to `n`. Falls back to
/// the built-in default when the config service is unavailable.
async fn resolve_execution_engine_config(
    max_rounds_override: Option<usize>,
) -> execution::ExecutionEngineConfig {
    let ai_config: AIConfig = match get_global_config_service().await {
        Ok(service) => service.get_config(Some("ai")).await.unwrap_or_default(),
        Err(_) => AIConfig::default(),
    };
    execution::ExecutionEngineConfig {
        max_rounds: resolve_max_rounds(ai_config.max_rounds, max_rounds_override),
        ..execution::ExecutionEngineConfig::default()
    }
}

/// Agentic runtime state shared by host adapters.
#[derive(Clone)]
pub struct AgenticSystem {
    pub coordinator: Arc<coordination::ConversationCoordinator>,
    pub event_queue: Arc<events::EventQueue>,
    pub token_usage_service: Arc<TokenUsageService>,
}

/// Initialize the agentic runtime and register the global coordinator.
pub async fn init_agentic_system() -> Result<AgenticSystem> {
    init_agentic_system_with_config(session::SessionManagerConfig::default()).await
}

/// Initialize the agentic runtime with a custom session manager configuration.
pub async fn init_agentic_system_with_config(
    session_config: session::SessionManagerConfig,
) -> Result<AgenticSystem> {
    init_agentic_system_with_options(session_config, None).await
}

/// Initialize the agentic runtime with a custom session manager configuration
/// and an explicit per-dialog-turn round-limit override (`0` = unlimited).
/// The override takes precedence over the configured `ai.max_rounds` /
/// `ai.max_turns` value.
pub async fn init_agentic_system_with_options(
    session_config: session::SessionManagerConfig,
    max_rounds_override: Option<usize>,
) -> Result<AgenticSystem> {
    info!("Initializing agentic system");

    let _ai_client_factory = AIClientFactory::get_global().await?;

    let event_queue = Arc::new(events::EventQueue::new(Default::default()));
    let event_router = Arc::new(events::EventRouter::new());

    let path_manager = try_get_path_manager_arc()?;
    let persistence_manager = Arc::new(persistence::PersistenceManager::new(path_manager.clone())?);
    let token_usage_service = Arc::new(TokenUsageService::new(path_manager.clone()).await?);
    let token_usage_subscriber = Arc::new(TokenUsageSubscriber::new(token_usage_service.clone()));
    event_router.subscribe_internal("token_usage".to_string(), token_usage_subscriber);
    event_router.subscribe_internal(
        "thread_goal_tokens".to_string(),
        Arc::new(ThreadGoalTokenSubscriber),
    );

    let context_store = Arc::new(session::SessionContextStore::new());
    let context_compressor = Arc::new(session::ContextCompressor::new(Default::default()));

    let session_manager = Arc::new(session::SessionManager::new(
        context_store,
        persistence_manager,
        session_config,
    ));

    let tool_registry = tools::registry::get_global_tool_registry();
    let tool_state_manager = Arc::new(tools::pipeline::ToolStateManager::new(event_queue.clone()));
    let tool_pipeline = Arc::new(tools::pipeline::ToolPipeline::new(
        tool_registry,
        tool_state_manager,
        None,
    ));

    let stream_processor = Arc::new(execution::StreamProcessor::new(event_queue.clone()));
    let round_executor = Arc::new(execution::RoundExecutor::new(
        stream_processor,
        event_queue.clone(),
        tool_pipeline.clone(),
    ));

    let execution_engine = Arc::new(execution::ExecutionEngine::new(
        round_executor,
        event_queue.clone(),
        session_manager.clone(),
        context_compressor,
        resolve_execution_engine_config(max_rounds_override).await,
    ));

    let coordinator = Arc::new(coordination::ConversationCoordinator::new(
        session_manager,
        execution_engine,
        tool_pipeline,
        event_queue.clone(),
        event_router.clone(),
    ));

    coordination::ConversationCoordinator::set_global(coordinator.clone());

    let mut internal_event_rx = event_queue.subscribe();
    let internal_event_router = event_router.clone();
    tokio::spawn(async move {
        loop {
            match internal_event_rx.recv().await {
                Ok(envelope) => {
                    if let Err(error) = internal_event_router.route(envelope).await {
                        log::warn!("Internal agentic event routing failed: {}", error);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    log::warn!("Internal agentic event router lagged by {} events", skipped);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    info!("Agentic system initialization complete");

    Ok(AgenticSystem {
        coordinator,
        event_queue,
        token_usage_service,
    })
}

#[cfg(test)]
mod tests {
    use super::resolve_max_rounds;

    #[test]
    fn max_rounds_override_pins_exact_value() {
        assert_eq!(resolve_max_rounds(200, Some(500)), 500);
        assert_eq!(resolve_max_rounds(200, Some(1)), 1);
        assert_eq!(resolve_max_rounds(200, Some(10_000)), 10_000);
    }

    #[test]
    fn max_rounds_override_zero_means_unlimited() {
        assert_eq!(resolve_max_rounds(200, Some(0)), usize::MAX);
        assert_eq!(resolve_max_rounds(50, Some(0)), usize::MAX);
    }

    #[test]
    fn max_rounds_without_override_uses_configured_value() {
        assert_eq!(resolve_max_rounds(200, None), 200);
        assert_eq!(resolve_max_rounds(500, None), 500);
        // Configured 0 is clamped to 1 so the loop always makes progress.
        assert_eq!(resolve_max_rounds(0, None), 1);
    }
}
