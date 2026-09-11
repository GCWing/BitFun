//! Account App Server wire schemas.

#[cfg(feature = "rpc")]
use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use serde::{Deserialize, Serialize};

pub use openbitfun_product_domains::account::{AccountDevice, AccountInfo};

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "account/snapshot", response = AccountSnapshotResponse))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountSnapshotRequest {
    pub workspace_path: String,
}

impl std::fmt::Debug for AccountSnapshotRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccountSnapshotRequest")
            .field("workspace_path", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
#[serde(rename_all = "camelCase")]
pub struct AccountSnapshotResponse {
    pub logged_in: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<AccountInfo>,
    #[serde(default)]
    pub devices: Vec<AccountDevice>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "account/githubStart", response = AccountGitHubStartResponse))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountGitHubStartRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
pub struct AccountGitHubStartResponse {
    #[serde(flatten)]
    pub authorization: openbitfun_product_domains::account::GitHubAuthStart,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "account/githubPoll", response = AccountGitHubPollResponse))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountGitHubPollRequest {
    pub transaction_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
pub struct AccountGitHubPollResponse {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "account/login", response = AccountLoginResponse))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountLoginRequest {
    pub operation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
#[serde(rename_all = "camelCase")]
pub struct AccountLoginResponse {
    pub user_id: String,
    pub relay_url: String,
    pub status_message: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "account/logout", response = AccountSnapshotResponse))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountLogoutRequest {
    pub operation_id: String,
    pub workspace_path: String,
}

impl std::fmt::Debug for AccountLogoutRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccountLogoutRequest")
            .field("operation_id", &self.operation_id)
            .field("workspace_path", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_accepts_no_password_or_relay_configuration() {
        assert!(serde_json::from_value::<AccountLoginRequest>(
            serde_json::json!({"operationId":"op"})
        )
        .is_ok());
        assert!(serde_json::from_value::<AccountLoginRequest>(
            serde_json::json!({"operationId":"op","password":"secret"})
        )
        .is_err());
    }

    #[test]
    fn account_methods_follow_lower_camel_method_contract() {
        for method in [
            "account/snapshot",
            "account/login",
            "account/logout",
            "account/githubStart",
            "account/githubPoll",
        ] {
            assert!(crate::method::is_valid_method_name(method));
        }
    }
}
