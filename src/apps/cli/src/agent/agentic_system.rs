//! Agentic System Initialization for CLI
//!
//! Initialize the complete agentic system, including coordinator, execution engine, session management, etc.

use anyhow::Result;
use bitfun_core::infrastructure::ai::AIClientFactory;
use std::sync::Arc;

// Import all agentic system modules
use bitfun_core::agentic::coordination;
use bitfun_core::agentic::events;
use bitfun_core::agentic::execution;
use bitfun_core::agentic::persistence;
use bitfun_core::agentic::session;
use bitfun_core::agentic::tools;
use bitfun_core::infrastructure::try_get_path_manager_arc;

/// Agentic system state
pub struct AgenticSystem {
    pub coordinator: Arc<coordination::ConversationCoordinator>,
    pub event_queue: Arc<events::EventQueue>,
}

/// Initialize Agentic system
pub async fn init_agentic_system() -> Result<AgenticSystem> {
    tracing::info!("Initializing Agentic system");

    let _ai_client_factory = AIClientFactory::get_global().await?;

    let event_queue = Arc::new(events::EventQueue::new(Default::default()));
    let event_router = Arc::new(events::EventRouter::new());

    let path_manager = try_get_path_manager_arc()?;
    let persistence_manager = Arc::new(persistence::PersistenceManager::new(path_manager.clone())?);

    let history_manager = Arc::new(session::MessageHistoryManager::new(
        persistence_manager.clone(),
        session::HistoryConfig {
            enable_persistence: false,
            ..Default::default()
        },
    ));

    let compression_manager = Arc::new(session::CompressionManager::new(
        persistence_manager.clone(),
        session::CompressionConfig {
            enable_persistence: false,
            ..Default::default()
        },
    ));

    let session_manager = Arc::new(session::SessionManager::new(
        history_manager.clone(),
        compression_manager,
        persistence_manager.clone(),
        Default::default(),
    ));

    let tool_registry = tools::registry::get_global_tool_registry();
    let tool_state_manager = Arc::new(tools::pipeline::ToolStateManager::new(event_queue.clone()));
    let tool_pipeline = Arc::new(tools::pipeline::ToolPipeline::new(
        tool_registry,
        tool_state_manager,
        None,
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
        Default::default(),
    ));

    let coordinator = Arc::new(coordination::ConversationCoordinator::new(
        session_manager,
        execution_engine,
        tool_pipeline,
        event_queue.clone(),
        event_router.clone(),
    ));

    coordination::ConversationCoordinator::set_global(coordinator.clone());
    tracing::info!("Agentic system initialization complete");

    Ok(AgenticSystem {
        coordinator,
        event_queue,
    })
}
