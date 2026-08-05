#![cfg(target_env = "ohos")]

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use bitfun_services_integrations::feedback::FeedbackCredentialStore;
use serde::{Deserialize, Serialize};

const ARKTS_FUNCTION: &str = "feedback_secure_credentials";

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum CredentialAction {
    Load,
    Store,
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

pub struct OhosFeedbackCredentialStore;

impl OhosFeedbackCredentialStore {
    pub fn new() -> Self {
        Self
    }

    async fn call(&self, request: CredentialRequest<'_>) -> Result<CredentialResponse> {
        let input = serde_json::to_string(&request).context("encode secure credential request")?;
        let output = bitfun_core::util::call_arkts_string_function(ARKTS_FUNCTION, input)
            .await
            .map_err(|error| anyhow!("call OpenHarmony secure credential store: {error}"))?;
        serde_json::from_str(&output).context("decode secure credential response")
    }
}

#[async_trait]
impl FeedbackCredentialStore for OhosFeedbackCredentialStore {
    async fn load(&self) -> Result<Option<String>> {
        let response = self
            .call(CredentialRequest {
                action: CredentialAction::Load,
                value: None,
            })
            .await?;
        match response.status.as_str() {
            "ok" => response
                .value
                .map(Some)
                .ok_or_else(|| anyhow!("secure credential response omitted its value")),
            "not_found" => Ok(None),
            _ => Err(anyhow!(
                "OpenHarmony secure credential load failed: code={}",
                response.code.as_deref().unwrap_or("UNKNOWN")
            )),
        }
    }

    async fn store(&self, value: &str) -> Result<()> {
        let response = self
            .call(CredentialRequest {
                action: CredentialAction::Store,
                value: Some(value),
            })
            .await?;
        if response.status == "ok" {
            return Ok(());
        }
        Err(anyhow!(
            "OpenHarmony secure credential store failed: code={}",
            response.code.as_deref().unwrap_or("UNKNOWN")
        ))
    }
}
