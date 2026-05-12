use bitfun_services_integrations::mcp::config::{
    get_mcp_remote_authorization_source, get_mcp_remote_authorization_value,
    has_mcp_remote_authorization, has_mcp_remote_oauth, has_mcp_remote_xaa,
    merge_mcp_server_config_sources, normalize_mcp_authorization_value, parse_mcp_config_array,
    remove_mcp_authorization_keys,
};
use log::{info, warn};
use std::sync::Arc;

use crate::service::config::ConfigService;
use crate::service::mcp::server::MCPServerConfig;
use crate::util::errors::{BitFunError, BitFunResult};

use super::ConfigLocation;

/// MCP configuration service.
pub struct MCPConfigService {
    pub(super) config_service: Arc<ConfigService>,
}

impl MCPConfigService {
    fn parse_config_array(
        &self,
        servers: &[serde_json::Value],
        location: ConfigLocation,
    ) -> Vec<MCPServerConfig> {
        parse_mcp_config_array(servers, location)
    }

    fn normalize_authorization_value(value: &str) -> Option<String> {
        normalize_mcp_authorization_value(value)
    }

    fn remove_authorization_keys(map: &mut std::collections::HashMap<String, String>) {
        remove_mcp_authorization_keys(map);
    }

    pub fn get_remote_authorization_value(config: &MCPServerConfig) -> Option<String> {
        get_mcp_remote_authorization_value(config)
    }

    pub fn get_remote_authorization_source(config: &MCPServerConfig) -> Option<&'static str> {
        get_mcp_remote_authorization_source(config)
    }

    pub fn has_remote_authorization(config: &MCPServerConfig) -> bool {
        has_mcp_remote_authorization(config)
    }

    pub fn has_remote_oauth(config: &MCPServerConfig) -> bool {
        has_mcp_remote_oauth(config)
    }

    pub fn has_remote_xaa(config: &MCPServerConfig) -> bool {
        has_mcp_remote_xaa(config)
    }

    /// Creates a new MCP configuration service.
    pub fn new(config_service: Arc<ConfigService>) -> BitFunResult<Self> {
        Ok(Self { config_service })
    }

    /// Loads all MCP server configurations.
    pub async fn load_all_configs(&self) -> BitFunResult<Vec<MCPServerConfig>> {
        let builtin_configs = self.load_builtin_configs().await?;
        let user_configs = match self.load_user_configs().await {
            Ok(user_configs) => user_configs,
            Err(e) => {
                warn!("Failed to load user-level MCP configs: {}", e);
                Vec::new()
            }
        };

        let project_configs = match self.load_project_configs().await {
            Ok(project_configs) => project_configs,
            Err(e) => {
                warn!("Failed to load project-level MCP configs: {}", e);
                Vec::new()
            }
        };

        Ok(merge_mcp_server_config_sources([
            builtin_configs,
            user_configs,
            project_configs,
        ]))
    }

    /// Loads built-in configurations.
    async fn load_builtin_configs(&self) -> BitFunResult<Vec<MCPServerConfig>> {
        Ok(Vec::new())
    }

    /// Loads user-level configuration (supports Cursor format `{ "mcpServers": { "id": {..} } }`
    /// and array format `[{..}]`).
    async fn load_user_configs(&self) -> BitFunResult<Vec<MCPServerConfig>> {
        match self
            .config_service
            .get_config::<serde_json::Value>(Some("mcp_servers"))
            .await
        {
            Ok(config_value) => {
                if config_value
                    .get("mcpServers")
                    .and_then(|v| v.as_object())
                    .is_some()
                {
                    return super::cursor_format::parse_cursor_format(&config_value);
                }

                if let Some(servers) = config_value.as_array() {
                    return Ok(self.parse_config_array(servers, ConfigLocation::User));
                }

                warn!("Invalid MCP config format, returning empty list");
                Ok(Vec::new())
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Loads project-level configuration.
    async fn load_project_configs(&self) -> BitFunResult<Vec<MCPServerConfig>> {
        match self
            .config_service
            .get_config::<serde_json::Value>(Some("project.mcp_servers"))
            .await
        {
            Ok(config_value) => {
                if config_value
                    .get("mcpServers")
                    .and_then(|v| v.as_object())
                    .is_some()
                {
                    let mut configs = super::cursor_format::parse_cursor_format(&config_value)?;
                    for config in &mut configs {
                        config.location = ConfigLocation::Project;
                    }
                    return Ok(configs);
                }

                if let Some(servers) = config_value.as_array() {
                    Ok(self.parse_config_array(servers, ConfigLocation::Project))
                } else {
                    Ok(Vec::new())
                }
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Gets a single server configuration.
    pub async fn get_server_config(
        &self,
        server_id: &str,
    ) -> BitFunResult<Option<MCPServerConfig>> {
        let all_configs = self.load_all_configs().await?;
        Ok(all_configs.into_iter().find(|c| c.id == server_id))
    }

    /// Saves a server configuration.
    pub async fn save_server_config(&self, config: &MCPServerConfig) -> BitFunResult<()> {
        match config.location {
            ConfigLocation::BuiltIn => Err(BitFunError::Configuration(
                "Cannot modify built-in MCP server configuration".to_string(),
            )),
            ConfigLocation::User => self.save_user_config(config).await,
            ConfigLocation::Project => self.save_project_config(config).await,
        }
    }

    pub async fn set_remote_authorization(
        &self,
        server_id: &str,
        authorization_value: &str,
    ) -> BitFunResult<MCPServerConfig> {
        let mut config = self.get_server_config(server_id).await?.ok_or_else(|| {
            BitFunError::NotFound(format!("MCP server config not found: {}", server_id))
        })?;

        if config.server_type != crate::service::mcp::server::MCPServerType::Remote {
            return Err(BitFunError::Validation(format!(
                "MCP server '{}' is not a remote server",
                server_id
            )));
        }

        let normalized =
            Self::normalize_authorization_value(authorization_value).ok_or_else(|| {
                BitFunError::Validation("Authorization value cannot be empty".to_string())
            })?;

        Self::remove_authorization_keys(&mut config.headers);
        Self::remove_authorization_keys(&mut config.env);
        config
            .headers
            .insert("Authorization".to_string(), normalized);

        self.save_server_config(&config).await?;
        Ok(config)
    }

    pub async fn clear_remote_authorization(
        &self,
        server_id: &str,
    ) -> BitFunResult<MCPServerConfig> {
        let mut config = self.get_server_config(server_id).await?.ok_or_else(|| {
            BitFunError::NotFound(format!("MCP server config not found: {}", server_id))
        })?;

        if config.server_type != crate::service::mcp::server::MCPServerType::Remote {
            return Err(BitFunError::Validation(format!(
                "MCP server '{}' is not a remote server",
                server_id
            )));
        }

        Self::remove_authorization_keys(&mut config.headers);
        Self::remove_authorization_keys(&mut config.env);
        self.save_server_config(&config).await?;
        Ok(config)
    }

    /// Saves user-level configuration.
    async fn save_user_config(&self, config: &MCPServerConfig) -> BitFunResult<()> {
        let current_value = self
            .config_service
            .get_config::<serde_json::Value>(Some("mcp_servers"))
            .await
            .unwrap_or_else(|_| serde_json::json!({ "mcpServers": {} }));

        let mut mcp_servers =
            if let Some(obj) = current_value.get("mcpServers").and_then(|v| v.as_object()) {
                obj.clone()
            } else {
                serde_json::Map::new()
            };

        let cursor_format = super::cursor_format::config_to_cursor_format(config);

        mcp_servers.insert(config.id.clone(), cursor_format);

        let new_value = serde_json::json!({
            "mcpServers": mcp_servers
        });

        self.config_service
            .set_config("mcp_servers", new_value)
            .await?;
        info!(
            "Saved user-level MCP server config (Cursor format): {}",
            config.id
        );
        Ok(())
    }

    /// Saves project-level configuration.
    async fn save_project_config(&self, config: &MCPServerConfig) -> BitFunResult<()> {
        let mut configs = self.load_project_configs().await.unwrap_or_default();

        if let Some(existing) = configs.iter_mut().find(|c| c.id == config.id) {
            *existing = config.clone();
        } else {
            configs.push(config.clone());
        }

        let value = serde_json::to_value(&configs).map_err(|e| {
            BitFunError::serialization(format!("Failed to serialize MCP config: {}", e))
        })?;

        self.config_service
            .set_config("project.mcp_servers", value)
            .await?;
        Ok(())
    }

    /// Deletes a server configuration.
    pub async fn delete_server_config(&self, server_id: &str) -> BitFunResult<()> {
        let current_value = self
            .config_service
            .get_config::<serde_json::Value>(Some("mcp_servers"))
            .await
            .unwrap_or_else(|_| serde_json::json!({ "mcpServers": {} }));

        let mut mcp_servers =
            if let Some(obj) = current_value.get("mcpServers").and_then(|v| v.as_object()) {
                obj.clone()
            } else {
                return Err(BitFunError::NotFound(format!(
                    "MCP server config not found: {}",
                    server_id
                )));
            };

        if mcp_servers.remove(server_id).is_none() {
            return Err(BitFunError::NotFound(format!(
                "MCP server config not found: {}",
                server_id
            )));
        }

        let new_value = serde_json::json!({
            "mcpServers": mcp_servers
        });

        self.config_service
            .set_config("mcp_servers", new_value)
            .await?;
        info!("Deleted MCP server config: {}", server_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::mcp::server::MCPServerType;
    use std::collections::HashMap;

    fn make_config(
        id: &str,
        location: ConfigLocation,
        server_type: MCPServerType,
        command: Option<&str>,
        url: Option<&str>,
    ) -> MCPServerConfig {
        MCPServerConfig {
            id: id.to_string(),
            name: id.to_string(),
            server_type,
            transport: None,
            command: command.map(str::to_string),
            args: Vec::new(),
            env: HashMap::new(),
            headers: HashMap::new(),
            url: url.map(str::to_string),
            auto_start: true,
            enabled: true,
            location,
            capabilities: Vec::new(),
            settings: Default::default(),
            oauth: None,
            xaa: None,
        }
    }

    #[test]
    fn remote_authorization_prefers_headers_and_normalizes_tokens() {
        let mut config = make_config(
            "remote-auth",
            ConfigLocation::User,
            MCPServerType::Remote,
            None,
            Some("https://example.com/mcp"),
        );
        config
            .env
            .insert("Authorization".to_string(), "legacy-token".to_string());
        config.headers.insert(
            "Authorization".to_string(),
            "Bearer header-token".to_string(),
        );

        assert_eq!(
            MCPConfigService::get_remote_authorization_value(&config).as_deref(),
            Some("Bearer header-token")
        );
        assert_eq!(
            MCPConfigService::get_remote_authorization_source(&config),
            Some("headers")
        );
        assert_eq!(
            MCPConfigService::normalize_authorization_value("plain-token").as_deref(),
            Some("Bearer plain-token")
        );
    }
}
