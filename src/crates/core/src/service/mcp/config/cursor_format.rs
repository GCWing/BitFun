use log::warn;

use crate::service::mcp::server::{MCPServerConfig, MCPServerType};
use crate::util::errors::BitFunResult;

use super::ConfigLocation;

pub(super) fn config_to_cursor_format(config: &MCPServerConfig) -> serde_json::Value {
    let mut cursor_config = serde_json::Map::new();

    let type_str = match config.server_type {
        MCPServerType::Local | MCPServerType::Container => "stdio",
        MCPServerType::Remote => "sse",
    };
    cursor_config.insert("type".to_string(), serde_json::json!(type_str));

    if let Some(command) = &config.command {
        cursor_config.insert("command".to_string(), serde_json::json!(command));
    }

    if !config.args.is_empty() {
        cursor_config.insert("args".to_string(), serde_json::json!(config.args));
    }

    if !config.env.is_empty() {
        cursor_config.insert("env".to_string(), serde_json::json!(config.env));
    }

    if let Some(url) = &config.url {
        cursor_config.insert("url".to_string(), serde_json::json!(url));
    }

    serde_json::Value::Object(cursor_config)
}

pub(super) fn parse_cursor_format(
    config: &serde_json::Value,
) -> BitFunResult<Vec<MCPServerConfig>> {
    let mut servers = Vec::new();

    if let Some(mcp_servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
        for (server_id, server_config) in mcp_servers {
            if let Some(obj) = server_config.as_object() {
                let server_type = match obj.get("type").and_then(|v| v.as_str()) {
                    Some("stdio") => MCPServerType::Local,
                    Some("sse") => MCPServerType::Remote,
                    Some("remote") => MCPServerType::Remote,
                    Some("http") => MCPServerType::Remote,
                    Some("local") => MCPServerType::Local,
                    Some("container") => MCPServerType::Container,
                    _ => {
                        if obj.contains_key("url") {
                            MCPServerType::Remote
                        } else {
                            MCPServerType::Local
                        }
                    }
                };

                let command = obj
                    .get("command")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let args = obj
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                let env = obj
                    .get("env")
                    .and_then(|v| v.as_object())
                    .map(|env_obj| {
                        env_obj
                            .iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect::<std::collections::HashMap<_, _>>()
                    })
                    .unwrap_or_default();

                let url = obj
                    .get("url")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let server_config = MCPServerConfig {
                    id: server_id.clone(),
                    name: server_id.clone(),
                    server_type,
                    command,
                    args,
                    env,
                    url,
                    auto_start: true,
                    enabled: true,
                    location: ConfigLocation::User,
                    capabilities: Vec::new(),
                    settings: Default::default(),
                };

                servers.push(server_config);
            } else {
                warn!("Server config is not an object type: {}", server_id);
            }
        }
    }

    Ok(servers)
}
