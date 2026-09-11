//! GitHub identity exchange for versioned OpenBitFun Relay device sessions.

use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::{Extension, Json};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::OnceLock;

use crate::db::{AuthToken, DeviceRow, UserRow};
use crate::routes::api::AppState;

/// Max login attempts per IP per minute (across all accounts — stops
/// credential-stuffing where one IP tries many usernames).
const MAX_LOGIN_ATTEMPTS_PER_MIN: usize = 10;
/// Max challenge requests per IP per minute (stops bulk salt harvesting).
const MAX_RATE_LIMIT_BUCKETS: usize = 50_000;
const MAX_DEVICE_ID_BYTES: usize = 128;
const MAX_DEVICE_NAME_BYTES: usize = 256;

fn valid_bounded_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn valid_device_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DEVICE_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_login_request_id(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok()
}

/// Absent is legal — clients that predate the field still log in, and their
/// stored kind is left untouched rather than overwritten with a guess.
fn valid_optional_device_kind(value: Option<&str>) -> bool {
    value.is_none_or(crate::db::is_valid_device_kind)
}

// ── IP rate limiter (sliding window, in-memory) ─────────────────────────

/// Per-IP sliding-window rate limiter. In-memory only; resets on restart,
/// which is acceptable for brute-force throttling (the account lockout in the
/// DB is the durable backstop).
pub struct LoginRateLimiter {
    attempts: DashMap<String, Vec<RateLimitAttempt>>,
}

struct RateLimitAttempt {
    timestamp: i64,
    replay_key: Option<String>,
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self {
            attempts: DashMap::new(),
        }
    }

    /// Record an attempt for one rate-limit scope and return `true` if the IP
    /// is still under the per-minute limit. An exact replay key is counted once
    /// so an ambiguous idempotent response cannot consume the full login budget.
    pub(crate) fn check_and_record(
        &self,
        scope: &str,
        ip: &str,
        max_per_min: usize,
        replay_key: Option<&str>,
    ) -> bool {
        let now = Utc::now().timestamp();
        let cutoff = now - 60;
        let bucket_key = format!("{scope}:{ip}");
        if self.attempts.len() >= MAX_RATE_LIMIT_BUCKETS && !self.attempts.contains_key(&bucket_key)
        {
            self.attempts.retain(|_, timestamps| {
                timestamps.retain(|attempt| attempt.timestamp > cutoff);
                !timestamps.is_empty()
            });
            if self.attempts.len() >= MAX_RATE_LIMIT_BUCKETS {
                return false;
            }
        }
        let mut entry = self.attempts.entry(bucket_key).or_default();
        let timestamps = entry.value_mut();
        timestamps.retain(|attempt| attempt.timestamp > cutoff);
        if replay_key.is_some_and(|key| {
            timestamps
                .iter()
                .any(|attempt| attempt.replay_key.as_deref() == Some(key))
        }) {
            return true;
        }
        if timestamps.len() >= max_per_min {
            return false;
        }
        timestamps.push(RateLimitAttempt {
            timestamp: now,
            replay_key: replay_key.map(str::to_string),
        });
        true
    }
}

impl Default for LoginRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the client IP from `X-Forwarded-For` (first hop) or fall back to a
/// static bucket so all headerless requests share one limiter entry.
pub(crate) fn client_ip(headers: &HeaderMap, peer_addr: Option<SocketAddr>) -> String {
    let Some(peer_addr) = peer_addr else {
        return "unknown".to_string();
    };

    // Forwarded headers are caller-controlled unless the immediate peer is a
    // local reverse proxy. Parse the value as an IP as well, so arbitrary
    // strings cannot create unbounded rate-limit buckets.
    if peer_addr.ip().is_loopback() {
        if let Some(forwarded_ip) = headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(',').next())
            .map(str::trim)
            .and_then(|value| value.parse::<std::net::IpAddr>().ok())
        {
            return forwarded_ip.to_string();
        }
    }

    peer_addr.ip().to_string()
}

// ── Request / response types ────────────────────────────────────────────

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub access_token: String,
    pub device_id: String,
    pub device_name: String,
    pub device_kind: String,
    pub public_key: String,
    pub request_id: String,
}

#[derive(Deserialize)]
pub struct ProvisionDeviceRequest {
    pub public_key: String,
    pub device_id: String,
    pub device_name: String,
    #[serde(default)]
    pub device_kind: Option<String>,
    pub request_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct ProvisionDeviceResponse {
    pub token: String,
    pub user_id: String,
    pub device_id: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_secs: Option<i64>,
}

fn err(error: &str, status: StatusCode) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: error.to_string(),
            retry_after_secs: None,
        }),
    )
}

fn identity_verifier(
    injected: Option<&crate::identity::IdentityVerifier>,
) -> Result<&crate::identity::IdentityVerifier, (StatusCode, Json<ErrorResponse>)> {
    static VERIFIER: OnceLock<Result<crate::identity::IdentityVerifier, String>> = OnceLock::new();
    if let Some(verifier) = injected {
        return Ok(verifier);
    }
    VERIFIER
        .get_or_init(|| {
            crate::identity::IdentityVerifier::new()
                .map_err(|_| "initialization failed".to_string())
        })
        .as_ref()
        .map_err(|_| {
            err(
                "identity service unavailable",
                StatusCode::SERVICE_UNAVAILABLE,
            )
        })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubPollRequest {
    transaction_id: String,
    transaction_secret: String,
}

pub(crate) async fn github_start(
    State(state): State<AppState>,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    verifier: Option<Extension<crate::identity::IdentityVerifier>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let ip = client_ip(
        &headers,
        connect_info.map(|Extension(ConnectInfo(addr))| addr),
    );
    if !state
        .login_rate_limiter
        .check_and_record("github-start", &ip, 10, None)
    {
        return Err(err(
            "too many sign-in attempts",
            StatusCode::TOO_MANY_REQUESTS,
        ));
    }
    identity_verifier(verifier.as_ref().map(|v| &v.0))?
        .start_auth()
        .await
        .map(Json)
        .map_err(|status| err("GitHub sign-in could not be started", status))
}

pub(crate) async fn github_poll(
    State(state): State<AppState>,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    verifier: Option<Extension<crate::identity::IdentityVerifier>>,
    Json(body): Json<GithubPollRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let ip = client_ip(
        &headers,
        connect_info.map(|Extension(ConnectInfo(addr))| addr),
    );
    if !state
        .login_rate_limiter
        .check_and_record("github-poll", &ip, 120, None)
    {
        return Err(err("too many sign-in polls", StatusCode::TOO_MANY_REQUESTS));
    }
    identity_verifier(verifier.as_ref().map(|v| &v.0))?
        .poll_auth(&body.transaction_id, &body.transaction_secret)
        .await
        .map(Json)
        .map_err(|status| err("GitHub sign-in could not be checked", status))
}

pub(crate) async fn verify_identity_credentials(
    state: &AppState,
    peer_addr: Option<SocketAddr>,
    headers: &HeaderMap,
    access_token: &str,
    injected_verifier: Option<&crate::identity::IdentityVerifier>,
) -> Result<UserRow, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db.as_ref();
    if !state.login_rate_limiter.check_and_record(
        "identity",
        &client_ip(headers, peer_addr),
        MAX_LOGIN_ATTEMPTS_PER_MIN,
        None,
    ) {
        return Err(err(
            "too many login attempts",
            StatusCode::TOO_MANY_REQUESTS,
        ));
    }
    let verifier = identity_verifier(injected_verifier)?;
    let identity = verifier.verify(access_token).await.map_err(|status| {
        err(
            if status == StatusCode::UNAUTHORIZED {
                "Sign in with GitHub to continue"
            } else {
                "identity service unavailable"
            },
            status,
        )
    })?;
    UserRow::upsert_verified(db, &identity.github_id.to_string(), &identity.login)
        .await
        .map_err(|error| {
            tracing::error!("Identity persistence failed: {error}");
            err("internal error", StatusCode::INTERNAL_SERVER_ERROR)
        })
}

/// Exchange a shared OpenBitFun GitHub session for a device-scoped relay token.
pub(crate) async fn login(
    State(state): State<AppState>,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    verifier: Option<Extension<crate::identity::IdentityVerifier>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let public_key = BASE64.decode(&body.public_key).ok();
    if !valid_device_id(&body.device_id)
        || !valid_bounded_text(&body.device_name, MAX_DEVICE_NAME_BYTES)
        || !crate::db::is_valid_device_kind(&body.device_kind)
        || !valid_login_request_id(&body.request_id)
        || !public_key
            .as_ref()
            .is_some_and(|key| key.len() == 32 && key.iter().any(|b| *b != 0))
    {
        return Err(err("invalid login parameters", StatusCode::BAD_REQUEST));
    }
    let user = verify_identity_credentials(
        &state,
        connect_info.map(|Extension(ConnectInfo(addr))| addr),
        &headers,
        &body.access_token,
        verifier.as_ref().map(|v| &v.0),
    )
    .await?;
    let db = state.db.as_ref();
    DeviceRow::upsert(
        db,
        &body.device_id,
        &user.user_id,
        &body.device_name,
        Some(&body.device_kind),
        Some(&body.public_key),
    )
    .await
    .map_err(|error| {
        err(
            "device registration failed",
            registration_error_status(&error),
        )
    })?;
    let token = AuthToken::create_idempotent(db, &user.user_id, &body.device_id, &body.request_id)
        .await
        .map_err(|error| err("token creation failed", registration_error_status(&error)))?;
    Ok(Json(AuthResponse {
        token: token.token,
        user_id: user.user_id,
    }))
}

/// `POST /api/auth/logout` — revoke the caller's token on the relay.
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> StatusCode {
    let db = state.db.as_ref();
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    let Some(token) = token else {
        return StatusCode::UNAUTHORIZED;
    };
    match AuthToken::find(db, &token).await {
        Ok(Some(auth)) => {
            let _presence_projection_guard = state.device_manager.lock_presence_projection().await;
            // The first lookup determines which lifecycle to serialize. Check
            // the exact token again under that lifecycle boundary so a
            // concurrent device deletion cannot leave this handler operating
            // on stale authorization state.
            let current = match AuthToken::find(db, &token).await {
                Ok(Some(current))
                    if current.user_id == auth.user_id
                        && current.device_id == auth.device_id
                        && current.token_kind == auth.token_kind =>
                {
                    current
                }
                _ => return StatusCode::UNAUTHORIZED,
            };
            // Delete the token row
            if let Err(error) = sqlx::query("DELETE FROM auth_tokens WHERE token = ?")
                .bind(&token)
                .execute(db)
                .await
            {
                tracing::error!(%error, "Failed to revoke account token");
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
            // A delegated control token borrows the desktop's device id but
            // does not own its live connection. Logging out that client must
            // never disconnect or mark the desktop offline.
            if current.is_device_token() {
                state.device_manager.disconnect_device_if_token(
                    &auth.user_id,
                    &auth.device_id,
                    &token,
                );
                // A failed token match means another login currently owns the
                // same machine's socket. Keep its durable presence online;
                // otherwise a rejected candidate login could make the still-
                // connected prior account appear offline.
                if !state
                    .device_manager
                    .is_device_online(&auth.user_id, &auth.device_id)
                {
                    let _ =
                        crate::db::DeviceRow::set_online(db, &auth.user_id, &auth.device_id, false)
                            .await;
                }
                drop(_presence_projection_guard);
                state
                    .device_manager
                    .broadcast_current_presence(&auth.user_id, |devices| {
                        let devices = devices
                            .iter()
                            .map(|(device_id, device_name)| {
                                crate::routes::websocket::DevicePresenceEntry {
                                    device_id: device_id.clone(),
                                    device_name: device_name.clone(),
                                }
                            })
                            .collect();
                        serde_json::to_string(
                            &crate::routes::websocket::OutboundProtocol::DevicePresence { devices },
                        )
                        .ok()
                    });
            } else {
                drop(_presence_projection_guard);
            }
            tracing::info!("Account token revoked for device_id={}", auth.device_id);
            StatusCode::NO_CONTENT
        }
        _ => StatusCode::UNAUTHORIZED,
    }
}

/// `POST /api/auth/delegate` — the caller (an already-authenticated desktop)
/// requests a new token for the same account, to be delegated to a paired
/// mobile-web or IM bot client. Returns `{token, user_id}`.
///
/// The delegated token carries the same `user_id` and references the caller's
/// `device_id` for lifetime tracking, but is limited to device discovery and
/// RPC. It cannot open a device WebSocket, mint more credentials, delete a
/// device, or access account sync/page APIs.
#[derive(Deserialize)]
pub struct DelegateRequest {
    pub public_key: String,
}

pub async fn delegate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DelegateRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    let db = state.db.as_ref();
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let auth = AuthToken::find(db, &token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !auth.is_device_token() {
        return Err(StatusCode::FORBIDDEN);
    }

    let public_key = BASE64.decode(&body.public_key).ok();
    if !public_key
        .as_ref()
        .is_some_and(|key| key.len() == 32 && key.iter().any(|byte| *byte != 0))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Issue a capability-limited token for the same account and bind its
    // lifetime to the delegating device row.
    let new_token =
        AuthToken::create_keyed_delegated(db, &auth.user_id, &auth.device_id, &body.public_key)
            .await
            .map_err(|error| registration_error_status(&error))?;

    tracing::info!(
        "Delegated token for user_id={} device_id={}",
        auth.user_id,
        auth.device_id
    );

    Ok(Json(AuthResponse {
        token: new_token.token,
        user_id: auth.user_id,
    }))
}

/// `POST /api/auth/provision-device` — mint a full device credential for a
/// distinct machine during an authenticated one-click SSH bootstrap.
///
/// Only a full device token can perform this operation. The endpoint never
/// receives the account master key; the controller sends that end-to-end over
/// its already trusted SSH channel after this call succeeds.
pub async fn provision_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ProvisionDeviceRequest>,
) -> Result<Json<ProvisionDeviceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let public_key = BASE64.decode(&body.public_key).ok();
    if !public_key
        .as_ref()
        .is_some_and(|key| key.len() == 32 && key.iter().any(|b| *b != 0))
        || body.device_id.len() != 32
        || !body
            .device_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        || !valid_bounded_text(&body.device_name, MAX_DEVICE_NAME_BYTES)
        || !valid_optional_device_kind(body.device_kind.as_deref())
        || !valid_login_request_id(&body.request_id)
    {
        return Err(err(
            "invalid device provisioning parameters",
            StatusCode::BAD_REQUEST,
        ));
    }

    let db = state.db.as_ref();
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| err("unauthorized", StatusCode::UNAUTHORIZED))?;
    let auth = AuthToken::find(db, token)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Failed to authenticate device provisioning");
            err("internal error", StatusCode::INTERNAL_SERVER_ERROR)
        })?
        .ok_or_else(|| err("unauthorized", StatusCode::UNAUTHORIZED))?;
    if !auth.is_device_token() {
        return Err(err("forbidden", StatusCode::FORBIDDEN));
    }

    // This route only ever bootstraps a machine over SSH, so an unreported
    // kind is a desktop rather than an unknown.
    let provisioned = AuthToken::provision_new_device(
        db,
        &auth.user_id,
        &body.device_id,
        body.device_name.trim(),
        Some(
            body.device_kind
                .as_deref()
                .unwrap_or(crate::db::DEVICE_KIND_DESKTOP),
        ),
        &body.request_id,
        &body.public_key,
    )
    .await
    .map_err(|error| {
        tracing::error!(%error, "Failed to provision account device");
        err(
            "device registration unavailable",
            registration_error_status(&error),
        )
    })?
    .ok_or_else(|| {
        err(
            "device is already registered on this account",
            StatusCode::CONFLICT,
        )
    })?;

    tracing::info!(
        user_id = %auth.user_id,
        device_id = %body.device_id,
        "Provisioned account device over authenticated SSH bootstrap"
    );
    Ok(Json(ProvisionDeviceResponse {
        token: provisioned.token,
        user_id: auth.user_id,
        device_id: body.device_id,
    }))
}

/// Validated principal extracted from the bearer token.
pub struct AuthUser {
    pub user_id: String,
    #[allow(dead_code)]
    pub device_id: String,
}

/// Validate the bearer token in `headers`; returns the owning user/device.
pub async fn validate_auth(state: &AppState, headers: &HeaderMap) -> Result<AuthUser, StatusCode> {
    let token = extract_bearer_token(headers).ok_or(StatusCode::UNAUTHORIZED)?;
    validate_token(state, &token).await
}

/// Extract `Bearer` token from the `Authorization` header.
pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Validate a raw token string against the account database.
pub async fn validate_token(state: &AppState, token: &str) -> Result<AuthUser, StatusCode> {
    let db = state.db.as_ref();
    let auth = AuthToken::find(db, token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !auth.is_device_token() {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(AuthUser {
        user_id: auth.user_id,
        device_id: auth.device_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{connect, DbPool};
    use crate::MemoryAssetStore;
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request};
    use std::sync::Arc;
    use tower::ServiceExt;

    #[test]
    fn forwarded_ip_is_only_trusted_from_a_local_proxy() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "198.51.100.25".parse().unwrap());

        assert_eq!(
            client_ip(&headers, Some("203.0.113.10:443".parse().unwrap())),
            "203.0.113.10"
        );
        assert_eq!(
            client_ip(&headers, Some("127.0.0.1:8080".parse().unwrap())),
            "198.51.100.25"
        );
    }

    async fn setup_app() -> (axum::Router, Arc<DbPool>, String) {
        let db = Arc::new(connect(":memory:").await.unwrap());
        UserRow::create(&db, "owner", "alice").await.unwrap();
        DeviceRow::upsert(&db, "owner-device", "owner", "Owner", None, None)
            .await
            .unwrap();
        let token = AuthToken::create(&db, "owner", "owner-device")
            .await
            .unwrap()
            .token;
        let app = crate::build_relay_router(
            Arc::new(MemoryAssetStore::new()),
            std::time::Instant::now(),
            db.clone(),
            "test",
        );
        (app, db, token)
    }

    async fn post(app: &axum::Router, path: &str, token: &str) -> axum::response::Response {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(if path == "/api/auth/delegate" {
                        serde_json::json!({"public_key": BASE64.encode([9u8; 32])}).to_string()
                    } else {
                        "{}".to_string()
                    }))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn post_json(
        app: &axum::Router,
        path: &str,
        token: &str,
        body: serde_json::Value,
    ) -> axum::response::Response {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn github_login_uses_verified_id_and_preserves_device_key_on_reconnect() {
        let (app, db, _) = setup_app().await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/me", listener.local_addr().unwrap());
        let authority = axum::Router::new().route(
            "/me",
            axum::routing::get(|headers: HeaderMap| async move {
                if headers
                    .get(header::AUTHORIZATION)
                    .and_then(|h| h.to_str().ok())
                    != Some("Bearer shared-account-token")
                {
                    return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({})));
                }
                (
                    StatusCode::OK,
                    Json(serde_json::json!({"user":{"githubId":123,"login":"github-user"}})),
                )
            }),
        );
        let task = tokio::spawn(async move {
            axum::serve(listener, authority).await.unwrap();
        });
        let app = app.layer(Extension(
            crate::identity::IdentityVerifier::with_url(&url).unwrap(),
        ));
        let public_key = BASE64.encode([9u8; 32]);
        let request = serde_json::json!({
            "access_token":"shared-account-token", "user_id":"attacker-chosen-id",
            "device_id":"new-device", "device_name":"Laptop", "device_kind":"desktop",
            "public_key":public_key, "request_id":uuid::Uuid::new_v4().to_string(),
        });
        let first = post_json(&app, "/api/auth/login", "", request.clone()).await;
        assert_eq!(first.status(), StatusCode::OK);
        let first: serde_json::Value =
            serde_json::from_slice(&to_bytes(first.into_body(), 16384).await.unwrap()).unwrap();
        assert_eq!(first["user_id"], "123");
        let second = post_json(&app, "/api/auth/login", "", request.clone()).await;
        let second: serde_json::Value =
            serde_json::from_slice(&to_bytes(second.into_body(), 16384).await.unwrap()).unwrap();
        assert_eq!(second["token"], first["token"]);
        DeviceRow::upsert(&db, "new-device", "123", "Laptop", None, None)
            .await
            .unwrap();
        let devices = DeviceRow::list_by_user(&db, "123").await.unwrap();
        assert_eq!(devices[0].public_key.as_deref(), Some(public_key.as_str()));
        let mut invalid = request;
        invalid["access_token"] = serde_json::json!("expired");
        invalid["device_id"] = serde_json::json!("must-not-register");
        assert_eq!(
            post_json(&app, "/api/auth/login", "", invalid)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(DeviceRow::list_by_user(&db, "123").await.unwrap().len(), 1);
        assert_eq!(
            post_json(
                &app,
                "/api/auth/login/challenge",
                "",
                serde_json::json!({"username":"alice"})
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        task.abort();
    }

    #[tokio::test]
    async fn device_provisioning_is_full_scope_idempotent_and_device_only() {
        let (app, db, device_token) = setup_app().await;
        let request_id = uuid::Uuid::new_v4().to_string();
        let device_id = "ab".repeat(16);
        let request = serde_json::json!({
            "device_id": device_id,
            "device_name": "SSH Build Host",
            "public_key": BASE64.encode([9u8; 32]),
            "request_id": request_id,
        });

        let first = post_json(
            &app,
            "/api/auth/provision-device",
            &device_token,
            request.clone(),
        )
        .await;
        assert_eq!(first.status(), StatusCode::OK);
        let first_body = to_bytes(first.into_body(), 16 * 1024).await.unwrap();
        let first: ProvisionDeviceResponse = serde_json::from_slice(&first_body).unwrap();
        assert_eq!(first.user_id, "owner");
        assert_eq!(first.device_id, device_id);
        let issued = AuthToken::find(&db, &first.token).await.unwrap().unwrap();
        assert!(issued.is_device_token());
        assert_eq!(issued.device_id, device_id);

        let replay = post_json(&app, "/api/auth/provision-device", &device_token, request).await;
        assert_eq!(replay.status(), StatusCode::OK);
        let replay_body = to_bytes(replay.into_body(), 16 * 1024).await.unwrap();
        let replay: ProvisionDeviceResponse = serde_json::from_slice(&replay_body).unwrap();
        assert_eq!(replay.token, first.token);

        let conflict = post_json(
            &app,
            "/api/auth/provision-device",
            &device_token,
            serde_json::json!({
                "device_id": device_id,
                "device_name": "SSH Build Host",
            "public_key": BASE64.encode([9u8; 32]),
                "request_id": uuid::Uuid::new_v4().to_string(),
            }),
        )
        .await;
        assert_eq!(conflict.status(), StatusCode::CONFLICT);

        let delegated_response = post(&app, "/api/auth/delegate", &device_token).await;
        let delegated_body = to_bytes(delegated_response.into_body(), 16 * 1024)
            .await
            .unwrap();
        let delegated_token = serde_json::from_slice::<serde_json::Value>(&delegated_body).unwrap()
            ["token"]
            .as_str()
            .unwrap()
            .to_string();
        let forbidden = post_json(
            &app,
            "/api/auth/provision-device",
            &delegated_token,
            serde_json::json!({
                "device_id": "cd".repeat(16),
                "device_name": "Another Host",
                "public_key": BASE64.encode([9u8; 32]),
                "request_id": uuid::Uuid::new_v4().to_string(),
            }),
        )
        .await;
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn delegated_tokens_cannot_chain_or_log_out_the_parent_device() {
        let (app, db, device_token) = setup_app().await;

        let response = post(&app, "/api/auth/delegate", &device_token).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
        let delegated_token = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["token"]
            .as_str()
            .unwrap()
            .to_string();
        let delegated = AuthToken::find(&db, &delegated_token)
            .await
            .unwrap()
            .unwrap();
        assert!(!delegated.is_device_token());

        assert_eq!(
            post(&app, "/api/auth/delegate", &delegated_token)
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
        DeviceRow::set_online(&db, "owner", "owner-device", true)
            .await
            .unwrap();
        assert_eq!(
            post(&app, "/api/auth/logout", &delegated_token)
                .await
                .status(),
            StatusCode::NO_CONTENT
        );
        assert!(AuthToken::find(&db, &device_token).await.unwrap().is_some());
        let devices = DeviceRow::list_by_user(&db, "owner").await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].online, 1);
    }

    #[test]
    fn exact_idempotent_login_replays_consume_one_rate_limit_slot() {
        let limiter = LoginRateLimiter::new();
        for _ in 0..20 {
            assert!(limiter.check_and_record("credentials", "127.0.0.1", 2, Some("same")));
        }
        assert!(limiter.check_and_record("credentials", "127.0.0.1", 2, Some("second")));
        assert!(!limiter.check_and_record("credentials", "127.0.0.1", 2, Some("third")));

        // Challenge traffic has its own budget and cannot exhaust credential
        // verification for the same client IP.
        assert!(limiter.check_and_record("challenge", "127.0.0.1", 1, None));
    }
}

fn registration_error_status(error: &anyhow::Error) -> StatusCode {
    if error.chain().any(|cause| {
        let text = cause.to_string();
        text.contains("account device quota exceeded")
            || text.contains("account token quota exceeded")
    }) {
        StatusCode::TOO_MANY_REQUESTS
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}
