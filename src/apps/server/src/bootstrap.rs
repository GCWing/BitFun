//! Server bootstrap - initializes all core services.
//!
//! Mirrors the Desktop app's init sequence without any Tauri dependency.

use bitfun_core::agentic::*;
use bitfun_core::infrastructure::ai::AIClientFactory;
use bitfun_core::infrastructure::try_get_path_manager_arc;
use bitfun_core::service::{config, filesystem, mcp, token_usage, workspace};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared application state for the server (mirrors Desktop's AppState).
pub struct ServerAppState {
    pub ai_client_factory: Arc<AIClientFactory>,
    pub workspace_service: Arc<workspace::WorkspaceService>,
    pub workspace_path: Arc<RwLock<Option<std::path::PathBuf>>>,
    pub config_service: Arc<config::ConfigService>,
    pub filesystem_service: Arc<filesystem::FileSystemService>,
    pub agent_registry: Arc<agents::AgentRegistry>,
    pub mcp_service: Option<Arc<mcp::MCPService>>,
    pub token_usage_service: Arc<token_usage::TokenUsageService>,
    pub coordinator: Arc<coordination::ConversationCoordinator>,
    pub scheduler: Arc<coordination::DialogScheduler>,
    pub event_queue: Arc<events::EventQueue>,
    pub event_router: Arc<events::EventRouter>,
    pub tool_registry_snapshot: Arc<Vec<Arc<dyn tools::framework::Tool>>>,
    pub start_time: std::time::Instant,
}

/// Initialize all core services and return the shared server state.
///
/// The optional `workspace` path, when provided, is opened automatically.
pub async fn initialize(workspace: Option<String>) -> anyhow::Result<Arc<ServerAppState>> {
    log::info!("Initializing BitFun server core services");

    // 1. Global config
    config::initialize_global_config().await?;
    let config_service = config::get_global_config_service().await?;

    // 2. AI client factory
    AIClientFactory::initialize_global().await?;
    let ai_client_factory = AIClientFactory::get_global().await?;

    // 3. Agentic system
    let path_manager = try_get_path_manager_arc()?;

    let event_queue = Arc::new(events::EventQueue::new(Default::default()));
    let event_router = Arc::new(events::EventRouter::new());

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
        tool_registry.clone(),
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
        Default::default(),
    ));

    let coordinator = Arc::new(coordination::ConversationCoordinator::new(
        session_manager.clone(),
        execution_engine,
        tool_pipeline,
        event_queue.clone(),
        event_router.clone(),
    ));

    coordination::ConversationCoordinator::set_global(coordinator.clone());

    // Token usage
    let token_usage_service =
        Arc::new(token_usage::TokenUsageService::new(path_manager.clone()).await?);
    let token_usage_subscriber = Arc::new(token_usage::TokenUsageSubscriber::new(
        token_usage_service.clone(),
    ));
    event_router.subscribe_internal("token_usage".to_string(), token_usage_subscriber);

    // Dialog scheduler
    let scheduler =
        coordination::DialogScheduler::new(coordinator.clone(), session_manager.clone());
    coordinator.set_scheduler_notifier(scheduler.outcome_sender());
    coordinator.set_round_preempt_source(scheduler.preempt_monitor());
    coordination::set_global_scheduler(scheduler.clone());

    // Cron service
    let cron_service =
        bitfun_core::service::cron::CronService::new(path_manager.clone(), scheduler.clone())
            .await?;
    bitfun_core::service::cron::set_global_cron_service(cron_service.clone());
    let cron_subscriber = Arc::new(bitfun_core::service::cron::CronEventSubscriber::new(
        cron_service.clone(),
    ));
    event_router.subscribe_internal("cron_jobs".to_string(), cron_subscriber);
    cron_service.start();

    // 4. Services
    let workspace_service = Arc::new(workspace::WorkspaceService::new().await?);
    workspace::set_global_workspace_service(workspace_service.clone());
    let filesystem_service = Arc::new(filesystem::FileSystemServiceFactory::create_default());

    let agent_registry = agents::get_agent_registry();

    let mcp_service = match mcp::MCPService::new(config_service.clone()) {
        Ok(service) => Some(Arc::new(service)),
        Err(e) => {
            log::warn!("Failed to initialize MCP service: {}", e);
            None
        }
    };

    // Tool registry snapshot
    let tool_registry_snapshot = {
        let lock = tool_registry.read().await;
        Arc::new(lock.get_all_tools())
    };

    // 5. Open workspace if specified
    let initial_workspace_path = if let Some(ws_path) = workspace {
        let path = std::path::PathBuf::from(&ws_path);
        match workspace_service.open_workspace(path.clone()).await {
            Ok(info) => {
                log::info!(
                    "Workspace opened: name={}, path={}",
                    info.name,
                    info.root_path.display()
                );

                // Initialize snapshot for workspace
                if let Err(e) =
                    bitfun_core::service::snapshot::initialize_snapshot_manager_for_workspace(
                        info.root_path.clone(),
                        None,
                    )
                    .await
                {
                    log::warn!("Failed to initialize snapshot system: {}", e);
                }

                Some(info.root_path)
            }
            Err(e) => {
                log::error!("Failed to open workspace '{}': {}", ws_path, e);
                None
            }
        }
    } else {
        // Try to restore last workspace
        workspace_service
            .get_current_workspace()
            .await
            .map(|w| w.root_path)
    };

    let state = Arc::new(ServerAppState {
        ai_client_factory,
        workspace_service,
        workspace_path: Arc::new(RwLock::new(initial_workspace_path)),
        config_service,
        filesystem_service,
        agent_registry,
        mcp_service,
        token_usage_service,
        coordinator,
        scheduler,
        event_queue,
        event_router,
        tool_registry_snapshot,
        start_time: std::time::Instant::now(),
    });

    log::info!("BitFun server core services initialized");
    Ok(state)
}
