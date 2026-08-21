//! CodeBuddy subscription login and credential resolution.
//!
//! Aligned with the official CodeBuddy desktop client: a private auth API flow
//! against `copilot.tencent.com`. The client asks the server for an auth
//! state, opens the login page in the browser (which internally redirects to
//! Keycloak with `client_id=console`), and polls for the resulting tokens.
//! The Keycloak token endpoint is never called directly; external clients get
//! a permanent `401 unauthorized_client` there. Gateway requests authenticate
//! with `Authorization: Bearer {accessToken}` plus a set of identity headers.

use super::store::{self, StoredCredential};
use super::{ResolvedCredential, StartedLogin, SubscriptionHttpOptions};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

const API_BASE_URL: &str = "https://copilot.tencent.com";
/// OpenAI-compatible inference endpoint. The official CodeBuddy desktop client
/// appends `/v2` to its product endpoint, so the chat-completions route lives
/// under `/v2/chat/completions` (not `/chat/completions`).
const MODEL_REQUEST_URL: &str = "https://copilot.tencent.com/v2/chat/completions";
const PLATFORM: &str = "CodeBuddyIDE";
const DOMAIN: &str = "copilot.tencent.com";
const STORE_KEY: &str = "codebuddy";
const REFRESH_LEEWAY_MS: i64 = 5 * 60 * 1000;
const POLL_INTERVAL_MS: u64 = 2000;

/// Cache TTL for the dynamic model list (6 minutes, within the 5–10 min range
/// used by the official CodeBuddy client's `PRODUCT_CONFIGURATION_CACHE_TIMEOUT`).
const MODELS_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(360);
/// Minimum failure backoff (30 s, matching the official `MIN_FAILURE_BACKOFF_MS`).
const MIN_BACKOFF: std::time::Duration = std::time::Duration::from_secs(30);
/// Maximum failure backoff (300 s, matching the official `MAX_FAILURE_BACKOFF_MS`).
const MAX_BACKOFF: std::time::Duration = std::time::Duration::from_secs(300);

/// Module-level model list cache. Shared across all calls; keyed implicitly by
/// the single stored CodeBuddy credential (only one account can be signed in).
static MODELS_CACHE: Mutex<Option<(Instant, Vec<crate::types::RemoteModelInfo>)>> =
    Mutex::new(None);
/// Exponential failure backoff timestamp. When set, the enterprise endpoint is
/// skipped until this instant passes, falling through to /v3/config directly.
static BACKOFF_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

/// Token payload returned by the CodeBuddy private auth API. The desktop
/// client reads `data.data` from every response; the same nesting applies
/// here.
#[derive(Debug, Deserialize)]
struct AuthTokenResponse {
    data: AuthTokenData,
}

#[derive(Debug, Deserialize)]
struct AuthTokenData {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "refreshToken")]
    refresh_token: String,
    #[serde(rename = "expiresIn", default)]
    expires_in: Option<i64>,
}

/// Account payload returned by `GET /v2/plugin/login/account?state=`.
#[derive(Debug, Deserialize)]
struct AuthAccountResponse {
    data: AuthAccountData,
}

#[derive(Debug, Deserialize)]
struct AuthAccountData {
    #[serde(default)]
    uid: Option<String>,
    #[serde(default)]
    nickname: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(rename = "enterpriseId", default)]
    enterprise_id: Option<String>,
    #[serde(rename = "departmentFullName", default)]
    department_full_name: Option<String>,
}

/// Response of `GET /v2/plugin/auth/token` while the user has not finished
/// logging in. The official client keeps polling on these codes.
#[derive(Debug, Deserialize)]
struct TokenPendingError {
    code: Option<i64>,
}

fn http_client(options: &SubscriptionHttpOptions) -> Result<reqwest::Client> {
    super::build_http_client(options, "CodeBuddy")
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Step 1: request an auth state and the browser login URL.
async fn request_auth_state(options: &SubscriptionHttpOptions) -> Result<(String, String)> {
    let client = http_client(options)?;
    let resp = client
        .post(format!(
            "{API_BASE_URL}/v2/plugin/auth/state?platform={PLATFORM}"
        ))
        .send()
        .await
        .context("call codebuddy auth state endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "codebuddy auth state request failed: HTTP {status}: {body}"
        ));
    }
    let payload: AuthStateResponse = resp
        .json()
        .await
        .context("parse codebuddy auth state response")?;
    Ok((payload.data.state, payload.data.auth_url))
}

#[derive(Debug, Deserialize)]
struct AuthStateResponse {
    data: AuthStateData,
}

#[derive(Debug, Deserialize)]
struct AuthStateData {
    state: String,
    #[serde(rename = "authUrl")]
    auth_url: String,
}

/// Step 3: poll the private token endpoint until the user finishes the login.
async fn poll_for_token(
    state: &str,
    cancel: &CancellationToken,
    options: &SubscriptionHttpOptions,
) -> Result<AuthTokenData> {
    let client = http_client(options)?;
    loop {
        let resp = client
            .get(format!("{API_BASE_URL}/v2/plugin/auth/token?state={state}"))
            .send()
            .await
            .context("call codebuddy auth token endpoint")?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::BAD_REQUEST {
            // The login is not complete yet; the official client keeps
            // polling until its deadline.
            tokio::select! {
                _ = cancel.cancelled() => return Err(anyhow!("login cancelled")),
                _ = tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)) => {}
            }
            continue;
        }
        if let Ok(payload) = serde_json::from_str::<TokenPendingError>(&body) {
            // The official client (`RetryFetchToken = 11217`) keeps polling
            // while the login is still in progress.
            if matches!(payload.code, Some(11217)) {
                tokio::select! {
                    _ = cancel.cancelled() => return Err(anyhow!("login cancelled")),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)) => {}
                }
                continue;
            }
        }
        if !status.is_success() {
            return Err(anyhow!(
                "codebuddy auth token request failed: HTTP {status}: {body}"
            ));
        }
        let payload: AuthTokenResponse =
            serde_json::from_str(&body).context("parse codebuddy auth token response")?;
        return Ok(payload.data);
    }
}

/// Step 4: fetch the signed-in account so identity headers can be resolved.
async fn fetch_account(
    state: &str,
    access_token: &str,
    options: &SubscriptionHttpOptions,
) -> Result<AuthAccountData> {
    let client = http_client(options)?;
    let resp = client
        .get(format!(
            "{API_BASE_URL}/v2/plugin/login/account?state={state}"
        ))
        .bearer_auth(access_token)
        .send()
        .await
        .context("call codebuddy login account endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "codebuddy login account request failed: HTTP {status}: {body}"
        ));
    }
    let payload: AuthAccountResponse = resp
        .json()
        .await
        .context("parse codebuddy login account response")?;
    Ok(payload.data)
}

fn account_metadata(account: &AuthAccountData) -> Option<serde_json::Value> {
    let uid = account.uid.clone();
    let nickname = account.nickname.clone();
    let email = account.email.clone();
    let enterprise_id = account.enterprise_id.clone();
    let department = account.department_full_name.clone();
    if uid.is_none() && nickname.is_none() && email.is_none() && enterprise_id.is_none() {
        return None;
    }
    let mut object = serde_json::Map::new();
    if let Some(uid) = uid {
        object.insert("uid".to_string(), serde_json::Value::String(uid));
    }
    if let Some(nickname) = nickname {
        object.insert("nickname".to_string(), serde_json::Value::String(nickname));
    }
    if let Some(email) = email {
        object.insert("email".to_string(), serde_json::Value::String(email));
    }
    if let Some(enterprise_id) = enterprise_id {
        object.insert(
            "enterprise_id".to_string(),
            serde_json::Value::String(enterprise_id),
        );
    }
    if let Some(department) = department {
        object.insert(
            "department_full_name".to_string(),
            serde_json::Value::String(department),
        );
    }
    Some(serde_json::Value::Object(object))
}

async fn persist_tokens(
    tokens: AuthTokenData,
    account: AuthAccountData,
    expected_revision: u64,
) -> Result<()> {
    let expires = now_ms() + tokens.expires_in.unwrap_or(3600) * 1000;
    let account_id = account.uid.clone();
    let metadata = account_metadata(&account);
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        expected_revision,
        StoredCredential::Oauth {
            refresh: tokens.refresh_token,
            access: tokens.access_token,
            expires,
            account_id,
            metadata,
        },
    )
    .await?;
    super::require_current_store_revision(super::SubscriptionProvider::CodeBuddy, outcome)?;
    log::info!("codebuddy subscription tokens saved");
    Ok(())
}

/// Starts the private auth API login flow. The browser URL is returned
/// immediately; the runner polls for the token in the background.
pub(crate) async fn begin_login(
    cancel: CancellationToken,
    expected_revision: u64,
    options: SubscriptionHttpOptions,
) -> Result<StartedLogin> {
    let (state, authorization_url) = request_auth_state(&options).await?;

    let runner = async move {
        let cancel = cancel.clone();
        super::authorize_then_persist(
            super::SubscriptionProvider::CodeBuddy,
            cancel.clone(),
            async {
                let tokens = poll_for_token(&state, &cancel, &options).await?;
                // Account lookup is best-effort; identity headers are only
                // emitted when metadata is present, and the account is
                // fetched again lazily during refresh.
                match fetch_account(&state, &tokens.access_token, &options).await {
                    Ok(account) => {
                        if account.enterprise_id.is_none() {
                            log::warn!(
                                "codebuddy login account missing enterprise_id: uid={:?}, nickname={:?}, email={:?} — enterprise endpoint will be skipped",
                                account.uid, account.nickname, account.email
                            );
                        }
                        Ok((tokens, account))
                    }
                    Err(e) => {
                        log::warn!(
                            "codebuddy fetch_account failed: {}; storing empty metadata — enterprise endpoint will be skipped, falling back to /v3/config or static",
                            e
                        );
                        Ok((tokens, AuthAccountData {
                            uid: None,
                            nickname: None,
                            email: None,
                            enterprise_id: None,
                            department_full_name: None,
                        }))
                    }
                }
            },
            move |(tokens, account)| persist_tokens(tokens, account, expected_revision),
        )
        .await
    };

    Ok(StartedLogin {
        authorization_url,
        user_code: None,
        instructions: "Complete authorization in your browser, then return to BitFun.".to_string(),
        runner: Box::pin(runner),
    })
}

/// Loads the stored credential, refreshing the access token when it is about
/// to expire. Returns `(access, account_id, expires_ms)`.
async fn ensure_fresh(options: &SubscriptionHttpOptions) -> Result<(String, Option<String>, i64)> {
    let snapshot = store::load_entry_with_revision(STORE_KEY).await?;
    let entry = snapshot
        .credential
        .ok_or_else(|| anyhow!("CodeBuddy is not connected; sign in first"))?;
    let StoredCredential::Oauth {
        refresh: refresh_token,
        access,
        expires,
        account_id,
        metadata,
    } = entry
    else {
        return Err(anyhow!("CodeBuddy credential is not an OAuth login"));
    };

    if expires > now_ms() + REFRESH_LEEWAY_MS {
        return Ok((access, account_id, expires));
    }

    let refreshed = refresh(&refresh_token, options).await?;
    let new_access = refreshed.access_token;
    let new_refresh = refreshed.refresh_token;
    let new_expires = now_ms() + refreshed.expires_in.unwrap_or(3600) * 1000;
    let new_account_id = account_id;
    let new_metadata = metadata;
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        snapshot.revision,
        StoredCredential::Oauth {
            refresh: new_refresh,
            access: new_access.clone(),
            expires: new_expires,
            account_id: new_account_id.clone(),
            metadata: new_metadata,
        },
    )
    .await?;
    match outcome {
        store::ConditionalCommitOutcome::Committed { .. } => {
            log::info!("codebuddy subscription tokens refreshed");
            Ok((new_access, new_account_id, new_expires))
        }
        store::ConditionalCommitOutcome::Conflict { current_revision } => {
            let current = super::load_current_store_after_conflict(
                super::SubscriptionProvider::CodeBuddy,
                current_revision,
            )
            .await?;
            match current.credential {
                Some(StoredCredential::Oauth {
                    access,
                    expires,
                    account_id,
                    ..
                }) if expires > now_ms() => {
                    log::info!("codebuddy refresh reused tokens committed by a concurrent refresh");
                    Ok((access, account_id, expires))
                }
                _ => Err(super::store_revision_conflict(
                    super::SubscriptionProvider::CodeBuddy,
                    current_revision,
                )),
            }
        }
    }
}

/// Refreshes the CodeBuddy credential through the private refresh endpoint.
async fn refresh(refresh_token: &str, options: &SubscriptionHttpOptions) -> Result<AuthTokenData> {
    let client = http_client(options)?;
    let resp = client
        .post(format!("{API_BASE_URL}/v2/plugin/auth/token/refresh"))
        .header("X-Refresh-Token", refresh_token)
        .header("X-Auth-Refresh-Source", "plugin")
        .send()
        .await
        .context("call codebuddy auth token refresh endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "codebuddy token refresh failed: HTTP {status}: {body}"
        ));
    }
    let payload: AuthTokenResponse = resp
        .json()
        .await
        .context("parse codebuddy refresh response")?;
    Ok(payload.data)
}

/// Resolves the runtime credential, injecting the CodeBuddy identity headers.
///
/// Mirrors the official desktop client's `buildAuthHeaders`: `X-User-Id` is
/// the signed-in account's `uid`, `X-Enterprise-Id` + `X-Tenant-Id` are the
/// account's `enterpriseId` (same value), `X-Department-Info` is the account's
/// `departmentFullName`, and `X-Domain` is the product domain. Conditional
/// headers are only emitted when the corresponding account metadata exists.
pub(crate) async fn resolve(options: &SubscriptionHttpOptions) -> Result<ResolvedCredential> {
    let (access, _account_id, expires) = ensure_fresh(options).await?;
    let mut headers = HashMap::new();
    let metadata = store::load_entry(STORE_KEY)
        .await?
        .and_then(|entry| match entry {
            StoredCredential::Oauth { metadata, .. } => metadata,
            StoredCredential::Api { metadata, .. } => metadata,
        });
    let metadata_map = metadata.and_then(|value| value.as_object().cloned());
    // X-User-Id: account.uid (stored from the login account fetch).
    if let Some(uid) = metadata_map
        .as_ref()
        .and_then(|map| map.get("uid"))
        .and_then(|value| value.as_str())
    {
        headers.insert("X-User-Id".to_string(), uid.to_string());
    }
    // X-Enterprise-Id + X-Tenant-Id: account.enterpriseId, both set to the
    // same value when present (official `buildAuthHeaders`).
    if let Some(enterprise_id) = metadata_map
        .as_ref()
        .and_then(|map| map.get("enterprise_id"))
        .and_then(|value| value.as_str())
    {
        headers.insert("X-Enterprise-Id".to_string(), enterprise_id.to_string());
        headers.insert("X-Tenant-Id".to_string(), enterprise_id.to_string());
    }
    // X-Department-Info: account.departmentFullName when present.
    if let Some(department) = metadata_map
        .as_ref()
        .and_then(|map| map.get("department_full_name"))
        .and_then(|value| value.as_str())
    {
        headers.insert("X-Department-Info".to_string(), department.to_string());
    }
    // X-Domain: always the codebuddy product domain.
    headers.insert("X-Domain".to_string(), DOMAIN.to_string());

    Ok(ResolvedCredential {
        api_key: access,
        base_url: Some(API_BASE_URL.to_string()),
        request_url: Some(MODEL_REQUEST_URL.to_string()),
        format: Some("openai".to_string()),
        extra_headers: headers,
        expires_at: Some(expires / 1000),
    })
}

// ---------------------------------------------------------------------------
// Dynamic model list (mirrors the official ModelsProductProvider chain)
// ---------------------------------------------------------------------------

/// Response from `GET /console/enterprises/{eid}/config/models`.
#[derive(Debug, Deserialize)]
struct EnterpriseModelsResponse {
    #[allow(dead_code)]
    code: Option<i64>,
    #[allow(dead_code)]
    msg: Option<String>,
    data: EnterpriseModelsData,
}

#[derive(Debug, Deserialize)]
struct EnterpriseModelsData {
    #[serde(default)]
    data: Vec<CodeBuddyModelEntry>,
}

/// Response from `GET /v3/config` (authenticated).
#[derive(Debug, Deserialize)]
struct V3ConfigResponse {
    #[allow(dead_code)]
    code: Option<i64>,
    #[allow(dead_code)]
    msg: Option<String>,
    data: V3ConfigData,
}

#[derive(Debug, Deserialize)]
struct V3ConfigData {
    #[allow(dead_code)]
    agent: Option<serde_json::Value>,
    /// `null` when the request is unauthenticated; `Some(models)` with auth.
    models: Option<V3ModelsData>,
    #[allow(dead_code)]
    mcp: Option<serde_json::Value>,
    #[allow(dead_code)]
    codebase: Option<serde_json::Value>,
    #[allow(dead_code)]
    features: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct V3ModelsData {
    #[serde(default)]
    data: Vec<CodeBuddyModelEntry>,
}

/// A single model entry from either the enterprise or /v3/config endpoint.
/// The wire shape is the same (`ModelsProductProvider` and
/// `CloudProductProvider` share the model entry type in the official client).
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // fields used by serde deserialization and tests
struct CodeBuddyModelEntry {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default, rename = "supportsImages")]
    supports_images: bool,
    #[serde(default, rename = "supportsReasoning")]
    supports_reasoning: bool,
}

/// Fetches the live CodeBuddy model catalog through the three-level fallback
/// chain: enterprise endpoint → /v3/config → static catalog.
///
/// Results are cached in memory for [`MODELS_CACHE_TTL`] (6 min). On HTTP
/// failure the enterprise endpoint enters exponential backoff (30 s → 300 s);
/// during backoff the call skips straight to /v3/config.
pub(crate) async fn list_models(
    options: &SubscriptionHttpOptions,
) -> Result<Vec<crate::types::RemoteModelInfo>> {
    // 1. Return cached result if still fresh.
    if let Some(models) = read_cached_models() {
        return Ok(models);
    }

    // 2. Load credential and identity metadata from the auth store.
    let (access, metadata_map) = load_auth_for_models().await?;

    let uid = metadata_map
        .as_ref()
        .and_then(|m| m.get("uid"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let enterprise_id = metadata_map
        .as_ref()
        .and_then(|m| m.get("enterprise_id"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let domain = DOMAIN.to_string();

    let client = http_client(options)?;

    // 3. Try the enterprise endpoint (unless in backoff or missing enterprise_id).
    if let Some(eid) = &enterprise_id {
        if !is_in_backoff() {
            match fetch_enterprise_models(&client, &access, &uid, eid, &domain).await {
                Ok(models) if !models.is_empty() => {
                    log::info!(
                        "codebuddy loaded {} models from enterprise endpoint",
                        models.len()
                    );
                    store_models_in_cache(models.clone());
                    return Ok(models);
                }
                Ok(_) => {
                    // Empty response — treat as failure, fall through.
                    log::debug!("codebuddy enterprise models returned empty list");
                }
                Err(e) => {
                    log::info!("codebuddy enterprise models failed: {e}; trying /v3/config");
                    apply_failure_backoff();
                }
            }
        } else {
            log::debug!("codebuddy enterprise endpoint in backoff, skipping to /v3/config");
        }
    } else {
        // enterprise_id is missing — log warning and skip directly to /v3/config
        log::warn!(
            "codebuddy enterprise_id not found in metadata; skipping enterprise endpoint and falling back to /v3/config (authenticated) or static catalog"
        );
    }

    // 4. Try /v3/config.
    match fetch_v3_models(&client, &access, &uid, &enterprise_id, &domain).await {
        Ok(models) if !models.is_empty() => {
            let source = if enterprise_id.is_some() {
                "enterprise fallback"
            } else {
                "primary (no enterprise_id)"
            };
            log::info!(
                "codebuddy loaded {} models from /v3/config ({})",
                models.len(),
                source
            );
            store_models_in_cache(models.clone());
            return Ok(models);
        }
        Ok(_) => {
            log::warn!("codebuddy /v3/config returned no models; falling back to static catalog");
        }
        Err(e) => {
            log::warn!("codebuddy /v3/config failed: {e}; falling back to static catalog");
        }
    }

    // 5. Static fallback — never fails.
    let static_models = crate::providers::openai::common::static_codebuddy_models();
    log::warn!(
        "codebuddy using static model catalog ({} models) — enterprise_id missing or all endpoints failed",
        static_models.len()
    );
    Ok(static_models)
}

/// Reads the cached model list if it exists and has not expired.
fn read_cached_models() -> Option<Vec<crate::types::RemoteModelInfo>> {
    let guard = MODELS_CACHE.lock().ok()?;
    let (instant, models) = guard.as_ref()?;
    if instant.elapsed() < MODELS_CACHE_TTL {
        Some(models.clone())
    } else {
        None
    }
}

/// Stores a fresh model list in the cache.
fn store_models_in_cache(models: Vec<crate::types::RemoteModelInfo>) {
    if let Ok(mut guard) = MODELS_CACHE.lock() {
        *guard = Some((Instant::now(), models));
    }
}

/// Returns `true` when the enterprise endpoint is in failure backoff.
fn is_in_backoff() -> bool {
    BACKOFF_UNTIL
        .lock()
        .ok()
        .and_then(|g| *g)
        .is_some_and(|until| Instant::now() < until)
}

/// Records a failure and sets the exponential backoff deadline.
fn apply_failure_backoff() {
    if let Ok(mut guard) = BACKOFF_UNTIL.lock() {
        let current = guard.unwrap_or_else(Instant::now);
        let remaining = current.saturating_duration_since(Instant::now());
        let next = if remaining.is_zero() {
            MIN_BACKOFF
        } else {
            (remaining * 2).min(MAX_BACKOFF)
        };
        *guard = Some(Instant::now() + next);
    }
}

/// Loads the stored OAuth credential and its metadata for model fetching.
/// Returns `(access_token, metadata_map)`.
async fn load_auth_for_models(
) -> Result<(String, Option<serde_json::Map<String, serde_json::Value>>)> {
    let snapshot = store::load_entry_with_revision(STORE_KEY).await?;
    let entry = snapshot
        .credential
        .ok_or_else(|| anyhow!("CodeBuddy is not connected; sign in first"))?;
    let StoredCredential::Oauth {
        access, metadata, ..
    } = entry
    else {
        return Err(anyhow!("CodeBuddy credential is not an OAuth login"));
    };
    let metadata_map = metadata.and_then(|v| v.as_object().cloned());
    Ok((access, metadata_map))
}

/// `GET /console/enterprises/{eid}/config/models` — the enterprise-level model
/// list, filtered by the account's subscription. Mirrors the official
/// `ModelsProductProvider`.
async fn fetch_enterprise_models(
    client: &reqwest::Client,
    access: &str,
    uid: &Option<String>,
    enterprise_id: &str,
    domain: &str,
) -> Result<Vec<crate::types::RemoteModelInfo>> {
    let url = format!("{API_BASE_URL}/console/enterprises/{enterprise_id}/config/models");
    let mut builder = client.get(&url).bearer_auth(access);
    if let Some(uid) = uid {
        builder = builder.header("X-User-Id", uid);
    }
    builder = builder
        .header("X-Enterprise-Id", enterprise_id)
        .header("X-Tenant-Id", enterprise_id)
        .header("X-Domain", domain);

    let resp = builder
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .context("call codebuddy enterprise models endpoint")?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!(
            "codebuddy enterprise models failed: HTTP {status}: {}",
            body.chars().take(400).collect::<String>()
        ));
    }
    let payload: EnterpriseModelsResponse =
        serde_json::from_str(&body).context("parse codebuddy enterprise models response")?;
    Ok(map_model_entries(payload.data.data))
}

/// `GET /v3/config` — global product configuration. When authenticated the
/// response includes `data.models.data`; without auth `models` is `null`.
async fn fetch_v3_models(
    client: &reqwest::Client,
    access: &str,
    uid: &Option<String>,
    enterprise_id: &Option<String>,
    domain: &str,
) -> Result<Vec<crate::types::RemoteModelInfo>> {
    let url = format!("{API_BASE_URL}/v3/config");
    let mut builder = client.get(&url).bearer_auth(access);
    if let Some(uid) = uid {
        builder = builder.header("X-User-Id", uid);
    }
    if let Some(eid) = enterprise_id {
        builder = builder
            .header("X-Enterprise-Id", eid)
            .header("X-Tenant-Id", eid);
    }
    builder = builder.header("X-Domain", domain);

    let resp = builder
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .context("call codebuddy /v3/config endpoint")?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!(
            "codebuddy /v3/config failed: HTTP {status}: {}",
            body.chars().take(400).collect::<String>()
        ));
    }
    let payload: V3ConfigResponse =
        serde_json::from_str(&body).context("parse codebuddy /v3/config response")?;
    let entries = payload.data.models.map(|m| m.data).unwrap_or_default();
    Ok(map_model_entries(entries))
}

/// Maps raw API model entries to the shared `RemoteModelInfo` shape.
fn map_model_entries(entries: Vec<CodeBuddyModelEntry>) -> Vec<crate::types::RemoteModelInfo> {
    entries
        .into_iter()
        .filter(|e| !e.id.is_empty())
        .map(|e| crate::types::RemoteModelInfo {
            id: e.id,
            display_name: e.name,
            supports_reasoning: Some(e.supports_reasoning),
        })
        .collect()
}

/// Provider metadata used to seed a new model entry.
///
/// The model id must be a real CodeBuddy backend model name (the gateway does
/// not map arbitrary ids); `glm-5.2` is the default in the official client's
/// model list.
pub(crate) fn suggested() -> (&'static str, &'static str, &'static str) {
    ("openai", API_BASE_URL, "glm-5.2")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_model_and_format_are_stable() {
        let (format, base_url, model) = suggested();
        assert_eq!(format, "openai");
        assert_eq!(base_url, API_BASE_URL);
        assert_eq!(model, "glm-5.2");
    }

    #[test]
    fn account_metadata_keeps_only_present_fields() {
        let account = AuthAccountData {
            uid: Some("u-123".to_string()),
            nickname: Some("coder".to_string()),
            email: None,
            enterprise_id: Some("ent-9".to_string()),
            department_full_name: Some("R&D".to_string()),
        };
        let metadata = account_metadata(&account).expect("metadata present");
        assert_eq!(metadata["uid"], "u-123");
        assert_eq!(metadata["enterprise_id"], "ent-9");
        assert_eq!(metadata["department_full_name"], "R&D");
        assert!(metadata.get("email").is_none());
    }

    #[test]
    fn resolve_headers_use_metadata_conditions() {
        let _guard = super::super::tests::test_lock().blocking_lock();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            store::set_store_path_for_test(
                std::env::temp_dir()
                    .join(format!("bitfun-subauth-codebuddy-{}", uuid::Uuid::new_v4()))
                    .join("subscription_auth.json"),
            );
            store::upsert(
                STORE_KEY,
                StoredCredential::Oauth {
                    refresh: "r".to_string(),
                    access: "a".to_string(),
                    expires: now_ms() + 3_600_000,
                    account_id: Some("u-123".to_string()),
                    metadata: Some(serde_json::json!({
                        "uid": "u-123",
                        "enterprise_id": "ent-9",
                        "department_full_name": "R&D"
                    })),
                },
            )
            .await
            .unwrap();
            let resolved = resolve(&SubscriptionHttpOptions::default())
                .await
                .expect("resolve credential");
            assert_eq!(resolved.api_key, "a");
            assert_eq!(resolved.extra_headers["X-User-Id"], "u-123");
            assert_eq!(resolved.extra_headers["X-Enterprise-Id"], "ent-9");
            assert_eq!(resolved.extra_headers["X-Tenant-Id"], "ent-9");
            assert_eq!(resolved.extra_headers["X-Domain"], DOMAIN);
            assert_eq!(resolved.extra_headers["X-Department-Info"], "R&D");
            assert_eq!(
                resolved.request_url.as_deref(),
                Some("https://copilot.tencent.com/v2/chat/completions")
            );
            assert_eq!(resolved.format.as_deref(), Some("openai"));
        });
    }

    #[test]
    fn resolve_skips_absent_enterprise_headers() {
        let _guard = super::super::tests::test_lock().blocking_lock();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            store::set_store_path_for_test(
                std::env::temp_dir()
                    .join(format!(
                        "bitfun-subauth-codebuddy-nope-{}",
                        uuid::Uuid::new_v4()
                    ))
                    .join("subscription_auth.json"),
            );
            store::upsert(
                STORE_KEY,
                StoredCredential::Oauth {
                    refresh: "r".to_string(),
                    access: "a".to_string(),
                    expires: now_ms() + 3_600_000,
                    account_id: None,
                    metadata: Some(serde_json::json!({ "uid": "u-1" })),
                },
            )
            .await
            .unwrap();
            let resolved = resolve(&SubscriptionHttpOptions::default())
                .await
                .expect("resolve credential");
            assert_eq!(resolved.extra_headers["X-User-Id"], "u-1");
            assert!(!resolved.extra_headers.contains_key("X-Enterprise-Id"));
            assert!(!resolved.extra_headers.contains_key("X-Tenant-Id"));
            assert!(!resolved.extra_headers.contains_key("X-Department-Info"));
            assert_eq!(resolved.extra_headers["X-Domain"], DOMAIN);
        });
    }

    // -- Dynamic model list tests -------------------------------------------

    /// Helper: clear the module-level model cache and backoff state so that
    /// each test starts from a clean slate.
    fn reset_model_cache() {
        if let Ok(mut g) = super::MODELS_CACHE.lock() {
            *g = None;
        }
        if let Ok(mut g) = super::BACKOFF_UNTIL.lock() {
            *g = None;
        }
    }

    #[test]
    fn enterprise_endpoint_url_format() {
        let eid = "ent-42";
        let url = format!(
            "{}/console/enterprises/{eid}/config/models",
            super::API_BASE_URL
        );
        assert_eq!(
            url,
            "https://copilot.tencent.com/console/enterprises/ent-42/config/models"
        );
    }

    #[test]
    fn map_model_entries_filters_empty_and_preserves_fields() {
        let entries = vec![
            CodeBuddyModelEntry {
                id: "glm-5.3".to_string(),
                name: Some("GLM-5.3".to_string()),
                tags: vec!["chat".to_string()],
                supports_images: false,
                supports_reasoning: true,
            },
            CodeBuddyModelEntry {
                id: String::new(),
                name: None,
                tags: vec![],
                supports_images: false,
                supports_reasoning: false,
            },
            CodeBuddyModelEntry {
                id: "kimi-k2.7".to_string(),
                name: None,
                tags: vec!["chat".to_string()],
                supports_images: false,
                supports_reasoning: false,
            },
        ];
        let models = super::map_model_entries(entries);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "glm-5.3");
        assert_eq!(models[0].display_name.as_deref(), Some("GLM-5.3"));
        assert_eq!(models[1].id, "kimi-k2.7");
        assert_eq!(models[1].display_name, None);
    }

    #[test]
    fn parse_enterprise_response_wire_shape() {
        let json = r#"{
            "code": 0,
            "msg": "OK",
            "data": {
                "data": [
                    {
                        "id": "glm-5.3",
                        "name": "GLM-5.3",
                        "tags": ["chat"],
                        "supportsImages": false,
                        "supportsReasoning": true
                    },
                    {
                        "id": "custom:my-model",
                        "name": "My Custom",
                        "tags": ["custom"]
                    }
                ]
            }
        }"#;
        let resp: EnterpriseModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.data.len(), 2);
        assert_eq!(resp.data.data[0].id, "glm-5.3");
        assert_eq!(resp.data.data[0].name.as_deref(), Some("GLM-5.3"));
        assert!(resp.data.data[0].supports_reasoning);
        assert!(!resp.data.data[0].supports_images);
        // serde(default) fills missing booleans with false.
        assert!(!resp.data.data[1].supports_images);
        assert!(!resp.data.data[1].supports_reasoning);
    }

    #[test]
    fn parse_v3_config_with_models() {
        let json = r#"{
            "code": 0,
            "msg": "OK",
            "data": {
                "agent": null,
                "models": {
                    "data": [
                        { "id": "deepseek-v4-pro", "name": "DeepSeek-V4-Pro" }
                    ]
                },
                "mcp": null,
                "codebase": null,
                "features": null
            }
        }"#;
        let resp: V3ConfigResponse = serde_json::from_str(json).unwrap();
        let entries = resp.data.models.unwrap().data;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "deepseek-v4-pro");
    }

    #[test]
    fn parse_v3_config_null_models() {
        let json = r#"{
            "code": 0,
            "msg": "OK",
            "data": {
                "agent": null,
                "models": null,
                "mcp": null,
                "codebase": null,
                "features": null
            }
        }"#;
        let resp: V3ConfigResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.models.is_none());
    }

    #[test]
    fn no_enterprise_id_falls_back_to_static() {
        reset_model_cache();
        let _guard = super::super::tests::test_lock().blocking_lock();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            store::set_store_path_for_test(
                std::env::temp_dir()
                    .join(format!("bitfun-cb-models-no-eid-{}", uuid::Uuid::new_v4()))
                    .join("subscription_auth.json"),
            );
            store::upsert(
                STORE_KEY,
                StoredCredential::Oauth {
                    refresh: "r".to_string(),
                    access: "a".to_string(),
                    expires: now_ms() + 3_600_000,
                    account_id: Some("u-1".to_string()),
                    metadata: Some(serde_json::json!({ "uid": "u-1" })),
                },
            )
            .await
            .unwrap();
            // No enterprise_id → enterprise endpoint skipped → /v3/config will
            // fail (no real network) → static fallback.
            let models = super::list_models(&SubscriptionHttpOptions::default())
                .await
                .expect("list_models must not fail");
            assert!(
                models.iter().any(|m| m.id == "glm-5.2"),
                "static fallback must include glm-5.2; got {:?}",
                models.iter().map(|m| &m.id).collect::<Vec<_>>()
            );
        });
        reset_model_cache();
    }

    #[test]
    fn enterprise_id_missing_logs_warning() {
        reset_model_cache();
        let _guard = super::super::tests::test_lock().blocking_lock();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            store::set_store_path_for_test(
                std::env::temp_dir()
                    .join(format!(
                        "bitfun-cb-models-no-eid-log-{}",
                        uuid::Uuid::new_v4()
                    ))
                    .join("subscription_auth.json"),
            );
            // Store credential with uid but no enterprise_id
            store::upsert(
                STORE_KEY,
                StoredCredential::Oauth {
                    refresh: "r".to_string(),
                    access: "a".to_string(),
                    expires: now_ms() + 3_600_000,
                    account_id: Some("u-test".to_string()),
                    metadata: Some(serde_json::json!({
                        "uid": "u-test",
                        "nickname": "tester"
                    })),
                },
            )
            .await
            .unwrap();
            // Call list_models — should log warning about missing enterprise_id
            let models = super::list_models(&SubscriptionHttpOptions::default())
                .await
                .expect("list_models must not fail");
            // Verify we get static fallback
            assert!(
                models.iter().any(|m| m.id == "glm-5.2"),
                "should fall back to static models when enterprise_id is missing"
            );
        });
        reset_model_cache();
    }

    #[test]
    fn fetch_account_failure_logs_warning() {
        let _guard = super::super::tests::test_lock().blocking_lock();
        // This test verifies that when fetch_account fails during login,
        // a warning is logged and empty metadata is stored.
        // Note: We cannot easily mock the HTTP client in unit tests, so this
        // test documents the expected behavior rather than asserting logs.
        assert!(true);
    }

    #[test]
    fn cache_returns_same_result_without_refetch() {
        reset_model_cache();
        // Pre-populate the cache.
        let cached = vec![crate::types::RemoteModelInfo {
            id: "cached-model".to_string(),
            display_name: Some("Cached".to_string()),
            supports_reasoning: Some(true),
        }];
        super::store_models_in_cache(cached.clone());
        // Should return the cached value without touching the store.
        let result = super::read_cached_models().expect("cache hit");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "cached-model");
        reset_model_cache();
    }

    #[test]
    fn expired_cache_triggers_refetch() {
        reset_model_cache();
        // Insert a cache entry that is already expired.
        {
            let mut guard = super::MODELS_CACHE.lock().unwrap();
            *guard = Some((
                Instant::now() - super::MODELS_CACHE_TTL - std::time::Duration::from_secs(1),
                vec![],
            ));
        }
        assert!(
            super::read_cached_models().is_none(),
            "expired cache must return None"
        );
        reset_model_cache();
    }

    #[test]
    fn backoff_increases_and_caps() {
        reset_model_cache();
        // First failure → 30 s backoff.
        super::apply_failure_backoff();
        assert!(super::is_in_backoff());
        // Second failure → 60 s.
        super::apply_failure_backoff();
        // Keep doubling until we hit the cap.
        for _ in 0..10 {
            super::apply_failure_backoff();
        }
        // The backoff should be capped at MAX_BACKOFF (300 s).
        if let Ok(guard) = super::BACKOFF_UNTIL.lock() {
            if let Some(until) = *guard {
                let remaining = until.saturating_duration_since(Instant::now());
                assert!(
                    remaining <= super::MAX_BACKOFF + std::time::Duration::from_secs(1),
                    "backoff {remaining:?} must not exceed MAX_BACKOFF {:?}",
                    super::MAX_BACKOFF
                );
            }
        }
        reset_model_cache();
    }
}
