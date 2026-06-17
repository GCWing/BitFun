//! Cursor-compatible MCP JSON config helpers.

use log::warn;

use crate::mcp::server::{MCPServerConfig, MCPServerTransport, MCPServerType};

use super::ConfigLocation;

fn parse_source(value: &str) -> Option<MCPServerType> {
    match value.trim() {
        "local" => Some(MCPServerType::Local),
        "remote" => Some(MCPServerType::Remote),
        _ => None,
    }
}

fn parse_transport(value: &str) -> Option<MCPServerTransport> {
    match value.trim() {
        "stdio" => Some(MCPServerTransport::Stdio),
        "sse" => Some(MCPServerTransport::Sse),
        "http" | "streamable_http" | "streamable-http" | "streamablehttp" => {
            Some(MCPServerTransport::StreamableHttp)
        }
        _ => None,
    }
}

fn parse_legacy_type(value: &str) -> Option<(Option<MCPServerType>, Option<MCPServerTransport>)> {
    match value.trim() {
        "stdio" => Some((None, Some(MCPServerTransport::Stdio))),
        "local" => Some((Some(MCPServerType::Local), Some(MCPServerTransport::Stdio))),
        "sse" => Some((Some(MCPServerType::Remote), Some(MCPServerTransport::Sse))),
        "remote" => Some((
            Some(MCPServerType::Remote),
            Some(MCPServerTransport::StreamableHttp),
        )),
        "http" | "streamable_http" | "streamable-http" | "streamablehttp" => Some((
            Some(MCPServerType::Remote),
            Some(MCPServerTransport::StreamableHttp),
        )),
        _ => None,
    }
}

pub fn config_to_cursor_format(config: &MCPServerConfig) -> serde_json::Value {
    let mut cursor_config = serde_json::Map::new();

    let type_str = match (config.server_type, config.resolved_transport()) {
        (MCPServerType::Local, _) => "stdio",
        (MCPServerType::Remote, MCPServerTransport::Sse) => "sse",
        (MCPServerType::Remote, MCPServerTransport::StreamableHttp) => "streamable-http",
        (MCPServerType::Remote, MCPServerTransport::Stdio) => "streamable-http",
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

    if let Some(oauth) = &config.oauth {
        cursor_config.insert("oauth".to_string(), serde_json::json!(oauth));
    }

    if let Some(xaa) = &config.xaa {
        cursor_config.insert("xaa".to_string(), serde_json::json!(xaa));
    }

    serde_json::Value::Object(cursor_config)
}

pub fn parse_cursor_format(config: &serde_json::Value) -> Vec<MCPServerConfig> {
    let mut servers = Vec::new();

    if let Some(mcp_servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
        for (server_id, server_config) in mcp_servers {
            if let Some(obj) = server_config.as_object() {
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

                let explicit_source_value = obj.get("source").and_then(|v| v.as_str());
                let explicit_source = match explicit_source_value {
                    Some(value) => match parse_source(value) {
                        Some(parsed) => Some(parsed),
                        None => {
                            warn!(
                                "Unsupported MCP source for server '{}': {}",
                                server_id, value
                            );
                            continue;
                        }
                    },
                    None => None,
                };
                let explicit_transport_value = obj.get("transport").and_then(|v| v.as_str());
                let explicit_transport = match explicit_transport_value {
                    Some(value) => match parse_transport(value) {
                        Some(parsed) => Some(parsed),
                        None => {
                            warn!(
                                "Unsupported MCP transport for server '{}': {}",
                                server_id, value
                            );
                            continue;
                        }
                    },
                    None => None,
                };
                let legacy_type_value = obj.get("type").and_then(|v| v.as_str());
                let legacy_type = match legacy_type_value {
                    Some(value) => match parse_legacy_type(value) {
                        Some(parsed) => Some(parsed),
                        None => {
                            warn!("Unsupported MCP type for server '{}': {}", server_id, value);
                            continue;
                        }
                    },
                    None => None,
                };

                let server_type = explicit_source
                    .or_else(|| legacy_type.and_then(|(source, _)| source))
                    .unwrap_or_else(|| {
                        if url.is_some() {
                            MCPServerType::Remote
                        } else {
                            MCPServerType::Local
                        }
                    });
                let transport = explicit_transport
                    .or_else(|| legacy_type.and_then(|(_, transport)| transport))
                    .unwrap_or(match server_type {
                        MCPServerType::Local => MCPServerTransport::Stdio,
                        MCPServerType::Remote => MCPServerTransport::StreamableHttp,
                    });

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
                    transport: Some(transport),
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
                    oauth: obj
                        .get("oauth")
                        .cloned()
                        .and_then(|value| serde_json::from_value(value).ok()),
                    xaa: obj
                        .get("xaa")
                        .cloned()
                        .and_then(|value| serde_json::from_value(value).ok()),
                };

                servers.push(server_config);
            } else {
                warn!("Server config is not an object type: {}", server_id);
            }
        }
    }

    servers
}
