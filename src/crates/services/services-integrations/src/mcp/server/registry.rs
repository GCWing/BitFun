//! MCP server registry
//!
//! Manages registration and lookup for all MCP servers.

use super::{MCPRuntimeError, MCPRuntimeResult, MCPServerConfig, MCPServerProcess};
use log::info;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// MCP server registry.
pub struct MCPServerRegistry {
    servers: Arc<RwLock<HashMap<String, Arc<RwLock<MCPServerProcess>>>>>,
    runtime_configs: Arc<RwLock<HashMap<String, MCPServerConfig>>>,
    lifecycle_lock: Arc<Mutex<()>>,
}

impl MCPServerRegistry {
    /// Creates a new registry.
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            runtime_configs: Arc::new(RwLock::new(HashMap::new())),
            lifecycle_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Registers a server.
    pub async fn register(&self, config: &MCPServerConfig) -> MCPRuntimeResult<()> {
        self.register_new(config).await.map(|_| ())
    }

    /// Registers a server if it is not already present.
    ///
    /// Returns `true` when a new runtime process was inserted and `false` when
    /// the server was already registered.
    pub async fn ensure_registered(&self, config: &MCPServerConfig) -> MCPRuntimeResult<bool> {
        self.register_with_duplicate_policy(config, true).await
    }

    async fn register_new(&self, config: &MCPServerConfig) -> MCPRuntimeResult<bool> {
        self.register_with_duplicate_policy(config, false).await
    }

    async fn register_with_duplicate_policy(
        &self,
        config: &MCPServerConfig,
        allow_existing: bool,
    ) -> MCPRuntimeResult<bool> {
        config.validate().map_err(|error| {
            MCPRuntimeError::validation(format!("Invalid MCP server config: {}", error))
        })?;

        let _lifecycle_guard = self.lifecycle_lock.lock().await;
        {
            let servers = self.servers.read().await;
            if servers.contains_key(&config.id) {
                if allow_existing {
                    return Ok(false);
                }
                return Err(MCPRuntimeError::validation(format!(
                    "MCP server is already registered: {}",
                    config.id
                )));
            }
        }

        let process =
            MCPServerProcess::new(config.id.clone(), config.name.clone(), config.server_type);

        {
            let mut servers = self.servers.write().await;
            servers.insert(config.id.clone(), Arc::new(RwLock::new(process)));
        }

        info!(
            "Registered MCP server: name={} id={}",
            config.name, config.id
        );
        Ok(true)
    }

    /// Unregisters a server.
    pub async fn unregister(&self, server_id: &str) -> MCPRuntimeResult<()> {
        let _lifecycle_guard = self.lifecycle_lock.lock().await;
        let process = self.servers.read().await.get(server_id).cloned();

        if let Some(process) = process {
            let mut proc = process.write().await;
            proc.stop().await?;
            drop(proc);
            self.servers.write().await.remove(server_id);
            info!("Unregistered MCP server: id={}", server_id);
            Ok(())
        } else {
            Err(MCPRuntimeError::not_found(format!(
                "MCP server not found: {}",
                server_id
            )))
        }
    }

    /// Gets a server process.
    pub async fn get_process(&self, server_id: &str) -> Option<Arc<RwLock<MCPServerProcess>>> {
        let servers = self.servers.read().await;
        servers.get(server_id).cloned()
    }

    /// Returns all server IDs.
    pub async fn get_all_server_ids(&self) -> Vec<String> {
        let servers = self.servers.read().await;
        servers.keys().cloned().collect()
    }

    /// Returns all server processes.
    pub async fn get_all_processes(&self) -> Vec<Arc<RwLock<MCPServerProcess>>> {
        let servers = self.servers.read().await;
        servers.values().cloned().collect()
    }

    /// Returns whether a server exists.
    pub async fn contains(&self, server_id: &str) -> bool {
        let servers = self.servers.read().await;
        servers.contains_key(server_id)
    }

    /// Stores a runtime-only server configuration.
    pub async fn insert_runtime_config(&self, config: MCPServerConfig) -> MCPRuntimeResult<()> {
        config.validate().map_err(|error| {
            MCPRuntimeError::validation(format!("Invalid MCP server config: {}", error))
        })?;

        let mut configs = self.runtime_configs.write().await;
        configs.insert(config.id.clone(), config);
        Ok(())
    }

    /// Returns a runtime-only server configuration.
    pub async fn get_runtime_config(&self, server_id: &str) -> Option<MCPServerConfig> {
        let configs = self.runtime_configs.read().await;
        configs.get(server_id).cloned()
    }

    /// Removes a runtime-only server configuration.
    pub async fn remove_runtime_config(&self, server_id: &str) -> Option<MCPServerConfig> {
        let mut configs = self.runtime_configs.write().await;
        configs.remove(server_id)
    }

    /// Clears the registry.
    pub async fn clear(&self) -> MCPRuntimeResult<()> {
        let _lifecycle_guard = self.lifecycle_lock.lock().await;
        let processes = self
            .servers
            .read()
            .await
            .iter()
            .map(|(server_id, process)| (server_id.clone(), process.clone()))
            .collect::<Vec<_>>();
        let mut first_error = None;

        for (server_id, process) in processes {
            let mut proc = process.write().await;
            match proc.stop().await {
                Ok(()) => {
                    drop(proc);
                    self.servers.write().await.remove(&server_id);
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }

        if let Some(error) = first_error {
            return Err(error);
        }

        info!("Cleared MCP server registry");
        Ok(())
    }
}

impl Default for MCPServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::config::ConfigLocation;
    use crate::mcp::server::MCPServerType;
    use crate::mcp::MCPRuntimeErrorKind;
    use std::collections::HashMap;

    fn local_config(id: &str) -> MCPServerConfig {
        MCPServerConfig {
            id: id.to_string(),
            name: format!("{id} server"),
            server_type: MCPServerType::Local,
            transport: None,
            command: Some("node".to_string()),
            args: Vec::new(),
            env: HashMap::new(),
            working_directory: None,
            inherit_parent_environment: None,
            headers: HashMap::new(),
            url: None,
            auto_start: false,
            enabled: true,
            location: ConfigLocation::User,
            capabilities: Vec::new(),
            settings: HashMap::new(),
            oauth: None,
            oauth_enabled: None,
            xaa: None,
        }
    }

    #[tokio::test]
    async fn registry_registers_and_unregisters_processes_without_core_errors() {
        let registry = MCPServerRegistry::new();
        let config = local_config("test");

        registry.register(&config).await.unwrap();
        assert!(registry.contains("test").await);
        assert_eq!(
            registry.get_all_server_ids().await,
            vec!["test".to_string()]
        );
        assert!(registry.get_process("test").await.is_some());

        registry.unregister("test").await.unwrap();
        assert!(!registry.contains("test").await);
    }

    #[tokio::test]
    async fn failed_unregister_retains_process_ownership_for_retry() {
        let registry = MCPServerRegistry::new();
        let config = local_config("retryable-stop");
        registry.register(&config).await.unwrap();
        let process = registry
            .get_process(&config.id)
            .await
            .expect("registered process should exist");
        process.write().await.fail_next_stop_for_test();

        let error = registry
            .unregister(&config.id)
            .await
            .expect_err("injected stop failure must propagate");

        assert_eq!(error.kind(), MCPRuntimeErrorKind::Process);
        assert!(registry.contains(&config.id).await);
        registry
            .unregister(&config.id)
            .await
            .expect("retained process should be retryable");
        assert!(!registry.contains(&config.id).await);
    }

    #[tokio::test]
    async fn failed_clear_retains_only_processes_that_still_need_cleanup() {
        let registry = MCPServerRegistry::new();
        let retryable = local_config("retryable-clear");
        let stoppable = local_config("stoppable-clear");
        registry.register(&retryable).await.unwrap();
        registry.register(&stoppable).await.unwrap();
        registry
            .get_process(&retryable.id)
            .await
            .expect("retryable process")
            .write()
            .await
            .fail_next_stop_for_test();

        let error = registry
            .clear()
            .await
            .expect_err("one failed stop must fail registry clear");

        assert_eq!(error.kind(), MCPRuntimeErrorKind::Process);
        assert!(registry.contains(&retryable.id).await);
        assert!(!registry.contains(&stoppable.id).await);
        registry
            .clear()
            .await
            .expect("retained process should retry");
        assert!(registry.get_all_server_ids().await.is_empty());
    }

    #[tokio::test]
    async fn registry_reports_validation_and_missing_errors_as_runtime_errors() {
        let registry = MCPServerRegistry::new();
        let mut invalid = local_config("");
        invalid.command = None;

        let validation = registry.register(&invalid).await.unwrap_err();
        assert_eq!(validation.kind(), MCPRuntimeErrorKind::Validation);

        let missing = registry.unregister("missing").await.unwrap_err();
        assert_eq!(missing.kind(), MCPRuntimeErrorKind::NotFound);
    }

    #[tokio::test]
    async fn registry_rejects_duplicate_register_without_replacing_process() {
        let registry = MCPServerRegistry::new();
        let config = local_config("duplicate");

        registry.register(&config).await.unwrap();
        let first_process = registry
            .get_process("duplicate")
            .await
            .expect("first process");

        let duplicate = registry.register(&config).await.unwrap_err();

        assert_eq!(duplicate.kind(), MCPRuntimeErrorKind::Validation);
        let current_process = registry
            .get_process("duplicate")
            .await
            .expect("process should remain registered");
        assert!(Arc::ptr_eq(&first_process, &current_process));
    }

    #[tokio::test]
    async fn registry_can_ensure_existing_registration_without_replacing_process() {
        let registry = MCPServerRegistry::new();
        let config = local_config("ensure");

        assert!(registry.ensure_registered(&config).await.unwrap());
        let first_process = registry.get_process("ensure").await.expect("first process");

        assert!(!registry.ensure_registered(&config).await.unwrap());
        let current_process = registry
            .get_process("ensure")
            .await
            .expect("process should remain registered");
        assert!(Arc::ptr_eq(&first_process, &current_process));
    }

    #[tokio::test]
    async fn registry_owns_runtime_only_config_overlay() {
        let registry = MCPServerRegistry::new();
        let config = local_config("runtime-only");

        registry
            .insert_runtime_config(config.clone())
            .await
            .unwrap();
        assert_eq!(
            registry
                .get_runtime_config("runtime-only")
                .await
                .unwrap()
                .id,
            config.id
        );

        registry
            .remove_runtime_config("runtime-only")
            .await
            .unwrap();
        assert!(registry.get_runtime_config("runtime-only").await.is_none());
    }
}
