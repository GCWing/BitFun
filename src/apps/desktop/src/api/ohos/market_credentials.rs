#![cfg(target_env = "ohos")]

use async_trait::async_trait;
use bitfun_services_integrations::miniapp_market::{
    MarketCredentialStore, StoredMarketCredentials,
};
use serde::{Deserialize, Serialize};

const ARKTS_FUNCTION: &str = "market_secure_credentials";

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum CredentialAction {
    Load,
    Store,
    Clear,
}

#[derive(Debug, Serialize)]
struct CredentialRequest<'a> {
    action: CredentialAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct CredentialResponse {
    status: String,
    value: Option<String>,
    code: Option<String>,
}

#[derive(Debug)]
pub struct OhosMarketCredentialStore;

impl OhosMarketCredentialStore {
    pub fn new() -> Self {
        Self
    }

    async fn call(&self, request: CredentialRequest<'_>) -> Result<CredentialResponse, String> {
        let input = serde_json::to_string(&request)
            .map_err(|error| format!("encode market credential request: {error}"))?;
        let output = bitfun_core::util::call_arkts_string_function(ARKTS_FUNCTION, input)
            .await
            .map_err(|error| format!("call OpenHarmony market credential store: {error}"))?;
        serde_json::from_str(&output)
            .map_err(|error| format!("decode market credential response: {error}"))
    }
}

#[async_trait]
impl MarketCredentialStore for OhosMarketCredentialStore {
    async fn load(&self) -> Result<Option<StoredMarketCredentials>, String> {
        let response = self
            .call(CredentialRequest {
                action: CredentialAction::Load,
                value: None,
            })
            .await?;
        match response.status.as_str() {
            "ok" => response
                .value
                .ok_or_else(|| "market credential response omitted its value".to_string())
                .and_then(|value| {
                    serde_json::from_str(&value)
                        .map_err(|error| format!("parse market credentials: {error}"))
                })
                .map(Some),
            "not_found" => Ok(None),
            _ => Err(format!(
                "OpenHarmony market credential load failed: code={}",
                response.code.as_deref().unwrap_or("UNKNOWN")
            )),
        }
    }

    async fn save(&self, credentials: &StoredMarketCredentials) -> Result<(), String> {
        let value = serde_json::to_string(credentials)
            .map_err(|error| format!("serialize market credentials: {error}"))?;
        let response = self
            .call(CredentialRequest {
                action: CredentialAction::Store,
                value: Some(&value),
            })
            .await?;
        if response.status == "ok" {
            Ok(())
        } else {
            Err(format!(
                "OpenHarmony market credential store failed: code={}",
                response.code.as_deref().unwrap_or("UNKNOWN")
            ))
        }
    }

    async fn clear(&self) -> Result<(), String> {
        let response = self
            .call(CredentialRequest {
                action: CredentialAction::Clear,
                value: None,
            })
            .await?;
        if matches!(response.status.as_str(), "ok" | "not_found") {
            Ok(())
        } else {
            Err(format!(
                "OpenHarmony market credential delete failed: code={}",
                response.code.as_deref().unwrap_or("UNKNOWN")
            ))
        }
    }
}
