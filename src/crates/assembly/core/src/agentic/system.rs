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
use crate::service::token_usage::{TokenUsageService, TokenUsageSubscriber};
use bitfun_product_capabilities::DeliveryProfile;

/// Agentic runtime state shared by host adapters.
#[derive(Clone)]
pub struct AgenticSystem {
    pub coordinator: Arc<coordination::ConversationCoordinator>,
    pub event_queue: Arc<events::EventQueue>,
    pub token_usage_service: Arc<TokenUsageService>,
}

/// Initialize the agentic runtime and register the global coordinator.
pub async fn init_agentic_system() -> Result<AgenticSystem> {
    init_agentic_system_for_profile(DeliveryProfile::ProductFull).await
}

/// Select the process-wide Agent delivery profile before any service reads the
/// global tool registry.
///
/// Product composition roots call this before configuration canonicalization;
/// later initialization verifies the same profile and rejects replacement.
pub fn select_agentic_system_profile(delivery_profile: DeliveryProfile) -> Result<()> {
    tools::registry::initialize_global_tool_registry_for_profile(delivery_profile)
        .map(|_| ())
        .map_err(anyhow::Error::msg)
}

/// Initialize the single process-wide agentic runtime for one product profile.
pub async fn init_agentic_system_for_profile(
    delivery_profile: DeliveryProfile,
) -> Result<AgenticSystem> {
    info!("Initializing agentic system for profile {delivery_profile}");

    select_agentic_system_profile(delivery_profile)?;

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
        Default::default(),
    ));

    let tool_registry = tools::registry::get_global_tool_registry();
    let tool_state_manager = Arc::new(tools::pipeline::ToolStateManager::new(event_queue.clone()));
    let permission_request_manager =
        crate::product_runtime::core_permission_request_manager().map_err(anyhow::Error::msg)?;
    let tool_pipeline = Arc::new(
        tools::pipeline::ToolPipeline::new(tool_registry, tool_state_manager, None)
            .with_permission_request_manager(permission_request_manager),
    );

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
        execution::ExecutionEngineConfig::default(),
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
