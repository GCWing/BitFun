use bitfun_product_domains::privacy::{
    AcceptPrivacyRequest, PrivacyChangeType, PrivacyConsentRecord, PrivacyEffectiveMode,
    PrivacyError, PrivacyLifecycleState, PrivacyPolicyView, PrivacyStatus,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

const CONSENT_VERSION: &str = "4";
const EFFECTIVE_AT: &str = "2026-07-30T00:00:00Z";
const POLICY_UPDATED_AT: &str = "2026-07-30T00:00:00Z";
const CHANGE_TYPE: PrivacyChangeType = PrivacyChangeType::Editorial;
const LEGAL_CONTENT_SENTINEL: &str = "LEGAL_CONTENT_REQUIRED";

const ZH_CN_CONTENT: &str = include_str!("assets/zh-CN.md");
const ZH_CN_SHA256: &str = "af8e3bff76330d3910ab2343a083922f0895d04f3e497c9cd02472cc2a5d9987";
const ACCEPTED_POLICY_DOCUMENTS: &[(&str, &str, &str)] =
    &[(POLICY_UPDATED_AT, "zh-CN", ZH_CN_SHA256)];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredPrivacyMode {
    Full,
    PrivacyNotAccepted,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrivacyStateFile {
    #[serde(default)]
    mode: Option<StoredPrivacyMode>,
    #[serde(default)]
    consent: Option<PrivacyConsentRecord>,
    #[serde(default)]
    viewed_policy_updated_at: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct BuiltinDocument {
    locale: &'static str,
    content: &'static str,
    expected_sha256: &'static str,
}

#[derive(Debug)]
pub struct PrivacyCollectionPolicy {
    collection_allowed: AtomicBool,
    #[cfg(test)]
    fail_full_application: AtomicBool,
}

impl PrivacyCollectionPolicy {
    pub fn new(collection_allowed: bool) -> Self {
        Self {
            collection_allowed: AtomicBool::new(collection_allowed),
            #[cfg(test)]
            fail_full_application: AtomicBool::new(false),
        }
    }

    pub fn apply(&self, mode: PrivacyEffectiveMode) -> Result<(), PrivacyError> {
        #[cfg(test)]
        if mode == PrivacyEffectiveMode::Full && self.fail_full_application.load(Ordering::SeqCst) {
            self.collection_allowed.store(false, Ordering::SeqCst);
            return Err(PrivacyError::new(
                "PRIVACY_POLICY_APPLY_FAILED",
                "The full collection policy could not be applied",
            ));
        }

        self.collection_allowed
            .store(mode == PrivacyEffectiveMode::Full, Ordering::SeqCst);
        Ok(())
    }

    pub fn effective_mode(&self) -> PrivacyEffectiveMode {
        if self.collection_allowed.load(Ordering::SeqCst) {
            PrivacyEffectiveMode::Full
        } else {
            PrivacyEffectiveMode::PrivacyNotAccepted
        }
    }

    pub fn collection_allowed(&self) -> bool {
        self.collection_allowed.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    fn set_fail_full_application(&self, fail: bool) {
        self.fail_full_application.store(fail, Ordering::SeqCst);
    }
}

pub struct PrivacyService {
    state_path: PathBuf,
    initial_locale: String,
    resources_valid: bool,
    release_ready: bool,
}

impl PrivacyService {
    pub fn new(storage_dir: PathBuf, initial_locale: &str) -> Self {
        let documents = builtin_documents();
        let resources_valid = documents.iter().all(|document| {
            sha256(document.content.as_bytes()) == document.expected_sha256
                && !document.content.trim().is_empty()
        });
        let legal_content_ready = documents
            .iter()
            .all(|document| !document.content.contains(LEGAL_CONTENT_SENTINEL));
        Self {
            state_path: storage_dir.join("privacy-state.json"),
            initial_locale: normalize_locale(initial_locale).to_string(),
            resources_valid,
            release_ready: resources_valid && (legal_content_ready || cfg!(debug_assertions)),
        }
    }

    pub async fn initialize(&self, app_version: &str) -> Result<PrivacyStatus, PrivacyError> {
        self.status(&self.initial_locale, app_version).await
    }

    pub async fn status(
        &self,
        locale: &str,
        _app_version: &str,
    ) -> Result<PrivacyStatus, PrivacyError> {
        if !self.resources_valid {
            return Ok(PrivacyStatus {
                enabled: true,
                lifecycle_state: PrivacyLifecycleState::ResourceError,
                effective_mode: PrivacyEffectiveMode::PrivacyNotAccepted,
                release_ready: false,
                has_unread_update: false,
                policy: None,
                consent: None,
                configuration_error: Some("BUILT_IN_POLICY_INVALID".to_string()),
            });
        }

        let document = self.document(locale)?;
        let state = self.load_state().await;
        let consent_valid = state
            .consent
            .as_ref()
            .is_some_and(|consent| self.consent_is_valid(consent));
        let lifecycle_state = match state.mode {
            Some(StoredPrivacyMode::Full) if consent_valid => PrivacyLifecycleState::Full,
            Some(StoredPrivacyMode::PrivacyNotAccepted) => {
                PrivacyLifecycleState::PrivacyNotAccepted
            }
            _ => PrivacyLifecycleState::ChoiceRequired,
        };
        let effective_mode = if lifecycle_state == PrivacyLifecycleState::Full {
            PrivacyEffectiveMode::Full
        } else {
            PrivacyEffectiveMode::PrivacyNotAccepted
        };
        let has_unread_update = CHANGE_TYPE == PrivacyChangeType::Editorial
            && state.mode.is_some()
            && state.viewed_policy_updated_at.as_deref() != Some(POLICY_UPDATED_AT);

        Ok(PrivacyStatus {
            enabled: true,
            lifecycle_state,
            effective_mode,
            release_ready: self.release_ready,
            has_unread_update,
            policy: Some(policy_view(document)),
            consent: state
                .consent
                .filter(|consent| self.consent_is_valid(consent)),
            configuration_error: (!self.release_ready)
                .then(|| "LEGAL_CONTENT_REQUIRED".to_string()),
        })
    }

    pub async fn accept(
        &self,
        request: &AcceptPrivacyRequest,
        app_version: &str,
    ) -> Result<PrivacyStatus, PrivacyError> {
        if !self.release_ready {
            return Err(PrivacyError::new(
                "PRIVACY_RELEASE_BLOCKED",
                "Bundled privacy resources are not release ready",
            ));
        }
        let document = self.document(&request.locale)?;
        let expected_hash = sha256(document.content.as_bytes());
        if request.policy_updated_at != POLICY_UPDATED_AT
            || request.consent_version != CONSENT_VERSION
            || request.document_sha256 != expected_hash
        {
            return Err(PrivacyError::new(
                "PRIVACY_POLICY_MISMATCH",
                "The displayed privacy policy no longer matches the bundled policy",
            ));
        }

        let consent = PrivacyConsentRecord {
            consent_version: CONSENT_VERSION.to_string(),
            accepted_policy_updated_at: POLICY_UPDATED_AT.to_string(),
            accepted_document_sha256: expected_hash,
            accepted_at: Utc::now().to_rfc3339(),
            locale: document.locale.to_string(),
            app_version: app_version.to_string(),
        };
        self.store_state(&PrivacyStateFile {
            mode: Some(StoredPrivacyMode::Full),
            consent: Some(consent),
            viewed_policy_updated_at: Some(POLICY_UPDATED_AT.to_string()),
        })
        .await?;
        self.status(document.locale, app_version).await
    }

    pub async fn enter_not_accepted(
        &self,
        locale: &str,
        app_version: &str,
    ) -> Result<PrivacyStatus, PrivacyError> {
        self.store_state(&PrivacyStateFile {
            mode: Some(StoredPrivacyMode::PrivacyNotAccepted),
            consent: None,
            viewed_policy_updated_at: Some(POLICY_UPDATED_AT.to_string()),
        })
        .await?;
        self.status(locale, app_version).await
    }

    pub async fn mark_viewed(
        &self,
        policy_updated_at: &str,
        app_version: &str,
        locale: &str,
    ) -> Result<PrivacyStatus, PrivacyError> {
        if policy_updated_at != POLICY_UPDATED_AT {
            return Err(PrivacyError::new(
                "PRIVACY_POLICY_MISMATCH",
                "Policy update timestamp does not match the bundled policy",
            ));
        }
        let mut state = self.load_state().await;
        state.viewed_policy_updated_at = Some(POLICY_UPDATED_AT.to_string());
        self.store_state(&state).await?;
        self.status(locale, app_version).await
    }

    pub async fn can_apply_full_mode(&self) -> Result<bool, PrivacyError> {
        let status = self.status(&self.initial_locale, "policy-check").await?;
        Ok(status.lifecycle_state == PrivacyLifecycleState::Full)
    }

    fn document(&self, locale: &str) -> Result<BuiltinDocument, PrivacyError> {
        if !self.resources_valid {
            return Err(PrivacyError::new(
                "BUILT_IN_POLICY_INVALID",
                "A bundled privacy document failed integrity validation",
            ));
        }
        let locale = normalize_locale(locale);
        builtin_documents()
            .into_iter()
            .find(|document| document.locale == locale)
            .ok_or_else(|| {
                PrivacyError::new(
                    "PRIVACY_LOCALE_UNAVAILABLE",
                    format!("Bundled privacy document is unavailable for {locale}"),
                )
            })
    }

    fn consent_is_valid(&self, consent: &PrivacyConsentRecord) -> bool {
        if consent.consent_version != CONSENT_VERSION
            || chrono::DateTime::parse_from_rfc3339(&consent.accepted_policy_updated_at).is_err()
            || consent.app_version.trim().is_empty()
            || chrono::DateTime::parse_from_rfc3339(&consent.accepted_at).is_err()
            || !is_sha256(&consent.accepted_document_sha256)
        {
            return false;
        }
        accepted_policy_document(
            &consent.accepted_policy_updated_at,
            normalize_locale(&consent.locale),
            &consent.accepted_document_sha256,
        )
    }

    async fn load_state(&self) -> PrivacyStateFile {
        let Ok(bytes) = tokio::fs::read(&self.state_path).await else {
            return PrivacyStateFile::default();
        };
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    async fn store_state(&self, state: &PrivacyStateFile) -> Result<(), PrivacyError> {
        let parent = self.state_path.parent().ok_or_else(|| {
            PrivacyError::new("PRIVACY_STORAGE_ERROR", "Privacy storage path is invalid")
        })?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(storage_error)?;
        let body = serde_json::to_vec_pretty(state).map_err(|error| {
            PrivacyError::new(
                "PRIVACY_STORAGE_ERROR",
                format!("Encode privacy state: {error}"),
            )
        })?;
        let temporary = self.state_path.with_extension("json.tmp");
        tokio::fs::write(&temporary, body)
            .await
            .map_err(storage_error)?;
        tokio::fs::rename(&temporary, &self.state_path)
            .await
            .map_err(storage_error)?;
        Ok(())
    }
}

fn builtin_documents() -> [BuiltinDocument; 1] {
    [BuiltinDocument {
        locale: "zh-CN",
        content: ZH_CN_CONTENT,
        expected_sha256: ZH_CN_SHA256,
    }]
}

fn policy_view(document: BuiltinDocument) -> PrivacyPolicyView {
    PrivacyPolicyView {
        consent_version: CONSENT_VERSION.to_string(),
        change_type: CHANGE_TYPE,
        effective_at: EFFECTIVE_AT.to_string(),
        updated_at: POLICY_UPDATED_AT.to_string(),
        locale: document.locale.to_string(),
        document_sha256: sha256(document.content.as_bytes()),
        content: document.content.to_string(),
    }
}

fn normalize_locale(_locale: &str) -> &'static str {
    "zh-CN"
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn accepted_policy_document(updated_at: &str, locale: &str, document_sha256: &str) -> bool {
    ACCEPTED_POLICY_DOCUMENTS
        .iter()
        .any(|(accepted_updated_at, accepted_locale, accepted_hash)| {
            *accepted_updated_at == updated_at
                && *accepted_locale == locale
                && *accepted_hash == document_sha256
        })
}

fn storage_error(error: std::io::Error) -> PrivacyError {
    PrivacyError::new(
        "PRIVACY_STORAGE_ERROR",
        format!("Persist privacy state: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_locale, PrivacyCollectionPolicy, PrivacyService, PrivacyStateFile,
        StoredPrivacyMode, CONSENT_VERSION, POLICY_UPDATED_AT, ZH_CN_SHA256,
    };
    use bitfun_product_domains::privacy::{
        AcceptPrivacyRequest, PrivacyChangeType, PrivacyConsentRecord, PrivacyEffectiveMode,
        PrivacyLifecycleState,
    };

    fn temporary_directory(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "bitfun-privacy-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    async fn accept(service: &PrivacyService) {
        let initial = service.initialize("1.2.3").await.unwrap();
        let policy = initial.policy.unwrap();
        service
            .accept(
                &AcceptPrivacyRequest {
                    policy_updated_at: policy.updated_at,
                    consent_version: policy.consent_version,
                    document_sha256: policy.document_sha256,
                    locale: policy.locale,
                },
                "1.2.3",
            )
            .await
            .unwrap();
    }

    fn previous_consent(consent_version: &str) -> PrivacyConsentRecord {
        PrivacyConsentRecord {
            consent_version: consent_version.to_string(),
            accepted_policy_updated_at: "2026-07-01T00:00:00Z".to_string(),
            accepted_document_sha256:
                "9164815a22b2b2021039a19ed6e92556ce6ea44e42dd0103869b7c0887ae48bb".to_string(),
            accepted_at: "2026-07-24T00:00:00Z".to_string(),
            locale: "zh-CN".to_string(),
            app_version: "1.2.3".to_string(),
        }
    }

    #[test]
    fn resolves_every_locale_to_the_chinese_policy() {
        assert_eq!(normalize_locale("zh-Hant-HK"), "zh-CN");
        assert_eq!(normalize_locale("zh-TW"), "zh-CN");
        assert_eq!(normalize_locale("zh-HK"), "zh-CN");
        assert_eq!(normalize_locale("en-GB"), "zh-CN");
        assert_eq!(normalize_locale("ja-JP"), "zh-CN");
    }

    #[tokio::test]
    async fn first_start_requires_a_choice_without_blocking_local_initialization() {
        let directory = temporary_directory("first-start");
        let service = PrivacyService::new(directory.clone(), "zh-TW");
        let status = service.initialize("test").await.unwrap();
        assert_eq!(
            status.lifecycle_state,
            PrivacyLifecycleState::ChoiceRequired
        );
        assert_eq!(
            status.effective_mode,
            PrivacyEffectiveMode::PrivacyNotAccepted
        );
        assert_eq!(status.policy.unwrap().locale, "zh-CN");
        let _ = tokio::fs::remove_dir_all(directory).await;
    }

    #[tokio::test]
    async fn not_accepted_survives_a_cold_start() {
        let directory = temporary_directory("not-accepted");
        let service = PrivacyService::new(directory.clone(), "en-US");
        service.enter_not_accepted("en-US", "1.2.3").await.unwrap();
        let restarted = PrivacyService::new(directory.clone(), "en-US");
        assert_eq!(
            restarted.initialize("1.2.3").await.unwrap().lifecycle_state,
            PrivacyLifecycleState::PrivacyNotAccepted
        );
        let _ = tokio::fs::remove_dir_all(directory).await;
    }

    #[tokio::test]
    async fn accept_persists_full_mode_atomically() {
        let directory = temporary_directory("accept");
        let service = PrivacyService::new(directory.clone(), "en-US");
        accept(&service).await;
        let status = PrivacyService::new(directory.clone(), "en-US")
            .initialize("1.2.3")
            .await
            .unwrap();
        assert_eq!(status.lifecycle_state, PrivacyLifecycleState::Full);
        assert_eq!(status.consent.unwrap().locale, "zh-CN");
        assert!(!directory.join("privacy-state.tmp").exists());
        let _ = tokio::fs::remove_dir_all(directory).await;
    }

    #[tokio::test]
    async fn chinese_policy_is_release_ready_and_editorial() {
        let directory = temporary_directory("current-policy");
        let status = PrivacyService::new(directory.clone(), "en-US")
            .initialize("1.2.3")
            .await
            .unwrap();
        let policy = status.policy.unwrap();

        assert!(status.release_ready);
        assert_eq!(policy.updated_at, POLICY_UPDATED_AT);
        assert_eq!(policy.consent_version, CONSENT_VERSION);
        assert_eq!(policy.change_type, PrivacyChangeType::Editorial);
        assert_eq!(policy.locale, "zh-CN");
        assert_eq!(policy.document_sha256, ZH_CN_SHA256);
        assert!(policy.content.starts_with("# 关于HUAWEI BitFun的隐私协议"));
        let _ = tokio::fs::remove_dir_all(directory).await;
    }

    #[tokio::test]
    async fn timestamp_marks_editorial_update_unread_until_viewed() {
        let directory = temporary_directory("editorial-update-marker");
        let service = PrivacyService::new(directory.clone(), "en-US");
        service
            .store_state(&PrivacyStateFile {
                mode: Some(StoredPrivacyMode::PrivacyNotAccepted),
                consent: None,
                viewed_policy_updated_at: Some("2026-07-01T00:00:00Z".to_string()),
            })
            .await
            .unwrap();

        assert!(service.initialize("1.2.3").await.unwrap().has_unread_update);
        let current = service.enter_not_accepted("en-US", "1.2.3").await.unwrap();
        assert!(!current.has_unread_update);
        let _ = tokio::fs::remove_dir_all(directory).await;
    }

    #[tokio::test]
    async fn changed_consent_generation_requires_a_new_choice() {
        let directory = temporary_directory("material-update");
        let service = PrivacyService::new(directory.clone(), "zh-CN");
        service
            .store_state(&PrivacyStateFile {
                mode: Some(StoredPrivacyMode::Full),
                consent: Some(previous_consent("previous-generation")),
                viewed_policy_updated_at: Some("2026-07-01T00:00:00Z".to_string()),
            })
            .await
            .unwrap();

        let status = service.initialize("1.2.3").await.unwrap();
        assert_eq!(
            status.lifecycle_state,
            PrivacyLifecycleState::ChoiceRequired
        );
        assert_eq!(
            status.effective_mode,
            PrivacyEffectiveMode::PrivacyNotAccepted
        );
        let _ = tokio::fs::remove_dir_all(directory).await;
    }

    #[tokio::test]
    async fn new_state_persists_timestamps_without_policy_versions() {
        let directory = temporary_directory("timestamp-state");
        let service = PrivacyService::new(directory.clone(), "zh-CN");
        accept(&service).await;

        let body = tokio::fs::read_to_string(directory.join("privacy-state.json"))
            .await
            .unwrap();
        assert!(body.contains("acceptedPolicyUpdatedAt"));
        assert!(body.contains("viewedPolicyUpdatedAt"));
        assert!(!body.contains("PolicyVersion"));
        let _ = tokio::fs::remove_dir_all(directory).await;
    }

    #[tokio::test]
    async fn persistence_failure_uses_a_stable_storage_error() {
        let directory = temporary_directory("storage-failure");
        tokio::fs::create_dir_all(&directory).await.unwrap();
        let blocking_file = directory.join("not-a-directory");
        tokio::fs::write(&blocking_file, b"blocked").await.unwrap();
        let service = PrivacyService::new(blocking_file.join("privacy"), "en-US");

        let error = service
            .enter_not_accepted("en-US", "1.2.3")
            .await
            .unwrap_err();
        assert_eq!(error.code, "PRIVACY_STORAGE_ERROR");
        let _ = tokio::fs::remove_dir_all(directory).await;
    }

    #[test]
    fn collection_policy_fails_closed() {
        let policy = PrivacyCollectionPolicy::new(false);
        policy.set_fail_full_application(true);
        assert!(policy.apply(PrivacyEffectiveMode::Full).is_err());
        assert!(!policy.collection_allowed());
        policy
            .apply(PrivacyEffectiveMode::PrivacyNotAccepted)
            .unwrap();
        assert!(!policy.collection_allowed());
    }
}
