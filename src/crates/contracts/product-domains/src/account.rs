//! Account identity and settings-sync projections shared by product surfaces.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub user_id: String,
    pub relay_url: String,
    pub device_id: String,
    pub device_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDevice {
    pub device_id: String,
    pub device_name: String,
    pub online: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SettingsSyncStatus {
    #[default]
    Idle,
    Syncing,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSyncProgress {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    pub status: SettingsSyncStatus,
    pub phase: String,
    pub percent: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub settings_synced: bool,
    pub sessions_exported: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSnapshotProjection {
    pub logged_in: bool,
    pub pending_sync_choice: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<AccountInfo>,
    #[serde(default)]
    pub devices: Vec<AccountDevice>,
    pub sync: SettingsSyncProgress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLoginProjection {
    pub user_id: String,
    pub relay_url: String,
    pub has_cloud_settings: bool,
    pub status_message: String,
}
