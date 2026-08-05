use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyLifecycleState {
    ChoiceRequired,
    Full,
    PrivacyNotAccepted,
    ResourceError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyEffectiveMode {
    Full,
    PrivacyNotAccepted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrivacyChangeType {
    Material,
    Editorial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyConsentRecord {
    pub consent_version: String,
    pub accepted_policy_updated_at: String,
    pub accepted_document_sha256: String,
    pub accepted_at: String,
    pub locale: String,
    pub app_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyPolicyView {
    pub consent_version: String,
    pub change_type: PrivacyChangeType,
    pub effective_at: String,
    pub updated_at: String,
    pub locale: String,
    pub document_sha256: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyStatus {
    pub enabled: bool,
    pub lifecycle_state: PrivacyLifecycleState,
    pub effective_mode: PrivacyEffectiveMode,
    pub release_ready: bool,
    pub has_unread_update: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<PrivacyPolicyView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consent: Option<PrivacyConsentRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration_error: Option<String>,
}

impl PrivacyStatus {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            lifecycle_state: PrivacyLifecycleState::Full,
            effective_mode: PrivacyEffectiveMode::Full,
            release_ready: true,
            has_unread_update: false,
            policy: None,
            consent: None,
            configuration_error: None,
        }
    }

    pub fn collection_allowed(&self) -> bool {
        !self.enabled || self.effective_mode == PrivacyEffectiveMode::Full
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializePrivacyRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPrivacyStatusRequest {
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptPrivacyRequest {
    pub policy_updated_at: String,
    pub consent_version: String,
    pub document_sha256: String,
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnterPrivacyNotAcceptedRequest {
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkPrivacyViewedRequest {
    pub policy_updated_at: String,
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPrivacyCollectionPolicyRequest {
    pub mode: PrivacyEffectiveMode,
    pub locale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyError {
    pub code: String,
    pub message: String,
}

impl PrivacyError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}
