//! Controller-local adapter for the shared OpenBitFun GitHub identity.
use openbitfun_product_domains::account::{
    GitHubAuthPollRequest, GitHubAuthPollResponse, GitHubAuthStart,
};
use openbitfun_services_integrations::account_identity::{self, AccountIdentityClient, MarketMe};
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub async fn account_github_start() -> Result<GitHubAuthStart, String> {
    account_identity::start_auth_flow()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn account_github_poll(
    app: AppHandle,
    request: GitHubAuthPollRequest,
) -> Result<GitHubAuthPollResponse, String> {
    let response = account_identity::poll_auth_flow(request)
        .await
        .map_err(|error| error.to_string())?;
    if response.status == "authorized" {
        emit_identity_changed(&app, "signed-in");
    }
    Ok(response)
}

#[tauri::command]
pub async fn account_github_info() -> Result<Option<MarketMe>, String> {
    let mut client = AccountIdentityClient::from_environment()
        .await
        .map_err(|error| error.to_string())?;
    client.me().await.map_err(|error| error.to_string())
}

pub(crate) fn emit_identity_changed(app: &AppHandle, status: &'static str) {
    let _ = app.emit(
        "account-identity-changed",
        serde_json::json!({ "status": status }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_auth_views_never_serialize_oauth_secrets() {
        let started = serde_json::to_value(GitHubAuthStart {
            transaction_id: "transaction-1".to_string(),
            authorization_url: "https://github.com/login/oauth/authorize".to_string(),
            expires_at: 123,
            poll_interval_seconds: 3,
        })
        .unwrap();
        assert!(started.get("transactionSecret").is_none());

        let polled = serde_json::to_value(GitHubAuthPollResponse {
            status: "authorized".to_string(),
        })
        .unwrap();
        assert!(polled.get("tokens").is_none());
        assert!(polled.get("accessToken").is_none());
        assert!(polled.get("refreshToken").is_none());
    }
}
