//! Verification against the shared OpenBitFun GitHub identity authority.
//! Relay receives an OpenBitFun access token, never a GitHub OAuth secret.

use axum::http::StatusCode;
use serde::Deserialize;
use std::{sync::Arc, time::Duration};

pub(crate) const IDENTITY_ME_URL: &str = "https://auth.openbitfun.com/api/v1/me";

#[derive(Clone)]
pub(crate) struct IdentityVerifier {
    client: reqwest::Client,
    me_url: reqwest::Url,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VerifiedIdentity {
    pub github_id: i64,
    pub login: String,
}

#[derive(Deserialize)]
struct IdentityResponse {
    user: VerifiedIdentity,
}

impl IdentityVerifier {
    pub(crate) async fn start_auth(&self) -> Result<serde_json::Value, StatusCode> {
        self.auth_request("auth/desktop/start", serde_json::json!({}))
            .await
    }

    pub(crate) async fn poll_auth(
        &self,
        transaction_id: &str,
        transaction_secret: &str,
    ) -> Result<serde_json::Value, StatusCode> {
        if transaction_id.is_empty()
            || transaction_id.len() > 256
            || transaction_secret.is_empty()
            || transaction_secret.len() > 1024
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        self.auth_request(
            "auth/desktop/poll",
            serde_json::json!({
                "transactionId": transaction_id,
                "transactionSecret": transaction_secret,
            }),
        )
        .await
    }

    async fn auth_request(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, StatusCode> {
        let url = self
            .me_url
            .join(path)
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let _permit = identity_request_permit()?;
        let mut response = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        if !response.status().is_success() {
            return Err(match response.status() {
                StatusCode::BAD_REQUEST
                | StatusCode::UNAUTHORIZED
                | StatusCode::GONE
                | StatusCode::TOO_MANY_REQUESTS => response.status(),
                _ => StatusCode::SERVICE_UNAVAILABLE,
            });
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        {
            if bytes.len() + chunk.len() > 65536 {
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
    }

    pub(crate) fn new() -> anyhow::Result<Self> {
        Self::with_url(IDENTITY_ME_URL)
    }

    pub(crate) fn with_url(url: &str) -> anyhow::Result<Self> {
        let me_url = reqwest::Url::parse(url)?;
        anyhow::ensure!(
            me_url.scheme() == "https"
                || (cfg!(test)
                    && me_url.scheme() == "http"
                    && me_url.host_str() == Some("127.0.0.1")),
            "The identity authority must use HTTPS"
        );
        // Standalone relay does not link the workspace service facade. Select
        // ring for this client without installing a second process provider.
        let roots =
            rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
        Ok(Self {
            client: reqwest::Client::builder()
                .tls_backend_preconfigured(tls)
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            me_url,
        })
    }

    pub(crate) async fn verify(&self, token: &str) -> Result<VerifiedIdentity, StatusCode> {
        if token.is_empty()
            || token.len() > 8192
            || token
                .bytes()
                .any(|c| c.is_ascii_whitespace() || c.is_ascii_control())
        {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let _permit = identity_request_permit()?;
        let mut response = self
            .client
            .get(self.me_url.clone())
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(StatusCode::UNAUTHORIZED);
        }
        if !response.status().is_success() {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        {
            if bytes.len() + chunk.len() > 65536 {
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
            bytes.extend_from_slice(&chunk);
        }
        let identity: IdentityResponse =
            serde_json::from_slice(&bytes).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let user = identity.user;
        if user.github_id <= 0
            || user.login.is_empty()
            || user.login.len() > 100
            || !user
                .login
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        Ok(user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};

    async fn verifier(
        status: StatusCode,
        body: &'static str,
    ) -> (IdentityVerifier, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/me", listener.local_addr().unwrap());
        let app = Router::new().route(
            "/me",
            get(move |headers: axum::http::HeaderMap| async move {
                assert_eq!(
                    headers.get("authorization").unwrap(),
                    "Bearer account-token"
                );
                (status, body)
            }),
        );
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (IdentityVerifier::with_url(&url).unwrap(), task)
    }

    #[tokio::test]
    async fn accepts_only_authority_verified_numeric_identity() {
        let (client, task) = verifier(
            StatusCode::OK,
            r#"{"user":{"githubId":123,"login":"alice"}}"#,
        )
        .await;
        let identity = client.verify("account-token").await.unwrap();
        assert_eq!(identity.github_id, 123);
        assert_eq!(identity.login, "alice");
        task.abort();
    }

    #[tokio::test]
    async fn fails_closed_on_expired_tokens_unavailable_authority_and_invalid_profiles() {
        for (status, body, expected) in [
            (StatusCode::UNAUTHORIZED, "", StatusCode::UNAUTHORIZED),
            (StatusCode::FOUND, "", StatusCode::SERVICE_UNAVAILABLE),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "",
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                StatusCode::OK,
                r#"{"user":{"githubId":0,"login":"alice"}}"#,
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                StatusCode::OK,
                r#"{"user":{"githubId":123,"login":"../bob"}}"#,
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (StatusCode::OK, "invalid", StatusCode::SERVICE_UNAVAILABLE),
        ] {
            let (client, task) = verifier(status, body).await;
            assert_eq!(client.verify("account-token").await.unwrap_err(), expected);
            task.abort();
        }
    }
}

fn identity_request_permit() -> Result<tokio::sync::OwnedSemaphorePermit, StatusCode> {
    static REQUESTS: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> = std::sync::OnceLock::new();
    Arc::clone(REQUESTS.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(64))))
        .try_acquire_owned()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}
