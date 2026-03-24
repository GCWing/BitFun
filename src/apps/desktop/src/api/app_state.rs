//! Application state management

use bitfun_core::agentic::side_question::SideQuestionRuntime;
use bitfun_core::agentic::{agents, tools};
use bitfun_core::infrastructure::ai::{AIClient, AIClientFactory};
use bitfun_core::miniapp::{initialize_global_miniapp_manager, JsWorkerPool, MiniAppManager};
use bitfun_core::service::{ai_rules, config, filesystem, mcp, token_usage, workspace};
use bitfun_core::service::remote_ssh::{
    init_remote_workspace_manager, SSHConnectionManager, RemoteFileService, RemoteTerminalManager,
};
use bitfun_core::util::errors::*;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// Errors that can occur when accessing SSH remote services
#[derive(Error, Debug)]
pub enum SSHServiceError {
    #[error("SSH manager not initialized")]
    ManagerNotInitialized,
    #[error("Remote file service not initialized")]
    FileServiceNotInitialized,
    #[error("Remote terminal manager not initialized")]
    TerminalManagerNotInitialized,
}

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

/// Remote workspace information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkspace {
    pub connection_id: String,
    pub connection_name: String,
    pub remote_path: String,
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
    pub macos_edit_menu_mode: Arc<RwLock<crate::macos_menubar::EditMenuMode>>,
    pub start_time: std::time::Instant,
    // SSH Remote connection state
    pub ssh_manager: Arc<RwLock<Option<SSHConnectionManager>>>,
    pub remote_file_service: Arc<RwLock<Option<RemoteFileService>>>,
    pub remote_terminal_manager: Arc<RwLock<Option<RemoteTerminalManager>>>,
    pub remote_workspace: Arc<RwLock<Option<RemoteWorkspace>>>,
}

impl AppState {
    pub async fn new_async(
        token_usage_service: Arc<token_usage::TokenUsageService>,
    ) -> BitFunResult<Self> {
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
        let workspace_identity_watch_service = Arc::new(
            workspace::WorkspaceIdentityWatchService::new(workspace_service.clone()),
        );
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
            if let Err(e) =
                bitfun_core::service::snapshot::initialize_snapshot_manager_for_workspace(
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

        // Initialize SSH Remote services synchronously so they're ready before app starts
        let ssh_data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("BitFun")
            .join("ssh");
        let ssh_manager = Arc::new(RwLock::new(None));
        let ssh_manager_clone = ssh_manager.clone();
        let remote_file_service = Arc::new(RwLock::new(None));
        let remote_file_service_clone = remote_file_service.clone();
        let remote_terminal_manager = Arc::new(RwLock::new(None));
        let remote_terminal_manager_clone = remote_terminal_manager.clone();
        // Create remote_workspace before spawn so we can pass it in
        let remote_workspace = Arc::new(RwLock::new(None));
        let remote_workspace_clone = remote_workspace.clone();

        // Initialize SSH services synchronously (not spawned) so they're ready before app starts
        let manager = SSHConnectionManager::new(ssh_data_dir.clone());
        if let Err(e) = manager.load_saved_connections().await {
            log::error!("Failed to load saved SSH connections: {}", e);
        } else {
            log::info!("SSH connections loaded successfully");
        }
        if let Err(e) = manager.load_known_hosts().await {
            log::error!("Failed to load known hosts: {}", e);
        }

        // Load persisted remote workspaces (may be multiple)
        match manager.load_remote_workspace().await {
            Ok(_) => {
                let workspaces = manager.get_remote_workspaces().await;
                if !workspaces.is_empty() {
                    log::info!("Loaded {} persisted remote workspace(s)", workspaces.len());
                    // Use the first one for the legacy single-workspace field
                    let first = &workspaces[0];
                    let app_workspace = RemoteWorkspace {
                        connection_id: first.connection_id.clone(),
                        remote_path: first.remote_path.clone(),
                        connection_name: first.connection_name.clone(),
                    };
                    *remote_workspace_clone.write().await = Some(app_workspace);
                }
            }
            Err(e) => {
                log::warn!("Failed to load remote workspace: {}", e);
            }
        }

        let manager_arc = Arc::new(manager);
        let manager_for_fs = Arc::new(tokio::sync::RwLock::new(Some(manager_arc.as_ref().clone())));
        let fs = RemoteFileService::new(manager_for_fs.clone());
        let tm = RemoteTerminalManager::new(manager_arc.as_ref().clone());

        // Clone for storing in AppState
        let fs_for_state = fs.clone();
        let tm_for_state = tm.clone();

        *ssh_manager_clone.write().await = Some((*manager_arc).clone());
        *remote_file_service_clone.write().await = Some(fs_for_state);
        *remote_terminal_manager_clone.write().await = Some(tm_for_state);

        // Note: We do NOT activate the global remote workspace state here because
        // there is no live SSH connection yet. The persisted workspace info is loaded
        // into self.remote_workspace so the frontend can query it via remote_get_workspace_info
        // and drive the reconnection flow. The global state will be activated when the
        // frontend successfully reconnects and calls remote_open_workspace → set_remote_workspace.

        log::info!("SSH Remote services initialized with SFTP, PTY, and known hosts support");

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
            macos_edit_menu_mode: Arc::new(RwLock::new(
                crate::macos_menubar::EditMenuMode::System,
            )),
            start_time,
            // SSH Remote connection state
            ssh_manager,
            remote_file_service,
            remote_terminal_manager,
            remote_workspace,
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

    // SSH Remote connection methods

    /// Get SSH connection manager synchronously (must be called within async context)
    pub async fn get_ssh_manager_async(&self) -> Result<SSHConnectionManager, SSHServiceError> {
        self.ssh_manager.read().await.clone()
            .ok_or(SSHServiceError::ManagerNotInitialized)
    }

    /// Get remote file service synchronously (must be called within async context)
    pub async fn get_remote_file_service_async(&self) -> Result<RemoteFileService, SSHServiceError> {
        self.remote_file_service.read().await.clone()
            .ok_or(SSHServiceError::FileServiceNotInitialized)
    }

    /// Get remote terminal manager synchronously (must be called within async context)
    pub async fn get_remote_terminal_manager_async(&self) -> Result<RemoteTerminalManager, SSHServiceError> {
        self.remote_terminal_manager.read().await.clone()
            .ok_or(SSHServiceError::TerminalManagerNotInitialized)
    }

    /// Set current remote workspace
    pub async fn set_remote_workspace(&self, workspace: RemoteWorkspace) -> Result<(), SSHServiceError> {
        // Update local state
        *self.remote_workspace.write().await = Some(workspace.clone());

        // Persist to SSHConnectionManager for restoration on restart
        if let Ok(manager) = self.get_ssh_manager_async().await {
            let core_workspace = bitfun_core::service::remote_ssh::RemoteWorkspace {
                connection_id: workspace.connection_id.clone(),
                remote_path: workspace.remote_path.clone(),
                connection_name: workspace.connection_name.clone(),
            };
            if let Err(e) = manager.set_remote_workspace(core_workspace).await {
                log::warn!("Failed to persist remote workspace: {}", e);
            }
        }

        // Register in the global workspace registry
        let state_manager = init_remote_workspace_manager();

        // Ensure shared services are set (idempotent if already set)
        let manager = self.get_ssh_manager_async().await?;
        let fs = self.get_remote_file_service_async().await?;
        let terminal = self.get_remote_terminal_manager_async().await?;

        state_manager.set_ssh_manager(manager.clone()).await;
        state_manager.set_file_service(fs.clone()).await;
        state_manager.set_terminal_manager(terminal.clone()).await;

        // Register this workspace (does not overwrite other workspaces)
        log::info!("register_remote_workspace: connection_id={}, remote_path={}, connection_name={}",
            workspace.connection_id, workspace.remote_path, workspace.connection_name);
        state_manager.register_remote_workspace(
            workspace.remote_path.clone(),
            workspace.connection_id.clone(),
            workspace.connection_name.clone(),
        ).await;
        state_manager
            .set_active_connection_hint(Some(workspace.connection_id.clone()))
            .await;
        log::info!("Remote workspace registered: {} on {}",
            workspace.remote_path, workspace.connection_name);
        Ok(())
    }

    /// Get current remote workspace
    pub async fn get_remote_workspace_async(&self) -> Option<RemoteWorkspace> {
        self.remote_workspace.read().await.clone()
    }

    /// Remove one remote workspace from persistence + registry (`connection_id` + `remote_path`).
    pub async fn unregister_remote_workspace_entry(&self, connection_id: &str, remote_path: &str) {
        let rp = bitfun_core::service::remote_ssh::normalize_remote_workspace_path(remote_path);
        if let Ok(manager) = self.get_ssh_manager_async().await {
            if let Err(e) = manager.remove_remote_workspace(connection_id, &rp).await {
                log::warn!("Failed to remove persisted remote workspace: {}", e);
            }
        }
        if let Some(state_manager) = bitfun_core::service::remote_ssh::get_remote_workspace_manager() {
            state_manager
                .unregister_remote_workspace(connection_id, &rp)
                .await;
        }
        let mut slot = self.remote_workspace.write().await;
        let clear_slot = slot
            .as_ref()
            .map(|w| {
                w.connection_id == connection_id
                    && bitfun_core::service::remote_ssh::normalize_remote_workspace_path(&w.remote_path)
                        == rp
            })
            .unwrap_or(false);
        if clear_slot {
            *slot = None;
            if let Some(m) = bitfun_core::service::remote_ssh::get_remote_workspace_manager() {
                m.set_active_connection_hint(None).await;
            }
        }
        log::info!(
            "Remote workspace entry removed: connection_id={}, remote_path={}",
            connection_id,
            rp
        );
    }

    /// Clear current remote pointer and remove its persisted/registry entry (legacy SSH "close").
    pub async fn clear_remote_workspace(&self) {
        let snap = { self.remote_workspace.read().await.clone() };
        if let Some(w) = snap {
            self.unregister_remote_workspace_entry(&w.connection_id, &w.remote_path)
                .await;
        }
    }

    /// Check if currently in a remote workspace
    pub async fn is_remote_workspace(&self) -> bool {
        self.remote_workspace.read().await.is_some()
    }
}