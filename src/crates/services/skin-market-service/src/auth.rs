use crate::db::{Database, LocalUser};
use crate::error::{SkinMarketError, SkinMarketResult};
use axum::http::{header, HeaderMap};
use openbitfun_product_domains::appearance_market::AppearanceMarketUserSummary;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use url::Url;

const SKIN_SESSION_COOKIE: &str = "openbitfun_skin_session";
const SKIN_CSRF_COOKIE: &str = "openbitfun_skin_csrf";

#[derive(Debug, Clone)]
pub(crate) struct IdentityVerifier {
    client: Client,
    me_url: Url,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthenticatedIdentity {
    pub user: LocalUser,
    pub is_admin: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdentityResponse {
    user: AppearanceMarketUserSummary,
    is_admin: bool,
}

impl IdentityVerifier {
    pub(crate) fn new(me_url: Url) -> anyhow::Result<Self> {
        openbitfun_services_core::tls_provider::ensure_ring_crypto_provider();
        Ok(Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            me_url,
        })
    }

    pub(crate) async fn require(
        &self,
        headers: &HeaderMap,
        database: &Database,
    ) -> SkinMarketResult<AuthenticatedIdentity> {
        self.verify(headers, database, false).await
    }

    pub(crate) async fn require_write(
        &self,
        headers: &HeaderMap,
        database: &Database,
    ) -> SkinMarketResult<AuthenticatedIdentity> {
        self.verify(headers, database, true).await
    }

    async fn verify(
        &self,
        headers: &HeaderMap,
        database: &Database,
        write: bool,
    ) -> SkinMarketResult<AuthenticatedIdentity> {
        let authorization = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .filter(|value| {
                value
                    .strip_prefix("Bearer ")
                    .is_some_and(|token| !token.trim().is_empty())
            });
        let mut request = if write {
            self.client.post(self.me_url.clone())
        } else {
            self.client.get(self.me_url.clone())
        };
        if let Some(authorization) = authorization {
            request = request.header(reqwest::header::AUTHORIZATION, authorization);
        } else {
            let session = cookie_value(headers, SKIN_SESSION_COOKIE)
                .ok_or_else(|| SkinMarketError::unauthorized("Sign in with GitHub to continue."))?;
            let mut cookies = format!("{SKIN_SESSION_COOKIE}={session}");
            if write {
                let csrf_cookie = cookie_value(headers, SKIN_CSRF_COOKIE).ok_or_else(|| {
                    SkinMarketError::forbidden("The CSRF token is missing or invalid.")
                })?;
                let csrf_header = headers
                    .get("x-csrf-token")
                    .and_then(|value| value.to_str().ok())
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        SkinMarketError::forbidden("The CSRF token is missing or invalid.")
                    })?;
                cookies.push_str(&format!("; {SKIN_CSRF_COOKIE}={csrf_cookie}"));
                request = request.header("x-csrf-token", csrf_header);
            }
            request = request.header(reqwest::header::COOKIE, cookies);
        }
        let response = request.send().await.map_err(|_| {
            SkinMarketError::unavailable("The identity service could not be reached.")
        })?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SkinMarketError::unauthorized(
                "The GitHub marketplace session is invalid or expired.",
            ));
        }
        if response.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(SkinMarketError::forbidden(
                "The CSRF token is missing or invalid.",
            ));
        }
        if !response.status().is_success() {
            return Err(SkinMarketError::unavailable(
                "The identity service rejected the verification request.",
            ));
        }
        let identity: IdentityResponse = response.json().await.map_err(|_| {
            SkinMarketError::unavailable("The identity service returned an invalid response.")
        })?;
        if identity.user.github_id <= 0
            || identity.user.login.trim().is_empty()
            || identity.user.login.len() > 100
            || identity.user.avatar_url.len() > 2_048
        {
            return Err(SkinMarketError::unavailable(
                "The identity service returned an invalid user profile.",
            ));
        }
        Ok(AuthenticatedIdentity {
            user: database.upsert_user(&identity.user).await?,
            is_admin: identity.is_admin,
        })
    }

    pub(crate) async fn require_admin(
        &self,
        headers: &HeaderMap,
        database: &Database,
    ) -> SkinMarketResult<AuthenticatedIdentity> {
        let identity = self.require(headers, database).await?;
        if !identity.is_admin {
            return Err(SkinMarketError::forbidden(
                "Appearance marketplace administrator access is required.",
            ));
        }
        Ok(identity)
    }

    pub(crate) async fn require_admin_write(
        &self,
        headers: &HeaderMap,
        database: &Database,
    ) -> SkinMarketResult<AuthenticatedIdentity> {
        let identity = self.require_write(headers, database).await?;
        if !identity.is_admin {
            return Err(SkinMarketError::forbidden(
                "Appearance marketplace administrator access is required.",
            ));
        }
        Ok(identity)
    }
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .filter_map(|cookie| cookie.split_once('='))
        .find_map(|(candidate, value)| (candidate == name).then(|| value.to_string()))
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router};

    #[tokio::test]
    async fn bearer_identity_is_forwarded_for_write_verification() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/me",
            axum::routing::post(|headers: HeaderMap| async move {
                assert_eq!(
                    headers.get(header::AUTHORIZATION).unwrap(),
                    "Bearer test-token"
                );
                Json(serde_json::json!({
                    "user": {"githubId": 42, "login": "owner", "avatarUrl": "https://example.invalid/a"},
                    "isAdmin": true
                }))
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let temporary = tempfile::tempdir().unwrap();
        let database = Database::open(&temporary.path().join("market.sqlite"))
            .await
            .unwrap();
        let verifier =
            IdentityVerifier::new(Url::parse(&format!("http://{address}/me")).unwrap()).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer test-token".parse().unwrap());
        let identity = verifier
            .require_admin_write(&headers, &database)
            .await
            .unwrap();
        assert!(identity.user.internal_id > 0);
    }

    #[tokio::test]
    async fn skin_cookie_write_forwards_only_shared_session_and_csrf() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/me",
            axum::routing::post(|headers: HeaderMap| async move {
                assert_eq!(
                    headers.get(header::COOKIE).unwrap(),
                    "openbitfun_skin_session=session-token; openbitfun_skin_csrf=csrf-token"
                );
                assert_eq!(headers.get("x-csrf-token").unwrap(), "csrf-token");
                assert!(headers.get(header::AUTHORIZATION).is_none());
                Json(serde_json::json!({
                    "user": {"githubId": 42, "login": "owner", "avatarUrl": "https://example.invalid/a"},
                    "isAdmin": true
                }))
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let temporary = tempfile::tempdir().unwrap();
        let database = Database::open(&temporary.path().join("market.sqlite"))
            .await
            .unwrap();
        let verifier =
            IdentityVerifier::new(Url::parse(&format!("http://{address}/me")).unwrap()).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "unrelated=private; openbitfun_skin_session=session-token; openbitfun_skin_csrf=csrf-token"
                .parse()
                .unwrap(),
        );
        headers.insert("x-csrf-token", "csrf-token".parse().unwrap());

        let identity = verifier
            .require_admin_write(&headers, &database)
            .await
            .unwrap();
        assert!(identity.user.internal_id > 0);
    }
}
