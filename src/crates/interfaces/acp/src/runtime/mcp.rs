use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use agent_client_protocol::schema::{McpServer, McpServerSse, McpServerStdio};
use agent_client_protocol::{Error, Result};
use bitfun_core::service::config::get_global_config_service;
use bitfun_core::service::mcp::{
    get_global_mcp_service, set_global_mcp_service, ConfigLocation, MCPServerConfig,
    MCPServerManager, MCPServerTransport, MCPServerType, MCPService,
};
use sha2::{Digest, Sha256};

use super::BitfunAcpRuntime;

impl BitfunAcpRuntime {
    pub(super) fn validate_mcp_servers(&self, servers: &[McpServer]) -> Result<()> {
        let configs = acp_mcp_server_configs("validation", servers.iter().cloned())?;
        ensure_unique_server_ids(&configs)
    }

    pub(super) async fn provision_mcp_servers(
        &self,
        acp_session_id: &str,
        servers: Vec<McpServer>,
        cleanup_recovery_action: &'static str,
    ) -> Result<Vec<String>> {
        if servers.is_empty() {
            return Ok(Vec::new());
        }

        let manager = mcp_server_manager().await?;
        let configs = acp_mcp_server_configs(acp_session_id, servers)?;
        ensure_unique_server_ids(&configs)?;
        let mut server_ids: Vec<String> = Vec::with_capacity(configs.len());

        for config in configs {
            let server_id = config.id.clone();
            // Claim cleanup responsibility before startup. Registration can
            // succeed before handshake/start later fails, so the current ID
            // must participate in compensation as well as earlier servers.
            server_ids.push(server_id.clone());

            if let Err(error) = manager.add_ephemeral_server(config).await {
                if let Err(cleanup_error) = self.release_mcp_servers(&server_ids).await {
                    log::warn!(
                        "Failed to clean up ACP MCP servers after provisioning error: session_id={}, error={}",
                        acp_session_id,
                        cleanup_error
                    );
                    return Err(Self::cleanup_required_error(
                        acp_session_id,
                        "MCP provisioning",
                        &["ephemeralMcp"],
                        false,
                        cleanup_recovery_action,
                    ));
                }
                return Err(Self::internal_error(error));
            }
        }

        Ok(server_ids)
    }

    pub(super) async fn release_mcp_servers(&self, server_ids: &[String]) -> Result<()> {
        if server_ids.is_empty() {
            return Ok(());
        }

        let manager = mcp_server_manager().await?;
        let mut first_error = None;
        for server_id in server_ids {
            if let Err(error) = manager.remove_ephemeral_server(server_id).await {
                log::warn!(
                    "Failed to remove ephemeral ACP MCP server: server_id={}, error={}",
                    server_id,
                    error
                );
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }

        match first_error {
            Some(error) => Err(Self::internal_error(error)),
            None => Ok(()),
        }
    }
}

fn acp_mcp_server_configs(
    acp_session_id: &str,
    servers: impl IntoIterator<Item = McpServer>,
) -> Result<Vec<MCPServerConfig>> {
    servers
        .into_iter()
        .map(|server| acp_mcp_server_config(acp_session_id, server))
        .collect()
}

fn ensure_unique_server_ids(configs: &[MCPServerConfig]) -> Result<()> {
    let mut ids = HashSet::with_capacity(configs.len());
    if configs.iter().all(|config| ids.insert(config.id.clone())) {
        Ok(())
    } else {
        Err(Error::invalid_params().data("MCP server names must be unique within a session"))
    }
}

async fn mcp_server_manager() -> Result<Arc<MCPServerManager>> {
    if let Some(service) = get_global_mcp_service() {
        return Ok(service.server_manager());
    }

    let config_service = get_global_config_service()
        .await
        .map_err(BitfunAcpRuntime::internal_error)?;
    let service =
        Arc::new(MCPService::new(config_service).map_err(BitfunAcpRuntime::internal_error)?);
    set_global_mcp_service(service.clone());
    Ok(service.server_manager())
}

fn acp_mcp_server_config(acp_session_id: &str, server: McpServer) -> Result<MCPServerConfig> {
    match server {
        McpServer::Stdio(server) => stdio_server_config(acp_session_id, server),
        McpServer::Http(server) => remote_server_config(
            acp_session_id,
            server.name,
            server.url,
            header_map(server.headers),
            MCPServerTransport::StreamableHttp,
        ),
        McpServer::Sse(server) => sse_server_config(acp_session_id, server),
        _ => Err(Error::invalid_params().data("unsupported MCP server transport")),
    }
}

fn stdio_server_config(acp_session_id: &str, server: McpServerStdio) -> Result<MCPServerConfig> {
    let name = clean_server_name(&server.name)?;
    Ok(MCPServerConfig {
        id: ephemeral_server_id(acp_session_id, &name),
        name,
        server_type: MCPServerType::Local,
        transport: Some(MCPServerTransport::Stdio),
        command: Some(server.command.to_string_lossy().to_string()),
        args: server.args,
        env: server
            .env
            .into_iter()
            .map(|env| (env.name, env.value))
            .collect(),
        working_directory: None,
        inherit_parent_environment: None,
        headers: HashMap::new(),
        url: None,
        auto_start: true,
        enabled: true,
        location: ConfigLocation::Project,
        capabilities: Vec::new(),
        settings: HashMap::new(),
        oauth: None,
        oauth_enabled: None,
        xaa: None,
    })
}

fn sse_server_config(acp_session_id: &str, server: McpServerSse) -> Result<MCPServerConfig> {
    remote_server_config(
        acp_session_id,
        server.name,
        server.url,
        header_map(server.headers),
        MCPServerTransport::Sse,
    )
}

fn remote_server_config(
    acp_session_id: &str,
    name: String,
    url: String,
    headers: HashMap<String, String>,
    transport: MCPServerTransport,
) -> Result<MCPServerConfig> {
    let name = clean_server_name(&name)?;
    Ok(MCPServerConfig {
        id: ephemeral_server_id(acp_session_id, &name),
        name,
        server_type: MCPServerType::Remote,
        transport: Some(transport),
        command: None,
        args: Vec::new(),
        env: HashMap::new(),
        working_directory: None,
        inherit_parent_environment: None,
        headers,
        url: Some(url),
        auto_start: true,
        enabled: true,
        location: ConfigLocation::Project,
        capabilities: Vec::new(),
        settings: HashMap::new(),
        oauth: None,
        oauth_enabled: None,
        xaa: None,
    })
}

fn header_map(headers: Vec<agent_client_protocol::schema::HttpHeader>) -> HashMap<String, String> {
    headers
        .into_iter()
        .map(|header| (header.name, header.value))
        .collect()
}

fn clean_server_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(Error::invalid_params().data("MCP server name cannot be empty"));
    }
    Ok(trimmed.to_string())
}

fn ephemeral_server_id(acp_session_id: &str, server_name: &str) -> String {
    let mut digest = Sha256::new();
    for value in [acp_session_id, server_name] {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }
    format!("acp-{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::ephemeral_server_id;

    #[test]
    fn ephemeral_server_ids_do_not_collapse_distinct_session_or_server_names() {
        assert_ne!(
            ephemeral_server_id("foo:bar", "tools"),
            ephemeral_server_id("foo bar", "tools")
        );
        assert_ne!(
            ephemeral_server_id("session", "tools:read"),
            ephemeral_server_id("session", "tools read")
        );
    }
}
