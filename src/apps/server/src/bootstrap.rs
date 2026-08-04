//! Server bootstrap - initializes all core services.
//!
//! Mirrors the Desktop app's init sequence without any Tauri dependency.

use bitfun_core::agentic::*;
use bitfun_core::infrastructure::ai::AIClientFactory;
use bitfun_core::infrastructure::try_get_path_manager_arc;
use bitfun_core::service::{config, filesystem, mcp, token_usage, workspace};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, PartialEq, Eq)]
enum StartupWorkspacePlan {
    Explicit(PathBuf),
    RestoreThenDefault {
        restored: Option<PathBuf>,
        default: PathBuf,
    },
}

fn startup_workspace_plan(
    explicit: Option<PathBuf>,
    restored: Option<PathBuf>,
    default: PathBuf,
) -> StartupWorkspacePlan {
    match explicit {
        Some(path) => StartupWorkspacePlan::Explicit(path),
        None => StartupWorkspacePlan::RestoreThenDefault { restored, default },
    }
}

fn restorable_local_workspace_path(
    workspace_kind: workspace::WorkspaceKind,
    root_path: PathBuf,
) -> Option<PathBuf> {
    (workspace_kind != workspace::WorkspaceKind::Remote).then_some(root_path)
}

fn default_assistant_workspace_path(
    path_manager: &bitfun_core::infrastructure::PathManager,
) -> PathBuf {
    prefer_current_assistant_workspace(
        path_manager.default_assistant_workspace_dir(None),
        path_manager.legacy_default_assistant_workspace_dir(None),
    )
}

fn prefer_current_assistant_workspace(current: PathBuf, legacy: PathBuf) -> PathBuf {
    if !current.exists() && legacy.is_dir() {
        legacy
    } else {
        current
    }
}

async fn open_owned_workspace(
    coordinator: &coordination::ConversationCoordinator,
    workspace_service: &workspace::WorkspaceService,
    path: PathBuf,
    snapshot_log_context: &str,
) -> anyhow::Result<workspace::WorkspaceInfo> {
    let path_label = path.display().to_string();
    coordinator
        .open_local_workspace_with_runtime_ownership(workspace_service, path, snapshot_log_context)
        .await
        .map_err(|error| anyhow::anyhow!("Failed to open Server workspace '{path_label}': {error}"))
}

async fn open_default_assistant_workspace(
    coordinator: &coordination::ConversationCoordinator,
    workspace_service: &workspace::WorkspaceService,
    path: PathBuf,
) -> anyhow::Result<workspace::WorkspaceInfo> {
    let path_label = path.display().to_string();
    coordinator
        .create_and_open_managed_local_workspace_with_runtime_ownership(
            workspace_service,
            path,
            "default Assistant Server bootstrap",
        )
        .await
        .map_err(|error| {
            anyhow::anyhow!(
                "Failed to open default Assistant Server workspace '{path_label}': {error}"
            )
        })
}

/// Shared application state for the server (mirrors Desktop's AppState).
///
/// Several fields are stored to keep the corresponding services alive (they
/// register global singletons during `initialize`), not because they are read
/// again after initialization.
#[allow(dead_code)]
pub(crate) struct ServerAppState {
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
/// Opens an explicit `workspace` as an authoritative request. Without one,
/// history is advisory and falls back to the default Assistant workspace when
/// its ownership-aware open fails.
pub(crate) async fn initialize(workspace: Option<String>) -> anyhow::Result<Arc<ServerAppState>> {
    log::info!("Initializing BitFun server core services");

    // 1. Global config
    config::initialize_global_config().await?;
    let config_service = config::get_global_config_service().await?;

    // Initialize the global I18nService so server-mode bot/remote-connect
    // consumers observe the same runtime locale lifecycle as Desktop.
    if let Err(e) =
        bitfun_core::service::i18n::initialize_global_i18n_service(Some(config_service.clone()))
            .await
    {
        log::warn!(
            "Failed to initialize global I18nService in server mode: {}",
            e
        );
    }

    // 2. AI client factory
    AIClientFactory::initialize_global().await?;
    let ai_client_factory = AIClientFactory::get_global().await?;

    // 3. Agentic system
    let path_manager = try_get_path_manager_arc()?;
    let runtime_ownership = Arc::new(
        bitfun_core::runtime_ownership::CoreRuntimeOwnership::embedded(
            path_manager.as_ref(),
            "server",
        ),
    );

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
        execution::ExecutionEngineConfig::default(),
    ));

    let coordinator = Arc::new(coordination::ConversationCoordinator::new(
        session_manager.clone(),
        execution_engine,
        tool_pipeline,
        event_queue.clone(),
        event_router.clone(),
        runtime_ownership,
    ));
    coordinator.set_terminal_port(
        bitfun_core::product_runtime::CoreRuntimeServicesProvider::terminal_port(),
    );
    coordinator.set_remote_exec_port(
        bitfun_core::product_runtime::CoreRuntimeServicesProvider::remote_exec_port(),
    );

    coordination::ConversationCoordinator::set_global(coordinator.clone());

    // Token usage
    let token_usage_service =
        Arc::new(token_usage::TokenUsageService::new(path_manager.clone()).await?);
    let token_usage_subscriber = Arc::new(token_usage::TokenUsageSubscriber::new(
        token_usage_service.clone(),
    ));
    event_router.subscribe_internal("token_usage".to_string(), token_usage_subscriber);
    event_router.subscribe_internal(
        "thread_goal_tokens".to_string(),
        Arc::new(bitfun_core::agentic::goal_mode::ThreadGoalTokenSubscriber),
    );

    // Dialog scheduler
    let scheduler =
        coordination::DialogScheduler::new(coordinator.clone(), session_manager.clone());
    coordinator.set_scheduler_notifier(scheduler.outcome_sender());
    coordinator.set_round_injection_source(scheduler.round_injection_monitor());
    coordination::set_global_scheduler(scheduler.clone());

    // Function agents
    let _ = bitfun_core::function_agents::git_func_agent::GitFunctionAgent::new(
        ai_client_factory.clone(),
    );
    let _ = bitfun_core::function_agents::startchat_func_agent::StartchatFunctionAgent::new(
        ai_client_factory.clone(),
    );

    // 4. Services
    let workspace_service =
        Arc::new(workspace::WorkspaceService::new_with_deferred_workspace_preparation().await?);
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

    // 5. Defer all restored-workspace preparation until Runtime ownership is
    // held. An explicit path is authoritative and fails closed. Persisted
    // history is only a startup hint; if it is stale or cannot acquire
    // ownership, use the product-owned default Assistant workspace instead.
    let restored_workspace = workspace_service.get_current_workspace().await.and_then(|workspace| {
        let kind = workspace.workspace_kind;
        let path = workspace.root_path;
        let restored = restorable_local_workspace_path(kind.clone(), path.clone());
        if restored.is_none() {
            log::warn!(
                "Skipping restored Remote workspace because the paused Server Host has no SSH manager; falling back to the default Assistant workspace: path={}",
                path.display()
            );
        }
        restored
    });
    let plan = startup_workspace_plan(
        workspace.map(PathBuf::from),
        restored_workspace,
        default_assistant_workspace_path(path_manager.as_ref()),
    );
    let workspace_info = match plan {
        StartupWorkspacePlan::Explicit(path) => {
            open_owned_workspace(
                coordinator.as_ref(),
                workspace_service.as_ref(),
                path,
                "explicit Server bootstrap",
            )
            .await?
        }
        StartupWorkspacePlan::RestoreThenDefault { restored, default } => match restored {
            Some(path) => match open_owned_workspace(
                coordinator.as_ref(),
                workspace_service.as_ref(),
                path,
                "restored Server bootstrap",
            )
            .await
            {
                Ok(info) => info,
                Err(error) => {
                    log::warn!(
                        "Failed to restore Server workspace; falling back to the default Assistant workspace: {}",
                        error
                    );
                    open_default_assistant_workspace(
                        coordinator.as_ref(),
                        workspace_service.as_ref(),
                        default,
                    )
                    .await?
                }
            },
            None => {
                open_default_assistant_workspace(
                    coordinator.as_ref(),
                    workspace_service.as_ref(),
                    default,
                )
                .await?
            }
        },
    };
    log::info!(
        "Workspace opened: name={}, path={}",
        workspace_info.name,
        workspace_info.root_path.display()
    );
    let initial_workspace_path = Some(workspace_info.root_path);

    // Construction loads and reconciles persisted jobs, so create, register,
    // and start Cron only after the Server has an owned workspace.
    let cron_service = bitfun_core::service::cron::CronService::new(
        path_manager.clone(),
        coordinator.clone(),
        scheduler.clone(),
    )
    .await?;
    bitfun_core::service::cron::set_global_cron_service(cron_service.clone());
    let cron_subscriber = Arc::new(bitfun_core::service::cron::CronEventSubscriber::new(
        cron_service.clone(),
    ));
    event_router.subscribe_internal("cron_jobs".to_string(), cron_subscriber);
    cron_service.start();

    // LSP
    if let Err(e) = bitfun_core::service::lsp::initialize_global_lsp_manager().await {
        log::error!("Failed to initialize LSP manager: {}", e);
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn explicit_workspace_plan_has_no_fallback() {
        let explicit = PathBuf::from("D:/requested-workspace");
        let default = PathBuf::from("D:/default-assistant");

        let plan = startup_workspace_plan(Some(explicit.clone()), None, default);

        assert_eq!(plan, StartupWorkspacePlan::Explicit(explicit));
    }

    #[test]
    fn implicit_workspace_plan_treats_history_as_advisory() {
        let restored = PathBuf::from("D:/restored-workspace");
        let default = PathBuf::from("D:/default-assistant");

        let plan = startup_workspace_plan(None, Some(restored.clone()), default.clone());

        assert_eq!(
            plan,
            StartupWorkspacePlan::RestoreThenDefault {
                restored: Some(restored),
                default,
            }
        );
    }

    #[test]
    fn remote_history_is_not_restored_without_a_server_ssh_manager() {
        let remote = PathBuf::from("/remote/workspace");
        let local = PathBuf::from("D:/local/workspace");

        assert_eq!(
            restorable_local_workspace_path(workspace::WorkspaceKind::Remote, remote),
            None
        );
        assert_eq!(
            restorable_local_workspace_path(workspace::WorkspaceKind::Normal, local.clone()),
            Some(local)
        );
    }

    #[test]
    fn legacy_default_assistant_workspace_remains_available_without_early_migration() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-server-legacy-workspace-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        let legacy = root.join("legacy-workspace");
        let current = root.join("personal-assistant").join("workspace");
        std::fs::create_dir_all(&legacy).expect("legacy Assistant workspace");

        assert_eq!(
            prefer_current_assistant_workspace(current.clone(), legacy.clone()),
            legacy
        );

        std::fs::create_dir_all(&current).expect("current Assistant workspace");
        assert_eq!(
            prefer_current_assistant_workspace(current.clone(), legacy),
            current
        );

        std::fs::remove_dir_all(root).expect("remove test workspace root");
    }

    #[test]
    fn default_assistant_creation_stays_inside_the_ownership_aware_coordinator() {
        let helper = include_str!("bootstrap.rs")
            .split("async fn open_default_assistant_workspace")
            .nth(1)
            .and_then(|source| source.split("pub(crate) struct ServerAppState").next())
            .expect("default Assistant workspace helper");

        assert!(helper.contains("create_and_open_managed_local_workspace_with_runtime_ownership"));
        assert!(!helper.contains("create_dir_all"));
    }

    #[test]
    fn cron_service_is_constructed_and_started_only_after_startup_workspace_is_owned() {
        let source = include_str!("bootstrap.rs");
        let workspace_ready = source
            .find("let initial_workspace_path = Some(workspace_info.root_path);")
            .expect("owned startup workspace marker");
        let cron_constructor = source
            .find("let cron_service = bitfun_core::service::cron::CronService::new(")
            .expect("Cron constructor");
        let cron_start = source
            .find("cron_service.start();")
            .expect("Cron runner start");

        assert!(
            workspace_ready < cron_constructor && cron_constructor < cron_start,
            "Cron reconciliation and execution must not run before startup workspace ownership succeeds"
        );
    }
}
