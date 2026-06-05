use crate::client::utils::{dedupe_remote_models, normalize_base_url_for_discovery};
use crate::client::AIClient;
use crate::types::RemoteModelInfo;
use anyhow::Result;
use serde::Deserialize;

use super::request::apply_headers;

#[derive(Debug, Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModelEntry>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModelEntry {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
}

pub(crate) fn resolve_models_url(client: &AIClient) -> String {
    let mut base = normalize_base_url_for_discovery(&client.config.base_url);

    if base.ends_with("/v1/messages") {
        base.truncate(base.len() - "/v1/messages".len());
        return format!("{}/v1/models", base);
    }

    if base.ends_with("/v1/models") {
        return base;
    }

    if base.ends_with("/v1") {
        return format!("{}/models", base);
    }

    if base.is_empty() {
        return "v1/models".to_string();
    }

    format!("{}/v1/models", base)
}

pub(crate) async fn list_models(client: &AIClient) -> Result<Vec<RemoteModelInfo>> {
    let url = resolve_models_url(client);
    let response = apply_headers(client, client.client.get(&url), &url)
        .send()
        .await?
        .error_for_status()?;

    let payload: AnthropicModelsResponse = response.json().await?;
    Ok(dedupe_remote_models(
        payload
            .data
            .into_iter()
            .map(|model| RemoteModelInfo {
                id: model.id,
                display_name: model.display_name,
            })
            .collect(),
    ))
}
