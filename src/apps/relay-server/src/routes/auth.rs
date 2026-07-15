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

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::db::{AuthToken, DeviceRow, UserRow};
use crate::routes::api::AppState;

/// Max login attempts per IP per minute (across all accounts — stops
/// credential-stuffing where one IP tries many usernames).
const MAX_LOGIN_ATTEMPTS_PER_MIN: usize = 10;
/// Max challenge requests per IP per minute (stops bulk salt harvesting).
const MAX_CHALLENGE_PER_MIN: usize = 20;

// ── IP rate limiter (sliding window, in-memory) ─────────────────────────

/// Per-IP sliding-window rate limiter. In-memory only; resets on restart,
/// which is acceptable for brute-force throttling (the account lockout in the
/// DB is the durable backstop).
pub struct LoginRateLimiter {
    attempts: DashMap<String, Vec<i64>>,
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self {
            attempts: DashMap::new(),
        }
    }

    /// Record an attempt for `ip` and return `true` if the IP is still under
    /// the per-minute limit (i.e. the attempt is allowed).
    fn check_and_record(&self, ip: &str, max_per_min: usize) -> bool {
        let now = Utc::now().timestamp();
        let cutoff = now - 60;
        let mut entry = self.attempts.entry(ip.to_string()).or_default();
        let timestamps = entry.value_mut();
        timestamps.retain(|t| *t > cutoff);
        if timestamps.len() >= max_per_min {
            return false;
        }
        timestamps.push(now);
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
fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
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

#[derive(Serialize)]
pub struct LoginChallengeResponse {
    pub salt: String,
    pub kdf_salt: String,
    pub argon2_params: String,
    pub wrapped_master_key: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password_hash: String,
    pub device_id: String,
    pub device_name: String,
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

// ── Handlers ────────────────────────────────────────────────────────────

/// `POST /api/auth/login/challenge` — fetch KDF params + wrapped master key
/// so the client can derive the KEK locally and attempt decryption.
pub async fn login_challenge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LoginChallengeRequest>,
) -> Result<Json<LoginChallengeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let Some(db) = state.db.as_ref() else {
        return Err(err(
            "account features disabled",
            StatusCode::NOT_IMPLEMENTED,
        ));
    };

    let ip = client_ip(&headers);
    if !state
        .login_rate_limiter
        .check_and_record(&ip, MAX_CHALLENGE_PER_MIN)
    {
        return Err(err(
            "too many requests, try later",
            StatusCode::TOO_MANY_REQUESTS,
        ));
    }

    let user = UserRow::find_by_username(db, body.username.trim())
        .await
        .map_err(|e| {
            tracing::error!("challenge: db error: {e}");
            err("internal error", StatusCode::INTERNAL_SERVER_ERROR)
        })?
        .ok_or_else(|| err("user not found", StatusCode::NOT_FOUND))?;

    Ok(Json(LoginChallengeResponse {
        salt: user.salt,
        kdf_salt: user.kdf_salt,
        argon2_params: user.argon2_params,
        wrapped_master_key: user.wrapped_master_key,
    }))
}

/// `POST /api/auth/login` — verify the password hash and issue a token.
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let Some(db) = state.db.as_ref() else {
        return Err(err(
            "account features disabled",
            StatusCode::NOT_IMPLEMENTED,
        ));
    };

    let ip = client_ip(&headers);
    if !state
        .login_rate_limiter
        .check_and_record(&ip, MAX_LOGIN_ATTEMPTS_PER_MIN)
    {
        return Err(err(
            "too many login attempts from this IP",
            StatusCode::TOO_MANY_REQUESTS,
        ));
    }

    let user = UserRow::find_by_username(db, body.username.trim())
        .await
        .map_err(|e| {
            tracing::error!("login: db error: {e}");
            err("internal error", StatusCode::INTERNAL_SERVER_ERROR)
        })?
        .ok_or_else(|| err("invalid username or password", StatusCode::UNAUTHORIZED))?;

    // Account-level lockout (durable backstop).
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

    // Verify password hash (constant-time comparison not needed: these are
    // Argon2id outputs over distinct salts, compared as strings).
    if user.password_hash != body.password_hash {
        let locked_until = UserRow::record_failed_attempt(db, &user.user_id)
            .await
            .map_err(|e| {
                tracing::error!("login: failed to record attempt: {e}");
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

    // Success: reset failure counter and issue a token.
    UserRow::reset_failed_attempts(db, &user.user_id)
        .await
        .map_err(|e| {
            tracing::error!("login: failed to reset attempts: {e}");
            err("internal error", StatusCode::INTERNAL_SERVER_ERROR)
        })?;

    DeviceRow::upsert(db, &body.device_id, &user.user_id, &body.device_name, None)
        .await
        .map_err(|e| {
            tracing::error!("login: failed to upsert device: {e}");
            err("internal error", StatusCode::INTERNAL_SERVER_ERROR)
        })?;

    let token = AuthToken::create(db, &user.user_id, &body.device_id)
        .await
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
            // Delete the token row
            let _ = sqlx::query("DELETE FROM auth_tokens WHERE token = ?")
                .bind(&token)
                .execute(&**db)
                .await;
            // Mark device offline
            let _ = crate::db::DeviceRow::set_online(db, &auth.device_id, false).await;
            tracing::info!("Token revoked for device_id={}", auth.device_id);
            StatusCode::NO_CONTENT
        }
        _ => StatusCode::UNAUTHORIZED,
    }
}

/// `POST /api/auth/delegate` — the caller (an already-authenticated desktop)
/// requests a new token for the same account, to be delegated to a paired
/// mobile-web or IM bot client. Returns `{token, user_id}`.
///
/// The delegate token carries the same `user_id` as the caller's token but
/// references the caller's `device_id` (the paired desktop) for tracking.
/// The desktop is responsible for securely transmitting the token + master_key
/// to the paired client via the existing E2E room channel.
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

    // Issue a new token for the same user, same device (the desktop's).
    let new_token = AuthToken::create(db, &auth.user_id, &auth.device_id)
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
