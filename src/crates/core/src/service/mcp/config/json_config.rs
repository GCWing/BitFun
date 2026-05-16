use bitfun_services_integrations::mcp::config::{
    format_mcp_json_config_value, validate_mcp_json_config,
};
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
            Ok(value) => format_mcp_json_config_value(Some(&value)).map_err(|e| {
                BitFunError::serialization(format!("Failed to serialize MCP config: {}", e))
            }),
            Err(_) => format_mcp_json_config_value(None).map_err(|e| {
                BitFunError::serialization(format!("Failed to serialize MCP config: {}", e))
            }),
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

        validate_mcp_json_config(&config_value).map_err(|e| {
            let error_msg = e.to_string();
            error!("{}", error_msg);
            BitFunError::validation(error_msg)
        })?;

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
