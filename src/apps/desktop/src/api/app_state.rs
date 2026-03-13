//! Application state management

use bitfun_core::agentic::{agents, tools};
use bitfun_core::agentic::side_question::SideQuestionRuntime;
use bitfun_core::infrastructure::ai::{AIClient, AIClientFactory};
use bitfun_core::miniapp::{initialize_global_miniapp_manager, JsWorkerPool, MiniAppManager};
use bitfun_core::service::{ai_rules, config, filesystem, mcp, token_usage, workspace};
use bitfun_core::util::errors::*;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub message: String,
    pub services: HashMap<String, bool>,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStatistics {
    pub sessions_created: u64,
    pub messages_processed: u64,
    pub tools_executed: u64,
    pub uptime_seconds: u64,
}

pub struct AppState {
    pub ai_client: Arc<RwLock<Option<AIClient>>>,
    pub ai_client_factory: Arc<AIClientFactory>,
    pub side_question_runtime: Arc<SideQuestionRuntime>,
    pub tool_registry: Arc<Vec<Arc<dyn tools::framework::Tool>>>,
    pub workspace_service: Arc<workspace::WorkspaceService>,
    pub workspace_identity_watch_service: Arc<workspace::WorkspaceIdentityWatchService>,
    pub workspace_path: Arc<RwLock<Option<std::path::PathBuf>>>,
    pub config_service: Arc<config::ConfigService>,
    pub filesystem_service: Arc<filesystem::FileSystemService>,
    pub ai_rules_service: Arc<ai_rules::AIRulesService>,
    pub agent_registry: Arc<agents::AgentRegistry>,
    pub mcp_service: Option<Arc<mcp::MCPService>>,
    pub token_usage_service: Arc<token_usage::TokenUsageService>,
    pub miniapp_manager: Arc<MiniAppManager>,
    pub js_worker_pool: Option<Arc<JsWorkerPool>>,
    pub statistics: Arc<RwLock<AppStatistics>>,
    pub start_time: std::time::Instant,
}

impl AppState {
    pub async fn new_async(token_usage_service: Arc<token_usage::TokenUsageService>) -> BitFunResult<Self> {
        let start_time = std::time::Instant::now();

        let config_service = config::get_global_config_service().await.map_err(|e| {
            BitFunError::config(format!("Failed to get global config service: {}", e))
        })?;

        let ai_client = Arc::new(RwLock::new(None));
        let ai_client_factory = AIClientFactory::get_global().await.map_err(|e| {
            BitFunError::service(format!("Failed to get global AIClientFactory: {}", e))
        })?;
        let side_question_runtime = Arc::new(SideQuestionRuntime::new());

        let tool_registry = {
            let registry = tools::registry::get_global_tool_registry();
            let lock = registry.read().await;
            Arc::new(lock.get_all_tools())
        };

        let workspace_service = Arc::new(workspace::WorkspaceService::new().await?);
        let workspace_identity_watch_service =
            Arc::new(workspace::WorkspaceIdentityWatchService::new(workspace_service.clone()));
        workspace::set_global_workspace_service(workspace_service.clone());
        let filesystem_service = Arc::new(filesystem::FileSystemServiceFactory::create_default());

        ai_rules::initialize_global_ai_rules_service()
            .await
            .map_err(|e| {
                BitFunError::service(format!("Failed to initialize AI rules service: {}", e))
            })?;
        let ai_rules_service = ai_rules::get_global_ai_rules_service()
            .await
            .map_err(|e| BitFunError::service(format!("Failed to get AI rules service: {}", e)))?;

        let agent_registry = agents::get_agent_registry();

        let mcp_service = match mcp::MCPService::new(config_service.clone()) {
            Ok(service) => {
                log::info!("MCP service initialized successfully");
                Some(Arc::new(service))
            }
            Err(e) => {
                log::warn!("Failed to initialize MCP service: {}", e);
                None
            }
        };
        let path_manager = workspace_service.path_manager().clone();
        let miniapp_manager = Arc::new(MiniAppManager::new(path_manager.clone()));
        initialize_global_miniapp_manager(miniapp_manager.clone());

        let worker_host_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("worker_host.js");
        let js_worker_pool = JsWorkerPool::new(path_manager, worker_host_path)
            .ok()
            .map(Arc::new);
        if js_worker_pool.is_none() {
            log::warn!("JsWorkerPool not initialized (missing worker_host.js or no Bun/Node)");
        }

        let statistics = Arc::new(RwLock::new(AppStatistics {
            sessions_created: 0,
            messages_processed: 0,
            tools_executed: 0,
            uptime_seconds: 0,
        }));

        let initial_workspace_path = workspace_service
            .get_current_workspace()
            .await
            .map(|workspace| workspace.root_path);

        if let Some(workspace_path) = initial_workspace_path.clone() {
            if let Err(e) = bitfun_core::service::snapshot::initialize_snapshot_manager_for_workspace(
                workspace_path.clone(),
                None,
            )
            .await
            {
                log::warn!(
                    "Failed to restore snapshot system on startup: path={}, error={}",
                    workspace_path.display(),
                    e
                );
            }
            if let Err(e) = ai_rules_service.set_workspace(workspace_path).await {
                log::warn!("Failed to restore AI rules workspace on startup: {}", e);
            }
        }

        let app_state = Self {
            ai_client,
            ai_client_factory,
            side_question_runtime,
            tool_registry,
            workspace_service,
            workspace_identity_watch_service,
            workspace_path: Arc::new(RwLock::new(initial_workspace_path)),
            config_service,
            filesystem_service,
            ai_rules_service,
            agent_registry,
            mcp_service,
            token_usage_service,
            miniapp_manager,
            js_worker_pool,
            statistics,
            start_time,
        };

        log::info!("AppState initialized successfully");
        Ok(app_state)
    }

    pub async fn get_health_status(&self) -> HealthStatus {
        let mut services = HashMap::new();
        services.insert(
            "ai_client".to_string(),
            self.ai_client.read().await.is_some(),
        );
        services.insert("workspace_service".to_string(), true);
        services.insert("config_service".to_string(), true);
        services.insert("filesystem_service".to_string(), true);

        let all_healthy = services.values().all(|&status| status);

        HealthStatus {
            status: if all_healthy {
                "healthy".to_string()
            } else {
                "degraded".to_string()
            },
            message: if all_healthy {
                "All services are running normally".to_string()
            } else {
                "Some services are unavailable".to_string()
            },
            services,
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }

    pub async fn get_statistics(&self) -> AppStatistics {
        let mut stats = self.statistics.read().await.clone();
        stats.uptime_seconds = self.start_time.elapsed().as_secs();
        stats
    }

    pub fn get_tool_names(&self) -> Vec<String> {
        self.tool_registry
            .iter()
            .map(|tool| tool.name().to_string())
            .collect()
    }
}
