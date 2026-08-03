use crate::db::{Database, LocalUser};
use crate::error::{SkinMarketError, SkinMarketResult};
use axum::http::{header, HeaderMap};
use bitfun_product_domains::appearance_market::AppearanceMarketUserSummary;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use url::Url;

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
        let authorization = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .filter(|value| {
                value
                    .strip_prefix("Bearer ")
                    .is_some_and(|token| !token.trim().is_empty())
            })
            .ok_or_else(|| {
                SkinMarketError::unauthorized(
                    "Appearance marketplace writes require a Desktop Bearer token.",
                )
            })?;
        let response = self
            .client
            .get(self.me_url.clone())
            .header(reqwest::header::AUTHORIZATION, authorization)
            .send()
            .await
            .map_err(|_| {
                SkinMarketError::unavailable("The identity service could not be reached.")
            })?;
        if matches!(
            response.status(),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
        ) {
            return Err(SkinMarketError::unauthorized(
                "The Desktop Bearer token is invalid or expired.",
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use axum::{Json, Router};

    #[tokio::test]
    async fn bearer_identity_is_forwarded_without_accepting_cookies() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/me",
            get(|headers: HeaderMap| async move {
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
        let identity = verifier.require_admin(&headers, &database).await.unwrap();
        assert!(identity.user.internal_id > 0);

        let mut cookie_only = HeaderMap::new();
        cookie_only.insert(header::COOKIE, "session=ignored".parse().unwrap());
        assert_eq!(
            verifier
                .require(&cookie_only, &database)
                .await
                .unwrap_err()
                .status,
            axum::http::StatusCode::UNAUTHORIZED
        );
    }
}
