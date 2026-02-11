use log::{debug, error, info};

use crate::util::errors::{BitFunError, BitFunResult};

use super::service::MCPConfigService;

impl MCPConfigService {
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

        if !config_value
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .is_some()
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

                    let inferred_transport = match (command.is_some(), url.is_some()) {
                        (true, true) => {
                            let error_msg = format!(
                                "Server '{}' must not set both 'command' and 'url' fields",
                                server_id
                            );
                            error!("{}", error_msg);
                            return Err(BitFunError::validation(error_msg));
                        }
                        (true, false) => "stdio",
                        (false, true) => "streamable-http",
                        (false, false) => {
                            let error_msg = format!(
                                "Server '{}' must provide either 'command' (stdio) or 'url' (streamable-http)",
                                server_id
                            );
                            error!("{}", error_msg);
                            return Err(BitFunError::validation(error_msg));
                        }
                    };

                    if let Some(t) = type_str {
                        let normalized_transport = match t {
                            "stdio" | "local" | "container" => "stdio",
                            "sse"
                            | "remote"
                            | "http"
                            | "streamable_http"
                            | "streamable-http"
                            | "streamablehttp" => "streamable-http",
                            _ => {
                                let error_msg = format!(
                                    "Server '{}' has unsupported 'type' value: '{}'",
                                    server_id, t
                                );
                                error!("{}", error_msg);
                                return Err(BitFunError::validation(error_msg));
                            }
                        };

                        if normalized_transport != inferred_transport {
                            let error_msg = format!(
                                "Server '{}' 'type' conflicts with provided fields (type='{}')",
                                server_id, t
                            );
                            error!("{}", error_msg);
                            return Err(BitFunError::validation(error_msg));
                        }
                    }

                    if inferred_transport == "stdio" && command.is_none() {
                        let error_msg = format!(
                            "Server '{}' (stdio) must provide 'command' field",
                            server_id
                        );
                        error!("{}", error_msg);
                        return Err(BitFunError::validation(error_msg));
                    }

                    if inferred_transport == "streamable-http" && url.is_none() {
                        let error_msg =
                            format!("Server '{}' (streamable-http) must provide 'url' field", server_id);
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
