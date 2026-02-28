use log::warn;

use crate::service::mcp::server::{MCPServerConfig, MCPServerType};
use crate::util::errors::BitFunResult;

use super::ConfigLocation;

pub(super) fn config_to_cursor_format(config: &MCPServerConfig) -> serde_json::Value {
    let mut cursor_config = serde_json::Map::new();

    let type_str = match config.server_type {
        MCPServerType::Local | MCPServerType::Container => "stdio",
        MCPServerType::Remote => "streamable-http",
    };
    cursor_config.insert("type".to_string(), serde_json::json!(type_str));

    if !config.name.is_empty() && config.name != config.id {
        cursor_config.insert("name".to_string(), serde_json::json!(config.name));
    }

    cursor_config.insert("enabled".to_string(), serde_json::json!(config.enabled));
    cursor_config.insert(
        "autoStart".to_string(),
        serde_json::json!(config.auto_start),
    );

    if let Some(command) = &config.command {
        cursor_config.insert("command".to_string(), serde_json::json!(command));
    }

    if !config.args.is_empty() {
        cursor_config.insert("args".to_string(), serde_json::json!(config.args));
    }

    if !config.env.is_empty() {
        cursor_config.insert("env".to_string(), serde_json::json!(config.env));
    }

    if !config.headers.is_empty() {
        cursor_config.insert("headers".to_string(), serde_json::json!(config.headers));
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
                    Some("streamable-http") => MCPServerType::Remote,
                    Some("streamable_http") => MCPServerType::Remote,
                    Some("streamablehttp") => MCPServerType::Remote,
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

                let headers = obj
                    .get("headers")
                    .and_then(|v| v.as_object())
                    .map(|headers_obj| {
                        headers_obj
                            .iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect::<std::collections::HashMap<_, _>>()
                    })
                    .unwrap_or_default();

                let url = obj
                    .get("url")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let name = obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| server_id.clone());

                let enabled = obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);

                let auto_start = obj
                    .get("autoStart")
                    .or_else(|| obj.get("auto_start"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let server_config = MCPServerConfig {
                    id: server_id.clone(),
                    name,
                    server_type,
                    command,
                    args,
                    env,
                    headers,
                    url,
                    auto_start,
                    enabled,
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
