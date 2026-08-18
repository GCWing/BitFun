//! Internationalization App Server wire schemas.

use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "i18n/getCurrentLanguage", response = I18nGetCurrentLanguageResponse)]
pub struct I18nGetCurrentLanguageMessage {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct I18nGetCurrentLanguageResponse {
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "i18n/setLanguage", response = I18nSetLanguageResponse)]
#[serde(rename_all = "camelCase")]
pub struct I18nSetLanguageMessage {
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct I18nSetLanguageResponse {
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "i18n/getConfig", response = I18nGetConfigResponse)]
pub struct I18nGetConfigMessage {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct I18nGetConfigResponse {
    pub current_language: String,
    pub fallback_language: String,
    pub auto_detect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "i18n/setConfig", response = I18nSetConfigResponse)]
#[serde(rename_all = "camelCase")]
pub struct I18nSetConfigMessage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_language: Option<String>,
    #[serde(default)]
    pub auto_detect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct I18nSetConfigResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "i18n/getSupportedLanguages", response = I18nGetSupportedLanguagesResponse)]
pub struct I18nGetSupportedLanguagesMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I18nLocaleMetadata {
    pub id: String,
    pub name: String,
    pub english_name: String,
    pub native_name: String,
    pub rtl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
pub struct I18nGetSupportedLanguagesResponse {
    pub locales: Vec<I18nLocaleMetadata>,
}
