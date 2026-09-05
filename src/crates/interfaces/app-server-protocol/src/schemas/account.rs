//! Account and settings-sync App Server wire schemas.

#[cfg(feature = "rpc")]
use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use serde::{Deserialize, Serialize};

pub use openbitfun_product_domains::account::{
    AccountDevice, AccountInfo, SettingsSyncProgress, SettingsSyncStatus,
};

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
    pub pending_sync_choice: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<AccountInfo>,
    #[serde(default)]
    pub devices: Vec<AccountDevice>,
    pub sync: SettingsSyncProgress,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "account/login", response = AccountLoginResponse))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountLoginRequest {
    pub operation_id: String,
    pub relay_url: String,
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for AccountLoginRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccountLoginRequest")
            .field("operation_id", &self.operation_id)
            .field("relay_url", &"<redacted>")
            .field("username", &"<redacted>")
            .field("password", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
#[serde(rename_all = "camelCase")]
pub struct AccountLoginResponse {
    pub user_id: String,
    pub relay_url: String,
    pub has_cloud_settings: bool,
    pub status_message: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "account/finalizeLogin", response = AccountSnapshotResponse))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountFinalizeLoginRequest {
    pub operation_id: String,
    pub choice: AccountSyncChoice,
    pub workspace_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountSyncChoice {
    Local,
    Cloud,
}

impl std::fmt::Debug for AccountFinalizeLoginRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccountFinalizeLoginRequest")
            .field("operation_id", &self.operation_id)
            .field("choice", &self.choice)
            .field("workspace_path", &"<redacted>")
            .finish()
    }
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

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "settingsSync/start", response = SettingsSyncResponse))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsSyncStartRequest {
    pub operation_id: String,
    pub workspace_path: String,
    pub is_first_login: bool,
}

impl std::fmt::Debug for SettingsSyncStartRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SettingsSyncStartRequest")
            .field("operation_id", &self.operation_id)
            .field("workspace_path", &"<redacted>")
            .field("is_first_login", &self.is_first_login)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "settingsSync/snapshot", response = SettingsSyncResponse))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsSyncSnapshotRequest {
    pub workspace_path: String,
}

impl std::fmt::Debug for SettingsSyncSnapshotRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SettingsSyncSnapshotRequest")
            .field("workspace_path", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "settingsSync/cancel", response = SettingsSyncResponse))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsSyncCancelRequest {
    pub operation_id: String,
    pub workspace_path: String,
}

impl std::fmt::Debug for SettingsSyncCancelRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SettingsSyncCancelRequest")
            .field("operation_id", &self.operation_id)
            .field("workspace_path", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "settingsSync/localChanged", response = SettingsSyncResponse))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsSyncLocalChangedRequest {
    pub operation_id: String,
    pub workspace_path: String,
}

impl std::fmt::Debug for SettingsSyncLocalChangedRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SettingsSyncLocalChangedRequest")
            .field("operation_id", &self.operation_id)
            .field("workspace_path", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
#[serde(rename_all = "camelCase")]
pub struct SettingsSyncResponse {
    pub progress: SettingsSyncProgress,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_debug_does_not_expose_secrets_or_paths() {
        let request = AccountLoginRequest {
            operation_id: "account-op-1".to_string(),
            relay_url: "https://secret.example".to_string(),
            username: "alice".to_string(),
            password: "password-value".to_string(),
        };
        let debug = format!("{request:?}");
        assert!(!debug.contains("secret.example"));
        assert!(!debug.contains("alice"));
        assert!(!debug.contains("password-value"));
        assert!(debug.contains("account-op-1"));

        let finalize = AccountFinalizeLoginRequest {
            operation_id: "account-op-2".to_string(),
            choice: AccountSyncChoice::Cloud,
            workspace_path: "C:/private/workspace".to_string(),
        };
        let debug = format!("{finalize:?}");
        assert!(!debug.contains("private/workspace"));
        assert!(debug.contains("account-op-2"));
    }

    #[test]
    fn account_methods_follow_lower_camel_method_contract() {
        for method in [
            "account/snapshot",
            "account/login",
            "account/finalizeLogin",
            "account/logout",
            "settingsSync/start",
            "settingsSync/snapshot",
            "settingsSync/cancel",
            "settingsSync/localChanged",
        ] {
            assert!(crate::method::is_valid_method_name(method));
        }
    }

    #[test]
    fn settings_sync_progress_carries_operation_identity_and_cancel_state() {
        let progress = SettingsSyncProgress {
            operation_id: Some("account-op-3".to_string()),
            status: SettingsSyncStatus::Cancelled,
            ..Default::default()
        };
        let value = serde_json::to_value(progress).expect("serialize progress");
        assert_eq!(value["operationId"], "account-op-3");
        assert_eq!(value["status"], "cancelled");
    }
}
