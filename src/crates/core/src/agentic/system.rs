//! Agentic system assembly shared by CLI, ACP, and other hosts.

use std::sync::Arc;

use anyhow::Result;
use log::info;

use crate::agentic::coordination;
use crate::agentic::events;
use crate::agentic::execution;
use crate::agentic::persistence;
use crate::agentic::session;
use crate::agentic::tools;
use crate::infrastructure::ai::AIClientFactory;
use crate::infrastructure::try_get_path_manager_arc;

/// Agentic runtime state shared by host adapters.
#[derive(Clone)]
pub struct AgenticSystem {
    pub coordinator: Arc<coordination::ConversationCoordinator>,
    pub event_queue: Arc<events::EventQueue>,
}

/// Initialize the agentic runtime and register the global coordinator.
pub async fn init_agentic_system() -> Result<AgenticSystem> {
    info!("Initializing agentic system");

    let _ai_client_factory = AIClientFactory::get_global().await?;

    let event_queue = Arc::new(events::EventQueue::new(Default::default()));
    let event_router = Arc::new(events::EventRouter::new());

    let path_manager = try_get_path_manager_arc()?;
    let persistence_manager = Arc::new(persistence::PersistenceManager::new(path_manager.clone())?);

    let context_store = Arc::new(session::SessionContextStore::new());
    let context_compressor = Arc::new(session::ContextCompressor::new(Default::default()));

    let session_manager = Arc::new(session::SessionManager::new(
        context_store,
        persistence_manager,
        Default::default(),
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
        execution::ExecutionEngineConfig::default(),
    ));

    let coordinator = Arc::new(coordination::ConversationCoordinator::new(
        session_manager,
        execution_engine,
        tool_pipeline,
        event_queue.clone(),
        event_router,
    ));

    coordination::ConversationCoordinator::set_global(coordinator.clone());
    info!("Agentic system initialization complete");

    Ok(AgenticSystem {
        coordinator,
        event_queue,
    })
}
