use crate::config::MarketConfig;
use crate::db::{token_hash, AuthenticatedUser, Database};
use crate::error::{MarketError, MarketResult};
use axum::http::{header, HeaderMap, HeaderValue};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{Duration, Utc};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use url::Url;
use uuid::Uuid;

const WEB_SESSION_COOKIE: &str = "openbitfun_market_session";
const CSRF_COOKIE: &str = "openbitfun_market_csrf";
const SKIN_SESSION_COOKIE: &str = "openbitfun_skin_session";
const SKIN_CSRF_COOKIE: &str = "openbitfun_skin_csrf";
const MINIAPP_COOKIE_PATH: &str = "/miniapp";
const SKIN_COOKIE_PATH: &str = "/skin";
const OAUTH_FLOW_MINUTES: i64 = 10;
const WEB_SESSION_DAYS: i64 = 7;
const ACCESS_TOKEN_MINUTES: i64 = 15;
const REFRESH_TOKEN_DAYS: i64 = 30;

#[derive(Debug, Clone)]
pub(crate) struct AuthService {
    config: MarketConfig,
    db: Database,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub(crate) struct RequestAuth {
    pub user: AuthenticatedUser,
    pub kind: RequestAuthKind,
}

#[derive(Debug, Clone)]
pub(crate) enum RequestAuthKind {
    Web {
        session_token: String,
        csrf_hash: String,
        expires_at: i64,
        surface: WebSessionSurface,
    },
    Bearer {
        family_id: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum WebSessionSurface {
    MiniApp,
    Skin,
}

impl WebSessionSurface {
    fn session_cookie(self) -> &'static str {
        match self {
            Self::MiniApp => WEB_SESSION_COOKIE,
            Self::Skin => SKIN_SESSION_COOKIE,
        }
    }

    fn csrf_cookie(self) -> &'static str {
        match self {
            Self::MiniApp => CSRF_COOKIE,
            Self::Skin => SKIN_CSRF_COOKIE,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum CompletedOAuth {
    Web {
        return_to: String,
        session_token: String,
        csrf_token: String,
        expires_at: i64,
    },
    Desktop,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopAuthStart {
    pub transaction_id: String,
    pub transaction_secret: String,
    pub authorization_url: String,
    pub expires_at: i64,
    pub poll_interval_seconds: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopAuthPollRequest {
    pub transaction_id: String,
    pub transaction_secret: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopAuthPollResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<MarketTokenPair>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MarketTokenPair {
    pub access_token: String,
    pub access_expires_at: i64,
    pub refresh_token: String,
    pub refresh_expires_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubUser {
    id: i64,
    login: String,
    avatar_url: String,
}

#[derive(Debug)]
struct OAuthFlowRecord {
    flow_kind: String,
    transaction_id: Option<String>,
    code_verifier: String,
    return_to: String,
}

impl AuthService {
    pub(crate) fn new(config: MarketConfig, db: Database) -> MarketResult<Self> {
        openbitfun_services_core::tls_provider::ensure_ring_crypto_provider();
        let client = reqwest::Client::builder()
            .user_agent("OpenBitFun-MiniApp-Market/1")
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(MarketError::internal)?;
        Ok(Self { config, db, client })
    }

    pub(crate) async fn optional_auth(
        &self,
        headers: &HeaderMap,
    ) -> MarketResult<Option<RequestAuth>> {
        if let Some(token) = bearer_token(headers) {
            let Some((user, family_id)) = self.db.api_token_user(token, "access").await? else {
                return Err(MarketError::unauthorized(
                    "The marketplace access token is invalid or expired.",
                ));
            };
            return Ok(Some(RequestAuth {
                user,
                kind: RequestAuthKind::Bearer { family_id },
            }));
        }
        for surface in [WebSessionSurface::MiniApp, WebSessionSurface::Skin] {
            let Some(token) = cookie_value(headers, surface.session_cookie()) else {
                continue;
            };
            let Some((user, csrf_hash, expires_at)) = self.db.web_session_user(&token).await?
            else {
                continue;
            };
            return Ok(Some(RequestAuth {
                user,
                kind: RequestAuthKind::Web {
                    session_token: token,
                    csrf_hash,
                    expires_at,
                    surface,
                },
            }));
        }
        Ok(None)
    }

    pub(crate) async fn require_auth(&self, headers: &HeaderMap) -> MarketResult<RequestAuth> {
        self.optional_auth(headers)
            .await?
            .ok_or_else(|| MarketError::unauthorized("Sign in with GitHub to continue."))
    }

    pub(crate) fn require_csrf(&self, headers: &HeaderMap, auth: &RequestAuth) -> MarketResult<()> {
        let RequestAuthKind::Web {
            csrf_hash, surface, ..
        } = &auth.kind
        else {
            return Ok(());
        };
        let header_token = headers
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        let cookie_token = cookie_value(headers, surface.csrf_cookie()).unwrap_or_default();
        if header_token.is_empty()
            || header_token != cookie_token
            || token_hash(header_token) != *csrf_hash
        {
            return Err(MarketError::forbidden(
                "The CSRF token is missing or invalid.",
            ));
        }
        Ok(())
    }

    pub(crate) fn is_admin(&self, user: &AuthenticatedUser) -> bool {
        self.config
            .admin_github_ids
            .contains(&user.profile.github_id)
    }

    pub(crate) async fn start_web_oauth(&self, return_to: &str) -> MarketResult<String> {
        let return_to = safe_return_to(return_to);
        self.create_oauth_flow("web", None, &return_to).await
    }

    pub(crate) async fn start_desktop_oauth(&self) -> MarketResult<DesktopAuthStart> {
        self.ensure_github_configured()?;
        let transaction_id = Uuid::new_v4().to_string();
        let transaction_secret = random_token(32);
        let now = Utc::now().timestamp();
        let expires_at = (Utc::now() + Duration::minutes(OAUTH_FLOW_MINUTES)).timestamp();
        sqlx::query(
            "INSERT INTO desktop_auth_transactions(
                id, secret_hash, status, expires_at, created_at, updated_at
             ) VALUES(?, ?, 'pending', ?, ?, ?)",
        )
        .bind(&transaction_id)
        .bind(token_hash(&transaction_secret))
        .bind(expires_at)
        .bind(now)
        .bind(now)
        .execute(self.db.pool())
        .await
        .map_err(MarketError::internal)?;
        let authorization_url = self
            .create_oauth_flow(
                "desktop",
                Some(&transaction_id),
                "/miniapp/auth/desktop-complete",
            )
            .await?;
        Ok(DesktopAuthStart {
            transaction_id,
            transaction_secret,
            authorization_url,
            expires_at,
            poll_interval_seconds: 3,
        })
    }

    async fn create_oauth_flow(
        &self,
        kind: &str,
        transaction_id: Option<&str>,
        return_to: &str,
    ) -> MarketResult<String> {
        self.ensure_github_configured()?;
        let state = random_token(32);
        let verifier = random_token(48);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let now = Utc::now().timestamp();
        let expires_at = (Utc::now() + Duration::minutes(OAUTH_FLOW_MINUTES)).timestamp();
        sqlx::query(
            "INSERT INTO oauth_flows(
                state_hash, flow_kind, transaction_id, code_verifier, return_to, expires_at, created_at
             ) VALUES(?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(token_hash(&state))
        .bind(kind)
        .bind(transaction_id)
        .bind(&verifier)
        .bind(return_to)
        .bind(expires_at)
        .bind(now)
        .execute(self.db.pool())
        .await
        .map_err(MarketError::internal)?;

        let mut url = Url::parse("https://github.com/login/oauth/authorize")
            .map_err(MarketError::internal)?;
        url.query_pairs_mut()
            .append_pair(
                "client_id",
                self.config.github_client_id.as_deref().unwrap_or_default(),
            )
            .append_pair("redirect_uri", &self.config.github_callback_url())
            .append_pair("scope", "")
            .append_pair("state", &state)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");
        Ok(url.to_string())
    }

    pub(crate) async fn complete_oauth(
        &self,
        code: &str,
        state: &str,
    ) -> MarketResult<CompletedOAuth> {
        self.ensure_github_configured()?;
        let flow = self.consume_oauth_flow(state).await?;
        let github_user = self.exchange_github_code(code, &flow.code_verifier).await?;
        let user = self
            .db
            .upsert_github_user(github_user.id, &github_user.login, &github_user.avatar_url)
            .await?;

        if flow.flow_kind == "desktop" {
            let transaction_id = flow.transaction_id.ok_or_else(|| {
                MarketError::internal("Desktop OAuth flow is missing its transaction")
            })?;
            let updated = sqlx::query(
                "UPDATE desktop_auth_transactions
                 SET status = 'authorized', user_id = ?, updated_at = ?
                 WHERE id = ? AND status = 'pending' AND expires_at > ?",
            )
            .bind(user.internal_id)
            .bind(Utc::now().timestamp())
            .bind(&transaction_id)
            .bind(Utc::now().timestamp())
            .execute(self.db.pool())
            .await
            .map_err(MarketError::internal)?;
            if updated.rows_affected() != 1 {
                return Err(MarketError::bad_request(
                    "desktop_auth_expired",
                    "The desktop authorization request has expired.",
                ));
            }
            return Ok(CompletedOAuth::Desktop);
        }

        let session_token = random_token(32);
        let csrf_token = random_token(24);
        let expires_at = (Utc::now() + Duration::days(WEB_SESSION_DAYS)).timestamp();
        self.db
            .create_web_session(user.internal_id, &session_token, &csrf_token, expires_at)
            .await?;
        Ok(CompletedOAuth::Web {
            return_to: flow.return_to,
            session_token,
            csrf_token,
            expires_at,
        })
    }

    async fn consume_oauth_flow(&self, state: &str) -> MarketResult<OAuthFlowRecord> {
        let mut transaction = self
            .db
            .pool()
            .begin()
            .await
            .map_err(MarketError::internal)?;
        let flow = sqlx::query(
            "SELECT flow_kind, transaction_id, code_verifier, return_to
             FROM oauth_flows WHERE state_hash = ? AND expires_at > ?",
        )
        .bind(token_hash(state))
        .bind(Utc::now().timestamp())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(MarketError::internal)?
        .ok_or_else(|| {
            MarketError::bad_request("invalid_oauth_state", "OAuth state is invalid or expired.")
        })?;
        sqlx::query("DELETE FROM oauth_flows WHERE state_hash = ?")
            .bind(token_hash(state))
            .execute(&mut *transaction)
            .await
            .map_err(MarketError::internal)?;
        transaction.commit().await.map_err(MarketError::internal)?;
        Ok(OAuthFlowRecord {
            flow_kind: flow.get("flow_kind"),
            transaction_id: flow.get("transaction_id"),
            code_verifier: flow.get("code_verifier"),
            return_to: flow.get("return_to"),
        })
    }

    pub(crate) async fn poll_desktop(
        &self,
        request: DesktopAuthPollRequest,
    ) -> MarketResult<DesktopAuthPollResponse> {
        let row = sqlx::query(
            "SELECT status, user_id, secret_hash, expires_at
             FROM desktop_auth_transactions WHERE id = ?",
        )
        .bind(&request.transaction_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(MarketError::internal)?
        .ok_or_else(|| MarketError::not_found("Desktop authorization request was not found."))?;
        let expected_hash: String = row.get("secret_hash");
        if token_hash(&request.transaction_secret) != expected_hash {
            return Err(MarketError::unauthorized(
                "The desktop authorization secret is invalid.",
            ));
        }
        let expires_at: i64 = row.get("expires_at");
        if expires_at <= Utc::now().timestamp() {
            return Ok(DesktopAuthPollResponse {
                status: "expired".to_string(),
                tokens: None,
            });
        }
        let status: String = row.get("status");
        if status != "authorized" {
            return Ok(DesktopAuthPollResponse {
                status,
                tokens: None,
            });
        }
        let user_id: i64 = row
            .try_get("user_id")
            .map_err(|_| MarketError::internal("Authorized transaction has no user"))?;
        let updated = sqlx::query(
            "UPDATE desktop_auth_transactions SET status = 'consumed', updated_at = ?
             WHERE id = ? AND status = 'authorized'",
        )
        .bind(Utc::now().timestamp())
        .bind(&request.transaction_id)
        .execute(self.db.pool())
        .await
        .map_err(MarketError::internal)?;
        if updated.rows_affected() != 1 {
            return Err(MarketError::conflict(
                "desktop_auth_consumed",
                "The desktop authorization was already consumed.",
            ));
        }
        let tokens = self.issue_token_pair(user_id, None).await?;
        Ok(DesktopAuthPollResponse {
            status: "authorized".to_string(),
            tokens: Some(tokens),
        })
    }

    pub(crate) async fn refresh_tokens(
        &self,
        refresh_token: &str,
    ) -> MarketResult<MarketTokenPair> {
        let Some((user, family_id)) = self.db.api_token_user(refresh_token, "refresh").await?
        else {
            return Err(MarketError::unauthorized(
                "The refresh token is invalid or expired.",
            ));
        };
        self.db.revoke_token_family(&family_id).await?;
        self.issue_token_pair(user.internal_id, Some(family_id))
            .await
    }

    async fn issue_token_pair(
        &self,
        user_id: i64,
        family_id: Option<String>,
    ) -> MarketResult<MarketTokenPair> {
        let access_token = random_token(32);
        let refresh_token = random_token(48);
        let family_id = family_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let access_expires_at = (Utc::now() + Duration::minutes(ACCESS_TOKEN_MINUTES)).timestamp();
        let refresh_expires_at = (Utc::now() + Duration::days(REFRESH_TOKEN_DAYS)).timestamp();
        self.db
            .create_api_token(
                user_id,
                &access_token,
                "access",
                &family_id,
                access_expires_at,
            )
            .await?;
        self.db
            .create_api_token(
                user_id,
                &refresh_token,
                "refresh",
                &family_id,
                refresh_expires_at,
            )
            .await?;
        Ok(MarketTokenPair {
            access_token,
            access_expires_at,
            refresh_token,
            refresh_expires_at,
        })
    }

    pub(crate) async fn logout(&self, auth: &RequestAuth) -> MarketResult<()> {
        match &auth.kind {
            RequestAuthKind::Web { session_token, .. } => {
                self.db.delete_web_session(session_token).await
            }
            RequestAuthKind::Bearer { family_id } => self.db.revoke_token_family(family_id).await,
        }
    }

    pub(crate) fn append_web_session_cookies(
        &self,
        headers: &mut HeaderMap,
        session_token: &str,
        csrf_token: &str,
        expires_at: i64,
    ) -> MarketResult<()> {
        let max_age = (expires_at - Utc::now().timestamp()).max(0);
        let secure = if self.config.public_base_url.starts_with("https://") {
            "; Secure"
        } else {
            ""
        };
        for (session_cookie, csrf_cookie, path) in [
            (WEB_SESSION_COOKIE, CSRF_COOKIE, MINIAPP_COOKIE_PATH),
            (SKIN_SESSION_COOKIE, SKIN_CSRF_COOKIE, SKIN_COOKIE_PATH),
        ] {
            append_set_cookie(
                headers,
                &format!(
                    "{session_cookie}={session_token}; Path={path}; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}"
                ),
            )?;
            append_set_cookie(
                headers,
                &format!(
                    "{csrf_cookie}={csrf_token}; Path={path}; Max-Age={max_age}; SameSite=Lax{secure}"
                ),
            )?;
        }
        Ok(())
    }

    pub(crate) fn append_shared_account_cookies(
        &self,
        response_headers: &mut HeaderMap,
        request_headers: &HeaderMap,
        auth: &RequestAuth,
    ) -> MarketResult<()> {
        let RequestAuthKind::Web {
            session_token,
            csrf_hash,
            expires_at,
            surface,
        } = &auth.kind
        else {
            return Ok(());
        };
        let Some(csrf_token) = cookie_value(request_headers, surface.csrf_cookie()) else {
            return Ok(());
        };
        if token_hash(&csrf_token) != *csrf_hash {
            return Ok(());
        }
        self.append_web_session_cookies(response_headers, session_token, &csrf_token, *expires_at)
    }

    pub(crate) fn append_clear_cookies(&self, headers: &mut HeaderMap) -> MarketResult<()> {
        let secure = if self.config.public_base_url.starts_with("https://") {
            "; Secure"
        } else {
            ""
        };
        for (session_cookie, csrf_cookie, path) in [
            (WEB_SESSION_COOKIE, CSRF_COOKIE, MINIAPP_COOKIE_PATH),
            (SKIN_SESSION_COOKIE, SKIN_CSRF_COOKIE, SKIN_COOKIE_PATH),
        ] {
            append_set_cookie(
                headers,
                &format!(
                    "{session_cookie}=; Path={path}; Max-Age=0; HttpOnly; SameSite=Lax{secure}"
                ),
            )?;
            append_set_cookie(
                headers,
                &format!("{csrf_cookie}=; Path={path}; Max-Age=0; SameSite=Lax{secure}"),
            )?;
        }
        Ok(())
    }

    async fn exchange_github_code(&self, code: &str, verifier: &str) -> MarketResult<GitHubUser> {
        let token_response = self
            .client
            .post("https://github.com/login/oauth/access_token")
            .header(header::ACCEPT, "application/json")
            .form(&[
                (
                    "client_id",
                    self.config.github_client_id.as_deref().unwrap_or_default(),
                ),
                (
                    "client_secret",
                    self.config
                        .github_client_secret
                        .as_deref()
                        .unwrap_or_default(),
                ),
                ("code", code),
                ("redirect_uri", &self.config.github_callback_url()),
                ("code_verifier", verifier),
            ])
            .send()
            .await
            .map_err(MarketError::internal)?
            .json::<GitHubTokenResponse>()
            .await
            .map_err(MarketError::internal)?;
        let access_token = token_response.access_token.ok_or_else(|| {
            MarketError::bad_request(
                "github_oauth_failed",
                token_response
                    .error_description
                    .or(token_response.error)
                    .unwrap_or_else(|| "GitHub did not return an access token.".to_string()),
            )
        })?;
        self.client
            .get("https://api.github.com/user")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(MarketError::internal)?
            .error_for_status()
            .map_err(MarketError::internal)?
            .json::<GitHubUser>()
            .await
            .map_err(MarketError::internal)
    }

    fn ensure_github_configured(&self) -> MarketResult<()> {
        if self.config.github_configured() {
            Ok(())
        } else {
            Err(MarketError::service_unavailable(
                "github_oauth_not_configured",
                "GitHub sign-in is not configured on this marketplace.",
            ))
        }
    }
}

fn random_token(bytes: usize) -> String {
    let mut value = vec![0_u8; bytes];
    OsRng.fill_bytes(&mut value);
    URL_SAFE_NO_PAD.encode(value)
}

fn safe_return_to(value: &str) -> String {
    const FALLBACK: &str = "/miniapp/";
    if value.len() > 2_048
        || !value.starts_with('/')
        || value.starts_with("//")
        || value.contains('\\')
        || value.chars().any(char::is_control)
    {
        return FALLBACK.to_string();
    }
    let encoded_path = value
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if ["%00", "%0a", "%0d", "%2f", "%5c"]
        .iter()
        .any(|encoded| encoded_path.contains(encoded))
    {
        return FALLBACK.to_string();
    }
    let Ok(base) = Url::parse("https://market.openbitfun.com/") else {
        return FALLBACK.to_string();
    };
    let Ok(target) = base.join(value) else {
        return FALLBACK.to_string();
    };
    let path = target.path();
    if target.origin() == base.origin()
        && (matches!(path, "/miniapp" | "/skin")
            || path.starts_with("/miniapp/")
            || path.starts_with("/skin/"))
    {
        value.to_string()
    } else {
        FALLBACK.to_string()
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
}

pub(crate) fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let (key, value) = cookie.trim().split_once('=')?;
                (key == name).then(|| value.to_string())
            })
        })
}

fn append_set_cookie(headers: &mut HeaderMap, value: &str) -> MarketResult<()> {
    headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(value).map_err(MarketError::internal)?,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::path::Path;

    fn test_config(root: &Path) -> MarketConfig {
        MarketConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            public_base_url: "https://market.openbitfun.com/miniapp".to_string(),
            database_path: root.join("market.sqlite"),
            artifact_dir: root.join("artifacts"),
            web_dir: root.join("web"),
            github_client_id: Some("client-id".to_string()),
            github_client_secret: Some("client-secret".to_string()),
            session_secret: "test-session-secret-at-least-24".to_string(),
            admin_github_ids: HashSet::from([24753352]),
            public_browse: false,
            web_submissions_enabled: false,
        }
    }

    #[tokio::test]
    async fn oauth_flow_uses_pkce_empty_scope_and_one_time_state() {
        let temporary = tempfile::tempdir().unwrap();
        let database = Database::open(&temporary.path().join("market.sqlite"))
            .await
            .unwrap();
        let service = AuthService::new(test_config(temporary.path()), database.clone()).unwrap();

        let authorization = service
            .start_web_oauth("https://attacker.invalid/")
            .await
            .unwrap();
        let url = Url::parse(&authorization).unwrap();
        let parameters = url.query_pairs().into_owned().collect::<HashMap<_, _>>();
        let state = parameters.get("state").unwrap();
        assert_eq!(parameters.get("scope").map(String::as_str), Some(""));
        assert_eq!(
            parameters.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        let verifier: String =
            sqlx::query_scalar("SELECT code_verifier FROM oauth_flows WHERE state_hash = ?")
                .bind(token_hash(state))
                .fetch_one(database.pool())
                .await
                .unwrap();
        assert_eq!(
            parameters.get("code_challenge").unwrap(),
            &URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
        );

        let flow = service.consume_oauth_flow(state).await.unwrap();
        assert_eq!(flow.return_to, "/miniapp/");
        let replay = service.consume_oauth_flow(state).await.unwrap_err();
        assert_eq!(replay.code, "invalid_oauth_state");
    }

    #[test]
    fn oauth_return_target_accepts_only_market_surfaces() {
        assert_eq!(
            safe_return_to("/skin/appearances/ocean-night?q=dark"),
            "/skin/appearances/ocean-night?q=dark"
        );
        assert_eq!(
            safe_return_to("/miniapp/apps/reviewed-app"),
            "/miniapp/apps/reviewed-app"
        );
        assert_eq!(safe_return_to("//attacker.invalid/skin/"), "/miniapp/");
        assert_eq!(safe_return_to("/skin/../admin"), "/miniapp/");
        assert_eq!(safe_return_to("/skin/%2e%2e/admin"), "/miniapp/");
        assert_eq!(safe_return_to("/skin/%2F%2Fattacker.invalid"), "/miniapp/");
        assert_eq!(safe_return_to("/skin/%5c%5cattacker"), "/miniapp/");
        assert_eq!(safe_return_to("/unrelated"), "/miniapp/");
    }

    #[tokio::test]
    async fn web_csrf_requires_matching_cookie_header_and_session_hash() {
        let temporary = tempfile::tempdir().unwrap();
        let database = Database::open(&temporary.path().join("market.sqlite"))
            .await
            .unwrap();
        let service = AuthService::new(test_config(temporary.path()), database.clone()).unwrap();
        let user = database
            .upsert_github_user(24753352, "bobleer", "https://example.invalid/avatar")
            .await
            .unwrap();
        let auth = RequestAuth {
            user,
            kind: RequestAuthKind::Web {
                session_token: "session".to_string(),
                csrf_hash: token_hash("csrf-value"),
                expires_at: (Utc::now() + Duration::hours(1)).timestamp(),
                surface: WebSessionSurface::MiniApp,
            },
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("openbitfun_market_csrf=csrf-value"),
        );
        headers.insert("x-csrf-token", HeaderValue::from_static("csrf-value"));
        service.require_csrf(&headers, &auth).unwrap();

        headers.insert("x-csrf-token", HeaderValue::from_static("different"));
        assert_eq!(
            service
                .require_csrf(&headers, &auth)
                .unwrap_err()
                .status
                .as_u16(),
            403
        );

        let mut response_headers = HeaderMap::new();
        service
            .append_shared_account_cookies(&mut response_headers, &headers, &auth)
            .unwrap();
        let cookies = response_headers
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|value| value.to_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(cookies.len(), 4);
        assert!(cookies.iter().any(|cookie| {
            cookie.starts_with("openbitfun_market_session=session; Path=/miniapp;")
        }));
        assert!(cookies.iter().any(|cookie| {
            cookie.starts_with("openbitfun_market_csrf=csrf-value; Path=/miniapp;")
        }));
        assert!(cookies
            .iter()
            .any(|cookie| { cookie.starts_with("openbitfun_skin_session=session; Path=/skin;") }));
        assert!(cookies
            .iter()
            .any(|cookie| { cookie.starts_with("openbitfun_skin_csrf=csrf-value; Path=/skin;") }));
        assert!(cookies.iter().all(|cookie| cookie.contains("SameSite=Lax")));
        assert!(cookies.iter().all(|cookie| cookie.contains("Secure")));
        assert!(cookies.iter().all(|cookie| !cookie.contains("Domain=")));
        assert!(cookies
            .iter()
            .filter(|cookie| cookie.contains("_session="))
            .all(|cookie| cookie.contains("HttpOnly")));

        let mut clear_headers = HeaderMap::new();
        service.append_clear_cookies(&mut clear_headers).unwrap();
        let clear_cookies = clear_headers
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|value| value.to_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(clear_cookies.len(), 4);
        assert!(clear_cookies
            .iter()
            .all(|cookie| cookie.contains("Max-Age=0")));
    }

    #[tokio::test]
    async fn refresh_rotation_revokes_the_old_pair_and_keeps_admin_id_numeric() {
        let temporary = tempfile::tempdir().unwrap();
        let database = Database::open(&temporary.path().join("market.sqlite"))
            .await
            .unwrap();
        let service = AuthService::new(test_config(temporary.path()), database.clone()).unwrap();
        let user = database
            .upsert_github_user(24753352, "bobleer", "https://example.invalid/avatar")
            .await
            .unwrap();
        assert!(service.is_admin(&user));
        let first = service
            .issue_token_pair(user.internal_id, None)
            .await
            .unwrap();
        let first_family = database
            .api_token_user(&first.refresh_token, "refresh")
            .await
            .unwrap()
            .unwrap()
            .1;

        let second = service.refresh_tokens(&first.refresh_token).await.unwrap();

        assert!(database
            .api_token_user(&first.access_token, "access")
            .await
            .unwrap()
            .is_none());
        assert!(database
            .api_token_user(&first.refresh_token, "refresh")
            .await
            .unwrap()
            .is_none());
        let second_family = database
            .api_token_user(&second.refresh_token, "refresh")
            .await
            .unwrap()
            .unwrap()
            .1;
        assert_eq!(second_family, first_family);
    }
}
