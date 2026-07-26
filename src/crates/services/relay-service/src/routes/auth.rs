//! Account authentication endpoints for the relay server.
//!
//! The relay stays zero-knowledge: it never sees the plaintext password or
//! the master key. Clients derive a KEK from the password (Argon2id) locally,
//! wrap a random master key, and send only:
//!   - `password_hash`   (Argon2id over a separate salt, for server-side verify)
//!   - `wrapped_master_key` (AES-GCM(KEK, master_key), server stores as-is)
//!
//! Brute-force protection is layered:
//!   - per-account exponential-backoff lockout (in the `users` table)
//!   - per-IP sliding-window rate limit (in-memory)
//!   - Argon2id high parameters slow offline attacks (client-enforced)

use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::{Extension, Json};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::sync::OnceLock;
use subtle::ConstantTimeEq;

use crate::db::{AuthToken, DeviceRow, UserRow};
use crate::routes::api::AppState;

/// Max login attempts per IP per minute (across all accounts — stops
/// credential-stuffing where one IP tries many usernames).
const MAX_LOGIN_ATTEMPTS_PER_MIN: usize = 10;
/// Max challenge requests per IP per minute (stops bulk salt harvesting).
const MAX_CHALLENGE_PER_MIN: usize = 20;
const MAX_RATE_LIMIT_BUCKETS: usize = 50_000;
const MAX_USERNAME_BYTES: usize = 128;
const MAX_PASSWORD_HASH_BYTES: usize = 128;
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

fn valid_password_hash(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PASSWORD_HASH_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'-' | b'_')
        })
}

fn valid_login_request_id(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok()
}

fn decoy_login_challenge(username: &str) -> LoginChallengeResponse {
    static SECRET: OnceLock<[u8; 32]> = OnceLock::new();
    let secret = SECRET.get_or_init(rand::random);
    let material = |label: &[u8]| {
        let mut hasher = Sha256::new();
        hasher.update(secret);
        hasher.update(label);
        hasher.update(username.as_bytes());
        hasher.finalize()
    };
    let salt_material = material(b"salt");
    let kdf_salt_material = material(b"kdf-salt");
    let ciphertext_head = material(b"wrapped-key-1");
    let ciphertext_tail = material(b"wrapped-key-2");
    let nonce_material = material(b"nonce");
    let mut ciphertext = Vec::with_capacity(48);
    ciphertext.extend_from_slice(&ciphertext_head);
    ciphertext.extend_from_slice(&ciphertext_tail[..16]);

    LoginChallengeResponse {
        salt: BASE64.encode(&salt_material[..16]),
        kdf_salt: BASE64.encode(&kdf_salt_material[..16]),
        argon2_params: r#"{"m":65536,"t":3,"p":4}"#.to_string(),
        wrapped_master_key: format!(
            "{}.{}",
            BASE64.encode(ciphertext),
            BASE64.encode(&nonce_material[..12])
        ),
        login_idempotency_supported: true,
    }
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
    fn check_and_record(
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
fn client_ip(headers: &HeaderMap, peer_addr: Option<SocketAddr>) -> String {
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
pub struct LoginChallengeRequest {
    pub username: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginChallengeResponse {
    pub salt: String,
    pub kdf_salt: String,
    pub argon2_params: String,
    pub wrapped_master_key: String,
    pub login_idempotency_supported: bool,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password_hash: String,
    pub device_id: String,
    pub device_name: String,
    #[serde(default)]
    pub request_id: Option<String>,
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

pub(crate) async fn verify_password_hash_credentials(
    state: &AppState,
    peer_addr: Option<SocketAddr>,
    headers: &HeaderMap,
    username: &str,
    password_hash: &str,
    rate_limit_replay_key: Option<&str>,
) -> Result<UserRow, (StatusCode, Json<ErrorResponse>)> {
    let Some(db) = state.db.as_ref() else {
        return Err(err(
            "account features disabled",
            StatusCode::NOT_IMPLEMENTED,
        ));
    };

    let ip = client_ip(headers, peer_addr);
    if !state.login_rate_limiter.check_and_record(
        "credentials",
        &ip,
        MAX_LOGIN_ATTEMPTS_PER_MIN,
        rate_limit_replay_key,
    ) {
        return Err(err(
            "too many login attempts from this IP",
            StatusCode::TOO_MANY_REQUESTS,
        ));
    }

    if !valid_bounded_text(username, MAX_USERNAME_BYTES) || !valid_password_hash(password_hash) {
        return Err(err("invalid login parameters", StatusCode::BAD_REQUEST));
    }

    let user = UserRow::find_by_username(db, username.trim())
        .await
        .map_err(|error| {
            tracing::error!("login: db error: {error}");
            err("internal error", StatusCode::INTERNAL_SERVER_ERROR)
        })?
        .ok_or_else(|| err("invalid username or password", StatusCode::UNAUTHORIZED))?;

    if user.is_locked() {
        let retry = user.locked_until - Utc::now().timestamp();
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse {
                error: "account temporarily locked, try later".to_string(),
                retry_after_secs: Some(retry.max(0)),
            }),
        ));
    }

    // The browser or native client already paid the Argon2id cost. Compare
    // the fixed secret without a data-dependent early exit.
    let password_matches = user.password_hash.len() == password_hash.len()
        && bool::from(
            user.password_hash
                .as_bytes()
                .ct_eq(password_hash.as_bytes()),
        );
    if !password_matches {
        let locked_until = UserRow::record_failed_attempt(db, &user.user_id)
            .await
            .map_err(|error| {
                tracing::error!("login: failed to record attempt: {error}");
                err("internal error", StatusCode::INTERNAL_SERVER_ERROR)
            })?;
        let now = Utc::now().timestamp();
        if locked_until > now {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse {
                    error: "too many failed attempts, account locked".to_string(),
                    retry_after_secs: Some(locked_until - now),
                }),
            ));
        }
        return Err(err(
            "invalid username or password",
            StatusCode::UNAUTHORIZED,
        ));
    }

    UserRow::reset_failed_attempts(db, &user.user_id)
        .await
        .map_err(|error| {
            tracing::error!("login: failed to reset attempts: {error}");
            err("internal error", StatusCode::INTERNAL_SERVER_ERROR)
        })?;
    Ok(user)
}

// ── Handlers ────────────────────────────────────────────────────────────

/// `POST /api/auth/login/challenge` — fetch KDF params + wrapped master key
/// so the client can derive the KEK locally and attempt decryption.
pub async fn login_challenge(
    State(state): State<AppState>,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Json(body): Json<LoginChallengeRequest>,
) -> Result<Json<LoginChallengeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let Some(db) = state.db.as_ref() else {
        return Err(err(
            "account features disabled",
            StatusCode::NOT_IMPLEMENTED,
        ));
    };

    let ip = client_ip(
        &headers,
        connect_info.map(|Extension(ConnectInfo(addr))| addr),
    );
    if !state
        .login_rate_limiter
        .check_and_record("challenge", &ip, MAX_CHALLENGE_PER_MIN, None)
    {
        return Err(err(
            "too many requests, try later",
            StatusCode::TOO_MANY_REQUESTS,
        ));
    }

    if !valid_bounded_text(&body.username, MAX_USERNAME_BYTES) {
        return Err(err("invalid username", StatusCode::BAD_REQUEST));
    }

    let username = body.username.trim();
    let user = UserRow::find_by_username(db, username).await.map_err(|e| {
        tracing::error!("challenge: db error: {e}");
        err("internal error", StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    // Unknown accounts receive a deterministic, process-keyed decoy with the
    // same shape and KDF cost as a real challenge. The client then fails with
    // the same local "invalid username or password" path, without exposing a
    // bulk username-enumeration oracle at this endpoint.
    let Some(user) = user else {
        return Ok(Json(decoy_login_challenge(username)));
    };

    Ok(Json(LoginChallengeResponse {
        salt: user.salt,
        kdf_salt: user.kdf_salt,
        argon2_params: user.argon2_params,
        wrapped_master_key: user.wrapped_master_key,
        login_idempotency_supported: true,
    }))
}

/// `POST /api/auth/login` — verify the password hash and issue a token.
pub async fn login(
    State(state): State<AppState>,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !valid_device_id(&body.device_id)
        || !valid_bounded_text(&body.device_name, MAX_DEVICE_NAME_BYTES)
        || body
            .request_id
            .as_deref()
            .is_some_and(|request_id| !valid_login_request_id(request_id))
    {
        return Err(err("invalid login parameters", StatusCode::BAD_REQUEST));
    }
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| err("account features disabled", StatusCode::NOT_IMPLEMENTED))?;
    let rate_limit_replay_key = body.request_id.as_ref().map(|request_id| {
        let mut hasher = Sha256::new();
        for value in [
            request_id.as_str(),
            body.username.as_str(),
            body.password_hash.as_str(),
            body.device_id.as_str(),
            body.device_name.as_str(),
        ] {
            hasher.update((value.len() as u64).to_be_bytes());
            hasher.update(value.as_bytes());
        }
        BASE64.encode(hasher.finalize())
    });
    let user = verify_password_hash_credentials(
        &state,
        connect_info.map(|Extension(ConnectInfo(addr))| addr),
        &headers,
        &body.username,
        &body.password_hash,
        rate_limit_replay_key.as_deref(),
    )
    .await?;

    DeviceRow::upsert(db, &body.device_id, &user.user_id, &body.device_name, None)
        .await
        .map_err(|e| {
            tracing::error!("login: failed to upsert device: {e}");
            err("internal error", StatusCode::INTERNAL_SERVER_ERROR)
        })?;

    let token = match body.request_id.as_deref() {
        Some(request_id) => {
            AuthToken::create_idempotent(db, &user.user_id, &body.device_id, request_id).await
        }
        None => AuthToken::create(db, &user.user_id, &body.device_id).await,
    }
    .map_err(|e| {
        tracing::error!("login: failed to create token: {e}");
        err("internal error", StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    tracing::info!("Account login: user_id={}", user.user_id);
    Ok(Json(AuthResponse {
        token: token.token,
        user_id: user.user_id,
    }))
}

/// `POST /api/auth/logout` — revoke the caller's token on the relay.
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> StatusCode {
    let db = match state.db.as_ref() {
        Some(db) => db,
        None => return StatusCode::NOT_IMPLEMENTED,
    };
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
                .execute(&**db)
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
pub async fn delegate(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthResponse>, StatusCode> {
    let db = state.db.as_ref().ok_or(StatusCode::NOT_IMPLEMENTED)?;
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

    // Issue a capability-limited token for the same account and bind its
    // lifetime to the delegating device row.
    let new_token = AuthToken::create_delegated(db, &auth.user_id, &auth.device_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{connect, DbPool};
    use crate::relay::RoomManager;
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
        UserRow::create(&db, "owner", "alice", "s", "ks", "{}", "hash", "wmk")
            .await
            .unwrap();
        DeviceRow::upsert(&db, "owner-device", "owner", "Owner", None)
            .await
            .unwrap();
        let token = AuthToken::create(&db, "owner", "owner-device")
            .await
            .unwrap()
            .token;
        let app = crate::build_relay_router(
            RoomManager::new(),
            Arc::new(MemoryAssetStore::new()),
            std::time::Instant::now(),
            Some(db.clone()),
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
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
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
        let sync_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/sync/settings")
                    .header(header::AUTHORIZATION, format!("Bearer {delegated_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(sync_response.status(), StatusCode::FORBIDDEN);

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

    #[tokio::test]
    async fn unknown_login_challenge_has_a_stable_valid_decoy_shape() {
        let (app, _db, _token) = setup_app().await;
        let request = || {
            Request::builder()
                .method("POST")
                .uri("/api/auth/login/challenge")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"username":"missing-user"}"#))
                .unwrap()
        };

        let first = app.clone().oneshot(request()).await.unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        let first_body = to_bytes(first.into_body(), 16 * 1024).await.unwrap();
        let first_json: LoginChallengeResponse = serde_json::from_slice(&first_body).unwrap();
        assert!(first_json.login_idempotency_supported);
        assert_eq!(BASE64.decode(&first_json.salt).unwrap().len(), 16);
        assert_eq!(BASE64.decode(&first_json.kdf_salt).unwrap().len(), 16);
        let (ciphertext, nonce) = first_json.wrapped_master_key.split_once('.').unwrap();
        assert_eq!(BASE64.decode(ciphertext).unwrap().len(), 48);
        assert_eq!(BASE64.decode(nonce).unwrap().len(), 12);

        let second = app.oneshot(request()).await.unwrap();
        let second_body = to_bytes(second.into_body(), 16 * 1024).await.unwrap();
        assert_eq!(first_body, second_body);
    }

    #[tokio::test]
    async fn login_request_id_reuses_the_issued_token() {
        let (app, db, _token) = setup_app().await;
        let request_id = uuid::Uuid::new_v4().to_string();
        let request = || {
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "alice",
                        "password_hash": "hash",
                        "device_id": "retry-device",
                        "device_name": "Retry Device",
                        "request_id": request_id,
                    })
                    .to_string(),
                ))
                .unwrap()
        };

        let first = app.clone().oneshot(request()).await.unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        let first_body = to_bytes(first.into_body(), 16 * 1024).await.unwrap();
        let second = app.oneshot(request()).await.unwrap();
        assert_eq!(second.status(), StatusCode::OK);
        let second_body = to_bytes(second.into_body(), 16 * 1024).await.unwrap();
        let first_token = serde_json::from_slice::<serde_json::Value>(&first_body).unwrap()
            ["token"]
            .as_str()
            .unwrap()
            .to_string();
        let second_token = serde_json::from_slice::<serde_json::Value>(&second_body).unwrap()
            ["token"]
            .as_str()
            .unwrap()
            .to_string();

        assert_eq!(first_token, second_token);
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM auth_tokens WHERE request_id = ?")
            .bind(&request_id)
            .fetch_one(&*db)
            .await
            .unwrap();
        assert_eq!(count.0, 1);
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
