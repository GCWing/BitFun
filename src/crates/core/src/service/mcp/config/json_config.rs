use log::{debug, error, info};

use crate::util::errors::{BitFunError, BitFunResult};

use super::service::MCPConfigService;

impl MCPConfigService {
    fn normalize_source(value: &str) -> Option<&'static str> {
        match value.trim() {
            "local" => Some("local"),
            "remote" => Some("remote"),
            _ => None,
        }
    }

    fn normalize_transport(value: &str) -> Option<&'static str> {
        match value.trim() {
            "stdio" => Some("stdio"),
            "sse" => Some("sse"),
            "http" | "streamable_http" | "streamable-http" | "streamablehttp" => {
                Some("streamable-http")
            }
            _ => None,
        }
    }

    fn normalize_legacy_type(value: &str) -> Option<(Option<&'static str>, Option<&'static str>)> {
        match value.trim() {
            "stdio" => Some((None, Some("stdio"))),
            "local" => Some((Some("local"), Some("stdio"))),
            "sse" => Some((Some("remote"), Some("sse"))),
            "remote" => Some((Some("remote"), Some("streamable-http"))),
            "http" | "streamable_http" | "streamable-http" | "streamablehttp" => {
                Some((Some("remote"), Some("streamable-http")))
            }
            _ => None,
        }
    }

    /// Loads MCP JSON config (Cursor format).
    pub async fn load_mcp_json_config(&self) -> BitFunResult<String> {
        match self
            .config_service
            .get_config::<serde_json::Value>(Some("mcp_servers"))
            .await
        {
            Ok(value) => {
                if value.get("mcpServers").is_some() {
                    return serde_json::to_string_pretty(&value).map_err(|e| {
                        BitFunError::serialization(format!("Failed to serialize MCP config: {}", e))
                    });
                }

                if let Some(servers) = value.as_array() {
                    let mut mcp_servers = serde_json::Map::new();
                    for server in servers {
                        if let Some(id) = server.get("id").and_then(|v| v.as_str()) {
                            mcp_servers.insert(id.to_string(), server.clone());
                        }
                    }
                    return Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "mcpServers": mcp_servers
                    }))?);
                }

                serde_json::to_string_pretty(&value).map_err(|e| {
                    BitFunError::serialization(format!("Failed to serialize MCP config: {}", e))
                })
            }
            Err(_) => Ok(serde_json::to_string_pretty(&serde_json::json!({
                "mcpServers": {}
            }))?),
        }
    }

    /// Saves MCP JSON config (Cursor format).
    pub async fn save_mcp_json_config(&self, json_config: &str) -> BitFunResult<()> {
        debug!("Saving MCP JSON config to app.json");

        let config_value: serde_json::Value = serde_json::from_str(json_config).map_err(|e| {
            let error_msg = format!("JSON parsing failed: {}. Please check JSON format", e);
            error!("{}", error_msg);
            BitFunError::validation(error_msg)
        })?;

        if config_value.get("mcpServers").is_none() {
            let error_msg = "Config missing 'mcpServers' field";
            error!("{}", error_msg);
            return Err(BitFunError::validation(error_msg.to_string()));
        }

        if config_value
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .is_none()
        {
            let error_msg = "'mcpServers' field must be an object";
            error!("{}", error_msg);
            return Err(BitFunError::validation(error_msg.to_string()));
        }

        if let Some(servers) = config_value.get("mcpServers").and_then(|v| v.as_object()) {
            for (server_id, server_config) in servers {
                if let Some(obj) = server_config.as_object() {
                    let type_str = obj
                        .get("type")
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty());
                    let source_str = obj
                        .get("source")
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty());
                    let transport_str = obj
                        .get("transport")
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty());

                    let command = obj
                        .get("command")
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty());

                    let url = obj
                        .get("url")
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty());

                    match (command.is_some(), url.is_some()) {
                        (true, true) => {
                            let error_msg = format!(
                                "Server '{}' must not set both 'command' and 'url' fields",
                                server_id
                            );
                            error!("{}", error_msg);
                            return Err(BitFunError::validation(error_msg));
                        }
                        (false, false) => {
                            let error_msg = format!(
                                "Server '{}' must provide either 'command' (stdio) or 'url' (streamable-http)",
                                server_id
                            );
                            error!("{}", error_msg);
                            return Err(BitFunError::validation(error_msg));
                        }
                        _ => {}
                    }

                    let legacy_type = match type_str {
                        Some(value) => Self::normalize_legacy_type(value).ok_or_else(|| {
                            BitFunError::validation(format!(
                                "Server '{}' has unsupported 'type' value: '{}'",
                                server_id, value
                            ))
                        })?,
                        None => (None, None),
                    };

                    let explicit_source = match source_str {
                        Some(value) => Some(Self::normalize_source(value).ok_or_else(|| {
                            BitFunError::validation(format!(
                                "Server '{}' has unsupported 'source' value: '{}'",
                                server_id, value
                            ))
                        })?),
                        None => legacy_type.0,
                    };
                    let explicit_transport = match transport_str {
                        Some(value) => Some(Self::normalize_transport(value).ok_or_else(|| {
                            BitFunError::validation(format!(
                                "Server '{}' has unsupported 'transport' value: '{}'",
                                server_id, value
                            ))
                        })?),
                        None => legacy_type.1,
                    };

                    let effective_source = match (command.is_some(), url.is_some()) {
                        (true, false) => match explicit_source {
                            Some("remote") => {
                                let error_msg = format!(
                                    "Server '{}' source='remote' conflicts with command-based configuration",
                                    server_id
                                );
                                error!("{}", error_msg);
                                return Err(BitFunError::validation(error_msg));
                            }
                            Some(source) => source,
                            None => "local",
                        },
                        (false, true) => match explicit_source {
                            Some("local") => {
                                let error_msg = format!(
                                    "Server '{}' source='{}' conflicts with url-based configuration",
                                    server_id,
                                    explicit_source.unwrap_or("unknown")
                                );
                                error!("{}", error_msg);
                                return Err(BitFunError::validation(error_msg));
                            }
                            Some(source) => source,
                            None => "remote",
                        },
                        _ => unreachable!(),
                    };

                    let effective_transport = match effective_source {
                        "local" => {
                            if let Some(transport) = explicit_transport {
                                if transport != "stdio" {
                                    let error_msg = format!(
                                        "Server '{}' source='{}' must use stdio transport",
                                        server_id, effective_source
                                    );
                                    error!("{}", error_msg);
                                    return Err(BitFunError::validation(error_msg));
                                }
                            }
                            "stdio"
                        }
                        "remote" => match explicit_transport.unwrap_or("streamable-http") {
                            "streamable-http" | "sse" => {
                                explicit_transport.unwrap_or("streamable-http")
                            }
                            _ => {
                                let error_msg = format!(
                                    "Server '{}' remote source must use 'streamable-http' or 'sse' transport",
                                    server_id
                                );
                                error!("{}", error_msg);
                                return Err(BitFunError::validation(error_msg));
                            }
                        },
                        _ => unreachable!(),
                    };

                    if effective_transport == "stdio" && command.is_none() {
                        let error_msg = format!(
                            "Server '{}' (stdio) must provide 'command' field",
                            server_id
                        );
                        error!("{}", error_msg);
                        return Err(BitFunError::validation(error_msg));
                    }

                    if (effective_transport == "streamable-http" || effective_transport == "sse")
                        && url.is_none()
                    {
                        let error_msg = format!(
                            "Server '{}' ({}) must provide 'url' field",
                            server_id, effective_transport
                        );
                        error!("{}", error_msg);
                        return Err(BitFunError::validation(error_msg));
                    }

                    if let Some(args) = obj.get("args") {
                        if !args.is_array() {
                            let error_msg =
                                format!("Server '{}' 'args' field must be an array", server_id);
                            error!("{}", error_msg);
                            return Err(BitFunError::validation(error_msg));
                        }
                    }

                    if let Some(env) = obj.get("env") {
                        if !env.is_object() {
                            let error_msg =
                                format!("Server '{}' 'env' field must be an object", server_id);
                            error!("{}", error_msg);
                            return Err(BitFunError::validation(error_msg));
                        }
                    }

                    if let Some(headers) = obj.get("headers") {
                        if !headers.is_object() {
                            let error_msg =
                                format!("Server '{}' 'headers' field must be an object", server_id);
                            error!("{}", error_msg);
                            return Err(BitFunError::validation(error_msg));
                        }
                    }

                    if let Some(oauth) = obj.get("oauth") {
                        if !oauth.is_object() {
                            let error_msg =
                                format!("Server '{}' 'oauth' field must be an object", server_id);
                            error!("{}", error_msg);
                            return Err(BitFunError::validation(error_msg));
                        }
                    }

                    if let Some(xaa) = obj.get("xaa") {
                        if !xaa.is_object() {
                            let error_msg =
                                format!("Server '{}' 'xaa' field must be an object", server_id);
                            error!("{}", error_msg);
                            return Err(BitFunError::validation(error_msg));
                        }
                    }
                } else {
                    let error_msg = format!("Server '{}' config must be an object", server_id);
                    error!("{}", error_msg);
                    return Err(BitFunError::validation(error_msg));
                }
            }
        }

        self.config_service
            .set_config("mcp_servers", config_value)
            .await
            .map_err(|e| {
                let error_msg = match e {
                    BitFunError::Io(ref io_err) => {
                        format!("Failed to write config file: {}", io_err)
                    }
                    BitFunError::Serialization(ref ser_err) => {
                        format!("Failed to serialize config: {}", ser_err)
                    }
                    _ => format!("Failed to save config: {}", e),
                };
                error!("{}", error_msg);
                BitFunError::config(error_msg)
            })?;

        info!("MCP config saved to app.json");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::MCPConfigService;

    #[test]
    fn normalize_legacy_type_rejects_container_and_preserves_sse() {
        assert_eq!(MCPConfigService::normalize_legacy_type("container"), None);
        assert_eq!(
            MCPConfigService::normalize_legacy_type("sse"),
            Some((Some("remote"), Some("sse")))
        );
    }
}
