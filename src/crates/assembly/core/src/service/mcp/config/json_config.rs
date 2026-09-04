use log::{debug, error, info};
use openbitfun_services_integrations::mcp::config::{validate_mcp_json_config, MCPImportError};
use serde::Serialize;

use crate::util::errors::{OpenBitFunError, OpenBitFunResult};

use super::service::MCPConfigService;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPJsonConfigSnapshot {
    pub json_config: String,
    pub fingerprint: String,
}

impl MCPConfigService {
    /// Loads MCP JSON config (Cursor format).
    pub async fn load_mcp_json_config(&self) -> OpenBitFunResult<MCPJsonConfigSnapshot> {
        let snapshot = self
            .inner
            .user_json_config_snapshot()
            .await
            .map_err(|error| {
                OpenBitFunError::config(format!("Failed to load MCP config: {error}"))
            })?;
        Ok(MCPJsonConfigSnapshot {
            json_config: snapshot.json_config,
            fingerprint: snapshot.fingerprint,
        })
    }

    /// Saves MCP JSON config (Cursor format).
    pub async fn save_mcp_json_config(
        &self,
        json_config: &str,
        expected_fingerprint: &str,
    ) -> OpenBitFunResult<()> {
        debug!("Saving MCP JSON config to app.json");

        let config_value: serde_json::Value = serde_json::from_str(json_config).map_err(|e| {
            let error_msg = format!("JSON parsing failed: {}. Please check JSON format", e);
            error!("{}", error_msg);
            OpenBitFunError::validation(error_msg)
        })?;

        validate_mcp_json_config(&config_value).map_err(|e| {
            let error_msg = e.to_string();
            error!("{}", error_msg);
            OpenBitFunError::validation(error_msg)
        })?;

        self.inner
            .replace_user_json_config(expected_fingerprint, config_value)
            .await
            .map_err(|e| {
                let error_msg = match e {
                    MCPImportError::StaleConfiguration => {
                        "MCP configuration changed; reload before saving".to_string()
                    }
                    _ => format!("Failed to save config: {e}"),
                };
                error!("{}", error_msg);
                OpenBitFunError::config(error_msg)
            })?;

        info!("MCP config saved to app.json");

        Ok(())
    }
}
