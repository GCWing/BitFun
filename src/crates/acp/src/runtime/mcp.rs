use std::collections::HashMap;
use std::sync::Arc;

use agent_client_protocol::schema::{McpServer, McpServerSse, McpServerStdio};
use agent_client_protocol::{Error, Result};
use bitfun_core::service::config::get_global_config_service;
use bitfun_core::service::mcp::{
    get_global_mcp_service, set_global_mcp_service, ConfigLocation, MCPServerConfig,
    MCPServerManager, MCPServerTransport, MCPServerType, MCPService,
};

use super::BitfunAcpRuntime;

impl BitfunAcpRuntime {
    pub(super) async fn provision_mcp_servers(
        &self,
        acp_session_id: &str,
        servers: Vec<McpServer>,
    ) -> Result<Vec<String>> {
        if servers.is_empty() {
            return Ok(Vec::new());
        }

        let manager = mcp_server_manager().await?;
        let mut server_ids: Vec<String> = Vec::with_capacity(servers.len());

        for server in servers {
            let config = acp_mcp_server_config(acp_session_id, server)?;
            let server_id = config.id.clone();

            if let Err(error) = manager.add_ephemeral_server(config).await {
                for provisioned_id in &server_ids {
                    let _ = manager.remove_ephemeral_server(provisioned_id).await;
                }
                return Err(Self::internal_error(error));
            }

            server_ids.push(server_id);
        }

        Ok(server_ids)
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
        headers: HashMap::new(),
        url: None,
        auto_start: true,
        enabled: true,
        location: ConfigLocation::Project,
        capabilities: Vec::new(),
        settings: HashMap::new(),
        oauth: None,
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
        headers,
        url: Some(url),
        auto_start: true,
        enabled: true,
        location: ConfigLocation::Project,
        capabilities: Vec::new(),
        settings: HashMap::new(),
        oauth: None,
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
    format!(
        "acp-{}-{}",
        sanitize_id_part(acp_session_id),
        sanitize_id_part(server_name)
    )
}

fn sanitize_id_part(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    sanitized.trim_matches('-').to_string()
}
