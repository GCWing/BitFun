//! Host-owned GitHub authorization transactions shared by native surfaces.
use super::{AccountIdentityClient, DesktopAuthPollRequest, MarketClientError};
use openbitfun_product_domains::account::{
    GitHubAuthPollRequest, GitHubAuthPollResponse, GitHubAuthStart,
};
use std::{
    collections::HashMap,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;

struct PendingAuth {
    request: DesktopAuthPollRequest,
    expires_at: i64,
}

fn pending() -> &'static Mutex<HashMap<String, PendingAuth>> {
    static PENDING: OnceLock<Mutex<HashMap<String, PendingAuth>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub async fn start_auth_flow() -> Result<GitHubAuthStart, MarketClientError> {
    let client = AccountIdentityClient::from_environment().await?;
    let started = client.start_desktop_auth().await?;
    let mut transactions = pending().lock().await;
    transactions.retain(|_, transaction| transaction.expires_at > now());
    transactions.insert(
        started.transaction_id.clone(),
        PendingAuth {
            request: DesktopAuthPollRequest {
                transaction_id: started.transaction_id.clone(),
                transaction_secret: started.transaction_secret,
            },
            expires_at: started.expires_at,
        },
    );
    Ok(GitHubAuthStart {
        transaction_id: started.transaction_id,
        authorization_url: started.authorization_url,
        expires_at: started.expires_at,
        poll_interval_seconds: started.poll_interval_seconds,
    })
}

pub async fn poll_auth_flow(
    request: GitHubAuthPollRequest,
) -> Result<GitHubAuthPollResponse, MarketClientError> {
    let mut transactions = pending().lock().await;
    let transaction = transactions.get(&request.transaction_id).ok_or_else(|| {
        super::local_error(
            "auth_transaction_missing",
            "GitHub authorization transaction was not found.",
        )
    })?;
    if transaction.expires_at <= now() {
        transactions.remove(&request.transaction_id);
        return Ok(GitHubAuthPollResponse {
            status: "expired".to_string(),
        });
    }
    let mut client = AccountIdentityClient::from_environment().await?;
    let result = client.poll_desktop_auth(&transaction.request).await?;
    if result.status != "pending" {
        transactions.remove(&request.transaction_id);
    }
    Ok(GitHubAuthPollResponse {
        status: result.status,
    })
}
