//! OpenCode account login, catalog discovery, and credential resolution.
//!
//! Uses the OAuth 2.0 Device Authorization Grant against
//! `opencode.ai/console`, aligned with OpenCode's `provider/opencode.ts`.
//! One OAuth identity can authenticate both the Zen and Go API products.

use super::store::{self, StoredCredential};
use super::{
    OpenCodePlan, ResolvedCredential, StartedLogin, SubscriptionApiOffering,
    SubscriptionHttpOptions, SubscriptionOfferingModel,
};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const SERVER: &str = "https://opencode.ai/console";
const CLIENT_ID: &str = "opencode-cli";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const ZEN_BASE_URL: &str = "https://opencode.ai/zen/v1";
const ZEN_REQUEST_URL: &str = "https://opencode.ai/zen/v1/chat/completions";
const ZEN_RESPONSES_URL: &str = "https://opencode.ai/zen/v1/responses";
const ZEN_MESSAGES_URL: &str = "https://opencode.ai/zen/v1/messages";
const GO_BASE_URL: &str = "https://opencode.ai/zen/go/v1";
const GO_REQUEST_URL: &str = "https://opencode.ai/zen/go/v1/chat/completions";
const GO_RESPONSES_URL: &str = "https://opencode.ai/zen/go/v1/responses";
const GO_MESSAGES_URL: &str = "https://opencode.ai/zen/go/v1/messages";
const DEFAULT_MODEL: &str = "gpt-5.4";
const REFRESH_LEEWAY_MS: i64 = 5 * 60 * 1000;
const STORE_KEY: &str = "opencode";
const OFFERINGS_METADATA_KEY: &str = "api_offerings";
const SUPPORTED_FORMATS: [&str; 3] = ["openai", "responses", "anthropic"];

struct FreshCredential {
    access: String,
    expires_at_ms: Option<i64>,
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri_complete: String,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct PendingResponse {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserResponse {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrgResponse {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RemoteConfigResponse {
    config: RemoteConfig,
}

#[derive(Debug, Default, Deserialize)]
struct RemoteConfig {
    #[serde(default)]
    provider: HashMap<String, RemoteProvider>,
}

#[derive(Debug, Default, Deserialize)]
struct RemoteProvider {
    #[serde(default)]
    npm: Option<String>,
    #[serde(default)]
    api: Option<String>,
    #[serde(default)]
    models: HashMap<String, RemoteModel>,
}

#[derive(Debug, Default, Deserialize)]
struct RemoteModel {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    provider: Option<RemoteModelProvider>,
}

#[derive(Debug, Default, Deserialize)]
struct RemoteModelProvider {
    #[serde(default)]
    npm: Option<String>,
    #[serde(default)]
    api: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct OpenCodeRoute {
    base_url: &'static str,
    request_url: &'static str,
    format: &'static str,
}

fn http_client(options: &SubscriptionHttpOptions) -> Result<reqwest::Client> {
    super::build_http_client(options, "OpenCode")
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

async fn request_device_code(options: &SubscriptionHttpOptions) -> Result<DeviceCodeResponse> {
    let client = http_client(options)?;
    let resp = client
        .post(format!("{SERVER}/auth/device/code"))
        .json(&serde_json::json!({ "client_id": CLIENT_ID }))
        .send()
        .await
        .context("call opencode device code endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "opencode device authorization failed: HTTP {status}: {body}"
        ));
    }
    resp.json().await.context("parse opencode device response")
}

/// Outcome of a single device-token poll.
enum DevicePoll {
    Authorized(TokenResponse),
    Pending,
    SlowDown,
}

fn classify_device_poll_error(
    status: reqwest::StatusCode,
    pending: &PendingResponse,
) -> Result<DevicePoll> {
    match pending.error.as_str() {
        "authorization_pending" => Ok(DevicePoll::Pending),
        "slow_down" => Ok(DevicePoll::SlowDown),
        "expired_token" => Err(anyhow!("opencode device authorization code expired")),
        "access_denied" | "authorization_denied" => {
            Err(anyhow!("opencode device authorization was denied"))
        }
        other => {
            let detail = pending
                .error_description
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(other);
            Err(anyhow!(
                "opencode device authorization failed: HTTP {status}: {detail}"
            ))
        }
    }
}

/// One poll attempt against the device-token endpoint.
async fn poll_once(device_code: &str, options: &SubscriptionHttpOptions) -> Result<DevicePoll> {
    let client = http_client(options)?;
    let resp = client
        .post(format!("{SERVER}/auth/device/token"))
        .json(&serde_json::json!({
            "grant_type": DEVICE_GRANT,
            "device_code": device_code,
            "client_id": CLIENT_ID,
        }))
        .send()
        .await
        .context("call opencode device token endpoint")?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if status.is_success() {
        let tokens = serde_json::from_str::<TokenResponse>(&body)
            .context("parse opencode device token response")?;
        return Ok(DevicePoll::Authorized(tokens));
    }
    if let Ok(pending) = serde_json::from_str::<PendingResponse>(&body) {
        return classify_device_poll_error(status, &pending);
    }
    Err(anyhow!(
        "opencode device token response unrecognized: HTTP {status}: {body}"
    ))
}

fn plan_base_url(plan: OpenCodePlan) -> &'static str {
    match plan {
        OpenCodePlan::Zen => ZEN_BASE_URL,
        OpenCodePlan::Go => GO_BASE_URL,
    }
}

fn route_for(plan: OpenCodePlan, format: &str) -> Result<OpenCodeRoute> {
    let normalized = format.trim().to_ascii_lowercase();
    let route = match (plan, normalized.as_str()) {
        (OpenCodePlan::Zen, "openai") => OpenCodeRoute {
            base_url: ZEN_BASE_URL,
            request_url: ZEN_REQUEST_URL,
            format: "openai",
        },
        (OpenCodePlan::Zen, "response" | "responses") => OpenCodeRoute {
            base_url: ZEN_BASE_URL,
            request_url: ZEN_RESPONSES_URL,
            format: "responses",
        },
        (OpenCodePlan::Zen, "anthropic") => OpenCodeRoute {
            base_url: ZEN_BASE_URL,
            request_url: ZEN_MESSAGES_URL,
            format: "anthropic",
        },
        (OpenCodePlan::Go, "openai") => OpenCodeRoute {
            base_url: GO_BASE_URL,
            request_url: GO_REQUEST_URL,
            format: "openai",
        },
        (OpenCodePlan::Go, "response" | "responses") => OpenCodeRoute {
            base_url: GO_BASE_URL,
            request_url: GO_RESPONSES_URL,
            format: "responses",
        },
        (OpenCodePlan::Go, "anthropic") => OpenCodeRoute {
            base_url: GO_BASE_URL,
            request_url: GO_MESSAGES_URL,
            format: "anthropic",
        },
        _ => {
            return Err(anyhow!(
                "OpenCode {:?} does not support BitFun request format '{}'",
                plan,
                format.trim()
            ));
        }
    };
    Ok(route)
}

fn empty_offering(plan: OpenCodePlan, format: &str) -> SubscriptionApiOffering {
    SubscriptionApiOffering {
        plan,
        format: format.to_string(),
        base_url: plan_base_url(plan).to_string(),
        suggested_model: String::new(),
        models: Vec::new(),
    }
}

fn fallback_offerings() -> Vec<SubscriptionApiOffering> {
    [OpenCodePlan::Zen, OpenCodePlan::Go]
        .into_iter()
        .flat_map(|plan| {
            SUPPORTED_FORMATS
                .into_iter()
                .map(move |format| empty_offering(plan, format))
        })
        .collect()
}

fn canonicalize_offerings(
    offerings: impl IntoIterator<Item = SubscriptionApiOffering>,
) -> Vec<SubscriptionApiOffering> {
    let mut result = fallback_offerings();

    for mut offering in offerings {
        let normalized_format = offering.format.trim().to_ascii_lowercase();
        let normalized_format = match normalized_format.as_str() {
            "response" | "responses" => "responses",
            "openai" => "openai",
            "anthropic" => "anthropic",
            _ => continue,
        };
        offering.format = normalized_format.to_string();
        offering.base_url = plan_base_url(offering.plan).to_string();

        let mut seen = HashSet::new();
        offering.models.retain(|model| {
            let id = model.id.trim();
            !id.is_empty() && seen.insert(id.to_ascii_lowercase())
        });
        offering.models.sort_by(|left, right| {
            left.display_name
                .as_deref()
                .unwrap_or(&left.id)
                .to_ascii_lowercase()
                .cmp(
                    &right
                        .display_name
                        .as_deref()
                        .unwrap_or(&right.id)
                        .to_ascii_lowercase(),
                )
                .then_with(|| left.id.cmp(&right.id))
        });
        offering.suggested_model = offering
            .models
            .first()
            .map(|model| model.id.clone())
            .unwrap_or_default();

        if let Some(slot) = result.iter_mut().find(|candidate| {
            candidate.plan == offering.plan && candidate.format == offering.format
        }) {
            *slot = offering;
        }
    }

    result
}

fn plan_for_provider(provider_id: &str) -> Option<OpenCodePlan> {
    match provider_id {
        "opencode" => Some(OpenCodePlan::Zen),
        "opencode-go" => Some(OpenCodePlan::Go),
        _ => None,
    }
}

fn format_for_remote_model(npm: Option<&str>, api: Option<&str>) -> Option<&'static str> {
    let package = npm.unwrap_or_default().to_ascii_lowercase();
    if package.contains("openai-compatible") {
        return Some("openai");
    }
    if package.contains("anthropic") {
        return Some("anthropic");
    }
    if package == "@ai-sdk/openai" || package.ends_with("/openai") {
        return Some("responses");
    }

    let api = api.unwrap_or_default().trim_end_matches('/');
    if api.ends_with("/chat/completions") {
        Some("openai")
    } else if api.ends_with("/responses") {
        Some("responses")
    } else if api.ends_with("/messages") {
        Some("anthropic")
    } else {
        None
    }
}

fn offerings_from_remote_config(config: RemoteConfig) -> Vec<SubscriptionApiOffering> {
    let mut offerings = fallback_offerings();

    for (provider_id, provider) in config.provider {
        let Some(plan) = plan_for_provider(&provider_id) else {
            continue;
        };
        for (catalog_id, model) in provider.models {
            if model.status.as_deref() == Some("deprecated") {
                continue;
            }
            let npm = model
                .provider
                .as_ref()
                .and_then(|item| item.npm.as_deref())
                .or(provider.npm.as_deref());
            let api = model
                .provider
                .as_ref()
                .and_then(|item| item.api.as_deref())
                .or(provider.api.as_deref());
            let Some(format) = format_for_remote_model(npm, api) else {
                // BitFun does not currently have a compatible adapter for
                // every AI SDK package returned by OpenCode (for example its
                // Google-native gateway shape). Do not offer a model with an
                // endpoint we cannot faithfully reproduce.
                continue;
            };
            let id = model.id.unwrap_or(catalog_id).trim().to_string();
            if id.is_empty() {
                continue;
            }
            let display_name = model
                .name
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty() && name != &id);
            if let Some(offering) = offerings
                .iter_mut()
                .find(|candidate| candidate.plan == plan && candidate.format == format)
            {
                offering
                    .models
                    .push(SubscriptionOfferingModel { id, display_name });
            }
        }
    }

    canonicalize_offerings(offerings)
}

pub(crate) fn offerings_from_metadata(
    metadata: Option<&serde_json::Value>,
) -> Vec<SubscriptionApiOffering> {
    let parsed = metadata
        .and_then(|value| value.get(OFFERINGS_METADATA_KEY))
        .cloned()
        .and_then(|value| serde_json::from_value::<Vec<SubscriptionApiOffering>>(value).ok());
    canonicalize_offerings(parsed.unwrap_or_default())
}

async fn fetch_remote_offerings(
    client: &reqwest::Client,
    access: &str,
    org_id: Option<&str>,
) -> Option<Vec<SubscriptionApiOffering>> {
    let mut request = client
        .get(format!("{SERVER}/api/config"))
        .bearer_auth(access);
    if let Some(org_id) = org_id {
        request = request.header("x-org-id", org_id);
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            log::warn!("fetch OpenCode provider catalog failed: {error}");
            return None;
        }
    };
    // OpenCode treats 404 as "no remote provider override" rather than an
    // authentication or transport failure. Keep the existing catalog/fallback
    // without emitting a misleading warning.
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return None;
    }
    if !response.status().is_success() {
        log::warn!(
            "fetch OpenCode provider catalog failed: status={}",
            response.status()
        );
        return None;
    }
    match response.json::<RemoteConfigResponse>().await {
        Ok(remote) => Some(offerings_from_remote_config(remote.config)),
        Err(error) => {
            log::warn!("parse OpenCode provider catalog failed: {error}");
            None
        }
    }
}

async fn fetch_metadata(
    access: &str,
    existing: Option<&serde_json::Value>,
    options: &SubscriptionHttpOptions,
) -> serde_json::Value {
    let mut metadata = existing
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    metadata.insert(
        "server".to_string(),
        serde_json::Value::String(SERVER.to_string()),
    );
    let client = match http_client(options) {
        Ok(client) => client,
        Err(_) => return serde_json::Value::Object(metadata),
    };

    if let Ok(resp) = client
        .get(format!("{SERVER}/api/user"))
        .bearer_auth(access)
        .send()
        .await
    {
        if let Ok(user) = resp.json::<UserResponse>().await {
            if let Some(email) = user.email {
                metadata.insert("email".to_string(), serde_json::Value::String(email));
            }
            if let Some(id) = user.id {
                metadata.insert("account_id".to_string(), serde_json::Value::String(id));
            }
        }
    }

    if let Ok(resp) = client
        .get(format!("{SERVER}/api/orgs"))
        .bearer_auth(access)
        .send()
        .await
    {
        if let Ok(mut orgs) = resp.json::<Vec<OrgResponse>>().await {
            orgs.sort_by(|a, b| {
                a.name
                    .as_deref()
                    .unwrap_or_default()
                    .cmp(b.name.as_deref().unwrap_or_default())
                    .then_with(|| {
                        a.id.as_deref()
                            .unwrap_or_default()
                            .cmp(b.id.as_deref().unwrap_or_default())
                    })
            });
            if let Some(org) = orgs.into_iter().next() {
                if let Some(id) = org.id {
                    metadata.insert("org_id".to_string(), serde_json::Value::String(id));
                }
                if let Some(name) = org.name {
                    metadata.insert("org_name".to_string(), serde_json::Value::String(name));
                }
            }
        }
    }

    let org_id = metadata.get("org_id").and_then(serde_json::Value::as_str);
    if let Some(offerings) = fetch_remote_offerings(&client, access, org_id).await {
        if let Ok(value) = serde_json::to_value(offerings) {
            metadata.insert(OFFERINGS_METADATA_KEY.to_string(), value);
        }
    }

    serde_json::Value::Object(metadata)
}

async fn persist_tokens(
    tokens: TokenResponse,
    metadata: serde_json::Value,
    expected_revision: u64,
) -> Result<()> {
    let expires = now_ms() + tokens.expires_in * 1000;
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        expected_revision,
        StoredCredential::Oauth {
            refresh: tokens.refresh_token,
            access: tokens.access_token,
            expires,
            account_id: None,
            metadata: Some(metadata),
        },
    )
    .await?;
    super::require_current_store_revision(super::SubscriptionProvider::Opencode, outcome)?;
    log::info!("opencode subscription tokens saved");
    Ok(())
}

async fn refresh(refresh_token: &str, options: &SubscriptionHttpOptions) -> Result<TokenResponse> {
    let client = http_client(options)?;
    let resp = client
        .post(format!("{SERVER}/auth/device/token"))
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "client_id": CLIENT_ID,
        }))
        .send()
        .await
        .context("call opencode device token endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "opencode token refresh failed: HTTP {status}: {body}"
        ));
    }
    resp.json().await.context("parse opencode token response")
}

/// Resolves OpenCode's device verification URI to an absolute URL.
///
/// The console API returns a path such as `/device?user_code=...` (same as
/// OpenCode's own client, which resolves relative paths against its console base URL).
fn absolute_verification_url(uri: &str) -> Result<String> {
    let trimmed = uri.trim();
    if trimmed.is_empty()
        || trimmed
            .chars()
            .any(|character| character.is_ascii_control())
    {
        return Err(anyhow!(
            "OpenCode returned an invalid device verification URL"
        ));
    }
    let base =
        reqwest::Url::parse(&format!("{SERVER}/")).context("parse OpenCode Console base URL")?;
    let resolved = base
        .join(trimmed)
        .context("resolve OpenCode device verification URL")?;
    if !matches!(resolved.scheme(), "http" | "https") || resolved.host_str().is_none() {
        return Err(anyhow!(
            "OpenCode returned an unsupported device verification URL"
        ));
    }
    Ok(resolved.to_string())
}

/// Starts the device-code login flow. The verification URL and user code are
/// returned immediately; the runner polls in the background.
pub(crate) async fn begin_login(
    cancel: CancellationToken,
    expected_revision: u64,
    options: SubscriptionHttpOptions,
) -> Result<StartedLogin> {
    let device = request_device_code(&options).await?;
    let interval = device.interval.unwrap_or(5).max(1);
    let expires_in = device
        .expires_in
        .unwrap_or(super::LOGIN_TIMEOUT.as_secs())
        .max(1)
        .min(super::LOGIN_TIMEOUT.as_secs());
    let device_code = device.device_code.clone();
    let user_code = device.user_code.clone();
    let authorization_url = absolute_verification_url(&device.verification_uri_complete)?;

    let runner = async move {
        super::authorize_then_persist(
            super::SubscriptionProvider::Opencode,
            cancel,
            async {
                let mut wait = interval;
                let deadline = tokio::time::Instant::now() + Duration::from_secs(expires_in);
                loop {
                    let sleep = Duration::from_secs(wait);
                    if tokio::time::Instant::now() + sleep > deadline {
                        return Err(anyhow!("OpenCode device authorization code expired"));
                    }
                    tokio::time::sleep(sleep).await;
                    match poll_once(&device_code, &options).await? {
                        DevicePoll::Authorized(tokens) => {
                            // Optional profile/org network calls belong to the
                            // cancellable authorization phase. The provider
                            // commit lock should cover only the credential
                            // store transaction, never up to 60 seconds of
                            // metadata fetching.
                            let metadata =
                                fetch_metadata(&tokens.access_token, None, &options).await;
                            return Ok((tokens, metadata));
                        }
                        DevicePoll::Pending => {
                            wait = interval;
                        }
                        // RFC 8628: on slow_down, increase the poll interval
                        // by 5 seconds.
                        DevicePoll::SlowDown => {
                            wait = wait.saturating_add(5);
                        }
                    }
                }
            },
            move |(tokens, metadata)| persist_tokens(tokens, metadata, expected_revision),
        )
        .await
    };

    Ok(StartedLogin {
        method: super::SubscriptionLoginMethod::Device,
        authorization_url,
        user_code: Some(user_code),
        instructions: "Open the verification link and enter the code, then return to BitFun."
            .to_string(),
        runner: Box::pin(runner),
    })
}

async fn ensure_fresh(options: &SubscriptionHttpOptions) -> Result<FreshCredential> {
    let _refresh_lease = store::acquire_provider_refresh_lease(STORE_KEY).await?;
    let snapshot = store::load_entry_with_revision(STORE_KEY).await?;
    let entry = snapshot
        .credential
        .ok_or_else(|| anyhow!("OpenCode is not connected; sign in first"))?;
    match entry {
        StoredCredential::Api { key, metadata } => Ok(FreshCredential {
            access: key,
            expires_at_ms: None,
            metadata,
        }),
        StoredCredential::Oauth {
            refresh: refresh_token,
            access,
            expires,
            account_id,
            metadata,
        } => {
            if expires > now_ms() + REFRESH_LEEWAY_MS {
                return Ok(FreshCredential {
                    access,
                    expires_at_ms: Some(expires),
                    metadata,
                });
            }
            let refreshed = refresh(&refresh_token, options).await?;
            let new_expires = now_ms() + refreshed.expires_in * 1000;
            let refreshed_access = refreshed.access_token.clone();
            let refreshed_metadata = metadata.clone();
            let outcome = store::upsert_if_revision(
                STORE_KEY,
                snapshot.revision,
                StoredCredential::Oauth {
                    refresh: refreshed.refresh_token,
                    access: refreshed_access.clone(),
                    expires: new_expires,
                    account_id,
                    metadata,
                },
            )
            .await?;
            match outcome {
                store::ConditionalCommitOutcome::Committed { .. } => {
                    log::info!("opencode subscription tokens refreshed");
                    Ok(FreshCredential {
                        access: refreshed_access,
                        expires_at_ms: Some(new_expires),
                        metadata: refreshed_metadata,
                    })
                }
                store::ConditionalCommitOutcome::Conflict { current_revision } => {
                    let current = super::load_current_store_after_conflict(
                        super::SubscriptionProvider::Opencode,
                        current_revision,
                    )
                    .await?;
                    match current.credential {
                        Some(StoredCredential::Api { key, metadata }) => {
                            log::info!(
                                "opencode refresh reused the current API credential after a concurrent update"
                            );
                            Ok(FreshCredential {
                                access: key,
                                expires_at_ms: None,
                                metadata,
                            })
                        }
                        Some(StoredCredential::Oauth {
                            access,
                            expires,
                            metadata,
                            ..
                        }) if expires > now_ms() => {
                            log::info!(
                                "opencode refresh reused tokens committed by a concurrent refresh"
                            );
                            Ok(FreshCredential {
                                access,
                                expires_at_ms: Some(expires),
                                metadata,
                            })
                        }
                        _ => Err(super::store_revision_conflict(
                            super::SubscriptionProvider::Opencode,
                            current_revision,
                        )),
                    }
                }
            }
        }
    }
}

/// Refreshes account/org/catalog metadata using a fresh credential.
pub(crate) async fn refresh_profile(options: &SubscriptionHttpOptions) -> Result<()> {
    let access = ensure_fresh(options).await?.access;
    let snapshot = store::load_entry_with_revision(STORE_KEY).await?;
    let entry = snapshot
        .credential
        .ok_or_else(|| anyhow!("OpenCode is not connected; sign in first"))?;
    let existing_metadata = match &entry {
        StoredCredential::Oauth { metadata, .. } | StoredCredential::Api { metadata, .. } => {
            metadata.as_ref()
        }
    };
    let metadata = fetch_metadata(&access, existing_metadata, options).await;
    if existing_metadata == Some(&metadata) {
        return Ok(());
    }

    let updated = match entry {
        StoredCredential::Oauth {
            refresh,
            access,
            expires,
            account_id,
            ..
        } => StoredCredential::Oauth {
            refresh,
            access,
            expires,
            account_id,
            metadata: Some(metadata),
        },
        StoredCredential::Api { key, .. } => StoredCredential::Api {
            key,
            metadata: Some(metadata),
        },
    };
    let outcome = store::upsert_if_revision(STORE_KEY, snapshot.revision, updated).await?;
    super::require_current_store_revision(super::SubscriptionProvider::Opencode, outcome)?;
    log::info!("opencode account metadata refreshed");
    Ok(())
}

fn inference_headers(metadata: Option<&serde_json::Value>) -> HashMap<String, String> {
    let org_id = metadata
        .and_then(serde_json::Value::as_object)
        .and_then(|metadata| metadata.get("org_id").or_else(|| metadata.get("orgID")))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    org_id
        .map(|org_id| HashMap::from([("x-org-id".to_string(), org_id.to_string())]))
        .unwrap_or_default()
}

async fn resolve_route(
    route: OpenCodeRoute,
    options: &SubscriptionHttpOptions,
) -> Result<ResolvedCredential> {
    let credential = ensure_fresh(options).await?;
    let extra_headers = inference_headers(credential.metadata.as_ref());
    Ok(ResolvedCredential {
        api_key: credential.access,
        base_url: Some(route.base_url.to_string()),
        request_url: Some(route.request_url.to_string()),
        format: Some(route.format.to_string()),
        extra_headers,
        expires_at: credential.expires_at_ms.map(|expires| expires / 1000),
    })
}

/// Resolves the legacy OpenCode target. Models saved before plan-aware auth
/// are kept on their historical Zen Chat Completions route.
pub(crate) async fn resolve(options: &SubscriptionHttpOptions) -> Result<ResolvedCredential> {
    resolve_route(route_for(OpenCodePlan::Zen, "openai")?, options).await
}

/// Resolves a concrete OpenCode plan and wire format to a trusted endpoint.
pub(crate) async fn resolve_for(
    plan: OpenCodePlan,
    format: &str,
    options: &SubscriptionHttpOptions,
) -> Result<ResolvedCredential> {
    resolve_route(route_for(plan, format)?, options).await
}

/// Provider metadata used to seed a new model entry.
pub(crate) fn suggested() -> (&'static str, &'static str, &'static str) {
    ("openai", ZEN_BASE_URL, DEFAULT_MODEL)
}

#[cfg(test)]
mod tests {
    use super::{
        absolute_verification_url, classify_device_poll_error, inference_headers,
        offerings_from_metadata, offerings_from_remote_config, route_for, DevicePoll, OpenCodePlan,
        PendingResponse, RemoteConfig, RemoteModel, RemoteModelProvider, RemoteProvider,
        GO_MESSAGES_URL, GO_REQUEST_URL, GO_RESPONSES_URL, ZEN_MESSAGES_URL, ZEN_REQUEST_URL,
        ZEN_RESPONSES_URL,
    };
    use std::collections::HashMap;

    #[test]
    fn prefixes_relative_device_verification_path() {
        assert_eq!(
            absolute_verification_url("/device?user_code=FBPH-VLFC&client_id=opencode-cli")
                .unwrap(),
            "https://opencode.ai/device?user_code=FBPH-VLFC&client_id=opencode-cli"
        );
        assert_eq!(
            absolute_verification_url("device?user_code=FBPH-VLFC").unwrap(),
            "https://opencode.ai/console/device?user_code=FBPH-VLFC"
        );
    }

    #[test]
    fn keeps_absolute_verification_url() {
        assert_eq!(
            absolute_verification_url(
                "https://opencode.ai/console/device?user_code=ABCD-1234&client_id=opencode-cli"
            )
            .unwrap(),
            "https://opencode.ai/console/device?user_code=ABCD-1234&client_id=opencode-cli"
        );
        assert!(absolute_verification_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn device_poll_errors_distinguish_pending_and_terminal_outcomes() {
        let pending = PendingResponse {
            error: "authorization_pending".to_string(),
            error_description: None,
        };
        assert!(matches!(
            classify_device_poll_error(reqwest::StatusCode::BAD_REQUEST, &pending).unwrap(),
            DevicePoll::Pending
        ));

        for error in ["expired_token", "access_denied"] {
            let terminal = PendingResponse {
                error: error.to_string(),
                error_description: Some("terminal".to_string()),
            };
            assert!(
                classify_device_poll_error(reqwest::StatusCode::BAD_REQUEST, &terminal).is_err()
            );
        }
    }

    #[test]
    fn maps_every_supported_plan_and_format_to_a_trusted_endpoint() {
        let cases = [
            (OpenCodePlan::Zen, "openai", ZEN_REQUEST_URL, "openai"),
            (
                OpenCodePlan::Zen,
                "responses",
                ZEN_RESPONSES_URL,
                "responses",
            ),
            (
                OpenCodePlan::Zen,
                "anthropic",
                ZEN_MESSAGES_URL,
                "anthropic",
            ),
            (OpenCodePlan::Go, "openai", GO_REQUEST_URL, "openai"),
            (OpenCodePlan::Go, "responses", GO_RESPONSES_URL, "responses"),
            (OpenCodePlan::Go, "anthropic", GO_MESSAGES_URL, "anthropic"),
        ];

        for (plan, format, request_url, canonical_format) in cases {
            let route = route_for(plan, format).expect("route should be supported");
            assert_eq!(route.request_url, request_url);
            assert_eq!(route.format, canonical_format);
        }
        assert!(route_for(OpenCodePlan::Go, "gemini").is_err());
    }

    #[test]
    fn groups_remote_catalog_models_by_plan_and_wire_format() {
        let config = RemoteConfig {
            provider: HashMap::from([
                (
                    "opencode".to_string(),
                    RemoteProvider {
                        npm: Some("@ai-sdk/openai-compatible".to_string()),
                        api: Some("https://opencode.ai/zen/v1".to_string()),
                        models: HashMap::from([
                            (
                                "glm-5.2".to_string(),
                                RemoteModel {
                                    name: Some("GLM 5.2".to_string()),
                                    ..Default::default()
                                },
                            ),
                            (
                                "gpt-5.6-sol".to_string(),
                                RemoteModel {
                                    provider: Some(RemoteModelProvider {
                                        npm: Some("@ai-sdk/openai".to_string()),
                                        api: Some("https://opencode.ai/zen/v1".to_string()),
                                    }),
                                    ..Default::default()
                                },
                            ),
                            (
                                "gemini-3.6-flash".to_string(),
                                RemoteModel {
                                    provider: Some(RemoteModelProvider {
                                        npm: Some("@ai-sdk/google".to_string()),
                                        api: Some("https://opencode.ai/zen/v1".to_string()),
                                    }),
                                    ..Default::default()
                                },
                            ),
                        ]),
                    },
                ),
                (
                    "opencode-go".to_string(),
                    RemoteProvider {
                        models: HashMap::from([(
                            "minimax-m3".to_string(),
                            RemoteModel {
                                provider: Some(RemoteModelProvider {
                                    npm: Some("@ai-sdk/anthropic".to_string()),
                                    api: Some("https://opencode.ai/zen/go/v1".to_string()),
                                }),
                                ..Default::default()
                            },
                        )]),
                        ..Default::default()
                    },
                ),
            ]),
        };

        let offerings = offerings_from_remote_config(config);
        let zen_chat = offerings
            .iter()
            .find(|item| item.plan == OpenCodePlan::Zen && item.format == "openai")
            .unwrap();
        assert_eq!(zen_chat.models[0].id, "glm-5.2");
        assert_eq!(zen_chat.models[0].display_name.as_deref(), Some("GLM 5.2"));
        let zen_responses = offerings
            .iter()
            .find(|item| item.plan == OpenCodePlan::Zen && item.format == "responses")
            .unwrap();
        assert_eq!(zen_responses.models[0].id, "gpt-5.6-sol");
        let go_messages = offerings
            .iter()
            .find(|item| item.plan == OpenCodePlan::Go && item.format == "anthropic")
            .unwrap();
        assert_eq!(go_messages.models[0].id, "minimax-m3");
        assert!(!offerings
            .iter()
            .flat_map(|item| &item.models)
            .any(|model| model.id == "gemini-3.6-flash"));
    }

    #[test]
    fn legacy_metadata_still_exposes_both_plans() {
        let offerings = offerings_from_metadata(Some(&serde_json::json!({
            "email": "user@example.com"
        })));
        assert_eq!(offerings.len(), 6);
        assert!(offerings.iter().any(|item| item.plan == OpenCodePlan::Zen));
        assert!(offerings.iter().any(|item| item.plan == OpenCodePlan::Go));
    }

    #[test]
    fn forwards_current_and_legacy_org_ids_to_subscription_inference() {
        for metadata in [
            serde_json::json!({ "org_id": "org-current" }),
            serde_json::json!({ "orgID": "org-legacy" }),
        ] {
            let headers = inference_headers(Some(&metadata));
            assert_eq!(headers.len(), 1);
            assert!(headers
                .get("x-org-id")
                .is_some_and(|value| value.starts_with("org-")));
        }
        assert!(inference_headers(Some(&serde_json::json!({ "org_id": " " }))).is_empty());
        assert!(inference_headers(None).is_empty());
    }
}
